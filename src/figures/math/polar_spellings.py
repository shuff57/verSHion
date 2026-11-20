"""Independent oracles for how the polar figure reads and prints equations (polar_tests::spellings_and_printing_agree_with_sympy).

Run with a Python that has sympy:  python polar_spellings.py > polar_spellings.json
Each case is a typed equation and its meaning as a textbook reads it (sin 2θ = sin(2θ), cos θ/2 = (cos θ)/2, sin²θ = (sin θ)²),
worked by sympy at sample values. The Rust test reads each typed equation, compares its values, then reads back what the
figure PRINTS for it and compares again: what is printed must be the same curve.
"""
import json
import sympy as sp

th, t = sp.symbols('theta t', real=True)
F = {'sin': sp.sin, 'cos': sp.cos, 'tan': sp.tan, 'sec': sp.sec, 'csc': sp.csc, 'cot': sp.cot}
ARGS = [('θ', 1, ''), ('2θ', 2, ''), ('3θ', 3, ''), ('0.5θ', sp.Rational(1, 2), ''), ('4θ', 4, '')]
SAMPLES = [sp.Rational(3, 10), sp.Rational(7, 10), sp.Rational(11, 10), sp.Rational(19, 10), sp.Rational(27, 10), sp.Rational(41, 10), sp.Rational(53, 10)]


def vals(expr, var):
    out = []
    for s in SAMPLES:
        v = sp.N(expr.subs(var, s), 30)
        if v.has(sp.zoo, sp.nan) or not v.is_real or abs(v) > 1e6:
            out.append(None)
        else:
            out.append(float(v))
    return [[float(s), v] for s, v in zip(SAMPLES, out)]


cases = []


def add(kind, typed, expr, var=th):
    cases.append({"kind": kind, "typed": typed, "values": vals(expr, var)})


for name, f in F.items():
    for a_txt, k, _ in ARGS:
        u = f(k * th)
        a_br = a_txt
        # spellings without brackets, as a textbook prints them
        add("spelling", f"r = {name}{a_txt}", u)
        add("spelling", f"r = {name} {a_txt}", u)
        add("spelling", f"r = 2{name}{a_txt}", 2 * u)
        add("spelling", f"r = 1+2{name} {a_txt}", 1 + 2 * u)
        add("spelling", f"r = 3 - {name} {a_txt}/2", 3 - u / 2)
        add("spelling", f"r = {name} {a_txt} cos θ", u * sp.cos(th))
        add("spelling", f"r = {name}^2 {a_txt}", u ** 2)
        add("spelling", f"r = |{name} {a_txt}|", sp.Abs(u))
        # bracketed equations: what the figure prints for them must read back as the same curve
        add("printed", f"r = {name}({a_br})/2", u / 2)
        add("printed", f"r = 1 + {name}({a_br})/2", 1 + u / 2)
        add("printed", f"r = 3{name}({a_br})/2", 3 * u / 2)
        add("printed", f"r = {name}({a_br})^2", u ** 2)
        add("printed", f"r = 2/{name}({a_br})", 2 / u)
        add("printed", f"r = {name}({a_br})*2", 2 * u)
        add("printed", f"r = {name}({a_br})sin(θ)", u * sp.sin(th))
        add("printed", f"r = {name}({a_br}) + {name}(θ)/3", u + f(th) / 3)
        add("printed", f"r = 2 - {name}({a_br})", 2 - u)
for name, f in [('ln', sp.log), ('sqrt', sp.sqrt)]:
    add("spelling", f"r = {name} θ", f(th))
    add("spelling", f"r = 2{name}θ", 2 * f(th))
    add("spelling", f"r = {name} θ/2", f(th) / 2)
    add("printed", f"r = {name}(θ)/2", f(th) / 2)
    add("printed", f"r = {name}(θ/2)", f(th / 2))
