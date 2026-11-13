//! # The fresh EBR-II MARK-II driver subassembly: geometry, composition and
//! ARC-228 Table 3.3 volume fractions (gh #1329)
//!
//! Reaches every function of `examples/common/ebr2_fresh_driver_model.rs`,
//! the model `examples/ebr2_fresh_driver_kinf.rs` runs. Nothing is
//! transported here (no nuclear data is processed: the tapes are read for
//! their headers and masses only), so it is fast.
//!
//! > Research, education and V&V only. AI draft, unreviewed; every input
//! > is an AI transcription (cited in the model file).
//!
//! ## Methodology, and the tolerances, set before the first run
//!
//! 1. **Pins and fit.** 6 rings give 91 pins (ARC-228 Table 3.1.1, p.17);
//!    the builder's exact checks pass (every pin with its wire inside the
//!    duct, no overlaps). Predicted clearance: the bundle's flat-to-flat
//!    envelope `sqrt(3)*5*p + D + 2s` = 5.5913 cm against a 5.6134 cm duct.
//! 2. **Volume fractions, analytic**, against ARC-228 Table 3.3 (p.23),
//!    fuel / 316SS / Na = 26.92 / 22.29 / 50.79 % over the lattice cell.
//!    Predicted 26.93 / 22.33 / 50.74 % (the helical wire adds 0.68 % to the
//!    wire's area; Table 3.3's wire is 1.245 mm, Table 3.1.1's 1.24).
//!    **Tolerance: 0.10 percentage points** on each, about the rounding of
//!    the printed values plus the 1.24/1.245 mm wire difference (0.03
//!    points) plus the helix factor (0.03 points).
//! 3. **Volume fractions of the ASSEMBLED geometry** (cell lookup on a
//!    1201 x 1201 grid at mid-height): within **1e-3** (absolute) of the
//!    analytic ones and **0.15 percentage points** of Table 3.3; no point
//!    unlocated.
//! 4. **Composition.** Mass density rebuilt from the atom densities and the
//!    tapes' masses: U-5Fs 18.2 x (1 - 0.0244) = 17.7559 g/cm³ in arm A
//!    (Ru, Rh, Pd, Nb omitted, not renormalised), 316SS 8.00 g/cm³; arm D
//!    (no Ni) 316SS 7.04 g/cm³; arm B fuel 18.2 x (1 - 0.0500) = 17.29
//!    g/cm³. Tolerance 1e-6 relative (only the mass-to-AWR round trip).
//!    U-235 is 67.0 wt% of the uranium. Arm E holds the same number of Mo
//!    atoms as A, all Mo-95; arm G holds A's Mo plus the Ru, Rh, Pd and Nb
//!    atoms counted with IUPAC weights.
//! 5. **Every arm assembles** with a reflective boundary.
//!
//! ## Results (2026-11-10, release)
//!
//! 1. 91 pins; checks pass; envelope **5.5913 cm** in the 5.6134 cm duct,
//!    P/D 1.2806. As predicted.
//! 2. Analytic **26.918 / 22.297 / 50.785 %** against 26.92 / 22.29 /
//!    50.79 %: worst 0.007 points. The prediction (26.93 / 22.33 / 50.74)
//!    was off by up to 0.04 points: it used Table 3.3's rounder cm
//!    dimensions, the model uses Table 3.1.1's mm ones.
//! 3. Sampled (1 081 801 points, none unlocated) **26.916 / 22.295 /
//!    50.789 %**, within 3.4e-5 of analytic. **The first grid aliased**: a
//!    plain grid aligned with the duct's flats gave 316SS 22.202 % at
//!    1201² and 22.386 % at 2003² (one row of points in or out of the
//!    0.1 cm duct wall), passing the 1e-3 gate by luck at 1201². The
//!    sampler now offsets each point by a row-dependent irrational fraction;
//!    at 2003² it gives 22.299 %. The tolerances were not changed.
//! 4. U-5Fs 17.75592 g/cm³ and 316SS 8.00000 g/cm³ in arm A; U-235
//!    2.968053e-2 /b-cm; arm G adds 2.631179e-3 /b-cm of Mo to A's
//!    2.810076e-3. All composition assertions pass.
//! 5. All 7 arms assemble (A 39 nuclides, B 22, C 23, D 34, E 33, F 32,
//!    G 39). Record:
//!    `verification_and_validation/ebr2_fresh_driver/README.md`.
//!
//! Skips (passes with a message) when the reference tapes or the ACE
//! submodule's `endf/` tapes are absent.

