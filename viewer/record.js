// Within Reason match viewer: parsing and the match model. No DOM in here, so it also runs under node
// (viewer/test/smoke.js). The record format is documented in docs/harness/record-format.md.
"use strict";

const WR = (() => {
  const FPS = 30;
  const FLAG = { beingBuilt: 1, idle: 2, attacker: 4, squad: 8 };

  // JSON Lines with a possibly truncated last line (a killed match). Returns the values and the bad line count.
  function parseLines(text) {
    const values = [];
    let bad = 0;
    for (const line of text.split("\n")) {
      if (!line.trim()) continue;
      try {
        values.push(JSON.parse(line));
      } catch {
        bad++;
      }
    }
    return { values, bad };
  }

  function parseRecord(text) {
    const { values, bad } = parseLines(text);
    const match = {
      header: null, samples: [], events: [], decisions: [], commands: [], intents: [], result: null,
      restarts: [], badLines: bad, census: [], truth: [], botLog: [],
    };
    for (const r of values) {
      switch (r.t) {
        case "header":
          if (match.header) match.restarts.push(r.first_tick_frame ?? 0);
          else match.header = r;
          break;
        case "s": match.samples.push(r); break;
        case "ev": match.events.push(r); break;
        case "d": match.decisions.push(r); break;
        case "cmd": match.commands.push(r); break;
        case "intent": match.intents.push(r); break;
        case "result": match.result = r; break;
        default: break; // records from a newer format version are skipped, not fatal
      }
    }
    if (!match.header) throw new Error("no header line: not a Within Reason match record");
    if (match.header.format !== "within-reason-record") throw new Error(`unknown format ${match.header.format}`);
    match.defs = match.header.unit_defs;
    match.classByName = new Map(match.defs.map((d) => [d.name, d.class]));
    match.lastFrame = match.samples.length ? match.samples[match.samples.length - 1].f : 0;
    match.series = ownSeries(match);
    return match;
  }

  // Per-sample numbers for the economy strip.
  function ownSeries(match) {
    const classOf = (def) => (def >= 0 && match.defs[def] ? match.defs[def].class : "other");
    return match.samples.map((s) => {
      let extractors = 0, army = 0, builders = 0, buildings = 0;
      for (const u of s.own) {
        if (u[5] & FLAG.beingBuilt) continue;
        const c = classOf(u[1]);
        if (c === "extractor") extractors++;
        else if (c === "army") army++;
        else if (c === "builder") builders++;
        if (c === "extractor" || c === "factory" || c === "turret" || c === "building") buildings++;
      }
      return { f: s.f, metalIncome: s.m[1], energyIncome: s.e[1], metal: s.m[0], energy: s.e[0], extractors, army, builders, buildings, enemiesVisible: s.en.length, decideMs: s.ms };
    });
  }

  // The strategist / commander transcript, folded into turns. A turn is also a decision record in the
  // source-agnostic shape of the match record: {f, source, kind, inputs, outputs, latency_ms}.
  function parseStrategist(text, source) {
    const { values } = parseLines(text);
    const turns = [];
    let turn = null;
    for (const r of values) {
      if (r.kind === "turn") {
        const woken = /Woken because: (.*)/.exec(r.prompt || "");
        turn = { f: r.frame, prompt: r.prompt || "", wake: woken ? woken[1] : null, calls: [], said: [], thinking: [], wallSeconds: null, cost: null, stopped: null };
        turns.push(turn);
      } else if (!turn) {
        continue;
      } else if (r.kind === "tool_call") {
        turn.calls.push({ tool: r.tool, arguments: r.arguments, result: r.result });
      } else if (r.kind === "assistant") {
        for (const part of r.message?.message?.content || []) {
          if (part.type === "text" && part.text.trim()) turn.said.push(part.text.trim());
          if (part.type === "thinking" && part.thinking?.trim()) turn.thinking.push(part.thinking.trim());
        }
      } else if (r.kind === "result") {
        turn.cost = r.message?.total_cost_usd ?? null;
      } else if (r.kind === "turn_end") {
        turn.wallSeconds = r.wall_seconds;
      } else if (r.kind === "stopped") {
        turn.stopped = r.reason || r.message || "stopped";
      }
    }
    return turns.map((t) => ({
      t: "d", f: t.f, source, kind: "turn",
      inputs: { wake: t.wake, prompt: t.prompt },
      outputs: { calls: t.calls, said: t.said, thinking: t.thinking, stopped: t.stopped },
      latency_ms: t.wallSeconds == null ? null : t.wallSeconds * 1000, cost: t.cost,
    }));
  }

  // Posts the commander gave its squads, from the `squad` tool calls: [{f, name, x, z, radius, until}].
  function squadPosts(decisions, lastFrame) {
    const posts = [];
    const open = new Map();
    const close = (name, f) => {
      const post = open.get(name);
      if (post) post.until = f;
      open.delete(name);
    };
    for (const d of decisions) {
      if (d.kind !== "turn") continue;
      for (const call of d.outputs.calls) {
        const a = call.arguments || {};
        if (call.tool !== "squad" || !a.name) continue;
        if (a.release || a.order || a.post) close(a.name, d.f);
        if (a.post && !a.release) {
          const post = { f: d.f, name: a.name, x: a.post.x, z: a.post.z, radius: a.post.radius, until: lastFrame + 1 };
          posts.push(post);
          open.set(a.name, post);
        }
      }
    }
    return posts;
  }

  // `census f=<frame> enemy|own namexCOUNT@x,z ...` lines of engine.log (WITHIN_REASON_OBSERVE=1).
  function parseCensus(text, classByName) {
    const byFrame = new Map();
    const line = /census f=(\d+) (enemy|own) ?(.*)/g;
    for (const m of text.matchAll(line)) {
      const f = Number(m[1]);
      const groups = [];
      for (const g of m[3].matchAll(/(\w+?)x(\d+)@(-?\d+),(-?\d+)/g)) {
        groups.push({ name: g[1], count: Number(g[2]), x: Number(g[3]), z: Number(g[4]), class: classByName.get(g[1]) || "other" });
      }
      if (!byFrame.has(f)) byFrame.set(f, { f, own: [], enemy: [] });
      byFrame.get(f)[m[2]] = groups;
    }
    const total = (groups, cls) => groups.reduce((n, g) => n + (g.class === cls ? g.count : 0), 0);
    return [...byFrame.values()].sort((a, b) => a.f - b.f).map((c) => ({
      ...c,
      enemyExtractors: total(c.enemy, "extractor"), enemyArmy: total(c.enemy, "army"),
      ownExtractors: total(c.own, "extractor"), ownArmy: total(c.own, "army"),
    }));
  }

  // truth-<ai>.jsonl (WITHIN_REASON_OBSERVE=1): every enemy unit every two seconds, `[id, name, x, z, health %, being built]`.
  // Entries carry the same totals as census entries, so the charts and the stats take either.
  function parseTruth(text, classByName) {
    const out = [];
    for (const line of text.split("\n")) {
      if (!line) continue;
      let row;
      try {
        row = JSON.parse(line);
      } catch (_) {
        continue; // a killed match may end mid-line
      }
      const units = row.enemy.map((u) => ({ id: u[0], name: u[1], class: classByName.get(u[1]) || "other", x: u[2], z: u[3], health: u[4], building: !!u[5] }));
      const count = (cls) => units.reduce((n, u) => n + (u.class === cls && !u.building ? 1 : 0), 0);
      out.push({ f: row.f, units, enemyExtractors: count("extractor"), enemyArmy: count("army") });
    }
    return out.sort((a, b) => a.f - b.f);
  }

  // bot.log lines that carry a frame: [{f, text}].
  function parseBotLog(text) {
    const lines = [];
    for (const raw of text.split("\n")) {
      const m = /^\[ai -?\d+\] f=(\d+) (.*)/.exec(raw);
      if (m) lines.push({ f: Number(m[1]), text: m[2] });
    }
    return lines;
  }

  // Index of the last item with f <= frame, or -1.
  function indexAt(items, frame) {
    let lo = 0, hi = items.length - 1, found = -1;
    while (lo <= hi) {
      const mid = (lo + hi) >> 1;
      if (items[mid].f <= frame) {
        found = mid;
        lo = mid + 1;
      } else {
        hi = mid - 1;
      }
    }
    return found;
  }

  function range(items, from, to) {
    return items.slice(indexAt(items, from - 1) + 1, indexAt(items, to) + 1);
  }

  // Units at `frame`, positions interpolated towards the next sample. Rows: {id, def, x, z, health, flags, damage}.
  function stateAt(match, frame) {
    const i = indexAt(match.samples, frame);
    if (i < 0) return { sample: null, own: [], allies: [], enemies: [] };
    const a = match.samples[i];
    const b = match.samples[i + 1];
    // A long gap is a bot restart or a held game, not motion.
    const t = b && b.f - a.f <= 4 * (match.header.sample_frames || FPS) ? (frame - a.f) / (b.f - a.f) : 0;
    const blend = (rows, next, healthIndex) => {
      const later = new Map((next || []).map((r) => [r[0], r]));
      return rows.map((r) => {
        const n = t > 0 ? later.get(r[0]) : null;
        const [x, z] = n ? [r[2] + (n[2] - r[2]) * t, r[3] + (n[3] - r[3]) * t] : [r[2], r[3]];
        return { id: r[0], def: r[1], x, z, health: r[healthIndex], flags: r[5] || 0 };
      });
    };
    const damaged = new Map(((b || a).dmg || []).map(([id, amount]) => [id, amount]));
    const own = blend(a.own, b && b.own, 4);
    for (const u of own) u.damage = damaged.get(u.id) || 0;
    // Allies (`al`: [id, def, x, z, team, being built]): other seats of ours, or anybody else on our side.
    const allies = blend(a.al || [], b && b.al, 4).map((u) => ({ ...u, team: u.health, health: null, flags: u.flags ? FLAG.beingBuilt : 0 }));
    return { sample: a, own, allies, enemies: blend(a.en, b && b.en, 4) };
  }

  // Heuristic firings summed over the game minute that contains `frame`: [[rule, count]] by count.
  function rulesInMinute(match, frame) {
    const start = Math.floor(frame / (60 * FPS)) * 60 * FPS;
    const totals = new Map();
    for (const d of range(match.decisions, start, start + 60 * FPS - 1)) {
      if (d.kind !== "rules") continue;
      for (const [rule, n] of Object.entries(d.outputs)) totals.set(rule, (totals.get(rule) || 0) + n);
    }
    return [...totals.entries()].sort((a, b) => b[1] - a[1]);
  }

  // The timeline's lanes, from events and decisions: {losses, buildingLosses, extractorLosses, kills, waves, turns}.
  function lanes(match) {
    const cls = (def) => (def >= 0 && match.defs[def] ? match.defs[def].class : "other");
    const out = { losses: [], buildingLosses: [], extractorLosses: [], kills: [], waves: [], turns: [] };
    for (const e of match.events) {
      if (e.k === "enemy_destroyed") out.kills.push(e);
      if (e.k !== "destroyed") continue;
      const c = cls(e.d);
      if (c === "extractor") out.extractorLosses.push(e);
      else if (c === "army" || c === "builder" || c === "commander" || c === "other") out.losses.push(e);
      else out.buildingLosses.push(e);
    }
    for (const d of match.decisions) {
      if (d.kind === "wave" || d.kind === "recall" || d.kind === "assault") out.waves.push(d);
      if (d.kind === "turn") out.turns.push(d);
    }
    return out;
  }

  function clock(frame) {
    const seconds = Math.floor(frame / FPS);
    return `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, "0")}`;
  }

  // Same cells as `World::grid` in crates/bot/src/world.rs.
  function gridName(match, x, z) {
    const { columns, rows } = match.header.grid;
    const cell = (v, extent, n) => Math.min(n - 1, Math.max(0, Math.floor((v / extent) * n)));
    return String.fromCharCode(65 + cell(x, match.header.map.width, columns)) + (cell(z, match.header.map.height, rows) + 1);
  }

  return { FPS, FLAG, parseRecord, parseStrategist, parseCensus, parseTruth, parseBotLog, squadPosts, indexAt, range, stateAt, rulesInMinute, lanes, clock, gridName };
})();

if (typeof module !== "undefined") module.exports = WR;
