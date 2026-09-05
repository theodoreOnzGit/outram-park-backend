// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.

//! Verification tests for the enthalpy-porosity solidification/melting model.
//!
//! # Scope of these tests
//!
//! These are **verification** checks — "is the model implemented correctly?" —
//! not validation. They compare against closed-form limits of the model's own
//! equations, against upstream's algebra, and against invariants the method
//! must satisfy. None of them compares against a melting experiment; judging
//! the model against physical reality is the maintainer's validation work and
//! is deliberately not claimed here.
//!
//! # Common test material
//!
//! Every test uses a gallium-like parameter set, chosen because gallium is the
//! material in upstream's own melting tutorial and its low melting point makes
//! the numbers easy to sanity-check:
//!
//! - solidus `T_sol = 302.0` K, liquidus `T_liq = 304.0` K
//! - latent heat `L = 80160` J/kg
//! - specific heat `Cp = 381.5` J/(kg·K)
//! - reference density `rho_ref = 6093` kg/m³
//! - expansion coefficient `beta = 1.2e-4` 1/K
//! - upstream numerical defaults: `Cu = 1e5`, `q = 1e-3`, `relax = 0.9`,
//!   `alpha1e = 0`
//!
//! The mesh is a 4-cell periodic 1-D mesh of length 4 m and face area 0.5 m²,
//! so every cell volume is exactly **0.5 m³**. A volume of 0.5 rather than 1.0
//! is deliberate: it makes a missing or doubled volume factor visible instead
//! of invisible.

use super::*;
use crate::mesh::FvMesh;
use std::sync::Arc;

const T_SOL: f64 = 302.0;
const T_LIQ: f64 = 304.0;
const LATENT_HEAT: f64 = 80160.0;
const SPECIFIC_HEAT: f64 = 381.5;
const RHO_REF: f64 = 6093.0;
const BETA: f64 = 1.2e-4;

const N_CELLS: usize = 4;
const CELL_VOLUME: f64 = 0.5;

/// Gravity, pointing down the negative y axis \[m/s²\].
fn gravity() -> Vector3 {
    Vector3::new(0.0, -9.81, 0.0)
}

fn mesh() -> Arc<FvMesh> {
    Arc::new(FvMesh::periodic_1d(N_CELLS, 4.0, 0.5))
}

fn coefficients() -> SolidificationMeltingCoefficients {
    SolidificationMeltingCoefficients::new(T_SOL, T_LIQ, LATENT_HEAT, SPECIFIC_HEAT, RHO_REF, BETA)
}

/// A model over the whole mesh, solving the energy equation in temperature.
fn model() -> SolidificationMelting {
    SolidificationMelting::new(
        "melting",
        "U",
        "T",
        true,
        CellSelection::All,
        coefficients(),
        gravity(),
        N_CELLS,
    )
}

fn uniform_temperature(mesh: &Arc<FvMesh>, value: f64) -> VolScalarField {
    VolScalarField::uniform("T", mesh.clone(), value)
}

// ── Darcy drag ───────────────────────────────────────────────────────────────

/// **Methodology.** The Carman-Kozeny drag coefficient is
/// `-Cu (1-α₁)² / (α₁³ + q)`. At `α₁ = 1` the numerator's `(1-α₁)²` factor is
/// identically zero, so a fully liquid cell must feel *exactly* no drag — not
/// merely a small one. Pass criterion: bit-exact zero, since any nonzero value
/// would mean the numerator had been mis-transcribed. Reference: upstream
/// `solidificationMelting.C`, `S = -Cu_*sqr(1.0 - alpha1c)/(pow3(alpha1c) + q_)`.
///
/// **Results.** Measured `darcy_coefficient(1.0) = 0` (printed as `-0`, the
/// negated zero produced by `-Cu * 0.0`, which compares equal to `0.0`). A
/// fully molten cell is therefore hydrodynamically free, as the enthalpy-
/// porosity method requires — the model must not perturb a fully liquid flow at
/// all. Taken 2026-08-05.
#[test]
fn darcy_drag_vanishes_in_a_fully_liquid_cell() {
    let m = model();
    let drag = m.darcy_coefficient(1.0);
    println!("darcy_coefficient(1.0) = {drag}");
    assert_eq!(drag, 0.0, "a fully liquid cell must feel exactly no drag");
}

