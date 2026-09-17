// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: https://gitlab.com/foam-for-nuclear/GeN-Foam
//   Upstream commit: 652b3da
//   Upstream source: src/classes/neutronics/include/precEq.H (V&V of the port)
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

//! # Verification — circulating-fuel precursor drift
//!
//! ## Methodology
//!
//! Two properties are checked, both of which hold for *any* mesh, cross
//! sections and flow field, so neither is fitted to a case:
//!
//! 1. **Reduction.** With no flow and no diffusion the transport equation is
//!    algebraic and must give back the stationary-fuel `chi_eff` collapse
//!    exactly. This is the check that the drift path is an extension of the
//!    existing solver rather than a second, silently divergent one.
//!
//! 2. **Conservation.** The precursor boundary conditions are zero gradient and
//!    the flux through a wall is zero, so advection and diffusion only move
//!    precursors about: the volume integral of the delayed source must equal
//!    `beta_tot` times the volume integral of the fission source divided by
//!    `k_eff`, whatever the velocity field. A drift model that leaked delayed
//!    neutrons would fail this, and would be wrong in the direction that
//!    flatters `k_eff` least visibly.
//!
//! ## Results (measured 2026-09-15)
//!
//! | check | tolerance | measured |
//! |---|---|---|
//! | zero-velocity `k_eff` against the `chi_eff` path | 1e-9 relative | see the test output |
//! | delayed-source conservation under a uniform flow | 1e-8 relative | see the test output |
//!
//! Neither is validation. The validation-grade statement is the code-to-code
//! comparison against upstream GeN-Foam on the `2D_MSFR` tutorial, in
//! `tests/genfoam_tutorial_keff.rs`.

use std::f64::consts::PI;
use std::sync::Arc;

use outram_foam_basic_lib::interface::one_dimensional_meshing::create_one_d_mesh;
use outram_foam_basic_lib::prelude::{BoundaryCondition, FvMesh};
use uom::si::area::square_meter;
use uom::si::f64::{Area, Length};
use uom::si::length::meter;

use super::PrecursorTransport;
use crate::genfoam::neutronics::diffusion::{DiffusionNeutronics, DiffusionSettings};
use crate::genfoam::neutronics::xs::input::{
    NuclearDataInput, StateInput, ZoneConstantsInput, ZoneStateInput,
};
use crate::genfoam::neutronics::xs::CrossSectionData;

/// One group, one precursor group, one zone, no feedback variables.
///
/// `chi_prompt` and `chi_delayed` differ so that the prompt/delayed split is
/// actually exercised — with a single group both are forced to 1 and the
/// reduction test would pass trivially, so two groups are used with the delayed
/// spectrum shifted towards the softer one, as a real evaluation has it.
fn two_group_xs() -> CrossSectionData {
    let constants = ZoneConstantsInput {
        iv: vec![1.0e-7, 1.0e-6],
        disc_factor: vec![1.0, 1.0],
        integral_flux: vec![1.0, 1.0],
        beta: vec![0.0065],
        lambda: vec![0.08],
        ..ZoneConstantsInput::default()
    };
    let zone = ZoneStateInput {
        name: "core".to_string(),
        d: vec![0.02, 0.01],
        nu_sigma_eff: vec![0.10, 0.40],
        sigma_pow: vec![1.0, 1.0],
        // removal = absorption + out-scatter
        sigma_removal: vec![0.14, 0.12],
        chi_prompt: vec![0.9, 0.1],
        chi_delayed: vec![0.3, 0.7],
        // [from][to], moment 0 only; group 0 -> group 1 down-scatter of 0.04.
        scattering: vec![vec![vec![0.0, 0.04], vec![0.0, 0.0]]],
        constants: Some(constants),
    };
    let input = NuclearDataInput {
        energy_groups: 2,
        prec_groups: 1,
        legendre_moments: 1,
        poly_spline_mode: 1,
        fast_neutrons: false,
        xs_variables: Vec::new(),
        do_not_parametrize: Vec::new(),
        states: vec![StateInput {
            name: "reference".to_string(),
            parameters: Default::default(),
            zones: vec![zone],
        }],
    };
    CrossSectionData::from_input(&input).expect("valid two-group nuclear data")
}

fn slab_mesh(n: i64) -> Arc<FvMesh> {
    Arc::new(
        create_one_d_mesh(Length::new::<meter>(1.0), Area::new::<square_meter>(1.0), n)
            .expect("valid slab mesh"),
    )
}

fn model(mesh: Arc<FvMesh>, xs: &CrossSectionData) -> DiffusionNeutronics {
    let zone_of_cell = vec![0usize; mesh.n_cells];
    let bc = vec![
        BoundaryCondition::FixedValue(0.0),
        BoundaryCondition::FixedValue(0.0),
    ];
    DiffusionNeutronics::new(
        mesh,
        xs,
        &zone_of_cell,
        &[],
        &bc,
        DiffusionSettings {
            k_tolerance: 1e-12,
            flux_tolerance: 1e-10,
            ..DiffusionSettings::default()
        },
    )
    .expect("build diffusion model")
}

