// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Ong Kay Chen Theodore, National University of Singapore
//
// This file is part of Outram Park (outram-park-backend), outram-foam-appbuilder-lib.
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

//! # Sod shock tube V&V — Sod (1978) Table II vs the `RhoCentralFoam` port
//!
//! Reference document: `tests/sod_shock_tube_validation/CLAUDE.md`.
//! Primary source: Sod, G. A. (1978), *A Survey of Several Finite Difference
//! Methods for Systems of Nonlinear Hyperbolic Conservation Laws*, JCP 27(1),
//! 1–31.
//!
//! This is the **validation** case: it judges the Rust port of OpenFOAM
//! `rhoCentralFoam` against the published Sod (1978) Table II reference profile.
//! (The companion *tutorial* case,
//! `tutorials/rho_central_foam_shock_tube.rs`, instead compares the Rust port
//! against an OpenFOAM `rhoCentralFoam` reference run.)
//!
//! ## Methodology
//!
//! - **Reference:** Sod (1978) Table II — a 9-point solution at τ = 0.2
//!   (dimensionless), γ = 1.4, ICs ρ_L=1, p_L=1, ρ_R=0.125, p_R=0.1.
//! - **Case:** the committed 100-cell polyMesh (x ∈ [−5, 5] m) with the Sod
//!   initial fields under `.../0/`. The port is run to
//!   t = 0.2·t₀ = 6.3246×10⁻³ s (exactly Sod's τ = 0.2, where t₀ = L₀/u₀,
//!   L₀ = 10 m, u₀ = √(P₀/ρ₀) = 316.228 m/s). Table II is redimensionalised to
//!   SI with those same scale factors (CLAUDE.md §3.3).
//! - **Sampling:** the port's cell-centred profile is linearly interpolated onto
//!   the 9 Table II x-locations (x/L = 0.1…0.9 ⇒ x_SI = −4…4 m). Interpolate the
//!   simulation onto the reference points, never the reverse (CLAUDE.md §3.4).
//! - **Faithfulness arbiter:** Table II is a coarse Glimm's random-choice
//!   solution; two of its points fall inside the rarefaction fan (x/L = 0.3, 0.4)
//!   and one straddles the contact (x/L = 0.7), where 9 grid points cannot
//!   resolve the true profile. The exact analytic Riemann solution (Toro ch. 4,
//!   computed on the fly) is used *only* to decide, per point and per variable,
//!   whether Table II is faithful (|Table − exact| below a threshold). The port
//!   is hard-asserted against Table II **only at faithful points**; unfaithful
//!   points are reported for the record. This isolates the port's error from
//!   Table II's own discretisation error.
//! - **Pass criterion:** at every faithful (point, variable), the port matches
//!   the redimensionalised Table II value to < 5% of the field's peak magnitude.
//!
//! ## Results (measured 2026-07-06; 100 cells, dt = 1×10⁻⁶ s, vanLeer MUSCL)
//!
//! Exact Riemann solver self-check (Test 1) — all star-state quantities within
//! < 1×10⁻⁴ of the analytic values (P\*=0.30313, u\*=0.92745, ρ\*_L=0.42632,
//! ρ\*_R=0.26557, S_shock=1.75216).
//!
//! Port vs Table II at τ = 0.2, faithful points (Test 3), max deviation over all
//! asserted (point, variable) pairs, normalised by field peak:
//!
//! - pressure: 0.43% of peak (worst faithful point)
//! - velocity: 0.96% of peak
//! - density:  0.43% of peak
//!
//! The unfaithful points behave exactly as expected. At x/L = 0.4 (inside the
//! rarefaction fan) Table II reads ρ = 0.426 (dimensionless) but the exact value
//! is 0.603 — and the port gives 0.609, tracking the *exact* solution, not
//! Table II. At x/L = 0.7 (just past the contact at x/L ≈ 0.685) Table II reads
//! ρ = 0.426 while the exact right-star value is 0.266; the port gives 0.295
//! (one-cell contact smearing), again tracking the exact solution. So the large
//! residuals at those two points are Table II's 9-point coarseness, not the
//! port. Interpretation: the Rust `rhoCentralFoam` port reproduces Sod's
//! benchmark to well under 1% at every point Table II resolves cleanly, the
//! expected accuracy of a 2nd-order shock-capturing central scheme, with
//! monotone (non-oscillatory) captures of the shock, contact, and rarefaction.

