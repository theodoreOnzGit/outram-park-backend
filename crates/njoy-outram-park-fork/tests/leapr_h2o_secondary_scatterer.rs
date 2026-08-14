//! V&V: light-water S(α,β) regenerated from `tsl-HinH2O.leapr`.
//!
//! # What this validates
//!
//! That LEAPR can **generate** the H-in-H₂O thermal scattering law from the
//! 12 kB card deck compiled into this crate, with no ENDF tape present, and
//! that the result reproduces the published evaluation. Two things had to be
//! ported for this to work at all, and both are exercised here:
//!
//! 1. **The secondary scatterer** (card 6). `tsl-HinH2O` is the only registered
//!    deck with `nss > 0`: H is the principal scatterer and the molecule's
//!    oxygen is a *secondary* one, declared free-gas (`b7 = 1`). Before
//!    2026-08-14 [`generate_tape`] refused every `nss != 0` deck, so light water
//!    could not be regenerated at all.
//! 2. **The translational and discrete-oscillator stages.** `generate_tape`
//!    previously ran only the phonon expansion (`contin`), silently dropping the
//!    `trans` and `discre` stages of the Fortran temperature loop
//!    (`leapr.f90:376-384`). That is a no-op for graphite — `twt = 0`, no
//!    oscillators — which is why it survived the graphite parity study, and it
//!    is badly wrong for a molecular liquid. See
//!    [`effective_temperature_includes_translation_and_oscillators`].
//!
//! ## Data / provenance
//!
//! - **Deck:** `src/leapr/decks/tsl-HinH2O.leapr`, the ENDF/B-VIII.0 LEAPR job
//!   for H in light water ("CAB Model from molecular dynamics calculations"),
//!   `MAT = 1`, `ZA = 1001`. Open-source (ENDF/B-VIII.0, 2018), per
//!   `DATA_POLICY.md`. Nothing is downloaded and no tape is read.
//! - **Kernel:** port of NJOY2016 (release 2016.79, commit `ac5adf5f`)
//!   `leapr.f90` — `contin` / `trans` / `discre` / `endout`.
//! - **Reference values** for the published evaluation are the measurements
//!   recorded in `tests/thermal_h2o_sab.rs` (taken 2026-07-15 from
//!   ENDF/B-VIII.0 `tsl-HinH2O.endf`, MAT 1, at 293.6 K). That 17.4 MB file is
//!   not checked in and is **not present in this container**, so those numbers
//!   are cited from the repository's own recorded V&V rather than re-derived
//!   here. This test therefore compares *generation against the evaluation as
//!   previously measured*, which is why its tolerances are stated as agreement
//!   bands rather than a bit-parity claim.
//!
//! # Methodology and measured results (2026-08-14, generated at 293.6 K)
//!
//! | Quantity | Published evaluation | Regenerated | Deviation |
//! |---|---|---|---|
//! | `T_eff` | 1194.3 K | **1195.35 K** | **+0.09 %** |
//! | σ_free | 20.436 b/H | **20.43608 b/H** | exact (a `B` constant) |
//! | σ_inel(0.0253 eV) | 52.10 b/H | **52.408 b/H** | **+0.59 %** |
//! | σ_inel(1 eV) | 21.713 b/H | **21.847 b/H** | **+0.62 %** |
//! | σ_inel(4 eV) | 20.951 b/H | **21.073 b/H** | **+0.58 %** |
//! | σ_inel(8 eV) | 20.707 b/H | **20.835 b/H** | **+0.62 %** |
//!
//! For contrast, the same generation path *without* the `trans` / `discre`
//! stages gave `T_eff = 482.49 K` (−60 %) and `σ_inel(8 eV) = 30.58 b` (+48 %).
//!
//! # Interpretation
//!
//! Agreement is uniform at **~0.6 %** across five decades of incident energy,
//! and `T_eff` — the short-collision-time parameter that governs the whole
//! high-energy tail — lands within 0.09 %. That is strong evidence the three
//! generation stages and the `B` constants are all wired correctly.
//!
//! It is **not** a bit-parity claim of the kind made for crystalline graphite
//! (60,000/60,000 stored values identical). A residual of a few tenths of a
//! percent is consistent with the deck's `rho(E)` being a tabulated input and
//! with accumulated quadrature differences; localising it would need the
//! published tape present for a point-by-point diff. Light water is therefore
//! still reported as **not validated against a reference tape** by
//! `SabRequest::validation`, and that reporting is correct — see bead `op-rvrg`.

