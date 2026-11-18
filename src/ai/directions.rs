//! Bounded current-page task recovery followed by same-document continuation selection.
//! Both phases edit mappings only, preserving source questions and uncertainty holds.
use super::*;
mod current_page;
pub use current_page::PageTaskRecoveryTrace;

#[derive(Debug, serde::Serialize)]
pub struct DirectionReviewTrace {
    pub targets: Vec<usize>,
    pub attempts: usize,
    pub replies: Vec<Value>,
    pub confirmed_indices: Vec<usize>,
    pub original_exams: Value,
    pub applied_exams: Value,
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_task_recovery: Option<PageTaskRecoveryTrace>,
}
fn parse(
    text: &str,
    targets: &[usize],
    context: &crate::ditto::DocContext,
    page_no: usize,
) -> Result<(Value, Vec<(usize, usize)>), String> {
    let v: Value =
        serde_json::from_str(text).map_err(|_| "Direction decisions were not valid JSON.")?;
    vershion_ai_core::continuation_directions().validate(&v)?;
    let mut mappings = Vec::new();
    let mut seen = Vec::new();
    for d in v["mappings"].as_array().expect("validated schema") {
        let index = d["key"]
            .as_str()
            .and_then(|s| s.strip_prefix("item"))
            .and_then(|s| s.parse::<usize>().ok())
            .filter(|i| format!("item{i}") == d["key"].as_str().unwrap())
            .ok_or("Unknown application item key.")?;
        if !targets.contains(&index) || seen.contains(&index) {
            return Err("Direction decisions must name each requested item exactly once.".into());
        }
        seen.push(index);
        if d["reason"].as_str().unwrap().trim().is_empty() {
            return Err("Every direction decision needs source evidence in its reason.".into());
        }
        if let Some(number) = d["direction"].as_u64() {
            let n = usize::try_from(number).map_err(|_| "Invalid direction index.")?;
            if context
                .directions
                .get(n)
                .is_none_or(|d| d.page == 0 || d.page >= page_no || d.text.trim().is_empty())
            {
                return Err(
                    "Selected direction is not from an earlier page of this document.".into(),
                );
            }
            mappings.push((index, n));
        }
    }
    if seen.len() != targets.len() {
        return Err("Direction decisions omitted requested items.".into());
    }
    Ok((v, mappings))
}

pub(super) async fn review(
    raw: String,
    bytes: &[u8],
    mime: &str,
    cfg: &AiCfg,
    page_no: usize,
    context: Option<&crate::ditto::DocContext>,
) -> (String, Option<DirectionReviewTrace>) {
    let original_exams = crate::ditto::json_object(&raw)
        .and_then(|v| v.get("exams").cloned())
        .unwrap_or(Value::Null);
    let (raw, page_task_recovery) =
        current_page::review(raw, bytes, mime, cfg, page_no, context).await;
    let blocked = page_task_recovery
        .as_ref()
        .map(|t| t.blocked_indices.as_slice())
        .unwrap_or(&[]);
    let (raw, continuation) =
        continuation_review(raw, bytes, mime, cfg, page_no, context, blocked).await;
    match (continuation, page_task_recovery) {
        (Some(mut trace), recovery) => {
            trace.original_exams = original_exams;
            trace.page_task_recovery = recovery;
            (raw, Some(trace))
        }
        (None, Some(recovery)) => {
            let applied_exams = crate::ditto::json_object(&raw)
                .and_then(|v| v.get("exams").cloned())
                .unwrap_or(Value::Null);
            let error = recovery.error.clone();
            (
                raw,
                Some(DirectionReviewTrace {
                    targets: Vec::new(),
                    attempts: 0,
                    replies: Vec::new(),
                    confirmed_indices: Vec::new(),
                    original_exams,
                    applied_exams,
                    error,
                    page_task_recovery: Some(recovery),
                }),
            )
        }
        (None, None) => (raw, None),
    }
}

