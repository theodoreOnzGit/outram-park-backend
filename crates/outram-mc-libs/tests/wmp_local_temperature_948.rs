// SPDX-License-Identifier: GPL-3.0

//! Pins for GitHub #948: windowed multipole (WMP) on a **pointwise** nuclide,
//! evaluated at the lookup temperature inside its range, as OpenMC does
//! (`src/nuclide.cpp:620-680`, `src/physics.cpp:734`).
//!
//! # Methodology
//!
//! Exactness pins, not accuracy measurements (the accuracy study is
//! `examples/wmp_local_temperature_948.rs`, recorded in
//! `verification_and_validation/wmp_local_temperature_948/README.md`):
//!
//! 1. **The evaluation check.** The embedded CORE WMP reproduces
//!    ENDF/B-VII.0's resolved range. `with_core_multipole` must accept O-16
//!    (and U-238) from VII.0 and refuse O-16 from ENDF/B-VIII.0
//!    (`MultipoleAttachError::EvaluationMismatch`), and refuse a LOW-tier
//!    nuclide, a name not in CORE and a set of another AWR.
//! 2. **The lookup.** Inside `[e_min, e_max]` every `MicroXS` field is the WMP
//!    value at the *lookup* temperature, bit for bit, through `xs_at_energy`,
//!    `total_at_energy` and `total_upper_bound`; outside it, and after
//!    `without_multipole`, the tables answer, bit for bit.
//! 3. **Upstream's side rules.** Elastic kinematics at the local kT at every
//!    energy. On U-238 the WMP range (to 19999.99 eV) and the URR (from
//!    20 keV) do not overlap, so attaching WMP must change nothing above it;
//!    the no-URR-inside-the-range gate itself is pinned by
//!    `material::nuclide::multipole::tests`.
//! 4. **Opt-in.** No ordinary constructor attaches WMP (the V&V record §7
//!    says why; the maintainer decides).
//! 5. **The majorant.** `Majorant::bounding` built on a WMP nuclide at the
//!    material temperature bounds `Σ_t` there (`Majorant::audit` ≤ 1) at
//!    293.6, 900, 1500 and 2500 K with the crate's WMP margin 0.05 (the raw
//!    deficit at margin 0 is printed), and a majorant built at 293.6 K does **not**
//!    bound the same material at 1500 K (the control that shows the audit
//!    can fail).
//!
//! # Results (2026-10-18)
//!
//! All pass. O-16 VII.0 agrees with CORE to 9.6e-5 (total, 1–2.36 MeV,
//! 600 K); VIII.0 is refused at +1.6e-2 (10–100 keV). U-238 majorant, worst
//! `Σ_t/Σ_maj`: margin 0 → 1.00269 (293.6 K), 1.00074 (900), 1.00056 (1500),
//! 1.00032 (2500), an under-bound of up to 0.27 % between pole nodes;
//! margin 0.05 → ≤ 0.95494 at every T. Control: 3.28 at 1500 K from a
//! 293.6 K build. Wall time ~40 s per U-238 test (3 cores).

use std::sync::Arc;

use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::process::{self, CrossSectionSettings, Reconstruction};
use njoy_outram_park_fork::reference_data::reference_endf;
use njoy_outram_park_fork::units::Temperature;
use njoy_outram_park_fork::wmp::WmpLibrary;
use outram_mc_libs::material::material::{Material, NuclideComponent};
use outram_mc_libs::material::nuclide::{MultipoleAttachError, Nuclide};
use outram_mc_libs::material::processed::ProcessedEvaluation;
use outram_mc_libs::physics::delta_tracking::majorant::Majorant;
use outram_mc_libs::physics::scatter::K_BOLTZMANN_EV_PER_K;
use uom::si::thermodynamic_temperature::kelvin;

/// `name` from `file` at `t_k`, through `process` (RECONR + BROADR at NJOY's
/// defaults, PURR and HEATR off), or `None` when the tape is absent.
fn nuclide(file: &str, name: &str, t_k: f64) -> Option<(Nuclide, Arc<Tape>, i32)> {
    let path = reference_endf(file)?;
    let tape = Arc::new(Tape::read_file(&path).expect("tape"));
    let mat = tape.materials()[0];
    let recon = Reconstruction {
        reconr: Nuclide::reconstruct_0k(&tape, mat, 1.0e-3).expect("RECONR"),
        tape: Arc::clone(&tape),
        mat,
        reconr_tolerance: 1.0e-3,
    };
    let s = CrossSectionSettings::default()
        .without_purr()
        .without_heatr();
    let ncs =
        process::cross_sections_at(&recon, Temperature::new::<kelvin>(t_k), &s).expect("process");
    let processed = ProcessedEvaluation {
        recon: ncs.pendf,
        elastic_0k: ncs.elastic_0k.unwrap_or_default(),
        urr: None,
        temp_k: t_k,
    };
    let n = Nuclide::from_processed(&tape, mat, name, t_k, processed).expect("nuclide");
    Some((n, tape, mat))
}

