//! TypeSafe's Jev (`docs/harness/jev.md`): one request carries a `state` and a map of typed questions; every question
//! is answered in parallel over the same state, with probabilities. No text comes back. The API key is read from
//! `TYPESAFE_API_KEY`, else from `~/.config/within-reason/jev.env`, and is never written anywhere by this crate:
//! the repository is public.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::Value;

const URL: &str = "https://api.typesafe.ai/v1/systemone";
const KEY_FILE: &str = ".config/within-reason/jev.env";
const KEY_VAR: &str = "TYPESAFE_API_KEY";
/// The alias resolves to the current release; the response names the versioned model that answered.
const DEFAULT_MODEL: &str = "jev-latest";
const MODEL_VAR: &str = "TYPESAFE_DEFAULT_MODEL";
const TIMEOUT: Duration = Duration::from_secs(20);
/// A 429 or a 5xx is retried this many times, waiting `retry-after` (at most `RETRY_WAIT_MAX`) each time.
const RETRIES: u32 = 2;
const RETRY_WAIT_MAX: Duration = Duration::from_secs(5);

/// A typed question, in the API's own shape (`type`, `instructions`, `criteria`).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Question {
    /// One of the options in `criteria` (at most 255): option name to its description (`Value::Null` for none).
    Choice { instructions: Value, criteria: BTreeMap<String, Value> },
    /// A yes/no; `criteria` may say what `true` and `false` mean.
    Noul {
        instructions: Value,
        #[serde(skip_serializing_if = "Option::is_none")]
        criteria: Option<Value>,
    },
    /// A position on ordered levels (2 to 10), each described as a situation, low to high.
    Score { instructions: Value, criteria: Vec<Value> },
}

impl Question {
    pub fn choice(instructions: impl Into<Value>, options: impl IntoIterator<Item = (impl Into<String>, impl Into<Value>)>) -> Question {
        Question::Choice { instructions: instructions.into(), criteria: options.into_iter().map(|(k, v)| (k.into(), v.into())).collect() }
    }

    pub fn noul(instructions: impl Into<Value>) -> Question {
        Question::Noul { instructions: instructions.into(), criteria: None }
    }

    pub fn score(instructions: impl Into<Value>, levels: impl IntoIterator<Item = impl Into<Value>>) -> Question {
        Question::Score { instructions: instructions.into(), criteria: levels.into_iter().map(Into::into).collect() }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Request {
    pub state: Value,
    pub questions: BTreeMap<String, Question>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Answer {
    Choice { choice: String, probabilities: BTreeMap<String, f64>, confidence: f64 },
    /// `noul` is the probability of yes; 0.5 is unsure, not "medium".
    Noul { noul: f64 },
    Score {
        score: f64,
        probabilities: BTreeMap<String, f64>,
        confidence: f64,
        #[serde(default)]
        legend: BTreeMap<String, Value>,
    },
}

impl Answer {
    /// The probability the answer puts on `option` (a Choice option, a Score level number, or `true`/`false` for a
    /// Noul).
    pub fn probability_of(&self, option: &str) -> f64 {
        match self {
            Answer::Choice { probabilities, .. } | Answer::Score { probabilities, .. } => probabilities.get(option).copied().unwrap_or(0.0),
            Answer::Noul { noul } => match option {
                "true" | "yes" => *noul,
                "false" | "no" => 1.0 - noul,
                _ => 0.0,
            },
        }
    }

    /// One line for a log: the pick, its confidence and the leading probabilities.
    pub fn summary(&self) -> String {
        match self {
            Answer::Noul { noul } => format!("yes {noul:.2}"),
            Answer::Choice { choice, probabilities, confidence } => format!("{choice} (confidence {confidence:.2}; {})", leading(probabilities)),
            Answer::Score { score, probabilities, confidence, .. } => format!("{score:.2} (confidence {confidence:.2}; {})", leading(probabilities)),
        }
    }
}

fn leading(probabilities: &BTreeMap<String, f64>) -> String {
    let mut ranked: Vec<(&String, &f64)> = probabilities.iter().collect();
    ranked.sort_by(|a, b| b.1.total_cmp(a.1));
    ranked.iter().take(4).map(|(name, p)| format!("{name}={p:.2}")).collect::<Vec<_>>().join(" ")
}

#[derive(Clone, Debug)]
pub struct Response {
    pub answers: BTreeMap<String, Answer>,
    /// The versioned model that answered (`jev-1.13.0`), whatever alias was asked.
    pub model: String,
    pub usage: Value,
    pub latency: Duration,
    /// Retries this call needed (429 or 5xx).
    pub retries: u32,
}

#[derive(Debug)]
pub enum Error {
    /// No key in the environment or the key file.
    NoKey(String),
    /// The connection or the transfer failed.
    Transport(String),
    /// Rate limited and out of retries.
    RateLimited { retry_after: Duration },
    /// The service answered with an error status.
    Status { status: u16, body: String },
    /// The service's answer did not parse.
    Malformed(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::NoKey(where_) => write!(f, "no Jev API key: put {KEY_VAR}=... in {where_}"),
            Error::Transport(e) => write!(f, "Jev unreachable: {e}"),
            Error::RateLimited { retry_after } => write!(f, "Jev rate limited (retry after {:.1} s)", retry_after.as_secs_f32()),
            Error::Status { status, body } => write!(f, "Jev answered HTTP {status}: {}", body.chars().take(300).collect::<String>()),
            Error::Malformed(e) => write!(f, "Jev's answer did not parse: {e}"),
        }
    }
}

impl std::error::Error for Error {}

pub struct Client {
    key: String,
    model: String,
    agent: ureq::Agent,
}

impl std::fmt::Debug for Client {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Client").field("model", &self.model).finish_non_exhaustive()
    }
}

/// The key file's path under the home directory.
pub fn key_file() -> PathBuf {
    PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(KEY_FILE)
}

fn key_from_env_or_file() -> Result<String, Error> {
    if let Some(key) = std::env::var(KEY_VAR).ok().filter(|k| !k.trim().is_empty()) {
        return Ok(key.trim().to_string());
    }
    let path = key_file();
    let text = std::fs::read_to_string(&path).map_err(|_| Error::NoKey(path.display().to_string()))?;
    text.lines()
        .filter_map(|line| line.trim().strip_prefix(KEY_VAR).and_then(|rest| rest.trim_start().strip_prefix('=')))
        .map(|value| value.trim().trim_matches('"').trim_matches('\'').to_string())
        .find(|value| !value.is_empty())
        .ok_or_else(|| Error::NoKey(path.display().to_string()))
}

impl Client {
    /// The key from `TYPESAFE_API_KEY` or the key file; the model from `TYPESAFE_DEFAULT_MODEL` or the alias.
    pub fn from_env() -> Result<Client, Error> {
        let key = key_from_env_or_file()?;
        let model = std::env::var(MODEL_VAR).ok().filter(|m| !m.is_empty()).unwrap_or_else(|| DEFAULT_MODEL.into());
        Ok(Client::new(key, model))
    }

