//! Replace non-finite entries of a vector.
//!
//! # Provenance
//!
//! - **Original author:** Than Yan Ren, Singapore Nuclear Research and Safety
//!   Institute (SNRSI).
//! - **Source file:** `fixinfnan.m`, `main_exec_diff3d_standalone` snapshot.
//! - **Permission:** given by the author for open-source release under OUTRAM
//!   PARK; see `docs/bedok-port-scoping.md` §6.
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
    let mut out = vector.to_vec();

    // `if any(mask)` — the reference only computes a substitute when there is
    // something to substitute, which matters because `min` over an empty
    // selection is not what we want to reach.
    if !out.iter().any(|x| !x.is_finite()) {
        return out;
    }

    let replacement = if use_min_abs {
        min_abs_finite(vector).unwrap_or(0.0)
    } else {
        0.0
    };

    for x in out.iter_mut() {
        if !x.is_finite() {
            *x = replacement;
        }
    }
    out
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
}
