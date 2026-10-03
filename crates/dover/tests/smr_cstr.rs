//! Verification for DOVER's steam-methane-reforming CSTR.
//!
//! **These are verification tests, not validation.** They check that the model
//! is *implemented consistently with its own stated physics* — atom balances,
//! thermodynamic consistency of the rate law, the equilibrium limit, and the
//! qualitative direction of every operating-variable response. Not one of them
//! compares against an experiment or a published reformer, so nothing here
//! licenses calling the model validated. See `src/smr.rs` for the scope.

use dover::deck::{Deck, DeckError};
use dover::headless;
use dover::smr::{ForwardRate, SmrCase, SmrReaction, R_GAS};
use dover::species::Species;

/// The deck's operating point, as a case.
fn base_case() -> SmrCase {
    SmrCase {
        methane_feed: 1.0,
        steam_to_carbon: 3.0,
        temperature: 1123.15,
        pressure: 2.0e6,
        volume: 2.0,
        forward: [
            ForwardRate { a: 1.0e7, e: 2.4e5 },
            ForwardRate { a: 1.0e2, e: 6.7e4 },
        ],
    }
}

// ---------------------------------------------------------------------------
// Thermochemistry — derived, so checkable against any textbook
// ---------------------------------------------------------------------------

/// **Methodology.** `ΔH°` and `ΔS°` are computed in `smr.rs` as `Σ νᵢ ΔH°f,ᵢ`
/// and `Σ νᵢ S°ᵢ` from the five-species table, never tabulated directly.
/// Compare them against the values every thermochemistry text quotes.
///
/// **Results (measured 2026-09-25).** Reforming `ΔH° = +205.9 kJ/mol`
/// (textbook +206), `ΔS° = +214.7 J/(mol·K)`. Shift `ΔH° = −41.2 kJ/mol`
/// (textbook −41.2), `ΔS° = −42.0 J/(mol·K)`. Both enthalpies agree to better
/// than 0.5 kJ/mol, which is the rounding in the formation data itself.
#[test]
fn reaction_enthalpies_match_the_textbook_values() {
    let dh_ref = SmrReaction::Reforming.delta_h();
    let dh_wgs = SmrReaction::WaterGasShift.delta_h();

    assert!(
        (dh_ref - 206_000.0).abs() < 500.0,
        "reforming ΔH° = {dh_ref} J/mol, expected about +206 kJ/mol"
    );
    assert!(
        (dh_wgs - -41_200.0).abs() < 500.0,
        "shift ΔH° = {dh_wgs} J/mol, expected about −41.2 kJ/mol"
    );
    // Signs are the whole character of the pair.
    assert!(dh_ref > 0.0, "reforming must be endothermic");
    assert!(dh_wgs < 0.0, "the shift must be exothermic");
    // Reforming raises the mole count; the shift does not.
    assert!(SmrReaction::Reforming.delta_s() > 0.0);
    assert!(SmrReaction::WaterGasShift.delta_s() < 0.0);
}

/// Why the third, "overall" reaction is deliberately absent.
///
/// `CH4 + 2H2O <=> CO2 + 4H2` is R1 + R2 exactly, so including it as a third
/// reaction would add a redundant extent and make the Newton Jacobian
/// singular. This proves the redundancy rather than asserting it.
#[test]
fn the_overall_reaction_is_the_sum_of_the_two() {
    let sum_h = SmrReaction::Reforming.delta_h() + SmrReaction::WaterGasShift.delta_h();
    let sum_s = SmrReaction::Reforming.delta_s() + SmrReaction::WaterGasShift.delta_s();

    // The overall reaction, computed independently from the species table.
    let overall_h = Species::CarbonDioxide.enthalpy_formation()
        + 4.0 * Species::Hydrogen.enthalpy_formation()
        - Species::Methane.enthalpy_formation()
        - 2.0 * Species::Steam.enthalpy_formation();
    let overall_s = Species::CarbonDioxide.entropy() + 4.0 * Species::Hydrogen.entropy()
        - Species::Methane.entropy()
        - 2.0 * Species::Steam.entropy();

    assert!((sum_h - overall_h).abs() < 1e-6, "{sum_h} vs {overall_h}");
    assert!((sum_s - overall_s).abs() < 1e-9, "{sum_s} vs {overall_s}");
    // And it is endothermic overall, less so than bare reforming.
    assert!(overall_h > 0.0 && overall_h < SmrReaction::Reforming.delta_h());
}

