//! Check 7: root denominators, tracer domains, isolated continuous extensions, and the render safety net.
use super::*;
use super::tests::{b,f,s,with};

fn p(fx:&str, a:f64, z:f64) -> Params {
    with(&[("f",s(fx)),("xmin",f(a)),("xmax",f(z)),("show_fpp",b(true)),("tracer",b(false)),
        ("ymin",f(-2.0)),("ymax",f(10.0)),("ymin1",f(-1.0)),("ymax1",f(2.0)),("ymin2",f(-0.2)),("ymax2",f(0.1)),("ystep2",f(0.05))])
}
fn near(a:f64,b:f64) -> bool { (a-b).abs()<1e-8*(1.0+b.abs()) }
fn cases() -> Vec<(String,f64,[f64;3])> {
    let mut cases=vec![];
    // (x−a²)/(sqrt x−a) = sqrt x+a: f′=1/(2 sqrt x), f″=−1/(4x^(3/2)).
    // Reciprocal = 1/(sqrt x+a): limits 1/(2a), −1/(8a³), 1/(8a⁵).
    for a in [0.5_f64,1.0,2.0,3.0] {
        cases.push((format!("(x-{})/(sqrt(x)-{a})",a*a),a*a,[2.0*a,1.0/(2.0*a),-1.0/(4.0*a.powi(3))]));
        cases.push((format!("(sqrt(x)-{a})/(x-{})",a*a),a*a,[1.0/(2.0*a),-1.0/(8.0*a.powi(3)),1.0/(8.0*a.powi(5))]));
    }
    // t = cbrt x, (x−a³)/(t−a) = t²+at+a². At x=a³: f=3a², f′=1/a, f″=−4/(9a⁴).
    // At a=2: f′=1/2, f″=−1/36.
    for a in [-2.0_f64,-1.0,1.0,2.0] {
        cases.push((format!("(x-{})/(x^(1/3)-({a}))",a.powi(3)),a.powi(3),[3.0*a*a,1.0/a,-4.0/(9.0*a.powi(4))]));
    }
    // Shifted sqrt: u=x+c; the same derivative formulas with u at the hole.
    for (a,c) in [(2.0_f64,3.0_f64),(3.0,-2.0),(1.0,0.5)] {
        cases.push((format!("(x+{c}-{})/(sqrt(x+{c})-{a})",a*a),a*a-c,[2.0*a,1.0/(2.0*a),-1.0/(4.0*a.powi(3))]));
    }
    // Arbitrary positive constant roots: sqrt 2 case, and a cube root whose value has no exact display form.
    cases.push(("(x-2)/(sqrt(x)-sqrt(2))".into(),2.0,[2.0*2f64.sqrt(),1.0/(2.0*2f64.sqrt()),-1.0/(8.0*2f64.sqrt())]));
    let a=2f64.cbrt();
    cases.push(("(x-2)/(x^(1/3)-2^(1/3))".into(),2.0,[3.0*a*a,1.0/a,-4.0/(9.0*a.powi(4))]));
    cases.extend([
        // Rational cancellations derived by division; |x| is ±x near the nonzero holes.
        ("(x^2-1)/(x-1)".into(),1.0,[2.0,1.0,0.0]),
        ("(x^3-8)/(x-2)".into(),2.0,[12.0,6.0,2.0]),
        ("(x^2-4)/(abs(x)-2)".into(),2.0,[4.0,1.0,0.0]),
        ("(x^2-4)/(abs(x)-2)".into(),-2.0,[4.0,-1.0,0.0]),
        ("(abs(x)^2-1)/(abs(x)-1)".into(),1.0,[2.0,1.0,0.0]),
        // Taylor series: sin t/t; expm1 t/t; ln(1+t)/t; t/ln(1+t); sin²t/(1−cos t)=1+cos t.
        ("(x^2-2)/sin(x^2-2)".into(),2f64.sqrt(),[1.0,0.0,8.0/3.0]),
        ("(x^2-2)/(e^(x^2-2)-1)".into(),2f64.sqrt(),[1.0,-2f64.sqrt(),1.0/3.0]),
        ("abs(x)/abs(x)".into(),0.0,[1.0,0.0,0.0]),
        ("sin(abs(x))/abs(x)".into(),0.0,[1.0,0.0,-1.0/3.0]),
        ("sin(x)/x".into(),0.0,[1.0,0.0,-1.0/3.0]),
        ("sin(x)*x^(-1)".into(),0.0,[1.0,0.0,-1.0/3.0]),
        ("(e^x-1)*x^(-1)".into(),0.0,[1.0,0.5,1.0/3.0]),
        ("(x-9)*(sqrt(x)-3)^(-1)".into(),9.0,[6.0,1.0/6.0,-1.0/108.0]),
        ("(e^x-1)/x".into(),0.0,[1.0,0.5,1.0/3.0]),
        ("ln(x)/(x-1)".into(),1.0,[1.0,-0.5,2.0/3.0]),
        ("(x-1)/ln(x)".into(),1.0,[1.0,0.5,-1.0/6.0]),
        ("sin(x)^2/(1-cos(x))".into(),0.0,[2.0,0.0,-1.0]),
        ("(1-cos(x))/x^2".into(),0.0,[0.5,0.0,-1.0/12.0]),
        ("sin(3x)/sin(x)".into(),0.0,[3.0,0.0,-8.0]),
        ("sin(3x)/sin(x)".into(),PI,[3.0,0.0,-8.0]),
        ("sin(3x)/sin(x)".into(),-PI,[3.0,0.0,-8.0]),
    ]);
    cases
}
fn texts(svg:&str) -> String {
    let mut text=String::new(); let mut tag=false;
    for c in svg.chars() { match c { '<'=>tag=true,'>'=>tag=false,_ if !tag=>text.push(c),_=>{} } }
    text.replace('\u{a0}'," ")
}
fn assert_circle(bl:&Built,fi:usize,x:f64,y:f64) {
    let pn=&bl.panels[fi];
    if y<pn.g.y0 || y>pn.g.y1 { return; }
    let want=format!("cx=\"{:.2}\" cy=\"{:.2}\" r=\"4.5\"",pn.sx(x),pn.sy(y));
    assert!(bl.svg.contains(&want),"no circle {fi} at ({x},{y})");
}

