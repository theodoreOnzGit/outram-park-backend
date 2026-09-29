// SPDX-License-Identifier: GPL-3.0
//
// Cross-check of buangkok's pyDOSEIA port against Liu and Cao (2002)'s
// published HTR-10 doses, EXTERNAL pathways only (gh:#379).

//! # V&V (gh:#379): `buangkok` external dose from Liu and Cao's releases vs their published doses
//!
//! **Scope limit, binding:** this is a *verification of `buangkok`* against a
//! published calculation by other codes (AIRDOS-EPA for Table 7, STOERNEU for
//! Table 9). It is **not** a dose assessment for HTR-10 or any site, and no
//! number here may be quoted as one (`RESPONSIBLE_USE.md`; #226).
//!
//! ## Methodology
//!
//! - **Releases (inputs):** Liu and Cao (2002) **Table 5** (normal operation,
//!   Bq/y; `changi::activity::airborne_release`) and **Table 8** (the two
//!   design-basis accidents, Bq; `changi::activity::accident_airborne_release`).
//! - **Dose model:** `buangkok::pydoseia`, the MIT pyDOSEIA port, verified
//!   code-to-code against upstream.
//!   - Normal operation: `dilution_long_term_no_met` (sector-averaged) with
//!     `Release::AnnualDischarge`.
//!   - Accidents: `dilution_single_plume_no_met` (ground-level centreline)
//!     with `Release::Instantaneous`.
//!   - Pathways: **submersion** (`submersion_dose`) and **ground shine**
//!     (`ground_shine_dose`; upstream deposition velocities, weathering off,
//!     1-year exposure, which is upstream's convention).
//! - **Coefficients:** adult **effective** dose-rate coefficients from **EPA
//!   FGR-15** (EPA 402-R-25-001, July 2025) Tables 4-6 and 4-1, read through
//!   `buangkok::coefficients` from buangkok's own shipped tables
//!   (`reference/fgr15_2025_*.csv`), the workspace's one source of FGR
//!   coefficients. ~~Read from
//!   `crates/kovan-literature/derived/epa-fgr15-adult-external-coefficients.csv`~~
//!   until the 2026-09-29 merge into `develop`; the 18 nuclides buangkok did not
//!   yet carry were appended from that extraction (identical values for the
//!   six it did), whose record stays beside it in `kovan-literature/derived/`. Half-lives for the ground build-up come from
//!   `boon-lay`'s TRISO-ATOPS nuclide table.
//! - **Meteorology: NOT in the paper**, so it is swept. Stability classes A-F,
//!   wind 1, 3 and 5 m/s, 40 m stack (published), wind measured at 10 m
//!   (assumed). The result is an **external-only dose band** per distance.
//! - **What is deliberately NOT computed:**
//!   - inhalation and ingestion: FGR-11's 1988 tables did not extract through
//!     kovan (scanned image), and ingestion needs transfer factors;
//!   - the **thyroid** column of Table 9, which is inhalation-dominated;
//!   - progeny other than Cs-137 -> Ba-137m (added in secular equilibrium by
//!     `buangkok::coefficients`' progeny correction at FGR-15's own 0.944,
//!     ~~the ENDF/B-VIII.0 0.94699 via boon-lay~~ before 2026-09-29; the ENDF
//!     value is kept as a cross-check test below);
//!   - plume rise;
//!   - building wake.
//!
//!   So the band is a **partial** dose, and Table 7/9's totals include
//!   pathways it lacks.
//! - **Pass criterion:** sanity only (finite, positive, monotone in the
//!   release). **Whether the published value falls inside the band is the
//!   finding, not an assertion.** Nothing is tuned.
//!
//! ## Results and interpretation
//!
//! See each test's doc comment (measured 2026-09-29).

use buangkok::published::accident_dose_by_distance::htr10_accident_dose_by_distance;
use buangkok::published::normal_operation_dose_by_distance::htr10_normal_operation_dose_by_distance;
use buangkok::pydoseia::dispersion::{
    dilution_long_term_no_met, dilution_single_plume_no_met, MeanSpeedScaling, PlumeGeometry,
    Receptor,
};
use buangkok::pydoseia::dose::{
    deposition_velocity_m_per_s, effective_buildup_time_s, ground_shine_dose, submersion_dose,
    Release, Weathering,
};
use changi::activity::accident_airborne_release::{htr10_accident_release, AccidentCase};
use changi::activity::airborne_release::htr10_normal_operation_annual_release;