async fn continuation_review(
    raw: String,
    bytes: &[u8],
    mime: &str,
    cfg: &AiCfg,
    page_no: usize,
    context: Option<&crate::ditto::DocContext>,
    blocked: &[usize],
) -> (String, Option<DirectionReviewTrace>) {
    let Some(context) = context.filter(|c| !c.directions.is_empty()) else {
        return (raw, None);
    };
    let targets: Vec<_> = crate::ditto::continuation_targets(&raw, context, page_no)
        .into_iter()
        .filter(|i| !blocked.contains(i))
        .collect();
    if targets.is_empty() {
        return (raw, None);
    }
    let original = crate::ditto::json_object(&raw).expect("targets require source object");
    let questions:Vec<_>=targets.iter().map(|i|json!({"key":format!("item{i}"),"source_question":original["bank"]["questions"][i]})).collect();
    let directions:Vec<_>=context.directions.iter().enumerate().map(|(i,d)|json!({"direction":i,"printed_on_page":d.page,"printed_question_labels":d.labels,"headings":d.headings,"headings_printed_on_page":d.heading_page,"text":d.text})).collect();
    let schema = vershion_ai_core::continuation_directions();
    let prompt = format!(
        "This is page {page_no} of ONE source document. Check TASK DIRECTION MAPPINGS only against the attached ORIGINAL PAGE. These source questions have no valid section direction mapping. Earlier pages of this same document printed the exact directions listed below. For EACH application item key select the index of an earlier direction ONLY if its printed problem group continues here and that task applies to this item. Do not ignore earlier directions just because they are not reprinted. Preserve qualifiers such as rounding and 'if necessary' by choosing the whole listed direction. Current-page directions always win: if this item has different printed directions, belongs to a different task/group, or its continuation is uncertain, return direction=null and explain. Do not apply a direction to unrelated prose problems merely because they follow a figure. Never invent or edit a task, source stem, identity, figure, heading or answer; never solve. Use application keys rather than printed numbers. Return exactly one decision per requested key, with a source-specific reason.\nKnown earlier directions:\n{}\nRead-only source items:\n{}\nRequired schema:\n{}",
        json!(directions),
        json!(questions),
        schema.schema
    );
    let cfg = cfg.with_output(schema);
    let mut trace = DirectionReviewTrace {
        targets: targets.clone(),
        attempts: 0,
        replies: Vec::new(),
        confirmed_indices: Vec::new(),
        original_exams: original.get("exams").cloned().unwrap_or(Value::Null),
        applied_exams: original.get("exams").cloned().unwrap_or(Value::Null),
        error: None,
        page_task_recovery: None,
    };
    let mut instruction = prompt.clone();
    for attempt in 0..2 {
        trace.attempts += 1;
        let result = match super::engine::chat_message(
            &cfg,
            &[single_turn(&instruction, mime, &[bytes])],
            0.0,
        )
        .await
        {
            Ok(message) => {
                let text = vershion_ai_core::result_text(&message, None);
                let result = text
                    .as_ref()
                    .map_err(Clone::clone)
                    .and_then(|text| parse(text, &targets, context, page_no));
                trace.replies.push(json!({"assistant_message":message,"raw_reply":text.as_ref().ok(),"result":result.as_ref().ok().map(|p|&p.0)}));
                result
            }
            Err(error) => {
                let retry = error == "The AI returned no answer."
                    || error.starts_with("Invalid AI result:");
                trace.replies.push(json!({"error":error}));
                if !retry {
                    trace.error = Some(error);
                    return (raw, Some(trace));
                }
                Err(error)
            }
        };
        match result {
            Ok((_, mappings)) => {
                match crate::ditto::inherit_direction_mappings(&raw, context, page_no, &mappings) {
                    Ok(corrected) => {
                        trace.confirmed_indices = mappings.iter().map(|m| m.0).collect();
                        trace.applied_exams = crate::ditto::json_object(&corrected)
                            .and_then(|v| v.get("exams").cloned())
                            .unwrap_or(Value::Null);
                        return (corrected, Some(trace));
                    }
                    Err(error) => {
                        trace.error = Some(error);
                        return (raw, Some(trace));
                    }
                }
            }
            Err(error) => {
                if attempt == 1 {
                    trace.error = Some(error);
                    break;
                }
                instruction = format!(
                    "{prompt}\nThe previous reply could not be used: {error} Return only the required JSON decisions, keeping all requested keys exactly once. The same source page is attached."
                );
            }
        }
    }
    (raw, Some(trace))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ditto::{ContextDirection, DocContext};
    fn context() -> DocContext {
        DocContext {
            document: "one-document".into(),
            directions: vec![ContextDirection {
                text: "Find the volume. Round if necessary.".into(),
                page: 1,
                labels: vec!["1".into(), "6".into()],
                headings: Vec::new(),
                heading_page: None,
            }],
        }
    }
    fn raw() -> Value {
        let dir =
            std::env::temp_dir().join(format!("vershion-direction-tests-{}", std::process::id()));
        unsafe {
            std::env::set_var("VERSHION_STORE_DIR", &dir);
        }
        crate::store::init(Box::new(crate::store::FsBackend::new(dir)));
        json!({"format":crate::ditto::FORMAT,"bank":{"name":"Source","questions":[{"id":"q1","label":"7","type":"short-answer","stem":"","suggestedAnswer":"","figure":{"box":[100,100,400,400],"alt":"Printed solid"}},{"id":"q2","label":"11","type":"short-answer","stem":"A solid has these dimensions.","suggestedAnswer":""}]},"report":{"notes":["Earlier directions apply but were not mapped."]}})
    }
    fn page() -> Vec<u8> {
        let image = image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(
            200,
            200,
            image::Rgb([0, 0, 0]),
        ));
        let mut png = Vec::new();
        image
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .unwrap();
        png
    }
    fn answer(mappings: Value) -> (u16, String) {
        (
            200,
            json!({"choices":[{"message":{"content":json!({"mappings":mappings}).to_string()}}]})
                .to_string(),
        )
    }
    #[test]
    fn bounded_metadata_recovery_maps_figure_and_prose_without_rewriting_source() {
        let original = raw();
        let c = context();
        let (base, server) = super::super::tests::stub_relay(vec![
            answer(json!([])),
            answer(
                json!([{"key":"item0","direction":0,"reason":"Continues the solid problems."},{"key":"item1","direction":0,"reason":"The prose still asks for the solid volume."}]),
            ),
        ]);
        let cfg = AiCfg {
            provider: "custom".into(),
            base,
            model: "stub".into(),
            ..AiCfg::default()
        };
        let (out, trace) = block_on(continuation_review(
            original.to_string(),
            b"source",
            "image/png",
            &cfg,
            2,
            Some(&c),
            &[],
        ));
        assert_eq!(server.join().unwrap().len(), 2);
        let v = crate::ditto::json_object(&out).unwrap();
        assert_eq!(v["bank"], original["bank"]);
        assert_eq!(v["report"], original["report"]);
        assert_eq!(v["exams"][0]["sections"][0]["inheritedFromPage"], 1);
        assert_eq!(trace.unwrap().confirmed_indices, vec![0, 1]);
        let got =
            crate::ditto::import_text_in(&out, Some(&page()), 2, "fixture", Some(&c)).unwrap();
        assert!(
            got.doc.exams.is_empty(),
            "metadata recovery must not create extra user tests"
        );
        assert!(
            got.items
                .iter()
                .all(|i| i.directions.as_ref().is_some_and(|d| d.origin
                    == "inherited_from_document_context"
                    && d.from_page == Some(1)))
        );
        assert!(
            got.doc.questions[1]
                .stem
                .starts_with("Find the volume. Round if necessary.")
        );
    }
    #[test]
    fn explicit_unresolved_decisions_do_not_automatically_choose_the_last_task() {
        let c = context();
        let original = raw();
        let (base, server) = super::super::tests::stub_relay(vec![answer(
            json!([{"key":"item0","direction":null,"reason":"Different group on this page."},{"key":"item1","direction":null,"reason":"Unclear task."}]),
        )]);
        let cfg = AiCfg {
            provider: "custom".into(),
            base,
            model: "stub".into(),
            ..AiCfg::default()
        };
        let (out, trace) = block_on(continuation_review(
            original.to_string(),
            b"source",
            "image/png",
            &cfg,
            2,
            Some(&c),
            &[],
        ));
        server.join().unwrap();
        assert_eq!(out, original.to_string());
        assert!(trace.unwrap().confirmed_indices.is_empty());
    }
    #[test]
    fn new_tasks_duplicates_out_of_document_pages_and_unknown_keys_are_rejected() {
        let c = context();
        for mapping in [
            json!([{"key":"item7","direction":0,"reason":"x"}]),
            json!([{"key":"item0","direction":5,"reason":"x"}]),
            json!([{"key":"item0","direction":0,"reason":"x"},{"key":"item0","direction":0,"reason":"x"}]),
            json!([{"key":"item0","direction":0,"reason":""}]),
            json!([{"key":"item0","direction":null,"reason":""}]),
        ] {
            assert!(parse(&json!({"mappings":mapping}).to_string(), &[0], &c, 2).is_err());
        }
        assert!(
            parse(
                &json!({"mappings":[{"key":"item0","direction":0,"reason":"x"}]}).to_string(),
                &[0],
                &c,
                1
            )
            .is_err()
        );
        assert!(
            crate::ditto::inherit_direction_mappings(&raw().to_string(), &c, 2, &[(0, 2)]).is_err()
        );
    }
    #[test]
    fn printed_mappings_null_provenance_and_document_boundaries_are_preserved() {
        let mut v = raw();
        v["exams"] = json!([{"sections":[{"title":"New group","instructions":"Use the current task.","inheritedFromPage":null}],"positions":[{"question":"q1","section":0}]}]);
        let c = context();
        assert_eq!(
            crate::ditto::continuation_targets(&v.to_string(), &c, 2),
            vec![1]
        );
        assert!(
            crate::ditto::inherit_direction_mappings(&v.to_string(), &c, 2, &[(0, 0)]).is_err()
        );
        let got = crate::ditto::import_text_in(&v.to_string(), Some(&page()), 2, "test", Some(&c))
            .unwrap();
        assert_eq!(
            got.items[0].directions.as_ref().unwrap().origin,
            "printed_on_page"
        );
        assert!(
            crate::ditto::continuation_targets(&v.to_string(), &DocContext::default(), 2)
                .is_empty()
        );
        let mut duplicate = raw();
        duplicate["bank"]["questions"][1]["id"] = json!("q1");
        assert!(crate::ditto::continuation_targets(&duplicate.to_string(), &c, 2).is_empty());
    }
    #[test]
    fn heading_qualifiers_follow_their_source_task_without_creating_exams() {
        let mut source = raw();
        source["bank"]["questions"][0]["stem"] = json!("First source item.");
        source["exams"] = json!([{"name":"Original test","sections":[{"title":"Earlier ordinary group","instructions":"Find the volume."},{"title":"Critical thinking questions","instructions":"Find the volume."}],"positions":[{"question":"q1","section":0},{"question":"q2","section":1}]}]);
        let first =
            crate::ditto::import_text_in(&source.to_string(), None, 1, "source", None).unwrap();
        let context = DocContext {
            document: "one".into(),
            ..Default::default()
        }
        .after_page(1, &first.items);
        assert_eq!(context.directions.len(), 1);
        assert_eq!(
            context.directions[0].headings,
            vec!["Critical thinking questions"]
        );
        assert_eq!(context.directions[0].labels, vec!["11"]);
        let later = crate::ditto::inherit_direction_mappings(
            &raw().to_string(),
            &context,
            2,
            &[(0, 0), (1, 0)],
        )
        .unwrap();
        let got =
            crate::ditto::import_text_in(&later, Some(&page()), 2, "continuation", Some(&context))
                .unwrap();
        assert!(got.doc.exams.is_empty());
        assert!(
            got.doc
                .questions
                .iter()
                .all(|q| q.stem.contains("Critical thinking questions")
                    && q.stem.contains("Find the volume."))
        );
        let next = context.after_page(2, &got.items);
        assert_eq!(next.directions[0], context.directions[0]);
        let mut mapped = crate::ditto::json_object(&later).unwrap();
        mapped["exams"][0]["sections"][0]["title"] = json!("");
        let retained = crate::ditto::import_text_in(
            &mapped.to_string(),
            Some(&page()),
            2,
            "continuation",
            Some(&context),
        )
        .unwrap();
        assert!(
            retained.doc.questions[0]
                .stem
                .contains("Critical thinking questions"),
            "an explicit inherited task retains its known heading even if the model omitted title"
        );

        source["bank"]["questions"].as_array_mut().unwrap().push(json!({"id":"q3","label":"12","type":"short-answer","stem":"Returns to the earlier group.","suggestedAnswer":""}));
        source["exams"][0]["positions"]
            .as_array_mut()
            .unwrap()
            .push(json!({"question":"q3","section":0}));
        let first =
            crate::ditto::import_text_in(&source.to_string(), None, 1, "source", None).unwrap();
        let next = DocContext {
            document: "one".into(),
            ..Default::default()
        }
        .after_page(1, &first.items);
        assert_eq!(next.directions[0].headings, vec!["Earlier ordinary group"]);
        assert!(
            next.prompt_block(2).contains("questions 7, 12"),
            "nonconsecutive labels must not claim the intervening group"
        );
    }
    #[test]
    fn current_heading_wins_while_original_named_exam_survives() {
        let mut c = context();
        c.directions[0].headings = vec!["Earlier Critical thinking".into()];
        c.directions[0].heading_page = Some(1);
        let mut original = raw();
        original["exams"] = json!([{"name":"Original source test","sections":[{"title":"Current group","instructions":""}],"positions":[{"question":"q1","section":0}]}]);
        let out = crate::ditto::inherit_direction_mappings(&original.to_string(), &c, 2, &[(0, 0)])
            .unwrap();
        let parsed = crate::ditto::json_object(&out).unwrap();
        assert_eq!(parsed["exams"][0], original["exams"][0]);
        assert_eq!(parsed["bank"], original["bank"]);
        let got = crate::ditto::import_text_in(&out, Some(&page()), 2, "source", Some(&c)).unwrap();
        assert_eq!(got.doc.exams.len(), 1);
        assert_eq!(got.doc.exams[0].name, "Original source test");
        assert!(got.doc.questions[0].stem.contains("Current group"));
        let next = c.after_page(2, &got.items);
        assert_eq!(next.directions[0].headings, vec!["Current group"]);
        assert_eq!(next.directions[0].heading_page, Some(2));

        assert!(
            !got.doc.questions[0]
                .stem
                .contains("Earlier Critical thinking")
        );
    }
    #[test]
    fn explicit_current_heading_beats_an_explicit_inherited_heading_in_either_order() {
        let mut c = context();
        c.directions[0].headings = vec!["Earlier Critical thinking".into()];
        c.directions[0].heading_page = Some(1);
        for reverse in [false, true] {
            let mut v = raw();
            let inherited = json!({"name":"","sections":[{"title":"Earlier Critical thinking","instructions":c.directions[0].text,"inheritedFromPage":1}],"positions":[{"question":"q1","section":0}]});
            let current = json!({"name":"Original test","sections":[{"title":"Current group","instructions":"","inheritedFromPage":null}],"positions":[{"question":"q1","section":0}]});
            v["exams"] = if reverse {
                json!([current, inherited])
            } else {
                json!([inherited, current])
            };
            let got =
                crate::ditto::import_text_in(&v.to_string(), Some(&page()), 2, "source", Some(&c))
                    .unwrap();
            assert_eq!(got.items[0].headings, vec!["Current group"]);
            assert!(
                !got.doc.questions[0]
                    .stem
                    .contains("Earlier Critical thinking")
            );
            let next = c.after_page(2, &got.items);
            assert_eq!(next.directions[0].headings, vec!["Current group"]);
            assert_eq!(next.directions[0].heading_page, Some(2));
        }
    }
    #[test]
    fn multiple_inherited_qualifiers_keep_the_original_source_page() {
        let mut c = context();
        c.directions[0].headings = vec!["Section A".into(), "Critical thinking questions".into()];
        c.directions[0].heading_page = Some(1);
        let out =
            crate::ditto::inherit_direction_mappings(&raw().to_string(), &c, 2, &[(0, 0), (1, 0)])
                .unwrap();
        let got = crate::ditto::import_text_in(&out, Some(&page()), 2, "source", Some(&c)).unwrap();
        let next = c.after_page(2, &got.items);
        assert_eq!(next.directions[0], c.directions[0]);
    }
    #[test]
    fn authentication_errors_are_not_retried_or_guessed_around() {
        let (base, server) = super::super::tests::stub_relay(vec![(
            401,
            json!({"error":{"message":"fixture auth failure"}}).to_string(),
        )]);
        let cfg = AiCfg {
            provider: "custom".into(),
            base,
            model: "stub".into(),
            ..AiCfg::default()
        };
        let original = raw();
        let (out, trace) = block_on(continuation_review(
            original.to_string(),
            b"source",
            "image/png",
            &cfg,
            2,
            Some(&context()),
            &[],
        ));
        assert_eq!(server.join().unwrap().len(), 1);
        assert_eq!(out, original.to_string());
        assert_eq!(trace.unwrap().attempts, 1);
    }
}
