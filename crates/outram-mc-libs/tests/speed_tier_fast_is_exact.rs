// SPDX-License-Identifier: GPL-3.0

//! **`SpeedTier::Fast` must give exactly the numbers `SpeedTier::Standard`
//! gives.** It is an optimization, not an approximation (GitHub #349).
//!
//! # Methodology
//!
//! Each nuclide is built **once**, from `reference-data/endf/` at 293.6 K and
//! RECONR/BROADR tolerance `1e-3`. The `Standard` copy is a clone with
//! `.with_speed(SpeedTier::Standard)`, so the two share their data and differ
//! only in the lookup, which is exactly what `Fast` changes. At 508 energies
//! from 1e-5 eV to 20 MeV (plus the thermal range densely), and at every
//! tabulated point of U-238's first inelastic level (a threshold), the test
//! requires, **bit for bit**:
//!
//! - every field of `xs_at_energy` (`total`, `elastic`, `fission`,
//!   `absorption`, `inelastic`, `n2n`, `n3n`, `mt5`, `nu_fission`);
//! - `total_at_energy`;
//! - `sample_inelastic`'s choice and the seed it leaves, over 64 draws;
//! - and, for a mixture, `Material::sample_nuclide`'s index sequence over
//!   2000 draws with the same seeds.
//!
//! Nuclides, and the branch each exercises:
//! - U-238: 40 inelastic levels, the deepest resonance structure here, and the
//!   MT=102 + charged-particle absorption sum.
//! - U-234: fissile, with an unresolved range.
//! - F-19: inelastic levels opening at thresholds (zero below them).
//! - H-1 carrying `c_H_in_H2O`: the S(a,b) branch, where the total is rebuilt
//!   instead of read from MT=1.
//!
//! **Pass criterion:** no difference at all. There is no tolerance: the two
//! paths share the interpolation code (`ReconrResult::eval_section`), so any
//! difference is a logic error, not rounding.
//!
//! # Results, 2026-09-27
//!
//! All four nuclides: identical in every compared value. The per-nuclide
//! counts are printed by the test.

use njoy_outram_park_fork::reference_data::reference_endf;
use outram_mc_libs::material::material::{Material, NuclideComponent};
use outram_mc_libs::material::nuclide::{MicroXS, Nuclide};
use outram_mc_libs::material::speed::SpeedTier;

const TEMP_K: f64 = 293.6;

fn probe_energies() -> Vec<f64> {
    let mut es = Vec::new();
    for i in 0..=400 {
        let f = i as f64 / 400.0;
        es.push(1.0e-5 * (2.0e7_f64 / 1.0e-5).powf(f));
    }
    for i in 0..=100 {
        es.push(1.0e-4 + i as f64 * (5.0 - 1.0e-4) / 100.0);
    }
    // Around U-238's and F-19's first inelastic thresholds (45 keV, 110 keV):
    // below, at and above them.
    for &t in &[44_900.0, 45_000.0, 45_100.0, 109_000.0, 115_840.0, 116_000.0] {
        es.push(t);
    }
    es
}

fn bits(x: &MicroXS) -> [u64; 9] {
    [
        x.total.to_bits(),
        x.elastic.to_bits(),
        x.fission.to_bits(),
        x.absorption.to_bits(),
        x.inelastic.to_bits(),
        x.n2n.to_bits(),
        x.n3n.to_bits(),
        x.mt5.to_bits(),
        x.nu_fission.to_bits(),
    ]
}

/// Compare the `Fast` nuclide against its `Standard` twin; returns the number
/// of values compared.
fn assert_identical(fast: &Nuclide, label: &str) -> usize {
    assert_eq!(fast.speed(), SpeedTier::Fast, "{label}: built at the wrong tier");
    let std = fast.clone().with_speed(SpeedTier::Standard);
    let mut compared = 0usize;
    for e in probe_energies() {
        let (a, b) = (fast.xs_at_energy(e, TEMP_K), std.xs_at_energy(e, TEMP_K));
        assert_eq!(bits(&a), bits(&b), "{label}: xs_at_energy differs at {e:e} eV: fast {a:?}, standard {b:?}");
        let (ta, tb) = (fast.total_at_energy(e, TEMP_K), std.total_at_energy(e, TEMP_K));
        assert_eq!(ta.to_bits(), tb.to_bits(), "{label}: total_at_energy differs at {e:e} eV");
        compared += 10;

        let (mut sa, mut sb) = (0x9E37_79B9_7F4A_7C15_u64 ^ e.to_bits(), 0x9E37_79B9_7F4A_7C15_u64 ^ e.to_bits());
        for _ in 0..64 {
            let (ia, ib) = (fast.sample_inelastic(e, &mut sa), std.sample_inelastic(e, &mut sb));
            assert_eq!(format!("{ia:?}"), format!("{ib:?}"), "{label}: sample_inelastic differs at {e:e} eV");
            assert_eq!(sa, sb, "{label}: sample_inelastic consumed a different number of draws at {e:e} eV");
            compared += 1;
        }
    }
    println!("{label}: {compared} values compared, all identical");
    compared
}

