//! **The tally estimator inside a delta-tracked region** (gh:#598).
//!
//! # What went wrong
//!
//! Step 8 of the `dhoby-ghaut` workbench condenses multigroup constants from
//! a k-eigenvalue run on HTR-10, tallied on an unstructured mesh. The pebble
//! bed is delta-tracked, where no track-length estimator exists, so the flux
//! there was scored with the collision estimator `w/Σ_t` at real collisions,
//! **passed through `score_track_length` with `1/Σ_t` as the length**. Since
//! gh:#492 an unstructured mesh filter splits a track-length segment across
//! the cells it crosses, so that "length" was rebuilt as a segment centred on
//! the collision and split. A helium collision (`1/Σ_t ~ 5e4 cm`, the core is
//! ~4 m) put almost all of its score outside the mesh, where it was dropped.
//! The bed lost the flux of its 39 % helium and every bed Σ came out
//! ~1/0.61 too large.
//!
//! # What is tested here
//!
//! 1. `score_collision_point_bins_at_the_point`: a collision score on an
//!    unstructured mesh lands in the cell holding the site, whole; the old
//!    route through `score_track_length` (kept here only as the contrast)
//!    smears it.
//! 2. `tentative_collision_estimator_recovers_the_volume_weighted_sigma`: an
//!    ANALYTIC case. A pure scatterer at one energy in an infinite BCC lattice
//!    of spheres at packing **f = 0.61** (the HTR-10 recipe's filling
//!    fraction), started uniform and isotropic and cut at a fixed path length,
//!    has a spatially flat flux, so the region-averaged `Σ_t` is exactly the
//!    volume-weighted `f·Σ_p + (1-f)·Σ_g`. Checked for helium-like gas
//!    (`Σ_g/Σ_p = 5e-5`) and for a true void (`Σ_g = 0`).
//! 3. `delta_tally_matches_surface_track_length_on_a_bcc_cell`: the full
//!    hybrid driver on a reflective BCC cell (U-235 in graphite spheres,
//!    helium between), tallied on an unstructured hex mesh: the delta-tracked
//!    region's `Σ_t` per group equals what surface tracking's track-length
//!    estimator (the OpenMC MGXS estimator) gives on the identical geometry,
//!    and the estimator choice leaves `k` bit-identical.
//!
//! # Predictions (written before the first run, 2026-10-06)
//!
//! - Test 2: the tentative estimator gives `0.61·Σ_p + 0.39·Σ_g` within
//!   statistics; the real-collision estimator gives exactly `Σ_p` on a true
//!   void (high by `1/0.61 = 1.64`) and, on helium, `Σ_p`-ish in most
//!   finite runs.
//! - Test 3: thermal `Σ_t` of the region about 0.25/cm (0.61 of graphite's
//!   ~0.40), equal between surface and tentative-delta within 4 σ.
//!
//! # Results
//!
//! Recorded in each test's doc comment.

use std::sync::Arc;

use outram_blender::unstructured::{Element, ElementKind, LengthUnit, UnstructuredMesh};
use outram_mc_libs::geometry::cell::{Cell, CellFill, HalfSpaceSense, RegionToken};
use outram_mc_libs::geometry::geometry::Geometry;
use outram_mc_libs::geometry::position::{Direction, Position};
use outram_mc_libs::geometry::surface::{
    BoundaryType, Sphere, SurfaceKind, XPlane, YPlane, ZPlane,
};
use outram_mc_libs::geometry::universe::Universe;
use outram_mc_libs::material::material::{Material, NuclideComponent};
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::pebble_beds::delta_tracking::{
    bounded_delta_flight_visiting, DeltaStep, Majorant,
};
use outram_mc_libs::physics::keff::{ComputeType, KeffSettings};
use outram_mc_libs::physics::transport_csg::{
    run_keff_csg, run_keff_csg_hybrid, DeltaTallyEstimator, SourceBox,
};
use outram_mc_libs::rng::distributions::isotropic_direction;
use outram_mc_libs::rng::lcg::prn;
use outram_mc_libs::tally::filter::{EnergyFilter, FilterKind, MeshFilter};
use outram_mc_libs::tally::mesh::MeshKind;
use outram_mc_libs::tally::scoring::{score_collision_point, score_track_length};
use outram_mc_libs::tally::tally::{ScoreType, Tally, TallyBin};