add("spelling", "r = sin θ cos θ", sp.sin(th) * sp.cos(th))
add("spelling", "r = 4/(1-cos θ)", 4 / (1 - sp.cos(th)))
add("spelling", "r = 2cosθ + 2sinθ", 2 * sp.cos(th) + 2 * sp.sin(th))
add("spelling", "r = 3sin2θ", 3 * sp.sin(2 * th))
add("spelling", "r = 2 cos 2θ", 2 * sp.cos(2 * th))
add("spelling", "r = sin²θ", sp.sin(th) ** 2)
add("spelling", "r = cos πθ", sp.cos(sp.pi * th))
add("printed", "r = 2^sin(θ)", 2 ** sp.sin(th))
add("printed", "r = e^cos(θ)", sp.exp(sp.cos(th)))
add("printed", "r = e^(cos(θ))", sp.exp(sp.cos(th)))
add("printed", "r = sin(θ)θ", sp.sin(th) * th)
add("printed", "r = (1 + cos(θ))/2", (1 + sp.cos(th)) / 2)
add("printed", "r = sin(θ/2)", sp.sin(th / 2))
add("printed", "r = cos(2θ/3)", sp.cos(2 * th / 3))
add("printed", "r = cos(θ)cos(2θ)", sp.cos(th) * sp.cos(2 * th))
# parametric: x and y in t
for name, f in F.items():
    for a_txt, k in [('t', 1), ('2t', 2)]:
        u = f(k * t)
        cases.append({"kind": "spelling", "typed": f"x = {name} {a_txt}, y = 2{name}{a_txt}/3", "values": vals(u, t), "y": vals(2 * u / 3, t)})
        cases.append({"kind": "printed", "typed": f"x = {name}({a_txt})/2, y = {name}({a_txt})^2", "values": vals(u / 2, t), "y": vals(u ** 2, t)})
cases.append({"kind": "spelling", "typed": "x = cos t, y = sin t", "values": vals(sp.cos(t), t), "y": vals(sp.sin(t), t)})
cases.append({"kind": "spelling", "typed": "x = t - sin t, y = 1 - cos t", "values": vals(t - sp.sin(t), t), "y": vals(1 - sp.cos(t), t)})
# check 2: a spaced coefficient belongs to the argument (sin 2 θ = sin(2θ), sin pi θ = sin(πθ), sin 2 π θ = sin(2πθ))
for name, f in F.items():
    for k in [2, 3, 4]:
        u = f(k * th)
        add("spelling", f"r = {name} {k} θ", u)
        add("spelling", f"r = 4 {name} {k} θ", 4 * u)
        add("spelling", f"r = 1 + 2 {name} {k} θ", 1 + 2 * u)
        add("spelling", f"r = 3{name} {k} θ", 3 * u)
        add("spelling", f"r = {name}{k} θ", u)
    add("spelling", f"r = {name} pi θ", f(sp.pi * th))
    add("spelling", f"r = {name} 2 pi θ", f(2 * sp.pi * th))
    add("spelling", f"r = {name} π θ/4", f(sp.pi * th) / 4)
for name, f in [('cos', sp.cos), ('sin', sp.sin)]:
    for k in [2, 3]:
        cases.append({"kind": "spelling", "typed": f"x = cos {k} t, y = sin {k} t", "values": vals(sp.cos(k * t), t), "y": vals(sp.sin(k * t), t)})