#[test]
fn root_denominators_and_other_spellings_have_hand_derived_limits_and_safe_curves() {
    let mut count=0;
    for (fx,x,ys) in cases() {
        for (a,z) in [(x-3.0,x+3.0),(x-1.1,x+4.3),(x-0.1,x+0.15),(x,x+3.0)] {
            let mut pp=p(&fx,a,z);
            pp.set_f("ymin",ys[0].min(-2.0)-1.0); pp.set_f("ymax",ys[0].max(10.0)+1.0);
            pp.set_f("ymin1",ys[1].min(-1.0)-0.5); pp.set_f("ymax1",ys[1].max(2.0)+0.5);
            pp.set_f("ymin2",ys[2].min(-0.2)-0.1); pp.set_f("ymax2",ys[2].max(0.1)+0.1);
            pp.set_b("tracer",true); pp.set_s("tracer_x",&format!("{x:?}"));
            for opt in ["extrema","inflect","sign_fp","sign_fpp","incdec","concavity","key_list"] { pp.set_b(opt,true); }
            let m=model(&pp,window(&pp)).unwrap();
            for fi in 0..3 {
                let got=m.local_value(0,fi,x).or_else(|| m.rat_of(0,fi)?.f(x));
                assert!(got.is_some_and(|v|near(v,ys[fi])),"{fx} at {x}, derivative {fi}: {got:?}, expected {}",ys[fi]);
                assert!(m.d(fi,x).is_none(),"{fx}: assigned derivative {fi} at hole {x}");
                for t in [-1e-7,1e-7] {
                    let v=m.dp(0,fi,x+t).unwrap();
                    assert!((v-ys[fi]).abs()<1e-5*(1.0+ys[fi].abs()),"{fx}: noisy derivative {fi} at {x}+{t}: {v}");
                }
            }
            let bl=build(&pp);
            for fi in 0..3 { assert_circle(&bl,fi,x,ys[fi]); }
            for (a,b,kind) in &bl.strokes {
                if *kind!="curve" { continue; }
                for pn in &bl.panels {
                    if (pn.ix(a.0)-x).abs()<0.001 && (pn.ix(b.0)-x).abs()<0.001 && a.1>=pn.plot()[1] && a.1<=pn.plot()[3] {
                        assert!((a.1-b.1).abs()<2.0,"{fx}: false stroke {a:?}..{b:?}");
                    }
                }
            }
            let text=texts(&bl.svg);
            assert!(!text.contains(&format!("f′({x}) =")) && !text.contains(&format!("f″({x}) =")),"{fx}: {text}");
            count+=3;
        }
    }
    eprintln!("round 7: {count} panels, all options enabled, limits checked by hand");
}

