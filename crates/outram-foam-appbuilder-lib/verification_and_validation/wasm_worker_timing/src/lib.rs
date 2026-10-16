//! The Sod `RhoCentralFoam` solve, packaged so the SAME code runs natively and
//! as `wasm32-unknown-unknown` inside a Web Worker (gh:#893). Methodology and
//! results: `README.md` beside this crate.
//!
//! No filesystem: the mesh comes from the committed blockMeshDict text through
//! `outram_foam_mesh::block_mesh`, and the initial fields are set in memory the
//! way the case's setFieldsDict does (x < 0 is the left state).
use outram_foam_appbuilder_lib::io::control_dict::{ControlDict, StartControl, StopControl};
use outram_foam_appbuilder_lib::io::fv_schemes::FvSchemes;
use outram_foam_appbuilder_lib::io::fv_solution::FvSolution;
use outram_foam_appbuilder_lib::solvers::rho_central_foam::RhoCentralFoam;
use std::sync::Arc;

const DICT: &str = include_str!("../../../tutorials/cases/rho_central_foam_shock_tube/system/blockMeshDict");
const R_GAS: f64 = 8314.46261815324 / 28.96;
const GAMMA: f64 = 1.4;
const T_END: f64 = 0.2 * 10.0 / 316.2277660168379;

pub fn build(dt: f64) -> RhoCentralFoam {
    let poly = outram_foam_mesh::block_mesh::block_mesh(DICT).expect("blockMesh");
    let mesh = Arc::new(poly.to_fv_mesh().expect("fv mesh"));
    let n = mesh.n_cells;
    let control = ControlDict {
        start: StartControl::StartTime(0.0),
        stop: StopControl::EndTime(T_END),
        delta_t: dt,
        ..ControlDict::default()
    };
    let mut s = RhoCentralFoam::new(mesh.clone(), control, FvSchemes::default(), FvSolution::default());
    for c in 0..n {
        let left = mesh.cell_centres[c].x < 0.0;
        let (p, t) = if left { (1.0e5, 348.432) } else { (1.0e4, 278.746) };
        let rho = p / (R_GAS * t);
        s.rho.internal.as_mut_slice()[c] = rho;
        s.e.internal.as_mut_slice()[c] = p / ((GAMMA - 1.0) * rho);
        s.p.internal.as_mut_slice()[c] = p;
    }
    s
}

/// Runs to tau = 0.2 and returns sum(rho) over cells, a checksum to compare
/// wasm against native.
pub fn run(dt: f64) -> f64 {
    let mut s = build(dt);
    s.run().expect("run");
    s.rho.internal.as_slice().iter().sum()
}

#[no_mangle]
pub extern "C" fn sod_rho_sum(dt: f64) -> f64 {
    run(dt)
}

#[no_mangle]
pub extern "C" fn sod_n_cells() -> u32 {
    build(1e-6).mesh.n_cells as u32
}

#[cfg(test)]
mod tests {
    /// Mass is conserved to round-off. By tau = 0.2 the shock is at x = +3.5 m
    /// and the rarefaction head at x = -2.4 m, so no wave has reached the
    /// zeroGradient ends of the [-5, 5] m tube, and the uniform-volume cell
    /// sum of rho must equal its initial value. A dropped flux, a wrong sign
    /// or a solve that did not run all break it.
    #[test]
    fn rho_sum_is_conserved_to_round_off() {
        let s0: f64 = super::build(4.0e-6).rho.internal.as_slice().iter().sum();
        let s1 = super::run(4.0e-6);
        assert!(((s1 - s0) / s0).abs() < 1e-12, "initial {s0}, final {s1}");
    }
}
