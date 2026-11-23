//! One current-page source-only task read, separate from continuation selection.
//! Literal model readings remain unverified; application keys bound metadata edits.
use super::*;
use sha2::{Digest, Sha256};

#[derive(Debug, serde::Serialize)]
pub struct PageTaskRecoveryTrace {
    pub page: usize,
    pub source_sha256: String,
    pub targets: Vec<usize>,
    pub attempts: usize,
    pub replies: Vec<Value>,
    pub confirmed_indices: Vec<usize>,
    pub not_printed_indices: Vec<usize>,
    pub blocked_indices: Vec<usize>,
    pub original_exams: Value,
    pub applied_exams: Value,
    /// Recovered directions the application did not apply, and why.
    pub skipped: Vec<String>,
    pub error: Option<String>,
}
struct Decisions {
    value: Value,
    directions: Vec<String>,
    printed: Vec<(usize, usize)>,
    not_printed: Vec<usize>,
    uncertain: Vec<usize>,
}
fn parse(text: &str, targets: &[usize]) -> Result<Decisions, String> {
    let value: Value = serde_json::from_str(text)
        .map_err(|_| "Current-page task decisions were not valid JSON.")?;
    vershion_ai_core::current_page_directions().validate(&value)?;
    let directions: Vec<String> = value["directions"]
        .as_array()
        .expect("schema checked directions")
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect();
    if directions.iter().any(|s| s.trim().is_empty()) {
        return Err("A printed task must have literal nonempty source text.".into());
    }
    let mut seen = Vec::new();
    let mut printed = Vec::new();
    let mut not_printed = Vec::new();
    let mut uncertain = Vec::new();
    for mapping in value["mappings"]
        .as_array()
        .expect("schema checked mappings")
    {
        let key = mapping["key"].as_str().unwrap();
        let index = key
            .strip_prefix("item")
            .and_then(|n| n.parse::<usize>().ok())
            .filter(|i| format!("item{i}") == key)
            .ok_or("Unknown application item key in current-page recovery.")?;
        if !targets.contains(&index) || seen.contains(&index) {
            return Err(
                "Current-page decisions must name each requested application key exactly once."
                    .into(),
            );
        }
        seen.push(index);
        if mapping["evidence"].as_str().unwrap().trim().is_empty() {
            return Err("Each current-page task decision needs literal source evidence.".into());
        }
        match mapping["status"].as_str().unwrap() {
            "printed" => {
                let direction = mapping["direction"]
                    .as_u64()
                    .and_then(|n| usize::try_from(n).ok())
                    .filter(|n| *n < directions.len())
                    .ok_or(
                        "A printed decision needs a valid current-page literal direction index.",
                    )?;
                printed.push((index, direction));
            }
            "not_printed" | "uncertain" => {
                if !mapping["direction"].is_null() {
                    return Err("Absent or uncertain current tasks must use direction=null.".into());
                }
                if mapping["status"] == "uncertain" {
                    uncertain.push(index);
                } else {
                    not_printed.push(index);
                }
            }
            _ => unreachable!("schema checked current task status"),
        }
    }
    if seen.len() != targets.len() {
        return Err("Current-page task decisions omitted requested items.".into());
    }
    Ok(Decisions {
        value,
        directions,
        printed,
        not_printed,
        uncertain,
    })
}

