//! Common request construction, output selection and validation for desktop and WASM.
use super::*;

// Match the hosted service's per-call limit. Leaving this unset can reserve a
// provider's entire output allowance and reject an affordable single-page job.
const MAX_OUTPUT_TOKENS: u32 = 8192;

pub(super) fn messages(turns: &[Turn], ollama: bool) -> Vec<Value> {
    turns.iter().map(|t| {
        if ollama { json!({"role":t.role,"content":t.text,"images":t.images.iter().map(|b|base64_encode(b)).collect::<Vec<_>>()}) }
        else {
            let mut parts=vec![json!({"type":"text","text":t.text})];
            for image in &t.images {
                // Imported crops are stored as WebP, while a figure render is PNG.
                // A conversation can contain both; the turn's default MIME is not
                // necessarily the type of every attached image.
                let mime = match image::guess_format(image) {
                    Ok(image::ImageFormat::Png) => "image/png",
                    Ok(image::ImageFormat::Jpeg) => "image/jpeg",
                    Ok(image::ImageFormat::WebP) => "image/webp",
                    Ok(image::ImageFormat::Gif) => "image/gif",
                    _ => &t.mime,
                };
                parts.push(json!({"type":"image_url","image_url":{"url":format!("data:{mime};base64,{}",base64_encode(image))}}));
            }
            json!({"role":t.role,"content":parts})
        }
    }).collect()
}

pub(super) fn body(cfg: &AiCfg, messages: Vec<Value>, temp: f64) -> Value {
    let mut body = json!({"model":cfg.model,"messages":messages,"stream":false,"temperature":temp.clamp(0.0,1.0)});
    if cfg.provider == "ollama" {
        body["format"] = match (cfg.output_mode, cfg.output.as_ref()) {
            (OutputMode::Schema, Some(s)) => s.schema.clone(),
            _ => json!("json"),
        };
        body["options"] = json!({"temperature":temp.clamp(0.0,1.0)});
        body.as_object_mut().unwrap().remove("temperature");
    } else {
        body["max_tokens"] = json!(MAX_OUTPUT_TOKENS);
        match (cfg.output_mode, cfg.output.as_ref()) {
            (OutputMode::Schema, Some(s)) => {
                body["response_format"] = s.response_format();
            }
            (OutputMode::Tool, Some(s)) => {
                body["tools"] = json!([s.result_tool()]);
                body["tool_choice"] =
                    json!({"type":"function","function":{"name":"submit_result"}});
                body["parallel_tool_calls"] = json!(false);
            }
            _ => {
                body["response_format"] = json!({"type":"json_object"});
            }
        }
        if cfg.provider == "openrouter" {
            body["reasoning"] = json!({"enabled":false});
            body["usage"] = json!({"include":true});
            body["provider"] = json!({"data_collection":"deny","require_parameters":cfg.output_mode!=OutputMode::Json});
        }
        if cfg.is_relay() {
            if let Some(op) = &cfg.operation {
                if let Some(session) = &op.session {
                    body["session"] = json!(session);
                }
                body["operation"] = json!(op.id);
            }
            body["protocol_version"] = json!(vershion_ai_core::VERSION);
        }
    }
    body
}

pub(super) async fn send(cfg: &AiCfg, mut body: Value) -> Result<Value, String> {
    if let Some(op) = &cfg.operation {
        op.check()?;
    }
    if cfg.base.is_empty() {
        account_signed_in_check(cfg)?;
        return Err("Set a base URL for the custom AI endpoint in Settings.".into());
    }
    let url = format!(
        "{}{}",
        cfg.base.trim_end_matches('/'),
        if cfg.provider == "ollama" {
            "/api/chat"
        } else {
            "/chat/completions"
        }
    );
    let target = format!("{}|{}|{}", cfg.provider, cfg.base, cfg.model);
    let compatibility = cfg.provider == "custom"
        && cfg.output_mode == OutputMode::Json
        && body.get("tools").is_none();
    if compatibility && json_mode_refused(&target) {
        body.as_object_mut().unwrap().remove("response_format");
    }
    let key = request_key(cfg);
    if cfg.provider == "account" && key.is_none() {
        return Err("Your sign-in has expired. Sign in again before continuing.".into());
    }
    let result = post_json_async(&url, &body, key.as_deref()).await;
    let result = match result {
        Err(e)
            if cfg.provider == "openrouter"
                && body["reasoning"]["enabled"] == false
                && e.contains("answered 400")
                && e.contains("Reasoning is mandatory")
                && e.contains("cannot be disabled") =>
        {
            if let Some(op) = &cfg.operation {
                op.check()?;
            }
            // Some vision models require reasoning. Let that model use its default,
            // retaining the selected output schema, tools and provider privacy policy.
            body.as_object_mut().unwrap().remove("reasoning");
            post_json_async(&url, &body, key.as_deref()).await
        }
        other => other,
    };
    let result = match result {
        Err(e)
            if compatibility
                && body.get("response_format").is_some()
                && e.contains("answered 400")
                && e.contains("response_format") =>
        {
            note_json_mode_refused(&target);
            body.as_object_mut().unwrap().remove("response_format");
            post_json_async(&url, &body, key.as_deref()).await
        }
        other => other,
    }
    .map_err(|e| {
        if cfg.output_mode!=OutputMode::Json && e.contains("answered 400") {
            format!("{e} Check that the selected model supports this output format, or choose JSON (compatible) in Settings.")
        } else { cloud_error_hint(cfg,e) }
    })?;
    if let Some(op) = &cfg.operation {
        op.check()?;
    }
    Ok(result)
}

