#!/usr/bin/env python3
"""Render one moment of a recorded match as a PNG: what a player looking at the map would see.

usage: run/render_scene.py MATCH MM:SS [--out FILE] [--size PIXELS] [--trails SECONDS] [--crop X0 Z0 X1 Z1] [--truth]
       run/render_scene.py MATCH MM:SS --svg FILE [--trails SECONDS]     the same scene as SVG source, for reading as text

Reads record-<ai>.jsonl and terrain-<ai>.bin (docs/harness/record-format.md). Relief is shaded by height, water is blue,
ground an ordinary bot cannot cross (too steep or too deep) is near black. Ours is green/cyan, theirs red:
  extractor = diamond, factory = big square, turret = triangle, other building = small square,
  commander = ring, builder = cyan dot, soldier = green dot; free metal spot = white ring.
`--trails N` draws where each of our soldiers walked in the last N seconds (yellow, older is fainter). The opponent is
what our units saw at that moment; `--truth` adds the opponent's every unit from truth-<ai>.jsonl (faint red).
The grid is the A-H / 1-8 grid of the reports, with map coordinates along the edges.

The SVG is meant to be read as text by a model, not drawn: coordinates are map elmos / 10 (x east, y south), the land and
the cliffs are simplified outlines, every unit is an element with a class, and soldiers' trails are polylines.
"""
import argparse, glob, json, os, sys

import numpy as np
from PIL import Image, ImageDraw, ImageFont

OURS, BUILDER, THEIRS, TRUTH, TRAIL, SPOT = (60, 255, 90), (70, 220, 255), (255, 60, 50), (170, 70, 70), (255, 220, 40), (255, 255, 255)


def load(match):
    path = glob.glob(os.path.join(match, "record-*.jsonl"))[0]
    header, samples = None, []
    for line in open(path):
        record = json.loads(line)
        if record["t"] == "header":
            header = record
        elif record["t"] == "s":
            samples.append(record)
    return header, samples


def relief(match, header):
    terrain = header["terrain"]
    width, height = terrain["width"], terrain["height"]
    raw = open(os.path.join(match, terrain["file"]), "rb").read()
    heights = np.frombuffer(raw[: width * height * 2], dtype="<i2").reshape(height, width).astype(np.float32)
    slopes = np.frombuffer(raw[width * height * 2 :], dtype=np.uint8).reshape(height, width).astype(np.float32) / 255.0
    # The ordinary walking class: the one most unit types use among bots that cannot wade deep water.
    walkers = [c for c in terrain["move_classes"] if c["kind"] == "bot" and c["depth"] < 1000 and c["max_slope"] < 0.99]
    walker = max(walkers, key=lambda c: c["units"])
    land = heights > 0
    top = max(float(heights.max()), 1.0)
    shade = 95 + 120 * np.clip(heights / top, 0, 1)
    # Light from the north-west, so cliffs read as edges.
    gradient = np.zeros_like(heights)
    gradient[1:, 1:] = (heights[1:, 1:] - heights[:-1, :-1]) / 16.0
    shade = np.clip(shade - 60 * gradient, 40, 235)
    image = np.zeros((height, width, 3), dtype=np.float32)
    image[..., 0], image[..., 1], image[..., 2] = shade * 0.80, shade * 0.78, shade * 0.62
    water = ~land
    depth = np.clip(-heights / 80.0, 0, 1)
    image[water] = np.stack([30 - 15 * depth, 70 - 30 * depth, 150 - 50 * depth], axis=-1)[water]
    blocked = land & (slopes > walker["max_slope"])
    image[blocked] = (25, 18, 18)
    return Image.fromarray(image.astype(np.uint8), "RGB"), terrain["cell"]


def simplify(points, tolerance):
    """Ramer-Douglas-Peucker."""
    if len(points) < 3:
        return points
    a, b = np.array(points[0], dtype=float), np.array(points[-1], dtype=float)
    span = b - a
    length = np.hypot(*span) or 1.0
    rel = np.array(points, dtype=float) - a
    offsets = np.abs(span[0] * rel[:, 1] - span[1] * rel[:, 0]) / length if np.any(span) else np.hypot(rel[:, 0], rel[:, 1])
    furthest = int(offsets.argmax())
    if offsets[furthest] <= tolerance:
        return [points[0], points[-1]]
    return simplify(points[: furthest + 1], tolerance)[:-1] + simplify(points[furthest:], tolerance)


def outlines(mask, cell, tolerance, smallest):
    """Closed outlines of a boolean grid as lists of (x, y) in elmos / 10, small ones dropped."""
    from contourpy import contour_generator

    padded = np.pad(mask.astype(np.float32), 1)
    found = []
    for line in contour_generator(z=padded).lines(0.5):
        points = [((x - 1) * cell / 10, (y - 1) * cell / 10) for x, y in line]
        xs, ys = np.array(points).T
        area = abs(np.dot(xs, np.roll(ys, 1)) - np.dot(ys, np.roll(xs, 1))) / 2
        if area >= smallest:
            half = len(points) // 2  # a closed ring has equal ends, which RDP cannot take in one piece
            ring = simplify(points[: half + 1], tolerance)[:-1] + simplify(points[half:], tolerance)
            found.append((area, ring))
    return [ring for _, ring in sorted(found, key=lambda f: -f[0])]


