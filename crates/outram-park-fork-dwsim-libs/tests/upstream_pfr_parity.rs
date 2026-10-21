// SPDX-License-Identifier: GPL-3.0-only
//! # V&V — PFR parity against upstream DWSIM (code-to-code)
//!
//! **Methodology.** Upstream DWSIM, built from the pinned commit
//! `1abf72d1b6b41d3e9a8cc770d3cc4e8fc76e5766` and run headless on Linux (driver
//! `docs/upstream-harness/pfr_driver.cs`). Each case builds a real `Reactor_PFR`
//! on a headless flowsheet (inlet, outlet and energy streams; Peng-Robinson;
//! isothermal; pressure drop pinned to zero, since this port has none), creates
//! its reactions with upstream's `CreateKineticReaction` /
//! `CreateHetCatReaction`, and calls `Calculate()` at upstream's defaults
//! (`dV = 0.01`, `InternalSolver = 0`, implicit RK5). The driver's full output,
//! including upstream's per-segment profile, is frozen under
//! `tests/fixtures/upstream_pfr/` and read by this test.
//!
//! **How the two are made comparable.** Upstream marches the volume in `1/dV`
//! segments, holds the volumetric flow `Q` at each segment's start value
//! (`PFR.vb:955-972`), integrates the segment, then re-flashes. This crate
//! holds `Q` at whatever it is handed. To compare the *integrators*, this crate
//! is run segment by segment on upstream's own per-segment `Q` (from the
//! profile, `Q = F_i/C_i`). What holding the inlet `Q` costs is measured
//! separately. Gate: **1e-8** worst per-species relative gap, as for the CSTR.
//!
//! **Results (measured 2026-10-02, release).**
//!
//! | case | port on upstream's segment `Q` | port at inlet `Q` (normal use) |
//! |---|---|---|
//! | `iso_liq` — liquid n-C4 → i-C4, k = 0.01 s⁻¹, V = 0.02 m³ | **1.9e-2 — upstream defect, below**; 1.2e-10 on the volume upstream actually integrates | 5.9e-2 |
//! | `iso_gas` — vapour, 400 K, 1 bar, V = 20 m³ | 1.4e-10 | 5.1e-3 |
//! | `smr` — steam reforming + shift, V = 2 m³ (CSTR deck) | 2.6e-9 | 6.0e-2 (CO; CH4 −3.9 %) |
//! | `hetcat_gas` — LH catalytic, 500 kg/m³ bed, ε = 0.4 | 2.0e-10 (**pre-fix: n-butane out +96.3 %**, coverage R3) | — |
//!
//! **Upstream defect: the PFR integrates only 99 % of each segment for some
//! volumes.** Each segment is solved by DotNumerics with
//! `Solve(y0, 0, 0.01·ΔV, ΔV, callback)` (`PFR.vb:1119`), and upstream keeps the
//! value of the *last* callback. DotNumerics sizes its output grid as
//! `(int)(|tf − t0| / |deltaT|) + 1` (`DWSIM.Math.DotNumerics/ODE/RKSolOut.cs:215`).
//! For `V = 0.02 m³` at `dV = 0.01`, `ΔV/(0.01·ΔV)` evaluates to
//! `99.99999999999999`, truncates to 99, and the last output — the segment
//! result — is at `0.99·ΔV`. Every segment loses 1 % of its volume: upstream's
//! n-butane outflow is 0.148058 where the full volume gives 0.145228 (+1.9 %).
//! **Proved, not inferred:** run on exactly `0.99·ΔV` per segment this crate
//! reproduces upstream to 1.2e-10, and upstream at `dV = 0.001` (ratio exactly
//! 100) gives 0.145302, moving 1.9 % toward the full-volume answer. For
//! `V = 20` and `V = 2` the ratio is exactly 100 and nothing is lost — the
//! error depends on the floating-point rounding of the user's volume.
//!
//! **Port defect found and fixed: heterogeneous catalytic reactions (coverage
//! R3).** The port integrated every reaction kind with the power-law `net_rate`
//! over the full volume. Upstream evaluates `numerator/denominator`, scales it
//! by `CatalystLoading/CatalystVoidFraction` (`PFR.vb:467`) and integrates over
//! the void volume (`PFR.vb:1069-1070`). The PFR now does the same
//! (`Pfr::with_catalyst_bed`); so does the CSTR (`Cstr::with_catalyst_amount`,
//! `tests/upstream_cstr_parity.rs`).
//!
//! **Upstream's ODE tolerance is not the limit.** Upstream never sets the
//! DotNumerics tolerances, so they stay at `RelTol = 1e-3`, `AbsTol = 1e-6`
//! (`xOdeBase.cs:96,105`). Its implicit RK5 still matches this crate's RK4 to
//! 1e-10 on these smooth problems; solvers 0 and 1 agree to 1e-11 at fixed `dV`.
//!
//! > Verification against upstream, not validation against experiment.

use outram_park_fork_dwsim_libs::reactions::{
    AdsorptionTerm, LangmuirHinshelwood, Reaction, ReactionBasis, ReactionComponent, ReactionKind,
};
use outram_park_fork_dwsim_libs::reactors::{Pfr, ReactorFeed};