/// The HTR-10 recipe's pebble filling fraction.
const F: f64 = 0.61;

/// BCC sphere radius for packing `F` in a cube of side `a`:
/// `2·(4/3)π r³ = F a³`.
fn bcc_radius(a: f64) -> f64 {
    a * (3.0 * F / (8.0 * std::f64::consts::PI)).cbrt()
}

/// Whether `p` is inside a sphere of the infinite BCC lattice of pitch `a`
/// (one sphere at each cube centre, one at each corner).
fn in_bcc_sphere(p: Position, a: f64, r: f64) -> bool {
    let h = 0.5 * a;
    let fold = |x: f64| x - a * (x / a).round(); // to [-h, h]
    let (x, y, z) = (fold(p.x), fold(p.y), fold(p.z));
    let centre = x * x + y * y + z * z;
    let (cx, cy, cz) = (h - x.abs(), h - y.abs(), h - z.abs());
    let corner = cx * cx + cy * cy + cz * cz;
    centre < r * r || corner < r * r
}

fn one_hex_mesh(h: f64) -> UnstructuredMesh {
    let mut pts = Vec::new();
    for x in [-h, h] {
        for (y, z) in [(-h, -h), (h, -h), (h, h), (-h, h)] {
            pts.push([x, y, z]);
        }
    }
    let el = Element {
        kind: ElementKind::Hex8,
        nodes: (0..8).collect(),
    };
    UnstructuredMesh::from_elements(LengthUnit::Centimetre, pts, vec![el], vec![], vec![])
        .expect("one hex")
}

/// **1. A collision score is a point, not a segment.**
///
/// Two unit hexes along x, a collision at x = 0.5 scoring `1/Σ = 5e4`
/// (helium). At the point it all lands in cell 0. Through
/// `score_track_length` the 5e4 "length" becomes a segment from
/// x = -25 000 to +25 000 cm and only the 2 cm inside the mesh survive,
/// 1 cm per cell: the gh:#598 defect, in miniature.
///
/// Results (2026-10-06): exact, as asserted.
#[test]
fn score_collision_point_bins_at_the_point() {
    let mut pts = Vec::new();
    for x in [0.0, 1.0, 2.0] {
        for (y, z) in [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)] {
            pts.push([x, y, z]);
        }
    }
    let el = |i: usize| Element {
        kind: ElementKind::Hex8,
        nodes: (0..8).map(|k| 4 * i + k).collect(),
    };
    let mesh =
        UnstructuredMesh::from_elements(LengthUnit::Centimetre, pts, vec![el(0), el(1)], vec![], vec![])
            .unwrap();
    let tally = Tally {
        id: 1,
        name: "point".into(),
        filters: vec![FilterKind::Mesh(MeshFilter {
            mesh: MeshKind::Unstructured(Arc::new(mesh)),
        })],
        scores: vec![ScoreType::Flux],
        bins: vec![TallyBin::default(); 2],
    };
    let at = Position::new(0.5, 0.5, 0.5);
    let u = Direction::new(1.0, 0.0, 0.0);
    let mut point = vec![0.0; 2];
    score_collision_point(&mut point, &tally, 0, 0, 0, 1.0, 5.0e4, at, None, 1.0, None, 0.0, u);
    assert_eq!(point, vec![5.0e4, 0.0]);

    let mut smeared = vec![0.0; 2];
    score_track_length(&mut smeared, &tally, 0, 0, 0, 1.0, 5.0e4, at, None, 1.0, None, 0.0, u);
    assert!(
        (smeared[0] - 1.0).abs() < 1e-9 && (smeared[1] - 1.0).abs() < 1e-9,
        "the contrast case changed: {smeared:?}"
    );
}

struct FlatFluxResult {
    /// Region Σ_t from the tentative-collision estimator, its σ.
    tentative: (f64, f64),
    /// Region Σ_t from the real-collision estimator, its σ.
    real: (f64, f64),
    /// Mean flux path per history from each, against the true `L`.
    flux_tentative: f64,
    flux_real: f64,
    /// `Σ_t` of the pebble and the gas.
    sig: [f64; 2],
}

