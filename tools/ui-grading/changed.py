#!/usr/bin/env python3
"""Shrink a round to the frames that actually changed since the last graded round.

Usage: python3 changed.py TEMPLATE_ROUND.json PREV_SHOTS NEW_SHOTS OUT_ROUND.json
                          [--views desktop,narrow,dark]

TEMPLATE_ROUND.json is the full round list (the UC_ROUND file of the last full round: PAIRS, P7,
SOLO_L, SOLO_7, SOLO_D, MIRROR_D, MIRROR_7, CONTROLS, ref_same_names). The output is the same file
cut down to the frames whose pixels differ from PREV_SHOTS, ready for `UC_ROUND=OUT make_blind.py`.

Why exact pixels and not a tolerance: the capture is deterministic (an untouched screen comes back
byte-identical), so any difference at all is a real change, and a 0.04%-of-the-frame edit (one
button colour) is exactly the sort of thing a tolerance would wave through.

Controls and mirrors are kept, because they are what proves each judge is honest, but they are
drawn from the changed screens so they still cost one slot each rather than extra items.

Exit 0 and a written file when something changed; exit 2 and no file when nothing did, in which
case last round's verdicts stand and there is nothing to spend tokens on.
"""
import json
import sys

from PIL import Image, ImageChops

# list name -> (frame suffix, view, theme)
LISTS = {
    'PAIRS': ('-light.png', 'desktop'),
    'P7': ('-light-760.png', 'narrow'),
    'SOLO_L': ('-light.png', 'desktop'),
    'SOLO_7': ('-light-760.png', 'narrow'),
    'SOLO_D': ('-dark.png', 'dark'),
}


def differs(prev, new):
    try:
        a = Image.open(prev).convert('RGB')
    except FileNotFoundError:
        return True
    b = Image.open(new).convert('RGB')
    return a.size != b.size or ImageChops.difference(a, b).getbbox() is not None


def main():
    args = [a for a in sys.argv[1:] if not a.startswith('--')]
    views = {'desktop', 'narrow', 'dark'}
    for a in sys.argv[1:]:
        if a.startswith('--views='):
            views = set(a.split('=', 1)[1].split(','))
    if len(args) != 4:
        sys.exit(__doc__)
    template, prev, new, out = args
    full = json.load(open(template))

    keep, skipped = {}, []
    for lst, (suffix, view) in LISTS.items():
        keep[lst] = []
        for name in full.get(lst, []):
            if view not in views:
                continue
            if differs(f'{prev}/{name}{suffix}', f'{new}/{name}{suffix}'):
                keep[lst].append(name)
            else:
                skipped.append(f'{name} [{view}]')

    rnd = dict(full)
    rnd.update(keep)

    # controls and mirrors come from the changed desktop pairs, same counts as the template
    # where there are enough pairs to supply them.
    pool = keep['PAIRS']
    n_ctrl = min(len(full.get('CONTROLS', [])), len(pool))
    rnd['CONTROLS'] = [[n, 'desktop', f'{n}-light.png'] for n in pool[:n_ctrl]]
    md = [s for s in full.get('MIRROR_D', []) if s in pool]
    md += [s for s in pool if s not in md][:max(0, len(full.get('MIRROR_D', [])) - len(md))]
    rnd['MIRROR_D'] = md
    rnd['MIRROR_7'] = [s for s in full.get('MIRROR_7', []) if s in keep['P7']]
    # a narrow mirror needs a narrow pair; top up from changed narrow pairs
    rnd['MIRROR_7'] += [s for s in keep['P7'] if s not in rnd['MIRROR_7']][
        :max(0, len(full.get('MIRROR_7', [])) - len(rnd['MIRROR_7']))]

    n_items = sum(len(keep[k]) for k in keep)
    if n_items == 0:
        print('nothing changed since', prev, '- last round\'s verdicts stand; no round to run.')
        sys.exit(2)

    json.dump(rnd, open(out, 'w'), indent=0)
    total = sum(len(full.get(k, [])) for k in LISTS)
    print(f'{n_items} of {total} frames changed -> {out}')
    print(f'  skipped (pixel-identical): {", ".join(skipped) if skipped else "none"}')
    print(f'  controls {[c[0] for c in rnd["CONTROLS"]]}  mirrors {rnd["MIRROR_D"]} + {rnd["MIRROR_7"]}')
    print('next: UC_ROUND=%s make_blind.py ... ; score with scoreboard.py NEW --carry OLD' % out)


if __name__ == '__main__':
    main()