use outram_foam_appbuilder_lib::io::control_dict::{ControlDict, StartControl, StopControl};
use outram_foam_appbuilder_lib::io::field_reader::{read_vol_scalar_field, read_vol_vector_field_full};
use outram_foam_appbuilder_lib::io::fv_schemes::FvSchemes;
use outram_foam_appbuilder_lib::io::fv_solution::FvSolution;
use outram_foam_appbuilder_lib::io::poly_mesh::read_poly_mesh;
use outram_foam_appbuilder_lib::solvers::rho_central_foam::RhoCentralFoam;
use std::path::{Path, PathBuf};

// ── Case geometry (shared with the rhoCentralFoam tutorial) ──────────────────
const CASE_DIR: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tutorials/cases/rho_central_foam_shock_tube"
);

// ── Gas + Sod initial conditions (SI) ────────────────────────────────────────
const MOL_WEIGHT: f64 = 28.96; // g/mol (air, matches thermophysicalProperties)
const R_UNIVERSAL: f64 = 8314.46261815324; // J/(kmol·K)
const R_GAS: f64 = R_UNIVERSAL / MOL_WEIGHT; // J/(kg·K) ≈ 287.1
const GAMMA: f64 = 1.4; // matches the solver's hard-coded γ

const RHO_LEFT: f64 = 1.0; // kg/m³
const P_LEFT: f64 = 1.0e5; // Pa
const RHO_RIGHT: f64 = 0.125; // kg/m³
const P_RIGHT: f64 = 1.0e4; // Pa
const X_DIAPHRAGM: f64 = 0.0; // m (domain x ∈ [−5, 5])

// Scale factors (CLAUDE.md §3.3): SI = Sod-dimensionless × factor.
const RHO0: f64 = 1.0; // kg/m³
const P0: f64 = 1.0e5; // Pa
const L0: f64 = 10.0; // m
const U0: f64 = 316.2277660168379; // m/s = √(P0/ρ0)
const T0: f64 = L0 / U0; // s ≈ 0.0316228

/// Sod canonical comparison time τ = 0.2, in SI seconds.
const T_TAU_02: f64 = 0.2 * T0; // ≈ 6.3246×10⁻³ s

// ═════════════════════════════════════════════════════════════════════════════
//  Exact Riemann solver (Toro ch. 4 / Sod Appendix) — used only as an arbiter
// ═════════════════════════════════════════════════════════════════════════════

/// A primitive gas state: density, velocity, pressure. Unit-agnostic — feed a
/// consistent set (all SI, or all Sod-dimensionless).
#[derive(Clone, Copy, Debug)]
struct GasState {
    rho: f64,
    u: f64,
    p: f64,
}

fn sound_speed(s: GasState) -> f64 {
    (GAMMA * s.p / s.rho).sqrt()
}

/// Pressure function f_K(p*) and derivative for one side (Toro eqs. 4.6–4.7).
fn pressure_fn(p_star: f64, side: GasState) -> (f64, f64) {
    let c_k = sound_speed(side);
    if p_star > side.p {
        let a_k = 2.0 / ((GAMMA + 1.0) * side.rho);
        let b_k = (GAMMA - 1.0) / (GAMMA + 1.0) * side.p;
        let sqrt_term = (a_k / (b_k + p_star)).sqrt();
        let f = (p_star - side.p) * sqrt_term;
        let df = sqrt_term * (1.0 - (p_star - side.p) / (2.0 * (b_k + p_star)));
        (f, df)
    } else {
        let f = 2.0 * c_k / (GAMMA - 1.0)
            * ((p_star / side.p).powf((GAMMA - 1.0) / (2.0 * GAMMA)) - 1.0);
        let df = 1.0 / (side.rho * c_k) * (p_star / side.p).powf(-(GAMMA + 1.0) / (2.0 * GAMMA));
        (f, df)
    }
}

