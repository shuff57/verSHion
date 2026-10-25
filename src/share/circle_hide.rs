//! Hiding a circle bank's answer keys from its viewers (phase C, D12): the decisions the pool asks for
//! before it can make the viewers' copy. No egui, no I/O: the bank page (`PATCH .../banks/:bid
//! {hide_key}`) and a working copy's send to such a bank both use it, and `circle_hide_ui` draws it.
//!
//! The pool asks step by step, with the same refusals as a hidden-key publish: letter labels to strip
//! or keep (`labels_need_decision`), choice orders that have to stay as written (`order_ack_required`),
//! formatting that might give the answer away and that the author may say does not
//! (`key_in_formatting` with `ack_allowed`). Rules kept here:
//!
//! - Only what the pool listed is ever decided: a decision for a question it did not name is refused
//!   by the pool and still costs one of the day's tries (critic 56a V5).
//! - What was decided at an earlier step goes again with the next one (the pool reads them all), unless
//!   the pool said the bank changed under them: then every earlier decision is dropped (critic 57 B1).
//! - The pool's echo of a choice (`decision` in a label finding) is never taken as made: it is what was
//!   sent, maybe for questions that are others now.
//! - A finding that can not be acknowledged has to be changed in the question itself; nothing is sent
//!   while one is left.

use std::collections::{BTreeMap, BTreeSet};

use super::circles_api::StripDecisions;
use super::pool::{Issue, LabelChoice, LabelDecision, LabelFinding, PayloadRefusal, Pos, RefusalKind};

/// The person's decisions for one refusal of the pool, on top of what earlier steps decided.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Decisions {
    pub refusal: PayloadRefusal,
    labels: BTreeMap<Pos, LabelChoice>,
    orders: BTreeSet<Pos>,
    formatting: BTreeSet<usize>,
    /// What earlier steps of this change decided (sent again with this one).
    carried: StripDecisions,
    /// The pool said a decision named a question that no longer has letter labels: the bank changed, and
    /// everything decided before (labels, checks, orders) is dropped.
    pub restarted: bool,
    /// Questions where the pool refused Keep (keeping the letters would give the answer away) or Strip.
    no_keep: BTreeSet<usize>,
    no_strip: BTreeSet<usize>,
    /// The bank version this page was made for, when the caller knows it (the bank page reads the head
    /// again right before sending, and sends nothing when it moved).
    pub head: Option<u64>,
}

/// What is left to do before the decisions can be sent.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Todo {
    /// Findings that must be changed in the question text (they can not be acknowledged).
    pub hard: usize,
    pub labels: usize,
    pub orders: usize,
    pub formatting: usize,
}

impl Todo {
    pub fn left(&self) -> usize {
        self.labels + self.orders + self.formatting
    }
}

/// A refusal the person can answer with decisions (rather than an error to show).
pub fn decision_refusal(k: &RefusalKind) -> Option<&PayloadRefusal> {
    match k {
        RefusalKind::LabelsNeedDecision(p) | RefusalKind::OrderAckRequired(p) => Some(p),
        // A hidden-key check that only found formatting the author may acknowledge.
        RefusalKind::InvalidPayload(p) if !p.issues.is_empty() && !ackable_of(p).is_empty() => Some(p),
        _ => None,
    }
}

impl Decisions {
    /// A refusal of the pool, with what earlier steps of the same change decided.
    pub fn new(refusal: PayloadRefusal, carried: StripDecisions) -> Decisions {
        // (critic 56a V5: `labels_unused` means a label decision named a question without labels: the bank
        // changed under the earlier steps; their label decisions are dropped and asked again)
        let restarted = refusal.issues.iter().any(|i| matches!(i.code.as_str(), "labels_unused" | "labels_out_of_range" | "labels_duplicate"));
        let refused = |code: &str| refusal.issues.iter().filter(|i| i.code == code).filter_map(Issue::question_index).collect::<BTreeSet<usize>>();
        let (no_keep, no_strip) = (refused("labels_keep_refused"), refused("labels_strip_refused"));
        // (critic 57 B1) The decisions are by position: once the bank changed under them, an "I checked" or an
        // order made for one question would go with another. None of them is kept.
        let mut carried = if restarted { StripDecisions::default() } else { carried };
        // A refused choice is asked again.
        carried.labels.retain(|l| !(no_keep.contains(&l.question) && l.choice == LabelChoice::Keep) && !(no_strip.contains(&l.question) && l.choice == LabelChoice::Strip));
        Decisions { refusal, labels: BTreeMap::new(), orders: BTreeSet::new(), formatting: BTreeSet::new(), carried, restarted, no_keep, no_strip, head: None }
    }

    /// Whether this choice may be made for a finding (the pool allows it, and did not refuse it).
    pub fn allowed(&self, at: Pos, choice: LabelChoice) -> bool {
        let Some(f) = self.finding(at) else { return false };
        match choice {
            LabelChoice::Strip => f.strip_allowed && !self.no_strip.contains(&at.question),
            LabelChoice::Keep => !self.no_keep.contains(&at.question),
        }
    }

