// SPDX-License-Identifier: GPL-3.0

//! **Which temperature does what in a CSG k-eigenvalue run** — pinned, so
//! the answer in `docs/temperatures.md` cannot drift. Maintainer direction,
//! 2026-09-27: *"it is good to check and document which one takes
//! precedence. Otherwise user will be confused."*
//!
//! A model carries up to five temperatures. `run_keff_csg` reads them as
//! follows (verified by reading `physics/transport_csg.rs` and
//! `material/nuclide.rs` on 2026-09-27):
//!
//! | Temperature | Where set | What it does |
//! |---|---|---|
//! | Nuclide build temperature | `Nuclide::from_endf_file(.., T, ..)`, or the ACE table's own | **Sets the Doppler broadening of pointwise cross sections.** It wins for `Pointwise` nuclides. |
//! | S(α,β) table temperature | `ThermalScattering::from_endf_file(.., T, ..)` | Selects the bound-scattering table |
//! | `Material::temperature` | the material | Passed to every cross-section lookup; **only multipole (`Core`) nuclides use it**. Pointwise nuclides ignore it |
//! | `KeffSettings::temperature_k` | the run | ~~**Free-gas elastic kinematics**~~ **Nothing in the CSG drivers** since 2026-09-30 (GitHub #313): the free-gas kT is the nuclide's data temperature (pointwise) or the material's (multipole), as OpenMC |
//! | `Cell::temperature` | the cell | **Not read by transport at all** |
//!
//! # Methodology
//!
//! Godiva (bare HEU sphere, r = 8.7407 cm) from the committed NJOY2016 ACE
//! tables at 293.6 K (`reference-data/ace`), 1000 histories × [5 + 10],
//! single thread, a fixed seed. A reference run has every temperature at
//! 293.6 K. Each other arm changes **one** temperature to 1200 K:
//!
//! - cell temperature → k must be **bit-identical** (never read);
//! - material temperature → k must be **bit-identical** (ACE nuclides are
//!   pointwise, so the lookup ignores it);
//! - ~~run temperature → k must **differ** (the free-gas target velocity
//!   distribution changes, and with it the random streams).~~ **CHANGED
//!   2026-09-30 (GitHub #313):** run temperature → k must be
//!   **bit-identical**. The free-gas kT of a pointwise nuclide is now the
//!   temperature its data were broadened to, as OpenMC takes it
//!   (`nuc->kTs_[i_temp]`, `src/physics.cpp:697`). Before the fix this arm
//!   differed, so the new assertion fails on the old kernel;
//! - **H-1 build temperature** → k must **differ**. This is the negative
//!   control that shows the harness can see a change at all. It replaced the
//!   run-temperature arm in that role on 2026-09-30.
//!
//! Pass criteria fixed before running.
//!
//! # Protocol fixed 2026-09-30: the control had gone blind
//!
//! On bare Godiva the third arm stopped being able to see anything. After the
//! ACE-route fixes of GitHub #365, #366 and #407, all four arms gave
//! bit-identical k = 0.982577. No collision in this short run reached the
//! free-gas range (below 400 kT, or below 1 keV for a DBRC nuclide), so the
//! run temperature was never read.
//!
//! This was checked, not assumed: the same all-identical result appears with
//! `physics/scatter.rs` reverted to its state before the #407 changes. So it
//! is the instrument, not a kernel defect.
//!
//! The criterion is kept, and the **model** is changed. H-1 is added at
//! 5.0e-3 atoms/(b·cm), from `reference-data/endf`. A neutron-mass target is
//! a free gas at every energy, so the run temperature is read on every H-1
//! collision. The model is therefore no longer the Godiva benchmark; it is a
//! lightly moderated HEU sphere, which is all this test needs.
//!
//! # Results
//!
//! Recorded in `docs/temperatures.md` from this test's printout (2026-09-27,
//! re-measured 2026-09-30 twice: once for the H-1 protocol, once for #313).

use outram_mc_libs::geometry::cell::{Cell, HalfSpaceSense, RegionToken};
use outram_mc_libs::geometry::geometry::Geometry;
use outram_mc_libs::geometry::position::Position;
use outram_mc_libs::geometry::surface::{BoundaryType, Sphere, SurfaceKind};
use outram_mc_libs::geometry::universe::Universe;
use outram_mc_libs::material::material::{Material, NuclideComponent};
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::physics::keff::{ComputeType, KeffSettings};
use outram_mc_libs::physics::transport_csg::{run_keff_csg, SourceBox};

const R: f64 = 8.7407;
const T_REF: f64 = 293.6;
const T_HOT: f64 = 1200.0;

fn nuclides() -> Option<Vec<Nuclide>> {
    nuclides_with_h1_at(T_REF)
}

fn nuclides_with_h1_at(h1_t: f64) -> Option<Vec<Nuclide>> {
    let base = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../reference-data/ace/reference-njoy/endf-b-viii.0/293.6K");
    let load = |n: &str| -> Option<Nuclide> {
        let p = base.join(format!("{n}.ace.gz"));
        p.exists().then_some(())?;
        Some(Nuclide::from_ace_file(&p, n).expect("ACE table decodes"))
    };
    // H-1 from the repository's ENDF/B-VIII.0 tape (pointwise, like the ACE
    // nuclides). A neutron-mass target is scattered as a free gas at every
    // energy (the at-rest gate needs `awr > 1`), so its collisions always use
    // its data temperature (GitHub #313). See "Protocol fixed 2026-09-30".
    let h = njoy_outram_park_fork::reference_data::reference_endf("n-001_H_001-ENDF8.0-Beta6.endf")?;
    let h1 = Nuclide::from_endf_file(&h, "H1", h1_t, 1.0e-3).expect("H-1 reconstructs");
    Some(vec![load("U235")?, load("U238")?, h1])
}

