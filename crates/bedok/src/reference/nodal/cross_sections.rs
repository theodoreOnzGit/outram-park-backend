//! Cross-section operators and diffusion coefficients.
//!
//! # Provenance
//!
//! Original author: **Than Yan Ren**, Singapore Nuclear Research and Safety
//! Institute (SNRSI). Snapshot `BEDOKfiles.zip`, sha256 `e45cd6f57be2087c`.
//! Translated under the permission recorded in `docs/bedok-port-scoping.md` §6.
//!
//! Sources: `makesigmadfxyz.m` (function `makesigmadfxyz`) and
//! `calcdiffvalues3d.m` (function `calcdiffvalues3d`).

use super::geometry::NodalParams;
use super::sparse::SparseMatrix;
use crate::reference::grid::Grid;

/// Per-material multigroup cross-section data — Yan Ren's `sigmavalues` struct.
///
/// Materials are indexed **0-based here** but referred to by the **1-based**
/// index stored in `which_sigma`, exactly as in the MATLAB, where
/// `whichsigma == 0` means "no material at this node". Convert with `m - 1`.
///
/// # Units
///
/// - `total`, `fission`, `fission_prompt`, `scatter`: macroscopic cross
///   sections \[cm⁻¹\].
/// - `nu`: neutrons per fission \[dimensionless\], typically 2.4–2.9.
/// - `chi`: fission spectrum \[dimensionless\], summing to 1 over groups.
#[derive(Debug, Clone, PartialEq)]
pub struct MaterialCrossSections {
    /// `sigmavalues.tot(m,g)` — total (here: removal-bearing) cross section,
    /// indexed `[material][group]` \[cm⁻¹\].
    pub total: Vec<Vec<f64>>,
    /// `sigmavalues.f(m,g)` — fission cross section, `[material][group]`
    /// \[cm⁻¹\].
    pub fission: Vec<Vec<f64>>,
    /// `sigmavalues.fp(m,g)` — prompt-fission cross section,
    /// `[material][group]` \[cm⁻¹\].
    ///
    /// May be left empty, reproducing the MATLAB's
    /// `if ~isfield(sigmavalues,'fp'), sigmavalues.fp=zeros(...)` default.
    pub fission_prompt: Vec<Vec<f64>>,
    /// `sigmavalues.s(m,gt,g)` — scattering from group `g` **into** group `gt`,
    /// indexed `[material][to_group][from_group]` \[cm⁻¹\].
    pub scatter: Vec<Vec<Vec<f64>>>,
    /// `sigmavalues.nu(m,g)` — neutrons per fission, `[material][group]`
    /// \[dimensionless\].
    ///
    /// # Note on the MATLAB's two ways of reading this
    ///
    /// `makesigmadfxyz.m` reads `nu` twice with different index counts:
    /// `nu(whichsigma(...))` (a *linear* index) when filling `sigma.nu`, and
    /// `nu(whichsigma(...),g)` (a 2-D index) when filling `sigma.f`. Under
    /// MATLAB's column-major linear indexing the first resolves to
    /// `nu(material, 1)`, so the port uses `nu[m][0]` there and `nu[m][g]` in
    /// the fission operator. Both readings are reproduced, not reconciled.
    pub nu: Vec<Vec<f64>>,
    /// `sigmavalues.chi(m,g)` — fission spectrum, `[material][group]`
    /// \[dimensionless\].
    pub chi: Vec<Vec<f64>>,
}

impl MaterialCrossSections {
    /// Prompt-fission cross section of material `m` (0-based) in group `g`,
    /// returning `0.0` when the optional `fission_prompt` table is absent.
    #[must_use]
    pub fn prompt_fission(&self, m: usize, g: usize) -> f64 {
        if self.fission_prompt.is_empty() {
            0.0
        } else {
            self.fission_prompt[m][g]
        }
    }
}

