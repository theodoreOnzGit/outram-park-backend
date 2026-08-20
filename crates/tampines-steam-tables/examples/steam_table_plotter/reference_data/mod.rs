//! Real reference and validation data, copied verbatim out of this crate's own
//! `#[cfg(test)]` fixtures.
//!
//! # The one rule this module exists to enforce
//!
//! **Nothing in here is invented.** Every number is traceable to a named
//! fixture file in this crate and, through it, to a named publication. GitHub
//! issue #26 states the requirement plainly: *"If some datasets are unavailable,
//! the corresponding layer should be disabled rather than filled with invented
//! data"* and *"Do not fabricate validation points."* Each submodule's `//!`
//! header records what its numbers are, where they came from, and how accurate
//! they are; several also record what the numbers are **not** (see
//! [`edwards`], whose GS-1 pressure history is deliberately not treated as a
//! thermodynamic state, and [`marviken`], whose test 24 is explicitly not
//! validated).
//!
//! # Why the data is duplicated here rather than imported
//!
//! The fixtures store their reference values as `let` bindings **inside**
//! `#[test]` functions, in modules that are `#[cfg(test)]`. An example binary
//! links against the crate's *library* build, where those modules do not exist,
//! so there is no import path to them. Reaching them would mean hoisting the
//! data to `pub const`s across roughly thirty-five files — a large, invasive
//! change to files whose diffs against the IAPWS tables are meant to stay
//! reviewable.
//!
//! The duplication is therefore deliberate, and is mitigated three ways:
//!
//! 1. every submodule cites its source fixture by path,
//! 2. the extraction was mechanical, not hand-typed (see the provenance block
//!    in each submodule),
//! 3. [`row_counts_match_fixtures`] pins every row count, so if a fixture grows
//!    or shrinks the mismatch is caught by `cargo test --example
//!    steam_table_plotter` rather than silently drifting.
//!
//! What that test **cannot** catch is a fixture value being *edited in place*.
//! If you change a reference number in a fixture, re-run the extraction.
//!
//! # Curves versus points
//!
//! Everything here is a *point set* — measured, digitised or published data.
//! The **curves** on every diagram (saturation dome, quality lines, isobars,
//! isotherms) are never taken from this module: they are computed live from
//! IAPWS-IF97 in [`crate::curves`], so the plots stay traceable to the
//! implementation under validation. That split is the point of the whole tool.

// `dead_code` is allowed across these modules on purpose. Each one is a
// faithful transcription of a cited dataset, and it carries the context that
// makes the numbers usable — the nozzle bore Marviken was measured through, the
// pipe geometry Edwards-O'Brien used, the name of the `#[test]` function each
// curve came from. Only some of that is plotted today. Deleting the rest to
// silence a lint would strip provenance from a reference record, which is
// exactly what `DATA_POLICY.md` and issue #26 are asking us not to do.
#[allow(dead_code)]
pub mod edwards;
#[allow(dead_code)]
pub mod marviken;
#[allow(dead_code)]
pub mod moody;
#[allow(dead_code)]
pub mod wagner;
#[allow(dead_code)]
pub mod zaloudek;

#[cfg(test)]
/// Row counts of every dataset in this module, pinned against the source
/// fixtures at extraction time (2026-08-20).
///
/// # Methodology
///
/// Each entry is `(dataset label, expected row count)`. The counts were
/// produced by the same mechanical pass that produced the data: strip `//` and
/// `/* */` comments from the fixture file — so that deliberately
/// commented-out rows are **not** harvested — then collect every array or tuple
/// literal of the expected arity inside the fixture's test-function body.
///
/// # Result (measured 2026-08-20)
///
/// 13 Moody isobars totalling 321 points; 21 Zaloudek quality curves totalling
/// 357 points; 2 Marviken envelopes totalling 69 points; 24 Edwards initial
/// node states and 16 GS-1 pressure samples; 220 Wagner saturation rows; 2334
/// Wagner single-phase rows. The comment-stripping step matters: without it the
/// single-phase count comes out at 2386, because 52 rows that the fixtures
/// deliberately comment out (for example the 0 °C row of the 10 bar table,
/// which the `(p,h)` flash cannot reproduce near the triple point) get pulled
/// in as if they were live reference data.
pub const PINNED_ROW_COUNTS: &[(&str, usize)] = &[
    ("moody isobars", 13),
    ("moody points", 321),
    ("zaloudek curves", 21),
    ("zaloudek points", 357),
    ("marviken envelopes", 2),
    ("marviken points", 69),
    ("edwards nodes", 24),
    ("edwards gs1 samples", 16),
    ("wagner saturation rows", 220),
    ("wagner single phase rows", 2334),
];

