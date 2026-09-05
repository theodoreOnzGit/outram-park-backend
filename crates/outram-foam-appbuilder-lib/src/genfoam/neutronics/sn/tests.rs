// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: https://gitlab.com/foam-for-nuclear/GeN-Foam
//   Upstream commit: 652b3da
//   Upstream source: src/classes/neutronics/SN/ (V&V of the port)
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//   Upstream license: GPL-3.0
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
//
// This offering is not approved or endorsed by EPFL, the OpenFOAM Foundation,
// nor OpenCFD Limited, producer and distributor of the OpenFOAM(R) software.

//! # Verification & validation — discrete-ordinates (S_N) transport
//!
//! ## Methodology
//!
//! The S_N solver is checked against closed-form transport results and against
//! the [`super::super::diffusion`] solver in the limit where the two must agree:
//!
//! 1. **Quadrature consistency** ([`super::quadrature`] tests) — the S2/S4/S6
//!    level-symmetric weights sum to `4*pi` and reproduce the first / second
//!    angular moments of the sphere (`0` and `4*pi/3`). Verifies the ordinate
//!    set before it is used.
//!
//! 2. **Infinite-medium `k_inf`, no scattering** (`sn_kinf_no_scatter`).
//!    Reflective (`ZeroGradient`) faces remove all leakage, so the discrete
//!    streaming term vanishes for the uniform flux (a closed cell has
//!    `sum_faces Omega . Sf = 0`) and `k_inf = nu Sigma_f / Sigma_a` **exactly**,
//!    independent of quadrature order and mesh. Verifies the collision term and
//!    the power-iteration `k` update.
//!
//! 3. **Infinite-medium `k_inf`, with self-scatter** (`sn_kinf_with_scatter`).
//!    Same medium plus a large isotropic self-scatter `Sigma_{s,0->0}`. Because
//!    scattering conserves neutrons, `k_inf` is still `nu Sigma_f / Sigma_a` —
//!    the total cross section on the LHS (`Sigma_r + Sigma_{s,0->0}`) and the
//!    in-scatter source cancel. Verifies the scattering-source inner iteration
//!    and the `Sigma_t` reconstruction.
//!
//! 4. **Diffusion limit** (`sn_approaches_diffusion_limit`). On a large,
//!    scattering-dominated, weakly-absorbing bare slab (vacuum faces), transport
//!    theory reduces to diffusion, so the S_N `k_eff` must approach the
//!    `k_eff` the [`super::super::diffusion`] solver gives for the same slab with
//!    `D = 1/(3 Sigma_t)`. The residual gap is the first-order-upwind streaming's
//!    numerical diffusion plus the genuine (small) transport-diffusion
//!    difference; it shrinks under mesh refinement.
//!
//! 5. **Angular convergence** (`sn_order_convergence`). Refining the quadrature
//!    S2 -> S4 -> S6 changes `k_eff` by less and less on the smooth slab flux —
//!    the ordinate set is converged.
//!
//! ## Results (measured 2026-08-07, `cargo test --release`, rustc 1.97.0)
//!
//! | Test | Measured | Reference | Deviation |
//! |---|---|---|---|
//! | `sn_kinf_no_scatter` | `k_inf = 3.000000000` (2 outers) | 3.000000000 | `4.44e-16` |
//! | `sn_kinf_with_scatter` (Σ_s = 1.40) | `k_inf = 2.999999584` (37 outers) | 3.000000000 | `4.16e-7` |
//! | `sn_order_convergence` | `k_S2 = 0.9811914`, `k_S4 = 0.9824130`, `k_S6 = 0.9825272` | — | `|Δ_{42}| = 1.222e-3`, `|Δ_{64}| = 1.142e-4` |
//! | `sn_approaches_diffusion_limit` (L = 30 m, n = 300, S6) | `k_SN = 1.0021486` | `k_diff = 1.0016921` | `4.557e-4` (**45.6 pcm**) |
//!
//! **Interpretation.** The scatter-free infinite medium is reproduced to
//! machine precision, so the fission-source normalisation and the ordinate
//! weights sum correctly. With self-scatter the residual `4.16e-7` is the
//! power-iteration convergence tolerance, not a discretisation error — it is
//! reached after 37 outers rather than 2. Angular convergence is clean and
//! roughly first-order in the ordinate count: refining S2→S4 moves `k_eff` by
//! 1222 pcm but S4→S6 by only 114 pcm, a ~10.7× reduction, so S6 is converged
//! for a smooth slab flux. Finally, in the optically thick, weakly absorbing
//! limit where diffusion theory is asymptotically exact, S_N lands 45.6 pcm
//! from the diffusion answer — small, of the expected sign, and consistent with
//! residual transport anisotropy plus the two solvers' differing spatial
//! discretisations.
//!
//! These are **verification** results against closed-form limits and an
//! independent in-crate solver — not validation against a published benchmark.
//! All are deterministic; repeat runs on the same build reproduce these digits.

