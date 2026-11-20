"""Outside check of derivative-graphs f', f'' against the TYPED f's domain, with sympy/mpmath only (never the app's code).

gen DIR: write DIR/cases.jsonl (params JSON, one per line).
chk DIR: check DIR/out/<n>.svg and <n>.json from the ignored Rust dump test.

Use Python with sympy/mpmath installed. Rendering uses round8::dump_renders_for_an_outside_check,
DG_DUMP=DIR/cases.jsonl and DG_OUT=DIR/out. Wrap every cargo command with the workspace cargo lock.
Truth uses this independent typed real-expression tree and sympy/mpmath, never verSHion's evaluator.
"""
import json, re, sys, math
from pathlib import Path
import sympy as sp
import mpmath as mp

mp.mp.dps = 30
X = sp.Symbol('x', real=True)

FUNCS = [
 ("1^ln(x)",-3,3),
 ("0*ln(x)",-3,3),("(ln(x)^2+1)^0",-3,3),("0*sqrt(x)",-3,3),("e^(0*ln(x))",-3,3),
 # polynomials and their kin (no restriction: controls)
 ("x^3-3x",-3,3),("x^2",-3,3),("x^4-2x^2",-2,2),("x^3",-2,2),("2x^3-3x^2-12x",-3,4),("x^5-5x",-2,2),("(x-1)^2(x+2)",-3,3),
 ("x^4",-2,2),("-x^2+4x",-1,5),("x^3-6x^2+9x",-1,5),
 # rational
 ("1/x",-3,3),("1/x^2",-3,3),("1/(x-1)",-3,3),("x/(x^2-4)",-4,4),("1/(x^2+1)",-3,3),("x/(x^2+1)",-4,4),("(x^2-1)/(x-1)",-3,3),
 ("1/(x^2-3)",-3,3),("x^2/(x^2-2)",-3,3),("(x+1)/(x-2)",-3,5),("x+1/x",-3,3),("x^2/(x-1)",-3,4),("(x^3-8)/(x-2)",-3,4),("x/(x-3)^2",-1,6),
 ("1/(x(x-2))",-2,4),("(2x)/(x^2-9)",-5,5),
 # logs
 ("ln(x)",-3,5),("ln(4-x^2)",-3,3),("x*ln(x)",-2,3),("ln(x+2)",-3,3),("ln(x^2-1)",-3,3),("ln(sin(x))",-7,7),("ln(cos(x))",-4,4),
 ("x-ln(x)",-2,4),("ln(x)-x^2",-2,3),("ln(ln(x))",-1,5),("ln(1/x)",-3,3),("ln(2x)",-3,3),("ln(-x)",-3,3),("ln(3-x)",-2,5),
 ("log(x)",-3,5),("ln(x*(x-1))",-2,3),("2*ln(x)",-3,3),("ln(x)/x",-2,4),("x^2*ln(x)",-2,2),("ln(x)^2",-2,4),("sqrt(ln(x))",-1,4),
 ("ln(sqrt(x))",-2,4),("ln(x+1)-ln(x)",-2,4),("ln(x)*sin(x)",-3,6),("ln(x)*(x-2)",-2,4),("e^x*ln(x)",-2,2),("ln(x)+ln(4-x)",-1,5),
 ("ln(x)-ln(x-2)",-1,6),("ln(x^2+1)",-3,3),("ln(abs(x))",-3,3),("x^x",-2,2.5),("ln(x^2)",-3,3),("x/ln(x)",-1,4),("1/ln(x)",-1,4),
 ("ln(x)/x^2",-1,4),("ln(9-x^2)",-4,4),("ln(x^3)",-2,3),("ln(e^x+1)",-3,3),("x*ln(x)-x",-2,3),("ln(x^2-4x)",-2,7),
 ("ln(1+x)",-3,3),("ln(1-x^2)",-2,2),("ln(tan(x))",-3,3),("x^2-8ln(x)",-1,4),
 # roots
 ("sqrt(x)",-3,5),("sqrt(4-x^2)",-3,3),("sqrt(x^2-4)",-4,4),("x*sqrt(x)",-2,3),("1/sqrt(x)",-2,4),("sqrt(x+3)",-4,3),("sqrt(9-x)",0,12),
 ("x^(3/2)",-2,3),("x^(1/4)",-2,3),("(x^2-1)^(1/2)",-3,3),("sqrt(cos(x))",-4,4),("sqrt(sin(x))",-4,7),("x*sqrt(4-x^2)",-3,3),
 ("sqrt(x)*ln(x)",-2,4),("sqrt(x-1)+sqrt(3-x)",0,4),("1/sqrt(4-x^2)",-3,3),("sqrt(x)/(x-1)",-2,4),("sqrt(abs(x))",-3,3),
 ("x^(2/3)",-3,3),("x^(1/3)",-3,3),("(x-1)^(1/3)",-2,4),("x^(4/3)",-3,3),("x^(1/3)*(x-4)",-2,6),("x^(2/3)*(x-5)",-2,6),
 ("sqrt(x)-x",-2,4),("x/sqrt(x+1)",-2,4),("sqrt(x^2+1)",-3,3),("x*sqrt(1-x)",-2,2),("sqrt(1-x^2)+x",-2,2),("(4-x^2)^(3/2)",-3,3),
 ("sqrt(x^3)",-2,3),("x^(5/2)",-2,3),("sqrt(2x-x^2)",-1,3),("x^(1/2)-x^(1/4)",-1,4),
 # rewrites
 ("(sqrt(x))^2",-3,3),("e^(ln(x))",-3,3),("x^(1/2)*x^(1/2)",-3,3),("sqrt(x)*sqrt(x)",-3,3),("e^(2*ln(x))",-3,3),
 ("(x^(1/4))^4",-3,3),("sqrt(x)^4",-3,3),("sqrt(x^2)",-3,3),("ln(e^x)",-3,3),("x*e^(ln(x))",-3,3),("sqrt(x)^2+x^2",-3,3),
 ("e^(ln(x)+1)",-3,3),("(sqrt(x+1))^2",-3,3),("sqrt(x)^2*ln(x)",-2,3),
 # exponentials
 ("e^x",-3,3),("x*e^x",-4,2),("x*e^(-x)",-1,5),("e^(-x^2)",-3,3),("x^2*e^(-x)",-1,6),("e^x/x",-3,3),("(e^x-1)/x",-3,3),
 ("e^(1/x)",-3,3),("2^x",-3,3),("e^(sqrt(x))",-2,4),
 # trig
 ("sin(x)",-7,7),("cos(x)",-7,7),("tan(x)",-3,3),("sec(x)",-3,3),("csc(x)",-4,4),("cot(x)",-4,4),("sin(x)/cos(x)",-3,3),
 ("x*tan(x)",-3,3),("tan(x)^2",-3,3),("1/sin(x)",-4,4),("sin(x)/x",-7,7),("x+2sin(x)",-7,7),("sin(x)^2",-4,4),("sin(2x)",-4,4),
 ("x*sin(x)",-7,7),("abs(sin(x))",-4,7),("cos(x)/(1+sin(x))",-4,4),("sin(x)+cos(x)",-4,4),("tan(x/2)",-6,6),("sec(x)^2",-3,3),
 ("1/(1+cos(x))",-4,4),("sin(x)*cot(x)",-4,4),("x*cot(x)",-4,4),
 # absolute value
 ("abs(x)",-3,3),("abs(x^2-4)",-3,3),("abs(x-1)+abs(x+1)",-3,3),("x*abs(x)",-3,3),("abs(x)/x",-3,3),
 ("abs(x)^3",-2,2),("(x^2-1)/abs(x-1)",-3,3),("abs(x^2-1)/(x-1)",-3,3),("(x^2-4)/abs(x-2)",-1,5),("sqrt(2-x^2)",-2,2),
 ("x*sqrt(2-x^2)",-2,2),("abs(x-1)*(x+2)",-3,3),("e^abs(x)",-2,2),("cos(abs(x))",-4,4),("sin(x)*abs(x)",-4,4),("x^2*abs(x-3)",0,5),
 ("abs(x)*x^3",-2,2),("x*abs(x-1)",-2,3),("abs(sin(x))*x",-4,4),("sqrt(5-x^2)",-3,3),("sqrt(1/2-x^2)",-1,1),("abs(x)^2",-3,3),
 ("(x-2)^(1/3)*(x-2)",-1,5),("x*x^(1/3)",-3,3),("x^(3/5)*x",-3,3),("x^(1/3)*sin(x)",-3,3),("sqrt(x^4)",-3,3),("x*sqrt(abs(x))",-3,3),
 ("sin(x)*sqrt(abs(x))",-3,3),("x^(2/3)*x",-3,3),("x^2*x^(1/3)",-2,2),("x^(1/3)*x^2*x",-2,2),("(x+1)^(2/3)*(x+1)",-4,2),("2*(x-3)^(1/3)*(x-3)^2",0,6),
 ("x^(1/5)*x^2",-2,2),("sqrt(x^6)",-2,2),("x*abs(x)^(1/3)",-3,3),("exp(x)*x^(1/3)*x",-3,2),("-x*x^(1/3)",-3,3),("(x^2-1)^(4/3)",-2,2),
 ("x^(1/3)*(x-1)^2",-2,3),("x*(x-4)^(1/3)",0,7),("x*(x-2)^(2/3)",-1,5),("x^(2/3)*(x^2-4)",-3,3),("(x^2-1)^(2/3)",-2,2),("x*(x^2-1)^(1/3)",-2,2),
 ("(x-1)*x^(2/3)",-2,3),("3*(x-1)^(1/3)*(x-1)",-2,4),("-(x+2)^(2/3)*(x+2)",-5,1),
]


