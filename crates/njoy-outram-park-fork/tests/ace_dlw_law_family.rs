// SPDX-License-Identifier: GPL-3.0

//! **Every DLW law upstream reads, read here — GitHub #307 item 3.**
//!
//! # Methodology
//!
//! `AceEnergyLaw` used to decode `{3, 4, 44, 61}` and refuse everything else,
//! on the strength of a census taken over `reference-data/ace`, which holds only
//! U-234/235/238. Upstream's `AngleEnergy.from_ace`
//! (`openmc/data/angle_energy.py:82-115`, OpenMC `afa7a14`) dispatches
//! `{2, 3, 33, 4, 5, 7, 9, 11, 44, 61, 66}`, and NJOY2016 writes four of the
//! missing ones from tapes this repository already holds:
//!
//! | tape | MT | ACE law | measured 2026-09-25 |
//! |---|---|---|---|
//! | H-2 VIII.0 | 16 | 66 (`n`-body phase space) | `NPSX = 3`, `APSX = 2.99862` |
//! | C-12 VIII.0 | 28, 91 | 9 (evaporation) | `U = 15.957`, `7.8864` MeV |
//! | Na-23 VIII.0 | 16 | 9 | `U = 12.414` MeV |
//! | Na-23 VIII.0 | 91 | 9, 9 — an `LNW` chain | applicabilities switch at 12 MeV |
//!
//! Each case here builds the **smallest ACE table that contains the law** —
//! header, `JXS(10)`/`JXS(11)`, and the exact XSS words NJOY writes — and asserts
//! the decoded values, in eV, against the layout upstream reads. A hand-built
//! table is used rather than a generated one because the law bodies are two to
//! nine words: the whole content of the test is the index arithmetic and the unit
//! scaling, and those are visible only when every word is written out. The real
//! tables are checked in `ace_dlw_law_family_vs_njoy2016`, which needs
//! multi-megabyte files and is gated on them.
//!
//! # Results (2026-09-25)
//!
//! All cases pass. Two deliberate refusals are asserted as refusals, with the
//! reason quoted from upstream rather than invented:
//!
//! - **LAW=5** (general evaporation) — upstream dispatches it and then raises
//!   `NotImplementedError` (`energy_distribution.py:192-194`). No held table
//!   carries it: NJOY linearises the one place ENDF LF=5 occurs here (the MT=455
//!   delayed spectra of the three uraniums) into ACE LAW=4, measured as
//!   `{LAW4: 6}` in every reference DNED block.
//! - **LAW=67** (laboratory angle-energy) — upstream refuses it too. NJOY's ACER
//!   converts the ENDF form it comes from (Be-9's MF=6 LAW=7, MT=16) into ACE
//!   LAW=61, measured, so no table generated from `reference-data/endf/` has it.

use njoy_outram_park_fork::acer::ce_laws::{decode_energy_law, AceEnergyLaw};
use njoy_outram_park_fork::acer::read::{AceClass, AceFileType, AceHeader, RawAceTable};
use njoy_outram_park_fork::acer::{jxs, nxs};
use njoy_outram_park_fork::nuclear_data::secondary::FissionSpectrum;

const EV_PER_MEV: f64 = 1.0e6;

/// The smallest table that holds a DLW block: one LDLW locator at XSS word 1,
/// the DLW block from word 2 on. `dlw` is the block exactly as NJOY writes it,
/// its own locators being 1-based *within the block*.
fn table_with_dlw(dlw: &[f64]) -> RawAceTable {
    let mut xss = vec![1.0]; // LDLW(1) = 1 → the first (only) law block
    xss.extend_from_slice(dlw);
    let mut jxs = [0i32; 32];
    jxs[jxs::LDLW] = 1; // 1-based into XSS
    jxs[jxs::DLW] = 2;
    let mut nxs_a = [0i32; 16];
    nxs_a[nxs::NR] = 1;
    RawAceTable {
        file_type: AceFileType::Type1Ascii,
        header: AceHeader {
            zaid: "1002.00c".into(),
            zaid_num: Some(1002.0),
            class: AceClass::ContinuousNeutron,
            awr: 1.9968,
            kt_mev: 0.0,
            date: String::new(),
            comment: String::new(),
            mat_id: String::new(),
            iz: [0; 16],
            aw: [0.0; 16],
            raw_text: [Vec::new(), Vec::new(), Vec::new(), Vec::new()],
        },
        nxs: nxs_a,
        jxs,
        xss,
        xss_is_int: None,
    }
}

