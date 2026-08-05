//! Shared machinery for BEDOK's parity and benchmark tests.
//!
//! This module is not a test target of its own — it has no `main.rs` and sits
//! in a subdirectory, so `cargo test` ignores it. Test files pull it in with
//! `mod support;` (flat files) or `#[path = "../support/mod.rs"] mod support;`
//! (directory targets).
//!
//! # What is here
//!
//! - [`compare`] — the field comparator. Given two flat fields it reports the
//!   maximum absolute difference, the maximum relative difference, the L2
//!   norms, **and where the worst point is**. Locating the divergence is the
//!   whole value of the thing; a bare pass/fail says a solve is wrong without
//!   giving anyone a place to start.
//! - [`tolerance`] — every tolerance the suite uses, named and documented in
//!   one place. Tolerances are an open decision (`docs/bedok-port-scoping.md`
//!   §8), so they are collected here to be argued about and changed in one
//!   edit rather than hunted for across test files.
//! - [`skip_unless_full_fixtures`] — the graceful skip for tests that need the
//!   uncommitted node-level fixtures.
//!
//! Some items here are used by only one of the test targets; each target
//! compiles the module separately, so `#[allow(dead_code)]` is applied at
//! module level rather than pretending otherwise.

#![allow(dead_code)]

use bedok::reference::fixtures;
use bedok::reference::Grid;

// ---------------------------------------------------------------------------
// Tolerances
// ---------------------------------------------------------------------------

/// Every tolerance the parity and benchmark suites use.
///
/// # Why they are in one module
///
/// `docs/bedok-port-scoping.md` §8 records parity tolerances as an **open
/// decision** — what counts as "reached parity" has not been settled. Putting
/// them here makes the decision visible and revisable in a single edit, and
/// makes it impossible for a test to smuggle in a looser number by writing a
/// literal inline.
///
/// # The three tiers, and the reasoning behind each
///
/// The tiers differ because the comparisons mean different things.
///
/// **1. Translation tier** — the Rust reference against Yan Ren's captured
/// fixtures. Same algorithm, same iteration order, so the only sources of
/// difference are floating-point accumulation and the number of iterations
/// taken. The binding limit is not machine epsilon but *the reference's own
/// convergence*: the capture stopped at a fission-source residual of
/// 9.611e-07 and a `k_eff` residual of 9.272e-10, so the fixtures themselves
/// only pin the answer to about that level. A tolerance tighter than the
/// reference's own convergence criterion would be measuring noise.
///
/// **2. Substitution tier** — stage 2 against stage 1. Deliberately looser,
/// because the substitutions change the numerics on purpose: an iterative
/// linear solve does not reproduce a direct factorisation bit for bit, and the
/// scoping document flags that row as the one most likely to move results
/// while being entirely correct. These are set *physically* — a difference
/// small enough that no engineering conclusion changes — rather than at
/// machine epsilon.
///
/// **3. Benchmark tier** — either path against the published IAEA-3D reference
/// values. This is the loosest, and correctly so: §4 sets the bar at "rough
/// parity with the V&V cases, at the accuracy a nodal-diffusion code can be
/// expected to reach". A coarse-mesh nodal method is not trying to reproduce a
/// fine-mesh reference exactly.
///
/// # Status
///
/// **None of these numbers has been validated by a passing comparison.** They
/// are the starting proposals, chosen from the reasoning above; the first real
/// runs will say whether they are achievable, and each change should be
/// recorded with the measurement that motivated it.
pub mod tolerance {
    use super::FieldTolerance;

    // -- Tier 1: translation (Rust reference vs Yan Ren's fixtures) ---------

    /// Absolute agreement required on `k_eff` \[-\] between the Rust reference
    /// and the captured fixture.
    ///
    /// `1e-8`, an order of magnitude above the reference's own final `k_eff`
    /// residual of 9.272e-10. Tighter than this would be asserting agreement
    /// on digits the reference did not converge.
    ///
    /// In reactor units this is 1e-6 in `k`, i.e. 0.1 pcm — far below anything
    /// physically meaningful, which is the point: at the translation tier a
    /// disagreement is a bug, not a modelling choice.
    pub const TRANSLATION_K_EFF_ABS: f64 = 1e-8;