    pub fn label_findings(&self) -> &[LabelFinding] {
        &self.refusal.label_findings
    }

    /// The person's (or an earlier step's) choice for a finding. Never the pool's echo of one (critic 57 B1).
    pub fn label(&self, at: Pos) -> Option<LabelChoice> {
        let earlier = self.carried.labels.iter().find(|l| (l.question, l.part) == (at.question, at.part)).map(|l| l.choice);
        self.labels.get(&at).copied().or(earlier.filter(|c| self.allowed(at, *c)))
    }

    fn finding(&self, at: Pos) -> Option<&LabelFinding> {
        self.refusal.label_findings.iter().find(|f| (f.question, f.part) == (at.question, at.part))
    }

    /// Strip or Keep for a finding the pool listed. Anything else is ignored (Strip where the pool does not
    /// allow it too).
    pub fn decide_label(&mut self, at: Pos, choice: LabelChoice) {
        if self.allowed(at, choice) {
            self.labels.insert(at, choice);
        }
    }

    pub fn order_acked(&self, at: Pos) -> bool {
        self.orders.contains(&at) || self.carried.order_acks.contains(&at)
    }

    /// "Students may see these choices in this order", for a position the pool asked about.
    pub fn set_order(&mut self, at: Pos, yes: bool) {
        if !self.refusal.needs_order_ack.contains(&at) {
            return;
        }
        if yes {
            self.orders.insert(at);
        } else {
            self.orders.remove(&at);
        }
    }

    pub fn ack_all_orders(&mut self) {
        let all = self.refusal.needs_order_ack.clone();
        self.orders.extend(all);
    }

    /// Questions whose every finding the author may acknowledge.
    pub fn ackable(&self) -> Vec<usize> {
        ackable_of(&self.refusal)
    }

    pub fn formatting_acked(&self, question: usize) -> bool {
        self.formatting.contains(&question) || self.carried.ack_formatting.contains(&question)
    }

    /// "I checked: this does not give the answer away", for a question the pool flagged as low risk.
    pub fn set_formatting(&mut self, question: usize, yes: bool) {
        if !self.ackable().contains(&question) {
            return;
        }
        if yes {
            self.formatting.insert(question);
        } else {
            self.formatting.remove(&question);
        }
    }

    /// Findings that have to be changed in the questions (not a decision, not acknowledgeable).
    pub fn hard(&self) -> Vec<&Issue> {
        let ackable = self.ackable();
        self.refusal
            .issues
            .iter()
            .filter(|i| i.code != "order_ack_required" && !i.code.starts_with("labels_"))
            .filter(|i| !(i.can_acknowledge() && i.question_index().is_some_and(|q| ackable.contains(&q))))
            .collect()
    }

    pub fn todo(&self) -> Todo {
        Todo {
            hard: self.hard().len(),
            labels: self.refusal.label_findings.iter().filter(|f| self.label(Pos { question: f.question, part: f.part }).is_none()).count(),
            orders: self.refusal.needs_order_ack.iter().filter(|a| !self.order_acked(**a)).count(),
            formatting: self.ackable().into_iter().filter(|q| !self.formatting_acked(*q)).count(),
        }
    }

    /// Everything listed is decided and nothing has to be changed in the text.
    pub fn ready(&self) -> bool {
        let t = self.todo();
        t.hard == 0 && t.left() == 0
    }

    /// What goes with the next request: the earlier steps' decisions and this step's, for listed findings only.
    pub fn decisions(&self) -> StripDecisions {
        let mut out = self.carried.clone();
        for f in &self.refusal.label_findings {
            let at = Pos { question: f.question, part: f.part };
            if let Some(c) = self.label(at) {
                out.labels.retain(|l| (l.question, l.part) != (at.question, at.part));
                out.labels.push(LabelDecision { question: at.question, part: at.part, choice: c });
            }
        }
        for a in &self.orders {
            if !out.order_acks.contains(a) {
                out.order_acks.push(*a);
            }
        }
        for q in &self.formatting {
            if !out.ack_formatting.contains(q) {
                out.ack_formatting.push(*q);
            }
        }
        out.labels.sort_by_key(|l| (l.question, l.part));
        out.order_acks.sort();
        out.ack_formatting.sort_unstable();
        out
    }
}

fn ackable_of(p: &PayloadRefusal) -> Vec<usize> {
    let mut by_q: BTreeMap<usize, bool> = BTreeMap::new();
    for i in &p.issues {
        if i.code == "order_ack_required" || i.code.starts_with("labels_") {
            continue;
        }
        if let Some(q) = i.question_index() {
            let e = by_q.entry(q).or_insert(true);
            *e &= i.can_acknowledge();
        }
    }
    by_q.into_iter().filter(|(_, ok)| *ok).map(|(q, _)| q).collect()
}