fn model(cell_t: f64, mat_t: f64) -> (Geometry, Vec<Material>) {
    let geom = Geometry {
        surfaces: vec![SurfaceKind::Sphere(Sphere {
            x0: 0.0,
            y0: 0.0,
            z0: 0.0,
            r: R,
            bc: BoundaryType::Vacuum,
        })],
        cells: vec![Cell::material(
            1,
            vec![RegionToken::HalfSpace {
                surface_idx: 0,
                sense: HalfSpaceSense::Inside,
            }],
            0,
            cell_t,
        )],
        universes: vec![Universe {
            id: 0,
            cell_indices: vec![0],
        }],
        lattices: vec![],
        root_universe: 0,
    };
    let mats = vec![Material {
        id: 1,
        name: "HEU".into(),
        components: vec![
            NuclideComponent {
                nuclide_idx: 0,
                atom_density: 4.4994e-2,
            },
            NuclideComponent {
                nuclide_idx: 1,
                atom_density: 2.4984e-3,
            },
            NuclideComponent {
                nuclide_idx: 2,
                atom_density: 5.0e-3,
            },
        ],
        temperature: mat_t,
    }];
    (geom, mats)
}

fn k(nucs: &[Nuclide], cell_t: f64, mat_t: f64, run_t: f64) -> (f64, f64) {
    let (geom, mats) = model(cell_t, mat_t);
    let s = KeffSettings {
        n_particles: 1000,
        n_inactive: 5,
        n_active: 10,
        temperature_k: run_t,
        seed: 20260927,
        compute: ComputeType::CpuSingleThread,
        ..KeffSettings::default()
    };
    let src = SourceBox {
        lower: Position::new(-R, -R, -R),
        upper: Position::new(R, R, R),
    };
    let r = run_keff_csg(&geom, &mats, nucs, src, &s, None);
    (r.k_mean, r.k_std)
}

#[test]
fn which_temperature_takes_precedence() {
    let Some(nucs) = nuclides() else {
        eprintln!("SKIP: reference-data/ace submodule not initialised");
        return;
    };
    let reference = k(&nucs, T_REF, T_REF, T_REF);
    let cell_hot = k(&nucs, T_HOT, T_REF, T_REF);
    let mat_hot = k(&nucs, T_REF, T_HOT, T_REF);
    let run_hot = k(&nucs, T_REF, T_REF, T_HOT);
    let h1_hot_nucs = nuclides_with_h1_at(T_HOT).expect("loaded once already");
    let h1_hot = k(&h1_hot_nucs, T_REF, T_REF, T_REF);
    for (label, (km, ks)) in [
        ("all 293.6 K (reference)", reference),
        ("Cell::temperature 1200 K", cell_hot),
        ("Material::temperature 1200 K", mat_hot),
        ("KeffSettings::temperature_k 1200 K", run_hot),
        ("H-1 built at 1200 K (control)", h1_hot),
    ] {
        println!("{label:<38} k = {km:.6} +/- {ks:.6}");
    }
    assert_eq!(
        cell_hot.0.to_bits(),
        reference.0.to_bits(),
        "Cell::temperature changed k: it is read by transport after all, and \
         docs/temperatures.md is wrong"
    );
    assert_eq!(
        mat_hot.0.to_bits(),
        reference.0.to_bits(),
        "Material::temperature changed k on pointwise (ACE) nuclides: the lookup \
         now uses it, and docs/temperatures.md is wrong"
    );
    assert_eq!(
        run_hot.0.to_bits(),
        reference.0.to_bits(),
        "KeffSettings::temperature_k changed k in a CSG run: the free-gas kT is \
         being taken from the run again rather than from the nuclide data \
         (GitHub #313), and docs/temperatures.md is wrong"
    );
    assert_ne!(
        h1_hot.0.to_bits(),
        reference.0.to_bits(),
        "H-1 built at 1200 K changed nothing: this harness cannot see a change"
    );
}

/// GitHub #313: the free-gas kT is the data temperature for a pointwise
/// nuclide, whatever lookup temperature is passed, and the lookup temperature
/// for a multipole one. OpenMC `src/physics.cpp:697`. Fails on the kernel
/// before 2026-09-30, where `free_gas_kt(T)` returned `k_B * T` for every
/// nuclide.
#[test]
fn free_gas_kt_is_the_data_temperature_for_pointwise_nuclides() {
    use outram_mc_libs::physics::scatter::K_BOLTZMANN_EV_PER_K as KB;
    let Some(nucs) = nuclides() else {
        eprintln!("SKIP: reference-data/ace submodule not initialised");
        return;
    };
    // ACE U-235: the table's own kT.
    let u235 = &nucs[0];
    let kt_table = u235.data_kt_ev().expect("an ACE nuclide is pointwise");
    assert!((kt_table - KB * T_REF).abs() < 1.0e-3 * kt_table, "ACE kT {kt_table} eV is not 293.6 K");
    assert_eq!(u235.free_gas_kt(T_HOT).to_bits(), kt_table.to_bits());
    // ENDF H-1 built at 293.6 K: k_B * 293.6, not k_B * 1200.
    let h1 = &nucs[2];
    assert_eq!(h1.free_gas_kt(T_HOT).to_bits(), (KB * T_REF).to_bits());
    // The target-at-rest ablation still wins.
    assert_eq!(h1.clone().with_target_at_rest().free_gas_kt(T_HOT), 0.0);
    // Multipole (`Core`): the lookup temperature.
    let core = Nuclide::from_core("U235").expect("embedded U-235");
    assert_eq!(core.data_kt_ev(), None);
    assert_eq!(core.free_gas_kt(T_HOT).to_bits(), (KB * T_HOT).to_bits());
}
