// SPDX-License-Identifier: AGPL-3.0-only

//! **V&V gate** — rotational periodic boundary conditions (GitHub #259, port
//! of OpenMC `RotationalPeriodicBC`, `src/boundary_condition.cpp:152-262`,
//! `8f79d9d`): a quarter of a rotationally symmetric model, closed by a
//! 90 degree periodic pair, against the full model.
//!
//! # Methodology
//!
//! A bare HEU cylinder (Godiva composition, CORE data, as
//! `tests/empty_fission_bank_467.rs`), `R = 7.0 cm`, `z` in `[-7, 7]`, vacuum
//! on the curved face and both end faces. Two arms:
//!
//! * **full**: the whole cylinder;
//! * **quarter**: the `x >= 0, y >= 0` wedge, its two flat faces
//!   (`y = 0` and `x = 0`) a `PeriodicWith` pair, so a neutron leaving through
//!   one re-enters through the other rotated by 90 degrees.
//!
//! The problem is invariant under a 90 degree rotation, so its fundamental
//! mode is too, and the wedge with a periodic pair solves the same eigenvalue
//! problem as the full cylinder: the two `k` must agree within statistics.
//! A **control** closes the same two faces with vacuum instead: that removes
//! three quarters of the neighbours and must lower `k` by far more than any
//! statistical fluctuation, which shows the comparison can fail.
//!
//! **Prediction, written before the first run (2026-11-14):** quarter minus
//! full within 1 % of `k` on the mean of N seeds, and the vacuum control more
//! than 5 % below the full model. The 1 % gate is fixed here, not tuned.
//! 1000 histories per generation, 10 inactive + 30 active, `N = 8` seeds per
//! arm (`OUTRAM_RP_SEEDS` overrides), single-threaded sequential driver.
//!
//! # Results
//!
//! Recorded on GitHub #259 and in
//! `verification_and_validation/rotational_periodic/README.md` (2026-11-14).

use outram_mc_libs::geometry::cell::{Cell, HalfSpaceSense, RegionToken};
use outram_mc_libs::geometry::crossing::GeometryExt;
use outram_mc_libs::geometry::geometry::Geometry;
use outram_mc_libs::geometry::plot::{render_material_slice, PlotBasis, Rgb, SlicePlot};
use outram_mc_libs::geometry::position::Position;
use outram_mc_libs::geometry::surface::{
    BoundaryType, SurfaceKind, XPlane, YPlane, ZCylinder, ZPlane,
};
use outram_mc_libs::geometry::universe::Universe;
use outram_mc_libs::material::material::{Material, NuclideComponent};
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::physics::keff::KeffSettings;
use outram_mc_libs::physics::transport_csg::{run_keff_csg_seq, SourceBox};

const R: f64 = 7.0;
const HALF_H: f64 = 7.0;

#[derive(Clone, Copy)]
enum Arm {
    Full,
    QuarterPeriodic,
    QuarterVacuum,
}

fn hs(i: usize, s: HalfSpaceSense) -> RegionToken {
    RegionToken::HalfSpace { surface_idx: i, sense: s }
}

/// RPN for the intersection of the half-spaces: every token after the first is
/// followed by an `Intersection`. Without them `Cell::contains` tests only the
/// last token (found 2026-11-14 when the first picture showed a plane, not a
/// cylinder).
fn all_of(tokens: Vec<RegionToken>) -> Vec<RegionToken> {
    let mut out = Vec::new();
    for (i, t) in tokens.into_iter().enumerate() {
        out.push(t);
        if i > 0 {
            out.push(RegionToken::Intersection);
        }
    }
    out
}

fn model(arm: Arm) -> (Geometry, SourceBox) {
    let cyl = SurfaceKind::ZCylinder(ZCylinder { x0: 0.0, y0: 0.0, r: R, bc: BoundaryType::Vacuum });
    let zlo = SurfaceKind::ZPlane(ZPlane { z0: -HALF_H, bc: BoundaryType::Vacuum });
    let zhi = SurfaceKind::ZPlane(ZPlane { z0: HALF_H, bc: BoundaryType::Vacuum });
    let h = 4.0;
    match arm {
        Arm::Full => {
            let g = Geometry {
                surfaces: vec![cyl, zlo, zhi],
                cells: vec![Cell::material(
                    1,
                    all_of(vec![
                        hs(0, HalfSpaceSense::Inside),
                        hs(1, HalfSpaceSense::Outside),
                        hs(2, HalfSpaceSense::Inside),
                    ]),
                    0,
                    293.6,
                )],
                universes: vec![Universe { id: 0, cell_indices: vec![0] }],
                lattices: vec![],
                root_universe: 0,
            };
            (g, SourceBox { lower: Position::new(-h, -h, -h), upper: Position::new(h, h, h) })
        }
        Arm::QuarterPeriodic | Arm::QuarterVacuum => {
            let (bc_y, bc_x) = match arm {
                Arm::QuarterPeriodic => (BoundaryType::PeriodicWith(4), BoundaryType::PeriodicWith(3)),
                _ => (BoundaryType::Vacuum, BoundaryType::Vacuum),
            };
            let g = Geometry {
                surfaces: vec![
                    cyl,
                    zlo,
                    zhi,
                    SurfaceKind::YPlane(YPlane { y0: 0.0, bc: bc_y }),
                    SurfaceKind::XPlane(XPlane { x0: 0.0, bc: bc_x }),
                ],
                cells: vec![Cell::material(
                    1,
                    all_of(vec![
                        hs(0, HalfSpaceSense::Inside),
                        hs(1, HalfSpaceSense::Outside),
                        hs(2, HalfSpaceSense::Inside),
                        hs(3, HalfSpaceSense::Outside),
                        hs(4, HalfSpaceSense::Outside),
                    ]),
                    0,
                    293.6,
                )],
                universes: vec![Universe { id: 0, cell_indices: vec![0] }],
                lattices: vec![],
                root_universe: 0,
            };
            (g, SourceBox { lower: Position::new(0.0, 0.0, -h), upper: Position::new(h, h, h) })
        }
    }
}