#[test]
fn the_evaluation_check_accepts_vii0_and_refuses_viii0_o16() {
    let (Some((vii, ..)), Some((viii, ..))) = (
        nuclide("n-008_O_016-ENDF7.0.endf", "O16", 600.0),
        nuclide("n-008_O_016-ENDF8.0.endf", "O16", 600.0),
    ) else {
        println!("[skip] O-16 tapes absent");
        return;
    };
    let w = WmpLibrary::core().get("O16").expect("O16 in CORE");
    let c = vii.multipole_consistency(&w).expect("comparable");
    println!("O-16 VII.0: {c:?}");
    assert!(
        vii.clone().with_core_multipole().is_ok(),
        "VII.0 is CORE's evaluation"
    );
    match viii.with_core_multipole() {
        Err(MultipoleAttachError::EvaluationMismatch(c)) => {
            println!("O-16 VIII.0 refused: {c:?}");
            assert!(c.worst_relative.abs() > 5.0e-3);
        }
        other => panic!("VIII.0 must be refused as another evaluation, got {other:?}"),
    }
    // A LOW-tier nuclide is already multipole; a name not in CORE is refused.
    let core = Nuclide::from_core("O16").expect("O16");
    assert_eq!(
        core.with_core_multipole().err(),
        Some(MultipoleAttachError::NotPointwise)
    );
    let renamed = Nuclide::from_core("O16").expect("O16");
    assert!(matches!(
        renamed.with_multipole(njoy_outram_park_fork::wmp::WindowedMultipole::default()),
        Err(MultipoleAttachError::NotPointwise)
    ));
    let mut odd = vii.clone().without_multipole();
    odd.name = "Xx999".into();
    assert!(matches!(
        odd.with_core_multipole(),
        Err(MultipoleAttachError::NotInCoreLibrary(_))
    ));
    // A set with another AWR is not this nuclide.
    let mut heavy = w.clone();
    heavy.awr *= 1.01;
    assert!(matches!(
        vii.with_multipole(heavy),
        Err(MultipoleAttachError::AwrMismatch { .. })
    ));
}

#[test]
fn inside_the_range_the_lookup_is_wmp_at_the_lookup_temperature() {
    let Some((tables, ..)) = nuclide("n-008_O_016-ENDF7.0.endf", "O16", 600.0) else {
        println!("[skip] O-16 VII.0 absent");
        return;
    };
    // Opt-in: the ordinary constructor attaches nothing.
    assert!(tables.multipole().is_none());
    let mp = tables.clone().with_core_multipole().expect("VII.0");
    let w = mp.multipole().expect("attached").clone();
    // Pole peaks give energies where T matters; plus fixed points in range.
    let mut energies: Vec<f64> = w
        .poles
        .iter()
        .filter(|p| p.re > 0.0)
        .map(|p| p.re * p.re)
        .filter(|&e| e > w.e_min && e < w.e_max)
        .collect();
    energies.extend([1.0e-5, 0.0253, 1.0, 1.0e3, w.e_max]);
    let mut moved = 0;
    for &e in &energies {
        for t in [293.6, 1200.0] {
            let x = mp.xs_at_energy(e, t);
            let v = w.evaluate(e, t);
            assert_eq!(
                x.total.to_bits(),
                v.total().to_bits(),
                "total at {e} eV, {t} K"
            );
            assert_eq!(x.elastic.to_bits(), v.scatter.to_bits());
            assert_eq!(x.absorption.to_bits(), v.absorption.to_bits());
            assert_eq!(x.fission.to_bits(), v.fission.to_bits());
            assert_eq!(x.inelastic + x.n2n + x.n3n + x.mt5 + x.other, 0.0);
            assert_eq!(mp.total_at_energy(e, t).to_bits(), x.total.to_bits());
            assert_eq!(mp.total_upper_bound(e, t).to_bits(), x.total.to_bits());
        }
        if mp.xs_at_energy(e, 293.6).total != mp.xs_at_energy(e, 1200.0).total {
            moved += 1;
        }
    }
    assert!(
        moved > 0,
        "the lookup temperature must reach the multipole evaluation"
    );
    // Outside the range, and after the ablation, the tables answer exactly.
    for e in [w.e_max * 1.0001, 5.0e6, 1.4e7] {
        assert_eq!(
            mp.xs_at_energy(e, 1200.0).total.to_bits(),
            tables.xs_at_energy(e, 1200.0).total.to_bits()
        );
    }
    let back = mp.clone().without_multipole();
    for &e in &energies {
        assert_eq!(
            back.xs_at_energy(e, 1200.0).total.to_bits(),
            tables.xs_at_energy(e, 1200.0).total.to_bits()
        );
    }
    // Elastic kinematics: the local kT with WMP, at every energy (upstream
    // `physics.cpp:734`); the data kT without.
    assert_eq!(mp.free_gas_kt(1200.0), K_BOLTZMANN_EV_PER_K * 1200.0);
    assert_eq!(tables.free_gas_kt(1200.0), K_BOLTZMANN_EV_PER_K * 600.0);
    assert_eq!(mp.clone().with_target_at_rest().free_gas_kt(1200.0), 0.0);
}