use njoy_outram_park_fork::leapr::decks::SabMaterial;
use njoy_outram_park_fork::leapr::deck::LeaprDeck;
use njoy_outram_park_fork::leapr::generate::{thermal_scattering_tape, ElasticChannel, SabRequest};
use njoy_outram_park_fork::leapr::input::SecondaryScattererKind;
use njoy_outram_park_fork::thermr::scattering::IncoherentInelasticScattering;
use njoy_outram_park_fork::units::{NeutronEnergy, Temperature};
use uom::si::{area::barn, energy::electronvolt, thermodynamic_temperature::kelvin};

/// The evaluation temperature every check below uses.
const T_K: f64 = 293.6;

/// Regenerate the law and hand back the consumer surface.
///
/// The elastic channel is omitted: light water is a liquid with no MF=7/MT=2
/// section, so there is nothing to generate and asking for one would only cost
/// time.
fn regenerate() -> IncoherentInelasticScattering {
    let request = SabRequest::new(SabMaterial::HInH2O, Temperature::new::<kelvin>(T_K))
        .with_elastic(ElasticChannel::Omit);
    let tape = thermal_scattering_tape(&request).expect("regenerate H-in-H2O from its deck");
    IncoherentInelasticScattering::from_tape(
        &tape,
        SabMaterial::HInH2O.mat(),
        Temperature::new::<kelvin>(T_K),
    )
    .expect("parse the generated MF=7/MT=4")
}

/// The deck declares oxygen as a free-gas secondary scatterer, and the parser
/// resolves card 6 into the typed form the writer wants.
///
/// **Pass criterion:** `nss = 1`, kind `FreeGas`, `aws = 15.85751`,
/// `sps = 3.7939` b, `mss = 1`, and — because `b7 > 0` — the deck reports **no**
/// unsupported features, so generation is allowed to proceed.
///
/// **Result (2026-08-14):** holds exactly.
#[test]
fn deck_declares_a_free_gas_oxygen_secondary_scatterer() {
    let text = njoy_outram_park_fork::leapr::decks::embedded_deck_text(SabMaterial::HInH2O)
        .expect("tsl-HinH2O.leapr is compiled in");
    let deck = LeaprDeck::parse(text).expect("parse tsl-HinH2O.leapr");

    assert_eq!(deck.nss, 1, "H2O declares one secondary scatterer");
    let s = deck
        .secondary_scatterer()
        .expect("card 6 resolves to a secondary scatterer");
    assert_eq!(
        s.kind,
        SecondaryScattererKind::FreeGas,
        "b7 = 1 is the free-gas kind"
    );
    assert!(
        !s.kind.merges_into_sab(),
        "free gas is carried in B(7..12), not merged"
    );
    assert!(
        (s.aws - 15.85751).abs() < 1e-5,
        "aws = O-16 mass ratio, got {}",
        s.aws
    );
    assert!(
        (s.sps - 3.7939).abs() < 1e-4,
        "sps = O free xs, got {}",
        s.sps
    );
    assert_eq!(s.mss, 1, "one O per H2O");

    assert!(
        deck.unsupported_features().is_empty(),
        "a b7 > 0 secondary must not block generation: {:?}",
        deck.unsupported_features()
    );
}

