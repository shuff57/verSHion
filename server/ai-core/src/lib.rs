//! The bounded, versioned AI contract shared by the native app, WASM app and relay.
//! No networking, credentials, UI, rendering or platform dependencies belong here.
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Task {
    Import,
    Figure,
    Handwriting,
    Alternatives,
    AnswerKey,
    Grade,
}
impl Task {
    pub fn parse(id: &str) -> Option<Self> {
        Some(match id {
            "import" => Self::Import,
            "figure" => Self::Figure,
            "handwriting" => Self::Handwriting,
            "alternatives" => Self::Alternatives,
            "answer_key" => Self::AnswerKey,
            "grade" => Self::Grade,
            _ => return None,
        })
    }
}

pub const VERSION: u32 = 1;
pub const MAX_SCHEMA_BYTES: usize = 64 * 1024;
pub const MAX_TOOL_CALLS: usize = 8;
pub const FIGURE_TOOLS: &[&str] = &[
    "list_figures",
    "get_figure",
    "render_figure",
    "finish_figure",
    "cannot_make_figure",
];

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OutputMode {
    #[default]
    Json,
    Schema,
    Tool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct OutputSchema {
    pub name: String,
    pub schema: Value,
}

impl OutputSchema {
    pub fn new(name: &str, schema: Value) -> Self {
        Self {
            name: name.into(),
            schema,
        }
    }
    pub fn response_format(&self) -> Value {
        json!({"type":"json_schema", "json_schema":{"name":self.name,"strict":true,"schema":self.schema}})
    }
    pub fn result_tool(&self) -> Value {
        json!({"type":"function","function":{"name":"submit_result","description":"Return the requested result.","strict":true,"parameters":self.schema}})
    }
    pub fn validate(&self, value: &Value) -> Result<(), String> {
        validate(&self.schema, value)
    }
}

pub fn object(properties: Value) -> Value {
    let required: Vec<_> = properties
        .as_object()
        .expect("schema properties")
        .keys()
        .cloned()
        .collect();
    json!({"type":"object","properties":properties,"required":required,"additionalProperties":false})
}
pub fn array(items: Value) -> Value {
    json!({"type":"array","items":items})
}
pub fn nullable(mut schema: Value) -> Value {
    let ty = schema["type"].take();
    schema["type"] = json!([ty, "null"]);
    schema
}

/// Schemas for compact task outputs. Ditto is provided by the app's generated format schema.
pub fn handwriting() -> OutputSchema {
    OutputSchema::new(
        "handwriting_v1",
        object(json!({
            "text":{"type":"string"},
            "uncertain":{"type":"array","maxItems":20,"items":object(json!({
                "word":{"type":"string"},"guess":{"type":"string"},"confidence":{"type":"number","minimum":0,"maximum":1}
            }))}
        })),
    )
}
pub fn figure_pick() -> OutputSchema {
    OutputSchema::new(
        "figure_pick_v1",
        object(json!({"id":{"type":["string","null"]},"reason":{"type":"string"}})),
    )
}
pub fn figure_review() -> OutputSchema {
    OutputSchema::new(
        "figure_review_v1",
        object(json!({"same":{"type":"boolean"},"differences":{"type":"string"}})),
    )
}
pub fn judge() -> OutputSchema {
    OutputSchema::new(
        "figure_judge_v1",
        object(json!({"matches":{"type":"boolean"},"problems":array(json!({"type":"string"}))})),
    )
}

/// Source crops are checked independently of redraws. Keys come from the app, never printed
/// question numbers. A proposed box is not a confirmation until its actual pixels are checked.
pub fn source_scope(proposal: bool) -> OutputSchema {
    let mut fields = json!({
        "key":{"type":"string"}, "complete":{"type":"boolean"},
        "problems":array(json!({"type":"string"})),
        "boundary_evidence":object(json!({
            "axes":{"type":"string","enum":["cartesian","polar","other","none","uncertain"]},
            "endpoints":{"type":"array","maxItems":4,"items":object(json!({
                "key":{"type":"string","enum":["x_negative","x_positive","y_negative","y_positive","polar_positive"]},
                "page_endpoint":{"type":"string","enum":["arrow","line","uncertain"]},
                "crop_endpoint":{"type":"string","enum":["same","clipped","uncertain"]},
                "evidence":{"type":"string","minLength":1,"maxLength":400}
            }))},
            "outer_ink":object(json!({
                "status":{"type":"string","enum":["included","clipped","uncertain"]},
                "evidence":{"type":"string","minLength":1,"maxLength":400}
            }))
        }))
    });
    if proposal {
        fields["box"] = nullable(
            json!({"type":"array","items":{"type":"number","minimum":0,"maximum":1000},"minItems":4,"maxItems":4}),
        );
    }
    OutputSchema::new(
        if proposal {
            "source_scope_proposal_v3"
        } else {
            "source_scope_verification_v3"
        },
        object(json!({"figures":array(object(fields))})),
    )
}

/// A source-only visual inventory, not a source-authored gold reference.
pub fn figure_observation() -> OutputSchema {
    let states = || json!({"type":"string","enum":["yes","no","uncertain","not_applicable"]});
    let ends = || json!({"type":"string","enum":["arrow","line","circle","uncertain","not_applicable"]});
    let plot = nullable(object(json!({
        "axis_ends":object(json!({"x_negative":ends(),"x_positive":ends(),"y_negative":ends(),"y_positive":ends()})),
        "axis_overhang":object(json!({"x_negative":states(),"x_positive":states(),"y_negative":states(),"y_positive":states()})),
        "grid_spill":object(json!({"left":states(),"right":states(),"bottom":states(),"top":states()})),
        "points_present":{"type":"string","enum":["yes","no","uncertain","not_applicable"]},
        "point_axis_contact":object(json!({"x":states(),"y":states()})),
        "evidence":{"type":"array","maxItems":12,"items":{"type":"string","minLength":1,"maxLength":256}}
    })));
    OutputSchema::new(
        "source_figure_observation_v4",
        object(json!({
            "structure":{"type":"string"},"labels":array(json!({"type":"string"})),
            "marks":array(object(json!({"kind":{"type":"string","enum":["angle_arcs","side_ticks","parallel_arrows","right_angle","point","other"]},"count":{"type":["integer","null"],"minimum":0,"maximum":128},"locations":{"type":"string"}}))),
            "connections":array(json!({"type":"string"})),"uncertain":array(json!({"type":"string"})),
            "plot":plot,
            "focus_box_xyxy_1000":nullable(json!({"type":"array","items":{"type":"number","minimum":0,"maximum":1000},"minItems":4,"maxItems":4})),
            "topology":object(json!({
                "status":{"type":"string","enum":["complete","not_applicable","uncertain"]},
                "reason":{"type":"string","minLength":1,"maxLength":512},
                "nodes":{"type":"array","maxItems":80,"items":object(json!({
                    "id":{"type":"string","minLength":1,"maxLength":16},
                    "position":{"type":"string","minLength":1,"maxLength":256}
                }))},
                "edges":{"type":"array","maxItems":128,"items":object(json!({
                    "id":{"type":"string","minLength":1,"maxLength":16},
                    "from":{"type":"string","minLength":1,"maxLength":16},
                    "to":{"type":"string","minLength":1,"maxLength":16},
                    "stroke":{"type":"string","minLength":1,"maxLength":120}
                }))},
                "crossings":{"type":"array","maxItems":48,"items":object(json!({
                    "first":{"type":"string","minLength":1,"maxLength":16},
                    "second":{"type":"string","minLength":1,"maxLength":16},
                    "joins":{"type":["boolean","null"]},
                    "evidence":{"type":"string","minLength":1,"maxLength":160}
                }))}
            }))
        })),
    )
}

/// Select only an existing same-document direction, or leave the item unresolved.
pub fn continuation_directions() -> OutputSchema {
    OutputSchema::new(
        "continuation_directions_v1",
        object(json!({"mappings":array(object(json!({
            "key":{"type":"string"},"direction":{"type":["integer","null"],"minimum":0},"reason":{"type":"string"}
        })))})),
    )
}

/// Literal printed tasks from this page only. Null decisions are explicit; the
/// application distinguishes absent current tasks from uncertain current ink.
pub fn current_page_directions() -> OutputSchema {
    OutputSchema::new("current_page_directions_v1", object(json!({
        "directions":{"type":"array","maxItems":16,"items":{"type":"string","minLength":1,"maxLength":2048}},
        "mappings":{"type":"array","maxItems":512,"items":object(json!({
            "key":{"type":"string","minLength":5,"maxLength":32},
            "status":{"type":"string","enum":["printed","not_printed","uncertain"]},
            "direction":{"type":["integer","null"],"minimum":0},
            "evidence":{"type":"string","minLength":1,"maxLength":512}
        }))}
    })))
}

pub fn ditto() -> OutputSchema {
    let string = json!({"type":"string"});
    let figure = nullable(object(
        json!({"box":{"type":"array","items":{"type":"number","minimum":0,"maximum":1000},"minItems":4,"maxItems":4},"alt":string}),
    ));
    // A choice can be a picture (four pie charts as options A-D): its own figure box, else null.
    let choices = array(object(json!({"text":string,"correct":{"type":"boolean"},"figure":figure.clone()})));
    let make_question = |parts: bool| {
        let mut variants = Vec::new();
        for ty in [
            "multiple-choice",
            "true-false",
            "matching",
            "fill-in-the-blank",
            "short-answer",
            "multipart",
        ] {
            if !parts && !["multiple-choice", "true-false", "short-answer"].contains(&ty) {
                continue;
            }
            let mut props =
                json!({"type":{"type":"string","enum":[ty]},"stem":string,"figure":figure});
            if parts {
                props["id"] = string.clone();
                props["label"] = json!({"type":["string","null"]});
                props["points"] = json!({"type":["number","null"]});
            }
            match ty {
                "multiple-choice" => props["choices"] = choices.clone(),
                "true-false" => props["answer"] = json!({"type":["boolean","null"]}),
                "matching" => {
                    props["pairs"] = array(object(json!({"left":string,"right":string})));
                    props["distractors"] = array(string.clone());
                }
                "fill-in-the-blank" => props["blanks"] = array(array(string.clone())),
                "short-answer" => props["suggestedAnswer"] = string.clone(),
                _ => {}
            }
            variants.push((ty.to_string(), props));
        }
        variants
    };
    let part =
        json!({"anyOf":make_question(false).into_iter().map(|(_,p)|object(p)).collect::<Vec<_>>()});
    let questions = json!({"anyOf":make_question(true).into_iter().map(|(ty,mut p)| { if ty=="multipart" {p["parts"]=array(part.clone());} object(p) }).collect::<Vec<_>>()});
    OutputSchema::new(
        "ditto_v1",
        object(json!({
            "format":{"type":"string","enum":["queshion/ditto"]},"formatVersion":{"type":"string","enum":["0.1.0"]},
            "bank":object(json!({"name":string,"questions":array(questions)})),
            "exams":array(object(json!({"name":string,"sections":array(object(json!({"title":string,"instructions":string,"inheritedFromPage":{"type":["integer","null"],"minimum":1}}))),"positions":array(object(json!({"question":string,"section":{"type":"integer","minimum":0}})))}))),
            "report":object(json!({"unconverted":array(object(json!({"page":{"type":"integer"},"label":string,"reason":string}))),"notes":array(string.clone())}))
        })),
    )
}

pub fn answer_check() -> OutputSchema {
    let string = json!({"type":"string"});
    let variants: Vec<_> = [
        ("choice", array(string.clone())),
        ("answer", json!({"type":"boolean"})),
        ("blanks", array(string.clone())),
        ("answer", string.clone()),
    ]
    .into_iter()
    .map(|(key, shape)| {
        let mut props = json!({"id":string,"sure":{"type":"boolean"}});
        props[key] = shape;
        object(props)
    })
    .collect();
    OutputSchema::new(
        "answer_check_v1",
        object(json!({"answers":array(json!({"anyOf":variants}))})),
    )
}

/// A deliberately small JSON Schema vocabulary. Unsupported keywords are rejected rather than
/// silently claiming validation. The same vocabulary builds our schemas on all three targets.
pub fn check_schema(schema: &Value) -> Result<(), String> {
    if schema.to_string().len() > MAX_SCHEMA_BYTES {
        return Err("The output schema is too large.".into());
    }
    fn check(s: &Value, depth: usize) -> Result<(), String> {
        if depth > 16 {
            return Err("The output schema is too deep.".into());
        }
        let o = s.as_object().ok_or("A schema must be an object.")?;
        for k in o.keys() {
            if ![
                "type",
                "properties",
                "required",
                "additionalProperties",
                "items",
                "enum",
                "anyOf",
                "description",
                "minimum",
                "maximum",
                "minItems",
                "maxItems",
                "minLength",
                "maxLength",
            ]
            .contains(&k.as_str())
            {
                return Err(format!("Unsupported schema keyword: {k}."));
            }
        }
        for key in ["minimum", "maximum"] {
            if o.get(key).is_some_and(|v| v.as_f64().is_none()) {
                return Err(format!("Invalid {key}."));
            }
        }
        for key in ["minItems", "maxItems", "minLength", "maxLength"] {
            if o.get(key).is_some_and(|v| v.as_u64().is_none()) {
                return Err(format!("Invalid {key}."));
            }
        }
        if o.get("enum")
            .is_some_and(|v| !v.as_array().is_some_and(|a| !a.is_empty()))
        {
            return Err("Invalid enum.".into());
        }
        if o.get("additionalProperties")
            .is_some_and(|v| !v.is_boolean() && !v.is_object())
        {
            return Err("Invalid additionalProperties.".into());
        }
        if let Some(ty) = o.get("type") {
            let ts: Vec<_> = match ty {
                Value::String(s) => vec![s.as_str()],
                Value::Array(a) => a.iter().map(|v| v.as_str().unwrap_or("")).collect(),
                _ => vec![""],
            };
            if ts.is_empty()
                || ts.iter().any(|s| {
                    ![
                        "object", "array", "string", "integer", "number", "boolean", "null",
                    ]
                    .contains(s)
                })
            {
                return Err("Invalid schema type.".into());
            }
        } else if !o.contains_key("anyOf") {
            return Err("A schema needs a type or anyOf.".into());
        }
        if let Some(p) = o.get("properties") {
            for s in p.as_object().ok_or("Invalid schema properties.")?.values() {
                check(s, depth + 1)?;
            }
        }
        if let Some(s) = o.get("items") {
            check(s, depth + 1)?;
        }
        if let Some(s) = o.get("additionalProperties").filter(|s| s.is_object()) {
            check(s, depth + 1)?;
        }
        if let Some(a) = o.get("anyOf") {
            let a = a
                .as_array()
                .filter(|a| !a.is_empty() && a.len() <= 16)
                .ok_or("Invalid schema alternatives.")?;
            for s in a {
                check(s, depth + 1)?;
            }
        }
        if let Some(r) = o.get("required") {
            if !r.as_array().is_some_and(|a| a.iter().all(Value::is_string)) {
                return Err("Invalid required fields.".into());
            }
        }
        Ok(())
    }
    check(schema, 0)
}

pub fn validate(schema: &Value, value: &Value) -> Result<(), String> {
    check_schema(schema)?;
    fn walk(s: &Value, v: &Value, path: &str, depth: usize) -> Result<(), String> {
        if depth > 32 {
            return Err("The result is too deep.".into());
        }
        let fail = || format!("{path} does not match the output schema");
        if let Some(a) = s["anyOf"].as_array() {
            if !a.iter().any(|s| walk(s, v, path, depth + 1).is_ok()) {
                return Err(fail());
            }
        }
        let matches = |ty: &str| match ty {
            "object" => v.is_object(),
            "array" => v.is_array(),
            "string" => v.is_string(),
            "boolean" => v.is_boolean(),
            "null" => v.is_null(),
            "number" => v.is_number(),
            "integer" => v.as_f64().is_some_and(|n| n.fract() == 0.0),
            _ => false,
        };
        if let Some(ty) = s.get("type") {
            let ok = ty.as_str().map(&matches).unwrap_or_else(|| {
                ty.as_array()
                    .is_some_and(|a| a.iter().any(|t| t.as_str().is_some_and(&matches)))
            });
            if !ok {
                return Err(fail());
            }
        }
        if let Some(a) = s["enum"].as_array() {
            if !a.contains(v) {
                return Err(fail());
            }
        }
        if let Some(o) = v.as_object() {
            for key in s["required"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
            {
                if !o.contains_key(key) {
                    return Err(format!("{path}.{key} is required"));
                }
            }
            for (key, val) in o {
                if let Some(child) = s["properties"].get(key) {
                    walk(child, val, &format!("{path}.{key}"), depth + 1)?;
                } else if s["additionalProperties"] == false {
                    return Err(format!("{path}.{key} is not allowed"));
                } else if s["additionalProperties"].is_object() {
                    walk(&s["additionalProperties"], val, path, depth + 1)?;
                }
            }
        }
        if let Some(a) = v.as_array() {
            if s["minItems"].as_u64().is_some_and(|n| a.len() < n as usize)
                || s["maxItems"].as_u64().is_some_and(|n| a.len() > n as usize)
            {
                return Err(fail());
            }
            if let Some(child) = s.get("items") {
                for (i, val) in a.iter().enumerate() {
                    walk(child, val, &format!("{path}[{i}]"), depth + 1)?;
                }
            }
        }
        if let Some(n) = v.as_f64() {
            if s["minimum"].as_f64().is_some_and(|x| n < x)
                || s["maximum"].as_f64().is_some_and(|x| n > x)
            {
                return Err(fail());
            }
        }
        if let Some(t) = v.as_str() {
            let len = t.chars().count() as u64;
            if let Some(min) = s["minLength"].as_u64().filter(|n| len < *n) {
                return Err(format!("{path} has {len} characters; minimum is {min}"));
            }
            if let Some(max) = s["maxLength"].as_u64().filter(|n| len > *n) {
                return Err(format!("{path} has {len} characters; maximum is {max}"));
            }
        }
        Ok(())
    }
    walk(schema, value, "result", 0)
}

/// Tool-only messages are valid answers. Preserve opaque reasoning for the next turn.
pub fn assistant_message(raw: &Value) -> Result<Value, &'static str> {
    let c = raw
        .pointer("/choices/0")
        .ok_or("The provider returned no choice.")?;
    if c["finish_reason"] == "length" {
        return Err("The AI reply was cut off. Try a smaller input or a larger output limit.");
    }
    if c["finish_reason"] == "content_filter" {
        return Err("The provider refused this request.");
    }
    let m = c
        .get("message")
        .ok_or("The provider returned no message.")?;
    if m.get("refusal").is_some_and(|v| !v.is_null() && v != "") {
        return Err("The provider refused this request.");
    }
    let calls = m["tool_calls"].as_array().filter(|a| !a.is_empty());
    let text = m["content"].as_str().filter(|t| !t.trim().is_empty());
    if calls.is_none() && text.is_none() {
        return Err("The AI returned no answer.");
    }
    if calls.is_some_and(|a| a.len() > MAX_TOOL_CALLS) {
        return Err("The AI requested too many tools.");
    }
    let mut out =
        json!({"role":"assistant","content":m.get("content").cloned().unwrap_or(Value::Null)});
    for k in ["tool_calls", "reasoning_details"] {
        if let Some(v) = m.get(k) {
            out[k] = v.clone();
        }
    }
    Ok(out)
}

pub fn result_text(message: &Value, output: Option<&OutputSchema>) -> Result<String, String> {
    let text = if let Some(calls) = message["tool_calls"].as_array() {
        if calls.len() != 1 || calls[0]["function"]["name"] != "submit_result" {
            return Err("The AI did not return the requested result tool.".into());
        }
        calls[0]["function"]["arguments"]
            .as_str()
            .ok_or("Tool arguments must be a JSON string.")?
    } else {
        message["content"]
            .as_str()
            .ok_or("The AI returned no text.")?
    };
    if let Some(output) = output {
        let (v, text) = parse_model_json(text)
            .ok_or_else(|| "The AI did not return valid JSON.".to_string())?;
        output.validate(&v)?;
        return Ok(text);
    }
    Ok(text.into())
}

/// A model writing prose inside a JSON string sometimes means a backslash literally ("the
/// segments lean \ at 45 degrees"); a lone backslash there is invalid JSON. This escapes every
/// backslash that does not begin a valid JSON escape. None when there was nothing to escape.
pub fn escape_stray_backslashes(text: &str) -> Option<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len() + 8);
    let mut changed = false;
    let mut i = 0;
    while i < chars.len() {
        if chars[i] != '\\' {
            out.push(chars[i]);
            i += 1;
            continue;
        }
        let valid = match chars.get(i + 1) {
            Some('"' | '\\' | '/' | 'b' | 'f' | 'n' | 'r' | 't') => true,
            Some('u') => chars.get(i + 2..i + 6).is_some_and(|h| h.iter().all(char::is_ascii_hexdigit)),
            _ => false,
        };
        if valid {
            out.push('\\');
            out.push(chars[i + 1]);
            i += 2;
        } else {
            out.push_str("\\\\");
            changed = true;
            i += 1;
        }
    }
    changed.then_some(out)
}

