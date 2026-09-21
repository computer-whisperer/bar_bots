//! `jev SCENARIO.json [...] [--repeat N]`: sends scenario files (`experiments/jev/`: `{about, opus_decided, state,
//! questions}`; `about` and `opus_decided` are ours and not sent) and prints the answers, latency and usage. The
//! Python probe of 2026-09-19 (`experiments/jev/probe.py`) did the same; this is the crate's smoke test.

use std::time::Duration;

use serde_json::Value;

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let repeat = match args.iter().position(|a| a == "--repeat") {
        Some(i) => {
            args.remove(i);
            args.remove(i).parse::<usize>().unwrap_or(1)
        }
        None => 1,
    };
    if args.is_empty() {
        eprintln!("usage: jev SCENARIO.json [...] [--repeat N]");
        std::process::exit(2);
    }
    let client = match jev::Client::from_env() {
        Ok(client) => client,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(2);
        }
    };
    for path in &args {
        let text = std::fs::read_to_string(path).unwrap_or_else(|e| {
            eprintln!("{path}: {e}");
            std::process::exit(2);
        });
        let mut scenario: Value = serde_json::from_str(&text).unwrap_or_else(|e| {
            eprintln!("{path}: {e}");
            std::process::exit(2);
        });
        println!("== {}: {}", path.rsplit('/').next().unwrap_or(path), scenario["about"].as_str().unwrap_or_default());
        if let Some(decided) = scenario["opus_decided"].as_str() {
            println!("   Opus decided: {decided}");
        }
        let request: jev::Request = match serde_json::from_value(serde_json::json!({ "state": scenario["state"].take(), "questions": scenario["questions"].take() })) {
            Ok(request) => request,
            Err(e) => {
                eprintln!("{path}: questions did not parse: {e}");
                std::process::exit(2);
            }
        };
        let mut latencies: Vec<Duration> = Vec::new();
        for run in 0..repeat {
            match client.ask(&request) {
                Ok(response) => {
                    latencies.push(response.latency);
                    if run == 0 {
                        for (id, answer) in &response.answers {
                            println!("   {id:<28} {}", answer.summary());
                        }
                        println!("   usage: {}  model: {}", response.usage, response.model);
                    }
                }
                Err(e) => {
                    eprintln!("   {e}");
                    std::process::exit(1);
                }
            }
        }
        latencies.sort();
        let ms = |d: &Duration| d.as_secs_f64() * 1000.0;
        println!(
            "   latency: median {:.0} ms over {} call(s), min {:.0}, max {:.0}",
            ms(&latencies[latencies.len() / 2]), latencies.len(), ms(&latencies[0]), ms(latencies.last().unwrap())
        );
    }
}
