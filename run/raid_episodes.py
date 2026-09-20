#!/usr/bin/env python3
"""Raids on our extractors in recorded games, as chase questions for `combatsim chase-file`, with what was played.

An episode starts when an extractor of ours dies with none of ours having died within 900 of it in the minute before.
At 10 s before that death: the party is the opponent's armed mobile units within 900 of the extractor (from the truth
file), the assets are our extractors within 600 and our turrets within 700 (they shoot), the pursuers are our soldiers that were within 600 or came at least
250 nearer in the following 20 s. Played outcome over 60 s from there: party units dead, our assets and pursuers lost.

    run/raid_episodes.py <batch dir>... > episodes.jsonl      (needs truth files: batches run by the arena since 2026-09-20)
    target/release/combatsim chase-file episodes.jsonl > verdicts.jsonl
    run/raid_episodes.py --compare episodes.jsonl verdicts.jsonl
"""
import bisect, collections, glob, json, math, statistics, sys

FPS = 30


def episodes(match):
    records = glob.glob(match + '/record-*.jsonl')
    truths = glob.glob(match + '/truth-*.jsonl')
    if not records or not truths:
        return
    truth = []
    for l in open(truths[0]):
        try:
            truth.append(json.loads(l))
        except ValueError:
            pass
    tframes = [t['f'] for t in truth]
    header, samples, deaths = None, [], []
    for l in open(records[0]):
        try:
            r = json.loads(l)
        except ValueError:
            continue
        if r.get('t') == 'header':
            header = r
            names = [d['name'] for d in r['unit_defs']]
            cls = [d['class'] for d in r['unit_defs']]
            metal = {d['name']: d['metal'] for d in r['unit_defs']}
            army = {d['name'] for d in r['unit_defs'] if d['class'] == 'army'}
        elif r.get('t') == 's':
            samples.append(r)
        elif r.get('t') == 'ev' and r.get('k') == 'destroyed':
            deaths.append(r)
    sframes = [s['f'] for s in samples]
    def sample_at(f):
        i = bisect.bisect_left(sframes, f)
        return samples[min(i, len(samples) - 1)] if samples else None
    def truth_at(f):
        i = bisect.bisect_left(tframes, f)
        return truth[min(i, len(truth) - 1)]
    last_episode = []
    for d in deaths:
        if cls[d['d']] != 'extractor' or d['f'] < 240 * FPS:
            continue
        at = (d['x'], d['z'])
        if any(math.dist(at, p) < 900 and d['f'] - f < 60 * FPS for p, f in last_episode):
            continue
        last_episode.append((at, d['f']))
        f0 = d['f'] - 10 * FPS
        party_units = [e for e in truth_at(f0)['enemy'] if e[1] in army and math.dist((e[2], e[3]), at) < 900]
        if not party_units:
            continue
        s0, s1 = sample_at(f0), sample_at(f0 + 20 * FPS)
        later = {u[0]: (u[2], u[3]) for u in s1['own']}
        pursuers, assets = [], []
        for u in s0['own']:
            pos, c = (u[2], u[3]), cls[u[1]]
            if u[5] & 1:
                continue
            if (c == 'extractor' and math.dist(pos, at) < 600) or (c == 'turret' and math.dist(pos, at) < 700):
                assets.append([names[u[1]], u[2], u[3]])
            if c == 'army':
                now = math.dist(pos, at)
                then = math.dist(later[u[0]], at) if u[0] in later else now
                if now < 600 or now - then >= 250:
                    pursuers.append((u[0], names[u[1]], u[2], u[3]))
        end = f0 + 60 * FPS
        alive_after = {e[0] for e in truth_at(end)['enemy']}
        party_metal = sum(metal.get(e[1], 0) for e in party_units)
        killed = sum(metal.get(e[1], 0) for e in party_units if e[0] not in alive_after)
        lost_ids = {x['u'] for x in deaths if f0 <= x['f'] <= end}
        cx = statistics.mean(e[2] for e in party_units); cz = statistics.mean(e[3] for e in party_units)
        home = next((x for x in [header.get('home')] if x), None)
        # The party walks on away from our pursuers (or from the map's middle of our side when nobody came).
        px = statistics.mean(p[2] for p in pursuers) if pursuers else header['map']['width'] / 2
        pz = statistics.mean(p[3] for p in pursuers) if pursuers else header['map']['height'] / 2
        n = math.hypot(cx - px, cz - pz) or 1.0
        party = collections.Counter(e[1] for e in party_units)
        yield {
            'match': match, 'frame': f0, 'at': [cx, cz], 'then': [cx + 3000 * (cx - px) / n, cz + 3000 * (cz - pz) / n],
            'party': [[k, v] for k, v in party.items()], 'pursuers': [[p[1], 1, p[2], p[3]] for p in pursuers],
            'assets': assets, 'seconds': 60,
            'played': {'party_metal': party_metal, 'party_killed': killed,
                       'assets_lost': sum(metal[names[x['d']]] for x in deaths if f0 <= x['f'] <= end and cls[x['d']] == 'extractor' and math.dist((x['x'], x['z']), at) < 600),
                       'pursuers_lost': sum(metal[p[1]] for p in pursuers if p[0] in lost_ids),
                       'nearest_pursuer': min((math.dist((p[2], p[3]), at) for p in pursuers), default=None)},
        }


def compare(episode_file, verdict_file):
    rows = [(json.loads(a), json.loads(b)) for a, b in zip(open(episode_file), open(verdict_file))]
    rows = [(e, v) for e, v in rows if v['unknown'] == 0]
    print(f'{len(rows)} episodes with every unit in the table')
    def bucket(e):
        d = e['played']['nearest_pursuer']
        return 'nobody came' if d is None else 'pursuer within 600' if d < 600 else 'pursuers from 600-1500' if d < 1500 else 'pursuers from beyond 1500'
    groups = collections.defaultdict(list)
    for e, v in rows:
        groups[bucket(e)].append((e, v))
    print('| who came | episodes | party of (metal) | killed: played / predicted | any raider killed: played / predicted | extractors lost (metal): played / predicted | pursuers lost: played / predicted |\n|---|---|---|---|---|---|---|')
    for name in ('nobody came', 'pursuer within 600', 'pursuers from 600-1500', 'pursuers from beyond 1500'):
        g = groups.get(name)
        if not g:
            continue
        m = lambda f: statistics.mean(f(e, v) for e, v in g)
        print(f"| {name} | {len(g)} | {m(lambda e, v: e['played']['party_metal']):.0f} | {m(lambda e, v: e['played']['party_killed']):.0f} / {m(lambda e, v: v['party_killed']):.0f} | "
              f"{100 * m(lambda e, v: e['played']['party_killed'] > 0):.0f} % / {100 * m(lambda e, v: v['party_killed'] > 0):.0f} % | "
              f"{m(lambda e, v: e['played']['assets_lost']):.0f} / {m(lambda e, v: v['assets_lost']):.0f} | {m(lambda e, v: e['played']['pursuers_lost']):.0f} / {m(lambda e, v: v['pursuers_lost']):.0f} |")
    agree = statistics.mean((e['played']['party_killed'] > 0) == (v['party_killed'] > 0) for e, v in rows)
    print(f'\n"was any raider killed" agrees in {100 * agree:.0f} % of episodes')


if sys.argv[1] == '--compare':
    compare(sys.argv[2], sys.argv[3])
else:
    for batch in sys.argv[1:]:
        for match in sorted(glob.glob(batch + '/[0-9]*')):
            for episode in episodes(match):
                print(json.dumps(episode))
