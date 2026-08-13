// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// Derived from OpenFOAM (www.openfoam.com)
// Copyright (C) 2004-2023 OpenFOAM Foundation
// Copyright (C) 2016-2023 OpenCFD Ltd.
//
// This file is part of OUTRAM PARK.
//
// OUTRAM PARK is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the
// Free Software Foundation, either version 3 of the License, or (at your
// option) any later version.
//
// OUTRAM PARK is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
// General Public License for more details.
//
// You should have received a copy of the GNU General Public License along
// with OUTRAM PARK.  If not, see <https://www.gnu.org/licenses/>.

//! # V&V — `fvSchemes` selection actually changes the discretisation
//!
//! `FvSchemes` has always parsed OpenFOAM's scheme dictionary into typed enums,
//! but until 2026-08-07 **no solver read a single one of them**: `PimpleFoam`
//! stored an `FvSchemes` and hardwired Euler time stepping with first-order
//! upwind convection regardless. `crate::solvers::schemes` makes the `ddt` and
//! `div` selections real; this file is the end-to-end evidence, measured against
//! a published benchmark rather than against the code's own output.
//!
//! **Reference data.** Ghia, Ghia & Shin (1982), *"High-Re solutions for
//! incompressible flow using the Navier-Stokes equations and a multigrid
//! method"*, J. Comput. Phys. 48(3):387-411 — Table I, Re = 100, the 17
//! tabulated `U_x/U_lid` values on the vertical centreline. Public literature
//! data; the same table the `pimple_foam_cavity` tutorial already uses.

use outram_foam_appbuilder_lib::io::control_dict::{ControlDict, StartControl, StopControl};
use outram_foam_appbuilder_lib::io::field_reader::{
    read_vol_scalar_field_full, read_vol_vector_field_full,
};
use outram_foam_appbuilder_lib::io::fv_schemes::{DdtScheme, DivScheme, FvSchemes};
use outram_foam_appbuilder_lib::io::fv_solution::FvSolution;
use outram_foam_appbuilder_lib::io::poly_mesh::read_poly_mesh;
use outram_foam_appbuilder_lib::solvers::pimple_foam::PimpleFoam;
use outram_foam_basic_lib::prelude::{Vector3, VolScalarField};
use std::path::Path;

const CASE_DIR: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tutorials/cases/pimple_foam_cavity"
);
const U_LID: f64 = 1.0; // m/s
const L: f64 = 0.1; // m, cavity side length

// Ghia et al. (1982) Table I, Re = 100, vertical-centreline U_x/U_lid.
const GHIA_Y: [f64; 17] = [
    0.0000, 0.0547, 0.0625, 0.0703, 0.1016, 0.1719, 0.2813, 0.4531, 0.5000, 0.6172, 0.7344, 0.8516,
    0.9531, 0.9609, 0.9688, 0.9766, 1.0000,
];
const GHIA_UX: [f64; 17] = [
    0.00000, -0.03717, -0.04192, -0.04775, -0.06434, -0.10150, -0.15662, -0.21090, -0.20581,
    -0.13641, 0.00332, 0.23151, 0.68717, 0.73722, 0.78871, 0.84123, 1.00000,
];

fn case_dir() -> &'static Path {
    Path::new(CASE_DIR)
}

/// Build the Re = 100 cavity (ν = 1e-3 m²/s, 20×20 mesh) with the given scheme
/// selection, advanced to `end_time` seconds at step `dt`.
fn build_cavity(schemes: FvSchemes, end_time: f64, dt: f64) -> PimpleFoam {
    let mesh = read_poly_mesh(&case_dir().join("constant").join("polyMesh"))
        .expect("read cavity polyMesh");
    let control = ControlDict {
        start: StartControl::StartTime(0.0),
        stop: StopControl::EndTime(end_time),
        delta_t: dt,
        ..ControlDict::default()
    };
    let mut solution = FvSolution::default();
    solution.pimple.n_outer_correctors = 1; // pure PISO ≡ icoFoam
    solution.pimple.n_correctors = 2;

    let mut solver = PimpleFoam::new(mesh.clone(), control, schemes, solution);
    solver.u =
        read_vol_vector_field_full(&case_dir().join("0").join("U"), &mesh).expect("read 0/U");
    solver.p =
        read_vol_scalar_field_full(&case_dir().join("0").join("p"), &mesh).expect("read 0/p");
    solver.nu = VolScalarField::uniform("nu", mesh, 1.0e-3);
    solver
}

