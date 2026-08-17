//! Fission-source extrapolation, to accelerate the outer power iteration.
//!
//! # Provenance
//!
//! - **Original author:** Than Yan Ren, Singapore Nuclear Research and Safety
//!   Institute (SNRSI).
//! - **Source file:** `fiss_src_extrapolatexyz.m`, `main_exec_diff3d_standalone`
//!   snapshot.
//! - **Permission:** given by the author for open-source release under OUTRAM
//!   PARK; see `docs/bedok-port-scoping.md` §6.
//! - **Licence:** GPL-3.0-only.
//!
//! # Method reference
//!
//! The reference cites p. 51 of B. R. Bandini, *A three-dimensional transient
//! neutronics routine for the TRAC-PF1 reactor thermal hydraulic computer
//! code*, PhD thesis, Pennsylvania State University, 1990. The citation is
//! carried over as provenance for the method; the thesis itself is not in this
//! repository.

use crate::matlab::{Array2, SparseMatrix};

/// Whether the extrapolation was applied, and if not, why.
///
/// The reference signals this only through `verbose` printing; returning it
/// lets a caller log or count outcomes without parsing stdout. The
/// `verbose = 0` default means the reference prints nothing at all, so no
/// behaviour is lost by not printing here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Extrapolation {
    /// Applied, with the weight used.
    Applied,
    /// Skipped: a zero norm in the dominance-ratio denominator. The reference's
    /// comment attributes this to a stagnant fission source, typically in early
    /// iterations.
    ZeroNorm,
    /// Skipped: the two dominance-ratio estimates disagree by more than 10%,
    /// so the iteration is not yet in its asymptotic regime.
    NotAsymptotic,
}

/// `[phirecord, fs] = fiss_src_extrapolatexyz(sigmaf, phirecord)`.
///
/// Estimates the dominance ratio from three successive fission-source
/// differences and, if the iteration looks asymptotic, extrapolates both the
/// fission source and the current flux along that direction.
///
/// # Arguments
///
/// - `sigmaf` — the fission production operator, `philen` square.
/// - `phirecord` — flux history, `philen` rows by **4** columns: current,
///   previous, the one before, and the one before that. Column 0 is
///   overwritten when the extrapolation is applied.
///
/// # Returns
///
/// `(fs, outcome)` — the fission source (extrapolated if it was applied), and
/// which branch was taken.
///
/// # The guards, and why they are there
///
/// Three of the four safety checks carry explanatory comments in the reference,
/// which is unusual for this codebase and suggests they were added in response
/// to real failures:
///
/// - **Zero norms.** A stagnant fission source makes the dominance-ratio
///   denominator zero. Returns early with `fs` already computed.
/// - **Clamping the ratio to `[0, 0.99]`.** A ratio at or above 1 means the
///   iteration is not converging asymptotically; the weight `w = r/(1-r)` would
///   be negative or infinite and would overshoot catastrophically.
/// - **Capping the weight at 5.** Guards against a ratio that survives the
///   clamp but sits close to 1.
/// - **The 10% agreement test.** Uses an absolute difference rather than a
///   relative one, to avoid dividing by a near-zero weight, with a `1e-14`
///   floor.
///
/// # `fs` is computed before the guards
///
/// The early return still hands back a valid, un-extrapolated `fs`, because the
/// reference assigns `fs = sigmaf*phi` before testing anything. A caller can
/// use the result unconditionally.
pub fn fiss_src_extrapolatexyz(
    sigmaf: &mut SparseMatrix,
    phirecord: &mut Array2<f64>,
) -> (Vec<f64>, Extrapolation) {
    assert_eq!(
        phirecord.cols(),
        4,
        "phirecord must carry 4 flux generations, got {}",
        phirecord.cols()
    );

    let column = |a: &Array2<f64>, j: usize| -> Vec<f64> {
        (0..a.rows()).map(|i| a.get(i, j)).collect()
    };

    let phi = column(phirecord, 0);
    let phiold = column(phirecord, 1);
    let phiolder = column(phirecord, 2);
    let phioldest = column(phirecord, 3);

    let fsoldest = sigmaf.mul_vec(&phioldest);
    let fsolder = sigmaf.mul_vec(&phiolder);
    let fsold = sigmaf.mul_vec(&phiold);
    let mut fs = sigmaf.mul_vec(&phi);

    let norm_diff = norm_of_difference(&fsold, &fsolder);
    let norm_diffold = norm_of_difference(&fsolder, &fsoldest);

    if norm_diff == 0.0 || norm_diffold == 0.0 {
        return (fs, Extrapolation::ZeroNorm);
    }

    let domir = norm_of_difference(&fs, &fsold) / norm_diff;
    let domirold = norm_diff / norm_diffold;

    // `min(max(x, 0), 0.99)`, kept as a max/min chain rather than `clamp`.
    //
    // Clippy suggests `clamp`, and for finite input the two agree — but they
    // differ on `NaN`. `f64::clamp` propagates it; `x.max(0.0).min(0.99)`
    // returns `0.0`, because Rust's `max`/`min` return the non-NaN operand.
    // MATLAB's `min`/`max` also ignore `NaN`, so the chain is the faithful
    // translation and `clamp` is not. A `NaN` dominance ratio is reachable
    // here — the zero-norm guard above does not exclude a `NaN` norm, which a
    // diverging solve can produce.
    #[allow(clippy::manual_clamp)]
    let domir_safe = domir.max(0.0).min(0.99);
    #[allow(clippy::manual_clamp)]
    let domirold_safe = domirold.max(0.0).min(0.99);

    let wn = domir_safe / (1.0 - domir_safe);
    let wnold = domirold_safe / (1.0 - domirold_safe);

    let wn_max = 5.0;
    let wn = wn.min(wn_max);
    let wnold = wnold.min(wn_max);

    if (wn - wnold).abs() < 0.1 * wn.abs().max(wnold.abs()) + 1e-14 {
        for n in 0..fs.len() {
            fs[n] += wn * (fs[n] - fsold[n]);
        }
        for i in 0..phirecord.rows() {
            phirecord.set(i, 0, phi[i] + wn * (phi[i] - phiold[i]));
        }
        (fs, Extrapolation::Applied)
    } else {
        (fs, Extrapolation::NotAsymptotic)
    }
}

