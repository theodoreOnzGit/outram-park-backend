// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK.
//
// OUTRAM PARK is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the
// Free Software Foundation, either version 3 of the License, or (at your
// option) any later version.
//
// OUTRAM PARK is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
// General Public License for more details.
//
// You should have received a copy of the GNU General Public License along
// with OUTRAM PARK.  If not, see <https://www.gnu.org/licenses/>.

//! **Differential comparison against PRISMS-Fatigue, PRISMS-Plasticity and
//! MOOSE (gh:#1434, epic gh:#1336).**
//!
//! Methodology, routes, predictions (committed before any run) and the
//! measured tables are in
//! `verification_and_validation/prisms_moose_differential_gh1434/`
//! (`PREDICTIONS.md`, `RESULTS.md`).  Each test compares the port against a
//! reference file produced by the upstream code (PRISMS-Fatigue: the real
//! Python, run unmodified; PRISMS-Plasticity and MOOSE: verbatim-transcribed
//! kernels in standalone C++ drivers, `UPSTREAM`-marked lines grep-checked).
//! Floating-point values travel as IEEE-754 bit patterns in hex, so a parse
//! can never be the difference.
//!
//! Upstream pins: PRISMS-Fatigue `2c8fc9a2`, PRISMS-Plasticity `ffdf4eb6`,
//! MOOSE `fe80d24f`.  Data provenance: all inputs are synthetic and generated
//! by the drivers from a fixed LCG; no external data is used.

use farrer_park::crystal::{CrystalState, SlipFamily, MAX_SLIP_SYSTEMS};
use farrer_park::fatigue::{region_fip, CycleExtremes, FatemiSocie};
use std::collections::BTreeMap;

const FIP_REFERENCE: &str =
    include_str!("../verification_and_validation/prisms_moose_differential_gh1434/reference_fip.txt");

fn f(hex: &str) -> f64 {
    f64::from_bits(u64::from_str_radix(hex, 16).expect("hex bit pattern"))
}

/// Distance in units in the last place between two finite doubles of the same
/// sign (bit-pattern difference); `u64::MAX` for differing signs or non-finite.
fn ulps(a: f64, b: f64) -> u64 {
    if a.to_bits() == b.to_bits() {
        return 0;
    }
    if !a.is_finite() || !b.is_finite() || (a.is_sign_negative() != b.is_sign_negative() && a != 0.0 && b != 0.0)
    {
        return u64::MAX;
    }
    a.to_bits().abs_diff(b.to_bits())
}

/// Tally of a comparison: how many values were bit-identical, the worst ulp
/// distance and the worst relative difference.
#[derive(Debug, Default, Clone, Copy)]
struct Tally {
    n: usize,
    identical: usize,
    worst_ulps: u64,
    worst_rel: f64,
}

impl Tally {
    fn add(&mut self, port: f64, upstream: f64) {
        self.n += 1;
        let u = ulps(port, upstream);
        if u == 0 {
            self.identical += 1;
        }
        self.worst_ulps = self.worst_ulps.max(u);
        let denom = upstream.abs().max(f64::MIN_POSITIVE);
        self.worst_rel = self.worst_rel.max((port - upstream).abs() / denom);
    }
}

struct FipReference {
    k: f64,
    sigma_y: f64,
    /// (element, system) -> (slip at tension, slip at compression, normal stress)
    inputs: BTreeMap<(usize, usize), (f64, f64, f64)>,
    fips: BTreeMap<(usize, usize), f64>,
    bands: BTreeMap<(usize, usize, usize), Vec<usize>>,
    band_results: BTreeMap<(usize, usize, usize), f64>,
    parse_differences: usize,
}

fn load_fip_reference() -> FipReference {
    let mut r = FipReference {
        k: 0.0,
        sigma_y: 0.0,
        inputs: BTreeMap::new(),
        fips: BTreeMap::new(),
        bands: BTreeMap::new(),
        band_results: BTreeMap::new(),
        parse_differences: 0,
    };
    for line in FIP_REFERENCE.lines() {
        let t: Vec<&str> = line.split_whitespace().collect();
        let u = |i: usize| t[i].parse::<usize>().unwrap();
        match t[0] {
            "P" => {
                r.k = f(t[1]);
                r.sigma_y = f(t[2]);
            }
            "I" => {
                r.inputs.insert((u(1), u(2)), (f(t[3]), f(t[4]), f(t[5])));
            }
            "F" => {
                r.fips.insert((u(1), u(2)), f(t[3]));
            }
            "D" => {
                let members = t[4].split(',').map(|s| s.parse::<usize>().unwrap()).collect();
                r.bands.insert((u(1), u(2), u(3)), members);
            }
            "B" => {
                r.band_results.insert((u(1), u(2), u(3)), f(t[4]));
            }
            "N" => r.parse_differences = u(1),
            _ => panic!("unknown reference line {line}"),
        }
    }
    r
}

