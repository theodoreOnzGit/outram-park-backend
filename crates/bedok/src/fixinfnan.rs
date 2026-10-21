//! Replace non-finite entries of a vector.
//!
//! # Provenance
//!
//! - **Original author:** Than Yan Ren, Singapore Nuclear Research and Safety
//!   Institute (SNRSI).
//! - **Source file:** `fixinfnan.m`, `main_exec_diff3d_standalone` snapshot.
//! - **Permission:** given by the author for open-source release under OUTRAM
//!   PARK; see the crate README, "Permission and attribution".
//! - **Licence:** GPL-3.0-only.

use crate::matlab::min_abs_finite;

/// `newvector = fixinfnan(vector)` and `fixinfnan(vector, anything)`.
///
/// Replaces every `Inf`, `-Inf` and `NaN` entry with either `0` (the default)
/// or `min(abs(vector))` (the special mode the reference selects by passing any
/// extra argument at all — the value is never inspected, only its presence).
///
/// The MATLAB `varargin` test becomes the `use_min_abs` flag: `false` is
/// `fixinfnan(v)`, `true` is `fixinfnan(v, _)`.
///
/// # Arguments
///
/// - `vector` — values to clean, in whatever units the caller works in; this
///   function is unit-agnostic.
/// - `use_min_abs` — `false` substitutes `0`, `true` substitutes the smallest
///   finite magnitude.
///
/// # Substitution value in the special mode
///
/// The reference evaluates `min(abs(vector))` on the **original** vector, so
/// the substitute is computed before any replacement happens. MATLAB's `min`
/// skips `NaN`, and `abs(Inf)` is `Inf`, so in practice the result is the
/// smallest finite magnitude — which is what
/// [`crate::matlab::min_abs_finite`] returns.
///
/// The one case where the two differ is a vector with **no** finite entry at
/// all: MATLAB would yield `Inf` (or `NaN` for an all-`NaN` input) and
/// propagate it, whereas `min_abs_finite` returns `None`. This translation
/// substitutes `0` there. That case cannot arise from the reference's own call
/// sites, and the divergence is recorded rather than hidden.
pub fn fixinfnan(vector: &[f64], use_min_abs: bool) -> Vec<f64> {
    fixinfnan_counted(vector, use_min_abs).0
}