/// `[LNW, LAW, IDAT]` plus a flat applicability `p(E) = 1` over `[e_lo, e_hi]`
/// MeV — nine words, so the law's own data always starts at block word 10.
fn header_and_applicability(lnw: f64, law: f64, e_lo: f64, e_hi: f64) -> Vec<f64> {
    vec![lnw, law, 10.0, 0.0, 2.0, e_lo, e_hi, 1.0, 1.0]
}

#[test]
fn law_9_is_an_evaporation_spectrum_in_ev() {
    let mut dlw = header_and_applicability(0.0, 9.0, 1.0, 20.0);
    // theta(E): NR = 0, NE = 3, E [MeV], theta [MeV]; then U [MeV].
    dlw.extend_from_slice(&[0.0, 3.0, 1.0, 10.0, 20.0, 0.5, 0.6, 0.7, 0.8]);
    let t = table_with_dlw(&dlw);

    let law = decode_energy_law(&t, 0).expect("law 9 decodes");
    assert_eq!(law.code(), "9");
    let AceEnergyLaw::Analytic {
        law: code,
        spectrum: FissionSpectrum::Evaporation { theta, u },
    } = &law
    else {
        panic!("expected an evaporation spectrum, got {law:?}");
    };
    assert_eq!(*code, 9);
    assert_eq!(
        theta.pairs,
        vec![
            (1.0 * EV_PER_MEV, 0.5 * EV_PER_MEV),
            (10.0 * EV_PER_MEV, 0.6 * EV_PER_MEV),
            (20.0 * EV_PER_MEV, 0.7 * EV_PER_MEV),
        ],
        "theta is an energy: both axes scale from MeV to eV"
    );
    assert_eq!(*u, 0.8 * EV_PER_MEV);
    // The same law, reachable as an MF=5 spectrum for the transport crate.
    assert!(matches!(
        law.as_fission_spectrum(),
        Some(FissionSpectrum::Evaporation { .. })
    ));
}

#[test]
fn law_7_is_a_maxwellian_and_keeps_its_interpolation_regions() {
    let mut dlw = header_and_applicability(0.0, 7.0, 1.0, 20.0);
    // NR = 1 with (NBT, INT) = (3, 1): a histogram region over all three points.
    dlw.extend_from_slice(&[1.0, 3.0, 1.0, 3.0, 1.0, 10.0, 20.0, 0.5, 0.6, 0.7, 0.25]);
    let t = table_with_dlw(&dlw);

    let AceEnergyLaw::Analytic {
        law: 7,
        spectrum: FissionSpectrum::Maxwell { theta, u },
    } = decode_energy_law(&t, 0).expect("law 7 decodes")
    else {
        panic!("expected a Maxwellian");
    };
    assert_eq!(
        theta.interp,
        vec![(3, 1)],
        "a histogram region must survive the read: evaluating it lin-lin is a \
         different theta(E)"
    );
    assert_eq!(theta.pairs.len(), 3);
    assert_eq!(u, 0.25 * EV_PER_MEV);
}

