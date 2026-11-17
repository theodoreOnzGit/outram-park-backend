//! Tests for gh:#1628's reader and collapse. Predictions in each doc
//! comment; measured values recorded 2026-11-17.

use super::*;
use crate::endf::records::Cont;

fn tab(pairs: &[(f64, f64)], law: u32) -> Tab1 {
    let head = Cont { c1: 0.0, c2: 0.0, l1: 0, l2: 0, n1: 1, n2: pairs.len() as i32 };
    Tab1 { head, interp: vec![(pairs.len() as u32, law)], pairs: pairs.to_vec() }
}

fn state(data: StateData) -> ProductionState {
    ProductionState { mt: 102, zap: 30069, lfs: 0, isomer: 0, level_energy_ev: 0.0, data }
}

/// `σ = 1 + 2t`, `y = t` on `x = 1 + 2t`, one flat group `[1, 3]`:
/// `∫yσ / ∫σ = (7/3) / 4 = 7/12` exactly (Simpson is exact for the
/// quadratic). Prediction: equal to 1e-15.
#[test]
fn collapse_is_exact_for_a_linear_times_linear_product() {
    let sigma = [(1.0, 1.0), (3.0, 3.0)];
    let s = state(StateData::Multiplicity(tab(&[(1.0, 0.0), (3.0, 1.0)], 2)));
    let r = collapse(&[s], &sigma, &[1.0, 3.0], &[5.0]).unwrap();
    assert!((r[0].branching_ratio - 7.0 / 12.0).abs() < 1e-15, "{}", r[0].branching_ratio);
}

/// A histogram multiplicity (INT = 1) stepping 0.2 → 0.8 at x = 2 on a
/// constant σ over `[1, 3]`: BR = 0.5. Two groups with fluxes 1 and 3:
/// `(0.2·1 + 0.8·3)/4 = 0.65`. MF=10 at half of σ: 0.5 in any spectrum.
#[test]
fn collapse_handles_histograms_groups_and_mf10() {
    let sigma = [(1.0, 2.0), (3.0, 2.0)];
    let h = state(StateData::Multiplicity(tab(&[(1.0, 0.2), (2.0, 0.8), (3.0, 0.8)], 1)));
    let one = collapse(&[h.clone()], &sigma, &[1.0, 3.0], &[1.0]).unwrap();
    assert!((one[0].branching_ratio - 0.5).abs() < 1e-15);
    let two = collapse(&[h], &sigma, &[1.0, 2.0, 3.0], &[1.0, 3.0]).unwrap();
    assert!((two[0].branching_ratio - 0.65).abs() < 1e-15);
    let x = state(StateData::CrossSection(tab(&[(1.0, 1.0), (3.0, 1.0)], 2)));
    let r = collapse(&[x], &sigma, &[1.0, 2.0, 3.0], &[7.0, 0.1]).unwrap();
    assert!((r[0].branching_ratio - 0.5).abs() < 1e-15);
    assert!(collapse(&[], &sigma, &[3.0, 1.0], &[1.0]).is_err());
    let bad = state(StateData::Multiplicity(tab(&[(1.0, 0.2), (3.0, 0.8)], 5)));
    assert!(collapse(&[bad], &sigma, &[1.0, 3.0], &[1.0]).is_err(), "log-log refused by name");
}

#[cfg(feature = "std")]
fn tape(file: &str) -> Option<Tape> {
    let p = crate::reference_data::reference_endf_or_skip(file, "isomeric_branching")?;
    Some(Tape::from_bytes(&std::fs::read(p).ok()?).unwrap())
}

/// Zn-68 (n,γ), ENDF/B-VIII.0: MF=8 lists Zn-69 (LFS 0) and Zn-69m (LFS 1,
/// 436.577 keV), both LMF = 9. MF=9 is constant 0.934 / 0.066 from 1e-5 to
/// 1 eV, so a flat thermal group there must give those numbers exactly,
/// whatever σ does. Prediction: 0.934 and 0.066 to 1e-14, sum 1.
///
/// Measured beside it: a flat 0.1–20 MeV group on RECONR's resonance-
/// reconstructed MT=102 gives **ground 0.755509, Zn-69m 0.244491** (sum 1 to
/// 1e-15; 2026-11-17), against 0.934 / 0.066 thermal. One preset ratio
/// would be wrong by a factor of 3.7 in the isomer for one spectrum or the
/// other. Gated only as `m1 > 0.066` and the sum; the value is a record.
#[cfg(feature = "std")]
#[test]
fn zn68_capture_branching_from_mf9() {
    let Some(t) = tape("n-030_Zn_068-ENDF8.0.endf") else { return };
    let mat = crate::process::single_material(&t).unwrap();
    let (states, skipped) = production_states(&t, mat, 102).unwrap();
    assert!(skipped.is_empty());
    let names: Vec<String> = states.iter().map(|s| s.product_name().unwrap()).collect();
    assert_eq!(names, vec!["Zn69", "Zn69_m1"]);
    assert!((states[1].level_energy_ev - 4.36577e5).abs() < 1.0);

    let rec = crate::process::reconstruct(&t, &crate::process::CrossSectionSettings::default()).unwrap();
    let mt102 = crate::endf::mt::MtReaction::from_any(102);
    let sigma = &rec.reconr.sections.iter().find(|s| s.mt == mt102).unwrap().pairs;
    let thermal = collapse(&states, sigma, &[1e-5, 1.0], &[1.0]).unwrap();
    assert!((thermal[0].branching_ratio - 0.934).abs() < 1e-14, "{thermal:?}");
    assert!((thermal[1].branching_ratio - 0.066).abs() < 1e-14, "{thermal:?}");

    let fast = collapse(&states, sigma, &[1e5, 2e7], &[1.0]).unwrap();
    let sum: f64 = fast.iter().map(|s| s.branching_ratio).sum();
    std::println!("Zn-68 (n,g), flat 0.1-20 MeV: ground {:.6}, m1 {:.6}, sum {sum:.15}", fast[0].branching_ratio, fast[1].branching_ratio);
    assert!(fast[1].branching_ratio > 0.066, "the isomer fraction grows with energy in MF=9");
    assert!((sum - 1.0).abs() < 1e-12);
}