/// Uniform isotropic starts in an infinite BCC lattice, pure scattering at one
/// energy, each history cut at path length `l`: the flux is flat, so both
/// estimators' `Σ RR / Σ φ` must converge to the volume average.
fn flat_flux_run(gas_density: f64, n_hist: usize, l: f64, seed0: u64) -> FlatFluxResult {
    let nuclides = vec![Nuclide::from_core("H1").expect("embedded H-1")];
    let e = 1.0e6;
    let pebble = Material {
        id: 1,
        name: "pebble".into(),
        temperature: 293.6,
        components: vec![NuclideComponent {
            nuclide_idx: 0,
            atom_density: 0.09,
        }],
    };
    let gas = Material {
        id: 2,
        name: "gas".into(),
        temperature: 293.6,
        components: if gas_density > 0.0 {
            vec![NuclideComponent {
                nuclide_idx: 0,
                atom_density: gas_density,
            }]
        } else {
            vec![]
        },
    };
    let materials = vec![pebble, gas];
    let sig: Vec<f64> = materials
        .iter()
        .map(|m| m.macro_xs_total(e, &nuclides))
        .collect();
    let maj = Majorant::uniform(sig[0]);
    let (a, r_s) = (6.0, bcc_radius(6.0));
    let mut seed = seed0;
    let n_batch = 20;
    let per = n_hist / n_batch;
    let mut tent_b = Vec::new();
    let mut real_b = Vec::new();
    let (mut ft_all, mut fr_all) = (0.0, 0.0);
    for _ in 0..n_batch {
        let (mut ft, mut rt, mut fr, mut rr) = (0.0, 0.0, 0.0, 0.0);
        for _ in 0..per {
            let mut p = Position::new(a * prn(&mut seed), a * prn(&mut seed), a * prn(&mut seed));
            let (x, y, z) = isotropic_direction(&mut seed);
            let mut u = Direction::new(x, y, z);
            let mut left = l;
            loop {
                let start = p;
                let dir = u;
                let step = bounded_delta_flight_visiting(
                    start,
                    dir,
                    e,
                    &maj,
                    &materials,
                    &nuclides,
                    1_000_000,
                    |q: Position, _| {
                        left - ((q.x - start.x) * dir.u + (q.y - start.y) * dir.v + (q.z - start.z) * dir.w)
                    },
                    |q: Position| Some(if in_bcc_sphere(q, a, r_s) { 0 } else { 1 }),
                    &mut seed,
                    None,
                    |_q, m, mj| {
                        ft += 1.0 / mj;
                        rt += sig[m] / mj;
                    },
                );
                match step {
                    DeltaStep::Collision { position, material, .. } => {
                        fr += 1.0 / sig[material];
                        rr += 1.0;
                        let d = (position.x - start.x) * dir.u
                            + (position.y - start.y) * dir.v
                            + (position.z - start.z) * dir.w;
                        left -= d;
                        p = position;
                        let (x, y, z) = isotropic_direction(&mut seed);
                        u = Direction::new(x, y, z);
                    }
                    DeltaStep::Exit { .. } => break,
                    DeltaStep::Exhausted { .. } => panic!("budget exhausted"),
                }
            }
        }
        tent_b.push(rt / ft);
        real_b.push(if fr > 0.0 { rr / fr } else { f64::NAN });
        ft_all += ft;
        fr_all += fr;
    }
    let ms = |v: &[f64]| {
        let n = v.len() as f64;
        let m = v.iter().sum::<f64>() / n;
        let s2 = v.iter().map(|x| (x - m) * (x - m)).sum::<f64>() / (n - 1.0);
        (m, (s2 / n).sqrt())
    };
    FlatFluxResult {
        tentative: ms(&tent_b),
        real: ms(&real_b),
        flux_tentative: ft_all / (per * n_batch) as f64,
        flux_real: fr_all / (per * n_batch) as f64,
        sig: [sig[0], sig[1]],
    }
}

