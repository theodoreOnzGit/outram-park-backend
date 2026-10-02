// SPDX-License-Identifier: GPL-3.0-only
//! # V&V — equilibrium-reactor parity against upstream DWSIM (code-to-code)
//!
//! **Methodology.** Upstream DWSIM, built from the pinned commit
//! `1abf72d1b6b41d3e9a8cc770d3cc4e8fc76e5766` and run headless on Linux (driver
//! `docs/upstream-harness/equilibrium_driver.cs`, procedure in that folder's
//! README). Each case builds a real `Reactor_Equilibrium` on a headless
//! flowsheet (inlet, vapour and liquid outlets, energy stream; Peng-Robinson;
//! isothermal), creates one reaction with upstream's own
//! `FlowsheetBase.CreateEquilibriumReaction` (`ReactionPhase` vapour, approach
//! 0 K), and calls `Calculate()`. Outlet flows are the sum of both outlet
//! streams. The same reaction is solved by this crate's [`EquilibriumReactor`]
//! with `tol = 1e-14`.
//!
//! **Why an explicit `ln K(T)`.** Both codes are given the *same* expression
//! (`K = exp(expr(T))`, upstream `ThermodynamicsBase.vb:274-284`; this crate's
//! `EquilibriumConstant::LnPolynomial`), so `K` is identical and the comparison
//! isolates the activity basis and the solver. Upstream's `Gibbs` option would
//! instead compute `K` from its compound database, which this crate does not
//! carry. The two forms used — water-gas shift `ln K = 4577.8/T − 4.33`, steam
//! reforming `ln K = 30.42 − 27106/T` — are fixed inputs typed into both codes,
//! **not validated correlations**; their accuracy does not enter a code-to-code
//! comparison.
//!
//! **Upstream was run at `InternalLoopTolerance = 1e-20`, not its default
//! `1e-3`** (`Equilibrium.vb:77`), which bounds the *sum of squared*
//! `ln`-residuals (`:1407`). At the default, upstream stops measurably short:
//! water-gas shift extent 0.531295 vs the closed form 0.530935 (+6.8e-4),
//! reforming on mole fractions 0.851456 vs 0.850515 (+1.1e-3), and the
//! partial-pressure reforming case returns **NaN** outlet flows (extent
//! 0.371670), reproducibly. The default-tolerance numbers measure upstream's
//! stopping rule and are not used below.
//!
//! **Results (measured 2026-10-02, release).** Extent `ξ` of the one reaction;
//! gap = this crate relative to upstream.
//!
//! | case | basis | upstream `ξ` | this crate `ξ` | gap | what it measures |
//! |---|---|---|---|---|---|
//! | WGS, 1000 K, 1 bar, N2 inert | mole fraction | 0.530935435426307 | 0.530935435424488 | 3.4e-12 (per species ≤ 3.9e-12) | solver; `MolarFrac` has no `φ` upstream (`:343`) |
//! | SMR, 900 K, 10 bar | mole fraction | 0.850515094310887 | 0.850515094310887 | per species ≤ 1.7e-15 | solver, `Δν = 2` |
//! | WGS, 1000 K, 1 bar | activity | 0.530914918961434 | 0.530935435424488 | +3.9e-5 | PR `φ` at 1 bar (`Δν = 0`) |
//! | SMR, 900 K, 10 bar | partial pressure [Pa] | 0.370634803380462 | 0.371811200881477 | **+0.32 %** | ideal-`φ` simplification |
//! | SMR, 900 K, 30 bar | partial pressure [Pa] | 0.219788818225247 | 0.222236771371677 | **+1.11 %** | ideal-`φ` simplification |
//! | SMR, 900 K, 10 bar | fugacity | 0.370634803381206 | before fix 0.850515 (**+129 %**); after 0.371811200881477 (**+0.32 %**) | port defect, fixed |
//! | SMR, 900 K, 10 bar | activity | 0.370634803381206 | 0.850515094310887 | **+129 %** | `(P/P0)^Δν` — recorded gap |
//!
//! **What the comparison found.**
//!
//! 1. **Solver parity.** Where upstream uses no fugacity coefficient (the
//!    mole-fraction basis) the two codes agree to ≤ 3.9e-12 per species, and
//!    the water-gas-shift extent equals the closed form `√K/(1+√K)`.
//! 2. **Port defect, fixed in the same change: the fugacity basis.**
//!    `basis_value` had no `Fugacity` arm and returned the mole fraction,
//!    contradicting `ReactionBasis::Fugacity`'s own `fᵢ = φᵢ yᵢ P` doc.
//!    Upstream's vapour fugacity basis is `φᵢ·yᵢ·P/P0`, `P0 = 101325 Pa`
//!    (`Equilibrium.vb:322`, `:338-339`, `:1076`). Now `xᵢ·P/P0`; reforming at
//!    10 bar moved from +129 % to +0.32 %, the residual being `φ`.
//! 3. **Recorded gap: the activity basis for vapour reactions.** Upstream
//!    treats `Activity` and `Fugacity` identically for a vapour reaction
//!    (`:338`), i.e. `φᵢ·yᵢ·P/P0`; this crate treats `Activity` as an ideal
//!    liquid, `xᵢ`. They differ by `(P/P0)^Δν`: +129 % extent for reforming at
//!    10 bar. Not changed — the crate has no `ReactionPhase` to pick the
//!    phase-dependent meaning, and its `Activity` doc (`aᵢ = γᵢ xᵢ`) is
//!    consistent with the code. A caller with a gas-phase `K` must use
//!    `Fugacity` or `PartialPressure`.
//! 4. **The ideal-`φ` simplification has a measured price**: +0.32 % extent at
//!    10 bar, +1.11 % at 30 bar (reforming, 900 K; PR `φ` of 0.996–1.016), and
//!    +3.9e-5 at 1 bar (WGS, `Δν = 0`, so only the `φ` ratio enters).
//!
//! > Verification against upstream, not validation against experiment.
//!
//! [`EquilibriumReactor`]: outram_park_fork_dwsim_libs::reactors::EquilibriumReactor

