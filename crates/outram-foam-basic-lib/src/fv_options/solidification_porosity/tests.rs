// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.

//! Verification tests for the porosity-model form of solidification.
//!
//! # Scope of these tests
//!
//! **Verification**, not validation — "is upstream's expression implemented
//! correctly?". Every check compares against a closed form of the model's own
//! algebra or against an invariant the discretisation must satisfy. None
//! compares against a solidification experiment; that judgement is the
//! maintainer's.
//!
//! # Common test material
//!
//! Upstream's own documented example table is used throughout: `D = 10000` 1/s
//! at and below 330.0 K, `D = 0` at and above 330.5 K, from the `Usage` block
//! of `solidification.H`. The mesh is the same 4-cell, 0.5 m³-per-cell periodic
//! 1-D mesh the enthalpy-porosity tests use, so a missing or doubled volume
//! factor shows up rather than cancelling.

use super::*;
use crate::mesh::FvMesh;
use std::sync::Arc;

const T_SOLID: f64 = 330.0;
const T_LIQUID: f64 = 330.5;
const D_SOLID: f64 = 10_000.0;

const N_CELLS: usize = 4;
const CELL_VOLUME: f64 = 0.5;

fn mesh() -> Arc<FvMesh> {
    Arc::new(FvMesh::periodic_1d(N_CELLS, 4.0, 0.5))
}

fn table() -> TemperatureTable {
    TemperatureTable::two_knot(T_SOLID, T_LIQUID, D_SOLID, 0.0)
}

fn model(form: MomentumEquationForm) -> SolidificationPorosity {
    SolidificationPorosity::new("iceZone", "U", CellSelection::All, table(), form)
}

fn uniform(name: &str, m: &Arc<FvMesh>, value: f64) -> VolScalarField {
    VolScalarField::uniform(name, m.clone(), value)
}

// ── The D(T) table ───────────────────────────────────────────────────────────

/// **Methodology.** `Foam::Function1`'s `table` entry clamps outside its range
/// rather than extrapolating. Upstream's example table `(330 10000) (330.5 0)`
/// must therefore give the full solid drag at *any* temperature at or below
/// 330 K — including far below it — and exactly zero at or above 330.5 K.
/// Extrapolation instead of clamping would send the drag negative above the
/// liquidus, which would *accelerate* the melt rather than releasing it. Pass
/// criterion: exact equality at and beyond both ends.
///
/// **Results.** Measured `D(200 K) = 10000`, `D(330 K) = 10000`,
/// `D(330.5 K) = 0`, `D(500 K) = 0` (printed values, all exact). The table is
/// therefore a clamped ramp, and no temperature anywhere on the real line
/// produces a negative drag. Taken 2026-08-05.
#[test]
fn drag_table_clamps_at_both_ends_instead_of_extrapolating() {
    let t = table();
    let far_cold = t.value(200.0);
    let at_solidus = t.value(T_SOLID);
    let at_liquidus = t.value(T_LIQUID);
    let far_hot = t.value(500.0);
    println!(
        "D(200) = {far_cold}, D({T_SOLID}) = {at_solidus}, \
         D({T_LIQUID}) = {at_liquidus}, D(500) = {far_hot}"
    );
    assert_eq!(far_cold, D_SOLID);
    assert_eq!(at_solidus, D_SOLID);
    assert_eq!(at_liquidus, 0.0);
    assert_eq!(far_hot, 0.0);
}

/// **Methodology.** Between the knots the table must be linear, so the midpoint
/// 330.25 K must give exactly half the solid drag. Pass criterion: agreement
/// with `D_solid/2` to 1e-12 relative.
///
/// **Results.** Measured `D(330.25) = 5000`, closed form `5000`, absolute
/// difference `0`. The mushy interval is a linear ramp, matching
/// `interpolateXY`. Taken 2026-08-05.
#[test]
fn drag_table_is_linear_between_the_knots() {
    let t = table();
    let mid = 0.5 * (T_SOLID + T_LIQUID);
    let d = t.value(mid);
    let closed_form = 0.5 * D_SOLID;
    println!(
        "D({mid}) = {d}, closed form = {closed_form}, diff = {}",
        d - closed_form
    );
    assert!((d - closed_form).abs() < 1e-12 * D_SOLID);
}

