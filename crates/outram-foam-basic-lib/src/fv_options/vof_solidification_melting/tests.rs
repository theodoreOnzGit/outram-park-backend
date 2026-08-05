// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.

//! Verification tests for the VoF solidification/melting model.
//!
//! # Scope of these tests
//!
//! **Verification**, not validation. Each check compares against a closed form
//! of upstream's own algebra or against an invariant the model must satisfy —
//! notably the cap `αₛ ≤ α_vof`, which is the only thing keeping the model from
//! claiming more solid than there is condensed phase in a cell.
//!
//! # Common test material
//!
//! Upstream's documented example table is used: solid fraction 1 at and below
//! 330 K, 0 at and above 335 K (`alphaSolidT table ((330 1) (335 0))` from the
//! `Usage` block of `VoFSolidificationMelting.H`), with `L = 334000` J/kg —
//! water's latent heat, upstream's own example value. The mesh is the same
//! 4-cell, 0.5 m³-per-cell periodic 1-D mesh the other `fv_options` tests use.

use super::*;
use crate::mesh::FvMesh;
use std::sync::Arc;

const T_SOLID: f64 = 330.0;
const T_LIQUID: f64 = 335.0;
const LATENT_HEAT: f64 = 334_000.0;

const N_CELLS: usize = 4;
const CELL_VOLUME: f64 = 0.5;

fn mesh() -> Arc<FvMesh> {
    Arc::new(FvMesh::periodic_1d(N_CELLS, 4.0, 0.5))
}

fn table() -> TemperatureTable {
    TemperatureTable::two_knot(T_SOLID, T_LIQUID, 1.0, 0.0)
}

fn model() -> VofSolidificationMelting {
    VofSolidificationMelting::new(
        "solidZone",
        "U",
        "h",
        CellSelection::All,
        table(),
        LATENT_HEAT,
        N_CELLS,
    )
}

fn uniform(name: &str, m: &Arc<FvMesh>, value: f64) -> VolScalarField {
    VolScalarField::uniform(name, m.clone(), value)
}

// ── The solid-fraction update ────────────────────────────────────────────────

/// **Methodology.** Upstream's `correct()` is
/// `αₛ ← min(relax·α_vof·αₛ(T) + (1 − relax)·αₛ_old, α_vof)`. Starting from
/// `αₛ_old = 0`, a full VoF cell (`α_vof = 1`) held well below the solidus has
/// `αₛ(T) = 1`, so one call must give exactly `relax = 0.9` — **not** 1. This
/// is the check that pins the relaxation as a *convex blend*: the
/// enthalpy-porosity model's `relax` scales an increment instead, and reading
/// one model's algebra into the other is the specific mistake this test
/// forecloses. Pass criterion: exact equality with `relax`.
///
/// **Results.** Measured `solid_fraction[0] = 0.9` after one call from
/// `αₛ = 0`, against `relax = 0.9`. Taken 2026-08-05.
#[test]
fn one_correct_from_zero_gives_exactly_the_relaxation_coefficient() {
    let m = mesh();
    let mut model = model();
    let t = uniform("T", &m, 300.0);
    let alpha_vof = uniform("alpha.liquid", &m, 1.0);

    model.correct(&t, &alpha_vof);

    println!("solid_fraction after one correct = {:?}", model.solid_fraction());
    assert_eq!(model.solid_fraction()[0], 0.9);
}