    pub fn new(key: String, model: String) -> Client {
        let config = ureq::Agent::config_builder().timeout_global(Some(TIMEOUT)).http_status_as_error(false).user_agent("within-reason").build();
        Client { key, model, agent: ureq::Agent::new_with_config(config) }
    }

    pub fn model(&self) -> &str {
        &self.model
    }

    /// One call: every question answered over the state. Retries a 429 or a 5xx, waiting what the service asks.
    pub fn ask(&self, request: &Request) -> Result<Response, Error> {
        let body = serde_json::json!({ "model": self.model, "state": request.state, "questions": request.questions });
        let started = Instant::now();
        let mut retries = 0;
        loop {
            match self.send(&body) {
                Ok(mut response) => {
                    response.latency = started.elapsed();
                    response.retries = retries;
                    return Ok(response);
                }
                Err(Retry::After(wait)) if retries < RETRIES => {
                    retries += 1;
                    std::thread::sleep(wait.min(RETRY_WAIT_MAX));
                }
                Err(Retry::After(wait)) => return Err(Error::RateLimited { retry_after: wait }),
                Err(Retry::Never(e)) => return Err(e),
            }
        }
    }

    fn send(&self, body: &Value) -> Result<Response, Retry> {
        let mut response = self
            .agent
            .post(URL)
            .header("Authorization", &format!("Bearer {}", self.key))
            .header("Content-Type", "application/json")
            .send_json(body)
            .map_err(|e| Retry::Never(Error::Transport(e.to_string())))?;
        let status = response.status().as_u16();
        if status == 429 || status >= 500 {
            let wait = response
                .headers()
                .get("retry-after")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.trim().parse::<f64>().ok())
                .map_or(Duration::from_secs(1), Duration::from_secs_f64);
            return Err(Retry::After(wait));
        }
        let text = response.body_mut().read_to_string().map_err(|e| Retry::Never(Error::Transport(e.to_string())))?;
        if !(200..300).contains(&status) {
            return Err(Retry::Never(Error::Status { status, body: text }));
        }
        let mut parsed: Value = serde_json::from_str(&text).map_err(|e| Retry::Never(Error::Malformed(e.to_string())))?;
        let answers = serde_json::from_value(parsed["answers"].take()).map_err(|e| Retry::Never(Error::Malformed(e.to_string())))?;
        Ok(Response {
            answers,
            model: parsed["model"].as_str().unwrap_or_default().to_string(),
            usage: parsed["usage"].take(),
            latency: Duration::ZERO,
            retries: 0,
        })
    }
}

enum Retry {
    After(Duration),
    Never(Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn questions_serialise_in_the_apis_shape() {
        let q = Question::choice("Which?", [("a", "the first"), ("b", "the second")]);
        let json = serde_json::to_value(&q).unwrap();
        assert_eq!(json, serde_json::json!({ "type": "choice", "instructions": "Which?", "criteria": { "a": "the first", "b": "the second" } }));
        let n = Question::noul("Is it?");
        assert_eq!(serde_json::to_value(&n).unwrap(), serde_json::json!({ "type": "noul", "instructions": "Is it?" }));
        let s = Question::score("How much?", ["none", "some", "all"]);
        assert_eq!(serde_json::to_value(&s).unwrap(), serde_json::json!({ "type": "score", "instructions": "How much?", "criteria": ["none", "some", "all"] }));
    }

    #[test]
    fn answers_parse_from_the_apis_shape() {
        let text = r#"{"a": {"type": "choice", "choice": "x", "probabilities": {"x": 0.7, "y": 0.3}, "confidence": 0.4},
                       "b": {"type": "noul", "noul": 0.82},
                       "c": {"type": "score", "score": 1.4, "probabilities": {"0": 0.0, "1": 0.6, "2": 0.4}, "confidence": 0.3, "legend": {"0": "none"}}}"#;
        let answers: BTreeMap<String, Answer> = serde_json::from_str(text).unwrap();
        assert_eq!(answers["a"].probability_of("x"), 0.7);
        assert_eq!(answers["b"].probability_of("no"), 1.0 - 0.82);
        assert!(matches!(answers["c"], Answer::Score { score, .. } if score == 1.4));
    }
}