/// Worst per-species gap gate for the integrator comparison.
const GATE: f64 = 1e-8;

/// One upstream run, parsed from its `KEY=value` fixture.
struct Upstream {
    v: f64,
    q_in: f64,
    f_in: Vec<f64>,
    f_out: Vec<f64>,
    /// `(position along unit length, Q there [m³/s], flows [mol/s])`.
    profile: Vec<(f64, f64, Vec<f64>)>,
}

fn load(name: &str) -> Upstream {
    let path = format!(
        "{}/tests/fixtures/upstream_pfr/{name}.txt",
        env!("CARGO_MANIFEST_DIR")
    );
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let num = |s: &str| s.parse::<f64>().unwrap();
    let mut u = Upstream {
        v: 0.0,
        q_in: 0.0,
        f_in: vec![],
        f_out: vec![],
        profile: vec![],
    };
    for line in text.lines() {
        let Some((key, val)) = line.split_once('=') else {
            continue;
        };
        if key == "V" {
            u.v = num(val);
        } else if key == "Q_IN" {
            u.q_in = num(val);
        } else if key.starts_with("F_IN[") {
            u.f_in.push(num(val));
        } else if key.starts_with("F_OUT[") {
            u.f_out.push(num(val));
        } else if key.starts_with("PROFILE[") {
            let mut parts = val.split(';');
            let pos = num(parts.next().unwrap());
            let q = num(parts.next().unwrap());
            let flows = parts.next().unwrap().split(',').map(num).collect();
            u.profile.push((pos, q, flows));
        }
    }
    assert!(u.profile.len() > 100, "{name}: profile not parsed");
    u
}

fn isomerisation(kind: ReactionKind, a: f64) -> Reaction {
    Reaction::new(
        kind,
        ReactionBasis::MolarConcentration,
        vec![
            ReactionComponent::new(0, -1.0, 1.0, 0.0, true),
            ReactionComponent::new(1, 1.0, 0.0, 0.0, false),
        ],
    )
    .with_forward(a, 0.0)
}

/// Steam reforming + shift, exactly the CSTR parity deck. Species CH4, H2O, CO,
/// CO2, H2.
fn smr() -> Vec<Reaction> {
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
    vec![
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
    ]
}

/// Run this crate segment by segment on upstream's per-segment `Q`, each
/// segment over `fraction` of its volume, with an optional catalyst bed.
fn on_upstream_q(
    u: &Upstream,
    reactions: &[Reaction],
    t: f64,
    p: f64,
    fraction: f64,
    bed: (f64, f64),
) -> Vec<f64> {
    let mut flows = u.f_in.clone();
    for w in u.profile.windows(2) {
        let dv = (w[1].0 - w[0].0) * u.v * fraction;
        if dv <= 0.0 {
            continue;
        }
        flows = Pfr::new(reactions.to_vec(), dv, 50)
            .with_catalyst_bed(bed.0, bed.1)
            .solve(&ReactorFeed::new(flows, t, p, w[0].1))
            .unwrap()
            .molar_flows;
    }
    flows
}

/// This crate as a caller would run it: one PFR at the inlet `Q`.
fn at_inlet_q(u: &Upstream, reactions: &[Reaction], t: f64, p: f64) -> Vec<f64> {
    Pfr::new(reactions.to_vec(), u.v, 2000)
        .solve(&ReactorFeed::new(u.f_in.clone(), t, p, u.q_in))
        .unwrap()
        .molar_flows
}

fn worst_gap(port: &[f64], upstream: &[f64]) -> f64 {
    port.iter()
        .zip(upstream)
        .map(|(p, u)| ((p - u) / u).abs())
        .fold(0.0, f64::max)
}

/// `iso_liq`: 1 mol/s liquid n-butane, 300 K, 10 bar, first order
/// `k = 0.01 s⁻¹`, `V = 0.02 m³` (the CSTR case's kinetics). Upstream's `Q`
/// grows 3.4 % (isomer densities differ).
///
/// **Result (2026-10-02).** Upstream n-butane out 0.148057647221. This crate
/// on upstream's segment `Q`: 0.145228335 over the full volume (gap 1.9e-2),
/// 0.148057647222 over `0.99·ΔV` per segment (gap **1.2e-10**) — the volume
/// upstream actually integrates (module docs). Gated on the latter; the
/// full-volume gap is asserted to stay at the measured size so a change
/// upstream or here is noticed. At the inlet `Q`: 5.9e-2.
#[test]
fn liquid_isomerisation_reproduces_upstream_truncated_segments() {
    let u = load("iso_liq");
    let rx = [isomerisation(ReactionKind::Kinetic, 0.01)];
    let truncated = on_upstream_q(&u, &rx, 300.0, 1.0e6, 0.99, (0.0, 0.0));
    let full = on_upstream_q(&u, &rx, 300.0, 1.0e6, 1.0, (0.0, 0.0));
    let (g_trunc, g_full) = (worst_gap(&truncated, &u.f_out), worst_gap(&full, &u.f_out));
    let g_inlet = worst_gap(&at_inlet_q(&u, &rx, 300.0, 1.0e6), &u.f_out);
    eprintln!("iso_liq: 0.99 dV {g_trunc:e}, full dV {g_full:e}, inlet Q {g_inlet:e}");
    assert!(g_trunc < GATE, "0.99·ΔV gap {g_trunc:e}");
    assert!(
        (1.8e-2..2.0e-2).contains(&g_full),
        "full-ΔV gap moved: {g_full:e}"
    );
}

