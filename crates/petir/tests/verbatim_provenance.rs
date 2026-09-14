// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Theodore Ong and the outram-park contributors.

//! Mechanically checks that PETIR's **verbatim lifts** still match their
//! sources.
//!
//! # Why this test exists
//!
//! Several PETIR modules are byte-for-byte copies of code that already lives —
//! and is already tested — elsewhere in this workspace. Each carries a
//! PROVENANCE block promising that the only differences are a short, listed set
//! of mechanical `no_std` edits. That promise is the entire value of the lift:
//! it is what lets a reviewer diff the two files, and what makes "a defect here
//! is a defect there too" a true statement rather than a hope.
//!
//! A promise in a comment decays. Someone fixes a bug in PETIR's copy and not
//! in the original, or vice versa, and nothing complains. This test is what
//! makes the promise enforceable: it re-applies the documented substitutions to
//! the source and compares the substantive lines. If the two have drifted, it
//! fails and names the line.
//!
//! # What "substantive" means here
//!
//! Comments, `use` declarations, attributes and blank lines are stripped before
//! comparison. Those are exactly what the lift is *allowed* to change — the
//! PROVENANCE block itself is a comment, and the `alloc` / `Real` imports are
//! `use` lines. Everything else must survive the substitutions unchanged.
//!
//! # Deviations are declared, not tolerated silently
//!
//! Two files depart from their source in a way no substitution captures:
//! `specfunc/inc_gamma.rs` and `specfunc/inv_inc_gamma.rs` replace an
//! `extern "C"` FFI block with calls to the pure-Rust `libm` crate. Those are
//! listed below with a count, so the deviation is pinned rather than waved
//! through: if the number of differing lines changes, this test fails and
//! someone has to say why.

use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

/// A lifted file and the source it was taken from.
struct Lift {
    /// Path under `crates/petir/src/`.
    lifted: &'static str,
    /// Path relative to the workspace `crates/` directory.
    source: &'static str,
    /// How many substantive lines are allowed to differ, and why.
    ///
    /// Zero for a true verbatim lift. Non-zero only where the file's own
    /// PROVENANCE block documents a substantive deviation.
    allowed_deviations: usize,
    /// The reason, for the failure message.
    reason: &'static str,
}

