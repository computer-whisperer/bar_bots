#!/usr/bin/env python3
"""Tempo model: from what our bot legitimately saw, estimate the opponent's economy and army right now.

usage: run/tempo_model.py [--matches run/matches] [--dataset PATH] [--params PATH] [--extract] [--jobs N]
                          [--map "Quicksilver Remake 1.24"] [--folds 5]

Two halves.

  extract   Walks match directories that have both a record (`record-<ai>.jsonl`, our fair view) and a truth file
            (`truth-<ai>.jsonl`, every enemy unit every two seconds). Every 30 game seconds it writes one row: the
            features our bot could have computed at that instant, and the opponent's real state at that instant.
            Rows go to a gzipped CSV so the fit does not have to re-read 3 GB of JSON.

  fit       Fits and evaluates, held out by BATCH (never by sample inside a game: samples 30 s apart in one match are
            nearly the same data point, and games in a batch share a bot version). Compares four estimators --
            time alone, what we have seen alive, the brain's present estimate, and the fitted model -- reports error
            by game minute, checks that the stated interval really covers what it claims out of fold, and writes the
            fitted numbers as JSON.

Opponent metal income is not in the truth file. The income target here is a PROXY: finished extractor counts times a
metal-per-extractor rate calibrated per map from OUR OWN income and OUR OWN extractor count in the same games, plus
converters at their nominal full-load rate. Read every income number as "extractor-equivalents", not as the engine's
number.
"""
import argparse
import csv
import glob
import gzip
import json
import os
import sys
from concurrent.futures import ProcessPoolExecutor

import numpy as np

STEP = 30  # game seconds between rows
FIRST_ROW = 60  # first row at this game second; before it the opponent is a commander and two extractors
RECENT_WINDOW = 120  # seconds, the "biggest force seen lately" window (brain/army.rs ENEMY_ARMY_MEMORY_FRAMES)
FPS = 30

# Converter output at full load, from upstream/Beyond-All-Reason (armmakr.lua, armmmkr.lua: energyconv_capacity x
# energyconv_efficiency). An idle converter makes nothing, so these are ceilings.
CONVERTER_METAL = 1.0
ADV_CONVERTER_METAL = 10.34
# An advanced extractor's extractsmetal is 0.004 against the tier-1 extractor's 0.001 on the same spot (armmoho.lua,
# armmex.lua), so it is worth four of them wherever it stands.
ADV_EXTRACTOR_RATIO = 4.0

# Cost thresholds that separate tier 2 from tier 1 inside a class. Factories: T1 labs are 470-570 metal, advanced
# labs 2600. Extractors: 50 against 620-640. Builders: T1 constructors 110-130, advanced 430-470.
T2_FACTORY_METAL = 1500
T2_EXTRACTOR_METAL = 300
T2_BUILDER_METAL = 300

# Mobile units that only a tier-2 lab can produce. Derived, not assumed: over the 144 games of v31/v32/v34 every
# `army` unit name in the truth files was counted before and after the opponent's first FINISHED advanced lab; these
# are the names with zero sightings before it and at least a hundred after. Names that appear before a T2 lab and are
# T2 elsewhere (armfido, armpship) are the opponent's resurrection bots raising OUR dead (K-army-dead-waves-are-
# resurrected), so a unit sighting alone does not prove the opponent has a lab -- the target below uses the building.
T2_ARMY = {
    "armaak", "armamph", "armbull", "armfast", "armfboy", "armlatnk", "armmanni", "armmart", "armmav", "armmerl",
    "armsnipe", "armsptk", "armvader", "armzeus", "coraak", "coramph", "corban", "corcan", "coresupp", "corgol",
    "corhrk", "corkarg", "cormort", "corpship", "corpyro", "correap", "corshiva", "corsktl", "corsumo", "cortermite",
}

# Evidence. `f_min` is not among them: the baseline is the time-only model, and these are what tilts it away from
# the typical curve for this minute.
FEATURES = [
    "f_seen_army_n", "f_seen_army_m", "f_live_army_m",
    "f_now_army_m", "f_max_army_m", "f_rec_army_m",
    "f_live_mex_n", "f_live_mex_m",
    "f_live_turret_m", "f_live_fact_n", "f_live_bld_m", "f_live_builder_n",
    "f_kills_m", "f_lost_m", "f_lost_army_m", "f_dmg",
    "f_t2_flag", "f_t2_age",
    "f_radar_n",
    "f_corner_nw",
    "f_own_mex", "f_own_army_m", "f_own_income",
]
# Evidence about the opponent alone: the three `f_own_*` features are about us, and the wave gate that will read
# this model is itself a consumer of our own state, so the study reports both sets.
ENEMY_ONLY = [f for f in FEATURES if not f.startswith("f_own_")]
META = ["batch", "game", "map", "mode", "side", "corner", "outcome", "t"]
RAW_TARGETS = ["y_t1mex", "y_t2mex", "y_conv1", "y_conv2", "y_army_m", "y_def_m", "y_t2lab"]
OWN_ECON = ["own_t1mex", "own_t2mex", "own_conv1", "own_conv2", "own_income"]
COLUMNS = META + ["m_spots", "f_min"] + FEATURES + RAW_TARGETS + OWN_ECON

BANDS = [(1, 5), (5, 8), (8, 11), (11, 15), (15, 20), (20, 40)]
DEFAULT_MAP = "Quicksilver Remake 1.24"  # 430 of the 511 games with ground truth; the other four maps have 16 each


# ----------------------------------------------------------------------------------------------- extraction


def is_converter(name):
    return name.endswith("makr") or name.endswith("mmkr") or name.endswith("fmkr") or name.endswith("gmm")