/// Star-region pressure P\* by Newton–Raphson (Toro §4.3.2).
fn star_pressure(l: GasState, r: GasState) -> f64 {
    let c_l = sound_speed(l);
    let c_r = sound_speed(r);
    let ppv = 0.5 * (l.p + r.p) - 0.125 * (r.u - l.u) * (l.rho + r.rho) * (c_l + c_r);
    let mut p = ppv.max(1e-6 * 0.5 * (l.p + r.p));
    for _ in 0..200 {
        let (fl, dfl) = pressure_fn(p, l);
        let (fr, dfr) = pressure_fn(p, r);
        let f = fl + fr + (r.u - l.u);
        let df = dfl + dfr;
        let p_new = (p - f / df).max(1e-12);
        if (p_new - p).abs() / (0.5 * (p_new + p)) < 1e-13 {
            return p_new;
        }
        p = p_new;
    }
    p
}

/// Star-region velocity u\* (Toro eq. 4.9).
fn star_velocity(l: GasState, r: GasState, p_star: f64) -> f64 {
    let (fl, _) = pressure_fn(p_star, l);
    let (fr, _) = pressure_fn(p_star, r);
    0.5 * (l.u + r.u) + 0.5 * (fr - fl)
}

/// Right-going shock speed (Sod's right wave is a shock).
fn right_shock_speed(r: GasState, p_star: f64) -> f64 {
    let c_r = sound_speed(r);
    r.u + c_r
        * ((GAMMA + 1.0) / (2.0 * GAMMA) * (p_star / r.p) + (GAMMA - 1.0) / (2.0 * GAMMA)).sqrt()
}

/// Sample the exact solution at ξ = (x − x₀)/t (Toro `sample`, Fig. 4.14).
fn sample(l: GasState, r: GasState, p_star: f64, u_star: f64, xi: f64) -> GasState {
    let c_l = sound_speed(l);
    let c_r = sound_speed(r);
    let g1 = (GAMMA - 1.0) / (GAMMA + 1.0);
    if xi <= u_star {
        if p_star > l.p {
            let s_l = l.u
                - c_l
                    * ((GAMMA + 1.0) / (2.0 * GAMMA) * (p_star / l.p)
                        + (GAMMA - 1.0) / (2.0 * GAMMA))
                    .sqrt();
            if xi <= s_l {
                l
            } else {
                let rho = l.rho * (p_star / l.p + g1) / (g1 * (p_star / l.p) + 1.0);
                GasState { rho, u: u_star, p: p_star }
            }
        } else {
            let c_star_l = c_l * (p_star / l.p).powf((GAMMA - 1.0) / (2.0 * GAMMA));
            let s_head = l.u - c_l;
            let s_tail = u_star - c_star_l;
            if xi <= s_head {
                l
            } else if xi >= s_tail {
                let rho = l.rho * (p_star / l.p).powf(1.0 / GAMMA);
                GasState { rho, u: u_star, p: p_star }
            } else {
                let u = 2.0 / (GAMMA + 1.0) * (c_l + (GAMMA - 1.0) / 2.0 * l.u + xi);
                let c = 2.0 / (GAMMA + 1.0) * (c_l + (GAMMA - 1.0) / 2.0 * (l.u - xi));
                let rho = l.rho * (c / c_l).powf(2.0 / (GAMMA - 1.0));
                let p = l.p * (c / c_l).powf(2.0 * GAMMA / (GAMMA - 1.0));
                GasState { rho, u, p }
            }
        }
    } else if p_star > r.p {
        let s_r = right_shock_speed(r, p_star);
        if xi >= s_r {
            r
        } else {
            let rho = r.rho * (p_star / r.p + g1) / (g1 * (p_star / r.p) + 1.0);
            GasState { rho, u: u_star, p: p_star }
        }
    } else {
        let c_star_r = c_r * (p_star / r.p).powf((GAMMA - 1.0) / (2.0 * GAMMA));
        let s_head = r.u + c_r;
        let s_tail = u_star + c_star_r;
        if xi >= s_head {
            r
        } else if xi <= s_tail {
            let rho = r.rho * (p_star / r.p).powf(1.0 / GAMMA);
            GasState { rho, u: u_star, p: p_star }
        } else {
            let u = 2.0 / (GAMMA + 1.0) * (-c_r + (GAMMA - 1.0) / 2.0 * r.u + xi);
            let c = 2.0 / (GAMMA + 1.0) * (c_r - (GAMMA - 1.0) / 2.0 * (r.u - xi));
            let rho = r.rho * (c / c_r).powf(2.0 / (GAMMA - 1.0));
            let p = r.p * (c / c_r).powf(2.0 * GAMMA / (GAMMA - 1.0));
            GasState { rho, u, p }
        }
    }
}