#[cfg(test)]
/// Looks a pinned count up by label. Panics if the label is unknown, which can
/// only happen if [`PINNED_ROW_COUNTS`] and its callers have drifted apart.
pub fn pinned(label: &str) -> usize {
    PINNED_ROW_COUNTS
        .iter()
        .find(|(name, _)| *name == label)
        .map(|(_, count)| *count)
        .unwrap_or_else(|| panic!("no pinned row count named {label:?}"))
}

/// Verifies that the extracted reference data still has exactly the shape it
/// had when it was extracted.
///
/// # Methodology
///
/// Counts the datasets and their points and compares against
/// [`PINNED_ROW_COUNTS`]. This is a *shape* gate, not a *value* gate: it
/// catches a dataset being truncated, duplicated, or half-pasted. It cannot
/// catch a single number being changed.
///
/// # Result
///
/// Passes as of 2026-08-20 with the counts listed on [`PINNED_ROW_COUNTS`].
#[cfg(test)]
#[test]
fn row_counts_match_fixtures() {
    assert_eq!(moody::MOODY_ISOBARS.len(), pinned("moody isobars"));
    let moody_points: usize = moody::MOODY_ISOBARS.iter().map(|i| i.points.len()).sum();
    assert_eq!(moody_points, pinned("moody points"));

    assert_eq!(zaloudek::ZALOUDEK_CURVES.len(), pinned("zaloudek curves"));
    let zaloudek_points: usize = zaloudek::ZALOUDEK_CURVES
        .iter()
        .map(|c| c.points.len())
        .sum();
    assert_eq!(zaloudek_points, pinned("zaloudek points"));

    assert_eq!(marviken::MARVIKEN_TESTS.len(), pinned("marviken envelopes"));
    let marviken_points: usize = marviken::MARVIKEN_TESTS
        .iter()
        .map(|t| t.points.len())
        .sum();
    assert_eq!(marviken_points, pinned("marviken points"));

    assert_eq!(edwards::EDWARDS_NODE_T_DEGF.len(), pinned("edwards nodes"));
    assert_eq!(
        edwards::EDWARDS_NODE_CENTRE_FT.len(),
        pinned("edwards nodes")
    );
    assert_eq!(
        edwards::EDWARDS_GS1_DATA_PSIA.len(),
        pinned("edwards gs1 samples")
    );

    assert_eq!(
        wagner::WAGNER_SATURATION_TABLE.len(),
        pinned("wagner saturation rows")
    );
    assert_eq!(
        wagner::WAGNER_SINGLE_PHASE_TABLE.len(),
        pinned("wagner single phase rows")
    );
}

/// Sanity-checks that no dataset carries a non-finite number.
///
/// # Methodology
///
/// Sweeps every extracted value and asserts `is_finite()`. A `NaN` or infinity
/// here would mean the mechanical extraction mangled a literal (for example by
/// splitting a number across a line break), which would then propagate silently
/// into a plotted "reference" point.
///
/// # Result
///
/// Passes as of 2026-08-20: no non-finite value in any extracted number.
#[cfg(test)]
#[test]
fn no_dataset_carries_a_non_finite_number() {
    for isobar in moody::MOODY_ISOBARS {
        assert!(isobar.p0_over_p_ref.is_finite());
        for (h, g) in isobar.points {
            assert!(h.is_finite() && g.is_finite(), "moody {}", isobar.test_name);
        }
    }
    for curve in zaloudek::ZALOUDEK_CURVES {
        assert!(curve.throat_quality.is_finite());
        for (p, g, h0) in curve.points {
            assert!(
                p.is_finite() && g.is_finite() && h0.is_finite(),
                "zaloudek {}",
                curve.test_name
            );
        }
    }
    for test in marviken::MARVIKEN_TESTS {
        for (p, g) in test.points {
            assert!(p.is_finite() && g.is_finite(), "marviken {}", test.label);
        }
    }
    for t in edwards::EDWARDS_NODE_T_DEGF {
        assert!(t.is_finite());
    }
    for (t, p) in edwards::EDWARDS_GS1_DATA_PSIA {
        assert!(t.is_finite() && p.is_finite());
    }
    for row in wagner::WAGNER_SATURATION_TABLE {
        assert!(row.iter().all(|v| v.is_finite()));
    }
    for row in wagner::WAGNER_SINGLE_PHASE_TABLE {
        assert!(row.iter().all(|v| v.is_finite()));
    }
}
