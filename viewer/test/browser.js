// Drives the real page in headless Chromium over the DevTools protocol (node >= 22 for the global WebSocket):
//   run/view_match.py <match dir> --no-browser &   then   node viewer/test/browser.js http://127.0.0.1:8137/
// Loads the match, plays, scrubs, hovers and toggles, and fails on any page exception or console error.
"use strict";
const { spawn } = require("child_process");
const fs = require("fs");
const os = require("os");
const path = require("path");

const url = process.argv[2] || "http://127.0.0.1:8137/";
const profile = fs.mkdtempSync(path.join(os.tmpdir(), "wr-viewer-"));
const chrome = spawn(process.env.CHROMIUM || "chromium", [
  "--headless", "--disable-gpu", "--no-sandbox", "--remote-debugging-port=0", `--user-data-dir=${profile}`, "--window-size=1600,1000", "about:blank",
], { stdio: ["ignore", "ignore", "pipe"] });

const fail = (message) => {
  console.error(`FAIL: ${message}`);
  chrome.kill();
  process.exit(1);
};
setTimeout(() => fail("timed out"), 120000);

let stderr = "";
chrome.stderr.on("data", async (chunk) => {
  stderr += chunk;
  const m = /DevTools listening on (ws:\/\/[^\s]+)/.exec(stderr);
  if (!m || chrome.started) return;
  chrome.started = true;
  const port = new URL(m[1]).port;
  const targets = await (await fetch(`http://127.0.0.1:${port}/json`)).json();
  run(new WebSocket(targets.find((t) => t.type === "page").webSocketDebuggerUrl));
});

function run(socket) {
  let id = 0;
  const waiting = new Map();
  const problems = [];
  socket.onmessage = ({ data }) => {
    const message = JSON.parse(data);
    if (message.id) waiting.get(message.id)?.(message.result);
    if (message.method === "Runtime.exceptionThrown") problems.push(message.params.exceptionDetails.exception?.description || message.params.exceptionDetails.text);
    if (message.method === "Runtime.consoleAPICalled" && message.params.type === "error") problems.push(JSON.stringify(message.params.args));
  };
  const send = (method, params = {}) => new Promise((resolve) => {
    waiting.set(++id, resolve);
    socket.send(JSON.stringify({ id, method, params }));
  });
  const evaluate = async (expression) => (await send("Runtime.evaluate", { expression, awaitPromise: true, returnByValue: true })).result.value;
  const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
  const mouse = (type, x, y, buttons = 0) => send("Input.dispatchMouseEvent", { type, x, y, button: buttons ? "left" : "none", buttons, clickCount: type === "mouseMoved" ? 0 : 1 });
  const centre = (selector, fx = 0.5, fy = 0.5) => evaluate(`(() => { const r = document.querySelector(${JSON.stringify(selector)}).getBoundingClientRect(); return [r.left + r.width * ${fx}, r.top + r.height * ${fy}]; })()`);

  socket.onopen = async () => {
    await send("Runtime.enable");
    await send("Page.enable");
    await send("Page.navigate", { url });
    for (let i = 0; i < 100 && !(await evaluate("document.body.dataset.loaded")); i++) await sleep(100);
    const loaded = await evaluate("document.body.dataset.loaded");
    if (!loaded) fail(`the match did not load: ${await evaluate("document.getElementById('status').textContent")}`);
    const report = { loaded, subtitle: await evaluate("document.getElementById('subtitle').textContent") };

    // Scrub: a click at 60 % of the timeline lands at about 60 % of the game.
    // A match still being played: the page follows it, and stops following once the reader scrubs.
    if (await evaluate("!viewer.match.result")) {
      const first = await evaluate("viewer.match.lastFrame");
      if (!(await evaluate("document.getElementById('follow').checked && viewer.frame === viewer.match.lastFrame"))) fail("a live match does not open on its newest sample");
      for (let i = 0; i < 450 && (await evaluate("viewer.match.lastFrame")) === first; i++) await sleep(100);
      report.live = { from: first, to: await evaluate("viewer.match.lastFrame"), following: await evaluate("viewer.frame === viewer.match.lastFrame") };
      if (!(report.live.to > first)) fail("the live match did not grow in 45 s (is it paused for a long turn, or over?)");
      if (!report.live.following) fail("the playhead did not follow the live match");
    }
    const [tx, ty] = await centre("#timeline", 0.6, 0.5);
    await mouse("mousePressed", tx, ty, 1);
    await mouse("mouseReleased", tx, ty);
    const share = await evaluate("viewer.frame / viewer.match.lastFrame");
    if (!(share > 0.5 && share < 0.7)) fail(`scrub landed at ${share}`);
    report.scrubbedTo = await evaluate("document.getElementById('clock').textContent");
    await mouse("mouseMoved", tx + 40, ty);
    report.chartTooltip = await evaluate("document.getElementById('chart-tooltip').hidden ? null : document.getElementById('chart-tooltip').textContent");
    if (!report.chartTooltip) fail("no timeline tooltip");

    // Play advances the playhead; pause stops it.
    const before = await evaluate("viewer.frame");
    await evaluate("document.getElementById('play').click()");
    await sleep(700);
    await evaluate("document.getElementById('play').click()");
    const after = await evaluate("viewer.frame");
    if (!(after > before)) fail(`play did not advance (${before} -> ${after})`);
    report.playedFrames = Math.round(after - before);

    // Hover one of our units on the map.
    const [ux, uy] = await evaluate("(() => { const u = viewer.state.own[0], b = viewer.mapBox, r = document.getElementById('map').getBoundingClientRect(); return [r.left + b.x + u.x * b.scale, r.top + b.y + u.z * b.scale]; })()");
    await mouse("mouseMoved", ux, uy);
    report.mapTooltip = await evaluate("document.getElementById('tooltip').textContent");
    if (!/ours: /.test(report.mapTooltip)) fail(`map tooltip: ${report.mapTooltip}`);

    await evaluate("document.querySelectorAll('#layers input').forEach((box) => box.click())");
    await evaluate("document.querySelectorAll('#decision-filters input').forEach((box) => box.click())");
    await evaluate("document.querySelectorAll('#decision-filters input').forEach((box) => box.click())");
    await send("Input.dispatchKeyEvent", { type: "keyDown", key: "ArrowLeft", code: "ArrowLeft" });
    report.panels = await evaluate("({ decisions: document.querySelectorAll('#decisions li').length, llmTurns: document.querySelectorAll('#decisions li.llm').length, rules: document.querySelectorAll('#rules .rule').length, stats: document.querySelectorAll('#now .stat').length, mapPixels: (() => { const c = document.getElementById('map'); const d = c.getContext('2d').getImageData(0, 0, c.width, c.height).data; let n = 0; for (let i = 0; i < d.length; i += 4) if (d[i + 2] > 200 && d[i] < 100) n++; return n; })() })");
    if (!report.panels.stats || !report.panels.mapPixels) fail(`empty panels: ${JSON.stringify(report.panels)}`);
    if (problems.length) fail(`page errors: ${problems.join(" | ")}`);
    console.log(JSON.stringify(report, null, 1));
    console.log("PASS");
    chrome.kill();
    fs.rmSync(profile, { recursive: true, force: true });
    process.exit(0);
  };
}