const LIFTS: &[Lift] = &[
    // --- from outram-foam-basic-lib -------------------------------------
    Lift {
        lifted: "linalg/square_matrix.rs",
        source: "outram-foam-basic-lib/src/matrix/square_matrix.rs",
        allowed_deviations: 0,
        reason: "true verbatim lift",
    },
    Lift {
        lifted: "poly/roots.rs",
        source: "outram-foam-basic-lib/src/polynomial/roots.rs",
        allowed_deviations: 0,
        reason: "true verbatim lift",
    },
    Lift {
        lifted: "poly/linear_eqn.rs",
        source: "outram-foam-basic-lib/src/polynomial/linear_eqn.rs",
        allowed_deviations: 0,
        reason: "true verbatim lift",
    },
    Lift {
        lifted: "poly/quadratic_eqn.rs",
        source: "outram-foam-basic-lib/src/polynomial/quadratic_eqn.rs",
        allowed_deviations: 0,
        reason: "true verbatim lift",
    },
    Lift {
        lifted: "poly/cubic_eqn.rs",
        source: "outram-foam-basic-lib/src/polynomial/cubic_eqn.rs",
        allowed_deviations: 0,
        reason: "true verbatim lift",
    },
    Lift {
        lifted: "poly/polynomial.rs",
        source: "outram-foam-basic-lib/src/polynomial/polynomial.rs",
        allowed_deviations: 0,
        reason: "true verbatim lift",
    },
    Lift {
        lifted: "specfunc/erf_inv.rs",
        source: "outram-foam-basic-lib/src/math/erf_inv.rs",
        allowed_deviations: 0,
        reason: "true verbatim lift",
    },
    Lift {
        lifted: "specfunc/inc_gamma.rs",
        source: "outram-foam-basic-lib/src/math/inc_gamma.rs",
        allowed_deviations: 7,
        reason: "extern \"C\" erf/erfc/tgamma FFI replaced by the pure-Rust libm crate \
                 the 7 lines being the `extern \"C\" {` opener, 3 `fn` declarations and 3 `unsafe` bodies; see the file DEVIATION note",
    },
    Lift {
        lifted: "specfunc/inv_inc_gamma.rs",
        source: "outram-foam-basic-lib/src/math/inv_inc_gamma.rs",
        allowed_deviations: 5,
        reason: "extern \"C\" tgamma/lgamma FFI replaced by the pure-Rust libm crate \
                 the 5 lines being the `extern \"C\" {` opener, 2 `fn` declarations and 2 `unsafe` bodies; see the file DEVIATION note",
    },
    // --- from chem-eng (itself a GNU Octave control-package port) --------
    Lift {
        lifted: "transfer_fn/cplx.rs",
        source: "chem-eng-real-time-process-control-simulator/src/lib/beta_testing/z_domain/cplx.rs",
        allowed_deviations: 0,
        reason: "true verbatim lift",
    },
    Lift {
        lifted: "transfer_fn/polynomial.rs",
        source: "chem-eng-real-time-process-control-simulator/src/lib/beta_testing/z_domain/polynomial.rs",
        allowed_deviations: 0,
        reason: "true verbatim lift",
    },
    Lift {
        lifted: "transfer_fn/continuous_tf.rs",
        source: "chem-eng-real-time-process-control-simulator/src/lib/beta_testing/z_domain/continuous_tf.rs",
        allowed_deviations: 0,
        reason: "true verbatim lift",
    },
    Lift {
        lifted: "transfer_fn/discrete_tf.rs",
        source: "chem-eng-real-time-process-control-simulator/src/lib/beta_testing/z_domain/discrete_tf.rs",
        allowed_deviations: 0,
        reason: "true verbatim lift",
    },
    Lift {
        lifted: "transfer_fn/conversion.rs",
        source: "chem-eng-real-time-process-control-simulator/src/lib/beta_testing/z_domain/conversion.rs",
        allowed_deviations: 0,
        reason: "true verbatim lift",
    },
    Lift {
        lifted: "transfer_fn/first_order_transfer_fn.rs",
        source: "chem-eng-real-time-process-control-simulator/src/lib/beta_testing/stable_transfer_functions/first_order_transfer_fn.rs",
        allowed_deviations: 0,
        reason: "true verbatim lift",
    },
    Lift {
        lifted: "transfer_fn/second_order_transfer_fn.rs",
        source: "chem-eng-real-time-process-control-simulator/src/lib/beta_testing/stable_transfer_functions/second_order_transfer_fn.rs",
        allowed_deviations: 0,
        reason: "true verbatim lift",
    },
    Lift {
        lifted: "transfer_fn/first_order_transfer_fn_with_zeroes.rs",
        source: "chem-eng-real-time-process-control-simulator/src/lib/beta_testing/stable_transfer_functions/first_order_transfer_fn_with_zeroes.rs",
        allowed_deviations: 0,
        reason: "true verbatim lift",
    },
    Lift {
        lifted: "transfer_fn/decaying_sinusoid.rs",
        source: "chem-eng-real-time-process-control-simulator/src/lib/beta_testing/stable_transfer_functions/decaying_sinusoid.rs",
        allowed_deviations: 0,
        reason: "true verbatim lift",
    },
    Lift {
        lifted: "transfer_fn/step_fn.rs",
        source: "chem-eng-real-time-process-control-simulator/src/lib/beta_testing/stable_transfer_functions/step_fn.rs",
        allowed_deviations: 0,
        reason: "true verbatim lift",
    },
];

/// The mechanical `no_std` substitutions, applied to the SOURCE before
/// comparison. This list must stay in step with `scripts`' lift transform and
/// with what each PROVENANCE block claims.
fn apply_substitutions(line: &str) -> String {
    let mut s = line.to_string();
    for (from, to) in [
        ("std::ops::", "core::ops::"),
        ("std::fmt::", "core::fmt::"),
        ("std::cmp::", "core::cmp::"),
        ("std::f64::consts", "core::f64::consts"),
        ("std::error::Error", "core::error::Error"),
        ("std::collections::VecDeque", "alloc::collections::VecDeque"),
        ("crate::primitives::scalar", "crate::scalar"),
        ("crate::math::erf_inv", "crate::specfunc::erf_inv"),
        ("crate::math::inc_gamma", "crate::specfunc::inc_gamma"),
        ("crate::math::inv_inc_gamma", "crate::specfunc::inv_inc_gamma"),
        (
            "crate::beta_testing::errors::ChemEngProcessControlSimulatorError",
            "crate::transfer_fn::ChemEngProcessControlSimulatorError",
        ),
        (
            "crate::beta_testing::stable_transfer_functions::",
            "crate::transfer_fn::",
        ),
        ("crate::beta_testing::z_domain::", "crate::transfer_fn::"),
        ("crate::polynomial::", "crate::poly::"),
    ] {
        s = s.replace(from, to);
    }
    s
}

