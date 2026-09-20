#!/usr/bin/env python3
"""Opening measures of an A/B batch, per arm: extractors and metal income by minute, first lab, first constructor,
army metal built, stored-energy stalls, and (where the bot logged an opening search) what the search predicted.

    run/opening_ab.py <batch dir>
"""
import collections, glob, json, re, statistics, sys

d = sys.argv[1]
arms = collections.defaultdict(lambda: collections.defaultdict(list))
for m in sorted(glob.glob(d + '/[0-9]*')):
    i = int(m.split('/')[-1])
    arm = 'A (as configured)' if (i // 4) % 2 == 0 else 'B (rule disabled)'
    names, cost, mex, lab, con, army, stalled, inc = None, None, [], None, [], 0.0, 0, {}
    for l in open(glob.glob(m + '/record-*.jsonl')[0]):
        try:
            r = json.loads(l)
        except ValueError:
            continue
        if r.get('t') == 'header':
            names = [x['name'] for x in r['unit_defs']]
            cost = [x['metal'] if x['class'] == 'army' else 0 for x in r['unit_defs']]
        elif r.get('t') == 'ev' and r.get('k') == 'finished':
            n, t = names[r['d']], r['f'] / 30
            if n.endswith('mex'): mex.append(t)
            if n.endswith('lab') and lab is None: lab = t
            if n.endswith('ck'): con.append(t)
            if t <= 300: army += cost[r['d']]
        elif r.get('t') == 's' and r['f'] <= 9000:
            if r['f'] % 1800 == 0: inc[r['f'] // 1800] = r['m'][1]
            if r['f'] % 30 == 0 and r['f'] > 1800 and r['e'][0] < 1: stalled += 1
    a = arms[arm]
    for k in (2, 3, 4, 5):
        a[f'mex@{k}'].append(sum(1 for t in mex if t <= k * 60))
        if k in inc: a[f'metal/s@{k}'].append(inc[k])
    a['lab s'].append(lab or 999)
    a['first con s'].append(con[0] if con else 999)
    a['cons@5'].append(sum(1 for t in con if t <= 300))
    a['army metal@5'].append(army)
    a['s at zero energy'].append(stalled)
    log = open(m + '/bot.log').read()
    p = re.search(r'at 2, 3, 5 min: \((\d+), ([\d.]+), ([\d.]+)\) \((\d+), ([\d.]+), ([\d.]+)\) \((\d+), ([\d.]+), ([\d.]+)\)', log)
    if p:
        g = [float(x) for x in p.groups()]
        for k, o in ((2, 0), (3, 3), (5, 6)):
            a[f'predicted mex@{k}'].append(g[o]); a[f'predicted metal/s@{k}'].append(g[o + 1])
        a['predicted army metal@5'].append(g[8])
    e = re.search(r'f=(\d+) opening plan ends: (.*)', log)
    if e: a['plan ended'].append(f'{int(e.group(1)) // 30}s {e.group(2)}')
for arm, a in sorted(arms.items()):
    print(f'## {arm}, {len(a["lab s"])} games')
    for k, v in a.items():
        print(f'  {k:26}', collections.Counter(v).most_common(4) if k == 'plan ended' else f'{statistics.mean(v):7.1f}   (median {statistics.median(v):.1f})')