use std::sync::Arc;

use outram_foam_basic_lib::interface::one_dimensional_meshing::create_one_d_mesh;
use outram_foam_basic_lib::prelude::{BoundaryCondition, FvMesh};
use uom::si::area::square_meter;
use uom::si::f64::{Area, Length};
use uom::si::length::meter;

use super::{QuadratureOrder, SnNeutronics, SnSettings};
use crate::genfoam::neutronics::diffusion::{DiffusionNeutronics, DiffusionSettings};
use crate::genfoam::neutronics::xs::input::{
    NuclearDataInput, StateInput, ZoneConstantsInput, ZoneStateInput,
};
use crate::genfoam::neutronics::xs::CrossSectionData;

/// Physical group constants for a single-group homogeneous medium.
struct OneGroup {
    /// Diffusion coefficient `D` (m) — for the S_N solve, set to
    /// `1/(3 Sigma_t)` so the diffusion comparison is consistent.
    d: f64,
    /// `nu Sigma_f` (m^-1).
    nu_sigma_f: f64,
    /// Absorption cross section `Sigma_a` (m^-1); the one-group removal.
    sigma_a: f64,
    /// Isotropic self-scatter `Sigma_{s,0->0}` (m^-1).
    sigma_s: f64,
}

/// Build a one-group, single-zone `CrossSectionData` with optional self-scatter
/// and no feedback variables.
fn one_group_xs(gc: &OneGroup) -> CrossSectionData {
    let constants = ZoneConstantsInput {
        iv: vec![1.0e-6],
        disc_factor: vec![1.0],
        integral_flux: vec![1.0],
        beta: vec![0.0065],
        lambda: vec![0.08],
        ..ZoneConstantsInput::default()
    };
    let zone = ZoneStateInput {
        name: "core".to_string(),
        d: vec![gc.d],
        nu_sigma_eff: vec![gc.nu_sigma_f],
        sigma_pow: vec![1.0],
        sigma_removal: vec![gc.sigma_a],
        chi_prompt: vec![1.0],
        chi_delayed: vec![1.0],
        scattering: vec![vec![vec![gc.sigma_s]]], // 1 moment, 1x1 self-scatter
        constants: Some(constants),
    };
    let reference = StateInput {
        name: "reference".to_string(),
        parameters: Default::default(),
        zones: vec![zone],
    };
    let input = NuclearDataInput {
        energy_groups: 1,
        prec_groups: 1,
        legendre_moments: 1,
        poly_spline_mode: 1,
        fast_neutrons: false,
        xs_variables: Vec::new(),
        do_not_parametrize: Vec::new(),
        states: vec![reference],
    };
    CrossSectionData::from_input(&input).expect("valid one-group nuclear data")
}