    /// Agreement required on the radial power map and axial power profile
    /// between the Rust reference and the captured fixtures.
    ///
    /// `1e-6` relative, matched to the fission-source residual of 9.611e-07 at
    /// which the reference stopped iterating. The fields are not determined
    /// more precisely than that by the capture itself.
    pub const TRANSLATION_POWER_SHAPE: FieldTolerance = FieldTolerance {
        name: "translation / power shape",
        max_relative: 1e-6,
        relative_l2: 1e-7,
    };

    /// Agreement required on the full node-level fields (power density,
    /// fission source, scalar flux) between the Rust reference and the
    /// captured fixtures.
    ///
    /// Same `1e-6` reasoning as [`TRANSLATION_POWER_SHAPE`]. Applied per node
    /// rather than to a summed shape, so it is the stricter of the two in
    /// practice: summing over `z` averages out node-level noise that this tier
    /// exposes.
    pub const TRANSLATION_NODE_FIELD: FieldTolerance = FieldTolerance {
        name: "translation / node-level field",
        max_relative: 1e-6,
        relative_l2: 1e-7,
    };

    // -- Tier 2: substitution (stage 2 vs stage 1) --------------------------

    /// Absolute agreement required on `k_eff` \[-\] between a substituted
    /// component and the reference.
    ///
    /// `1e-5`, i.e. **1 pcm**. Chosen because pcm is the unit reactor physics
    /// reports eigenvalues in, and a 1 pcm shift changes no conclusion drawn
    /// from a nodal-diffusion calculation — benchmark comparisons are quoted
    /// to tens of pcm at best. Tight enough that a genuine implementation
    /// error is very unlikely to slip under it.
    ///
    /// The linear-solver substitution is the one expected to test this number:
    /// if an iterative solve cannot reach 1 pcm of the direct factorisation,
    /// the honest response is to record the measured figure and revisit the
    /// tolerance, not to widen it quietly.
    pub const SUBSTITUTION_K_EFF_ABS: f64 = 1e-5;

    /// Agreement required on power shapes between a substituted component and
    /// the reference.
    ///
    /// `1e-3` relative — 0.1 % on a normalised node power. That is roughly an
    /// order of magnitude below the ~1–2 % node-power spread a nodal method
    /// shows against a fine-mesh reference, so a substitution that passes this
    /// changes nothing physically meaningful while still being a real
    /// constraint.
    pub const SUBSTITUTION_POWER_SHAPE: FieldTolerance = FieldTolerance {
        name: "substitution / power shape",
        max_relative: 1e-3,
        relative_l2: 1e-4,
    };

    // -- Tier 3: benchmark (either path vs published values) ----------------

    /// Absolute agreement required on `k_eff` \[-\] against the **published**
    /// IAEA-3D reference eigenvalue.
    ///
    /// `1e-3`, i.e. 100 pcm. This is a "rough parity" bar in the sense of §4,
    /// not a precision claim: the published IAEA-3D reference is a fine-mesh
    /// finite-difference result and a coarse-mesh nodal method is expected to
    /// differ from it by tens of pcm.
    ///
    /// **The published value is deliberately not hard-coded here.** Citing a
    /// benchmark number requires the source, edition and page recorded
    /// alongside it per `DATA_POLICY.md`; that belongs with the benchmark case
    /// data, not in a tolerance table. The tolerance is stated now so the gate
    /// is ready; the value it applies to is supplied when the case lands.
    pub const BENCHMARK_K_EFF_ABS: f64 = 1e-3;

