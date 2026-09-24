// SPDX-License-Identifier: GPL-3.0

//! **Cross-code: reading a library OpenMC itself produced** — GitHub #303.
//!
//! A round trip through this crate's own writer and reader proves the two
//! halves of one codec agree. It cannot detect a format that is
//! self-consistent and wrong — which is the finding #303 and #304 were both
//! filed on. These tests read a file **upstream produced**:
//!
//! ```text
//! reference-data/endf  --NJOY2016-->  ACE
//!                      --openmc.data.IncidentNeutron.from_ace-->
//!                      --.export_to_hdf5()-->  U235.h5
//! ```
//!
//! so the conversion is the reference implementation's own, not a
//! reimplementation of it.
//!
//! # Where the file comes from, and why these tests skip without it
//!
//! The library is a **generated artefact**, not repository content: ~74 MB for
//! three nuclides, built by `scripts`-driven NJOY + OpenMC runs. It is
//! therefore not committed, and these tests **skip with a printed reason**
//! rather than fail when it is absent, in the same spirit as the crate's other
//! reference-data-dependent gates. Point `OUTRAM_OPENMC_XS_DIR` at a directory
//! holding `U235.h5` (and optionally `U238.h5`, `U234.h5`) to run them.
//!
//! # Results, 2026-09-24
//!
//! Printed at run time; the recorded values are in
//! `verification_and_validation/nuclide_h5_read/`.

use std::path::PathBuf;

use njoy_outram_park_fork::hdf5::nuclide_read::read_nuclide;

/// The directory holding an OpenMC-produced neutron library, if one is
/// available.
fn xs_dir() -> Option<PathBuf> {
    if let Ok(d) = std::env::var("OUTRAM_OPENMC_XS_DIR") {
        let p = PathBuf::from(d);
        if p.join("U235.h5").exists() {
            return Some(p);
        }
    }
    None
}

fn skip(what: &str) {
    println!(
        "SKIP {what}: no OpenMC-produced neutron library. Set \
         OUTRAM_OPENMC_XS_DIR to a directory containing U235.h5 (built by \
         NJOY2016 ACE -> openmc.data.IncidentNeutron.from_ace -> \
         export_to_hdf5). This is a generated ~74 MB artefact and is \
         deliberately not committed."
    );
}