/// The generated MF=7/MT=4 carries all twelve `B` constants, and the six
/// principal ones match the published evaluation.
///
/// **Methodology.** Regenerate, parse, and compare `B(1)`, `B(3)`, `B(6)` with
/// the values `tests/thermal_h2o_sab.rs` records from the ENDF/B-VIII.0 tape,
/// and `B(7)..B(12)` with card 6 of the deck (`leapr.f90:3315-3321`).
///
/// **Pass criterion:** `NI = 12`; `B(1) = 40.872`, `B(3) = 0.99917`,
/// `B(6) = 2`; `B(7) = 1`, `B(8) = mss*sps = 3.7939`, `B(9) = 15.85751`,
/// `B(10) = B(11) = 0`, `B(12) = 1`.
///
/// **Result (2026-08-14):** `B = [40.87216, 395.26, 0.9991673, 10.00008, 0,
/// 2, 1, 3.7939, 15.85751, 0, 0, 1]` — every asserted entry matches.
#[test]
fn generated_b_constants_carry_the_secondary_scatterer() {
    let sab = regenerate();
    let b = &sab.kernel().b;

    assert_eq!(
        b.len(),
        12,
        "NI = 6*(nss+1) = 12 with a secondary scatterer, got {b:?}"
    );
    // Principal scatterer — cross-checked against the published evaluation.
    assert!(
        (b[0] - 40.872).abs() < 1e-3,
        "B(1) = npr*spr = 40.872, got {}",
        b[0]
    );
    assert!(
        (b[2] - 0.99917).abs() < 1e-4,
        "B(3) = A(H) = 0.99917, got {}",
        b[2]
    );
    assert!(
        (b[5] - 2.0).abs() < 1e-9,
        "B(6) = 2 principal H atoms, got {}",
        b[5]
    );
    // Secondary scatterer — the newly ported constants.
    assert!(
        (b[6] - 1.0).abs() < 1e-9,
        "B(7) = 1 (free gas), got {}",
        b[6]
    );
    assert!(
        (b[7] - 3.7939).abs() < 1e-4,
        "B(8) = mss*sps = 3.7939, got {}",
        b[7]
    );
    assert!(
        (b[8] - 15.85751).abs() < 1e-5,
        "B(9) = aws = 15.85751, got {}",
        b[8]
    );
    assert!(
        b[9].abs() < 1e-12 && b[10].abs() < 1e-12,
        "B(10), B(11) = 0"
    );
    assert!(
        (b[11] - 1.0).abs() < 1e-9,
        "B(12) = 1 O atom, got {}",
        b[11]
    );
}

/// The effective temperature reflects **all three** generation stages.
///
/// This is the regression guard for the `trans` / `discre` omission. `T_eff` is
/// the single most sensitive scalar to that bug: the continuum alone puts it at
/// 482 K, and the molecule's translational term plus its two vibrational modes
/// carry it to the ~1194 K the evaluation reports. It also propagates — `T_eff`
/// sets the short-collision-time tail, hence the whole high-energy σ(E).
///
/// **Pass criterion:** `T_eff` within 2 % of the evaluation's 1194.3 K, and
/// comfortably above the continuum-only 482.49 K that the pre-fix path gave.
///
/// **Result (2026-08-14):** `T_eff = 1195.35 K`, +0.09 % against 1194.3 K.
#[test]
fn effective_temperature_includes_translation_and_oscillators() {
    let sab = regenerate();
    let teff = sab.effective_temperature().get::<kelvin>();

    const PUBLISHED_TEFF_K: f64 = 1194.3;
    const CONTINUUM_ONLY_TEFF_K: f64 = 482.49;

    let rel = (teff - PUBLISHED_TEFF_K).abs() / PUBLISHED_TEFF_K;
    assert!(
        rel < 0.02,
        "T_eff = {teff} K must match the evaluation's {PUBLISHED_TEFF_K} K within 2 % \
         (got {:.2} %)",
        rel * 100.0
    );
    assert!(
        teff > 2.0 * CONTINUUM_ONLY_TEFF_K,
        "T_eff = {teff} K looks continuum-only ({CONTINUUM_ONLY_TEFF_K} K): the trans/discre \
         stages of leapr.f90:376-384 are not running"
    );
}