#[test]
fn rational_hole_tracers_never_assign_an_exact_derivative() {
    for (fx,x) in [("(x^2-1)/(x-1)","1"),("(x^3-8)/(x-2)","2")] {
        let mut pp=p(fx,-3.0,3.0); pp.set_b("tracer",true); pp.set_s("tracer_x",x);
        let text=texts(&render(&pp));
        for prime in ["′","″"] { assert!(text.contains(&format!("f{prime}({x}) does not exist")),"{text}"); }
    }
}

#[test]
fn isolated_pieces_use_derivatives_of_the_continuous_extension() {
    for (fx,value,ys) in [
        ("sin(x)/x","1",[0.0,-1.0/3.0]),
        ("(e^x-1)/x","1",[0.5,1.0/3.0]),
        ("(1-cos(x))/x^2","1/2",[0.0,-1.0/12.0]),
        ("x^2","0",[0.0,2.0]),
    ] {
        let fx=format!("{fx}, x<0; {value}, x=0; {fx}, x>0");
        let mut pp=p(&fx,-3.0,3.0); pp.set_f("ymin2",-1.0); pp.set_f("ymax2",3.0); pp.set_b("tracer",true); pp.set_s("tracer_x","0");
        let m=model(&pp,window(&pp)).unwrap();
        for fi in 1..=2 { assert!(m.d(fi,0.0).is_some_and(|v| near(v,ys[fi-1])),"{fx}: derivative {fi} = {:?}",m.d(fi,0.0)); }
        let bl=build(&pp);
        let pn=&bl.panels[2];
        assert!(bl.tracer_pts.iter().any(|q| (q.0-pn.sx(0.0)).abs()<0.01 && (q.1-pn.sy(ys[1])).abs()<0.01),"{fx}: missing correct tracer dot");
        assert!(!bl.svg.contains("data-mark=\"hole\""),"{fx}: continuous extension still draws a hole");
        // Starting from this continuous extension as f′ shifts the derivative order by one.
        let mut ap=pp.clone(); ap.set_s("start",STARTS[1]); ap.set_s("fp",&fx); ap.set_s("pt_x","0"); ap.set_s("pt_y","0");
        let am=model(&ap,window(&ap)).unwrap();
        assert!(am.d(2,0.0).is_some_and(|v|near(v,ys[0])));
        assert!(am.d(3,0.0).is_some_and(|v|near(v,ys[1])));
    }
    for fx in ["sin(x)/x, x<0; 2, x=0; sin(x)/x, x>0", "-x, x<0; 0, x=0; x, x>0", "x, x<0; 1, x=0; x+1, x>0"] {
        let pp=p(fx,-3.0,3.0); let m=model(&pp,window(&pp)).unwrap();
        assert!(m.d(1,0.0).is_none() && m.d(2,0.0).is_none(),"{fx}: differentiated a jump/corner");
    }
}

#[test]
fn numerical_denominator_roots_use_algebraic_zero_substitution_not_noise() {
    // r solves cos r=r. exp(cos x−x)−1 over cos x−x has limit 1; f′=u′/2,
    // f″=u″/2+(u′)²/3, u=cos x−x. These follow by differentiating 1+u/2+u²/6+... .
    let pp=p("(e^(cos(x)-x)-1)/(cos(x)-x)",0.0,1.5);
    let m=model(&pp,window(&pp)).unwrap();
    assert_eq!(m.locals.len(),1);
    let x=m.locals[0].x;
    assert!((x.cos()-x).abs()<1e-14);
    for (fi,y) in [1.0,(-x.sin()-1.0)/2.0,-x.cos()/2.0+(-x.sin()-1.0).powi(2)/3.0].into_iter().enumerate() {
        assert!(near(m.local_value(0,fi,x).unwrap(),y));
    }
}

