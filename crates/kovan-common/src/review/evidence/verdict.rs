//! What the recorded run says about **one function**, from the tests that
//! reach it (`kovan.toml` `reached_by`, [`crate::call_graph::reach`]) and
//! the `[test_run]` table ([`TestRun`]). Called by the staleness engine
//! (#765).
//!
//! # The rule, in order
//!
//! 1. No test reaches the function → [`ReachVerdict::NoReachingTests`].
//! 2. No recorded run → pending ([`Pending::NoEvidence`]).
//! 3. A `quick` run → pending ([`Pending::QuickSuite`]): only the full suite
//!    counts (#739 decision 12).
//! 4. The run's `Cargo.lock` hash differs from the current one → pending
//!    ([`Pending::CargoLockChanged`]): "pending workspace test" until a full
//!    pass at the new lock (#739, external dependency updates).
//! 5. Any reaching test failed → [`ReachVerdict::Failed`] (the engine makes
//!    the function inherited-stale and blocks re-confirm).
//! 6. At least one reaching test passed → [`ReachVerdict::Passed`], listing
//!    the reaching tests that did not run (`#[ignore]`d, newer than the run,
//!    or edited in the change).
//! 7. Otherwise → [`ReachVerdict::NoneRan`].
//!
//! Tests listed in `edited` never count as passes (#739: tests edited in the
//! change under test are not evidence); their failures still count.
//!
//! **Not decided here:** whether the function's hash is unchanged since the
//! run's commit. That needs git, and is the engine's check: a verdict is
//! only meaningful for a function the engine has found unchanged.

use std::collections::BTreeSet;

use super::super::index::{Suite, TestRun};

/// Why the run cannot speak for the function yet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Pending {
    NoEvidence,
    QuickSuite,
    CargoLockChanged { recorded: String, current: String },
}

/// Rule list in the module doc.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReachVerdict {
    NoReachingTests,
    Pending(Pending),
    Failed {
        failed: Vec<String>,
    },
    Passed {
        passed: Vec<String>,
        not_run: Vec<String>,
    },
    NoneRan {
        not_run: Vec<String>,
    },
}

/// The verdict for a function reached by `reached_by` (module doc).
pub fn reach_verdict(
    reached_by: &[String],
    run: Option<&TestRun>,
    current_cargo_lock: &str,
) -> ReachVerdict {
    if reached_by.is_empty() {
        return ReachVerdict::NoReachingTests;
    }
    let Some(run) = run else {
        return ReachVerdict::Pending(Pending::NoEvidence);
    };
    if run.suite == Suite::Quick {
        return ReachVerdict::Pending(Pending::QuickSuite);
    }
    if run.cargo_lock != current_cargo_lock {
        return ReachVerdict::Pending(Pending::CargoLockChanged {
            recorded: run.cargo_lock.clone(),
            current: current_cargo_lock.to_string(),
        });
    }
    fn set(v: &[String]) -> BTreeSet<&str> {
        v.iter().map(String::as_str).collect()
    }
    let (passed, failed, edited) = (set(&run.passed), set(&run.failed), set(&run.edited));
    let mut reach: Vec<&str> = reached_by.iter().map(String::as_str).collect();
    reach.sort();
    reach.dedup();
    let failing: Vec<String> = reach
        .iter()
        .filter(|t| failed.contains(*t))
        .map(|t| t.to_string())
        .collect();
    if !failing.is_empty() {
        return ReachVerdict::Failed { failed: failing };
    }
    let (ok, not_run): (Vec<&str>, Vec<&str>) = reach
        .iter()
        .partition(|t| passed.contains(*t) && !edited.contains(*t));
    let not_run: Vec<String> = not_run.into_iter().map(str::to_string).collect();
    if ok.is_empty() {
        ReachVerdict::NoneRan { not_run }
    } else {
        ReachVerdict::Passed {
            passed: ok.into_iter().map(str::to_string).collect(),
            not_run,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lock(c: char) -> String {
        format!("sha256:{}", c.to_string().repeat(64))
    }

    fn run() -> TestRun {
        TestRun {
            commit: "0123456789abcdef0123456789abcdef01234567".into(),
            cargo_lock: lock('a'),
            suite: Suite::Full,
            passed: vec!["t.rs::p1".into(), "t.rs::p2".into(), "t.rs::e".into()],
            failed: vec!["t.rs::f".into()],
            edited: vec!["t.rs::e".into()],
        }
    }

    fn v(x: &[&str]) -> Vec<String> {
        x.iter().map(|s| s.to_string()).collect()
    }

    /// Methodology: every rule of the module doc, one case each, against a
    /// run with two passes, one failure and one edited (passing) test.
    ///
    /// Result (2026-10-07): passes.
    #[test]
    fn every_rule_in_order() {
        let r = run();
        let a = lock('a');
        assert_eq!(
            reach_verdict(&[], Some(&r), &a),
            ReachVerdict::NoReachingTests
        );
        assert_eq!(
            reach_verdict(&v(&["t.rs::p1"]), None, &a),
            ReachVerdict::Pending(Pending::NoEvidence)
        );
        let mut q = r.clone();
        q.suite = Suite::Quick;
        assert_eq!(
            reach_verdict(&v(&["t.rs::p1"]), Some(&q), &a),
            ReachVerdict::Pending(Pending::QuickSuite)
        );
        assert!(matches!(
            reach_verdict(&v(&["t.rs::p1"]), Some(&r), &lock('b')),
            ReachVerdict::Pending(Pending::CargoLockChanged { .. })
        ));
        assert_eq!(
            reach_verdict(&v(&["t.rs::p1", "t.rs::f"]), Some(&r), &a),
            ReachVerdict::Failed {
                failed: v(&["t.rs::f"])
            }
        );
        assert_eq!(
            reach_verdict(
                &v(&["t.rs::p2", "t.rs::p1", "t.rs::new", "t.rs::e"]),
                Some(&r),
                &a
            ),
            ReachVerdict::Passed {
                passed: v(&["t.rs::p1", "t.rs::p2"]),
                not_run: v(&["t.rs::e", "t.rs::new"])
            }
        );
        assert_eq!(
            reach_verdict(&v(&["t.rs::e", "t.rs::ignored"]), Some(&r), &a),
            ReachVerdict::NoneRan {
                not_run: v(&["t.rs::e", "t.rs::ignored"])
            }
        );
    }
}
