// SPDX-License-Identifier: GPL-3.0-only
//! # V&V — CSTR parity against upstream DWSIM (code-to-code)
//!
//! **Methodology.** Upstream DWSIM 9.0.5.0, built from the pinned commit
//! `1abf72d1b6b41d3e9a8cc770d3cc4e8fc76e5766` and run headless on Linux
//! (driver `docs/upstream-harness/cstr_driver.cs`, procedure in that folder's
//! README). Each case builds a real `Reactor_CSTR` on a headless flowsheet —
//! inlet, outlet and energy streams connected, a kinetic reaction set on a
//! `MolarConc` basis, isothermal mode, Peng-Robinson — and calls `Calculate()`.
//! The same reactions and volume are then solved by this crate's [`Cstr`].
//!
//! **Why this crate is handed upstream's *outlet* `Q`.** The two codes form
//! concentrations differently. This crate holds the volumetric flow fixed
//! (`Cᵢ = Fᵢ/Q`, `Q` an input). Upstream re-flashes the tank contents every
//! relaxation sweep, so its `Q` is the real volumetric flow *of the outlet
//! composition* (`CSTR.vb:584`). Those coincide only when `Q` does not change
//! through the reactor. To compare the *solvers* rather than that modelling
//! choice, this crate is given `Q = V / τ_L`, with `τ_L` the residence time
//! upstream reports — the `Q` its own rate law saw on the last sweep. What the
//! modelling choice costs is measured separately and recorded below.
//!
//! **Upstream was run at a tight `Tolerance` (1e-11, and 1e-13 for steam
//! reforming), not its default `1e-5`.** Its
//! loop stops on a *per-step change* test (`IErr < Tolerance Or MaxChange <
//! Tolerance*10`, `CSTR.vb:968`), not on the balance residual, and on a stiff
//! system the step-size cap makes the per-step change small long before steady
//! state. At the default tolerance the steam-reforming case below stops at
//! methane conversion **0.389621**, whose steady-state balance residual is
//! **0.138 mol/s** against a 4 mol/s feed; tightening the tolerance moves it to
//! 0.375803 and the residual closes. The default-tolerance numbers therefore
//! measure upstream's stopping rule, not its model, and are not used here.
//!
//! **Results (measured 2026-09-25, release).**
//!
//! | case | upstream | this crate | relative gap |
//! |---|---|---|---|
//! | liquid n-butane → isobutane, first order | X = 0.657703283985040 | 0.657703283988240 | 4.9e-12 |
//! | vapour n-butane → isobutane (*diagnostic build*) | X = 0.858781337972923 | 0.858781337973070 | 1.7e-13 |
//! | steam reforming + shift, 5 species (*diagnostic build*) | X(CH4) = 0.375803270 | 0.375803270 | ≤ 6.1e-10 per species flow |
//!
//! In both isomerisation cases this crate also equals the closed form
//! `X = kτ/(1 + kτ)` to every printed digit.
//!
//! **Two upstream defects found, recorded here so nobody mistakes them for
//! port errors (GitHub issue #326).**
//!
//! 1. **An all-vapour CSTR returns zero conversion.** `CSTR.vb:508` seeds the
//!    relaxation step as `dT = ResidenceTimeL / 10`, and `ResidenceTimeL =
//!    Volume / (QL + QS)` is **zero** when there is no liquid or solid. Nothing
//!    later increases `dT` (the `dT *= 1.2` acceleration at `CSTR.vb:917` is
//!    commented out), so every sweep changes nothing and the loop "converges"
//!    immediately to the inlet. Measured on the pristine build: the vapour
//!    isomerisation returns `X = 0` where the closed form gives 0.8589, and the
//!    steam-reforming case returns the feed composition. **Proved, not
//!    inferred**: a one-line diagnostic patch seeding `dT` from `V/Q` when
//!    `ResidenceTimeL` is zero (`docs/upstream-harness/cstr_diagnostic_dt_seed.patch`,
//!    applied to a build copy only) restores 0.858781 and leaves the liquid case
//!    bit-identical. The two vapour-phase rows above use that diagnostic build,
//!    and say so; they compare this crate against upstream's *model*, which the
//!    pristine build cannot currently exercise in the vapour.
//! 2. **The default stopping rule under-converges stiff cases** — the
//!    0.389621-vs-0.375803 result above.
//!
//! **What the fixed-`Q` choice costs this crate.** Handed the *inlet* `Q`
//! instead, as a caller normally would, the steam-reforming case gives
//! methane conversion **0.411745** against upstream's 0.375803 — **+9.6 %** —
//! because reforming takes 2 mol to 4 and upstream's `Q` grows **19.0 %**
//! through the reactor (0.018678 → 0.022219 m³/s). For the isomerisation
//! (`Δn = 0`) the same choice costs 0.87 %, from the liquid density difference
//! between the isomers alone. This is a documented modelling simplification
//! (see [`outram_park_fork_dwsim_libs::reactors`]), now with a measured size;
//! it is not a solver defect, and the parity above is what shows that.
//!
//! > Verification against upstream, not validation against experiment. No
//! > case here is compared with a measured reactor.
//!
//! [`Cstr`]: outram_park_fork_dwsim_libs::reactors::Cstr

