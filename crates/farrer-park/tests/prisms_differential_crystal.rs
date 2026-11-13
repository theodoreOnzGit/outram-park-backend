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

//! **Case 1 of gh:#1434, kernel level: PRISMS-Plasticity RateDependentModel
//! kernels against the port's crystal layer.**
//!
//! Methodology, predictions (committed before the run) and measured tables:
//! `verification_and_validation/prisms_moose_differential_gh1434/`
//! (`PREDICTIONS.md`, `RESULTS.md`).  The reference is
//! `reference_cp_kernels.txt`, written by `dump_cp_kernels.cpp`, whose
//! `UPSTREAM` lines are verbatim copies of PRISMS-Plasticity `ffdf4eb6`
//! (checked by `check_verbatim.sh`).  Floats travel as IEEE-754 bit patterns.
//! Inputs are synthetic (fixed LCG) plus the FCC deck files shipped with
//! upstream (`applications/crystalPlasticity/fcc/FCC_Random_RateDependent`,
//! LGPL-2.1-or-later, read by upstream's own `init.cc` lines).
//!
//! The full single-crystal material-point comparison is NOT done here: the
//! port is small-strain, upstream finite-deformation with backstress; see
//! `RESULTS.md` and the child issue.

use farrer_park::crystal::{
    CrystalElasticity, CrystalState, Orientation, PowerLawFlow, SaturatingHardening, SlipFamily,
    SlipSystem, MAX_SLIP_SYSTEMS,
};
use farrer_park::tensor::Voigt6;

const CP_REFERENCE: &str =
    include_str!("../verification_and_validation/prisms_moose_differential_gh1434/reference_cp_kernels.txt");

fn f(hex: &str) -> f64 {
    f64::from_bits(u64::from_str_radix(hex, 16).expect("hex bit pattern"))
}

/// Distance in units in the last place between two finite doubles (bit-pattern
/// difference); `u64::MAX` for opposite signs or non-finite values.
fn ulps(a: f64, b: f64) -> u64 {
    if a.to_bits() == b.to_bits() {
        return 0;
    }
    if !a.is_finite() || !b.is_finite() || (a.is_sign_negative() != b.is_sign_negative()) {
        return u64::MAX;
    }
    a.to_bits().abs_diff(b.to_bits())
}

/// Count of bit-identical values, worst ulp distance and worst relative
/// difference over a comparison.
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

/// Reference lines of one tag, each split on whitespace with the tag dropped.
fn cp_lines(tag: &str) -> Vec<Vec<String>> {
    CP_REFERENCE
        .lines()
        .filter_map(|l| {
            let t: Vec<&str> = l.split_whitespace().collect();
            if t[0] == tag {
                Some(t[1..].iter().map(|s| s.to_string()).collect())
            } else {
                None
            }
        })
        .collect()
}

fn fl(v: &[String], i: usize) -> f64 {
    f(&v[i])
}

/// K1 (gh:#1434): slip-system tables against `slipNormals.txt`,
/// `slipDirections.txt` and `LatentHardeningRatio.txt`, read by upstream's own
/// `init.cc` read-and-normalise lines.
///
/// (a) [`SlipSystem::from_miller`] fed the deck's raw 0.577 / 0.707 numbers
/// must reproduce upstream's normalised vectors bit for bit (same expression).
/// (b) The port's integer-Miller table against the deck path: different inputs
/// to the normalisation, so ulp differences are expected and counted.
/// (c) The integer table through upstream's normalisation lines equals the
/// port's table bit for bit.  (d) The q matrix equals the deck file.
#[test]
fn slip_tables_match_the_prisms_deck() {
    let deck = cp_lines("K1");
    let ints = cp_lines("K1I");
    let port = SlipFamily::FccOctahedral.systems();
    let mut from_deck = Tally::default();
    let mut integer_vs_deck = Tally::default();
    let mut integer_vs_upstream_integer = Tally::default();
    for s in 0..12 {
        let v = &deck[s];
        let raw_n = [fl(v, 1), fl(v, 2), fl(v, 3)];
        let raw_m = [fl(v, 4), fl(v, 5), fl(v, 6)];
        let up_n = [fl(v, 7), fl(v, 8), fl(v, 9)];
        let up_m = [fl(v, 10), fl(v, 11), fl(v, 12)];
        let fed = SlipSystem::from_miller(raw_n, raw_m);
        for k in 0..3 {
            from_deck.add(fed.normal[k], up_n[k]);
            from_deck.add(fed.direction[k], up_m[k]);
            integer_vs_deck.add(port[s].normal[k], up_n[k]);
            integer_vs_deck.add(port[s].direction[k], up_m[k]);
        }
        let w = &ints[s];
        for k in 0..3 {
            integer_vs_upstream_integer.add(port[s].normal[k], fl(w, 1 + k));
            integer_vs_upstream_integer.add(port[s].direction[k], fl(w, 4 + k));
        }
    }
    println!(
        "K1a port(deck numbers) vs upstream deck: n={}, identical {}, worst {} ulp",
        from_deck.n, from_deck.identical, from_deck.worst_ulps
    );
    println!(
        "K1b port integer table vs upstream deck: n={}, identical {}, worst {} ulp, worst rel {:.2e}",
        integer_vs_deck.n, integer_vs_deck.identical, integer_vs_deck.worst_ulps, integer_vs_deck.worst_rel
    );
    println!(
        "K1c port integer table vs upstream normalisation of the same integers: n={}, identical {}, worst {} ulp",
        integer_vs_upstream_integer.n, integer_vs_upstream_integer.identical, integer_vs_upstream_integer.worst_ulps
    );
    assert_eq!(from_deck.identical, from_deck.n);
    assert_eq!(integer_vs_upstream_integer.identical, integer_vs_upstream_integer.n);
    assert!(integer_vs_deck.worst_ulps <= 2, "worst {} ulp", integer_vs_deck.worst_ulps);

    let q = SlipFamily::FccOctahedral.latent_hardening_matrix(1.0, 1.4);
    for l in cp_lines("Q") {
        let (a, b) = (l[0].parse::<usize>().unwrap(), l[1].parse::<usize>().unwrap());
        assert_eq!(q[a][b].to_bits(), f(&l[2]).to_bits(), "q[{a}][{b}] differs from the deck file");
    }
}

