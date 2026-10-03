// SPDX-License-Identifier: GPL-3.0-only
//! # V&V — conversion-reactor parity against upstream DWSIM (code-to-code)
//!
//! **Methodology.** Upstream DWSIM, built from the pinned commit
//! `1abf72d1b6b41d3e9a8cc770d3cc4e8fc76e5766` and run headless on Linux (driver
//! `docs/upstream-harness/conversion_driver.cs`, procedure in that folder's
//! README). Each case builds a real `Reactor_Conversion` on a headless
//! flowsheet (inlet, vapour and liquid outlets, energy stream; Peng-Robinson;
//! isothermal), creates its reactions with upstream's own
//! `FlowsheetBase.CreateConversionReaction`, places them in a reaction set at
//! the stated **ranks**, and calls `Calculate()`. Per-species outlet flows are
//! the sum of the two outlet streams after upstream's own stream calculation.
//! The same reactions are then solved by this crate's [`ConversionReactor`].
//!
//! The conversion reactor has no iteration in the feasible case: outlet flows
//! are closed-form in the feed, so agreement is expected to round-off. The gate
//! is **1e-12 relative** per species (absolute where the reference is zero).
//!
//! **Results (measured 2026-10-02, release, upstream at the pinned commit).**
//!
//! | case | what it tests | worst per-species gap |
//! |---|---|---|
//! | `single` — CH4 + 2 O2, base O2 (`ν_BC = −2`), X = 60 % | the `−X·ν/ν_BC·n_BC` update | 3.7e-16 |
//! | `sequential` — total + partial combustion, ranks 0, 1 | group-to-group chaining | 0 (bit-identical) |
//! | `parallel` — same reactions, **both rank 0** | group-inlet `n_BC` | **before the fix: CH4 +75 %, CO −50 %**; after: 0 (bit-identical) |
//! | `overspec` — rank-0 pair, X = 80 % + 60 % of CH4 | infeasible group | 1.8e-3 (upstream's simplex stops short; see test) |
//! | `unchecked` — rank-0 pair, O2 short, O2 only in the first | upstream penalty scope | upstream **defect**: 25 % of the carbon lost |
//! | `liq_two_phase` — liquid-phase reaction, two-phase feed | `ReactionPhase` | **not ported**: +67 % isobutane |
//!
//! **What the comparison found.**
//!
//! 1. **Port defect, fixed in the same change.** The port applied every
//!    reaction in list order, on the flow left by the previous one, and its doc
//!    called that "DWSIM's sequential-group treatment". Upstream groups by rank
//!    and runs equal ranks in *parallel* on the group-inlet flow
//!    (`Conversion.vb:81-117`, `:596-621`). Before the fix the `parallel` case
//!    gave CH4 0.35 / CO 0.15 mol/s against upstream's 0.20 / 0.30. Ranks are
//!    now ported (`ConversionReactor::with_ranks`); `new` keeps the old
//!    list-order behaviour as rank `0, 1, 2, …`, which upstream reproduces
//!    bit-for-bit (`sequential`).
//! 2. **Upstream defect** (`unchecked`). Upstream's negative-flow penalty only
//!    sees the compounds of the **last** reaction in a group: `nif` is reset
//!    from the inlet for every reaction inside the objective
//!    (`Conversion.vb:599`, `:603`, `:607`), so a reactant used only by an
//!    earlier reaction is never checked. With O2 for half the specified total
//!    combustion, upstream keeps X = 100 %, clamps O2 from −1 to 0
//!    (`:715`), and rebuilds the outlet from the conserved *mass* flow, which
//!    rescales every species by 0.746: carbon in 2.0 mol/s, out 1.49 mol/s.
//!    This port constrains every compound and returns X = 50 % / 50 % with the
//!    element balance closed.
//! 3. **Upstream approximation** (`overspec`). For an infeasible group both
//!    codes minimise `Σ (X_spec − X)²` subject to non-negative flows. This port
//!    solves it exactly (0.6 / 0.4); upstream's Nelder-Mead on a penalised
//!    objective returns 0.600712 / 0.399288 — 1.2e-3 to 1.8e-3 off its own
//!    optimum. The port is gated against the exact optimum, and the upstream
//!    gap is recorded, not gated.
//! 4. **Not ported** (`liq_two_phase`). Upstream takes `n_BC` from the
//!    reaction's phase of the flashed inlet (`Conversion.vb:596-621`); the port's
//!    [`ReactorFeed`] has no phase split, so it converts 50 % of all the
//!    n-butane where upstream converts 50 % of the 0.5996 mol/s in the liquid.
//!
//! **Heat of reaction (definition check, not gated).** Upstream's
//! `ReactionHeat` is per mol of **base reactant** — `(H_p − H_r)/|ν_BC|`
//! (`FlowsheetBase.vb:4368`): −401 309 J/mol for the `single` case with O2 as
//! base — and `Conversion.vb:704` multiplies it by `|Δn_BC|`. This crate's
//! `Reaction::reaction_heat` is per mol of **extent**, so a value copied from
//! upstream must be multiplied by `|ν_BC|` first. The doc of that field said
//! both; it is corrected in the same change.
//!
//! > Verification against upstream, not validation against experiment.
//!
//! [`ConversionReactor`]: outram_park_fork_dwsim_libs::reactors::ConversionReactor
//! [`ReactorFeed`]: outram_park_fork_dwsim_libs::reactors::ReactorFeed