use outram_park_fork_dwsim_libs::reactions::{Reaction, ReactionBasis, ReactionComponent, ReactionKind};
use outram_park_fork_dwsim_libs::reactors::{Cstr, ReactorFeed};

/// First-order irreversible `n-butane → isobutane`, `k = 0.01 s⁻¹`, `E = 0`,
/// exactly as the driver defines it.
fn isomerisation() -> Reaction {
    Reaction::new(
        ReactionKind::Kinetic,
        ReactionBasis::MolarConcentration,
        vec![
            ReactionComponent::new(0, -1.0, 1.0, 0.0, true),
            ReactionComponent::new(1, 1.0, 0.0, 0.0, false),
        ],
    )
    .with_forward(0.01, 0.0)
}

/// Solve the isomerisation at upstream's `τ` and return the isobutane mole
/// fraction, which is also the conversion (`Δn = 0`, 1 mol/s feed).
fn isomerisation_conversion(t: f64, p: f64, volume: f64, upstream_tau: f64) -> f64 {
    let q = volume / upstream_tau;
    let out = Cstr::new(vec![isomerisation()], volume)
        .solve(&ReactorFeed::new(vec![1.0, 0.0], t, p, q))
        .expect("isomerisation converges");
    out.molar_flows[1] / out.molar_flows.iter().sum::<f64>()
}

/// Liquid-phase isomerisation against the **pristine** upstream build.
///
/// **Case.** 1 mol/s n-butane, 300 K, 10 bar (single liquid phase), `V =
/// 0.02 m³`. Upstream reports `τ_L = 192.14419923492437 s` and `X =
/// 0.65770328398504` at `Tolerance = 1e-11`.
///
/// **Result (2026-09-25).** This crate: 0.657703283988240, gap **4.9e-12**
/// relative, and equal to `kτ/(1 + kτ)` at upstream's `τ`. At upstream's
/// *default* tolerance the gap is 2.4e-6, all of it upstream stopping short of
/// its own steady state.
#[test]
fn liquid_isomerisation_matches_upstream() {
    const UPSTREAM_TAU: f64 = 192.144_199_234_924_37;
    const UPSTREAM_X: f64 = 0.657_703_283_985_04;

    let x = isomerisation_conversion(300.0, 1.0e6, 0.02, UPSTREAM_TAU);
    let rel = ((x - UPSTREAM_X) / UPSTREAM_X).abs();
    assert!(
        rel < 1e-10,
        "port {x} vs upstream {UPSTREAM_X}: rel {rel:e}"
    );

    let analytic = 0.01 * UPSTREAM_TAU / (1.0 + 0.01 * UPSTREAM_TAU);
    assert!(
        (x - analytic).abs() < 1e-12,
        "port {x} vs closed form {analytic}"
    );
}

/// Vapour-phase isomerisation against the **diagnostic** upstream build.
///
/// **Case.** 1 mol/s n-butane, 400 K, 1 bar (single vapour phase), `V = 20 m³`,
/// `ReactionPhase.Mixture`. The pristine build returns `X = 0` here — defect 1
/// in the module docs — so upstream was run with the one-line `dT` seed patch.
/// Upstream then reports `τ_L = 608.12170689544155 s`, `X =
/// 0.85878133797292333` at `Tolerance = 1e-11`.
///
/// **Result (2026-09-25).** This crate: 0.858781337973070, gap **1.7e-13**
/// relative.
#[test]
fn vapour_isomerisation_matches_upstream_diagnostic_build() {
    const UPSTREAM_TAU: f64 = 608.121_706_895_441_55;
    const UPSTREAM_X: f64 = 0.858_781_337_972_923_33;

    let x = isomerisation_conversion(400.0, 1.0e5, 20.0, UPSTREAM_TAU);
    let rel = ((x - UPSTREAM_X) / UPSTREAM_X).abs();
    assert!(
        rel < 1e-10,
        "port {x} vs upstream {UPSTREAM_X}: rel {rel:e}"
    );
}