#[test]
fn law_11_scales_watt_b_by_the_inverse_of_a() {
    let mut dlw = header_and_applicability(0.0, 11.0, 1.0, 20.0);
    // a(E): NR = 0, NE = 2, E [MeV], a [MeV]
    dlw.extend_from_slice(&[0.0, 2.0, 1.0, 20.0, 0.9, 1.1]);
    // b(E): NR = 0, NE = 2, E [MeV], b [1/MeV]
    dlw.extend_from_slice(&[0.0, 2.0, 1.0, 20.0, 3.0, 4.0]);
    // U [MeV]
    dlw.push(0.1);
    let t = table_with_dlw(&dlw);

    let AceEnergyLaw::Analytic {
        law: 11,
        spectrum: FissionSpectrum::WattEnergyDependent { a, b, u },
    } = decode_energy_law(&t, 0).expect("law 11 decodes")
    else {
        panic!("expected an energy-dependent Watt");
    };
    assert_eq!(a.pairs[0], (1.0 * EV_PER_MEV, 0.9 * EV_PER_MEV));
    assert_eq!(a.pairs[1], (20.0 * EV_PER_MEV, 1.1 * EV_PER_MEV));
    // b is an inverse energy: 3 MeV^-1 = 3e-6 eV^-1. Scaling it the same way as
    // `a` would harden the spectrum by twelve orders of magnitude and would not
    // fail anything that only checks the read.
    assert_eq!(b.pairs[0], (1.0 * EV_PER_MEV, 3.0 / EV_PER_MEV));
    assert_eq!(b.pairs[1], (20.0 * EV_PER_MEV, 4.0 / EV_PER_MEV));
    assert_eq!(u, 0.1 * EV_PER_MEV);
}

#[test]
fn law_66_carries_npsx_and_apsx_and_refuses_to_pose_as_a_spectrum() {
    let mut dlw = header_and_applicability(0.0, 66.0, 3.339, 150.0);
    // The whole law: NPSX, APSX. H-2's own numbers.
    dlw.extend_from_slice(&[3.0, 2.99862]);
    let t = table_with_dlw(&dlw);

    let law = decode_energy_law(&t, 0).expect("law 66 decodes");
    assert!(matches!(
        law,
        AceEnergyLaw::PhaseSpace {
            npsx: 3,
            apsx: 2.99862
        }
    ));
    assert_eq!(law.code(), "66");
    assert!(
        law.as_fission_spectrum().is_none(),
        "phase space is not an MF=5 spectrum: it needs Q and the target mass \
         before it is a distribution at all"
    );
}

#[test]
fn law_2_is_a_discrete_line_and_law_33_is_law_3() {
    let mut dlw = header_and_applicability(0.0, 2.0, 1.0, 20.0);
    dlw.extend_from_slice(&[2.0, 4.438]); // LP = 2 (primary), EG = 4.438 MeV
    let t = table_with_dlw(&dlw);
    let AceEnergyLaw::DiscretePhoton { lp, eg } =
        decode_energy_law(&t, 0).expect("law 2 decodes")
    else {
        panic!("expected a discrete photon");
    };
    assert_eq!(lp, 2);
    assert!((eg - 4.438 * EV_PER_MEV).abs() < 1.0);

    let mut dlw = header_and_applicability(0.0, 33.0, 1.0, 20.0);
    dlw.extend_from_slice(&[4.8, 0.84]); // (A+1)/A |Q| [MeV], (A/(A+1))^2
    let t = table_with_dlw(&dlw);
    let law = decode_energy_law(&t, 0).expect("law 33 decodes as law 3");
    assert!(matches!(law, AceEnergyLaw::TwoBodyLevel { .. }));
    let AceEnergyLaw::TwoBodyLevel { ldat1, ldat2 } = law else {
        unreachable!()
    };
    assert!((ldat1 - 4.8 * EV_PER_MEV).abs() < 1.0);
    assert_eq!(ldat2, 0.84);
}

