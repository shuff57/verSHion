//! The pool's route table (worker/src/pool/index.ts `ROUTES`): which path is which handler and which methods it takes. HTTP-free and regex-free; the
//! order of the Worker's table is kept (static names before `:id`). A path that matches no route is not the pool's (the caller answers 404).

/// A handler, with the path parameters it captured.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Route {
    Items,
    ItemsPreview,
    ItemPreview(String),
    Mine,
    Prefs,
    ItemVersions,
    Item(String),
    ItemUndelete(String),
    ItemImport(String),
    ItemReport(String),
    ItemAppeal(String),
    ItemCounterNotice(String),
    ItemBlob(String, String),
    BlobPut(String),
    MeExport,
    MeInvite,
    Circles,
    CircleJoinPreview,
    CircleJoin,
    CircleHeads,
    Circle(String),
    CircleReopen(String),
    CircleActivity(String),
    CircleInvites(String),
    CircleInvitesRevoke(String),
    CircleInvitesEmail(String),
    CircleLeave(String),
    CircleMembersRemove(String),
    CircleMembersRole(String),
    CircleTransfer(String),
    CircleTransferAccept(String),
    CircleTransferCancel(String),
    Banks(String),
    Bank(String, String),
    BankUndelete(String, String),
    BankReport(String, String),
    BankPurge(String, String),
    BankBlob(String, String, String),
    BankVersions(String, String),
    BankVersion(String, String, String),
    BankChanges(String, String),
    BankMerge(String, String),
    BankRollback(String, String),
    BankTrim(String, String),
}

/// `[^/]{1,80}`
fn word(s: &str) -> Option<String> {
    (!s.is_empty() && s.chars().count() <= 80).then(|| s.to_string())
}
/// `[0-9a-f]{32}`
fn id32(s: &str) -> Option<String> {
    (s.len() == 32 && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))).then(|| s.to_string())
}
/// `0|[1-9][0-9]{0,9}`
fn number(s: &str) -> Option<String> {
    let ok = s == "0" || (!s.is_empty() && s.len() <= 10 && !s.starts_with('0') && s.bytes().all(|b| b.is_ascii_digit()));
    ok.then(|| s.to_string())
}

/// The route a path belongs to, if any.
pub fn route_of(path: &str) -> Option<Route> {
    let rest = path.strip_prefix('/')?;
    let seg: Vec<&str> = rest.split('/').collect();
    use Route::*;
    match seg.as_slice() {
        ["items"] => Some(Items),
        ["items", "preview"] => Some(ItemsPreview),
        ["items", id, "preview"] => word(id).map(ItemPreview),
        ["items", "mine"] => Some(Mine),
        ["items", "prefs"] => Some(Prefs),
        ["items", "versions"] => Some(ItemVersions),
        ["items", id] => word(id).map(Item),
        ["items", id, "undelete"] => word(id).map(ItemUndelete),
        ["items", id, "import"] => word(id).map(ItemImport),
        ["items", id, "report"] => word(id).map(ItemReport),
        ["items", id, "appeal"] => word(id).map(ItemAppeal),
        ["items", id, "counter-notice"] => word(id).map(ItemCounterNotice),
        ["items", id, "blobs", h] => Some(ItemBlob(word(id)?, word(h)?)),
        ["blobs", h] => word(h).map(BlobPut),
        ["me", "export"] => Some(MeExport),
        ["me", "invite"] => Some(MeInvite),
        ["circles"] => Some(Circles),
        ["circles", "join", "preview"] => Some(CircleJoinPreview),
        ["circles", "join"] => Some(CircleJoin),
        ["circles", "heads"] => Some(CircleHeads),
        ["circles", c] => id32(c).map(Circle),
        ["circles", c, tail @ ..] => {
            let c = id32(c)?;
            match tail {
                ["reopen"] => Some(CircleReopen(c)),
                ["activity"] => Some(CircleActivity(c)),
                ["invites"] => Some(CircleInvites(c)),
                ["invites", "revoke"] => Some(CircleInvitesRevoke(c)),
                ["invites", "email"] => Some(CircleInvitesEmail(c)),
                ["leave"] => Some(CircleLeave(c)),
                ["members", "remove"] => Some(CircleMembersRemove(c)),
                ["members", "role"] => Some(CircleMembersRole(c)),
                ["transfer"] => Some(CircleTransfer(c)),
                ["transfer", "accept"] => Some(CircleTransferAccept(c)),
                ["transfer", "cancel"] => Some(CircleTransferCancel(c)),
                ["banks"] => Some(Banks(c)),
                ["banks", b, rest @ ..] => {
                    let b = id32(b)?;
                    match rest {
                        [] => Some(Bank(c, b)),
                        ["undelete"] => Some(BankUndelete(c, b)),
                        ["report"] => Some(BankReport(c, b)),
                        ["purge"] => Some(BankPurge(c, b)),
                        ["blobs", h] => Some(BankBlob(c, b, word(h)?)),
                        ["versions"] => Some(BankVersions(c, b)),
                        ["versions", n] => Some(BankVersion(c, b, number(n)?)),
                        ["changes"] => Some(BankChanges(c, b)),
                        ["merge"] => Some(BankMerge(c, b)),
                        ["rollback"] => Some(BankRollback(c, b)),
                        ["trim"] => Some(BankTrim(c, b)),
                        _ => None,
                    }
                }
                _ => None,
            }
        }
        _ => None,
    }
}

impl Route {
    /// The methods the route answers, in the Worker's order (the `Allow` header of a 405).
    pub fn methods(&self) -> &'static [&'static str] {
        use Route::*;
        match self {
            Items => &["GET", "POST"],
            ItemsPreview | ItemPreview(_) | ItemVersions | ItemUndelete(_) | ItemImport(_) | ItemReport(_) | ItemAppeal(_) | ItemCounterNotice(_) => &["POST"],
            Mine | ItemBlob(..) | MeExport | CircleActivity(_) | BankBlob(..) | BankVersions(..) | BankVersion(..) | BankChanges(..) => &["GET"],
            Prefs => &["GET", "PUT"],
            Item(_) => &["GET", "HEAD", "PUT", "PATCH", "DELETE"],
            BlobPut(_) => &["PUT"],
            MeInvite | CircleJoinPreview | CircleJoin | CircleHeads | CircleReopen(_) | CircleInvitesRevoke(_) | CircleInvitesEmail(_) | CircleLeave(_) | CircleMembersRemove(_) | CircleMembersRole(_) | CircleTransfer(_)
            | CircleTransferAccept(_) | CircleTransferCancel(_) | Banks(_) | BankUndelete(..) | BankReport(..) | BankPurge(..) | BankMerge(..) | BankRollback(..) | BankTrim(..) => &["POST"],
            Circles => &["GET", "POST"],
            Circle(_) => &["GET", "PATCH", "DELETE"],
            CircleInvites(_) => &["GET", "POST"],
            Bank(..) => &["GET", "HEAD", "PUT", "PATCH", "DELETE"],
        }
    }
}

#[cfg(test)]
#[path = "routetable_tests.rs"]
mod tests;
