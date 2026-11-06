#!/usr/bin/env python3
# Capture our frames from a throwaway copy of the fixture store. Usage: capture.py <outdir> [name-filter]
import json, os, shutil, subprocess, sys, tempfile, concurrent.futures as cf
BIN = os.environ.get('VS_BIN', 'target/debug/vershion')
SEED = os.environ.get('UC_SEED', '/tmp/vs-design-store')
FIX = os.environ.get('UC_FIXTURES')   # files copied into every seeded store (e.g. submissions.json)
OUT = sys.argv[1]; FILT = sys.argv[2] if len(sys.argv) > 2 else ''
os.makedirs(OUT, exist_ok=True)
STYLES = {'paper-standard':'Standard','paper-classic':'Classic','paper-condensed':'Condensed','paper-exam-board':'ExamBoard'}
F = [ # name, store, env
 ('welcome','empty',{}),
 ('home-empty','empty',{'VERSHION_SCREEN':'home'}),
 ('home','seed',{'VERSHION_SCREEN':'home'}),
 ('banks','seed',{'VERSHION_SCREEN':'banks'}),
 ('bank','seed',{'VERSHION_SCREEN':'bank'}),
 ('question-editor-bank','seed',{'VERSHION_SCREEN':'bank','VERSHION_OPEN_Q':'1'}),
 ('exams','seed',{'VERSHION_SCREEN':'exams'}),
 ('exam','seed',{'VERSHION_SCREEN':'exam'}),
 ('imports','seed',{'VERSHION_SCREEN':'imports'}),
 ('figures','seed',{'VERSHION_SCREEN':'figures'}),
 ('figure-editor','seed',{'VERSHION_SCREEN':'figures','VERSHION_FIG':'collision-momentum'}),
 ('settings','seed',{'VERSHION_SCREEN':'settings'}),
 ('shared','seed',{'VERSHION_SCREEN':'shared'}),
 ('account','seed',{'VERSHION_SCREEN':'account'}),
] + [(n,'seed',{'VERSHION_SCREEN':'exam','VS_STYLE':s}) for n,s in STYLES.items()]
EXTRA = json.loads(os.environ.get('VS_EXTRA', '[]'))   # extra (name, store, env) frames from a JSON file
F = ([] if os.environ.get('VS_ONLY_EXTRA') else F) + [tuple(x) for x in EXTRA]
def one(job):
    (name, store, env), theme, size = job
    tag = f"{name}-{theme}" + ('' if size=='1280x800' else '-760')
    out = os.path.join(OUT, tag + '.png')
    d = tempfile.mkdtemp(prefix='vs-run-')
    try:
        s = os.path.join(d, 's')
        if store == 'seed':
            shutil.copytree(SEED, s)
            if FIX:
                for fn in os.listdir(FIX): shutil.copy(os.path.join(FIX, fn), s)
        else: os.makedirs(s)
        json.dump({'dark': theme=='dark'}, open(os.path.join(s,'settings.json'),'w'))
        e = dict(os.environ); e.update(VERSHION_STORE_DIR=s, VERSHION_WINSIZE=size, VERSHION_SHOT=out, VERSHION_SHOT_FRAME=env.get('VERSHION_SHOT_FRAME','90'))
        for k,v in env.items():
            if k=='VS_STYLE':
                p=os.path.join(s,'exams.json'); x=json.load(open(p)); x[0]['paper_style']=v; json.dump(x,open(p,'w'))
            else: e[k]=v
        r = subprocess.run(['xvfb-run','-a',BIN],env=e,capture_output=True,timeout=170,text=True)
        return tag, os.path.exists(out), r.returncode, r.stderr[-160:] if not os.path.exists(out) else ''
    finally:
        shutil.rmtree(d, ignore_errors=True)
jobs = []
for f in F:
    if FILT and FILT not in f[0]: continue
    for theme in ('light','dark'):
        jobs.append((f,theme,'1280x800'))
    jobs.append((f,'light','760x800'))
with cf.ThreadPoolExecutor(6) as ex:
    bad = 0
    for tag, ok, rc, err in ex.map(one, jobs):
        if not ok: bad += 1; print('MISSING', tag, rc, err)
print('done', len(jobs), 'jobs', bad, 'missing')