class NoValue(Exception):
    pass


def strict(e, x):
    """The typed tree evaluated in the reals only, node by node: any part with no real value means f has none."""
    if e.is_Symbol:
        return x
    if e.is_Number or e in (sp.E, sp.pi):
        return mp.mpf(sp.N(e, 40))
    a = [strict(u, x) for u in e.args]
    if isinstance(e, sp.Add):
        return mp.fsum(a)
    if isinstance(e, sp.Mul):
        r = mp.mpf(1)
        for v in a:
            r *= v
        return r
    if isinstance(e, sp.Pow):
        b, k = a
        kr = e.args[1].doit()
        if b == 0 and k <= 0:
            raise NoValue
        if b < 0:
            if kr.is_Integer:
                return b ** int(kr)
            if kr.is_Rational and kr.q % 2 == 1:
                r = abs(b) ** k
                return r if kr.p % 2 == 0 else -r
            raise NoValue
        return b ** k
    f = e.func
    u = a[0] if a else None
    if f == sp.log:
        if len(a) == 2:
            if a[0] <= 0:
                raise NoValue
            return mp.log(a[0]) / mp.log(a[1])
        if u <= 0:
            raise NoValue
        return mp.log(u)
    if f == sp.exp:
        return mp.exp(u)
    if f == sp.Abs:
        return abs(u)
    if f in (sp.sin, sp.cos):
        return (mp.sin if f == sp.sin else mp.cos)(u)
    if f in (sp.tan, sp.sec):
        c = mp.cos(u)
        if abs(c) < mp.mpf(10) ** -25:
            raise NoValue
        return mp.sin(u) / c if f == sp.tan else 1 / c
    if f in (sp.cot, sp.csc):
        sn = mp.sin(u)
        if abs(sn) < mp.mpf(10) ** -25:
            raise NoValue
        return mp.cos(u) / sn if f == sp.cot else 1 / sn
    raise RuntimeError("unknown node %r" % (e,))