use outram_park_fork_dwsim_libs::reactions::{
    EquilibriumConstant, Reaction, ReactionBasis, ReactionComponent, ReactionKind,
};
use outram_park_fork_dwsim_libs::reactors::{EquilibriumReactor, ReactorFeed, ReactorOutcome};

/// Gate on the parity cases (no fugacity coefficient involved).
const GATE: f64 = 1e-10;
/// `2 ln(101325)`, typed into both codes to put the reforming `K` in Pa².
const LN_P0_SQUARED: f64 = 23.052_176_902_993_02;

fn rxn(basis: ReactionBasis, comps: &[(usize, f64)], a: f64, b: f64) -> Reaction {
    let components = comps
        .iter()
        .enumerate()
        .map(|(k, &(i, nu))| ReactionComponent::new(i, nu, 0.0, 0.0, k == 0))
        .collect();
    Reaction::new(ReactionKind::Equilibrium, basis, components).with_k_eq(
        EquilibriumConstant::LnPolynomial {
            a,
            b,
            c: 0.0,
            d: 0.0,
        },
    )
}

/// CO + H2O ⇌ CO2 + H2; species CO, H2O, CO2, H2, N2.
fn wgs(basis: ReactionBasis) -> Reaction {
    rxn(
        basis,
        &[(0, -1.0), (1, -1.0), (2, 1.0), (3, 1.0)],
        -4.33,
        4577.8,
    )
}

/// CH4 + H2O ⇌ CO + 3 H2; species CH4, H2O, CO, H2.
fn smr(basis: ReactionBasis, a: f64) -> Reaction {
    rxn(
        basis,
        &[(0, -1.0), (1, -1.0), (2, 1.0), (3, 3.0)],
        a,
        -27106.0,
    )
}

