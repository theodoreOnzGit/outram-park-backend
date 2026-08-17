//! `kovan historian` — the pre-merge-to-`main` accounting report.
//!
//! A thin frontend over [`kovan_metrics::historian`]. Generates the markdown
//! report that accompanies a `develop` → `main` merge: tokens spent and
//! lines/KLOC written across the released window, with a per-crate breakdown
//! and a per-commit ledger.
//!
//! With no `--from`, the window is everything on `--branch` not yet on
//! `--base` — exactly what the pending merge would deliver.

use std::path::PathBuf;

use kovan_metrics::{date::Date, historian};

/// Generate a historian report.
pub fn run(
    from: Option<String>,
    to: Option<String>,
    branch: String,
    base: String,
    outfile: Option<PathBuf>,
) -> Result<(), String> {
    let parse = |s: Option<String>| -> Result<Option<Date>, String> {
        match s {
            Some(v) => Date::parse_ddmmyy(&v).map(Some).map_err(|e| e.to_string()),
            None => Ok(None),
        }
    };
    historian::generate(parse(from)?, parse(to)?, &branch, &base, outfile)?;
    Ok(())
}
