//! (critic 71, round 5) How a restore, trim, send, Get latest, unlink or check went, when the person did not see it
//! on the screen where it happened: one list, kept per bank. Each result names its bank (the linked bank on this
//! computer, or, for a circle bank with no copy here, the circle bank itself) and its kind. A result stays until
//! the person dismisses it, or until the next outcome of the same kind for the same bank replaces it (some kinds are
//! also made out of date by another: "press Get latest to catch up" by a Get latest). Another bank's activity never
//! touches it. What one account was told goes when another account signs in (`clear`); a sign-out and the same
//! account signing in again keep it. Pure: no I/O, nothing drawn.

/// What the result is about.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResultKind {
    Restore,
    Trim,
    Send,
    GetLatest,
    Unlink,
    Check,
    /// The session ended while something of this bank was on its way.
    Session,
    /// (critic 73 minors 1, 6) No answer came in time for a restore or trim: it may still go through.
    NoAnswer,
}

impl ResultKind {
    /// The other kinds whose next outcome for the same bank makes a result of this kind out of date.
    fn replaced_by(self) -> &'static [ResultKind] {
        use ResultKind::*;
        match self {
            // "The version was sent, but the answer could not be read. Press Get latest to catch up."
            Send => &[GetLatest],
            // "Your bank is now used as it is ... the next send sends it as it is."
            GetLatest => &[Send],
            // "Your session ended": anything that went through since says more.
            Session => &[Restore, Trim, Send, GetLatest, Unlink, Check],
            // "No answer came for the restore ...": the next restore or trim of the bank says how that one went
            NoAnswer => &[Restore, Trim],
            Restore | Trim | Unlink | Check => &[],
        }
    }
}

/// The bank a result is about: its linked bank on this computer (`local`), and its circle bank.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ResultBank {
    pub local: Option<String>,
    pub cid: String,
    pub bid: String,
    /// The bank's name, said where a result of a bank with no copy here is drawn.
    pub name: String,
}

