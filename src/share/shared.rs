//! The Shared page's state machine: which tab is showing and which guided step a person who is
//! not ready yet is on. No egui and no I/O: the pool models (`browse`, `mine`, `invite`, the
//! account) keep doing their own work; this only decides what the page asks for.
//!
//! # Guided steps
//!
//! ```text
//! 1 Sign in or create an account
//! 2 Confirm your email
//! 3 Wait for approval        (an invite code can already be entered here)
//! 4 Enter your invite code   (asked for when the pool answers invite_required)
//! 5 Browse                   (the tabs)
//! ```
//!
//! One step is shown at a time. Anything the person cannot fix from here (no server in this
//! build, offline, a rejected or suspended account) is shown as a plain note, not a step.

use super::api::Status;
use super::model::Page;
use super::publish::Gate;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Tab {
    /// Search the pool.
    Browse,
    /// Items I shared.
    Mine,
    /// Things I imported.
    Imported,
    /// Reports and appeals.
    Reports,
    /// Account details, invite code, data download, deleting the account, legal pages.
    Settings,
}

impl Tab {
    pub const ALL: [Tab; 5] = [Tab::Browse, Tab::Mine, Tab::Imported, Tab::Reports, Tab::Settings];

    pub fn label(self) -> &'static str {
        match self {
            Tab::Browse => "Browse the pool",
            Tab::Mine => "Items I shared",
            Tab::Imported => "Things I imported",
            Tab::Reports => "Reports and appeals",
            Tab::Settings => "My sharing settings",
        }
    }
}

/// Where a person stands on the way to the pool.
#[derive(Clone, Debug, PartialEq)]
pub enum Guide {
    /// The account is being looked up.
    Checking,
    /// A build made without a pool server: sharing is off.
    NotSetUp,
    /// A development build with no server typed in yet.
    NoServer,
    /// The sign-in could not be checked (offline).
    Offline,
    /// Step 1.
    SignIn,
    /// Step 2.
    ConfirmEmail,
    /// Step 3.
    Waiting,
    /// The account was rejected, revoked or suspended: nothing to do but read why.
    Blocked(Status),
    /// Step 4.
    NeedsInvite,
    /// Step 5: the tabs.
    Ready,
}

pub const STEPS: u8 = 5;

/// The one name of the sharing action, wherever it is offered or mentioned: the Shared page's
/// button, a bank's Share menu, an exam's File menu, and the hints that point to them.
pub const SHARE_ACTION: &str = "Share to pool…";

impl Guide {
    /// 1 to 5 for a guided step, `None` for a note.
    pub fn step(&self) -> Option<u8> {
        match self {
            Guide::SignIn => Some(1),
            Guide::ConfirmEmail => Some(2),
            Guide::Waiting => Some(3),
            Guide::NeedsInvite => Some(4),
            Guide::Ready => Some(5),
            _ => None,
        }
    }

    pub fn title(&self) -> &'static str {
        match self {
            Guide::Checking => "Checking your account...",
            Guide::NotSetUp => "Sharing is not set up in this build",
            Guide::NoServer => "No pool server yet",
            Guide::Offline => "Could not reach the pool",
            Guide::SignIn => "Sign in or create an account",
            Guide::ConfirmEmail => "Confirm your email",
            Guide::Waiting => "Waiting for approval",
            Guide::Blocked(_) => "This account can not share",
            Guide::NeedsInvite => "Enter your invite code",
            Guide::Ready => "Ready",
        }
    }

    /// One plain sentence on what to do next.
    pub fn help(&self) -> &'static str {
        match self {
            Guide::Checking => "Looking for a saved sign-in. Nothing is sent until you press a button.",
            Guide::NotSetUp => "Everything else in verSHion works as usual. The person who builds verSHion for your school sets the pool server.",
            Guide::NoServer => "Enter the address of a pool server below. Release builds have it built in.",
            Guide::Offline => "You may be offline. You can still use everything else in verSHion.",
            Guide::SignIn => "An account lets you find questions other teachers shared and share your own. It is optional; nothing else in verSHion needs one.",
            Guide::ConfirmEmail => "We sent a link to your email. Open it, or paste the link or code below.",
            Guide::Waiting => "A person at the pool checks every account by hand before it can share. This page checks again every minute. If you were given an invite code you can already enter it below.",
            Guide::Blocked(_) => "The reason, if the reviewer gave one, is below.",
            Guide::NeedsInvite => "The pool is invite-only. Enter the code an admin gave you; it is sent once and not kept.",
            Guide::Ready => "",
        }
    }
}

/// What the page should show, from the account's state. Pure.
pub fn guide_of(gate: &Gate, page: Page, invite_needed: bool, invite_redeemed: bool) -> Guide {
    match gate {
        Gate::Checking => Guide::Checking,
        Gate::NotSetUp => Guide::NotSetUp,
        Gate::NoServer => Guide::NoServer,
        Gate::Unreachable => Guide::Offline,
        Gate::SignedOut => match page {
            Page::CheckEmail(_) | Page::Confirm => Guide::ConfirmEmail,
            _ => Guide::SignIn,
        },
        Gate::NotApproved(Status::Pending) => Guide::Waiting,
        Gate::NotApproved(Status::Unverified) => Guide::ConfirmEmail,
        Gate::NotApproved(other) => Guide::Blocked(other.clone()),
        Gate::Ready if invite_needed && !invite_redeemed => Guide::NeedsInvite,
        Gate::Ready => Guide::Ready,
    }
}

/// The page's own state: the tab, whether an invite code is known to be needed, and the
/// "Share to pool…" chooser.
#[derive(Clone, Debug)]
pub struct SharedModel {
    pub tab: Tab,
    /// A pool call answered `invite_required` and no code was redeemed since.
    pub invite_needed: bool,
    /// The "Share to pool…" chooser is open.
    pub choosing: bool,
}

impl Default for SharedModel {
    fn default() -> Self {
        SharedModel { tab: Tab::Browse, invite_needed: false, choosing: false }
    }
}

impl SharedModel {
    pub fn goto(&mut self, tab: Tab) {
        self.tab = tab;
        self.choosing = false;
    }

    pub fn guide(&self, gate: &Gate, page: Page, invite_redeemed: bool) -> Guide {
        guide_of(gate, page, self.invite_needed, invite_redeemed)
    }

    /// A pool call said an invite code is needed.
    pub fn note_invite_required(&mut self) {
        self.invite_needed = true;
    }

    /// The code was accepted: the tabs come back and their windows ask again.
    pub fn note_redeemed(&mut self) {
        self.invite_needed = false;
    }

    pub fn toggle_chooser(&mut self) {
        self.choosing = !self.choosing;
    }
}