pub(super) async fn review(
    raw: String,
    bytes: &[u8],
    mime: &str,
    cfg: &AiCfg,
    page: usize,
    context: Option<&crate::ditto::DocContext>,
) -> (String, Option<PageTaskRecoveryTrace>) {
    let targets = crate::ditto::current_page_direction_targets(&raw, page, context);
    if targets.is_empty() {
        return (raw, None);
    }
    let original = crate::ditto::json_object(&raw).expect("targets require source object");
    let items: Vec<_> = targets
        .iter()
        .map(|i| {
            json!({
                "key":format!("item{i}"),"source_question":original["bank"]["questions"][i]
            })
        })
        .collect();
    let schema = vershion_ai_core::current_page_directions();
    let instruction = format!(
        "Read PRINTED TASK DIRECTIONS on this attached ORIGINAL PAGE {page} only. Recover metadata omitted by conversion, never solve or rewrite a question. Earlier-page tasks are deliberately not supplied. The read-only source items below are identifiers/context, NOT authority for task wording. Transcribe only task directions visibly printed on THIS page, keeping every qualifier, rounding clause, condition, unit and 'if necessary' phrase exactly. Do not manufacture a task from a worksheet title, heading, figure, expected exercise type, question body, answer, earlier memory or the conversion's report. A title alone is not a task. For EACH application key return: status=printed with a direction index ONLY if a literal task on this page is readable and its group clearly applies to that item; status=not_printed,direction=null when no current-page task is printed for it (a later, separate review may check same-document continuation); status=uncertain,direction=null when current task ink, wording, group ownership or applicability is unclear, including an unreadable/different current task. Never guess past uncertainty. Keep current headings/group boundaries: do not spread one printed task across unrelated or critical-thinking groups. Explain the visible source evidence for each decision. Return exactly one decision per application key, including text-only items; preserve their bodies, answers, figures and headings by returning NONE of those fields. Use application keys, never printed question numbers. Empty directions and explicit null decisions are valid. All literal readings remain model assertions, not independently verified source truth.\nRead-only items:\n{}\nRequired schema:\n{}",
        json!(items),
        schema.schema
    );
    let instruction = format!("{instruction}\nKeep each mapping's evidence concise, nonempty and at most 512 characters. Cite only the local printed task and group evidence; do not repeat the whole question or all part prompts. This limit does not shorten the literal directions themselves.");
    let original_exams = original.get("exams").cloned().unwrap_or(Value::Null);
    let mut trace = PageTaskRecoveryTrace {
        page,
        source_sha256: format!("{:x}", Sha256::digest(bytes)),
        targets: targets.clone(),
        attempts: 1,
        replies: Vec::new(),
        confirmed_indices: Vec::new(),
        not_printed_indices: Vec::new(),
        // Failure is conservative: only a fully valid explicit not_printed or
        // printed decision can unblock a targeted key.
        blocked_indices: targets.clone(),
        original_exams: original_exams.clone(),
        applied_exams: original_exams,
        skipped: Vec::new(),
        error: None,
    };
    let cfg = cfg.with_output(schema);
    let mut provider_reply = None;
    // Exactly one logical model request: no current-page format retry.
    let response = super::super::chat_message_traced_async(
        &cfg,
        &[single_turn(&instruction, mime, &[bytes])],
        0.0,
        &mut provider_reply,
    )
    .await;
    let parsed = match response {
        Ok(message) => {
            let text = vershion_ai_core::result_text(&message, None);
            let parsed = text
                .as_ref()
                .map_err(Clone::clone)
                .and_then(|s| parse(s, &targets));
            trace.replies.push(json!({"instruction":instruction,"source_sha256":trace.source_sha256,"provider_reply":provider_reply,"assistant_message":message,"raw_reply":text.as_ref().ok(),"result":parsed.as_ref().ok().map(|p|&p.value),"error":parsed.as_ref().err()}));
            parsed
        }
        Err(error) => {
            trace.replies.push(json!({"instruction":instruction,"source_sha256":trace.source_sha256,"provider_reply":provider_reply,"error":error}));
            Err(error)
        }
    };
    match parsed {
        Ok(decisions) => match crate::ditto::recover_printed_direction_mappings_noting(
            &raw,
            page,
            context,
            &decisions.directions,
            &decisions.printed,
        ) {
            Ok((corrected, skipped)) => {
                trace.skipped = skipped;
                trace.confirmed_indices = decisions.printed.iter().map(|p| p.0).collect();
                trace.not_printed_indices = decisions.not_printed;
                trace.blocked_indices = decisions.uncertain;
                trace.applied_exams = crate::ditto::json_object(&corrected)
                    .and_then(|v| v.get("exams").cloned())
                    .unwrap_or(Value::Null);
                (corrected, Some(trace))
            }
            Err(error) => {
                trace.error = Some(error);
                (raw, Some(trace))
            }
        },
        Err(error) => {
            trace.error = Some(error);
            (raw, Some(trace))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ditto::{ContextDirection, DocContext};
    const TASK: &str = "Estimate each measure.  Round to the nearest quarter-unit, if needed.";
    fn original() -> Value {
        let dir = std::env::temp_dir().join(format!(
            "vershion-current-task-tests-{}",
            std::process::id()
        ));
        unsafe {
            std::env::set_var("VERSHION_STORE_DIR", &dir);
        }
        crate::store::init(Box::new(crate::store::FsBackend::new(dir)));
        json!({"format":crate::ditto::FORMAT,"bank":{"name":"A title is not a task","exams":[],"questions":[
            {"id":"q1","label":"1","type":"short-answer","stem":"","suggestedAnswer":"","figure":{"box":[100,100,400,400],"alt":"Printed source shape"}},
            {"id":"q2","label":"2","type":"short-answer","stem":"A supplied shape has these printed dimensions.","suggestedAnswer":""}
        ]},"report":{"notes":["Original review annotation stays."],"unconverted":["An unrelated source issue."]}})
    }
    fn page() -> Vec<u8> {
        let mut png = Vec::new();
        image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(
            200,
            200,
            image::Rgb([0, 0, 0]),
        ))
        .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        .unwrap();
        png
    }
    fn context() -> DocContext {
        DocContext {
            document: "synthetic-document".into(),
            directions: vec![ContextDirection {
                text: "Earlier exact task. Keep its qualifier.".into(),
                page: 1,
                labels: vec!["1".into()],
                headings: Vec::new(),
                heading_page: None,
            }],
        }
    }
    fn decision(key: &str, status: &str, direction: Option<usize>) -> Value {
        json!({"key":key,"status":status,"direction":direction,"evidence":"Literal current-page group evidence; no title inference."})
    }
    fn answer(value: Value) -> (u16, String) {
        (
            200,
            json!({"choices":[{"message":{"content":value.to_string()}}]}).to_string(),
        )
    }
    fn cfg(base: String) -> AiCfg {
        AiCfg {
            provider: "custom".into(),
            base,
            model: "stub".into(),
            ..AiCfg::default()
        }
    }
    #[test]
    fn first_page_recovers_literal_qualifiers_then_same_document_continuation_uses_its_own_cap() {
        let raw = original();
        let bytes = page();
        let empty = DocContext {
            document: "synthetic-document".into(),
            directions: Vec::new(),
        };
        let reply = json!({"directions":[TASK],"mappings":[decision("item0","printed",Some(0)),decision("item1","printed",Some(0))]});
        let (base, server) = crate::ai::tests::stub_relay(vec![answer(reply)]);
        let (out, trace) = block_on(super::super::review(
            raw.to_string(),
            &bytes,
            "image/png",
            &cfg(base),
            1,
            Some(&empty),
        ));
        let sent = server.join().unwrap();
        assert_eq!(sent.len(), 1);
        let v = crate::ditto::json_object(&out).unwrap();
        assert_eq!(v["bank"], raw["bank"]);
        assert_eq!(v["report"], raw["report"]);
        assert_eq!(v["exams"][0]["sections"][0]["instructions"], TASK);
        assert!(v["exams"][0]["sections"][0]["inheritedFromPage"].is_null());
        let trace = trace.unwrap();
        assert_eq!(trace.attempts, 0);
        let recovery = trace.page_task_recovery.unwrap();
        assert_eq!(recovery.attempts, 1);
        assert_eq!(recovery.confirmed_indices, vec![0, 1]);
        assert!(recovery.blocked_indices.is_empty());
        assert_eq!(
            recovery.source_sha256,
            format!("{:x}", Sha256::digest(&bytes))
        );
        assert!(sent[0].1.contains("A title alone is not a task"));
        let imported =
            crate::ditto::import_text_in(&out, Some(&bytes), 1, "source", Some(&empty)).unwrap();
        assert!(imported.doc.exams.is_empty());
        assert_eq!(imported.items[0].status, "saved_figure_only");
        assert!(imported.items.iter().all(|i| {
            i.directions
                .as_ref()
                .is_some_and(|d| d.origin == "printed_on_page" && d.from_page == Some(1))
        }));
        assert!(imported.doc.questions.iter().all(|q| q.stem.contains(TASK)));
        let context = empty.after_page(1, &imported.items);
        assert_eq!(context.directions[0].text, TASK);
        let no_task = json!({"directions":[],"mappings":[decision("item0","not_printed",None),decision("item1","not_printed",None)]});
        let inherited = json!({"mappings":[{"key":"item0","direction":0,"reason":"Continues this exact source group."},{"key":"item1","direction":0,"reason":"Continues this exact source group."}]});
        let (base, server) = crate::ai::tests::stub_relay(vec![
            answer(no_task),
            answer(json!({"mappings":[]})),
            answer(inherited),
        ]);
        let (out, trace) = block_on(super::super::review(
            raw.to_string(),
            &bytes,
            "image/png",
            &cfg(base),
            2,
            Some(&context),
        ));
        assert_eq!(
            server.join().unwrap().len(),
            3,
            "One current-page read plus the unchanged maximum two continuation attempts."
        );
        let trace = trace.unwrap();
        assert_eq!(trace.attempts, 2);
        assert_eq!(trace.page_task_recovery.unwrap().attempts, 1);
        let imported =
            crate::ditto::import_text_in(&out, Some(&bytes), 2, "source", Some(&context)).unwrap();
        assert!(
            imported
                .items
                .iter()
                .all(|i| i.directions.as_ref().is_some_and(|d| d.origin
                    == "inherited_from_document_context"
                    && d.from_page == Some(1)))
        );
        assert_eq!(
            context.after_page(2, &imported.items).directions[0],
            context.directions[0]
        );
    }
    #[test]
    fn current_uncertainty_blocks_only_its_key_and_importer_never_auto_inherits_it() {
        let raw = original();
        let bytes = page();
        let c = context();
        let current = json!({"directions":[],"mappings":[decision("item0","uncertain",None),decision("item1","not_printed",None)]});
        let prior = json!({"mappings":[{"key":"item1","direction":0,"reason":"Earlier literal direction continues for this prose item."}]});
        let (base, server) = crate::ai::tests::stub_relay(vec![answer(current), answer(prior)]);
        let (out, trace) = block_on(super::super::review(
            raw.to_string(),
            &bytes,
            "image/png",
            &cfg(base),
            2,
            Some(&c),
        ));
        assert_eq!(server.join().unwrap().len(), 2);
        let trace = trace.unwrap();
        assert_eq!(trace.targets, vec![1]);
        assert_eq!(trace.page_task_recovery.unwrap().blocked_indices, vec![0]);
        let imported =
            crate::ditto::import_text_in(&out, Some(&bytes), 2, "source", Some(&c)).unwrap();
        assert!(imported.items[0].directions.is_none());
        assert_eq!(imported.items[0].status, "saved_figure_only_unresolved");
        assert!(
            imported.doc.questions[0]
                .stem
                .contains("Source task not visible")
        );
        assert_eq!(
            imported.items[1].directions.as_ref().unwrap().from_page,
            Some(1)
        );
    }
    #[test]
    fn no_figure_expression_and_self_contained_prose_do_not_infer_from_a_title() {
        let mut raw = original();
        raw["bank"]["questions"][0]
            .as_object_mut()
            .unwrap()
            .remove("figure");
        raw["bank"]["questions"][0]["stem"] = json!("$z^2+3z$");
        let bytes = page();
        assert_eq!(
            crate::ditto::current_page_direction_targets(&raw.to_string(), 1, None),
            vec![0, 1]
        );
        let value = json!({"directions":[TASK],"mappings":[decision("item0","printed",Some(0)),decision("item1","not_printed",None)]});
        let (base, server) = crate::ai::tests::stub_relay(vec![answer(value)]);
        let (out, trace) = block_on(super::super::review(
            raw.to_string(),
            &bytes,
            "image/png",
            &cfg(base),
            1,
            None,
        ));
        assert_eq!(server.join().unwrap().len(), 1);
        assert_eq!(
            crate::ditto::json_object(&out).unwrap()["bank"],
            raw["bank"]
        );
        assert_eq!(
            trace
                .unwrap()
                .page_task_recovery
                .unwrap()
                .not_printed_indices,
            vec![1]
        );
        let all_null = json!({"directions":[],"mappings":[decision("item0","not_printed",None),decision("item1","not_printed",None)]});
        let (base, server) = crate::ai::tests::stub_relay(vec![answer(all_null)]);
        let (out, trace) = block_on(super::super::review(
            raw.to_string(),
            &bytes,
            "image/png",
            &cfg(base),
            1,
            None,
        ));
        assert_eq!(server.join().unwrap().len(), 1);
        assert_eq!(out, raw.to_string());
        assert!(
            trace
                .unwrap()
                .page_task_recovery
                .unwrap()
                .confirmed_indices
                .is_empty()
        );
    }
    #[test]
    fn invalid_duplicate_or_malicious_decisions_are_atomic_and_failure_blocks_inheritance() {
        for mappings in [
            json!([
                decision("item99", "printed", Some(0)),
                decision("item1", "not_printed", None)
            ]),
            json!([
                decision("item0", "printed", Some(0)),
                decision("item0", "printed", Some(0))
            ]),
            json!([
                decision("item00", "printed", Some(0)),
                decision("item1", "not_printed", None)
            ]),
            json!([
                decision("item0", "printed", None),
                decision("item1", "not_printed", None)
            ]),
            json!([
                decision("item0", "printed", Some(9)),
                decision("item1", "not_printed", None)
            ]),
            json!([
                decision("item0", "uncertain", Some(0)),
                decision("item1", "not_printed", None)
            ]),
            json!([decision("item0", "not_printed", None)]),
        ] {
            assert!(
                parse(
                    &json!({"directions":[TASK],"mappings":mappings}).to_string(),
                    &[0, 1]
                )
                .is_err()
            );
        }
        let mut malicious = json!({"directions":[TASK],"mappings":[decision("item0","printed",Some(0)),decision("item1","printed",Some(0))]});
        malicious["questions"] = json!([{"stem":"rewrite source"}]);
        assert!(parse(&malicious.to_string(), &[0, 1]).is_err());
        let raw = original();
        let bytes = page();
        let bad = json!({"directions":[TASK],"mappings":[decision("item0","printed",Some(0)),decision("item0","printed",Some(0))]});
        for response in [
            answer(bad),
            (
                401,
                json!({"error":{"message":"Synthetic access failure"}}).to_string(),
            ),
            answer(json!({"directions":[],"mappings":[]})),
        ] {
            let (base, server) = crate::ai::tests::stub_relay(vec![response]);
            let (out, trace) = block_on(super::super::review(
                raw.to_string(),
                &bytes,
                "image/png",
                &cfg(base),
                2,
                Some(&context()),
            ));
            assert_eq!(server.join().unwrap().len(), 1);
            assert_eq!(out, raw.to_string());
            let trace = trace.unwrap();
            assert_eq!(trace.attempts, 0);
            let r = trace.page_task_recovery.unwrap();
            assert_eq!(r.attempts, 1);
            assert!(r.error.is_some());
            assert_eq!(r.blocked_indices, vec![0, 1]);
        }
    }
    #[test]
    fn valid_mappings_headings_question_payloads_and_unrelated_scope_holds_survive() {
        let mut raw = original();
        raw["bank"]["questions"][1]["suggestedAnswer"] =
            json!("original supplied source-key token");
        raw["bank"]["questions"][1]["explanation"] =
            json!("Original source note must survive metadata recovery.");
        raw["bank"]["questions"][1]["figure"] = json!({"box":[200,200,450,450],"alt":"Second source shape","review":"Original scope hold remains"});
        raw["exams"] = json!([{"name":"Original named test","sections":[{"title":"Protected current group","instructions":"Keep the original task."},{"title":"Second printed group","instructions":""}],"positions":[{"question":"q1","section":0},{"question":"q2","section":1}]}]);
        assert_eq!(
            crate::ditto::current_page_direction_targets(&raw.to_string(), 1, None),
            vec![1]
        );
        let bytes = page();
        let reply = json!({"directions":[TASK],"mappings":[decision("item1","printed",Some(0))]});
        let (base, server) = crate::ai::tests::stub_relay(vec![answer(reply)]);
        let (out, _) = block_on(super::super::review(
            raw.to_string(),
            &bytes,
            "image/png",
            &cfg(base),
            1,
            None,
        ));
        assert_eq!(server.join().unwrap().len(), 1);
        let v = crate::ditto::json_object(&out).unwrap();
        assert_eq!(v["bank"], raw["bank"]);
        assert_eq!(v["report"], raw["report"]);
        assert_eq!(v["exams"][0], raw["exams"][0]);
        let imported = crate::ditto::import_text_in(&out, Some(&bytes), 1, "source", None).unwrap();
        assert_eq!(imported.doc.exams.len(), 1);
        assert!(
            imported
                .items
                .iter()
                .all(|i| i.status == "saved_figure_box_unresolved"),
            "Adding a task must not erase independent crop overlap holds."
        );
        assert_eq!(imported.items[1].headings, vec!["Second printed group"]);
        assert!(imported.doc.questions[1].stem.contains(TASK));
        assert!(
            imported.doc.questions[1]
                .stem
                .contains("A supplied shape has these printed dimensions.")
        );
        let mut complete = original();
        complete["exams"] = json!([{"sections":[{"instructions":TASK}],"positions":[{"question":"q1","section":0},{"question":"q2","section":0}]}]);
        assert!(
            crate::ditto::current_page_direction_targets(&complete.to_string(), 1, None).is_empty()
        );
        let (out, trace) = block_on(super::super::review(
            complete.to_string(),
            &bytes,
            "image/png",
            &cfg("http://127.0.0.1:1/v1".into()),
            1,
            None,
        ));
        assert_eq!(out, complete.to_string());
        assert!(trace.is_none());
        let mut duplicate = original();
        duplicate["bank"]["questions"][1]["id"] = json!("q1");
        assert!(
            crate::ditto::current_page_direction_targets(&duplicate.to_string(), 1, None)
                .is_empty()
        );
        assert!(
            crate::ditto::recover_printed_direction_mappings(
                &complete.to_string(),
                1,
                None,
                &[TASK.into()],
                &[(0, 0)]
            )
            .is_err()
        );
    }
}
