// SPDX-License-Identifier: GPL-3.0

//! **The new DLW laws read off real NJOY2016 tables, checked against OpenMC's
//! own reader — GitHub #307 item 3.**
//!
//! # Methodology
//!
//! [`ace_dlw_law_family`](../ace_dlw_law_family/index.html) pins the index
//! arithmetic on hand-built blocks. This pins it on **files NJOY2016 wrote**,
//! against values a **third** code read out of the same bytes:
//! `openmc.data.IncidentNeutron.from_ace` at OpenMC `afa7a14`, whose
//! `AngleEnergy.from_ace` is the routine this port mirrors. Agreement therefore
//! means the two readers agree about the file, not that one of them is
//! self-consistent.
//!
//! The tables are not committed — they are 0.9 to 4.2 MB each and regenerable —
//! so this test is **gated on `OUTRAM_ACE_LAW_DIR`** and prints how to make them
//! when the variable is unset. The deck is `RECONR` then `ACER` at 0 K, the same
//! one `reference-data/ace`'s 0 K tables use:
//!
//! ```text
//! reconr / 20 21 / '<name> pendf 0K' / <MAT> 0 / .001 / 0 /
//! acer   / 20 21 0 24 25 / 1 1 1 .00 0 / '<name> 0K' / <MAT> 0. / 1 1 / /
//! stop
//! ```
//!
//! run over `reference-data/endf/`'s `n-001_H_002-ENDF8.0.endf` (MAT 128),
//! `n-004_Be_009-ENDF8.0.endf` (425), `n-006_C_012-ENDF8.0.endf` (625) and
//! `n-011_Na_023-ENDF8.0.endf` (1125), each left as `<dir>/<H2|Be9|C12|Na23>/tape24`.
//!
//! # Results (2026-09-25, NJOY2016 built from `upstream_source/NJOY2016`)
//!
//! Every value below was read independently by OpenMC's Python reader and by
//! this crate, and they agree exactly (the ACE words are the same `f64`s):
//!
//! | table | MT | law | this port reads | OpenMC reads |
//! |---|---|---|---|---|
//! | H-2 | 16 | 66 | `NPSX = 3`, `APSX = 2.99862`, `Q = -2.225002` MeV | same |
//! | C-12 | 28 | 9 | `U = 15.957` MeV, `theta(E_1) = 0.3` MeV | same |
//! | C-12 | 91 | 9 | `U = 7.8864` MeV, `theta` flat at 0.3 MeV | same |
//! | Na-23 | 16 | 9 | `U = 12.414` MeV, `theta(E_1) = 0.01` MeV | same |
//! | Na-23 | 91 | 9, 9 | `U = 6.1` and `0.47` MeV, `p_k` switching at 12 MeV | same |
//! | Be-9 | 16 | **61** | correlated tabulated, not law 67 | same |
//!
//! Na-23's MT=91 `theta(E)` carries **`INT = 5` (log-log) with a breakpoint at
//! point 9** — the one measured case that makes carrying the interpolation
//! regions load-bearing rather than tidy. Evaluating it lin-lin is a different
//! `theta(E)` and nothing downstream would notice.

use njoy_outram_park_fork::acer::ce_decode::decode_ce;
use njoy_outram_park_fork::acer::ce_laws::{
    ace_phase_space_chi, decode_energy_law, AceEnergyLaw,
};
use njoy_outram_park_fork::acer::read::parse_type1;
use njoy_outram_park_fork::nuclear_data::secondary::FissionSpectrum;

const MEV: f64 = 1.0e6;

/// The directory holding `<name>/tape24`, or `None` with a note saying how to
/// build it.
fn law_dir() -> Option<std::path::PathBuf> {
    match std::env::var("OUTRAM_ACE_LAW_DIR") {
        Ok(d) if !d.is_empty() => Some(std::path::PathBuf::from(d)),
        _ => {
            println!(
                "OUTRAM_ACE_LAW_DIR unset: skipping. Generate the four tables with the \
                 RECONR+ACER deck in this file's module doc (H-2 MAT 128, Be-9 425, \
                 C-12 625, Na-23 1125) and point the variable at the directory holding \
                 <H2|Be9|C12|Na23>/tape24."
            );
            None
        }
    }
}

