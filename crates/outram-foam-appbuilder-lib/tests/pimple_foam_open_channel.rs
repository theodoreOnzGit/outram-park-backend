//! `PimpleFoam` on an OPEN domain: 2-D plane Poiseuille flow between parallel
//! plates, fixed-velocity inlet, fixed-pressure outlet (gh:#1154).
//!
//! # Why this test exists
//!
//! Until gh:#1154 every `PimpleFoam` case was a closed cavity. The first open
//! case (the stage 0 browser pipe) exposed three differences from upstream
//! pimpleFoam, all on the boundary:
//!
//! 1. `p_eqn.set_reference(0, 0.0)` was applied unconditionally. Upstream
//!    `fvMatrix::setReference` acts only when `p.needReference()`, false as
//!    soon as a patch fixes p, so with a fixed-p outlet cell 0 became a mass
//!    sink. Measured on the 5k-cell pipe before the fix: outlet/inlet flux
//!    0.94 on step 1, 144 on step 2, then negative.
//! 2. The pressure source took the boundary flux from the U boundary
//!    *values*, which are zero on a zeroGradient outlet, so the outflow of
//!    `phiHbyA` was missing. Upstream uses `fvc::div(phiHbyA)` with
//!    `HbyA_P·Sf` there.
//! 3. The boundary flux of `phi` was never set to the prescribed `U_b·Sf` on
//!    fixed-velocity patches (`constrainHbyA`), nor corrected by the pressure
//!    flux on fixed-p patches (`phi = phiHbyA − pEqn.flux()`).
//!
//! # Methodology
//!
//! Channel height h = 0.1 m, length 1 m, 50 × 20 cells, one empty cell in z.
//! ν = 0.01 m²/s, inlet U = 0.1 m/s uniform, so Re_h = U h / ν = 1 and the
//! entry length is a small fraction of h. Started from rest, PISO (1 outer,
//! 2 correctors), Δt = 0.002 s (Co ≈ 0.015, viscous number ν Δt / Δy² = 0.8),
//! run to t = 2 s, twenty times the slowest viscous time h² / (π² ν) = 0.1 s.
//!
//! **Protocol change after the first run (criteria unchanged).** The first
//! run used Δt = 0.02 s (viscous number 8). It went unstable: |U|max grew
//! from 0.11 to 0.30 m/s in 10 steps and the run ended in NaN, with mass
//! still conserved to 1e-10 at every step. Δt = 0.002 s is stable. The
//! instability is a `PimpleFoam` time-step limit at large viscous number, not
//! a boundary defect; it is recorded in the stage 0 README and its issue.
//!
//! References (analytical): plane Poiseuille, u_max = 1.5 U and
//! dp/dx = −12 ν U / h² (kinematic).
//!
//! Pass criteria, fixed before the first run (2026-11-03):
//! - mass: outlet flux = inlet flux to 1e-9 relative;
//! - u_max / 1.5 U within 3 % (intercept of a u-against-(y − h/2)² fit over
//!   the cells within h/4 of the centreline, x ∈ [0.5, 0.8] m);
//! - dp/dx / (−12 ν U / h²) within 5 % (slope of a p-against-x fit over
//!   x ∈ [0.5, 0.8] m). The 5 % allows the O(Δy) error of the half-cell wall
//!   gradient (Δy / 2h = 2.5 % per wall on 20 cells).
//!
//! # Results (2026-11-03, this commit)
//!
//! Measured: outlet/inlet flux mismatch 2.3e-11, u_max / 1.5 U = 0.99751,
//! dp/dx / (−12 ν U / h²) = 0.99502. All three pass. Also recorded in
//! `verification_and_validation/wasm_pimple_3d_pipe/README.md`.
//!
//! **The test can fail:** with fix 1 undone (the reference pin applied
//! unconditionally again) it fails: outlet/inlet
//! flux 0.597 (mass mismatch 0.40), u_max ratio 0.595, dp/dx ratio 0.594
//! (checked 2026-11-03).
use std::sync::Arc;