/// **Methodology.** At `α₁ = 0` the drag reduces to `-Cu/q`, its most negative
/// value: the regularisation constant `q` is what stops it diverging, and it is
/// the only thing bounding the coefficient in a fully solid cell. Inputs are
/// upstream's defaults `Cu = 1e5`, `q = 1e-3`. Pass criterion: agreement with
/// the closed form `-Cu/q` to a relative tolerance of 1e-12.
///
/// **Results.** Measured `darcy_coefficient(0.0) = -100000000` against the
/// closed form `-100000000`, relative error `0`. The saturated coefficient is
/// therefore `1e8` kg/(m³·s) — eight orders of magnitude above the density,
/// which is what pins the velocity in solid cells to numerical zero rather than
/// merely damping it. Taken 2026-08-05.
#[test]
fn darcy_drag_saturates_at_minus_cu_over_q_when_fully_solid() {
    let m = model();
    let c = coefficients();
    let drag = m.darcy_coefficient(0.0);
    let closed_form = -c.darcy_coefficient / c.darcy_regularisation;
    let rel = ((drag - closed_form) / closed_form).abs();
    println!("darcy_coefficient(0.0) = {drag}, closed form = {closed_form}, rel err = {rel:e}");
    assert!(rel < 1e-12, "drag {drag} != closed form {closed_form}");
}

/// **Methodology.** The drag must strengthen monotonically as a cell freezes,
/// or the mushy zone would locally *release* a partially frozen cell and the
/// method would lose its physical basis. Sweep `α₁` over
/// `0.0, 0.1, …, 1.0` and require each successive coefficient to be strictly
/// greater (less negative) than the last. Pass criterion: strict monotonicity
/// across all eleven samples.
///
/// **Results.** Measured, in kg/(m³·s):
///
/// ```text
/// alpha = 0.0  ->  -100000000
/// alpha = 0.1  ->  -40500000
/// alpha = 0.2  ->  -7111111.11111111
/// alpha = 0.3  ->  -1750000
/// alpha = 0.4  ->  -553846.1538461538
/// alpha = 0.5  ->  -198412.6984126984
/// alpha = 0.6  ->  -73732.71889400922
/// alpha = 0.7  ->  -26162.79069767443
/// alpha = 0.8  ->  -7797.270955165686
/// alpha = 0.9  ->  -1369.8630136986294
/// alpha = 1.0  ->  -0
/// ```
///
/// Strictly monotonic, as required. The interpretation worth recording is the
/// **span**: the coefficient falls from `1e8` at `α₁ = 0` to `1.37e3` at
/// `α₁ = 0.9` — nearly five orders of magnitude — before reaching exactly zero
/// at `α₁ = 1`. Note also where the regularisation stops mattering: at
/// `α₁ = 0.1` the cube `α₁³` equals `q` exactly, so the two terms in the
/// denominator are equal there, and above it the physical `α₁³` dominates. A
/// momentum solve crossing the mushy zone therefore sees a coefficient varying
/// by ~1e8 between neighbouring cells, which is why the term must be implicit —
/// an explicit treatment at this stiffness would need an unusably small
/// timestep. Taken 2026-08-05.
#[test]
fn darcy_drag_increases_monotonically_as_the_cell_freezes() {
    let m = model();
    let mut previous = f64::NEG_INFINITY;
    for i in 0..=10 {
        let alpha = f64::from(i) / 10.0;
        let drag = m.darcy_coefficient(alpha);
        println!("alpha = {alpha:.1}  ->  {drag}");
        assert!(
            drag > previous,
            "drag must weaken as alpha rises: {drag} not > {previous} at alpha = {alpha}"
        );
        previous = drag;
    }
}

// ── Effective liquidus ───────────────────────────────────────────────────────