/// **2. Analytic: a flat flux in a 61 %-packed lattice.**
///
/// Methodology: pebble = H-1 at 0.09 /(b cm) (`Σ_p` at 1 MeV from the
/// embedded evaluation), gas = H-1 at `5e-5` of that (helium-like ratio) or
/// nothing (true void); BCC pitch 6 cm, sphere radius for f = 0.61 exactly;
/// majorant `Σ_p`; 20 000 histories of 40 cm each, in 20 batches. Pass:
/// tentative-collision `Σ_t` within 4 σ and 0.5 % of `f·Σ_p + (1-f)·Σ_g`,
/// and its mean flux path within 1 % of 40 cm; on the true void the
/// real-collision estimator gives exactly `Σ_p` (the defect class this
/// estimator has, independent of gh:#598's binning bug).
///
/// Results (2026-10-06, this test's printout): see the record in
/// `crates/dhoby-ghaut/verification_and_validation/workbench_steps_7_8/README.md`
/// (gh:#598 section) for the numbers of the first run.
#[test]
fn tentative_collision_estimator_recovers_the_volume_weighted_sigma() {
    for (label, gas) in [("helium-like", 0.09 * 5.0e-5), ("void", 0.0)] {
        let res = flat_flux_run(gas, 20_000, 40.0, 0x5eed_0598);
        let [sig_p, sig_g] = res.sig;
        let exact = F * sig_p + (1.0 - F) * sig_g;
        let (m, s) = res.tentative;
        let (mr, sr) = res.real;
        println!(
            "{label}: Σ_p = {sig_p:.6}, Σ_g = {sig_g:.3e}, volume-weighted {exact:.6}\n  \
             tentative  {m:.6} ± {s:.6}  ({:+.3} %, {:.2} σ), flux path {:.3} cm\n  \
             real-coll. {mr:.6} ± {sr:.6}  ({:+.3} %), flux path {:.3} cm",
            100.0 * (m / exact - 1.0),
            (m - exact).abs() / s,
            res.flux_tentative,
            100.0 * (mr / exact - 1.0),
            res.flux_real
        );
        assert!(
            (m - exact).abs() < 4.0 * s && (m / exact - 1.0).abs() < 5.0e-3,
            "{label}: tentative estimator {m} ± {s} vs volume-weighted {exact}"
        );
        assert!(
            (res.flux_tentative / 40.0 - 1.0).abs() < 0.01,
            "{label}: tentative flux path {} vs 40 cm",
            res.flux_tentative
        );
        if gas == 0.0 {
            assert!(
                (mr / sig_p - 1.0).abs() < 1e-9,
                "void: the real-collision estimator should see only pebbles, got {mr} vs Σ_p {sig_p}"
            );
        }
    }
}

// ── 3. The full hybrid driver ──────────────────────────────────────────────

const TEMP: f64 = 293.6;

fn endf(name: &str, file: &str) -> Option<Nuclide> {
    let base =
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../reference-data/endf");
    let p = base.join(file);
    p.exists().then_some(())?;
    Nuclide::from_endf_file(&p, name, TEMP, 1.0e-3).ok()
}