/// **Methodology.** A descending temperature column is a case-setup error that
/// would make `interpolate_xy`'s binary search return an arbitrary bracket, so
/// the constructor must reject it rather than silently mis-interpolating. Pass
/// criterion: a panic.
///
/// **Results.** Constructing `TemperatureTable::new(vec![330.5, 330.0], …)`
/// panicked as required. Taken 2026-08-05.
#[test]
#[should_panic(expected = "strictly ascending")]
fn drag_table_rejects_descending_temperatures() {
    let _ = TemperatureTable::new(vec![330.5, 330.0], vec![0.0, D_SOLID]);
}

// ── Momentum drag placement ──────────────────────────────────────────────────

/// **Methodology.** Upstream writes `Udiag[celli] += V*alpha*rho*D(T)` straight
/// into the solved `UEqn`, so in a fully solid, kinematic case with `alpha`
/// absent the diagonal must gain exactly `V·D_solid = 0.5 × 10000 = 5000` in
/// every cell, on top of whatever the equation already held. The equation is
/// started from a zero matrix so the contribution is read directly. Pass
/// criterion: exact equality with `V·D_solid` in all 4 cells.
///
/// **Results.** Measured `diag = [5000, 5000, 5000, 5000]` against the closed
/// form `5000`. The sign is **positive** — the drag increases diagonal
/// dominance, which is what damps the velocity. Note this is the opposite
/// written sign to the enthalpy-porosity fvModel's `S`, and the two agree once
/// upstream's `solve(UEqn == fvModels.source(...))` negation is applied to the
/// latter; see the `add_momentum_source` doc comment. Taken 2026-08-05.
#[test]
fn kinematic_drag_adds_volume_times_d_to_the_diagonal() {
    let m = mesh();
    let model = model(MomentumEquationForm::Kinematic);
    let t = uniform("T", &m, 300.0); // well below the solidus
    let rho = uniform("rho", &m, 6093.0); // must be ignored in this form
    let mut eqn = FvVectorMatrix::new(m.clone());

    model.add_momentum_source(&t, &rho, None, &mut eqn);

    let closed_form = CELL_VOLUME * D_SOLID;
    println!("diag = {:?}, closed form = {closed_form}", eqn.ldu.diag);
    for c in 0..N_CELLS {
        assert_eq!(eqn.ldu.diag[c], closed_form);
    }
}

/// **Methodology.** The density-weighted branch must multiply by `rho`, and the
/// kinematic branch must not. Running the same model twice — once in each form,
/// with the same `rho = 6093` kg/m³ — the ratio of the two diagonal
/// contributions must be exactly `rho`. This is the check that catches the
/// branch being wired the wrong way round, which no dimensional check in Rust
/// would catch because both are `f64`. Pass criterion: ratio equals `rho` to
/// 1e-12 relative.
///
/// **Results.** Measured kinematic `diag[0] = 5000`, density-weighted
/// `diag[0] = 30465000`, ratio `6093`, against `rho = 6093`, relative error
/// `0`. The branch is wired correctly. Taken 2026-08-05.
#[test]
fn density_weighted_form_scales_the_drag_by_rho() {
    let m = mesh();
    let rho_value = 6093.0;
    let t = uniform("T", &m, 300.0);
    let rho = uniform("rho", &m, rho_value);

    let mut kin = FvVectorMatrix::new(m.clone());
    model(MomentumEquationForm::Kinematic).add_momentum_source(&t, &rho, None, &mut kin);

    let mut dw = FvVectorMatrix::new(m.clone());
    model(MomentumEquationForm::DensityWeighted).add_momentum_source(&t, &rho, None, &mut dw);

    let ratio = dw.ldu.diag[0] / kin.ldu.diag[0];
    println!(
        "kinematic diag[0] = {}, density-weighted diag[0] = {}, ratio = {ratio}, rho = {rho_value}",
        kin.ldu.diag[0], dw.ldu.diag[0]
    );
    assert!((ratio - rho_value).abs() < 1e-12 * rho_value);
}

/// **Methodology.** Above the liquidus the table gives `D = 0`, so a fully
/// molten cell must gain *bit-exactly* nothing — not a small residue. A nonzero
/// value would mean the blockage never fully releases the melt, which would
/// show up as an unphysical drag throughout a melting calculation. Pass
/// criterion: `diag == 0` exactly in all cells.
///
/// **Results.** Measured `diag = [0, 0, 0, 0]` at `T = 400 K`. Taken
/// 2026-08-05.
#[test]
fn fully_molten_cells_gain_exactly_no_drag() {
    let m = mesh();
    let model = model(MomentumEquationForm::Kinematic);
    let t = uniform("T", &m, 400.0);
    let rho = uniform("rho", &m, 1.0);
    let mut eqn = FvVectorMatrix::new(m.clone());

    model.add_momentum_source(&t, &rho, None, &mut eqn);

    println!("diag = {:?}", eqn.ldu.diag);
    for c in 0..N_CELLS {
        assert_eq!(eqn.ldu.diag[c], 0.0);
    }
}