/// **Methodology.** With no eutectic offset (`α₁ₑ = 0`) the effective liquidus
/// is a straight line from the solidus at `α₁ = 0` to the liquidus at
/// `α₁ = 1`, and the `max(T_sol, …)` clamp never binds because the linear term
/// never falls below the solidus. Check the two endpoints and the midpoint
/// against the closed form, tolerance 1e-12 absolute.
///
/// **Results.** Measured `T_eff(0) = 302`, `T_eff(0.5) = 303`,
/// `T_eff(1) = 304` K — the endpoints reproduce the solidus and liquidus
/// exactly and the midpoint sits at the arithmetic mean, confirming the ramp is
/// linear and unclamped over the whole range. Taken 2026-08-05.
#[test]
fn effective_liquidus_spans_solidus_to_liquidus_with_no_eutectic_offset() {
    let m = model();
    let at_zero = m.effective_liquidus(0.0);
    let at_half = m.effective_liquidus(0.5);
    let at_one = m.effective_liquidus(1.0);
    println!("T_eff(0) = {at_zero}, T_eff(0.5) = {at_half}, T_eff(1) = {at_one}");
    assert!((at_zero - T_SOL).abs() < 1e-12);
    assert!((at_half - 0.5 * (T_SOL + T_LIQ)).abs() < 1e-12);
    assert!((at_one - T_LIQ).abs() < 1e-12);
}

/// **Methodology.** With a eutectic fraction `α₁ₑ = 0.3`, the linear term
/// `T_sol + (T_liq - T_sol)(α₁ - α₁ₑ)/(1 - α₁ₑ)` is *below* the solidus for
/// every `α₁ < α₁ₑ`, so the `max` clamp binds and the effective liquidus must
/// sit flat at the solidus. That plateau is what makes a eutectic a eutectic:
/// the material holds one temperature while the first fraction melts. Sample
/// `α₁ = 0.0, 0.1, 0.2, 0.3` (clamped region and its boundary) and
/// `α₁ = 0.65, 1.0` (unclamped). Pass criterion: exactly `T_sol` below the
/// eutectic fraction; strictly above it beyond.
///
/// **Results.** Measured, in K:
///
/// ```text
/// alpha = 0.00  ->  T_eff = 302 (clamped)
/// alpha = 0.10  ->  T_eff = 302 (clamped)
/// alpha = 0.20  ->  T_eff = 302 (clamped)
/// alpha = 0.30  ->  T_eff = 302 (clamped)
/// alpha = 0.65  ->  T_eff = 303
/// alpha = 1.00  ->  T_eff = 304
/// ```
///
/// The plateau holds flat at 302 K across the whole sub-eutectic range and
/// releases exactly at `α₁ = α₁ₑ = 0.3`; beyond it the ramp is linear and still
/// terminates at the liquidus, confirming the eutectic fraction compresses the
/// ramp into `[α₁ₑ, 1]` rather than shifting it. Taken 2026-08-05.
#[test]
fn effective_liquidus_holds_flat_at_the_solidus_below_the_eutectic_fraction() {
    let mut c = coefficients();
    c.eutectic_fraction = 0.3;
    let m = SolidificationMelting::new(
        "eutectic",
        "U",
        "T",
        true,
        CellSelection::All,
        c,
        gravity(),
        N_CELLS,
    );

    for alpha in [0.0, 0.1, 0.2, 0.3] {
        let t = m.effective_liquidus(alpha);
        println!("alpha = {alpha:.2}  ->  T_eff = {t} (clamped)");
        assert!(
            (t - T_SOL).abs() < 1e-12,
            "below the eutectic fraction T_eff must clamp to the solidus, got {t}"
        );
    }
    for alpha in [0.65, 1.0] {
        let t = m.effective_liquidus(alpha);
        println!("alpha = {alpha:.2}  ->  T_eff = {t}");
        assert!(t > T_SOL);
    }
    assert!((m.effective_liquidus(1.0) - T_LIQ).abs() < 1e-12);
}

// ── Momentum source placement (sign regressions) ─────────────────────────────