def strict_def(tree, x):
    try:
        v = strict(tree, mp.mpf(x))
        return mp.isfinite(v)
    except (NoValue, ZeroDivisionError):
        return False


def to_sympy(t, evaluate=True):
    s = t.replace('^', '**')
    s = re.sub(r'(\d)([a-z(])', r'\1*\2', s)
    s = re.sub(r'\)\(', ')*(', s)
    s = re.sub(r'\bx\(', 'x*(', s)
    s = s.replace('ln(', 'LN(').replace('log(', 'LOG10(')
    loc = {'x': X, 'e': sp.E, 'pi': sp.pi, 'LN': sp.log, 'LOG10': lambda u: sp.log(u, 10), 'sqrt': sp.sqrt, 'abs': sp.Abs,
           'sec': sp.sec, 'csc': sp.csc, 'cot': sp.cot, 'sin': sp.sin, 'cos': sp.cos, 'tan': sp.tan}
    e = sp.sympify(s, locals=loc, evaluate=evaluate)
    return e


def real_roots(e):
    # x^(p/q) with q odd: the real root (as the app and the class read it)
    def rep(b, ex):
        if ex.is_Rational and ex.q % 2 == 1 and ex.q > 1:
            r = sp.Abs(b) ** ex
            return r if ex.p % 2 == 0 else sp.sign(b) * r
        return b ** ex
    return e.replace(lambda z: z.is_Pow and z.exp.is_Rational and z.exp.q % 2 == 1 and z.exp.q > 1, lambda z: rep(z.base, z.exp))


