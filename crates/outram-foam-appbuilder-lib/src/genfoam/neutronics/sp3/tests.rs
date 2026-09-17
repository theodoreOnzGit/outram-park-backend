// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: https://gitlab.com/foam-for-nuclear/GeN-Foam
//   Upstream commit: 652b3da
//   Upstream source: src/classes/neutronics/SP3/ (V&V of the port)
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

//! # Verification & validation — SP3 transport
//!
//! ## Methodology
//!
//! SP3 has no closed-form `k_eff` for a finite reactor, so it is verified
//! against its two defining limits on a homogeneous (single-zone), physically
//! consistent one-group medium (`Sigma_t = Sigma_a + Sigma_{s,g->g}`,
//! `D_0 = 1/(3 Sigma_t)`, isotropic scatter):
//!
//! 1. **Infinite-medium reduction to diffusion** (`kinf_reduces_to_diffusion`).
//!    With reflective (`ZeroGradient`) boundaries there is no leakage, the flux
//!    is uniform, and the second SP3 moment `phi2` is *identically zero*
//!    (`-div(D2 grad phi2) + A2 phi2 = (2/3)(Sigma_r Phi0 - Q)` has zero RHS in a
//!    critical infinite medium). SP3 therefore collapses to diffusion and
//!    `k_inf = nu Sigma_f / Sigma_a` exactly. The test asserts both
//!    `|k_SP3 - k_inf| < 1e-7` and `max|phi2| < 1e-9`.
//!
//! 2. **Convergence to diffusion as the system grows**
//!    (`approaches_diffusion_as_slab_grows`). A bare slab of width `L` has
//!    geometric buckling `B^2 = (pi/L)^2`; as `L` grows the flux becomes
//!    near-isotropic (`B^2 -> 0`), so SP3 must converge onto the diffusion
//!    `k_eff`. The test builds an SP3 and a diffusion model on the *same* cross
//!    sections and mesh for a growing slab and asserts the `|k_SP3 - k_diff|`
//!    gap shrinks monotonically.
//!
//! 3. **Anisotropy correction is active on a small slab** (`differs_on_small_slab`).
//!    On an optically-leaky slab SP3 and diffusion differ measurably (the SP3
//!    second moment is non-negligible), confirming the correction is not a no-op.
//!
//! 4. **Null transient** (`null_transient_holds_steady`). Starting from the
//!    converged eigenvector + equilibrium precursors with `k_eff` frozen, the
//!    backward-Euler SP3 transient reproduces the steady state (invariant total
//!    power), verifying the transient assembly is consistent with the eigenvalue
//!    assembly.
//!
//! ## Results (measured 2026-07-15, this port, `cargo test --release`, CG inner
//! solve; single-group medium `Sigma_t = 1.0`, `Sigma_a = 0.2`,
//! `Sigma_{s,g->g} = 0.8`, `D_0 = 1/3`, `nu Sigma_f = 0.24`, `k_inf = 1.2`)
//!
//! - **Infinite medium**: SP3 `k_inf = 1.200000000` vs analytic `1.2`
//!   (`|Δ| = 2.2e-15`), `max|phi2| = 5.7e-16` (identically zero to round-off), 2
//!   outer iterations. SP3 collapses exactly onto diffusion. PASS.
//! - **Convergence to diffusion** (bare slab, 200 cells, zero-flux edges):
//!   the `|k_SP3 - k_diff|` gap falls `2.60e-2` (L = 5 m) → `3.66e-3` (10) →
//!   `2.96e-4` (20) → `1.98e-5` (40) as buckling `B^2 = (pi/L)^2 -> 0`, the
//!   expected reduction to diffusion for a near-isotropic flux. PASS.
//! - **Small-slab correction** (L = 5 m): `k_SP3 = 0.7497361`,
//!   `k_diff = 0.7237810`, a `+2595 pcm` (`3.6 %`) SP3 correction — bounded and
//!   active, not a no-op. (Sign is with the zero-flux/Mark SP3 edge on both
//!   moments; a Marshak/albedo edge would shift it. No transport reference is
//!   asserted here — only that the correction is present and bounded.) PASS.
//! - **Null transient** (L = 10 m, 100 cells): total power
//!   `4.166667 -> 4.166668` over 50 steps of `dt = 1 ms`, relative drift
//!   `3.0e-7 < 1e-6`. PASS.
//!
//! The exact measured numbers are emitted to stderr by each test (run with
//! `--nocapture`); the assertions below encode the pass criteria.

use std::f64::consts::PI;
use std::sync::Arc;

use outram_foam_basic_lib::interface::one_dimensional_meshing::create_one_d_mesh;
use outram_foam_basic_lib::prelude::{BoundaryCondition, FvMesh};
use uom::si::area::square_meter;
use uom::si::f64::{Area, Length, Time};
use uom::si::length::meter;
use uom::si::time::second;

