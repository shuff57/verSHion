"""Independent oracles for the polar and parametric figure (polar_tests::the_sweep_agrees_with_sympy).

Run with a Python that has sympy and mpmath:  python polar_oracles.py > polar_oracles.json
Nothing here uses the app: points, slopes and areas are worked by sympy (exactly, simplified) and checked by mpmath.
"""
import json
import itertools
import sympy as sp
import mpmath as mp

mp.mp.dps = 40
th, t = sp.symbols('theta t', real=True)
PI = sp.pi

# polar curves: (typed, sympy)
polar = []
for a, b in [(0, 1), (0, 2), (0, 3), (1, 1), (2, 2), (1, 2), (2, 1), (3, 2), (1, 3), (4, 0)]:
    for kind in ['cos', 'sin']:
        for k in [1, 2, 3]:
            if b == 0 and (kind == 'sin' or k > 1):
                continue
            fn = sp.cos if kind == 'cos' else sp.sin
            expr = a + b * fn(k * th)
            ks = '' if k == 1 else str(k)
            parts = []
            if a:
                parts.append(str(a))
            if b:
                bs = '' if b == 1 else str(b)
                parts.append(f"{bs}{kind}({ks}θ)")
            polar.append((' + '.join(parts), expr))
polar += [
    ('θ', th), ('2θ', 2 * th), ('θ/2', th / 2), ('θ^2/4', th ** 2 / 4),
    ('sin(θ/2)', sp.sin(th / 2)), ('cos(2θ/3)', sp.cos(2 * th / 3)), ('2 - 2sin(θ)', 2 - 2 * sp.sin(th)),
    ('3 - 2cos(θ)', 3 - 2 * sp.cos(th)), ('1 - 2sin(θ)', 1 - 2 * sp.sin(th)), ('4sin(θ)', 4 * sp.sin(th)),
    ('2cos(θ) + 2sin(θ)', 2 * sp.cos(th) + 2 * sp.sin(th)), ('cos(θ)^2', sp.cos(th) ** 2), ('1 + sin(θ)^2', 1 + sp.sin(th) ** 2),
    ('e^(θ/5)', sp.exp(th / 5)), ('2sqrt(cos(2θ))', 2 * sp.sqrt(sp.cos(2 * th))), ('1/(1 - cos(θ))', 1 / (1 - sp.cos(th))),
    ('2/(1 + cos(θ))', 2 / (1 + sp.cos(th))), ('3sec(θ)', 3 / sp.cos(th)), ('5cos(4θ)', 5 * sp.cos(4 * th)), ('0.5 + cos(θ)', sp.Rational(1, 2) + sp.cos(th)),
]
for k in [4, 5, 6]:
    polar.append((f"2sin({k}θ)", 2 * sp.sin(k * th)))
    polar.append((f"2cos({k}θ)", 2 * sp.cos(k * th)))
for a_, b_ in [(3, 1), (4, 3), (5, 2), (1, 4)]:
    polar.append((f"{a_} + {b_}cos(θ)".replace(" 1cos", " cos"), a_ + b_ * sp.cos(th)))
    polar.append((f"{a_} - {b_}sin(θ)".replace(" 1sin", " sin"), a_ - b_ * sp.sin(th)))
polar += [('θ + 1', th + 1), ('3θ', 3 * th), ('e^(θ/3)', sp.exp(th / 3)), ('sqrt(θ)', sp.sqrt(th)), ('1/θ', 1 / th), ('θcos(θ)', th * sp.cos(th)),
          ('cos(θ) - sin(θ)', sp.cos(th) - sp.sin(th)), ('sin(3θ) + 1', sp.sin(3 * th) + 1), ('2 + sin(4θ)', 2 + sp.sin(4 * th)), ('tan(θ)', sp.tan(th))]