use uom::si::f64::{Length, Radioactivity, Velocity};
use uom::si::length::{kilometer, meter};
use uom::si::radioactivity::becquerel;
use uom::si::velocity::meter_per_second;

/// Adult `(air submersion Sv m^3/(Bq s), ground surface Sv m^2/(Bq s))` for a
/// released nuclide, **with** its short-lived progeny (only Cs-137 -> Ba-137m
/// among these), through buangkok's own coefficient tables and progeny
/// correction -- the same path `htgr_sim_v1`'s dose-rate map uses.
fn fgr15_with_progeny(nuclide: &str) -> (f64, f64) {
    use buangkok::coefficients::{
        external_coefficient, fgr15_air_submersion, fgr15_ground_surface,
        fgr15_short_lived_progeny,
    };
    use buangkok::pydoseia::dcf::AgeBracket;
    let chains = fgr15_short_lived_progeny();
    let get = |t| {
        external_coefficient(&t, &chains, nuclide, AgeBracket::Adult)
            .unwrap_or_else(|| panic!("{nuclide}: no adult FGR-15 coefficient in buangkok"))
    };
    (get(fgr15_air_submersion()), get(fgr15_ground_surface()))
}

/// Element symbol from a label such as `"Xe-133m"`.
fn element(nuclide: &str) -> &str {
    nuclide.split('-').next().unwrap()
}

/// Decay constant \[1/s\] for a depositing nuclide, from boon-lay. Only called
/// where the deposition velocity is nonzero; noble gases, H and C never reach
/// it.
fn decay_constant(nuclide: &str) -> f64 {
    boon_lay::triso_atops_fork::nuclide_model::nuclide_database::find_nuclide(nuclide)
        .unwrap_or_else(|| panic!("{nuclide}: no half-life in boon-lay's TRISO-ATOPS table"))
        .decay_constant()
        .get::<uom::si::frequency::hertz>()
}

const SPEEDS_M_PER_S: [f64; 3] = [1.0, 3.0, 5.0];
const CLASSES: [&str; 6] = ["A", "B", "C", "D", "E", "F"];

fn geometry() -> PlumeGeometry {
    PlumeGeometry {
        release_height: Length::new::<meter>(40.0),
        measurement_height: Length::new::<meter>(10.0),
        receptor: Receptor::GroundLevelCentreline,
    }
}

/// Cs-137 -> Ba-137m branching fraction, from boon-lay's ENDF/B-VIII.0 decay
/// library (`openmc-endf-8-depletion-lib-b`), for the cross-check against
/// FGR-15's 0.944 only (the dose itself uses buangkok's progeny correction).
fn cs137_to_ba137m_branching() -> f64 {
    use boon_lay::prelude::decay_library::DecayLibrary;
    let data = DecayLibrary::new()
        .try_match_nuclides_to_decay_data(boon_lay::Nuclide::Cs137)
        .expect("Cs-137 decay data");
    data.decay_information
        .iter()
        .filter(|d| d.target == Some(boon_lay::Nuclide::Ba137m))
        .map(|d| d.branching_ratio.get::<uom::si::ratio::ratio>())
        .sum()
}

/// External (submersion + ground-shine) dose \[mSv\] for one release list at
/// one chi/Q, summed over nuclides. Returns (submersion, ground shine).
fn external_dose(
    releases: &[(&str, f64)],
    chi_over_q: changi::activity::units::DilutionFactor,
    instantaneous: bool,
) -> (f64, f64) {
    let mut sub = 0.0;
    let mut gs = 0.0;
    for &(nuclide, bq) in releases {
        let (dcf_sub, dcf_gs) = fgr15_with_progeny(nuclide);
        let q = Radioactivity::new::<becquerel>(bq);
        let release = if instantaneous {
            Release::Instantaneous(q)
        } else {
            Release::AnnualDischarge(q)
        };
        sub += submersion_dose(chi_over_q, release, dcf_sub).millisieverts();
        let el = element(nuclide);
        let vd = deposition_velocity_m_per_s(el);
        if vd > 0.0 {
            let t_b = effective_buildup_time_s(decay_constant(nuclide), el, Weathering::Off, 1.0);
            gs += ground_shine_dose(chi_over_q, release, vd, t_b, dcf_gs).millisieverts();
        }
    }
    (sub, gs)
}