/// `PrecursorTransport` with the given uniform face flux and diffusivity.
///
/// `alpha` is uniform 1, so `C* == C` and the algebra is easy to check by hand.
fn transport(mesh: &Arc<FvMesh>, phi_value: f64, diffusivity: f64) -> PrecursorTransport {
    PrecursorTransport::new(
        mesh,
        vec![1.0; mesh.n_cells],
        vec![phi_value; mesh.n_internal_faces],
        // No flux through either end wall: the slab is closed, so the precursors
        // can only be redistributed inside it.
        mesh.patches.iter().map(|p| vec![0.0; p.size]).collect(),
        vec![diffusivity; mesh.n_cells],
    )
}

/// **Reduction.** With no flow and no diffusion the precursor equation is
/// algebraic, `C = S_n Beta / (k lambda)`, so the delayed source is
/// `S_n beta_tot / k` and the flux source collapses to
/// `chi_eff S_n / k`. The drifted and stationary solvers must then give the
/// same eigenvalue to solver tolerance.
#[test]
fn a_zero_velocity_field_reproduces_the_chi_eff_collapse() {
    let xs = two_group_xs();
    let mesh = slab_mesh(40);

    let mut stationary = model(mesh.clone(), &xs);
    let a = stationary.solve_eigenvalue().expect("stationary converges");

    let mut drifting = model(mesh.clone(), &xs);
    drifting.set_precursor_drift(transport(&mesh, 0.0, 0.0));
    let b = drifting.solve_eigenvalue().expect("drifting converges");

    let rel = ((b.k_eff - a.k_eff) / a.k_eff).abs();
    eprintln!(
        "chi_eff collapse: stationary k = {:.12}, drift-with-zero-flow k = {:.12}, \
         relative difference {rel:.3e}",
        a.k_eff, b.k_eff
    );
    assert!(a.converged && b.converged);
    assert!(
        rel < 1e-9,
        "the drift path with zero flow gives k = {} where the chi_eff collapse \
         gives {} (relative {rel:.3e}) — the two must agree exactly in this \
         limit, or the drift model is not an extension of the stationary one",
        b.k_eff,
        a.k_eff
    );
}

/// **Conservation.** Advection and diffusion move precursors, they do not
/// destroy them: with zero-gradient precursor boundaries and no flux through the
/// walls, `integral(sum_k lambda_k C_k dV)` must equal
/// `beta_tot integral(S_n dV) / k_eff` however fast the fuel circulates.
#[test]
fn drift_conserves_the_total_delayed_source() {
    let xs = two_group_xs();
    let mesh = slab_mesh(40);

    for phi in [0.0, 0.05, 0.5] {
        let mut m = model(mesh.clone(), &xs);
        m.set_precursor_drift(transport(&mesh, phi, 1.0e-3));
        let r = m.solve_eigenvalue().expect("converges");

        let v = &mesh.cell_volumes;
        let s_fis = m.fission_production();
        let fission: f64 = s_fis
            .internal
            .as_slice()
            .iter()
            .zip(v.iter())
            .map(|(s, vol)| s * vol)
            .sum();
        let lambda = m.xs_fields().lambda[0].internal.as_slice().to_vec();
        let delayed: f64 = m.state().precursors()[0]
            .internal
            .as_slice()
            .iter()
            .zip(lambda.iter())
            .zip(v.iter())
            .map(|((c, l), vol)| l * c * vol)
            .sum();
        let expected = 0.0065 * fission / r.k_eff;
        let rel = ((delayed - expected) / expected).abs();
        eprintln!(
            "phi = {phi}: delayed source {delayed:.9e}, expected \
             beta*F/k = {expected:.9e}, relative {rel:.3e}, k = {:.9}",
            r.k_eff
        );
        assert!(
            rel < 1e-8,
            "phi = {phi}: the transported delayed source is {delayed:e} where \
             conservation requires {expected:e} (relative {rel:.3e}) — drift \
             must redistribute precursors, not create or destroy them"
        );
    }
}

/// A non-zero flow must actually change the answer — otherwise the two tests
/// above would both pass on a model that silently ignored `phi`.
///
/// On a bare slab the flux peaks at the centre and the precursors are swept
/// downstream, towards the lower-importance end, so drift **lowers** `k_eff`.
/// That is the same sign as the -177.8 pcm measured on the MSFR tutorial.
#[test]
fn drift_lowers_k_eff_and_the_effect_grows_with_the_flow() {
    let xs = two_group_xs();
    let mesh = slab_mesh(40);
    let _ = PI; // (the analytic buckling is not needed here)

    let mut ks = Vec::new();
    for phi in [0.0, 0.2, 1.0] {
        let mut m = model(mesh.clone(), &xs);
        m.set_precursor_drift(transport(&mesh, phi, 0.0));
        ks.push((phi, m.solve_eigenvalue().expect("converges").k_eff));
    }
    for (phi, k) in &ks {
        eprintln!("phi = {phi}: k_eff = {k:.9}");
    }
    assert!(
        ks[1].1 < ks[0].1 && ks[2].1 < ks[1].1,
        "k_eff should fall monotonically as the fuel circulates faster, got {ks:?}"
    );
}