fn slab_mesh(length_m: f64, n: i64) -> Arc<FvMesh> {
    Arc::new(
        create_one_d_mesh(
            Length::new::<meter>(length_m),
            Area::new::<square_meter>(1.0),
            n,
        )
        .expect("valid slab mesh"),
    )
}

fn reflective() -> Vec<BoundaryCondition<f64>> {
    vec![
        BoundaryCondition::ZeroGradient,
        BoundaryCondition::ZeroGradient,
    ]
}

fn vacuum() -> Vec<BoundaryCondition<f64>> {
    vec![
        BoundaryCondition::FixedValue(0.0),
        BoundaryCondition::FixedValue(0.0),
    ]
}

#[test]
fn sn_kinf_no_scatter() {
    let gc = OneGroup {
        d: 1.0 / 3.0,
        nu_sigma_f: 0.30,
        sigma_a: 0.10,
        sigma_s: 0.0,
    };
    let xs = one_group_xs(&gc);
    let mesh = slab_mesh(1.0, 4);
    let zone_of_cell = vec![0usize; mesh.n_cells];
    let mut model = SnNeutronics::with_cross_sections(
        mesh,
        &xs,
        &zone_of_cell,
        &[],
        &reflective(),
        QuadratureOrder::S4,
        SnSettings::default(),
    )
    .expect("build SN model");

    let report = model.solve_eigenvalue().expect("eigenvalue converges");
    let k_inf = gc.nu_sigma_f / gc.sigma_a; // 3.0
    eprintln!(
        "SN k_inf (no scatter): measured {:.9}, analytic {:.9}, |Δ| {:.2e}, outer {}",
        report.k_eff,
        k_inf,
        (report.k_eff - k_inf).abs(),
        report.outer_iterations
    );
    assert!(report.converged);
    assert!(
        (report.k_eff - k_inf).abs() < 1e-6,
        "k_inf = {} != {}",
        report.k_eff,
        k_inf
    );
}

#[test]
fn sn_kinf_with_scatter() {
    // Large self-scatter must not change k_inf = nu Sigma_f / Sigma_a.
    let gc = OneGroup {
        d: 1.0 / (3.0 * 1.5),
        nu_sigma_f: 0.30,
        sigma_a: 0.10,
        sigma_s: 1.40, // Sigma_t = 1.5
    };
    let xs = one_group_xs(&gc);
    let mesh = slab_mesh(1.0, 6);
    let zone_of_cell = vec![0usize; mesh.n_cells];
    let mut model = SnNeutronics::with_cross_sections(
        mesh,
        &xs,
        &zone_of_cell,
        &[],
        &reflective(),
        QuadratureOrder::S6,
        SnSettings::default(),
    )
    .expect("build SN model");

    let report = model.solve_eigenvalue().expect("eigenvalue converges");
    let k_inf = gc.nu_sigma_f / gc.sigma_a; // 3.0
    eprintln!(
        "SN k_inf (self-scatter Σs=1.40): measured {:.9}, analytic {:.9}, |Δ| {:.2e}, outer {}",
        report.k_eff,
        k_inf,
        (report.k_eff - k_inf).abs(),
        report.outer_iterations
    );
    assert!(report.converged);
    assert!(
        (report.k_eff - k_inf).abs() < 1e-6,
        "k_inf with scatter = {} != {}",
        report.k_eff,
        k_inf
    );
}