/// `fixinfnan`, and **how many entries it had to replace** — defect C5.
///
/// # Why this exists
///
/// The reference applies `fixinfnan` to the flux straight out of every linear
/// solve. A solve that has blown up returns `Inf`/`NaN`, and this function
/// quietly turns those into zeros (or into the smallest finite magnitude). The
/// residual norms are then computed on the *patched* vector, so a diverged
/// solve can report a small residual and be indistinguishable from a converged
/// one. That is defect **C5**, and it is why an unstable case can look healthy
/// right up until the answer is examined.
///
/// **The numbers are unchanged.** This returns exactly what
/// [`fixinfnan`] returns; the count is additional information, so reporting it
/// costs no fidelity to the reference. A non-zero count is not a rounding
/// detail — it means the linear solve produced values that are not numbers,
/// and every quantity derived from that vector afterwards is suspect.
///
/// # Returns
///
/// `(patched, replaced)` — the vector as the reference would leave it, and the
/// number of non-finite entries that were substituted.
pub fn fixinfnan_counted(vector: &[f64], use_min_abs: bool) -> (Vec<f64>, usize) {
    let mut out = vector.to_vec();

    // `if any(mask)` — the reference only computes a substitute when there is
    // something to substitute, which matters because `min` over an empty
    // selection is not what we want to reach.
    if !out.iter().any(|x| !x.is_finite()) {
        return (out, 0);
    }

    let replacement = if use_min_abs {
        min_abs_finite(vector).unwrap_or(0.0)
    } else {
        0.0
    };

    let mut replaced = 0usize;
    for x in out.iter_mut() {
        if !x.is_finite() {
            *x = replacement;
            replaced += 1;
        }
    }
    (out, replaced)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_mode_zeroes_non_finite_entries() {
        let v = [1.0, f64::NAN, -2.0, f64::INFINITY, f64::NEG_INFINITY];
        assert_eq!(fixinfnan(&v, false), vec![1.0, 0.0, -2.0, 0.0, 0.0]);
    }

    #[test]
    fn special_mode_substitutes_smallest_finite_magnitude() {
        let v = [4.0, f64::NAN, -2.0, f64::INFINITY];
        assert_eq!(fixinfnan(&v, true), vec![4.0, 2.0, -2.0, 2.0]);
    }

    #[test]
    fn a_finite_vector_is_returned_untouched() {
        let v = [1.0, 2.0, 3.0];
        assert_eq!(fixinfnan(&v, true), vec![1.0, 2.0, 3.0]);
    }

    /// Pins the documented divergence: with no finite entry there is no
    /// smallest magnitude, and this translation substitutes `0` where MATLAB
    /// would propagate `Inf`.
    #[test]
    fn all_non_finite_input_falls_back_to_zero() {
        let v = [f64::NAN, f64::INFINITY];
        assert_eq!(fixinfnan(&v, true), vec![0.0, 0.0]);
    }

    /// **C5 — the substitution is counted, and the patched values are
    /// unchanged.**
    ///
    /// # Methodology
    ///
    /// Defect C5 is that `fixinfnan` turns a blown-up solve into a finite
    /// vector and the residual norms are then computed on the patch, so
    /// divergence can report a small residual. The correction does **not**
    /// change the patch — that would be a numeric change to every solve — it
    /// makes the patch *countable*.
    ///
    /// Two properties are required, and the first matters most: for any input,
    /// [`fixinfnan_counted`] must return exactly what [`fixinfnan`] returns, so
    /// no result anywhere moves. The second is that the count equals the number
    /// of non-finite entries.
    ///
    /// # Results — measured 2026-08-22
    ///
    /// Over the four cases below — clean, one NaN, mixed non-finites, and all
    /// non-finite — the patched vectors are **identical** to `fixinfnan`'s in
    /// every element and the counts are 0, 1, 4 and 3 respectively, matching
    /// the non-finite entries by construction.
    ///
    /// **Interpretation.** The correction is information-only. A caller that
    /// ignores the count gets bit-identical behaviour to the reference; a
    /// caller that reads it can tell a solve blew up. That is the whole of the
    /// C5 fix at this level — the solvers then have to carry the count
    /// outward, which `sanodaldiffusion_solverxyz` does through
    /// `SaNodalOutput::non_finite_substitutions`.
    #[test]
    fn c5_the_substitution_is_counted_without_changing_the_patch() {
        let cases: Vec<(&str, Vec<f64>, usize)> = vec![
            ("clean", vec![1.0, 2.0, -3.5], 0),
            ("one NaN", vec![1.0, f64::NAN, 3.0], 1),
            (
                "mixed",
                vec![f64::INFINITY, 1.0, f64::NEG_INFINITY, f64::NAN, 2.0, f64::NAN],
                4,
            ),
            ("all bad", vec![f64::NAN, f64::INFINITY, f64::NEG_INFINITY], 3),
        ];

        for (name, v, want) in cases {
            for use_min_abs in [false, true] {
                let plain = fixinfnan(&v, use_min_abs);
                let (counted, n) = fixinfnan_counted(&v, use_min_abs);
                eprintln!("{name:<8} min_abs={use_min_abs:<5} replaced {n} (expect {want})");
                assert_eq!(n, want, "{name}: wrong count");
                assert_eq!(
                    plain.len(),
                    counted.len(),
                    "{name}: length changed"
                );
                for (a, b) in plain.iter().zip(&counted) {
                    assert_eq!(a, b, "{name}: the patched value changed");
                }
            }
        }
    }
}
