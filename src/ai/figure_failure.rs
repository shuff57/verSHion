//! Reasons an editable redraw was not attached. A model's refusal is not proof
//! that the Figures catalogue lacks a generator or a particular capability.
use crate::figure_agent::{AgentRun, ExecutionTrace};
use super::figure_observation::ObservationErrorStage;

#[derive(Debug, serde::Serialize)]
pub struct FigureFailure {
    pub kind: &'static str,
    pub detail: String,
    pub generator: Option<String>,
    pub generator_registered: Option<bool>,
    pub reviewer_problems: Vec<String>,
    /// Actual failed attempts, including failures after the selected candidate.
    /// These are execution errors, not proof of a missing catalogue capability.
    #[serde(skip_serializing_if="Vec::is_empty")]
    pub attempt_errors: Vec<AttemptError>,
}

#[derive(Debug, serde::Serialize)]
pub struct AttemptError {
    pub execution: usize,
    pub attempt: usize,
    pub kind: &'static str,
    pub detail: String,
}

impl FigureFailure {
    pub(super) fn new(
        kind: &'static str,
        detail: impl Into<String>,
        generator: Option<&str>,
    ) -> Self {
        Self {
            kind,
            detail: detail.into(),
            generator: generator.map(str::to_owned),
            generator_registered: generator
                .map(|id| crate::figure_agent::registry().iter().any(|g| g.id == id)),
            reviewer_problems: Vec::new(),
            attempt_errors: Vec::new(),
        }
    }

    pub(super) fn warning(&self, label: &str) -> String {
        let available = self
            .generator
            .as_deref()
            .and_then(|id| crate::figure_agent::registry().iter().find(|g| g.id == id))
            .map(|g| format!("The {} figure type is available in Figures. ", g.title))
            .unwrap_or_default();
        let mut other: Vec<(&str, usize)> = Vec::new();
        for error in &self.attempt_errors {
            if self.detail.contains(&error.detail) { continue; }
            if let Some((_, count)) = other.iter_mut().find(|(text, _)| *text == error.detail) {
                *count += 1;
            } else {
                other.push((&error.detail, 1));
            }
        }
        let other = if other.is_empty() { String::new() } else {
            format!(" Attempt errors: {}.", other.iter().map(|(text,count)| {
                if *count == 1 { text.to_string() } else { format!("{count} attempts: {text}") }
            }).collect::<Vec<_>>().join("; "))
        };
        format!("Question {label}: editable figure not created. {available}{}{other} The source image was kept.", self.detail)
    }
}

fn source_failure(stage: Option<&ObservationErrorStage>, why: &str, generator: Option<&str>) -> FigureFailure {
    let (kind, detail) = match stage {
        Some(ObservationErrorStage::ImageInput) => ("source_image_error",format!("The source image could not be opened: {why}")),
        Some(ObservationErrorStage::Request) => ("source_read_request_failed",format!("The source-reading request failed: {why}")),
        Some(ObservationErrorStage::DescriptionFormat | ObservationErrorStage::DescriptionSchema | ObservationErrorStage::StructuralContract) =>
            ("source_description_invalid",format!("The AI returned an invalid figure description with missing or inconsistent fields: {why}")),
        Some(ObservationErrorStage::UncertainSource) => ("source_read_uncertain",format!("The AI could not resolve a detail in the source figure: {why}")),
        None => ("source_read_failed",format!("Source figure reading failed: {why}")),
    };
    FigureFailure::new(kind,detail,generator)
}

pub(super) fn from_run(
    run: &AgentRun,
    executions: &[ExecutionTrace],
    content_problem: Option<&str>,
) -> FigureFailure {
    let mut failure = primary_failure(run, executions, content_problem);
    for (execution_index, execution) in executions.iter().enumerate() {
        for attempt in &execution.attempts {
            for (kind, detail) in [
                ("generation_error", attempt.error.as_deref()),
                ("review_unavailable", attempt.judge_error.as_deref()),
            ] {
                if let Some(detail) = detail {
                    let bare = detail.strip_prefix("the model call failed: ").unwrap_or(detail);
                    let kind = if crate::figure_agent::native_protocol_error(bare) { "tool_protocol_error" }
                        else if bare.contains("reply was cut off") { "provider_output_truncated" }
                        else { kind };
                    failure.attempt_errors.push(AttemptError { execution:execution_index+1,
                        attempt:attempt.number, kind, detail:detail.to_owned() });
                }
            }
        }
    }
    failure
}