/// Strip everything the lift is permitted to change: comments, `use`
/// declarations, attributes and blank lines.
///
/// Attributes go because the lift adds `#[allow(unused_imports)]` and
/// `#[cfg(test)]` around the imports it inserts. Doc comments go because the
/// PROVENANCE block is one.
fn substantive_lines(src: &str) -> Vec<String> {
    src.lines()
        .map(|l| l.trim())
        .filter(|l| {
            !l.is_empty()
                && !l.starts_with("//")
                && !l.starts_with("use ")
                && !l.starts_with("pub use ")
                && !l.starts_with("#[")
                && !l.starts_with("#![")
                && !l.starts_with("extern crate")
        })
        .map(|l| l.to_string())
        .collect()
}

fn crates_dir() -> PathBuf {
    // CARGO_MANIFEST_DIR is <workspace>/crates/petir
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("petir must live under crates/")
        .to_path_buf()
}

#[test]
fn every_lifted_file_still_matches_its_source() {
    let crates = crates_dir();
    let petir_src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");

    let mut failures: Vec<String> = Vec::new();

    for lift in LIFTS {
        let lifted_path = petir_src.join(lift.lifted);
        let source_path = crates.join(lift.source);

        let lifted_text = match fs::read_to_string(&lifted_path) {
            Ok(t) => t,
            Err(e) => {
                failures.push(format!("cannot read lifted {}: {e}", lift.lifted));
                continue;
            }
        };
        let source_text = match fs::read_to_string(&source_path) {
            Ok(t) => t,
            Err(e) => {
                // A moved or deleted source is itself a provenance failure: the
                // lift's PROVENANCE block now points at nothing.
                failures.push(format!(
                    "cannot read source {} for {}: {e} \
                     (if the source moved, update the PROVENANCE block AND this test)",
                    lift.source, lift.lifted
                ));
                continue;
            }
        };

        let lifted_lines = substantive_lines(&lifted_text);
        let source_lines: Vec<String> = substantive_lines(&source_text)
            .iter()
            .map(|l| apply_substitutions(l))
            .collect();

        // Count lines present in one but not the other, in either direction.
        let lifted_set: HashSet<&String> = lifted_lines.iter().collect();
        let source_set: HashSet<&String> = source_lines.iter().collect();
        let only_in_lifted: Vec<&&String> = lifted_set.difference(&source_set).collect();
        let only_in_source: Vec<&&String> = source_set.difference(&lifted_set).collect();

        let deviation = only_in_lifted.len().max(only_in_source.len());

        if deviation > lift.allowed_deviations {
            failures.push(format!(
                "\n{} has drifted from {}\n  \
                 allowed deviations: {} ({})\n  \
                 actual deviation:   {}\n  \
                 only in PETIR ({}): {:?}\n  \
                 only in source ({}): {:?}",
                lift.lifted,
                lift.source,
                lift.allowed_deviations,
                lift.reason,
                deviation,
                only_in_lifted.len(),
                only_in_lifted.iter().take(6).collect::<Vec<_>>(),
                only_in_source.len(),
                only_in_source.iter().take(6).collect::<Vec<_>>(),
            ));
        }
    }

    assert!(
        failures.is_empty(),
        "verbatim lifts have drifted from their sources.\n\
         A lift is only worth having if it still diffs clean against its origin: \
         fix BOTH copies, or -- if the divergence is deliberate -- record it in the \
         file's PROVENANCE block and raise `allowed_deviations` here with a reason.\n{}",
        failures.join("\n")
    );
}

/// The lift list must not silently shrink.
///
/// Deleting an entry here is the easy way to make the test above pass, so the
/// count is pinned: removing a lift requires editing this number, which is a
/// visible, reviewable act.
#[test]
fn the_lift_inventory_is_complete() {
    assert_eq!(
        LIFTS.len(),
        19,
        "the number of verbatim lifts changed; if that is intended, update this count"
    );
}