/// **THE #303 GATE: this crate reads a real fissile nuclide OpenMC wrote.**
///
/// Not a file of our own making. Every field asserted here is one a consumer
/// needs in order to transport: the header, the temperature grid, the energy
/// grid, the per-MT cross sections with their thresholds, ν̄, and the URR
/// tables.
#[test]
fn we_read_the_u235_library_openmc_produced() {
    let Some(d) = xs_dir() else {
        return skip("we_read_the_u235_library_openmc_produced");
    };
    let n = read_nuclide(d.join("U235.h5")).expect("U235.h5 must read");

    assert_eq!(n.name, "U235");
    assert_eq!(n.z, 92);
    assert_eq!(n.a, 235);
    assert_eq!(n.metastable, 0);
    assert!(
        (n.atomic_weight_ratio - 233.0248).abs() < 1.0e-4,
        "awr {} is not U-235's 233.0248",
        n.atomic_weight_ratio
    );

    let e = n.any_energy().expect("an energy grid");
    assert!(
        e.len() > 50_000,
        "a real U-235 grid is tens of thousands of points, got {}",
        e.len()
    );
    assert!(
        e.windows(2).all(|w| w[1] > w[0]),
        "the energy grid must be strictly ascending"
    );
    assert!(!n.kts.is_empty(), "at least one temperature");

    // The reactions a criticality calculation needs.
    for mt in [2, 18, 102] {
        assert!(
            n.reactions.contains_key(&mt),
            "U-235 must carry MT={mt}; got {:?}",
            n.reactions.keys().take(12).collect::<Vec<_>>()
        );
    }
    assert!(n.is_fissionable(), "U-235 is fissionable");

    // Total nu-bar, and its magnitude. 2.4 at thermal rising past 3 at 10 MeV
    // is U-235's textbook behaviour; a reader that mixed up the vstack halves
    // would return an ENERGY here, not a yield, so the bound is the check.
    let nu_th = n.nu_total(0.0253).expect("total_nu must be present");
    let nu_fast = n.nu_total(1.0e7).expect("total_nu at 10 MeV");
    println!("U235 nu_total: thermal {nu_th:.5}, 10 MeV {nu_fast:.5}");
    assert!(
        (2.0..2.8).contains(&nu_th),
        "thermal nu-bar {nu_th} is not near U-235's 2.43 -- if this is ~1e-5 \
         the reader has returned the x half of the vstack instead of the y half"
    );
    assert!(
        nu_fast > nu_th,
        "nu-bar must rise with energy: {nu_fast} at 10 MeV against {nu_th} thermal"
    );

    // Threshold reactions: the invariant this crate now enforces on write.
    let mut thresholded = 0;
    for (mt, rx) in &n.reactions {
        assert_eq!(
            rx.threshold_idx + rx.xs.len(),
            e.len(),
            "MT={mt}: threshold_idx + len(xs) must equal the grid length"
        );
        if rx.threshold_idx > 0 {
            thresholded += 1;
            assert_eq!(
                rx.xs[0], 0.0,
                "MT={mt}: upstream stores exactly zero AT the threshold point"
            );
        }
    }
    assert!(
        thresholded > 30,
        "U-235 has ~40 discrete levels plus (n,xn); only {thresholded} threshold \
         reactions were read"
    );
    println!(
        "U235: {} reactions, {} with a threshold, {} grid points, temps {:?}, \
         urr at {:?}",
        n.reactions.len(),
        thresholded,
        e.len(),
        n.kts.keys().collect::<Vec<_>>(),
        n.urr_temperatures
    );
    assert!(
        !n.urr_temperatures.is_empty(),
        "U-235's library carries unresolved-resonance probability tables"
    );
}

/// **The law inventory the reader sees must match what upstream wrote.**
///
/// Independently measured with `h5py` on the same file: U-235 and U-238 use
/// `{level: 39, continuous, correlated}` for their neutron products while
/// U-234 uses `kalbach-mann`. This crate's ACE-side decoder
/// (`crate::acer::ce_laws`) records the same split from the ACE laws
/// (`{LAW3: 39, LAW4, LAW61}` and `{LAW3: 40, LAW4, LAW44}`), reached by a
/// different route — so agreement here is a cross-check of two independent
/// paths, not a restatement of one.
#[test]
fn the_law_inventory_matches_what_upstream_wrote() {
    let Some(d) = xs_dir() else {
        return skip("the_law_inventory_matches_what_upstream_wrote");
    };
    for (file, expect_kalbach) in [("U235.h5", false), ("U234.h5", true)] {
        let p = d.join(file);
        if !p.exists() {
            println!("SKIP {file}: not present");
            continue;
        }
        let n = read_nuclide(&p).unwrap();
        let mut counts: std::collections::BTreeMap<String, usize> = Default::default();
        for rx in n.reactions.values() {
            for (law, part) in rx.product_laws.iter().zip(&rx.product_particles) {
                if part == "neutron" {
                    *counts.entry(law.clone()).or_default() += 1;
                }
            }
        }
        println!("{file} neutron-product laws: {counts:?}");
        assert!(
            counts.get("uncorrelated").copied().unwrap_or(0) > 30,
            "{file}: the discrete levels are uncorrelated(angle + level); got {counts:?}"
        );
        if expect_kalbach {
            assert!(
                counts.contains_key("kalbach-mann"),
                "{file}: U-234's continuum laws are Kalbach-Mann; got {counts:?}"
            );
        } else {
            assert!(
                counts.contains_key("correlated"),
                "{file}: U-235/U-238's continuum laws are correlated; got {counts:?}"
            );
        }
    }
}

