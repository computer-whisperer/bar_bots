// Smoke test for the viewer's model: node viewer/test/smoke.js <match dir>
// Parses a real record with its siblings and checks the invariants the UI relies on.
"use strict";
const fs = require("fs");
const path = require("path");
const assert = require("assert");
const WR = require("../record.js");

const dir = process.argv[2];
if (!dir) throw new Error("usage: node viewer/test/smoke.js <match dir>");
const read = (name) => (fs.existsSync(path.join(dir, name)) ? fs.readFileSync(path.join(dir, name), "utf8") : null);
const recordName = fs.readdirSync(dir).find((f) => /^record-.*\.jsonl$/.test(f));
assert(recordName, "no record-*.jsonl");
const text = read(recordName);

const match = WR.parseRecord(text);
assert.strictEqual(match.header.version, 1);
assert(match.samples.length > 10, "samples");
assert(match.samples.every((s, i, all) => i === 0 || s.f > all[i - 1].f), "samples are in frame order");
for (const list of [match.events, match.decisions, match.commands, match.intents]) {
  assert(list.every((r, i, all) => i === 0 || r.f >= all[i - 1].f), "records are in frame order");
}
const defs = match.defs.length;
for (const s of match.samples) for (const u of s.own) assert(u.length === 6 && u[1] >= 0 && u[1] < defs, "own unit row");
for (const s of match.samples) for (const e of s.en) assert(e.length === 5 && e[1] >= -1 && e[1] < defs, "enemy row");

// Interpolation stays between the two samples.
const [a, b] = [match.samples[20], match.samples[21]];
const mid = WR.stateAt(match, (a.f + b.f) / 2);
assert.strictEqual(mid.own.length, a.own.length);
for (const u of mid.own) {
  const next = b.own.find((r) => r[0] === u.id);
  const prev = a.own.find((r) => r[0] === u.id);
  if (next) assert(Math.abs(u.x - (prev[2] + next[2]) / 2) < 1, "midpoint");
}
assert.strictEqual(WR.stateAt(match, -5).own.length, 0);
assert.strictEqual(WR.gridName(match, 0, 0), "A1");
assert.strictEqual(WR.gridName(match, match.header.map.width, match.header.map.height), "H8");

// A killed match: cut the file mid-line; everything before the cut must still load.
const cut = WR.parseRecord(text.slice(0, Math.floor(text.length / 2)));
assert(cut.badLines <= 1 && cut.samples.length > 5 && cut.result === null, "truncated record loads");

const lanes = WR.lanes(match);
const rules = WR.rulesInMinute(match, 5 * 60 * WR.FPS);
const report = {
  record: recordName, samples: match.samples.length, events: match.events.length, decisions: match.decisions.length,
  lastClock: WR.clock(match.lastFrame), result: match.result && match.result.result.outcome,
  lanes: Object.fromEntries(Object.entries(lanes).map(([k, v]) => [k, v.length])), rulesInMinute5: rules.slice(0, 3),
};

const engineLog = read("engine.log");
if (engineLog) {
  const census = WR.parseCensus(engineLog, match.classByName);
  report.censusMinutes = census.length;
  if (census.length) {
    const last = census[census.length - 1];
    assert(last.enemy.every((g) => g.count > 0 && Number.isFinite(g.x)), "census groups");
    report.lastCensus = { f: last.f, enemyArmy: last.enemyArmy, enemyExtractors: last.enemyExtractors, unclassified: last.enemy.filter((g) => g.class === "other").map((g) => g.name) };
  }
}
const strategist = read((match.header.siblings.decision_logs || [])[0] || "strategist-0.jsonl");
if (strategist) {
  const turns = WR.parseStrategist(strategist, "llm:test");
  assert(turns.length > 0 && turns.every((t) => Number.isFinite(t.f) && Array.isArray(t.outputs.calls)), "turns");
  report.llmTurns = turns.length;
  report.toolCalls = turns.reduce((n, t) => n + t.outputs.calls.length, 0);
  report.turnsWithWake = turns.filter((t) => t.inputs.wake).length;
  report.squadPosts = WR.squadPosts(turns, match.lastFrame).length;
}
const botLog = read("bot.log");
if (botLog) report.botLogLines = WR.parseBotLog(botLog).length;
console.log(JSON.stringify(report, null, 1));