/// Per class and speed, the six-class dilution array divided by the speed.
fn scaled(speed: f64) -> MeanSpeedScaling {
    MeanSpeedScaling::PerClass([Velocity::new::<meter_per_second>(speed); 6])
}

/// **Normal operation: Table 5 releases -> external dose, vs Table 7.**
///
/// **Results (2026-09-29).** Band = min..max over classes A-F x 1/3/5 m/s. The
/// "D, 3 m/s" column is one representative condition. Ground shine is 1.4 % of
/// external there: **Ar-41 submersion dominates** (1e11 Bq/y x 6.16e-14).
///
/// | x \[km\] | Table 7 \[mSv/y\] | band min | band max | D, 3 m/s | inside |
/// |---|---|---|---|---|---|
/// | 0.5 | 1.1e-4 | 2.82e-9 | 3.38e-4 | 3.82e-5 | yes |
/// | 1.5 | 1.4e-4 | 1.47e-6 | 1.13e-4 | 3.77e-5 | no |
/// | 4.0 | 7.6e-5 | 7.01e-8 | 3.15e-5 | 1.05e-5 | no |
/// | 15 | 2.5e-5 | 1.17e-9 | 8.13e-6 | 1.45e-6 | no |
/// | 75 | 7.7e-6 | 8.06e-12 | 1.18e-6 | 1.24e-7 | no |
///
/// **Interpretation.** buangkok's external-only dose is **below** Table 7 at
/// every distance past 0.5 km (by 1.2x at 1.5 km, up to 6.5x at 75 km, against
/// the band maximum). That is the expected direction: Table 7 also carries
/// inhalation and ingestion. At D, 3 m/s, external is ~27 % of Table 7 at
/// 1.5 km. **The shapes differ:** Table 7 falls 18x from 1.5 to 75 km, and
/// buangkok's D, 3 m/s falls 300x. A slowly falling component is what
/// regional ingestion (H-3 and C-14 into food) would give, but the paper's
/// site meteorology and pathway breakdown would be needed to say so. This is
/// **consistent, not verified**: no contradiction, and no tight agreement
/// either.
#[test]
fn normal_operation_external_band_against_table_7() {
    let releases: Vec<(&str, f64)> = htr10_normal_operation_annual_release()
        .iter()
        .map(|e| (e.nuclide, e.annual_release.get::<becquerel>()))
        .collect();
    println!("NORMAL OPERATION (mSv/y): Table 7 vs buangkok external-only band over A-F x {SPEEDS_M_PER_S:?} m/s");
    println!(
        "{:>7} {:>11} {:>11} {:>11} {:>11} {:>9} {:>6}",
        "x [km]", "Table 7", "band min", "band max", "D, 3 m/s", "gs share", "inside"
    );
    for row in htr10_normal_operation_dose_by_distance() {
        let x = row.distance;
        let mut lo = f64::INFINITY;
        let mut hi: f64 = 0.0;
        let mut reference = (0.0, 0.0);
        for speed in SPEEDS_M_PER_S {
            let chi = dilution_long_term_no_met(x, geometry(), scaled(speed));
            for (i, c) in chi.iter().enumerate() {
                let (sub, gs) = external_dose(&releases, *c, false);
                let total = sub + gs;
                assert!(
                    total.is_finite() && total > 0.0,
                    "non-physical dose at {x:?}"
                );
                lo = lo.min(total);
                hi = hi.max(total);
                if CLASSES[i] == "D" && speed == 3.0 {
                    reference = (total, gs / total);
                }
            }
        }
        let t7 = row.effective_dose_msv_per_year;
        println!(
            "{:>7.2} {t7:>11.3e} {lo:>11.3e} {hi:>11.3e} {:>11.3e} {:>9.3} {:>6}",
            x.get::<kilometer>(),
            reference.0,
            reference.1,
            if t7 >= lo && t7 <= hi { "yes" } else { "no" }
        );
    }
}

