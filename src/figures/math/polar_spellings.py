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
# what can be read two ways is refused (no values)
for typed in ["r = cos θ^2", "r = sin θ(1 + θ)", "r = sin -θ", "r = 2cos", "x = cos t^2, y = t",
              "x = sin t(e^cos t - 2cos 4t), y = t", "r = sin θ²", "r = cos θ²", "r = 1 - sin θ²", "r = sin θ^2", "r = sin 1/2θ", "x = sin t², y = t"]:
    cases.append({"kind": "refused", "typed": typed})
cases.append({"kind": "unsupported", "typed": "r = sinh θ"})
print(json.dumps(cases, ensure_ascii=False, indent=0))