/// Case 2 (gh:#1434): the Fatemi-Socie FIP, per element and slip system, port
/// against the real `calculate_FIPs.import_PRISMS_data`.
///
/// Methodology: 27 elements x 12 systems, synthetic slips (about 1e-3) and
/// plane-normal stresses (+-600 MPa, both signs so the clip is exercised),
/// k = 10, sigma_y = 517 as in upstream's `main()`.  The port's amplitude
/// comes from [`CycleExtremes`] (two samples, one per extreme) and the FIP
/// from [`FatemiSocie::fip`].  Pass criterion: 100 % bit-identical.
/// Results: see `RESULTS.md` (before and after the operation-order fix).
#[test]
fn fatemi_socie_fip_is_bit_identical_to_prisms_fatigue() {
    let r = load_fip_reference();
    assert!(r.fips.len() == 27 * 12, "reference holds 27 x 12 FIPs");
    let family = SlipFamily::FccOctahedral;
    let param = FatemiSocie::new(r.k, r.sigma_y).unwrap();
    let mut amplitude = Tally::default();
    let mut fip = Tally::default();
    let mut positive = Tally::default();
    for e in 0..27 {
        let mut slip_t = [0.0; MAX_SLIP_SYSTEMS];
        let mut slip_c = [0.0; MAX_SLIP_SYSTEMS];
        for s in 0..12 {
            let (t, c, _) = r.inputs[&(e, s)];
            slip_t[s] = t;
            slip_c[s] = c;
        }
        let mut cycle = CycleExtremes::new(family);
        let mut tension = CrystalState::default();
        tension.slip = slip_t;
        let mut compression = CrystalState::default();
        compression.slip = slip_c;
        cycle.sample(1.0, &tension, &farrer_park::tensor::Voigt6::ZERO);
        cycle.sample(-1.0, &compression, &farrer_park::tensor::Voigt6::ZERO);
        let amp = cycle.shear_strain_amplitudes();
        for s in 0..12 {
            let (t, c, n) = r.inputs[&(e, s)];
            // Upstream: abs((tens - comp) * 0.5).
            amplitude.add(amp[s], ((t - c) * 0.5).abs());
            let port = param.fip(amp[s], n);
            let upstream = r.fips[&(e, s)];
            fip.add(port, upstream);
            if n > 0.0 {
                positive.add(port, upstream);
            }
        }
    }
    println!("FIP amplitude (n={}): identical {}, worst {} ulp", amplitude.n, amplitude.identical, amplitude.worst_ulps);
    println!(
        "FIP all (n={}): identical {}, worst {} ulp, worst rel {:.3e}",
        fip.n, fip.identical, fip.worst_ulps, fip.worst_rel
    );
    println!(
        "FIP with n>0 (n={}): identical {}, worst {} ulp",
        positive.n, positive.identical, positive.worst_ulps
    );
    println!("pandas CSV parse differences vs correctly rounded: {}", r.parse_differences);
    assert_eq!(amplitude.identical, amplitude.n, "amplitude must be bit-identical");
    assert_eq!(fip.identical, fip.n, "FIP must be bit-identical to PRISMS-Fatigue");
}

/// Case 2b (gh:#1434): band averaging, port [`region_fip`] against the real
/// `Al7075_band_averaging`.
///
/// Methodology: 8 grains, 4 planes, 2 layers per plane, bands of up to 7
/// elements; upstream averages each plane's three systems within a band
/// (unweighted, `FIP_sum / n`) and keeps the strict-`>` maximum per grain.
/// The port is fed unit weights (w = 1.0 makes `w*f` and `volume = n` exact,
/// so the two reductions are the same operations) and, as a second pass,
/// an equal non-unit weight 3.7e-18 m^3 where `sum(w f)/(n w)` is a different
/// rounding path (a documented generalisation to unequal element volumes).
#[test]
fn band_average_matches_prisms_fatigue_for_equal_weights() {
    let r = load_fip_reference();
    for (label, weight, require_identical) in [("w = 1.0", 1.0_f64, true), ("w = 3.7e-18", 3.7e-18_f64, false)] {
        let mut tally = Tally::default();
        let mut systems_agree = 0;
        let mut systems_total = 0;
        for g in 0..8 {
            // Upstream control flow: strict `>` from 0.0 over planes then layers.
            let mut best = 0.0_f64;
            let mut best_system = 0;
            for plane in 0..4 {
                for layer in 0..2 {
                    let members = &r.bands[&(g, plane, layer)];
                    let rows: Vec<[f64; MAX_SLIP_SYSTEMS]> = members
                        .iter()
                        .map(|m| {
                            let mut row = [0.0; MAX_SLIP_SYSTEMS];
                            for k in 0..3 {
                                row[k] = r.fips[&(m - 1, plane * 3 + k)];
                            }
                            row
                        })
                        .collect();
                    let weights = vec![weight; rows.len()];
                    let region = region_fip(&rows, &weights, 3).unwrap();
                    if region.fip > best {
                        best = region.fip;
                        best_system = plane * 3 + region.system;
                    }
                }
            }
            let (&(_, us, _), &uv) = r
                .band_results
                .iter()
                .find(|((gg, _, _), _)| *gg == g)
                .expect("upstream result per grain");
            tally.add(best, uv);
            systems_total += 1;
            if best_system == us {
                systems_agree += 1;
            }
        }
        println!(
            "band average {label}: n={}, identical {}, worst {} ulp, winning system agrees {}/{}",
            tally.n, tally.identical, tally.worst_ulps, systems_agree, systems_total
        );
        assert_eq!(systems_agree, systems_total, "same winning slip system ({label})");
        assert!(tally.worst_ulps <= 2, "worst {} ulp ({label})", tally.worst_ulps);
        if require_identical {
            assert_eq!(tally.identical, tally.n, "unit weights must be bit-identical");
        }
    }
}
