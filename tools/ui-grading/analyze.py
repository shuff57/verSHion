"""Read the critics' verdicts and report: per-judge honesty first, scores second.

Controls (the same image twice) and mirrors (one pair with the sides swapped) exist to catch a
judge that is measuring itself. They are reported, and excluded from every aggregate: a control
splitting by 1.5 is not a 7.5 and a 2.5 averaged into a mean is noise wearing a score.
"""
import collections
import glob
import json
import os
import statistics as st

B = os.environ.get('UC_BLIND', 'blind')
KEY = os.path.expanduser(os.environ.get('UC_KEY', '~/.local/state/ui-hill-climb/key.json'))
K = json.load(open(KEY))
DIM = ['hierarchy', 'spacing', 'type', 'state']
M = sorted(os.path.basename(f)[:-6] for f in glob.glob(f'{B}/out/*.jsonl'))


def parse_lines(path):
    """Read verdicts, gluing a JSON object a model wrapped over two lines."""
    out, carry = [], ''
    for raw in open(path):
        ln = raw.strip()
        if not ln:
            continue
        cand = (carry + ln) if carry else ln
        try:
            out.append(json.loads(cand))
            carry = ''
        except Exception:
            if ln[:1] in '"},{':
                carry = cand
                if len(carry) > 4000:
                    carry = ''
            else:
                carry = ''
    if carry:
        try:
            out.append(json.loads(carry))
        except Exception:
            pass
    return out


R = {}
for m in M:
    R[m] = {}
    for o in parse_lines(f'{B}/out/{m}.jsonl'):
        if 'id' in o:
            R[m][o['id']] = o

print('PARSED')
for m in M:
    miss = [i for i in K if i not in R[m]]
    print(f'  {m:9s} {len(R[m]):2d} of {len(K):2d}  missing {miss if miss else "-"}')


def SOLO(o):
    for k in ('S', o.get('id')):
        if k in o and isinstance(o[k], dict):
            return o[k]
    if 'hierarchy' in o:
        return o
    d = [v for v in o.values() if isinstance(v, dict)]
    if d:
        return d[0]
    raise KeyError(str(o)[:120])


def sc(x):
    return {d: x.get(d) for d in DIM}


avg = lambda s: sum(s[d] for d in DIM) / 4

rows = []
for i, k in K.items():
    for m in M:
        o = R[m].get(i)
        if not o:
            continue
        if k['kind'] == 'pair':
            ou, re = k['ours'], ('B' if k['ours'] == 'A' else 'A')
            rows.append(dict(id=i, model=m, kind='mirror' if k.get('mirror_of') else 'pair',
                             partner=k.get('mirror_of'), screen=k['screen'], view=k['view'],
                             what=o.get('what'),
                             better={'tie': 'tie', ou: 'ours', re: 'ref'}.get(o.get('better'), '?'),
                             ours=sc(o[ou]), ref=sc(o[re]), gap=o[ou].get('gap')))
        elif k['kind'] == 'control':
            rows.append(dict(id=i, model=m, kind='control', screen=k['screen'], view=k['view'],
                             better=o.get('better'), A=sc(o['A']), B=sc(o['B'])))
        else:
            rows.append(dict(id=i, model=m, kind='solo', screen=k['screen'], view=k['view'],
                             theme=k['theme'], ours=sc(SOLO(o)), gap=SOLO(o).get('gap')))
json.dump(rows, open(os.environ.get('UC_RESULTS', 'results.json'), 'w'), indent=0)

print('\nCONTROLS  (the same image twice: the score gap is the judge measuring itself)')
err = collections.defaultdict(list)
for r in rows:
    if r['kind'] == 'control':
        e = abs(avg(r['A']) - avg(r['B']))
        err[r['model']].append(e)
        print(f'  {r["model"]:9s} {r["screen"]:5s} A={avg(r["A"]):.2f} B={avg(r["B"]):.2f} '
              f'error={e:.2f} verdict={r["better"]}')
if err:
    print('  error by judge:', {k: round(st.mean(v), 2) for k, v in err.items()},
          '  (0.00-0.50 trustworthy, over 1.0 unusable)')

print('\nMIRRORS  (the same pair, sides swapped: a judge that keeps picking the left image is caught here)')
pairs = {(r['model'], r['id']): r for r in rows if r['kind'] == 'pair'}
flips = collections.defaultdict(lambda: [0, 0])
for r in rows:
    if r['kind'] != 'mirror':
        continue
    base = pairs.get((r['model'], r['partner']))
    if not base:
        continue
    same = base['better'] == r['better']
    flips[r['model']][0] += same
    flips[r['model']][1] += 1
    flag = '' if same else '  <- FLIPPED'
    print(f'  {r["model"]:9s} {r["screen"]:20s} {r["view"]:7s} {base["better"]:5s} then '
          f'{r["better"]:5s}{flag}')
if flips:
    print('  agrees with itself under swap:',
          {k: f'{a}/{b}' for k, (a, b) in flips.items()})

print('\nPAIRS')
g = collections.defaultdict(list)
for r in rows:
    if r['kind'] == 'pair':
        g[(r['screen'], r['view'])].append(r)
won = 0
for k, v in sorted(g.items()):
    win = sum(1 for x in v if x['better'] == 'ours')
    loss = sum(1 for x in v if x['better'] == 'ref')
    if win > loss:
        won += 1
    print(f'  {k[0]:20s} {k[1]:7s} ' + ' '.join(f'{x["model"][:2]}:{x["better"]}' for x in v))
votes = collections.Counter(r['better'] for r in rows if r['kind'] == 'pair')
print(f'  votes {dict(votes)}  pairs won {won}/{len(g)}')

print('\nBY JUDGE')
for m in M:
    c = collections.Counter(r['better'] for r in rows if r['kind'] == 'pair' and r['model'] == m)
    print(f'  {m:9s} ours={c["ours"]:2d} ref={c["ref"]:2d} tie={c["tie"]:2d}')

print('\nOUR MEAN, desktop light (pairs and singles; controls and mirrors excluded)')
S = collections.defaultdict(list)
for r in rows:
    if r['kind'] == 'solo' and r['view'] == 'desktop' and r['theme'] == 'light':
        S[r['screen']].append(r)
means = []
print(f"  {'screen':22s} n  mean  trimmed  H    S    T    St")
for k in sorted(S):
    rs = S[k]
    v = [avg(x['ours']) for x in rs]
    trim = st.mean(sorted(v)[1:-1]) if len(v) >= 3 else st.mean(v)
    dims = {d: st.mean(x['ours'][d] for x in rs) for d in DIM}
    means.append(trim)
    print(f'  {k:22s} {len(v)}  {st.mean(v):.2f}  {trim:.2f}   ' +
          ' '.join(f'{dims[d]:.1f}' for d in DIM))
print(f'  DESKTOP TRIMMED MEAN over {len(means)} screens: {st.mean(means):.2f}')

for view, theme in (('narrow', 'light'), ('desktop', 'dark')):
    xs = [avg(r['ours']) for r in rows if r['kind'] == 'solo'
          and r['view'] == view and r['theme'] == theme]
    if xs:
        dims = {d: round(st.mean(r['ours'][d] for r in rows if r['kind'] == 'solo'
                                and r['view'] == view and r['theme'] == theme), 2) for d in DIM}
        print(f'  {view}/{theme}: {st.mean(xs):.2f} over {len(xs)} screens  {dims}')
