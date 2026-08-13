//! A one-group 2-D x–y test case: a square of UO₂ in a moderator box.
//!
//! # Provenance
//!
//! | | |
//! |---|---|
//! | Original author | Than Yan Ren, Singapore Nuclear Research and Safety Institute (SNRSI) |
//! | Source file | `geom2dxycase1.m` |
//! | Snapshot | `BEDOKfiles.zip`, sha256 `e45cd6f57be2087c…` |
//! | Permission | translation and open-source publication approved by the author and the project lead; see `docs/bedok-port-scoping.md` §6 |
//!
//! # What it is for
//!
//! Not a benchmark — a smoke test. One energy group, two materials, no
//! feedback, no thermal hydraulics, and a stated answer (`k_eff = 0.487` at
//! the default dimensions) so a broken solver shows up immediately. It is the
//! only 2-D case in the snapshot; `main_exec_diff3d.m` keeps its call
//! commented out.
//!
//! # Two ways it differs from every 3-D case
//!
//! - **Different boundary field names.** It sets `geometry.left`, `.right`,
//!   `.top`, `.bottom` instead of `.xmin` … `.zmax`. They are mapped here as
//!   left → `x_min`, right → `x_max`, bottom → `y_min`, top → `y_max`, with
//!   the two z faces given the same condition. The 2-D `geometry` is not
//!   interchangeable with a 3-D one in the MATLAB either.
//! - **No `geometry_ends3d` call**, so there are no fuelled-extent arrays.
//!   The solvers test `isfield(geometry,'xlows')` and fall back to the full
//!   range, which is correct here because no node is void.
//!
//! # Representation as a degenerate 3-D grid
//!
//! [`Grid`] has no 2-D form, so the case is
//! built with `nz = 1`. `z_total` is set to `0.0` because the MATLAB defines
//! no `geometry.Ztot`; node volumes are consequently **areas** \[cm²\], which
//! is exactly what `geometry.Vi` holds in the 2-D MATLAB (its own comment
//! calls it "area of each cell").

use crate::error::Result;
use crate::reference::grid::{Geometry, Grid};

use super::geometry::{Boundaries, Boundary, CaseGeometry, GridScale};
use super::params::CaseParams;
use super::sigmas::{CaseConstants, SigmaSet, SigmaValues};
use super::BuiltCase;

/// Half-width of the fuel square \[cm\]. MATLAB `Lux`, `Luy`.
const FUEL_HALF_WIDTH_CM: f64 = 8.0;
/// Moderator thickness on each side \[cm\]. MATLAB `Lpx`, `Lpy`.
const MODERATOR_THICKNESS_CM: f64 = 8.0;
/// Total macroscopic cross section of both materials \[1/cm\]. MATLAB
/// `usigmat`, `psigmat` — the same value for fuel and moderator.
const TOTAL_SIGMA: f64 = 5.0;