/// Vertical-centreline `U_x/U_lid` profile as `(y/L, U_x/U_lid)` pairs, from the
/// central column of cells.
fn centreline_profile(centres: &[Vector3], u: &[Vector3]) -> Vec<(f64, f64)> {
    let dx = L / 20.0;
    let x_mid = L / 2.0;
    let mut rows: Vec<(f64, f64, usize)> = Vec::new();
    for (c, centre) in centres.iter().enumerate() {
        if (centre.x - x_mid).abs() < dx * 0.9 {
            let y = centre.y;
            if let Some(r) = rows
                .iter_mut()
                .find(|(ry, _, _)| (ry - y).abs() < dx * 0.25)
            {
                r.1 += u[c].x;
                r.2 += 1;
            } else {
                rows.push((y, u[c].x, 1));
            }
        }
    }
    rows.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    rows.into_iter()
        .map(|(y, sum, n)| (y / L, sum / n as f64 / U_LID))
        .collect()
}

/// Linear interpolation of the centreline profile at `y_over_l`, anchored at the
/// physical wall (0) and lid (1) values.
fn interp_ux(profile: &[(f64, f64)], y_over_l: f64) -> f64 {
    if y_over_l <= 0.0 {
        return 0.0;
    }
    if y_over_l >= 1.0 {
        return 1.0;
    }
    let mut left = (0.0, 0.0);
    let mut right = (1.0, 1.0);
    for &(y, ux) in profile {
        if y <= y_over_l && y >= left.0 {
            left = (y, ux);
        }
        if y >= y_over_l && y <= right.0 {
            right = (y, ux);
        }
    }
    if (right.0 - left.0).abs() < 1e-12 {
        return left.1;
    }
    let t = (y_over_l - left.0) / (right.0 - left.0);
    left.1 + t * (right.1 - left.1)
}

/// `(max |err|, RMS err)` of the centreline profile against Ghia (1982), in
/// units of `U_x/U_lid`.
fn ghia_error(solver: &PimpleFoam) -> (f64, f64) {
    let profile = centreline_profile(&solver.mesh.cell_centres, solver.u.internal.as_slice());
    let mut max_abs = 0.0_f64;
    let mut sum_sq = 0.0_f64;
    for (y, ux_ref) in GHIA_Y.iter().zip(GHIA_UX.iter()) {
        let err = (interp_ux(&profile, *y) - ux_ref).abs();
        max_abs = max_abs.max(err);
        sum_sq += err * err;
    }
    (max_abs, (sum_sq / GHIA_UX.len() as f64).sqrt())
}