    /// Agreement required on the radial power map against the published
    /// IAEA-3D assembly powers.
    ///
    /// `5e-2` — 5 % on a normalised assembly power. Published nodal-diffusion
    /// comparisons for IAEA-3D typically land within a few percent of the
    /// fine-mesh reference, with the largest errors at the core periphery
    /// where the flux gradient is steepest. This is the accuracy of the
    /// *method*, not of the implementation.
    pub const BENCHMARK_POWER_SHAPE: FieldTolerance = FieldTolerance {
        name: "benchmark / power shape",
        max_relative: 5e-2,
        relative_l2: 2e-2,
    };

    // -- Comparator behaviour ----------------------------------------------

    /// Fraction of a field's peak magnitude below which an entry is treated as
    /// negligible for the *relative* difference statistic.
    ///
    /// `1e-9`. Reactor fields are full of exact and near-exact zeros — 112 of
    /// the 289 radial positions are unfuelled reflector — and a relative
    /// difference against a value of 1e-30 is arithmetic noise that would
    /// dominate the statistic and hide a real discrepancy elsewhere.
    ///
    /// Entries below the floor are **not ignored**: they are still fully
    /// covered by the absolute-difference and L2 statistics, which is the
    /// right way to check that something expected to be zero really is.
    pub const SIGNIFICANCE_FRACTION: f64 = 1e-9;
}

// ---------------------------------------------------------------------------
// Field shape — how a flat index is named back to the reader
// ---------------------------------------------------------------------------

/// What a flat field's indices mean, so a divergence can be reported at a
/// coordinate rather than at an offset.
///
/// Enum dispatch rather than a trait object, per the workspace Rust rules: the
/// set of field shapes in this crate is closed.
#[derive(Debug, Clone, Copy)]
pub enum FieldShape {
    /// A full state vector — `g`, `ix`, `iy`, `iz`, flattened as
    /// [`Grid::index`].
    State(Grid),
    /// A radial map — `ix * ny + iy`, row-major with `ix` as the row.
    Radial {
        /// Nodes in x.
        nx: usize,
        /// Nodes in y.
        ny: usize,
    },
    /// An axial profile — one entry per `iz`.
    Axial {
        /// Nodes in z.
        nz: usize,
    },
}

impl FieldShape {
    /// Number of entries a field of this shape must have.
    #[must_use]
    pub fn len(&self) -> usize {
        match self {
            Self::State(grid) => grid.state_len(),
            Self::Radial { nx, ny } => nx * ny,
            Self::Axial { nz } => *nz,
        }
    }

    /// Names flat index `idx` in the coordinates a reader can act on.
    ///
    /// Both conventions are printed: this crate's 0-based indices, and the
    /// **1-based MATLAB indices** the fixtures and the original source use.
    /// Anyone cross-checking a divergence against Yan Ren's code needs the
    /// latter, and converting by hand at 2 a.m. is exactly how off-by-ones get
    /// introduced.
    #[must_use]
    pub fn describe(&self, idx: usize) -> String {
        match self {
            Self::State(grid) => match grid.unindex(idx) {
                Ok((g, ix, iy, iz)) => format!(
                    "flat {idx} = (g={g}, ix={ix}, iy={iy}, iz={iz}) 0-based \
                     / (g={}, ix={}, iy={}, iz={}) MATLAB",
                    g + 1,
                    ix + 1,
                    iy + 1,
                    iz + 1
                ),
                Err(e) => format!("flat {idx} (uninterpretable: {e})"),
            },
            Self::Radial { ny, .. } => {
                let (ix, iy) = (idx / ny, idx % ny);
                format!(
                    "flat {idx} = (ix={ix}, iy={iy}) 0-based / (ix={}, iy={}) MATLAB",
                    ix + 1,
                    iy + 1
                )
            }
            Self::Axial { .. } => {
                format!("flat {idx} = (iz={idx}) 0-based / (iz={}) MATLAB", idx + 1)
            }
        }
    }
}

// ---------------------------------------------------------------------------
// The comparator
// ---------------------------------------------------------------------------