/// Select the assistant message before validation so a bounded caller can retain malformed
/// structured replies in its execution trace, then apply the same schema itself.
pub(super) async fn chat_message(cfg: &AiCfg, turns: &[Turn], temp: f64) -> Result<Value, String> {
    chat_message_traced(cfg, turns, temp, &mut None).await
}

/// Keep only the model's selected reply, never the HTTP envelope or request credentials.
/// Image content returned by a provider is represented by its hash rather than its bytes.
fn reply_evidence(reply: &Value, ollama: bool) -> Value {
    fn redact(value: &Value) -> Value {
        match value {
            Value::String(s) if s.contains("data:image/") || s.starts_with("data:application/octet-stream;") => {
                use sha2::{Digest, Sha256};
                json!({"redacted_image_sha256":format!("{:x}",Sha256::digest(s.as_bytes()))})
            }
            Value::Array(values) => Value::Array(values.iter().map(redact).collect()),
            Value::Object(values) => Value::Object(values.iter().filter_map(|(key,value)| {
                if matches!(key.to_ascii_lowercase().as_str(),"headers"|"authorization"|"api_key"|"api-key") { return None; }
                if matches!(key.as_str(),"images"|"image"|"image_url"|"b64_json") {
                    use sha2::{Digest, Sha256};
                    return Some((format!("{key}_sha256"),json!(format!("{:x}",Sha256::digest(value.to_string().as_bytes())))));
                }
                Some((key.clone(),redact(value)))
            }).collect()),
            _ => value.clone(),
        }
    }
    if ollama { redact(&json!({"message":reply.get("message"),"done_reason":reply.get("done_reason")})) }
    else { redact(&json!({"choice":reply.pointer("/choices/0")})) }
}

pub(super) async fn chat_message_traced(cfg: &AiCfg, turns: &[Turn], temp: f64, provider_reply: &mut Option<Value>) -> Result<Value, String> {
    *provider_reply = None;
    let mut turns = turns.to_vec();
    if cfg.output_mode != OutputMode::Json && cfg.output.is_some() {
        turns.insert(0,Turn::new("system","Follow the supplied output schema exactly. Include all required fields; use null for absent optional values and the stated defaults for unchanged parameters. Never invent missing source answers."));
    }
    let reply = send(
        cfg,
        body(cfg, messages(&turns, cfg.provider == "ollama"), temp),
    )
    .await?;
    *provider_reply = Some(reply_evidence(&reply, cfg.provider == "ollama"));
    let message = if cfg.provider == "ollama" {
        if reply["done_reason"] == "length" {
            return Err("Invalid AI result: the reply was cut off.".into());
        }
        reply
            .get("message")
            .cloned()
            .ok_or("Invalid AI result: no Ollama message.")?
    } else {
        vershion_ai_core::assistant_message(&reply)
            .map_err(|e| format!("Invalid AI result: {e}"))?
    };
    Ok(message)
}

pub(super) async fn chat(cfg: &AiCfg, turns: &[Turn], temp: f64) -> Result<String, String> {
    chat_traced(cfg, turns, temp, &mut None).await
}

pub(super) async fn chat_traced(cfg: &AiCfg, turns: &[Turn], temp: f64, provider_reply: &mut Option<Value>) -> Result<String, String> {
    let message=chat_message_traced(cfg,turns,temp,provider_reply).await?;
    // Compatibility mode retains the established tolerant Ditto importer. Strict modes
    // never silently accept a schema miss or downgrade to unvalidated text.
    let schema = if cfg.output_mode == OutputMode::Json {
        None
    } else {
        cfg.output.as_ref()
    };
    vershion_ai_core::result_text(&message, schema).map_err(|e| format!("Invalid AI result: {e}"))
}

/// Bridge the existing local figure command loop to native function tools. The opaque
/// assistant message is retained verbatim, including reasoning metadata, for replay.
pub(super) async fn figure(
    cfg: &AiCfg,
    turns: &[Turn],
    saved: &mut std::collections::BTreeMap<usize, Value>,
) -> Result<String, String> {
    figure_traced(cfg, turns, saved, &mut None).await
}