def real_value(fn, x):
    """The typed expression's real value at x (mpmath, 30 digits), None where it has none."""
    try:
        v = fn(mp.mpf(x))
    except (ZeroDivisionError, ValueError, OverflowError, TypeError, NameError):
        return None
    try:
        v = mp.mpc(v)
    except Exception:
        return None
    if not (mp.isfinite(v.real) and mp.isfinite(v.imag)):
        return None
    if abs(v.imag) > mp.mpf(10) ** -20 * (1 + abs(v.real)):
        return None
    return float(v.real)


def lam(e):
    return sp.lambdify(X, e, modules=[{'sign': lambda u: mp.sign(u), 'Abs': lambda u: abs(u)}, 'mpmath'])


def gen(path):
    Path(path, 'out').mkdir(parents=True, exist_ok=True)
    assert len(set(t for t, _, _ in FUNCS)) >= 150
    with open(path + '/cases.jsonl', 'w') as fh:
        for (t, a, b) in FUNCS:
            fh.write(json.dumps({"f": t, "xmin": a, "xmax": b, "show_fpp": True, "tracer": False}) + "\n")
    print(len(FUNCS), "cases")


COLS = {1: "#c0392b", 2: "#2a8a3e"}


def polylines(svg, k, col):
    i = svg.find('data-panel="%d"' % k)
    if i < 0:
        return []
    rest = svg[i:]
    j = rest.find('data-panel="', 1)
    body = rest if j < 0 else rest[:j]
    out = []
    for part in body.split('<polyline ')[1:]:
        head = part[:part.find('>')]
        if ('stroke="%s"' % col) not in head:
            continue
        m = re.search(r'points="([^"]*)"', head)
        if not m:
            continue
        pts = [tuple(map(float, q.split(','))) for q in m.group(1).split()]
        if len(pts) > 1:
            out.append(pts)
    return out