/// `iso_gas`: the same reaction in the vapour, 400 K, 1 bar, `V = 20 m³`
/// (no truncation at this volume). Upstream n-butane out 0.00228500223.
///
/// **Result (2026-10-02).** On upstream's segment `Q`: gap **1.4e-10**. At the
/// inlet `Q`: 5.1e-3 (upstream's `Q` grows 0.1 %, amplified by `kτ ≈ 6`).
#[test]
fn vapour_isomerisation_matches_upstream() {
    let u = load("iso_gas");
    let rx = [isomerisation(ReactionKind::Kinetic, 0.01)];
    let port = on_upstream_q(&u, &rx, 400.0, 1.0e5, 1.0, (0.0, 0.0));
    let gap = worst_gap(&port, &u.f_out);
    let g_inlet = worst_gap(&at_inlet_q(&u, &rx, 400.0, 1.0e5), &u.f_out);
    eprintln!("iso_gas: segment Q {gap:e}, inlet Q {g_inlet:e}");
    assert!(gap < GATE, "gap {gap:e}");
}

/// `smr`: steam reforming + water-gas shift (the CSTR deck), 1 / 3 mol/s
/// CH4 / H2O, 1123.15 K, 20 bar, `V = 2 m³`. Upstream's `Q` grows 24.7 %
/// (0.018678 → 0.023296 m³/s); CH4 out 0.509632.
///
/// **Result (2026-10-02).** On upstream's segment `Q`: worst per-species gap
/// **2.6e-9**. At the inlet `Q`: 6.0e-2 (CO 0.30143 vs 0.28435; CH4 0.48994 vs
/// 0.50963, −3.9 %) — the measured cost of the constant-`Q` simplification.
#[test]
fn steam_reforming_matches_upstream() {
    let u = load("smr");
    let rx = smr();
    let port = on_upstream_q(&u, &rx, 1123.15, 2.0e6, 1.0, (0.0, 0.0));
    let gap = worst_gap(&port, &u.f_out);
    let g_inlet = worst_gap(&at_inlet_q(&u, &rx, 1123.15, 2.0e6), &u.f_out);
    eprintln!("smr: segment Q {gap:e}, inlet Q {g_inlet:e}");
    assert!(gap < GATE, "gap {gap:e}");
    assert!(
        (5.5e-2..6.5e-2).contains(&g_inlet),
        "constant-Q cost moved: {g_inlet:e}"
    );
}

/// `hetcat_gas`: n-butane → isobutane on a packed bed, vapour, 400 K, 1 bar,
/// `V = 20 m³`, loading 500 kg/m³, `ε = 0.4`,
/// `rate = 1e-5·C_A / (1 + 0.05·C_A)²` mol/(kg·s) (upstream expressions
/// `0.00001*R1`, `(1+0.05*R1)^2`). Upstream n-butane out
/// 0.506419425.
///
/// **Result (2026-10-02).** With the catalyst bed ported: gap **2.0e-10**. The
/// pre-fix behaviour (power-law over the full volume, no denominator, no
/// loading), reproduced here by the equivalent `Kinetic` reaction, leaves
/// n-butane at 0.99393 — **+96.3 %**, i.e. almost no reaction.
#[test]
fn heterogeneous_catalytic_bed_matches_upstream() {
    let u = load("hetcat_gas");
    let rx = [isomerisation(ReactionKind::HeterogeneousCatalytic, 1.0e-5)
        .with_langmuir_hinshelwood(LangmuirHinshelwood::new(
            vec![AdsorptionTerm::new(0, 0.05, 0.0, 1.0)],
            2.0,
        ))];
    let port = on_upstream_q(&u, &rx, 400.0, 1.0e5, 1.0, (500.0, 0.4));
    let gap = worst_gap(&port, &u.f_out);
    let old = on_upstream_q(
        &u,
        &[isomerisation(ReactionKind::Kinetic, 1.0e-5)],
        400.0,
        1.0e5,
        1.0,
        (0.0, 0.0),
    );
    eprintln!(
        "hetcat: gap {gap:e}; pre-fix n-butane {} ({:+.1} %)",
        old[0],
        100.0 * (old[0] / u.f_out[0] - 1.0)
    );
    assert!(gap < GATE, "gap {gap:e}");

    // Without a bed a catalytic reaction is refused, not silently mis-integrated.
    assert!(Pfr::new(rx.to_vec(), 20.0, 10)
        .solve(&ReactorFeed::new(u.f_in.clone(), 400.0, 1.0e5, u.q_in))
        .is_err());
}