/// A named tolerance pair applied to a [`FieldComparison`].
///
/// Two numbers rather than one because they fail differently: `max_relative`
/// catches a single badly wrong node, `relative_l2` catches a field that is
/// slightly wrong everywhere. A substitution can pass one and fail the other,
/// and which one it fails says what kind of mistake it is.
#[derive(Debug, Clone, Copy)]
pub struct FieldTolerance {
    /// What this tolerance is for, printed in failure messages.
    pub name: &'static str,
    /// Largest permitted relative difference at any significant entry \[-\].
    pub max_relative: f64,
    /// Largest permitted L2 norm of the difference, relative to the L2 norm of
    /// the reference field \[-\].
    pub relative_l2: f64,
}

/// Where a field's worst disagreement is, and how big it is.
#[derive(Debug, Clone, Copy)]
pub struct WorstPoint {
    /// Flat index of the point.
    pub index: usize,
    /// Value from the field under test.
    pub actual: f64,
    /// Value from the reference field.
    pub reference: f64,
    /// The difference statistic at this point — absolute or relative depending
    /// on which maximum it is reporting.
    pub difference: f64,
}

/// The result of comparing a field against a reference field.
///
/// Carries the four statistics and, for each maximum, **where it occurred**.
/// The [`std::fmt::Display`] implementation prints the lot as a report; it is
/// what a failing assertion shows.
#[derive(Debug, Clone)]
pub struct FieldComparison {
    /// Shape used to interpret indices.
    pub shape: FieldShape,
    /// Number of entries compared.
    pub len: usize,
    /// Largest absolute difference \[field units\].
    pub max_absolute: f64,
    /// Where [`Self::max_absolute`] occurred.
    pub max_absolute_at: WorstPoint,
    /// Largest relative difference over the significant entries \[-\].
    ///
    /// Zero if no entry cleared the significance floor.
    pub max_relative: f64,
    /// Where [`Self::max_relative`] occurred.
    pub max_relative_at: WorstPoint,
    /// L2 norm of the difference \[field units\].
    pub l2_absolute: f64,
    /// L2 norm of the reference field \[field units\].
    pub l2_reference: f64,
    /// [`Self::l2_absolute`] divided by [`Self::l2_reference`] \[-\].
    ///
    /// Infinite if the reference field is identically zero.
    pub l2_relative: f64,
    /// How many entries cleared the significance floor and so contributed to
    /// [`Self::max_relative`].
    pub significant: usize,
    /// The magnitude below which an entry was treated as negligible for the
    /// relative statistic \[field units\].
    pub significance_floor: f64,
}

impl FieldComparison {
    /// Whether both statistics are within `tolerance`.
    #[must_use]
    pub fn is_within(&self, tolerance: FieldTolerance) -> bool {
        self.max_relative <= tolerance.max_relative && self.l2_relative <= tolerance.relative_l2
    }

    /// Panics with the full report unless both statistics are within
    /// `tolerance`.
    ///
    /// # Panics
    ///
    /// If either statistic exceeds its tolerance. The panic message carries
    /// the whole comparison, including the coordinates of the worst point in
    /// both index conventions.
    pub fn assert_within(&self, tolerance: FieldTolerance) {
        assert!(
            self.is_within(tolerance),
            "{} parity FAILED (limits: max relative {:.3e}, relative L2 {:.3e})\n{self}",
            tolerance.name,
            tolerance.max_relative,
            tolerance.relative_l2
        );
    }
}

impl std::fmt::Display for FieldComparison {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "  entries compared     {}", self.len)?;
        writeln!(
            f,
            "  max absolute diff    {:.6e}  at {}",
            self.max_absolute,
            self.shape.describe(self.max_absolute_at.index)
        )?;
        writeln!(
            f,
            "                       actual {:.17e} vs reference {:.17e}",
            self.max_absolute_at.actual, self.max_absolute_at.reference
        )?;
        writeln!(
            f,
            "  max relative diff    {:.6e}  at {}",
            self.max_relative,
            self.shape.describe(self.max_relative_at.index)
        )?;
        writeln!(
            f,
            "                       actual {:.17e} vs reference {:.17e}",
            self.max_relative_at.actual, self.max_relative_at.reference
        )?;
        writeln!(
            f,
            "  L2 of difference     {:.6e}  (reference L2 {:.6e}, ratio {:.6e})",
            self.l2_absolute, self.l2_reference, self.l2_relative
        )?;
        write!(
            f,
            "  relative statistic covered {} of {} entries (floor {:.3e}; \
             the rest are checked in absolute terms only)",
            self.significant, self.len, self.significance_floor
        )
    }
}