fn primary_failure(
    run: &AgentRun,
    executions: &[ExecutionTrace],
    content_problem: Option<&str>,
) -> FigureFailure {
    let generator = run.spec.as_ref().map(|s| s.id.as_str());
    if let Some(problem) = content_problem {
        return FigureFailure::new(
            "added_question_text",
            format!("The redraw added external question text: {problem}."),
            generator,
        );
    }
    if let Some(observation) = executions
        .last()
        .and_then(|e| e.observation.as_ref())
        .filter(|o| o.error.is_some())
    {
        return source_failure(observation.error_stage.as_ref(),observation.error.as_deref().unwrap(),generator);
    }
    if let Some(why) = &run.declined {
        return FigureFailure::new(
            "model_declined",
            format!(
                "The AI did not find a faithful drawing method: {why} This does not confirm a missing figure type or renderer option."
            ),
            generator,
        );
    }
    if let Some(why) = &run.error {
        if let Some(detail)=why.strip_prefix("Source plot contract conflict:") {
            return FigureFailure::new("source_contract_conflict",format!("The source-only reading and native drawing disagree: {}. The source reading is unverified; the original needs review before choosing which to correct.",detail.trim()),generator);
        }
        if crate::figure_agent::native_protocol_error(why.strip_prefix("the model call failed: ").unwrap_or(why)) {
            return FigureFailure::new("tool_protocol_error",
                format!("The model did not return a usable native figure tool call within the allowed turn budget: {why}"),generator);
        }
        return FigureFailure::new(
            "generation_error",
            format!("Drawing stopped with an error: {why}"),
            generator,
        );
    }
    if let Some(review) = run.review.as_ref().filter(|r| !r.matches) {
        let problems: Vec<_> = review
            .problems
            .iter()
            .filter(|s| !s.trim().is_empty())
            .cloned()
            .collect();
        let detail = if problems.is_empty() {
            "The automated source comparison rejected the redraw without a specific mismatch."
                .to_string()
        } else {
            format!(
                "The automated source comparison rejected the redraw: {}",
                problems.join("; ")
            )
        };
        let mut failure = FigureFailure::new("comparison_rejected", detail, generator);
        failure.reviewer_problems = problems;
        return failure;
    }
    let selected = executions
        .last()
        .and_then(|e| e.attempts.iter().find(|a| a.number == e.selected_attempt));
    let judge_error = selected
        .and_then(|a| a.judge_error.as_deref())
        .or_else(|| run.judge_turn.as_ref().and_then(|t| t.error.as_deref()));
    if run.spec.is_some() && run.png.is_some() && run.review.is_none() {
        return FigureFailure::new(
            "review_unavailable",
            format!(
                "The source comparison could not be completed: {}",
                judge_error.unwrap_or("no reviewer verdict was returned.")
            ),
            generator,
        );
    }
    FigureFailure::new(
        "unverified_output",
        "No complete, rendered and source-reviewed editable figure was produced.",
        generator,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::figure_agent::Verdict;

    #[test]
    fn rejected_candidate_keeps_later_output_failures() {
        let run = AgentRun { spec:Some(crate::model::FigureSpec{id:"direction-field".into(),params:Default::default()}),
            review:Some(Verdict{matches:false,problems:vec!["Two source-absent field rows.".into()]}),..Default::default() };
        let attempt = |number, error:&str| crate::figure_agent::AttemptTrace {
            number, turns:Vec::new(), steps:Vec::new(), spec:None, png_sha256:None,
            note:String::new(), sees:None, review:None, judge_error:None, judge_turn:None,
            error:Some(error.into()), declined:None, source_plot_check:None,
        };
        let cutoff = "the model call failed: The AI reply was cut off. Try a smaller input or a larger output limit.";
        let executions = vec![ExecutionTrace { protocol:"figure_execution_v1", selected_attempt:1,
            attempts:vec![attempt(2,cutoff),attempt(3,cutoff)],
            observation:None, first_observation:None, rejection_reason:None, error:None, declined:None }];
        let failure = from_run(&run,&executions,None);
        assert_eq!(failure.kind,"comparison_rejected");
        assert_eq!(failure.generator_registered,Some(true));
        assert_eq!(failure.attempt_errors.iter().map(|e|(e.attempt,e.kind)).collect::<Vec<_>>(),
            [(2,"provider_output_truncated"),(3,"provider_output_truncated")]);
        let warning=failure.warning("5");
        assert!(warning.contains("automated source comparison")&&warning.contains("Two source-absent field rows."));
        assert!(warning.contains("2 attempts:")&&warning.contains(cutoff));
        let value=serde_json::to_value(&failure).unwrap();
        assert_eq!(value["attempt_errors"][0]["execution"],1);
        assert_eq!(value["attempt_errors"][1]["attempt"],3);
    }

    #[test]
    fn application_plot_conflict_is_not_a_model_verdict_or_missing_generator() {
        let run=AgentRun{spec:Some(crate::model::FigureSpec{id:"coordinate-grid".into(),params:Default::default()}),error:Some("Source plot contract conflict: the native vertical lines extend above the source reading's top grid boundary".into()),..Default::default()};
        let failure=from_run(&run,&[],None);
        assert_eq!(failure.kind,"source_contract_conflict");
        assert_eq!(failure.generator_registered,Some(true));
        assert!(failure.detail.contains("unverified")&&failure.detail.contains("vertical lines"));
        assert!(failure.reviewer_problems.is_empty());
    }

    #[test]
    fn bad_ai_format_is_distinct_from_uncertain_source_and_failed_transport() {
        let format=source_failure(Some(&ObservationErrorStage::DescriptionSchema),"topology.status is required",None);
        let uncertain=source_failure(Some(&ObservationErrorStage::UncertainSource),"junction not settled",None);
        let request=source_failure(Some(&ObservationErrorStage::Request),"timeout",None);
        assert_eq!(format.kind,"source_description_invalid");
        assert!(format.detail.contains("AI returned an invalid figure description"));
        assert_eq!(uncertain.kind,"source_read_uncertain");
        assert_eq!(request.kind,"source_read_request_failed");
        for reason in [format,uncertain,request] { assert_eq!(reason.generator_registered,None); }
    }

    #[test]
    fn real_rejections_expose_reviewers_objections_instead_of_makers_compromise_notes() {
        let cases: serde_json::Value = serde_json::from_str(include_str!(
            "../../tests/worksheet_banks/fixtures/figure_rejections.json"
        ))
        .unwrap();
        for case in cases.as_array().unwrap() {
            let problems: Vec<String> = serde_json::from_value(case["problems"].clone()).unwrap();
            let run = AgentRun {
                spec: Some(crate::model::FigureSpec {
                    id: case["generator"].as_str().unwrap().into(),
                    params: Default::default(),
                }),
                note: case["candidate_note"].as_str().unwrap().into(),
                review: Some(Verdict {
                    matches: false,
                    problems: problems.clone(),
                }),
                ..Default::default()
            };
            let failure = from_run(&run, &[], None);
            let warning = failure.warning("27");
            assert_eq!(failure.kind, "comparison_rejected");
            assert_eq!(failure.generator_registered, Some(true));
            assert_eq!(failure.reviewer_problems, problems);
            for problem in problems {
                assert!(warning.contains(&problem));
            }
            assert!(warning.starts_with("Question 27:"));
            assert!(warning.contains("figure type is available in Figures."));
            assert!(
                !warning.contains(&run.note),
                "a maker's compromise note must not replace the actual rejection"
            );
        }
    }

    #[test]
    fn a_models_claim_of_no_generator_is_not_a_confirmed_catalogue_gap() {
        let run = AgentRun {
            declined: Some("No generator supports this projection".into()),
            ..Default::default()
        };
        let failure = from_run(&run, &[], None);
        assert_eq!(failure.kind, "model_declined");
        assert_eq!(failure.generator_registered, None);
        assert!(
            failure
                .detail
                .contains("does not confirm a missing figure type")
        );
    }

    #[test]
    fn an_unreachable_reviewer_is_not_reported_as_an_inaccurate_redraw() {
        let run = AgentRun {
            spec: Some(crate::model::FigureSpec {
                id: "scatterplot".into(),
                params: Default::default(),
            }),
            png: Some(vec![1]),
            ..Default::default()
        };
        let failure = from_run(&run, &[], None);
        assert_eq!(failure.kind, "review_unavailable");
        assert!(failure.detail.contains("no reviewer verdict"));
        assert!(failure.reviewer_problems.is_empty());
    }

    #[test]
    fn tool_and_service_errors_keep_the_actual_error_without_claiming_missing_features() {
        for (error,kind) in [
            ("The figure AI must request exactly one tool.","tool_protocol_error"),
            ("the model call failed: The figure AI must request exactly one tool.","tool_protocol_error"),
            ("the model call failed: Invalid figure tool arguments: missing id","tool_protocol_error"),
            ("HTTP 429","generation_error"),
            ("renderer could not decode params","generation_error"),
        ] {
            let run = AgentRun {
                error: Some(error.into()),
                ..Default::default()
            };
            let failure = from_run(&run, &[], None);
            assert_eq!(failure.kind,kind);
            assert!(failure.detail.contains(error));
            assert_eq!(failure.generator_registered, None);
        }
    }
}
