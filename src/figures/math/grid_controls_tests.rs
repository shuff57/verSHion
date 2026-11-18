use super::*;
use serde_json::Value as Json;
fn blank()->Params{let mut p=Params::defaults(&coordinate_grid_params());p.set_s("equations","");p.set_s("xlabel","");p.set_s("ylabel","");p.set_b("square",false);p.set_f("xmin",-2.0);p.set_f("xmax",4.0);p.set_f("ymin",-3.0);p.set_f("ymax",2.0);p.set_f("xstep",1.0);p.set_f("ystep",0.5);p}
fn near(v:&Json,n:f64){assert!((v.as_f64().unwrap()-n).abs()<0.001,"{v} != {n}")}
const NEW_KEYS:[&str;10]=["x_left_extension","x_right_extension","y_bottom_extension","y_top_extension","x_major_every","y_major_every","x_major_style","y_major_style","point_radius","pt_outline"];
#[test]
fn direct_controls_missing_keys_and_explicit_legacy_defaults_are_identical(){
    for style in ["Colour","Print"]{for arrows in [false,true]{for end in ["Automatic","Circle","None"]{
        let mut p=blank();p.set_s("style",style);p.set_b("arrows",arrows);p.set_s("x_right_end",end);p.set_s("points","A(0,0);B(1,1)");p.set_s("equations","x^2+y^2=2");let before=coordinate_grid_render(&p);for k in NEW_KEYS{p.0.remove(k);}assert_eq!(coordinate_grid_render(&p),before);assert_eq!(grid_render_with_diagnostics(&p).0,before);
    }}}
}
#[test]
fn each_plain_axis_extension_is_independent_of_window_and_grid(){
    let mut p=blank();p.set_b("arrows",false);let(_,old)=grid_render_with_diagnostics(&p);
    for(i,key)in grid_b7::EXTENSION_KEYS.iter().enumerate(){let mut q=p.clone();q.set_f(key,1.25);let(svg,r)=grid_render_with_diagnostics(&q);assert_eq!(svg,coordinate_grid_render(&q));assert_eq!(r["window"],old["window"]);assert_eq!(r["grid"]["vertical_all_painted"]["position_range"],old["grid"]["vertical_all_painted"]["position_range"]);assert_eq!(r["grid"]["horizontal_all_painted"]["position_range"],old["grid"]["horizontal_all_painted"]["position_range"]);
        let g=grid_layout(&q);let s=r["axes"]["segments_svg"].as_array().unwrap();let axis=&s[if i<2{0}else{1}];let endpoint=match i{0=>&axis["from"][0],1=>&axis["to"][0],2=>&axis["from"][1],_=>&axis["to"][1]};let expected=match i{0=>g.ml-1.25*g.xs*g.ux,1=>g.ml+g.pw+1.25*g.xs*g.ux,2=>g.mt+g.ph+1.25*g.ys*g.uy,_=>g.mt-1.25*g.ys*g.uy};near(endpoint,expected);for name in ["x_left","x_right","y_bottom","y_top"]{assert_eq!(r["axes"]["ends"][name]["arrow_present"],false)}
        let b=&r["canvas_map"]["plot_bounds_svg"];near(&r["grid"]["vertical_all_painted"]["stroke_bounds_svg"][1],b[1].as_f64().unwrap());near(&r["grid"]["vertical_all_painted"]["stroke_bounds_svg"][3],b[3].as_f64().unwrap());
    }
}
#[test]
fn zero_extensions_put_shafts_on_frame_and_large_extensions_fit_canvas_and_handles(){
    let mut p=blank();for key in grid_b7::EXTENSION_KEYS{p.set_f(key,0.0);}let(_,r)=grid_render_with_diagnostics(&p);let g=grid_layout(&p);near(&r["axes"]["ends"]["x_left"]["line_end"][0],g.x0);near(&r["axes"]["ends"]["x_right"]["line_end"][0],g.x1);near(&r["axes"]["ends"]["y_bottom"]["line_end"][1],g.y0);near(&r["axes"]["ends"]["y_top"]["line_end"][1],g.y1);
    for key in grid_b7::EXTENSION_KEYS{p.set_f(key,4.0);}p.set_s("y_top_end","Circle");p.set_s("x_right_end","Line arrow");p.set_s("points","A(1,0)");p.set_s("pt_label","None");let(svg,r)=grid_render_with_diagnostics(&p);let doc=roxmltree::Document::parse(&svg).unwrap();let w: f64=doc.root_element().attribute("width").unwrap().parse().unwrap();let h:f64=doc.root_element().attribute("height").unwrap().parse().unwrap();assert!(w>900.0&&h>700.0);
    for s in r["axes"]["segments_svg"].as_array().unwrap(){for k in ["from","to"]{let a=s[k][0].as_f64().unwrap();let b=s[k][1].as_f64().unwrap();assert!(a>=0.0&&a<w&&b>=0.0&&b<h)}}
    for n in doc.descendants().filter(|n|n.has_tag_name("polygon")||n.has_tag_name("polyline")){if let Some(s)=n.attribute("points"){let v:Vec<f64>=s.split(|c:char|c==','||c.is_ascii_whitespace()).filter(|s|!s.is_empty()).map(|s|s.parse().unwrap()).collect();for xy in v.chunks_exact(2){assert!(xy[0]>0.0&&xy[0]<w&&xy[1]>0.0&&xy[1]<h)}}}
    let g=grid_layout(&p);let c=&r["point_geometry"]["markers"][0]["center"];near(&c[0],g.ml+(1.0-g.x0)*g.ux);near(&c[1],g.mt+g.ph-(0.0-g.y0)*g.uy);let handles=crate::figures::math_plot::handles("coordinate-grid",&p,(w,h)).unwrap();assert!(handles.iter().any(|handle|(handle.pos.0-c[0].as_f64().unwrap()).abs()<0.02&&(handle.pos.1-c[1].as_f64().unwrap()).abs()<0.02));
}
#[test]
fn unsafe_extension_is_explicitly_unsupported_without_allocating_a_huge_canvas(){let mut p=blank();p.set_f("xmin",0.0);p.set_f("xmax",0.25);p.set_f("xstep",180.0);p.set_f("x_right_extension",20.0);let(svg,r)=grid_render_with_diagnostics(&p);assert_eq!(svg,coordinate_grid_render(&p));assert!(svg.contains("safe canvas limit"));assert_eq!(r["status"],"unsupported_request");assert_eq!(r["reason"],"unsafe_axis_extension_canvas");assert!(grid_warnings(&p).iter().any(|s|s.contains("safe canvas")));}
#[test]
fn periodic_emphasis_preserves_every_lattice_and_label_position(){let mut p=blank();p.set_s("grid","Light");p.set_i("x_minor",2);p.set_s("vertical_grid","Light");p.set_s("horizontal_grid","Light");let(a,old)=grid_render_with_diagnostics(&p);p.set_i("x_major_every",3);p.set_i("y_major_every",2);let(b,new)=grid_render_with_diagnostics(&p);let inventory=|svg:&str|{let d=roxmltree::Document::parse(svg).unwrap();d.descendants().filter(|n|n.has_tag_name("line")||n.has_tag_name("text")).map(|n|(n.tag_name().name().to_string(),["x1","x2","y1","y2","x","y"].map(|k|n.attribute(k).unwrap_or("").to_string()),n.text().unwrap_or("").to_string())).collect::<Vec<_>>()};assert_eq!(inventory(&a),inventory(&b));for k in ["vertical_major","vertical_minor","horizontal_major","horizontal_minor"]{assert_eq!(old["grid"][k]["position_samples"],new["grid"][k]["position_samples"])}assert!(b.contains("stroke=\"#111111\" stroke-width=\"1.4\""));p.set_s("vertical_grid","None");let(_,r)=grid_render_with_diagnostics(&p);assert_eq!(r["grid"]["vertical_major"]["painted_line_count"],0);}
#[test]
fn authored_dot_controls_preserve_centers_and_exclude_other_circle_roles(){let mut p=blank();p.set_s("points","A(0,0);B(1,1)");p.set_s("pt_label","None");p.set_s("equations","y=x, -1<=x<=1");p.set_s("x_left_end","Circle");let(_,base)=grid_render_with_diagnostics(&p);assert_eq!(base["point_geometry"]["markers"].as_array().unwrap().len(),2);p.set_f("point_radius",12.0);p.set_b("pt_outline",false);let(svg,r)=grid_render_with_diagnostics(&p);assert_eq!(svg,coordinate_grid_render(&p));assert_eq!(r["point_geometry"]["status"],"complete");for i in 0..2{assert_eq!(r["point_geometry"]["markers"][i]["center"],base["point_geometry"]["markers"][i]["center"]);near(&r["point_geometry"]["markers"][i]["radius"],12.0);near(&r["point_geometry"]["markers"][i]["outline_width"],0.0);assert_eq!(r["point_geometry"]["markers"][i]["outline_color"],"none")}
    p.set_s("pt_style","Cross");let a=coordinate_grid_render(&p);p.set_f("point_radius",2.0);p.set_b("pt_outline",true);assert_eq!(a,coordinate_grid_render(&p));assert_eq!(grid_render_with_diagnostics(&p).1["point_geometry"]["status"],"unsupported");p.set_s("points","");let(_,r)=grid_render_with_diagnostics(&p);assert_eq!(r["point_geometry"]["status"],"complete");assert!(r["point_geometry"]["markers"].as_array().unwrap().is_empty());
}
#[test]
fn point_geometry_is_bounded_and_map_uses_selected_pi_units(){let mut p=blank();p.set_s("xunits","Multiples of π");p.set_f("xmin",-0.5);p.set_f("xmax",1.5);p.set_s("points",&(0..40).map(|i|format!("P{i}(0,0)")).collect::<Vec<_>>().join(";"));let(_,r)=grid_render_with_diagnostics(&p);near(&r["canvas_map"]["px_per_unit"][0],240.0);assert_eq!(r["point_geometry"]["status"],"unsupported");assert_eq!(r["point_geometry"]["markers"].as_array().unwrap().len(),32);assert_eq!(r["point_geometry"]["omitted"],8);assert!(serde_json::to_vec(&r).unwrap().len()<grid_diagnostics::MAX_REPORT_BYTES);}
#[test]
fn direct_controls_are_in_figures_sections_and_native_schema(){let defs=coordinate_grid_params();let sections=crate::figures::layout::sections("coordinate-grid",&defs,None);for k in NEW_KEYS{let s=sections.iter().find(|s|s.items.iter().any(|i|defs[*i].key==k)).unwrap();assert!(matches!(s.title.as_str(),"x-axis"|"y-axis"|"Grid"));assert!(crate::figure_agent::catalog_text().contains(k));}}
#[test]
fn marker_report_uses_final_print_colors_and_axis_caps_from_emitted_svg(){let mut p=blank();p.set_s("points","A(0,0)");p.set_s("pt_label","None");p.set_s("pt_color","#111827");p.set_s("style","Print");p.set_b("pt_outline",false);p.set_b("arrows",false);let(svg,r)=grid_render_with_diagnostics(&p);assert_eq!(svg,coordinate_grid_render(&p));assert_eq!(r["point_geometry"]["markers"][0]["fill_color"],"#111111");assert_eq!(r["axes"]["segments_svg"][0]["linecap"],"round");p.set_f("x_right_extension",1.0);let(_,r)=grid_render_with_diagnostics(&p);assert_eq!(r["axes"]["segments_svg"][0]["linecap"],"butt");}
#[test]
fn authored_point_inventory_scope_does_not_claim_coverage_of_curve_endpoints(){
    let mut p=blank();p.set_s("points","A(0,0)");
    for(rows,other)in[("",false),("; ;",false),("y=x, -1<=x<=1",true),("x^2+y^2=2",true),("unparsed candidate row",true)]{
        p.set_s("equations",rows);let(svg,r)=grid_render_with_diagnostics(&p);assert_eq!(svg,coordinate_grid_render(&p),"metadata cannot change the drawing");assert_eq!(r["point_geometry"]["scope"],"authored_points_only");assert_eq!(r["point_geometry"]["other_point_marks_may_exist"],other);assert_eq!(r["point_geometry"]["markers"].as_array().unwrap().len(),1);
    }
}
