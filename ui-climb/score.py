#!/usr/bin/env python3
"""Unblind one Opus climb round: controls, mirrors, per-screen new/old means, and the named gaps.

Usage: python3 score.py N [judge]   (reads ui-climb/blind-rN/out/<judge>.jsonl, default opus, and key-climb-rN.json)
The level is the mean of the NEW build's four-dimension average over the six primary pairs.
"""
import json
import os
import sys

n = sys.argv[1]
here = os.path.dirname(os.path.abspath(__file__))
key = json.load(open(os.path.expanduser(f'~/.local/state/ui-hill-climb/key-climb-r{n}.json')))
rows = {}
judge = sys.argv[2] if len(sys.argv) > 2 else 'opus'
for line in open(f'{here}/blind-r{n}/out/{judge}.jsonl'):
    line = line.strip()
    if line:
        r = json.loads(line)
        rows[r['id']] = r
dims = ('hierarchy', 'spacing', 'type', 'state')
avg = lambda d: sum(d[k] for k in dims) / 4

missing = sorted(set(key) - set(rows))
print(f'parsed {len(rows)} of {len(key)}' + (f'  MISSING {missing}' if missing else ''))
ctl = []
for i, k in sorted(key.items()):
    if k['kind'] == 'control' and i in rows:
        e = abs(avg(rows[i]['A']) - avg(rows[i]['B']))
        ctl.append(e)
        print(f'control {k["screen"]:8} error={e:.2f} verdict={rows[i]["better"]}')

primary, mirrors, levels = {}, [], []
for i, k in sorted(key.items()):
    if k['kind'] == 'control' or i not in rows:
        continue
    r = rows[i]
    new, old = k['ours'], ('B' if k['ours'] == 'A' else 'A')
    pick = 'new' if r['better'] == new else ('tie' if r['better'] == 'tie' else 'old')
    rec = dict(screen=k['screen'], view=k['view'], new=avg(r[new]), old=avg(r[old]), pick=pick,
               dims={d: r[new][d] for d in dims}, gap=r[new]['gap'])
    if k.get('mirror_of'):
        mirrors.append((k.get('mirror_of'), rec))
    else:
        primary[i] = rec
for partner, m in mirrors:
    p = primary.get(partner)
    same = p and p['pick'] == m['pick']
    print(f'mirror  {m["screen"]:8} {m["view"]:7} {p["pick"] if p else "?"} then {m["pick"]}' + ('' if same else '   <- FLIPPED'))

print(f'\n{"screen":9} {"view":8} {"new":>5} {"old":>5} {"delta":>6} pick   H  S  T  St')
for i, p in sorted(primary.items(), key=lambda x: (x[1]['screen'], x[1]['view'])):
    d = p['dims']
    print(f'{p["screen"]:9} {p["view"]:8} {p["new"]:5.2f} {p["old"]:5.2f} {p["new"] - p["old"]:+6.2f} {p["pick"]:5} '
          f'{d["hierarchy"]:2} {d["spacing"]:2} {d["type"]:2} {d["state"]:2}')
lv = [p['new'] for p in primary.values()]
votes = sum(p['pick'] == 'new' for p in primary.values())
print(f'\nLEVEL (new build mean over {len(lv)} frames): {sum(lv) / len(lv):.2f}   '
      f'new preferred {votes}/{len(lv)}   control error max {max(ctl) if ctl else float("nan"):.2f}')
print('\nnamed gaps on the new build:')
for i, p in sorted(primary.items(), key=lambda x: x[1]['new']):
    print(f'- [{p["screen"]} {p["view"]} {p["new"]:.2f}] {p["gap"]}')