/// Compares `actual` against `reference`, reporting where they diverge.
///
/// The relative statistic is taken only over entries whose reference magnitude
/// clears [`tolerance::SIGNIFICANCE_FRACTION`] of the field's peak — see that
/// constant for why. Every entry contributes to the absolute and L2
/// statistics regardless.
///
/// # Panics
///
/// If the two slices differ in length, or either differs from `shape.len()`.
/// A shape mismatch is a programming error in the test, not a physics result,
/// so it fails immediately rather than producing a meaningless number.
#[must_use]
pub fn compare(actual: &[f64], reference: &[f64], shape: FieldShape) -> FieldComparison {
    assert_eq!(
        actual.len(),
        reference.len(),
        "compared fields differ in length"
    );
    assert_eq!(
        actual.len(),
        shape.len(),
        "field length does not match the declared shape"
    );
    assert!(!actual.is_empty(), "cannot compare empty fields");

    let peak = reference.iter().fold(0.0_f64, |acc, v| acc.max(v.abs()));
    let significance_floor = peak * tolerance::SIGNIFICANCE_FRACTION;

    let mut max_absolute = WorstPoint {
        index: 0,
        actual: actual[0],
        reference: reference[0],
        difference: 0.0,
    };
    let mut max_relative = WorstPoint {
        index: 0,
        actual: actual[0],
        reference: reference[0],
        difference: 0.0,
    };
    let mut sum_sq_diff = 0.0_f64;
    let mut sum_sq_ref = 0.0_f64;
    let mut significant = 0usize;

    for (i, (a, r)) in actual.iter().zip(reference.iter()).enumerate() {
        let diff = (a - r).abs();
        sum_sq_diff += diff * diff;
        sum_sq_ref += r * r;

        if diff > max_absolute.difference {
            max_absolute = WorstPoint {
                index: i,
                actual: *a,
                reference: *r,
                difference: diff,
            };
        }

        if r.abs() > significance_floor {
            significant += 1;
            let rel = diff / r.abs();
            if rel > max_relative.difference {
                max_relative = WorstPoint {
                    index: i,
                    actual: *a,
                    reference: *r,
                    difference: rel,
                };
            }
        }
    }

    let l2_absolute = sum_sq_diff.sqrt();
    let l2_reference = sum_sq_ref.sqrt();
    let l2_relative = if l2_reference > 0.0 {
        l2_absolute / l2_reference
    } else if l2_absolute == 0.0 {
        0.0
    } else {
        f64::INFINITY
    };

    FieldComparison {
        shape,
        len: actual.len(),
        max_absolute: max_absolute.difference,
        max_absolute_at: max_absolute,
        max_relative: max_relative.difference,
        max_relative_at: max_relative,
        l2_absolute,
        l2_reference,
        l2_relative,
        significant,
        significance_floor,
    }
}

/// Asserts two scalars agree to an absolute tolerance, reporting the signed
/// difference and the difference in pcm.
///
/// Used for `k_eff`, where pcm is the unit the reader thinks in.
///
/// # Panics
///
/// If `|actual - reference| > tolerance`.
pub fn assert_k_eff_within(actual: f64, reference: f64, tolerance: f64, what: &str) {
    let diff = actual - reference;
    assert!(
        diff.abs() <= tolerance,
        "{what}: k_eff parity FAILED\n  \
         actual    {actual:.10}\n  \
         reference {reference:.10}\n  \
         difference {diff:+.3e} ({:+.2} pcm), tolerance {tolerance:.3e}",
        diff / reference * 1e5
    );
}

