//! Candidate-only geometry read from role-scoped fragments of the actual SVG drawing pass.
//! It intentionally has no source image, observer, expected range or fidelity-verdict input.
use super::{GridL, Params, PI};
use serde_json::{json, Value};

pub(super) const MAX_REPORT_BYTES: usize = 16_384;
const SAMPLES: usize = 8;
#[derive(Clone, Copy)]
pub(super) enum Role { VerticalMajor, VerticalMinor, HorizontalMajor, HorizontalMinor }
impl Role {
    fn index(self) -> usize { match self { Self::VerticalMajor=>0,Self::VerticalMinor=>1,Self::HorizontalMajor=>2,Self::HorizontalMinor=>3 } }
    fn vertical(self) -> bool { self.index()<2 }
}
#[derive(Clone, Copy)]
struct Line { a:[f64;2], b:[f64;2], width:f64, painted:bool, cap:&'static str }
#[derive(Clone, Copy)]
struct Mapping { x0:f64,x1:f64,y0:f64,y1:f64,ml:f64,mt:f64,pw:f64,ph:f64,ux:f64,uy:f64,k:f64,xs:f64,ys:f64,w:f64,h:f64 }
impl Mapping {
    fn point(self,p:[f64;2])->[f64;2] { [(self.x0+(p[0]-self.ml)/self.ux)/self.k,self.y0+(self.mt+self.ph-p[1])/self.uy] }
    fn step(self)->[f64;2] { [self.xs/self.k,self.ys] }
    fn valid(self)->bool { [self.x0,self.x1,self.y0,self.y1,self.ml,self.mt,self.pw,self.ph,self.ux,self.uy,self.k,self.xs,self.ys].iter().all(|v|v.is_finite())&&self.ux>0.0&&self.uy>0.0&&self.k>0.0&&self.xs>0.0&&self.ys>0.0 }
}
#[derive(Default)]
pub(super) struct Collector { mapping:Option<Mapping>, units:&'static str, layers:[Vec<Line>;4], axes:Option<Value>, points:Option<Value>, rejected:Option<&'static str>, errors:Vec<&'static str> }
fn rounded(v:f64)->f64 { let v=(v*1e6).round()/1e6; if v==0.0{0.0}else{v} }
fn point(p:[f64;2])->Value { json!([rounded(p[0]),rounded(p[1])]) }
fn number(n:roxmltree::Node<'_,'_>,key:&str)->Option<f64>{n.attribute(key)?.parse::<f64>().ok().filter(|v|v.is_finite())}
fn line(n:roxmltree::Node<'_,'_>)->Option<Line>{
    let width=number(n,"stroke-width")?;
    let cap=match n.attribute("stroke-linecap"){None|Some("butt")=>"butt",Some("round")=>"round",_=>return None};
    Some(Line{a:[number(n,"x1")?,number(n,"y1")?],b:[number(n,"x2")?,number(n,"y2")?],width,painted:n.attribute("stroke").is_some_and(|s|s!="none"&&s!="transparent")&&width>0.0,cap})
}
fn svg_bounds(lines:&[Line])->Option<[f64;4]>{let mut b=[f64::INFINITY,f64::INFINITY,f64::NEG_INFINITY,f64::NEG_INFINITY];let mut any=false;for l in lines.iter().filter(|l|l.painted){for p in [l.a,l.b]{b[0]=b[0].min(p[0]);b[1]=b[1].min(p[1]);b[2]=b[2].max(p[0]);b[3]=b[3].max(p[1]);any=true}}any.then_some(b)}
fn bounds(lines:&[Line],m:Mapping)->Option<[f64;4]>{
    let mut b=[f64::INFINITY,f64::INFINITY,f64::NEG_INFINITY,f64::NEG_INFINITY];let mut any=false;
    for l in lines.iter().filter(|l|l.painted){for p in [l.a,l.b]{let p=m.point(p);b[0]=b[0].min(p[0]);b[1]=b[1].min(p[1]);b[2]=b[2].max(p[0]);b[3]=b[3].max(p[1]);any=true;}}
    any.then_some(b)
}
fn positions(lines:&[Line],m:Mapping,vertical:bool)->Vec<f64>{
    let mut v:Vec<_>=lines.iter().filter(|l|l.painted).map(|l|m.point(l.a)[if vertical{0}else{1}]).collect();v.sort_by(f64::total_cmp);v.dedup_by(|a,b|(*a-*b).abs()<1e-9);v
}
fn range(v:&[f64])->Value{if v.is_empty(){Value::Null}else{json!([rounded(v[0]),rounded(v[v.len()-1])])}}
fn summary(lines:&[Line],m:Mapping,vertical:bool)->Value{
    let v=positions(lines,m,vertical);let sample:Vec<_>=if v.len()<=SAMPLES{v.clone()}else{v[..SAMPLES/2].iter().chain(&v[v.len()-SAMPLES/2..]).copied().collect()};
    let painted:Vec<_>=lines.iter().filter(|l|l.painted).collect();
    let pos_svg:Vec<_>=painted.iter().map(|l|l.a[if vertical{0}else{1}]).collect();let pos_svg=if pos_svg.is_empty(){Value::Null}else{json!([pos_svg.iter().copied().fold(f64::INFINITY,f64::min),pos_svg.iter().copied().fold(f64::NEG_INFINITY,f64::max)])};
    json!({"status":if painted.is_empty(){"no_painted_strokes"}else{"emitted"},"emitted_line_count":lines.len(),"painted_line_count":painted.len(),"unpainted_line_count":lines.len()-painted.len(),"unique_position_count":v.len(),"position_range":range(&v),"position_range_svg":pos_svg,"position_samples":sample.into_iter().map(rounded).collect::<Vec<_>>(),"positions_omitted":v.len().saturating_sub(SAMPLES),"stroke_bounds":bounds(lines,m).map(|b|b.map(rounded)),"stroke_bounds_svg":svg_bounds(lines),"stroke_width_px_range":if painted.is_empty(){Value::Null}else{json!([painted.iter().map(|l|l.width).fold(f64::INFINITY,f64::min),painted.iter().map(|l|l.width).fold(f64::NEG_INFINITY,f64::max)])}})
}
fn marker_points(n:roxmltree::Node<'_,'_>)->Option<Vec<[f64;2]>>{
    if n.has_tag_name("circle"){
        let(x,y,r)=(number(n,"cx")?,number(n,"cy")?,number(n,"r")?);return Some(vec![[x-r,y],[x+r,y],[x,y-r],[x,y+r]])
    }
    let values:Vec<f64>=n.attribute("points")?.split(|c:char|c==','||c.is_ascii_whitespace()).filter(|s|!s.is_empty()).map(str::parse).collect::<Result<_,_>>().ok()?;
    if values.len()%2!=0||values.is_empty()||values.iter().any(|v|!v.is_finite()){return None}Some(values.chunks_exact(2).map(|p|[p[0],p[1]]).collect())
}
impl Collector {
    pub(super) fn configure(&mut self,p:&Params,g:&GridL){
        let k=if p.s("xunits")=="Multiples of π"{PI}else{1.0};self.units=if k!=1.0{"multiples_of_pi"}else if p.s("xunits")=="Degrees"{"degrees"}else{"numbers"};
        self.mapping=Some(Mapping{x0:g.x0,x1:g.x1,y0:g.y0,y1:g.y1,ml:g.ml,mt:g.mt,pw:g.pw,ph:g.ph,ux:g.ux,uy:g.uy,k,xs:g.xs,ys:g.ys,w:g.ml+g.pw+g.mr,h:g.mt+g.ph+g.mb});
    }
    pub(super) fn reject(&mut self,reason:&'static str){self.rejected=Some(reason)}
    pub(super) fn points(&mut self,fragment:&str,authored:usize,cross:bool,declared:bool,other_point_marks_may_exist:bool){
        if cross&&authored>0{self.points=Some(json!({"status":"unsupported","scope":"authored_points_only","other_point_marks_may_exist":other_point_marks_may_exist,"markers":[],"omitted":authored,"reason":"authored_cross_marks_are_not_filled_dots"}));return}
        let wrapped=format!("<svg>{fragment}</svg>");let Ok(xml)=roxmltree::Document::parse(&wrapped)else{self.points=Some(json!({"status":"unsupported","scope":"authored_points_only","other_point_marks_may_exist":other_point_marks_may_exist,"markers":[],"omitted":authored,"reason":"authored_points_fragment_parse_failed"}));return};
        let mut markers=Vec::new();let mut bad=false;let mut count=0;
        for n in xml.descendants().filter(|n|n.has_tag_name("circle")){
            count+=1;if markers.len()>=32{continue}
            let Some((cx,cy,r,sw))=number(n,"cx").zip(number(n,"cy")).zip(number(n,"r")).zip(number(n,"stroke-width")).map(|(((x,y),r),w)|(x,y,r,w))else{bad=true;continue};
            if r<=0.0||sw<0.0{bad=true;continue}
            markers.push(json!({"id":format!("authored_point_{count}"),"center":[cx,cy],"radius":r,"outline_width":sw,"outline_color":n.attribute("stroke").unwrap_or("none"),"fill_color":n.attribute("fill").unwrap_or("black")}));
        }
        let omitted=authored.saturating_sub(markers.len());let complete=!bad&&count==authored&&omitted==0&&(!declared||authored>0);
        self.points=Some(json!({"status":if complete{"complete"}else{"unsupported"},"scope":"authored_points_only","other_point_marks_may_exist":other_point_marks_may_exist,"markers":markers,"omitted":omitted}));
    }
    fn error(&mut self,s:&'static str){if self.errors.len()<4&&!self.errors.contains(&s){self.errors.push(s)}}
    pub(super) fn lines(&mut self,role:Role,fragment:&str){
        let wrapped=format!("<svg>{fragment}</svg>");let Ok(xml)=roxmltree::Document::parse(&wrapped)else{self.error("grid_fragment_parse_failed");return};
        for n in xml.descendants().filter(|n|n.is_element()&&!n.has_tag_name("svg")){
            if !n.has_tag_name("line"){self.error("unexpected_grid_fragment_element");continue}
            let Some(l)=line(n)else{self.error("unsupported_grid_line_attributes");continue};
            if (role.vertical()&&l.a[0]!=l.b[0])||(!role.vertical()&&l.a[1]!=l.b[1]){self.error("unexpected_grid_line_orientation");continue}
            if self.layers[role.index()].len()<super::MAX_LINES{self.layers[role.index()].push(l)}else{self.error("capture_line_limit_exceeded")}
        }
    }
    pub(super) fn axes(&mut self,fragment:&str,ends:Option<[&str;4]>,arrows:bool){
        let Some(m)=self.mapping.filter(|m|m.valid())else{self.error("invalid_mapping");return};
        let wrapped=format!("<svg>{fragment}</svg>");let Ok(xml)=roxmltree::Document::parse(&wrapped)else{self.error("axis_fragment_parse_failed");return};
        let lines:Vec<_>=xml.descendants().filter(|n|n.has_tag_name("line")).filter_map(line).collect();
        let(horizontal,vertical):(Vec<&Line>,Vec<&Line>)=lines.iter().partition(|l|l.a[1]==l.b[1]);
        if horizontal.len()!=1||vertical.len()!=1||vertical[0].a[0]!=vertical[0].b[0]||!lines.iter().all(|l|l.painted){self.error("unsupported_axis_lines");return}
        let(h,v)=(horizontal[0],vertical[0]);let origin=m.point([v.a[0],h.a[1]]);
        let hp=[m.point(h.a),m.point(h.b)];let vp=[m.point(v.a),m.point(v.b)];
        let endpoints=[[hp[0][0].min(hp[1][0]),origin[1]],[hp[0][0].max(hp[1][0]),origin[1]],[origin[0],vp[0][1].min(vp[1][1])],[origin[0],vp[0][1].max(vp[1][1])]];
        let kinds=ends.unwrap_or([if arrows{"Triangle arrow"}else{"None"};4]);
        // These are the actual emission orders of the two drawing branches. Every marker
        // is still verified against its emitted SVG shape; no coordinate-nearness guess.
        let order=if ends.is_some(){[0,1,2,3]}else{[1,0,3,2]};
        let markers:Vec<_>=xml.descendants().filter(|n|n.is_element()&&!n.has_tag_name("svg")&&!n.has_tag_name("line")).collect();
        if markers.len()!=kinds.iter().filter(|k|**k!="None").count(){self.error("axis_marker_count_mismatch");return}
        let mut terminals=endpoints;let mut next=0;
        for i in order{
            if kinds[i]=="None"{continue}
            let n=markers[next];next+=1;
            let tag=match kinds[i]{"Triangle arrow"=>"polygon","Line arrow"=>"polyline","Circle"=>"circle",_=>{self.error("unsupported_axis_marker_kind");return}};
            if !n.has_tag_name(tag){self.error("axis_marker_shape_mismatch");return}
            let Some(points)=marker_points(n)else{self.error("unsupported_axis_marker_geometry");return};
            let dim=if i<2{0}else{1};let vals:Vec<_>=points.into_iter().map(|p|m.point(p)[dim]).collect();
            terminals[i][dim]=if i==0||i==2{vals.into_iter().fold(f64::INFINITY,f64::min)}else{vals.into_iter().fold(f64::NEG_INFINITY,f64::max)};
        }
        let names=["x_left","x_right","y_bottom","y_top"];let mut value=serde_json::Map::new();
        for i in 0..4{value.insert(names[i].into(),json!({"kind":kinds[i],"arrow_present":matches!(kinds[i],"Triangle arrow"|"Line arrow"),"line_end":point(endpoints[i]),"terminal_extent":point(terminals[i])}));}
        self.axes=Some(json!({"origin":point(origin),"ends":value,"line_width_px":{"x":h.width,"y":v.width},"segments_svg":[{"role":"x","from":h.a,"to":h.b,"stroke_width":h.width,"linecap":h.cap},{"role":"y","from":v.a,"to":v.b,"stroke_width":v.width,"linecap":v.cap}]}));
    }
    pub(super) fn finish(self)->Value{
        if let Some(reason)=self.rejected{return json!({"protocol":"coordinate_grid_geometry_v1","scope":"candidate_only","status":"unsupported_request","reason":reason,"drawing_omitted":true})}
        let Some(m)=self.mapping.filter(|m|m.valid())else{return json!({"protocol":"coordinate_grid_geometry_v1","status":"unsupported_mapping","scope":"candidate_only"})};
        let vertical:Vec<_>=self.layers[0].iter().chain(&self.layers[1]).copied().collect();let horizontal:Vec<_>=self.layers[2].iter().chain(&self.layers[3]).copied().collect();
        let xp=positions(&vertical,m,true);let yp=positions(&horizontal,m,false);let vb=bounds(&vertical,m);let hb=bounds(&horizontal,m);let step=m.step();
        let mut overhang=Value::Null;
        if let Some(a)=&self.axes{
            let read=|name:&str,key:&str,dim:usize|a["ends"][name][key][dim].as_f64();
            let extents=|key:&str|->Value{json!({"left":xp.first().zip(read("x_left",key,0)).map(|(g,e)|rounded((g-e)/step[0])),"right":xp.last().zip(read("x_right",key,0)).map(|(g,e)|rounded((e-g)/step[0])),"bottom":yp.first().zip(read("y_bottom",key,1)).map(|(g,e)|rounded((g-e)/step[1])),"top":yp.last().zip(read("y_top",key,1)).map(|(g,e)|rounded((e-g)/step[1]))})};
            let ox=a["origin"][0].as_f64();let oy=a["origin"][1].as_f64();
            overhang=json!({"convention":"positive_outside_negative_inside; grid anchors exclude axes; null means no painted grid or unavailable axis","axis_line_beyond_grid_positions":extents("line_end"),"axis_terminal_beyond_grid_positions":extents("terminal_extent"),"grid_strokes_from_crossing_axis":{"left":hb.zip(ox).map(|(b,o)|rounded((o-b[0])/step[0])),"right":hb.zip(ox).map(|(b,o)|rounded((b[2]-o)/step[0])),"bottom":vb.zip(oy).map(|(b,o)|rounded((o-b[1])/step[1])),"top":vb.zip(oy).map(|(b,o)|rounded((b[3]-o)/step[1]))}});
        }
        let report=json!({"protocol":"coordinate_grid_geometry_v1","scope":"candidate_only_emitted_vector_geometry_not_source_fidelity","status":if self.errors.is_empty(){"ok"}else{"partial"},"units":{"x":self.units,"y":"numbers"},"canvas_map":{"px_per_unit":[m.ux*m.k,m.uy],"plot_bounds_svg":[m.ml,m.mt,m.ml+m.pw,m.mt+m.ph],"canvas_size":[m.w,m.h]},"window":{"x":[rounded(m.x0/m.k),rounded(m.x1/m.k)],"y":[rounded(m.y0),rounded(m.y1)]},"grid_step":[rounded(step[0]),rounded(step[1])],"grid":{"vertical_major":summary(&self.layers[0],m,true),"vertical_minor":summary(&self.layers[1],m,true),"horizontal_major":summary(&self.layers[2],m,false),"horizontal_minor":summary(&self.layers[3],m,false),"vertical_all_painted":summary(&vertical,m,true),"horizontal_all_painted":summary(&horizontal,m,false)},"axes":self.axes,"point_geometry":self.points.unwrap_or(json!({"status":"unsupported","markers":[],"omitted":0})),"overhang_grid_steps":overhang,"omitted_semantics":["tick_text_and_curves_not_inventoried","stroke_thickness_antialiasing_clipping_occlusion_and_raster_visibility_not_certified","emitted_positions_only_renderer_may_thin_or_cap_loops","coordinates_from_actual_0.01px_SVG_then_rounded_to_1e-6_units"],"capture_errors":self.errors});
        if serde_json::to_vec(&report).map_or(true,|v|v.len()>MAX_REPORT_BYTES){json!({"protocol":"coordinate_grid_geometry_v1","scope":"candidate_only","status":"report_size_limit_exceeded","geometry_omitted":true})}else{report}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::{coordinate_grid_params,coordinate_grid_render,grid_render_with_diagnostics,grid_layout};
    fn blank()->Params{let mut p=Params::defaults(&coordinate_grid_params());p.set_s("equations","");p.set_b("square",false);p.set_s("xlabel","");p.set_s("ylabel","");p}
    fn near(actual:&Value,expected:f64){assert!((actual.as_f64().unwrap()-expected).abs()<0.001,"{actual} != {expected}")}
    #[test]
    fn diagnostics_svg_is_identical_for_default_legacy_print_curve_and_axis_modes(){
        for style in ["Colour","Print"]{for equations in ["","x^2+y^2=5"]{for end in ["Automatic","None","Circle","Line arrow"]{
            let mut p=blank();p.set_s("style",style);p.set_s("equations",equations);p.set_s("x_right_end",end);p.set_i("x_minor",3);p.set_s("vertical_grid","Light");
            let ordinary=coordinate_grid_render(&p);let(svg,report)=grid_render_with_diagnostics(&p);assert_eq!(svg,ordinary);assert_eq!(report["status"],"ok","{report}");
        }}}
        let mut p=blank();p.0.remove("number_halo");assert_eq!(grid_render_with_diagnostics(&p).0,coordinate_grid_render(&p));
    }
    #[test]
    fn fractional_window_report_matches_emitted_positions_bounds_and_signed_overhangs(){
        let mut p=blank();p.set_f("xmin",-0.35);p.set_f("xmax",5.3);p.set_f("xstep",1.0);p.set_f("ymin",-2.4);p.set_f("ymax",3.7);p.set_f("ystep",1.0);
        p.set_s("grid_bounds_mode","Explicit");p.set_f("grid_xmin",0.0);p.set_f("grid_xmax",5.0);p.set_f("grid_ymin",-2.0);p.set_f("grid_ymax",3.0);p.set_s("vertical_grid","Light");p.set_s("horizontal_grid","Standard");
        p.set_s("x_left_end","None");p.set_s("x_right_end","Line arrow");p.set_s("y_bottom_end","Circle");p.set_s("y_top_end","None");
        let(svg,r)=grid_render_with_diagnostics(&p);let g=grid_layout(&p);let xml=roxmltree::Document::parse(&svg).unwrap();
        let xs:Vec<_>=xml.descendants().filter(|n|n.has_tag_name("line")&&n.attribute("stroke")==Some("#bdbdbd")).map(|n|number(n,"x1").unwrap()).collect();
        assert_eq!(xs.len(),6);assert_eq!(r["grid"]["vertical_major"]["unique_position_count"],6);
        for(px,value)in xs.iter().zip(r["grid"]["vertical_major"]["position_samples"].as_array().unwrap()){near(value,g.x0+(*px-g.ml)/g.ux);}
        near(&r["grid"]["horizontal_major"]["stroke_bounds"][0],-0.35);near(&r["grid"]["horizontal_major"]["stroke_bounds"][2],5.3);
        near(&r["grid"]["vertical_major"]["stroke_bounds"][1],-2.4);near(&r["grid"]["vertical_major"]["stroke_bounds"][3],3.7);
        assert_eq!(r["axes"]["ends"]["x_left"]["arrow_present"],false);assert_eq!(r["axes"]["ends"]["x_right"]["kind"],"Line arrow");assert_eq!(r["axes"]["ends"]["y_bottom"]["kind"],"Circle");
        near(&r["axes"]["ends"]["x_left"]["line_end"][0],-0.35);near(&r["overhang_grid_steps"]["axis_line_beyond_grid_positions"]["left"],0.35);
        near(&r["overhang_grid_steps"]["grid_strokes_from_crossing_axis"]["left"],0.35);
        // No hidden assumption that the axis lies outside the grid: a nonzero-origin axis
        // can start inside the first/last painted perpendicular lattice positions.
        p.set_f("grid_xmin",-0.35);p.set_f("xmin",-1.35);let(_,r)=grid_render_with_diagnostics(&p);near(&r["overhang_grid_steps"]["axis_line_beyond_grid_positions"]["left"],1.35);
    }
    #[test]
    fn minor_and_unpainted_directions_are_reported_from_actual_strokes(){
        let mut p=blank();p.set_f("xmin",-1.0);p.set_f("xmax",2.0);p.set_f("ymin",-1.0);p.set_f("ymax",2.0);p.set_i("x_minor",2);p.set_i("y_minor",0);p.set_s("vertical_grid","None");
        let(_,r)=grid_render_with_diagnostics(&p);assert_eq!(r["grid"]["vertical_major"]["painted_line_count"],0);assert_eq!(r["grid"]["vertical_major"]["emitted_line_count"],4);assert_eq!(r["grid"]["vertical_minor"]["unique_position_count"],7);assert_eq!(r["grid"]["horizontal_minor"]["emitted_line_count"],0);
        p.set_s("grid","None");let(_,r)=grid_render_with_diagnostics(&p);assert_eq!(r["grid"]["vertical_all_painted"]["painted_line_count"],0);assert!(r["overhang_grid_steps"]["axis_line_beyond_grid_positions"]["left"].is_null());assert_eq!(r["axes"]["ends"]["x_right"]["arrow_present"],true);
    }
    #[test]
    fn pi_units_samples_and_dense_reports_remain_bounded_without_copying_text(){
        let mut p=blank();p.set_s("xunits","Multiples of π");p.set_f("xmin",-0.75);p.set_f("xmax",2.3);p.set_f("xstep",0.25);let(_,r)=grid_render_with_diagnostics(&p);assert_eq!(r["units"]["x"],"multiples_of_pi");near(&r["grid"]["vertical_major"]["position_range"][0],-0.75);near(&r["grid"]["vertical_major"]["position_range"][1],2.25);assert!(r["grid"]["vertical_major"]["positions_omitted"].as_u64().unwrap()>0);
        p.set_s("xunits","Numbers");p.set_f("xmin",0.0);p.set_f("xmax",1000.0);p.set_f("xstep",0.05);p.set_i("x_minor",10);p.set_s("title","DO_NOT_COPY_TITLE");let(svg,r)=grid_render_with_diagnostics(&p);assert_eq!(svg,coordinate_grid_render(&p));let s=serde_json::to_string(&r).unwrap();assert!(s.len()<=MAX_REPORT_BYTES);assert!(!s.contains("DO_NOT_COPY_TITLE"));assert_eq!(r["grid"]["vertical_major"]["emitted_line_count"],super::super::MAX_LINES);assert!(r["grid"]["vertical_major"]["position_samples"].as_array().unwrap().len()<=SAMPLES);assert!(!r["omitted_semantics"].as_array().unwrap().is_empty());
    }
}
