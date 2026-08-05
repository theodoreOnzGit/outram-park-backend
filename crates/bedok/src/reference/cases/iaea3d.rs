//! IAEA-3D PWR benchmark — the two-group, no-feedback steady case.
//!
//! # Provenance
//!
//! | | |
//! |---|---|
//! | Original author | Than Yan Ren, Singapore Nuclear Research and Safety Institute (SNRSI) |
//! | Source file | `iaea3ds.m` (function `iaea3ds`) |
//! | Snapshot | `BEDOKfiles.zip`, sha256 `e45cd6f57be2087c…` |
//! | Permission | translation and open-source publication approved by the author and the project lead; see `docs/bedok-port-scoping.md` §6 |
//!
//! # The case
//!
//! The IAEA 3-D PWR benchmark: a quarter core, 17 × 17 radial nodes of 10 cm
//! and 19 axial nodes of 20 cm, two energy groups, five materials, and **no
//! thermal-hydraulic feedback of any kind** — no boron, no Doppler, no
//! moderator density. That makes it the cleanest possible exercise of the
//! neutronics path alone, which is why it is the first case ported.
//!
//! Reference eigenvalues quoted in the source header: PARCS `1.029096`,
//! ADPRES `1.029082`.

use crate::error::Result;
use crate::reference::grid::{Geometry, Grid};

use super::csv_maps::CompositionMap;
use super::geometry::{
    geometry_ends_3d, matlab_int64_scale, Boundaries, Boundary, CaseGeometry, GridScale,
};
use super::params::CaseParams;
use super::sigmas::{CaseConstants, SigmaSet, SigmaValues};
use super::BuiltCase;

/// Radial nodes in the native mesh.
const NATIVE_NX: usize = 17;
/// Axial nodes in the native mesh — **19, not the 18 the driver asks for.**
const NATIVE_NZ: usize = 19;
/// Number of materials in the cross-section tables.
const MATERIALS: usize = 5;

