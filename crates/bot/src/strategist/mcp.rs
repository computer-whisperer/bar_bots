//! Minimal MCP server over streamable HTTP: the five methods Claude Code was observed to use
//! (`docs/harness/claude-p.md`), answered with plain JSON.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use bot_protocol::Vec3;
use serde_json::{Value, json};
use tiny_http::{Header, Method, Response, Server};

use super::shared::{Focus, Shared, Stance, Timed};
use super::transcript::Transcript;

const DEFAULT_TTL_SECONDS: i64 = 120;
const MAX_TTL_SECONDS: i64 = 600;

pub struct McpServer {
    pub port: u16,
    stop: Arc<AtomicBool>,
}

impl McpServer {
    /// Serves on a free localhost port until dropped.
    pub fn start(shared: Arc<Shared>, transcript: Arc<Transcript>) -> std::io::Result<Self> {
        let server = Server::http("127.0.0.1:0").map_err(std::io::Error::other)?;
        let port = server.server_addr().to_ip().map_or(0, |addr| addr.port());
        let stop = Arc::new(AtomicBool::new(false));
        let stopping = stop.clone();
        std::thread::spawn(move || {
            while !stopping.load(Ordering::Relaxed) {
                let Ok(Some(mut request)) = server.recv_timeout(std::time::Duration::from_millis(250)) else { continue };
                if *request.method() != Method::Post {
                    // No server-initiated stream.
                    let _ = request.respond(Response::empty(405));
                    continue;
                }
                let mut body = String::new();
                let _ = request.as_reader().read_to_string(&mut body);
                let reply = serde_json::from_str::<Value>(&body).ok().and_then(|call| handle(&call, &shared, &transcript));
                let _ = match reply {
                    Some(reply) => request.respond(
                        Response::from_string(reply.to_string())
                            .with_header(Header::from_bytes("Content-Type", "application/json").unwrap()),
                    ),
                    // Notifications carry no id and get no body.
                    None => request.respond(Response::empty(202)),
                };
            }
        });
        Ok(McpServer { port, stop })
    }
}

impl Drop for McpServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

fn handle(call: &Value, shared: &Shared, transcript: &Transcript) -> Option<Value> {
    let id = call.get("id")?.clone();
    let method = call["method"].as_str().unwrap_or_default();
    let result = match method {
        "initialize" => Ok(json!({
            "protocolVersion": call["params"]["protocolVersion"].as_str().unwrap_or("2025-06-18"),
            "capabilities": { "tools": {} },
            "serverInfo": { "name": "within-reason", "version": env!("CARGO_PKG_VERSION") },
        })),
        "tools/list" => Ok(json!({ "tools": tool_list() })),
        "tools/call" => {
            let name = call["params"]["name"].as_str().unwrap_or_default();
            let arguments = &call["params"]["arguments"];
            let outcome = call_tool(name, arguments, shared);
            transcript.record(json!({ "kind": "tool_call", "tool": name, "arguments": arguments,
                "result": outcome.as_ref().map_or_else(|e| e.clone(), |text| summary(text)) }));
            Ok(match outcome {
                Ok(text) => json!({ "content": [{ "type": "text", "text": text }] }),
                Err(problem) => json!({ "content": [{ "type": "text", "text": problem }], "isError": true }),
            })
        }
        _ => Err(json!({ "code": -32601, "message": format!("method not found: {method}") })),
    };
    Some(match result {
        Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
        Err(error) => json!({ "jsonrpc": "2.0", "id": id, "error": error }),
    })
}

/// Briefings are long; the transcript keeps what was asked and that it was answered.
fn summary(text: &str) -> String {
    if text.len() > 200 { format!("{} bytes", text.len()) } else { text.to_string() }
}