/// **Methodology — this is a sign regression test, and the reason it exists.**
///
/// Upstream writes `Sp[celli] += Vc*S` (with `S` negative) into the `fvModels`
/// matrix, but that matrix is *subtracted* from the solver's equation by
/// `solve(UEqn == fvModels.source(...))`, since `operator==(A, B)` expands to
/// `A - B`. A literal transcription into a matrix that **is** the solved system
/// therefore flips the drag: the diagonal would *lose* `V·Cu/q` instead of
/// gaining it, destroying diagonal dominance exactly where the coefficient is
/// largest, and the momentum sink would become a source inside the solid.
///
/// Hold all four cells at 290 K — well below the solidus, so the liquid
/// fraction clamps to 0 and the drag saturates at `-Cu/q`. Reference: the
/// closed form `diag = -V·(-Cu/q) = +V·Cu/q = 0.5 × 1e8`. Pass criterion:
/// the diagonal is strictly **positive** and matches to a relative tolerance of
/// 1e-12.
///
/// **Results.** Measured `diag[0] = 50000000` against the closed form
/// `50000000`, relative error `0`; liquid fraction `0` in every cell as
/// expected below the solidus. The diagonal gains `5e7`, seven orders of
/// magnitude above any convective or diffusive coefficient this mesh would
/// produce, which is what pins the solid. The test fails loudly on the inverted
/// sign, which is what it is for. Taken 2026-08-05.
#[test]
fn momentum_diagonal_gains_the_drag_rather_than_losing_it() {
    let mesh = mesh();
    let mut m = model();
    let t = uniform_temperature(&mesh, 290.0);
    let mut eqn = FvVectorMatrix::new(mesh.clone());

    m.add_momentum_source(&t, &mut eqn);

    let c = coefficients();
    let closed_form = CELL_VOLUME * c.darcy_coefficient / c.darcy_regularisation;
    let rel = ((eqn.ldu.diag[0] - closed_form) / closed_form).abs();
    println!(
        "diag[0] = {}, closed form = {closed_form}, rel err = {rel:e}, alpha = {:?}",
        eqn.ldu.diag[0],
        m.liquid_fraction()
    );

    assert!(
        eqn.ldu.diag[0] > 0.0,
        "the Darcy drag must ADD to the diagonal; got {} — the upstream sign \
         was transcribed without accounting for the `==` negation",
        eqn.ldu.diag[0]
    );
    assert!(rel < 1e-12);
    assert!(m.liquid_fraction().iter().all(|&a| a == 0.0));
}

/// **Methodology — the companion sign regression, for buoyancy.**
///
/// The Boussinesq force on a cell hotter than its surroundings is
/// `-ρ_ref β ΔT g`, which for `ΔT > 0` and gravity along `-y` points along
/// `+y`: hot melt rises. Upstream's `Sb = rhoRef*g*beta*deltaT` carries the
/// opposite sign precisely because it is negated on the way into the solved
/// system; transcribed literally it would sink hot melt and float cold melt,
/// inverting the natural-convection cell that drives the whole melting process.
///
/// Hold all cells at 320 K (above the liquidus, so `ΔT > 0`) and check the
/// momentum source. Pass criterion: `source.y > 0`, with `x` and `z` exactly
/// zero since gravity has no component there.
///
/// **Results.** Measured momentum source
/// `(0, 64.00110602410335, 0)` N and liquid fraction `0.0770995508982036`
/// after one update. The `+y` sign confirms a superheated cell is pushed
/// against gravity. The magnitude is small next to the `5e7` drag diagonal of
/// the previous test, which is the correct physical picture: buoyancy drives
/// the melt only where the Darcy term has already released it. Taken
/// 2026-08-05.
#[test]
fn buoyancy_pushes_a_superheated_cell_against_gravity() {
    let mesh = mesh();
    let mut m = model();
    let t = uniform_temperature(&mesh, 320.0);
    let mut eqn = FvVectorMatrix::new(mesh.clone());

    m.add_momentum_source(&t, &mut eqn);

    let s = eqn.source[0];
    println!(
        "momentum source[0] = ({}, {}, {}), alpha = {}",
        s.x,
        s.y,
        s.z,
        m.liquid_fraction()[0]
    );

    assert!(
        s.y > 0.0,
        "a cell above its effective liquidus must be pushed against gravity; \
         got y = {} — the upstream buoyancy sign was transcribed without \
         accounting for the `==` negation",
        s.y
    );
    assert_eq!(s.x, 0.0);
    assert_eq!(s.z, 0.0);
}

