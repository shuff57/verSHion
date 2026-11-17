//! Read the rendered working back using ordinary arithmetic precedence. This
//! checks substituted expressions, rather than just the model's final answer.
use super::*;
use super::tests::{read_form, s, with};

/// Preserve powers and root indices while reading the SVG's actual tspans.
fn written(svg: &str) -> Vec<String> {
    let mut out = vec![];
    let mut rest = svg;
    while let Some(k) = rest.find("<text data-rs=\"line\"") {
        rest = &rest[k..];
        let end = rest.find("</text>").unwrap();
        let inner = &rest[rest.find('>').unwrap()+1..end];
        let mut spans = vec![];
        let mut tail = inner;
        while let Some(k) = tail.find("<tspan") {
            tail = &tail[k..];
            let e = tail.find('>').unwrap(); let close = tail.find("</tspan>").unwrap();
            let sc = if tail[..e].contains("baseline-shift=\"super\"") { 1 } else if tail[..e].contains("baseline-shift=\"sub\"") { -1 } else { 0 };
            let t = tail[e+1..close].replace("&amp;","&").replace("&lt;","<").replace("&gt;",">");
            spans.push((sc,t)); tail = &tail[close+8..];
        }
        let mut text = String::new(); let mut i = 0;
        while i < spans.len() {
            let sc = spans[i].0; let mut body = String::new();
            while i < spans.len() && spans[i].0 == sc { body.push_str(&spans[i].1); i += 1; }
            match sc {
                1 if body == "3" && spans.get(i).is_some_and(|s|s.1.starts_with('√')) => text.push_str("^3"),
                1 => text.push_str(&format!("^({body})")),
                -1 => { text.push('_'); text.push_str(&body); }
                _ => text.push_str(&body),
            }
        }
        out.push(text); rest = &rest[end+7..];
    }
    out
}
fn arithmetic(t: &str, m: Option<&Model>) -> Option<f64> {
    let t = t.trim().trim_start_matches("Key: ").trim();
    if t.is_empty() { return m.and_then(|m|m.sum.as_ref().map(|s|s.v)); }
    let mut src = t.to_string();
    if let Some(m) = m {
        if t.starts_with('∫') || t == "Exact area" || t == "Exact value" { return m.exact.as_ref().map(|v|v.v); }
        if t == "Area shaded" { return m.abs_area.as_ref().map(|v|v.v); }
        if t == "Error" || t == "error" { return Some(m.sum.as_ref()?.v-m.exact.as_ref()?.v); }
        if let Some(v) = &m.sum { src = src.replace(&m.sum_name,&format!("({})",fixed(v.v,None,17))); }
        let mut expanded = String::new(); let c: Vec<char> = src.chars().collect(); let mut i=0;
        while i < c.len() {
            if c[i].is_ascii_alphabetic() || matches!(c[i],'Δ') {
                let st=i; i+=1; while i<c.len() && c[i].is_ascii_alphabetic() {i+=1;}
                let word: String = c[st..i].iter().collect();
                let value = match word.as_str() { "a"=>Some(m.xa),"b"=>Some(m.xb),"c"=>Some(m.ylo),"d"=>Some(m.yhi),"n"=>Some(m.strips.len() as f64),"exact"=>m.exact.as_ref().map(|v|v.v),"Δx"|"Δy"=>m.delta.as_ref().map(|v|v.v),_=>None };
                if let Some(v)=value {expanded.push_str(&format!("({})",fixed(v,None,17)));} else {expanded.push_str(&word);}
            } else {expanded.push(c[i]);i+=1;}
        }
        src=expanded;
    }
    read_form(&src,false)
}
/// Each '=' is checked locally, so even '≈ 0.50 − 0.5 = −0.0008'
/// fails. Approximation signs do not hide a later false equality.
fn audit(line: &str, m: Option<&Model>) -> (usize,Vec<String>) {
    let mut checked=0; let mut faults=vec![];
    for statement in line.split(';') {
        let mut previous: Option<f64>=None; let mut previous_text=""; let mut separator=None; let mut start=0;
        let breaks: Vec<(usize,char)> = statement.char_indices().filter(|(_,c)| *c=='='||*c=='≈').chain(std::iter::once((statement.len(),'\0'))).collect();
        for (end, next) in breaks {
            let t=statement[start..end].trim();
            let value=arithmetic(t,m);
            // A complete numerical expression must be readable. Ellipses and
            // symbolic f(x_i), integrands etc. are not substituted arithmetic.
            if !t.is_empty() && !t.contains('…') && t.chars().next().is_some_and(|c|c.is_ascii_digit()||matches!(c,'('|'−'|'+'|'√'|'π'|'^'))
                && !t.chars().any(|c|c.is_ascii_alphabetic() && !matches!(c,'e'|'l'|'n')) {
                assert!(value.is_some(),"unreadable numerical working {t:?} in {line:?}");
            }
            if separator==Some('=') && let (Some(a),Some(b))=(previous,value) {
                checked+=1;
                if (a-b).abs()>2e-11*a.abs().max(b.abs()).max(1.) {faults.push(format!("{previous_text} = {t}: {a} != {b} in {line}"));}
            }
            previous=value; previous_text=t; separator=Some(next); start=end+next.len_utf8();
        }
    }
    (checked,faults)
}
fn base(set: &[(&str,Value)]) -> Params {
    let mut data=vec![("sum_line",Value::B(true)),("integral_line",Value::B(true)),("exact_line",Value::B(true)),("error_line",Value::B(true)),("show_dx",Value::B(true)),("key_line",Value::B(true)),("xmin",Value::F(-4.)),("xmax",Value::F(5.)),("ymin",Value::F(-10.)),("ymax",Value::F(10.))];
    data.extend_from_slice(set); with(&data)
}
fn crossing(f:&str,g:&str)->Params {base(&[("region",s("Between f(x) and g(x)")),("f",s(f)),("g",s(g)),("bounds",s("Where f and g cross"))])}
fn cases()->Vec<Params> {
    let mut out=vec![];
    for (f,g) in [("2x+1","x^2"),("x^2-2x","1"),("0.5x^2","x+1"),("3x","x^2+x-1"),("x^3+x^2","2x+2"),("x","x^3"),("(x-1)^3","x-1"),("x","x^3-x"),("x^3","2x"),("(x-1)^3+1","x^2-2x+2")] { out.push(crossing(f,g)); }
    for (a,b) in [("1-sqrt(2)","1+sqrt(2)"),("1-sqrt2","2"),("2-pi","1"),("-1-sqrt(3)","0"),("1/2-sqrt(2)/3","1+sqrt(2)/3"),("1-2^(1/3)","2"),("1-ln(2)","2+ln(2)")] {
        out.push(base(&[("f",s("x^2")),("a",s(a)),("b",s(b))]));
    }
    out.push(base(&[("f",s("4-x^2")),("a",s("1-sqrt(2)")),("b",s("1+sqrt(2)")),("signed",Value::B(true))]));
    out.push(base(&[("f",s("x^3-3x")),("a",s("0")),("b",s("2")),("signed",Value::B(true))]));
    out.push(base(&[("f",s("sqrt(x)")),("a",s("0")),("b",s("4"))]));
    out.push(base(&[("region",s("From a table of values")),("table",s("0, 1-sqrt(2); 2, -sqrt(2)-1; 4, 2+sqrt(2)"))]));
    out
}
#[test]
fn riemann_substitution_auditor_rejects_the_reported_false_chains() {
    for bad in ["Δx = (√2 + 1 − 1 − √2)/4 = √2/2","≈ 0.50 − 0.5 = −0.0008","≈ 0.5 − 1 = −0.0008","≈ 2 − 2 = −0.5","2·1 − √2 = 2 − 2√2"] {
        let (n,faults)=audit(bad,None); assert!(n>0&&!faults.is_empty(),"auditor accepted {bad}");
    }
    for good in ["Δx = (√2 + 1 − (1 − √2))/4 = √2/2","2·(1 − √2) = 2 − 2√2"] {assert!(audit(good,None).1.is_empty());}
}
#[test]
fn riemann_substitution_operands_preserve_precedence_in_every_position() {
    assert_eq!(factor("(1 − √2)"),"(1 − √2)");
    for (t,v) in [("1 − √2",1.-2f64.sqrt()),("1−√2",1.-2f64.sqrt()),("−1 − √2",-1.-2f64.sqrt()),("2 − π",2.-PI),("(√5 + 1)/2",(5f64.sqrt()+1.)/2.),("1 + ln 2",1.+2f64.ln())] {
        for (expr,want) in [(format!("3 − {}",subtrahend(t)),3.-v),(format!("3·{}",factor(t)),3.*v),(format!("3/{}",factor(t)),3./v),(format!("{}^2",factor(t)),v*v)] {
            let got=read_form(&expr,false).unwrap(); assert!((got-want).abs()<1e-12,"{expr}: {got} != {want}");
        }
    }
}
#[test]
fn riemann_substitution_written_middle_expressions_are_true() {
    let mut checked=0; let mut renders=0;
    for template in cases() {for strips in ["Vertical","Horizontal"] {for rule in RULES {for n in [2,4,5,7] {for dec in -1..=8 {
        let mut p=template.clone(); if region(&p)!=Region::Table {p.set_s("strips",strips);}
        if (strips=="Horizontal"&&rule=="Trapezoid") || (region(&p)==Region::Table&&matches!(rule,"Upper"|"Lower"|"Midpoint")) {continue;}
        p.set_s("rule",rule);p.set_i("n",n);p.set_s("values",if dec<0 {"Exact"}else{"Decimals"});p.set_i("dec",dec.max(0));
        let m=model(&p).unwrap_or_else(|e|panic!("{p:?}: {e}"));
        let lines=written(&render(&p));
        for line in lines {
            let (count,faults)=audit(&line,Some(&m));checked+=count;
            assert!(faults.is_empty(),"{} {} {} {strips} {rule} n{n} dec{dec}:\n{}",p.s("f"),p.s("g"),p.s("table"),faults.join("\n"));
        }
        renders+=1;
    }}}}}
    assert!(renders>8_000 && checked>30_000,"{renders} renders, {checked} equalities");
    eprintln!("mechanical substitution audit: {renders} renders, {checked} numerical equalities");
}
#[test]
fn riemann_substitution_hand_worked_blocking_repros_and_tangency() {
    let mut p=crossing("2x+1","x^2");p.set_s("rule","Left");p.set_i("n",4);
    let lines=written(&render(&p));
    assert!(lines.iter().any(|l|l.contains("(√2 + 1 − (1 − √2))/4 = √2/2")),"{lines:?}");
    let m=model(&p).unwrap();
    // Roots 1±√2; L4=5√2/2, integral=8√2/3, error=-√2/6.
    let r=Ex::int(2).pow_rat(Rat::new(1,2).unwrap()).unwrap();
    for (value,n,d) in [(m.sum.as_ref().unwrap(),5,2),(m.exact.as_ref().unwrap(),8,3),(&m.sum.as_ref().unwrap().sub(m.exact.as_ref().unwrap()),-1,6)] {
        assert!(value.ex.as_ref().unwrap().sub(&r.scale(Rat::new(n,d).unwrap()).unwrap()).unwrap().is_zero());
    }
    for (f,g,want) in [("3x","x^2+x-1",r.scale(Rat::new(3,2).unwrap()).unwrap()),("x^3+x^2","2x+2",r.clone())] {
        let mut h=crossing(f,g);h.set_s("strips","Horizontal");h.set_i("n",4);
        assert!(model(&h).unwrap().delta.unwrap().ex.unwrap().sub(&want).unwrap().is_zero());
    }
    let mut q=crossing("x","x^3");q.set_s("rule","Midpoint");q.set_i("n",5);
    let m=model(&q).unwrap();
    // M5=312/625, integral=1/2, error=-1/1250 (hand sums of |x-x³|).
    assert_eq!(m.sum.as_ref().unwrap().ex.as_ref().unwrap().as_rat(),Rat::new(312,625));
    assert_eq!(m.exact.as_ref().unwrap().ex.as_ref().unwrap().as_rat(),Rat::new(1,2));
    for dec in 0..=8 {q.set_s("values","Decimals");q.set_i("dec",dec);let ls=written(&render(&q));
        let error=ls.iter().find(|l|l.starts_with("Error =")).unwrap();
        assert!(audit(error,Some(&m)).1.is_empty(),"{error}");
        if dec<=2 {assert_eq!(error,"Error = M_5 − exact = −0.0008");}
        if dec==3 {assert_eq!(error,"Error = M_5 − exact ≈ −0.001");}
    }
    // Two table intervals of width 2. L=2(1-√2)+2(-√2-1)=-4√2;
    // R=2(-√2-1)+2(2+√2)=2; T=(L+R)/2=1-2√2.
    let mut table=base(&[("region",s("From a table of values")),("table",s("0, 1-sqrt(2); 2, -sqrt(2)-1; 4, 2+sqrt(2)"))]);
    for (rule,want) in [("Left",r.scale(Rat::int(-4)).unwrap()),("Right",Ex::int(2)),("Trapezoid",Ex::int(1).sub(&r.scale(Rat::int(2)).unwrap()).unwrap())] {
        table.set_s("rule",rule);assert!(model(&table).unwrap().sum.unwrap().ex.unwrap().sub(&want).unwrap().is_zero());
    }
    let t=crossing("(x-1)^3+1","x^2-2x+2");
    let ls=written(&render(&t));assert!(ls.iter().any(|l|l=="f and g meet at x = 1 and x = 2"));
    if let Ok(dir)=std::env::var("RIEMANN_R9_EVIDENCE_DIR") {
        std::fs::create_dir_all(&dir).unwrap();
        let base=serde_json::json!({"region":"Between f(x) and g(x)","f":"2x+1","g":"x^2","bounds":"Where f and g cross","rule":"Left","n":4,"xmin":-1,"xmax":3.5,"ymin":-1,"ymax":7,"sum_line":true,"integral_line":true,"exact_line":true,"error_line":true,"show_dx":true,"key_line":true,"strip_labels":"Heights","x_marks":true});
        for (name,changes) in [("dx-exact",serde_json::json!({})),("dy-exact",serde_json::json!({"f":"3x","g":"x^2+x-1","strips":"Horizontal"})),("error-dec2",serde_json::json!({"f":"x","g":"x^3","rule":"Midpoint","n":5,"xmin":-2,"xmax":2,"ymin":-2,"ymax":2,"values":"Decimals","dec":2})),("student",serde_json::json!({"show_answers":false})),("print",serde_json::json!({"style":"Print"}))] {
            let mut params=base.clone();for(k,v)in changes.as_object().unwrap(){params[k]=v.clone();}
            let(_,bytes)=crate::figure_agent::render_png(ID,&params).unwrap();std::fs::write(format!("{dir}/{name}.png"),bytes).unwrap();
        }
    }
}