use super::{Sp3Neutronics, Sp3Settings};
use crate::genfoam::neutronics::diffusion::{DiffusionNeutronics, DiffusionSettings};
use crate::genfoam::neutronics::xs::input::{
    NuclearDataInput, StateInput, ZoneConstantsInput, ZoneStateInput,
};
use crate::genfoam::neutronics::xs::CrossSectionData;

/// A physically consistent one-group homogeneous medium (`D_0 = 1/(3 Sigma_t)`,
/// isotropic self-scatter).
struct OneGroup {
    /// Total cross section `Sigma_t` (m^-1).
    sigma_t: f64,
    /// Absorption = removal cross section `Sigma_a` (m^-1, one group).
    sigma_a: f64,
    /// Fission production `nu Sigma_f` (m^-1).
    nu_sigma_f: f64,
}

impl OneGroup {
    /// Isotropic self-scatter `Sigma_{s,g->g} = Sigma_t - Sigma_a`.
    fn self_scatter(&self) -> f64 {
        self.sigma_t - self.sigma_a
    }

    /// 0th-moment diffusion coefficient `D_0 = 1/(3 Sigma_t)` (m).
    fn d0(&self) -> f64 {
        1.0 / (3.0 * self.sigma_t)
    }

    /// Infinite-medium eigenvalue `k_inf = nu Sigma_f / Sigma_a`.
    fn k_inf(&self) -> f64 {
        self.nu_sigma_f / self.sigma_a
    }

    /// Diffusion bare-slab eigenvalue `k = nu Sigma_f / (Sigma_a + D_0 (pi/L)^2)`.
    fn diffusion_slab_k(&self, length: f64) -> f64 {
        self.nu_sigma_f / (self.sigma_a + self.d0() * (PI / length).powi(2))
    }
}

/// Build a one-group, single-zone, one-precursor `CrossSectionData` with
/// isotropic self-scatter and no feedback variables.
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
        d: vec![gc.d0()],
        nu_sigma_eff: vec![gc.nu_sigma_f],
        sigma_pow: vec![1.0],
        sigma_removal: vec![gc.sigma_a],
        chi_prompt: vec![1.0],
        chi_delayed: vec![1.0],
        // 1 Legendre moment, 1x1 scattering matrix, self-scatter Sigma_{s,g->g}.
        scattering: vec![vec![vec![gc.self_scatter()]]],
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

fn slab_mesh(n: i64) -> Arc<FvMesh> {
    Arc::new(
        create_one_d_mesh(Length::new::<meter>(1.0), Area::new::<square_meter>(1.0), n)
            .expect("valid slab mesh"),
    )
}

fn slab_mesh_length(length: f64, n: i64) -> Arc<FvMesh> {
    Arc::new(
        create_one_d_mesh(
            Length::new::<meter>(length),
            Area::new::<square_meter>(1.0),
            n,
        )
        .expect("valid slab mesh"),
    )
}

fn medium() -> OneGroup {
    OneGroup {
        sigma_t: 1.0,
        sigma_a: 0.2,
        nu_sigma_f: 0.24,
    }
}

#[test]
fn kinf_reduces_to_diffusion() {
    let gc = medium();
    let xs = one_group_xs(&gc);
    let mesh = slab_mesh(4);
    let bc = vec![
        BoundaryCondition::ZeroGradient,
        BoundaryCondition::ZeroGradient,
    ];
    let zone_of_cell = vec![0usize; mesh.n_cells];
    let mut model = Sp3Neutronics::with_cross_sections(
        mesh,
        &xs,
        &zone_of_cell,
        &[],
        &bc,
        Sp3Settings::default(),
    )
    .expect("build SP3 model");

    let report = model.solve_eigenvalue().expect("SP3 eigenvalue converges");
    let k_inf = gc.k_inf();
    // Second moment must vanish in an infinite medium (SP3 == diffusion).
    let max_phi2 = model.flux_star2()[0]
        .internal
        .as_slice()
        .iter()
        .fold(0.0_f64, |a, &v| a.max(v.abs()));
    eprintln!(
        "SP3 k_inf: measured {:.9}, analytic {:.9}, |Δ| {:.2e}, max|phi2| {:.2e}, outer iters {}",
        report.k_eff,
        k_inf,
        (report.k_eff - k_inf).abs(),
        max_phi2,
        report.outer_iterations
    );
    assert!(report.converged);
    assert!(
        (report.k_eff - k_inf).abs() < 1e-7,
        "SP3 k_inf {} != diffusion k_inf {}",
        report.k_eff,
        k_inf
    );
    assert!(
        max_phi2 < 1e-9,
        "phi2 not zero in infinite medium: {max_phi2:e}"
    );
}