/// V&V — selecting `Gauss linear` convection instead of `Gauss upwind` measurably
/// improves the lid-driven-cavity solution against Ghia et al. (1982).
///
/// # Methodology
///
/// **What is computed.** The Re = 100 lid-driven cavity (20×20 mesh, L = 0.1 m,
/// U_lid = 1 m/s, ν = 1e-3 m²/s) advanced to t = 12 s (≈ 120 lid-transit times,
/// well into steady state) at Δt = 4e-3 s (Co ≈ 0.8), run twice with identical
/// everything except `schemes.default_div`:
///
/// * [`DivScheme::GaussUpwind`] — first-order upwind (the historical hardwired
///   behaviour and the new honest default).
/// * [`DivScheme::GaussLinear`] — second-order central differencing by deferred
///   correction.
///
/// The vertical-centreline `U_x/U_lid` profile is extracted from the central
/// column of cells and compared with the 17 tabulated Ghia points; the metric is
/// the RMS of the 17 absolute errors.
///
/// **Reference.** Ghia, Ghia & Shin (1982), J. Comput. Phys. 48(3):387-411,
/// Table I (Re = 100). Public literature data. Ghia used a 129×129 mesh, so a
/// 20×20 solution cannot reach their accuracy; the *comparison between the two
/// schemes on the same mesh* is what this test verifies, and the direction of
/// the change is the physically predicted one — upwind's leading truncation term
/// is a numerical diffusivity ~|u|Δx/2, which smears the recirculation, and
/// removing it must move the profile toward the benchmark.
///
/// **Pass criteria.** (a) the two runs must differ (max |ΔU| > 1e-6 m/s), so a
/// silently-ignored scheme selection fails; (b) the `Gauss linear` RMS error
/// against Ghia must be **strictly smaller** than the upwind RMS error;
/// (c) both must stay finite and below the existing 0.10 regression guard.
///
/// # Results (run 2026-08-07, release mode, this machine)
///
/// | `default_div` | max &#124;err&#124; vs Ghia | RMS vs Ghia |
/// |---|---|---|
/// | `Gauss upwind` (first order) | 0.0634 | 0.0363 |
/// | `Gauss linear` (second order) | 0.0426 | 0.0224 |
///
/// max &#124;U_linear − U_upwind&#124; = 6.3653e-2 m/s (6.4 % of U_lid).
///
/// **Interpretation.** Choosing `Gauss linear` cuts the centreline RMS error
/// against Ghia from 0.0363 to 0.0224 — a 38 % reduction — and the peak error
/// from 0.0634 to 0.0426, on an unchanged mesh, time step and solver. This is the
/// signature of removing first-order upwind's numerical diffusion, and it is
/// direct evidence that the `fvSchemes` selection now reaches the
/// discretisation. The residual error is the coarse 20×20 mesh against Ghia's
/// 129×129.
///
/// This is verification that the *scheme selection works*, and a favourable
/// comparison against public benchmark data at one Reynolds number. It is not a
/// claim that the solver is validated across a range of conditions.
#[test]
fn gauss_linear_convection_beats_upwind_against_ghia_1982() {
    let end_time = 12.0;
    let dt = 4.0e-3;

    let mut upwind_schemes = FvSchemes::default();
    upwind_schemes.default_div = DivScheme::GaussUpwind;
    let mut s_up = build_cavity(upwind_schemes, end_time, dt);
    s_up.run().expect("upwind run");

    let mut linear_schemes = FvSchemes::default();
    linear_schemes.default_div = DivScheme::GaussLinear;
    let mut s_lin = build_cavity(linear_schemes, end_time, dt);
    s_lin.run().expect("linear run");

    let n_bad = s_lin
        .u
        .internal
        .as_slice()
        .iter()
        .chain(s_up.u.internal.as_slice())
        .filter(|v| !(v.x.is_finite() && v.y.is_finite() && v.z.is_finite()))
        .count();
    assert_eq!(n_bad, 0, "{n_bad} non-finite cells (a run diverged)");

    let d = s_lin
        .u
        .internal
        .as_slice()
        .iter()
        .zip(s_up.u.internal.as_slice())
        .map(|(a, b)| (*a - *b).mag())
        .fold(0.0_f64, f64::max);

    let (max_up, rms_up) = ghia_error(&s_up);
    let (max_lin, rms_lin) = ghia_error(&s_lin);
    println!("Ghia (1982) Re=100 centreline, 20x20 mesh, t = {end_time} s, dt = {dt}:");
    println!("  Gauss upwind : max|err| = {max_up:.4}, RMS = {rms_up:.4}");
    println!("  Gauss linear : max|err| = {max_lin:.4}, RMS = {rms_lin:.4}");
    println!("  max |U_linear - U_upwind| = {d:.4e} m/s");

    assert!(
        d > 1e-6,
        "the div scheme selection must change the solution: max |ΔU| = {d:e}"
    );
    assert!(
        rms_lin < rms_up,
        "second-order convection must beat first-order upwind against Ghia: \
         linear RMS {rms_lin:.4} vs upwind RMS {rms_up:.4}"
    );
    assert!(
        rms_up < 0.10 && rms_lin < 0.10,
        "both schemes must stay inside the existing regression guard: \
         upwind {rms_up:.4}, linear {rms_lin:.4}"
    );
}