#[test]
fn render_guard_preserves_real_discontinuities_and_removes_uncertified_stacks() {
    for fx in ["1/x","1/x^2","tan(x)","abs(x)","sin(x)/x^2","(e^x-1)/x^2","abs(x)/x","sin(abs(x))/x","x, x<0; x+1, x>=0","e^(1/x)","x*ln(x)"] {
        let pp=p(fx,-3.0,3.0); let m=model(&pp,window(&pp)).unwrap();
        for fi in 1..3 {
            let gp=grid_params(&pp,&place_rows(&[(fi,rows_of(&m,fi,"Solid"))]),yrange(&pp,fi));
            let g=grid_layout_in(&gp,(480.0,260.0));
            let mut svg=coordinate_grid_build_in(&gp,&mut vec![],(480.0,260.0));
            let before=svg.clone();
            assert!(!holes::guard(&mut svg,&m,&pp,&[fi],&g),"{fx}: refused a real discontinuity");
            assert_eq!(svg,before,"{fx}, derivative {fi}: altered real strokes");
        }
    }
    // Exercise the safety net independently of discovery, using exactly the old noisy grid rows.
    let pp=p("(x-9)/(sqrt(x)-3)",0.0,12.0);
    let mut m=model(&pp,window(&pp)).unwrap(); m.locals.clear();
    let gp=grid_params(&pp,&place_rows(&[(1,rows_of(&m,1,"Solid"))]),yrange(&pp,1));
    let g=grid_layout_in(&gp,(480.0,260.0));
    let mut svg=coordinate_grid_build_in(&gp,&mut vec![],(480.0,260.0));
    assert!(holes::guard(&mut svg,&m,&pp,&[1],&g),"unrecognized noisy stack was not refused");
}

#[test]
#[ignore="foreground render timing and PNG evidence"]
fn derivative_round7_render_time_and_evidence() {
    let mut worst=std::time::Duration::ZERO;
    for (i,(fx,x,_)) in cases().into_iter().enumerate() {
        let mut pp=p(&fx,x-2.0,x+3.0);
        for opt in ["tracer","extrema","inflect","sign_fp","sign_fpp","incdec","concavity","key_list"] { pp.set_b(opt,true); }
        pp.set_s("tracer_x",&format!("{x:?}"));
        let start=std::time::Instant::now(); let bl=build(&pp); let time=start.elapsed(); worst=worst.max(time);
        assert!(time.as_secs_f64()<0.2,"{fx}: {time:?}");
        if let Ok(dir)=std::env::var("DERIVATIVE_R7_EVIDENCE") {
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(format!("{dir}/case-{i}.png"),crate::figures::to_png(&bl.svg,1.0).unwrap()).unwrap();
        }
    }
    if let Ok(dir)=std::env::var("DERIVATIVE_R7_EVIDENCE") {
        let mut root=p("(x-9)/(sqrt(x)-3)",0.0,12.0);
        root.set_f("ymin",0.0); root.set_f("ymax",8.0); root.set_f("ystep",1.0);
        root.set_f("ymin1",0.0); root.set_f("ymax1",1.0); root.set_f("ystep1",0.25);
        let save=|name:&str, pp:&Params| {
            let start=std::time::Instant::now(); let bl=build(pp); let time=start.elapsed();
            assert!(time.as_secs_f64()<0.2,"{name}: {time:?}");
            std::fs::write(format!("{dir}/{name}.svg"),&bl.svg).unwrap();
            std::fs::write(format!("{dir}/{name}.png"),crate::figures::to_png(&bl.svg,1.0).unwrap()).unwrap();
        };
        save("root9-colour",&root);
        root.set_s("style","Print"); save("root9-print",&root);
        root.set_s("style","Colour"); root.set_b(crate::figures::SHOW_ANSWERS,false); save("root9-student",&root);
        let mut ext=p("sin(x)/x, x<0; 1, x=0; sin(x)/x, x>0",-6.0,6.0);
        ext.set_f("ymin",-1.0); ext.set_f("ymax",1.5); ext.set_f("ystep",0.5);
        ext.set_f("ymin1",-1.0); ext.set_f("ymax1",1.0); ext.set_f("ystep1",0.5);
        ext.set_f("ymin2",-1.0); ext.set_f("ymax2",1.0); ext.set_f("ystep2",0.5);
        for opt in ["tracer","extrema","inflect","key_list"] { ext.set_b(opt,true); }
        ext.set_s("tracer_x","0"); save("sinc-extension",&ext);
    }
    let mut pp=p("sin(3x)/sin(x)",-4.0,4.0);
    pp.set_s("label_size","Large");
    for opt in ["tracer","extrema","inflect","sign_fp","sign_fpp","incdec","concavity","key_list","absolute"] { pp.set_b(opt,true); }
    pp.set_s("abs_a","-4"); pp.set_s("abs_b","4");
    let start=std::time::Instant::now(); let _=build(&pp); let time=start.elapsed(); worst=worst.max(time);
    assert!(time.as_secs_f64()<0.2,"sin(3x)/sin x, Large: {time:?}");
    eprintln!("round 7 worst render: {worst:?}");
}