/// **Methodology.** Repeated calls at a fixed temperature must converge
/// geometrically to the table value, since the blend is a contraction with
/// factor `1 − relax = 0.1`. After `n` calls from zero the fraction is
/// `1 − 0.1ⁿ`. Ten calls therefore give `1 − 1e-10`. Pass criterion: agreement
/// with that closed form to 1e-15 absolute.
///
/// **Results.** Measured `solid_fraction[0] = 0.9999999999` after 10 calls,
/// closed form `0.9999999999`, absolute difference `0.0000000000`. The blend
/// contracts at exactly the rate `(1 − relax)` predicts. Taken 2026-08-05.
#[test]
fn repeated_correct_converges_geometrically_to_the_table_value() {
    let m = mesh();
    let mut model = model();
    let t = uniform("T", &m, 300.0);
    let alpha_vof = uniform("alpha.liquid", &m, 1.0);

    for _ in 0..10 {
        model.correct(&t, &alpha_vof);
    }

    let measured = model.solid_fraction()[0];
    let closed_form = 1.0 - 0.1_f64.powi(10);
    println!(
        "after 10 corrects: {measured:.10}, closed form {closed_form:.10}, diff {:.10}",
        measured - closed_form
    );
    assert!((measured - closed_form).abs() < 1e-15);
}

/// **Methodology.** The `min(…, α_vof)` cap is what stops the model reporting
/// more solid than there is condensed phase. Driving the fraction to
/// near-saturation in a full cell and then dropping `α_vof` to 0.3 must clamp
/// the solid fraction to 0.3 immediately — the blend term alone would give
/// `0.9·0.3·1 + 0.1·0.9999999999 = 0.3699999999…`, which is larger. Pass
/// criterion: exactly `α_vof`, and strictly less than the uncapped blend.
///
/// **Results.** Uncapped blend would be `0.3699999999900000`; measured
/// `solid_fraction[0] = 0.3` = `α_vof` exactly. The cap is active and binding.
/// Taken 2026-08-05.
#[test]
fn solid_fraction_is_capped_by_the_vof_phase_fraction() {
    let m = mesh();
    let mut model = model();
    let t = uniform("T", &m, 300.0);
    let full = uniform("alpha.liquid", &m, 1.0);
    for _ in 0..10 {
        model.correct(&t, &full);
    }
    let before = model.solid_fraction()[0];

    let partial = uniform("alpha.liquid", &m, 0.3);
    model.correct(&t, &partial);
    let after = model.solid_fraction()[0];

    let uncapped = 0.9 * 0.3 * 1.0 + 0.1 * before;
    println!("before = {before:.16}, uncapped blend = {uncapped:.16}, measured = {after:.16}");
    assert!(uncapped > 0.3, "the cap must actually be binding in this test");
    assert_eq!(after, 0.3);
}

/// **Methodology.** Above the liquidus the table gives `αₛ(T) = 0`, so the
/// blend decays towards zero rather than sticking at whatever the cell last
/// held. From a saturated `αₛ ≈ 1`, one call must give exactly `0.1·αₛ_old`.
/// Pass criterion: agreement to 1e-15.
///
/// **Results.** From `αₛ_old = 0.9999999999`, measured
/// `solid_fraction[0] = 0.09999999999`, closed form `0.09999999999`, difference
/// `0.00000000000`. Melting releases the blockage. Taken 2026-08-05.
#[test]
fn heating_above_the_liquidus_decays_the_solid_fraction() {
    let m = mesh();
    let mut model = model();
    let cold = uniform("T", &m, 300.0);
    let alpha_vof = uniform("alpha.liquid", &m, 1.0);
    for _ in 0..10 {
        model.correct(&cold, &alpha_vof);
    }
    let before = model.solid_fraction()[0];

    let hot = uniform("T", &m, 400.0);
    model.correct(&hot, &alpha_vof);
    let after = model.solid_fraction()[0];

    let closed_form = 0.1 * before;
    println!("before = {before:.11}, after = {after:.11}, closed form = {closed_form:.11}");
    assert!((after - closed_form).abs() < 1e-15);
}

// ── Momentum drag ────────────────────────────────────────────────────────────

/// **Methodology.** With no solid at all, `α_fluid = 1` and the Carman-Kozeny
/// numerator `(1 − α_fluid)²` is identically zero, so a fully fluid cell must
/// feel bit-exactly no drag. Pass criterion: `diag == 0` exactly.
///
/// **Results.** Measured `diag = [0.0, 0.0, 0.0, 0.0]` with `αₛ = 0`
/// throughout. Taken 2026-08-05.
#[test]
fn fully_fluid_cells_feel_exactly_no_drag() {
    let m = mesh();
    let model = model();
    let rho = uniform("rho", &m, 1000.0);
    let mut eqn = FvVectorMatrix::new(m.clone());

    model.add_momentum_source(&rho, &mut eqn);

    println!("diag = {:?}", eqn.ldu.diag);
    for c in 0..N_CELLS {
        assert_eq!(eqn.ldu.diag[c], 0.0);
    }
}

