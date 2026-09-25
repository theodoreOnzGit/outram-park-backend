// SPDX-License-Identifier: GPL-3.0

//! **The ACE laws added for GitHub #307 item 3 reach the transport kernel.**
//!
//! # Methodology
//!
//! `njoy-outram-park-fork`'s `ace_dlw_law_family_vs_njoy2016` checks that laws
//! 9, 66 and the `LNW` chain are *read* correctly off NJOY2016's own tables.
//! Reading is half the claim: before this change `Nuclide::from_ace` refused
//! every one of them, so a table carrying one could not be built at all, and a
//! law that is read but not wired reaches no neutron.
//!
//! Each case here builds the nuclide through the ordinary `Nuclide::from_ace`
//! path and samples the emission the transport kernel would sample, then
//! compares against an oracle that is **not** the sampler: a closed-form moment
//! of the law for the evaporation case, and the law's own kinematic ceiling
//! `E'_max(E)` for phase space. The Na-23 case is sharper than a moment — its
//! two links have different restriction energies, so the *support* of the
//! distribution moves when the applicability switches, and sampling either side
//! of 12 MeV tests the switch itself rather than an average over it.
//!
//! The tables are regenerable rather than committed; see
//! `njoy-outram-park-fork/tests/ace_dlw_law_family_vs_njoy2016.rs` for the NJOY
//! deck, and set `OUTRAM_ACE_LAW_DIR` to the directory holding
//! `<H2|C12|Na23>/tape24`.
//!
//! # Results (2026-09-25)
//!
//! | case | sampled | oracle | |
//! |---|---|---|---|
//! | C-12 MT=91 evaporation, `<E'>` at 12 MeV | see the test's own print | closed-form truncated moment | within 4 sigma |
//! | Na-23 MT=91 at 10 MeV | `max E' <= 3.900 MeV` | `E - U_1`, `U_1 = 6.1` MeV | hard bound |
//! | Na-23 MT=91 at 15 MeV | `max E' > 3.9 MeV` | second link, `U_2 = 0.47` MeV | the switch fires |
//! | H-2 MT=16 phase space at 14 MeV | `max E' <= E'_max` | `(APSX-1)/APSX (AWR/(AWR+1) E + Q)` | hard bound |

use njoy_outram_park_fork::nuclear_data::secondary::FissionSpectrum;
use outram_mc_libs::geometry::position::Direction;
use outram_mc_libs::material::nuclide::Nuclide;

const MEV: f64 = 1.0e6;

fn nuclide(name: &str, label: &str) -> Option<Nuclide> {
    let dir = match std::env::var("OUTRAM_ACE_LAW_DIR") {
        Ok(d) if !d.is_empty() => std::path::PathBuf::from(d),
        _ => {
            println!(
                "OUTRAM_ACE_LAW_DIR unset: skipping. See this file's module doc for the \
                 NJOY deck that builds the tables."
            );
            return None;
        }
    };
    let path = dir.join(name).join("tape24");
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let raw = njoy_outram_park_fork::acer::read::parse_type1(&text)
        .unwrap_or_else(|e| panic!("{name}: {e}"));
    Some(Nuclide::from_ace(&raw, label).unwrap_or_else(|e| panic!("{name}: from_ace: {e}")))
}

/// `int_0^y x^n e^-x dx` for n = 1, 2 — the two moments the evaporation density
/// needs, in closed form so the oracle is not another numerical integral.
fn gamma_lower(n: u32, y: f64) -> f64 {
    match n {
        1 => 1.0 - (1.0 + y) * (-y).exp(),
        2 => 2.0 - (y * y + 2.0 * y + 2.0) * (-y).exp(),
        _ => unreachable!(),
    }
}

#[test]
fn c12_mt91_evaporation_from_ace_samples_its_own_mean() {
    let Some(n) = nuclide("C12", "C12") else { return };
    let law = n
        .uncorrelated_law(91)
        .expect("C-12 MT=91 is ACE law 9, so from_ace must place it as MF=4+5");
    let FissionSpectrum::Evaporation { theta, u } = &law.energy else {
        panic!("expected an evaporation law, got {:?}", law.energy);
    };
    assert_eq!(law.yield_n, 1, "MT=91 emits one neutron");
    assert_eq!(law.lct, 1, "laboratory frame");

    let e_in = 12.0 * MEV;
    let t = njoy_outram_park_fork::endf::interp::eval_tab1(e_in, &theta.interp, &theta.pairs)
        .expect("theta(E) covers 12 MeV");
    let y = (e_in - u) / t;
    // <E'> = theta * gamma(3, y) / gamma(2, y) for f(E') ∝ E' exp(-E'/theta)
    // truncated at E - U. The sampler never sees this expression.
    let want = t * gamma_lower(2, y) / gamma_lower(1, y);

    let mut seed = 20_260_925_u64;
    let dir = Direction::new(0.0, 0.0, 1.0);
    let n_hist = 200_000;
    let (mut sum, mut sum_sq, mut max) = (0.0, 0.0, 0.0_f64);
    for _ in 0..n_hist {
        let (e_out, _) = n.sample_inelastic_emission(91, e_in, dir, -7.8864 * MEV, &mut seed);
        sum += e_out;
        sum_sq += e_out * e_out;
        max = max.max(e_out);
    }
    let mean = sum / n_hist as f64;
    let sd = ((sum_sq / n_hist as f64 - mean * mean) / n_hist as f64).sqrt();
    let sigma = (mean - want).abs() / sd;
    println!(
        "C-12 MT=91 at 12 MeV: <E'> = {:.5e} +/- {:.1e} eV vs closed form {:.5e} ({:.2} sigma); \
         theta = {:.4e} eV, U = {:.4e} eV, max E' = {:.4e}",
        mean, sd, want, sigma, t, u, max
    );
    assert!(sigma < 4.0, "sampled mean is {sigma:.2} sigma from the law's own moment");
    assert!(
        max <= e_in - u + 1.0,
        "the restriction energy is not being applied: max E' = {max:.4e} > E - U"
    );
}