// ── Latent heat ──────────────────────────────────────────────────────────────

/// **Methodology.** The latent-heat source is proportional to `∂(ρα₁)/∂t`, so a
/// cell whose liquid fraction does not change over a step must contribute
/// exactly nothing — otherwise a fully molten pool would keep absorbing latent
/// heat forever. Drive the model to full melt with a 600 K field (which
/// saturates `α₁` at 1), call `advance_time`, then apply the energy source at
/// the same temperature. Pass criterion: bit-exact zero contribution in every
/// cell.
///
/// **Results.** Measured liquid fraction `1` before and after the second
/// update, and energy source `[0.0, 0.0, 0.0, 0.0]` — exactly zero in all four
/// cells. The saturated state is a genuine fixed point of the update, not one
/// that merely leaks a small residual source. Taken 2026-08-05.
#[test]
fn latent_heat_vanishes_when_the_liquid_fraction_is_unchanged() {
    let mesh = mesh();
    let mut m = model();
    let hot = uniform_temperature(&mesh, 600.0);
    let rho = VolScalarField::uniform("rho", mesh.clone(), RHO_REF);

    // Melt fully, then roll the history forward so old == new.
    m.update(&hot);
    let before = m.liquid_fraction()[0];
    m.advance_time();

    let mut eqn = FvMatrix::new(mesh.clone());
    m.add_energy_source(&rho, &hot, 0.01, &mut eqn);

    println!(
        "alpha before = {before}, after = {}, energy source = {:?}",
        m.liquid_fraction()[0],
        eqn.source.as_slice()
    );
    assert_eq!(before, 1.0, "600 K must saturate the liquid fraction");
    for cell in 0..N_CELLS {
        assert_eq!(
            eqn.source[cell], 0.0,
            "an unchanging liquid fraction must contribute no latent heat"
        );
    }
}

/// **Methodology.** Melting absorbs energy, so a cell whose liquid fraction is
/// rising must *remove* energy from the right-hand side of its equation — that
/// removal is what holds a melting cell at its melting point instead of letting
/// it heat straight through. Start from fully solid (`α₁ = 0` at the previous
/// step), apply a 320 K field, and inspect the sign. Reference: the closed form
/// `-V·(L/Cp)·ρ·Δα₁/Δt`. Pass criterion: strictly negative, matching the closed
/// form to a relative tolerance of 1e-12.
///
/// **Results.** Measured energy source `-4935329.999999999` W against the
/// closed form `-4935329.999999999`, relative error `0`, from
/// `Δα₁ = 0.0770995508982036` over `Δt = 0.01` s. The sink is ~4.94 MW in a
/// 0.5 m³ cell, which is the correct order for melting 7.7 % of 6093 kg/m³ of
/// gallium at 80160 J/kg over a hundredth of a second, and it is negative —
/// energy leaves the equation. Taken 2026-08-05.
#[test]
fn melting_removes_energy_from_the_equation() {
    let mesh = mesh();
    let mut m = model();
    let t = uniform_temperature(&mesh, 320.0);
    let rho = VolScalarField::uniform("rho", mesh.clone(), RHO_REF);
    let dt = 0.01;

    let mut eqn = FvMatrix::new(mesh.clone());
    m.add_energy_source(&rho, &t, dt, &mut eqn);

    let d_alpha = m.liquid_fraction()[0];
    let closed_form = -CELL_VOLUME * (LATENT_HEAT / SPECIFIC_HEAT) * RHO_REF * d_alpha / dt;
    let rel = ((eqn.source[0] - closed_form) / closed_form).abs();
    println!(
        "energy source[0] = {}, closed form = {closed_form}, rel err = {rel:e}, d_alpha = {d_alpha}",
        eqn.source[0]
    );

    assert!(
        eqn.source[0] < 0.0,
        "melting must absorb energy, got {}",
        eqn.source[0]
    );
    assert!(rel < 1e-12);
}