/// **Methodology.** The drag must be **positive** on the diagonal — a
/// stabilising sink — even though upstream writes it as `Sp -= Vc*rho*S` with a
/// *positive* `S`, the opposite written sign to the enthalpy-porosity model's
/// `Sp += Vc*S` with a negative `S`. The two agree once upstream's
/// `solve(UEqn == fvModels.source(...))` negation is applied. The closed form
/// here, for a cell driven to `αₛ = 0.9` (so `α_fluid = 0.1`) with
/// `V = 0.5` m³ and `ρ = 1000` kg/m³, is
/// `V·ρ·Cu(1−α_f)²/(α_f³+q) = 0.5·1000·1e5·0.81/(0.001+0.001)`. Pass
/// criterion: agreement with that closed form to 1e-9 relative, and a positive
/// sign.
///
/// **Results.** Measured `diag[0] = 20250000000`, closed form
/// `20250000000`, relative error `0`. Positive, as required. Note the
/// magnitude: `2.0e10` against a `V/dt` term of order `0.5/0.01 = 50`, i.e.
/// nine orders of magnitude larger — which is how the method pins a solid
/// cell's velocity to numerical zero rather than merely damping it. Taken
/// 2026-08-05.
#[test]
fn a_mostly_solid_cell_gains_a_large_positive_diagonal_drag() {
    let m = mesh();
    let mut model = model();
    let t = uniform("T", &m, 300.0);
    let alpha_vof = uniform("alpha.liquid", &m, 1.0);
    model.correct(&t, &alpha_vof); // αₛ = 0.9 exactly, α_fluid = 0.1

    let rho_value = 1000.0;
    let rho = uniform("rho", &m, rho_value);
    let mut eqn = FvVectorMatrix::new(m.clone());
    model.add_momentum_source(&rho, &mut eqn);

    let alpha_fluid = 0.1_f64;
    let closed_form = CELL_VOLUME
        * rho_value
        * 1.0e5
        * (1.0 - alpha_fluid).powi(2)
        / (alpha_fluid.powi(3) + 1.0e-3);
    println!(
        "alpha_solid = {}, diag[0] = {}, closed form = {closed_form}",
        model.solid_fraction()[0],
        eqn.ldu.diag[0]
    );
    assert!(eqn.ldu.diag[0] > 0.0, "the drag must be a stabilising sink");
    assert!((eqn.ldu.diag[0] - closed_form).abs() < 1e-9 * closed_form);
}

// ── Latent heat ──────────────────────────────────────────────────────────────

/// **Methodology.** Upstream writes `eqn += L*fvc::ddt(rho, alphaSolid_)` into
/// the intermediate `fvModels` matrix, which `solve(hEqn == fvModels.source())`
/// then subtracts, so the solved right-hand side receives
/// `−V·L·ρ·Δαₛ/Δt`. **Freezing** (`Δαₛ > 0`) must therefore give a *negative*
/// source contribution. With `Δαₛ = 0.9` in one step of `Δt = 0.5` s,
/// `ρ = 1000` kg/m³, `V = 0.5` m³ and `L = 334000` J/kg the closed form is
/// `−0.5·334000·1000·0.9/0.5`. Pass criterion: agreement to 1e-9 relative.
///
/// **Results.** Measured `source[0] = -300600000`, closed form `-300600000`,
/// relative error `0`. Sign negative, i.e. freezing *releases* heat into an
/// equation whose sources are subtracted — the opposite sign to melting, which
/// absorbs it. Taken 2026-08-05.
#[test]
fn freezing_puts_a_negative_latent_heat_source_in_the_enthalpy_equation() {
    let m = mesh();
    let mut model = model();
    let t = uniform("T", &m, 300.0);
    let alpha_vof = uniform("alpha.liquid", &m, 1.0);

    model.advance_time(); // αₛ_old = 0
    model.correct(&t, &alpha_vof); // αₛ = 0.9

    let dt = 0.5;
    let rho_value = 1000.0;
    let rho = uniform("rho", &m, rho_value);
    let mut eqn = FvMatrix::new(m.clone());
    model.add_enthalpy_source(&rho, dt, &mut eqn);

    let closed_form = -CELL_VOLUME * LATENT_HEAT * rho_value * 0.9 / dt;
    println!("source[0] = {}, closed form = {closed_form}", eqn.source[0]);
    assert!(eqn.source[0] < 0.0, "freezing must release heat");
    assert!((eqn.source[0] - closed_form).abs() < 1e-9 * closed_form.abs());
}

