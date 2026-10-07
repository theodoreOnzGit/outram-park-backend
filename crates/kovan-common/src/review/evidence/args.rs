//! Build the `cargo test` command from the user's extra arguments, and
//! decide whether it is still the **full** suite.
//!
//! The base command is always
//!
//! ```text
//! cargo test --workspace --lib --tests --release --no-fail-fast \
//!     --message-format=json-render-diagnostics
//! ```
//!
//! `--no-fail-fast` is added so one failing binary does not stop the rest
//! of a 2.5-hour run (the results of every binary are evidence);
//! `--message-format=json-render-diagnostics` only changes how cargo reports
//! the *build* (diagnostics stay human-readable on stderr) and gives the
//! exact package and source file of every test binary ([`super::output`]).
//! `--workspace` is left out when a package is selected (`-p`), which makes
//! the run partial anyway.
//!
//! # Result-neutral extras (the run stays full)
//!
//! cargo: `-j N`/`--jobs N`, `--release`, `--lib`, `--tests`, `--workspace`,
//! `--all`, `--no-fail-fast`, `--offline`, `--locked`, `--frozen`,
//! `-v`/`-vv`/`--verbose`, `--color X`, `--target-dir X`.
//! After `--` (libtest): `--test-threads N`, `--show-output`,
//! `--include-ignored` (runs more, never fewer).
//!
//! # Refused (the output could not be parsed)
//!
//! `-q`/`--quiet` (terse libtest output), `--message-format`, and after
//! `--`: `--format`, `-q`/`--quiet`, `-Z`, `--list`, `--logfile`,
//! `--nocapture`/`--no-capture` (test output interleaves with result lines).
//!
//! # Everything else makes the run partial
//!
//! `-p`, `--exclude`, `--features`, `--no-default-features` (switches the
//! long tests off), `--test X`, test-name filters, `--skip`, `--exact`,
//! `--ignored`, …: recorded as `partial_reasons`, never counted. The
//! classification is deliberately conservative: an unknown argument is
//! partial, not neutral.

/// The planned command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    /// The arguments after `cargo`.
    pub args: Vec<String>,
    /// Why the run is not the full suite; empty when it is.
    pub partial_reasons: Vec<String>,
}

/// Why the extra arguments cannot be run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refused(pub String);

impl std::fmt::Display for Refused {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "refusing {}: kovan-cli test must parse libtest's default output",
            self.0
        )
    }
}

impl std::error::Error for Refused {}

/// The base arguments (module doc), without `--workspace`.
const BASE: [&str; 6] = [
    "test",
    "--lib",
    "--tests",
    "--release",
    "--no-fail-fast",
    "--message-format=json-render-diagnostics",
];

const CARGO_NEUTRAL_FLAGS: [&str; 11] = [
    "--release",
    "--lib",
    "--tests",
    "--workspace",
    "--all",
    "--no-fail-fast",
    "--offline",
    "--locked",
    "--frozen",
    "--verbose",
    "-v",
];
const CARGO_NEUTRAL_VALUED: [&str; 4] = ["-j", "--jobs", "--color", "--target-dir"];
/// cargo options that take a value (kept with it in a partial reason).
const CARGO_VALUED: [&str; 13] = [
    "-p",
    "--package",
    "--exclude",
    "-F",
    "--features",
    "--test",
    "--bin",
    "--example",
    "--bench",
    "--target",
    "--manifest-path",
    "--config",
    "--profile",
];
const HARNESS_REFUSED: [&str; 9] = [
    "--format",
    "-q",
    "--quiet",
    "-Z",
    "--list",
    "--logfile",
    "--nocapture",
    "--no-capture",
    "--report-time",
];