/// **Accidents: Table 8 releases -> external dose, vs Table 9 whole-body.**
///
/// **Results (2026-09-29).** Band = min..max over classes A-F x 1/3/5 m/s.
/// Cs-137 includes Ba-137m in equilibrium (FGR-15's 0.944 through
/// `buangkok::coefficients`; ~~0.94699 from ENDF/B-VIII.0 via boon-lay~~ until
/// the 2026-09-29 merge). Without it the accident external dose was ~15x lower. Ground
/// shine is 97 % (depressurization) and 99.6 % (water ingress) of external at
/// D, 3 m/s: **Cs-137/Ba-137m deposition dominates**, not the noble gases.
///
/// | x \[km\] | Depr. WB | band max | D, 3 m/s | Water WB | band max | D, 3 m/s |
/// |---|---|---|---|---|---|---|
/// | 0.25 | 7.7e-2 | 1.42e-3 | 4.74e-6 | 2.0e-1 | 3.32e-3 | 1.11e-5 |
/// | 1.5 | 1.9e-2 | 6.70e-4 | 2.23e-4 | 5.2e-2 | 1.57e-3 | 5.22e-4 |
/// | 7.5 | 2.4e-3 | 2.45e-4 | 2.87e-5 | 6.1e-3 | 5.73e-4 | 6.70e-5 |
/// | 15 | 6.6e-4 | 1.23e-4 | 1.07e-5 | 1.6e-3 | 2.86e-4 | 2.51e-5 |
/// | 45 | 2.8e-5 | 3.69e-5 | 2.23e-6 | 6.9e-5 | 8.62e-5 | 5.21e-6 |
/// | 75 | 2.0e-6 | 2.08e-5 | 1.07e-6 | 4.7e-6 | 4.86e-5 | 2.51e-6 |
///
/// The table above was measured with the ENDF 0.94699 branching. **Re-measured
/// 2026-09-29 with FGR-15's 0.944** (buangkok's progeny table): every accident
/// value is 0.3 % or less lower, e.g. depressurization 0.25 km band max
/// 1.42e-3 -> 1.419e-3, water ingress 0.25 km band max 3.32e-3 -> 3.311e-3,
/// water ingress 1.5 km D/3 m/s 5.22e-4 -> 5.200e-4. No inside/outside verdict
/// changes. The normal-operation table above is unchanged at its printed
/// precision: Ar-41 submersion dominates there.
///
/// Table 9 falls inside the band from 45 km out, and above it closer in.
///
/// **Interpretation.**
/// 1. Near the site, Table 9 whole-body is **54-60x above** the external-only
///    band maximum at 0.25 km and **~28x** at 1.5 km. Two unstated inputs can
///    each account for an order of magnitude, so this is **not** evidence of
///    a buangkok error:
///    - **(a) The dose integration period.** The ground shine here uses
///      upstream's 1-year exposure. If Table 9 is a 50-year dose, Cs-137
///      ground shine integrates `int_0^50y exp(-lambda t) dt` = 29.7 y
///      instead of ~1 y, i.e. **~30x**. That is arithmetic, not a run, and
///      it is the size of the 1-15 km gap.
///    - **(b) Inhalation and ingestion**, which are not computed.
/// 2. **The shapes disagree** more than the magnitudes do. Table 9 falls
///    3.85e4x from 0.25 to 75 km. No single stability class here falls that
///    fast (the band maximum falls ~70x), which suggests Table 9's
///    meteorology or deposition (e.g. rain-out close in) differs in kind from
///    the dry Gaussian sweep.
/// 3. Thyroid (Table 9's other column) is not attempted; it needs FGR-11
///    inhalation coefficients, whose tables did not extract through kovan.
///
/// **Net:** no contradiction was found. Resolving the gap needs the paper's
/// integration period and meteorology, which it does not state.
#[test]
fn accident_external_band_against_table_9_whole_body() {
    let entries = htr10_accident_release();
    for case in [AccidentCase::Depressurization, AccidentCase::WaterIngress] {
        let releases: Vec<(&str, f64)> = entries
            .iter()
            .map(|e| (e.nuclide, e.release(case).get::<becquerel>()))
            .collect();
        println!("{case:?} (mSv): Table 9 whole-body vs buangkok external-only band over A-F x {SPEEDS_M_PER_S:?} m/s");
        println!(
            "{:>7} {:>11} {:>11} {:>11} {:>11} {:>9} {:>6}",
            "x [km]", "Table 9 WB", "band min", "band max", "D, 3 m/s", "gs share", "inside"
        );
        for row in htr10_accident_dose_by_distance() {
            let x = row.distance;
            let wb = match case {
                AccidentCase::Depressurization => row.depressurization.whole_body_msv,
                AccidentCase::WaterIngress => row.water_ingress.whole_body_msv,
            };
            let mut lo = f64::INFINITY;
            let mut hi: f64 = 0.0;
            let mut reference = (0.0, 0.0);
            for speed in SPEEDS_M_PER_S {
                let chi = dilution_single_plume_no_met(x, geometry(), scaled(speed));
                for (i, c) in chi.iter().enumerate() {
                    let (sub, gs) = external_dose(&releases, *c, true);
                    let total = sub + gs;
                    assert!(
                        total.is_finite() && total >= 0.0,
                        "non-physical dose at {x:?}"
                    );
                    lo = lo.min(total);
                    hi = hi.max(total);
                    if CLASSES[i] == "D" && speed == 3.0 {
                        reference = (total, if total > 0.0 { gs / total } else { 0.0 });
                    }
                }
            }
            println!(
                "{:>7.2} {wb:>11.3e} {lo:>11.3e} {hi:>11.3e} {:>11.3e} {:>9.3} {:>6}",
                x.get::<kilometer>(),
                reference.0,
                reference.1,
                if wb >= lo && wb <= hi { "yes" } else { "no" }
            );
        }
    }
}