pub(super) async fn figure_traced(
    cfg: &AiCfg,
    turns: &[Turn],
    saved: &mut std::collections::BTreeMap<usize, Value>,
    provider_reply: &mut Option<Value>,
) -> Result<String, String> {
    *provider_reply = None;
    if !cfg.native_tools || cfg.provider == "ollama" {
        return chat_traced(cfg, turns, 0.2, provider_reply).await;
    }
    if turns.len() <= 2 {
        saved.clear();
    }
    let definitions = figure_tools();
    let mut wire = Vec::new();
    let mut pending = None;
    let last_image = turns.iter().rposition(|t| !t.images.is_empty());
    for (i, turn) in turns.iter().enumerate() {
        if let Some(message) = saved.get(&i) {
            wire.push(message.clone());
            pending = message["tool_calls"][0]["id"].as_str().map(str::to_string);
            continue;
        }
        if let Some(id) = pending.take() {
            wire.push(json!({"role":"tool","tool_call_id":id,"content":turn.text}));
            if Some(i) == last_image && !turn.images.is_empty() {
                let mut image_turn = turn.clone();
                image_turn.text = "Rendered figure for the preceding tool result.".into();
                wire.extend(messages(&[image_turn], false));
            }
        } else {
            let mut turn = turn.clone();
            if i > 1 && Some(i) != last_image {
                turn.images.clear();
            }
            if i == 0 {
                turn.text.push_str("\nNative tools are available. Use list_figures, get_figure and render_figure to work. End with an actual finish_figure or cannot_make_figure function call. Request exactly one tool per turn; content-only prose or JSON is not a tool call. For a reference copy, put the first literal source observation in the optional sees string in the first tool's arguments, including readable labels/markers, connected versus separate strokes and visible gaps, actual grid boundaries/subdivisions or curve landmarks; do not solve or infer missing givens.");
            }
            wire.extend(messages(&[turn], false));
        }
    }
    let mut request = body(cfg, wire, 0.2);
    request.as_object_mut().unwrap().remove("response_format");
    request["tools"] = json!(definitions);
    request["tool_choice"] = json!("required");
    request["parallel_tool_calls"] = json!(false);
    let raw = send(cfg, request).await?;
    *provider_reply = Some(reply_evidence(&raw, false));
    let message = vershion_ai_core::assistant_message(&raw).map_err(str::to_string)?;
    let calls = message["tool_calls"]
        .as_array()
        .filter(|c| c.len() == 1)
        .ok_or("The figure AI must request exactly one tool.")?;
    calls[0]["id"].as_str().filter(|id| !id.trim().is_empty())
        .ok_or("The native tool call has no usable id.")?;
    let function = &calls[0]["function"];
    let name = function["name"].as_str().ok_or("The tool has no name.")?;
    let def = definitions
        .iter()
        .find(|d| d["function"]["name"] == name)
        .ok_or("The figure AI requested an unknown tool.")?;
    let args: Value = serde_json::from_str(
        function["arguments"]
            .as_str()
            .ok_or("Tool arguments must be a JSON string.")?,
    )
    .map_err(|_| "The tool arguments were not valid JSON.")?;
    if matches!(name, "render_figure" | "finish_figure") {
        if let Some(params) = args.get("params").filter(|p| !p.is_object()) {
            let kind = match params {
                Value::Null => "null",
                Value::Bool(_) => "boolean",
                Value::Number(_) => "number",
                Value::String(_) => "string",
                Value::Array(_) => "array",
                Value::Object(_) => unreachable!(),
            };
            return Err(format!("Invalid figure tool arguments: {name}.params must be a JSON object; received {kind}. Do not stringify outer params. A nested diagram value alone may be a JSON object or JSON text; prefer a nested object."));
        }
    }
    vershion_ai_core::validate(&def["function"]["parameters"], &args)
        .map_err(|e|format!("Invalid figure tool arguments: {e}"))?;
    let command = figure_command(name,args);
    saved.insert(turns.len(), message);
    Ok(command.to_string())
}

fn figure_command(name:&str,args:Value)->Value {
    let sees=args.get("sees").and_then(Value::as_str).map(str::to_string);
    let mut command = match name {
        "finish_figure" => json!({"final":args}),
        "cannot_make_figure" => json!({"cannot":args["reason"]}),
        _ => json!({"tool":name,"args":args}),
    };
    if let Some(sees)=sees {command["sees"]=json!(sees);}
    command
}