/// **Methodology.** The optional phase fraction scales the drag linearly, so
/// `alpha = 0.25` must give exactly a quarter of the `alpha = 1` (i.e. `None`)
/// contribution. Pass criterion: ratio equals 0.25 to 1e-12.
///
/// **Results.** Measured `diag[0]` with no phase fraction `= 5000`, with
/// `alpha = 0.25` `= 1250`, ratio `0.25`, absolute error from 0.25 is `0`.
/// Taken 2026-08-05.
#[test]
fn phase_fraction_scales_the_drag_linearly() {
    let m = mesh();
    let model = model(MomentumEquationForm::Kinematic).with_phase_fraction("alpha.solid");
    let t = uniform("T", &m, 300.0);
    let rho = uniform("rho", &m, 1.0);
    let alpha = uniform("alpha.solid", &m, 0.25);

    let mut without = FvVectorMatrix::new(m.clone());
    model.add_momentum_source(&t, &rho, None, &mut without);

    let mut with = FvVectorMatrix::new(m.clone());
    model.add_momentum_source(&t, &rho, Some(&alpha), &mut with);

    let ratio = with.ldu.diag[0] / without.ldu.diag[0];
    println!(
        "no alpha: diag[0] = {}, alpha=0.25: diag[0] = {}, ratio = {ratio}",
        without.ldu.diag[0], with.ldu.diag[0]
    );
    assert_eq!(model.phase_fraction_name(), Some("alpha.solid"));
    assert!((ratio - 0.25).abs() < 1e-12);
}

/// **Methodology.** A cell zone must restrict the drag to its own cells and
/// leave the rest of the mesh untouched. Selecting cells 1 and 2 of 4, the
/// diagonal must gain `V·D_solid` there and exactly zero in cells 0 and 3.
/// Pass criterion: exact, per cell.
///
/// **Results.** Measured `diag = [0, 5000, 5000, 0]`. The zone restriction
/// holds and does not leak into neighbouring cells. Taken 2026-08-05.
#[test]
fn a_cell_zone_restricts_the_drag_to_its_own_cells() {
    let m = mesh();
    let model = SolidificationPorosity::new(
        "iceZone",
        "U",
        CellSelection::zone(vec![1, 2]),
        table(),
        MomentumEquationForm::Kinematic,
    );
    let t = uniform("T", &m, 300.0);
    let rho = uniform("rho", &m, 1.0);
    let mut eqn = FvVectorMatrix::new(m.clone());

    model.add_momentum_source(&t, &rho, None, &mut eqn);

    println!("diag = {:?}", eqn.ldu.diag);
    assert_eq!(eqn.ldu.diag[0], 0.0);
    assert_eq!(eqn.ldu.diag[1], CELL_VOLUME * D_SOLID);
    assert_eq!(eqn.ldu.diag[2], CELL_VOLUME * D_SOLID);
    assert_eq!(eqn.ldu.diag[3], 0.0);
}

/// **Methodology.** Upstream's `calcForce` builds the *same* `Udiag` the
/// momentum correction builds — `V·α·ρ·D(T)` — and then multiplies by `U`. The
/// diagnostic must therefore agree cell-for-cell with the diagonal contribution
/// `add_momentum_source` makes, or the reported force would not be the force
/// the solver actually applies. Pass criterion: exact equality against the
/// assembled diagonal.
///
/// **Results.** Measured `force_diagonal = [5000.0, 5000.0, 5000.0, 5000.0]`
/// and assembled `diag = [5000.0, 5000.0, 5000.0, 5000.0]` — identical. Taken
/// 2026-08-05.
#[test]
fn force_diagonal_matches_the_assembled_momentum_diagonal() {
    let m = mesh();
    let model = model(MomentumEquationForm::Kinematic);
    let t = uniform("T", &m, 300.0);
    let rho = uniform("rho", &m, 1.0);

    let mut eqn = FvVectorMatrix::new(m.clone());
    model.add_momentum_source(&t, &rho, None, &mut eqn);

    let force = model.force_diagonal(&t, &rho, None, &m.cell_volumes);
    println!(
        "force_diagonal = {force:?}, assembled diag = {:?}",
        eqn.ldu.diag
    );
    for c in 0..N_CELLS {
        assert_eq!(force[c], eqn.ldu.diag[c]);
    }
}