/// The assembled cross-section operators — Yan Ren's `sigma` struct.
///
/// Every matrix is `philenf` × `philenf` (see
/// [`NodalParams::philenf`](super::geometry::NodalParams::philenf)) and carries
/// units of cm⁻¹. Rows and columns are state-vector indices, so a scattering
/// entry at `(idx_to, idx_from)` couples two energy groups at the *same*
/// spatial node — every operator here is block-diagonal in space.
#[derive(Debug, Clone, PartialEq)]
pub struct CrossSectionOperators {
    /// `sigma.tot` — diagonal total cross section \[cm⁻¹\].
    pub total: SparseMatrix,
    /// `sigma.f` — the full fission-production operator
    /// `chi(gt) * nu(g) * sigma_f(g)` \[cm⁻¹\], mapping the flux in group `g`
    /// to production in group `gt`.
    pub fission: SparseMatrix,
    /// `sigma.fp` — the same operator built from the *prompt* fission cross
    /// section, `chi(gt) * sigma_fp(g)` \[cm⁻¹\].
    ///
    /// # Unfinished in the reference
    ///
    /// Note the asymmetry with `fission`: `sigma.fp` omits the `nu` factor that
    /// `sigma.f` includes. Whether `sigmavalues.fp` is meant to already contain
    /// `nu * beta` is not stated anywhere in the snapshot. Translated as
    /// written.
    pub fission_prompt: SparseMatrix,
    /// `sigma.fb` — the "bare" diagonal fission cross section `sigma_f(g)`,
    /// with no `nu` and no `chi` \[cm⁻¹\].
    pub fission_bare: SparseMatrix,
    /// `sigma.s` — the full group-to-group scattering operator \[cm⁻¹\],
    /// including within-group scattering on the diagonal.
    pub scatter: SparseMatrix,
    /// `sigma.sd` — only the within-group (diagonal) part of `scatter`
    /// \[cm⁻¹\].
    pub scatter_self: SparseMatrix,
    /// `sigma.nu` — neutrons per fission at each state index
    /// \[dimensionless\]; zero outside the core.
    pub nu: Vec<f64>,
    /// `sigma.chi` — the fission spectrum as a `G` × `philen` table flattened
    /// row-major, so entry `(gt, idx)` lives at `gt * philen + idx`
    /// \[dimensionless\].
    ///
    /// Note this is `philen`-wide, not `philenf`-wide, exactly as the MATLAB's
    /// `schi=zeros(G,philen)`.
    pub chi: Vec<f64>,
}