/// Exact primitive state at (x, t), diaphragm at x₀.
fn exact_state(l: GasState, r: GasState, x: f64, x0: f64, t: f64) -> GasState {
    let p_star = star_pressure(l, r);
    let u_star = star_velocity(l, r, p_star);
    sample(l, r, p_star, u_star, (x - x0) / t)
}

// ═════════════════════════════════════════════════════════════════════════════
//  Test 1 — the exact Riemann arbiter reproduces the analytic star state
// ═════════════════════════════════════════════════════════════════════════════

/// Verify the exact Riemann solver (used as the faithfulness arbiter) against the
/// closed-form star-state values for the Sod problem (CLAUDE.md §5.2).
#[test]
fn exact_riemann_reproduces_sod_star_state() {
    let l = GasState { rho: 1.0, u: 0.0, p: 1.0 };
    let r = GasState { rho: 0.125, u: 0.0, p: 0.1 };

    let p_star = star_pressure(l, r);
    let u_star = star_velocity(l, r, p_star);
    let rho_star_l = l.rho * (p_star / l.p).powf(1.0 / GAMMA);
    let g1 = (GAMMA - 1.0) / (GAMMA + 1.0);
    let rho_star_r = r.rho * (p_star / r.p + g1) / (g1 * (p_star / r.p) + 1.0);
    let shock_speed = right_shock_speed(r, p_star);

    println!("── exact Riemann star state (Sod, dimensionless) ──");
    println!("quantity,computed,reference");
    println!("P_star,{p_star:.5},0.30313");
    println!("u_star,{u_star:.5},0.92745");
    println!("rho_star_L,{rho_star_l:.5},0.42632");
    println!("rho_star_R,{rho_star_r:.5},0.26557");
    println!("shock_speed,{shock_speed:.5},1.75216");

    assert!((p_star - 0.30313).abs() < 1e-3, "P* = {p_star}");
    assert!((u_star - 0.92745).abs() < 1e-3, "u* = {u_star}");
    assert!((rho_star_l - 0.42632).abs() < 1e-3, "rho*_L = {rho_star_l}");
    assert!((rho_star_r - 0.26557).abs() < 1e-3, "rho*_R = {rho_star_r}");
    assert!((shock_speed - 1.75216).abs() < 1e-3, "S_shock = {shock_speed}");
}

// ═════════════════════════════════════════════════════════════════════════════
//  Sod (1978) Table II — the validation reference
// ═════════════════════════════════════════════════════════════════════════════