impl ResultBank {
    /// The same bank: the same linked bank, or the same circle bank.
    fn same(&self, o: &ResultBank) -> bool {
        let local = matches!((&self.local, &o.local), (Some(a), Some(b)) if a == b);
        let circle = !self.bid.is_empty() && self.cid == o.cid && self.bid == o.bid;
        local || circle
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActResult {
    pub id: u64,
    pub bank: ResultBank,
    pub kind: ResultKind,
    pub text: String,
}

/// The results not dismissed yet, oldest first.
#[derive(Clone, Debug, Default)]
pub struct ActResults {
    list: Vec<ActResult>,
    next: u64,
}

impl ActResults {
    /// Says how something went for a bank. The bank's earlier result of the same kind goes (the next outcome replaces
    /// it), and so do its results this kind makes out of date. Returns the new result's id.
    pub fn put(&mut self, bank: ResultBank, kind: ResultKind, text: impl Into<String>) -> u64 {
        self.settle(&bank, kind);
        self.next += 1;
        self.list.push(ActResult { id: self.next, bank, kind, text: text.into() });
        self.next
    }

    /// Something of this kind happened to the bank, or started, where the person sees it: the bank's results it
    /// replaces go, and nothing new is said.
    pub fn settle(&mut self, bank: &ResultBank, kind: ResultKind) {
        self.list.retain(|r| !(r.bank.same(bank) && (r.kind == kind || r.kind.replaced_by().contains(&kind))));
    }

    /// (critic 78 I1) The bank's result of this kind says something else now (what a check found): its words change, it
    /// stays where it is. True when there was such a result; nothing is said when it was dismissed.
    pub fn replace_text(&mut self, bank: &ResultBank, kind: ResultKind, text: &str) -> bool {
        let mut found = false;
        for r in self.list.iter_mut().filter(|r| r.bank.same(bank) && r.kind == kind) {
            r.text = text.to_string();
            found = true;
        }
        found
    }

    /// "Dismiss". True when there was such a result.
    pub fn dismiss(&mut self, id: u64) -> bool {
        let n = self.list.len();
        self.list.retain(|r| r.id != id);
        self.list.len() != n
    }

    /// The results of one linked bank (its strip, and its screens).
    pub fn of_local(&self, local: &str) -> Vec<&ActResult> {
        self.list.iter().filter(|r| r.bank.local.as_deref() == Some(local)).collect()
    }

    /// The results of one circle bank, linked here or not (its Bank settings page, and its screens there).
    pub fn of_circle_bank(&self, cid: &str, bid: &str) -> Vec<&ActResult> {
        self.list.iter().filter(|r| r.bank.cid == cid && r.bank.bid == bid).collect()
    }

    /// The results of circle banks with no copy on this computer (said at the top of the Banks list, named).
    pub fn bankless(&self) -> Vec<&ActResult> {
        self.list.iter().filter(|r| r.bank.local.is_none()).collect()
    }

    /// (critic 73 minor 6) Drops the bank's results of one kind, and only those: what they said stopped being true (the
    /// circle's newest version moved on: "used as it is, as matching the newest version", "it may still go through").
    pub fn drop_of(&mut self, bank: &ResultBank, kind: ResultKind) {
        self.list.retain(|r| !(r.bank.same(bank) && r.kind == kind));
    }

    /// Drops the results of one kind wherever they are (a sign-in again: "your session ended" is no longer so).
    pub fn drop_kind(&mut self, kind: ResultKind) {
        self.list.retain(|r| r.kind != kind);
    }

    /// The bank is no longer linked here: its results, about a link that is gone, go too.
    pub fn forget_local(&mut self, local: &str) {
        self.list.retain(|r| r.bank.local.as_deref() != Some(local));
    }

    /// Another account signed in: nothing the earlier one was told stays.
    pub fn clear(&mut self) {
        self.list.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.list.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::ResultKind::*;
    use super::*;

    fn a() -> ResultBank {
        ResultBank { local: Some("bank-a".into()), cid: "c".into(), bid: "x".into(), name: "A".into() }
    }
    fn b() -> ResultBank {
        ResultBank { local: Some("bank-b".into()), cid: "c".into(), bid: "y".into(), name: "B".into() }
    }
    fn z() -> ResultBank {
        ResultBank { local: None, cid: "c".into(), bid: "z".into(), name: "Z".into() }
    }
    fn texts(v: Vec<&ActResult>) -> Vec<String> {
        v.into_iter().map(|r| r.text.clone()).collect()
    }

    #[test]
    fn another_banks_result_never_replaces_this_ones() {
        let mut r = ActResults::default();
        r.put(a(), Restore, "A failed");
        r.put(b(), Restore, "B done");
        r.put(b(), Trim, "B trimmed");
        assert_eq!(texts(r.of_local("bank-a")), ["A failed"]);
        assert_eq!(texts(r.of_local("bank-b")), ["B done", "B trimmed"]);
    }

    #[test]
    fn the_next_outcome_of_the_same_kind_replaces_the_last() {
        let mut r = ActResults::default();
        r.put(a(), Restore, "did not go through");
        r.put(a(), Trim, "trimmed");
        r.put(a(), Restore, "went through");
        assert_eq!(texts(r.of_local("bank-a")), ["trimmed", "went through"]);
        // started again where it is seen: the old one goes, nothing new is said
        r.settle(&a(), Trim);
        assert_eq!(texts(r.of_local("bank-a")), ["went through"]);
    }

    #[test]
    fn a_result_goes_when_a_kind_makes_it_out_of_date() {
        let mut r = ActResults::default();
        r.put(a(), Send, "press Get latest to catch up");
        r.put(a(), Session, "your session ended");
        r.settle(&a(), Restore);
        assert_eq!(texts(r.of_local("bank-a")), ["press Get latest to catch up"], "a restore says more than the session note");
        r.settle(&a(), GetLatest);
        assert!(r.of_local("bank-a").is_empty());
        r.put(a(), GetLatest, "used as it is");
        r.settle(&a(), Send);
        assert!(r.of_local("bank-a").is_empty());
        // and only for its own bank
        r.put(b(), Send, "B: catch up");
        r.settle(&a(), GetLatest);
        assert_eq!(texts(r.of_local("bank-b")), ["B: catch up"]);
    }

    #[test]
    fn drop_of_takes_one_kind_of_one_bank_only() {
        let mut r = ActResults::default();
        r.put(a(), GetLatest, "used as it is");
        r.put(a(), Send, "press Get latest");
        r.put(b(), GetLatest, "B used as it is");
        r.drop_of(&a(), GetLatest);
        assert_eq!(texts(r.of_local("bank-a")), ["press Get latest"], "a Send result made out of date by a Get latest stays");
        assert_eq!(texts(r.of_local("bank-b")), ["B used as it is"]);
        r.put(a(), NoAnswer, "may still go through");
        r.settle(&a(), Trim);
        assert_eq!(texts(r.of_local("bank-a")), ["press Get latest"], "the next restore or trim replaces it");
    }

    #[test]
    fn dismiss_removes_one_result_only() {
        let mut r = ActResults::default();
        let one = r.put(a(), Restore, "one");
        r.put(a(), Unlink, "two");
        assert!(r.dismiss(one));
        assert!(!r.dismiss(one));
        assert_eq!(texts(r.of_local("bank-a")), ["two"]);
    }

    #[test]
    fn a_circle_bank_with_no_copy_here_is_kept_apart_and_found_by_its_circle_bank() {
        let mut r = ActResults::default();
        r.put(z(), Restore, "Z restored");
        r.put(a(), Restore, "A restored");
        assert_eq!(texts(r.bankless()), ["Z restored"]);
        assert_eq!(texts(r.of_circle_bank("c", "z")), ["Z restored"]);
        assert_eq!(texts(r.of_circle_bank("c", "x")), ["A restored"], "a linked bank's results are on its Bank settings too");
        assert!(r.of_local("bank-a").iter().all(|x| x.text != "Z restored"));
        // the same circle bank, linked meanwhile: one result per kind
        let mut linked = z();
        linked.local = Some("bank-z".into());
        r.put(linked, Restore, "Z again");
        assert_eq!(texts(r.of_circle_bank("c", "z")), ["Z again"]);
        assert!(r.bankless().is_empty());
    }

    #[test]
    fn forget_local_drop_kind_and_clear() {
        let mut r = ActResults::default();
        r.put(a(), Restore, "A");
        r.put(b(), Session, "B session");
        r.put(z(), Trim, "Z");
        r.forget_local("bank-a");
        assert!(r.of_local("bank-a").is_empty());
        r.drop_kind(Session);
        assert!(r.of_local("bank-b").is_empty());
        assert!(!r.is_empty());
        r.clear();
        assert!(r.is_empty());
    }
}