fn materials() -> (Vec<Material>, Vec<Nuclide>) {
    let nuclides = vec![
        Nuclide::from_core("U234").unwrap(),
        Nuclide::from_core("U235").unwrap(),
        Nuclide::from_core("U238").unwrap(),
    ];
    let heu = Material {
        id: 1,
        name: "HEU".into(),
        temperature: 293.6,
        components: vec![
            NuclideComponent { nuclide_idx: 0, atom_density: 4.9184e-4 },
            NuclideComponent { nuclide_idx: 1, atom_density: 4.4994e-2 },
            NuclideComponent { nuclide_idx: 2, atom_density: 2.4984e-3 },
        ],
    };
    (vec![heu], nuclides)
}

fn n_seeds() -> usize {
    std::env::var("OUTRAM_RP_SEEDS").ok().and_then(|s| s.parse().ok()).unwrap_or(8)
}

/// Mean and standard error of the mean of `k_mean` over seeds.
fn ensemble(arm: Arm) -> (f64, f64) {
    let (geom, sb) = model(arm);
    geom.validate_boundary_conditions().expect("boundary conditions are well formed");
    let (mats, nucs) = materials();
    let n = n_seeds();
    let ks: Vec<f64> = (1..=n as u64)
        .map(|seed| {
            let settings = KeffSettings {
                n_particles: 1000,
                n_inactive: 10,
                n_active: 30,
                seed,
                ..Default::default()
            };
            run_keff_csg_seq(&geom, &mats, &nucs, &[], None, sb, &settings, None, &[], None)
                .k_mean
        })
        .collect();
    let mean = ks.iter().sum::<f64>() / n as f64;
    let var = ks.iter().map(|k| (k - mean) * (k - mean)).sum::<f64>() / (n as f64 - 1.0);
    (mean, (var / n as f64).sqrt())
}

#[test]
fn quarter_with_rotational_periodic_matches_the_full_cylinder() {
    let (kf, sf) = ensemble(Arm::Full);
    let (kq, sq) = ensemble(Arm::QuarterPeriodic);
    let (kv, sv) = ensemble(Arm::QuarterVacuum);
    let combined = (sf * sf + sq * sq).sqrt();
    println!("full             k = {kf:.5} +/- {sf:.5}");
    println!("quarter periodic k = {kq:.5} +/- {sq:.5}");
    println!("quarter vacuum   k = {kv:.5} +/- {sv:.5}");
    println!(
        "quarter - full = {:+.0} pcm ({:+.2} sigma, combined sem {:.0} pcm); {:+.2} % of k",
        (kq - kf) * 1e5,
        (kq - kf) / combined,
        combined * 1e5,
        100.0 * (kq - kf) / kf
    );
    println!("vacuum control - full = {:+.2} % of k", 100.0 * (kv - kf) / kf);
    assert!(((kq - kf) / kf).abs() < 0.01, "quarter periodic differs from full by more than 1 %");
    assert!((kv - kf) / kf < -0.05, "the vacuum control must sit more than 5 % below");
}

/// Pictures of what the solver sees (AGENTS.md geometry rule): the xy slice at
/// z = 0 of the full and the quarter model and the xz slice of the quarter.
/// Set `OUTRAM_PLOT_OUT` to a directory to write them.
#[test]
fn draw_the_wedge_models() {
    let out = std::env::var_os("OUTRAM_PLOT_OUT").map(std::path::PathBuf::from);
    if let Some(dir) = &out {
        std::fs::create_dir_all(dir).expect("create plot dir");
    }
    let palette = [(Rgb::new(200, 90, 40), "HEU (Godiva composition)")];
    for (arm, name) in [(Arm::Full, "full"), (Arm::QuarterPeriodic, "quarter")] {
        let (geom, _) = model(arm);
        for (basis, tag, width) in [
            (PlotBasis::Xy, "xy_z0", [18.0, 18.0]),
            (PlotBasis::Xz, "xz_y3", [18.0, 18.0]),
        ] {
            let origin = if tag == "xy_z0" {
                Position::new(0.0, 0.0, 0.0)
            } else {
                Position::new(0.0, 3.0, 0.0)
            };
            let plot = SlicePlot::new(basis, origin, width, [360, 360]);
            let title = format!(
                "{name}: {tag}; R = 7 cm, z = +/-7 cm; quarter faces y=0 and x=0 periodic"
            );
            let (raw, img) = render_material_slice(&geom, &plot, &palette, &title);
            let fuel = raw.pixels.iter().filter(|c| **c == palette[0].0).count();
            assert!(fuel > 0, "{name} {tag}: no fuel pixels");
            if let Some(dir) = &out {
                img.write_png(dir.join(format!("{name}_{tag}.png"))).expect("write png");
            }
        }
    }
}