/// `Δn` drives the whole `Kp` vs `Kc` distinction.
#[test]
fn mole_count_change_is_two_for_reforming_and_zero_for_the_shift() {
    assert!((SmrReaction::Reforming.delta_n() - 2.0).abs() < 1e-12);
    assert!(SmrReaction::WaterGasShift.delta_n().abs() < 1e-12);

    // Where Δn = 0 the two constants must coincide exactly.
    for t in [800.0, 1123.15, 1300.0] {
        let kp = SmrReaction::WaterGasShift.equilibrium_constant(t);
        let kc = SmrReaction::WaterGasShift.kc(t);
        assert!(
            (kp - kc).abs() / kp < 1e-12,
            "shift Kp {kp} and Kc {kc} must agree at Δn = 0"
        );
    }
    // And where Δn = 2 they must not, by the documented factor.
    let t = 1123.15;
    let ratio = SmrReaction::Reforming.equilibrium_constant(t) / SmrReaction::Reforming.kc(t);
    let expected = (R_GAS * t / 100_000.0).powi(2);
    assert!(
        (ratio - expected).abs() / expected < 1e-12,
        "Kp/Kc should be (RT/P°)^2 = {expected}, got {ratio}"
    );
}

/// The consistency guarantee `consistent_reverse` exists to provide.
///
/// **Methodology.** Rebuild the reaction set at each of a range of
/// temperatures, exactly as a solve does, and check the rate law's own
/// equilibrium ratio `k_f(T)/k_r(T)` equals `Kc(T)` — the thermodynamics the
/// species table implies.
///
/// **Results (measured 2026-09-25).** Agreement to better than 1e-12 relative
/// at every temperature from 700 K to 1400 K, for both reactions.
#[test]
fn reverse_rate_is_thermodynamically_consistent() {
    let fwd = base_case().forward;
    let mut worst = 0.0_f64;

    for i in 0..=14 {
        let t = 700.0 + 50.0 * i as f64;
        let reactions = dover::smr::smr_reactions(fwd, t);
        for (r, rxn) in SmrReaction::BOTH.iter().enumerate() {
            let kf = reactions[r].forward_rate_constant(t);
            let kr = reactions[r].reverse_rate_constant(t);
            assert!(kr > 0.0, "{rxn:?} at {t} K has a non-positive k_r");
            let rel = ((kf / kr) / rxn.kc(t) - 1.0).abs();
            worst = worst.max(rel);
            assert!(
                rel < 1e-12,
                "{rxn:?} at {t} K: k_f/k_r = {} but Kc = {} (rel {rel})",
                kf / kr,
                rxn.kc(t)
            );
        }
    }
    assert!(worst < 1e-12, "worst relative inconsistency {worst}");
}

// ---------------------------------------------------------------------------
// The reactor
// ---------------------------------------------------------------------------

/// Atoms are conserved. The strongest invariant available without a reference.
///
/// C, H and O must each balance between inlet and outlet to solver precision,
/// whatever the reaction extents come out as.
#[test]
fn every_atom_balances() {
    for t in [800.0, 1000.0, 1123.15, 1300.0] {
        let mut case = base_case();
        case.temperature = t;
        let out = case.solve().expect("solve");

        let inlet = case.inlet_flows();
        // (C, H, O) per molecule, in Species::INDEX_ORDER.
        let atoms = [
            (1.0, 4.0, 0.0), // CH4
            (0.0, 2.0, 1.0), // H2O
            (1.0, 0.0, 1.0), // CO
            (1.0, 0.0, 2.0), // CO2
            (0.0, 2.0, 0.0), // H2
        ];
        for (k, name) in [(0, "C"), (1, "H"), (2, "O")] {
            let f_in: f64 = inlet
                .iter()
                .zip(atoms)
                .map(|(&f, a)| f * [a.0, a.1, a.2][k])
                .sum();
            let f_out: f64 = out
                .outlet_flows
                .iter()
                .zip(atoms)
                .map(|(&f, a)| f * [a.0, a.1, a.2][k])
                .sum();
            assert!(
                (f_in - f_out).abs() < 1e-9 * f_in.max(1.0),
                "{name} balance at {t} K: in {f_in}, out {f_out}"
            );
        }
    }
}