use outram_foam_appbuilder_lib::io::control_dict::{ControlDict, StartControl, StopControl};
use outram_foam_appbuilder_lib::io::fv_schemes::FvSchemes;
use outram_foam_appbuilder_lib::io::fv_solution::FvSolution;
use outram_foam_appbuilder_lib::solvers::pimple_foam::PimpleFoam;
use outram_foam_basic_lib::prelude::*;

const H: f64 = 0.1;
const NU: f64 = 0.01;
const U: f64 = 0.1;

const DICT: &str = r#"
convertToMeters 1;
vertices
(
    (0 0 0) (1 0 0) (1 0.1 0) (0 0.1 0)
    (0 0 0.01) (1 0 0.01) (1 0.1 0.01) (0 0.1 0.01)
);
blocks ( hex (0 1 2 3 4 5 6 7) (50 20 1) simpleGrading (1 1 1) );
edges ();
boundary
(
    inlet  { type patch; faces ((0 4 7 3)); }
    outlet { type patch; faces ((1 2 6 5)); }
    walls  { type wall;  faces ((0 1 5 4) (3 7 6 2)); }
    frontAndBack { type empty; faces ((0 3 2 1) (4 5 6 7)); }
);
"#;

fn fit_line(s: &[(f64, f64)]) -> (f64, f64) {
    let n = s.len() as f64;
    let (sx, sy) = s.iter().fold((0.0, 0.0), |a, p| (a.0 + p.0, a.1 + p.1));
    let (sxx, sxy) = s.iter().fold((0.0, 0.0), |a, p| (a.0 + p.0 * p.0, a.1 + p.0 * p.1));
    let b = (n * sxy - sx * sy) / (n * sxx - sx * sx);
    ((sy - b * sx) / n, b)
}

fn patch_flux(s: &PimpleFoam, name: &str) -> f64 {
    let i = s.mesh.patches.iter().position(|p| p.name == name).expect("patch");
    s.phi.boundary[i].values.as_slice().iter().sum()
}

/// The channel case at `stop` s; `ablate` selects the pre-gh:#1161 orthogonal
/// Laplacian; `tight` solves U and p to 1e-14 instead of the defaults.
fn channel(stop: f64, ablate: bool, tight: bool) -> PimpleFoam {
    let mesh = Arc::new(
        outram_foam_mesh::block_mesh::block_mesh(DICT)
            .expect("blockMesh")
            .to_fv_mesh()
            .expect("fv mesh"),
    );
    let n = mesh.n_cells;
    let control = ControlDict {
        start: StartControl::StartTime(0.0),
        stop: StopControl::EndTime(stop),
        delta_t: 0.002,
        ..ControlDict::default()
    };
    let mut solution = FvSolution::default();
    solution.pimple.n_outer_correctors = 1;
    solution.pimple.n_correctors = 2;
    let mut s = PimpleFoam::new(mesh.clone(), control, FvSchemes::default(), solution);
    let (mut ub, mut pb) = (Vec::new(), Vec::new());
    for p in &mesh.patches {
        let (u_bc, p_bc) = match p.name.as_str() {
            "inlet" => (PatchField::fixed_value_vec(p.size, Vector3::new(U, 0.0, 0.0)), PatchField::zero_gradient(p.size)),
            "outlet" => (PatchField::zero_gradient_vec(p.size), PatchField::fixed_value(p.size, 0.0)),
            "walls" => (PatchField::fixed_value_vec(p.size, Vector3::ZERO), PatchField::zero_gradient(p.size)),
            _ => (
                PatchField { bc: BoundaryCondition::Empty, values: Field::zero_vec(p.size) },
                PatchField { bc: BoundaryCondition::Empty, values: Field::zeros(p.size) },
            ),
        };
        ub.push(u_bc);
        pb.push(p_bc);
    }
    s.u = VolVectorField::new("U", mesh.clone(), Field::new(vec![Vector3::ZERO; n]), ub);
    s.p = VolScalarField::new("p", mesh.clone(), Field::new(vec![0.0; n]), pb);
    s.nu = VolScalarField::uniform("nu", mesh.clone(), NU);
    if ablate {
        s = s.without_non_orthogonal_correction();
    }
    if tight {
        let t = SolverSettings { tolerance: 1e-14, max_iter: 20_000 };
        s = s.with_linear_solver_settings(t, t);
    }
    s.run().expect("run");
    s
}

