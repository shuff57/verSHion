//! Crossings solved by hand, including irrational limits and numerical-only
//! transcendental roots. All equalities below are tested against those roots.
use super::*;
use super::tests::{s, with};
fn root(n: i128) -> Ex { Ex::int(n).pow_rat(Rat::new(1, 2).unwrap()).unwrap() }
fn fraction(x: Ex, n: i128, d: i128) -> Ex { x.scale(Rat::new(n,d).unwrap()).unwrap() }
fn plus(a: Ex, b: Ex) -> Ex { a.add(&b).unwrap() }
fn texts(svg: &str, kind: &str) -> Vec<String> {
    let mut out = vec![];
    let pat = format!("<text data-rs=\"{kind}\"");
    let mut rest = svg;
    while let Some(k) = rest.find(&pat) {
        rest = &rest[k..];
        let end = rest.find("</text>").unwrap();
        let mut text = String::new(); let mut tag = false;
        for c in rest[rest.find('>').unwrap()+1..end].chars() { match c { '<' => tag = true, '>' => tag = false, _ if !tag => text.push(c), _ => {} } }
        out.push(text.replace("&amp;", "&").replace("&lt;", "<").replace("&gt;", ">"));
        rest = &rest[end..];
    }
    out
}
struct Pair { f: &'static str, g: &'static str, roots: Vec<Ex>, numerical: Vec<f64>, xmin: f64 }
fn pairs() -> Vec<Pair> {
    let q = |f, g, roots| Pair { f, g, roots, numerical: vec![], xmin: -3. };
    vec![
        // x²=2; x²-x-1=0; 2x²=3; 2x²=4.
        q("x^2", "2", vec![root(2).neg(),root(2)]),
        q("x+1", "x^2", vec![fraction(plus(Ex::int(1),root(5).neg()),1,2),fraction(plus(Ex::int(1),root(5)),1,2)]),
        q("4-x^2", "x^2+1", vec![fraction(root(6),-1,2),fraction(root(6),1,2)]),
        q("3-x^2", "x^2-1", vec![root(2).neg(),root(2)]),
        // x³-2x=x(x²-2); x³-x²-x=x(x²-x-1).
        q("x", "x^3-x", vec![root(2).neg(),Ex::zero(),root(2)]),
        q("x^2", "x^3-x", vec![fraction(plus(Ex::int(1),root(5).neg()),1,2),Ex::zero(),fraction(plus(Ex::int(1),root(5)),1,2)]),
        q("x", "x^3", vec![Ex::int(-1),Ex::zero(),Ex::int(1)]),
        q("x^2", "x^3", vec![Ex::zero(),Ex::int(1)]),
        q("x^3", "2x^2", vec![Ex::zero(),Ex::int(2)]),
        // x³-3x+2=(x-1)²(x+2): include the double root too.
        q("x^3", "3x-2", vec![Ex::int(-2),Ex::int(1)]),
        // x(x³-2)=0 and (x-1)((x-1)³-2)=0.
        q("x^4", "2x", vec![Ex::zero(),Ex::int(2).pow_rat(Rat::new(1,3).unwrap()).unwrap()]),
        q("(x-1)^4", "2(x-1)", vec![Ex::int(1),plus(Ex::int(1),Ex::int(2).pow_rat(Rat::new(1,3).unwrap()).unwrap())]),
        q("x^4", "2x^2", vec![root(2).neg(),Ex::zero(),root(2)]),
        // General quadratic formula: 3x²+2x-2=0 => (-1±√7)/3.
        q("3x^2+2x", "2", vec![fraction(plus(Ex::int(-1),root(7).neg()),1,3),fraction(plus(Ex::int(-1),root(7)),1,3)]),
        q("x^2+2x", "1", vec![plus(Ex::int(-1),root(2).neg()),plus(Ex::int(-1),root(2))]),
        q("5-2x^2", "x^2+1", vec![fraction(root(3),-2,3),fraction(root(3),2,3)]),
        // sin x=cos x iff x=π/4+kπ. Only k=0,1 are in [0,4.5].
        Pair { f: "sin(x)", g: "cos(x)", roots: vec![fraction(Ex::pi(),1,4),fraction(Ex::pi(),5,4)], numerical: vec![], xmin: 0. },
        // eˣ=x+2 has two roots; no elementary exact form is claimed.
        // Independent Decimal exp/bisection gives these values (to 16 places).
        Pair { f: "e^x", g: "x+2", roots: vec![], numerical: vec![-1.8414056604369606,1.1461932206205826], xmin: -3. },
        // Fourth roots and the irreducible cubic's 2cos(2π/9),
        // 2cos(4π/9), 2cos(8π/9) are outside the exact-number type.
        // They must stay ≈ even though these are polynomial crossings.
        Pair { f: "x^4", g: "2", roots: vec![], numerical: vec![-1.189207115002721,1.189207115002721], xmin: -3. },
        Pair { f: "x^3", g: "3x-1", roots: vec![], numerical: vec![-1.8793852415718169,0.3472963553338607,1.532088886237956], xmin: -3. },
    ]
}
fn params_for(c: &Pair, strips: &str, rule: &str, n: i64) -> Params {
    with(&[("region",s("Between f(x) and g(x)")),("f",s(c.f)),("g",s(c.g)),("bounds",s("Where f and g cross")),("strips",s(strips)),("rule",s(rule)),("n",Value::I(n)),("xmin",Value::F(c.xmin)),("xmax",Value::F(4.5)),("ymin",Value::F(-3.)),("ymax",Value::F(5.)),("sum_line",Value::B(true)),("integral_line",Value::B(true)),("exact_line",Value::B(true)),("error_line",Value::B(true)),("show_dx",Value::B(true)),("key_line",Value::B(true))])
}
#[test]
fn riemann_crossing_textbook_pairs_never_equate_rounded_bounds() {
    let mut count = 0;
    for c in pairs() { for strips in ["Vertical","Horizontal"] { for rule in RULES { if strips == "Horizontal" && rule == "Trapezoid" { continue; } for n in [2,4,7,12] {
        for dec in -1..=8 {
            let mut p = params_for(&c,strips,rule,n);
            p.set_s("values",if dec < 0 { "Exact" } else { "Decimals" }); p.set_i("dec",dec.max(0));
            let fm = fmt_of(&p);
            let m = model(&p).unwrap_or_else(|e| panic!("{} {} {strips} {rule} n{n}: {e}",c.f,c.g));
            let want: Vec<f64> = if c.roots.is_empty() { c.numerical.clone() } else { c.roots.iter().map(Ex::approx).collect() };
            assert_eq!(m.crossings.len(),want.len(),"{} {}",c.f,c.g);
            let svg = render(&p);
            let lines = texts(&svg,"line");
            let cross = lines.iter().find(|t| t.starts_with("f and g cross")).unwrap();
            let bounds = texts(&svg,"bound");
            for (i,(x,w)) in m.crossings.iter().zip(&want).enumerate() {
                assert!((x.v-w).abs() < 1e-12,"{} {} root{i}: {} != {w}",c.f,c.g,x.v);
                if let Some(e) = c.roots.get(i) { assert!(x.ex.as_ref().unwrap().sub(e).unwrap().is_zero(),"{} {} root{i}",c.f,c.g); }
                else { assert!(x.ex.is_none()); }
                let (t,eq) = fm.text(x);
                let plain = t.replace('^', ""); // superscript 3 in the cube-root index
                assert!(cross.contains(&format!("x {} {plain}",if eq { "=" } else { "≈" })),"{} {} {strips} n{n} dec{dec}: {cross}; expected {t}",c.f,c.g);
                if eq { assert!((x.ex.as_ref().unwrap().approx()-w).abs() < 1e-12); }
                else {
                    let places = t.split_once('.').map_or(0,|(_,s)| s.len());
                    assert_eq!(t,fixed(*w,None,places));
                }
                if i == 0 || i+1 == want.len() {
                    let name = if i == 0 { "a" } else { "b" };
                    let label = format!("{name} {} {plain}",if eq { "=" } else { "≈" });
                    for written in bounds.iter().filter(|t| t.starts_with(&format!("{name} "))) { assert_eq!(written,&label); }
                    if !bounds.contains(&label) {
                        let notes = warnings(&p);
                        assert!(notes.iter().any(|t| t.contains(&format!("label of {name} has no room")) || t.contains("labels of a and b have no room")),"{label} missing without a note: {notes:?}");
                    }
                }
            }
            if strips == "Vertical" && !fm.text(&m.crossings[0]).1 && !fm.text(m.crossings.last().unwrap()).1 {
                assert!(lines.iter().any(|l| l.contains("∫ab ")),"{lines:?}");
                assert!(lines.iter().any(|l| l.contains(&format!("(b − a)/{n}"))),"{lines:?}");
            }
            count += 1;
        }
    } } } }
    assert_eq!(count,20*11*4*10);
    eprintln!("irrational crossing sweep: {count} rendered states");
}
#[test]
fn riemann_crossing_hand_worked_key_and_student_copy() {
    let c = pairs().remove(0);
    let mut p = params_for(&c,"Vertical","Midpoint",4);
    p.set_s("strip_labels","Heights"); p.set_b("x_marks",true);
    let m = model(&p).unwrap();
    // a=-√2,b=√2, Δx=√2/2; midpoints ±3√2/4, ±√2/4.
    // Heights 7/8,15/8,15/8,7/8; M4=11√2/4; area=8√2/3;
    // error=(33-32)√2/12=√2/12.
    assert!(m.delta.unwrap().ex.unwrap().sub(&fraction(root(2),1,2)).unwrap().is_zero());
    for (strip,n) in m.strips.iter().zip([7,15,15,7]) { assert_eq!(strip.h.ex.as_ref().unwrap().as_rat(),Rat::new(n,8)); }
    assert!(m.sum.as_ref().unwrap().ex.as_ref().unwrap().sub(&fraction(root(2),11,4)).unwrap().is_zero());
    assert!(m.exact.as_ref().unwrap().ex.as_ref().unwrap().sub(&fraction(root(2),8,3)).unwrap().is_zero());
    assert!(m.sum.as_ref().unwrap().sub(m.exact.as_ref().unwrap()).ex.unwrap().sub(&fraction(root(2),1,12)).unwrap().is_zero());
    let exact = texts(&render(&p),"line").join("\n");
    assert!(exact.contains("x = −√2 and x = √2"),"{exact}");
    assert!(exact.contains("11√2/4") && exact.contains("8√2/3"),"{exact}");
    p.set_s("values","Decimals"); p.set_i("dec",0);
    let rounded = texts(&render(&p),"line").join(" ");
    assert!(rounded.contains("M4 ≈ 1·(1 + 2 + 2 + 1)"),"{rounded}");
    assert!(rounded.contains("∫ab "),"{rounded}");
    p.set_s("values","Exact");
    p.set_s("strips","Horizontal");
    let h = model(&p).unwrap();
    // y=1/4,3/4,5/4,7/4; width=2√y. M4=(1+√3+√5+√7)/2.
    let hm = fraction(plus(plus(plus(Ex::int(1),root(3)),root(5)),root(7)),1,2);
    assert!(h.sum.unwrap().ex.unwrap().sub(&hm).unwrap().is_zero());
    for strips in ["Vertical","Horizontal"] { for size in ["Small","Medium","Large"] {
        p.set_s("strips",strips); p.set_s("label_size",size); p.set_b("show_answers",false);
        let svg = render(&p);
        assert_eq!(texts(&svg,"bound"),vec!["a","b"]);
        let ls = texts(&svg,"line").join(" ");
        assert!(!ls.contains("√2") && !ls.contains("1.4142") && !ls.contains("11√2/4"),"{ls}");
        assert!(!svg.contains("Key:"));
    } }
    if let Ok(dir) = std::env::var("RIEMANN_R8_EVIDENCE_DIR") {
        std::fs::create_dir_all(&dir).unwrap();
        let base = serde_json::json!({"region":"Between f(x) and g(x)","f":"x^2","g":"2","bounds":"Where f and g cross","rule":"Midpoint","n":4,"xmin":-3,"xmax":4.5,"ymin":-3,"ymax":4.5,"sum_line":true,"integral_line":true,"exact_line":true,"error_line":true,"show_dx":true,"key_line":true,"strip_labels":"Heights","x_marks":true});
        for (name,changes) in [("quadratic-exact",serde_json::json!({})),("quadratic-dec0",serde_json::json!({"values":"Decimals","dec":0})),("golden-horizontal",serde_json::json!({"f":"x+1","g":"x^2","strips":"Horizontal"})),("transcendental",serde_json::json!({"f":"e^x","g":"x+2"})),("student",serde_json::json!({"show_answers":false})),("print",serde_json::json!({"style":"Print"}))] {
            let mut params = base.clone(); for (k,v) in changes.as_object().unwrap() { params[k] = v.clone(); }
            let (_,bytes) = crate::figure_agent::render_png(ID,&params).unwrap();
            std::fs::write(format!("{dir}/{name}.png"),bytes).unwrap();
        }
    }
}
#[test]
#[ignore = "Run alone for render latency"]
fn riemann_crossing_render_timing() {
    let c = pairs().remove(16);
    let mut worst = std::time::Duration::ZERO;
    for n in [12,30,100] { for rule in ["Upper","Lower"] {
        let p = params_for(&c,"Horizontal",rule,n);
        let start = std::time::Instant::now(); let svg = render(&p); let elapsed = start.elapsed();
        worst = worst.max(elapsed);
        assert!(texts(&svg,"line").iter().any(|s| s.starts_with("f and g cross")));
        assert!(elapsed.as_secs_f64() < 1.,"sin/cos {rule} n={n}: {elapsed:?}");
        eprintln!("sin/cos {rule} n={n}: {elapsed:?}");
    } }
    eprintln!("worst sin/cos horizontal render: {worst:?}");
}