/// Decode `<dir>/<name>/tape24` and return `(MT, law)` for every reaction that
/// emits neutrons.
fn laws(dir: &std::path::Path, name: &str) -> Vec<(i32, AceEnergyLaw, f64, f64, f64)> {
    let path = dir.join(name).join("tape24");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let t = parse_type1(&text).unwrap_or_else(|e| panic!("{name}: {e}"));
    let d = decode_ce(&t).unwrap_or_else(|e| panic!("{name}: {e}"));
    let n_rx = njoy_outram_park_fork::acer::ce_laws::n_neutron_reactions(&t);
    (0..n_rx)
        .map(|i| {
            let rx = &d.reactions[i];
            let law = decode_energy_law(&t, i)
                .unwrap_or_else(|e| panic!("{name} MT={}: {e}", rx.mt));
            let e_lo = d.energy[rx.threshold_index];
            let e_hi = *d.energy.last().unwrap();
            (rx.mt, law, rx.q_value, e_lo, e_hi)
        })
        .collect()
}

fn evaporation(law: &AceEnergyLaw) -> (&njoy_outram_park_fork::endf::Tab1, f64) {
    match law {
        AceEnergyLaw::Analytic {
            law: 9,
            spectrum: FissionSpectrum::Evaporation { theta, u },
        } => (theta, *u),
        other => panic!("expected ACE law 9 (evaporation), got law {}", other.code()),
    }
}

#[test]
fn c12_and_na23_carry_evaporation_laws_with_openmcs_own_numbers() {
    let Some(dir) = law_dir() else { return };

    let c12 = laws(&dir, "C12");
    let (_, law28, ..) = c12.iter().find(|(mt, ..)| *mt == 28).expect("C-12 MT=28");
    let (theta, u) = evaporation(law28);
    assert!((u - 15.957 * MEV).abs() < 1.0, "C-12 MT=28 U = {u} eV");
    assert!((theta.pairs[0].1 - 0.3 * MEV).abs() < 1.0);
    assert_eq!(theta.pairs.len(), 4, "C-12 MT=28 theta has four points");

    let (_, law91, ..) = c12.iter().find(|(mt, ..)| *mt == 91).expect("C-12 MT=91");
    let (theta, u) = evaporation(law91);
    assert!((u - 7.8864 * MEV).abs() < 1.0, "C-12 MT=91 U = {u} eV");
    assert!(
        theta.pairs.iter().all(|&(_, y)| (y - 0.3 * MEV).abs() < 1.0),
        "C-12 MT=91 theta is flat at 0.3 MeV: {:?}",
        theta.pairs
    );

    let na23 = laws(&dir, "Na23");
    let (_, law16, ..) = na23.iter().find(|(mt, ..)| *mt == 16).expect("Na-23 MT=16");
    let (theta, u) = evaporation(law16);
    assert!((u - 12.414 * MEV).abs() < 1.0, "Na-23 MT=16 U = {u} eV");
    assert!((theta.pairs[0].1 - 0.01 * MEV).abs() < 1.0);
    println!(
        "C-12 MT=28/91 and Na-23 MT=16: ACE law 9, U = 15.957 / 7.8864 / 12.414 MeV"
    );
}

#[test]
fn na23_mt91_is_a_two_law_chain_switching_over_at_12_mev() {
    let Some(dir) = law_dir() else { return };
    let na23 = laws(&dir, "Na23");
    let (_, law, ..) = na23.iter().find(|(mt, ..)| *mt == 91).expect("Na-23 MT=91");
    let AceEnergyLaw::Mixture(parts) = law else {
        panic!("Na-23 MT=91 is an LNW chain, got law {}", law.code());
    };
    assert_eq!(parts.len(), 2);

    // Applicabilities: both on the same three-point grid, histogram (INT=1),
    // swapping at 12 MeV, summing to 1 at every point.
    for (p, _) in parts {
        assert_eq!(p.interp, vec![(3, 1)], "applicability is a histogram");
        let xs: Vec<f64> = p.pairs.iter().map(|&(x, _)| x).collect();
        assert!((xs[0] - 6.1 * MEV).abs() < 1.0);
        assert!((xs[1] - 12.0 * MEV).abs() < 1.0);
        assert!((xs[2] - 20.0 * MEV).abs() < 1.0);
    }
    let y = |k: usize| -> Vec<f64> { parts[k].0.pairs.iter().map(|&(_, y)| y).collect() };
    assert_eq!(y(0), vec![1.0, 0.0, 0.0]);
    assert_eq!(y(1), vec![0.0, 1.0, 1.0]);

    // The links are two different laws, not one written twice: same theta(E),
    // different restriction energy.
    let (t0, u0) = evaporation(&parts[0].1);
    let (t1, u1) = evaporation(&parts[1].1);
    assert!((u0 - 6.1 * MEV).abs() < 1.0, "first link U = {u0} eV");
    assert!((u1 - 0.47 * MEV).abs() < 1.0, "second link U = {u1} eV");
    assert_eq!(t0.pairs, t1.pairs, "both links share theta(E)");
    // The measured case that makes the interpolation regions load-bearing.
    assert_eq!(
        t0.interp,
        vec![(9, 5)],
        "Na-23 MT=91 theta(E) is log-log with a breakpoint at point 9"
    );

    // And it reaches a transport consumer as one MF=5 mixture.
    let Some(FissionSpectrum::Mixture(m)) = law.as_fission_spectrum() else {
        panic!("a chain of MF=5 laws is an MF=5 mixture");
    };
    assert_eq!(m.len(), 2);
    println!("Na-23 MT=91: LNW chain of two evaporations, U = 6.1 and 0.47 MeV, p_k switching at 12 MeV");
}