/// **A file this crate wrote is read back by OpenMC and transported** — the
/// #304 gate, asserted from the Rust side.
///
/// The writer emits a fissile nuclide whose cross sections are all flat, so
/// `k_inf = nu * Sigma_f / Sigma_a = 2.5 * 2.0 / 2.5 = 2.0` **exactly** and
/// independently of the fission spectrum, the scattering law and the grid.
/// This test writes the file; running OpenMC on it is done by the driver in
/// `verification_and_validation/nuclide_h5_write_fissile/openmc_inputs/`,
/// because OpenMC is not a build dependency of this crate.
///
/// # Results, 2026-09-24
///
/// OpenMC read every field back and transported it: `k = 1.99854` on the first
/// generation against the analytic 2.0. The converged value is recorded in the
/// V&V note.
#[test]
fn the_fissile_file_this_crate_writes_is_structurally_complete() {
    use njoy_outram_park_fork::hdf5::nuclide_read::read_nuclide;

    // Rebuild the fissile nuclide the unit tests write, through the public API,
    // and check the three invariants that cost OpenMC a segfault to find.
    let p = std::env::temp_dir()
        .join("njoy_nuclide_write_test")
        .join("SynF.h5");
    if !p.exists() {
        println!(
            "SKIP: run the crate's unit tests first -- \
             `cargo test --release -p njoy-outram-park-fork --lib hdf5::nuclide_write` \
             writes {}",
            p.display()
        );
        return;
    }
    let n = read_nuclide(&p).expect("the fissile file must read back");
    assert!(n.is_fissionable(), "MT=18 must be present");
    assert!(n.total_nu.is_some(), "total_nu must be present");
    let nu = n.nu_total(1.0e6).unwrap();
    assert!((nu - 2.5).abs() < 1e-9, "nu-bar {nu} is not the 2.5 written");

    let e = n.any_energy().unwrap().clone();
    for (mt, rx) in &n.reactions {
        assert_eq!(
            rx.threshold_idx + rx.xs.len(),
            e.len(),
            "MT={mt} violates the threshold invariant"
        );
        if rx.threshold_idx > 0 {
            assert_eq!(rx.xs[0], 0.0, "MT={mt}: zero at the threshold point");
        }
    }
    // The analytic k_inf the V&V note gates on, recomputed from the file itself
    // rather than from the test that wrote it.
    let sig_f = rx_at(&n, 18, &e, 1.0e6);
    let sig_c = rx_at(&n, 102, &e, 1.0e6);
    let k_inf = nu * sig_f / (sig_f + sig_c);
    println!(
        "from the file: nu={nu}, sigma_f={sig_f}, sigma_c={sig_c}, k_inf={k_inf:.6}"
    );
    assert!(
        (k_inf - 2.0).abs() < 1.0e-9,
        "the file's own numbers must give k_inf = 2.0 exactly, got {k_inf}"
    );
}

fn rx_at(n: &njoy_outram_park_fork::hdf5::nuclide_read::ReadNuclide, mt: i32, e: &[f64], at: f64) -> f64 {
    let rx = &n.reactions[&mt];
    let full = rx.xs_on_grid(e.len());
    let i = e.partition_point(|&v| v <= at).saturating_sub(1);
    full[i]
}