use outram_park_fork_dwsim_libs::reactions::{Reaction, ReactionBasis, ReactionComponent, ReactionKind};
use outram_park_fork_dwsim_libs::reactors::{ConversionReactor, ReactorFeed};

/// Gate for the closed-form cases.
const GATE: f64 = 1e-12;

fn conv(components: Vec<ReactionComponent>, x: f64) -> Reaction {
    Reaction::new(
        ReactionKind::Conversion,
        ReactionBasis::MolarConcentration,
        components,
    )
    .with_conversion(x)
}

// Species order for the combustion cases: CH4, O2, CO2, CO, H2O, N2.
fn total(x: f64, base: usize) -> Reaction {
    conv(
        vec![
            ReactionComponent::new(0, -1.0, 0.0, 0.0, base == 0),
            ReactionComponent::new(1, -2.0, 0.0, 0.0, base == 1),
            ReactionComponent::new(2, 1.0, 0.0, 0.0, false),
            ReactionComponent::new(4, 2.0, 0.0, 0.0, false),
        ],
        x,
    )
}

fn partial(x: f64) -> Reaction {
    conv(
        vec![
            ReactionComponent::new(0, -1.0, 0.0, 0.0, true),
            ReactionComponent::new(1, -1.5, 0.0, 0.0, false),
            ReactionComponent::new(3, 1.0, 0.0, 0.0, false),
            ReactionComponent::new(4, 2.0, 0.0, 0.0, false),
        ],
        x,
    )
}

fn worst_gap(port: &[f64], upstream: &[f64]) -> f64 {
    port.iter()
        .zip(upstream)
        .map(|(&p, &u)| {
            if u == 0.0 {
                p.abs()
            } else {
                ((p - u) / u).abs()
            }
        })
        .fold(0.0, f64::max)
}

fn combustion_feed(o2: f64, n2: f64) -> ReactorFeed {
    ReactorFeed::new(vec![1.0, o2, 0.0, 0.0, 0.0, n2], 1000.0, 1.0e5, 0.0)
}

/// `single`: CH4 + 2 O2 → CO2 + 2 H2O with **O2 as base reactant**
/// (`|ν_BC| = 2`), X = 60 %, feed CH4 1 / O2 1.5 / N2 5.64 mol/s, 1000 K, 1 bar.
/// Exercises `Δnᵢ = −X·νᵢ/ν_BC·n_BC` with a non-unit base coefficient.
///
/// **Result (2026-10-02).** Upstream
/// `[0.55, 0.6, 0.45, 0, 0.9, 5.64]` (to its printed round-off); worst gap
/// 3.7e-16.
#[test]
fn single_reaction_matches_upstream() {
    const UPSTREAM: [f64; 6] = [
        0.549_999_999_999_999_93,
        0.599_999_999_999_999_87,
        0.449_999_999_999_999_84,
        0.0,
        0.899_999_999_999_999_69,
        5.639_999_999_999_998_8,
    ];
    let out = ConversionReactor::new(vec![total(0.6, 1)])
        .solve(&combustion_feed(1.5, 5.64))
        .unwrap();
    let gap = worst_gap(&out.molar_flows, &UPSTREAM);
    eprintln!("worst gap {gap:e}");
    assert!(gap < GATE, "worst gap {gap:e}: {:?}", out.molar_flows);
}

/// `sequential`: total combustion (X = 50 % of CH4) at rank 0, then partial
/// combustion to CO (X = 30 % of CH4) at rank 1; feed CH4 1 / O2 4 / N2 15.
/// The second group sees the first group's outlet (0.5 mol/s CH4).
///
/// **Result (2026-10-02).** Upstream `[0.35, 2.775, 0.5, 0.15, 1.3, 15]`;
/// this crate (both `new` and explicit ranks) bit-identical.
#[test]
fn sequential_ranks_match_upstream() {
    const UPSTREAM: [f64; 6] = [0.35, 2.775, 0.5, 0.15, 1.3, 15.0];
    let feed = combustion_feed(4.0, 15.0);
    for reactor in [
        ConversionReactor::new(vec![total(0.5, 0), partial(0.3)]),
        ConversionReactor::new(vec![total(0.5, 0), partial(0.3)]).with_ranks(vec![0, 1]),
    ] {
        let out = reactor.solve(&feed).unwrap();
        let gap = worst_gap(&out.molar_flows, &UPSTREAM);
        eprintln!("worst gap {gap:e}");
        assert!(gap < GATE, "worst gap {gap:e}: {:?}", out.molar_flows);
    }
}

