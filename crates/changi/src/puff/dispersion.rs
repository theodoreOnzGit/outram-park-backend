// SPDX-License-Identifier: GPL-3.0
//
// Ported from puff (R, MIT) `R/helpers.R` — `compute_sigma_vals`.
// Upstream: Hammerling-Research-Group/puff @ 5213d58. See ../mod.rs for the
// full provenance block.

//! Pasquill–Gifford dispersion coefficients `sigma_y` and `sigma_z`.
//!
//! These are the standard deviations of the Gaussian concentration profile
//! crosswind (`sigma_y`) and vertically (`sigma_z`), as functions of how far
//! the puff has travelled and of the [stability
//! class](super::stability::StabilityClass). They are empirical fits to the
//! Prairie Grass and related tracer campaigns, in the algebraic form given by
//! Martin (1976) as used by the US EPA's ISC models:
//!
//! ```text
//!   sigma_z = a * x^b                       (x in km, sigma_z in m)
//!   sigma_y = 465.11628 * x * tan(theta),  theta = (pi/180) * (c - d * ln x)
//! ```
//!
//! with `(a, b)` selected from a per-class table of distance bins and `(c, d)`
//! constant per class. The `465.11628` is `1000 / (2 * 2.15)` — the metres-per-
//! kilometre conversion folded into the half-width-to-sigma factor 2.15 that
//! the original nomograms were drawn with.
//!
//! **These fits are only defined over roughly 0.1–10 km.** Upstream applies
//! them at any positive distance; see [`pasquill_gifford_sigmas`] for what that
//! produces beyond the fitted range, which is measured rather than assumed.

use uom::si::f64::Length;
use uom::si::length::{kilometer, meter};

use super::stability::StabilityClass;

/// The crosswind and vertical spread of a puff.
///
/// Both are standard deviations of a Gaussian, in metres.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DispersionSigmas {
    /// Crosswind (horizontal, perpendicular to travel) standard deviation.
    pub sigma_y: Length,
    /// Vertical standard deviation.
    pub sigma_z: Length,
}

/// Upstream's hard cap on `sigma_z`, in metres.
///
/// `sigma_z <- min(a * x^b, 5000)`. Physically this stands in for the mixing
/// height: a plume cannot spread vertically past the capping inversion, and
/// 5 km is a generous upper bound on a daytime mixed layer. Note there is
/// **no matching cap on `sigma_y`**, which is why the horizontal spread runs
/// away at long range — see [`pasquill_gifford_sigmas`].
pub const SIGMA_Z_CAP_METERS: f64 = 5000.0;

/// `1000 m/km` divided by the 2.15 half-width-to-sigma factor of the original
/// Pasquill–Gifford nomograms, doubled: `1000 / (2 * 2.15) = 232.558…`, which
/// upstream writes as `465.11628` for the full width.
const SIGMA_Y_PREFACTOR: f64 = 465.11628;

/// Degrees-to-radians, as upstream spells it: `0.017453293`.
///
/// Kept to upstream's 9-digit literal rather than replaced by
/// `std::f64::consts::PI / 180.0`. The two differ in the 9th significant digit
/// (`1.7453292519943295e-2` against `1.7453293e-2`, a relative difference of
/// `2.8e-8`), which is far above `f64` epsilon and therefore visible in a
/// code-to-code comparison. Substituting the "better" constant would be a
/// silent change to the answer, so the literal stays and the choice is
/// recorded here.
const DEG_TO_RAD: f64 = 0.017_453_293;

/// The `(a, b)` power-law coefficients for `sigma_z`, binned by distance.
///
/// `cutoffs` are upper edges in km; the first bin whose cutoff the distance
/// does not exceed is selected, matching upstream's
/// `which(d_i <= params$cutoffs)[1]`. The final cutoff is `f64::INFINITY`
/// (upstream's `Inf`), so the last bin always matches and the lookup is total.
struct SigmaZTable {
    a: &'static [f64],
    b: &'static [f64],
    cutoffs: &'static [f64],
}

/// Per-class coefficients, transcribed from upstream's `class_params` list.
///
/// Classes `A`, `B`, `D`, `E`, `F` are binned; class `C` is a single unbinned
/// power law, which upstream represents by omitting `c_vals`/`b_vals`
/// entirely.
struct ClassParams {
    sigma_z: Option<SigmaZTable>,
    /// Unbinned `(a, b)` — used only by class `C`.
    unbinned: Option<(f64, f64)>,
    /// `sigma_y` angle coefficients.
    c: f64,
    d: f64,
}