/// Sod (1978) Table II, γ = 1.4, τ = 0.2, diaphragm at x/L = 0.5.
/// Columns: x/L, ρ/ρ0, u/u0, P/P0. (CLAUDE.md §4.)
const SOD_TABLE_II: [(f64, f64, f64, f64); 9] = [
    (0.1, 1.000, 0.000, 1.000),
    (0.2, 1.000, 0.000, 1.000),
    (0.3, 0.869, 0.164, 0.822),
    (0.4, 0.426, 0.927, 0.303),
    (0.5, 0.426, 0.927, 0.303),
    (0.6, 0.426, 0.927, 0.303),
    (0.7, 0.426, 0.927, 0.303),
    (0.8, 0.266, 0.927, 0.303),
    (0.9, 0.125, 0.000, 0.100),
];

// ═════════════════════════════════════════════════════════════════════════════
//  Test 2 — the exact solution overlaid on Sod Table II (documents fidelity)
// ═════════════════════════════════════════════════════════════════════════════

/// Overlay the exact Riemann solution on Sod Table II at τ = 0.2. Constant-state
/// points must agree to < 0.01; the fan/contact points (x/L = 0.3, 0.4, 0.7) are
/// printed only — this is exactly the Table II coarseness the port validation
/// routes around via the arbiter.
#[test]
fn exact_riemann_matches_sod_table_ii() {
    let l = GasState { rho: 1.0, u: 0.0, p: 1.0 };
    let r = GasState { rho: 0.125, u: 0.0, p: 0.1 };
    let tau = 0.2;
    let x0 = 0.5;

    println!("── exact vs Sod Table II (τ = 0.2, dimensionless) ──");
    println!("x_over_L,rho_exact,rho_tab,u_exact,u_tab,P_exact,P_tab,faithful");
    for &(x, rho_t, u_t, p_t) in SOD_TABLE_II.iter() {
        let s = exact_state(l, r, x, x0, tau);
        let faithful = (s.rho - rho_t).abs() < 0.02
            && (s.u - u_t).abs() < 0.02
            && (s.p - p_t).abs() < 0.02;
        println!(
            "{x:.1},{:.3},{rho_t:.3},{:.3},{u_t:.3},{:.3},{p_t:.3},{faithful}",
            s.rho, s.u, s.p
        );
        // Assert only where Table II is a faithful sample of the exact solution.
        // Constant states agree to < 0.001; the sole faithful-but-in-fan point
        // (x/L = 0.3) carries ~1.1% Glimm's-method scatter, hence the 0.015 bound.
        if faithful {
            assert!((s.rho - rho_t).abs() < 0.015, "rho @ x={x}: {} vs {rho_t}", s.rho);
            assert!((s.u - u_t).abs() < 0.015, "u @ x={x}: {} vs {u_t}", s.u);
            assert!((s.p - p_t).abs() < 0.015, "P @ x={x}: {} vs {p_t}", s.p);
        }
    }
}

// ═════════════════════════════════════════════════════════════════════════════
//  Test 3 — the Rust RhoCentralFoam port vs Sod Table II
// ═════════════════════════════════════════════════════════════════════════════

fn poly_mesh_dir() -> PathBuf {
    Path::new(CASE_DIR).join("constant").join("polyMesh")
}

/// Writes `contents` to `verification_and_validation/<filename>` under the
/// crate root, creating the folder if needed. These `.csv` files are the
/// full generated datasets the committed `.md` V&V records reference; they
/// are gitignored and excluded from `cargo publish` (see this crate's
/// `Cargo.toml` `exclude` and the V&V folder README), so regenerating them
/// is just a matter of re-running this test.
fn write_vandv_csv(filename: &str, contents: &str) {
    use std::io::Write;
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("verification_and_validation");
    std::fs::create_dir_all(&dir)
        .unwrap_or_else(|e| panic!("failed to create {}: {e}", dir.display()));
    let path = dir.join(filename);
    let mut f = std::fs::File::create(&path)
        .unwrap_or_else(|e| panic!("failed to create {}: {e}", path.display()));
    f.write_all(contents.as_bytes())
        .unwrap_or_else(|e| panic!("failed to write {}: {e}", path.display()));
    println!("wrote V&V CSV: {}", path.display());
}

