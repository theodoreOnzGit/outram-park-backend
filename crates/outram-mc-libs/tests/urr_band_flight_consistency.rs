// SPDX-License-Identifier: GPL-3.0

//! **One URR band per nuclide and energy serves the flight, the choice of
//! nuclide and the reaction, as in OpenMC.** GitHub #407.
//!
//! # What changed
//!
//! OpenMC computes a nuclide's micro cross sections once per energy
//! (`Nuclide::calculate_xs`, `src/nuclide.cpp`). In the unresolved range they
//! come from one probability-table band, with `r = future_prn(index_,
//! seeds[STREAM_URR_PTABLE])` (`calculate_urr_xs`). That band's total feeds:
//! - the macroscopic total, hence the flight distance;
//! - the choice of nuclide;
//! - the reaction.
//!
//! The URR stream advances by the number of nuclides after a collision that
//! changed the energy (`physics.cpp`, `advance_prn_seed`).
//!
//! Before this change, outram-mc flew and chose the nuclide on the smooth
//! (infinitely-dilute) total, then drew a fresh band from the transport stream
//! at the collision. That is not the same law: the reaction probabilities
//! become `<σ_t> <σ_x/σ_t>` rather than `<σ_x>`, and the flux depression
//! inside a high-`σ_t` band is lost.
//!
//! # Methodology
//!
//! These are kernel-level checks of the new functions on NJOY2016 U-235 and
//! U-238 (293.6 K, ENDF/B-VIII.0, `reference-data/ace`). The integral check
//! against OpenMC (URR worth, route 3 against route 1) and the paired k A/B are
//! recorded on #407.
//!
//! 1. **Upper bound:** `total_upper_bound(e)` equals the maximum of
//!    `band_total(e, xi)` over 200 000 random `xi`, to 1e-12 relative. It is
//!    exact because `band_representatives` visits every selectable band
//!    pair. This is what a delta-tracking majorant needs.
//! 2. **Macroscopic total:** `macro_xs_total_urr` equals
//!    `sum_i N_i band_total_i(urr_xi(i, seed))`. Outside the unresolved
//!    range it equals `macro_xs_total` bit for bit.
//! 3. **Nuclide choice:** for a fixed URR seed inside the range,
//!    `sample_nuclide_urr` picks each nuclide in proportion to its band
//!    total. That is a binomial test at 5 sigma over 400 000 draws.
//!
//! # Results (2026-09-29)
//!
//! All pass; the printed numbers are recorded on #407.

use outram_mc_libs::material::material::{urr_xi, Material, NuclideComponent};
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::rng::lcg::prn;
use std::path::PathBuf;

const T: f64 = 293.6;

fn load(name: &str) -> Option<Nuclide> {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!(
        "../../reference-data/ace/reference-njoy/endf-b-viii.0/293.6K/{name}.ace.gz"
    ));
    p.is_file().then(|| Nuclide::from_ace_file(&p, name).expect("loads"))
}

#[test]
fn band_total_bound_and_macroscopic_total() {
    let (Some(u5), Some(u8)) = (load("U235"), load("U238")) else {
        println!("reference ACE absent: skipping");
        return;
    };
    let nucs = vec![u5, u8];
    // 1. The bound is the exact maximum.
    let mut checked = 0;
    for (k, nuc) in nucs.iter().enumerate() {
        for i in 0..40 {
            let e = 2.3e3 * (1.4e5_f64 / 2.3e3).powf(i as f64 / 39.0);
            if !nuc.needs_urr_draw(e) {
                continue;
            }
            let bound = nuc.total_upper_bound(e, T);
            let mut s = 0xB0_0000 + (k * 100 + i) as u64;
            let sampled = (0..200_000)
                .map(|_| nuc.band_total(e, T, prn(&mut s)))
                .fold(0.0_f64, f64::max);
            assert!(sampled <= bound * (1.0 + 1e-12), "E={e}: sampled {sampled} > bound {bound}");
            assert!(
                (bound - sampled).abs() <= 1e-12 * bound || bound == nuc.total_at_energy(e, T),
                "E={e}: bound {bound} is not attained (max sampled {sampled})"
            );
            checked += 1;
        }
    }
    println!("upper bound exact at {checked} energies");
    assert!(checked > 20);

    // 2. The macroscopic total.
    let mat = Material {
        id: 1,
        name: "mix".into(),
        components: vec![
            NuclideComponent { nuclide_idx: 0, atom_density: 0.045 },
            NuclideComponent { nuclide_idx: 1, atom_density: 0.0025 },
        ],
        temperature: T,
    };
    for &e in &[1.0, 1.0e3, 5.0e3, 3.0e4, 1.0e5, 2.0e6] {
        for seed in [1_u64, 99, 12345] {
            let got = mat.macro_xs_total_urr(e, &nucs, seed);
            let want: f64 = mat
                .components
                .iter()
                .map(|c| {
                    c.atom_density * nucs[c.nuclide_idx].band_total(e, T, urr_xi(c.nuclide_idx, seed))
                })
                .sum();
            assert!((got - want).abs() <= 1e-14 * want, "E={e}: {got} vs {want}");
            if !nucs.iter().any(|n| n.needs_urr_draw(e)) {
                assert_eq!(got, mat.macro_xs_total(e, &nucs), "E={e}: must equal the smooth total");
            }
        }
    }
}

#[test]
fn nuclide_choice_follows_band_totals() {
    let (Some(u5), Some(u8)) = (load("U235"), load("U238")) else {
        println!("reference ACE absent: skipping");
        return;
    };
    let nucs = vec![u5, u8];
    let mat = Material {
        id: 1,
        name: "mix".into(),
        components: vec![
            NuclideComponent { nuclide_idx: 0, atom_density: 0.02 },
            NuclideComponent { nuclide_idx: 1, atom_density: 0.02 },
        ],
        temperature: T,
    };
    let e = 2.2e4; // inside both unresolved ranges
    assert!(nucs.iter().all(|n| n.needs_urr_draw(e)));
    for urr_seed in [7_u64, 8, 9] {
        let w0 = 0.02 * nucs[0].band_total(e, T, urr_xi(0, urr_seed));
        let w1 = 0.02 * nucs[1].band_total(e, T, urr_xi(1, urr_seed));
        let p = w0 / (w0 + w1);
        let n = 400_000;
        let mut seed = 0x5E1E_C700 + urr_seed;
        let hits = (0..n)
            .filter(|_| mat.sample_nuclide_urr(e, &mut seed, &nucs, urr_seed) == 0)
            .count() as f64;
        let z = (hits / n as f64 - p) / (p * (1.0 - p) / n as f64).sqrt();
        println!("urr_seed {urr_seed}: P(U-235) {:.5} vs band weights {p:.5}, z {z:+.2}", hits / n as f64);
        assert!(z.abs() < 5.0);
    }
}