/// K2 (gh:#1434): `odfpoint` against [`Orientation::from_rodrigues`] on 200
/// Rodrigues vectors (one zero, 149 with components in [-1, 1], 50 in
/// [-50, 50]).  Pass criterion: bit-identical.
#[test]
fn rodrigues_matches_odfpoint_bit_for_bit() {
    let mut t = Tally::default();
    for l in cp_lines("R") {
        let r = [fl(&l, 1), fl(&l, 2), fl(&l, 3)];
        let m = Orientation::from_rodrigues(r).matrix();
        for i in 0..3 {
            for j in 0..3 {
                t.add(m[i][j], fl(&l, 4 + 3 * i + j));
            }
        }
    }
    println!("K2 odfpoint: n={}, identical {}, worst {} ulp", t.n, t.identical, t.worst_ulps);
    assert_eq!(t.identical, t.n);
}

/// K3 (gh:#1434): rotated cubic stiffness, upstream `elasticmoduli` (Bond
/// matrix, `N C N^T`) against [`CrystalElasticity::stiffness_in_sample_frame`]
/// (contraction one index at a time), constants 170/124/75 (the PRISMS deck's
/// numbers; the arithmetic is scale-free).  Metric: worst |difference| /
/// 170e3 and the bit-identical count.  The driver's `mmult` is a sequential
/// sum, which is not guaranteed to be deal.II's order.  Orientation 0 is the
/// identity (r = 0) and must reproduce the unrotated matrix exactly.
#[test]
fn rotated_stiffness_matches_elasticmoduli() {
    let el = CrystalElasticity::cubic(170.0e3, 124.0e3, 75.0e3).unwrap();
    let mut t = Tally::default();
    let mut worst_abs = 0.0_f64;
    let mut identity_exact = true;
    let rs = cp_lines("R");
    for (idx, l) in cp_lines("C").iter().enumerate() {
        let r = &rs[idx];
        let o = Orientation::from_rodrigues([fl(r, 1), fl(r, 2), fl(r, 3)]);
        let c = el.stiffness_in_sample_frame(&o);
        for i in 0..6 {
            for j in 0..6 {
                let up = fl(l, 1 + 6 * i + j);
                t.add(c.0[i][j], up);
                worst_abs = worst_abs.max((c.0[i][j] - up).abs() / 170.0e3);
                if idx == 0 && c.0[i][j] != up {
                    identity_exact = false;
                }
            }
        }
    }
    println!(
        "K3 stiffness rotation: n={}, identical {}, worst {} ulp, worst |d|/170e3 = {:.3e}",
        t.n, t.identical, t.worst_ulps, worst_abs
    );
    assert!(identity_exact, "identity orientation must match exactly");
    assert!(worst_abs < 1.0e-13, "worst |d|/C11 = {worst_abs:e}");
}

