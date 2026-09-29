// SPDX-License-Identifier: GPL-3.0

//! **Equiprobable (ACE IFENG = 0) S(α,β) tables are sampled as OpenMC samples
//! them, by default.** GitHub #407.
//!
//! # What changed
//!
//! OpenMC samples this form with `IncoherentInelasticAEDiscrete`
//! (`src/secondary_thermal.cpp`, `skewed_ = false`):
//! 1. `i, f` come from `get_energy_index`;
//! 2. one bin `j = floor(prn n)`, every bin equally likely;
//! 3. `E' = (1-f) E(i,j) + f E(i+1,j)`;
//! 4. one cosine `k = floor(prn m)`, with `mu` interpolated the same way.
//!
//! So `E'` is one of `n` discrete values. Until 2026-09-29 outram used another
//! scheme (GitHub #188): it chose table `i` or `i+1` statistically and drew
//! `E'` continuously inside the bin. That scheme was recorded as a maintainer
//! decision; it never was one. It is now the explicit ablation
//! `ThermalScattering::with_legacy_equiprobable_sampling`
//! (`--ablate legacy-thermal-sampling`), off by default and pinned off by
//! `tests/correct_physics_is_default.rs`.
//!
//! # Methodology
//!
//! `openmc_inputs/thermal_equiprobable_reference.py` reads two NJOY2016 tables
//! with OpenMC's reader:
//! - H in H2O at 293.6 K: the five-route library, `iwt = 1`, 64 bins,
//!   20 cosines;
//! - C in graphite at 296 K: `njoy_decks/Cgraph.input`, 16 bins.
//!
//! It writes the **exact** `<E'>`, `<E'^2>`, `<mu>`, `<mu^2>` and `<E' mu>` of
//! OpenMC's scheme, and the exact CDF of `E'` at every support point. The
//! draws are taken through the transport path,
//! `ThermalScattering::sample_inelastic`.
//!
//! **Pass:** every moment within 5 sample-sem, and the KS distance of `E'`
//! below `1.95/sqrt(N)` (alpha = 1e-3; conservative for a discrete law).
//!
//! **Power:** with the legacy ablation on, the KS test must fail at every
//! energy. A continuous `E'` cannot match a discrete CDF.
//!
//! # Results (2026-09-29)
//!
//! Printed by the test and recorded on #407.

use outram_mc_libs::material::thermal::ThermalScattering;
use std::path::PathBuf;

const N: usize = 1_000_000;

fn ws() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn load(name: &str) -> Option<ThermalScattering> {
    let p = match name {
        "HinH2O" => ws().join("target/five_route_keff/njoy/293.6K/HinH2O.ace"),
        "Cgraph" => ws().join("target/ace_extra/Cgraph/tape30"),
        _ => unreachable!(),
    };
    if !p.is_file() {
        println!("{} absent: skipping {name}", p.display());
        return None;
    }
    let raw = njoy_outram_park_fork::acer::read::read(&p).expect("read");
    Some(ThermalScattering::from_ace(&raw, name).expect("IFENG = 0 loads"))
}

fn mean_sem(v: &[f64]) -> (f64, f64) {
    let n = v.len() as f64;
    let m = v.iter().sum::<f64>() / n;
    let var = v.iter().map(|x| (x - m).powi(2)).sum::<f64>() / (n - 1.0);
    (m, (var / n).sqrt())
}

