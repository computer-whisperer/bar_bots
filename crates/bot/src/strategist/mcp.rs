//! Minimal MCP server over streamable HTTP: the five methods Claude Code was observed to use
//! (`docs/harness/claude-p.md`), answered with plain JSON.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use bot_protocol::Vec3;
use serde_json::{Value, json};
use tiny_http::{Header, Method, Response, Server};

use super::Mode;
use super::shared::{Focus, OrderKind, OutpostTurrets, Post, Shared, Stance, Timed};
use super::transcript::Transcript;

const DEFAULT_TTL_SECONDS: i64 = 120;
const MAX_TTL_SECONDS: i64 = 600;
/// The player's packet at most: Jev reads it every second beside a picture of about 4k tokens, within 64k.
const INSTRUCTIONS_LIMIT: usize = 8000;

pub struct McpServer {
    pub port: u16,
    stop: Arc<AtomicBool>,
}

impl McpServer {
    /// Serves on a free localhost port until dropped.
    pub fn start(shared: Arc<Shared>, transcript: Arc<Transcript>, mode: Mode) -> std::io::Result<Self> {
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
                let reply = serde_json::from_str::<Value>(&body).ok().and_then(|call| handle(&call, &shared, &transcript, mode));
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

fn handle(call: &Value, shared: &Shared, transcript: &Transcript, mode: Mode) -> Option<Value> {
    let id = call.get("id")?.clone();
    let method = call["method"].as_str().unwrap_or_default();
    let result = match method {
        "initialize" => Ok(json!({
            "protocolVersion": call["params"]["protocolVersion"].as_str().unwrap_or("2025-06-18"),
            "capabilities": { "tools": {} },
            "serverInfo": { "name": "within-reason", "version": env!("CARGO_PKG_VERSION") },
        })),
        "tools/list" => Ok(json!({ "tools": tool_list(mode) })),
        "tools/call" => {
            let name = call["params"]["name"].as_str().unwrap_or_default();
            let arguments = &call["params"]["arguments"];
            let outcome = if shared.turn_over.load(Ordering::Relaxed) {
                Err("Your turn is over and the game is running. Stop now: call nothing more and write nothing more. You will be woken with a new report.".to_string())
            } else if name == "orders" {
                orders(arguments, shared, mode)
            } else {
                call_tool(name, arguments, shared, mode)
            };
            // Full results, briefings included: they are what the strategist decided on, and the
            // labelled state for evaluating faster models against its decisions.
            let recorded = outcome.as_ref().map_or_else(
                |problem| Value::String(problem.clone()),
                |text| serde_json::from_str(text).unwrap_or_else(|_| Value::String(text.clone())),
            );
            transcript.record(json!({ "kind": "tool_call", "tool": name, "arguments": arguments, "result": recorded }));
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

/// The tools a mode offers. The commander's levers steer the heuristics; the player has none of them: its lever is
/// `instruct`, and its `situation` is the picture its hands read.
fn tool_list(mode: Mode) -> Value {
    let overview = json!({ "name": "overview",
        "description": "Current state of the game as the bot sees it: time, economy, unit counts, our army groups, enemies in sight, remembered enemy buildings, recent events, and the directives in force.",
        "inputSchema": { "type": "object", "properties": {}, "additionalProperties": false } });
    let map = json!({ "name": "map",
        "description": "Static map facts: size, grid naming, our start, the presumed enemy start, and every metal spot with its grid cell.",
        "inputSchema": { "type": "object", "properties": {}, "additionalProperties": false } });
    let note = json!({ "name": "note",
        "description": "Record your reasoning in a sentence or two. Kept with the game time for post-game analysis; it changes nothing in the game.",
        "inputSchema": { "type": "object", "properties": { "text": { "type": "string" } }, "required": ["text"], "additionalProperties": false } });
    let wait = |engaged: &str, pool: &str| json!({ "name": "wait",
        "description": "Ends your turn: the game resumes the moment this is called, so call it last and write nothing after it. Sets when you are next woken; the settings hold until you change them. The game is paused during your turn and runs fast between turns, so a long quiet wait costs nothing and a raid still wakes you at once. You are always woken for a base attack, the commander under fire, or a wiped-out wave.",
        "inputSchema": { "type": "object", "additionalProperties": false, "properties": {
            "max_seconds": { "type": "integer", "minimum": 5, "maximum": 180, "description": "Game seconds after which you are woken whatever happens (default 30)." },
            "enemy_near_extractor": { "type": "boolean", "description": "Enemies appear within 600 of an extractor that had none near." },
            "squad_engaged": { "type": "boolean", "description": engaged },
            "extractor_lost": { "type": "boolean" },
            "pool_reaches": { "type": "object", "additionalProperties": { "type": "integer", "minimum": 1 }, "description": pool } } } });
    let orders = |tools: &[&str], what: &str| json!({ "name": "orders",
        "description": format!("Your whole turn in one call, and it ENDS the turn: {what}, carried out in the order listed, then the game resumes. Each entry names one of the other tools and its arguments, exactly as you would call it alone. Include a `wait` entry to change when you are next woken; without one the wake settings in force stand. Call nothing and write nothing after it."),
        "inputSchema": { "type": "object", "additionalProperties": false, "required": ["calls"], "properties": {
            "calls": { "type": "array", "minItems": 1, "items": { "type": "object", "additionalProperties": false, "required": ["tool"], "properties": {
                "tool": { "type": "string", "enum": tools },
                "arguments": { "type": "object", "description": "That tool's arguments, e.g. {\"text\": \"...\"} for note." } } } } } } });
    match mode {
        Mode::Player => json!([
            overview, map,
            { "name": "situation",
              "description": "The picture your hands read this second, exactly as Jev sees it (without your instructions and the standing rules): economy, ours, enemy, places by name, every actor with what it is doing, recent events. You are sent a summary of it at the start of every turn; call this to read the whole picture, or a place's entry by name.",
              "inputSchema": { "type": "object", "properties": {}, "additionalProperties": false } },
            { "name": "instruct",
              "description": format!("Your standing instructions to your hands: the whole packet, replacing the last one. Jev reads it every second beside the picture and picks each actor's next action from a menu, so write it as standing orders in plain words: the build order per builder as a sequence, what the lab makes and when that changes, where each group stands, when it engages, scouts and attacks, what to do about raids. Name places as the picture does (home, enemy_base, spot_N, passage_N) and groups as group_A, group_B. No arithmetic for the hands to do: say \"when we have about ten soldiers\", not a formula. At most {INSTRUCTIONS_LIMIT} characters."),
              "inputSchema": { "type": "object", "additionalProperties": false, "required": ["text"], "properties": { "text": { "type": "string" } } } },
            orders(&["instruct", "note", "wait"], "your instructions, a note and when to be woken"),
            wait("A group of ours starts fighting an enemy party.", "Woken when this many soldiers of each named unit type are alive, e.g. {\"armham\": 6}. {} clears it."),
            note,
        ]),
        Mode::Strategist | Mode::Commander => json!([
            overview, map,
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
                  "max_converters": { "type": ["integer", "null"], "minimum": 0, "maximum": 40,
                      "description": "No more converters than this, whatever energy is banked (each takes 70 energy a second to run and costs 1150 to build; the bot's own rule builds one only when energy income exceeds usage by that much and stops at 40). 0 stops them." },
                  "base_turrets": { "type": ["integer", "null"], "minimum": 0, "maximum": 6,
                      "description": "How many light turrets (85 metal each) the bot puts up on its own 450 forward of home: unset it builds 2 after the lab and up to 6 when constructors are idle; 0 stops them. Turrets are for Ticks: one at an extractor repels them where no army stands; Hammers and Pawns are the army's to counter." },
                  "outpost_turrets": { "type": ["string", "array", "null"], "items": { "type": "integer", "minimum": 0 },
                      "description": "Which of our extractors get a light turret of the bot's own accord: \"all\" (unset: one at every extractor more than 500 from home), \"none\" (only where request_turret asks), or a list of metal spot numbers n from the map (a turret at each of those extractors as they stand, none elsewhere). Give the spots your squads do not cover." },
                  "expansion_radius": { "type": ["integer", "null"], "minimum": 500, "maximum": 20000,
                      "description": "Constructors build extractors only on metal spots within this walking distance of home (see walk_from_home in the map). Use it to stop expansion into places you cannot defend." },
                  "tier2": { "type": ["boolean", "null"],
                      "description": "The advanced bot lab (2600 metal, then advanced constructors that upgrade our extractors to four times the yield, and tier-2 units). Unset, the bot starts it when metal income reaches 22 and energy income 450 with nothing dying at home. true starts it now; false holds it back." },
                  "resurrect": { "type": ["boolean", "null"],
                      "description": "Resurrection bots (the bot builds one per 600 metal of wrecks lying on held ground, at most 6) raise wrecked soldiers worth 100 metal or more when stored energy is above half and 100 metal is banked, and take everything else apart for its metal. false: they raise nothing and reclaim everything." },
                  "commander_station": { "type": ["object", "null"], "properties": { "x": { "type": "number" }, "z": { "type": "number" } },
                      "required": ["x", "z"], "description": "The commander walks here and builds only near here (it is a strong builder and fighter, and the game is lost if it dies). Without this it builds within 24 seconds of its own walking from home." },
                  "economy_focus": { "enum": ["expand", "energy", "production", "defence", null],
                      "description": "What constructors prefer once the opening is done." },
                  "pressure": { "type": ["boolean", "null"],
                      "description": "The early raider pressure: from the first Pawn the bot sends raiders at the opponent's extractors and base, priced by its fight simulator, retreating from the commander and turrets and harassing elsewhere. false keeps them home (they join the home group); unset or true lets it run." },
                  "scout_at": { "type": ["object", "null"], "properties": { "x": { "type": "number" }, "z": { "type": "number" } },
                      "required": ["x", "z"], "description": "Send the next scout (one raider, a route of metal spots) round this point first. The bot scouts by itself: the enemy base's spots when stale, the rest of its box, then the map." },
                  "ttl_seconds": { "type": "integer", "minimum": 10, "maximum": MAX_TTL_SECONDS } } } },
            { "name": "situation",
              "description": "The field picture: unassigned soldiers by type, your squads (composition, health, where, post, whether engaged), every extractor with enemies near it and whether a turret covers it, turrets, what the factories can build with metal cost, the production mix in force. You are sent this at the start of every turn; call it only to look again mid-turn.",
              "inputSchema": { "type": "object", "properties": {}, "additionalProperties": false } },
            { "name": "squad",
              "description": "Create or change a squad. Soldiers in a squad are yours; all others follow the bot's heuristics (home group, attack waves). take draws that many more of each unit type from the unassigned soldiers, nearest to `near` (else to the post); units not yet built are added as they appear. post is a standing defensive position: the squad stands there, engages any enemy that comes within radius of it, and returns. order is a one-off move or fight (attack-move) to a position and cancels the post. release hands the squad back to the heuristics.",
              "inputSchema": { "type": "object", "additionalProperties": false, "required": ["name"], "properties": {
                  "name": { "type": "string" },
                  "take": { "type": "object", "additionalProperties": { "type": "integer", "minimum": 0, "maximum": 50 },
                      "description": "Unit name to how many more to draw, e.g. {\"armpw\": 3, \"armham\": 2}." },
                  "near": { "type": "object", "properties": { "x": { "type": "number" }, "z": { "type": "number" } }, "required": ["x", "z"] },
                  "post": { "type": "object", "properties": { "x": { "type": "number" }, "z": { "type": "number" }, "radius": { "type": "number", "minimum": 100, "maximum": 1500 } },
                      "required": ["x", "z", "radius"] },
                  "order": { "type": "object", "properties": { "kind": { "enum": ["move", "fight"] }, "x": { "type": "number" }, "z": { "type": "number" } },
                      "required": ["kind", "x", "z"] },
                  "release": { "type": "boolean" } } } },
            { "name": "set_production",
              "description": "The unit mix the factories build, as unit name to weight (names from `buildable` in the situation). Factories build whichever type is furthest below its share of what is alive. The bot keeps its own floor of constructors. An empty object returns production to the bot's default batch.",
              "inputSchema": { "type": "object", "additionalProperties": false, "required": ["weights"], "properties": {
                  "weights": { "type": "object", "additionalProperties": { "type": "integer", "minimum": 0, "maximum": 100 } } } } },
            { "name": "request_turret",
              "description": "Ask for a light defence turret at a position; the next free constructor builds it near there.",
              "inputSchema": { "type": "object", "additionalProperties": false, "required": ["x", "z"],
                  "properties": { "x": { "type": "number" }, "z": { "type": "number" } } } },
            { "name": "expansion",
              "description": "Which metal spots the constructors take. Spots are numbered as in the map's metal_spots list (`n`). `take_first`: spots taken before any other, in this order, wherever they lie and even if they were raided before (this is also how you order a lost extractor rebuilt, or leave it lost by not listing it). `leave_alone`: spots never taken, e.g. ones you cannot hold. Other spots follow the bot's rule (nearest first, within expansion_radius, skipping recently raided ones without cover). Each call replaces the whole plan; {} clears it.",
              "inputSchema": { "type": "object", "additionalProperties": false, "properties": {
                  "take_first": { "type": "array", "items": { "type": "integer", "minimum": 0 } },
                  "leave_alone": { "type": "array", "items": { "type": "integer", "minimum": 0 } } } } },
            orders(&["squad", "set_directives", "set_production", "request_turret", "expansion", "note", "wait"], "every order you want to give"),
            wait("A posted squad starts fighting.", "Woken when this many of each named unit type stand unassigned, e.g. {\"armham\": 4}. {} clears it."),
            note,
        ]),
    }
}

/// What `orders` may batch in a mode.
fn batchable(mode: Mode) -> &'static [&'static str] {
    match mode {
        Mode::Player => &["instruct", "note", "wait"],
        Mode::Strategist | Mode::Commander => &["squad", "set_directives", "set_production", "request_turret", "expansion", "note", "wait"],
    }
}

/// The `orders` tool: several tool calls in one request. `wait` goes last wherever it was listed, since it ends the turn.
fn orders(arguments: &Value, shared: &Shared, mode: Mode) -> Result<String, String> {
    let calls = arguments["calls"].as_array().ok_or("calls must be a list")?;
    let (mut waits, others): (Vec<&Value>, Vec<&Value>) = calls.iter().partition(|c| c["tool"] == "wait");
    // `orders` is the whole turn: with no `wait` listed the wake settings stand and the turn ends all the same. (In
    // commander game 11 half the turns were `orders` and then a separate `wait`: a second request, 2 s against 5.)
    let keep = json!({ "tool": "wait" });
    if waits.is_empty() {
        waits.push(&keep);
    }
    let empty = json!({});
    let mut lines = Vec::new();
    for call in others.into_iter().chain(waits) {
        // As the model writes them: sometimes with the server's prefix on the name, and often with a small tool's
        // arguments beside `tool` instead of under `arguments` (40 of 46 notes in commander game 11, all recorded empty,
        // so the sessions that took over mid-game inherited blank notes).
        let tool = call["tool"].as_str().unwrap_or_default();
        let tool = tool.rsplit("__").next().unwrap_or(tool);
        if !batchable(mode).contains(&tool) {
            lines.push(format!("{tool}: not a tool that can be batched"));
            continue;
        }
        let mut beside = call.clone();
        if let Some(fields) = beside.as_object_mut() {
            fields.remove("tool");
            fields.remove("arguments");
        }
        let arguments = call.get("arguments").filter(|a| a.is_object()).unwrap_or(if beside.as_object().is_some_and(|f| !f.is_empty()) { &beside } else { &empty });
        match call_tool(tool, arguments, shared, mode) {
            Ok(text) => lines.push(format!("{tool}: {text}")),
            Err(problem) => lines.push(format!("{tool}: REFUSED: {problem}")),
        }
    }
    Ok(lines.join("\n"))
}

fn call_tool(name: &str, arguments: &Value, shared: &Shared, mode: Mode) -> Result<String, String> {
    if !tool_list(mode).as_array().is_some_and(|tools| tools.iter().any(|t| t["name"] == name)) {
        return Err(format!("{name} is not a tool in this mode"));
    }
    match name {
        "overview" => serde_json::to_string(&shared.briefing()).map_err(|e| e.to_string()),
        "map" => {
            let mut map = shared.map.lock().unwrap().clone();
            map["ground"] = serde_json::json!({
                "legend": "whose ground is whose right now, one character per 256 elmos, north up: + held by us, ? contested, - theirs",
                "rows": *shared.ground_sketch.lock().unwrap(),
            });
            Ok(map.to_string())
        }
        "note" => {
            let time = shared.briefing().game_time;
            let text = arguments["text"].as_str().filter(|t| !t.trim().is_empty()).ok_or("a note needs its words under \"text\"")?;
            shared.notes.lock().unwrap().push(format!("[{time}] {text}"));
            Ok("noted".into())
        }
        "wait" => {
            let mut wake = shared.wake.lock().unwrap();
            if let Some(seconds) = arguments["max_seconds"].as_u64() {
                wake.max_seconds = seconds.clamp(5, 180) as u32;
            }
            let flag = |field: &str, slot: &mut bool| {
                if let Some(on) = arguments[field].as_bool() {
                    *slot = on;
                }
            };
            flag("enemy_near_extractor", &mut wake.enemy_near_extractor);
            flag("squad_engaged", &mut wake.squad_engaged);
            flag("extractor_lost", &mut wake.extractor_lost);
            if let Some(pool) = arguments["pool_reaches"].as_object() {
                wake.pool_reaches = pool.iter().map(|(name, n)| (name.clone(), n.as_u64().unwrap_or(1) as usize)).collect();
            }
            let reply = format!("Turn over: the game is running and you will be woken with a new report. Stop now, call nothing more and write nothing more. Wake conditions in force until you change them: {}", serde_json::to_string(&*wake).map_err(|e| e.to_string())?);
            drop(wake);
            // The turn is over here, not when the model has finished its closing sentence: that sentence is one more
            // request to the model, a second or two with the game held (a third of a median turn, commander game 9).
            shared.end_turn_at_wait();
            Ok(reply)
        }
        "situation" if mode == Mode::Player => Ok(shared.hands.lock().unwrap().picture.to_string()),
        "situation" => serde_json::to_string(&shared.field()).map_err(|e| e.to_string()),
        "instruct" => {
            let text = arguments["text"].as_str().map(str::trim).filter(|t| !t.is_empty()).ok_or("instruct needs the packet under \"text\"")?;
            if text.chars().count() > INSTRUCTIONS_LIMIT {
                return Err(format!("the packet is {} characters; at most {INSTRUCTIONS_LIMIT}. Cut it: your hands read it every second", text.chars().count()));
            }
            *shared.instructions.lock().unwrap() = text.to_string();
            Ok(format!("instructions replaced ({} characters); your hands read them from their next look, once your turn ends", text.chars().count()))
        }
        "squad" => squad(arguments, shared),
        "set_production" => {
            let weights = arguments["weights"].as_object().ok_or("weights must be an object")?;
            let known: Vec<String> = shared.field().buildable.iter().map(|(name, _)| name.clone()).collect();
            if let Some(unknown) = weights.keys().find(|name| !known.contains(name)) {
                return Err(format!("{unknown} is not something our factories build; see `buildable`"));
            }
            let mix = weights.iter().map(|(name, w)| (name.clone(), w.as_u64().unwrap_or(0) as u32)).collect();
            shared.field_orders.lock().unwrap().production = mix;
            Ok("production mix set".into())
        }
        "request_turret" => {
            let at = position(arguments, "request_turret")?.ok_or("needs x and z")?;
            // Anywhere we already stand: a turret asked for on ground we hold nothing near is a constructor sent to die.
            let field = shared.field();
            let held = field.extractors.iter().map(|x| &x.at).chain(field.squads.iter().filter_map(|q| q.centre.as_ref())).chain(field.turrets.iter());
            let near = held.map(|p| (p.x as f32 - at.x).hypot(p.z as f32 - at.z)).fold(f32::INFINITY, f32::min);
            if near > 1000.0 {
                return Err(format!("nothing of ours (extractor, turret or squad) within 1000 of there (nearest is {near:.0} away); a constructor would walk there alone. Move a squad there first"));
            }
            let mut orders = shared.field_orders.lock().unwrap();
            orders.turret_requests.push(at);
            Ok("turret requested".into())
        }
        "expansion" => {
            let list = |key: &str| -> Vec<usize> {
                arguments.get(key).and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_u64).map(|n| n as usize).collect()).unwrap_or_default()
            };
            let mut orders = shared.field_orders.lock().unwrap();
            orders.spot_priority = list("take_first");
            orders.spot_avoid = list("leave_alone");
            Ok(format!("expansion plan set: {} to take first, {} left alone", orders.spot_priority.len(), orders.spot_avoid.len()))
        }
        "set_directives" => set_directives(arguments, shared),
        _ => Err(format!("unknown tool {name}")),
    }
}