/// A model's JSON reply as a value and the text it was read from: the reply itself when it is
/// valid, else the reply with stray backslashes escaped when that is valid. Nothing else is
/// repaired.
pub fn parse_model_json(text: &str) -> Option<(Value, String)> {
    if let Ok(v) = serde_json::from_str::<Value>(text) {
        return Some((v, text.to_string()));
    }
    let fixed = escape_stray_backslashes(text)?;
    let v = serde_json::from_str::<Value>(&fixed).ok()?;
    Some((v, fixed))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_lone_backslash_in_prose_is_read_literally_and_valid_escapes_are_kept() {
        // Seen live: a slope-field description wrote the direction of a lean as a backslash.
        let raw = r#"{"structure":"Right of the axis the segments lean \ at about 45 degrees; left they lean like '\'.","note":"a \"quote\", a \\ pair, a tab\t and é"}"#;
        assert!(serde_json::from_str::<Value>(raw).is_err());
        let (v, text) = parse_model_json(raw).unwrap();
        assert_eq!(v["structure"], "Right of the axis the segments lean \\ at about 45 degrees; left they lean like '\\'.");
        assert_eq!(v["note"], "a \"quote\", a \\ pair, a tab\t and \u{e9}");
        assert_eq!(serde_json::from_str::<Value>(&text).unwrap(), v);
        // Valid JSON is returned unchanged, and other damage is not repaired.
        assert_eq!(parse_model_json(r#"{"a":"b\\c"}"#).unwrap().1, r#"{"a":"b\\c"}"#);
        assert!(escape_stray_backslashes(r#"{"a":"\u12"}"#).is_some());
        assert!(parse_model_json(r#"{"a": "unterminated}"#).is_none());
        // A schema-bound result goes through the same reading.
        let schema = judge();
        let message = json!({"content": r#"{"matches": false, "problems": ["the edge leans \ not /"]}"#});
        let text = result_text(&message, Some(&schema)).unwrap();
        assert_eq!(serde_json::from_str::<Value>(&text).unwrap()["problems"][0], "the edge leans \\ not /");
    }

    fn plain_origin_source_facts() -> Value {
        json!({"structure":"Finite plain axes meet at an origin corner without negative arms.","labels":[],"marks":[],"connections":[],"uncertain":[],"plot":{"axis_ends":{"x_negative":"line","x_positive":"line","y_negative":"line","y_positive":"line"},"axis_overhang":{"x_negative":"no","x_positive":"yes","y_negative":"no","y_positive":"yes"},"grid_spill":{"left":"not_applicable","right":"no","bottom":"not_applicable","top":"no"},"points_present":"not_applicable","point_axis_contact":{"x":"not_applicable","y":"not_applicable"},"evidence":["The two visible plain shafts meet at their finite origin endpoints."]},"focus_box_xyxy_1000":null,"topology":{"status":"not_applicable","reason":"Only axes and grid strokes.","nodes":[],"edges":[],"crossings":[]}})
    }
    #[test]
    fn source_v4_plain_origin_ends_and_absent_axis_applicability_do_not_accept_legacy_none() {
        let schema=figure_observation();let facts=plain_origin_source_facts();schema.validate(&facts).unwrap();
        for side in ["x_negative","x_positive","y_negative","y_positive"] {
            let mut legacy=facts.clone();legacy["plot"]["axis_ends"][side]=json!("none");let before=legacy.clone();assert!(schema.validate(&legacy).is_err());assert_eq!(legacy,before,"Validation must not coerce legacy arm semantics into source facts.");
        }
        let mut no_axes=facts;
        for side in ["x_negative","x_positive","y_negative","y_positive"] {no_axes["plot"]["axis_ends"][side]=json!("not_applicable");no_axes["plot"]["axis_overhang"][side]=json!("not_applicable");}
        schema.validate(&no_axes).unwrap();
    }
    #[test]
    fn source_v4_genuine_endpoint_and_contact_uncertainty_remain_literal() {
        let mut facts=plain_origin_source_facts();facts["plot"]["axis_ends"]["y_positive"]=json!("uncertain");facts["plot"]["points_present"]=json!("yes");facts["plot"]["point_axis_contact"]["y"]=json!("uncertain");let before=facts.clone();figure_observation().validate(&facts).unwrap();assert_eq!(facts,before);
        let schema=figure_observation();let ends=&schema.schema["properties"]["plot"]["properties"]["axis_ends"]["properties"]["x_negative"]["enum"];
        assert_eq!(ends,&json!(["arrow","line","circle","uncertain","not_applicable"]));
    }
    #[test]
    fn source_v4_nullable_plot_focus_and_node_positions_remain_strictly_required() {
        let schema = figure_observation();
        assert_eq!(schema.name, "source_figure_observation_v4");
        let mut facts = json!({"structure":"An observed straight stroke.","labels":[],"marks":[],"connections":[],"uncertain":[],"plot":null,"focus_box_xyxy_1000":null,"topology":{"status":"complete","reason":"Visible endpoints.","nodes":[{"id":"a","position":"x".repeat(256)},{"id":"b","position":"Other endpoint"}],"edges":[{"id":"e","from":"a","to":"b","stroke":"Solid black"}],"crossings":[]}});
        schema.validate(&facts).unwrap();
        for text in ["x".repeat(257), String::new()] {
            let mut bad = facts.clone();
            bad["topology"]["nodes"][0]["position"] = json!(text);
            assert!(schema.validate(&bad).is_err());
        }
        for key in ["plot", "focus_box_xyxy_1000"] {
            let mut missing = facts.clone();
            missing.as_object_mut().unwrap().remove(key);
            assert!(
                schema
                    .validate(&missing)
                    .unwrap_err()
                    .contains(&format!(".{key} is required"))
            );
        }
        facts["plot"] = json!({"axis_ends":{"x_negative":"line","x_positive":"line","y_negative":"circle","y_positive":"arrow"},"axis_overhang":{"x_negative":"not_applicable","x_positive":"no","y_negative":"yes","y_positive":"uncertain"},"grid_spill":{"left":"not_applicable","right":"yes","bottom":"no","top":"uncertain"},"points_present":"not_applicable","point_axis_contact":{"x":"not_applicable","y":"not_applicable"},"evidence":["Only supplied source ink."]});
        facts["focus_box_xyxy_1000"] = json!([50.5, 0, 200, 1000]);
        schema.validate(&facts).unwrap();
        let mut missing = facts.clone();
        missing["plot"]["axis_ends"]
            .as_object_mut()
            .unwrap()
            .remove("x_positive");
        assert!(schema.validate(&missing).is_err());
        for focus in [json!([0, 0, 1001, 500]), json!([0, 0, 50]), json!("none")] {
            let mut bad = facts.clone();
            bad["focus_box_xyxy_1000"] = focus;
            assert!(schema.validate(&bad).is_err());
        }
        let mut bad = facts.clone();
        bad["plot"]["evidence"] = json!(["x".repeat(257)]);
        assert!(schema.validate(&bad).is_err());
        fn strict(s: &Value) {
            if let Some(p) = s["properties"].as_object() {
                assert_eq!(s["additionalProperties"], false);
                let required = s["required"].as_array().unwrap();
                assert_eq!(required.len(), p.len());
                for (k, v) in p {
                    assert!(required.contains(&json!(k)));
                    strict(v);
                }
            }
            if let Some(items) = s.get("items") {
                strict(items);
            }
        }
        strict(&schema.response_format()["json_schema"]["schema"]);
    }
    #[test]
    fn length_errors_identify_the_constraint_without_echoing_source_text() {
        let schema = OutputSchema::new(
            "length_example",
            object(json!({"reason":{"type":"string","minLength":1,"maxLength":4}})),
        );
        let error = schema
            .validate(&json!({"reason":"private text"}))
            .unwrap_err();
        assert_eq!(error, "result.reason has 12 characters; maximum is 4");
        assert!(!error.contains("private text"));
        assert_eq!(
            schema.validate(&json!({"reason":""})).unwrap_err(),
            "result.reason has 0 characters; minimum is 1"
        );
        schema.validate(&json!({"reason":"ππππ"})).unwrap();
    }
    #[test]
    fn source_observation_reason_is_required_nonempty_and_bounded_without_losing_metadata() {
        let mut facts = json!({"structure":"Only a non-Cartesian curve.","labels":[],"marks":[],"connections":[],"uncertain":[],"plot":null,"focus_box_xyxy_1000":null,"topology":{"status":"not_applicable","reason":"x".repeat(512),"nodes":[],"edges":[],"crossings":[]}});
        figure_observation().validate(&facts).unwrap();
        facts["topology"]["reason"] = json!("x".repeat(513));
        assert!(figure_observation().validate(&facts).is_err());
        facts["topology"]["reason"] = json!("");
        assert!(figure_observation().validate(&facts).is_err());
        facts["topology"].as_object_mut().unwrap().remove("reason");
        assert!(figure_observation().validate(&facts).is_err());
        facts["topology"]["reason"] = json!("All source evidence was retained.");
        facts["topology"].as_object_mut().unwrap().remove("status");
        assert!(figure_observation().validate(&facts).is_err());
    }
    #[test]
    fn source_contracts_require_endpoint_and_structural_evidence_without_new_labels() {
        let legacy = json!({"figures":[{"key":"figure0","complete":true,"problems":[]}]});
        assert!(source_scope(false).validate(&legacy).is_err());
        let mut scoped = legacy;
        scoped["figures"][0]["boundary_evidence"] = json!({"axes":"none","endpoints":[],"outer_ink":{"status":"included","evidence":"All source margins visible."}});
        source_scope(false).validate(&scoped).unwrap();
        scoped["figures"][0]["boundary_evidence"]["outer_ink"]["status"] = json!("probably");
        assert!(source_scope(false).validate(&scoped).is_err());
        let mut facts = json!({"structure":"A non-Cartesian polar grid.","labels":[],"marks":[],"connections":[],"uncertain":[],"plot":null,"focus_box_xyxy_1000":null,"topology":{"status":"not_applicable","reason":"Only polar grid curves; no structural edges.","nodes":[],"edges":[],"crossings":[]}});
        figure_observation().validate(&facts).unwrap();
        facts["topology"]["nodes"] =
            json!([{"id":"internal","position":"Upper-left endpoint","printed_label":"invented"}]);
        assert!(
            figure_observation().validate(&facts).is_err(),
            "Internal endpoint IDs cannot introduce printed labels."
        );
    }
    #[test]
    fn all_task_schemas_are_bounded_and_accept_a_chemistry_package() {
        for output in [
            ditto(),
            handwriting(),
            answer_check(),
            figure_pick(),
            figure_review(),
            judge(),
            source_scope(true),
            source_scope(false),
            continuation_directions(),
            figure_observation(),
        ] {
            check_schema(&output.schema).unwrap();
        }
        let package = json!({"format":"queshion/ditto","formatVersion":"0.1.0","bank":{"name":"Chemistry","questions":[{"id":"q1","label":"1","type":"short-answer","stem":"Name H₂O.","figure":null,"points":null,"suggestedAnswer":"water"}]},"exams":[],"report":{"unconverted":[],"notes":[]}});
        ditto().validate(&package).unwrap();
        let mut bad = package;
        bad["bank"]["questions"][0]["type"] = json!("invented");
        assert!(ditto().validate(&bad).is_err());
        for bad in [
            json!({"type":"string","maxLength":-1}),
            json!({"type":"object","additionalProperties":42}),
            json!({"type":"number","minimum":"small"}),
        ] {
            assert!(check_schema(&bad).is_err());
        }
    }

    #[test]
    fn structured_ditto_preserves_printed_labels_and_optional_source_page_provenance() {
        let mut v = json!({"format":"queshion/ditto","formatVersion":"0.1.0","bank":{"name":"Worksheet","questions":[{"id":"q1","label":"7","type":"short-answer","stem":"","figure":null,"points":null,"suggestedAnswer":""}]},"exams":[{"name":"Worksheet","sections":[{"title":"Critical thinking","instructions":"Find the volume.","inheritedFromPage":null}],"positions":[{"question":"q1","section":0}]}],"report":{"unconverted":[],"notes":[]}});
        ditto().validate(&v).unwrap();
        v["exams"][0]["sections"][0]["inheritedFromPage"] = json!(1);
        ditto().validate(&v).unwrap();
        v["exams"][0]["sections"][0]["inheritedFromPage"] = json!("1");
        assert!(ditto().validate(&v).is_err());
    }

    #[test]
    fn validates_nested_fields_ranges_and_unknown_properties() {
        let s = handwriting();
        assert!(s.validate(&json!({"text":"x","uncertain":[]})).is_ok());
        for v in [
            json!({"text":1,"uncertain":[]}),
            json!({"text":"x"}),
            json!({"text":"x","uncertain":[],"extra":true}),
            json!({"text":"x","uncertain":[{"word":"x","guess":"y","confidence":2}]}),
        ] {
            assert!(s.validate(&v).is_err());
        }
        assert!(check_schema(&json!({"type":"object","$ref":"external"})).is_err());
    }
    #[test]
    fn preserves_tool_only_reply_and_opaque_reasoning_but_rejects_truncation() {
        let raw = json!({"choices":[{"finish_reason":"tool_calls","message":{"content":null,"tool_calls":[{"id":"a","type":"function","function":{"name":"submit_result","arguments":"{\"id\":null,\"reason\":\"none\"}"}}],"reasoning_details":[{"type":"future","opaque":42}]}}]});
        let m = assistant_message(&raw).unwrap();
        assert_eq!(
            m["reasoning_details"],
            raw["choices"][0]["message"]["reasoning_details"]
        );
        assert!(result_text(&m, Some(&figure_pick())).is_ok());
        let mut bad = raw;
        bad["choices"][0]["finish_reason"] = json!("length");
        assert!(assistant_message(&bad).is_err());
    }
}
