//! Logical tool conversations, with image hashes instead of image data or transport headers.
use super::*;
use sha2::{Digest, Sha256};
pub fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
#[derive(Clone, Debug, Serialize)]
pub struct MessageTrace {
    pub role: &'static str,
    pub text: String,
    pub image_sha256: Vec<String>,
}
#[derive(Clone, Debug, Serialize)]
pub struct TurnTrace {
    pub turn: usize,
    pub limit: usize,
    pub appended_messages: Vec<MessageTrace>,
    pub reply: Option<String>,
    pub provider_reply: Option<Json>,
    pub error: Option<String>,
    /// Deterministic pixel-only views, not model observations or expected source facts.
    #[serde(skip_serializing_if="Vec::is_empty")]
    pub image_derivations: Vec<super::ImageDerivation>,
    #[serde(skip_serializing_if="Option::is_none")]
    pub image_detail_error: Option<String>,
}
impl TurnTrace {
    pub(super) fn capture(
        turn: usize,
        limit: usize,
        messages: &[Turn],
        response: &Result<String, String>,
    ) -> Self {
        Self {
            turn,
            limit,
            appended_messages: messages
                .iter()
                .map(|t| MessageTrace {
                    role: t.role,
                    text: t.text.clone(),
                    image_sha256: t.images.iter().map(|i| digest(i)).collect(),
                })
                .collect(),
            reply: response.as_ref().ok().cloned(),
            provider_reply: None,
            error: response.as_ref().err().cloned(),
            image_derivations:Vec::new(),
            image_detail_error:None,
        }
    }
}
#[derive(Clone, Debug, Serialize)]
pub struct AttemptTrace {
    pub number: usize,
    pub turns: Vec<TurnTrace>,
    pub steps: Vec<StepLog>,
    pub spec: Option<FigureSpec>,
    pub png_sha256: Option<String>,
    pub note: String,
    pub sees: Option<String>,
    pub review: Option<Verdict>,
    pub judge_error: Option<String>,
    pub judge_turn: Option<TurnTrace>,
    pub error: Option<String>,
    pub declined: Option<String>,
    #[serde(skip_serializing_if="Option::is_none")]
    pub source_plot_check: Option<Json>,
}
impl AttemptTrace {
    pub(super) fn capture(number: usize, run: &AgentRun, judge_error: Option<String>) -> Self {
        Self {
            number,
            turns: run.turns.clone(),
            steps: run.steps.clone(),
            spec: run.spec.clone(),
            png_sha256: run.png.as_deref().map(digest),
            note: run.note.clone(),
            sees: run.sees.clone(),
            review: run.review.clone(),
            judge_error,
            judge_turn: run.judge_turn.clone(),
            error: run.error.clone(),
            declined: run.declined.clone(),
            source_plot_check: run.source_plot_check.clone(),
        }
    }
}
#[derive(Debug, Serialize)]
pub struct ExecutionTrace {
    pub protocol: &'static str,
    pub selected_attempt: usize,
    pub attempts: Vec<AttemptTrace>,
    pub observation: Option<crate::ai::figure_observation::SourceObservation>,
    pub rejection_reason: Option<String>,
    pub error: Option<String>,
    pub declined: Option<String>,
}
impl AgentRun {
    pub fn take_execution_trace(&mut self) -> ExecutionTrace {
        ExecutionTrace {
            protocol: "figure_execution_v1",
            selected_attempt: self.selected_attempt,
            attempts: std::mem::take(&mut self.attempts),
            observation: self.observation.take(),
            rejection_reason: None,
            error: self.error.clone(),
            declined: self.declined.clone(),
        }
    }
}
