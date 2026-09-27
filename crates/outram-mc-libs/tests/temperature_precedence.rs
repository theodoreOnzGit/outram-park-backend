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
//! | `KeffSettings::temperature_k` | the run | **Free-gas elastic kinematics** (target thermal motion, `free_gas_kt`) |
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
//! - run temperature → k must **differ** (the free-gas target velocity
//!   distribution changes, and with it the random streams).
//!
//! Pass criteria fixed before running; the third arm is the negative control
//! that shows the harness can see a change at all.
//!
//! # Results (2026-09-27)
//!
//! Recorded in `docs/temperatures.md` from this test's printout.

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
    let base = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../reference-data/ace/reference-njoy/endf-b-viii.0/293.6K");
    let load = |n: &str| -> Option<Nuclide> {
        let p = base.join(format!("{n}.ace.gz"));
        p.exists().then_some(())?;
        Some(Nuclide::from_ace_file(&p, n).expect("ACE table decodes"))
    };
    Some(vec![load("U235")?, load("U238")?])
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
    for (label, (km, ks)) in [
        ("all 293.6 K (reference)", reference),
        ("Cell::temperature 1200 K", cell_hot),
        ("Material::temperature 1200 K", mat_hot),
        ("KeffSettings::temperature_k 1200 K", run_hot),
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
    assert_ne!(
        run_hot.0.to_bits(),
        reference.0.to_bits(),
        "KeffSettings::temperature_k changed nothing: either the free-gas kernel \
         no longer reads it, or this harness cannot see a change"
    );
}