fn figure_tools() -> Vec<Value> {
    use vershion_ai_core::object;
    let string = json!({"type":"string"});
    // A generator parameter may be a scalar, a list, a list of rows, or explicit structured drawing JSON; a value the
    // model leaves out is null. The local renderer reports a value of the wrong kind back to the model, so
    // the wire schema must not reject it first: that used to abort the whole figure run.
    let scalar = json!({"anyOf":[{"type":"string"},{"type":"number"},{"type":"boolean"},{"type":"null"}]});
    let params = json!({"type":"object","description":r#"Generator parameters as a JSON object. Do not stringify this outer params object. For a generator with a diagram parameter, prefer a nested JSON object for diagram; diagram alone may also be JSON text. Example params: {"width":220,"height":180,"diagram":{"nodes":{"A":[30,40],"B":[160,120]},"segments":[{"from":"A","to":"B"}]}}. The native function arguments are encoded once by the tool protocol; do not double-escape the drawing."#,"additionalProperties":{"anyOf":[{"type":"string"},{"type":"number"},{"type":"boolean"},{"type":"null"},{"type":"object","additionalProperties":true},
        {"type":"array","items":{"anyOf":[{"type":"string"},{"type":"number"},{"type":"boolean"},{"type":"null"},{"type":"array","items":scalar}]}}]}});
    let args = |note: bool| {
        let mut p = json!({"id":string,"params":params});
        if note {
            p["note"] = string.clone();
        }
        object(p)
    };
    [("list_figures","List available figure generators.",object(json!({"subject":{"type":["string","null"]}}))),
     ("get_figure","Describe a generator and its parameters.",object(json!({"id":string}))),
     ("render_figure","Render a local figure; inspect its PNG before finishing.",args(false)),
     ("finish_figure","Return the final figure and a note naming any compromises.",args(true)),
     ("cannot_make_figure","Explain why no generator can satisfy the request.",object(json!({"reason":string})))].into_iter().map(|(name,description,mut parameters)|{
         // Optional: old clients/canned commands remain valid, while a native tool call can
         // carry the literal source observation the local figure protocol already supports.
         parameters["properties"]["sees"]=json!({"type":["string","null"],"description":"First reference observation: visible shapes/connectivity/gaps, labels/markers, exact grid boundaries/subdivisions or curve landmarks; no solved or inferred givens."});
         json!({"type":"function","function":{"name":name,"description":description,"parameters":parameters}})
     }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::tests::stub_relay;
    fn answer(text: &str) -> (u16, String) {
        (
            200,
            json!({"choices":[{"message":{"content":text},"finish_reason":"stop"}]}).to_string(),
        )
    }

    #[test]
    fn traced_native_failures_retain_the_reply_before_tool_validation() {
        unsafe { std::env::set_var("VERSHION_STORE_DIR",std::env::temp_dir().join(format!("vershion-native-invalid-trace-tests-{}",std::process::id()))); }
        let tool = |name: &str, arguments: &str| json!({"id":"one","type":"function","function":{"name":name,"arguments":arguments}});
        let messages = vec![
            json!({"content":null,"tool_calls":[tool("get_figure","not JSON")]}),
            json!({"content":null,"tool_calls":[tool("unknown_tool","{}")]}),
            json!({"content":null,"tool_calls":[tool("get_figure","{}")]}),
            json!({"content":null,"tool_calls":[tool("get_figure","{}"),tool("get_figure","{}")]}),
            json!({"content":null,"tool_calls":[{"type":"function","function":{"name":"get_figure","arguments":"{\"id\":\"geometry-diagram\"}"}}]}),
            json!({"content":null,"tool_calls":[{"id":"  ","type":"function","function":{"name":"get_figure","arguments":"{\"id\":\"geometry-diagram\"}"}}]}),
            json!({"content":null,"tool_calls":[tool("render_figure",&json!({"id":"geometry-diagram","params":"{broken JSON"}).to_string())]}),
            json!({"content":null,"tool_calls":[tool("finish_figure",&json!({"id":"geometry-diagram","params":"{}","note":"Must not execute."}).to_string())]})
        ];
        let choices:Vec<_>=messages.into_iter().map(|message|json!({"finish_reason":"tool_calls","message":message})).collect();
        let replies=choices.iter().flat_map(|choice|(0..3).map(move |_|(200,json!({"choices":[choice],"headers":{"Authorization":"must not persist"}}).to_string()))).collect();
        let(base,server)=stub_relay(replies);
        let cfg=AiCfg{provider:"custom".into(),base,native_tools:true,..AiCfg::default()};
        for choice in &choices {
            let run=block_on(crate::figure_agent::run_model_async("copy",None,&cfg));
            assert!(run.error.is_some());
            assert_eq!((run.selected_attempt,run.attempts.len()),(1,1));
            assert_eq!(run.attempts[0].turns.len(),3,"two bounded repairs must not reset the attempt or hide failed calls");
            assert!(!run.steps.iter().any(|s|s.tool=="render_figure"||s.tool=="final"),"invalid native arguments must never reach drawing or final execution");
            for turn in &run.attempts[0].turns {
                assert!(turn.reply.is_none()&&turn.error.is_some());
                assert_eq!(turn.provider_reply,Some(json!({"choice":choice})),"invalid provider reply must survive every tracing layer");
            }
        }
        assert_eq!(server.join().unwrap().len(),choices.len()*3);
    }

    #[test]
    fn native_outer_params_strings_are_repaired_without_execution() {
        let tool = |name: &str, args: Value| json!({"finish_reason":"tool_calls","message":{"content":null,"tool_calls":[{"id":format!("call_{name}"),"type":"function","function":{"name":name,"arguments":args.to_string()}}]}});
        let response = |choice: &Value| (200,json!({"choices":[choice]}).to_string());
        let diagram = json!({"nodes":{"A":[30,40],"B":[160,120]},"segments":[{"from":"A","to":"B"}]});
        for name in ["render_figure","finish_figure"] {
            for malformed in [false,true] {
                let drawing = if malformed { json!(diagram.to_string()) } else { diagram.clone() };
                let params = json!({"width":220,"height":180,"diagram":drawing});
                let invalid_params = if malformed { "{\"diagram\":\"unterminated".to_string() } else { params.to_string() };
                let mut invalid_args = json!({"id":"geometry-diagram","params":invalid_params});
                if name == "finish_figure" { invalid_args["note"] = json!("Must not execute."); }
                let invalid = tool(name,invalid_args);
                let (base,server) = stub_relay(vec![
                    response(&tool("get_figure",json!({"id":"geometry-diagram"}))),
                    response(&invalid),
                    response(&tool("render_figure",json!({"id":"geometry-diagram","params":params}))),
                    response(&tool("finish_figure",json!({"id":"geometry-diagram","params":params,"note":"One explicit segment."}))),
                ]);
                let cfg = AiCfg {provider:"custom".into(),base,native_tools:true,..AiCfg::default()};
                let run = block_on(crate::figure_agent::run_model_async("Make one explicit segment.",None,&cfg));
                assert!(run.error.is_none(),"{:?}",run.error);
                assert!(run.png.is_some());
                let stored = &run.spec.as_ref().unwrap().params["diagram"];
                assert_eq!(serde_json::from_str::<Value>(stored.as_str().unwrap()).unwrap(),diagram,"nested drawing objects and ordinary diagram JSON text must both survive decoding");
                assert_eq!(run.steps.iter().filter(|s|s.tool=="render_figure").count(),1,"the invalid params string must not be rendered, parsed or coerced");
                assert_eq!(run.steps.iter().filter(|s|s.tool=="retry_native_tool_reply").count(),1);
                let turns = &run.attempts[0].turns;
                assert_eq!(turns.len(),4);
                assert!(turns.iter().all(|t|t.limit==8));
                let expected = format!("Invalid figure tool arguments: {name}.params must be a JSON object; received string. Do not stringify outer params. A nested diagram value alone may be a JSON object or JSON text; prefer a nested object.");
                assert_eq!(turns[1].error.as_deref(),Some(expected.as_str()));
                assert!(turns[1].reply.is_none());
                assert_eq!(turns[1].provider_reply,Some(json!({"choice":invalid})),"the invalid raw provider reply must remain in the trace");
                let seen: Vec<Value> = server.join().unwrap().into_iter().map(|(_,body)|serde_json::from_str(&body).unwrap()).collect();
                assert_eq!(seen.len(),4);
                let repair = seen[2]["messages"].as_array().unwrap().iter().flat_map(|m|m["content"].as_array().into_iter().flatten()).filter_map(|p|p["text"].as_str()).collect::<Vec<_>>().join("\n");
                assert!(repair.contains(&expected)&&repair.contains("No command from that response was executed."));
                for request in seen {
                    assert_eq!(request["tool_choice"],"required");
                    assert_eq!(request["parallel_tool_calls"],false);
                    assert!(request.get("response_format").is_none());
                    let definition = request["tools"].as_array().unwrap().iter().find(|d|d["function"]["name"]==name).unwrap();
                    let shape = &definition["function"]["parameters"]["properties"]["params"];
                    assert_eq!(shape["type"],"object");
                    let description = shape["description"].as_str().unwrap();
                    assert!(description.contains("Do not stringify this outer params object")&&description.contains(r#""diagram":{"nodes"#));
                }
            }
        }
    }

    #[test]
    fn native_content_only_final_is_repaired_not_executed_as_a_json_command() {
        let content=json!({"choices":[{"finish_reason":"stop","message":{"content":"{\"final\":{\"id\":\"not-a-real-generator\",\"params\":{},\"note\":\"content is not a native tool\"}}"}}]});
        let tool=|name:&str,args:Value| (200,json!({"choices":[{"finish_reason":"tool_calls","message":{"content":null,"tool_calls":[{"id":format!("call_{name}"),"type":"function","function":{"name":name,"arguments":args.to_string()}}]}}]}).to_string());
        let(base,server)=stub_relay(vec![(200,content.to_string()),
            tool("get_figure",json!({"id":"geometry-diagram"})),
            tool("render_figure",json!({"id":"geometry-diagram","params":{}})),
            tool("finish_figure",json!({"id":"geometry-diagram","params":{},"note":"Native result."}))]);
        let cfg=AiCfg{provider:"custom".into(),base,native_tools:true,..AiCfg::default()};
        let run=block_on(crate::figure_agent::run_model_async("Make a diagram.",None,&cfg));
        assert!(run.error.is_none(),"{:?}",run.error);
        assert_eq!(run.spec.as_ref().unwrap().id,"geometry-diagram");
        let turns=&run.attempts[0].turns;
        assert_eq!(turns.len(),4);
        assert_eq!(turns[0].provider_reply,Some(json!({"choice":content["choices"][0]})));
        assert!(turns[0].error.is_some()&&turns[0].reply.is_none());
        assert!(run.steps.iter().any(|s|s.tool=="retry_native_tool_reply"));
        let seen:Vec<Value>=server.join().unwrap().into_iter().map(|(_,body)|serde_json::from_str(&body).unwrap()).collect();assert_eq!(seen.len(),4);
        let system=seen[0]["messages"][0]["content"].as_array().unwrap().iter()
            .filter_map(|part|part["text"].as_str()).collect::<Vec<_>>().join("\n");
        assert!(system.contains("NATIVE FUNCTION TOOL CALL"));
        assert!(!system.contains("Reply with exactly one JSON object per turn"));
        for request in seen {
            assert_eq!(request["tool_choice"],"required");
            assert_eq!(request["parallel_tool_calls"],false);
            assert!(request.get("response_format").is_none());
        }
    }

    #[test]
    fn traced_compatibility_replies_survive_envelope_and_schema_failures() {
        unsafe { std::env::set_var("VERSHION_STORE_DIR",std::env::temp_dir().join(format!("vershion-compat-trace-tests-{}",std::process::id()))); }
        let cutoff=json!({"finish_reason":"length","message":{"content":"partial raw reply"}});
        let bad_schema=json!({"finish_reason":"stop","message":{"content":"{\"id\":42,\"reason\":\"bad\"}"}});
        let(base,server)=stub_relay(vec![(200,json!({"choices":[cutoff.clone()]}).to_string()),(200,json!({"choices":[bad_schema.clone()]}).to_string())]);
        let cfg=AiCfg{provider:"custom".into(),base,output_mode:OutputMode::Schema,output:Some(vershion_ai_core::figure_pick()),..AiCfg::default()};
        for choice in [cutoff,bad_schema] {
            let mut capture=None;
            assert!(block_on(chat_traced(&cfg,&[Turn::new("user","pick")],0.0,&mut capture)).is_err());
            assert_eq!(capture,Some(json!({"choice":choice})));
        }
        assert_eq!(server.join().unwrap().len(),2);
    }

    #[test]
    fn reply_trace_excludes_envelopes_credentials_and_image_data() {
        let raw=json!({"headers":{"Authorization":"transport secret"},"api_key":"top secret","choices":[{"finish_reason":"stop","message":{"content":[{"type":"image_url","image_url":{"url":"data:image/png;base64,AAAA"}},{"type":"text","text":"literal text"}],"headers":{"secret":"message secret"},"authorization":"message token","images":["BBBB"]}}]});
        let trace=reply_evidence(&raw,false);
        let encoded=trace.to_string();
        for secret in ["transport secret","top secret","message secret","message token","AAAA","BBBB","data:image"] {assert!(!encoded.contains(secret),"{encoded}");}
        assert!(encoded.contains("literal text")&&encoded.contains("sha256"));
    }
    #[test]
    fn figure_params_may_be_lists_of_rows_text_or_null_without_aborting_the_run() {
        let tools = figure_tools();
        let render = tools.iter().find(|d| d["function"]["name"] == "render_figure").unwrap();
        let schema = &render["function"]["parameters"];
        for params in [json!({"angles":[30,60]}), json!({"labels":["a","b"]}), json!({"points":[[1,2],[3,4]]}), json!({"title":null,"on":true}), json!({"names":[["a",1]]})] {
            assert!(vershion_ai_core::validate(schema, &json!({"id":"angles","params":params})).is_ok());
        }
        for params in [json!({"diagram":{"nodes":{"A":[20,30],"B":[80,90]},"segments":[{"from":"A","to":"B"}]}}),json!({"curve_points":{"paths":[[[1,0],[0,1],[-1,0]]]}})] {
            assert!(vershion_ai_core::validate(schema,&json!({"id":"geometry-diagram","params":params})).is_ok(),"structured drawing reaches the local generator for semantic validation");
        }
    }

    #[test]
    fn optional_native_source_observation_is_valid_and_reaches_the_local_command(){
        let tools=figure_tools();let get=tools.iter().find(|d|d["function"]["name"]=="get_figure").unwrap();let schema=&get["function"]["parameters"];let args=json!({"id":"geometry-diagram","sees":"Two joined triangles, one arc at the left vertex and two at the right; no numeric angle labels."});assert!(vershion_ai_core::validate(schema,&args).is_ok());assert!(vershion_ai_core::validate(schema,&json!({"id":"geometry-diagram"})).is_ok());assert!(vershion_ai_core::validate(schema,&json!({"id":"geometry-diagram","sees":42})).is_err());let command=figure_command("get_figure",args.clone());assert_eq!(command["sees"],args["sees"]);assert_eq!(command["args"]["id"],"geometry-diagram");let command=figure_command("cannot_make_figure",json!({"reason":"Only an unreadable fragment is supplied.","sees":"Clipped label at the left edge."}));assert_eq!(command["sees"],"Clipped label at the left edge.");assert_eq!(command["cannot"],"Only an unreadable fragment is supplied.");
    }

    #[test]
    fn mixed_stored_crops_and_rendered_figures_keep_their_actual_mime_types() {
        let mut png = std::io::Cursor::new(Vec::new());
        let mut webp = std::io::Cursor::new(Vec::new());
        let im = image::DynamicImage::new_rgb8(8, 8);
        im.write_to(&mut png, image::ImageFormat::Png).unwrap();
        im.write_to(&mut webp, image::ImageFormat::WebP).unwrap();
        let mut turn = Turn::new("user", "Compare the source crop and the redraw.");
        turn.images = vec![webp.into_inner(), png.into_inner()];
        let wire = messages(&[turn], false);
        assert!(wire[0]["content"][1]["image_url"]["url"].as_str().unwrap().starts_with("data:image/webp;base64,"));
        assert!(wire[0]["content"][2]["image_url"]["url"].as_str().unwrap().starts_with("data:image/png;base64,"));
    }

    #[test]
    fn multipage_import_retries_invalid_json_and_preserves_chemistry_and_source_answers() {
        // Task metadata is already mapped here; missing-task recovery has its own
        // source-only tests. This test isolates parallel JSON repair/body preservation.
        let package=json!({"format":"queshion/ditto","formatVersion":"0.1.0","bank":{"name":"Chemistry","questions":[{"id":"q1","type":"short-answer","stem":"Name H₂O.","suggestedAnswer":"water"}]},"exams":[{"name":"Chemistry","sections":[{"title":"","instructions":"Use the source formula."}],"positions":[{"question":"q1","section":0}]}],"report":{"unconverted":[],"notes":[]}}).to_string();
        let (base, h) = stub_relay(vec![answer("not JSON"), answer(&package), answer(&package)]);
        let cfg = AiCfg {
            provider: "custom".into(),
            base,
            ..AiCfg::default()
        };
        let pages = vec![Vec::new(), Vec::new()];
        let results = crate::ai::convert_pages(&pages, "Chemistry", &cfg, &|_, _| {});
        assert_eq!(results.len(), 2);
        for result in results {
            let page = result.unwrap();
            assert_eq!(page.doc.questions.len(), 1);
            assert!(page.doc.questions[0].stem.ends_with("Name H₂O."));
            assert!(page.doc.questions[0].stem.contains("Use the source formula."));
            assert!(
                matches!(&page.doc.questions[0].kind,crate::model::Kind::Short{sample} if sample=="water")
            );
        }
        let sent = h.join().unwrap();
        assert_eq!(sent.len(), 3);
        assert!(
            sent.iter()
                .any(|(_, body)| body.contains("previous reply could not be used"))
        );
    }

    #[test]
    fn overlapping_operations_keep_their_own_sessions_in_every_chat_path() {
        let (base, h) = stub_relay(vec![
            (
                200,
                json!({"session":"session-a","credits":2,"left":38}).to_string(),
            ),
            (
                200,
                json!({"session":"session-b","credits":1,"left":37}).to_string(),
            ),
            answer("{}"),
            answer("{}"),
        ]);
        let cfg = AiCfg {
            provider: "workers".into(),
            base,
            ..AiCfg::default()
        };
        let a = begin_service(&cfg, "import", 2).unwrap();
        let b = begin_service(&cfg, "answer_key", 0).unwrap();
        chat_turns(&a.cfg, &[Turn::new("user", "first")], 0.0).unwrap();
        block_on(ask_images_async(&[], &b.cfg, "second", 0.0)).unwrap();
        let seen = h.join().unwrap();
        let bodies: Vec<Value> = seen
            .iter()
            .map(|(_, b)| serde_json::from_str(b).unwrap())
            .collect();
        assert_ne!(bodies[0]["operation"], bodies[1]["operation"]);
        assert_eq!(bodies[2]["session"], "session-a");
        assert_eq!(bodies[3]["session"], "session-b");
        a.cfg.operation.as_ref().unwrap().cancel();
        assert!(
            chat_turns(&a.cfg, &[Turn::new("user", "cancelled")], 0.0)
                .unwrap_err()
                .contains("cancelled")
        );
    }
    #[test]
    fn schemas_are_sent_and_bad_results_are_rejected_instead_of_silently_downgraded() {
        let (base, h) = stub_relay(vec![answer(r#"{"id":42,"reason":"bad"}"#)]);
        let cfg = AiCfg {
            provider: "openrouter".into(),
            base,
            output_mode: OutputMode::Schema,
            output: Some(vershion_ai_core::figure_pick()),
            ..AiCfg::default()
        };
        let err = chat_turns(&cfg, &[Turn::new("user", "pick")], 0.0).unwrap_err();
        assert!(err.starts_with("Invalid AI result:"));
        let seen = h.join().unwrap();
        let sent: Value = serde_json::from_str(&seen[0].1).unwrap();
        assert_eq!(sent["response_format"]["type"], "json_schema");
        assert_eq!(sent["provider"]["require_parameters"], true);
        assert_eq!(sent["provider"]["data_collection"], "deny");
    }
    #[test]
    fn mandatory_reasoning_retries_without_downgrading_structured_output() {
        let (base, h) = stub_relay(vec![
            (400, json!({"error":{"message":"Reasoning is mandatory for this endpoint and cannot be disabled."}}).to_string()),
            answer(r#"{"id":null,"reason":"none"}"#),
        ]);
        let cfg = AiCfg {
            provider: "openrouter".into(),
            base,
            output_mode: OutputMode::Schema,
            output: Some(vershion_ai_core::figure_pick()),
            ..AiCfg::default()
        };
        let raw = block_on(chat_turns_async(&cfg, &[Turn::new("user", "pick")], 0.0)).unwrap();
        assert_eq!(serde_json::from_str::<Value>(&raw).unwrap()["id"], Value::Null);
        let seen = h.join().unwrap();
        assert_eq!(seen.len(), 2);
        let mut first: Value = serde_json::from_str(&seen[0].1).unwrap();
        let second: Value = serde_json::from_str(&seen[1].1).unwrap();
        assert_eq!(first["reasoning"]["enabled"], false);
        first.as_object_mut().unwrap().remove("reasoning");
        assert_eq!(first, second);
        assert_eq!(second["response_format"]["type"], "json_schema");
        assert_eq!(second["max_tokens"], 8192);
    }
    #[test]
    fn unrelated_schema_error_is_not_retried_as_a_reasoning_error() {
        let (base, h) = stub_relay(vec![(400, json!({"error":{"message":"Invalid response_format schema"}}).to_string())]);
        let cfg = AiCfg {
            provider: "openrouter".into(),
            base,
            output_mode: OutputMode::Schema,
            output: Some(vershion_ai_core::figure_pick()),
            ..AiCfg::default()
        };
        let err = block_on(chat_turns_async(&cfg, &[Turn::new("user", "pick")], 0.0)).unwrap_err();
        assert!(err.contains("Invalid response_format schema"));
        assert_eq!(h.join().unwrap().len(), 1);
    }
    #[test]
    fn forced_result_tool_returns_validated_arguments() {
        let (base,h)=stub_relay(vec![(200,json!({"choices":[{"finish_reason":"tool_calls","message":{"content":null,"tool_calls":[{"id":"result1","type":"function","function":{"name":"submit_result","arguments":"{\"id\":null,\"reason\":\"none\"}"}}]}}]}).to_string())]);
        let cfg = AiCfg {
            provider: "custom".into(),
            base,
            output_mode: OutputMode::Tool,
            output: Some(vershion_ai_core::figure_pick()),
            ..AiCfg::default()
        };
        let raw = block_on(chat_turns_async(&cfg, &[Turn::new("user", "pick")], 0.0)).unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(&raw).unwrap()["id"],
            Value::Null
        );
        let seen = h.join().unwrap();
        let sent: Value = serde_json::from_str(&seen[0].1).unwrap();
        assert!(sent.get("response_format").is_none());
        assert_eq!(sent["tool_choice"]["function"]["name"], "submit_result");
    }
    #[test]
    fn native_figure_loop_executes_local_tools_and_replays_reasoning_unchanged() {
        let details = json!([{"type":"unknown-future-format","opaque":{"keep":true}}]);
        let call = |id: &str, name: &str, args: Value, reasoning: Value| {
            (200,json!({"choices":[{"finish_reason":"tool_calls","message":{"content":null,"reasoning_details":reasoning,"tool_calls":[{"id":id,"type":"function","function":{"name":name,"arguments":args.to_string()}}]}}]}).to_string())
        };
        let (base, h) = stub_relay(vec![
            call(
                "one",
                "get_figure",
                json!({"id":"regular-polygon"}),
                details.clone(),
            ),
            call("two","render_figure",json!({"id":"regular-polygon","params":{}}),json!([])),
            call(
                "three",
                "finish_figure",
                json!({"id":"regular-polygon","params":{},"note":"A regular polygon."}),
                json!([]),
            ),
        ]);
        let cfg = AiCfg {
            provider: "custom".into(),
            base,
            native_tools: true,
            ..AiCfg::default()
        };
        let run = block_on(crate::figure_agent::run_model_async(
            "a regular polygon",
            None,
            &cfg,
        ));
        assert!(run.error.is_none(), "{:?}", run.error);
        assert_eq!(run.spec.unwrap().id, "regular-polygon");
        assert!(run.png.unwrap().starts_with(b"\x89PNG"));
        let seen = h.join().unwrap();
        assert_eq!(seen.len(),3);
        let sent: Value = serde_json::from_str(&seen[1].1).unwrap();
        assert_eq!(sent["messages"][2]["reasoning_details"], details);
        assert_eq!(sent["messages"][3]["role"], "tool");
        assert_eq!(sent["messages"][3]["tool_call_id"], "one");
        assert!(
            sent["messages"][3]["content"]
                .as_str()
                .unwrap()
                .contains("regular-polygon")
        );
        assert!(sent.get("response_format").is_none());
    }

    #[test]
    fn traced_native_source_workflow_preserves_observer_maker_and_judge_evidence() {
        unsafe { std::env::set_var("VERSHION_STORE_DIR",std::env::temp_dir().join(format!("vershion-native-source-trace-tests-{}",std::process::id()))); }
        let marker="PRIVATE_SOURCE_OBSERVER_MARKER";
        let facts=json!({"structure":format!("{marker}: supplied synthetic source strokes"),"labels":[],"marks":[],"connections":[],"uncertain":[],"plot":null,"focus_box_xyxy_1000":null,"topology":{"status":"complete","reason":"One visible diagonal stroke.","nodes":[{"id":"a","position":"Top-left endpoint"},{"id":"b","position":"Bottom-right endpoint"}],"edges":[{"id":"diagonal","from":"a","to":"b","stroke":"Solid black"}],"crossings":[]}});
        let call=|id:&str,name:&str,args:Value|(200,json!({"choices":[{"finish_reason":"tool_calls","message":{"content":null,"tool_calls":[{"id":id,"type":"function","function":{"name":name,"arguments":args.to_string()}}]}}]}).to_string());
        let(base,server)=stub_relay(vec![
            answer(&facts.to_string()),
            call("get","get_figure",json!({"id":"regular-polygon"})),
            call("render","render_figure",json!({"id":"regular-polygon","params":{}})),
            call("finish","finish_figure",json!({"id":"regular-polygon","params":{},"note":"Synthetic trace fixture only."})),
            answer(r#"{"matches":true,"problems":[]}"#),
        ]);
        let cfg=AiCfg{provider:"custom".into(),base,native_tools:true,..AiCfg::default()};
        let source=image::DynamicImage::ImageRgb8(image::RgbImage::from_fn(16,16,|x,y|if x==y{image::Rgb([0,0,0])}else{image::Rgb([255,255,255])}));
        let mut source_png=Vec::new();source.write_to(&mut std::io::Cursor::new(&mut source_png),image::ImageFormat::Png).unwrap();
        let mut run=block_on(crate::figure_agent::run_model_async("Copy this synthetic source only.",Some(&source_png),&cfg));
        assert!(run.error.is_none(),"{:?}",run.error);
        assert!(run.review.as_ref().is_some_and(|v|v.matches));
        assert_eq!((run.selected_attempt,run.attempts.len()),(1,1));
        let observation=run.observation.as_ref().unwrap();
        assert_eq!(observation.facts.as_ref(),Some(&facts));
        let attempt=&run.attempts[0];
        assert_eq!(attempt.turns.len(),3);
        assert_eq!(attempt.turns[0].appended_messages[1].image_sha256,vec![observation.vision_sha256.clone()]);
        for(turn,name)in attempt.turns.iter().zip(["get_figure","render_figure","finish_figure"]) {
            assert_eq!(turn.provider_reply.as_ref().unwrap()["choice"]["message"]["tool_calls"][0]["function"]["name"],name);
            assert!(turn.reply.is_some()&&turn.error.is_none());
        }
        let judge=attempt.judge_turn.as_ref().unwrap();
        let hashes=&judge.appended_messages[0].image_sha256;
        assert_eq!(hashes.len(),4);
        assert_eq!(&hashes[..2],&[attempt.png_sha256.clone().unwrap(),observation.vision_sha256.clone()]);
        assert_eq!(judge.image_derivations.len(),2);
        assert!(judge.image_detail_error.is_none());
        for (i,detail) in judge.image_derivations.iter().enumerate() {
            assert_eq!((detail.source_image,detail.output_image),(i+1,i+3));
            assert_eq!(detail.png_sha256,hashes[i+2]);
            assert_eq!(detail.tiles.len(),2);
        }
        assert_eq!(judge.provider_reply.as_ref().unwrap()["choice"]["message"]["content"],r#"{"matches":true,"problems":[]}"#);
        let sent=server.join().unwrap();assert_eq!(sent.len(),5);
        assert!(sent[0].1.contains("No candidate drawing exists yet"));
        assert!(!sent[0].1.contains("finish_figure"));
        for request in &sent[1..4] {assert!(request.1.contains(marker));}
        assert!(!sent[4].1.contains(marker),"source observation does not contaminate the independent model reviewer");
        let judge_body:Value=serde_json::from_str(&sent[4].1).unwrap();
        assert_eq!(judge_body["messages"][0]["content"][0]["text"],judge.appended_messages[0].text);
        assert_eq!(judge_body["messages"][0]["content"].as_array().unwrap().iter().filter(|v|v["type"]=="image_url").count(),4);
        if let Ok(dir)=std::env::var("VERSHION_TRACE_FIXTURES_OUT") {
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(std::path::Path::new(&dir).join("native-observer-maker-judge.json"),serde_json::to_vec_pretty(&run.take_execution_trace()).unwrap()).unwrap();
        }
    }
    #[test]
    fn old_relays_negotiate_compatible_json_without_new_tool_fields() {
        let (base, h) = stub_relay(vec![
            (
                200,
                json!({"session":"old","credits":2,"left":38}).to_string(),
            ),
            answer("{}"),
        ]);
        let cfg = AiCfg {
            provider: "workers".into(),
            base,
            output_mode: OutputMode::Schema,
            native_tools: true,
            ..AiCfg::default()
        };
        let started = begin_service(&cfg, "import", 1).unwrap();
        assert_eq!(started.cfg.output_mode, OutputMode::Json);
        assert!(!started.cfg.native_tools);
        chat_turns(&started.cfg, &[Turn::new("user", "p")], 0.0).unwrap();
        let seen = h.join().unwrap();
        let sent: Value = serde_json::from_str(&seen[1].1).unwrap();
        assert_eq!(sent["response_format"]["type"], "json_object");
        assert!(sent.get("tools").is_none());
    }
}
