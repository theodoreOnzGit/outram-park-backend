// SPDX-License-Identifier: GPL-3.0

//! **One elastic scatter off U-238 near its low resonances, with DBRC, against
//! OpenMC.** GitHub #407: on the lattice, the DBRC worth is −42.6 ± 20.0 pcm in
//! outram-mc and +27.6 ± 10.5 in OpenMC. This compares the sampler itself.
//!
//! # Methodology
//!
//! **Reference.**
//! `verification_and_validation/ace_route_physics/openmc_inputs/dbrc_single_scatter_reference.py`
//! runs OpenMC with:
//! - a monoenergetic isotropic source at the centre of a U-238 sphere
//!   (19.05 g/cm³, 293.6 K), thin enough that the escaping spectrum is the
//!   uncollided line plus single scatters;
//! - the surface current tallied on 60 bins around `E0`;
//! - `E0` = 6.4, 20.5 and 36.4 eV, just below the 6.67, 20.9 and 36.7 eV
//!   resonances, where the target's thermal motion carries the relative energy
//!   into the resonance;
//! - DBRC on and off.
//!
//! **Data.** Both codes read the five-route NJOY2016 library, with U-238's 0 K
//! elastic from the library's own 0 K table.
//!
//! **outram-mc side.** `N` draws of the transport path at `E0`:
//! `Nuclide::sample_elastic_mu_cm`, then `free_gas_elastic_scatter_dbrc` with
//! the nuclide's own DBRC table (or `None`). They are binned on the same edges.
//!
//! **Comparison.** The shapes are compared after dropping the bin that holds
//! the uncollided line, each normalised to unit sum. The statistic is a
//! chi-square over the populated bins, from both sides' variances, reported as
//! a z value `(χ² − ν)/sqrt(2ν)`.
//!
//! # Results (2026-09-30)
//!
//! Worst |z| over the six rows (three energies, DBRC on and off): **3.6**, at
//! 20.5 eV with DBRC on. There `<E'>` agrees with OpenMC to −1.4e-5 relative.
//!
//! **Before the fix it failed.** The DBRC rejection loop was capped at 4096
//! trials and returned an unaccepted candidate on a miss. At 20.5 eV that gave
//! χ²/ν = 407/58 (z = +32) and `<E'>` −5.7e-4 against OpenMC, because the
//! window reaches the 20.87 eV peak and the acceptance is small. OpenMC loops
//! unbounded, and so does outram-mc now (`physics::scatter`).
//!
//! The reference spheres were thinned until the DBRC-off rows agree
//! (|z| ≤ 2.4). A thicker first attempt showed a spurious z ≈ 20 with DBRC off,
//! from multiple scattering.

use outram_mc_libs::geometry::position::Direction;
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::physics::scatter::free_gas_elastic_scatter_dbrc;
use std::path::PathBuf;

const N: usize = 4_000_000;

#[test]
#[cfg_attr(
    not(feature = "long-tests"),
    ignore = "24 M near-resonance DBRC scatters, ~8 minutes; runs by default"
)]
fn dbrc_single_scatter_matches_openmc() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let ace = root.join("target/five_route_keff/njoy/293.6K/U238.ace");
    if !ace.is_file() {
        println!("{} absent: skipping", ace.display());
        return;
    }
    let nuc = Nuclide::from_ace_file(&ace, "U238").expect("U-238 loads");
    assert!(nuc.has_dbrc(), "the 0 K companion must attach");
    let text = std::fs::read_to_string(root.join(
        "crates/outram-mc-libs/verification_and_validation/ace_route_physics/data/dbrc_single_scatter_openmc.csv",
    ))
    .expect("reference");
    let rows: Vec<Vec<f64>> = text
        .lines()
        .filter(|l| !l.starts_with('#') && !l.is_empty())
        .map(|l| l.split(',').map(|x| x.parse().unwrap()).collect())
        .collect();
    let kt = nuc.free_gas_kt(293.6);
    let u = Direction::new(0.0, 0.0, 1.0);
    let mut worst: f64 = 0.0;
    for &e0 in &[6.4, 20.5, 36.4] {
        for dbrc in [1.0, 0.0] {
            let sel: Vec<&Vec<f64>> = rows.iter().filter(|r| r[0] == e0 && r[1] == dbrc).collect();
            let edges: Vec<f64> = sel.iter().map(|r| r[2]).chain(std::iter::once(sel.last().unwrap()[3])).collect();
            let nb = sel.len();
            let line = sel.iter().position(|r| r[2] <= e0 && e0 < r[3]).unwrap();
            let mut h = vec![0.0_f64; nb];
            let mut seed = 0xDB5_0000 + (e0 * 10.0) as u64 + dbrc as u64;
            let table = if dbrc > 0.0 { nuc.dbrc_table() } else { None };
            for _ in 0..N {
                let mu = nuc.sample_elastic_mu_cm(e0, &mut seed).unwrap_or(0.0);
                let (e1, _) = free_gas_elastic_scatter_dbrc(e0, u, nuc.awr, kt, mu, &mut seed, table);
                let k = edges.partition_point(|&x| x <= e1);
                if k >= 1 && k <= nb {
                    h[k - 1] += 1.0;
                }
            }
            h[line] = 0.0;
            let (ours, omc): (f64, f64) = (
                h.iter().sum(),
                sel.iter().enumerate().filter(|(k, _)| *k != line).map(|(_, r)| r[4]).sum(),
            );
            let (mut chi2, mut nu) = (0.0, 0usize);
            for k in 0..nb {
                if k == line || sel[k][4] <= 0.0 {
                    continue;
                }
                let p = h[k] / ours;
                let q = sel[k][4] / omc;
                let var = p / ours + (q * sel[k][5]).powi(2);
                if var > 0.0 {
                    chi2 += (p - q).powi(2) / var;
                    nu += 1;
                }
            }
            let z = (chi2 - nu as f64) / (2.0 * nu as f64).sqrt();
            // Mean outgoing energy of the scattered part, both codes.
            let mean = |w: &dyn Fn(usize) -> f64, tot: f64| {
                (0..nb).filter(|&k| k != line).map(|k| w(k) * 0.5 * (edges[k] + edges[k + 1])).sum::<f64>() / tot
            };
            let m_ours = mean(&|k| h[k], ours);
            let m_omc = mean(&|k| sel[k][4], omc);
            println!(
                "E0={e0} DBRC={}: chi2/nu = {chi2:.1}/{nu} (z {z:+.1}); <E'> ours {m_ours:.5} OpenMC {m_omc:.5} ({:+.2e})",
                dbrc as i32,
                m_ours / m_omc - 1.0
            );
            worst = worst.max(z.abs());
        }
    }
    println!("worst |z| {worst:.1}");
    assert!(worst < 5.0, "single-scatter spectrum differs from OpenMC's (worst z {worst:.1})");
}