/// The uncalibrated model's defining property.
///
/// **Methodology.** The forward rate sets only *how fast* equilibrium is
/// approached; the equilibrium itself is pure thermodynamics. So growing the
/// reactor volume must drive the outlet to the composition satisfying
/// `Π Cᵢ^νᵢ = Kc(T)` for both reactions, regardless of the fitted `A_f`/`E_f`.
/// Solve at volumes spanning five decades and measure the residual
/// `|ΠCᵢ^νᵢ / Kc − 1|`.
///
/// **Results (measured 2026-09-25).** At V = 2 m³ the reactor is far from
/// equilibrium (methane conversion 0.412). By V = 2e5 m³ both reactions'
/// residuals are below 1e-3, and the approach is monotone throughout. The
/// model's equilibrium limit is therefore the one its thermodynamics
/// specifies, not one its kinetics happen to land on.
#[test]
fn long_residence_time_approaches_thermodynamic_equilibrium() {
    let mut previous = f64::INFINITY;

    for exp in [0, 1, 2, 3, 4, 5] {
        let mut case = base_case();
        case.volume = 2.0 * 10f64.powi(exp);
        let out = case.solve().expect("solve");

        let q = case.volumetric_flow();
        let conc: Vec<f64> = out.outlet_flows.iter().map(|&f| f.max(0.0) / q).collect();

        let mut worst = 0.0_f64;
        for rxn in SmrReaction::BOTH {
            let quotient: f64 = rxn
                .stoichiometry()
                .iter()
                .map(|&(sp, nu)| conc[sp.index()].powf(nu))
                .product();
            let rel = (quotient / rxn.kc(case.temperature) - 1.0).abs();
            worst = worst.max(rel);
        }

        assert!(
            worst <= previous * 1.001,
            "approach to equilibrium should be monotone: V = {} m³ gave {worst}, \
             previous {previous}",
            case.volume
        );
        previous = worst;
    }

    assert!(
        previous < 1e-3,
        "the largest reactor should sit essentially at equilibrium, residual {previous}"
    );
}

/// Reforming is endothermic, so conversion must rise with temperature.
///
/// **Results (measured 2026-09-25).** Methane conversion rises monotonically
/// from 800 K to 1300 K across all 11 sweep points.
#[test]
fn conversion_rises_with_temperature() {
    let mut last = -1.0;
    for i in 0..=10 {
        let mut case = base_case();
        case.temperature = 800.0 + 50.0 * i as f64;
        let x = case.solve().expect("solve").methane_conversion;
        assert!(
            x > last,
            "conversion fell at {} K: {x} after {last}",
            case.temperature
        );
        last = x;
    }
}

/// Le Chatelier, tested where Le Chatelier applies.
///
/// **Methodology, and a corrected premise.** Reforming increases the mole
/// count (`Δn = +2`), so raising the pressure must suppress conversion — *at
/// equilibrium*. The first version of this test asserted that trend against
/// the base 2 m³ reactor and failed, and the failure was the test's, not the
/// model's: at 2 m³ the reactor is far from equilibrium (`Q/Kc = 0.26` for
/// reforming), so the dominant effect of pressure is kinetic — higher `P`
/// raises every concentration and therefore the forward rate. The
/// thermodynamic trend is measured here at `V = 2e5 m³`, where the outlet sits
/// essentially at equilibrium; the kinetic-regime behaviour is a separate
/// documented finding in
/// `pressure_raises_conversion_in_the_kinetically_limited_regime`.
///
/// **Results (measured 2026-09-25, V = 2e5 m³).** Conversion falls
/// monotonically with pressure: 0.805223 at 5 bar, 0.667196 at 10 bar,
/// 0.529272 at 20 bar, 0.456137 at 30 bar, 0.408776 at 40 bar.
#[test]
fn conversion_falls_with_pressure_at_equilibrium() {
    let mut last = f64::INFINITY;
    for p in [5.0e5, 1.0e6, 2.0e6, 3.0e6, 4.0e6] {
        let mut case = base_case();
        case.pressure = p;
        case.volume = 2.0e5;
        let x = case.solve().expect("solve").methane_conversion;
        assert!(x < last, "conversion rose at {p} Pa: {x} after {last}");
        last = x;
    }
}

