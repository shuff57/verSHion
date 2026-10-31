//! THE visible-derivation boundary (structural rule). Anything a non-author can see, or that decides an outcome a
//! non-author can observe (which source a chain credits, the "modified" flag, the licence a copy may carry,
//! may_republish_as, whether a source counts as evidence), is computed ONLY by comparing a `Subject` with a served
//! source here. When the publish hides its key, its Subject carries no full-content hash at all: there is nothing
//! key-bearing to compare, so no visible result can depend on the key. Full-content comparisons are possible only for
//! a publish that shows its key (whose key is public anyway). Port of worker/src/pool/visible.ts.
use std::collections::HashSet;

/// The new question, as far as visible derivations may look at it. Fields are private to this module's constructors
/// by convention: build one with `subject_for`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Subject {
    /// Key-free facts (match_hash, points, difficulty, topics, served image).
    pub keyfree: String,
    /// Full content (key included): only for a publish that shows its key; None when hidden.
    pub full: Option<String>,
    /// Licence sense: the full content without points, difficulty and topics; None when hidden.
    pub near: Option<String>,
    /// ... and its key-free part (match_hash and the served image): what a hidden subject is compared on.
    pub near_kf: String,
    /// The licence-sense hash without the explanation, and whether the subject has none.
    pub near_bare: Option<String>,
    pub no_expl: bool,
}

/// A served source question as the evidence query groups it (one per source item and qid).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Served {
    /// Some serve of it equals the subject (`same_sql`, decided in SQL from the Subject alone).
    pub same: bool,
    /// ... and that serve was the source's full form.
    pub same_full: bool,
}

pub fn subject_for(hide_key: bool, keyfree: &str, full: &str, near: &str, near_kf: &str, near_bare: Option<&str>, no_expl: bool) -> Subject {
    Subject {
        keyfree: keyfree.to_string(),
        full: (!hide_key).then(|| full.to_string()),
        near: (!hide_key).then(|| near.to_string()),
        near_kf: near_kf.to_string(),
        near_bare: (!hide_key).then(|| near_bare.unwrap_or(near).to_string()),
        no_expl: !hide_key && no_expl,
    }
}

/// The subject as the evidence query receives it: a hidden subject's full content is null (by construction).
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct SubjectParam {
    pub mh: String,
    pub full: Option<String>,
    pub kf: String,
    pub nfull: Option<String>,
    pub nkf: String,
    pub nbare: Option<String>,
    pub noexpl: i64,
    pub refs: Vec<SourceRef>,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct SourceRef {
    pub item: String,
    pub qid: String,
}

pub fn subject_param(s: &Subject, mh: &str, refs: Vec<SourceRef>) -> SubjectParam {
    SubjectParam { mh: mh.to_string(), full: s.full.clone(), kf: s.keyfree.clone(), nfull: s.near.clone(), nkf: s.near_kf.clone(), nbare: s.near_bare.clone(), noexpl: s.no_expl as i64, refs }
}

/// THE comparison, as SQL over a subject row `s` (from `subject_param`) and an evidence row `iq`: is the subject the
/// same question as that serve? A subject that shows its key is compared in full with exactly what was served (the
/// full form, or the stripped form of a hidden source: adding a key or an explanation to a stripped serve is a
/// modification); a hidden subject (full IS NULL) only on key-free facts. Nothing else anywhere compares questions for
/// a visible outcome.
pub fn same_sql(s: &str, iq: &str) -> String {
    format!("(CASE WHEN {s}.full IS NOT NULL THEN {iq}.content_hash = {s}.full ELSE {iq}.keyfree_hash = {s}.kf END)")
}

/// The licence-sense comparison: the same question as that serve, points, difficulty and topics aside (changing those
/// is not an adaptation, so it cannot relabel a copy). Same boundary as `same_sql`: a hidden subject (nfull IS NULL) is
/// compared on key-free facts only (match_hash and the served image). Whenever `same_sql` holds, so does this.
///
/// A shown subject is also a copy of a STRIPPED serve it matches key-free (adding the key is not an adaptation, so the
/// show/hide choice cannot flip the outcome), and of a serve it matches without the explanation when it has none
/// (dropping an explanation is not one either).
pub fn near_sql(s: &str, iq: &str) -> String {
    format!(
        "COALESCE(CASE WHEN {s}.nfull IS NOT NULL THEN ({iq}.near_hash = {s}.nfull OR ({s}.noexpl = 1 AND {iq}.near_bare = {s}.nbare)
     OR ({iq}.form = 'stripped' AND {s}.noexpl = 1 AND {iq}.near_kf = {s}.nkf)) ELSE {iq}.near_kf = {s}.nkf END, 0)"
    )
}

/// The author's own earlier work that was SHOWN: full contents of publishes that did not hide their key. Built only by
/// `own_shown`, which drops every hidden publish's content, so a hidden key of the author's other items can never
/// decide an evidence outcome others see.
#[derive(Clone, Debug, Default)]
pub struct OwnShown {
    shown: HashSet<String>,
}

impl OwnShown {
    pub fn contains(&self, content: &str) -> bool {
        self.shown.contains(content)
    }
}

pub fn own_shown<'a>(rows: impl IntoIterator<Item = (&'a str, bool)>) -> OwnShown {
    OwnShown { shown: rows.into_iter().filter(|(_, hidden)| !hidden).map(|(c, _)| c.to_string()).collect() }
}

/// One candidate source as the evidence query returns it.
#[derive(Clone, Debug)]
pub struct Candidate {
    pub served: Served,
    pub item_id: String,
    pub chain_items: Vec<String>,
    pub first_seq: i64,
}

/// Evidence admissibility. A source counts unless it is the publisher's own lineage (its item, or a chain through one)
/// or came later than their own first publish of the question. Such a source still counts when the publisher took ITS
/// additions: a publish that shows its key and equals the source's full served form, which is not the publisher's own
/// earlier SHOWN content (an original author copying an adapter's explanation verbatim).
pub fn admissible_with(h: &Candidate, s: &Subject, mine: &HashSet<String>, my_first_seq: Option<i64>, own: Option<&OwnShown>, voided: &HashSet<String>, voided_lineage: Option<&HashSet<String>>) -> bool {
    if voided.contains(&h.item_id) {
        return false;
    }
    // A source whose lineage runs through void (infringing) material counts only for a verbatim copy of it, i.e. of its
    // own additions: what else it shares came from the void item.
    let lineage = voided_lineage.unwrap_or(voided);
    if h.chain_items.iter().any(|x| lineage.contains(x)) && !h.served.same {
        return false;
    }
    let derived_from_mine = mine.contains(&h.item_id) || h.chain_items.iter().any(|x| mine.contains(x));
    let later = my_first_seq.is_some_and(|m| h.first_seq >= m);
    if !derived_from_mine && !later {
        return true;
    }
    if mine.contains(&h.item_id) {
        return false;
    }
    match &s.full {
        Some(full) => h.served.same_full && !own.is_some_and(|o| o.contains(full)),
        None => false,
    }
}

#[cfg(test)]
#[path = "visible_tests.rs"]
mod tests;
