//! Expand per-material cross-section data onto the spatial mesh.
//!
//! # Provenance
//!
//! - **Original author:** Than Yan Ren, Singapore Nuclear Research and Safety
//!   Institute (SNRSI).
//! - **Source file:** `makesigmadfxyz.m`, `main_exec_diff3d_standalone`
//!   snapshot.
//! - **Permission:** given by the author for open-source release under OUTRAM
//!   PARK; see the crate README, "Permission and attribution".
//! - **Licence:** GPL-3.0-only.

use crate::handle3dcoords::handle3dcoords;
use crate::matlab::{Array2, Array3, SparseMatrix};
use crate::types::{Params, Sigma, SigmaValues};

/// Which index grid the operators are built on.
///
/// The reference passes these as the bare integers `1` and `2` in `varargin`,
/// defaulting to `1`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SigmaIndexMode {
    /// Mode 1 — full indices only, one entry per node. Every call site in the
    /// snapshot uses this.
    Full,
    /// Mode 2 — the `(2n+1)` half-index grid.
    ///
    /// **Carries a reference defect that truncates the axial extent** — see
    /// [`makesigmadfxyz`].
    HalfIndex,
}

/// `sigma = makesigmadfxyz(params, sigmavalues, whichsigma, mode)`.
///
/// Builds the six sparse cross-section operators, plus the per-node `nu` and
/// `chi` arrays, by looking up each node's material and scattering its data
/// into the flattened `(group, node)` index space.
///
/// # Arguments
///
/// - `params` — supplies `G`, `Nc` and the extents.
/// - `sigmavalues` — per-material data; see [`SigmaValues`].
/// - `whichsigma` — material identifier per node, 1-based, `0` for void.
/// - `mode` — `None` selects the reference's default of [`SigmaIndexMode::Full`].
///
/// # Returns
///
/// [`Sigma`], with every matrix `philenf = philen + Nc*es` square. The
/// precursor tail beyond `philen` is left empty; it is the solvers that fill
/// it.
///
/// # The operators, and how they differ
///
/// | Field | Content | Shape |
/// |---|---|---|
/// | `tot` | total cross section | diagonal |
/// | `sd` | within-group scattering `Sigma_s(g -> g)` | diagonal |
/// | `fb` | bare `Sigma_f`, no `chi`, no `nu` | diagonal |
/// | `s` | full scattering, `g` into `gt` | off-diagonal |
/// | `f` | `chi * nu * Sigma_f` | off-diagonal |
/// | `fp` | `chi * Sigma_fp` — **no `nu` factor** | off-diagonal |
///
/// `f` and `fp` share the same sparsity pattern, since the reference builds
/// both from the same row/column arrays. That means `fp` inherits `f`'s
/// structural filter: an entry appears only where `Sigma_f` **and** `chi` are
/// both non-zero. A material with zero `Sigma_f` but non-zero `Sigma_fp` would
/// contribute nothing to `fp`. Whether that combination is physical is a
/// question for the case data, not for this translation.
///
/// # Reference defect — mode 2 truncates the axial extent
///
/// The three loops read
///
/// ```text
/// for ix=m:m:m*maxix
///     for iy=m:m:m*maxiy
///         for iz=m:m:maxiz
/// ```
///
/// The `iz` bound is `maxiz`, not `m*maxiz` as the other two are. At
/// `mode == 1` (`m == 1`) the two are the same and nothing is wrong. At
/// `mode == 2` (`m == 2`) the loop runs `iz = 2, 4, … maxiz`, covering only
/// `iz/m = 1 … maxiz/2` — **the upper half of the core silently gets no cross
/// sections at all**, leaving those rows of every operator empty.
///
/// This is latent: every call site in the snapshot passes `1` explicitly, so
/// mode 2 is never exercised. Translated as written per the no-silent-repairs
/// rule in the crate README, "Translation policy", and pinned by a test below.
///
/// # Reference wart — `nu` is indexed two different ways
///
/// Within the same loop body the reference reads `nu` as
/// `nu(material)` when filling `snu`, but as `nu(material, g)` when building
/// the fission operator. The first is a linear index into a 2-D array, which in
/// MATLAB's column-major order lands on `nu(material, 1)` — the group-1 value —
/// regardless of `g`.
///
/// So `sigma.nu` carries the **group-1** `nu` at every entry, while `sigma.f`
/// uses the true per-group `nu`. Reproduced here via
/// [`Array2::get_linear_column_major`].
///
/// The reference also accepts a scalar `nu` and expands it with
/// `nu = sigmavalues.nu * ones(G)`, giving a `G`-by-`G` matrix of that value.
/// Both index forms then read the same number, so the inconsistency is
/// invisible in that case.
///
/// # Panics
///
/// If more entries are assembled than the reference's preallocation allows,
/// reproducing `error('Error in makesigma.tot')` and its siblings. The limits
/// are `philen` for the diagonals, `philen*10` for fission and `philen*15` for
/// scattering — i.e. up to 10 and 15 groups respectively before the guard
/// trips.
pub fn makesigmadfxyz(
    params: &Params,
    sigmavalues: &SigmaValues,
    whichsigma: &Array3<usize>,
    mode: Option<SigmaIndexMode>,
) -> Sigma {
    let g_count = params.g;
    let (maxix, maxiy, maxiz) = handle3dcoords(params);
    let nc = params.nc_or_zero();
    let mode = mode.unwrap_or(SigmaIndexMode::Full);

    let (xstep, ystep, philen, energyindexstep, m) = match mode {
        SigmaIndexMode::HalfIndex => {
            let xstep = (2 * maxiy + 1) * (2 * maxiz + 1);
            let ystep = 2 * maxiz + 1;
            let eis = (2 * maxix + 1) * (2 * maxiy + 1) * (2 * maxiz + 1);
            (xstep, ystep, g_count * eis, eis, 2usize)
        }
        SigmaIndexMode::Full => {
            let eis = maxix * maxiy * maxiz;
            (maxiy * maxiz, maxiz, g_count * eis, eis, 1usize)
        }
    };
    let philenf = philen + nc * maxix * maxiy * maxiz;

    // Diagonal triplets — one per (group, live node).
    let mut diag_idx: Vec<usize> = Vec::new();
    let mut tot_ele: Vec<f64> = Vec::new();
    let mut fb_ele: Vec<f64> = Vec::new();
    let mut sd_ele: Vec<f64> = Vec::new();

    // Fission triplets; `f` and `fp` share this pattern.
    let mut f_row: Vec<usize> = Vec::new();
    let mut f_col: Vec<usize> = Vec::new();
    let mut f_ele: Vec<f64> = Vec::new();
    let mut fp_ele: Vec<f64> = Vec::new();

    // Scattering triplets.
    let mut s_row: Vec<usize> = Vec::new();
    let mut s_col: Vec<usize> = Vec::new();
    let mut s_ele: Vec<f64> = Vec::new();

    let mut snu = vec![0.0; philen];
    let mut schi = Array2::<f64>::zeros(g_count, philen);

    // The reference's loops step by `m` and divide by `m` to recover the node.
    // The `iz` bound is `maxiz`, not `m*maxiz` — see the defect note.
    let mut ix = m;
    while ix <= m * maxix {
        let mut iy = m;
        while iy <= m * maxiy {
            let mut iz = m;
            while iz <= maxiz {
                let node_x = ix / m;
                let node_y = iy / m;
                let node_z = iz / m;

                // `whichsigma` is 0-based with 1-based material values.
                let material = whichsigma.get(node_x - 1, node_y - 1, node_z - 1);
                if material == 0 {
                    iz += m;
                    continue;
                }
                let row = material - 1;

                for g in 0..g_count {
                    // The reference's 1-based idx, converted: the `-1`s in
                    // `(ix-1)*xstep` and the trailing `+iz` cancel to this.
                    let idx = g * energyindexstep + (ix - 1) * xstep + (iy - 1) * ystep + (iz - 1);

                    // Linear index into `nu`, which lands on the group-1 value.
                    snu[idx] = sigmavalues.nu.get_linear_column_major(row);

                    diag_idx.push(idx);
                    tot_ele.push(sigmavalues.tot.get(row, g));
                    fb_ele.push(sigmavalues.f.get(row, g));
                    sd_ele.push(sigmavalues.s.get(row, g, g));

                    for gt in 0..g_count {
                        schi.set(gt, idx, sigmavalues.chi.get(row, gt));
                        if sigmavalues.f.get(row, g) != 0.0 && sigmavalues.chi.get(row, gt) != 0.0 {
                            let idxto = gt * energyindexstep
                                + (ix - 1) * xstep
                                + (iy - 1) * ystep
                                + (iz - 1);
                            f_row.push(idxto);
                            f_col.push(idx);
                            f_ele.push(
                                sigmavalues.chi.get(row, gt)
                                    * sigmavalues.nu.get(row, g)
                                    * sigmavalues.f.get(row, g),
                            );
                            fp_ele.push(
                                sigmavalues.chi.get(row, gt)
                                    * sigmavalues
                                        .fp
                                        .as_ref()
                                        .map(|fp| fp.get(row, g))
                                        .unwrap_or(0.0),
                            );
                        }
                    }

                    for gt in 0..g_count {
                        if sigmavalues.s.get(row, gt, g) != 0.0 {
                            let idxto = gt * energyindexstep
                                + (ix - 1) * xstep
                                + (iy - 1) * ystep
                                + (iz - 1);
                            s_row.push(idxto);
                            s_col.push(idx);
                            s_ele.push(sigmavalues.s.get(row, gt, g));
                        }
                    }
                }
                iz += m;
            }
            iy += m;
        }
        ix += m;
    }

    assert!(diag_idx.len() <= philen, "Error in makesigma.tot");
    assert!(f_row.len() <= philen * 10, "Error in makesigma.f");
    assert!(s_row.len() <= philen * 15, "Error in makesigma.s");

    Sigma {
        tot: SparseMatrix::assemble(&diag_idx, &diag_idx, &tot_ele, philenf, philenf),
        f: SparseMatrix::assemble(&f_row, &f_col, &f_ele, philenf, philenf),
        fp: SparseMatrix::assemble(&f_row, &f_col, &fp_ele, philenf, philenf),
        fb: SparseMatrix::assemble(&diag_idx, &diag_idx, &fb_ele, philenf, philenf),
        s: SparseMatrix::assemble(&s_row, &s_col, &s_ele, philenf, philenf),
        sd: SparseMatrix::assemble(&diag_idx, &diag_idx, &sd_ele, philenf, philenf),
        nu: snu,
        chi: schi,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two nodes in `z`, two groups, one material. `es = 2`, `philen = 4`.
    fn setup(maxiz: usize) -> (Params, SigmaValues, Array3<usize>) {
        let params = Params {
            maxix: Some(1),
            maxiy: Some(1),
            maxiz: Some(maxiz),
            g: 2,
            ..Default::default()
        };

        let mut tot = Array2::<f64>::zeros(1, 2);
        tot.set(0, 0, 1.0);
        tot.set(0, 1, 2.0);

        let mut f = Array2::<f64>::zeros(1, 2);
        f.set(0, 0, 0.0);
        f.set(0, 1, 0.5); // thermal fission only

        // s(material, gt, g): within-group plus a 1 -> 2 downscatter.
        let mut s = Array3::<f64>::zeros(1, 2, 2);
        s.set(0, 0, 0, 0.1);
        s.set(0, 1, 1, 0.2);
        s.set(0, 1, 0, 0.3);

        let mut nu = Array2::<f64>::zeros(1, 2);
        nu.set(0, 0, 2.4);
        nu.set(0, 1, 2.5);

        let mut chi = Array2::<f64>::zeros(1, 2);
        chi.set(0, 0, 1.0); // all fission neutrons born fast
        chi.set(0, 1, 0.0);

        let sigmavalues = SigmaValues {
            tot,
            f,
            s,
            nu,
            chi,
            fp: None,
        };

        let mut whichsigma = Array3::<usize>::zeros(1, 1, maxiz);
        for iz in 0..maxiz {
            whichsigma.set(0, 0, iz, 1);
        }
        (params, sigmavalues, whichsigma)
    }

    #[test]
    fn diagonals_carry_total_within_group_scattering_and_bare_fission() {
        let (params, sv, ws) = setup(1);
        let mut sigma = makesigmadfxyz(&params, &sv, &ws, None);

        let tot = sigma.tot.find();
        assert_eq!(tot.len(), 2);
        assert!(tot.iter().any(|t| t.i == 0 && t.j == 0 && t.v == 1.0));
        assert!(tot.iter().any(|t| t.i == 1 && t.j == 1 && t.v == 2.0));

        let sd = sigma.sd.find();
        assert!(sd.iter().any(|t| t.i == 0 && t.j == 0 && t.v == 0.1));
        assert!(sd.iter().any(|t| t.i == 1 && t.j == 1 && t.v == 0.2));

        let fb = sigma.fb.find();
        // Group 1 has zero fission, so only the thermal diagonal survives.
        assert_eq!(fb.len(), 1);
        assert!(fb.iter().any(|t| t.i == 1 && t.j == 1 && t.v == 0.5));
    }

    /// Fission is `chi * nu * Sigma_f`, and lands in the row of the
    /// **destination** group.
    #[test]
    fn fission_operator_applies_chi_and_nu() {
        let (params, sv, ws) = setup(1);
        let mut sigma = makesigmadfxyz(&params, &sv, &ws, None);

        let f = sigma.f.find();
        // Only (g = 1 thermal source) -> (gt = 0 fast destination) is non-zero,
        // since chi(0) = 1 and chi(1) = 0 and only group 1 fissions.
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].i, 0);
        assert_eq!(f[0].j, 1);
        assert_eq!(f[0].v, 1.0 * 2.5 * 0.5);
    }

    /// Scattering is stored destination-first, so the `1 -> 2` downscatter
    /// appears at row 1, column 0.
    #[test]
    fn scattering_is_indexed_destination_first() {
        let (params, sv, ws) = setup(1);
        let mut sigma = makesigmadfxyz(&params, &sv, &ws, None);

        let s = sigma.s.find();
        assert!(s.iter().any(|t| t.i == 1 && t.j == 0 && t.v == 0.3));
    }

    /// Absent `fp` yields an all-zero prompt operator, not a panic.
    #[test]
    fn absent_fp_becomes_zeros() {
        let (params, sv, ws) = setup(1);
        let mut sigma = makesigmadfxyz(&params, &sv, &ws, None);
        assert_eq!(sigma.fp.nnz(), 0);
    }

    /// `sigma.nu` carries the group-1 value at every entry — the reference's
    /// linear indexing of a 2-D `nu`. Group 2's `nu` of 2.5 appears in
    /// `sigma.f` but never in `sigma.nu`.
    #[test]
    fn nu_vector_takes_the_group_one_value_everywhere() {
        let (params, sv, ws) = setup(1);
        let sigma = makesigmadfxyz(&params, &sv, &ws, None);
        assert_eq!(sigma.nu[0], 2.4);
        assert_eq!(sigma.nu[1], 2.4);
    }

    /// Pins the mode-2 axial truncation described in the doc comment.
    ///
    /// With `maxiz = 4`, mode 2 should cover `iz/m = 1..4` but its loop bound
    /// of `maxiz` stops at `iz = 4`, i.e. `iz/m = 1..2`. Half the axial nodes
    /// get no cross sections, so the diagonal has half the entries it should.
    #[test]
    fn half_index_mode_truncates_the_axial_extent() {
        let (params, sv, ws) = setup(4);

        let mut full = makesigmadfxyz(&params, &sv, &ws, Some(SigmaIndexMode::Full));
        // 4 axial nodes x 2 groups.
        assert_eq!(full.tot.nnz(), 8);

        let mut half = makesigmadfxyz(&params, &sv, &ws, Some(SigmaIndexMode::HalfIndex));
        // Should also be 8; the truncated loop delivers 2 nodes x 2 groups.
        assert_eq!(half.tot.nnz(), 4);
    }
}