fn tool_list() -> Value {
    json!([
        { "name": "overview",
          "description": "Current state of the game as the bot sees it: time, economy, unit counts, our army groups, enemies in sight, remembered enemy buildings, recent events, and the directives in force.",
          "inputSchema": { "type": "object", "properties": {}, "additionalProperties": false } },
        { "name": "map",
          "description": "Static map facts: size, grid naming, our start, the presumed enemy start, and every metal spot with its grid cell.",
          "inputSchema": { "type": "object", "properties": {}, "additionalProperties": false } },
        { "name": "set_directives",
          "description": "Set standing orders for the bot's heuristics. Give only the fields you want to change. Every directive expires after ttl_seconds of game time (default 120, max 600) and the heuristic's own default takes over, so renew what should persist. Pass null for a field to clear it now.",
          "inputSchema": { "type": "object", "additionalProperties": false, "properties": {
              "army_stance": { "enum": ["defend", "gather", "attack", null],
                  "description": "defend: army stays home. gather: keep massing, launch nothing. attack: commit the home group now regardless of size." },
              "attack_target": { "type": ["object", "null"], "properties": { "x": { "type": "number" }, "z": { "type": "number" } },
                  "required": ["x", "z"], "description": "Where attackers go, in map coordinates." },
              "wave_size": { "type": ["integer", "null"], "minimum": 1, "maximum": 200,
                  "description": "Home-group size at which the bot launches a wave on its own." },
              "army_station": { "type": ["object", "null"], "properties": { "x": { "type": "number" }, "z": { "type": "number" } },
                  "required": ["x", "z"], "description": "Where the home group waits and gathers. By default it stands just ahead of our most exposed extractors; it still turns on raiders near any of our extractors and on intruders at the base." },
              "min_constructors": { "type": ["integer", "null"], "minimum": 1, "maximum": 10,
                  "description": "Factories keep at least this many constructors alive (the bot's own floor is 3)." },
              "min_converters": { "type": ["integer", "null"], "minimum": 0, "maximum": 40,
                  "description": "Constructors build energy-to-metal converters up to this count before expanding further, energy permitting." },
              "economy_focus": { "enum": ["expand", "energy", "production", "defence", null],
                  "description": "What constructors prefer once the opening is done." },
              "ttl_seconds": { "type": "integer", "minimum": 10, "maximum": MAX_TTL_SECONDS } } } },
        { "name": "note",
          "description": "Record your reasoning in a sentence or two. Kept with the game time for post-game analysis; it changes nothing in the game.",
          "inputSchema": { "type": "object", "properties": { "text": { "type": "string" } }, "required": ["text"], "additionalProperties": false } },
    ])
}

fn call_tool(name: &str, arguments: &Value, shared: &Shared) -> Result<String, String> {
    match name {
        "overview" => serde_json::to_string(&*shared.briefing.lock().unwrap()).map_err(|e| e.to_string()),
        "map" => Ok(shared.map.lock().unwrap().to_string()),
        "note" => Ok("noted".into()),
        "set_directives" => set_directives(arguments, shared),
        _ => Err(format!("unknown tool {name}")),
    }
}

fn set_directives(arguments: &Value, shared: &Shared) -> Result<String, String> {
    let frame = shared.briefing.lock().unwrap().frame;
    let ttl = arguments["ttl_seconds"].as_i64().unwrap_or(DEFAULT_TTL_SECONDS).clamp(10, MAX_TTL_SECONDS);
    let expires_frame = frame + ttl as i32 * 30;
    let fields = arguments.as_object().ok_or("arguments must be an object")?;
    let mut directives = shared.directives.lock().unwrap();
    for (field, value) in fields {
        match field.as_str() {
            "ttl_seconds" => {}
            "army_stance" => {
                directives.army_stance = parse::<Stance>(value)?.map(|value| Timed { value, expires_frame });
            }
            "economy_focus" => {
                directives.economy_focus = parse::<Focus>(value)?.map(|value| Timed { value, expires_frame });
            }
            "wave_size" => {
                directives.wave_size = parse::<usize>(value)?.map(|value| Timed { value, expires_frame });
            }
            "min_constructors" => {
                directives.min_constructors = parse::<usize>(value)?.map(|value| Timed { value, expires_frame });
            }
            "min_converters" => {
                directives.min_converters = parse::<usize>(value)?.map(|value| Timed { value, expires_frame });
            }
            "attack_target" => directives.attack_target = position(value, field)?.map(|value| Timed { value, expires_frame }),
            "army_station" => directives.army_station = position(value, field)?.map(|value| Timed { value, expires_frame }),
            other => return Err(format!("unknown directive {other}")),
        }
    }
    Ok(format!("in force: {}", directives.describe(frame).join("; ")))
}

fn position(value: &Value, field: &str) -> Result<Option<Vec3>, String> {
    if value.is_null() {
        return Ok(None);
    }
    let coordinate = |axis: &str| value[axis].as_f64().ok_or(format!("{field} needs a number {axis}"));
    Ok(Some(Vec3 { x: coordinate("x")? as f32, y: 0.0, z: coordinate("z")? as f32 }))
}

fn parse<T: serde::de::DeserializeOwned>(value: &Value) -> Result<Option<T>, String> {
    if value.is_null() {
        return Ok(None);
    }
    serde_json::from_value(value.clone()).map(Some).map_err(|e| e.to_string())
}
