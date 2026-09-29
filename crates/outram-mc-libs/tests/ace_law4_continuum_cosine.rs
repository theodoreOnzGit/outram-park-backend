// SPDX-License-Identifier: GPL-3.0

//! **An ACE LAW=4 continuum on MT=91/16/17 keeps its AND cosine** — GitHub
//! #365 audit.
//!
//! # What was wrong
//!
//! `Nuclide::from_ace` stored a LAW=4 continuum as a correlated law with
//! `ContinuumAngular::EvaluatedIsotropic`, never reading AND. The comment
//! claimed AND "is isotropic in the evaluation itself" for these reactions,
//! and nothing checked that. It is now placed as the MF=4 + MF=5 pair
//! (`UncorrelatedEmission`, AND's cosine sampled independently), which is what
//! OpenMC's `UncorrelatedAngleEnergy` is
//! (`openmc/data/reaction.py:1131-1135`) and what the ENDF route builds from
//! the same evaluation. No held table has a LAW=4 continuum, so two were
//! generated with NJOY2016 (deck: `openmc_godiva_cross_code/make_ace.sh`):
//! Li-7 ENDF/B-VIII.0 (MT=16 LAW=4, anisotropic AND) and U-238 JENDL-3.3
//! (MT=91 LAW=4, AND isotropic below a few MeV and `<mu>` ~ 0.35 at 14 MeV).
//!
//! # Methodology
//!
//! 1. **Placement.** On both tables the reaction must come back as
//!    `uncorrelated_law(mt)` with an anisotropic AND, and not as
//!    `continuum_law(mt)`.
//! 2. **Against OpenMC.** `N = 400 000` draws of
//!    `Nuclide::sample_inelastic_emission` per energy. Pass: the sample means
//!    of `E'` and of the laboratory cosine lie within **5 sigma** (sample sem)
//!    of the exact means of OpenMC's sampling scheme. Those means come from
//!    OpenMC's reader via
//!    `verification_and_validation/ace_route_physics/openmc_inputs/law4_continuum_reference.py`
//!    into `data/law4_continuum_openmc.csv`.
//! 3. **ACE route against ENDF route.** The same draws from
//!    `UncorrelatedEmission::from_endf` on the evaluation. Two-sample KS on
//!    `E'` and on `mu`, `N = 100 000`, alpha = 0.001, critical
//!    `1.95 sqrt(2/N) = 8.72e-3`.
//!
//! Tables live in `target/ace_extra/` (regenerable); skipped when absent.
//!
//! # Results (2026-09-29)
//!
//! Printed by the test and recorded in the #365 thread.

use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::nuclear_data::secondary::UncorrelatedEmission;
use outram_mc_libs::geometry::position::Direction;
use outram_mc_libs::material::nuclide::{sample_uncorrelated_emission, Nuclide};
use std::path::PathBuf;

fn ws() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn ks_two(a: &mut [f64], b: &mut [f64]) -> f64 {
    a.sort_by(|x, y| x.partial_cmp(y).unwrap());
    b.sort_by(|x, y| x.partial_cmp(y).unwrap());
    let (na, nb) = (a.len() as f64, b.len() as f64);
    let (mut i, mut j, mut d) = (0usize, 0usize, 0.0f64);
    while i < a.len() && j < b.len() {
        let x = a[i].min(b[j]);
        while i < a.len() && a[i] <= x {
            i += 1;
        }
        while j < b.len() && b[j] <= x {
            j += 1;
        }
        d = d.max((i as f64 / na - j as f64 / nb).abs());
    }
    d
}

fn mean_sem(v: &[f64]) -> (f64, f64) {
    let n = v.len() as f64;
    let m = v.iter().sum::<f64>() / n;
    let var = v.iter().map(|x| (x - m).powi(2)).sum::<f64>() / (n - 1.0);
    (m, (var / n).sqrt())
}