#[test]
fn an_lnw_chain_becomes_a_mixture_carrying_each_applicability() {
    // Two law-9 links, as Na-23's MT=91 has: the first applies below 12 MeV,
    // the second above. Block word layout, 1-based within the DLW block:
    //   1..11   link 1 header (3) + applicability over three energies (8)
    //   12..18  link 1 theta(E) (6) + U (1)
    //   19..29  link 2 header + applicability (p: 0 → 1)
    //   30..36  link 2 theta(E) + U
    let mut dlw = vec![19.0, 9.0, 12.0, 0.0, 3.0, 6.1, 12.0, 20.0, 1.0, 0.0, 0.0];
    dlw.extend_from_slice(&[0.0, 2.0, 6.1, 20.0, 1.53, 1.74, 0.61]);
    assert_eq!(dlw.len(), 18, "link 1 must end on block word 18");
    dlw.extend_from_slice(&[0.0, 9.0, 30.0, 0.0, 3.0, 6.1, 12.0, 20.0, 0.0, 1.0, 1.0]);
    dlw.extend_from_slice(&[0.0, 2.0, 6.1, 20.0, 1.53, 1.74, 0.047]);
    assert_eq!(dlw.len(), 36, "link 2 must end on block word 36");
    let t = table_with_dlw(&dlw);

    let law = decode_energy_law(&t, 0).expect("a two-law chain decodes");
    assert_eq!(law.code(), "9 (x2)");
    let AceEnergyLaw::Mixture(parts) = &law else {
        panic!("expected a mixture, got {law:?}");
    };
    assert_eq!(parts.len(), 2);
    // The applicabilities are what make the chain a distribution rather than two
    // answers to one question: p_1 falls 1 → 0 and p_2 rises 0 → 1 over the same
    // grid, and they sum to 1 at every point.
    let p1: Vec<f64> = parts[0].0.pairs.iter().map(|&(_, y)| y).collect();
    let p2: Vec<f64> = parts[1].0.pairs.iter().map(|&(_, y)| y).collect();
    assert_eq!(p1, vec![1.0, 0.0, 0.0]);
    assert_eq!(p2, vec![0.0, 1.0, 1.0]);
    for (a, b) in p1.iter().zip(p2.iter()) {
        assert!((a + b - 1.0).abs() < 1e-12, "sum_k p_k(E) = 1");
    }
    assert_eq!(parts[0].0.pairs[1].0, 12.0 * EV_PER_MEV);
    // The two links differ in U, so the mixture is not two copies of one law.
    let u_of = |law: &AceEnergyLaw| match law {
        AceEnergyLaw::Analytic {
            spectrum: FissionSpectrum::Evaporation { u, .. },
            ..
        } => *u,
        other => panic!("expected evaporation, got {other:?}"),
    };
    assert_eq!(u_of(&parts[0].1), 0.61 * EV_PER_MEV);
    assert_eq!(u_of(&parts[1].1), 0.047 * EV_PER_MEV);

    // And it reaches the transport crate as one MF=5 mixture.
    let Some(FissionSpectrum::Mixture(m)) = law.as_fission_spectrum() else {
        panic!("a chain of MF=5 laws is an MF=5 mixture");
    };
    assert_eq!(m.len(), 2);
}

#[test]
fn law_5_and_law_67_are_refused_with_upstreams_own_reason() {
    for (code, must_mention) in [(5.0, "NotImplementedError"), (67.0, "LAW=61")] {
        let mut dlw = header_and_applicability(0.0, code, 1.0, 20.0);
        dlw.extend_from_slice(&[0.0, 2.0, 1.0, 20.0, 0.5, 0.6, 0.1]);
        let t = table_with_dlw(&dlw);
        let err = decode_energy_law(&t, 0).expect_err("must refuse, not guess");
        let msg = format!("{err}");
        assert!(
            msg.contains(must_mention),
            "the refusal must say what upstream does with LAW={code}: {msg}"
        );
    }
}

#[test]
fn an_unknown_law_names_the_set_upstream_accepts() {
    let mut dlw = header_and_applicability(0.0, 22.0, 1.0, 20.0);
    dlw.extend_from_slice(&[0.0, 2.0, 1.0, 20.0, 0.5, 0.6]);
    let t = table_with_dlw(&dlw);
    let msg = format!(
        "{}",
        decode_energy_law(&t, 0).expect_err("LAW=22 is not read by upstream either")
    );
    assert!(msg.contains("22"), "{msg}");
    assert!(msg.contains("66"), "the message must name the accepted set: {msg}");
}
