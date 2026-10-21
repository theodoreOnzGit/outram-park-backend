//! **Recently reviewed** (GitHub #740 U1, maintainer 2026-10-07: "a small
//! corner tab or sidebar lists the last places reviewed. Optional; it
//! never replaces the map."), read from `review.md` itself: every review
//! and needs-fix entry, newest first by its signing time (`signed_at`,
//! else `modified`), then date. Nothing is stored beside `review.md`, so
//! the list is the same on every machine with the same checkout.

use std::collections::BTreeMap;

use kovan_common::review::review_md::Entry;

use super::queue::FnInfo;
use super::Workspace;

/// What was recorded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecentWhat {
    Stamped,
    NeedsFix,
}

/// One place reviewed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecentEntry {
    pub what: RecentWhat,
    /// `file.rs::qual` as the entry records it.
    pub path: String,
    /// kovan-web's key of the function now (empty when it is gone).
    pub call_graph_id: String,
    pub by: String,
    /// `YYYY-MM-DD`.
    pub date: String,
    /// Sort key: `signed_at`, else the entry's `modified`.
    pub when: String,
}

impl RecentEntry {
    /// "stamped twice · github:a · 2026-10-10".
    pub fn line(&self) -> String {
        let what = match self.what {
            RecentWhat::Stamped => "stamped",
            RecentWhat::NeedsFix => "needs fix",
        };
        let name = self
            .path
            .split_once(".rs::")
            .map_or(self.path.as_str(), |(_, q)| q);
        format!("{what} {name} \u{b7} {} \u{b7} {}", self.by, self.date)
    }
}

/// The `limit` most recent review and needs-fix entries of the workspace
/// (module doc). `info` maps `fn:` ids to where the function is now.
pub fn recently_reviewed(
    ws: &Workspace,
    info: &BTreeMap<String, FnInfo>,
    limit: usize,
) -> Vec<RecentEntry> {
    let mut out = Vec::new();
    for m in ws.reviews.values() {
        for e in &m.doc.entries {
            let (what, id, path, by, date, when) = match &e.entry {
                Entry::Review(r) => (
                    RecentWhat::Stamped,
                    r.function_id(),
                    r.path(),
                    r.review.by.clone(),
                    r.review.date.clone(),
                    r.review
                        .signed_at
                        .clone()
                        .unwrap_or_else(|| r.kovan.modified.clone()),
                ),
                Entry::NeedsFix(n) => (
                    RecentWhat::NeedsFix,
                    n.function_id(),
                    n.path(),
                    n.needs_fix.by.clone(),
                    n.needs_fix.date.clone(),
                    n.kovan.modified.clone(),
                ),
                _ => continue,
            };
            out.push(RecentEntry {
                what,
                call_graph_id: info
                    .get(&id)
                    .map(|f| f.call_graph_id.clone())
                    .unwrap_or_default(),
                path: path.unwrap_or(id),
                by,
                date,
                when,
            });
        }
    }
    out.sort_by(|a, b| {
        (b.when.as_str(), b.date.as_str(), a.path.as_str()).cmp(&(
            a.when.as_str(),
            a.date.as_str(),
            b.path.as_str(),
        ))
    });
    out.truncate(limit);
    out
}