fn solve(reaction: Reaction, feed: &ReactorFeed) -> ReactorOutcome {
    let mut reactor = EquilibriumReactor::new(vec![reaction]);
    reactor.tol = 1e-14;
    reactor.solve(feed).expect("equilibrium converges")
}

fn wgs_feed() -> ReactorFeed {
    ReactorFeed::new(vec![1.0, 1.0, 0.0, 0.0, 1.0], 1000.0, 1.0e5, 0.0)
}

fn smr_feed(pressure: f64) -> ReactorFeed {
    ReactorFeed::new(vec![1.0, 3.0, 0.0, 0.0], 900.0, pressure, 0.0)
}

fn worst_gap(port: &[f64], upstream: &[f64]) -> f64 {
    port.iter()
        .zip(upstream)
        .map(|(&p, &u)| ((p - u) / u).abs())
        .fold(0.0, f64::max)
}

fn rel(port: f64, upstream: f64) -> f64 {
    (port - upstream) / upstream
}

/// Water-gas shift on the **mole-fraction** basis: 1 mol/s each of CO and H2O
/// with 1 mol/s N2 inert, 1000 K, 1 bar. `K = 1.28120367`.
///
/// **Result (2026-10-02).** Upstream ξ = 0.530935435426307, this crate
/// 0.530935435424488: worst per-species gap 3.9e-12 (N2 is untouched in both).
/// This crate equals the closed form `√K/(1+√K)` to < 1e-10 (gated).
#[test]
fn wgs_mole_fraction_matches_upstream() {
    const UPSTREAM: [f64; 5] = [
        0.469_064_564_573_693,
        0.469_064_564_573_693,
        0.530_935_435_426_307,
        0.530_935_435_426_307,
        0.999_999_999_999_999_78,
    ];
    let out = solve(wgs(ReactionBasis::MolarFraction), &wgs_feed());
    let gap = worst_gap(&out.molar_flows, &UPSTREAM);
    eprintln!("wgs molfrac: xi {:.15}, worst gap {gap:e}", out.extents[0]);
    assert!(gap < GATE, "worst gap {gap:e}: {:?}", out.molar_flows);

    let sqrt_k = (4577.8_f64 / 1000.0 - 4.33).exp().sqrt();
    let closed = sqrt_k / (1.0 + sqrt_k);
    assert!(
        (out.extents[0] - closed).abs() < GATE,
        "{} vs {closed}",
        out.extents[0]
    );
}

/// Steam reforming on the **mole-fraction** basis (`Δν = +2`, so the mole
/// total changes): CH4 1, H2O 3 mol/s, 900 K, 10 bar. `K = 1.35286183`.
///
/// **Result (2026-10-02).** Upstream ξ = 0.850515094310887, this crate
/// 0.850515094310887: worst per-species gap 1.7e-15.
#[test]
fn smr_mole_fraction_matches_upstream() {
    const UPSTREAM: [f64; 4] = [
        0.149_484_905_689_113_18,
        2.149_484_905_689_112_8,
        0.850_515_094_310_886_57,
        2.551_545_282_932_66,
    ];
    let out = solve(smr(ReactionBasis::MolarFraction, 30.42), &smr_feed(1.0e6));
    let gap = worst_gap(&out.molar_flows, &UPSTREAM);
    eprintln!("smr molfrac: xi {:.15}, worst gap {gap:e}", out.extents[0]);
    assert!(gap < GATE, "worst gap {gap:e}: {:?}", out.molar_flows);
}