/// U-235 MT=4, LMF = 10: the ground state and U-235m at 77 eV.
#[cfg(feature = "std")]
#[test]
fn u235_inelastic_isomer_reads_from_mf10() {
    let Some(t) = tape("n-092_U_235-ENDF8.0.endf") else { return };
    let (states, _) = production_states(&t, 9228, 4).unwrap();
    let names: Vec<String> = states.iter().map(|s| s.product_name().unwrap()).collect();
    assert_eq!(names, vec!["U235", "U235_m1"]);
    assert!(matches!(states[1].data, StateData::CrossSection(_)));
    assert_eq!(states[1].level_energy_ev, 77.0);
    assert!(production_states(&t, 9228, 102).unwrap().0.is_empty(), "no MF=8 for capture: reported empty");
}

/// Zn-68 (n,γ) applied to a three-nuclide chain: a thermal spectrum writes
/// 0.934 / 0.066, a fast one 0.755509 / 0.244491 (the collapse above), and
/// the chain still exports and parses. A chain without Zn-69m reports it.
#[cfg(feature = "std")]
#[test]
fn zn68_ratios_written_to_a_chain() {
    use crate::hdf5::depletion_chain_ops::export_to_xml;
    use crate::hdf5::depletion_chain_xml::DepletionChainXml;
    let Some(t) = tape("n-030_Zn_068-ENDF8.0.endf") else { return };
    let mat = crate::process::single_material(&t).unwrap();
    let (states, _) = production_states(&t, mat, 102).unwrap();
    let rec = crate::process::reconstruct(&t, &crate::process::CrossSectionSettings::default()).unwrap();
    let mt102 = crate::endf::mt::MtReaction::from_any(102);
    let sigma = rec.reconr.sections.iter().find(|s| s.mt == mt102).unwrap().pairs.clone();
    let xml = "<depletion_chain>\n  <nuclide name=\"Zn68\" reactions=\"1\">\n    <reaction type=\"(n,gamma)\" Q=\"6482070.0\" target=\"Zn69\"/>\n  </nuclide>\n  <nuclide name=\"Zn69\" reactions=\"0\"/>\n  <nuclide name=\"Zn69_m1\" reactions=\"0\"/>\n</depletion_chain>\n";
    let base = DepletionChainXml::parse(xml).unwrap();

    let mut c = base.clone();
    let a = apply_to_chain(&mut c, "Zn68", "(n,gamma)", &states, &sigma, &[1e-5, 1.0], &[1.0]).unwrap();
    assert!(a.not_in_chain.is_empty());
    let r = &c.get("Zn68").unwrap().reactions;
    assert_eq!(r.len(), 2);
    assert!((r[0].branching_ratio - 0.934).abs() < 1e-14 && (r[1].branching_ratio - 0.066).abs() < 1e-14);
    assert_eq!(r[0].q_ev, 6482070.0);
    assert_eq!(DepletionChainXml::parse(&export_to_xml(&c)).unwrap(), c);

    let mut f = base.clone();
    apply_to_chain(&mut f, "Zn68", "(n,gamma)", &states, &sigma, &[1e5, 2e7], &[1.0]).unwrap();
    let m1 = f.get("Zn68").unwrap().reactions.iter().find(|x| x.target.as_deref() == Some("Zn69_m1")).unwrap().branching_ratio;
    assert!((m1 - 0.244491).abs() < 1e-6, "{m1}");

    let short = DepletionChainXml::parse(&xml.replace("  <nuclide name=\"Zn69_m1\" reactions=\"0\"/>\n", "")).unwrap();
    let mut s = short.clone();
    let res = apply_to_chain(&mut s, "Zn68", "(n,gamma)", &states, &sigma, &[1e-5, 1.0], &[1.0]);
    // Ground alone at 0.934 with the ground among the products: upstream's
    // strict sum check refuses it rather than renormalising.
    assert!(res.is_err(), "{res:?}");
}