/// **Reproducer for an upstream OpenMC robustness defect**, written from
/// entirely *conventional* data — GitHub #306.
///
/// The segfault found while verifying #304 was first triggered by a cross
/// section that **stepped** from 0 to 1 barn at a `level` law's threshold, which
/// real evaluations do not do. That alone would make it garbage-in rather than a
/// defect. This test emits a file that breaks **no** convention:
///
/// - the threshold sits exactly on a grid point;
/// - the cross section is exactly zero there and rises smoothly above it;
/// - the grid starts at 1 keV, which is legal and common (a fast-only library
///   need not extend to 1e-5 eV).
///
/// The last point is what makes the window visible rather than rare.
/// `LevelInelastic::sample` returns `mass_ratio * (E - threshold)` unclamped, so
/// a collision just above the threshold emits a neutron far below the library's
/// minimum energy. OpenMC's neutron `energy_cutoff` defaults to **0.0**
/// (`src/settings.cpp:114`), so `physics.cpp:81` kills a *negative* energy but
/// **not** a positive sub-minimum one — and `Material::calculate_neutron_xs`
/// then computes
///
/// ```cpp
/// int i_grid = std::log(p.E() / data::energy_min[neutron]) / simulation::log_spacing;
/// ```
///
/// (`src/material.cpp:832`) which is **negative** for `E < energy_min` and is
/// used to index the cross-section arrays without a bounds check.
///
/// This test only **writes** the file; running OpenMC on it is the reproducer
/// step, documented in
/// `verification_and_validation/nuclide_h5_fissile/openmc_inputs/`. It is
/// `#[ignore]`d because its product is an input to a crash, not an assertion.
#[test]
#[ignore = "emits a reproducer file for an upstream segfault; run explicitly"]
fn emit_sub_minimum_level_emission_reproducer() {
    use njoy_outram_park_fork::hdf5::nuclide_laws::{
        AngleDistribution, AngleEnergy, EnergyDist, Product,
    };
    use njoy_outram_park_fork::hdf5::nuclide_write::{write_nuclide, NuclideData, ReactionData};

    // A fast-only grid: 1 keV to 20 MeV. Entirely legal, and it puts the
    // library minimum four decades above where the level law can emit.
    let n_pts = 200usize;
    let mut energy: Vec<f64> = (0..n_pts)
        .map(|i| {
            let (lo, hi) = (1.0e3_f64.ln(), 2.0e7_f64.ln());
            (lo + (hi - lo) * i as f64 / (n_pts - 1) as f64).exp()
        })
        .collect();

    let awr = 55.0; // roughly iron, so the mass ratio is unremarkable
    let q = -1.0e5_f64;
    let threshold = q.abs() * (awr + 1.0) / awr;
    let ti = energy.iter().position(|&x| x > threshold).unwrap();
    energy.insert(ti, threshold);
    let n = energy.len();
    let (lo, hi) = (energy[0], energy[n - 1]);

    // MT=51: exactly zero at the threshold, rising SMOOTHLY above it, which is
    // what every real evaluation does.
    let mut inel = vec![0.0; n];
    for (i, v) in inel.iter_mut().enumerate().skip(ti + 1) {
        let x = (i - ti) as f64 / (n - ti) as f64;
        *v = 2.0 * x; // linear rise from zero
    }

    let d = NuclideData {
        name: "LvMin".into(),
        z: 26,
        a: 56,
        metastable: 0,
        atomic_weight_ratio: awr,
        temperature: "294K".into(),
        kt_ev: 2.5301e-2,
        energy: energy.clone(),
        reactions: vec![
            ReactionData::elastic(
                vec![5.0; n],
                AngleDistribution::isotropic(vec![lo, hi]),
                lo,
                hi,
            )
            .unwrap(),
            ReactionData::capture(1.0e6, vec![0.1; n]),
            ReactionData::from_full_grid(
                51,
                q,
                true,
                &inel,
                vec![Product::prompt_neutron(
                    AngleEnergy::Uncorrelated {
                        angle: Some(AngleDistribution::isotropic(vec![threshold, hi])),
                        energy: Some(EnergyDist::Level {
                            threshold,
                            mass_ratio: (awr / (awr + 1.0)).powi(2),
                        }),
                    },
                    threshold,
                    hi,
                )
                .unwrap()],
            ),
        ],
        total_nu: None,
        urr: vec![],
    };

    let out = std::env::var("OUTRAM_REPRO_DIR").unwrap_or_else(|_| {
        std::env::temp_dir()
            .join("njoy_level_repro")
            .to_string_lossy()
            .into_owned()
    });
    std::fs::create_dir_all(&out).unwrap();
    let p = std::path::Path::new(&out).join("LvMin.h5");
    write_nuclide(&p, &d).expect("this file breaks no convention, so it must write");
    println!(
        "wrote {} -- grid {:.3e}..{:.3e} eV ({n} pts), MT=51 threshold {threshold:.6} eV \
         at index {ti}, xs zero there and rising linearly.\n\
         A collision just above the threshold emits \
         mass_ratio*(E - threshold) << 1 keV, i.e. BELOW the library minimum.",
        p.display(),
        lo,
        hi
    );
}
