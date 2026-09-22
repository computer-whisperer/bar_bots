// Within Reason match viewer: loading, the map, the timeline and the panels. Parsing and the model are in record.js.
//
// URL parameters: match=<base URL of the match files, default "match/">, record=<file name>, t=<seconds>,
// bg=<image URL drawn under the map>. Without a server, use "Open files" and pick the record and its siblings.
// A record without a result line is a match still being played: its files are polled for new bytes
// (run/view_match.py serves `?from=<offset>`) and, with "follow live" ticked, the playhead stays on the newest sample.
"use strict";

(() => {
  const $ = (id) => document.getElementById(id);
  const css = (name) => getComputedStyle(document.documentElement).getPropertyValue(name).trim();
  const COLOR = {};
  for (const name of ["surface", "surface-2", "line", "ink", "ink-2", "muted", "ours", "theirs", "good", "critical", "warning", "llm", "jev"]) COLOR[name] = css(`--${name}`);
  const OWN_ATTACKER = "#86b6ef";
  const ALLY = "#3fae8f";
  const ORDER_COLOR = { build: COLOR.good, fight: COLOR.critical, move: COLOR.muted };
  /// How long an order line and a death mark stay on the map, in frames.
  const ORDER_FRAMES = 3 * WR.FPS;
  const DEATH_FRAMES = 20 * WR.FPS;

  const view = {
    match: null, lanes: null, posts: [], frame: 0, playing: false, speed: 10, lastTime: 0,
    layers: { terrain: true, nobots: true, notanks: false, grid: true, spots: true, truth: true, census: true, orders: true, intent: true, deaths: true, pianist: true },
    terrain: null,
    background: null, mapBox: null, hoverFrame: null, decisionItems: [], currentDecision: -2,
    show: { heuristic: true, llm: true, event: false, jev: true, jevChangesOnly: true },
    /// The pianist's actor the decision list is narrowed to ("" for all), and the call drawn last.
    jevActor: "", callShown: null, tab: null,
  };
  window.viewer = view;

  // ---------------------------------------------------------------- loading

  async function fetchText(url) {
    try {
      const response = await fetch(url);
      return response.ok ? await response.text() : null;
    } catch {
      return null;
    }
  }

  /// A file of the match being followed: the text so far and how many bytes of the file it is.
  const pulled = new Map();
  const POLL_MS = 3000;

  /// Fetches what a file has gained since the last pull. True if it grew.
  async function pull(base, name) {
    const file = pulled.get(name) || { bytes: 0, text: "", decoder: new TextDecoder() };
    try {
      const response = await fetch(`${base}${name}?from=${file.bytes}`);
      if (!response.ok) return false;
      const chunk = new Uint8Array(await response.arrayBuffer());
      // A server that does not know `from` (anything but run/view_match.py) sends the whole file every time.
      const tail = response.headers.get("X-From") === String(file.bytes);
      if (tail ? !chunk.length : chunk.length === file.bytes) return false;
      if (!tail) Object.assign(file, { bytes: 0, text: "", decoder: new TextDecoder() });
      file.text += file.decoder.decode(chunk, { stream: true });
      file.bytes += chunk.length;
      pulled.set(name, file);
      return true;
    } catch {
      return false;
    }
  }

  /// The text up to its last complete line: the writer may be half way through one.
  const complete = (name) => {
    const text = (pulled.get(name) || {}).text;
    return text ? text.slice(0, text.lastIndexOf("\n") + 1) : null;
  };

  /// Pulls every file of the match; returns the texts `open` takes, or null when nothing has changed.
  async function pullMatch(live) {
    const index = await fetchText(`${live.base}index.json`);
    const listing = index ? JSON.parse(index) : { files: [], replays: [] };
    const files = listing.files || [];
    showReplay(live.base, listing.replays || []);
    if (!live.record) live.record = files.find((f) => /^record-.*\.jsonl$/.test(f));
    if (!live.record) return { files };
    let grew = await pull(live.base, live.record);
    const record = complete(live.record);
    if (!record) return { files };
    const header = JSON.parse(record.slice(0, record.indexOf("\n")));
    const siblings = header.siblings || {};
    // Transcripts the header names, and any other the directory lists (a record and a transcript brought together by hand).
    const logs = [...new Set([...(siblings.decision_logs || []), ...files.filter((f) => /^(strategist|jev)-.*\.jsonl$/.test(f))])];
    const names = { engineLog: siblings.engine_log || "engine.log", botLog: siblings.bot_log || "bot.log", truth: `truth-${header.ai_id}.jsonl` };
    for (const name of [...logs, ...Object.values(names)]) grew = (await pull(live.base, name)) || grew;
    if (!grew) return null;
    return {
      files,
      texts: {
        record,
        strategist: logs.filter((f) => /^strategist/.test(f)).map(complete).filter(Boolean),
        jev: logs.filter((f) => /^jev/.test(f)).map(complete).find(Boolean) || null,
        engineLog: complete(names.engineLog), botLog: complete(names.botLog), truth: complete(names.truth),
      },
    };
  }

  /// The engine's replay of the match (demos/*.sdfz, written when the engine quits), as a download link in the header.
  function showReplay(base, replays) {
    const link = $("replay");
    if (!replays.length) return;
    const path = replays[replays.length - 1];
    link.href = base + path.split("/").map(encodeURIComponent).join("/");
    link.download = path.split("/").pop();
    link.hidden = false;
  }

  async function boot() {
    const params = new URLSearchParams(location.search);
    const live = { base: params.get("match") || "match/", record: params.get("record") };
    const first = await pullMatch(live);
    if (!first.texts) {
      if (first.files.length) return status(`No record-*.jsonl in the match directory (was it played with WITHIN_REASON_RECORD=1?). Files: ${first.files.join(", ")}`);
      return status("No match loaded. Start with run/view_match.py <match dir>, or use Open files.");
    }
    open(first.texts, Number(params.get("t") || 0) * WR.FPS);
    const bg = params.get("bg") || `maps/${encodeURIComponent(view.match.header.map.name)}.png`;
    loadBackground(bg);
    loadTerrain(live.base, view.match.header.terrain);
    if (!view.match.result) follow(live, !params.get("t"));
  }

  /// Keeps a match that is still being played up to date, until its result line arrives.
  function follow(live, stayOnNewest) {
    $("follow-label").hidden = false;
    $("follow").checked = stayOnNewest;
    if (stayOnNewest) seek(view.match.lastFrame);
    const timer = setInterval(async () => {
      const next = await pullMatch(live);
      if (!next || !next.texts) return;
      open(next.texts, $("follow").checked ? Infinity : view.frame, true);
      if (view.match.result) {
        clearInterval(timer);
        $("follow-label").hidden = true;
      }
    }, POLL_MS);
  }

  // The record's terrain grid (docs/harness/record-format.md): heights then slopes, rendered once into three
  // canvases the map draws under everything else: relief with water, and where bots and vehicles cannot go.
  async function loadTerrain(base, terrain) {
    if (!terrain || !terrain.file) return;
    let bytes;
    try {
      const response = await fetch(base + terrain.file);
      if (!response.ok) return;
      bytes = await response.arrayBuffer();
    } catch (_) {
      return;
    }
    const { width, height } = terrain;
    const cells = width * height;
    if (bytes.byteLength < cells * 3) return;
    const heights = new Int16Array(bytes, 0, cells);
    const slopes = new Uint8Array(bytes, cells * 2, cells);
    const canvasOf = (paint) => {
      const canvas = document.createElement("canvas");
      canvas.width = width;
      canvas.height = height;
      const ctx = canvas.getContext("2d");
      const image = ctx.createImageData(width, height);
      for (let i = 0; i < cells; i++) paint(i, image.data, i * 4);
      ctx.putImageData(image, 0, 0);
      return canvas;
    };
    let top = 1;
    for (let i = 0; i < cells; i++) if (heights[i] > top) top = heights[i];
    const relief = canvasOf((i, out, o) => {
      const h = heights[i];
      // Light from the north-west: a cell brighter than its south-east neighbour faces the light.
      const x = i % width, z = (i / width) | 0;
      const other = heights[Math.min(z + 1, height - 1) * width + Math.min(x + 1, width - 1)];
      const shade = Math.max(-40, Math.min(40, (h - other) * 6));
      if (h < 0) {
        const depth = Math.min(1, -h / 120);
        out[o] = 18; out[o + 1] = 52 - 20 * depth; out[o + 2] = 96 - 36 * depth;
      } else {
        const t = h / top;
        out[o] = 46 + 96 * t + shade; out[o + 1] = 62 + 78 * t + shade; out[o + 2] = 40 + 60 * t + shade;
      }
      out[o + 3] = 255;
    });
    const blocked = (kind, rgb) => {
      const classes = (terrain.move_classes || []).filter((c) => c.kind === kind);
      if (!classes.length) return null;
      // The ordinary class of the kind: not the amphibians (any depth) or climbers (any slope), then the one most
      // unit types use.
      const ordinary = classes.filter((c) => c.depth < 1000 && c.max_slope < 0.99);
      const usual = (ordinary.length ? ordinary : classes).reduce((a, b) => ((b.units || 0) > (a.units || 0) ? b : a));
      const maxSlope = usual.max_slope * 255;
      const depth = usual.depth;
      return canvasOf((i, out, o) => {
        const no = slopes[i] > maxSlope || heights[i] < -depth;
        out[o] = rgb[0]; out[o + 1] = rgb[1]; out[o + 2] = rgb[2]; out[o + 3] = no ? 150 : 0;
      });
    };
    view.terrain = { relief, nobots: blocked("bot", [200, 40, 40]), notanks: blocked("tank", [230, 140, 30]), heights, width, height, cell: terrain.cell };
    drawMap();
  }

  // Terrain hook: any image of the whole map, north up. Missing is normal.
  function loadBackground(url) {
    const image = new Image();
    image.onload = () => {
      view.background = image;
      drawMap();
    };
    image.src = url;
  }

  async function openFiles(fileList) {
    const texts = { strategist: [] };
    for (const file of fileList) {
      const text = await file.text();
      if (/^strategist.*\.jsonl$/.test(file.name)) texts.strategist.push(text);
      else if (/^jev.*\.jsonl$/.test(file.name)) texts.jev = text;
      else if (/\.jsonl$/.test(file.name)) texts.record = text;
      else if (/engine/.test(file.name)) texts.engineLog = text;
      else if (/bot/.test(file.name)) texts.botLog = text;
    }
    if (!texts.record) return status("Pick a record-*.jsonl (and optionally strategist-*.jsonl, engine.log, bot.log).");
    open(texts, 0);
  }

  /// `refresh`: the same match with more of it; what the reader has open and where they have scrolled is kept.
  function open(texts, frame, refresh = false) {
    let match;
    try {
      match = WR.parseRecord(texts.record);
    } catch (e) {
      return status(`Cannot read the record: ${e.message}`);
    }
    const mode = match.header.mode;
    const source = mode === "heuristic" ? "llm" : `llm:${mode}`;
    for (const text of texts.strategist || []) match.decisions.push(...WR.parseStrategist(text, source));
    match.decisions.sort((a, b) => a.f - b.f);
    if (texts.engineLog) match.census = WR.parseCensus(texts.engineLog, match.classByName);
    if (texts.truth) match.truth = WR.parseTruth(texts.truth, match.classByName);
    // The opponent's curve: ground truth every two seconds when the match has it, else the once-a-minute census.
    match.theirs = match.truth.length ? match.truth : match.census;
    if (texts.botLog) match.botLog = WR.parseBotLog(texts.botLog);
    match.jev = texts.jev ? WR.parseJev(texts.jev) : null;
    document.body.dataset.pianist = match.jev ? "1" : "";
    document.querySelector('#tabs [data-tab="pianist"]').hidden = !match.jev;
    if (match.jev) buildPianistMinutes(match.jev);
    view.callShown = null;
    view.match = match;
    view.lanes = WR.lanes(match);
    view.posts = WR.squadPosts(match.decisions, match.lastFrame);
    view.frame = Math.min(frame, match.lastFrame);
    describeMatch();
    if (refresh) rebuildDecisionList();
    else buildDecisionList();
    if (!refresh) buildLegend();
    if (refresh) {
      renderAll();
    } else {
      // The tab last used, unless it is the pianist's and this match has no log; setTab renders everything.
      let saved = null;
      try { saved = localStorage.getItem("wr-tab"); } catch (_) { /* no storage */ }
      setTab(saved && (saved !== "pianist" || match.jev) ? saved : match.jev ? "pianist" : "decisions");
    }
    document.body.dataset.loaded = `${match.samples.length} samples`;
  }

  function status(text) {
    $("status").textContent = text;
  }

  function describeMatch() {
    const { header, result, samples, badLines, restarts, census } = view.match;
    const side = { arm: "Armada", cor: "Cortex", leg: "Legion" }[header.side] || header.side || "?";
    const parts = [`${header.map.name}`, `${side}, team ${header.team}`, header.mode];
    if (result) {
      const r = result.result;
      parts.push(`vs ${result.opponent}`, `${r.outcome} after ${r.game_minutes.toFixed(1)} min (${r.our_corner})`);
      if (result.replay) parts.push(`engine replay: ${result.replay}`);
    } else {
      parts.push("no result line (match unfinished or killed)");
    }
    $("subtitle").textContent = parts.join("  ·  ");
    const notes = [`${samples.length} samples`];
    if (badLines) notes.push(`${badLines} unreadable line(s) skipped`);
    if (restarts.length) notes.push(`bot restarted at ${restarts.map(WR.clock).join(", ")}`);
    if (view.match.jev) notes.push(`pianist: ${view.match.jev.calls.length} calls to ${view.match.jev.header?.model || view.match.jev.calls[0]?.model || "Jev"}${view.match.jev.errors.length ? `, ${view.match.jev.errors.length} failed` : ""}`);
    notes.push(view.match.truth.length ? "opponent ground truth" : census.length ? `${census.length} census minutes` : "no census (play with WITHIN_REASON_OBSERVE=1 for the opponent's truth)");
    status(notes.join(" · "));
  }

  // ---------------------------------------------------------------- canvas helpers

  function fitCanvas(canvas) {
    const dpr = window.devicePixelRatio || 1;
    const { clientWidth: w, clientHeight: h } = canvas;
    if (canvas.width !== Math.round(w * dpr) || canvas.height !== Math.round(h * dpr)) {
      canvas.width = Math.round(w * dpr);
      canvas.height = Math.round(h * dpr);
    }
    const ctx = canvas.getContext("2d");
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.clearRect(0, 0, w, h);
    return { ctx, w, h };
  }

  // One glyph per unit class; buildings are angular, mobile units round.
  function glyph(ctx, cls, x, y, color, scale = 1) {
    ctx.fillStyle = color;
    ctx.strokeStyle = color;
    ctx.lineWidth = 1.5;
    const square = (r) => ctx.fillRect(x - r, y - r, 2 * r, 2 * r);
    ctx.beginPath();
    switch (cls) {
      case "commander":
        ctx.moveTo(x, y - 7 * scale); ctx.lineTo(x + 6 * scale, y); ctx.lineTo(x, y + 7 * scale); ctx.lineTo(x - 6 * scale, y);
        ctx.closePath(); ctx.fill();
        break;
      case "army": ctx.arc(x, y, 3 * scale, 0, 7); ctx.fill(); break;
      case "builder": ctx.arc(x, y, 3.5 * scale, 0, 7); ctx.stroke(); break;
      case "extractor":
        square(3.5 * scale);
        ctx.fillStyle = COLOR.surface; ctx.fillRect(x - 1.2 * scale, y - 1.2 * scale, 2.4 * scale, 2.4 * scale);
        break;
      case "factory": square(5.5 * scale); break;
      case "turret":
        ctx.moveTo(x, y - 5 * scale); ctx.lineTo(x + 4.5 * scale, y + 3.5 * scale); ctx.lineTo(x - 4.5 * scale, y + 3.5 * scale);
        ctx.closePath(); ctx.fill();
        break;
      case "building": square(2.5 * scale); break;
      case "unknown": ctx.arc(x, y, 2.5 * scale, 0, 7); ctx.setLineDash([2, 2]); ctx.stroke(); ctx.setLineDash([]); break;
      default: ctx.arc(x, y, 2 * scale, 0, 7); ctx.fill();
    }
  }

  function buildLegend() {
    const list = $("map-legend");
    list.textContent = "";
    const entry = (cls, color, text) => {
      const li = document.createElement("li");
      const icon = document.createElement("canvas");
      icon.width = icon.height = 16;
      glyph(icon.getContext("2d"), cls, 8, 8, color);
      li.append(icon, text);
      list.append(li);
    };
    for (const cls of ["commander", "army", "builder", "extractor", "factory", "turret", "building"]) entry(cls, COLOR.ours, cls);
    entry("army", OWN_ATTACKER, "attacker");
    entry("army", ALLY, "ally (another seat of ours, or another player)");
    entry("army", COLOR.theirs, "enemy seen");
    entry("unknown", COLOR.theirs, "radar contact");
  }

  // ---------------------------------------------------------------- map

  function classOf(def) {
    const d = def >= 0 ? view.match.defs[def] : null;
    return d ? d.class : "unknown";
  }

  function defName(def) {
    const d = def >= 0 ? view.match.defs[def] : null;
    return d ? d.name : "unidentified";
  }

  function drawMap() {
    const { ctx, w, h } = fitCanvas($("map"));
    const match = view.match;
    if (!match) return;
    const map = match.header.map;
    const margin = 18;
    const scale = Math.min((w - 2 * margin) / map.width, (h - 2 * margin) / map.height);
    const box = { x: (w - map.width * scale) / 2, y: (h - map.height * scale) / 2, w: map.width * scale, h: map.height * scale, scale };
    view.mapBox = box;
    const px = (x) => box.x + x * scale;
    const pz = (z) => box.y + z * scale;

    ctx.fillStyle = "#121211";
    ctx.fillRect(box.x, box.y, box.w, box.h);
    if (view.background) {
      ctx.globalAlpha = 0.55;
      ctx.drawImage(view.background, box.x, box.y, box.w, box.h);
      ctx.globalAlpha = 1;
    }
    if (view.terrain) {
      ctx.imageSmoothingEnabled = true;
      if (view.layers.terrain) {
        ctx.globalAlpha = 0.75;
        ctx.drawImage(view.terrain.relief, box.x, box.y, box.w, box.h);
      }
      ctx.globalAlpha = 0.55;
      if (view.layers.notanks && view.terrain.notanks) ctx.drawImage(view.terrain.notanks, box.x, box.y, box.w, box.h);
      if (view.layers.nobots && view.terrain.nobots) ctx.drawImage(view.terrain.nobots, box.x, box.y, box.w, box.h);
      ctx.globalAlpha = 1;
    }

    if (view.layers.grid) {
      const { columns, rows } = match.header.grid;
      ctx.strokeStyle = COLOR.line;
      ctx.lineWidth = 1;
      ctx.fillStyle = COLOR.muted;
      ctx.font = "11px system-ui";
      ctx.textAlign = "center";
      ctx.textBaseline = "middle";
      for (let c = 0; c <= columns; c++) {
        const x = Math.round(box.x + (box.w * c) / columns) + 0.5;
        ctx.beginPath(); ctx.moveTo(x, box.y); ctx.lineTo(x, box.y + box.h); ctx.stroke();
        if (c < columns) ctx.fillText(String.fromCharCode(65 + c), x + box.w / columns / 2, box.y - 9);
      }
      for (let r = 0; r <= rows; r++) {
        const y = Math.round(box.y + (box.h * r) / rows) + 0.5;
        ctx.beginPath(); ctx.moveTo(box.x, y); ctx.lineTo(box.x + box.w, y); ctx.stroke();
        if (r < rows) ctx.fillText(String(r + 1), box.x - 9, y + box.h / rows / 2);
      }
    }

    if (view.layers.spots) {
      ctx.strokeStyle = COLOR.muted;
      ctx.lineWidth = 1;
      for (const [x, z] of match.header.metal_spots) {
        ctx.beginPath(); ctx.arc(px(x), pz(z), 5, 0, 7); ctx.stroke();
      }
    }

    // Ground truth: every enemy unit where it really was, faint; what our units could see is drawn solid on top.
    const truth = view.layers.truth ? match.truth[WR.indexAt(match.truth, view.frame)] : null;
    if (truth) {
      ctx.globalAlpha = 0.5;
      for (const u of truth.units) glyph(ctx, u.class, px(u.x), pz(u.z), COLOR.theirs, u.building ? 0.7 : 1);
      ctx.globalAlpha = 1;
    }
    const census = view.layers.census && !truth ? match.census[WR.indexAt(match.census, view.frame)] : null;
    if (census) {
      ctx.globalAlpha = 0.4;
      ctx.font = "10px system-ui";
      ctx.textAlign = "left";
      // Label the biggest groups first and skip labels that would land on one already drawn; hover names the rest.
      const labelled = [];
      for (const g of [...census.enemy].sort((a, b) => b.count - a.count)) {
        const [x, y] = [px(g.x), pz(g.z)];
        glyph(ctx, g.class, x, y, COLOR.theirs, 1.3);
        if (labelled.some(([lx, ly]) => Math.abs(lx - x) < 70 && Math.abs(ly - y) < 11)) continue;
        labelled.push([x, y]);
        ctx.fillStyle = COLOR["ink-2"];
        ctx.fillText(`${g.name} x${g.count}`, x + 8, y);
      }
      ctx.globalAlpha = 1;
    }

    ctx.strokeStyle = COLOR.llm;
    ctx.fillStyle = COLOR.llm;
    ctx.font = "11px system-ui";
    ctx.textAlign = "center";
    for (const post of view.posts) {
      if (post.f > view.frame || post.until <= view.frame) continue;
      ctx.setLineDash([4, 3]);
      ctx.beginPath(); ctx.arc(px(post.x), pz(post.z), post.radius * scale, 0, 7); ctx.stroke();
      ctx.setLineDash([]);
      ctx.fillText(post.name, px(post.x), pz(post.z) - post.radius * scale - 6);
    }

    const intent = view.layers.intent ? match.intents[WR.indexAt(match.intents, view.frame)] : null;
    if (intent) {
      const mark = (at, label, color) => {
        if (!at) return;
        const [x, y] = [px(at[0]), pz(at[1])];
        ctx.strokeStyle = color;
        ctx.lineWidth = 1.5;
        ctx.beginPath();
        ctx.moveTo(x - 8, y); ctx.lineTo(x + 8, y); ctx.moveTo(x, y - 8); ctx.lineTo(x, y + 8);
        ctx.stroke();
        ctx.fillStyle = COLOR["ink-2"];
        ctx.textAlign = "left";
        ctx.fillText(label, x + 9, y - 8);
      };
      mark(intent.home, "home", COLOR.ours);
      mark(intent.enemy_start, "enemy (presumed)", COLOR.theirs);
      mark(intent.station, "station", COLOR.ours);
      mark(intent.staging, "staging", COLOR.warning);
      mark(intent.target, "target", COLOR.critical);
    }

    const state = WR.stateAt(match, view.frame);
    view.state = state;
    const byId = new Map(state.own.map((u) => [u.id, u]));

    // The pianist's names at the playhead: places, parties, groups and where each group is going.
    const call = view.layers.pianist && match.jev ? match.jev.calls[WR.indexAt(match.jev.calls, view.frame)] : null;
    if (call && view.frame - call.f < 60 * WR.FPS) {
      ctx.font = "10px system-ui";
      ctx.textAlign = "left";
      ctx.textBaseline = "middle";
      ctx.fillStyle = COLOR.muted;
      for (const p of call.places) {
        if (p.name === "home" || p.name === "enemy_base") continue;
        ctx.fillText(p.name.replace("spot_", "#").replace("passage_", "pass "), px(p.x) + 7, pz(p.z) - 7);
      }
      ctx.strokeStyle = COLOR.theirs;
      ctx.fillStyle = COLOR.theirs;
      for (const p of call.parties) {
        ctx.setLineDash([3, 3]);
        ctx.beginPath(); ctx.arc(px(p.x), pz(p.z), 12, 0, 7); ctx.stroke();
        ctx.setLineDash([]);
        ctx.fillText(`${p.name} ${p.composition || ""}`, px(p.x) + 14, pz(p.z) - 12);
      }
      ctx.font = "bold 11px system-ui";
      for (const g of call.groups) {
        const members = g.members.map((id) => byId.get(id)).filter(Boolean);
        const at = members.length ? [members.reduce((a, u) => a + u.x, 0) / members.length, members.reduce((a, u) => a + u.z, 0) / members.length] : g.at;
        if (!at) continue;
        const [x, y] = [px(at[0]), pz(at[1])];
        const kind = g.task?.kind || "hold";
        if (g.task?.to && kind !== "hold") {
          ctx.strokeStyle = kind === "engage" ? COLOR.critical : kind === "fight_to" ? COLOR.warning : COLOR.jev;
          ctx.lineWidth = 1.5;
          ctx.setLineDash(kind === "move_to" ? [4, 3] : []);
          ctx.beginPath(); ctx.moveTo(x, y); ctx.lineTo(px(g.task.to[0]), pz(g.task.to[1])); ctx.stroke();
          ctx.setLineDash([]);
        }
        ctx.fillStyle = COLOR.jev;
        ctx.fillText(`${g.name}${members.length > 1 ? ` (${members.length})` : ""} ${kind === "hold" ? "" : kind}`, x + 8, y + 9);
      }
      ctx.font = "11px system-ui";
    }

    if (view.layers.orders) {
      ctx.lineWidth = 1;
      ctx.globalAlpha = 0.6;
      for (const record of WR.range(match.commands, view.frame - ORDER_FRAMES, view.frame)) {
        for (const c of record.c) {
          const unit = byId.get(c[1]);
          const to = c[0] === "build" ? c.slice(3, 5) : c.slice(2, 4);
          if (!unit || to.length < 2 || !ORDER_COLOR[c[0]]) continue;
          ctx.strokeStyle = ORDER_COLOR[c[0]];
          ctx.beginPath(); ctx.moveTo(px(unit.x), pz(unit.z)); ctx.lineTo(px(to[0]), pz(to[1])); ctx.stroke();
        }
      }
      ctx.globalAlpha = 1;
    }

    // Buildings under mobile units, enemies over ours so a raid in the base stays visible.
    const mobileLast = (a, b) => (view.match.defs[a.def]?.speed > 0) - (view.match.defs[b.def]?.speed > 0);
    // Allies first, under our own: the same glyphs in the allied colour. A team game recorded by one seat would
    // otherwise show half our side's map as empty.
    for (const u of [...state.allies].sort(mobileLast)) {
      ctx.globalAlpha = u.flags & WR.FLAG.beingBuilt ? 0.4 : 1;
      glyph(ctx, classOf(u.def), px(u.x), pz(u.z), ALLY);
    }
    for (const u of [...state.own].sort(mobileLast)) {
      ctx.globalAlpha = u.flags & WR.FLAG.beingBuilt ? 0.4 : 1;
      const color = u.flags & WR.FLAG.attacker ? OWN_ATTACKER : u.flags & WR.FLAG.squad ? COLOR.llm : COLOR.ours;
      glyph(ctx, classOf(u.def), px(u.x), pz(u.z), color);
      if (u.damage > 0) {
        ctx.globalAlpha = 1;
        ctx.strokeStyle = COLOR.critical;
        ctx.beginPath(); ctx.arc(px(u.x), pz(u.z), 8, 0, 7); ctx.stroke();
      }
    }
    ctx.globalAlpha = 1;
    for (const e of state.enemies) glyph(ctx, classOf(e.def), px(e.x), pz(e.z), COLOR.theirs);

    if (view.layers.deaths) {
      ctx.lineWidth = 2;
      for (const e of WR.range(match.events, view.frame - DEATH_FRAMES, view.frame)) {
        if ((e.k !== "destroyed" && e.k !== "enemy_destroyed") || (e.x === 0 && e.z === 0)) continue;
        ctx.globalAlpha = 1 - (view.frame - e.f) / DEATH_FRAMES;
        ctx.strokeStyle = e.k === "destroyed" ? COLOR.ours : COLOR.theirs;
        const [x, y] = [px(e.x), pz(e.z)];
        ctx.beginPath();
        ctx.moveTo(x - 5, y - 5); ctx.lineTo(x + 5, y + 5); ctx.moveTo(x + 5, y - 5); ctx.lineTo(x - 5, y + 5);
        ctx.stroke();
      }
      ctx.globalAlpha = 1;
    }
  }

  function mapHover(event) {
    const tip = $("tooltip");
    const box = view.mapBox;
    if (!box || !view.state) return;
    const rect = $("map").getBoundingClientRect();
    const [mx, my] = [event.clientX - rect.left, event.clientY - rect.top];
    const [x, z] = [(mx - box.x) / box.scale, (my - box.y) / box.scale];
    const map = view.match.header.map;
    if (x < 0 || z < 0 || x > map.width || z > map.height) return void (tip.hidden = true);
    const reach = 10 / box.scale;
    let best = null;
    const consider = (u, ours) => {
      const d = Math.hypot(u.x - x, u.z - z);
      if (d < reach && (!best || d < best.d)) best = { d, u, ours };
    };
    view.state.own.forEach((u) => consider(u, true));
    view.state.enemies.forEach((u) => consider(u, false));
    const lines = [`${WR.gridName(view.match, x, z)}  (${Math.round(x)}, ${Math.round(z)})`];
    if (view.terrain) {
      const t = view.terrain;
      const cx = Math.min(t.width - 1, Math.max(0, Math.floor(x / t.cell))), cz = Math.min(t.height - 1, Math.max(0, Math.floor(z / t.cell)));
      const h = t.heights[cz * t.width + cx];
      lines.push(h < 0 ? `water, ${-h} deep` : `height ${h}`);
    }
    if (best) {
      const { u, ours } = best;
      const flags = [];
      if (u.flags & WR.FLAG.beingBuilt) flags.push("being built");
      if (u.flags & WR.FLAG.idle) flags.push("idle");
      if (u.flags & WR.FLAG.attacker) flags.push("attacker");
      if (u.flags & WR.FLAG.squad) flags.push("squad");
      lines.push(`${ours ? "ours" : "enemy"}: ${defName(u.def)} #${u.id}`, ours ? `health ${u.health}%` : `health ${u.health}`);
      if (flags.length) lines.push(flags.join(", "));
    }
    const truthNow = view.layers.truth ? view.match.truth[WR.indexAt(view.match.truth, view.frame)] : null;
    if (truthNow) {
      const near = truthNow.units.filter((u) => Math.hypot(u.x - x, u.z - z) < reach);
      const names = new Map();
      for (const u of near) names.set(u.name, (names.get(u.name) || 0) + 1);
      if (near.length) lines.push(`theirs (truth): ${[...names].map(([n, k]) => `${n} x${k}`).join(", ")}${near.length === 1 ? `, health ${near[0].health}%` : ""}`);
    }
    const census = view.layers.census && !truthNow ? view.match.census[WR.indexAt(view.match.census, view.frame)] : null;
    const groups = census ? census.enemy.filter((g) => Math.hypot(g.x - x, g.z - z) < reach * 1.5) : [];
    if (groups.length) lines.push(`census ${WR.clock(census.f)} (mean positions): ${groups.map((g) => `${g.name} x${g.count}`).join(", ")}`);
    tip.textContent = lines.join("\n");
    tip.hidden = false;
    tip.style.left = `${Math.min(mx + 14, rect.width - 180)}px`;
    tip.style.top = `${my + 14}px`;
  }

  // ---------------------------------------------------------------- timeline

  const GUTTER = 112;
  const LANE_H = 14;
  const CHART_H = 38;
  const LANES = [
    ["losses", "unit losses", "ours"], ["buildingLosses", "building losses", "ours"], ["extractorLosses", "extractor losses", "ours"],
    ["kills", "kills", "theirs"], ["waves", "waves / recalls", "ink"], ["turns", "LLM turns", "llm"],
    ["jevBuilders", "pianist: builders", "jev"], ["jevLabs", "pianist: labs", "jev"], ["jevGroups", "pianist: army", "jev"],
  ];
  // Every chart has its own zero-based axis; "theirs" comes from the census and exists once a minute at most.
  const CHARTS = [
    { label: "metal income", ours: "metalIncome" },
    { label: "energy income", ours: "energyIncome" },
    { label: "extractors", ours: "extractors", theirs: "enemyExtractors" },
    { label: "army", ours: "army", theirs: "enemyArmy" },
  ];

  function timelineGeometry(w) {
    const last = Math.max(view.match.lastFrame, 1);
    return { x0: GUTTER, x1: w - 12, last, fx: (f) => GUTTER + ((w - 12 - GUTTER) * f) / last };
  }

  function drawTimeline() {
    const { ctx, w } = fitCanvas($("timeline"));
    const match = view.match;
    if (!match) return;
    const g = timelineGeometry(w);
    ctx.font = "11px system-ui";
    ctx.textBaseline = "middle";
    let y = 6;

    for (const [key, label, color] of LANES) {
      const items = view.lanes[key];
      if ((key === "turns" || key.startsWith("jev")) && !items.length) continue;
      ctx.fillStyle = COLOR["ink-2"];
      ctx.textAlign = "right";
      ctx.fillText(`${label} (${items.length})`, GUTTER - 8, y + LANE_H / 2);
      ctx.fillStyle = COLOR["surface-2"];
      ctx.fillRect(g.x0, y + 1, g.x1 - g.x0, LANE_H - 2);
      ctx.fillStyle = COLOR[color];
      for (const item of items) {
        ctx.globalAlpha = item.faint ? 0.25 : 1;
        ctx.fillRect(Math.round(g.fx(item.f)) - 1, y + 2, 2, LANE_H - 4);
      }
      ctx.globalAlpha = 1;
      y += LANE_H;
    }
    y += 4;

    // Folded: the lanes and the axis only.
    const compact = document.body.dataset.compact === "1";
    if (compact) view.chartRows = [];

    // Legend, once, for the two-series charts below.
    if (!compact) {
    ctx.textAlign = "left";
    ctx.fillStyle = COLOR["ink-2"];
    ctx.strokeStyle = COLOR.ours;
    ctx.lineWidth = 2;
    ctx.beginPath(); ctx.moveTo(g.x0, y + 6); ctx.lineTo(g.x0 + 18, y + 6); ctx.stroke();
    ctx.fillText("ours (bot's view)", g.x0 + 24, y + 6);
    ctx.fillStyle = COLOR.theirs;
    ctx.beginPath(); ctx.arc(g.x0 + 140, y + 6, 4, 0, 7); ctx.fill();
    ctx.fillStyle = COLOR["ink-2"];
    ctx.fillText(match.truth.length ? "theirs (ground truth)" : match.census.length ? "theirs (census, once a minute)" : "theirs: no census in this match", g.x0 + 150, y + 6);
    y += 14;

    view.chartRows = [];
    for (const chart of CHARTS) {
      const top = y + 4;
      const bottom = y + CHART_H - 2;
      const theirs = chart.theirs ? match.theirs.map((c) => c[chart.theirs]) : [];
      const max = Math.max(1, ...match.series.map((s) => s[chart.ours]), ...theirs);
      const vy = (v) => bottom - ((bottom - top) * v) / max;
      ctx.strokeStyle = COLOR.line;
      ctx.lineWidth = 1;
      ctx.beginPath(); ctx.moveTo(g.x0, bottom + 0.5); ctx.lineTo(g.x1, bottom + 0.5); ctx.stroke();
      ctx.fillStyle = COLOR["ink-2"];
      ctx.textAlign = "right";
      ctx.fillText(chart.label, GUTTER - 8, (top + bottom) / 2);
      ctx.fillStyle = COLOR.muted;
      ctx.fillText(String(Math.round(max)), GUTTER - 8, top + 2);

      ctx.strokeStyle = COLOR.ours;
      ctx.lineWidth = 2;
      ctx.lineJoin = "round";
      ctx.beginPath();
      // One point per horizontal pixel is all the canvas can show.
      const step = Math.max(1, Math.floor(match.series.length / (g.x1 - g.x0)));
      for (let i = 0; i < match.series.length; i += step) {
        const s = match.series[i];
        if (i === 0) ctx.moveTo(g.fx(s.f), vy(s[chart.ours]));
        else ctx.lineTo(g.fx(s.f), vy(s[chart.ours]));
      }
      ctx.stroke();
      if (chart.theirs && match.truth.length) {
        ctx.strokeStyle = COLOR.theirs;
        ctx.lineWidth = 1.5;
        ctx.beginPath();
        match.truth.forEach((c, i) => (i === 0 ? ctx.moveTo(g.fx(c.f), vy(c[chart.theirs])) : ctx.lineTo(g.fx(c.f), vy(c[chart.theirs]))));
        ctx.stroke();
      } else if (chart.theirs) {
        for (const c of match.census) {
          ctx.fillStyle = COLOR.surface;
          ctx.beginPath(); ctx.arc(g.fx(c.f), vy(c[chart.theirs]), 5, 0, 7); ctx.fill();
          ctx.fillStyle = COLOR.theirs;
          ctx.beginPath(); ctx.arc(g.fx(c.f), vy(c[chart.theirs]), 3.5, 0, 7); ctx.fill();
        }
      }
      view.chartRows.push({ chart, top, bottom });
      y += CHART_H;
    }
    }

    // Time axis: a label every few minutes, however long the game.
    const minutes = g.last / (60 * WR.FPS);
    const every = [1, 2, 5, 10, 20].find((n) => minutes / n <= 12) || 30;
    ctx.fillStyle = COLOR.muted;
    ctx.textAlign = "center";
    for (let m = 0; m <= minutes; m += every) {
      const x = Math.round(g.fx(m * 60 * WR.FPS)) + 0.5;
      ctx.strokeStyle = COLOR.line;
      ctx.lineWidth = 1;
      ctx.beginPath(); ctx.moveTo(x, y); ctx.lineTo(x, y + 4); ctx.stroke();
      ctx.fillText(`${m}:00`, x, y + 12);
    }
    view.timelineBottom = y;

    const line = (frame, color) => {
      const x = Math.round(g.fx(frame)) + 0.5;
      ctx.strokeStyle = color;
      ctx.lineWidth = 1;
      ctx.beginPath(); ctx.moveTo(x, 4); ctx.lineTo(x, y); ctx.stroke();
    };
    if (view.hoverFrame != null) line(view.hoverFrame, COLOR.muted);
    line(view.frame, COLOR.ink);
  }

  function timelineFrame(event) {
    const rect = $("timeline").getBoundingClientRect();
    const g = timelineGeometry(rect.width);
    const t = (event.clientX - rect.left - g.x0) / (g.x1 - g.x0);
    return Math.round(Math.min(1, Math.max(0, t)) * g.last);
  }

  function timelineHover(event) {
    if (!view.match) return;
    const frame = timelineFrame(event);
    view.hoverFrame = frame;
    const match = view.match;
    const s = match.series[WR.indexAt(match.series, frame)];
    const c = match.theirs[WR.indexAt(match.theirs, frame)];
    const lines = [WR.clock(frame)];
    if (s) {
      lines.push(`metal +${s.metalIncome.toFixed(1)}   energy +${s.energyIncome.toFixed(0)}`);
      lines.push(`extractors ${s.extractors}${c ? ` / theirs ${c.enemyExtractors}` : ""}`);
      lines.push(`army ${s.army}${c ? ` / theirs ${c.enemyArmy}` : ""}`);
    }
    if (c && !match.truth.length) lines.push(`(census of ${WR.clock(c.f)})`);
    const near = (items) => WR.range(items, frame - 5 * WR.FPS, frame + 5 * WR.FPS);
    const lost = near(view.lanes.losses).concat(near(view.lanes.buildingLosses), near(view.lanes.extractorLosses));
    if (lost.length) lines.push(`lost: ${summarise(lost.map((e) => defName(e.d)))}`);
    const killed = near(view.lanes.kills);
    if (killed.length) lines.push(`killed: ${summarise(killed.map((e) => defName(e.d)))}`);
    for (const d of near(view.lanes.waves)) lines.push(decisionTitle(d));
    const tip = $("chart-tooltip");
    tip.textContent = lines.join("\n");
    tip.hidden = false;
    const rect = $("timeline").getBoundingClientRect();
    const x = event.clientX - rect.left;
    tip.style.left = `${x > rect.width - 260 ? x - 250 : x + 14}px`;
    tip.style.bottom = "24px";
    if (event.buttons & 1) seek(frame);
    else drawTimeline();
  }

  function summarise(names) {
    const counts = new Map();
    for (const n of names) counts.set(n, (counts.get(n) || 0) + 1);
    return [...counts].map(([n, k]) => (k > 1 ? `${n} x${k}` : n)).join(", ");
  }

  // ---------------------------------------------------------------- panels

  function renderNow() {
    const match = view.match;
    const s = match.samples[WR.indexAt(match.samples, view.frame)];
    const series = match.series[WR.indexAt(match.series, view.frame)];
    const census = match.theirs[WR.indexAt(match.theirs, view.frame)];
    const now = $("now");
    now.textContent = "";
    if (!s) return;
    const stat = (label, value, theirs) => {
      const div = document.createElement("div");
      div.className = "stat";
      const b = document.createElement("b");
      b.textContent = value;
      if (theirs != null) {
        const small = document.createElement("small");
        small.textContent = ` / ${theirs}`;
        small.title = "theirs, from the last census";
        b.append(small);
      }
      const span = document.createElement("span");
      span.textContent = label;
      div.append(b, span);
      now.append(div);
    };
    stat(`metal (+${s.m[1].toFixed(1)} / -${s.m[2].toFixed(1)})`, Math.round(s.m[0]));
    stat(`energy of ${s.e[3]} (+${s.e[1].toFixed(0)} / -${s.e[2].toFixed(0)})`, Math.round(s.e[0]));
    stat("extractors", series.extractors, census?.enemyExtractors);
    stat("army", series.army, census?.enemyArmy);
    stat("builders", series.builders);
    stat("buildings", series.buildings);
    stat("enemies in view", series.enemiesVisible);
    // With a commander the game stands still during its turns, and a turn's wall time lands in this number too:
    // it measures the bot's own speed only in heuristic games.
    const turns = view.match.decisions.filter((d) => d.kind === "turn" && d.latency_ms != null && d.f <= view.frame);
    if (turns.length) {
      const seconds = turns.reduce((sum, d) => sum + d.latency_ms / 1000, 0);
      const sorted = turns.map((d) => d.latency_ms / 1000).sort((a, b) => a - b);
      stat("commander turns so far", turns.length);
      stat("thinking, median s a turn", sorted[Math.floor(sorted.length / 2)].toFixed(1));
      // 1.0 would mean it thinks for as long as the game runs: in a live game it would never catch up.
      stat("thinking per game second, s", (seconds / Math.max(1, view.frame / WR.FPS)).toFixed(2));
    } else {
      stat("slowest tick, ms", s.ms.toFixed(1));
    }
  }

  function renderRules() {
    if (view.tab !== "rules") return;
    const rules = WR.rulesInMinute(view.match, view.frame);
    const minute = Math.floor(view.frame / (60 * WR.FPS));
    $("rules-minute").textContent = `(minute ${minute})`;
    const box = $("rules");
    box.textContent = "";
    const max = Math.max(1, ...rules.map(([, n]) => n));
    for (const [rule, n] of rules.slice(0, 10)) {
      const row = document.createElement("div");
      row.className = `rule${rule.startsWith("D-") ? " directive" : ""}`;
      const name = document.createElement("span");
      name.textContent = rule;
      const bar = document.createElement("i");
      bar.style.width = `${(100 * n) / max}%`;
      const count = document.createElement("span");
      count.textContent = n;
      row.append(name, bar, count);
      box.append(row);
    }
    if (!rules.length) box.textContent = "none";
  }

  function decisionTitle(d) {
    const o = d.outputs || {};
    switch (d.kind) {
      case "wave": return `wave ${o.wave}: ${d.inputs.home_group} units to ${o.target.grid}${o.first_stop.grid !== o.target.grid ? `, staging at ${o.first_stop.grid}` : ""}`;
      case "recall": return `recall: ${d.inputs.intruders} enemies at the base, ${o.attackers_called_home} attackers called home`;
      case "assault": return `assault: ${d.inputs.gathered} of ${d.inputs.attackers} gathered, going in`;
      case "event": return String(o);
      case "turn": return d.inputs.wake ? `woken: ${d.inputs.wake}` : "turn";
      case "builder": case "lab": case "group": {
        const kept = o.played !== o.choice;
        const params = [o.where_extractor && o.played === "extractor" ? o.where_extractor : null, o.where && /_to|_at|walk|split/.test(o.played) ? o.where : null, o.whom && o.played === "engage" ? o.whom : null, o.where_scout && o.played === "scout" ? o.where_scout : null].filter(Boolean);
        return `${d.inputs.actor}: ${o.played}${params.length ? ` ${params.join(" ")}` : ""}${kept ? ` (kept; it chose ${o.choice})` : ""}`;
      }
      case "global": return `global: ${Object.entries(o).map(([k, v]) => `${k.replace("global.", "")} ${Number(v).toFixed(2)}`).join(", ")}`;
      default: return `${d.kind}: ${JSON.stringify(o)}`;
    }
  }

  const short = (value, limit) => {
    const text = typeof value === "string" ? value : JSON.stringify(value);
    return text.length > limit ? `${text.slice(0, limit)} ...` : text;
  };

  function el(tag, className, text) {
    const node = document.createElement(tag);
    if (className) node.className = className;
    if (text != null) node.textContent = text;
    return node;
  }

  function details(summary, text) {
    const d = el("details");
    d.append(el("summary", null, summary), el("pre", null, text));
    d.addEventListener("click", (e) => e.stopPropagation());
    return d;
  }

  function buildDecisionList() {
    const list = $("decisions");
    list.textContent = "";
    view.decisionItems = [];
    view.currentDecision = -2;
    const filters = $("decision-filters");
    filters.textContent = "";
    const hasJev = !!view.match.jev || view.match.decisions.some((d) => d.source === "jev");
    const keys = hasJev ? ["heuristic", "llm", "event", "jev", "jevChangesOnly"] : ["heuristic", "llm", "event"];
    const words = { event: "brain events", jev: "pianist", jevChangesOnly: "changes only" };
    for (const key of keys) {
      const label = el("label");
      const box = el("input");
      box.type = "checkbox";
      box.checked = view.show[key];
      box.addEventListener("change", () => {
        view.show[key] = box.checked;
        buildDecisionList();
        renderDecisions();
      });
      label.append(box, ` ${words[key] || key}`);
      filters.append(label);
    }
    if (hasJev) {
      const select = el("select");
      const actors = [...new Set(view.match.decisions.filter((d) => d.source === "jev" && d.inputs?.actor).map((d) => d.inputs.actor))].sort();
      select.append(new Option("every actor", ""));
      for (const a of actors) select.append(new Option(a, a));
      select.value = actors.includes(view.jevActor) ? view.jevActor : "";
      select.addEventListener("change", () => {
        view.jevActor = select.value;
        buildDecisionList();
        renderDecisions();
        renderPianist();
      });
      filters.append(select);
    }
    let shown = 0;
    const LIMIT = 4000;
    for (const d of view.match.decisions) {
      if (d.kind === "rules") continue;
      const jev = d.source === "jev";
      const group = jev ? "jev" : d.kind === "event" ? "event" : d.source.startsWith("llm") ? "llm" : "heuristic";
      if (!view.show[group]) continue;
      if (jev) {
        const o = d.outputs || {};
        if (d.kind === "global") continue;
        if (view.jevActor && d.inputs?.actor !== view.jevActor) continue;
        if (view.show.jevChangesOnly && (o.played === "continue" || o.played === "nothing" || o.played === "wait")) continue;
      }
      if (++shown > LIMIT) break;
      const li = el("li", jev ? `jev${d.outputs?.played !== d.outputs?.choice ? " kept" : ""}` : d.source.startsWith("llm") ? "llm" : "heuristic");
      const head = el("div");
      head.append(el("span", "when", WR.clock(d.f)), el("span", d.kind === "turn" ? "wake" : null, decisionTitle(d)), el("span", "source", d.source));
      if (jev && d.outputs) {
        head.append(el("span", "p", `p ${Number(d.outputs.probability).toFixed(2)} c ${Number(d.outputs.confidence).toFixed(2)}`));
        if (d.outputs.did) head.append(el("div", "did", d.outputs.did));
      }
      li.append(head);
      if (d.kind === "turn") {
        for (const text of d.outputs.said) li.append(el("div", "said", text));
        for (const call of d.outputs.calls) {
          const row = el("div", "call");
          row.append(el("b", null, call.tool), `(${short(call.arguments, 400)})`);
          const result = short(call.result, 160);
          row.append(el("span", "result", `  -> ${result}`));
          li.append(row);
          if (result.endsWith(" ...")) li.append(details(`full result of ${call.tool}`, JSON.stringify(call.result, null, 1)));
        }
        if (d.outputs.thinking.length) li.append(details("thinking", d.outputs.thinking.join("\n\n")));
        li.append(details("prompt", d.inputs.prompt));
        const meta = [];
        if (d.latency_ms != null) meta.push(`${(d.latency_ms / 1000).toFixed(1)} s wall`);
        if (d.cost != null) meta.push(`$${d.cost.toFixed(3)} list price`);
        if (d.outputs.stopped) meta.push(`stopped: ${d.outputs.stopped}`);
        if (meta.length) li.append(el("div", "source", meta.join(" · ")));
      }
      li.addEventListener("click", () => seek(d.f));
      list.append(li);
      view.decisionItems.push({ f: d.f, li });
    }
    if (shown > LIMIT) list.append(el("li", null, `... ${view.match.decisions.length - LIMIT} more; narrow the filters`));
    if (!view.decisionItems.length) list.append(el("li", null, "no decision records of the selected kinds"));
  }

  // ---------------------------------------------------------------- the pianist

  function setTab(name) {
    view.tab = name;
    for (const button of document.querySelectorAll("#tabs [data-tab]")) button.classList.toggle("active", button.dataset.tab === name);
    for (const tab of document.querySelectorAll(".tab")) tab.hidden = tab.dataset.tab !== name;
    try { localStorage.setItem("wr-tab", name); } catch (_) { /* no storage */ }
    view.callShown = null;
    view.currentDecision = -2;
    renderAll();
  }

  /// Each actor as the call at the playhead saw it, with its last decision; opened, its history and its entry.
  function renderPianist() {
    const jev = view.match.jev;
    if (!jev || view.tab !== "pianist") return;
    const i = WR.indexAt(jev.calls, view.frame);
    const call = i >= 0 ? jev.calls[i] : null;
    $("pianist-summary").textContent = call ? `call ${i + 1} of ${jev.calls.length} at ${WR.clock(call.f)}` : "before the first call";
    const box = $("pianist-actors");
    box.textContent = "";
    if (!call) return;
    const actors = call.state.actors || {};
    const decisionLine = (d, withClock) => {
      const line = el("span", "hist");
      if (withClock) line.append(`${WR.clock(d.f)}  `);
      line.append(el("b", null, d.played));
      if (d.kept) line.append(el("span", "kept", ` kept (it chose ${d.choice})`));
      line.append(` p ${Number(d.probability).toFixed(2)} c ${Number(d.confidence).toFixed(2)}`);
      if (d.did) line.append(` · ${d.did}`);
      return line;
    };
    for (const [name, entry] of Object.entries(actors)) {
      const selected = view.jevActor === name;
      const row = el("div", `actor${selected ? " selected" : ""}`);
      row.append(el("b", null, name), el("span", "doing", `${entry.doing || ""}${entry.enemies_near ? ` · ${entry.enemies_near}` : ""}${entry.under_fire ? " · UNDER FIRE" : ""}`));
      const history = jev.actors.get(name);
      const at = history ? WR.indexAt(history.decisions, view.frame) : -1;
      const last = at >= 0 ? history.decisions[at] : null;
      const line = el("div", "last");
      if (last) {
        line.append(decisionLine(last, false), el("span", "ago", `  ${WR.clock(last.f)}`));
      } else {
        line.append("not asked yet");
      }
      row.append(line);
      if (selected) {
        const more = el("div", "more");
        more.append(el("div", "meta", "its entry in the picture:"), el("pre", null, JSON.stringify(entry, null, 1)));
        if (history && at >= 0) {
          more.append(el("div", "meta", "its decisions up to now, newest first:"));
          for (const d of history.decisions.slice(Math.max(0, at - 11), at + 1).reverse()) {
            const item = el("div");
            item.append(decisionLine(d, true));
            more.append(item);
          }
        }
        more.addEventListener("click", (e) => e.stopPropagation());
        row.append(more);
      }
      row.addEventListener("click", () => {
        view.jevActor = selected ? "" : name;
        buildDecisionList();
        renderDecisions();
        renderPianist();
      });
      box.append(row);
    }
    renderCall(call, i);
  }

  /// The call at the playhead: what Jev was shown and what it answered; rebuilt when the call changes. With an actor
  /// selected, its questions come first.
  function renderCall(call, index) {
    $("call-summary").textContent = `${WR.clock(call.f)} · ${call.ms} ms · ${call.tokens.toLocaleString()} tokens · ${Object.keys(call.questions).length} questions${call.retries ? ` · ${call.retries} retries` : ""}`;
    const key = `${index}:${view.jevActor}`;
    if (view.callShown === key) return;
    view.callShown = key;
    const box = $("call");
    box.textContent = "";
    const questions = el("div", "questions");
    const picture = el("div", "picture");
    box.append(questions, picture);
    const played = new Map(call.played.map((d) => [d.actor, d]));
    const ordered = Object.entries(call.questions).sort(([a], [b]) => (b.startsWith(`${view.jevActor}.`) - a.startsWith(`${view.jevActor}.`)));
    for (const [id, q] of ordered) {
      const [actor, what] = id.split(".");
      const a = call.answers[id];
      const block = el("div", "q");
      block.append(el("div", "id", id));
      const ask = typeof q.instructions === "string" ? q.instructions : JSON.stringify(q.instructions);
      block.append(el("div", "ask", short(ask, 220)));
      if (!a) {
        block.append(el("div", "ask", "no answer"));
      } else if (a.type === "noul") {
        block.append(bar("yes", a.noul, false, 1));
      } else {
        const chosen = (what === "do" || what === "next") ? played.get(actor) : null;
        const ranked = Object.entries(a.probabilities || {}).sort((x, y) => y[1] - x[1]).slice(0, 6);
        for (const [option, p] of ranked) block.append(bar(option, p, chosen ? option === chosen.played : option === a.choice, ranked[0][1]));
        if (chosen?.kept) block.append(el("div", "ask", `kept its course: ${chosen.choice} did not beat continue by the margin`));
        const rest = Object.keys(a.probabilities || {}).length - ranked.length;
        if (rest > 0) block.append(el("div", "ask", `and ${rest} more under ${(ranked[ranked.length - 1][1]).toFixed(2)}`));
        const crit = q.criteria && typeof q.criteria === "object" && !Array.isArray(q.criteria) ? q.criteria : null;
        if (crit) block.append(details("the options as worded", Object.entries(crit).map(([k, v]) => `${k}: ${typeof v === "string" ? v : JSON.stringify(v)}`).join("\n")));
      }
      questions.append(block);
    }
    picture.append(el("div", "meta", `the picture Jev was shown (${call.model || "Jev"}):`));
    picture.append(details("instructions", call.instructions || "(none)"));
    for (const section of ["economy", "ours", "enemy", "actors", "places", "recent"]) {
      if (call.state[section] !== undefined) picture.append(details(section, JSON.stringify(call.state[section], null, 1)));
    }
    picture.append(details("rules", call.rules || "(none)"));
  }

  function bar(label, p, played, top) {
    const row = el("div", `bar${played ? " played" : ""}`);
    const fill = el("i");
    fill.style.width = `${Math.max(1, (100 * p) / Math.max(top, 1e-6))}%`;
    row.append(el("span", null, label), fill, el("span", null, Number(p).toFixed(2)));
    return row;
  }

  function buildPianistMinutes(jev) {
    const table = $("pianist-minutes");
    table.textContent = "";
    const head = el("tr");
    for (const h of ["minute", "calls", "median ms", "max ms", "tokens", "questions", "changes", "kept", "failed"]) head.append(el("th", null, h));
    table.append(head);
    for (const r of WR.jevMinutes(jev)) {
      const tr = el("tr");
      for (const v of [r.minute, r.calls, r.medianMs, r.maxMs, r.tokens.toLocaleString(), r.questions, r.changes, r.kept, r.errors]) tr.append(el("td", null, String(v)));
      table.append(tr);
    }
  }

  /// Rebuilds the list for a match that has grown, keeping the opened details open and the scroll position.
  function rebuildDecisionList() {
    const list = $("decisions");
    const key = (d) => `${d.closest("li").querySelector(".when").textContent} ${d.querySelector("summary").textContent}`;
    const opened = new Set([...list.querySelectorAll("details[open]")].map(key));
    const scroll = list.scrollTop;
    const current = view.currentDecision;
    buildDecisionList();
    for (const d of list.querySelectorAll("details")) if (opened.has(key(d))) d.open = true;
    // Following the newest decision scrolls by itself (renderDecisions); otherwise stay where the reader was.
    if (!$("follow").checked) {
      view.currentDecision = current;
      list.scrollTop = scroll;
      view.decisionItems.forEach((item, i) => {
        item.li.classList.toggle("future", i > current);
        item.li.classList.toggle("current", i === current);
      });
    }
  }

  function renderDecisions() {
    if (view.tab !== "decisions") return;
    const items = view.decisionItems;
    const current = WR.indexAt(items, view.frame);
    if (current === view.currentDecision) return;
    view.currentDecision = current;
    items.forEach((item, i) => {
      item.li.classList.toggle("future", i > current);
      item.li.classList.toggle("current", i === current);
    });
    if (current >= 0) {
      // Scroll inside the list only; scrollIntoView would move the page too.
      const list = $("decisions");
      const li = items[current].li;
      list.scrollTop = li.offsetTop - list.offsetTop - 24;
    }
  }

  function renderBotLog() {
    if (view.tab !== "rules") return;
    const lines = WR.range(view.match.botLog, view.frame - 60 * WR.FPS, view.frame).slice(-30);
    $("botlog").textContent = lines.length ? lines.map((l) => `${WR.clock(l.f)}  ${l.text}`).join("\n") : view.match.botLog.length ? "" : "bot.log not loaded";
  }

  // ---------------------------------------------------------------- playhead

  function renderAll() {
    if (!view.match) return;
    $("clock").textContent = WR.clock(view.frame);
    drawMap();
    drawTimeline();
    renderNow();
    renderRules();
    renderDecisions();
    renderBotLog();
    renderPianist();
  }

  function seek(frame) {
    view.frame = Math.min(Math.max(0, frame), view.match ? view.match.lastFrame : 0);
    renderAll();
  }

  function setPlaying(playing) {
    view.playing = playing && !!view.match;
    $("play").textContent = view.playing ? "Pause" : "Play";
    if (view.playing) {
      if (view.frame >= view.match.lastFrame) view.frame = 0;
      view.lastTime = performance.now();
      requestAnimationFrame(step);
    }
  }

  function step(now) {
    if (!view.playing) return;
    const elapsed = Math.min(0.1, (now - view.lastTime) / 1000);
    view.lastTime = now;
    seek(view.frame + elapsed * WR.FPS * view.speed);
    if (view.frame >= view.match.lastFrame) return setPlaying(false);
    requestAnimationFrame(step);
  }

  for (const button of document.querySelectorAll("#tabs [data-tab]")) button.addEventListener("click", () => setTab(button.dataset.tab));
  const setWide = (wide) => {
    document.body.dataset.wide = wide ? "1" : "";
    try { localStorage.setItem("wr-wide", wide ? "1" : ""); } catch (_) { /* no storage */ }
    renderAll();
  };
  $("wide").addEventListener("click", () => setWide(document.body.dataset.wide !== "1"));
  const setCompact = (compact) => {
    document.body.dataset.compact = compact ? "1" : "";
    try { localStorage.setItem("wr-compact", compact ? "1" : ""); } catch (_) { /* no storage */ }
    renderAll();
  };
  $("compact").addEventListener("click", () => setCompact(document.body.dataset.compact !== "1"));
  try { if (localStorage.getItem("wr-compact") === "1") document.body.dataset.compact = "1"; } catch (_) { /* no storage */ }
  try { if (localStorage.getItem("wr-wide") === "1") document.body.dataset.wide = "1"; } catch (_) { /* no storage */ }
  $("play").addEventListener("click", () => setPlaying(!view.playing));
  // Going anywhere by hand stops following the live match; ticking the box again jumps back to the newest sample.
  const leaveLive = () => ($("follow").checked = false);
  $("follow").addEventListener("change", (e) => e.target.checked && view.match && seek(view.match.lastFrame));
  $("play").addEventListener("click", leaveLive);
  $("back").addEventListener("click", leaveLive);
  $("timeline").addEventListener("mousedown", leaveLive);
  $("back").addEventListener("click", () => seek(view.frame - 10 * WR.FPS));
  $("forward").addEventListener("click", () => seek(view.frame + 10 * WR.FPS));
  $("forward").addEventListener("click", leaveLive);
  $("speed").addEventListener("change", (e) => (view.speed = Number(e.target.value)));
  $("files").addEventListener("change", (e) => openFiles(e.target.files));
  $("timeline").addEventListener("mousedown", (e) => view.match && seek(timelineFrame(e)));
  $("timeline").addEventListener("mousemove", timelineHover);
  $("timeline").addEventListener("mouseleave", () => {
    view.hoverFrame = null;
    $("chart-tooltip").hidden = true;
    drawTimeline();
  });
  $("map").addEventListener("mousemove", mapHover);
  $("map").addEventListener("mouseleave", () => ($("tooltip").hidden = true));
  for (const box of document.querySelectorAll("#layers input")) {
    box.addEventListener("change", () => {
      view.layers[box.dataset.layer] = box.checked;
      drawMap();
    });
  }
  document.addEventListener("keydown", (e) => {
    if (e.target.tagName === "INPUT" || e.target.tagName === "SELECT") return;
    if (e.key === " ") setPlaying(!view.playing);
    else if (e.key === "ArrowLeft") seek(view.frame - (e.shiftKey ? 60 : 10) * WR.FPS);
    else if (e.key === "ArrowRight") seek(view.frame + (e.shiftKey ? 60 : 10) * WR.FPS);
    else if (e.key === "w") return setWide(document.body.dataset.wide !== "1");
    else if (e.key === "c") return setCompact(document.body.dataset.compact !== "1");
    else return;
    leaveLive();
    e.preventDefault();
  });
  new ResizeObserver(() => renderAll()).observe(document.body);

  boot();
})();