/// U-238 (ENDF/B-VII.0): the URR gate at the seam, and the majorant pin.
#[test]
fn u238_urr_gate_and_majorant_at_each_temperature() {
    let Some((tables, tape, mat)) = nuclide("n-092_U_238-ENDF7.0.endf", "U238", 900.0) else {
        println!("[skip] U-238 VII.0 absent");
        return;
    };
    // Small PURR settings: the gate is what is tested, not the tables.
    let tables = tables
        .with_urr_probability_tables(&tape, mat, 900.0, 15, 2, 100)
        .expect("PURR");
    let (lo, _) = tables.urr_range_ev().expect("U-238 has a URR");
    let mp = tables
        .clone()
        .with_core_multipole()
        .expect("VII.0 is CORE's evaluation");
    let e_max = mp.multipole().expect("attached").e_max;
    println!("URR from {lo} eV, WMP to {e_max} eV");
    // On this data the two ranges do not overlap (WMP ends at 19999.99 eV,
    // the URR starts at 20 keV), so the URR gate never acts here; the gate
    // itself is pinned by `material::nuclide::multipole::tests`. What must
    // hold is that attaching WMP changes nothing above its range.
    assert!(lo >= e_max);
    for e in [lo, lo * 1.5, 1.0e5] {
        assert_eq!(mp.needs_urr_draw(e), tables.needs_urr_draw(e));
        assert_eq!(
            mp.xs_at_energy_urr(e, 900.0, 0.3).total.to_bits(),
            tables.xs_at_energy_urr(e, 900.0, 0.3).total.to_bits()
        );
    }

    // The majorant: bounds Σ_t at the material's temperature.
    let mat_at = |t: f64| Material {
        id: 1,
        name: "U-238".into(),
        components: vec![NuclideComponent {
            nuclide_idx: 0,
            atom_density: 0.02,
        }],
        temperature: t,
    };
    let nucs = vec![mp];
    // `margin` 0.05 is what the crate's other WMP majorant pins use
    // (`tests/delta_tracking_audit.rs`, `delta_tracking_bit_identity.rs`):
    // between the pole nodes the WMP shape is sampled, not bounded by
    // construction (`Majorant::bounding` doc), and the margin covers it. The
    // raw deficit at margin 0 is printed and recorded in the V&V record §5.5.
    for t in [293.6, 900.0, 1500.0, 2500.0] {
        let m = mat_at(t);
        let raw = Majorant::bounding(std::slice::from_ref(&m), &nucs, 1.0e-5, 2.0e7, 1024, 8, 0.0)
            .audit(std::slice::from_ref(&m), &nucs, 1.0e-5, 2.0e7, 2000);
        let maj = Majorant::bounding(
            std::slice::from_ref(&m),
            &nucs,
            1.0e-5,
            2.0e7,
            1024,
            8,
            0.05,
        );
        let a = maj.audit(std::slice::from_ref(&m), &nucs, 1.0e-5, 2.0e7, 2000);
        println!(
            "T {t} K: {} nodes; margin 0: worst Σt/Σmaj {:.6} at {:.4e} eV; margin 0.05: {:.6} at {:.4e} eV",
            maj.len(),
            raw.worst_ratio,
            raw.energy_ev,
            a.worst_ratio,
            a.energy_ev
        );
        assert!(a.worst_ratio <= 1.0, "under-bound at {t} K: {a:?}");
    }
    let cold = Majorant::bounding(&[mat_at(293.6)], &nucs, 1.0e-5, 2.0e7, 1024, 8, 0.05);
    let a = cold.audit(&[mat_at(1500.0)], &nucs, 1.0e-5, 2.0e7, 2000);
    println!("control, built at 293.6 K, audited at 1500 K: {a:?}");
    assert!(
        a.worst_ratio > 1.0,
        "the control must fail, or the audit cannot see T"
    );
}
