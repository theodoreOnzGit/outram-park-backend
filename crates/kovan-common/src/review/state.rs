//! The **stamp states** of code review (GitHub #765), shared by the
//! staleness engine ([`super::engine`]), desktop kovan and web-kovan's bottom
//! bar, so every view names a state the same way.

use serde::{Deserialize, Serialize};

/// What a function's (or one review's) state is. The plain kind; the
/// engine's [`super::engine::StampState`] carries the details.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StateKind {
    /// Reviewed, and nothing it rests on has changed. The only kind that
    /// counts towards maturity.
    Valid,
    /// Its own code changed since the review (or a callee now resolves
    /// differently): human re-review.
    DirectlyStale,
    /// Only its `///` doc changed: a quick look.
    DocChanged,
    /// A callee changed: human re-confirm, blocked while a reaching test
    /// fails (#739 D6, corrected 2026-10-07).
    InheritedStale,
    /// Found at a new place (rename, file or folder move) with the same
    /// hash and callees: awaiting acknowledge.
    Moved,
    /// Its function is gone; the review goes to the deleted history.
    Deleted,
    /// No review at all (including a function renamed and edited at once).
    New,
    /// An open needs-fix: blocks the function whatever its stamps.
    NeedsFixOpen,
    /// Edited after a needs-fix: ready for re-review.
    Fixed,
    /// The stamp's authenticity does not hold (agent trailer, time-bound
    /// check, unregistered or revoked reviewer, signature).
    Unverified,
    /// The reviewer's scope does not cover the function.
    OutsideScope,
    /// The `review.md` entry cannot be read: no review.
    Unreadable,
    /// `Cargo.lock` changed since the review: a full workspace test at the
    /// new lock clears it, no re-confirm.
    PendingWorkspaceTest,
    /// The entry reads but contradicts what is derived (e.g. a recorded
    /// rung the answers and git do not give): shown, never counted.
    Invalid,
}

/// How a view should colour a state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    /// Valid.
    Good,
    /// Needs a person (stale, moved, doc changed, fixed, pending).
    Attention,
    /// Blocks the function or does not count at all (needs-fix,
    /// unverified, outside scope, unreadable, deleted).
    Blocked,
    /// Never reviewed.
    Neutral,
}

impl StateKind {
    /// Every kind, in declaration order.
    pub const ALL: [StateKind; 14] = [
        Self::Valid,
        Self::DirectlyStale,
        Self::DocChanged,
        Self::InheritedStale,
        Self::Moved,
        Self::Deleted,
        Self::New,
        Self::NeedsFixOpen,
        Self::Fixed,
        Self::Unverified,
        Self::OutsideScope,
        Self::Unreadable,
        Self::PendingWorkspaceTest,
        Self::Invalid,
    ];

    /// Plain-English label.
    pub fn label(self) -> &'static str {
        match self {
            Self::Valid => "valid",
            Self::DirectlyStale => "changed since review",
            Self::DocChanged => "doc changed",
            Self::InheritedStale => "a callee changed",
            Self::Moved => "moved",
            Self::Deleted => "deleted",
            Self::New => "unreviewed",
            Self::NeedsFixOpen => "needs fix",
            Self::Fixed => "fixed, awaiting re-review",
            Self::Unverified => "unverified",
            Self::OutsideScope => "outside reviewer scope",
            Self::Unreadable => "review unreadable",
            Self::PendingWorkspaceTest => "pending workspace test",
            Self::Invalid => "review invalid",
        }
    }

    /// Whether the state counts as reviewed for maturity (only valid).
    pub fn counts(self) -> bool {
        self == Self::Valid
    }

    /// Whether it belongs in the desktop ⚑ queue (#740 U1).
    pub fn needs_person(self) -> bool {
        matches!(
            self,
            Self::DirectlyStale
                | Self::DocChanged
                | Self::InheritedStale
                | Self::Moved
                | Self::Fixed
                | Self::Unreadable
                | Self::Invalid
        )
    }

    pub fn tone(self) -> Tone {
        match self {
            Self::Valid => Tone::Good,
            Self::New => Tone::Neutral,
            Self::DirectlyStale
            | Self::DocChanged
            | Self::InheritedStale
            | Self::Moved
            | Self::Fixed
            | Self::PendingWorkspaceTest => Tone::Attention,
            Self::Deleted
            | Self::NeedsFixOpen
            | Self::Unverified
            | Self::OutsideScope
            | Self::Unreadable
            | Self::Invalid => Tone::Blocked,
        }
    }
}