/// σ_inel(E) reproduces the published evaluation across five decades, and
/// relaxes to the free-atom limit at high energy.
///
/// **Methodology.** Evaluate the regenerated σ_inel at 0.0253, 1, 4 and 8 eV and
/// compare against the values `tests/thermal_h2o_sab.rs` measured from the
/// ENDF/B-VIII.0 tape. Separately assert the physics that must hold regardless
/// of the reference: σ_free = B(1)/natom = 20.436 b/H, and σ_inel decreasing
/// monotonically toward it over 1 → 4 → 8 eV.
///
/// **Pass criterion:** every point within 2 % of the published value; monotone
/// decrease 1 → 4 → 8 eV; σ_inel(8 eV) within 5 % of σ_free.
///
/// **Result (2026-08-14):** 52.408 / 21.847 / 21.073 / 20.835 b/H against the
/// published 52.10 / 21.713 / 20.951 / 20.707 b/H — **+0.59 / +0.62 / +0.58 /
/// +0.62 %**. σ_inel(8 eV) is +1.95 % of σ_free = 20.43608 b.
#[test]
fn cross_section_matches_the_published_evaluation() {
    let sab = regenerate();

    let sigma_free = sab.free_cross_section().get::<barn>();
    assert!(
        (sigma_free - 20.436).abs() < 1e-3,
        "sigma_free = B(1)/natom = 20.436 b/H, got {sigma_free}"
    );

    let xs = |e_ev: f64| {
        sab.inelastic_xs(NeutronEnergy::new::<electronvolt>(e_ev))
            .get::<barn>()
    };

    // (incident energy [eV], published sigma_inel [b/H] — see the module docs)
    for (e_ev, published) in [
        (0.0253_f64, 52.10_f64),
        (1.0, 21.713),
        (4.0, 20.951),
        (8.0, 20.707),
    ] {
        let got = xs(e_ev);
        let rel = (got - published).abs() / published;
        assert!(
            rel < 0.02,
            "sigma_inel({e_ev} eV) = {got} b/H vs published {published} b/H \
             (deviation {:.2} %, budget 2 %)",
            rel * 100.0
        );
    }

    let (s1, s4, s8) = (xs(1.0), xs(4.0), xs(8.0));
    assert!(
        s1 > s4 && s4 > s8,
        "sigma_inel must decrease toward the free-atom limit: {s1} {s4} {s8}"
    );
    let rel = (s8 - sigma_free).abs() / sigma_free;
    assert!(
        rel < 0.05,
        "sigma_inel(8 eV) = {s8} b within 5 % of sigma_free = {sigma_free} b (got {:.2} %)",
        rel * 100.0
    );
}

/// A short-collision-time secondary scatterer is still refused, loudly.
///
/// The `b7 = 0` case needs the genuine mixed-moderator merge — a second LEAPR
/// pass over the secondary scatterer, merged as
/// `S = S_principal + (sbs/sb)*S_secondary` (`leapr.f90:3018-3030`) — which is
/// **not** ported (bead `op-bax5`). No registered deck uses it. This asserts the
/// refusal rather than the merge, so that porting `b7 > 0` cannot be mistaken
/// for having ported all of `nss`.
///
/// **Methodology.** Take the real water deck, rewrite card 6's `b7` from `1` to
/// `0`, and re-parse.
///
/// **Pass criterion:** `unsupported_features` names the short-collision-time
/// secondary.
///
/// **Result (2026-08-14):** reported as
/// `"nss = 1 with b7 = 0 (short-collision-time secondary: the mixed-moderator
/// S(alpha,beta) merge is not ported)"`.
#[test]
fn short_collision_time_secondary_is_still_refused() {
    let text = njoy_outram_park_fork::leapr::decks::embedded_deck_text(SabMaterial::HInH2O)
        .expect("tsl-HinH2O.leapr is compiled in");
    // Card 6 of the real deck; only b7 changes.
    let patched = text.replace(
        "1 1 15.85751 3.7939 1  / NSS B7 AWS SPS MSS",
        "1 0 15.85751 3.7939 1  / NSS B7 AWS SPS MSS",
    );
    assert_ne!(patched, text, "card 6 must have been found and rewritten");

    let deck = LeaprDeck::parse(&patched).expect("the patched deck still parses");
    let unsupported = deck.unsupported_features();
    assert!(
        unsupported
            .iter()
            .any(|f| f.contains("short-collision-time")),
        "a b7 = 0 secondary must be refused, got {unsupported:?}"
    );
}
