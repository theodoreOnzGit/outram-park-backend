// SPDX-License-Identifier: GPL-3.0

//! **Unresolved-resonance cross sections are sampled as OpenMC samples them.**
//! GitHub #365 audit.
//!
//! # What was different
//!
//! 1. `UrrProbabilityTables::sample` took the band from the **nearest**
//!    tabulated energy. OpenMC's `Nuclide::calculate_urr_xs`
//!    (`src/nuclide.cpp`) instead draws the band in **both** bracketing tables
//!    with the same `xi` and interpolates each partial between them (lin-lin
//!    for `INT = 2`, which ACER always writes).
//! 2. `Nuclide::xs_at_energy_urr` set the total from the band's own total
//!    column. OpenMC rebuilds it as `elastic + inelastic + capture + fission`,
//!    with `inelastic` the smooth cross section of the reaction the UNR block's
//!    `ILF` names. The difference matters because the collision kernel picks
//!    elastic as the remainder of the total.
//!
//! Both routes share this code: the ENDF route's PURR tables and the ACE
//! route's UNR block feed the same `UrrProbabilityTables`.
//!
//! # Methodology
//!
//! Reference: OpenMC's reader of the NJOY2016 U-234/235/238 tables in
//! `reference-data/ace`, with `calculate_urr_xs` transcribed in
//! `verification_and_validation/ace_route_physics/openmc_inputs/urr_sampling_reference.py`
//! (OpenMC 0.16.1.dev25, `d7d3284a1`), giving
//! `data/urr_sampling_openmc.csv`. It covers table energies and bin interiors
//! (quarter, mid and 0.3 points) crossed with six `xi` values, including 0.
//! The three nuclides between them exercise LSSF = 0 (U-234: barns) and
//! LSSF = 1 (U-235, U-238: factors on the smooth values), and ILF = 51 and
//! ILF = 4.
//!
//! For every row, `Nuclide::xs_at_energy_urr(E, 293.6, xi)` on the nuclide
//! built by `Nuclide::from_ace` must reproduce OpenMC's elastic, fission,
//! capture (= absorption − fission), inelastic and total. The tolerance is
//! **1e-9 relative** (with a 1e-12 b floor): both sides do the same lin-lin
//! arithmetic on the same numbers, so anything larger is a difference in
//! algorithm, not round-off. The total must also equal the sum of its partials
//! exactly as the kernel will read them.
//!
//! # Results (2026-09-29)
//!
//! **186 rows equal to OpenMC, worst relative difference 0.0** (bit-equal
//! arithmetic). Mutations, run and reverted: restoring the old nearest-point
//! sampler fails at the first bin-interior row (U-234, 1600.0005 eV, elastic
//! off by 35 %); restoring the band-total rule fails at U-234 1700 eV (total
//! off by 9.4e-4). Record, including the k_eff effect (consistent with zero,
//! as predicted before measuring):
//! `verification_and_validation/ace_route_physics/urr_sampling_2026-09-29.md`.

use njoy_outram_park_fork::acer::read;
use njoy_outram_park_fork::reference_data::ace_reference_file_or_skip;
use outram_mc_libs::material::nuclide::Nuclide;
use std::collections::BTreeMap;
use std::path::PathBuf;

fn nuclide(name: &str) -> Option<Nuclide> {
    let p = ace_reference_file_or_skip(
        &format!("reference-njoy/endf-b-viii.0/293.6K/{name}.ace.gz"),
        &format!("urr sampling/{name}"),
    )?;
    let raw = read::read(&p).expect("read ACE");
    Some(Nuclide::from_ace(&raw, name).expect("from_ace"))
}

#[test]
fn urr_cross_sections_match_openmc_calculate_urr_xs() {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("verification_and_validation/ace_route_physics/data/urr_sampling_openmc.csv");
    let text = std::fs::read_to_string(&p).expect("reference csv");
    let mut cache: BTreeMap<String, Option<Nuclide>> = BTreeMap::new();
    let (mut n, mut worst) = (0usize, 0.0f64);
    for line in text
        .lines()
        .filter(|l| !l.starts_with('#') && !l.starts_with("nuclide"))
    {
        let f: Vec<&str> = line.split(',').collect();
        let name = f[0].to_string();
        let v: Vec<f64> = f[1..].iter().map(|x| x.parse().unwrap()).collect();
        let (e, xi) = (v[0], v[1]);
        let want = [v[2], v[3], v[4], v[5], v[6]]; // el, fis, cap, inel, total
        let nuc = cache.entry(name.clone()).or_insert_with(|| nuclide(&name));
        let Some(nuc) = nuc.as_ref() else { continue };
        let x = nuc.xs_at_energy_urr(e, 293.6, xi);
        let got = [
            x.elastic,
            x.fission,
            x.absorption - x.fission,
            x.inelastic,
            x.total,
        ];
        for (k, label) in ["elastic", "fission", "capture", "inelastic", "total"]
            .iter()
            .enumerate()
        {
            let rel = (got[k] - want[k]).abs() / want[k].abs().max(1e-12);
            let rel = if (got[k] - want[k]).abs() < 1e-12 {
                0.0
            } else {
                rel
            };
            worst = worst.max(rel);
            assert!(
                rel < 1e-9,
                "{name} E = {e} xi = {xi}: {label} = {} vs OpenMC {} (rel {rel:.2e})",
                got[k],
                want[k]
            );
        }
        let sum = x.elastic + x.inelastic + x.absorption;
        assert!(
            (x.total - sum).abs() <= 1e-12 * x.total.abs().max(1.0),
            "{name} E = {e}: total {} is not the sum of its partials {sum}",
            x.total
        );
        n += 1;
    }
    println!("{n} (nuclide, E, xi) rows equal to OpenMC; worst relative difference {worst:.2e}");
    if cache.values().any(Option::is_some) {
        assert!(n >= 150, "expected the full reference set, compared {n}");
    }
}