/// Build the IAEA-3D case.
///
/// Rust translation of `iaea3ds.m`.
///
/// # The grid is overwritten, and that matters
///
/// The first three statements of `iaea3ds.m` are
///
/// ```text
/// params.maxix = 17;
/// params.maxiy = 17;
/// params.maxiz = 19;
/// ```
///
/// **unconditionally**, discarding whatever the caller asked for.
/// `main_exec_diff3d.m` requests `maxiz = 18`; this case runs on 19 axial
/// nodes, the extra one being the top axial reflector plane read from
/// `IAEA3DS_4.csv`. The returned grid is therefore 17 × 17 × 19 = 5,491 nodes,
/// 10,982 state entries at two groups — always read the grid back from the
/// returned [`CaseParams`], never from the request.
///
/// A side effect of the same forcing: `xscale`, `yscale` and `zscale` are
/// computed *after* it, so they are always 1 and the mesh-refinement machinery
/// (`ceil(ix/maxix*17)` sampling of the composition maps) is dead code here.
/// It is ported anyway, because the NEACRP cases do not force the grid and
/// genuinely use it.
///
/// # Materials
///
/// | Index | Material |
/// |---|---|
/// | 1 | Outer fuel |
/// | 2 | Inner fuel |
/// | 3 | Inner fuel + control rod |
/// | 4 | Reflector |
/// | 5 | Reflector + control rod |
///
/// # Axial layout
///
/// | Axial nodes (1-based) | Composition map |
/// |---|---|
/// | 1 | `IAEA3DS_1` — bottom axial reflector |
/// | 2 … 14 | `IAEA3DS_2` — lower fuel |
/// | 15 … 18 | `IAEA3DS_3` — upper fuel, rodded region |
/// | 19 | `IAEA3DS_4` — top axial reflector |
///
/// # Cross sections
///
/// Given directly as `tot` (total/removal), `f` (`nu*Sigma_f`) and `s`
/// (scattering). `sigmavalues.a` and `sigmavalues.fp` are **not defined** —
/// `makesigmadfxyz.m` substitutes zeros for `fp`, and absorption is implicit
/// in the total minus the scattering rows. Units \[1/cm\].
///
/// # Errors
///
/// Propagates CSV-parse and grid-construction failures. In practice these can
/// only fire if the embedded composition maps are corrupted.
pub fn iaea_3d(input: &CaseParams) -> Result<BuiltCase> {
    let ngroups = 2;
    let grid = Grid::new(NATIVE_NX, NATIVE_NX, NATIVE_NZ, ngroups)?;

    let scale = GridScale {
        x: matlab_int64_scale(grid.nx, NATIVE_NX, grid)?,
        y: matlab_int64_scale(grid.ny, NATIVE_NX, grid)?,
        z: matlab_int64_scale(grid.nz, NATIVE_NZ, grid)?,
    };

    // ----- reactor dimensions [cm] -----
    let x_total = 170.0;
    let y_total = 170.0;
    let z_total = 380.0;
    let step_x = x_total / grid.nx as f64;
    let step_y = y_total / grid.ny as f64;
    let step_z = z_total / grid.nz as f64;

    let nodes = grid.nodes();
    let lx = vec![step_x; nodes];
    let ly = vec![step_y; nodes];
    let lz = vec![step_z; nodes];
    let volume = vec![step_x * step_y * step_z; nodes];

    let mut centers = vec![[0.0f64; 3]; nodes];
    for ix in 0..grid.nx {
        for iy in 0..grid.ny {
            for iz in 0..grid.nz {
                centers[grid.index(0, ix, iy, iz)] = [
                    (ix as f64 + 0.5) * step_x,
                    (iy as f64 + 0.5) * step_y,
                    (iz as f64 + 0.5) * step_z,
                ];
            }
        }
    }

    let boundaries = Boundaries {
        x_min: Boundary::Reflective,
        x_max: Boundary::Vacuum,
        y_min: Boundary::Reflective,
        y_max: Boundary::Vacuum,
        z_min: Boundary::Vacuum,
        z_max: Boundary::Vacuum,
    };

    // ----- cross sections [1/cm] -----
    let mut base = SigmaSet::zeros(MATERIALS, ngroups);
    // sigmavalues.tot = [1/4.5 1/1.2; 1/4.5 1/1.2; 1/4.5 1/1.2; 1/6 1/0.9; 1/6 1/0.9]
    for m in 0..3 {
        base.total[m] = vec![1.0 / 4.5, 1.0 / 1.2];
    }
    for m in 3..5 {
        base.total[m] = vec![1.0 / 6.0, 1.0 / 0.9];
    }
    // sigmavalues.f = [0 0.135; 0 0.135; 0 0.135; 0 0; 0 0]
    for m in 0..3 {
        base.nu_fission[m] = vec![0.0, 0.135];
    }
    // Materials 4 and 5 (the reflectors) are already zero.

    // sigmavalues.s(m,:,:) = [ s(1,1) s(1,2); s(2,1) s(2,2) ]
    base.scatter
        .set_block_2x2(0, [[1.0 / 4.5 - 0.03, 0.0], [0.020, 1.0 / 1.2 - 0.08]]);
    base.scatter
        .set_block_2x2(1, [[1.0 / 4.5 - 0.03, 0.0], [0.020, 1.0 / 1.2 - 0.085]]);
    base.scatter
        .set_block_2x2(2, [[1.0 / 4.5 - 0.03, 0.0], [0.020, 1.0 / 1.2 - 0.13]]);
    base.scatter
        .set_block_2x2(3, [[1.0 / 6.0 - 0.04, 0.0], [0.040, 1.0 / 0.9 - 0.01]]);
    base.scatter
        .set_block_2x2(4, [[1.0 / 6.0 - 0.04, 0.0], [0.040, 1.0 / 0.9 - 0.055]]);

    // `iaea3ds.m` defines neither sigmavalues.a nor sigmavalues.fp. An empty
    // Vec is how an absent MATLAB field is represented here.
    base.absorption.clear();
    base.kappa_fission.clear();

    let constants = CaseConstants::fast_group_birth(MATERIALS, ngroups, Some(1.0));
    let sigmas = SigmaValues {
        base,
        nu: constants.nu.clone(),
        chi: constants.chi.clone(),
        boron: None,
        fuel_temperature: None,
        coolant_temperature: None,
        coolant_density: None,
        control_rod: None,
    };

    // ----- material map -----
    let mut which_sigma = vec![0usize; nodes];
    let bands: [(usize, usize, CompositionMap); 4] = [
        (1, scale.z, CompositionMap::Iaea3dsBottomReflector),
        (scale.z + 1, 14 * scale.z, CompositionMap::Iaea3dsLowerFuel),
        (
            14 * scale.z + 1,
            18 * scale.z,
            CompositionMap::Iaea3dsUpperFuel,
        ),
        (
            18 * scale.z + 1,
            19 * scale.z,
            CompositionMap::Iaea3dsTopReflector,
        ),
    ];
    for (iz_first, iz_last, map) in bands {
        let data = map.load()?;
        for iz in iz_first..=iz_last.min(grid.nz) {
            for ix in 1..=grid.nx {
                for iy in 1..=grid.ny {
                    // whichdata(ceil(ix/maxix*17), ceil(iy/maxiy*17))
                    let row = sample_index(ix, grid.nx);
                    let col = sample_index(iy, grid.ny);
                    which_sigma[grid.index(0, ix - 1, iy - 1, iz - 1)] =
                        data.index_at_matlab(row, col)?;
                }
            }
        }
    }

    let ends = geometry_ends_3d(grid, &which_sigma)?;

    let geometry = CaseGeometry {
        base: Geometry {
            grid,
            x_total,
            y_total,
            z_total,
            lx,
            ly,
            lz,
            volume,
            which_sigma,
        },
        scale,
        centers,
        boundaries,
        ends: Some(ends),
        fuel: None,
        control_rods: None,
    };

    let params = CaseParams {
        grid,
        num_extra_unknowns: 0,
        prompt_fraction: Some(1.0),
        ..input.clone()
    };

    Ok(BuiltCase {
        params,
        geometry,
        constants,
        sigmas,
        th: None,
    })
}