// ---------------------------------------------------------------------------
// Graceful skipping for the uncommitted fixtures
// ---------------------------------------------------------------------------

/// A steady-state solve reduced to the quantities the gates compare.
///
/// Deliberately a plain data record rather than a view onto solver internals:
/// the parity harness must not depend on how either path stores its state, or
/// it would need editing every time a solver is refactored.
#[derive(Debug, Clone)]
pub struct SteadyStateSolution {
    /// The grid the solve ran on.
    pub grid: Grid,
    /// Converged multiplication factor \[-\].
    pub k_eff: f64,
    /// Radial power map \[-\], normalised as
    /// [`fixtures::Iaea3dReduced::radial_power_map`].
    pub radial_power_map: Vec<f64>,
    /// Axial power profile \[-\], normalised the same way.
    pub axial_power_profile: Vec<f64>,
    /// Full node-level power density, if the solve exposes it.
    ///
    /// Optional because only the node-level gates need it, and they are the
    /// ones already conditional on the uncommitted fixtures.
    pub power_density: Option<Vec<f64>>,
}

/// **The wiring point.** Runs the stage-1 reference solve of IAEA-3D, or
/// returns `None` while the solver modules are still stubs.
///
/// # How to activate the ignored gates
///
/// Two edits, and no test body changes:
///
/// 1. Replace the body below with a call to the reference case constructor and
///    steady-state driver once `reference::cases` and `reference::coupling`
///    land, reducing the result with
///    [`fixtures::radial_power_map`] and [`fixtures::axial_power_profile`].
/// 2. Remove `#[ignore]` from the gates in `tests/benchmark/` and
///    `tests/parity/`.
///
/// Until step 1 happens the gates report a skip and pass, so removing an
/// `#[ignore]` early is harmless rather than a spurious red.
#[must_use]
pub fn solve_iaea3d_reference() -> Option<SteadyStateSolution> {
    // TODO(bedok-solver): wire to reference::cases::iaea3d + the steady driver.
    None
}

/// Reports that a gate could not run because [`solve_iaea3d_reference`] is not
/// wired yet.
///
/// Same libtest caveat as [`skip_unless_full_fixtures`]: a skip shows up as a
/// pass, and the message needs `--nocapture` to be seen.
pub fn report_solver_not_wired(what: &str) {
    println!(
        "SKIP {what}: the stage-1 reference solver is not wired into the \
         harness yet. See support::solve_iaea3d_reference."
    );
}

/// Loads the full node-level IAEA-3D fixtures, or returns `None` after
/// printing why it could not.
///
/// The full fields are gitignored and regenerable, so a fresh clone will not
/// have them. A test that needs them calls this and returns early on `None`:
///
/// ```ignore
/// let Some(full) = support::skip_unless_full_fixtures("my test") else { return };
/// ```
///
/// # A caveat worth knowing
///
/// libtest has no notion of a skipped test, so a skip is reported as a pass.
/// The message is printed to stdout and is therefore only visible under
/// `cargo test -- --nocapture`. That is the trade being made: a fresh clone
/// gets a green suite rather than a red one it cannot fix without Octave.
///
/// # Panics
///
/// If the files are present but malformed — that is a real failure, not a
/// missing optional input.
#[must_use]
pub fn skip_unless_full_fixtures(what: &str) -> Option<fixtures::Iaea3dFullFields> {
    match fixtures::Iaea3dFullFields::try_load() {
        Ok(Some(full)) => Some(full),
        Ok(None) => {
            println!(
                "SKIP {what}: full node-level fixtures not present at {}.\n\
                 They are gitignored and regenerable in ~77 s:\n    {}",
                fixtures::full_fixture_dir(fixtures::IAEA3D).display(),
                fixtures::REGENERATE_FULL_FIXTURES
            );
            None
        }
        Err(e) => panic!("full fixtures are present but unreadable: {e}"),
    }
}