#[test]
fn equiprobable_sab_samples_as_openmc() {
    let text = std::fs::read_to_string(ws().join(
        "crates/outram-mc-libs/verification_and_validation/ace_route_physics/data/thermal_equiprobable_openmc.csv",
    ))
    .expect("reference");
    let mut tables: Vec<(String, Option<ThermalScattering>)> = Vec::new();
    let crit = 1.95 / (N as f64).sqrt();
    let (mut worst_z, mut worst_ks, mut legacy_min_ks) = (0.0_f64, 0.0_f64, f64::INFINITY);
    let mut n_rows = 0;
    let lines: Vec<&str> = text.lines().filter(|l| !l.starts_with('#') && !l.is_empty()).collect();
    for (row, line) in lines.iter().enumerate() {
        let f: Vec<&str> = line.split(',').collect();
        if f[0] != "M" {
            continue;
        }
        let name = f[1];
        let v: Vec<f64> = f[2..].iter().map(|x| x.parse().unwrap()).collect();
        let e = v[0];
        if !tables.iter().any(|(n, _)| n == name) {
            tables.push((name.to_string(), load(name)));
        }
        let Some(th) = tables.iter().find(|(n, _)| n == name).unwrap().1.as_ref() else {
            continue;
        };
        let cdf: Vec<(f64, f64)> = lines
            .iter()
            .filter_map(|l| {
                let g: Vec<&str> = l.split(',').collect();
                (g[0] == "C" && g[1] == name && g[2].parse::<f64>().unwrap() == e)
                    .then(|| (g[3].parse().unwrap(), g[4].parse().unwrap()))
            })
            .collect();
        let ks = |th: &ThermalScattering, seed0: u64| -> (f64, Vec<(f64, f64)>) {
            let mut seed = seed0;
            let d: Vec<(f64, f64)> =
                (0..N).map(|_| th.sample_inelastic(e, &mut seed).expect("below cutoff")).collect();
            let mut es: Vec<f64> = d.iter().map(|p| p.0).collect();
            es.sort_by(|a, b| a.partial_cmp(b).unwrap());
            let mut dmax: f64 = 0.0;
            for &(x, fx) in &cdf {
                // Support points are exact reproductions of OpenMC's arithmetic
                // to rounding, so count draws within a relative 1e-12 of x.
                let emp = es.partition_point(|&y| y <= x * (1.0 + 1e-12)) as f64 / N as f64;
                dmax = dmax.max((emp - fx).abs());
            }
            (dmax, d)
        };
        let (d_ks, draws) = ks(th, 0xE9_0000 + row as u64);
        let cols: [(&str, fn(&(f64, f64)) -> f64); 5] = [
            ("E'", |p| p.0),
            ("E'^2", |p| p.0 * p.0),
            ("mu", |p| p.1),
            ("mu^2", |p| p.1 * p.1),
            ("E'mu", |p| p.0 * p.1),
        ];
        let mut zs = Vec::new();
        for (k, (label, g)) in cols.iter().enumerate() {
            let xs: Vec<f64> = draws.iter().map(g).collect();
            let (m, s) = mean_sem(&xs);
            let z = (m - v[1 + k]) / s;
            assert!(z.abs() < 5.0, "{name} E={e} {label}: {m} vs {} z {z}", v[1 + k]);
            zs.push(z);
            worst_z = worst_z.max(z.abs());
        }
        assert!(d_ks < crit, "{name} E={e}: KS D {d_ks} >= {crit}");
        worst_ks = worst_ks.max(d_ks / crit);
        let legacy = th.clone().with_legacy_equiprobable_sampling();
        let (d_leg, _) = ks(&legacy, 0xE9_8000 + row as u64);
        legacy_min_ks = legacy_min_ks.min(d_leg / crit);
        println!(
            "{name} E={e}: z {:?}; KS D/crit {:.2} (legacy ablation {:.1})",
            zs.iter().map(|z| (z * 100.0).round() / 100.0).collect::<Vec<_>>(),
            d_ks / crit,
            d_leg / crit
        );
        n_rows += 1;
    }
    println!("{n_rows} rows: worst |z| {worst_z:.2}, worst KS D/crit {worst_ks:.2}, legacy min D/crit {legacy_min_ks:.1}");
    if n_rows > 0 {
        assert!(legacy_min_ks > 1.0, "the KS test cannot see the legacy scheme");
    }
}