param = [
    ('3cos(t)', '3sin(t)', 3 * sp.cos(t), 3 * sp.sin(t)), ('2cos(t)', 'sin(t)', 2 * sp.cos(t), sp.sin(t)),
    ('t', 't^2', t, t ** 2), ('t^2', 't^3', t ** 2, t ** 3), ('48t', '64t - 16t^2', 48 * t, 64 * t - 16 * t ** 2),
    ('t - sin(t)', '1 - cos(t)', t - sp.sin(t), 1 - sp.cos(t)), ('cos(t)^3', 'sin(t)^3', sp.cos(t) ** 3, sp.sin(t) ** 3),
    ('2t + 1', '3 - t', 2 * t + 1, 3 - t), ('sin(2t)', 'cos(t)', sp.sin(2 * t), sp.cos(t)), ('e^t', 'e^(-t)', sp.exp(t), sp.exp(-t)),
    ('cos(t) + t', 'sin(t)', sp.cos(t) + t, sp.sin(t)), ('t^3 - 3t', 't^2', t ** 3 - 3 * t, t ** 2),
]
angles = [('0', 0), ('pi/6', PI / 6), ('pi/4', PI / 4), ('pi/3', PI / 3), ('pi/2', PI / 2), ('2pi/3', 2 * PI / 3), ('3pi/4', 3 * PI / 4),
          ('pi', PI), ('5pi/4', 5 * PI / 4), ('3pi/2', 3 * PI / 2), ('7pi/4', 7 * PI / 4), ('1', sp.Integer(1)), ('0.5', sp.Rational(1, 2)), ('9pi/4', 9 * PI / 4)]
tvals = [('0', 0), ('1', sp.Integer(1)), ('2', sp.Integer(2)), ('pi/4', PI / 4), ('pi/2', PI / 2), ('pi', PI), ('-1', sp.Integer(-1)), ('0.5', sp.Rational(1, 2))]


def num(e):
    return float(sp.N(e, 30))


def defined(e):
    if e.has(sp.zoo, sp.nan, sp.oo, -sp.oo):
        return False
    v = complex(sp.N(e, 30))
    return abs(v.imag) < 1e-25 and abs(v.real) < 1e9


points, slopes, areas = [], [], []
for typed, f in polar:
    df = sp.diff(f, th)
    for at, a in angles:
        r = sp.simplify(f.subs(th, a))
        if not defined(r):
            points.append({"curve": f"r = {typed}", "at": at, "defined": False})
            continue
        x, y = sp.simplify(r * sp.cos(a)), sp.simplify(r * sp.sin(a))
        points.append({"curve": f"r = {typed}", "at": at, "defined": True, "r": num(r), "x": num(x), "y": num(y)})
        dr = sp.simplify(df.subs(th, a))
        if not defined(dr):
            slopes.append({"curve": f"r = {typed}", "at": at, "kind": "undefined"})
            continue
        dx = sp.simplify(dr * sp.cos(a) - r * sp.sin(a))
        dy = sp.simplify(dr * sp.sin(a) + r * sp.cos(a))
        zx, zy = sp.simplify(dx) == 0, sp.simplify(dy) == 0
        kind = "zero_over_zero" if zx and zy else "vertical" if zx else "finite"
        e = {"curve": f"r = {typed}", "at": at, "kind": kind, "dx": num(dx), "dy": num(dy)}
        if kind == "finite":
            e["slope"] = num(sp.simplify(dy / dx))
        slopes.append(e)
for tx, ty, X, Y in param:
    for at, a in tvals:
        x, y = sp.simplify(X.subs(t, a)), sp.simplify(Y.subs(t, a))
        if not (defined(x) and defined(y)):
            continue
        points.append({"curve": f"x = {tx}, y = {ty}", "at": at, "defined": True, "x": num(x), "y": num(y)})
        dx, dy = sp.simplify(sp.diff(X, t).subs(t, a)), sp.simplify(sp.diff(Y, t).subs(t, a))
        zx, zy = dx == 0, dy == 0
        kind = "zero_over_zero" if zx and zy else "vertical" if zx else "finite"
        e = {"curve": f"x = {tx}, y = {ty}", "at": at, "kind": kind, "dx": num(dx), "dy": num(dy)}
        if kind == "finite":
            e["slope"] = num(sp.simplify(dy / dx))
        slopes.append(e)