/// The kinetic regime runs the *other* way, and that is not a defect.
///
/// **Methodology.** Repeat the pressure sweep at the deck's own 2 m³ volume,
/// where the reactor is rate-limited rather than equilibrium-limited, and
/// confirm the reforming reaction quotient stays well below `Kc` throughout —
/// which is what makes the regime kinetic rather than thermodynamic.
///
/// **Results (measured 2026-09-25, V = 2 m³).** Conversion *rises* from
/// 0.066283 at 5 bar to 0.207439 at 10 bar and 0.411745 at 20 bar, peaks near
/// 0.421096 at 30 bar, and turns over to 0.394227 at 40 bar as the growing
/// reverse rate begins to bite. Reforming's `Q/Kc` is 0.26 at the base point,
/// i.e. a factor of four from equilibrium.
///
/// This is recorded rather than smoothed over because the non-monotonicity is
/// the honest output of the stated physics, and a reader comparing against a
/// textbook Le Chatelier statement needs to know which regime they are in.
#[test]
fn pressure_raises_conversion_in_the_kinetically_limited_regime() {
    let low = {
        let mut c = base_case();
        c.pressure = 5.0e5;
        c.solve().expect("solve").methane_conversion
    };
    let high = {
        let mut c = base_case();
        c.pressure = 2.0e6;
        c.solve().expect("solve").methane_conversion
    };
    assert!(
        high > low,
        "in the kinetic regime higher pressure must raise conversion: \
         {high} at 20 bar vs {low} at 5 bar"
    );

    // And confirm the regime really is kinetic, so the test above is not
    // silently measuring something else.
    let case = base_case();
    let out = case.solve().expect("solve");
    let q = case.volumetric_flow();
    let conc: Vec<f64> = out.outlet_flows.iter().map(|&f| f.max(0.0) / q).collect();
    let quotient: f64 = SmrReaction::Reforming
        .stoichiometry()
        .iter()
        .map(|&(sp, nu)| conc[sp.index()].powf(nu))
        .product();
    let ratio = quotient / SmrReaction::Reforming.kc(case.temperature);
    assert!(
        ratio < 0.5,
        "the base case must be kinetically limited for this test to mean \
         anything; reforming Q/Kc = {ratio}"
    );
}

/// Excess steam drives the equilibrium toward products — again, at equilibrium.
///
/// **Methodology, and the same corrected premise as the pressure pair.** More
/// steam raises conversion thermodynamically, but it also *dilutes* the
/// methane, and at a finite rate the dilution eventually wins. Measured at
/// `V = 2e5 m³`, where the equilibrium trend is what is on show.
///
/// **Results (measured 2026-09-25, V = 2e5 m³).** Monotone: 0.294611 at
/// S/C = 1, 0.425521 at 2, 0.529272 at 3, 0.614871 at 4, 0.686103 at 5,
/// 0.745281 at 6. At the deck's 2 m³ the same sweep instead peaks at
/// S/C ≈ 4 (0.424675) and falls to 0.401227 by S/C = 6 — the dilution
/// turnover, recorded so it is not mistaken for a defect later.
#[test]
fn conversion_rises_with_steam_to_carbon_at_equilibrium() {
    let mut last = -1.0;
    for sc in [1.0, 2.0, 3.0, 4.0, 5.0, 6.0] {
        let mut case = base_case();
        case.steam_to_carbon = sc;
        case.volume = 2.0e5;
        let x = case.solve().expect("solve").methane_conversion;
        assert!(x > last, "conversion fell at S/C = {sc}: {x} after {last}");
        last = x;
    }
}