#[path = "../examples/common/ebr2_fresh_driver_model.rs"]
#[allow(dead_code)]
mod ebr2;

use ebr2::*;
use njoy_outram_park_fork::reference_data::{ace_submodule_dir, reference_endf_dir};
use outram_mc_libs::model::prelude::*;
use outram_mc_libs::model::{pins_in_rings, AtomicMasses, Isotope, AVOGADRO_PER_MOL};

fn dirs() -> Option<(std::path::PathBuf, std::path::PathBuf)> {
    let endf = reference_endf_dir();
    let ace = ace_submodule_dir().join("endf/endf-b-viii.0");
    let all = tape_paths(&endf, &ace).iter().all(|p| p.is_file());
    if all {
        Some((endf, ace))
    } else {
        eprintln!("SKIP: reference ENDF tapes or reference-data/ace/endf not present");
        None
    }
}

/// g/cm³ from `(isotope, atoms/b-cm)` with the library's masses.
fn mass_density(masses: &AtomicMasses, atoms: &[(Isotope, f64)]) -> f64 {
    atoms
        .iter()
        .map(|(i, n)| n * 1e24 * masses.mass_u(*i).expect("mass in library") / AVOGADRO_PER_MOL)
        .sum()
}

fn element_atoms(atoms: &[(Isotope, f64)], z: u8) -> f64 {
    atoms.iter().filter(|(i, _)| i.z() == z).map(|(_, n)| n).sum()
}

#[test]
fn pins_fit_and_analytic_fractions_match_arc228_table_3_3() {
    let sa = driver_subassembly();
    assert_eq!(sa.pin_count(), 91);
    assert_eq!(pins_in_rings(RINGS), 91);
    sa.check().unwrap();
    let envelope = 3.0_f64.sqrt() * 5.0 * pin_pitch_mm() + CLAD_OD_MM + 2.0 * WIRE_DIAMETER_MM;
    println!("bundle envelope {envelope:.4} mm in a {DUCT_INSIDE_MM} mm duct; P/D {:.4}", pin_pitch_mm() / CLAD_OD_MM);
    assert!(envelope < DUCT_INSIDE_MM);
    let f = analytic_cell_fractions(&sa);
    for (k, name) in ["fuel", "316SS", "Na"].iter().enumerate() {
        println!(
            "{name:>6}: analytic {:.4} %, ARC-228 Table 3.3 {:.2} %",
            100.0 * f[k],
            100.0 * TABLE_3_3_FRACTIONS[k]
        );
        assert!((f[k] - TABLE_3_3_FRACTIONS[k]).abs() < 1e-3, "{name}");
    }
}