/// `parallel`: the same two reactions, **both at rank 0** — upstream's default
/// when reactions are added to a set without distinct ranks. Both take `n_BC`
/// from the group inlet: CH4 falls by 0.5 + 0.3 = 0.8 mol/s.
///
/// **Result (2026-10-02).** Upstream `[0.2, 2.55, 0.5, 0.3, 1.6, 15]`.
/// Before the fix this crate (list order) gave `[0.35, 2.775, 0.5, 0.15, 1.3,
/// 15]` — CH4 +75 %, CO −50 %. After: bit-identical (gap 0).
#[test]
fn parallel_group_matches_upstream() {
    const UPSTREAM: [f64; 6] = [0.199_999_999_999_999_96, 2.55, 0.5, 0.3, 1.6, 15.0];
    let out = ConversionReactor::new(vec![total(0.5, 0), partial(0.3)])
        .with_ranks(vec![0, 0])
        .solve(&combustion_feed(4.0, 15.0))
        .unwrap();
    let gap = worst_gap(&out.molar_flows, &UPSTREAM);
    eprintln!("worst gap {gap:e}");
    assert!(gap < GATE, "worst gap {gap:e}: {:?}", out.molar_flows);
}

/// `overspec`: a rank-0 pair asking for 80 % + 60 % of the CH4. Both codes
/// minimise `(0.8 − X₁)² + (0.6 − X₂)²` subject to non-negative flows; the
/// exact optimum is on `X₁ + X₂ = 1` at `(0.6, 0.4)`.
///
/// **Result (2026-10-02).** This crate: `(0.6, 0.4)` to 1e-12 — gated.
/// Upstream: `(0.600712177898819, 0.399287822101181)`; outlet CO 0.399288 vs
/// this crate's 0.4, worst per-species gap **1.8e-3**, all of it upstream's
/// Nelder-Mead stopping short (its objective value 0.080001 vs the optimum
/// 0.08). Recorded, not gated: upstream's result is the less accurate one.
#[test]
fn overspecified_group_reaches_exact_optimum() {
    const UPSTREAM_X: [f64; 2] = [0.600_712_177_898_819, 0.399_287_822_101_181_08];
    let reactor = ConversionReactor::new(vec![total(0.8, 0), partial(0.6)]).with_ranks(vec![0, 0]);
    let out = reactor.solve(&combustion_feed(4.0, 15.0)).unwrap();
    // extent per unit conversion is n_CH4 = 1 for both reactions here.
    let x = [out.extents[0], out.extents[1]];
    assert!(
        (x[0] - 0.6).abs() < GATE && (x[1] - 0.4).abs() < GATE,
        "{x:?}"
    );
    assert!(
        out.molar_flows[0].abs() < GATE,
        "CH4 {}",
        out.molar_flows[0]
    );

    let upstream_gap = (x[1] - UPSTREAM_X[1]).abs() / UPSTREAM_X[1];
    assert!(
        (1e-3..2e-3).contains(&upstream_gap),
        "recorded upstream gap moved: {upstream_gap:e}"
    );
}