/// Build the 2-D x–y smoke-test case.
///
/// Rust translation of `geom2dxycase1.m`.
///
/// A `Lux` × `Luy` = 8 × 8 cm square of UO₂ centred in a 24 × 24 cm box of
/// moderator, one energy group, vacuum on all four sides. The source header
/// states `k_eff = 0.487` at these dimensions.
///
/// # Cross sections
///
/// Both materials have `Sigma_t = 5 /cm` and `Sigma_s = 0.9 * Sigma_t`; the
/// fuel additionally has `nu*Sigma_f = 0.05 * Sigma_t`. Absorption is
/// implicit in the total, as in `iaea3ds.m`.
///
/// # Questionable in the reference
///
/// `constants.chi = [1; 1]` is a **column** of length 2 — one entry per
/// *material* — where the 3-D cases build a `materials × G` matrix. With
/// `G = 1` the two readings coincide numerically, so nothing is wrong at
/// runtime; it is reproduced here as the `materials × G` form the rest of the
/// code expects, and flagged because a second energy group would break it.
///
/// The file also writes `params.nu`, `params.chi` and `params.frac_p`
/// alongside the `constants` struct — fields no other case sets and no solver
/// reads. Recorded, not carried into [`CaseParams`].
///
/// # Errors
///
/// [`crate::error::BedokError::EmptyGrid`] if the requested radial node counts
/// are zero.
pub fn geom2d_xy_case1(input: &CaseParams) -> Result<BuiltCase> {
    let ngroups = 1;
    let grid = Grid::new(input.grid.nx, input.grid.ny, 1, ngroups)?;

    let x_total = FUEL_HALF_WIDTH_CM + 2.0 * MODERATOR_THICKNESS_CM;
    let y_total = FUEL_HALF_WIDTH_CM + 2.0 * MODERATOR_THICKNESS_CM;
    let step_x = x_total / grid.nx as f64;
    let step_y = y_total / grid.ny as f64;

    let nodes = grid.nodes();
    let lx = vec![step_x; nodes];
    let ly = vec![step_y; nodes];
    // The MATLAB defines no Lz for a 2-D case; Vi is an area.
    let lz = vec![0.0; nodes];
    let volume = vec![step_x * step_y; nodes];

    let mut centers = vec![[0.0f64; 3]; nodes];
    for ix in 0..grid.nx {
        for iy in 0..grid.ny {
            centers[grid.index(0, ix, iy, 0)] =
                [(ix as f64 + 0.5) * step_x, (iy as f64 + 0.5) * step_y, 0.0];
        }
    }

    let boundaries = Boundaries {
        x_min: Boundary::Vacuum,
        x_max: Boundary::Vacuum,
        y_min: Boundary::Vacuum,
        y_max: Boundary::Vacuum,
        z_min: Boundary::Vacuum,
        z_max: Boundary::Vacuum,
    };

    // ----- cross sections [1/cm] -----
    let mut base = SigmaSet::zeros(2, ngroups);
    base.total[0] = vec![TOTAL_SIGMA];
    base.total[1] = vec![TOTAL_SIGMA];
    base.nu_fission[0] = vec![0.05 * TOTAL_SIGMA];
    base.nu_fission[1] = vec![0.0];
    base.scatter.set(0, 0, 0, 0.9 * TOTAL_SIGMA);
    base.scatter.set(1, 0, 0, 0.9 * TOTAL_SIGMA);
    // No sigmavalues.a and no sigmavalues.fp, as in the MATLAB.
    base.absorption.clear();
    base.kappa_fission.clear();

    let constants = CaseConstants::fast_group_birth(2, ngroups, Some(1.0));
    let sigmas = SigmaValues {
        nu: constants.nu.clone(),
        chi: constants.chi.clone(),
        boron: None,
        fuel_temperature: None,
        coolant_temperature: None,
        coolant_density: None,
        control_rod: None,
        base,
    };

    // ----- material map: 1 = fuel inside the square, 2 = moderator -----
    let mut which_sigma = vec![1usize; nodes];
    for ix in 0..grid.nx {
        for iy in 0..grid.ny {
            let cx = (ix as f64 + 0.5) * step_x;
            let cy = (iy as f64 + 0.5) * step_y;
            let inside_fuel = cx >= MODERATOR_THICKNESS_CM
                && MODERATOR_THICKNESS_CM + FUEL_HALF_WIDTH_CM >= cx
                && cy >= MODERATOR_THICKNESS_CM
                && MODERATOR_THICKNESS_CM + FUEL_HALF_WIDTH_CM >= cy;
            if !inside_fuel {
                which_sigma[grid.index(0, ix, iy, 0)] = 2;
            }
        }
    }

    let geometry = CaseGeometry {
        base: Geometry {
            grid,
            x_total,
            y_total,
            // No geometry.Ztot in the 2-D case.
            z_total: 0.0,
            lx,
            ly,
            lz,
            volume,
            which_sigma,
        },
        scale: GridScale { x: 1, y: 1, z: 1 },
        centers,
        boundaries,
        // geom2dxycase1.m does not call geometry_ends3d.
        ends: None,
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

#[cfg(test)]
mod tests {
    use super::*;

    fn built(nx: usize, ny: usize) -> BuiltCase {
        let mut input = CaseParams::main_exec_defaults();
        input.grid = Grid::new(nx, ny, 1, 1).expect("valid grid");
        geom2d_xy_case1(&input).expect("2-D case builds")
    }

    #[test]
    fn the_box_is_twenty_four_centimetres_across() {
        let case = built(24, 24);
        assert_eq!(case.geometry.base.x_total, 24.0);
        assert_eq!(case.geometry.base.y_total, 24.0);
        assert_eq!(case.grid().nz, 1, "represented as a single-plane 3-D grid");
        assert_eq!(case.grid().ngroups, 1);
        assert!((case.geometry.base.lx[0] - 1.0).abs() < 1e-12);
        // Vi is an area in the 2-D case.
        assert!((case.geometry.base.volume[0] - 1.0).abs() < 1e-12);
    }

    /// The fuel square occupies the middle third in each direction.
    #[test]
    fn fuel_sits_in_the_middle_and_moderator_around_it() {
        let case = built(24, 24);
        // Node centres at 8.5 .. 15.5 cm are inside [8, 16].
        assert_eq!(case.material_at(0, 0, 0), 2, "corner is moderator");
        assert_eq!(case.material_at(8, 8, 0), 1, "centre is fuel");
        assert_eq!(case.material_at(15, 15, 0), 1);
        assert_eq!(case.material_at(7, 8, 0), 2, "just outside the square");
        assert_eq!(case.material_at(16, 8, 0), 2);

        // Exactly 8 x 8 of the 24 x 24 nodes are fuel.
        let fuel = case.which_sigma().iter().filter(|m| **m == 1).count();
        assert_eq!(fuel, 64);
        assert_eq!(case.active_nodes(), 24 * 24, "no node is void");
    }

    #[test]
    fn cross_sections_match_the_source() {
        let case = built(16, 16);
        let s = &case.sigmas.base;
        assert_eq!(s.materials(), 2);
        assert_eq!(s.ngroups(), 1);
        assert_eq!(s.total[0], vec![5.0]);
        assert_eq!(s.nu_fission[0], vec![0.25]);
        assert_eq!(s.nu_fission[1], vec![0.0], "moderator does not fission");
        assert_eq!(s.scatter.get(0, 0, 0), 4.5);
        assert_eq!(s.scatter.get(1, 0, 0), 4.5);
        assert!(s.absorption.is_empty());
    }

    #[test]
    fn there_are_no_ends_and_no_thermal_hydraulics() {
        let case = built(16, 16);
        assert!(
            case.geometry.ends.is_none(),
            "geometry_ends3d is not called"
        );
        assert!(case.th.is_none());
        assert!(case.geometry.fuel.is_none());
        assert!(case.sigmas.coolant_density.is_none());
        assert_eq!(case.constants.frac_p, Some(1.0));
        // Vacuum on all four sides.
        assert_eq!(case.geometry.boundaries.x_min, Boundary::Vacuum);
        assert_eq!(case.geometry.boundaries.y_max, Boundary::Vacuum);
    }
}