/// K4 (gh:#1434): resolved shear stress on all 12 systems, upstream
/// `trace(T* S^T)` with `S = R (m x n) R^T` unsymmetrised, against
/// [`CrystalState::resolved_shear_stresses`] (`sigma : R sym(m x n) R^T`).
/// Metric: worst |difference| / max |sigma component|.
#[test]
fn resolved_shear_matches_prisms_trace_form() {
    let rs = cp_lines("R");
    let mut t = Tally::default();
    let mut worst = 0.0_f64;
    for (idx, l) in cp_lines("T").iter().enumerate() {
        let r = &rs[idx];
        let o = Orientation::from_rodrigues([fl(r, 1), fl(r, 2), fl(r, 3)]);
        let sv: Vec<f64> = (0..6).map(|k| fl(l, 1 + k)).collect();
        let scale = sv.iter().fold(0.0_f64, |a, b| a.max(b.abs()));
        let sigma = Voigt6::new(sv[0], sv[1], sv[2], sv[3], sv[4], sv[5]);
        let state = CrystalState::default().with_orientation(o);
        let tau = state.resolved_shear_stresses(SlipFamily::FccOctahedral, &sigma);
        for s in 0..12 {
            let up = fl(l, 7 + s);
            t.add(tau[s], up);
            worst = worst.max((tau[s] - up).abs() / scale);
        }
    }
    println!(
        "K4 resolved shear: n={}, identical {}, worst |d|/|sigma|max = {:.3e}",
        t.n, t.identical, worst
    );
    assert!(worst < 1.0e-13, "worst relative {worst:e}");
}

/// K5 (gh:#1434): power-law slip increment, upstream's
/// `delgam_ref*pow(fabs((tau-W)/s),1/m)*sgnm` against
/// [`PowerLawFlow::slip_increment`]; m in {0.02,0.04,0.1,0.2,0.5}, s in
/// {16,40,100}, |tau/s| from 1e-3 to ~3.2 plus exact zero, both signs.  The
/// only expected difference is `pow` (glibc against PETIR `r_powf`).
#[test]
fn power_law_increment_matches_prisms() {
    let mut t = Tally::default();
    for l in cp_lines("G") {
        let (m, tau, s, up) = (fl(&l, 0), fl(&l, 1), fl(&l, 2), fl(&l, 3));
        let flow = PowerLawFlow::new(1.0e-3, m).unwrap();
        t.add(flow.slip_increment(tau, s, 0.1), up);
    }
    println!(
        "K5 power law: n={}, identical {}, worst {} ulp, worst rel {:.3e}",
        t.n, t.identical, t.worst_ulps, t.worst_rel
    );
    assert!(t.worst_ulps <= 2, "worst {} ulp", t.worst_ulps);
}

/// K6 and K7 (gh:#1434): single-system hardening rate `h_beta` and the
/// resistance update, upstream lines 645-657 / 698-708 of the rate-dependent
/// `calculatePlasticity.cc` against [`SaturatingHardening`].  400 cases of 12
/// systems (300 below saturation, 100 within 8 of the 148 saturation so the
/// clamp fires), constants 180 / 148 / 2.25, q 1.0 coplanar / 1.4.  The
/// resistance update is also judged where all twelve `h` are bit-identical, to
/// separate the summation order from `pow`.
#[test]
fn hardening_matches_prisms() {
    let h = SaturatingHardening::new(180.0, 148.0, 2.25, 1.0, 1.4).unwrap();
    let q = SlipFamily::FccOctahedral.latent_hardening_matrix(1.0, 1.4);
    let mut rate = Tally::default();
    let mut update = Tally::default();
    let mut update_given_same_h = Tally::default();
    for l in cp_lines("W") {
        let mut s_t = [0.0; MAX_SLIP_SYSTEMS];
        let mut dg = [0.0; MAX_SLIP_SYSTEMS];
        let mut hb = [0.0; MAX_SLIP_SYSTEMS];
        let mut up = [0.0; MAX_SLIP_SYSTEMS];
        for i in 0..12 {
            s_t[i] = fl(&l, 1 + i);
            dg[i] = fl(&l, 13 + i);
            hb[i] = fl(&l, 25 + i);
            up[i] = fl(&l, 37 + i);
        }
        let mut all_h_same = true;
        for i in 0..12 {
            let mine = h.single_system_rate(s_t[i]);
            rate.add(mine, hb[i]);
            if mine.to_bits() != hb[i].to_bits() {
                all_h_same = false;
            }
        }
        let out = h.advance(&q, &s_t, &dg, 12);
        for i in 0..12 {
            update.add(out[i], up[i]);
            if all_h_same {
                update_given_same_h.add(out[i], up[i]);
            }
        }
    }
    println!("K6 h_beta: n={}, identical {}, worst {} ulp", rate.n, rate.identical, rate.worst_ulps);
    println!(
        "K7 resistance update: n={}, identical {}, worst {} ulp; where all 12 h identical: n={}, identical {}, worst {} ulp",
        update.n, update.identical, update.worst_ulps, update_given_same_h.n, update_given_same_h.identical,
        update_given_same_h.worst_ulps
    );
    assert!(rate.worst_ulps <= 2);
    assert!(update.worst_ulps <= 4, "worst {} ulp", update.worst_ulps);
    assert_eq!(
        update_given_same_h.identical, update_given_same_h.n,
        "with identical h the update must be bit-identical (summation order)"
    );
}