const fn params(class: StabilityClass) -> ClassParams {
    match class {
        StabilityClass::A => ClassParams {
            sigma_z: Some(SigmaZTable {
                a: &[
                    122.8, 158.08, 170.22, 179.52, 217.41, 258.89, 346.75, 453.85,
                ],
                b: &[
                    0.9447, 1.0542, 1.0932, 1.1262, 1.2644, 1.4094, 1.7283, 2.1166,
                ],
                cutoffs: &[0.1, 0.15, 0.2, 0.25, 0.3, 0.4, 0.5, f64::INFINITY],
            }),
            unbinned: None,
            c: 24.167,
            d: 2.5334,
        },
        StabilityClass::B => ClassParams {
            sigma_z: Some(SigmaZTable {
                a: &[90.673, 98.483, 109.3],
                b: &[0.93198, 0.98332, 1.0971],
                cutoffs: &[0.2, 0.4, f64::INFINITY],
            }),
            unbinned: None,
            c: 18.333,
            d: 1.8096,
        },
        StabilityClass::C => ClassParams {
            sigma_z: None,
            unbinned: Some((61.141, 0.91465)),
            c: 12.5,
            d: 1.0857,
        },
        StabilityClass::D => ClassParams {
            sigma_z: Some(SigmaZTable {
                a: &[34.459, 32.093, 32.093, 33.504, 36.65, 44.053],
                b: &[0.86974, 0.81066, 0.64403, 0.60486, 0.56589, 0.51179],
                cutoffs: &[0.3, 1.0, 3.0, 10.0, 30.0, f64::INFINITY],
            }),
            unbinned: None,
            c: 8.333,
            d: 0.72382,
        },
        StabilityClass::E => ClassParams {
            sigma_z: Some(SigmaZTable {
                a: &[
                    24.26, 23.331, 21.628, 21.628, 22.534, 24.703, 26.97, 35.42, 47.618,
                ],
                b: &[
                    0.8366, 0.81956, 0.7566, 0.63077, 0.57154, 0.50527, 0.46713, 0.37615, 0.29592,
                ],
                cutoffs: &[0.1, 0.3, 1.0, 2.0, 4.0, 10.0, 20.0, 40.0, f64::INFINITY],
            }),
            unbinned: None,
            c: 6.25,
            d: 0.54287,
        },
        StabilityClass::F => ClassParams {
            sigma_z: Some(SigmaZTable {
                a: &[
                    15.209, 14.457, 13.953, 13.953, 14.823, 16.187, 17.836, 22.651, 27.074, 34.219,
                ],
                b: &[
                    0.81558, 0.78407, 0.68465, 0.63227, 0.54503, 0.4649, 0.41507, 0.32681, 0.27436,
                    0.21716,
                ],
                cutoffs: &[
                    0.2,
                    0.7,
                    1.0,
                    2.0,
                    3.0,
                    7.0,
                    15.0,
                    30.0,
                    60.0,
                    f64::INFINITY,
                ],
            }),
            unbinned: None,
            c: 4.1667,
            d: 0.36191,
        },
    }
}

