//! **Agent justifications** (GitHub #1019): `kovan-cli review justify` and
//! `kovan-cli review justifications`, the file side of
//! [`kovan_common::review::draft::draft_justification`] and
//! [`kovan_common::review::review_md_write::upsert_justification`].
//!
//! Maintainer, 2026-10-22: "agent can list justification as to why
//! something is ok or not"; "agents should use kovan-cli to interact with
//! kovan.toml"; "kovan-cli should only make unsigned reviews". So:
//!
//! - an agent writes, against one function, a verdict (`ok`, `not_ok`,
//!   `unsure`) and the evidence it rests on, as an **unsigned** entry
//!   labelled agent-drafted in the function's folder `review.md`;
//! - the entry records the function's stable `fn:` id, its path, `HEAD`
//!   and the function's code hash there, so a reader can tell whether the
//!   code changed since;
//! - **nothing here signs**: no key, no keystore, no passphrase. The entry
//!   type has no signature field, and the staleness engine ignores the kind
//!   (it never counts as review, alone or as a second sign-off).
//!
//! The function is located as a stamp is ([`super::locate`]): it must be in
//! a `kovan.toml`, its file must have no uncommitted change, and the index
//! hash must be its hash at `HEAD`, so the recorded commit and hash agree.

use std::path::Path;

use kovan_common::review::draft::draft_justification;
use kovan_common::review::review_md::{JustificationEntry, Verdict};
use kovan_common::review::review_md_write::upsert_justification;
use kovan_common::review::signed_at::{date_of, now_local};
use kovan_common::review::types::check_agent_id;

use super::{join, load_workspace, locate, review_md_for, write_text, WrittenEntry, REVIEW_MD};

/// What the agent asked to record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JustifyRequest {
    /// An `fn:` id, or a call-graph id / path, `file.rs::Type::name`.
    pub function: String,
    /// `agent:<name>`.
    pub by: String,
    pub verdict: Verdict,
    /// One item per line; at least one.
    pub evidence: Vec<String>,
}

/// Draft and write an agent justification (module doc). Replaces the same
/// agent's earlier justification of the function (keeping its id), else
/// appends; every other byte of `review.md` is kept.
pub fn justify(root: &Path, req: &JustifyRequest) -> Result<(JustificationEntry, WrittenEntry), String> {
    check_agent_id(&req.by).map_err(|e| e.to_string())?;
    let ws = load_workspace(root)?;
    let (idx, file, f) = locate(root, &ws, &req.function)?;
    let previous = ws.reviews.get(&idx.dir).and_then(|r| {
        r.doc
            .justifications()
            .find(|j| j.function_id() == f.id && j.justification.by == req.by)
            .map(|j| j.kovan.clone())
    });
    let now = now_local();
    let entry = draft_justification(
        f,
        &idx.file_path(file),
        &req.by,
        &date_of(&now).unwrap_or_default(),
        &now,
        &ws.head,
        req.verdict,
        &req.evidence,
        previous.as_ref(),
    )
    .map_err(|e| e.to_string())?;
    let path = review_md_for(root, entry.path())?;
    let old = std::fs::read_to_string(&path).unwrap_or_default();
    let w = upsert_justification(&old, &entry).map_err(|e| e.to_string())?;
    write_text(&path, &w.text)?;
    Ok((
        entry,
        WrittenEntry {
            review_md: path,
            replaced: w.replaced,
        },
    ))
}

