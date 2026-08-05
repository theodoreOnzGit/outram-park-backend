//! Minimal in-memory fixtures for the coupling unit tests.
//!
//! Test-only (`#[cfg(test)]`), and deliberately not a case constructor: these
//! build the smallest [`CaseParams`] / [`CoreGeometry`] that the pure functions
//! in this directory can be exercised on. Real benchmark cases belong to
//! [`crate::reference::cases`], and fixture-based comparisons against Yan Ren's
//! MATLAB belong in `tests/`.

use super::seam::{
    CaseParams, CoolantState, CoreGeometry, FuelGeometry, MaterialMap, NodalCoefficients,
    SigmaValues, ThermalState,
};
use crate::reference::grid::{Geometry, Grid};

/// A 2×2×4 two-group grid — small enough to reason about by hand.
pub fn minimal_grid() -> Grid {
    Grid::new(2, 2, 4, 2).expect("valid grid")
}

/// [`CaseParams`] with every optional control unset.
pub fn minimal_params() -> CaseParams {
    CaseParams {
        grid: minimal_grid(),
        n_components: 0,
        fuel_max_ir: 5,
        fuel_n: 3,
        boron: 1000.0,
        fuel_temp_avg_init: 800.0,
        cool_temp_avg_init: 560.0,
        cool_den_avg_init: 0.75,
        fuel_temp_tol: None,
        flux_tol: None,
        th_max_iter: None,
        th_relax: None,
        inexact_inner: None,
        inexact_eta: None,
        inner_tol: None,
        crit_tol: None,
        t_end: None,
        t_grid: None,
        time_picard: None,
        nodal_upd_time: None,
        time_scheme: None,
        freq_iter: None,
        freq_mode: None,
        out_prefix: None,
        velocities: vec![1.0e7, 5.0e5],
        beta_dnp: vec![0.0002, 0.001, 0.0012, 0.0025, 0.0008, 0.0003],
        lambda_dnp: vec![0.0124, 0.0305, 0.111, 0.301, 1.14, 3.01],
        eject_duration: None,
        steady_file: None,
        debug_dump: false,
        output_dir: None,
        jfnk_precon: None,
        jfnk_rel: None,
        jfnk_verb: None,
    }
}

/// A uniform geometry on [`minimal_grid`], with the given radial material tags.
pub fn minimal_geometry_with_which_k(params: &CaseParams, which_k: Vec<usize>) -> CoreGeometry {
    let grid = params.grid;
    let nodes = grid.nodes();
    let base = Geometry {
        grid,
        x_total: 2.0 * 10.0,
        y_total: 2.0 * 10.0,
        z_total: 4.0 * 10.0,
        lx: vec![10.0; nodes],
        ly: vec![10.0; nodes],
        lz: vec![10.0; nodes],
        volume: vec![1000.0; nodes],
        which_sigma: vec![1; nodes],
    };
    CoreGeometry {
        base,
        fuel: FuelGeometry {
            which_k,
            ctr: vec![0.1, 0.2, 0.3, 0.4, 0.5],
            fuel_rad: 0.41,
        },
        crod: vec![0.0],
        crod_banks: vec![0; grid.nx * grid.ny],
        crod_btm: 0.0,
        crod_step: 1.0,
        crod_eject: None,
        crod_eject_to: None,
        zlows: vec![1; grid.nx * grid.ny],
        zhis: vec![grid.nz; grid.nx * grid.ny],
        zscale: 1,
        nodal_coeffs: NodalCoefficients::default(),
    }
}

/// A flat thermal state on [`minimal_grid`].
#[allow(dead_code)] // used once the nodal/th seam is removed
pub fn minimal_thermal_state(params: &CaseParams, n_solution_ids: usize) -> ThermalState {
    let nodes = params.grid.nodes();
    ThermalState {
        fuel_temp_avg: vec![params.fuel_temp_avg_init; nodes],
        fuel_temp_doppler: vec![params.fuel_temp_avg_init; nodes],
        fuel_temp: vec![params.fuel_temp_avg_init; nodes * n_solution_ids],
        n_solution_ids,
        mod_temp: None,
        coolant: CoolantState {
            temps: vec![params.cool_temp_avg_init; nodes],
            dens: vec![params.cool_den_avg_init; nodes],
            inlet_temp: 550.0,
            inlet_press: 15.5,
        },
        heat_flux: vec![0.0; nodes],
        power_ratio: 1.0,
        max_power: 3.0e9,
        n_fuel_pins: 264.0,
        coolant_heat_fraction: 0.019,
        flow_rate: 0.35,
        flow_dir: 1.0,
        inlet_temp_schedule: None,
    }
}

/// Two compositions, two groups, with no feedback tables.
#[allow(dead_code)] // used once the nodal/th seam is removed
pub fn minimal_sigma_values() -> SigmaValues {
    let ngroups = 2;
    let rows = 2;
    SigmaValues {
        ngroups,
        tot: vec![0.2, 0.6, 0.25, 0.8],
        f: vec![0.005, 0.1, 0.006, 0.12],
        fp: vec![1.6e-13, 3.2e-12, 1.9e-13, 3.8e-12],
        s: vec![0.0; rows * ngroups * ngroups],
        nu: vec![1.0; rows * ngroups],
        chi: vec![1.0, 0.0, 1.0, 0.0],
        feedback: Default::default(),
    }
}

/// Every node in composition 1.
#[allow(dead_code)] // used once the nodal/th seam is removed
pub fn minimal_material_map(params: &CaseParams) -> MaterialMap {
    MaterialMap {
        grid: params.grid,
        ids: vec![1; params.grid.nodes()],
    }
}