/// A reflective cube of side `a` holding one BCC cell: a sphere at the centre
/// and one-eighth of a sphere at each corner, gas between. `delta` decides
/// whether the cube's contents are delta-tracked; otherwise identical.
fn bcc_cell_geometry(a: f64, delta: bool) -> Geometry {
    let h = 0.5 * a;
    let r = bcc_radius(a);
    let mut surfaces = vec![
        SurfaceKind::XPlane(XPlane { x0: -h, bc: BoundaryType::Reflective }),
        SurfaceKind::XPlane(XPlane { x0: h, bc: BoundaryType::Reflective }),
        SurfaceKind::YPlane(YPlane { y0: -h, bc: BoundaryType::Reflective }),
        SurfaceKind::YPlane(YPlane { y0: h, bc: BoundaryType::Reflective }),
        SurfaceKind::ZPlane(ZPlane { z0: -h, bc: BoundaryType::Reflective }),
        SurfaceKind::ZPlane(ZPlane { z0: h, bc: BoundaryType::Reflective }),
    ];
    let mut centres = vec![(0.0, 0.0, 0.0)];
    for sx in [-h, h] {
        for sy in [-h, h] {
            for sz in [-h, h] {
                centres.push((sx, sy, sz));
            }
        }
    }
    let first_sphere = surfaces.len();
    for &(x0, y0, z0) in &centres {
        surfaces.push(SurfaceKind::Sphere(Sphere {
            x0,
            y0,
            z0,
            r,
            bc: BoundaryType::Transmissive,
        }));
    }
    let hs = |i: usize, sense| RegionToken::HalfSpace { surface_idx: i, sense };
    // Universe 1: nine sphere cells (pebble, material 0) and the gas between
    // them (material 1). The spheres do not overlap (centre-corner 0.866 a,
    // corner-corner a, both > 2 r = 0.835 a).
    let mut cells = Vec::new();
    let mut inner = Vec::new();
    for k in 0..centres.len() {
        inner.push(cells.len());
        cells.push(Cell::material(10 + k as i32, vec![hs(first_sphere + k, HalfSpaceSense::Inside)], 0, TEMP));
    }
    let mut gas_region = vec![hs(first_sphere, HalfSpaceSense::Outside)];
    for k in 1..centres.len() {
        gas_region.push(hs(first_sphere + k, HalfSpaceSense::Outside));
        gas_region.push(RegionToken::Intersection);
    }
    inner.push(cells.len());
    cells.push(Cell::material(30, gas_region, 1, TEMP));
    // Universe 0: the reflective cube filled with universe 1.
    let mut cube = vec![
        hs(0, HalfSpaceSense::Outside),
        hs(1, HalfSpaceSense::Inside),
        RegionToken::Intersection,
        hs(2, HalfSpaceSense::Outside),
        RegionToken::Intersection,
        hs(3, HalfSpaceSense::Inside),
        RegionToken::Intersection,
        hs(4, HalfSpaceSense::Outside),
        RegionToken::Intersection,
        hs(5, HalfSpaceSense::Inside),
        RegionToken::Intersection,
    ];
    cube.shrink_to_fit();
    let root = Cell::fill(1, cube, CellFill::Universe(1), Position::ZERO);
    let root_idx = cells.len();
    cells.push(if delta { root.delta_tracked(0) } else { root });
    Geometry {
        surfaces,
        cells,
        universes: vec![
            Universe { id: 0, cell_indices: vec![root_idx] },
            Universe { id: 1, cell_indices: inner },
        ],
        lattices: vec![],
        root_universe: 0,
    }
}

const GROUPS: [f64; 3] = [1.0e-5, 0.625, 2.0e7];

fn region_tally(a: f64) -> Tally {
    Tally {
        id: 598,
        name: "bcc region".into(),
        filters: vec![
            FilterKind::Mesh(MeshFilter {
                mesh: MeshKind::Unstructured(Arc::new(one_hex_mesh(0.5 * a))),
            }),
            FilterKind::Energy(EnergyFilter { bins: GROUPS.to_vec() }),
        ],
        scores: vec![ScoreType::Flux, ScoreType::Total],
        bins: vec![TallyBin::default(); 2 * 2],
    }
}

/// `(Σ_t, rel σ)` per group of the one-cell region tally.
fn sigma_t(t: &Tally, n: u64) -> Vec<(f64, f64)> {
    (0..2)
        .map(|g| {
            let f = &t.bins[g * 2];
            let x = &t.bins[g * 2 + 1];
            let rf = f.rel_std_dev(n);
            let rx = x.rel_std_dev(n);
            (x.mean(n) / f.mean(n), (rf * rf + rx * rx).sqrt())
        })
        .collect()
}

