//! Diffusion coefficients from total cross sections.
//!
//! # Provenance
//!
//! - **Original author:** Than Yan Ren, Singapore Nuclear Research and Safety
//!   Institute (SNRSI).
//! - **Source file:** `calcdiffvalues3d.m`, `main_exec_diff3d_standalone`
//!   snapshot.
//! - **Permission:** given by the author for open-source release under OUTRAM
//!   PARK; see `docs/bedok-port-scoping.md` §6.
//! - **Licence:** GPL-3.0-only.

use crate::handle3dcoords::handle3dcoords;
use crate::matlab::{Array2, Array3, Array4};
use crate::types::Params;

/// `diffvalues = calcdiffvalues3d(params, sigmatotvalues, whichsigma)` and its
/// `mode`-carrying form.
///
/// Fills a per-node, per-group diffusion coefficient array from the material
/// total cross sections:
///
/// $$ D = \frac{n}{(2n + 1)\,\Sigma_{tot}} $$
///
/// with `n` the `mode` argument. `mode = 1` gives the familiar
/// $D = 1/(3\Sigma_{tot})$; higher values correspond to the higher P-N closure
/// the reference leaves available but never calls with.
///
/// # Arguments
///
/// - `params` — supplies `G` and the extents.
/// - `sigmatotvalues` — total macroscopic cross section, **0-based**
///   `(material_row, group)`. Units are the case file's, typically
///   cm<sup>-1</sup>.
/// - `whichsigma` — see the material-numbering note below.
/// - `mode` — the P-N order. `None` selects the reference's default of `1`,
///   matching its `isempty(varargin)` branch.
///
/// # Material numbering — 1-based values in a 0-based array
///
/// `whichsigma` is a 0-based **array** whose stored **values** are 1-based
/// material identifiers, with `0` meaning "no material". That split is
/// deliberate: the identifiers come straight out of the benchmark composition
/// CSVs, where `0` is the void marker and materials count from 1, so
/// renumbering them would mean rewriting the input data.
///
/// The consequence is one visible `- 1`: a node holding material `m` reads row
/// `m - 1` of `sigmatotvalues`.
///
/// # Returns
///
/// `(maxix, maxiy, maxiz, G)` diffusion coefficients, in the reciprocal of
/// `sigmatotvalues`' units (cm where the input is cm<sup>-1</sup>).
///
/// **Nodes with `whichsigma == 0` are left at zero**, not filled — the
/// reference `continue`s past them. Downstream code must read a zero `D` as
/// "absent material" rather than as a physical value.
///
/// # Panics
///
/// If a `whichsigma` entry indexes past the end of `sigmatotvalues`.
///
/// # Division by zero
///
/// A material with `sigmatotvalues == 0` yields an infinite `D`. The reference
/// does not guard this; the translation does not either.
pub fn calcdiffvalues3d(
    params: &Params,
    sigmatotvalues: &Array2<f64>,
    whichsigma: &Array3<usize>,
    mode: Option<f64>,
) -> Array4<f64> {
    let g_count = params.g;
    let (maxix, maxiy, maxiz) = handle3dcoords(params);

    let mut diffvalues = Array4::<f64>::zeros(maxix, maxiy, maxiz, g_count);

    // `if isempty(varargin) ... else mode = varargin{1}` — the reference's
    // comment calls 1 the "default definition".
    let mode = mode.unwrap_or(1.0);

    for ix in 0..maxix {
        for iy in 0..maxiy {
            for iz in 0..maxiz {
                let material = whichsigma.get(ix, iy, iz);
                if material == 0 {
                    continue;
                }
                // Material identifiers are 1-based; the array is 0-based.
                let row = material - 1;
                for g in 0..g_count {
                    let d = mode / ((2.0 * mode + 1.0) * sigmatotvalues.get(row, g));
                    diffvalues.set(ix, iy, iz, g, d);
                }
            }
        }
    }

    diffvalues
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> (Params, Array2<f64>, Array3<usize>) {
        let params = Params {
            maxix: Some(2),
            maxiy: Some(1),
            maxiz: Some(1),
            g: 2,
            ..Default::default()
        };
        // One material, two groups: Sigma_tot = 0.5 and 1.0.
        let mut sigmatot = Array2::<f64>::zeros(1, 2);
        sigmatot.set(0, 0, 0.5);
        sigmatot.set(0, 1, 1.0);
        // Node (0,0,0) is material 1; node (1,0,0) is void.
        let mut whichsigma = Array3::<usize>::zeros(2, 1, 1);
        whichsigma.set(0, 0, 0, 1);
        (params, sigmatot, whichsigma)
    }

    #[test]
    fn default_mode_gives_one_over_three_sigma() {
        let (params, sigmatot, whichsigma) = setup();
        let d = calcdiffvalues3d(&params, &sigmatot, &whichsigma, None);
        assert_eq!(d.get(0, 0, 0, 0), 1.0 / (3.0 * 0.5));
        assert_eq!(d.get(0, 0, 0, 1), 1.0 / (3.0 * 1.0));
    }

    /// Void nodes are skipped, so they keep the zero they were initialised to.
    #[test]
    fn nodes_without_material_stay_zero() {
        let (params, sigmatot, whichsigma) = setup();
        let d = calcdiffvalues3d(&params, &sigmatot, &whichsigma, None);
        assert_eq!(d.get(1, 0, 0, 0), 0.0);
        assert_eq!(d.get(1, 0, 0, 1), 0.0);
    }

    #[test]
    fn explicit_mode_uses_the_general_formula() {
        let (params, sigmatot, whichsigma) = setup();
        let d = calcdiffvalues3d(&params, &sigmatot, &whichsigma, Some(2.0));
        // 2 / ((2*2 + 1) * 0.5) = 2 / 2.5
        assert_eq!(d.get(0, 0, 0, 0), 2.0 / 2.5);
    }

    /// Material 2 must read row 1 — the 1-based-identifier / 0-based-array
    /// offset described in the doc comment.
    #[test]
    fn material_identifiers_are_offset_by_one() {
        let params = Params {
            maxix: Some(1),
            maxiy: Some(1),
            maxiz: Some(1),
            g: 1,
            ..Default::default()
        };
        let mut sigmatot = Array2::<f64>::zeros(2, 1);
        sigmatot.set(0, 0, 0.5); // material 1
        sigmatot.set(1, 0, 0.25); // material 2
        let mut whichsigma = Array3::<usize>::zeros(1, 1, 1);
        whichsigma.set(0, 0, 0, 2);

        let d = calcdiffvalues3d(&params, &sigmatot, &whichsigma, None);
        assert_eq!(d.get(0, 0, 0, 0), 1.0 / (3.0 * 0.25));
    }
}
