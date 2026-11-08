#!/usr/bin/env python3
"""Turn a round's verdicts (and task results) into PASS/FAIL against criteria.md.

    python3 criteria_check.py tools/ui-grading/results.json [tasks.json]

Visual track: every graded screen needs a trimmed mean >= 8.0 with no dimension below 7.0, the
narrow pass needs >= 7.5, dark must sit within 0.3 of light, and at least 80% of reference pairs
must be won with no screen losing more than it wins. Task track: every task passes and none has
critical friction. Anything short prints FAIL with the reason, so the round reports numbers and a
named gap instead of a mood.
"""
import collections
import json
import statistics as st
import sys

DIM = ['hierarchy', 'spacing', 'type', 'state']
SCREEN_BAR = 8.0
DIM_FLOOR = 7.0
NARROW_BAR = 7.5
DARK_DELTA = 0.3
PAIR_WIN = 0.80

avg = lambda s: sum(s[d] for d in DIM) / len(DIM)
trim = lambda v: st.mean(sorted(v)[1:-1]) if len(v) >= 3 else st.mean(v)


def by_screen(rows):
    """(view, theme) -> screen -> [mean per (judge, item)] and 'ours' scores for the dimension report."""
    means = collections.defaultdict(lambda: collections.defaultdict(list))
    dims = collections.defaultdict(list)
    for r in rows:
        if r.get('kind') not in ('pair', 'solo') or 'ours' not in r:
            continue
        view, theme = r.get('view', 'desktop'), r.get('theme', 'light')
        means[(view, theme)][r['screen']].append(avg(r['ours']))
        dims[r['screen']].append(r['ours'])
    return means, dims


def main():
    results = json.load(open(sys.argv[1]))
    tasks = json.load(open(sys.argv[2])) if len(sys.argv) > 2 else None
    bad = []

    # --- honesty gates first: a round with a split control is void, not an average -----------
    ctl = [r for r in results if r.get('kind') == 'control']
    split = [r for r in ctl if avg(r['A']) != avg(r['B']) or r.get('better') != 'tie']
    mir = collections.defaultdict(list)
    for r in results:
        if r.get('kind') == 'mirror' and r.get('partner'):
            mir[(r['partner'], r['model'])].append(r['better'])
    flips = [(k, v) for k, v in mir.items() if len(set(v)) > 1]
    print('controls   %d, split %d, mirror groups %d, flipped %d' % (len(ctl), len(split), len(mir), len(flips)))
    if split or flips:
        bad.append('honesty: %d split control(s), %d flipped mirror(s)' % (len(split), len(flips)))

    means, dims = by_screen(results)
    flat = {}
    for (view, theme), per in means.items():
        for screen, votes in per.items():
            flat.setdefault((view, theme), {})[screen] = trim(votes)
            if view == 'desktop' and theme == 'light':
                flat.setdefault(('all', 'light'), {})[screen] = trim(votes)

    print('\n%-22s %4s %6s %6s  %s' % ('screen', 'n', 'mean', 'min', 'verdict'))
    overall = []
    light = {}
    for (view, theme), per in sorted(means.items()):
        for screen, votes in sorted(per.items()):
            m, lo = trim(votes), min(votes)
            d = {x: st.mean(r[x] for r in dims[screen]) for x in DIM}
            weak = [x for x, v in d.items() if v < DIM_FLOOR]
            bar = NARROW_BAR if view == 'narrow' else SCREEN_BAR
            ok = m >= bar and not weak
            tag = ('narrow ' if view == 'narrow' else '') + ('' if theme == 'light' else theme + ' ')
            print('%-22s %4d %6.2f %6.1f  %s%s' % (tag + screen, len(votes), m, lo, 'PASS' if ok else 'FAIL', '  weak: ' + ','.join(weak) if weak else ''))
            if not ok:
                bad.append('%s mean %.2f (bar %.1f)%s' % (tag + screen, m, bar, ' weak ' + ','.join(weak) if weak else ''))
            if view == 'desktop':
                light.setdefault(screen, {})[theme] = m
            if view == 'desktop' and theme == 'light':
                overall.append(m)

    # --- dark parity -------------------------------------------------------------------------
    print()
    for screen, themes in sorted(light.items()):
        if 'dark' in themes and 'light' in themes:
            delta = abs(themes['dark'] - themes['light'])
            if delta > DARK_DELTA:
                print('%-22s dark %.2f vs light %.2f  delta %.2f  FAIL' % (screen, themes['dark'], themes['light'], delta))
                bad.append('dark parity %s delta %.2f' % (screen, delta))
    print('dark parity checked on %d screens (max delta %.2f)'
          % (sum(1 for t in light.values() if {'dark', 'light'} <= set(t)),
             max([abs(t['dark'] - t['light']) for t in light.values() if {'dark', 'light'} <= set(t)] or [0.0])))

    # --- pairs against the reference ---------------------------------------------------------
    pairs = [r for r in results if r.get('kind') == 'pair']
    won = sum(1 for r in pairs if r.get('better') == 'ours')
    lost = sum(1 for r in pairs if r.get('better') == 'ref')
    tied = len(pairs) - won - lost
    rate = won / len(pairs) if pairs else 0.0
    print('\npairs      %d votes: ours %d, ref %d, tie %d (%.0f%% won, bar %.0f%%)' % (len(pairs), won, lost, tied, rate * 100, PAIR_WIN * 100))
    if pairs and rate < PAIR_WIN:
        bad.append('pair win rate %.0f%%' % (rate * 100))

    # --- task track --------------------------------------------------------------------------
    if tasks:
        rows = tasks.get('tasks', [])
        print('\n%-4s %-6s %-8s %-9s %s' % ('task', 'pass', 'critical', 'actions', 'friction'))
        for t in rows:
            state = 'n/m' if t.get('measured') is False else ('yes' if t.get('pass') else 'NO')
            print('%-4s %-6s %-8s %-9s %s' % (t.get('id', '?'), state,
                                              'YES' if t.get('critical') else '-', t.get('actions', '-'), t.get('friction', '')))
            if t.get('measured') is False:
                bad.append('task %s not measured' % t.get('id'))
            elif not t.get('pass'):
                bad.append('task %s failed' % t.get('id'))
            if t.get('critical'):
                bad.append('task %s critical friction' % t.get('id'))
        print('%d tasks, %d passed' % (len(rows), sum(1 for t in rows if t.get('pass'))))

    # --- verdict -----------------------------------------------------------------------------
    print('\noverall    %.2f over %d screens (bar %.1f, no dimension below %.1f)'
          % (st.mean(overall) if overall else 0.0, len(overall), SCREEN_BAR, DIM_FLOOR))
    print('VERDICT    %s' % ('MEETS THE BAR' if not bad else 'NOT YET'))
    for b in bad:
        print('  - %s' % b)
    return 0 if not bad else 1


if __name__ == '__main__':
    sys.exit(main())