/// Cross-check of the Cs-137 -> Ba-137m branching the dose uses (FGR-15's
/// 0.944, buangkok's progeny table) against boon-lay's ENDF/B-VIII.0 decay
/// library. Both must be a real branch (a missing one silently drops the gamma
/// that dominates caesium ground shine); the difference is printed, not
/// gated. Measured 2026-09-29: ENDF 0.94699 vs FGR-15 0.944 (+0.32 %).
#[test]
fn the_ba137m_branching_agrees_with_the_decay_library() {
    let b = cs137_to_ba137m_branching();
    let (with, _) = fgr15_with_progeny("Cs-137");
    let (bare, _) = fgr15_with_progeny("Ba-137m");
    let parent = 9.37e-17; // FGR-15 Table 4-6 Cs-137 Adult, as shipped
    let fgr = (with - parent) / bare;
    println!(
        "Cs-137 -> Ba-137m branching: ENDF/B-VIII.0 via boon-lay {b}, FGR-15 (buangkok) {fgr:.4} \
         ({:+.2} %)",
        (b / fgr - 1.0) * 100.0
    );
    assert!(b > 0.9 && b < 1.0, "ENDF branching {b}");
    assert!(fgr > 0.9 && fgr < 1.0, "FGR-15 branching {fgr}");
}

/// Doubling every release doubles the dose: the chain is linear in the source,
/// as every term in it is. A wiring check, not physics.

#[test]
fn the_external_dose_is_linear_in_the_release() {
    let releases: Vec<(&str, f64)> = htr10_normal_operation_annual_release()
        .iter()
        .map(|e| (e.nuclide, e.annual_release.get::<becquerel>()))
        .collect();
    let doubled: Vec<(&str, f64)> = releases.iter().map(|&(n, q)| (n, 2.0 * q)).collect();
    let chi = dilution_long_term_no_met(Length::new::<kilometer>(1.5), geometry(), scaled(3.0))[3];
    let (s1, g1) = external_dose(&releases, chi, false);
    let (s2, g2) = external_dose(&doubled, chi, false);
    assert!(((s2 + g2) / (s1 + g1) - 2.0).abs() < 1e-12);
}
