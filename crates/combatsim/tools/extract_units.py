#!/usr/bin/env python3
"""Derive data/units.json from the game's unit definition files (only the numbers combat needs are copied).

usage: tools/extract_units.py <path to Beyond-All-Reason checkout> > data/units.json

A unit file is a pure Lua data table (`return { name = {...} }`), so it is loaded with `lua` and serialised to JSON
rather than scraped with regexes: weapondefs are nested three deep and the fields we need vary by weapon type.
Only the files listed below are loaded, and nothing else in the checkout is executed.

Post-processing (`gamedata/alldefs_post.lua`) is applied here where it changes combat numbers with the arena's
default mod options (every `multiplier_*` is 1): reload and burst times are rounded down to whole engine frames,
and a BeamLaser with `impactonly` becomes areaofeffect 11 / edgeeffectiveness 1. Nothing else in `_post` touches
health, damage, range or speed; the rest is graphics, mod options and categories.
"""
import json, math, re, subprocess, sys, pathlib

root = pathlib.Path(sys.argv[1])
FPS = 30

# Which units: the two factions' commanders, everything the tier-1 and tier-2 bot labs and vehicle plants build,
# and every land defence of either faction. That covers the duel tables, our own tier-2 options and what BARb
# fields against us (which includes tier-2 vehicles it builds from an advanced plant).
FACTORIES = ["lab", "vp", "alab", "avp"]
EXTRA = ["com"]
# What raiders come for: economy buildings, unarmed, in a scenario only to be burned or saved (chase scenarios).
ASSETS = ["mex", "win", "solar", "makr", "estor", "nanotc", "rad"]
DEFENCE_DIRS = ["ArmBuildings/LandDefenceOffence", "CorBuildings/LandDefenceOffence"]

# Units whose whole reason for existing is a mechanism the simulation has no model for. Keeping them would put a
# plausible-looking number on a fight the simulator cannot judge, so they are left out and the reason is written
# into the table (`_excluded`), which makes a query for one fail loudly instead of quietly lying.
EXCLUDED = {
    "armvader": "crawling bomb: walks into the enemy and self-destructs; the sim has no suicide attack",
    "corroach": "crawling bomb: walks into the enemy and self-destructs; the sim has no suicide attack",
    "corsktl": "crawling bomb: walks into the enemy and self-destructs; the sim has no suicide attack",
    "armspid": "its only weapon is a paralyser, and the sim has no stun; it would fight as an unarmed walker",
}

ENCODE = """
local function esc(s) return (s:gsub('[%c"\\\\]', function(c)
    return ({['"']='\\\\"', ['\\\\']='\\\\\\\\', ['\\n']='\\\\n', ['\\t']='\\\\t'})[c] or string.format('\\\\u%04x', c:byte())
end)) end
local function enc(v)
    local t = type(v)
    if t == 'number' then return tostring(v) end
    if t == 'boolean' then return tostring(v) end
    if t == 'string' then return '"' .. esc(v) .. '"' end
    if t ~= 'table' then return 'null' end
    if #v > 0 then
        local out = {}
        for i = 1, #v do out[i] = enc(v[i]) end
        return '[' .. table.concat(out, ',') .. ']'
    end
    local keys = {}
    for k in pairs(v) do keys[#keys + 1] = tostring(k) end
    table.sort(keys)
    local out = {}
    for _, k in ipairs(keys) do out[#out + 1] = '"' .. esc(k) .. '":' .. enc(v[k] ~= nil and v[k] or rawget(v, tonumber(k))) end
    return '{' .. table.concat(out, ',') .. '}'
end
"""


def lua(body):
    """Run `body` with the JSON encoder in scope and read back what it writes."""
    done = subprocess.run(["lua", "-e", ENCODE + body], capture_output=True, text=True)
    if done.returncode != 0:
        sys.exit("lua: " + done.stderr)
    return json.loads(done.stdout)


def load(paths):
    """The unit tables of `paths`, keyed by unit name. The files are pure `return {...}` data."""
    listed = ",".join('"%s"' % p for p in paths)
    return lua("""
        local all = {}
        for _, path in ipairs({%s}) do
            for name, def in pairs(dofile(path)) do all[name] = def end
        end
        io.write(enc(all))
    """ % listed)