/// The **fugacity** basis, after the fix: this crate's `xᵢ·P/P0` must give the
/// same extent as its `xᵢ·P` partial-pressure basis with `K·P0²` — the two are
/// the same equation — and must sit at upstream's ideal-`φ` gap.
///
/// **Result (2026-10-02).** Fugacity and partial pressure agree to 1e-15 in
/// this crate (0.371811200881477). Against upstream's 0.370634803381206 the
/// gap is **+0.317 %**, all of it PR `φ` (upstream `:313`). Before the fix the
/// fugacity basis returned the mole-fraction answer, 0.850515: **+129 %**.
#[test]
fn smr_fugacity_basis_matches_partial_pressure_and_records_phi_gap() {
    const UPSTREAM_XI: f64 = 0.370_634_803_381_205_91;
    let fug = solve(smr(ReactionBasis::Fugacity, 30.42), &smr_feed(1.0e6));
    let pp = solve(
        smr(ReactionBasis::PartialPressure, 30.42 + LN_P0_SQUARED),
        &smr_feed(1.0e6),
    );
    let gap = rel(fug.extents[0], UPSTREAM_XI);
    eprintln!(
        "smr fugacity: xi {:.15} (pp {:.15}), gap vs upstream {gap:e}",
        fug.extents[0], pp.extents[0]
    );
    assert!((fug.extents[0] - pp.extents[0]).abs() < GATE);
    assert!(
        (0.003..0.0035).contains(&gap),
        "recorded phi gap moved: {gap:e}"
    );
}

/// The ideal-`φ` simplification on the **partial-pressure** basis [Pa], at 10
/// and 30 bar. Upstream multiplies by its PR vapour `φ` (`Equilibrium.vb:345`);
/// this crate does not.
///
/// **Result (2026-10-02).** 10 bar: upstream 0.370634803380462, this crate
/// 0.371811200881477, **+0.317 %**. 30 bar: upstream 0.219788818225247, this
/// crate 0.222236771371677, **+1.114 %**. Upstream's PR `φ` at the 30 bar
/// outlet: CH4 1.0144, H2O 0.9871, CO 1.0156, H2 1.0107. Pinned so that adding
/// real fugacity coefficients has to update it.
#[test]
fn ideal_phi_cost_is_recorded() {
    for (pressure, upstream_xi, lo, hi) in [
        (1.0e6, 0.370_634_803_380_462_06, 0.003, 0.0035),
        (3.0e6, 0.219_788_818_225_246_98, 0.0105, 0.0115),
    ] {
        let out = solve(
            smr(ReactionBasis::PartialPressure, 30.42 + LN_P0_SQUARED),
            &smr_feed(pressure),
        );
        let gap = rel(out.extents[0], upstream_xi);
        eprintln!(
            "smr pp {pressure} Pa: xi {:.15}, gap {gap:e}",
            out.extents[0]
        );
        assert!(
            (lo..hi).contains(&gap),
            "recorded gap at {pressure} Pa moved: {gap:e}"
        );
    }

    // WGS at 1 bar, Δν = 0: only the φ ratio enters.
    const UPSTREAM_WGS_ACTIVITY: f64 = 0.530_914_918_961_433_72;
    let out = solve(wgs(ReactionBasis::Activity), &wgs_feed());
    let gap = rel(out.extents[0], UPSTREAM_WGS_ACTIVITY);
    eprintln!("wgs activity: gap {gap:e}");
    assert!((3e-5..5e-5).contains(&gap), "recorded gap moved: {gap:e}");
}

/// The **activity** basis for a vapour reaction with `Δν ≠ 0` — a recorded
/// gap. Upstream: `φᵢ·yᵢ·P/P0` (`Equilibrium.vb:338`), ξ = 0.370634803381206.
/// This crate: `xᵢ` (ideal liquid), ξ = 0.850515094310887, **+129 %** — the
/// `(P/P0)^2` factor at 10 bar. Pinned so that porting `ReactionPhase` has to
/// update it.
#[test]
fn activity_basis_vapour_gap_is_recorded() {
    const UPSTREAM_XI: f64 = 0.370_634_803_381_205_91;
    let out = solve(smr(ReactionBasis::Activity, 30.42), &smr_feed(1.0e6));
    let gap = rel(out.extents[0], UPSTREAM_XI);
    eprintln!("smr activity: xi {:.15}, gap {gap:e}", out.extents[0]);
    assert!((1.28..1.31).contains(&gap), "recorded gap moved: {gap:e}");
}