/// The composition-map sampling index MATLAB writes as
/// `ceil(ix/maxix*17)`, with `ix` 1-based.
///
/// On the native 17-node mesh this is the identity; on a refined mesh it maps
/// each group of `scale` nodes onto one map entry.
fn sample_index(one_based: usize, max_index: usize) -> usize {
    ((one_based as f64) / (max_index as f64) * (NATIVE_NX as f64)).ceil() as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    fn built() -> BuiltCase {
        iaea_3d(&CaseParams::main_exec_defaults()).expect("IAEA-3D builds")
    }

    /// The headline invariant: the case overrides the requested 18 axial nodes
    /// with 19, giving 5,491 nodes and a 10,982-entry state vector.
    #[test]
    fn grid_is_seventeen_by_seventeen_by_nineteen() {
        let input = CaseParams::main_exec_defaults();
        assert_eq!(input.grid.nz, 18, "the driver asks for 18");

        let case = iaea_3d(&input).expect("builds");
        assert_eq!(case.params.grid.nx, 17);
        assert_eq!(case.params.grid.ny, 17);
        assert_eq!(case.params.grid.nz, 19, "the case forces 19");
        assert_eq!(case.params.grid.ngroups, 2);
        assert_eq!(case.grid().nodes(), 5_491);
        assert_eq!(case.grid().state_len(), 10_982);
        // The geometry agrees with the params.
        assert_eq!(case.geometry.base.grid, case.params.grid);
    }

    #[test]
    fn node_dimensions_are_ten_by_ten_by_twenty_centimetres() {
        let case = built();
        assert_eq!(case.geometry.base.x_total, 170.0);
        assert_eq!(case.geometry.base.z_total, 380.0);
        assert!((case.geometry.base.lx[0] - 10.0).abs() < 1e-12);
        assert!((case.geometry.base.ly[0] - 10.0).abs() < 1e-12);
        assert!((case.geometry.base.lz[0] - 20.0).abs() < 1e-12);
        assert!((case.geometry.base.volume[0] - 2000.0).abs() < 1e-9);
        assert_eq!(case.geometry.base.volume.len(), 5_491);
        // Centres are at the node midpoints.
        let grid = case.grid();
        assert_eq!(
            case.geometry.centers[grid.index(0, 0, 0, 0)],
            [5.0, 5.0, 10.0]
        );
    }

    /// Axial banding: node 1 and node 19 are reflector planes, the middle is
    /// fuel. Checked at the core centre, where the map holds a rodded inner
    /// fuel assembly.
    #[test]
    fn axial_bands_come_from_the_right_maps() {
        let case = built();
        let grid = case.grid();
        let at = |iz: usize| case.geometry.base.which_sigma[grid.index(0, 0, 0, iz)];
        assert_eq!(at(0), 4, "bottom axial reflector (IAEA3DS_1)");
        assert_eq!(at(1), 3, "lower fuel, rodded inner fuel (IAEA3DS_2)");
        assert_eq!(at(13), 3, "still lower fuel at iz=14");
        assert_eq!(at(14), 3, "upper fuel (IAEA3DS_3)");
        assert_eq!(at(18), 5, "top axial reflector, rodded (IAEA3DS_4)");
    }

    /// The core outline: the far corner of the quadrant is outside the core.
    #[test]
    fn the_outer_corner_is_void() {
        let case = built();
        let grid = case.grid();
        assert_eq!(
            case.geometry.base.which_sigma[grid.index(0, 16, 16, 9)],
            0,
            "corner assembly is outside the modelled core"
        );
        // The ends therefore stop short of the last node on that line.
        let ends = case.geometry.ends.as_ref().expect("ends are built");
        assert!(ends.x_high(16, 9) < 16);
    }

    #[test]
    fn cross_sections_match_the_source_tables() {
        let case = built();
        let s = &case.sigmas.base;
        assert_eq!(s.materials(), 5);
        assert!((s.total[0][0] - 1.0 / 4.5).abs() < 1e-15);
        assert!((s.total[3][1] - 1.0 / 0.9).abs() < 1e-15);
        assert_eq!(s.nu_fission[1][1], 0.135);
        assert_eq!(s.nu_fission[3][1], 0.0, "reflector does not fission");
        // Down-scatter from group 1 into group 2.
        assert_eq!(s.scatter.get(0, 1, 0), 0.020);
        assert_eq!(s.scatter.get(3, 1, 0), 0.040);
        assert_eq!(s.scatter.get(0, 0, 1), 0.0, "no up-scatter");
        // Absorption and kappa-fission are absent, as in the MATLAB.
        assert!(s.absorption.is_empty());
        assert!(s.kappa_fission.is_empty());
        // Every fission neutron is born fast.
        assert_eq!(case.constants.chi[0], vec![1.0, 0.0]);
        assert_eq!(case.constants.frac_p, Some(1.0));
        assert_eq!(case.params.prompt_fraction, Some(1.0));
    }

    #[test]
    fn there_is_no_feedback_and_no_thermal_hydraulics() {
        let case = built();
        assert!(case.th.is_none());
        assert!(case.sigmas.boron.is_none());
        assert!(case.sigmas.fuel_temperature.is_none());
        assert!(case.sigmas.coolant_density.is_none());
        assert!(case.sigmas.control_rod.is_none());
        assert!(case.geometry.fuel.is_none());
        assert!(case.params.transient.is_none());
    }

    #[test]
    fn refinement_sampling_is_the_identity_on_the_native_mesh() {
        assert_eq!(sample_index(1, 17), 1);
        assert_eq!(sample_index(17, 17), 17);
        // On a doubled mesh two nodes share one map entry.
        assert_eq!(sample_index(1, 34), 1);
        assert_eq!(sample_index(2, 34), 1);
        assert_eq!(sample_index(3, 34), 2);
    }
}