def is_adv_converter(name):
    return name.endswith("mmkr") or name.endswith("gmm")


def def_table(header):
    """index -> (name, class, metal, tier2)."""
    by_index, by_name = [], {}
    for d in header["unit_defs"]:
        name, klass, metal = d["name"], d["class"], d["metal"]
        t2 = (
            (klass == "factory" and metal >= T2_FACTORY_METAL)
            or (klass == "extractor" and metal >= T2_EXTRACTOR_METAL)
            or (klass == "builder" and metal >= T2_BUILDER_METAL)
            or (klass == "army" and name in T2_ARMY)
            or is_adv_converter(name)
        )
        entry = (name, klass, metal, t2)
        by_index.append(entry)
        by_name[name] = entry
    return by_index, by_name


def read_truth(path, by_name):
    """second -> the opponent's real state, from the ground-truth file."""
    out = {}
    for raw in open(path):
        try:
            line = json.loads(raw)
        except ValueError:
            continue
        t1mex = t2mex = conv1 = conv2 = 0
        army_m = def_m = 0.0
        t2lab = 0
        for unit in line["enemy"]:
            if unit[5]:  # still being built
                continue
            entry = by_name.get(unit[1])
            if entry is None:
                continue
            name, klass, metal, _ = entry
            if klass == "extractor":
                if metal >= T2_EXTRACTOR_METAL:
                    t2mex += 1
                else:
                    t1mex += 1
            elif klass == "army":
                army_m += metal
            elif klass == "turret":
                def_m += metal
            elif klass == "factory" and metal >= T2_FACTORY_METAL:
                t2lab = 1
            if is_converter(name):
                if is_adv_converter(name):
                    conv2 += 1
                else:
                    conv1 += 1
        out[line["f"] // FPS] = (t1mex, t2mex, conv1, conv2, army_m, def_m, t2lab)
    return out


class View:
    """What the bot could know about the opponent, swept forward frame by frame."""

    def __init__(self, by_index):
        self.defs = by_index
        self.id_def = {}  # enemy id -> def index, from the first look that identified it
        self.seen = set()  # every enemy id ever in sight or on radar
        self.killed = set()
        self.seen_army_n = 0
        self.seen_army_m = 0.0
        self.live_army_m = 0.0
        self.live_mex_n = 0
        self.live_mex_m = 0.0
        self.live_turret_m = 0.0
        self.live_fact_n = 0
        self.live_bld_m = 0.0
        self.live_builder_n = 0
        self.kills_m = 0.0
        self.lost_m = 0.0
        self.lost_army_m = 0.0
        self.dmg = 0.0
        self.t2_seen_at = None
        self.now_army_m = 0.0
        self.max_army_m = 0.0
        self.recent = []  # (second, army metal in sight) over the rolling window

    def identify(self, uid, index, second):
        if uid in self.id_def or index is None or index < 0 or index >= len(self.defs):
            return
        self.id_def[uid] = index
        _, klass, metal, t2 = self.defs[index]
        if t2 and self.t2_seen_at is None:
            self.t2_seen_at = second
        if uid in self.killed:  # identified only by the kill event: it never counted as alive
            self.kills_m += metal
            if klass == "army":
                self.seen_army_n += 1
                self.seen_army_m += metal
            return
        if klass == "army":
            self.seen_army_n += 1
            self.seen_army_m += metal
            self.live_army_m += metal
        elif klass == "extractor":
            self.live_mex_n += 1
            self.live_mex_m += metal
        elif klass == "turret":
            self.live_turret_m += metal
        elif klass == "factory":
            self.live_fact_n += 1
        elif klass == "builder":
            self.live_builder_n += 1
        elif klass == "building":
            self.live_bld_m += metal

    def enemy_destroyed(self, uid, index, second):
        if uid in self.killed:
            return
        self.killed.add(uid)
        self.seen.add(uid)
        if uid not in self.id_def:
            self.identify(uid, index, second)
            return
        _, klass, metal, _ = self.defs[self.id_def[uid]]
        self.kills_m += metal
        if klass == "army":
            self.live_army_m -= metal
        elif klass == "extractor":
            self.live_mex_n -= 1
            self.live_mex_m -= metal
        elif klass == "turret":
            self.live_turret_m -= metal
        elif klass == "factory":
            self.live_fact_n -= 1
        elif klass == "builder":
            self.live_builder_n -= 1
        elif klass == "building":
            self.live_bld_m -= metal

    def sample(self, second, enemies):
        army_m = 0.0
        for e in enemies:
            uid, index = e[0], e[1]
            self.seen.add(uid)
            self.identify(uid, index, second)
            known = self.id_def.get(uid)
            if known is not None and self.defs[known][1] == "army":
                army_m += self.defs[known][2]
        self.now_army_m = army_m
        self.max_army_m = max(self.max_army_m, army_m)
        self.recent.append((second, army_m))
        cut = second - RECENT_WINDOW
        while self.recent and self.recent[0][0] < cut:
            self.recent.pop(0)

    def row(self, second):
        return {
            "f_min": second / 60.0,
            "f_seen_army_n": self.seen_army_n,
            "f_seen_army_m": self.seen_army_m,
            "f_live_army_m": max(0.0, self.live_army_m),
            "f_now_army_m": self.now_army_m,
            "f_max_army_m": self.max_army_m,
            "f_rec_army_m": max((m for _, m in self.recent), default=0.0),
            "f_live_mex_n": max(0, self.live_mex_n),
            "f_live_mex_m": max(0.0, self.live_mex_m),
            "f_live_turret_m": max(0.0, self.live_turret_m),
            "f_live_fact_n": max(0, self.live_fact_n),
            "f_live_bld_m": max(0.0, self.live_bld_m),
            "f_live_builder_n": max(0, self.live_builder_n),
            "f_kills_m": self.kills_m,
            "f_lost_m": self.lost_m,
            "f_lost_army_m": self.lost_army_m,
            "f_dmg": self.dmg,
            "f_t2_flag": 0 if self.t2_seen_at is None else 1,
            "f_t2_age": 0.0 if self.t2_seen_at is None else (second - self.t2_seen_at) / 60.0,
            "f_radar_n": len(self.seen) - len(self.id_def),
        }


def own_state(own, by_index):
    """Our own finished economy and army from a state sample's `own` list."""
    t1mex = t2mex = conv1 = conv2 = 0
    army_m = 0.0
    for u in own:
        if u[5] & 1:  # being built
            continue
        index = u[1]
        if index < 0 or index >= len(by_index):
            continue
        name, klass, metal, _ = by_index[index]
        if klass == "extractor":
            if metal >= T2_EXTRACTOR_METAL:
                t2mex += 1
            else:
                t1mex += 1
        elif klass == "army":
            army_m += metal
        if is_converter(name):
            if is_adv_converter(name):
                conv2 += 1
            else:
                conv1 += 1
    return t1mex, t2mex, conv1, conv2, army_m


def extract_match(match):
    """One match directory -> a list of rows, or []."""
    records = sorted(glob.glob(os.path.join(match, "record-*.jsonl")))
    truths = sorted(glob.glob(os.path.join(match, "truth-*.jsonl")))
    if not records or not truths:
        return []
    batch = os.path.basename(os.path.dirname(match))
    game = os.path.basename(match)
    header = None
    with open(records[0]) as f:
        for raw in f:
            try:
                header = json.loads(raw)
            except ValueError:
                continue
            break
    if not header or header.get("t") != "header":
        return []
    by_index, by_name = def_table(header)
    truth = read_truth(truths[0], by_name)
    if not truth:
        return []
    view = View(by_index)
    rows = []
    next_row = FIRST_ROW
    own = (0, 0, 0, 0, 0.0)
    income = 0.0
    last_truth = None
    known_own = {}  # our unit id -> def index, so a destroyed unit's cost is known

    def emit(second):
        state = truth.get(second) or truth.get(second - 1) or truth.get(second + 1) or last_truth
        if state is None:
            return
        t1mex, t2mex, conv1, conv2, army_m, def_m, t2lab = state
        row = {"batch": batch, "game": game, "map": header["map"]["name"], "mode": header.get("mode", ""),
               "side": header.get("side", ""), "corner": "", "outcome": "", "t": second,
               "m_spots": len(header.get("metal_spots") or [])}
        row.update(view.row(second))
        row["f_own_mex"] = own[0] + own[1]
        row["f_own_army_m"] = own[4]
        row["f_own_income"] = income
        row["f_corner_nw"] = 0.0  # which start we drew; set from the result line once the match ends
        row.update({"y_t1mex": t1mex, "y_t2mex": t2mex, "y_conv1": conv1, "y_conv2": conv2,
                    "y_army_m": army_m, "y_def_m": def_m, "y_t2lab": t2lab})
        row.update({"own_t1mex": own[0], "own_t2mex": own[1], "own_conv1": own[2], "own_conv2": own[3],
                    "own_income": income})
        rows.append(row)

    with open(records[0]) as f:
        for raw in f:
            try:
                line = json.loads(raw)
            except ValueError:
                continue
            kind = line.get("t")
            if kind == "result":
                outcome = line.get("result", {}).get("outcome", "")
                corner = line.get("result", {}).get("our_corner", "")
                for row in rows:
                    row["outcome"], row["corner"] = outcome, corner
                    row["f_corner_nw"] = 1.0 if corner == "NW" else 0.0
                continue
            frame = line.get("f")
            if frame is None:
                continue
            second = frame // FPS
            while second >= next_row:
                emit(next_row)
                next_row += STEP
            if kind == "s":
                view.sample(second, line.get("en") or [])
                own = own_state(line.get("own") or [], by_index)
                income = (line.get("m") or [0, 0])[1]
                for u in line.get("own") or []:
                    known_own[u[0]] = u[1]
                for _, amount in line.get("dmg") or []:
                    view.dmg += amount
                if truth.get(second):
                    last_truth = truth[second]
            elif kind == "ev":
                k = line.get("k")
                if k == "enemy_seen":
                    view.seen.add(line["u"])
                    view.identify(line["u"], line.get("d"), second)
                elif k == "enemy_destroyed":
                    view.enemy_destroyed(line["u"], line.get("d"), second)
                elif k == "destroyed":
                    index = known_own.pop(line["u"], line.get("d"))
                    if index is not None and 0 <= index < len(by_index):
                        _, klass, metal, _ = by_index[index]
                        view.lost_m += metal
                        if klass == "army":
                            view.lost_army_m += metal
    return rows


def extract(match_root, dataset, jobs):
    matches = []
    for batch in sorted(glob.glob(os.path.join(match_root, "*"))):
        matches += sorted(glob.glob(os.path.join(batch, "[0-9][0-9]")))
    matches = [m for m in matches if glob.glob(os.path.join(m, "truth-*.jsonl"))]
    print(f"extracting from {len(matches)} matches with a record and a truth file", file=sys.stderr)
    os.makedirs(os.path.dirname(dataset), exist_ok=True)
    written = 0
    with gzip.open(dataset, "wt", newline="") as out:
        writer = csv.DictWriter(out, fieldnames=COLUMNS)
        writer.writeheader()
        with ProcessPoolExecutor(max_workers=jobs) as pool:
            for n, rows in enumerate(pool.map(extract_match, matches, chunksize=4)):
                for row in rows:
                    writer.writerow(row)
                written += len(rows)
                if (n + 1) % 100 == 0:
                    print(f"  {n + 1}/{len(matches)} matches, {written} rows", file=sys.stderr)
    print(f"wrote {written} rows to {dataset}", file=sys.stderr)


# ----------------------------------------------------------------------------------------------- fitting

MINUTES = BANDS[-1][1]


def load(dataset):
    with gzip.open(dataset, "rt", newline="") as f:
        rows = list(csv.DictReader(f))
    for row in rows:
        for key in row:
            if key not in META:
                row[key] = float(row[key])
        row["t"] = int(row["t"])
    return rows


def minute_of(row):
    return min(int(row["t"] // 60), MINUTES - 1)


def mex_rate(rows):
    """Metal per finished tier-1 extractor, per map, fitted on OUR OWN income in these same games.

    Least squares with no intercept on samples from minute 3 on that had no converter and no advanced extractor
    running, so the only metal source besides extraction is reclaim. Our reclaim is real income but it is not the
    opponent's, so this rate over-states a bare extractor; `early` repeats the fit on minutes 3-6, where there is much
    less wreckage about, and the gap between the two is the size of that contamination.
    """
    out = {}
    by_map = {}
    for row in rows:
        by_map.setdefault(row["map"], []).append(row)
    for name, group in sorted(by_map.items()):
        clean = [r for r in group if r["t"] >= 180 and r["own_conv1"] == 0 and r["own_conv2"] == 0
                 and r["own_t2mex"] == 0 and r["own_t1mex"] > 0]
        if len(clean) < 100:
            clean = [r for r in group if r["t"] >= 180 and r["own_t1mex"] > 0]
        a = np.array([[r["own_t1mex"]] for r in clean], float)
        y = np.array([r["own_income"] for r in clean], float)
        coef = float(np.linalg.lstsq(a, y, rcond=None)[0][0])
        resid = y - a[:, 0] * coef
        early = [r for r in clean if r["t"] <= 360]
        early_coef = coef
        if len(early) >= 50:
            ae = np.array([[r["own_t1mex"]] for r in early], float)
            early_coef = float(np.linalg.lstsq(ae, np.array([r["own_income"] for r in early], float), rcond=None)[0][0])
        out[name] = {"t1": coef, "t2": coef * ADV_EXTRACTOR_RATIO, "early_t1": early_coef, "samples": len(clean),
                     "r2": float(1 - resid.var() / y.var()), "mad": float(np.median(np.abs(resid)))}
    return out


def add_derived(rows, rates):
    fallback = next(iter(rates.values()))
    for row in rows:
        rate = rates.get(row["map"], fallback)
        row["y_mex"] = row["y_t1mex"] + row["y_t2mex"]
        row["y_income"] = (rate["t1"] * row["y_t1mex"] + rate["t2"] * row["y_t2mex"]
                           + CONVERTER_METAL * row["y_conv1"] + ADV_CONVERTER_METAL * row["y_conv2"])


class Frame:
    """Rows as arrays: the evidence, the targets, the game minute and the batch each row came from."""

    def __init__(self, rows, targets):
        self.n = len(rows)
        self.minute = np.array([minute_of(r) for r in rows], int)
        self.x = np.array([[r[f] for f in FEATURES] for r in rows], float)
        self.y = {t: np.array([r[t] for r in rows], float) for t in targets}
        self.batch = np.array([r["batch"] for r in rows])
        self.spots = np.array([r["m_spots"] for r in rows], float)

    def take(self, pick):
        out = Frame.__new__(Frame)
        out.n = int(pick.sum()) if pick.dtype == bool else len(pick)
        out.minute, out.x, out.spots = self.minute[pick], self.x[pick], self.spots[pick]
        out.y = {k: v[pick] for k, v in self.y.items()}
        out.batch = self.batch[pick]
        return out


def per_minute(values, minutes):
    """(MINUTES, k) mean and standard deviation, gaps filled from the nearest minute that has data."""
    k = values.shape[1]
    mean, sd = np.zeros((MINUTES, k)), np.ones((MINUTES, k))
    have = np.zeros(MINUTES, bool)
    for m in range(MINUTES):
        sel = minutes == m
        if sel.sum() >= 2:
            mean[m], sd[m] = values[sel].mean(axis=0), np.maximum(values[sel].std(axis=0), 1e-6)
            have[m] = True
    known = np.flatnonzero(have)
    if len(known) == 0:
        return mean, sd
    for m in range(MINUTES):
        if not have[m]:
            near = known[np.argmin(np.abs(known - m))]
            mean[m], sd[m] = mean[near], sd[near]
    return mean, sd


class Fit:
    """Baseline by game minute plus a pooled linear tilt on the evidence. `log` makes the tilt multiplicative."""

    def __init__(self, frame, target, columns, ridge=0.02, log=False):
        self.target, self.columns, self.log = target, list(columns), log
        x = self.shape(frame.x[:, self.columns])
        y = self.shape(frame.y[target].reshape(-1, 1))
        self.fx_mean, self.fx_sd = per_minute(x, frame.minute)
        base, base_sd = per_minute(y, frame.minute)
        self.base, self.base_sd = base[:, 0], base_sd[:, 0]
        z = self.z(frame)
        r = (y[:, 0] - self.base[frame.minute]) / self.base_sd[frame.minute]
        gram = z.T @ z + ridge * frame.n * np.eye(z.shape[1])
        self.beta = np.linalg.solve(gram, z.T @ r)

    def shape(self, v):
        return np.log1p(np.maximum(v, 0.0)) if self.log else v

    def z(self, frame):
        x = self.shape(frame.x[:, self.columns])
        return np.clip((x - self.fx_mean[frame.minute]) / self.fx_sd[frame.minute], -6.0, 6.0)

    def raw(self, frame):
        out = self.base[frame.minute] + self.base_sd[frame.minute] * (self.z(frame) @ self.beta)
        return np.maximum(np.expm1(out) if self.log else out, 0.0)

    def predict(self, frame, floor=None, trust=None):
        out = self.raw(frame)
        if trust is not None:
            base = self.baseline(frame)
            out = base + trust[frame.minute] * (out - base)
        out = np.maximum(out, 0.0)
        if floor is not None:
            out = np.maximum(out, frame.x[:, FEATURES.index(floor)])
        return out

    def baseline(self, frame):
        out = self.base[frame.minute]
        return np.expm1(out) if self.log else out


def fold_index(frame, k, seed=0):
    batches = sorted(set(frame.batch.tolist()))
    assign = {b: (i + seed) % k for i, b in enumerate(batches)}
    return np.array([assign[b] for b in frame.batch], int)


TRUST_GRID = np.linspace(0.0, 1.0, 21)
# How much of the estimate we have actually laid eyes on. A minute-only error bar is far too tight when we have seen
# almost nothing and too loose when we have seen most of it, so the width is fitted inside these buckets as well.
SHARE_EDGES = (0.25, 0.6)
SHARE_LABELS = ("seen under a quarter of the estimate", "seen a quarter to 60%", "seen over 60%")


def share_bucket(frame, pred, floor):
    if floor is None:
        return np.zeros(len(pred), int)
    seen = frame.x[:, FEATURES.index(floor)]
    return np.clip(np.digitize(seen / np.maximum(pred, 1.0), SHARE_EDGES), 0, len(SHARE_EDGES))


def fit_trust(frame, target, columns, floor, log, ridge, k=3):
    """How far to follow the evidence away from the clock, per game minute.

    Early on the evidence is mostly noise and the fitted tilt costs more than it buys. The weight is chosen per
    minute on predictions the inner fit never saw (an inner split by batch inside the training set), not on the
    training residuals, so a model that only looks good in sample gets a low weight.
    """
    inner = fold_index(frame, k, seed=1)
    raw, base = np.zeros(frame.n), np.zeros(frame.n)
    for i in range(k):
        test_pick, train_pick = inner == i, inner != i
        if test_pick.sum() == 0 or train_pick.sum() == 0:
            return np.ones(MINUTES)
        fit = Fit(frame.take(train_pick), target, columns, ridge, log)
        held = frame.take(test_pick)
        raw[test_pick], base[test_pick] = fit.raw(held), fit.baseline(held)
    trust = np.ones(MINUTES)
    y = frame.y[target]
    for m in range(MINUTES):
        sel = frame.minute == m
        if sel.sum() < 20:
            continue
        blend = base[sel][None, :] + TRUST_GRID[:, None] * (raw[sel] - base[sel])[None, :]
        if floor is not None:
            blend = np.maximum(blend, frame.x[sel, FEATURES.index(floor)][None, :])
        trust[m] = TRUST_GRID[int(np.argmin(np.abs(blend - y[sel][None, :]).mean(axis=1)))]
    return trust


def cross_validate(frame, target, columns, floor=None, log=False, ridge=0.02, k=5, interval=0.8, shrink=True):
    """Held out by batch. Returns the truth, the two estimates, the minute and the interval half-width per row."""
    which = fold_index(frame, k)
    out = {key: np.zeros(frame.n) for key in ("truth", "time", "model", "half", "bucket")}
    out["minute"] = frame.minute.copy()
    out["truth"] = frame.y[target].copy()
    betas, trusts, widths = [], [], []
    for i in range(k):
        test_pick, train_pick = which == i, which != i
        if test_pick.sum() == 0 or train_pick.sum() == 0:
            continue
        train, test = frame.take(train_pick), frame.take(test_pick)
        fit = Fit(train, target, columns, ridge, log)
        betas.append(fit.beta)
        trust = fit_trust(train, target, columns, floor, log, ridge) if shrink else None
        trusts.append(trust if trust is not None else np.ones(MINUTES))
        # The interval's half-width comes from the TRAINING residuals only; coverage is then measured on the held-out
        # fold, so a width fitted too tight shows up as coverage below the claim.
        on_train = fit.predict(train, floor, trust)
        err = np.abs(train.y[target] - on_train)
        width = fit_width(err, train.minute, share_bucket(train, on_train, floor), interval)
        pred = fit.predict(test, floor, trust)
        out["time"][test_pick] = fit.baseline(test)
        out["model"][test_pick] = pred
        out["half"][test_pick] = width[test.minute, share_bucket(test, pred, floor)]
        out["bucket"][test_pick] = share_bucket(test, pred, floor)
        widths.append(width)
    out["trust"] = np.mean(trusts, axis=0) if trusts else np.ones(MINUTES)
    out["width"] = np.mean(widths, axis=0) if widths else np.zeros((MINUTES, len(SHARE_EDGES) + 1))
    return out, (np.mean(betas, axis=0) if betas else np.zeros(len(columns)))


def fit_width(err, minutes, buckets, interval):
    """(MINUTES, buckets) half-widths, each the `interval` quantile of the absolute error in that cell."""
    n = len(SHARE_EDGES) + 1
    width = np.full((MINUTES, n), np.nan)
    for m in range(MINUTES):
        row = minutes == m
        if row.sum() >= 5:
            width[m, :] = np.quantile(err[row], interval)
        for b in range(n):
            cell = row & (buckets == b)
            if cell.sum() >= 30:
                width[m, b] = np.quantile(err[cell], interval)
    for b in range(n):
        column = width[:, b]
        good = ~np.isnan(column)
        if good.any():
            column[~good] = np.interp(np.flatnonzero(~good), np.flatnonzero(good), column[good])
        else:
            column[:] = 0.0
    return width


def mae(truth, pred, pick=None):
    if pick is None:
        return float(np.abs(truth - pred).mean())
    return float(np.abs(truth[pick] - pred[pick]).mean())


def report(name, unit, cv, extra, interval):
    truth, minutes = cv["truth"], cv["minute"]
    print(f"\n{name} -- held-out mean absolute error ({unit}). Games held out by batch.")
    head = f"{'minute':>8} {'n':>6} {'truth':>8} {'time only':>10}"
    for label in extra:
        head += f" {label:>12}"
    print(head + f" {'model':>8} {'vs time':>8}")
    for lo, hi in BANDS + [(0, MINUTES)]:
        pick = (minutes >= lo) & (minutes < hi)
        if pick.sum() == 0:
            continue
        t, m = mae(truth, cv["time"], pick), mae(truth, cv["model"], pick)
        label = "all" if (lo, hi) == (0, MINUTES) else f"{lo}-{hi}"
        line = f"{label:>8} {pick.sum():>6} {truth[pick].mean():>8.0f} {t:>10.0f}"
        for key in extra:
            line += f" {mae(truth, extra[key], pick):>12.0f}"
        print(line + f" {m:>8.0f} {1 - m / t if t else 0:>7.0%}")
    print(f"  mean signed error {float((cv['model'] - truth).mean()):+.0f}; "
          f"median absolute error {np.median(np.abs(cv['model'] - truth)):.0f}")
    print("  how far the fit follows the evidence away from the clock, per minute band: "
          + ", ".join(f"{lo}-{hi} {cv['trust'][lo:hi].mean():.2f}" for lo, hi in BANDS))
    print(f"  stated interval (claims {interval:.0%}; half-width fitted on the training folds, coverage measured out of fold):")
    print(f"{'minute':>8} {'half-width':>11} {'covered':>8} {'width / truth':>14}")
    for lo, hi in BANDS:
        pick = (minutes >= lo) & (minutes < hi)
        if pick.sum() < 20:
            continue
        covered = (np.abs(truth[pick] - cv["model"][pick]) <= cv["half"][pick]).mean()
        print(f"{f'{lo}-{hi}':>8} {cv['half'][pick].mean():>11.0f} {covered:>8.0%} "
              f"{cv['half'][pick].mean() / max(truth[pick].mean(), 1e-9):>13.0%}")
    if cv["bucket"].max() > 0:
        print("  and by how much of the estimate we had actually laid eyes on (minutes 8 on):")
        for b, label in enumerate(SHARE_LABELS):
            pick = (cv["bucket"] == b) & (minutes >= 8)
            if pick.sum() < 50:
                continue
            covered = (np.abs(truth[pick] - cv["model"][pick]) <= cv["half"][pick]).mean()
            print(f"    {label:<38} n={pick.sum():>6} half-width {cv['half'][pick].mean():>7.0f} covered {covered:>4.0%}"
                  f"  mean |error| {mae(truth, cv['model'], pick):>7.0f}")


def feature_value(frame, target, floor, log, k):
    """What each observation is worth on its own, then a forward greedy search for a small set.

    Single-feature numbers are immune to the sign flips that collinear features produce in the full model's
    coefficients, so they are the honest answer to "what moves the estimate". The search runs without the
    per-minute shrinkage (it is a ranking, and the inner folds would make it twenty times slower).
    """
    single = []
    for j, f in enumerate(FEATURES):
        cv, _ = cross_validate(frame, target, [j], floor, log, k=k, shrink=False)
        single.append((1 - mae(cv["truth"], cv["model"]) / mae(cv["truth"], cv["time"]), f))
    single.sort(reverse=True)
    print("\n  each observation on its own, as a share of the time-only error removed:")
    for gain, f in single[:8]:
        print(f"    {f:<18} {gain:>6.1%}")
    print("    rest: " + ", ".join(f"{f} {gain:.0%}" for gain, f in single[8:]))
    chosen, best = [], 0.0
    while len(chosen) < 5:
        candidates = []
        for j, f in enumerate(FEATURES):
            if j in chosen:
                continue
            cv, _ = cross_validate(frame, target, chosen + [j], floor, log, k=k, shrink=False)
            candidates.append((1 - mae(cv["truth"], cv["model"]) / mae(cv["truth"], cv["time"]), j))
        gain, j = max(candidates)
        if gain <= best + 0.005:
            break
        chosen, best = chosen + [j], gain
        print(f"    greedy {len(chosen)}: + {FEATURES[j]:<18} {gain:>6.1%} of the time-only error removed")
    return [FEATURES[j] for j in chosen], chosen


def t2_report(frame, k):
    """Does the opponent have a finished advanced lab right now? Time-only base rate against the same evidence."""
    which = fold_index(frame, k)
    truth = frame.y["y_t2lab"]
    base, model = np.zeros(frame.n), np.zeros(frame.n)
    weights = np.zeros(len(FEATURES))
    logit = lambda p: np.log(np.clip(p, 1e-3, 1 - 1e-3) / (1 - np.clip(p, 1e-3, 1 - 1e-3)))
    for i in range(k):
        test_pick, train_pick = which == i, which != i
        if test_pick.sum() == 0 or train_pick.sum() == 0:
            continue
        train, test = frame.take(train_pick), frame.take(test_pick)
        rate, _ = per_minute(train.y["y_t2lab"].reshape(-1, 1), train.minute)
        fit = Fit(train, "y_t2lab", range(len(FEATURES)), log=True)
        x, xt = fit.z(train), fit.z(test)
        # The time-only base rate goes in as an offset, so the evidence can only tilt what the clock already says.
        off, off_t = logit(rate[train.minute, 0]), logit(rate[test.minute, 0])
        y = train.y["y_t2lab"]
        w = np.zeros(x.shape[1])
        for _ in range(50):
            p = 1 / (1 + np.exp(-np.clip(off + x @ w, -30, 30)))
            step = np.linalg.solve(x.T @ (x * (p * (1 - p))[:, None]) + 2.0 * np.eye(x.shape[1]),
                                   x.T @ (y - p) - 2.0 * w)
            w += step
            if np.abs(step).max() < 1e-7:
                break
        weights = w
        base[test_pick] = rate[test.minute, 0]
        model[test_pick] = 1 / (1 + np.exp(-np.clip(off_t + xt @ w, -30, 30)))
    print("\ny_t2lab -- does the opponent have a finished advanced lab right now? (Brier score: lower is better)")
    print(f"{'minute':>8} {'n':>6} {'really has':>11} {'time only':>10} {'+evidence':>10} {'AUC':>6}")
    for lo, hi in BANDS + [(0, MINUTES)]:
        pick = (frame.minute >= lo) & (frame.minute < hi)
        if pick.sum() < 20:
            continue
        t, b, m = truth[pick], base[pick], model[pick]
        pos, neg = m[t == 1], m[t == 0]
        auc = float((pos[:, None] > neg[None, :]).mean() + 0.5 * (pos[:, None] == neg[None, :]).mean()) if len(pos) and len(neg) else float("nan")
        label = "all" if (lo, hi) == (0, MINUTES) else f"{lo}-{hi}"
        print(f"{label:>8} {pick.sum():>6} {t.mean():>10.0%} {((b - t) ** 2).mean():>10.3f} {((m - t) ** 2).mean():>10.3f} {auc:>6.2f}")
    return weights


def json_tables(fit, columns):
    names = [FEATURES[j] for j in columns]
    return {
        "log": fit.log,
        "baseline": [round(float(v), 3) for v in fit.base],
        "baseline_sd": [round(float(v), 3) for v in fit.base_sd],
        "feature_mean": {f: [round(float(v), 3) for v in fit.fx_mean[:, i]] for i, f in enumerate(names)},
        "feature_sd": {f: [round(float(v), 3) for v in fit.fx_sd[:, i]] for i, f in enumerate(names)},
        "beta": {f: round(float(b), 4) for f, b in zip(names, fit.beta)},
    }


def main():
    here = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    data = os.path.join(here, "docs", "studies", "data", "tempo-2026-09-20")
    parser = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    parser.add_argument("--matches", default=os.path.join(here, "run", "matches"))
    parser.add_argument("--dataset", default=os.path.join(data, "samples.csv.gz"))
    parser.add_argument("--params", default="", help="where to write the fitted numbers (default: beside the dataset)")
    parser.add_argument("--extract", action="store_true", help="re-read the match records even if the dataset exists")
    parser.add_argument("--jobs", type=int, default=os.cpu_count())
    parser.add_argument("--map", default=DEFAULT_MAP, help='fit on this map ("" for every map)')
    parser.add_argument("--folds", type=int, default=5)
    parser.add_argument("--interval", type=float, default=0.8)
    parser.add_argument("--no-search", action="store_true", help="skip the per-feature and greedy searches")
    args = parser.parse_args()
    if not args.params:
        # A run with other settings writes its own file rather than quietly replacing the shipped one.
        default = (args.map, args.folds, args.interval, args.no_search) == (DEFAULT_MAP, 5, 0.8, False)
        suffix = "" if default else "-variant"
        args.params = os.path.join(data, f"tempo-params{suffix}.json")

    if args.extract or not os.path.exists(args.dataset):
        extract(args.matches, args.dataset, args.jobs)
    every = load(args.dataset)
    heuristic = [r for r in every if r["mode"] == "heuristic"]
    rates = mex_rate(heuristic)
    add_derived(every, rates)
    targets = ["y_army_m", "y_mex", "y_income", "y_def_m", "y_t2lab"]
    rows = [r for r in heuristic if not args.map or r["map"] == args.map]
    held = {"other maps": [r for r in heuristic if args.map and r["map"] != args.map],
            "commander games": [r for r in every if r["mode"] == "commander"]}

    print(f"{len(rows)} rows from {len({(r['batch'], r['game']) for r in rows})} games in "
          f"{len({r['batch'] for r in rows})} batches on {args.map or 'every map'}; one row every {STEP} s from {FIRST_ROW} s")
    print("held out entirely: " + ", ".join(f"{len(v)} rows of {k}" for k, v in held.items()))
    print("\nmetal per finished extractor, fitted on OUR OWN income and extractor count in these games")
    print("(tier 2 is fixed at 4x tier 1 from the unit files, not fitted):")
    print(f"{'map':<26} {'tier 1':>7} {'minutes 3-6':>12} {'samples':>8} {'R2':>6} {'median resid':>13}")
    for name, rate in sorted(rates.items()):
        print(f"{name:<26} {rate['t1']:>7.2f} {rate['early_t1']:>12.2f} {rate['samples']:>8} {rate['r2']:>6.2f} {rate['mad']:>13.1f}")

    frame = Frame(rows, targets)
    held_frames = {k: Frame(v, targets) for k, v in held.items() if v}
    params = {"step_seconds": STEP, "recent_window_seconds": RECENT_WINDOW, "features": FEATURES,
              "mex_rate": rates, "map": args.map, "interval": args.interval, "minutes": MINUTES, "targets": {}}
    plan = [
        ("y_army_m", "metal", "f_live_army_m", {"seen alive": "f_live_army_m", "brain now": "f_rec_army_m"}),
        ("y_mex", "extractors", "f_live_mex_n", {"seen alive": "f_live_mex_n"}),
        ("y_income", "metal/s proxy", None, {}),
        ("y_def_m", "metal", "f_live_turret_m", {"seen alive": "f_live_turret_m"}),
    ]
    every_column = list(range(len(FEATURES)))
    for target, unit, floor, naive in plan:
        scores = {}
        for log in (False, True):
            cv, beta = cross_validate(frame, target, every_column, floor, log, k=args.folds, interval=args.interval)
            scores[log] = (mae(cv["truth"], cv["model"]), cv, beta)
        log = min(scores, key=lambda key: scores[key][0])
        _, cv, beta = scores[log]
        print(f"\n{'=' * 96}\n{target}: {'multiplicative' if log else 'additive'} tilt, held-out MAE "
              f"{scores[log][0]:.1f} against {scores[not log][0]:.1f} for the other")
        extra = {label: frame.x[:, FEATURES.index(f)] for label, f in naive.items()}
        report(target, unit, cv, extra, args.interval)
        enemy_columns = [FEATURES.index(f) for f in ENEMY_ONLY]
        cv_enemy, _ = cross_validate(frame, target, enemy_columns, floor, log, k=args.folds, interval=args.interval)
        cv_corner, _ = cross_validate(frame, target, [FEATURES.index("f_corner_nw")], floor, log, k=args.folds)
        print(f"  time alone {mae(cv['truth'], cv['time']):.1f}; time and which start we drew "
              f"{mae(cv_corner['truth'], cv_corner['model']):.1f}; evidence about the opponent only "
              f"{mae(cv_enemy['truth'], cv_enemy['model']):.1f}; everything {mae(cv['truth'], cv['model']):.1f}")
        small = []
        if not args.no_search:
            small, small_columns = feature_value(frame, target, floor, log, args.folds)
            cv_small, _ = cross_validate(frame, target, small_columns, floor, log, k=args.folds, interval=args.interval)
            print(f"    the {len(small)}-feature model scores {mae(cv_small['truth'], cv_small['model']):.1f} "
                  f"against {mae(cv['truth'], cv['model']):.1f} for all {len(FEATURES)}")
        whole = Fit(frame, target, every_column, log=log)
        whole_trust = fit_trust(frame, target, every_column, floor, log, 0.02)
        entry = json_tables(whole, every_column)
        # The set a rule should be wired to: no feature about our own state, so a gate reading the estimate cannot
        # move its own inputs. It costs little (see the study) and the tables below are the ones to compile in.
        entry["enemy_only"] = json_tables(Fit(frame, target, enemy_columns, log=log), enemy_columns)
        entry["enemy_only"]["trust"] = [round(float(v), 2) for v in fit_trust(frame, target, enemy_columns, floor, log, 0.02)]
        entry["enemy_only"]["held_out_mae"] = {
            f"{lo}-{hi}": round(mae(cv_enemy["truth"], cv_enemy["model"], (cv_enemy["minute"] >= lo) & (cv_enemy["minute"] < hi)), 2)
            for lo, hi in BANDS if ((cv_enemy["minute"] >= lo) & (cv_enemy["minute"] < hi)).sum()}
        entry["enemy_only"]["half_width"] = [[round(float(v), 2) for v in row] for row in cv_enemy["width"]]
        entry.update({
            "floor_feature": floor, "small_set": small,
            "trust": [round(float(v), 2) for v in whole_trust],
            "seen_share_edges": list(SHARE_EDGES),
            "half_width": [[round(float(v), 2) for v in row] for row in cv["width"]],
            "half_width_by_band": {f"{lo}-{hi}": round(float(cv["half"][(cv["minute"] >= lo) & (cv["minute"] < hi)].mean()), 2)
                                   for lo, hi in BANDS if ((cv["minute"] >= lo) & (cv["minute"] < hi)).sum()},
            "held_out_mae": {f"{lo}-{hi}": round(mae(cv["truth"], cv["model"], (cv["minute"] >= lo) & (cv["minute"] < hi)), 2)
                             for lo, hi in BANDS if ((cv["minute"] >= lo) & (cv["minute"] < hi)).sum()},
            "time_only_mae": {f"{lo}-{hi}": round(mae(cv["truth"], cv["time"], (cv["minute"] >= lo) & (cv["minute"] < hi)), 2)
                              for lo, hi in BANDS if ((cv["minute"] >= lo) & (cv["minute"] < hi)).sum()},
        })
        for label, other in held_frames.items():
            pred, time_only, t = whole.predict(other, floor, whole_trust), whole.baseline(other), other.y[target]
            line = (f"  transfer to {label} ({other.n} rows, never trained on): time only {mae(t, time_only):.0f}, "
                    f"model {mae(t, pred):.0f} {unit}")
            if target in ("y_mex", "y_income"):
                # Both scale with how many metal spots the map has; try the same numbers per spot.
                scale = other.spots / frame.spots.mean()
                line += f", model x (this map's spots / {frame.spots.mean():.0f}) {mae(t, pred * scale):.0f}"
            print(line)
            entry[f"transfer_{label.replace(' ', '_')}_mae"] = round(mae(t, pred), 2)
        params["targets"][target] = entry

    params["t2lab"] = {"beta": {f: round(float(b), 4) for f, b in zip(FEATURES, t2_report(frame, args.folds))}}

    os.makedirs(os.path.dirname(args.params), exist_ok=True)
    with open(args.params, "w") as f:
        json.dump(params, f, indent=1, sort_keys=True)
    print(f"\nfitted numbers written to {args.params}")


if __name__ == "__main__":
    main()