fn squad(arguments: &Value, shared: &Shared) -> Result<String, String> {
    let name = arguments["name"].as_str().filter(|n| !n.is_empty()).ok_or("squad needs a name")?;
    let known: Vec<String> = shared.field().buildable.iter().map(|(name, _)| name.clone()).collect();
    let mut orders = shared.field_orders.lock().unwrap();
    let request = orders.squads.entry(name.to_string()).or_default();
    if let Some(take) = arguments["take"].as_object() {
        if let Some(unknown) = take.keys().find(|unit| !known.contains(unit)) {
            return Err(format!("{unknown} is not a unit our factories build; see `buildable`"));
        }
        for (unit, count) in take {
            *request.take.entry(unit.clone()).or_default() += count.as_u64().unwrap_or(0) as usize;
        }
    }
    if let Some(near) = position(&arguments["near"], "near")? {
        request.near = Some(near);
    }
    if let Some(at) = position(&arguments["post"], "post")? {
        let radius = arguments["post"]["radius"].as_f64().ok_or("post needs a radius")? as f32;
        request.post = Some(Post { at, radius: radius.clamp(100.0, 1500.0) });
    }
    if let Some(to) = position(&arguments["order"], "order")? {
        let kind = parse::<OrderKind>(&arguments["order"]["kind"])?.ok_or("order needs a kind")?;
        request.post = None;
        request.order = Some((kind, to));
        request.seen_by.clear();
    }
    if arguments["release"].as_bool() == Some(true) {
        request.release = true;
        request.seen_by.clear();
    }
    Ok(format!("squad {name} updated. The game is paused during your turn, so members are drawn and orders carried out when the turn ends; your next report shows the result"))
}