/// gh:#1161: on this Cartesian mesh the non-orthogonal correction is zero up
/// to the rounding of `n = Sf/|Sf|` and `1/(n·d)` against `1/|d|`, so the
/// default (corrected) run and the ablation agree to round-off.
///
/// Prediction (written before the run): below 1e-10 relative in U and p.
/// Measured 2026-11-03: see the println (recorded in
/// `verification_and_validation/non_orthogonal_correction_gh1161.md`).
///
/// **Both arms are solved to 1e-14 since 2026-11-05.** The warm-started
/// momentum predictor (gh:#1202, `fd79a96f9`, found by `git bisect` with this
/// test) lets two arms that differ by round-off stop at different points
/// inside the default linear tolerances (U 1e-7, p 1e-8): at the defaults
/// this gave 5.4e-10 in U and 1.6e-8 in p, the solver's tolerance, not the
/// correction. Solved to 1e-14 they agree to 2.3e-15 and 1.1e-13
/// (measured 2026-11-05), so the correction is zero to round-off as
/// claimed, and the 1e-10 bound is unchanged.
#[test]
fn cartesian_channel_moves_by_round_off_only() {
    let a = channel(0.2, false, true);
    let b = channel(0.2, true, true);
    let du = a.u.internal.as_slice().iter().zip(b.u.internal.as_slice()).fold(0.0_f64, |m, (x, y)| m.max((*x - *y).mag())) / U;
    let pmax = b.p.internal.as_slice().iter().fold(0.0_f64, |m, x| m.max(x.abs()));
    let dp = a.p.internal.as_slice().iter().zip(b.p.internal.as_slice()).fold(0.0_f64, |m, (x, y)| m.max((x - y).abs())) / pmax;
    let same = a.u.internal.as_slice() == b.u.internal.as_slice();
    println!("corrected vs orthogonal: max |dU|/U {du:.3e}, max |dp|/max p {dp:.3e}, bit-identical U: {same}");
    assert!(du < 1e-10 && dp < 1e-10, "du {du:.3e} dp {dp:.3e}");
}

#[test]
fn plane_poiseuille_with_fixed_pressure_outlet() {
    let s = channel(2.0, false, false);
    let mesh = s.mesh.clone();
    let n = mesh.n_cells;

    let q_in = -patch_flux(&s, "inlet");
    let q_out = patch_flux(&s, "outlet");
    let mass = (q_out - q_in).abs() / q_in;

    let u = s.u.internal.as_slice();
    let p = s.p.internal.as_slice();
    let in_slab = |c: usize| (0.5..=0.8).contains(&mesh.cell_centres[c].x);
    let centre: Vec<(f64, f64)> = (0..n)
        .filter(|&c| in_slab(c) && (mesh.cell_centres[c].y - 0.5 * H).abs() < 0.25 * H)
        .map(|c| ((mesh.cell_centres[c].y - 0.5 * H).powi(2), u[c].x))
        .collect();
    let u_max = fit_line(&centre).0 / (1.5 * U);
    let px: Vec<(f64, f64)> = (0..n).filter(|&c| in_slab(c)).map(|c| (mesh.cell_centres[c].x, p[c])).collect();
    let dpdx = fit_line(&px).1 / (-12.0 * NU * U / (H * H));
    println!("q_in {q_in:.6e} q_out {q_out:.6e} mass {mass:.2e} u_max/1.5U {u_max:.5} dpdx/HP {dpdx:.5}");

    assert!(mass < 1e-9, "outlet/inlet flux mismatch {mass:.3e}");
    assert!((u_max - 1.0).abs() < 0.03, "u_max / 1.5U = {u_max}");
    assert!((dpdx - 1.0).abs() < 0.05, "dp/dx / HP = {dpdx}");
}