# check 2: an exponent is read whole: e^-θ = e^(−θ), e^cos θ = e^(cos θ), e^2θ = e^(2θ), 2^-θ = 2^(−θ)
E = sp.E
for typed, expr in [
    ("r = e^-θ", sp.exp(-th)), ("r = e^-θ/4", sp.exp(-th) / 4), ("r = 2^-θ", 2 ** (-th)), ("r = e^(-θ)", sp.exp(-th)),
    ("r = e^-(θ/4)", sp.exp(-th / 4)), ("r = 3e^-0.2θ", 3 * sp.exp(-sp.Rational(1, 5) * th)),
    ("r = e^cos θ", sp.exp(sp.cos(th))), ("r = e^cos(θ)", sp.exp(sp.cos(th))), ("r = e^sin(θ)", sp.exp(sp.sin(th))), ("r = 2^sin θ", 2 ** sp.sin(th)),
    ("r = e^-cos θ", sp.exp(-sp.cos(th))), ("r = e^sin 2θ", sp.exp(sp.sin(2 * th))), ("r = e^cos θ - 2cos 4θ", sp.exp(sp.cos(th)) - 2 * sp.cos(4 * th)),
    ("r = e^cos(θ) - 2cos(4θ) + sin^5(θ/12)", sp.exp(sp.cos(th)) - 2 * sp.cos(4 * th) + sp.sin(th / 12) ** 5),
    ("r = e^cos θ - 2cos 4θ + sin^5(θ/12)", sp.exp(sp.cos(th)) - 2 * sp.cos(4 * th) + sp.sin(th / 12) ** 5),
    ("r = e^2θ", sp.exp(2 * th)), ("r = e^0.1θ", sp.exp(sp.Rational(1, 10) * th)), ("r = e^(θ/5)", sp.exp(th / 5)), ("r = e^θ", sp.exp(th)),
    ("r = θ^2", th ** 2), ("r = θ^-1", 1 / th), ("r = θ^0.5", sp.sqrt(th)), ("r = 2^θ", 2 ** th), ("r = e^θ/4", sp.exp(th) / 4),
    ("r = sin^3 θ", sp.sin(th) ** 3), ("r = cos²θ", sp.cos(th) ** 2), ("r = 1 - sin²θ", 1 - sp.sin(th) ** 2),
    ("r = e^(-θ)cos(θ)", sp.exp(-th) * sp.cos(th)), ("r = e^-θ cos θ", sp.exp(-th) * sp.cos(th)),
]:
    add("spelling", typed, expr)
for typed, X, Y in [
    ("x = e^-t cos t, y = e^-t sin t", sp.exp(-t) * sp.cos(t), sp.exp(-t) * sp.sin(t)),
    ("x = e^cos t, y = e^sin t", sp.exp(sp.cos(t)), sp.exp(sp.sin(t))),
    ("x = 2^-t, y = e^2t", 2 ** (-t), sp.exp(2 * t)),
    ("x = sin(t)(e^cos t - 2cos 4t), y = cos(t)(e^cos t - 2cos 4t)", sp.sin(t) * (sp.exp(sp.cos(t)) - 2 * sp.cos(4 * t)), sp.cos(t) * (sp.exp(sp.cos(t)) - 2 * sp.cos(4 * t))),
]:
    cases.append({"kind": "spelling", "typed": typed, "values": vals(X, t), "y": vals(Y, t)})
# check 3: a division by an unbracketed run of factors is refused (θ/2π: θ/(2π) or (θ/2)π?); a plain number over a plain
# number before a factor is the coefficient (1/2 cos θ = (1/2)cos θ); one factor after / is the whole denominator
NUMS = [('θ', lambda v: v, False), ('3θ', lambda v: 3 * v, False), ('sin θ', lambda v: sp.sin(v), False), ('cos(θ)', lambda v: sp.cos(v), False),
        ('1', lambda v: sp.Integer(1), True), ('2', lambda v: sp.Integer(2), True), ('e^θ', lambda v: sp.exp(v), False),
        ('(θ+1)', lambda v: v + 1, False), ('cos 2θ', lambda v: sp.cos(2 * v), False)]
# (spelling, value of the denominator alone or None, the coefficient reading n/d0 · rest for a plain number n, or None)
DENS = [('2', lambda v: 2, None), ('2π', None, lambda n, v: n / 2 * sp.pi), ('2θ', None, lambda n, v: n / 2 * v), ('π', lambda v: sp.pi, None),
        ('θ', lambda v: v, None), ('2 θ', None, lambda n, v: n / 2 * v), ('(2π)', lambda v: 2 * sp.pi, None), ('2cosθ', None, lambda n, v: n / 2 * sp.cos(v)),
        ('cosθ', lambda v: sp.cos(v), None), ('2 sin θ', None, lambda n, v: n / 2 * sp.sin(v)), ('πθ', None, None), ('4', lambda v: 4, None),
        ('(θ+2)', lambda v: v + 2, None), ('2(θ+1)', None, None), ('3t', None, lambda n, v: n / 3 * v)]