/// Build a `RhoCentralFoam` solver from the committed mesh + Sod initial fields,
/// configured to run to `t_end` seconds with dt = 1×10⁻⁶ s.
fn build_solver(t_end: f64) -> RhoCentralFoam {
    let case = Path::new(CASE_DIR);
    let mesh = read_poly_mesh(&poly_mesh_dir()).expect("read_poly_mesh failed");
    let n = mesh.n_cells;

    let p0 = read_vol_scalar_field(&case.join("0").join("p"), n).expect("read 0/p failed");
    let t0 = read_vol_scalar_field(&case.join("0").join("T"), n).expect("read 0/T failed");
    let u0 = read_vol_vector_field_full(&case.join("0").join("U"), &mesh).expect("read 0/U failed");

    let control = ControlDict {
        start: StartControl::StartTime(0.0),
        stop: StopControl::EndTime(t_end),
        delta_t: 1e-6,
        ..ControlDict::default()
    };
    let mut solver =
        RhoCentralFoam::new(mesh.clone(), control, FvSchemes::default(), FvSolution::default());

    let rho = solver.rho.internal.as_mut_slice();
    for c in 0..n {
        rho[c] = p0[c] / (R_GAS * t0[c]);
    }
    let rho_vals: Vec<f64> = solver.rho.internal.as_slice().to_vec();
    let e = solver.e.internal.as_mut_slice();
    for c in 0..n {
        e[c] = p0[c] / ((GAMMA - 1.0) * rho_vals[c]);
    }
    {
        let p_dst = solver.p.internal.as_mut_slice();
        for c in 0..n {
            p_dst[c] = p0[c];
        }
    }
    solver.u = u0;
    solver
}

/// Linear interpolation of a cell-centred field (sorted ascending by x) onto an
/// arbitrary position `x`. Clamps to the end cells outside the domain.
fn interp_at(xs: &[f64], vals: &[f64], x: f64) -> f64 {
    let n = xs.len();
    if x <= xs[0] {
        return vals[0];
    }
    if x >= xs[n - 1] {
        return vals[n - 1];
    }
    let mut i = 0;
    while i + 1 < n && xs[i + 1] < x {
        i += 1;
    }
    let t = (x - xs[i]) / (xs[i + 1] - xs[i]);
    vals[i] + t * (vals[i + 1] - vals[i])
}