#[test]
fn riemann_crossing_trig_horizontal_widths_by_hand() {
    let c = pairs().remove(16);
    // On [π/4,5π/4], cos x≤y≤sin x. For |y|≤√2/2 the
    // endpoints are acos y and π-asin y, separated by π/2.
    // Outside the plateau the section is a sine/cosine cap, width
    // 2acos|y|. The area is [-cos x-sin x] = 2√2.
    let w = |y: f64| if y.abs() <= 2f64.sqrt()/2. { PI/2. } else { 2.*y.abs().min(1.).acos() };
    for n in [4,12,30,100] { for rule in ["Left","Right","Midpoint","Upper","Lower"] {
        let p = params_for(&c,"Horizontal",rule,n);
        let m = model(&p).unwrap();
        let dy = 2./n as f64;
        let mut want = 0.;
        for (j,strip) in m.strips.iter().enumerate() {
            let a = -1.+j as f64*dy;
            let b = if j+1 == n as usize { 1. } else { -1.+(j+1) as f64*dy };
            let expected = match rule { "Left" => w(a), "Right" => w(b), "Midpoint" => w((a+b)/2.), "Lower" => w(a).min(w(b)), _ => w(0f64.clamp(a,b)) };
            assert!((strip.h.v-expected).abs() < 2e-12,"{rule} n{n} #{j}: {} != {expected}",strip.h.v);
            want += expected*dy;
        }
        assert!((m.sum.as_ref().unwrap().v-want).abs() < 2e-12);
        assert!((m.exact.as_ref().unwrap().v-2.*2f64.sqrt()).abs() < 1e-12);
        for dec in 2..=8 {
            let val = m.sum.as_ref().unwrap(); let fm = Fmt { exact:false,dec };
            let error = val.sub(m.exact.as_ref().unwrap());
            for (value,truth) in [(val,want),(&error,want-2.*2f64.sqrt())] {
                let (shown,exact) = fm.text(value);
                if !exact {
                    if shown == "0" { assert!(truth.abs() < 1e-9); }
                    else { let places = shown.split_once('.').map_or(0,|(_,t)|t.len()); assert_eq!(shown,fixed(truth,None,places)); }
                }
            }
        }
        if n == 4 {
            let by_hand = match rule { "Right" | "Left" => Some(3.*PI/4.), "Upper" => Some(PI), "Lower" => Some(PI/2.), _ => None };
            if let Some(value) = by_hand { assert!((want-value).abs() < 1e-14); }
        }
    } }
}