/// `norm(a - b)` — the Euclidean norm of the difference.
fn norm_of_difference(a: &[f64], b: &[f64]) -> f64 {
    a.iter()
        .zip(b.iter())
        .map(|(x, y)| (x - y) * (x - y))
        .sum::<f64>()
        .sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Identity fission operator, so `fs == phi` and the arithmetic is easy to
    /// follow by hand.
    fn identity(n: usize) -> SparseMatrix {
        let idx: Vec<usize> = (0..n).collect();
        SparseMatrix::assemble(&idx, &idx, &vec![1.0; n], n, n)
    }

    /// Build a `phirecord` from four generations, newest first.
    fn record(cols: [&[f64]; 4]) -> Array2<f64> {
        let rows = cols[0].len();
        let mut a = Array2::<f64>::zeros(rows, 4);
        for (j, c) in cols.iter().enumerate() {
            for i in 0..rows {
                a.set(i, j, c[i]);
            }
        }
        a
    }

    /// A stagnant source gives a zero denominator and must return early with an
    /// un-extrapolated but valid `fs`.
    #[test]
    fn a_stagnant_source_is_not_extrapolated() {
        let mut sigmaf = identity(2);
        let mut phirecord = record([&[1.0, 1.0], &[1.0, 1.0], &[1.0, 1.0], &[1.0, 1.0]]);

        let (fs, outcome) = fiss_src_extrapolatexyz(&mut sigmaf, &mut phirecord);
        assert_eq!(outcome, Extrapolation::ZeroNorm);
        assert_eq!(fs, vec![1.0, 1.0]);
        // The history is left untouched.
        assert_eq!(phirecord.get(0, 0), 1.0);
    }

    /// A geometrically converging sequence is extrapolated straight to its
    /// limit — which is the whole point of the method.
    ///
    /// # Methodology
    ///
    /// Generations `10.5, 11, 12, 14` (newest first) are `10 + 2^-n`, so they
    /// converge to 10 with a constant ratio of 0.5. Successive differences are
    /// `-0.5, -1, -2`, giving `domir = 0.5/1 = 0.5` and
    /// `domirold = 1/2 = 0.5`. The two agree, so the extrapolation fires with
    /// `wn = 0.5/(1 - 0.5) = 1`, and `fs + 1*(fs - fsold) = 10.5 - 0.5 = 10`.
    ///
    /// Note the sequence must **converge** for the ratio to come out below 1.
    /// A growing sequence such as `8, 4, 2, 1` newest-first has ratio 2, which
    /// the guards clamp to 0.99 and then cap the weight at 5 — the behaviour
    /// exercised by `a_diverging_ratio_is_clamped_rather_than_blowing_up`.
    ///
    /// # Results — measured 2026-08-12
    ///
    /// Both the source and column 0 of the history land exactly on 10.
    #[test]
    fn a_converging_sequence_is_extrapolated_to_its_limit() {
        let mut sigmaf = identity(1);
        let mut phirecord = record([&[10.5], &[11.0], &[12.0], &[14.0]]);

        let (fs, outcome) = fiss_src_extrapolatexyz(&mut sigmaf, &mut phirecord);
        assert_eq!(outcome, Extrapolation::Applied);
        assert_eq!(fs, vec![10.0]);
        assert_eq!(phirecord.get(0, 0), 10.0);
    }

    /// When the two ratio estimates disagree by more than 10%, the iteration is
    /// not asymptotic and nothing is changed.
    #[test]
    fn a_non_asymptotic_sequence_is_left_alone() {
        let mut sigmaf = identity(1);
        // Differences 90, 9, 0.9 give domir = 10 (clamped) against
        // domirold = 10 as well... so instead make the ratios differ sharply:
        // differences 1, 8, 1 -> domir = 1/8, domirold = 8.
        let mut phirecord = record([&[10.0], &[9.0], &[1.0], &[0.0]]);

        let (_, outcome) = fiss_src_extrapolatexyz(&mut sigmaf, &mut phirecord);
        assert_eq!(outcome, Extrapolation::NotAsymptotic);
        // Column 0 is untouched.
        assert_eq!(phirecord.get(0, 0), 10.0);
    }

    /// A diverging sequence would give a ratio above 1 and a negative or
    /// infinite weight; the clamp keeps the weight finite and capped.
    #[test]
    fn a_diverging_ratio_is_clamped_rather_than_blowing_up() {
        let mut sigmaf = identity(1);
        // Differences 8, 4, 2 -> domir = 2, domirold = 2; both clamp to 0.99,
        // giving wn = 99 before the cap and 5 after it.
        let mut phirecord = record([&[14.0], &[6.0], &[2.0], &[0.0]]);

        let (fs, outcome) = fiss_src_extrapolatexyz(&mut sigmaf, &mut phirecord);
        assert_eq!(outcome, Extrapolation::Applied);
        // fs = 14 + 5*(14-6) = 54, finite rather than divergent.
        assert_eq!(fs, vec![54.0]);
    }
}