def write_svg(match, header, samples, index, trails, out):
    terrain = header["terrain"]
    width, height, cell = terrain["width"], terrain["height"], terrain["cell"]
    raw = open(os.path.join(match, terrain["file"]), "rb").read()
    heights = np.frombuffer(raw[: width * height * 2], dtype="<i2").reshape(height, width)
    slopes = np.frombuffer(raw[width * height * 2 :], dtype=np.uint8).reshape(height, width) / 255.0
    walkers = [c for c in terrain["move_classes"] if c["kind"] == "bot" and c["depth"] < 1000 and c["max_slope"] < 0.99]
    steep = max(walkers, key=lambda c: c["units"])["max_slope"]
    land = heights > 0
    path = lambda ring: "M" + " ".join(f"{x:.0f},{y:.0f}" for x, y in ring) + "Z"
    defs, sample = header["unit_defs"], samples[index]
    size = header["map"]["width"] / 10
    lines = [
        f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {size:.0f} {header["map"]["height"] / 10:.0f}">',
        "<!-- units are map elmos / 10; x grows east, y grows south; everything outside a land outline is water -->",
        '<g id="land">' + "".join(f'<path d="{path(r)}"/>' for r in outlines(land, cell, 2.5, 150)) + "</g>",
        '<g id="cliffs-walkers-cannot-cross">' + "".join(f'<path d="{path(r)}"/>' for r in outlines(land & (slopes > steep), cell, 2.0, 40)) + "</g>",
    ]
    taken = [(u[2], u[3]) for u in sample["own"] if defs[u[1]]["class"] == "extractor"]
    free = [(x, z) for x, z in header["metal_spots"] if not any(abs(x - tx) < 60 and abs(z - tz) < 60 for tx, tz in taken)]
    lines.append('<g id="free-metal-spots">' + "".join(f'<circle cx="{x / 10:.0f}" cy="{z / 10:.0f}" r="2"/>' for x, z in free) + "</g>")
    by_class = {}
    for u in sample["own"]:
        by_class.setdefault(defs[u[1]]["class"], []).append(u)
    for kind in ("commander", "factory", "extractor", "turret", "building", "builder", "army"):
        if kind in by_class:
            lines.append(f'<g id="our-{kind}">' + "".join(f'<use href="#{defs[u[1]]["name"]}" x="{u[2] / 10:.0f}" y="{u[3] / 10:.0f}"/>' for u in by_class[kind]) + "</g>")
    lines.append('<g id="enemy-in-sight">' + "".join(f'<use href="#{defs[e[1]]["name"] if 0 <= e[1] < len(defs) else "unknown"}" x="{e[2] / 10:.0f}" y="{e[3] / 10:.0f}"/>' for e in sample["en"]) + "</g>")
    if trails:
        paths = {}
        for s in samples[max(0, index - trails) : index + 1 : 5]:
            for u in s["own"]:
                if defs[u[1]]["class"] == "army":
                    paths.setdefault(u[0], []).append((u[2] / 10, u[3] / 10))
        moved = [simplify(p, 3.0) for p in paths.values() if np.hypot(p[-1][0] - p[0][0], p[-1][1] - p[0][1]) > 20 or len(p) > 2 and max(np.hypot(q[0] - p[0][0], q[1] - p[0][1]) for q in p) > 20]
        lines.append(f'<g id="our-soldiers-paths-last-{trails}s">' + "".join('<polyline points="' + " ".join(f"{x:.0f},{y:.0f}" for x, y in p) + '"/>' for p in moved) + "</g>")
    lines.append("</svg>")
    open(out, "w").write("\n".join(lines) + "\n")
    print(f"{out}: {sum(len(l) for l in lines)} characters")


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    parser.add_argument("match")
    parser.add_argument("time", help="MM:SS of game time")
    parser.add_argument("--out", default="scene.png")
    parser.add_argument("--size", type=int, default=768, help="longer side in pixels (default 768)")
    parser.add_argument("--trails", type=int, default=0, help="seconds of our soldiers' movement to draw")
    parser.add_argument("--crop", type=float, nargs=4, metavar=("X0", "Z0", "X1", "Z1"), help="map rectangle to show")
    parser.add_argument("--truth", action="store_true", help="add the opponent's every unit (ground truth)")
    parser.add_argument("--svg", help="write the scene as SVG source to this file instead of a picture")
    args = parser.parse_args()

    header, samples = load(args.match)
    minutes, seconds = args.time.split(":")
    frame = (int(minutes) * 60 + int(seconds)) * header["frames_per_second"]
    index = min(range(len(samples)), key=lambda i: abs(samples[i]["f"] - frame))
    if args.svg:
        return write_svg(args.match, header, samples, index, args.trails, args.svg)
    sample = samples[index]
    defs = header["unit_defs"]  # samples name a type by its index in this list
    base, cell = relief(args.match, header)
    x0, z0, x1, z1 = args.crop or (0, 0, header["map"]["width"], header["map"]["height"])
    base = base.crop((int(x0 / cell), int(z0 / cell), int(x1 / cell), int(z1 / cell)))
    scale = args.size / max(x1 - x0, z1 - z0)
    image = base.resize((round((x1 - x0) * scale), round((z1 - z0) * scale)), Image.BILINEAR)
    draw = ImageDraw.Draw(image, "RGBA")
    font = ImageFont.load_default()
    px = lambda x, z: ((x - x0) * scale, (z - z0) * scale)
    unit = max(2.0, args.size / 256)  # marker size grows with the picture

    columns, rows = header["grid"]["columns"], header["grid"]["rows"]
    for c in range(columns + 1):
        x = header["map"]["width"] * c / columns
        draw.line([px(x, z0), px(x, z1)], fill=(255, 255, 255, 50))
    for r in range(rows + 1):
        z = header["map"]["height"] * r / rows
        draw.line([px(x0, z), px(x1, z)], fill=(255, 255, 255, 50))
    for c in range(columns):
        for r in range(rows):
            x, z = header["map"]["width"] * c / columns, header["map"]["height"] * r / rows
            if x0 <= x < x1 and z0 <= z < z1:
                left, top = px(x, z)
                draw.text((left + 3, top + 2), f"{'ABCDEFGH'[c]}{r + 1} ({int(x)},{int(z)})", fill=(255, 255, 255, 170), font=font)

    own_now = sample["own"]
    taken = [(u[2], u[3]) for u in own_now if defs[u[1]]["class"] == "extractor"]
    for sx, sz in header["metal_spots"]:
        if not any(abs(sx - tx) < 60 and abs(sz - tz) < 60 for tx, tz in taken):
            cx, cz = px(sx, sz)
            draw.ellipse([cx - unit * 1.5, cz - unit * 1.5, cx + unit * 1.5, cz + unit * 1.5], outline=SPOT + (230,))

    if args.trails:
        first = max(0, index - args.trails)
        paths = {}
        for s in samples[first : index + 1]:
            for u in s["own"]:
                if defs[u[1]]["class"] == "army":
                    paths.setdefault(u[0], []).append((u[2], u[3]))
        for path in paths.values():
            for i in range(1, len(path)):
                alpha = int(40 + 180 * i / len(path))
                draw.line([px(*path[i - 1]), px(*path[i])], fill=TRAIL + (alpha,), width=1)

    def marker(x, z, kind, colour):
        cx, cz = px(x, z)
        if kind == "extractor":
            r = unit * 2
            draw.polygon([(cx, cz - r), (cx + r, cz), (cx, cz + r), (cx - r, cz)], fill=colour, outline=(0, 0, 0))
        elif kind == "factory":
            r = unit * 2.5
            draw.rectangle([cx - r, cz - r, cx + r, cz + r], fill=colour, outline=(0, 0, 0))
        elif kind == "turret":
            r = unit * 2
            draw.polygon([(cx, cz - r), (cx + r, cz + r), (cx - r, cz + r)], fill=colour, outline=(0, 0, 0))
        elif kind == "building":
            r = unit * 1.2
            draw.rectangle([cx - r, cz - r, cx + r, cz + r], fill=colour)
        elif kind == "commander":
            r = unit * 3
            draw.ellipse([cx - r, cz - r, cx + r, cz + r], outline=colour, width=2)
        else:
            r = unit
            draw.ellipse([cx - r, cz - r, cx + r, cz + r], fill=colour, outline=(0, 0, 0))

    if args.truth:
        truth_files = glob.glob(os.path.join(args.match, "truth-*.jsonl"))
        if truth_files:
            nearest = min((json.loads(l) for l in open(truth_files[0])), key=lambda t: abs(t["f"] - sample["f"]))
            names = {d["name"]: d["class"] for d in defs}
            for u in nearest["enemy"]:
                marker(u[2], u[3], names.get(u[1], "army"), TRUTH)
    order = {"building": 0, "extractor": 1, "turret": 2, "factory": 3, "builder": 4, "army": 5, "commander": 6}
    for u in sorted(own_now, key=lambda u: order.get(defs[u[1]]["class"], 0)):
        kind = defs[u[1]]["class"]
        marker(u[2], u[3], kind, BUILDER if kind == "builder" else OURS)
    for e in sample["en"]:
        marker(e[2], e[3], defs[e[1]]["class"] if 0 <= e[1] < len(defs) else "army", THEIRS)

    image.save(args.out)
    print(f"{args.out}: {image.width}x{image.height}, game time {sample['f'] // 1800}:{sample['f'] // 30 % 60:02d}, {len(own_now)} of ours, {len(sample['en'])} of theirs in sight")


if __name__ == "__main__":
    main()