def armor_classes():
    """Unit name -> armour class. Only the literal table at the head of gamedata/armordefs.lua is read; what follows
    it adds scavenger copies and per-unit overrides through engine globals we do not have."""
    text = (root / "gamedata" / "armordefs.lua").read_text()
    table = text[: text.index("\n}\n") + 3] + "\nreturn armorDefs\n"
    defs = lua("io.write(enc(load([==[" + table + "]==])()))")
    return {name: cls for cls, names in defs.items() for name in names}


def move_classes():
    """Movement class -> (slope it can climb in the engine's 0-255 slope units, deepest water it wades).

    The unit file's own `maxslope` is legacy; what the engine uses comes from the class named by `movementclass`
    in gamedata/movedefs.lua, in degrees, which the engine stores as 1 - cos(angle). The record format's
    `move_classes` confirms it: bots 0.412 = 1 - cos(54 degrees). The file ends in engine-only code, so only its
    tables are read, by pattern."""
    text = (root / "gamedata" / "movedefs.lua").read_text()

    def constants(name):
        block = re.search(r"^local %s = \{(.*?)^\}" % name, text, re.M | re.S)
        return {k: float(v) for k, v in re.findall(r"(\w+) = ([\d.]+)", block[1])}

    slopes, depths = constants("SLOPE"), constants("DEPTH")
    classes = {}
    for name, body in re.findall(r"^\t(\w+) = \{$(.*?)^\t\},$", text, re.M | re.S):
        slope = re.search(r"maxslope = SLOPE\.(\w+)", body)
        depth = re.search(r"maxwaterdepth = DEPTH\.(\w+)", body)
        degrees = slopes.get(slope[1], 0.0) if slope else 0.0
        classes[name] = (
            round((1.0 - math.cos(math.radians(degrees))) * 255.0),
            depths.get(depth[1], 0.0) if depth else 0.0,
        )
    return classes


def frames(value):
    """The engine runs reloads in whole frames and `alldefs_post` rounds the def to match."""
    return max(1, int(value * FPS + 1e-3)) / FPS


def radius(udef):
    """Collision radius in elmos: half the mean of the collision volume's x and z scales, falling back to the
    build footprint (8 elmos a square) for a unit with no volume. Projectiles collide with this volume, and it is
    also what the engine pushes units apart with, so it decides both whether a near miss lands and how tightly a
    blob can pack. It is nearly twice the footprint for most units."""
    scales = udef.get("collisionvolumescales")
    if scales:
        x, _, z = (float(v) for v in scales.split())
        return (x + z) / 4.0
    return max(udef.get("footprintx", 1), udef.get("footprintz", 1)) * 8.0 / 2.0


def weapon(wdef, mount):
    """One weapon, post-processed. `mount` is the unit's `weapons[i]` entry (target categories live there)."""
    kind = wdef.get("weapontype", "Cannon")
    aoe, edge = wdef.get("areaofeffect", 8.0), wdef.get("edgeeffectiveness", 0.0)
    if kind == "BeamLaser" and wdef.get("impactonly") == 1:
        aoe, edge = 11.0, 1.0
    return {
        "kind": kind,
        "range": wdef.get("range", 0.0),
        "reload": frames(wdef.get("reloadtime", 1.0)),
        "burst": wdef.get("burst", 1),
        "burst_rate": frames(wdef.get("burstrate", 0.1)),
        "projectiles": wdef.get("projectiles", 1),
        "damage": wdef.get("damage", {}),
        "aoe": aoe,
        "edge": edge,
        "velocity": wdef.get("weaponvelocity", 0.0),
        "start_velocity": wdef.get("startvelocity", 0.0),
        "acceleration": wdef.get("weaponacceleration", 0.0),
        # A tracking missile steers onto the target; without it (Rocketeer, Aggravator) the shot flies where the
        # target was aimed at and a unit that has moved on is missed.
        "tracks": bool(wdef.get("tracks")),
        # Aim error in the engine's raw units; it turns them into an angle with sin(x * pi / 0xafff).
        # `accuracy` is drawn once a salvo, `sprayangle` once a projectile.
        "accuracy": wdef.get("accuracy", 0.0),
        "spray": wdef.get("sprayangle", 0.0),
        # How well the shot leads a moving target: at 0 (the engine's default, and most BAR ground weapons) the
        # unit over- or under-estimates the target's speed by anything from 0 to 2x, redrawn twice a second.
        "predict_boost": wdef.get("predictboost", 0.0),
        "lead_limit": wdef.get("leadlimit", -1.0),
        "energy_per_shot": wdef.get("energypershot", 0.0),
        "only_targets": mount.get("onlytargetcategory", ""),
        # Four ways a weapon does not take part in a stand-up land fight, all of which the simulation would
        # otherwise score as ordinary damage: a paralyser stuns instead of killing, a stockpiled launcher (nuke,
        # anti-nuke) has nothing to fire until one has been built, a torpedo or submerged gun needs water, and a
        # `commandfire` weapon (the commander's D-Gun) only fires when a player or AI orders it by hand.
        "paralyzer": bool(wdef.get("paralyzer")),
        "stockpile": bool(wdef.get("stockpile")),
        "water_only": bool(wdef.get("waterweapon")) or kind == "TorpedoLauncher",
        "command_fire": bool(wdef.get("commandfire")),
    }


