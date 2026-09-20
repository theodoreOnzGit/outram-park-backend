//! Capture build provenance so generated nuclear data can carry it.
//!
//! An ACE table written by this crate is handed to other codes, copied onto
//! other machines, and read months later. The ACE `hk` comment field is the one
//! place provenance travels *with* the data instead of beside it, so the values
//! captured here end up written into every table this crate produces
//! (`acer::build`, `acer::thermal`).
//!
//! Both are best-effort and degrade to `"unknown"` rather than failing the
//! build: a crate published to crates.io has no `.git`, and `git` may not be
//! installed at all. A table stamped `unknown` is honest; a build that refuses
//! to compile outside a git checkout would be a defect.

use std::process::Command;

fn main() {
    // ── Short commit, when this is a git checkout ────────────────────────────
    let sha = Command::new("git")
        .args(["rev-parse", "--short=7", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".to_string());

    // Mark the tree dirty, so a stamp can never claim a clean commit built
    // something that had uncommitted edits in it.
    let dirty = Command::new("git")
        .args(["status", "--porcelain", "--untracked-files=no"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| !o.stdout.is_empty())
        .unwrap_or(false);
    let sha = if dirty && sha != "unknown" {
        format!("{sha}+")
    } else {
        sha
    };

    println!("cargo:rustc-env=NJOY_OP_GIT_SHA={sha}");
    println!("cargo:rustc-env=NJOY_OP_BUILD_DATE={}", utc_date());

    // Re-run when the checked-out commit moves, and no more often than that.
    for p in ["../../.git/HEAD", "../../.git/index"] {
        if std::path::Path::new(p).exists() {
            println!("cargo:rerun-if-changed={p}");
        }
    }
}

/// Today's UTC date as `YYYY-MM-DD`, with no dependency on `chrono` or on a
/// `date` binary — this crate is built for Android and wasm, where neither is
/// reliably present.
///
/// Days-to-civil is Howard Hinnant's algorithm (public domain), valid for any
/// date this will ever see.
fn utc_date() -> String {
    let secs = match std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
    {
        Ok(d) => d.as_secs() as i64,
        Err(_) => return "unknown".to_string(),
    };
    let z = secs.div_euclid(86_400) + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02}")
}