/// Pasquill–Gifford `sigma_y` and `sigma_z` at a travel distance.
///
/// Ports `compute_sigma_vals` for a single class and distance. Upstream
/// vectorises over both and returns a `2 × n` matrix; a scalar function plus an
/// iterator is the Rust equivalent and avoids the column-major indexing that
/// makes upstream's own `gpuff` discard its second stability class.
///
/// # Arguments
/// - `class` — the stability class selecting the coefficient set.
/// - `distance` — distance travelled from the source. Converted to **kilometres**
///   internally, because the fits are defined in km; the `uom` type means the
///   caller cannot get that conversion wrong, which upstream's bare `f64` and
///   its `total_dist <- total_dist / 1000` line inside `gpuff` can.
///
/// # Returns
/// `None` when `distance <= 0`, which is upstream's `NA` case: a puff that has
/// not moved has no defined spread, and upstream's `gpuff` converts the
/// resulting `NA` concentration to `0`. Returning `Option` rather than a NaN
/// makes that a case the caller must handle.
///
/// # Valid range, and what happens outside it
///
/// The fits are empirical over roughly **0.1–10 km**. Upstream applies them at
/// any positive distance and this port reproduces that, so the caller can hit
/// two regimes where the answer is formally defined and physically meaningless:
///
/// * **`sigma_z` saturates** at [`SIGMA_Z_CAP_METERS`], by upstream's explicit
///   `min(..., 5000)`.
/// * **`sigma_y` does not saturate, and eventually goes negative.** It is
///   `465.11628 * x * tan(theta)` with `theta = (pi/180)(c - d ln x)`, so
///   `theta` falls through zero as `x` grows and `tan(theta)` follows it.
///   Measured for class `D` (`c = 8.333`, `d = 0.72382`): `theta` reaches zero
///   at `x = exp(8.333 / 0.72382) = 9.6e4 km`, well past any sane use, but for
///   class `F` (`c = 4.1667`, `d = 0.36191`) it is `x = 9.9e4 km`. Both are
///   beyond Earth's circumference, so the sign change is unreachable in
///   practice — but a *negative* `sigma_y` would not be caught by the model,
///   because [`super::concentration`] only ever uses `sigma_y^2`.
///
/// Neither regime is clamped here. Adding a range check upstream does not have
/// would make this function disagree with upstream inside the fitted range's
/// own edge cases, and the code-to-code comparison is the point. Callers who
/// need a bound should apply it at the call site.
#[must_use]
pub fn pasquill_gifford_sigmas(
    class: StabilityClass,
    distance: Length,
) -> Option<DispersionSigmas> {
    let x = distance.get::<kilometer>();
    if !(x > 0.0) {
        // Upstream: `if (d_i <= 0) { sigma_y <- NA; sigma_z <- NA }`. Written
        // negated so a NaN distance also lands here rather than falling
        // through to produce a NaN sigma.
        return None;
    }
    let p = params(class);

    let (a, b) = match (&p.sigma_z, p.unbinned) {
        (Some(table), _) => {
            let bin = table
                .cutoffs
                .iter()
                .position(|c| x <= *c)
                .expect("the last cutoff is infinite, so a bin always matches");
            (table.a[bin], table.b[bin])
        }
        (None, Some(ab)) => ab,
        (None, None) => unreachable!("every class has either a table or unbinned coefficients"),
    };

    let sigma_z = (a * x.powf(b)).min(SIGMA_Z_CAP_METERS);
    let big_theta = DEG_TO_RAD * (p.c - p.d * x.ln());
    let sigma_y = SIGMA_Y_PREFACTOR * x * big_theta.tan();

    Some(DispersionSigmas {
        sigma_y: Length::new::<meter>(sigma_y),
        sigma_z: Length::new::<meter>(sigma_z),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn km(v: f64) -> Length {
        Length::new::<kilometer>(v)
    }

    /// Upstream's own documented example: `compute_sigma_vals("A", 0.7)`.
    ///
    /// Reference values taken by running the upstream R at `5213d58`:
    /// `sigma_y = 152.3098 m`, `sigma_z = 213.3275 m`.
    #[test]
    fn matches_upstreams_documented_example() {
        let s = pasquill_gifford_sigmas(StabilityClass::A, km(0.7)).unwrap();
        assert!((s.sigma_y.get::<meter>() - 152.3098).abs() < 1e-3);
        assert!((s.sigma_z.get::<meter>() - 213.3275).abs() < 1e-3);
    }

    #[test]
    fn non_positive_distance_has_no_defined_spread() {
        assert!(pasquill_gifford_sigmas(StabilityClass::D, km(0.0)).is_none());
        assert!(pasquill_gifford_sigmas(StabilityClass::D, km(-1.0)).is_none());
        assert!(pasquill_gifford_sigmas(StabilityClass::D, km(f64::NAN)).is_none());
    }

    #[test]
    fn sigma_z_saturates_at_the_cap() {
        let s = pasquill_gifford_sigmas(StabilityClass::A, km(1e6)).unwrap();
        assert_eq!(s.sigma_z.get::<meter>(), SIGMA_Z_CAP_METERS);
    }

    /// The bin edges are `<=`, not `<`: a distance exactly on a cutoff belongs
    /// to the *lower* bin. Upstream's `which(d_i <= cutoffs)[1]` says so, and
    /// getting it backwards would be invisible except exactly on an edge.
    #[test]
    fn a_distance_on_a_bin_edge_takes_the_lower_bin() {
        // Class D's first cutoff is 0.3 km, with (a, b) = (34.459, 0.86974);
        // the second bin is (32.093, 0.81066).
        let on_edge = pasquill_gifford_sigmas(StabilityClass::D, km(0.3))
            .unwrap()
            .sigma_z
            .get::<meter>();
        let expected_lower = 34.459 * 0.3_f64.powf(0.86974);
        assert!((on_edge - expected_lower).abs() < 1e-9);
    }

    /// Class C is the only unbinned class; a bug that gave it a table would
    /// most likely show up as a discontinuity, so check it is smooth across
    /// where other classes have edges.
    #[test]
    fn class_c_is_a_single_unbinned_power_law() {
        for x in [0.05, 0.2, 0.31, 1.0, 3.01, 30.0] {
            let got = pasquill_gifford_sigmas(StabilityClass::C, km(x))
                .unwrap()
                .sigma_z
                .get::<meter>();
            let want = (61.141 * x.powf(0.91465)).min(SIGMA_Z_CAP_METERS);
            assert!((got - want).abs() < 1e-9, "x = {x}");
        }
    }

    /// The sign change in `sigma_y` is real but unreachable on Earth. Pinning
    /// the crossing distance means a change to `c` or `d` cannot quietly move
    /// it into the usable range.
    #[test]
    fn sigma_y_sign_change_is_far_beyond_any_physical_range() {
        for (class, c, d) in [
            (StabilityClass::D, 8.333, 0.72382),
            (StabilityClass::F, 4.1667, 0.36191),
        ] {
            let crossing_km: f64 = (c / d as f64).exp();
            assert!(
                crossing_km > 4.0e4,
                "{} crosses zero at {crossing_km:e} km, inside Earth's circumference",
                class.letter()
            );
            let just_inside = pasquill_gifford_sigmas(class, km(crossing_km * 0.5))
                .unwrap()
                .sigma_y
                .get::<meter>();
            assert!(just_inside > 0.0);
        }
    }
}