/// V&V — the `ddt` selection reaches the solver; and a **measured, diagnosed
/// limitation** of the second-order `Backward` scheme in this PISO port.
///
/// # Methodology
///
/// **What is computed.** The same Re = 100 cavity, run with
/// [`DdtScheme::Euler`] and with [`DdtScheme::Backward`] (BDF2), both with
/// `Gauss upwind` convection so the only difference is the time scheme. Three
/// quantities are recorded: the transient difference after 5 steps; the
/// difference at long time (t = 12 s and t = 30 s, well past steady state); and
/// each run's centreline RMS error against Ghia et al. (1982), Table I, Re = 100.
///
/// **References.**
///
/// 1. *Analytical.* At steady state U^n = U^{n−1} = U^{n−2}, so the BDF2
///    momentum term `(3U − 4U + U)/2Δt` and the Euler term `(U − U)/Δt` both
///    vanish, and the two momentum equations have the **same** fixed point. Any
///    residual difference must therefore come from outside the momentum
///    equation.
/// 2. *Public benchmark.* Ghia, Ghia & Shin (1982), J. Comput. Phys.
///    48(3):387-411, Table I (Re = 100), 17 centreline points.
///
/// **Pass criteria.** (a) max |U_backward − U_euler| > 1e-9 m/s after 5 steps —
/// a silently ignored `ddt` selection would give a bitwise-identical field;
/// (b) both runs finite; (c) the Backward run's Ghia RMS is no worse than the
/// Euler run's, and both inside the existing 0.10 regression guard.
///
/// # Results (run 2026-08-07, release mode, this machine)
///
/// At Δt = 4e-3 s: transient difference after 5 steps = 2.829482e-2 m/s; long-
/// time difference = 1.649884e-2 m/s at **both** t = 12 s and t = 30 s (identical
/// to all printed digits); Ghia RMS — Euler 0.0363, Backward 0.0359.
///
/// Δt sweep of the long-time difference and the benchmark error:
///
/// | Δt [s] | max &#124;ΔU&#124; at t = 12 s | max &#124;ΔU&#124; at t = 30 s | Ghia RMS Euler | Ghia RMS Backward |
/// |---|---|---|---|---|
/// | 8e-3 | 1.0394e-2 | 1.0394e-2 | 0.0365 | 0.0362 |
/// | 4e-3 | 1.6499e-2 | 1.6499e-2 | 0.0363 | 0.0359 |
/// | 2e-3 | 2.3420e-2 | 2.3420e-2 | 0.0361 | 0.0355 |
/// | 1e-3 | 2.9258e-2 | 2.9258e-2 | 0.0358 | 0.0350 |
///
/// # Interpretation — what works, and the limitation this measured
///
/// **Works.** The `ddtSchemes` selection reaches the momentum assembly: the
/// transient differs by 2.8e-2 m/s (2.8 % of U_lid) after five steps, and BDF2's
/// benchmark error is consistently *slightly better* than Euler's at every Δt
/// tested (e.g. 0.0350 vs 0.0358 at Δt = 1e-3 s).
///
/// **Limitation, measured and diagnosed — do not treat `Backward` as verified
/// second order.** The two schemes converge to *different* steady states,
/// differing by 1.0e-2 to 2.9e-2 m/s (1–3 % of U_lid). Two observations pin the
/// cause down:
///
/// - The difference is **identical at t = 12 s and t = 30 s**, so it is a
///   genuine difference of fixed points, not incomplete convergence.
/// - The difference **grows as Δt is reduced** (1.04e-2 → 2.93e-2 over an 8×
///   refinement), which rules out discretisation error, whose whole character is
///   to shrink with Δt.
///
/// The remaining Δt-dependent, scheme-dependent term in the algorithm is the
/// Rhie–Chow flux correction: `PimpleFoam::step` adds
/// `rAU_f · fvc::ddt_corr(U_old, φ_old, Δt)` to `φ_HbyA`, and **`ddt_corr` is
/// hardwired to the Euler form** while `rAU = V/A` picks up BDF2's `1.5 V/Δt`
/// diagonal instead of Euler's `V/Δt`. The two are therefore inconsistent, by a
/// ratio that tends to 1.5 as Δt → 0 — exactly the observed trend. Making
/// `Backward` fully consistent needs a backward-form `ddtCorr` (OpenFOAM's
/// `backwardDdtScheme::fvcDdtPhiCorr`), which lives in `outram-foam-basic-lib`
/// and is outside this crate.
///
/// This test therefore asserts only what is verified: the selection is live, and
/// BDF2 is not worse against the benchmark. It deliberately does **not** assert
/// steady-state equivalence, because that is currently false and the number
/// above is the evidence.
#[test]
fn backward_ddt_selection_is_live_and_not_worse_against_ghia() {
    let dt = 4.0e-3;

    let run = |ddt: DdtScheme, end: f64| {
        let mut sch = FvSchemes::default();
        sch.ddt = ddt;
        sch.default_div = DivScheme::GaussUpwind;
        let mut s = build_cavity(sch, end, dt);
        s.run().expect("cavity run");
        s
    };
    let diff = |a: &PimpleFoam, b: &PimpleFoam| -> f64 {
        a.u.internal
            .as_slice()
            .iter()
            .zip(b.u.internal.as_slice())
            .map(|(x, y)| (*x - *y).mag())
            .fold(0.0_f64, f64::max)
    };

    let d_transient = diff(
        &run(DdtScheme::Euler, 5.0 * dt),
        &run(DdtScheme::Backward, 5.0 * dt),
    );

    let e_steady = run(DdtScheme::Euler, 12.0);
    let b_steady = run(DdtScheme::Backward, 12.0);
    let d_steady = diff(&e_steady, &b_steady);
    let (_, rms_e) = ghia_error(&e_steady);
    let (_, rms_b) = ghia_error(&b_steady);

    println!("max |U_backward - U_euler|: transient (5 steps) = {d_transient:.6e} m/s, long-time (12 s) = {d_steady:.6e} m/s");
    println!("Ghia Re=100 centreline RMS at 12 s: Euler {rms_e:.4}, Backward {rms_b:.4}");
    println!(
        "NOTE: the long-time difference is a known inconsistency between the BDF2 momentum \
         diagonal and the Euler-only Rhie-Chow ddtCorr — see this test's documentation."
    );

    assert!(
        b_steady
            .u
            .internal
            .as_slice()
            .iter()
            .all(|v| v.mag().is_finite()),
        "the backward-differencing run must stay finite"
    );
    assert!(
        d_transient > 1e-9,
        "the ddt scheme selection must change the transient: max |ΔU| = {d_transient:e}"
    );
    assert!(
        rms_b <= rms_e + 1e-4,
        "backward differencing must not be worse than Euler against Ghia: \
         Backward RMS {rms_b:.4} vs Euler RMS {rms_e:.4}"
    );
    assert!(
        rms_b < 0.10,
        "backward run must stay inside the existing regression guard: RMS {rms_b:.4}"
    );
}