/// Validate the Rust `rhoCentralFoam` port against Sod (1978) Table II.
///
/// Runs to τ = 0.2, interpolates the port profile onto the 9 Table II
/// x-locations, redimensionalises Table II to SI, and asserts agreement at the
/// points where Table II is a faithful sample of the exact solution (fan/contact
/// points are report-only — see module header).
#[test]
fn rho_central_foam_matches_sod_table_ii() {
    let mut solver = build_solver(T_TAU_02);
    solver.run().expect("solver run failed");

    let mesh = solver.mesh.clone();
    let n = mesh.n_cells;

    // Cell-centred profiles, sorted by x for interpolation.
    let mut idx: Vec<usize> = (0..n).collect();
    idx.sort_by(|&a, &b| mesh.cell_centres[a].x.partial_cmp(&mesh.cell_centres[b].x).unwrap());
    let xs: Vec<f64> = idx.iter().map(|&c| mesh.cell_centres[c].x).collect();
    let rho_p: Vec<f64> = idx.iter().map(|&c| solver.rho.internal.as_slice()[c]).collect();
    let u_p: Vec<f64> = idx.iter().map(|&c| solver.u.internal.as_slice()[c].x).collect();
    let p_p: Vec<f64> = idx.iter().map(|&c| solver.p.internal.as_slice()[c]).collect();

    // Sanity: finite everywhere.
    for (name, f) in [("rho", &rho_p), ("u", &u_p), ("p", &p_p)] {
        let bad = f.iter().filter(|v| !v.is_finite()).count();
        assert_eq!(bad, 0, "{bad} non-finite {name} cells (solver diverged)");
    }

    let l = GasState { rho: RHO_LEFT, u: 0.0, p: P_LEFT };
    let r = GasState { rho: RHO_RIGHT, u: 0.0, p: P_RIGHT };

    // Peak magnitudes for relative-error normalisation.
    let rho_scale = RHO_LEFT;
    let u_scale = 0.92745 * U0; // |u*|, the profile peak
    let p_scale = P_LEFT;

    println!("── RhoCentralFoam port vs Sod Table II (τ = 0.2, t = {T_TAU_02:.6} s) ──");
    let header =
        "x_over_L,x_m,rho_port,rho_tab,rho_exact,u_port,u_tab,u_exact,p_port,p_tab,p_exact,faithful";
    println!("{header}");
    let mut csv = String::new();
    csv.push_str(header);
    csv.push('\n');

    let mut worst_p = 0.0_f64;
    let mut worst_u = 0.0_f64;
    let mut worst_rho = 0.0_f64;

    for &(xl, rho_t_nd, u_t_nd, p_t_nd) in SOD_TABLE_II.iter() {
        let x_si = (xl - 0.5) * L0; // x/L = x_OF/L0 + 0.5  ⇒  x_OF = (x/L − 0.5)·L0
        // Redimensionalise Table II to SI.
        let rho_tab = rho_t_nd * RHO0;
        let u_tab = u_t_nd * U0;
        let p_tab = p_t_nd * P0;
        // Exact solution (arbiter) at this point, in SI.
        let ex = exact_state(l, r, x_si, X_DIAPHRAGM, T_TAU_02);
        // Port, interpolated onto the reference x.
        let rho_port = interp_at(&xs, &rho_p, x_si);
        let u_port = interp_at(&xs, &u_p, x_si);
        let p_port = interp_at(&xs, &p_p, x_si);

        // Is Table II faithful to the exact solution here? (per-variable)
        let rho_ok = (rho_tab - ex.rho).abs() / rho_scale < 0.02;
        let u_ok = (u_tab - ex.u).abs() / u_scale < 0.02;
        let p_ok = (p_tab - ex.p).abs() / p_scale < 0.02;
        let faithful = rho_ok && u_ok && p_ok;

        let row = format!(
            "{xl:.1},{x_si:.1},{rho_port:.4},{rho_tab:.4},{:.4},{u_port:.2},{u_tab:.2},{:.2},{p_port:.1},{p_tab:.1},{:.1},{faithful}",
            ex.rho, ex.u, ex.p
        );
        println!("{row}");
        csv.push_str(&row);
        csv.push('\n');

        // Hard-assert the port against Table II only where Table II is faithful.
        if rho_ok {
            let e = (rho_port - rho_tab).abs() / rho_scale;
            worst_rho = worst_rho.max(e);
            assert!(e < 0.05, "density @ x/L={xl}: port {rho_port:.4} vs Table II {rho_tab:.4} (rel {e:.3})");
        }
        if u_ok {
            let e = (u_port - u_tab).abs() / u_scale;
            worst_u = worst_u.max(e);
            assert!(e < 0.05, "velocity @ x/L={xl}: port {u_port:.2} vs Table II {u_tab:.2} (rel {e:.3})");
        }
        if p_ok {
            let e = (p_port - p_tab).abs() / p_scale;
            worst_p = worst_p.max(e);
            assert!(e < 0.05, "pressure @ x/L={xl}: port {p_port:.1} vs Table II {p_tab:.1} (rel {e:.3})");
        }
    }

    println!(
        "── worst faithful-point rel error: p={worst_p:.4}, u={worst_u:.4}, rho={worst_rho:.4} ──"
    );
    write_vandv_csv("sod_shock_tube_rhocentralfoam_vs_table_ii.csv", &csv);
}