fn set_directives(arguments: &Value, shared: &Shared) -> Result<String, String> {
    let frame = shared.briefing().frame;
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
            "max_converters" => {
                directives.max_converters = parse::<usize>(value)?.map(|value| Timed { value, expires_frame });
            }
            "expansion_radius" => {
                directives.expansion_radius = parse::<usize>(value)?.map(|value| Timed { value, expires_frame });
            }
            "base_turrets" => {
                directives.base_turrets = parse::<usize>(value)?.map(|value| Timed { value, expires_frame });
            }
            "outpost_turrets" => {
                directives.outpost_turrets = parse::<OutpostTurrets>(value).map_err(|e| format!("outpost_turrets is \"all\", \"none\" or a list of spot numbers: {e}"))?.map(|value| Timed { value, expires_frame });
            }
            "resurrect" => directives.resurrect = parse::<bool>(value)?.map(|value| Timed { value, expires_frame }),
            "tier2" => directives.tier2 = parse::<bool>(value)?.map(|value| Timed { value, expires_frame }),
            "commander_station" => directives.commander_station = position(value, field)?.map(|value| Timed { value, expires_frame }),
            "pressure" => directives.pressure = parse::<bool>(value)?.map(|value| Timed { value, expires_frame }),
            "scout_at" => directives.scout_at = position(value, field)?.map(|value| Timed { value, expires_frame }),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_player_has_its_lever_and_none_of_the_commanders() {
        let names = |mode: Mode| tool_list(mode).as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap().to_string()).collect::<Vec<_>>();
        let player = names(Mode::Player);
        assert_eq!(player, ["overview", "map", "situation", "instruct", "orders", "wait", "note"]);
        let commander = names(Mode::Commander);
        assert!(commander.contains(&"squad".to_string()) && !commander.contains(&"instruct".to_string()));
        for tool in batchable(Mode::Player) {
            assert!(player.contains(&tool.to_string()));
        }
        let shared = Shared::default();
        assert!(call_tool("squad", &json!({ "name": "a" }), &shared, Mode::Player).is_err());
        assert!(call_tool("instruct", &json!({ "text": "commander: build the lab first." }), &shared, Mode::Player).is_ok());
        assert_eq!(*shared.instructions.lock().unwrap(), "commander: build the lab first.");
        assert!(call_tool("instruct", &json!({ "text": "x".repeat(INSTRUCTIONS_LIMIT + 1) }), &shared, Mode::Player).is_err());
    }
}
