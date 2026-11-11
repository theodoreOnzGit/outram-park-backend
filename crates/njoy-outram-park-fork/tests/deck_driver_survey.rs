// SPDX-License-Identifier: AGPL-3.0-only
//! **Deck-driver survey** (gh:#1369, differential #1366): every upstream
//! `tests/NN/input` deck that is present, run through the NJOY2016 binary and
//! through [`run_deck`], every output tape compared, one status row per deck
//! written to `verification_and_validation/deck_driver/survey.md`.
//!
//! Methodology. The test directory is rebuilt as upstream's `CMakeLists.txt`
//! does (`configure_file` lines copy `tests/resources/*` to `tapeNN`), the
//! unmodified deck is run by `build/njoy` (300 s limit) and by `run_deck` in a
//! second directory. Each tape file NJOY left is compared with ours: text tapes
//! line by line, anything NJOY wrote in blocked-binary form by bytes (this crate
//! writes no blocked-binary file). Status per deck: `identical` (every NJOY tape
//! present and equal, deck ran to its end), `differs` (tape, differing-line
//! count), or `NotPorted` (module or card named by `DeckRun::blocked`; tapes ours
//! did produce are still compared and listed). A panic in the port is reported
//! as `PANIC`. Pass criterion: none; this is a map, not a gate. `#[ignore]`d
//! (long); run with `NJOY2016_ROOT=... cargo test --release -p
//! njoy-outram-park-fork --test deck_driver_survey -- --ignored`.
//! Results: see `survey.md` (dated, with the toolchain).

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use njoy_outram_park_fork::deck::{run_deck, DeckOptions};

fn njoy_root() -> PathBuf {
    std::env::var("NJOY2016_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("upstream_source/NJOY2016"))
}

/// `(source, destination)` of the `configure_file` lines of a test's CMakeLists.
fn configure_files(cmake: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut rest = cmake;
    while let Some(i) = rest.find("configure_file(") {
        rest = &rest[i + 15..];
        let end = rest.find(')').unwrap_or(rest.len());
        let q: Vec<&str> = rest[..end].split('"').skip(1).step_by(2).collect();
        if q.len() >= 2 {
            out.push((q[0].to_string(), q[1].to_string()));
        }
    }
    out
}

fn stage(root: &Path, case: &str, dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let t = root.join("tests").join(case);
    let cm = std::fs::read_to_string(t.join("CMakeLists.txt")).map_err(|e| e.to_string())?;
    for (src, dst) in configure_files(&cm) {
        let dname = dst.rsplit('/').next().unwrap();
        if dname == "input" || dname.starts_with("reference") {
            continue;
        }
        let s = if let Some(r) = src.strip_prefix("${RESOURCES}/") {
            root.join("tests/resources").join(r)
        } else if let Some(r) = src.strip_prefix("${CMAKE_CURRENT_SOURCE_DIR}/") {
            t.join(r)
        } else {
            return Err(format!("unknown source {src}"));
        };
        std::fs::copy(&s, dir.join(dname)).map_err(|e| format!("{}: {e}", s.display()))?;
    }
    std::fs::copy(t.join("input"), dir.join("deck")).map_err(|e| e.to_string())?;
    Ok(())
}

fn is_text(b: &[u8]) -> bool {
    !b.is_empty() && b.iter().take(4096).all(|&c| c == b'\n' || c == b'\r' || c == b'\t' || (32..127).contains(&c))
}

fn tapes(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .filter(|n| n.starts_with("tape"))
        .collect();
    v.sort();
    v
}

fn survey_case(root: &Path, case: &str) -> String {
    let base = std::env::temp_dir().join(format!("njoy_survey_{case}_{}", std::process::id()));
    let (ours, theirs) = (base.join("ours"), base.join("njoy"));
    for d in [&ours, &theirs] {
        if let Err(e) = stage(root, case, d) {
            return format!("skip ({e})");
        }
    }
    let st = Command::new("sh")
        .arg("-c")
        .arg(format!("timeout 300 {} < deck > log 2>&1", root.join("build/njoy").display()))
        .current_dir(&theirs)
        .status();
    if !st.map(|s| s.success()).unwrap_or(false) {
        return "NJOY binary failed or timed out".into();
    }
    let text = std::fs::read_to_string(ours.join("deck")).unwrap();
    let o2 = ours.clone();
    let res = std::panic::catch_unwind(move || run_deck(&text, &DeckOptions { workdir: Some(o2), parse_only: false }));
    let (blocked, err) = match res {
        Ok(Ok(r)) => (r.blocked, None),
        Ok(Err(e)) => (None, Some(format!("Err: {e}"))),
        Err(_) => (None, Some("PANIC".to_string())),
    };
    let (mut same, mut diff, mut missing) = (0, Vec::new(), Vec::new());
    for t in tapes(&theirs) {
        let b = std::fs::read(theirs.join(&t)).unwrap();
        let Ok(a) = std::fs::read(ours.join(&t)) else { missing.push(t); continue };
        if a == b {
            same += 1;
        } else if is_text(&b) && is_text(&a) {
            let (sa, sb) = (String::from_utf8_lossy(&a), String::from_utf8_lossy(&b));
            let (al, bl): (Vec<_>, Vec<_>) = (sa.lines().collect(), sb.lines().collect());
            let n = (0..al.len().max(bl.len())).filter(|&i| al.get(i) != bl.get(i)).count();
            diff.push(format!("{t} {n}/{} lines", bl.len()));
        } else {
            diff.push(format!("{t} bytes"));
        }
    }
    let mut s = String::new();
    let status = match (&err, &blocked, diff.is_empty() && missing.is_empty()) {
        (Some(e), _, _) => e.clone(),
        (None, None, true) => "identical".to_string(),
        (None, None, false) => "differs".to_string(),
        (None, Some((m, why)), _) => format!("NotPorted ({m}: {why})"),
    };
    let _ = write!(s, "{status} | {same} | {} | {}", diff.join("; "), missing.join(" "));
    let _ = std::fs::remove_dir_all(&base);
    s
}

/// Writes `verification_and_validation/deck_driver/survey.md`.
#[test]
#[ignore = "long: runs every upstream deck in NJOY and here"]
fn survey_every_upstream_deck() {
    let root = njoy_root();
    if !root.join("build/njoy").exists() {
        eprintln!("skip: no NJOY binary under {}", root.display());
        return;
    }
    let mut cases: Vec<String> = std::fs::read_dir(root.join("tests"))
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().join("input").exists())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();
    cases.sort();
    let mut md = String::from(
        "# Deck-driver survey (gh:#1369, #1366)\n\nGenerated by `tests/deck_driver_survey.rs`; method in its module doc.\n\n\
         | deck | status | tapes identical | differing tapes | NJOY tapes we did not write |\n|---|---|---|---|---|\n",
    );
    for c in cases {
        let r = survey_case(&root, &c);
        eprintln!("{c}: {r}");
        let _ = writeln!(md, "| {c} | {} |", r.replace('\n', " "));
    }
    let out = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("verification_and_validation/deck_driver/survey.md");
    std::fs::write(out, md).unwrap();
}