/// The shift's forward rate sets only how fast equilibrium is reached, not
/// where it is — which is what licenses choosing it for conditioning.
///
/// **Methodology.** The deck's shift pre-exponential is `1e2`, picked because
/// larger values make the residual `ζ − V·rate` stiff enough to be
/// unsolvable (see the "Stiffness" section of `src/smr.rs`). That choice is
/// only legitimate if the reported answer does not depend on it. Sweep `A`
/// over five decades and check both that the conversion is invariant and that
/// the shift is genuinely at equilibrium across the range.
///
/// **Results (measured 2026-09-25).** Conversion 0.41174540 at `A = 1e2`
/// through 0.41174800 at `A = 1e6` — 6.3e-6 relative over four decades — with
/// the shift's `Q/Kc` running 0.999159 to 1.000000. At `A = 1e1` it is
/// 0.41172225 (`Q/Kc = 0.9917`), i.e. the approach to equilibrium is only
/// just visible one decade below the deck's value.
#[test]
fn shift_kinetics_only_set_the_approach_rate() {
    let mut conversions = Vec::new();
    for a in [1.0e2, 1.0e3, 1.0e4, 1.0e5, 1.0e6] {
        let mut case = base_case();
        case.forward[1] = ForwardRate { a, e: 6.7e4 };
        let out = case.solve().expect("solve");
        conversions.push(out.methane_conversion);

        let q = case.volumetric_flow();
        let conc: Vec<f64> = out.outlet_flows.iter().map(|&f| f.max(0.0) / q).collect();
        let quotient: f64 = SmrReaction::WaterGasShift
            .stoichiometry()
            .iter()
            .map(|&(sp, nu)| conc[sp.index()].powf(nu))
            .product();
        let ratio = quotient / SmrReaction::WaterGasShift.kc(case.temperature);
        assert!(
            (ratio - 1.0).abs() < 1e-3,
            "at A = {a} the shift should be at equilibrium, Q/Kc = {ratio}"
        );
    }

    let spread = (conversions.iter().cloned().fold(f64::MIN, f64::max)
        - conversions.iter().cloned().fold(f64::MAX, f64::min))
        / conversions[0];
    assert!(
        spread < 1e-4,
        "conversion must not depend on the shift pre-exponential; \
         spread {spread} over {conversions:?}"
    );
}

/// A reforming reactor must need heat, and the products must be hydrogen-rich.
#[test]
fn the_base_case_is_endothermic_and_makes_hydrogen() {
    let case = base_case();
    let out = case.solve().expect("solve");

    assert!(
        out.heat_duty > 0.0,
        "reforming must require heat, duty {} W",
        out.heat_duty
    );
    assert!(
        out.hydrogen_yield(case.methane_feed) > 1.0,
        "yield {} mol H2 per mol CH4 is too low to be reforming",
        out.hydrogen_yield(case.methane_feed)
    );
    // The stoichiometric ceiling is 4 mol H2 per mol CH4.
    assert!(
        out.hydrogen_yield(case.methane_feed) < 4.0,
        "yield exceeds the stoichiometric maximum"
    );
    // Mole fractions form a distribution.
    let total: f64 = Species::INDEX_ORDER
        .iter()
        .map(|&sp| out.mole_fraction(sp))
        .sum();
    assert!((total - 1.0).abs() < 1e-9, "mole fractions sum to {total}");
}

// ---------------------------------------------------------------------------
// The deck
// ---------------------------------------------------------------------------

#[test]
fn the_bundled_decks_parse_and_run() {
    for path in ["decks/smr_cstr.toml", "decks/smr_temperature_sweep.toml"] {
        let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{path}: {e}"));
        let csv = headless::run_toml(&text).unwrap_or_else(|e| panic!("{path}: {e}"));
        assert!(csv.starts_with(headless::CSV_HEADER));
        for line in csv.lines().skip(1) {
            assert!(
                line.ends_with(",ok"),
                "{path} produced a failed row: {line}"
            );
        }
    }
}

#[test]
fn a_deck_round_trips_through_toml() {
    let text = std::fs::read_to_string("decks/smr_cstr.toml").expect("read");
    let deck = Deck::from_toml(&text).expect("parse");
    let again = Deck::from_toml(&deck.to_toml().expect("serialise")).expect("reparse");
    assert_eq!(deck, again);
}