fn load(file: &str, name: &str) -> Option<Nuclide> {
    let Some(p) = reference_endf(file) else {
        eprintln!("SKIP {name}: {file} absent");
        return None;
    };
    Some(Nuclide::from_endf_file_with_speed(&p, name, TEMP_K, SpeedTier::Fast).unwrap_or_else(|e| panic!("{name}: {e}")))
}

#[test]
fn fast_matches_standard_on_u238_u234_and_f19_and_in_nuclide_selection() {
    let Some(u238) = load("n-092_U_238.endf", "U238") else { return };
    let Some(u234) = load("n-092_U_234-ENDF8.0.endf", "U234") else { return };
    let Some(f19) = load("n-009_F_019-ENDF8.0.endf", "F19") else { return };
    assert_identical(&u238, "U238");
    assert_identical(&u234, "U234");
    assert_identical(&f19, "F19");

    // Nuclide selection over a three-component mixture, same seeds, both tiers.
    let fast = vec![u238.clone(), u234.clone(), f19.clone()];
    let std: Vec<Nuclide> = fast.iter().cloned().map(|n| n.with_speed(SpeedTier::Standard)).collect();
    let mat = Material {
        id: 1,
        name: "mix".into(),
        components: vec![
            NuclideComponent { nuclide_idx: 0, atom_density: 0.04 },
            NuclideComponent { nuclide_idx: 1, atom_density: 0.002 },
            NuclideComponent { nuclide_idx: 2, atom_density: 0.01 },
        ],
        temperature: TEMP_K,
    };
    let (mut sa, mut sb) = (12345_u64, 12345_u64);
    let mut picks = 0usize;
    for (k, e) in probe_energies().into_iter().cycle().take(2000).enumerate() {
        let (a, b) = (mat.sample_nuclide(e, &mut sa, &fast), mat.sample_nuclide(e, &mut sb, &std));
        assert_eq!(a, b, "sample_nuclide differs at draw {k}, E = {e:e} eV");
        assert_eq!(sa, sb, "sample_nuclide consumed different draws at {k}");
        picks += 1;
    }
    println!("mixture: {picks} nuclide selections compared, all identical");
}

#[test]
fn fast_matches_standard_through_the_sab_cutoff() {
    let Some(h1) = load("n-001_H_001-ENDF8.0-Beta6.endf", "H1") else { return };
    assert_identical(&h1, "H1 (free gas)");
    let Some(sab_tape) = reference_endf("tsl-HinH2O.endf") else {
        eprintln!("SKIP bound H1: tsl-HinH2O tape absent (free-gas arm checked)");
        return;
    };
    let sab = outram_mc_libs::material::thermal::ThermalScattering::from_endf_file(
        sab_tape.to_str().expect("path"),
        1,
        TEMP_K,
        "c_H_in_H2O",
    )
    .expect("c_H_in_H2O");
    assert_identical(&h1.with_thermal_scattering(sab), "H1 (c_H_in_H2O)");
}

/// A nuclide built by the ordinary constructors gets the default tier, which
/// is `Fast`: the cheapest EXACT path is the default (crate `CLAUDE.md`).
#[test]
fn ordinary_constructors_default_to_the_exact_fast_tier() {
    let Some(p) = reference_endf("n-009_F_019-ENDF8.0.endf") else {
        eprintln!("SKIP: F-19 absent");
        return;
    };
    let n = Nuclide::from_endf_file(&p, "F19", TEMP_K, 1.0e-3).expect("F19");
    assert_eq!(n.speed(), SpeedTier::Fast);
    assert!(n.speed().is_exact());
}