/// **Methodology.** The temperature and enthalpy forms of the energy equation
/// differ by exactly one factor of `Cp`: upstream applies `L/Cp·∂(ρα₁)/∂t` when
/// the solved field is a temperature and `L·∂(ρα₁)/∂t` when it is an enthalpy.
/// Getting that branch wrong scales the latent heat by several hundred and is
/// silent — the equation still solves, it just melts at the wrong rate. Build
/// two models identical but for the `energy_is_temperature` flag, drive both
/// with the same field, and take the ratio. Pass criterion: the ratio equals
/// `Cp = 381.5` to a relative tolerance of 1e-12.
///
/// **Results.** Measured enthalpy-form source `-1882828394.9999998` W,
/// temperature-form source `-4935329.999999999` W, ratio `381.5` against
/// `Cp = 381.5`, relative error `0`. The two forms differ by exactly the
/// specific heat — to the last bit, not merely to tolerance — confirming the
/// branch is the only difference between them. The absolute gap is worth
/// noting: 1.88 GW against 4.94 MW. Taking the wrong branch is a 382-fold error
/// that the equation would still solve without complaint. Taken 2026-08-05.
#[test]
fn temperature_and_enthalpy_forms_differ_by_exactly_the_specific_heat() {
    let mesh = mesh();
    let rho = VolScalarField::uniform("rho", mesh.clone(), RHO_REF);
    let t = uniform_temperature(&mesh, 320.0);
    let dt = 0.01;

    let mut in_temperature = model();
    let mut in_enthalpy = SolidificationMelting::new(
        "melting",
        "U",
        "h",
        false,
        CellSelection::All,
        coefficients(),
        gravity(),
        N_CELLS,
    );

    let mut eqn_t = FvMatrix::new(mesh.clone());
    let mut eqn_h = FvMatrix::new(mesh.clone());
    in_temperature.add_energy_source(&rho, &t, dt, &mut eqn_t);
    in_enthalpy.add_energy_source(&rho, &t, dt, &mut eqn_h);

    let ratio = eqn_h.source[0] / eqn_t.source[0];
    let rel = ((ratio - SPECIFIC_HEAT) / SPECIFIC_HEAT).abs();
    println!(
        "enthalpy form = {}, temperature form = {}, ratio = {ratio}, Cp = {SPECIFIC_HEAT}, rel err = {rel:e}",
        eqn_h.source[0], eqn_t.source[0]
    );
    assert!(rel < 1e-12, "ratio {ratio} != Cp {SPECIFIC_HEAT}");
}

// ── Once-per-timestep guard ──────────────────────────────────────────────────

/// **Methodology.** A solver applies this model to both the momentum and the
/// energy equation within one timestep, and each apply calls `update`. Without
/// upstream's `curTimeIndex_` guard the under-relaxed explicit iteration would
/// advance twice per step, making the effective relaxation depend on how many
/// equations happen to reference the model. Call `update` at 320 K, record the
/// fraction, then call it again at a far hotter 600 K within the same step and
/// require no change; then `advance_time` and update again and require it to
/// move. Pass criterion: bit-exact equality within the step, strict inequality
/// across it.
///
/// **Results.** Measured `α₁` after the first update `0.0770995508982036`;
/// after a second in-step update at 600 K, unchanged at
/// `0.0770995508982036`; after `advance_time` and a further update at 600 K,
/// `1`. The guard holds against a 280 K temperature jump — a difference that
/// would be impossible to miss had it leaked — and releases exactly when the
/// timestep is advanced. Taken 2026-08-05.
#[test]
fn update_runs_at_most_once_per_timestep() {
    let mesh = mesh();
    let mut m = model();
    let warm = uniform_temperature(&mesh, 320.0);
    let hot = uniform_temperature(&mesh, 600.0);

    m.update(&warm);
    let first = m.liquid_fraction()[0];

    m.update(&hot);
    let second = m.liquid_fraction()[0];

    m.advance_time();
    m.update(&hot);
    let third = m.liquid_fraction()[0];

    println!("alpha: first = {first}, same step at 600 K = {second}, next step = {third}");
    assert_eq!(
        first, second,
        "a second update within one timestep must be a no-op"
    );
    assert!(
        third > second,
        "advance_time must re-arm the update: {third} not > {second}"
    );
}

// ── Cell selection ───────────────────────────────────────────────────────────