/// Every agent justification in the workspace (or those whose `fn:` id or
/// path contains `function`), as text: the function, the agent, the
/// verdict, the date and commit, whether the function's code changed since
/// (its index hash now against the recorded one), and the evidence. Each is
/// labelled agent-drafted and unsigned. Unreadable justifications are
/// listed with the parser's reason.
pub fn render_justifications(root: &Path, function: Option<&str>) -> Result<String, String> {
    let ws = load_workspace(root)?;
    let hashes = ws.hashes();
    let mut out = String::from(
        "Agent justifications (GitHub #1019): agent-drafted and unsigned. \
         They never count as review, alone or as a second sign-off.\n",
    );
    let mut shown = 0usize;
    for (dir, file) in &ws.reviews {
        for j in file.doc.justifications() {
            let id = j.function_id();
            let path = j.path().unwrap_or_default();
            if function.is_some_and(|f| !(id.contains(f) || path.contains(f))) {
                continue;
            }
            shown += 1;
            let b = &j.justification;
            let code = match hashes.get(&id) {
                Some(h) if *h == b.hash => "code unchanged since",
                Some(_) => "code changed since",
                None => "function no longer indexed",
            };
            let commit: String = b.commit.chars().take(10).collect();
            out.push_str(&format!(
                "\n{path} ({id})\n  {} verdict {} on {}, commit {commit}; {code}\n",
                b.by,
                b.verdict.as_str(),
                b.date
            ));
            for e in &b.evidence {
                out.push_str(&format!("    - {e}\n"));
            }
        }
        for u in file
            .doc
            .unreadable
            .iter()
            .filter(|u| u.kind.as_deref() == Some("justification"))
        {
            if function.is_some_and(|f| !u.function.as_deref().unwrap_or("").contains(f)) {
                continue;
            }
            shown += 1;
            out.push_str(&format!(
                "\nunreadable justification in {} line {}: {}\n",
                join(dir, REVIEW_MD),
                u.line,
                u.message
            ));
        }
    }
    if shown == 0 {
        out.push_str("\nNo justification matches.\n");
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::super::tests::{Repo, BY, DIR, LIB};
    use super::super::{draft_needs_fix_for, stamp_states, write_needs_fix};
    use super::*;
    use kovan_common::review::review_md::parse_review_md;
    use kovan_common::review::state::StateKind;

    fn req(qual: &str, verdict: Verdict, evidence: &[&str]) -> JustifyRequest {
        JustifyRequest {
            function: format!("{LIB}::{qual}"),
            by: "agent:claude".into(),
            verdict,
            evidence: evidence.iter().map(|s| s.to_string()).collect(),
        }
    }

    /// Methodology (GitHub #1019): on the stamp layer's throwaway git
    /// fixture, with no key anywhere, justify `twice` (`ok`), then `other`
    /// (`not_ok`), then re-justify `twice`, committing after each. Compare
    /// the review states (`stamp_states`, signatures enforced) before and
    /// after. Then raise a needs-fix on `other` and justify it `ok`. Pass:
    /// the entries land in the folder's `review.md` with the `fn:` id,
    /// `HEAD` and the function's hash; the re-justification replaces the
    /// first; the file holds no signature; the states are identical before
    /// and after (no function becomes reviewed); `other` stays "needs fix";
    /// the listing shows each entry labelled unsigned, "code unchanged
    /// since", then "code changed since" after an edit. A human reviewer id
    /// and an uncommitted edit are refused.
    ///
    /// Result (2026-10-22): passes.
    #[test]
    fn justify_writes_unsigned_entries_that_never_count() {
        let r = Repo::new();
        let before = stamp_states(r.path()).unwrap();
        let (j1, w) = justify(r.path(), &req("twice", Verdict::Ok, &["kovan-cli test: 3 passed"])).unwrap();
        assert!(!w.replaced && w.review_md == r.path().join(DIR).join(REVIEW_MD));
        let head = r.git(&["rev-parse", "HEAD"]).trim().to_string();
        assert_eq!(j1.justification.commit, head);
        r.commit("justify twice");
        justify(r.path(), &req("other", Verdict::NotOk, &["returns 1, doc says one: fine; but no test"])).unwrap();
        r.commit("justify other");
        let (j2, w) = justify(r.path(), &req("twice", Verdict::Unsure, &["re-read", "leaf untested"])).unwrap();
        assert!(w.replaced && j2.kovan.id == j1.kovan.id);
        r.commit("re-justify twice");

        let text = r.read(&format!("{DIR}/{REVIEW_MD}"));
        assert!(!text.contains("signature"), "{text}");
        let doc = parse_review_md(&text);
        assert!(doc.unreadable.is_empty(), "{:?}", doc.unreadable);
        assert_eq!(doc.justifications().count(), 2);
        assert_eq!(stamp_states(r.path()).unwrap(), before, "a justification changed a review state");

        let n = draft_needs_fix_for(r.path(), &format!("{LIB}::other"), BY, "wrong unit").unwrap();
        write_needs_fix(r.path(), &n, "").unwrap();
        r.commit("needs fix");
        justify(r.path(), &req("other", Verdict::Ok, &["looks fine to me"])).unwrap();
        r.commit("justify other again");
        let s = stamp_states(r.path())
            .unwrap()
            .into_iter()
            .find(|s| s.function == format!("{LIB}::other"))
            .expect("other has a needs-fix state");
        assert_eq!(s.state, Some(StateKind::NeedsFixOpen), "{}", s.reason);

        let list = render_justifications(r.path(), Some("twice")).unwrap();
        assert!(list.contains("agent-drafted and unsigned"), "{list}");
        assert!(list.contains("agent:claude verdict unsure") && list.contains("- leaf untested"), "{list}");
        assert!(list.contains("code unchanged since") && !list.contains("other"), "{list}");
        r.edit("leaf(x) + 1.0", "leaf(x) + 2.0");
        let list = render_justifications(r.path(), None).unwrap();
        assert!(list.contains("code changed since") && list.contains("verdict ok"), "{list}");
        let err = justify(r.path(), &req("twice", Verdict::Ok, &["tests pass"])).unwrap_err();
        assert!(err.contains("not committed"), "{err}");
        r.commit("edit twice");

        let mut human = req("twice", Verdict::Ok, &["tests pass"]);
        human.by = BY.into();
        let err = justify(r.path(), &human).unwrap_err();
        assert!(err.contains("agent:<name>"), "{err}");
        assert!(render_justifications(r.path(), Some("nothing-like-this")).unwrap().contains("No justification matches"));
    }

    /// Methodology (GitHub #1019): a hand-broken justification (a human
    /// `by`) in `review.md`. Pass: the listing names it as unreadable with
    /// the parser's reason, and the review states still build.
    ///
    /// Result (2026-10-22): passes.
    #[test]
    fn listing_shows_unreadable_justifications() {
        let r = Repo::new();
        justify(r.path(), &req("twice", Verdict::Ok, &["tests pass"])).unwrap();
        let p = format!("{DIR}/{REVIEW_MD}");
        r.write(&p, &r.read(&p).replace("by = \"agent:claude\"", "by = \"github:someone\""));
        r.commit("break it");
        let list = render_justifications(r.path(), None).unwrap();
        assert!(list.contains("unreadable justification in crates/demo/src/review.md"), "{list}");
        assert!(stamp_states(r.path()).is_ok());
    }
}