for polar in [True, False]:
    v = th if polar else t
    for n_txt, n_f, plain in NUMS:
        for d_txt, d_f, coef in DENS:
            if d_txt == '3t' and polar:
                continue
            num, den = n_txt, d_txt
            if not polar:
                num, den = num.replace('θ', 't'), den.replace('θ', 't')
            for pre, pre_f in [('', 0), ('1 + ', 1)]:
                typed = f"r = {pre}{num}/{den}" if polar else f"x = {pre}{num}/{den}, y = t"
                if d_f is not None:
                    expr = pre_f + n_f(v) / d_f(v)
                elif plain and coef is not None:
                    expr = pre_f + coef(n_f(v), v)
                else:
                    cases.append({"kind": "refused", "typed": typed})
                    continue
                if polar:
                    add("spelling", typed, expr)
                else:
                    cases.append({"kind": "spelling", "typed": typed, "values": vals(expr, t), "y": vals(t, t)})
for typed in ["r = e^(θ/2π)", "x = cos(t/2π), y = sin(t/2π)", "r = sin θ/2π", "r = θ/2π + 1", "r = θ / 2π"]:
    cases.append({"kind": "refused", "typed": typed})
# check 3: √2θ with nothing between is √(2θ) or √2·θ: refused; √2·θ prints so, apart from √(2θ)
for typed in ["r = √2θ", "r = √3θ", "r = 2√2θ", "r = √2θ cos θ", "r = √2cos θ", "x = √2t, y = t", "r = √2(θ+1)"]:
    cases.append({"kind": "refused", "typed": typed})
for typed, expr in [("r = √(2θ)", sp.sqrt(2 * th)), ("r = sqrt(2)θ", sp.sqrt(2) * th), ("r = √2 θ", sp.sqrt(2) * th), ("r = √(2)θ", sp.sqrt(2) * th),
                    ("r = 2√2 θ", 2 * sp.sqrt(2) * th), ("r = sqrt(2θ)", sp.sqrt(2 * th)), ("r = √θ", sp.sqrt(th)), ("r = √2·θ", sp.sqrt(2) * th),
                    ("r = √3 cos θ", sp.sqrt(3) * sp.cos(th)), ("r = sqrt(3)cos(θ)", sp.sqrt(3) * sp.cos(th)), ("r = √(2θ)cos θ", sp.sqrt(2 * th) * sp.cos(th))]:
    add("spelling", typed, expr)
# check 3, cheap: asin is arcsin; Pi is pi; θ^1/2 and sin(θ)^2 θ are refused; | sin θ| prints |sin θ|
for typed, expr in [("r = asin(θ/7)", sp.asin(th / 7)), ("r = acos(θ/7)", sp.acos(th / 7)), ("r = atan(θ)", sp.atan(th)), ("r = 2 + atan θ", 2 + sp.atan(th)),
                    ("r = sin 2Pi θ", sp.sin(2 * sp.pi * th)), ("r = SIN 2PI θ", sp.sin(2 * sp.pi * th)), ("r = cos Pi θ", sp.cos(sp.pi * th)),
                    ("r = | sin θ|", sp.Abs(sp.sin(th))), ("r = θ^2/4", th ** 2 / 4), ("r = θ^3/6 + 1", th ** 3 / 6 + 1), ("r = θ^(1/2)", sp.sqrt(th)), ("r = sin(θ)^2", sp.sin(th) ** 2)]:
    add("spelling", typed, expr)
for typed in ["r = θ^1/2", "r = 2θ^1/3", "r = sin(θ)^2 θ", "r = e^2 θ"]:
    cases.append({"kind": "refused", "typed": typed})
