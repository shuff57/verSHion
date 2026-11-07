# Per-screen score for the loop: every non-control, non-mirror verdict on "ours" (desktop pair, narrow pair,
# dark single), one avg per (judge,item), trimmed mean (drop top+bottom) per screen. Also "ref" (before) for pairs.
import json,sys,statistics as st,collections
# usage: scoreboard.py RESULTS.json [GAPS_OUT.txt] [--carry OLD_RESULTS.json]
# --carry: screens absent from RESULTS (their frames were pixel-identical, see changed.py) keep the
# verdicts from OLD_RESULTS and are marked "carried" so a stale number is never mistaken for a fresh one.
argv=[a for a in sys.argv[1:]]; carry=None
if '--carry' in argv:
    i=argv.index('--carry'); carry=argv[i+1]; del argv[i:i+2]
rows=json.load(open(argv[0])); out=argv[1] if len(argv)>1 else None
fresh={r['screen'] for r in rows}; carried=set()
if carry:
    for r in json.load(open(carry)):
        if r['screen'] not in fresh and r['kind'] in ('pair','solo'):
            rows.append(r); carried.add(r['screen'])
D=['hierarchy','spacing','type','state']; a=lambda s:sum(s[d] for d in D)/4
S=collections.defaultdict(list); R=collections.defaultdict(list); G=collections.defaultdict(list)
for r in rows:
    if r['kind'] in ('pair','solo'):
        v=a(r['ours']); S[r['screen']].append(v)
        G[r['screen']].append(f"[{r['view']}/{r.get('theme','light')} {r['model']} {v:.1f}] {r.get('gap','')}")
        if r['kind']=='pair': R[r['screen']].append(a(r['ref']))
tr=lambda v: st.mean(sorted(v)[1:-1]) if len(v)>=3 else st.mean(v)
print(f"{'screen':16s} n   now   min  before"); allok=True
for k in sorted(S):
    t=tr(S[k]); allok&=t>=7.5
    print(f"{k:16s} {len(S[k]):2d}  {t:.2f}  {min(S[k]):.1f}  {tr(R[k]) if R[k] else float('nan'):.2f}  {'OK' if t>=7.5 else '<7.5'}{'  (carried)' if k in carried else ''}")
print('ALL >= 7.5' if allok else 'NOT YET', ' overall', round(st.mean(tr(v) for v in S.values()),2))
if out:
    with open(out,'w') as f:
        for k in sorted(S, key=lambda k: tr(S[k])):
            f.write(f"## {k}  (now {tr(S[k]):.2f})\n"+'\n'.join(sorted(G[k]))+'\n\n')