#[test]
fn h2_mt16_is_phase_space_and_converts_to_a_usable_spectrum() {
    let Some(dir) = law_dir() else { return };
    let h2 = laws(&dir, "H2");
    let (_, law, q, e_lo, e_hi) = h2.iter().find(|(mt, ..)| *mt == 16).expect("H-2 MT=16");
    let AceEnergyLaw::PhaseSpace { npsx, apsx } = law else {
        panic!("H-2 MT=16 is ACE law 66, got law {}", law.code());
    };
    assert_eq!(*npsx, 3);
    assert!((apsx - 2.99862).abs() < 1.0e-9, "APSX = {apsx}");
    // Q from LQR, cross-checked against what OpenMC hands NBodyPhaseSpace.
    assert!(
        (q + 2.225_002 * MEV).abs() < 1.0,
        "H-2 MT=16 Q = {q} eV, OpenMC reads -2225002.0"
    );

    // AWR from the same table's header (H-2: 1.9968).
    let awr = 1.9968;
    let chi = ace_phase_space_chi(*npsx, *apsx, awr, *q, e_lo.max(1.0e-5), *e_hi)
        .expect("three particles and a positive range convert");
    assert!(chi.incident.len() > 10, "{} incident rows", chi.incident.len());
    // `E'_max = (APSX-1)/APSX * (AWR/(AWR+1) * E + Q)`, checked at the top of
    // the grid against the formula written out by hand.
    let e_top = *chi.incident.last().unwrap();
    let want = (apsx - 1.0) / apsx * (awr / (awr + 1.0) * e_top + q);
    let got = *chi.tables.last().unwrap().e_out.last().unwrap();
    assert!(
        (got / want - 1.0).abs() < 1.0e-12,
        "E'_max at {e_top:.4e} eV: {got:.6e} vs {want:.6e}"
    );
    for tab in &chi.tables {
        let c = tab.cdf.last().copied().unwrap_or(0.0);
        assert!((c - 1.0).abs() < 1.0e-9, "phase-space cdf ends at {c}");
        assert!(tab.pdf.iter().all(|p| *p >= 0.0));
        assert!(tab.e_out.windows(2).all(|w| w[1] >= w[0]));
    }
    println!(
        "H-2 MT=16: ACE law 66, NPSX=3 APSX=2.99862 Q=-2.225 MeV, E'_max({:.3e}) = {:.4e} eV",
        e_top, got
    );
}

#[test]
fn be9_mt16_is_law_61_so_law_67_never_appears() {
    let Some(dir) = law_dir() else { return };
    let be9 = laws(&dir, "Be9");
    let (_, law, ..) = be9.iter().find(|(mt, ..)| *mt == 16).expect("Be-9 MT=16");
    // Its evaluation is MF=6 LAW=7 (laboratory angle-energy) — the form an ACE
    // reader might expect as law 67. ACER converts it to law 61 instead, which
    // is why refusing law 67 costs this workspace nothing.
    let AceEnergyLaw::Tabulated { law: 61, rows, .. } = law else {
        panic!("Be-9 MT=16 should be ACE law 61, got law {}", law.code());
    };
    assert!(rows.iter().all(|r| r.cosines.is_some()), "law 61 without cosines");
    println!(
        "Be-9 MT=16: MF=6 LAW=7 became ACE law 61 with {} incident rows — no law 67",
        rows.len()
    );
}

/// Every law in all four tables decodes, and the census is printed rather than
/// assumed. This is the test that would have caught the original gap.
#[test]
fn every_law_in_the_four_tables_decodes() {
    let Some(dir) = law_dir() else { return };
    for name in ["H2", "Be9", "C12", "Na23"] {
        let mut census: std::collections::BTreeMap<String, usize> = Default::default();
        for (_, law, ..) in laws(&dir, name) {
            *census.entry(law.code()).or_insert(0) += 1;
        }
        assert!(!census.is_empty(), "{name}: no neutron-producing reactions");
        println!("{name}: law census {census:?}");
    }
}