# areas: intervals where r keeps one sign (checked by mpmath on a fine grid) and r is defined throughout
intervals = [('0', 'pi/2', 0, PI / 2), ('0', 'pi', 0, PI), ('0', '2pi', 0, 2 * PI), ('pi/6', 'pi/3', PI / 6, PI / 3), ('-pi/4', 'pi/4', -PI / 4, PI / 4),
             ('0', 'pi/3', 0, PI / 3), ('pi/2', 'pi', PI / 2, PI), ('0', '1', 0, sp.Integer(1)), ('pi/4', '3pi/4', PI / 4, 3 * PI / 4), ('-pi/2', 'pi/2', -PI / 2, PI / 2)]


def sign_ok(fl, lo, hi):
    vals = []
    # a pole at an end or inside (tan at pi/2) makes the integral improper: no area
    for u in (lo, hi):
        if not defined(F_now.subs(th, sp.nsimplify(u, [sp.pi]))):
            return False
    for k in range(4001):
        u = lo + (hi - lo) * k / 4000
        try:
            v = fl(mp.mpf(u))
        except (ZeroDivisionError, ValueError):
            return False
        if not mp.isfinite(v) or abs(mp.im(v)) > 1e-20 or abs(v) > 1e6:
            return False
        vals.append(float(mp.re(v)))
    m = max(abs(v) for v in vals) or 1
    return all(v >= -1e-12 * m for v in vals) or all(v <= 1e-12 * m for v in vals)


for typed, f in polar:
    F_now = f
    fl = sp.lambdify(th, f, 'mpmath')
    for sa, sb, a, b in intervals:
        lo, hi = num(a), num(b)
        if not sign_ok(fl, lo, hi):
            continue
        val = mp.quad(lambda u: fl(u) ** 2, [lo, (lo + hi) / 2, hi]) / 2
        if val < 1e-20:
            continue
        areas.append({"curves": f"r = {typed}", "from": sa, "to": sb, "area": float(mp.re(val))})
# inside one, outside another (both r >= 0 and the first outside the second throughout)
C, S = sp.cos(th), sp.sin(th)
pairs = [('3cos(θ)', '1 + cos(θ)', 3 * C, 1 + C, '-pi/3', 'pi/3', -PI / 3, PI / 3), ('3sin(θ)', '1 + sin(θ)', 3 * S, 1 + S, 'pi/6', '5pi/6', PI / 6, 5 * PI / 6),
         ('2', '2 - 2cos(θ)', sp.Integer(2), 2 - 2 * C, '-pi/2', 'pi/2', -PI / 2, PI / 2), ('4', '2 + 2cos(θ)', sp.Integer(4), 2 + 2 * C, 'pi/2', '3pi/2', PI / 2, 3 * PI / 2),
         ('1 + cos(θ)', '1', 1 + C, sp.Integer(1), '-pi/2', 'pi/2', -PI / 2, PI / 2), ('2 + 2sin(θ)', '2', 2 + 2 * S, sp.Integer(2), '0', 'pi', 0, PI),
         ('3', '2', sp.Integer(3), sp.Integer(2), '0', '2pi', 0, 2 * PI), ('2sin(2θ)', '1', 2 * sp.sin(2 * th), sp.Integer(1), 'pi/12', '5pi/12', PI / 12, 5 * PI / 12),
         ('4cos(θ)', '2', 4 * C, sp.Integer(2), '-pi/3', 'pi/3', -PI / 3, PI / 3), ('6sin(θ)', '3', 6 * S, sp.Integer(3), 'pi/6', '5pi/6', PI / 6, 5 * PI / 6)]
for fi, gi, F, G, sa, sb, a, b in pairs:
    exact = sp.simplify(sp.integrate((F ** 2 - G ** 2) / 2, (th, a, b)))
    check = mp.quad(sp.lambdify(th, (F ** 2 - G ** 2) / 2, 'mpmath'), [mp.mpf(str(sp.N(a, 45))), mp.mpf(str(sp.N(b, 45)))])
    assert abs(check - sp.N(exact, 40)) < 1e-25, (fi, gi)
    areas.append({"curves": f"r = {fi}\nr = {gi}", "from": sa, "to": sb, "area": num(exact), "exact": str(exact)})

print(json.dumps({"points": points, "slopes": slopes, "areas": areas, "curves": len(polar) + len(param)}, ensure_ascii=False, indent=0))
