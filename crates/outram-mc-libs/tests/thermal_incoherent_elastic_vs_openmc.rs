// SPDX-License-Identifier: GPL-3.0

//! **Incoherent-elastic S(α,β) cosines are sampled as OpenMC samples them** —
//! GitHub #365 audit.
//!
//! # What was wrong
//!
//! `IncoherentElasticTable::sample` took the **nearest** incident table's
//! equiprobable cosine, with no interpolation and no smearing, while its doc
//! said it mirrored OpenMC. OpenMC's `IncoherentElasticAEDiscrete::sample`
//! (`src/secondary_thermal.cpp`) interpolates the cosine between the two
//! bracketing tables and smears it over half the distance to its neighbours.
//! This applies to both routes wherever a scatterer has incoherent elastic
//! (H in ZrH, for example).
//!
//! # Methodology
//!
//! Table: NJOY2016 H in ZrH (ENDF/B-VIII.0, MAT 7) at 296 K, `IDPNC = 3`,
//! 16 cosines per energy. It is built into `target/ace_extra/HZrH/tape30` by
//! the deck in `target/ace_extra/HZrH/input` (recorded on #365); the test
//! skips when the table is absent.
//!
//! Reference: exact `<mu>` and `<mu^2>` of OpenMC's scheme on OpenMC's reader
//! of the same file
//! (`verification_and_validation/ace_route_physics/openmc_inputs/thermal_incoherent_elastic_reference.py`
//! → `data/thermal_incoherent_elastic_openmc.csv`). `N = 1e6` draws of the
//! elastic channel alone (`ThermalScattering::elastic().sample`) at six
//! energies. Pass: both means within 5 sample-sem of the reference.
//!
//! # Results (2026-09-29)
//!
//! Printed by the test and recorded in the #365 thread.

use outram_mc_libs::material::thermal::ThermalScattering;
use std::path::PathBuf;

const N: usize = 1_000_000;

#[test]
fn incoherent_elastic_cosines_match_openmc() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let tape = root.join("../../target/ace_extra/HZrH/tape30");
    if !tape.is_file() {
        println!("{} absent: skipping", tape.display());
        return;
    }
    let raw = njoy_outram_park_fork::acer::read::read(&tape).expect("read");
    let th = ThermalScattering::from_ace(&raw, "H in ZrH").expect("load");
    let el = th.elastic();
    let csv = root.join(
        "verification_and_validation/ace_route_physics/data/thermal_incoherent_elastic_openmc.csv",
    );
    let text = std::fs::read_to_string(csv).expect("reference");
    let mut rows = 0;
    for line in text
        .lines()
        .filter(|l| !l.starts_with('#') && !l.is_empty())
    {
        let v: Vec<f64> = line.split(',').map(|x| x.parse().unwrap()).collect();
        let (e, want, want2) = (v[0], v[1], v[2]);
        let mut seed = 0x1E1A_0000 + rows as u64;
        let mus: Vec<f64> = (0..N)
            .map(|_| el.sample(e, &mut seed).expect("elastic").1)
            .collect();
        let stat = |g: &dyn Fn(f64) -> f64| {
            let m = mus.iter().map(|&x| g(x)).sum::<f64>() / N as f64;
            let var = mus.iter().map(|&x| (g(x) - m).powi(2)).sum::<f64>() / (N - 1) as f64;
            (m, (var / N as f64).sqrt())
        };
        let (m1, s1) = stat(&|x| x);
        let (m2, s2) = stat(&|x| x * x);
        let (z1, z2) = ((m1 - want) / s1, (m2 - want2) / s2);
        println!("E = {e} eV: <mu> {m1:.5} (OpenMC {want:.5}, z {z1:+.2}); <mu^2> z {z2:+.2}");
        assert!(z1.abs() < 5.0 && z2.abs() < 5.0, "E = {e}: z = {z1}, {z2}");
        rows += 1;
    }
    assert_eq!(rows, 6);
}
