#!/usr/bin/env python3
"""Rebuild every blind image from the key and prove the directory matches it.

Exit code 0 means the set is safe to grade; 1 means it is not, and it names the ids that are
wrong. Run this after make_blind.py and before the critics, every round.
"""
import hashlib
import json
import os
import sys

from blindlib import build

B = sys.argv[1] if len(sys.argv) > 1 else os.environ.get('UC_BLIND', 'blind')
KEY = os.path.expanduser(os.environ.get('UC_KEY', '~/.local/state/ui-hill-climb/key.json'))
key = json.load(open(KEY))
assert not os.path.exists(f'{B}/key.json'), 'key.json is inside the blind folder the critics read'


def digest(im, path):
    im.save(path, 'PNG', optimize=True)
    h = hashlib.sha256(open(path, 'rb').read()).hexdigest()
    os.unlink(path)
    return h


bad = []
for id_, rec in sorted(key.items()):
    narrow = rec.get('view') == 'narrow'
    sides = ('A',) if rec['kind'] == 'solo' else ('A', 'B')
    for side in sides:
        info = rec[side]
        p = f'{B}/{id_}.png' if rec['kind'] == 'solo' else f'{B}/{id_}/{side}.png'
        if not os.path.exists(p):
            bad.append(f'{id_} {side}: missing {p}')
            continue
        if hashlib.sha256(open(info['src'], 'rb').read()).hexdigest() != info['sha_src']:
            bad.append(f'{id_} {side}: source {info["src"]} changed since assembly')
            continue
        rebuilt = digest(build(info['src'], rec['screen'], info['side'], narrow),
                         f'{B}/.verify-{id_}{side}.png')
        if rebuilt != info['sha_blind']:
            bad.append(f'{id_} {side}: image does not match a rebuild from {info["src"]}')
    if rec['kind'] == 'control':
        if hashlib.sha256(open(f'{B}/{id_}/A.png', 'rb').read()).hexdigest() != \
           hashlib.sha256(open(f'{B}/{id_}/B.png', 'rb').read()).hexdigest():
            bad.append(f'{id_}: control A and B are not the same image')

for id_, rec in sorted(key.items()):
    if rec['kind'] == 'pair' and rec.get('mirror_of'):
        p = rec['mirror_of']
        if p not in key:
            bad.append(f'{id_}: mirror names missing partner {p}')
        elif key[p]['ours'] == rec['ours']:
            bad.append(f'{id_}: mirror of {p} has the same side assignment, so it measures nothing')

n_solo = sum(1 for r in key.values() if r['kind'] == 'solo')
n_pair = len(key) - n_solo
n_ctl = sum(1 for r in key.values() if r['kind'] == 'control')
n_mir = sum(1 for r in key.values() if r.get('mirror_of'))
if len(bad):
    print(f'FAIL: {len(bad)} problems')
    for b in bad[:40]:
        print(' ', b)
    sys.exit(1)
print(f'OK: {len(key)} items verified against the key — {n_pair} pairs '
      f'({n_ctl} controls, {n_mir} mirrors), {n_solo} singles')