/// `unchecked`: a rank-0 pair — total combustion of CH4 at X = 100 % with O2
/// for only half of it (O2 1 mol/s), and CO shift (`CO + H2O → CO2 + H2`,
/// X = 50 % of CO). Species CH4, O2, CO2, CO, H2O, H2; feed `[1, 1, 0, 1, 1, 0]`.
///
/// **Result (2026-10-02).** Upstream keeps `X = (1.0, 0.5)` and returns
/// `[0, 0, 1.119260, 0.373087, 1.865433, 0.373087]` — carbon in 2.0 mol/s, out
/// 1.4923 mol/s, because O2 (only in the *first* reaction) is outside its
/// penalty (`Conversion.vb:599-635`), goes to −1, is clamped, and the outlet is
/// rebuilt from the conserved mass flow. **Upstream defect, filed.** This
/// crate returns `X = (0.5, 0.5)`, flows `[0.5, 0, 1.0, 0.5, 1.5, 0.5]`, with
/// C, H and O balances closed to 1e-12.
#[test]
fn penalty_scope_upstream_defect_is_not_reproduced() {
    // CH4 + 2 O2 -> CO2 + 2 H2O (base CH4), and CO + H2O -> CO2 + H2 (base CO).
    let combustion = conv(
        vec![
            ReactionComponent::new(0, -1.0, 0.0, 0.0, true),
            ReactionComponent::new(1, -2.0, 0.0, 0.0, false),
            ReactionComponent::new(2, 1.0, 0.0, 0.0, false),
            ReactionComponent::new(4, 2.0, 0.0, 0.0, false),
        ],
        1.0,
    );
    let shift = conv(
        vec![
            ReactionComponent::new(3, -1.0, 0.0, 0.0, true),
            ReactionComponent::new(4, -1.0, 0.0, 0.0, false),
            ReactionComponent::new(2, 1.0, 0.0, 0.0, false),
            ReactionComponent::new(5, 1.0, 0.0, 0.0, false),
        ],
        0.5,
    );
    let feed = ReactorFeed::new(vec![1.0, 1.0, 0.0, 1.0, 1.0, 0.0], 1000.0, 1.0e5, 0.0);
    let out = ConversionReactor::new(vec![combustion, shift])
        .with_ranks(vec![0, 0])
        .solve(&feed)
        .unwrap();
    let expected = [0.5, 0.0, 1.0, 0.5, 1.5, 0.5];
    let gap = worst_gap(&out.molar_flows, &expected);
    eprintln!("worst gap {gap:e}");
    assert!(gap < GATE, "worst gap {gap:e}: {:?}", out.molar_flows);

    // Element balances: C = CH4 + CO2 + CO; H = 4 CH4 + 2 H2O + 2 H2;
    // O = 2 O2 + 2 CO2 + CO + H2O.
    let atoms = |f: &[f64]| {
        [
            f[0] + f[2] + f[3],
            4.0 * f[0] + 2.0 * f[4] + 2.0 * f[5],
            2.0 * f[1] + 2.0 * f[2] + f[3] + f[4],
        ]
    };
    let (a_in, a_out) = (atoms(&feed.molar_flows), atoms(&out.molar_flows));
    for k in 0..3 {
        assert!(
            (a_in[k] - a_out[k]).abs() < GATE,
            "element {k}: {a_in:?} vs {a_out:?}"
        );
    }

    // The upstream outlet, frozen, loses a quarter of its carbon.
    const UPSTREAM: [f64; 6] = [
        0.0,
        0.0,
        1.119_259_648_005_035_9,
        0.373_086_549_335_012,
        1.865_432_746_675_06,
        0.373_086_549_335_012,
    ];
    let carbon_out = atoms(&UPSTREAM)[0];
    assert!((carbon_out - 1.492_346).abs() < 1e-6, "{carbon_out}");
}

/// `liq_two_phase`: n-butane → isobutane, `ReactionPhase.Liquid`, X = 50 %, in
/// a methane / n-butane feed (1 / 1 mol/s) at 300 K, 10 bar, which flashes to
/// vapour fraction 0.6875 with 0.599611 mol/s of the n-butane in the liquid.
///
/// **Result (2026-10-02) — a recorded gap, not parity.** Upstream converts
/// 50 % of the *liquid* n-butane: isobutane out 0.299805 mol/s. This crate has
/// no phase split in [`ReactorFeed`] and converts 50 % of all of it: 0.5, i.e.
/// **+67 %**. Upstream's outlet here also carries a 1.4e-5 relative methane
/// imbalance (1.0000142 out for 1 in) from rebuilding the outlets from flash
/// mass fractions. This test pins the gap so that porting `ReactionPhase` has
/// to update it.
///
/// [`ReactorFeed`]: outram_park_fork_dwsim_libs::reactors::ReactorFeed
#[test]
fn liquid_phase_reaction_gap_is_recorded() {
    const UPSTREAM_ISOBUTANE: f64 = 0.299_804_748_582_196_86;
    let iso = conv(
        vec![
            ReactionComponent::new(1, -1.0, 0.0, 0.0, true),
            ReactionComponent::new(2, 1.0, 0.0, 0.0, false),
        ],
        0.5,
    );
    let out = ConversionReactor::new(vec![iso])
        .solve(&ReactorFeed::new(vec![1.0, 1.0, 0.0], 300.0, 1.0e6, 0.0))
        .unwrap();
    assert!((out.molar_flows[2] - 0.5).abs() < GATE);
    let excess = out.molar_flows[2] / UPSTREAM_ISOBUTANE - 1.0;
    assert!(
        (excess - 0.6678).abs() < 1e-3,
        "recorded gap moved: {excess}"
    );
}
