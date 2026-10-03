// SPDX-License-Identifier: GPL-3.0

//! **The ENDF-route S(α,β) cutoff is the evaluation's own E_max, as in OpenMC
//! and NJOY.** GitHub #459.
//!
//! # Methodology
//!
//! OpenMC's library generator takes the S(α,β) upper energy from MF=7/MT=4
//! `B(4)` (`openmc/data/njoy.py`, `energy_max = values[3]`). Transport drops
//! S(α,β) only when `E > energy_max` (`src/material.cpp:866`). For ENDF/B-VIII.0
//! `tsl-HinH2O.endf`, B(4) = 10.00008 eV. NJOY2016's HinH2O ACE built with
//! that deck tops out at 10.0 eV.
//!
//! This test builds `ThermalScattering::from_endf_file` for H in H2O at
//! 293.6 K and checks two things:
//! - the cutoff is the top of THERMR's table run with `emax = B(4)`, where B(4)
//!   is read independently from the tape: at most B(4), and within 0.1 % of
//!   it (NJOY's table stops at 10.0 eV);
//! - a 6 eV neutron (between the old 4 eV cutoff and E_max) is scattered by
//!   the bound law, with a finite inelastic cross section.
//!
//! # Results (2026-09-30)
//!
//! Before the fix the cutoff was a hard-coded 4.0 eV, and the test failed on
//! it (`cutoff 4 != B(4) 10.00008`). After the fix the cutoff is 10.0 eV, the
//! top of THERMR's table at `emax = B(4)` (NJOY's own table also stops at
//! 10.0 eV), and σ_inel(6 eV) = 20.535 b.

use outram_mc_libs::material::thermal::ThermalScattering;

#[test]
fn hinh2o_cutoff_is_the_tapes_b4() {
    let Some(p) = njoy_outram_park_fork::reference_data::reference_endf("tsl-HinH2O.endf") else {
        println!("tsl-HinH2O.endf absent: skipping");
        return;
    };
    // B(4) of MF=7/MT=4, read straight from the tape's second record.
    let text = std::fs::read_to_string(&p).expect("read tape");
    let b4 = text
        .lines()
        .filter(|l| l.len() >= 75 && l[70..72].trim() == "7" && l[72..75].trim() == "4")
        .nth(2)
        .map(|l| parse_endf(&l[33..44]))
        .expect("B(4)");
    let th = ThermalScattering::from_endf_file(p.to_str().unwrap(), 1, 293.6, "H in H2O")
        .expect("builds");
    println!("tape B(4) = {b4} eV; cutoff = {} eV", th.cutoff_ev());
    // THERMR run with emax = B(4) tops its table out at the last grid energy
    // at or below it (10.0 eV here), which is NJOY's last ITIE energy and
    // OpenMC's energy_max. So the cutoff is at most B(4) and within 0.1 % of it.
    assert!(th.cutoff_ev() <= b4 * (1.0 + 1e-12), "cutoff {} above B(4) {b4}", th.cutoff_ev());
    assert!(th.cutoff_ev() >= 0.999 * b4, "cutoff {} is not the tape's B(4) {b4}", th.cutoff_ev());
    let s6 = th.inelastic_xs(6.0);
    println!("sigma_inel(6 eV) = {s6} b");
    assert!(s6 > 1.0, "no bound inelastic cross section at 6 eV");
    let mut seed = 7_u64;
    assert!(th.sample(6.0, &mut seed).is_some(), "6 eV must be inside the S(a,b) treatment");
}

fn parse_endf(s: &str) -> f64 {
    let s = s.trim();
    if let Ok(v) = s.parse::<f64>() {
        return v;
    }
    // ENDF exponent without 'E': 1.000008+1
    let b = s.as_bytes();
    let k = (1..b.len()).rev().find(|&i| b[i] == b'+' || b[i] == b'-').unwrap();
    format!("{}e{}", &s[..k], &s[k..]).parse().unwrap()
}