#[test]
fn na23_mt91_switches_law_at_12_mev_and_the_support_moves() {
    let Some(n) = nuclide("Na23", "Na23") else { return };
    let law = n.uncorrelated_law(91).expect("Na-23 MT=91 is an LNW chain of two law 9s");
    let FissionSpectrum::Mixture(parts) = &law.energy else {
        panic!("expected a mixture, got {:?}", law.energy);
    };
    assert_eq!(parts.len(), 2);

    let sample_max = |e_in: f64, seed0: u64| -> f64 {
        let mut seed = seed0;
        let dir = Direction::new(0.0, 0.0, 1.0);
        (0..50_000)
            .map(|_| n.sample_inelastic_emission(91, e_in, dir, -0.44 * MEV, &mut seed).0)
            .fold(0.0_f64, f64::max)
    };

    // Below the switch only the first link applies (p = 1, 0), so no neutron can
    // leave with more than E - U_1 = 10 - 6.1 = 3.9 MeV.
    let max_lo = sample_max(10.0 * MEV, 1);
    assert!(
        max_lo <= 3.9 * MEV + 1.0,
        "below 12 MeV the U = 6.1 MeV link must bound E': max = {max_lo:.4e} eV"
    );
    // Above it the second link (U = 0.47 MeV) takes over and the support opens
    // up to 14.53 MeV. If the applicability were ignored — or both links were
    // read as the same law — this would still be bounded by 8.9 MeV.
    let max_hi = sample_max(15.0 * MEV, 2);
    assert!(
        max_hi > 9.0 * MEV,
        "above 12 MeV the U = 0.47 MeV link must be reachable: max = {max_hi:.4e} eV"
    );
    assert!(
        max_hi <= 15.0 * MEV - 0.47 * MEV + 1.0,
        "even the second link is restricted: max = {max_hi:.4e} eV"
    );
    println!(
        "Na-23 MT=91: max E' = {:.4e} eV at 10 MeV (bound 3.900e6), {:.4e} eV at 15 MeV \
         (bound 1.453e7)",
        max_lo, max_hi
    );
}

#[test]
fn h2_mt16_phase_space_from_ace_respects_its_kinematic_ceiling() {
    let Some(n) = nuclide("H2", "H2") else { return };
    let law = n
        .continuum_law(16)
        .expect("H-2 MT=16 is ACE law 66, so from_ace must place it as a continuum emission");
    assert_eq!(law.branches.len(), 1);
    assert!(
        law.cm_frame,
        "H-2 MT=16 has TY = -2: the phase-space distribution is centre-of-mass"
    );

    // E'_max from the law's own parameters, in the centre-of-mass frame the
    // table is written in.
    let (npsx, apsx, awr, q) = (3.0, 2.99862, 1.9968, -2.225_002 * MEV);
    assert_eq!(npsx as i32, 3);
    let e_in = 14.0 * MEV;
    let e_max_cm = (apsx - 1.0) / apsx * (awr / (awr + 1.0) * e_in + q);

    let mut seed = 7_u64;
    let dir = Direction::new(0.0, 0.0, 1.0);
    let (mut sum, mut max) = (0.0, 0.0_f64);
    let n_hist = 50_000;
    for _ in 0..n_hist {
        let (e_out, _) = n.sample_inelastic_emission(16, e_in, dir, q, &mut seed);
        sum += e_out;
        max = max.max(e_out);
    }
    let mean = sum / n_hist as f64;
    // The sampled energy is transformed CM -> lab, so the ceiling to check is
    // the kinematic one: a neutron emitted forwards adds the centre-of-mass
    // speed, `E_lab,max = (sqrt(E'_max,cm) + sqrt(E/(A+1)^2))^2`. That is a
    // sharper bound than the incident energy and it is the one that fails if
    // the CM->lab transform is applied to the wrong quantity.
    let e_cm_motion = e_in / (awr + 1.0).powi(2);
    let lab_ceiling = (e_max_cm.sqrt() + e_cm_motion.sqrt()).powi(2);
    assert!(
        max <= lab_ceiling * (1.0 + 1.0e-9),
        "lab E' = {max:.4e} exceeds the kinematic ceiling {lab_ceiling:.4e}"
    );
    assert!(max < e_in, "lab E' = {max:.4e} exceeds the incident energy");
    assert!(
        mean > 0.1 * e_max_cm && mean < e_in,
        "<E'> = {mean:.4e} eV is not a phase-space spectrum (E'_max,cm = {e_max_cm:.4e})"
    );
    println!(
        "H-2 MT=16 at 14 MeV: <E'> = {:.4e} eV, max = {:.4e} eV, E'_max(cm) = {:.4e} eV, \
         lab ceiling = {:.4e} eV",
        mean, max, e_max_cm, lab_ceiling
    );
}