def segd(p, a, b):
    dx, dy = b[0] - a[0], b[1] - a[1]
    l2 = dx * dx + dy * dy
    t = 0 if l2 == 0 else max(0, min(1, ((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / l2))
    return math.hypot(p[0] - a[0] - t * dx, p[1] - a[1] - t * dy)


def typed_limit_derivative(tree, x, order):
    """Check adjoining typed values where symbolic Abs/sign derivatives are unreliable.

    For example cos(|x|)=cos(x) has f''(0)=-1, while exp(|x|) has no f'(0)
    and therefore no f''(0). Dropping DiracDelta from a symbolic formula cannot decide this.
    """
    with mp.workdps(70):
        x = mp.mpf(x)
        try:
            f0 = strict(tree, x)
            result = None
            for k in range(1, order + 1):
                limits = []
                for side in (-1, 1):
                    values = []
                    for scale in ('1e-4', '1e-6', '1e-8'):
                        h = side * mp.mpf(scale) * (1 + abs(x))
                        if k == 1:
                            v = (strict(tree, x+h)-f0)/h
                        else:
                            v = (strict(tree, x+2*h)-2*strict(tree, x+h)+f0)/(h*h)
                        if not mp.isfinite(v):
                            return None
                        values.append(v)
                    d0, d1 = values[1]-values[0], values[2]-values[1]
                    if abs(d1) <= mp.mpf('1e-9') * (1+abs(values[2])):
                        limit = values[2]
                    elif d0 and abs(d1/d0) < mp.mpf('0.5'):
                        ratio = d1/d0
                        limit = (values[2]-ratio*values[1])/(1-ratio)
                    else:
                        return None
                    limits.append(limit)
                if abs(limits[0]-limits[1]) > mp.mpf('1e-6')*(1+abs(limits[0])+abs(limits[1])):
                    return None
                result = (limits[0]+limits[1])/2
            return float(result)
        except (NoValue, ZeroDivisionError, ValueError, OverflowError):
            return None


def rounded_vertex_matches(derivative, x, y, ux, uy):
    """SVG writes two decimals: find a true point within that x rounding interval.

    Solve within connected subintervals and verify the residual, rather than accepting the
    range across a pole. Fixed samples can miss a steep but correct curve between samples.
    """
    xs = [x-0.0051/ux, x, x+0.0051/ux]
    vals = [derivative(t) for t in xs]
    for a,b,va,vb in zip(xs,xs[1:],vals,vals[1:]):
        if va is None or vb is None or (va-y)*(vb-y)>0:
            continue
        for _ in range(60):
            mid = (a+b)/2
            v = derivative(mid)
            if v is None:
                break
            if abs(v-y)*uy <= 0.1:
                return True
            if (v-y)*(va-y)>0:
                a,va = mid,v
            else:
                b,vb = mid,v
    return False


def chk(path):
    bad_dom, bad_val, missing, pts, present, slow = [], [], [], 0, 0, []
    for n, (t, a, b) in enumerate(FUNCS):
        if n % 20 == 0:
            print(f"checked {n}/{len(FUNCS)} functions; domain faults={len(bad_dom)}, wrong values={len(bad_val)}, missing={len(missing)}", flush=True)
        svg = open(f'{path}/out/{n}.svg').read()
        meta = json.load(open(f'{path}/out/{n}.json'))
        if meta['ms'] > 500:
            slow.append((t, meta['ms']))
        e = real_roots(to_sympy(t))
        f0 = lam(e)
        ds = {k: sp.diff(e, X, k).replace(sp.DiracDelta, lambda *a: sp.Integer(0)) for k in (1, 2)}
        fd = {k: lam(v) for k, v in ds.items()}
        tree = to_sympy(t, evaluate=False)
        fdef = lambda x: strict_def(tree, x)
        critical = []
        for node in sp.preorder_traversal(tree):
            arg = node.args[0] if node.func == sp.Abs else node.base if node.is_Pow and node.exp.doit().is_Rational and not node.exp.doit().is_Integer else None
            if arg is not None and arg not in critical:
                critical.append(arg)
        critical_fns = [lam(arg) for arg in critical]
        def derivative(k, x):
            if not fdef(x):
                return None
            if any(real_value(fn,x) == 0 for fn in critical_fns):
                return typed_limit_derivative(tree,x,k)
            return real_value(fd[k],x)
        for fi in (1, 2):
            g = meta['panels'][fi]
            lines = polylines(svg, fi, COLS[fi])
            x_of = lambda px: g['x0'] + (px - g['ml']) / g['ux']
            hpx = 1.0 / g['ux']
            ph_top, ph_bot = g['mt'], g['mt'] + (g['y1'] - g['y0']) * g['uy']
            for L in lines:
                for (px, py) in L:
                    x = x_of(px)
                    pts += 1
                    if not any(fdef(x + s * 0.5 * hpx) for s in (0, -1, 1)):
                        bad_dom.append((t, fi, round(x, 5)))
                        continue
                    if ph_top + 1 < py < ph_bot - 1:
                        y = g['y1'] - (py - g['mt']) / g['uy']
                        ok = False
                        for s in (-0.5, -0.25, 0, 0.25, 0.5):
                            xv = x + s * hpx
                            if not fdef(xv):
                                continue
                            v = derivative(fi,xv)
                            if v is not None and abs(v - y) * g['uy'] <= 2.0:
                                ok = True
                                break
                        if not ok:
                            ok = rounded_vertex_matches(lambda t: derivative(fi,t),x,y,g['ux'],g['uy'])
                        if not ok:
                            # (near a corner or the end of a stretch the drawn end may sit between samples)
                            if not all(fdef(x + s * 3 * hpx) for s in (-1, 1)):
                                continue
                            bad_val.append((t, fi, round(x, 5), round(y, 4)))
            for i in range(301):
                x = g['x0'] + (g['x1'] - g['x0']) * i / 300
                if not all(fdef(x + j * 3 * hpx) for j in range(-3, 4)):
                    continue
                v = derivative(fi,x)
                if v is None:
                    continue
                span = g['y1'] - g['y0']
                if not (g['y0'] + 0.02 * span < v < g['y1'] - 0.02 * span):
                    continue
                vl, vr = derivative(fi,x - 3 * hpx), derivative(fi,x + 3 * hpx)
                if vl is None or vr is None or abs(vl - vr) * g['uy'] > 40:
                    continue
                want = (g['ml'] + (x - g['x0']) * g['ux'], g['mt'] + (g['y1'] - v) * g['uy'])
                d = min((segd(want, w0, w1) for L in lines for w0, w1 in zip(L, L[1:])), default=1e9)
                present += 1
                if d > 3:
                    missing.append((t, fi, round(x, 4), round(v, 4), round(d, 1)))
    print(f"{len(FUNCS)} functions, {pts} drawn f'/f'' points, {present} presence checks")
    print("drawn where typed f has no value:", len(bad_dom))
    for q in sorted(set((b[0], b[1]) for b in bad_dom)):
        print("   ", q, [b[2] for b in bad_dom if (b[0], b[1]) == q][:4])
    print("wrong value:", len(bad_val))
    for q in sorted(set((b[0], b[1]) for b in bad_val)):
        print("   ", q, [b[2:] for b in bad_val if (b[0], b[1]) == q][:4])
    print("missing:", len(missing))
    for q in sorted(set((b[0], b[1]) for b in missing)):
        print("   ", q, [b[2:] for b in missing if (b[0], b[1]) == q][:4])
    print("renders over 500 ms:", slow)
    maximum = max(json.load(open(f'{path}/out/{n}.json'))['ms'] for n in range(len(FUNCS)))
    print("max render ms:", maximum)
    result = dict(functions=len(FUNCS), drawn_points=pts, presence_checks=present,
                  outside_domain=bad_dom, wrong_values=bad_val, missing=missing,
                  slow=slow, max_render_ms=maximum)
    Path(path, 'summary.json').write_text(json.dumps(result, indent=2))
    if bad_dom or bad_val or missing or slow:
        raise SystemExit(1)


if __name__ == '__main__':
    {'gen': gen, 'chk': chk}[sys.argv[1]](sys.argv[2])