/// Assembles the cross-section operators — `makesigmadfxyz.m`, `mode == 1`.
///
/// Walks every node with a nonzero material index and, for each energy group,
/// writes the diagonal total/fission/self-scatter entries and the off-diagonal
/// fission-production and scattering entries.
///
/// `which_sigma` is the **1-based** material index per spatial node, indexed by
/// `ix*ny*nz + iy*nz + iz`; `0` means the node is outside the core and is
/// skipped entirely, leaving every operator zero there.
///
/// # Mode 2 is not ported
///
/// The MATLAB supports a second index convention (`mode == 2`) that carries
/// half-indices for a 2-D layout. Every call site in the SANM path passes
/// `mode = 1`, so only that is translated. Recording the reason it is not worth
/// resurrecting: mode 2's node loop reads
/// `for ix=m:m:m*maxix, for iy=m:m:m*maxiy, for iz=m:m:maxiz` — the `iz` bound
/// is missing its `m*` factor, so with `m = 2` the loop covers only the first
/// `maxiz/2` axial nodes. That is a defect in the reference, left unfixed here
/// per the translation rules.
///
/// # Panics
///
/// If `which_sigma.len()` differs from the node count, or a material index
/// exceeds the supplied cross-section tables.
#[must_use]
pub fn assemble_operators(
    params: &NodalParams,
    values: &MaterialCrossSections,
    which_sigma: &[usize],
) -> CrossSectionOperators {
    let grid = params.grid;
    let ngroups = grid.ngroups;
    let nodes = grid.nodes();
    let philen = params.philen();
    let philenf = params.philenf();
    assert_eq!(which_sigma.len(), nodes, "which_sigma length");

    let mut total = Vec::new();
    let mut fission = Vec::new();
    let mut fission_prompt = Vec::new();
    let mut fission_bare = Vec::new();
    let mut scatter = Vec::new();
    let mut scatter_self = Vec::new();
    let mut nu = vec![0.0; philen];
    let mut chi = vec![0.0; ngroups * philen];

    for ix in 0..grid.nx {
        for iy in 0..grid.ny {
            for iz in 0..grid.nz {
                let node = ix * (grid.ny * grid.nz) + iy * grid.nz + iz;
                let material = which_sigma[node];
                if material == 0 {
                    continue;
                }
                let m = material - 1;
                for g in 0..ngroups {
                    let idx = grid.index(g, ix, iy, iz);
                    // sigma.nu uses the *linear* MATLAB index nu(material),
                    // which is column 1 of a (material, group) table.
                    nu[idx] = values.nu[m][0];
                    total.push((idx, idx, values.total[m][g]));
                    fission_bare.push((idx, idx, values.fission[m][g]));
                    scatter_self.push((idx, idx, values.scatter[m][g][g]));

                    for gt in 0..ngroups {
                        chi[gt * philen + idx] = values.chi[m][gt];
                        if values.fission[m][g] != 0.0 && values.chi[m][gt] != 0.0 {
                            let idx_to = grid.index(gt, ix, iy, iz);
                            fission.push((
                                idx_to,
                                idx,
                                values.chi[m][gt] * values.nu[m][g] * values.fission[m][g],
                            ));
                            fission_prompt.push((
                                idx_to,
                                idx,
                                values.chi[m][gt] * values.prompt_fission(m, g),
                            ));
                        }
                    }

                    for gt in 0..ngroups {
                        if values.scatter[m][gt][g] != 0.0 {
                            let idx_to = grid.index(gt, ix, iy, iz);
                            scatter.push((idx_to, idx, values.scatter[m][gt][g]));
                        }
                    }
                }
            }
        }
    }

    CrossSectionOperators {
        total: SparseMatrix::from_triplets(philenf, philenf, &total),
        fission: SparseMatrix::from_triplets(philenf, philenf, &fission),
        fission_prompt: SparseMatrix::from_triplets(philenf, philenf, &fission_prompt),
        fission_bare: SparseMatrix::from_triplets(philenf, philenf, &fission_bare),
        scatter: SparseMatrix::from_triplets(philenf, philenf, &scatter),
        scatter_self: SparseMatrix::from_triplets(philenf, philenf, &scatter_self),
        nu,
        chi,
    }
}

