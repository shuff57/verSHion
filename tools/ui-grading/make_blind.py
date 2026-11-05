#!/usr/bin/env python3
"""Assemble the blind grading set from a shots directory and the reference screenshots.

Usage: python3 make_blind.py [shots_dir] [out_dir]

Round 3 was voided because two runs of the old script raced: one rewrote the key while the other
rewrote the directory, so 6 of 21 pairs were graded against the wrong screen and the "control"
(one image twice) was actually two different images. Three things stop that here:

  * a lock file, so a second run refuses instead of interleaving;
  * assembly into `<out>.tmp`, then one rename, so the directory is never half written;
  * the key records each side's source path, screen, view and role, so `verify_blind.py` can
    rebuild every image byte for byte and prove the directory matches the key.

Also adds a second control and five mirrors (the same pair with A and B swapped). A judge that
keeps preferring the left-hand image is only visible in the mirrors.
"""
import hashlib
import json
import os
import random
import secrets
import shutil
import sys

from blindlib import build, save

# ---- config: point these at your own screenshot and reference directories, and list the screens
# your app has (PAIRS = screens the reference also has; SOLO_* = screens only yours has).
SH = sys.argv[1] if len(sys.argv) > 1 else os.environ.get('UC_SHOTS', 'shots')
B = sys.argv[2] if len(sys.argv) > 2 else os.environ.get('UC_BLIND', 'blind')
T = os.environ.get('UC_REFS', 'refs/desktop')
T7 = os.environ.get('UC_REFS_NARROW', 'refs/narrow')
KEY = os.path.expanduser(os.environ.get('UC_KEY', '~/.local/state/ui-hill-climb/key.json'))

PAIRS = ['banks', 'bank', 'question-editor-bank', 'question-editor-exam', 'exams', 'exam',
         'paper-standard', 'paper-classic', 'paper-condensed', 'paper-exam-board', 'export',
         'imports']
P7 = ['banks', 'bank', 'question-editor-bank', 'question-editor-exam', 'exams', 'exam',
      'export', 'imports']
SOLO_L = ['welcome', 'home-empty', 'home', 'figures', 'figure-editor', 'settings', 'shared',
          'circles', 'circle-open', 'account']
SOLO_7 = ['welcome', 'home-empty', 'home', 'figures', 'figure-editor', 'settings', 'shared',
          'circles', 'account']
SOLO_D = ['welcome', 'home-empty', 'home', 'banks', 'bank', 'question-editor-bank',
          'question-editor-exam', 'exams', 'exam', 'paper-standard', 'export', 'imports',
          'figures', 'figure-editor', 'settings', 'shared', 'circles', 'circle-open', 'account']

# Screens whose pair is shown twice, the second time with A and B swapped.
MIRROR_D = ['banks', 'exam', 'export']
MIRROR_7 = ['banks', 'exam']
CONTROLS = [('exams', 'desktop', 'exams-light.png'), ('bank', 'desktop', 'bank-light.png')]


def sha(path):
    return hashlib.sha256(open(path, 'rb').read()).hexdigest()


def main():
    lock = B + '.lock'
    if os.path.exists(lock):
        sys.exit(f'refusing to run: {lock} exists (another make_blind may be running; '
                 f'delete it only if you are sure it is stale)')
    with open(lock, 'x') as f:
        f.write(str(os.getpid()))
    try:
        run()
    finally:
        os.unlink(lock)