/// **Methodology.** A melting model normally occupies one material region, not
/// the whole domain, so a zone selection must leave every unselected cell
/// bit-exactly untouched. A source that leaked outside its zone would melt
/// material that is not there. Apply a model restricted to cells `{1, 2}` on
/// the 4-cell mesh and inspect all four diagonals and sources. Pass criterion:
/// cells 0 and 3 hold exactly zero in both the matrix diagonal and the source;
/// cells 1 and 2 are nonzero.
///
/// **Results.** Measured momentum diagonals
/// `[0.0, 50000000.0, 50000000.0, 0.0]` and source y-components
/// `[0.0, -43.036077600000006, -43.036077600000006, 0.0]`. The two selected
/// cells carry the
/// full saturated drag and a downward buoyancy (they are below the solidus at
/// 290 K, so cold-and-sinking is correct); the two unselected cells are exactly
/// zero. Taken 2026-08-05.
#[test]
fn a_zone_selection_leaves_unselected_cells_untouched() {
    let mesh = mesh();
    let mut m = SolidificationMelting::new(
        "zoned",
        "U",
        "T",
        true,
        CellSelection::zone(vec![1, 2]),
        coefficients(),
        gravity(),
        N_CELLS,
    );
    let t = uniform_temperature(&mesh, 290.0);
    let mut eqn = FvVectorMatrix::new(mesh.clone());

    m.add_momentum_source(&t, &mut eqn);

    let diag: Vec<f64> = (0..N_CELLS).map(|c| eqn.ldu.diag[c]).collect();
    let source_y: Vec<f64> = (0..N_CELLS).map(|c| eqn.source[c].y).collect();
    println!("diag = {diag:?}");
    println!("source y = {source_y:?}");

    for cell in [0, 3] {
        assert_eq!(diag[cell], 0.0, "cell {cell} is outside the zone");
        assert_eq!(source_y[cell], 0.0, "cell {cell} is outside the zone");
    }
    for cell in [1, 2] {
        assert!(diag[cell] > 0.0);
        assert!(source_y[cell] != 0.0);
    }
}

// ── Bounds ───────────────────────────────────────────────────────────────────

/// **Methodology.** The liquid fraction is a physical fraction and must stay in
/// `[0, 1]` under any temperature history, including ones that would drive the
/// unclamped explicit update far outside it. Run twenty steps heating from
/// 250 K to 700 K and twenty cooling back, calling `advance_time` between
/// steps, and check the bound at every step. Pass criterion: `0 ≤ α₁ ≤ 1`
/// throughout, and the cycle actually exercises both clamps.
///
/// **Results.** Over 40 steps the fraction stayed within `[0, 1]` at every
/// step, reaching a maximum of `1` and a minimum of `0`. Both clamps were
/// exercised, so the bound is enforced rather than merely never approached. The
/// cycle is **not** reversible — the fraction returns to 0 only because the
/// clamp catches it, not because the update is a bijection — which is expected
/// of an under-relaxed explicit scheme and is why the model is a per-timestep
/// iteration rather than a state function of temperature. Taken 2026-08-05.
#[test]
fn the_liquid_fraction_stays_in_bounds_over_a_heating_and_cooling_cycle() {
    let mesh = mesh();
    let mut m = model();

    let mut lowest = f64::INFINITY;
    let mut highest = f64::NEG_INFINITY;

    let ramp = (0..20)
        .map(|i| 250.0 + 450.0 * f64::from(i) / 19.0)
        .chain((0..20).map(|i| 700.0 - 450.0 * f64::from(i) / 19.0));

    for temperature in ramp {
        let field = uniform_temperature(&mesh, temperature);
        m.update(&field);
        let alpha = m.liquid_fraction()[0];
        assert!(
            (0.0..=1.0).contains(&alpha),
            "liquid fraction {alpha} out of bounds at T = {temperature}"
        );
        lowest = lowest.min(alpha);
        highest = highest.max(alpha);
        m.advance_time();
    }

    println!("over the cycle: min alpha = {lowest}, max alpha = {highest}");
    assert_eq!(highest, 1.0, "the cycle must exercise the upper clamp");
    assert_eq!(lowest, 0.0, "the cycle must exercise the lower clamp");
}