/// Diffusion coefficients per node and group \[cm\] — `calcdiffvalues3d.m`.
///
/// Computes `D = mode / ((2*mode + 1) * sigma_tot)`. With the default
/// `mode = 1` this is the standard `D = 1/(3 sigma_tr)` of P1 diffusion theory,
/// with `sigmavalues.tot` playing the part of the transport cross section;
/// higher `mode` values reproduce the alternative definitions the MATLAB
/// comment alludes to ("diffusion coefficients based on different
/// definitions") without documenting them further.
///
/// Nodes with `which_sigma == 0` are left at exactly `0.0`. Downstream code
/// keys "is this node in the core?" off `D == 0`, so that zero is load-bearing,
/// not just an initialiser.
///
/// The result is a flat state vector of length `G * nodes`, indexed by
/// [`Grid::index`] — the same layout the MATLAB reaches by
/// `reshape(permute(diffvalues,[3 2 1 4]), philen, 1)`.
///
/// # Valid ranges
///
/// `sigma_tot` must be strictly positive for any in-core material; a zero
/// entry yields an infinite `D`, which the MATLAB also produces and does not
/// guard against.
///
/// # Panics
///
/// If `which_sigma.len()` differs from `grid.nodes()`.
#[must_use]
pub fn diffusion_coefficients(
    grid: Grid,
    material_total: &[Vec<f64>],
    which_sigma: &[usize],
    mode: f64,
) -> Vec<f64> {
    assert_eq!(which_sigma.len(), grid.nodes(), "which_sigma length");
    let mut d = vec![0.0; grid.state_len()];
    for ix in 0..grid.nx {
        for iy in 0..grid.ny {
            for iz in 0..grid.nz {
                let node = ix * (grid.ny * grid.nz) + iy * grid.nz + iz;
                if which_sigma[node] == 0 {
                    continue;
                }
                let m = which_sigma[node] - 1;
                for g in 0..grid.ngroups {
                    d[grid.index(g, ix, iy, iz)] =
                        mode / ((2.0 * mode + 1.0) * material_total[m][g]);
                }
            }
        }
    }
    d
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reference::nodal::geometry::NodalParams;

    /// Two materials, two groups, no upscatter — the shape of every benchmark
    /// in the snapshot.
    fn two_group_data() -> MaterialCrossSections {
        MaterialCrossSections {
            total: vec![vec![0.2, 0.8], vec![0.25, 1.0]],
            fission: vec![vec![0.0, 0.1], vec![0.0, 0.0]],
            fission_prompt: Vec::new(),
            // scatter[m][to][from]
            scatter: vec![
                vec![vec![0.15, 0.0], vec![0.02, 0.7]],
                vec![vec![0.2, 0.0], vec![0.04, 0.9]],
            ],
            nu: vec![vec![2.5, 2.5], vec![0.0, 0.0]],
            chi: vec![vec![1.0, 0.0], vec![1.0, 0.0]],
        }
    }

    #[test]
    fn diffusion_coefficient_is_one_third_over_sigma_t_in_default_mode() {
        let grid = Grid::new(2, 1, 1, 2).expect("valid grid");
        let d = diffusion_coefficients(grid, &two_group_data().total, &[1, 0], 1.0);
        // node 0, material 1: 1/(3*0.2), 1/(3*0.8)
        assert!((d[grid.index(0, 0, 0, 0)] - 1.0 / 0.6).abs() < 1e-14);
        assert!((d[grid.index(1, 0, 0, 0)] - 1.0 / 2.4).abs() < 1e-14);
        // node 1 has no material: exactly zero, in both groups.
        assert_eq!(d[grid.index(0, 1, 0, 0)], 0.0);
        assert_eq!(d[grid.index(1, 1, 0, 0)], 0.0);
    }

    #[test]
    fn fission_operator_places_production_in_the_chi_group() {
        let grid = Grid::new(1, 1, 1, 2).expect("valid grid");
        let params = NodalParams::with_matlab_defaults(grid);
        let ops = assemble_operators(&params, &two_group_data(), &[1]);
        let fast = grid.index(0, 0, 0, 0);
        let thermal = grid.index(1, 0, 0, 0);
        // chi = (1,0): all production lands in the fast group, sourced from the
        // thermal group where sigma_f is nonzero.
        assert!((ops.fission.get(fast, thermal) - 1.0 * 2.5 * 0.1).abs() < 1e-14);
        assert_eq!(ops.fission.get(thermal, thermal), 0.0);
        assert_eq!(ops.fission.get(fast, fast), 0.0);
    }

    #[test]
    fn scatter_operator_is_indexed_to_group_by_from_group() {
        let grid = Grid::new(1, 1, 1, 2).expect("valid grid");
        let params = NodalParams::with_matlab_defaults(grid);
        let ops = assemble_operators(&params, &two_group_data(), &[1]);
        let fast = grid.index(0, 0, 0, 0);
        let thermal = grid.index(1, 0, 0, 0);
        // s[0][1][0] = 0.02 is fast -> thermal downscatter.
        assert_eq!(ops.scatter.get(thermal, fast), 0.02);
        assert_eq!(ops.scatter.get(fast, thermal), 0.0);
        // The self-scatter operator holds only the diagonal.
        assert_eq!(ops.scatter_self.get(fast, fast), 0.15);
        assert_eq!(ops.scatter_self.get(thermal, fast), 0.0);
    }

    #[test]
    fn nodes_without_material_stay_empty() {
        let grid = Grid::new(2, 1, 1, 2).expect("valid grid");
        let params = NodalParams::with_matlab_defaults(grid);
        let ops = assemble_operators(&params, &two_group_data(), &[1, 0]);
        let empty = grid.index(0, 1, 0, 0);
        assert_eq!(ops.total.get(empty, empty), 0.0);
        assert_eq!(ops.nu[empty], 0.0);
    }

    #[test]
    fn prompt_fission_defaults_to_zero_when_the_table_is_absent() {
        let grid = Grid::new(1, 1, 1, 2).expect("valid grid");
        let params = NodalParams::with_matlab_defaults(grid);
        let ops = assemble_operators(&params, &two_group_data(), &[1]);
        assert_eq!(ops.fission_prompt.diagonal().iter().sum::<f64>(), 0.0);
    }
}