/// **3. Hybrid driver: delta-tracked `Σ_t` equals surface track-length `Σ_t`.**
///
/// Methodology: reflective cube, a = 6 cm, BCC spheres at f = 0.61 of
/// graphite (C-12 at 0.0867 /(b cm)) with U-235 at 2e-5 /(b cm), helium-4 at
/// 2.4452e-5 /(b cm) between (the HTR-10 coolant density at 300 K, 1 atm);
/// ENDF/B-VIII.0 at 293.6 K, no S(α,β). k-eigenvalue, 2000 × (20 + 30),
/// seed 1, single thread. Tally: one-hex unstructured mesh over the cube ×
/// 2 groups (0.625 eV), `Flux` and `Total`. Arms: surface tracking
/// (track-length, the OpenMC MGXS estimator: the reference), delta tracking
/// with the default tentative-collision estimator, delta tracking with the
/// real-collision ablation. Pass: tentative vs surface `Σ_t` within 4 σ
/// combined (ratio σ with the flux/rate correlation ignored, so
/// conservative) in both groups; the two delta arms give bit-identical `k`.
///
/// Results: see the gh:#598 section of
/// `crates/dhoby-ghaut/verification_and_validation/workbench_steps_7_8/README.md`.
#[test]
fn delta_tally_matches_surface_track_length_on_a_bcc_cell() {
    let (Some(u5), Some(c12), Some(he4)) = (
        endf("U235", "n-092_U_235-ENDF8.0.endf"),
        endf("C12", "n-006_C_012-ENDF8.0.endf"),
        endf("He4", "n-002_He_004-ENDF8.0.endf"),
    ) else {
        eprintln!("SKIP: ENDF tapes not in this checkout");
        return;
    };
    let nucs = vec![u5, c12, he4];
    let mats = vec![
        Material {
            id: 1,
            name: "fuelled graphite".into(),
            temperature: TEMP,
            components: vec![
                NuclideComponent { nuclide_idx: 0, atom_density: 2.0e-5 },
                NuclideComponent { nuclide_idx: 1, atom_density: 8.67e-2 },
            ],
        },
        Material {
            id: 2,
            name: "helium".into(),
            temperature: TEMP,
            components: vec![NuclideComponent { nuclide_idx: 2, atom_density: 2.4452e-5 }],
        },
    ];
    let a = 6.0;
    let grid: Vec<f64> = (0..4096)
        .map(|i| (1.0e-5_f64.ln() + (2.0e7_f64.ln() - 1.0e-5_f64.ln()) * i as f64 / 4095.0).exp())
        .collect();
    let maj = Majorant::over_indices(&mats, &[0, 1], &nucs, &grid, 0.3);
    let h = 0.5 * a;
    let source = SourceBox {
        lower: Position::new(-h, -h, -h),
        upper: Position::new(h, h, h),
    };
    let settings = |est| KeffSettings {
        n_particles: 2000,
        n_inactive: 20,
        n_active: 30,
        temperature_k: TEMP,
        seed: 1,
        compute: ComputeType::CpuSingleThread,
        delta_tally_estimator: est,
        ..KeffSettings::default()
    };
    let n = 30_u64;

    let mut t_surf = region_tally(a);
    let surf = run_keff_csg(
        &bcc_cell_geometry(a, false),
        &mats,
        &nucs,
        source,
        &settings(DeltaTallyEstimator::default()),
        Some(&mut t_surf),
    );
    let mut t_tent = region_tally(a);
    let tent = run_keff_csg_hybrid(
        &bcc_cell_geometry(a, true),
        &mats,
        &nucs,
        std::slice::from_ref(&maj),
        None,
        source,
        &settings(DeltaTallyEstimator::TentativeCollision),
        Some(&mut t_tent),
    );
    let mut t_real = region_tally(a);
    let real = run_keff_csg_hybrid(
        &bcc_cell_geometry(a, true),
        &mats,
        &nucs,
        std::slice::from_ref(&maj),
        None,
        source,
        &settings(DeltaTallyEstimator::RealCollision),
        Some(&mut t_real),
    );

    let (s, d, rc) = (sigma_t(&t_surf, n), sigma_t(&t_tent, n), sigma_t(&t_real, n));
    println!("k: surface {:.6} ± {:.6}, delta {:.6} ± {:.6}, delta (real-collision tally) {:.6}",
        surf.k_mean, surf.k_std, tent.k_mean, tent.k_std, real.k_mean);
    println!("delta virtual collisions: {}", tent.virtual_collisions);
    for g in 0..2 {
        let z = (d[g].0 - s[g].0).abs()
            / ((d[g].0 * d[g].1).powi(2) + (s[g].0 * s[g].1).powi(2)).sqrt();
        println!(
            "group {g}: Σ_t surface {:.6} ± {:.2e} | delta tentative {:.6} ± {:.2e} ({:+.2} %, {z:.2} σ) | delta real-collision {:.6} ± {:.2e} ({:+.1} %)",
            s[g].0, s[g].0 * s[g].1, d[g].0, d[g].0 * d[g].1,
            100.0 * (d[g].0 / s[g].0 - 1.0),
            rc[g].0, rc[g].0 * rc[g].1,
            100.0 * (rc[g].0 / s[g].0 - 1.0),
        );
        assert!(
            z < 4.0,
            "group {g}: delta-tracked Σ_t {} vs surface track-length {} ({z:.2} σ)",
            d[g].0,
            s[g].0
        );
    }
    assert!(tent.virtual_collisions > 0, "the delta region was never entered");
    assert_eq!(
        tent.k_mean.to_bits(),
        real.k_mean.to_bits(),
        "the tally estimator must not change the histories"
    );
}