/// Steam-methane reforming plus water-gas shift, five species, against the
/// **diagnostic** upstream build.
///
/// **Case.** DOVER's base deck (`crates/dover/decks/smr_cstr.toml`): 1 mol/s
/// CH4 and 3 mol/s H2O, 1123.15 K, 20 bar, `V = 2 m³`, mass-action orders.
/// Forward Arrhenius pairs from the deck; reverse pairs are the values DOVER's
/// `consistent_reverse` produces at 1123.15 K, printed at full precision and
/// typed into both codes, so neither derives them independently.
///
/// **Upstream settings.** Diagnostic build (the pristine one returns the feed
/// composition here — defect 1), `Tolerance = 1e-13`, 28 s wall. Upstream
/// reports `τ_L = 90.014058704599151 s`; its `Q` has grown 19.0 % from the
/// inlet by then.
///
/// **Result (2026-09-25).** Worst per-species relative gap in outlet molar
/// flow: **6.1e-10** (CO); the others 7.4e-11 to 3.6e-10. Methane conversion
/// 0.375803270 in both. The gap shrinks as upstream is converged harder, which
/// is what identifies it as upstream's stopping rule rather than a difference
/// between the codes:
///
/// | upstream `Tolerance` | worst per-species gap | upstream wall |
/// |---|---|---|
/// | 1e-5 (default) | 5.4e-2 in mole fraction — not a steady state, residual 0.138 mol/s | 4.3 s |
/// | 1e-11 | 6.1e-8 | — |
/// | 1e-13 | 6.1e-10 | 28 s |
#[test]
fn steam_reforming_matches_upstream() {
    // Species order: CH4, H2O, CO, CO2, H2.
    fn rxn(nu: [(usize, f64); 4], base: usize, af: f64, ef: f64, ar: f64, er: f64) -> Reaction {
        let components = nu
            .iter()
            .map(|&(i, n)| {
                let (direct, reverse) = if n < 0.0 { (-n, 0.0) } else { (0.0, n) };
                ReactionComponent::new(i, n, direct, reverse, i == base)
            })
            .collect();
        Reaction::new(
            ReactionKind::Kinetic,
            ReactionBasis::MolarConcentration,
            components,
        )
        .with_forward(af, ef)
        .with_reverse(ar, er)
    }
    let reactions = vec![
        rxn(
            [(0, -1.0), (1, -1.0), (2, 1.0), (4, 3.0)],
            0,
            1.0e7,
            2.4e5,
            5.314_382_778_663_273e-7,
            34_100.0,
        ),
        rxn(
            [(2, -1.0), (1, -1.0), (3, 1.0), (4, 1.0)],
            2,
            1.0e2,
            6.7e4,
            15_628.549_082_752_832,
            108_200.0,
        ),
    ];

    const UPSTREAM_TAU: f64 = SMR_UPSTREAM_TAU;
    let volume = 2.0;
    let q = volume / UPSTREAM_TAU;
    let mut reactor = Cstr::new(reactions, volume);
    reactor.max_iter = 20_000;
    let out = reactor
        .solve(&ReactorFeed::new(
            vec![1.0, 3.0, 0.0, 0.0, 0.0],
            1123.15,
            2.0e6,
            q,
        ))
        .expect("steam reforming converges");

    let mut worst = 0.0_f64;
    for (i, (&port, &up)) in out
        .molar_flows
        .iter()
        .zip(SMR_UPSTREAM_FLOWS.iter())
        .enumerate()
    {
        let rel = ((port - up) / up).abs();
        worst = worst.max(rel);
        assert!(
            rel < SMR_GATE,
            "species {i}: port {port} vs upstream {up}, rel {rel:e}"
        );
    }
    assert!(worst < SMR_GATE, "worst per-species gap {worst:e}");
}

// Upstream at `Tolerance = 1e-13` (28 s wall). See the doc comment on the test.
const SMR_UPSTREAM_TAU: f64 = 90.014_058_704_599_151;
const SMR_UPSTREAM_FLOWS: [f64; 5] = [
    0.624_196_729_947_493_31,
    2.438_362_934_441_930_6,
    0.189_969_474_546_943_52,
    0.185_833_795_505_563_12,
    1.313_243_605_663_083_4,
];
/// Gate on the worst per-species relative gap. Measured 6.1e-10; set at 1e-8.
/// The upstream vector is frozen above, so only this crate can move the gap.
const SMR_GATE: f64 = 1e-8;