/// Validation must reject, not interpret. Each of these parses as TOML and
/// describes something the model cannot honestly answer.
#[test]
fn invalid_decks_are_rejected_with_a_reason() {
    let good = std::fs::read_to_string("decks/smr_cstr.toml").expect("read");

    let cases: [(&str, &str, &str); 6] = [
        ("negative volume", "volume_m3 = 2.0", "volume_m3 = -2.0"),
        ("zero feed", "methane_mol_s = 1.0", "methane_mol_s = 0.0"),
        (
            "zero steam",
            "steam_to_carbon = 3.0",
            "steam_to_carbon = 0.0",
        ),
        (
            "unimplemented reactor",
            r#"kind = "cstr""#,
            r#"kind = "pfr""#,
        ),
        ("wrong schema", "schema_version = 1", "schema_version = 99"),
        (
            "negative pre-exponential",
            "a_forward = 1.0e7",
            "a_forward = -1.0e7",
        ),
    ];

    for (label, from, to) in cases {
        let broken = good.replacen(from, to, 1);
        assert_ne!(broken, good, "{label}: fixture substitution did not apply");
        match Deck::from_toml(&broken) {
            Err(DeckError::Invalid(_)) => {}
            Err(other) => panic!("{label}: expected Invalid, got {other}"),
            Ok(_) => panic!("{label}: should have been rejected"),
        }
    }
}

/// An unknown key is a typo, not an extension point.
#[test]
fn unknown_keys_are_rejected_rather_than_ignored() {
    let good = std::fs::read_to_string("decks/smr_cstr.toml").expect("read");
    let typo = good.replacen("volume_m3", "volume_m", 1);
    match Deck::from_toml(&typo) {
        Err(DeckError::Parse(_)) => {}
        other => panic!("a misspelled key must not be silently ignored, got {other:?}"),
    }
}

/// Deterministic: same deck in, byte-identical CSV out. Every committed
/// fixture depends on this.
#[test]
fn the_headless_run_is_deterministic() {
    let text = std::fs::read_to_string("decks/smr_temperature_sweep.toml").expect("read");
    assert_eq!(
        headless::run_toml(&text).expect("run"),
        headless::run_toml(&text).expect("run")
    );
}

/// The sweep expands to the requested number of points, inclusive of both
/// bounds.
#[test]
fn a_sweep_expands_to_its_endpoints() {
    let text = std::fs::read_to_string("decks/smr_temperature_sweep.toml").expect("read");
    let deck = Deck::from_toml(&text).expect("parse");
    let cases = deck.cases();
    assert_eq!(cases.len(), 11);
    assert!((cases[0].1.temperature - 800.0).abs() < 1e-9);
    assert!((cases[10].1.temperature - 1300.0).abs() < 1e-9);
}

/// The committed CSV fixtures still reproduce, byte for byte.
///
/// **Why this exists.** The workspace's headless rule requires a committed
/// fixture that diffs cleanly, and it is the only check here that can catch a
/// numerical change nobody intended — every other test asserts a *direction* or
/// an *invariant*, which a shifted number can satisfy while still being wrong.
/// Regenerate deliberately with
/// `cargo run --release -p dover --example smr_cstr -- crates/dover/decks/<deck>.toml`
/// and say in the commit message what moved and why.
///
/// **Results (generated 2026-09-25).** The base deck gives one row at
/// `X_CH4 = 0.411745`, `H2_per_CH4 = 1.428409`, duty 76 819.6383 W; the
/// temperature sweep gives 11 rows from 800 K to 1300 K.
#[test]
fn the_committed_fixtures_still_reproduce() {
    for (deck, fixture) in [
        ("decks/smr_cstr.toml", "tests/fixtures/smr_cstr.csv"),
        (
            "decks/smr_temperature_sweep.toml",
            "tests/fixtures/smr_temperature_sweep.csv",
        ),
    ] {
        let text = std::fs::read_to_string(deck).unwrap_or_else(|e| panic!("{deck}: {e}"));
        let produced = headless::run_toml(&text).unwrap_or_else(|e| panic!("{deck}: {e}"));
        let expected =
            std::fs::read_to_string(fixture).unwrap_or_else(|e| panic!("{fixture}: {e}"));
        assert_eq!(
            produced, expected,
            "{deck} no longer reproduces {fixture}; if the change is intended, \
             regenerate the fixture and record what moved"
        );
    }
}