#[test]
fn compositions_are_the_cited_ones_and_gaps_are_left_out_unrenormalised() {
    let Some((endf, ace)) = dirs() else { return };
    let data = library(&endf, &ace).unwrap();
    let masses = data.atomic_masses();
    let arms = GapTreatment::arms();
    assert_eq!(arms.len(), 7);
    let a = &arms[0];
    let fuel_a = u5fs_atoms(&masses, a).unwrap();
    let steel_a = steel_316_atoms(&masses, a).unwrap();
    let rel = |got: f64, want: f64| (got / want - 1.0).abs();
    let rho_fuel = mass_density(&masses, &fuel_a);
    let rho_steel = mass_density(&masses, &steel_a);
    println!("arm A: U-5Fs {rho_fuel:.5} g/cm3, 316SS {rho_steel:.5} g/cm3");
    assert!(rel(rho_fuel, 18.2 * (1.0 - 0.0244)) < 1e-6);
    assert!(rel(rho_steel, 8.00) < 1e-6);
    let u = |z235: bool| {
        let i = if z235 { U235 } else { U238 };
        fuel_a.iter().filter(|(j, _)| *j == i).map(|(j, n)| n * masses.mass_u(*j).unwrap()).sum::<f64>()
    };
    assert!(rel(u(true) / (u(true) + u(false)), 0.67) < 1e-9);
    println!("arm A U-235 {:.6e} /b-cm", fuel_a.iter().find(|(i, _)| *i == U235).unwrap().1);

    let by = |arm: &str| arms.iter().find(|t| t.arm == arm).unwrap();
    // B: fuel loses Zr, Mo, Ru, Rh, Pd, Nb; steel loses Ni and Mo.
    let fuel_b = u5fs_atoms(&masses, by("B")).unwrap();
    let steel_b = steel_316_atoms(&masses, by("B")).unwrap();
    assert!(rel(mass_density(&masses, &fuel_b), 18.2 * (1.0 - 0.0500)) < 1e-6);
    assert!(rel(mass_density(&masses, &steel_b), 8.00 * (1.0 - 0.145)) < 1e-6);
    // D: no nickel, steel 7.04 g/cm3.
    let steel_d = steel_316_atoms(&masses, by("D")).unwrap();
    assert_eq!(element_atoms(&steel_d, 28), 0.0);
    assert!(rel(mass_density(&masses, &steel_d), 7.04) < 1e-6);
    // E: Mo atoms conserved, all Mo-95; C likewise.
    let mo95 = Isotope::new(42, 95).unwrap();
    for arm in ["C", "E"] {
        let t = by(arm);
        for (got, want) in [
            (u5fs_atoms(&masses, t).unwrap(), &fuel_a),
            (steel_316_atoms(&masses, t).unwrap(), &steel_a),
        ] {
            let mo = element_atoms(&got, 42);
            assert!(rel(mo, element_atoms(want, 42)) < 1e-12, "{arm}");
            assert!(got.iter().filter(|(i, _)| i.z() == 42).all(|(i, _)| *i == mo95));
        }
    }
    // F: no Mo anywhere.
    assert_eq!(element_atoms(&u5fs_atoms(&masses, by("F")).unwrap(), 42), 0.0);
    assert_eq!(element_atoms(&steel_316_atoms(&masses, by("F")).unwrap(), 42), 0.0);
    // G: A's Mo plus the Ru, Rh, Pd, Nb atoms.
    let fp: f64 = [
        (Element::Ruthenium, 1.96),
        (Element::Rhodium, 0.28),
        (Element::Palladium, 0.19),
        (Element::Niobium, 0.01),
    ]
    .iter()
    .map(|(e, wt)| 18.2 * wt / 100.0 * AVOGADRO_PER_MOL / standard_atomic_weight(*e).unwrap() * 1e-24)
    .sum();
    let mo_g = element_atoms(&u5fs_atoms(&masses, by("G")).unwrap(), 42);
    println!("arm G: Mo {mo_g:.6e} = A's {:.6e} + Ru/Rh/Pd/Nb {fp:.6e} /b-cm", element_atoms(&fuel_a, 42));
    assert!(rel(mo_g, element_atoms(&fuel_a, 42) + fp) < 1e-12);
    assert!(standard_atomic_weight(Element::Molybdenum).is_none());
}

#[test]
fn every_arm_assembles_and_the_assembled_fractions_match_table_3_3() {
    let Some((endf, ace)) = dirs() else { return };
    let data = library(&endf, &ace).unwrap();
    let sa = driver_subassembly();
    let analytic = analytic_cell_fractions(&sa);
    for t in GapTreatment::arms() {
        let model = build_model(data.clone(), &t).unwrap();
        let a = model.assemble().unwrap();
        println!("arm {}: {} nuclides, {} materials", t.arm, a.nuclides.len(), a.materials.len());
        if t.arm != "A" {
            continue;
        }
        let (f, inside, lost) = sampled_cell_fractions(&a, &sa, 1201);
        assert_eq!(lost, 0, "unlocated points");
        for (k, name) in ["fuel", "316SS", "Na"].iter().enumerate() {
            println!(
                "{name:>6}: sampled {:.4} %, analytic {:.4} %, Table 3.3 {:.2} % ({inside} points)",
                100.0 * f[k],
                100.0 * analytic[k],
                100.0 * TABLE_3_3_FRACTIONS[k]
            );
            assert!((f[k] - analytic[k]).abs() < 1e-3, "{name} vs analytic");
            assert!((f[k] - TABLE_3_3_FRACTIONS[k]).abs() < 1.5e-3, "{name} vs Table 3.3");
        }
    }
}