/// Plan `cargo test` with the user's `extra` arguments (module doc).
pub fn plan(extra: &[String]) -> Result<Plan, Refused> {
    let (cargo_side, harness_side) = match extra.iter().position(|a| a == "--") {
        Some(i) => (&extra[..i], &extra[i + 1..]),
        None => (extra, &extra[..0]),
    };
    let mut args: Vec<String> = BASE.iter().map(|s| s.to_string()).collect();
    let mut reasons = Vec::new();
    let mut user = Vec::new();
    let mut selects_package = false;
    let mut i = 0;
    while i < cargo_side.len() {
        let a = cargo_side[i].as_str();
        let (flag, inline) = match a.split_once('=') {
            Some((f, v)) if f.starts_with('-') => (f, Some(v)),
            _ => (a, None),
        };
        if a == "-q" || a == "--quiet" || flag == "--message-format" {
            return Err(Refused(a.to_string()));
        }
        if CARGO_NEUTRAL_FLAGS.contains(&a) || a == "-vv" {
            if !args.iter().any(|x| x == a) && a != "--workspace" && a != "--all" {
                user.push(a.to_string());
            }
            i += 1;
            continue;
        }
        // `-j8` as well as `-j 8` / `--jobs=8`.
        let jn = a
            .strip_prefix("-j")
            .filter(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()));
        if jn.is_some() {
            user.push(a.to_string());
            i += 1;
            continue;
        }
        if CARGO_NEUTRAL_VALUED.contains(&flag) {
            user.push(a.to_string());
            if inline.is_none() {
                if let Some(v) = cargo_side.get(i + 1) {
                    user.push(v.clone());
                }
                i += 1;
            }
            i += 1;
            continue;
        }
        // `-p x`, `--package=x`, `-px`, `--manifest-path …`.
        if matches!(flag, "-p" | "--package" | "--manifest-path")
            || (a.starts_with("-p") && !a.starts_with("--"))
        {
            selects_package = true;
        }
        let mut reason = a.to_string();
        user.push(a.to_string());
        if CARGO_VALUED.contains(&flag) && inline.is_none() {
            if let Some(v) = cargo_side.get(i + 1) {
                reason = format!("{a} {v}");
                user.push(v.clone());
            }
            i += 1;
        }
        reasons.push(reason);
        i += 1;
    }
    if !selects_package {
        args.insert(1, "--workspace".to_string());
    }
    args.extend(user);
    if !harness_side.is_empty() {
        args.push("--".to_string());
        let mut j = 0;
        while j < harness_side.len() {
            let a = harness_side[j].as_str();
            let flag = a.split_once('=').map_or(a, |(f, _)| f);
            if HARNESS_REFUSED.contains(&flag) {
                return Err(Refused(format!("-- {a}")));
            }
            args.push(a.to_string());
            if flag == "--test-threads" {
                if !a.contains('=') {
                    if let Some(v) = harness_side.get(j + 1) {
                        args.push(v.clone());
                    }
                    j += 1;
                }
            } else if a != "--show-output" && a != "--include-ignored" {
                let mut reason = format!("-- {a}");
                if matches!(flag, "--skip") && !a.contains('=') {
                    if let Some(v) = harness_side.get(j + 1) {
                        reason = format!("-- {a} {v}");
                        args.push(v.clone());
                    }
                    j += 1;
                }
                reasons.push(reason);
            }
            j += 1;
        }
    }
    Ok(Plan {
        args,
        partial_reasons: reasons,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }

    /// Methodology: no extras give exactly the canonical command (plus
    /// `--no-fail-fast` and the build-message format) and a full scope;
    /// result-neutral extras keep it full; `-p`, a filter, `--skip` and
    /// `--no-default-features` make it partial and drop `--workspace` only
    /// for `-p`; terse and `--nocapture` output are refused.
    ///
    /// Result (2026-10-07): passes.
    #[test]
    fn full_partial_and_refused_arguments() {
        let p = plan(&[]).unwrap();
        assert_eq!(
            p.args,
            s(&[
                "test",
                "--workspace",
                "--lib",
                "--tests",
                "--release",
                "--no-fail-fast",
                "--message-format=json-render-diagnostics"
            ])
        );
        assert!(p.partial_reasons.is_empty());

        let p = plan(&s(&[
            "-j",
            "1",
            "--release",
            "--",
            "--test-threads",
            "4",
            "--include-ignored",
        ]))
        .unwrap();
        assert!(p.partial_reasons.is_empty(), "{:?}", p.partial_reasons);
        assert_eq!(p.args.iter().filter(|a| *a == "--release").count(), 1);
        assert!(p.args.ends_with(&s(&[
            "-j",
            "1",
            "--",
            "--test-threads",
            "4",
            "--include-ignored"
        ])));
        assert!(plan(&s(&["-j8", "--jobs=2"]))
            .unwrap()
            .partial_reasons
            .is_empty());

        let p = plan(&s(&["-p", "bishan", "-j", "1"])).unwrap();
        assert_eq!(p.partial_reasons, s(&["-p bishan"]));
        assert!(!p.args.contains(&"--workspace".to_string()));
        assert!(p.args.ends_with(&s(&["-p", "bishan", "-j", "1"])));

        let p = plan(&s(&[
            "--no-default-features",
            "--",
            "flash",
            "--skip",
            "slow",
        ]))
        .unwrap();
        assert_eq!(
            p.partial_reasons,
            s(&["--no-default-features", "-- flash", "-- --skip slow"])
        );
        assert!(p.args.contains(&"--workspace".to_string()));

        assert!(plan(&s(&["-q"])).is_err());
        assert!(plan(&s(&["--message-format", "json"])).is_err());
        assert!(plan(&s(&["--", "--nocapture"])).is_err());
        assert!(plan(&s(&["--", "--format=json"])).is_err());
    }
}