/// Solve the same cross sections + slab with both SP3 and diffusion.
fn sp3_and_diffusion_k(gc: &OneGroup, length: f64, n: i64) -> (f64, f64) {
    let xs = one_group_xs(gc);
    let bc = vec![
        BoundaryCondition::FixedValue(0.0),
        BoundaryCondition::FixedValue(0.0),
    ];

    let mesh = slab_mesh_length(length, n);
    let zone_of_cell = vec![0usize; mesh.n_cells];
    let mut sp3 = Sp3Neutronics::with_cross_sections(
        mesh.clone(),
        &xs,
        &zone_of_cell,
        &[],
        &bc,
        Sp3Settings::default(),
    )
    .expect("build SP3 model");
    let k_sp3 = sp3.solve_eigenvalue().expect("SP3 converges").k_eff;

    let mut diff = DiffusionNeutronics::new(
        mesh,
        &xs,
        &zone_of_cell,
        &[],
        &bc,
        DiffusionSettings::default(),
    )
    .expect("build diffusion model");
    let k_diff = diff.solve_eigenvalue().expect("diffusion converges").k_eff;

    (k_sp3, k_diff)
}

#[test]
fn approaches_diffusion_as_slab_grows() {
    let gc = medium();
    let mut last_gap = f64::INFINITY;
    for &length in &[5.0_f64, 10.0, 20.0, 40.0] {
        let (k_sp3, k_diff) = sp3_and_diffusion_k(&gc, length, 200);
        let gap = (k_sp3 - k_diff).abs();
        eprintln!(
            "L={:>4} m: k_SP3 {:.7}, k_diff {:.7}, gap {:.3e} (diffusion analytic {:.7})",
            length,
            k_sp3,
            k_diff,
            gap,
            gc.diffusion_slab_k(length)
        );
        assert!(
            gap < last_gap,
            "SP3–diffusion gap did not shrink as slab grew: {gap:.3e} !< {last_gap:.3e}"
        );
        last_gap = gap;
    }
}

#[test]
fn differs_on_small_slab() {
    let gc = medium();
    // A leaky slab: buckling is large, so anisotropy (phi2) is non-negligible.
    let (k_sp3, k_diff) = sp3_and_diffusion_k(&gc, 5.0, 200);
    let rel = (k_sp3 - k_diff).abs() / k_diff;
    eprintln!(
        "small slab (L=5 m): k_SP3 {:.7}, k_diff {:.7}, rel diff {:.3e} ({:+.1} pcm)",
        k_sp3,
        k_diff,
        rel,
        (k_sp3 - k_diff) * 1e5
    );
    assert!(
        rel > 1e-4,
        "SP3 indistinguishable from diffusion on a leaky slab (correction inactive): rel {rel:e}"
    );
    // The correction stays physically bounded (a few percent), not a blow-up.
    assert!(rel < 0.1, "SP3 correction implausibly large: rel {rel:e}");
}

#[test]
fn null_transient_holds_steady() {
    let gc = medium();
    let xs = one_group_xs(&gc);
    let mesh = slab_mesh_length(10.0, 100);
    let bc = vec![
        BoundaryCondition::FixedValue(0.0),
        BoundaryCondition::FixedValue(0.0),
    ];
    let zone_of_cell = vec![0usize; mesh.n_cells];
    let mut model = Sp3Neutronics::with_cross_sections(
        mesh,
        &xs,
        &zone_of_cell,
        &[],
        &bc,
        Sp3Settings::default(),
    )
    .expect("build SP3 model");

    model.solve_eigenvalue().expect("SP3 eigenvalue converges");
    let p0 = model.state().total_power_raw();
    assert!(p0 > 0.0, "eigenvalue power must be positive, got {p0}");

    let dt = Time::new::<second>(1.0e-3);
    for _ in 0..50 {
        model.step(dt).unwrap();
    }
    let p1 = model.state().total_power_raw();
    let rel = (p1 - p0).abs() / p0;
    eprintln!("SP3 null transient: P0 {p0:.6e}, P50 {p1:.6e}, rel drift {rel:.3e}");
    assert!(
        rel < 1e-6,
        "SP3 null transient drifted: relative change {rel:e}"
    );
}

#[test]
fn scaffold_reports_not_implemented() {
    // A state-only scaffold (no cross sections) cannot solve.
    let mut sp3 = Sp3Neutronics::new(slab_mesh(4), 1, 0);
    assert!(matches!(
        sp3.solve_eigenvalue(),
        Err(
            crate::genfoam::neutronics::NeutronicsError::ModelNotImplemented(
                crate::genfoam::neutronics::NeutronicsModelKind::Sp3
            )
        )
    ));
}