/// **Methodology.** Without a call to `advance_time` the old solid fraction
/// never moves, so the Euler rate `(αₛ − αₛ_old)/Δt` keeps reporting the *whole*
/// change since the model was built rather than the change over one step. Two
/// `correct` calls without an intervening `advance_time` must therefore give a
/// larger source than the same two with one. This is the check that makes the
/// bookkeeping requirement visible rather than silent. Pass criterion: the
/// un-advanced source is strictly larger in magnitude.
///
/// **Results.** With `advance_time` between steps: `source[0] = -30060000.0`.
/// Without: `source[0] = -330660000.0` — 11.0× larger, because the rate is
/// measured against `αₛ_old = 0` instead of the previous step's `0.9`. Taken
/// 2026-08-05.
#[test]
fn omitting_advance_time_inflates_the_latent_heat_rate() {
    let m = mesh();
    let t = uniform("T", &m, 300.0);
    let alpha_vof = uniform("alpha.liquid", &m, 1.0);
    let dt = 0.5;
    let rho = uniform("rho", &m, 1000.0);

    let mut correct_model = model();
    correct_model.advance_time();
    correct_model.correct(&t, &alpha_vof);
    correct_model.advance_time();
    correct_model.correct(&t, &alpha_vof);
    let mut correct_eqn = FvMatrix::new(m.clone());
    correct_model.add_enthalpy_source(&rho, dt, &mut correct_eqn);

    let mut stale_model = model();
    stale_model.correct(&t, &alpha_vof);
    stale_model.correct(&t, &alpha_vof);
    let mut stale_eqn = FvMatrix::new(m.clone());
    stale_model.add_enthalpy_source(&rho, dt, &mut stale_eqn);

    println!(
        "with advance_time: source[0] = {}, without: source[0] = {}, ratio = {}",
        correct_eqn.source[0],
        stale_eqn.source[0],
        stale_eqn.source[0] / correct_eqn.source[0]
    );
    assert!(stale_eqn.source[0].abs() > correct_eqn.source[0].abs());
}

/// **Methodology.** A non-positive timestep would divide by zero in the Euler
/// rate. Upstream never reaches that state, but a Rust caller can, so the guard
/// must return without touching the equation rather than writing an infinity.
/// Pass criterion: the source stays exactly zero.
///
/// **Results.** Measured `source = [0.0, 0.0, 0.0, 0.0]` for `dt = 0`. Taken
/// 2026-08-05.
#[test]
fn a_zero_timestep_contributes_nothing_instead_of_dividing_by_zero() {
    let m = mesh();
    let mut model = model();
    let t = uniform("T", &m, 300.0);
    let alpha_vof = uniform("alpha.liquid", &m, 1.0);
    model.correct(&t, &alpha_vof);

    let rho = uniform("rho", &m, 1000.0);
    let mut eqn = FvMatrix::new(m.clone());
    model.add_enthalpy_source(&rho, 0.0, &mut eqn);

    println!("source = {:?}", eqn.source);
    for c in 0..N_CELLS {
        assert_eq!(eqn.source[c], 0.0);
    }
}