#[test]
fn law4_continuum_keeps_its_and_cosine() {
    let dir = ws().join("target/ace_extra");
    if !dir.join("Li7/tape24").is_file() || !dir.join("U238J33/tape24").is_file() {
        println!("target/ace_extra absent: skipping (see the module doc)");
        return;
    }
    let csv = ws().join(
        "crates/outram-mc-libs/verification_and_validation/ace_route_physics/data/law4_continuum_openmc.csv",
    );
    let text = std::fs::read_to_string(csv).expect("reference");
    let u = Direction::new(0.0, 0.0, 1.0);
    let mut rows = 0;
    for line in text
        .lines()
        .filter(|l| !l.starts_with('#') && !l.is_empty())
    {
        let f: Vec<&str> = line.split(',').collect();
        let (file, mt) = (f[0], f[1].parse::<i32>().unwrap());
        let (e, want_e, want_mu): (f64, f64, f64) = (
            f[2].parse().unwrap(),
            f[3].parse().unwrap(),
            f[4].parse().unwrap(),
        );
        let name = if file.starts_with("Li7") {
            "Li7"
        } else {
            "U238"
        };
        let nuc = Nuclide::from_ace_file(dir.join(file), name).expect("from_ace_file");
        assert!(
            nuc.continuum_law(mt).is_none(),
            "{file} MT={mt}: must not be a correlated law"
        );
        let law = nuc.uncorrelated_law(mt).expect("placed as MF=4 + MF=5");
        assert!(law.is_anisotropic(), "{file} MT={mt}: AND must be carried");
        const N: usize = 400_000;
        let mut seed = 0x1a44_0000 + rows as u64;
        let draws: Vec<(f64, f64)> = (0..N)
            .map(|_| {
                let (eo, d) = nuc.sample_inelastic_emission(mt, e, u, 0.0, &mut seed);
                (eo, d.w)
            })
            .collect();
        let (me, se) = mean_sem(&draws.iter().map(|d| d.0).collect::<Vec<_>>());
        let (mm, sm) = mean_sem(&draws.iter().map(|d| d.1).collect::<Vec<_>>());
        let (ze, zm) = ((me - want_e) / se, (mm - want_mu) / sm);
        println!(
            "{file} MT={mt} E = {:.1} MeV: <E'> {:.5} MeV (OpenMC {:.5}, z {ze:+.2}); <mu> {mm:+.5} (OpenMC {want_mu:+.5}, z {zm:+.2})",
            e / 1e6,
            me / 1e6,
            want_e / 1e6
        );
        assert!(
            ze.abs() < 5.0 && zm.abs() < 5.0,
            "{file} MT={mt} E = {e}: z_E {ze}, z_mu {zm}"
        );

        // ACE route vs ENDF route on the same evaluation.
        let tape_name = if name == "Li7" {
            "n-003_Li_007-ENDF8.0.endf"
        } else {
            "n-092_U_238-JENDL3.3.endf"
        };
        let tape =
            Tape::read_file(&ws().join("reference-data/endf").join(tape_name)).expect("tape");
        let mat = tape.materials()[0];
        let endf: UncorrelatedEmission = UncorrelatedEmission::from_endf(&tape, mat, mt)
            .expect("parse")
            .expect("MF=4+5");
        const M: usize = 100_000;
        let (mut ae, mut am, mut be, mut bm) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
        let mut s1 = 0xACE0_0000 + rows as u64;
        let mut s2 = 0xE0F0_0000 + rows as u64;
        for _ in 0..M {
            let (eo, d) = nuc.sample_inelastic_emission(mt, e, u, 0.0, &mut s1);
            ae.push(eo);
            am.push(d.w);
            let (eo2, mu2) = sample_uncorrelated_emission(&endf, e, &mut s2);
            be.push(eo2);
            bm.push(mu2);
        }
        let (de, dm) = (ks_two(&mut ae, &mut be), ks_two(&mut am, &mut bm));
        let crit = 1.95 * (2.0 / M as f64).sqrt();
        println!("   ACE vs ENDF route: KS D(E') = {de:.2e}, D(mu) = {dm:.2e} (crit {crit:.2e})");
        assert!(
            de < crit && dm < crit,
            "{file} MT={mt} E = {e}: ACE vs ENDF KS {de} / {dm}"
        );
        rows += 1;
    }
    assert_eq!(rows, 6);
}