/// Solve the same one-group bare slab with diffusion and with S_N, return
/// `(k_diffusion, k_sn)`.
fn diffusion_vs_sn(gc: &OneGroup, length_m: f64, n: i64, order: QuadratureOrder) -> (f64, f64) {
    let xs = one_group_xs(gc);

    let mesh = slab_mesh(length_m, n);
    let zone_of_cell = vec![0usize; mesh.n_cells];
    let mut dif = DiffusionNeutronics::new(
        mesh.clone(),
        &xs,
        &zone_of_cell,
        &[],
        &vacuum(),
        DiffusionSettings::default(),
    )
    .expect("build diffusion model");
    let k_dif = dif
        .solve_eigenvalue()
        .expect("diffusion eigenvalue converges")
        .k_eff;

    let settings = SnSettings {
        inner_tolerance: 1e-6,
        max_inner_iterations: 40,
        ..SnSettings::default()
    };
    let mut sn = SnNeutronics::with_cross_sections(
        mesh,
        &xs,
        &zone_of_cell,
        &[],
        &vacuum(),
        order,
        settings,
    )
    .expect("build SN model");
    let k_sn = sn
        .solve_eigenvalue()
        .expect("SN eigenvalue converges")
        .k_eff;

    (k_dif, k_sn)
}

#[test]
fn sn_approaches_diffusion_limit() {
    // Large, scattering-dominated, weakly-absorbing slab: Sigma_t = 1.0,
    // c = Sigma_s / Sigma_t = 0.8, D = 1/(3 Sigma_t). Transport -> diffusion.
    let gc = OneGroup {
        d: 1.0 / 3.0,
        nu_sigma_f: 0.204,
        sigma_a: 0.20,
        sigma_s: 0.80,
    };
    let length = 30.0;
    let n = 300; // dx = 0.1 m, Sigma_t*dx = 0.1 (fine enough to limit upwind diffusion)
    let (k_dif, k_sn) = diffusion_vs_sn(&gc, length, n, QuadratureOrder::S6);
    let rel = (k_sn - k_dif).abs() / k_dif;
    eprintln!(
        "SN vs diffusion (L={length} m, n={n}, S6): k_dif {:.7}, k_sn {:.7}, rel {:.3e} ({:.1} pcm)",
        k_dif,
        k_sn,
        rel,
        (k_sn - k_dif) * 1e5
    );
    // The residual is dominated by first-order-upwind numerical diffusion in the
    // streaming term (shrinks under refinement) plus the genuine, small
    // transport-diffusion difference for this weakly-leaking slab.
    assert!(
        rel < 1.0e-2,
        "SN k {k_sn} deviates from diffusion k {k_dif} by {rel:.3e} (> 1%)"
    );
}

#[test]
fn sn_order_convergence() {
    // On a smooth slab flux, refining the quadrature must change k_eff by less
    // and less: |k_S4 - k_S2| > |k_S6 - k_S4|.
    let gc = OneGroup {
        d: 1.0 / 3.0,
        nu_sigma_f: 0.204,
        sigma_a: 0.20,
        sigma_s: 0.80,
    };
    let length = 20.0;
    let n = 200;
    let k2 = diffusion_vs_sn(&gc, length, n, QuadratureOrder::S2).1;
    let k4 = diffusion_vs_sn(&gc, length, n, QuadratureOrder::S4).1;
    let k6 = diffusion_vs_sn(&gc, length, n, QuadratureOrder::S6).1;
    let d42 = (k4 - k2).abs();
    let d64 = (k6 - k4).abs();
    eprintln!(
        "SN order convergence: k_S2 {k2:.7}, k_S4 {k4:.7}, k_S6 {k6:.7}; |Δ42| {d42:.3e}, |Δ64| {d64:.3e}"
    );
    assert!(
        d64 <= d42 + 1e-9,
        "angular convergence not monotone: |Δ64| {d64:.3e} > |Δ42| {d42:.3e}"
    );
}

#[test]
fn scaffold_reports_not_implemented() {
    use crate::genfoam::neutronics::{NeutronicsError, NeutronicsModelKind};
    let mesh = slab_mesh(1.0, 4);
    let mut sn = SnNeutronics::new(mesh, 1, 0);
    assert!(matches!(
        sn.solve_eigenvalue(),
        Err(NeutronicsError::ModelNotImplemented(
            NeutronicsModelKind::Sn
        ))
    ));
    assert!(sn.quadrature().is_none());
}
