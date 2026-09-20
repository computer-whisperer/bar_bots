import json,glob,sys,collections,statistics,math
# per batch: by minute, soldiers, share within 1500 of home, share committed attackers; extractor deaths with no soldier within 900
for b in sys.argv[1:]:
    d=glob.glob(f'run/matches/*{b}')[0]
    res={json.loads(l)['index']:json.loads(l) for l in open(d+'/results.jsonl')}
    rows=collections.defaultdict(lambda: collections.defaultdict(list)); undefended=[0,0]; launches=[]
    for m in sorted(glob.glob(d+'/[0-9]*')):
        f=glob.glob(m+'/record-*.jsonl')[0]; cls=None; home=None; last_own=[]; was_attacking=False; n_launch=0
        for l in open(f):
            try: r=json.loads(l)
            except ValueError: continue
            if r.get('t')=='header': cls=[x['class'] for x in r['unit_defs']]; continue
            if r.get('t')=='ev' and r.get('k')=='created' and home is None: home=(r['x'],r['z'])
            if r.get('t')=='ev' and r.get('k')=='destroyed' and cls[r['d']]=='extractor' and r['f']>30*240:
                near=any(cls[u[1]]=='army' and math.hypot(u[2]-r['x'],u[3]-r['z'])<900 for u in last_own)
                undefended[0]+=1; undefended[1]+= (not near)
            if r.get('t')=='s':
                last_own=r['own']
                army=[u for u in r['own'] if cls[u[1]]=='army' and not u[5]&1]
                att=sum(1 for u in army if u[5]&4)
                if att>=4 and not was_attacking: n_launch+=1
                was_attacking=att>=4
                if r['f']%(30*60)==0 and r['f']//1800 in (6,10,14,18,22) and army:
                    k=r['f']//1800
                    rows[k]['n'].append(len(army)); rows[k]['home'].append(sum(1 for u in army if math.hypot(u[2]-home[0],u[3]-home[1])<1500)/len(army)); rows[k]['att'].append(att/len(army))
        launches.append(n_launch)
    print(f"## {b}: wave launches per game median {statistics.median(launches)}, games with none {sum(1 for x in launches if x==0)}/{len(launches)}; extractors lost after 4:00: {undefended[0]}, with no soldier of ours within 900: {undefended[1]} ({100*undefended[1]//max(1,undefended[0])} %)")
    for k in sorted(rows): print(f"   min {k:2}: games {len(rows[k]['n']):2}  soldiers {statistics.mean(rows[k]['n']):5.1f}  within 1500 of home {100*statistics.mean(rows[k]['home']):3.0f} %  committed attackers {100*statistics.mean(rows[k]['att']):3.0f} %")