def run():
    pairs, singles = [], []
    for f in PAIRS:
        pairs.append(dict(screen=f, view='desktop', ours=f'{SH}/{f}-light.png', ref=f'{T}/{f}.png'))
    for f in P7:
        pairs.append(dict(screen=f, view='narrow', ours=f'{SH}/{f}-light-760.png',
                          ref=f'{T7}/{f}.png'))
    for f in SOLO_L:
        singles.append(dict(screen=f, view='desktop', theme='light', ours=f'{SH}/{f}-light.png'))
    for f in SOLO_7:
        singles.append(dict(screen=f, view='narrow', theme='light',
                            ours=f'{SH}/{f}-light-760.png'))
    for f in SOLO_D:
        singles.append(dict(screen=f, view='desktop', theme='dark', ours=f'{SH}/{f}-dark.png'))

    missing = [i['ours'] for i in pairs + singles if not os.path.exists(i['ours'])]
    missing += [i['ref'] for i in pairs if not os.path.exists(i['ref'])]
    assert not missing, f'missing sources: {missing}'

    rng = random.Random(secrets.randbits(64))
    rng.shuffle(pairs)
    rng.shuffle(singles)

    # ids are handed out before anything is written so a mirror can name its partner
    ids, n = [], 0
    for p in pairs:
        n += 1
        ids.append(('P%02d' % n, p))
    for name, view, fname in CONTROLS:
        n += 1
        ids.append(('P%02d' % n, dict(screen=name, view=view, control=True,
                                      ours=f'{SH}/{fname}', ref=f'{SH}/{fname}')))
    # every shown pair gets its side assignment now: a mirror is simply the opposite
    for _, it in ids:
        if not it.get('control'):
            it['flip'] = rng.random() < 0.5   # flip = the reference sits in slot A
    by_id = dict(ids)
    mirrors = []
    for screen, view in [(s, 'desktop') for s in MIRROR_D] + [(s, 'narrow') for s in MIRROR_7]:
        partner = next((i for i, it in ids if not it.get('control')
                        and it['screen'] == screen and it['view'] == view), None)
        if not partner:
            continue
        src = by_id[partner]
        mirrors.append((partner, dict(screen=screen, view=view, mirror_of=partner,
                                      ours=src['ours'], ref=src['ref'],
                                      flip=not src['flip'])))
    for partner, m in mirrors:
        n += 1
        ids.append(('P%02d' % n, m))

    tmp = B + '.tmp'
    shutil.rmtree(tmp, ignore_errors=True)
    os.makedirs(tmp + '/out')
    key, manifest = {}, []
    for id_, it in ids:
        d = f'{tmp}/{id_}'
        os.makedirs(d)
        narrow = it['view'] == 'narrow'
        ours = build(it['ours'], it['screen'], 'ours', narrow)
        rec = dict(kind='pair', screen=it['screen'], view=it['view'])
        if it.get('control'):
            rec['kind'] = 'control'
            save(ours, d + '/A.png')
            save(ours, d + '/B.png')
            for side in ('A', 'B'):
                rec[side] = dict(src=it['ours'], side='ours')
        else:
            ref = build(it['ref'], it['screen'], 'ref', narrow)
            flip = it['flip']
            a, b = (ref, ours) if flip else (ours, ref)
            rec['ours'] = 'B' if flip else 'A'
            rec['A'] = dict(src=(it['ref'] if flip else it['ours']),
                            side=('ref' if flip else 'ours'))
            rec['B'] = dict(src=(it['ours'] if flip else it['ref']),
                            side=('ours' if flip else 'ref'))
            save(a, d + '/A.png')
            save(b, d + '/B.png')
            if it.get('mirror_of'):
                rec['mirror_of'] = it['mirror_of']
        for side in ('A', 'B'):
            rec[side]['sha_src'] = sha(rec[side]['src'])
            rec[side]['sha_blind'] = sha(f'{d}/{side}.png')
        key[id_] = rec
        manifest.append(f'{id_}\tpair\t{d}/A.png\t{d}/B.png')

    for m, it in enumerate(singles, 1):
        id_ = 'S%02d' % m
        p = f'{tmp}/{id_}.png'
        save(build(it['ours'], it['screen'], 'ours', it['view'] == 'narrow'), p)
        key[id_] = dict(kind='solo', screen=it['screen'], view=it['view'], theme=it['theme'],
                        A=dict(src=it['ours'], side='ours', sha_src=sha(it['ours']),
                               sha_blind=sha(p)))
        manifest.append(f'{id_}\tsingle\t{p}')

    rng.shuffle(manifest)
    json.dump(key, open(tmp + '/key.json', 'w'), indent=0)
    os.chmod(tmp + '/key.json', 0o600)

    shutil.rmtree(B, ignore_errors=True)
    os.rename(tmp, B)
    # the manifest names the final paths, so it is written after the rename
    open(B + '/manifest.txt', 'w').write(
        '\n'.join(line.replace(tmp, B) for line in manifest) + '\n')
    shutil.copyfile(B + '/key.json', KEY)
    os.chmod(KEY, 0o600)
    npair = sum(1 for k in key.values() if k['kind'] != 'solo')
    print(f'{len(key)} items: {npair} pairs ({len(CONTROLS)} controls, {len(mirrors)} mirrors), '
          f'{len(singles)} singles -> {B}')
    print('next: python3 verify_blind.py   (nothing is graded until it passes)')


if __name__ == '__main__':
    main()
