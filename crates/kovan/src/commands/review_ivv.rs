//! `kovan-cli review ivv [<function>]` (GitHub #810): read-only report of
//! rung 5, independent V&V, per function, from `review.md` judged by the
//! staleness engine with signatures enforced (the same
//! [`crate::stamping::evaluate_workspace`] desktop kovan and kovan-web
//! use).
//!
//! It prints, for every function with a valid review (or only those whose
//! `fn:` id, call-graph id or `file.rs::qual` path contains `<function>`):
//! the headline (passed, not reached, or "independent V&V not counted"),
//! every review with its plain-English reasons, each audit record labelled
//! "audit record (not verified by kovan)", then the `kovan_root.toml`
//! records that do not verify and the need-you queue's rung-5 rows.
//!
//! **Read-only.** It never signs and never writes: `kovan-cli` has no
//! signing command (AI agents never stamp; signing is desktop kovan's,
//! with the human's passphrase). Offline: the audit records are never
//! fetched.

use std::path::Path;

use kovan_common::review::ivv_text::record_warning;
use kovan_common::review::ivv_view::{ivv_lines, ivv_queue, summarise};

use crate::stamping::evaluate_workspace;

/// The report as text (module doc). `Err` when the workspace cannot be
/// read (an unparsable `kovan_root.toml`).
pub fn render(root: &Path, function: Option<&str>) -> Result<String, String> {
    let we = evaluate_workspace(root)?;
    let ev = &we.evaluation;
    let mut out = String::from(
        "Rung 5, independent V&V (IV&V), judged from review.md with signatures enforced.\n\
         Concept areas are not resolved by kovan yet, so no function reaches rung 5 here; \
         the reason \"the function has no known concept area\" says so on each review.\n",
    );
    let mut shown = 0usize;
    for (id, fr) in &ev.functions {
        let cg = we.call_graph_ids.get(id).cloned().unwrap_or_default();
        let path = fr
            .location
            .as_ref()
            .map(|l| format!("{}::{}", l.file, l.qual))
            .unwrap_or_default();
        if let Some(f) = function {
            if !(id.contains(f) || cg.contains(f) || path.contains(f)) {
                continue;
            }
        }
        let Some(s) = summarise(fr, &we.review_root, &ev.ivv_warnings) else {
            continue;
        };
        shown += 1;
        out.push_str(&format!("\n{path} ({id})\n  {}\n", s.headline()));
        for l in ivv_lines(&s) {
            match &l.link {
                Some(u) => out.push_str(&format!("  {} {u}\n", l.text)),
                None => out.push_str(&format!("  {}\n", l.text)),
            }
        }
    }
    if shown == 0 {
        out.push_str(match function {
            Some(_) => "\nNo reviewed function matches.\n",
            None => "\nNo function has a valid review.\n",
        });
    }
    if function.is_none() {
        if !ev.ivv_warnings.is_empty() {
            out.push_str(
                "\nkovan_root.toml records that do not verify (they count for nothing):\n",
            );
            for w in &ev.ivv_warnings {
                out.push_str(&format!("  - {}\n", record_warning(w)));
            }
        }
        let q = ivv_queue(ev);
        if !q.is_empty() {
            out.push_str("\nNeeds a person (rung 5):\n");
            for r in q {
                let what = r
                    .path
                    .or(r.function)
                    .unwrap_or_else(|| "kovan_root.toml".into());
                out.push_str(&format!(
                    "  - {what}: {}\n    next: {}\n",
                    r.reason, r.action
                ));
            }
        }
    }
    Ok(out)
}

/// Print [`render`]. CLI glue (exempt from the test rule): `render` is
/// tested.
pub fn run(root: &Path, function: Option<&str>) -> Result<(), String> {
    print!("{}", render(root, function)?);
    Ok(())
}