def mounted(weapons):
    """`weapons` is a Lua array in the file, but the JSON encoder gives a map when its keys are sparse."""
    return list(weapons.values()) if isinstance(weapons, dict) else list(weapons)


def main():
    files = {p.stem: p for p in (root / "units").rglob("*.lua")}
    wanted = []
    for side in ("arm", "cor"):
        wanted += [side + s for s in FACTORIES + EXTRA + ASSETS]
        for factory in FACTORIES:
            wanted += load([files[side + factory]])[side + factory].get("buildoptions", [])
    wanted += [p.stem for d in DEFENCE_DIRS for p in (root / "units" / d).glob("*.lua")]
    wanted = sorted({n for n in wanted if n in files})

    defs = load([files[n] for n in wanted])
    classes = armor_classes()
    moves = move_classes()
    commit = subprocess.run(["git", "-C", str(root), "log", "-1", "--format=%H %ad", "--date=short"],
                            capture_output=True, text=True).stdout.strip()
    units = {}
    for name in wanted:
        udef = defs.get(name)
        if udef is None or "health" not in udef or name in EXCLUDED:
            continue
        # One entry per mount, not per weapondef: Janus carries the same launcher twice and fires both at once.
        # The exception is BAR's smart-trajectory plasma battery (armguard, corpun, armamb, cortoast), which
        # mounts the same gun twice — `plasma` and `plasma_high` — plus a `smart_trajectory_dummy`, and a gadget
        # picks one arc per shot. Firing both would double the battery's rate of fire, so the high-arc twin is
        # dropped; with no height in the model the two are the same shot anyway.
        defs_by_key = {k.lower(): w for k, w in udef.get("weapondefs", {}).items()}
        mounts = mounted(udef.get("weapons", []))
        keys = [m.get("def", "").lower() for m in mounts]
        weapons = [weapon(defs_by_key[key], mount) for mount, key in zip(mounts, keys)
                   if key in defs_by_key and not (key.endswith("_high") and key[: -len("_high")] in keys)]
        units[name] = {
            "metal": udef.get("metalcost", 0.0),
            "energy": udef.get("energycost", 0.0),
            "health": udef["health"],
            "speed": udef.get("speed", 0.0),
            "sight": udef.get("sightdistance", 0.0),
            "radius": round(radius(udef), 2),
            "armor": classes.get(name, "standard"),
            "air": bool(udef.get("canfly")),
            "builder": bool(udef.get("workertime")),
            # `customparams.techlevel`, which only the tier-2 and tier-3 files set; everything else is tier 1.
            "tech": int(udef.get("customparams", {}).get("techlevel", 1)),
            "max_slope": moves.get(udef.get("movementclass", ""), (0, 0.0))[0],
            "max_depth": moves.get(udef.get("movementclass", ""), (0, 0.0))[1],
            "weapons": weapons,
        }
    print(json.dumps({
        "_source": f"Beyond-All-Reason units/**/*.lua at {commit}, by crates/combatsim/tools/extract_units.py",
        "_note": "Post-processed for the arena's default mod options; see the tool's docstring.",
        "_excluded": EXCLUDED,
        "units": units,
    }, indent=0, sort_keys=True))


main()
