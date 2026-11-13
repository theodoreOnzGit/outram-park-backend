// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//! **V&V, code-to-code (gh:#1096): MELCOR's heat-structure surface moved to
//! `tampines` is bit-identical to `sembawang`'s original.**
//!
//! > Research, education and V&V only. AI draft, unreviewed.
//!
//! # Methodology
//!
//! `sembawang::bwr_mark1::pool_surface::structure_surface` (MELCOR HS RM
//! §2.6.1, §2.7: natural convection plus condensation of steam by diffusion
//! through the non-condensables) moved verbatim to
//! `tampines::components::wall_condensation` on 2026-11-01, and `sembawang`
//! now forwards to it. **How the pre-move outputs were kept:** this test was
//! written and run on the *unmodified* code first (commit `8841fd46c`, the
//! function still in `sembawang`), over the grid below; it printed the FNV-1a
//! 64-bit hash of the bits of every output, and that hash and twelve sample
//! values (as `f64` bit patterns) were pinned here **before** the code moved.
//! After the move the same grid is evaluated (a) through `sembawang`'s
//! forwarding path and (b) straight through `tampines`' function, and both
//! must reproduce the pins bit for bit.
//!
//! The grid, 2 646 states, chosen to reach every branch: gas temperature
//! {300, 350, 400, 450, 500, 600, 800} K; wall temperature {290, 320, 350,
//! 380, 420, 500} K; pressure {0.1, 0.3, 0.8} MPa; seven compositions (pure
//! N2; 5 % steam in N2; 30 % steam, 10 % H2; 60 % steam in N2; 95 % steam
//! in H2; 99.99 % steam, above MELCOR's `VPFRAC = 0.9995`; pure steam, no
//! non-condensables); characteristic heights {0.3, 5, 30} m (laminar,
//! transitional and turbulent Rayleigh numbers). Pass criterion: every bit
//! equal (no round-off allowance: the move kept the operation order).
//!
//! **Prediction, written before the move:** bit-identical, hash unchanged.
//!
//! # Results (2026-11-01)
//!
//! Pre-move hash `0xbd0af0807827d1c9`, 2 646 states (1 050 of them
//! condensing), 5 292 outputs. After the move both paths reproduce it and
//! all twelve samples bit for bit. **Held.**

use sembawang::bwr_mark1::pool_surface::{structure_surface, Atmosphere};
use tampines::components::wall_condensation as moved;

const R: f64 = 8.314_462;

fn grid() -> Vec<(Atmosphere, f64, f64)> {
    let comps: [(f64, f64, f64); 7] = [
        (0.0, 1.0, 0.0),
        (0.05, 0.95, 0.0),
        (0.3, 0.6, 0.1),
        (0.6, 0.4, 0.0),
        (0.95, 0.0, 0.05),
        (0.9999, 0.0001, 0.0),
        (1.0, 0.0, 0.0),
    ];
    let mut v = Vec::new();
    for &t_g in &[300.0, 350.0, 400.0, 450.0, 500.0, 600.0, 800.0] {
        for &t_w in &[290.0, 320.0, 350.0, 380.0, 420.0, 500.0] {
            for &p in &[1.0e5, 3.0e5, 8.0e5] {
                for &(xs, xn, xh) in &comps {
                    // Moles in a 1000 m^3 volume, as the containment carries them.
                    let n = p * 1000.0 / (R * t_g);
                    let atm = Atmosphere { t_k: t_g, p_pa: p, n_steam: xs * n, n_n2: xn * n, n_h2: xh * n };
                    for &l in &[0.3, 5.0, 30.0] {
                        v.push((atm, t_w, l));
                    }
                }
            }
        }
    }
    v
}

fn fnv(h: &mut u64, x: f64) {
    for b in x.to_bits().to_le_bytes() {
        *h ^= b as u64;
        *h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
}

fn hash_of(f: impl Fn(&Atmosphere, f64, f64) -> (f64, f64)) -> (u64, Vec<(f64, f64)>) {
    let mut h = 0xcbf2_9ce4_8422_2325_u64;
    let mut out = Vec::new();
    for (atm, t_w, l) in grid() {
        let (a, b) = f(&atm, t_w, l);
        fnv(&mut h, a);
        fnv(&mut h, b);
        out.push((a, b));
    }
    (h, out)
}

/// Pinned on the pre-move code (see the module doc).
const PINNED_HASH: u64 = 0xbd0a_f080_7827_d1c9;
/// `(index into the grid, h bits, condensation-flux bits)`, pre-move.
const PINNED_SAMPLES: [(usize, u64, u64); 12] = [
    (0, 0x400d14ab8fe24dce, 0x0000000000000000),
    (221, 0x40216d09ab09e25a, 0x3f585a7d16a6505f),
    (442, 0x400a2822c324bd60, 0x0000000000000000),
    (663, 0x4023d0dbd181e48b, 0x0000000000000000),
    (884, 0x400c30481295f14e, 0x0000000000000000),
    (1105, 0x402362c91174ef6f, 0x0000000000000000),
    (1326, 0x40142a666b4b6feb, 0x0000000000000000),
    (1547, 0x4028193bca512bb6, 0x3fb3b648c4a95e83),
    (1768, 0x400c46017575f3ae, 0x0000000000000000),
    (1989, 0x402b8c0483487f6c, 0x3fc007a569577d0c),
    (2210, 0x400aff34d625ced0, 0x0000000000000000),
    (2431, 0x402c3b01826a7ffd, 0x3fc4db0fc39ff14b),
];

/// (a) Through `sembawang`'s path (the re-export after the move), and (b)
/// straight through `tampines`, converting the state to `tampines`' own
/// `Atmosphere` field by field.
#[test]
fn the_moved_structure_surface_is_bit_identical_to_sembawangs_original() {
    check(hash_of(structure_surface));
    check(hash_of(|a: &Atmosphere, t_w: f64, l: f64| {
        let t = moved::Atmosphere { t_k: a.t_k, p_pa: a.p_pa, n_steam: a.n_steam, n_n2: a.n_n2, n_h2: a.n_h2 };
        moved::structure_surface(&t, t_w, l)
    }));
}

fn check((h, out): (u64, Vec<(f64, f64)>)) {
    let n_condensing = out.iter().filter(|o| o.1 > 0.0).count();
    println!("{} states, {n_condensing} condensing", out.len());
    assert_eq!(out.len(), 2646);
    println!("hash {h:#018x}");
    for i in (0..out.len()).step_by(221) {
        println!("({i}, {:#018x}, {:#018x}),", out[i].0.to_bits(), out[i].1.to_bits());
    }
    assert_eq!(h, PINNED_HASH, "hash moved");
    for (i, a, b) in PINNED_SAMPLES {
        assert_eq!((out[i].0.to_bits(), out[i].1.to_bits()), (a, b), "sample {i}");
    }
}