# check 4: an explicit multiplication after a division multiplies the quotient: θ/2*θ = (θ/2)θ, printed (θ/2)·θ; so do a
# bar and abs(…) after it: θ/2|θ| = (θ/2)|θ|, 1/2 abs(θ) = (1/2)|θ|
NUM5 = [('θ', lambda v: v), ('3θ', lambda v: 3 * v), ('sin θ', lambda v: sp.sin(v)), ('e^θ', lambda v: sp.exp(v)), ('θ^2', lambda v: v ** 2), ('1', lambda v: sp.Integer(1)), ('(1+θ)', lambda v: 1 + v)]
DEN5 = [('2', lambda v: 2), ('4', lambda v: 4), ('π', lambda v: sp.pi), ('θ', lambda v: v), ('cos θ', lambda v: sp.cos(v)), ('(2θ)', lambda v: 2 * v)]
OPS = ['*', '·', '×', '⋅', ' * ']
FOL = [('θ', lambda v: v), ('π', lambda v: sp.pi), ('cos θ', lambda v: sp.cos(v)), ('sin(θ)', lambda v: sp.sin(v)), ('(1+θ)', lambda v: 1 + v), ('2', lambda v: 2), ('e^θ', lambda v: sp.exp(v))]
for polar in [True, False]:
    v = th if polar else t
    for n_txt, n_f in NUM5:
        for d_txt, d_f in DEN5:
            for op in (OPS if polar else ['*', '·']):
                for f_txt, f_f in FOL:
                    body = f"{n_txt}/{d_txt}{op}{f_txt}"
                    if not polar:
                        body = body.replace('θ', 't')
                    expr = n_f(v) / d_f(v) * f_f(v)
                    if polar:
                        add("spelling", f"r = {body}", expr)
                    else:
                        cases.append({"kind": "spelling", "typed": f"x = {body}, y = t", "values": vals(expr, t), "y": vals(t, t)})
    for n_txt, n_f in NUM5:
        for d_txt, d_f in [('2', lambda v: 2), ('4', lambda v: 4), ('π', lambda v: sp.pi)]:
            for f_txt, f_f in [('|θ|', lambda v: sp.Abs(v)), ('|cos θ|', lambda v: sp.Abs(sp.cos(v))), (' abs(θ)', lambda v: sp.Abs(v)), ('|sin(θ)|', lambda v: sp.Abs(sp.sin(v)))]:
                body = f"{n_txt}/{d_txt}{f_txt}"
                if not polar:
                    body = body.replace('θ', 't')
                expr = n_f(v) / d_f(v) * f_f(v)
                if polar:
                    add("spelling", f"r = {body}", expr)
                else:
                    cases.append({"kind": "spelling", "typed": f"x = {body}, y = t", "values": vals(expr, t), "y": vals(t, t)})
for typed, expr in [("r = θ/2Π", None), ("r = 2Πθ", 2 * sp.pi * th), ("r = sin Πθ", sp.sin(sp.pi * th)), ("r = √θ θ", sp.sqrt(th) * th), ("r = √(θ)θ", sp.sqrt(th) * th),
                    ("r = sqrt(θ)θ", sp.sqrt(th) * th), ("r = √3 cos θ", sp.sqrt(3) * sp.cos(th)), ("x = t/2*cos(t), y = t/2*sin(t)", None)]:
    if expr is not None:
        add("spelling", typed, expr)
cases.append({"kind": "refused", "typed": "r = θ/2Π"})
cases.append({"kind": "spelling", "typed": "x = t/2*cos(t), y = t/2*sin(t)", "values": vals(t / 2 * sp.cos(t), t), "y": vals(t / 2 * sp.sin(t), t)})
# what can be read two ways is refused (no values)
for typed in ["r = cos θ^2", "r = sin θ(1 + θ)", "r = sin -θ", "r = 2cos", "x = cos t^2, y = t",
              "x = sin t(e^cos t - 2cos 4t), y = t", "r = sin θ²", "r = cos θ²", "r = 1 - sin θ²", "r = sin θ^2", "r = sin 1/2θ", "x = sin t², y = t"]:
    cases.append({"kind": "refused", "typed": typed})
cases.append({"kind": "unsupported", "typed": "r = sinh θ"})
print(json.dumps(cases, ensure_ascii=False, indent=0))
