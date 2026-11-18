// SPDX-License-Identifier: GPL-3.0

//! **V&V gate** -- WHITE and PERIODIC boundary conditions against OpenMC, on
//! seed ensembles. GitHub #259, acceptance items A and B.
//!
//! Full methodology, prediction (committed before the first run), OpenMC
//! deck, provenance and results:
//! `verification_and_validation/white_periodic_vs_openmc_259/README.md`.
//!
//! In short: one-group multi-group transport (no nuclear data), 20 000
//! particles x (20 inactive + 100 active) generations, seeds 1..=16 in both
//! codes. Case A: slab, left face reflective or white, right face vacuum,
//! T in {0.5, 1.0, 3.0}. Case B: 6 x 6 cm periodic-or-reflective box with a
//! z-axis rod, off-centre (B1, mirror-asymmetric lattice) or centred (B0,
//! control). The statistic is the ensemble mean and its standard error over
//! seeds; the gate is `|z| <= 2` against OpenMC on every k and on the
//! boundary-condition difference `d`. The gate is fixed by the record's
//! prediction and is not tuned.
//!
//! Set `OUTRAM_BC259_OUT=<dir>` to write the per-seed k values as JSON, and
//! `OUTRAM_BC259_SEEDS`/`OUTRAM_BC259_FIRST` to change the seed block.
//! Pictures of every geometry: `OUTRAM_PLOT_OUT=<dir>` with `draw_the_models`.

use outram_mc_libs::geometry::cell::{Cell, HalfSpaceSense, RegionToken};
use outram_mc_libs::geometry::crossing::GeometryExt;
use outram_mc_libs::geometry::geometry::Geometry;
use outram_mc_libs::geometry::plot::{render_material_slice, PlotBasis, Rgb, SlicePlot};
use outram_mc_libs::geometry::position::Position;
use outram_mc_libs::geometry::surface::{
    BoundaryType, SurfaceKind, XPlane, YPlane, ZCylinder, ZPlane,
};
use outram_mc_libs::geometry::universe::Universe;
use outram_mc_libs::physics::physics_mg::{run_keff_mg, MgSettings, Mgxs, MgxsLibrary};
use outram_mc_libs::physics::transport_csg::SourceBox;

const PARTICLES: usize = 20_000;
const INACTIVE: usize = 30;
const ACTIVE: usize = 100;

/// Diagnostic knobs (README section "B1 reflective discrepancy: diagnosis"):
/// `OUTRAM_BC259_PARTICLES`, `OUTRAM_BC259_INACTIVE`, `OUTRAM_BC259_ACTIVE`
/// override the defaults above; unset, the gate is exactly as recorded.
fn env_usize(name: &str, default: usize) -> usize {
    std::env::var(name).ok().and_then(|s| s.parse().ok()).unwrap_or(default)
}

fn fuel_slab_xs() -> Mgxs {
    Mgxs::new("fuel", vec![1.0], vec![0.30], vec![0.20], vec![0.50], vec![1.0], vec![0.70])
}
fn fuel2_xs() -> Mgxs {
    Mgxs::new("fuel2", vec![2.0], vec![0.60], vec![0.40], vec![1.00], vec![1.0], vec![1.40])
}
fn mod_xs() -> Mgxs {
    Mgxs::new("mod", vec![0.5], vec![0.01], vec![0.0], vec![0.0], vec![0.0], vec![0.49])
}

/// RPN intersection of half-spaces (an `Intersection` after every token past
/// the first, so `Cell::contains` sees the whole region).
fn intersection(tokens: &[(usize, HalfSpaceSense)]) -> Vec<RegionToken> {
    let mut region = Vec::new();
    for (k, (idx, sense)) in tokens.iter().enumerate() {
        region.push(RegionToken::HalfSpace { surface_idx: *idx, sense: *sense });
        if k > 0 {
            region.push(RegionToken::Intersection);
        }
    }
    region
}

/// Case A: slab x in [0, t], transverse reflective, left face `bc`, right vacuum.
fn slab(t: f64, bc: BoundaryType) -> Geometry {
    use HalfSpaceSense::{Inside, Outside};
    let surfaces = vec![
        SurfaceKind::XPlane(XPlane { x0: 0.0, bc }),
        SurfaceKind::XPlane(XPlane { x0: t, bc: BoundaryType::Vacuum }),
        SurfaceKind::YPlane(YPlane { y0: -1.0, bc: BoundaryType::Reflective }),
        SurfaceKind::YPlane(YPlane { y0: 1.0, bc: BoundaryType::Reflective }),
        SurfaceKind::ZPlane(ZPlane { z0: -1.0, bc: BoundaryType::Reflective }),
        SurfaceKind::ZPlane(ZPlane { z0: 1.0, bc: BoundaryType::Reflective }),
    ];
    let region = intersection(&[
        (0, Outside),
        (1, Inside),
        (2, Outside),
        (3, Inside),
        (4, Outside),
        (5, Inside),
    ]);
    Geometry {
        surfaces,
        cells: vec![Cell::material(1, region, 0, 293.6)],
        universes: vec![Universe { id: 0, cell_indices: vec![0] }],
        lattices: vec![],
        root_universe: 0,
    }
}

/// Case B: 6 x 6 x 2 cm box, rod of radius 0.8 cm at `(cx, cy)`; x and y
/// faces periodic pairs when `periodic`, reflective otherwise.
fn rod_box(cx: f64, cy: f64, periodic: bool) -> Geometry {
    use HalfSpaceSense::{Inside, Outside};
    let bc = |partner: usize| {
        if periodic {
            BoundaryType::PeriodicWith(partner)
        } else {
            BoundaryType::Reflective
        }
    };
    let surfaces = vec![
        SurfaceKind::XPlane(XPlane { x0: 0.0, bc: bc(1) }),
        SurfaceKind::XPlane(XPlane { x0: 6.0, bc: bc(0) }),
        SurfaceKind::YPlane(YPlane { y0: 0.0, bc: bc(3) }),
        SurfaceKind::YPlane(YPlane { y0: 6.0, bc: bc(2) }),
        SurfaceKind::ZPlane(ZPlane { z0: -1.0, bc: BoundaryType::Reflective }),
        SurfaceKind::ZPlane(ZPlane { z0: 1.0, bc: BoundaryType::Reflective }),
        SurfaceKind::ZCylinder(ZCylinder { x0: cx, y0: cy, r: 0.8, bc: BoundaryType::Transmissive }),
    ];
    let box_tokens = [
        (0, Outside),
        (1, Inside),
        (2, Outside),
        (3, Inside),
        (4, Outside),
        (5, Inside),
    ];
    let mut fuel = box_tokens.to_vec();
    fuel.push((6, Inside));
    let mut moderator = box_tokens.to_vec();
    moderator.push((6, Outside));
    Geometry {
        surfaces,
        cells: vec![
            Cell::material(1, intersection(&fuel), 0, 293.6),
            Cell::material(2, intersection(&moderator), 1, 293.6),
        ],
        universes: vec![Universe { id: 0, cell_indices: vec![0, 1] }],
        lattices: vec![],
        root_universe: 0,
    }
}

/// Case B1U: the method-of-images unfolding of the reflective B1 box. The
/// reflective 6 x 6 box with a rod at (1.2, 1.7) is exactly the periodic
/// 12 x 12 box (pitch 12, faces paired x=0<->x=12, y=0<->y=12) holding the
/// rod and its three mirror images: (1.2, 1.7), (10.8, 1.7), (1.2, 10.3),
/// (10.8, 10.3). Uses only the periodic crossing path, no reflective x/y wall.
fn unfolded_b1() -> Geometry {
    use HalfSpaceSense::{Inside, Outside};
    let pb = |partner: usize| BoundaryType::PeriodicWith(partner);
    let rods = [(1.2, 1.7), (10.8, 1.7), (1.2, 10.3), (10.8, 10.3)];
    let mut surfaces = vec![
        SurfaceKind::XPlane(XPlane { x0: 0.0, bc: pb(1) }),
        SurfaceKind::XPlane(XPlane { x0: 12.0, bc: pb(0) }),
        SurfaceKind::YPlane(YPlane { y0: 0.0, bc: pb(3) }),
        SurfaceKind::YPlane(YPlane { y0: 12.0, bc: pb(2) }),
        SurfaceKind::ZPlane(ZPlane { z0: -1.0, bc: BoundaryType::Reflective }),
        SurfaceKind::ZPlane(ZPlane { z0: 1.0, bc: BoundaryType::Reflective }),
    ];
    for (cx, cy) in rods {
        surfaces.push(SurfaceKind::ZCylinder(ZCylinder {
            x0: cx,
            y0: cy,
            r: 0.8,
            bc: BoundaryType::Transmissive,
        }));
    }
    let box_tokens =
        [(0, Outside), (1, Inside), (2, Outside), (3, Inside), (4, Outside), (5, Inside)];
    let mut cells = Vec::new();
    for k in 0..4 {
        let mut t = box_tokens.to_vec();
        t.push((6 + k, Inside));
        cells.push(Cell::material(1 + k as i32, intersection(&t), 0, 293.6));
    }
    let mut m = box_tokens.to_vec();
    for k in 0..4 {
        m.push((6 + k, Outside));
    }
    cells.push(Cell::material(5, intersection(&m), 1, 293.6));
    Geometry {
        surfaces,
        cells,
        universes: vec![Universe { id: 0, cell_indices: vec![0, 1, 2, 3, 4] }],
        lattices: vec![],
        root_universe: 0,
    }
}

/// One configuration: geometry, library, source box.
struct Config {
    name: &'static str,
    bc: &'static str,
    geom: Geometry,
    lib: MgxsLibrary,
    src: SourceBox,
}

fn configs() -> Vec<Config> {
    let mut v = Vec::new();
    for (name, t) in [("A0.5", 0.5), ("A1.0", 1.0), ("A3.0", 3.0)] {
        for (bc_name, bc) in [("reflective", BoundaryType::Reflective), ("white", BoundaryType::White)]
        {
            v.push(Config {
                name,
                bc: bc_name,
                geom: slab(t, bc),
                lib: MgxsLibrary::new(vec![fuel_slab_xs()]),
                src: SourceBox {
                    lower: Position::new(0.0, -1.0, -1.0),
                    upper: Position::new(t, 1.0, 1.0),
                },
            });
        }
    }
    for (name, cx, cy) in [("B1", 1.2, 1.7), ("B0", 3.0, 3.0)] {
        for (bc_name, periodic) in [("reflective", false), ("periodic", true)] {
            v.push(Config {
                name,
                bc: bc_name,
                geom: rod_box(cx, cy, periodic),
                lib: MgxsLibrary::new(vec![fuel2_xs(), mod_xs()]),
                src: SourceBox {
                    lower: Position::new(cx - 0.8, cy - 0.8, -1.0),
                    upper: Position::new(cx + 0.8, cy + 0.8, 1.0),
                },
            });
        }
    }
    // Diagnostic only, never part of the gate: `OUTRAM_BC259_ONLY=B1U`.
    if std::env::var("OUTRAM_BC259_ONLY").as_deref() == Ok("B1U") {
        v.push(Config {
            name: "B1U",
            bc: "periodic12",
            geom: unfolded_b1(),
            lib: MgxsLibrary::new(vec![fuel2_xs(), mod_xs()]),
            src: SourceBox {
                lower: Position::new(0.0, 0.0, -1.0),
                upper: Position::new(12.0, 12.0, 1.0),
            },
        });
    }
    v
}

fn run_one(c: &Config, seed: u64) -> f64 {
    let settings = MgSettings {
        n_particles: env_usize("OUTRAM_BC259_PARTICLES", PARTICLES),
        n_inactive: env_usize("OUTRAM_BC259_INACTIVE", INACTIVE),
        n_active: env_usize("OUTRAM_BC259_ACTIVE", ACTIVE),
        seed,
    };
    run_keff_mg(&c.geom, &c.lib, SourceBox { lower: c.src.lower, upper: c.src.upper }, &settings)
        .k_mean
}

fn mean_sem(k: &[f64]) -> (f64, f64) {
    let n = k.len() as f64;
    let m = k.iter().sum::<f64>() / n;
    let var = k.iter().map(|x| (x - m) * (x - m)).sum::<f64>() / (n - 1.0);
    (m, (var / n).sqrt())
}

/// OpenMC 0.15.3 (`27e38e8`) ensembles, seeds 1..=16 (the gate's N; the B1
/// replication on seeds 17-32 is in the README, not here), from
/// `openmc_inputs/ensemble.py`: `(case, bc, mean, sem over seeds)`.
/// Filled from `verification_and_validation/white_periodic_vs_openmc_259/
/// openmc_results/*.json` by `openmc_inputs/analyze.py`.
const OPENMC: &[(&str, &str, f64, f64)] = &[
    ("A0.5", "reflective", 0.546074, 0.000082),
    ("A0.5", "white", 0.524712, 0.000075),
    ("A1.0", "reflective", 0.866375, 0.000098),
    ("A1.0", "white", 0.853907, 0.000134),
    ("A3.0", "reflective", 1.395651, 0.000160),
    ("A3.0", "white", 1.394523, 0.000104),
    ("B1", "reflective", 1.380647, 0.000110),
    ("B1", "periodic", 1.358985, 0.000102),
    ("B0", "reflective", 1.358901, 0.000103),
    ("B0", "periodic", 1.358970, 0.000114),
];

fn openmc_ref(case: &str, bc: &str) -> Option<(f64, f64)> {
    OPENMC.iter().find(|r| r.0 == case && r.1 == bc).map(|r| (r.2, r.3))
}

/// Worker-count independent: each (config, seed) is its own run.
fn ensemble(cfgs: &[Config], seeds: &[u64]) -> Vec<Vec<f64>> {
    let jobs: Vec<(usize, u64)> =
        (0..cfgs.len()).flat_map(|i| seeds.iter().map(move |s| (i, *s))).collect();
    let next = std::sync::atomic::AtomicUsize::new(0);
    let out = std::sync::Mutex::new(vec![vec![f64::NAN; seeds.len()]; cfgs.len()]);
    std::thread::scope(|sc| {
        for _ in 0..2 {
            sc.spawn(|| loop {
                let j = next.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                if j >= jobs.len() {
                    break;
                }
                let (i, s) = jobs[j];
                let k = run_one(&cfgs[i], s);
                let col = seeds.iter().position(|x| *x == s).unwrap();
                out.lock().unwrap()[i][col] = k;
            });
        }
    });
    out.into_inner().unwrap()
}

fn z_of(a: (f64, f64), b: (f64, f64)) -> f64 {
    (a.0 - b.0) / (a.1 * a.1 + b.1 * b.1).sqrt()
}

/// The gate: every k and every boundary-condition difference `d` within
/// `|z| <= 2` of OpenMC, and the Case B1 / A differences resolved in the
/// predicted direction. Prints the full table.
#[test]
fn white_and_periodic_ensembles_match_openmc() {
    let n: usize = std::env::var("OUTRAM_BC259_SEEDS").ok().and_then(|s| s.parse().ok()).unwrap_or(16);
    let first: u64 = std::env::var("OUTRAM_BC259_FIRST").ok().and_then(|s| s.parse().ok()).unwrap_or(1);
    let seeds: Vec<u64> = (first..first + n as u64).collect();
    // OUTRAM_BC259_ONLY=<case> runs one case only (replication blocks) and
    // writes the JSON without the cross-case table or the gate.
    let only = std::env::var("OUTRAM_BC259_ONLY").ok();
    let mut cfgs = configs();
    if let Some(o) = &only {
        cfgs.retain(|c| c.name == o.as_str());
    }
    for c in &cfgs {
        c.geom.validate_boundary_conditions().expect("boundary conditions implemented");
    }
    let t0 = std::time::Instant::now();
    let ks = ensemble(&cfgs, &seeds);
    println!("wall {:.0} s for {} runs", t0.elapsed().as_secs_f64(), cfgs.len() * seeds.len());

    if let Some(dir) = std::env::var_os("OUTRAM_BC259_OUT") {
        let dir = std::path::PathBuf::from(dir);
        std::fs::create_dir_all(&dir).unwrap();
        for (c, k) in cfgs.iter().zip(&ks) {
            let body = format!(
                "{{\"case\":\"{}\",\"bc\":\"{}\",\"seeds\":{:?},\"k\":{:?}}}",
                c.name, c.bc, seeds, k
            );
            std::fs::write(dir.join(format!("{}_{}_s{}.json", c.name, c.bc, first)), body).unwrap();
        }
    }

    if only.is_some() {
        return;
    }
    let mut worst: f64 = 0.0;
    println!("case  bc          ours (mean +/- sem)     openmc (mean +/- sem)      z");
    let mut stats = std::collections::BTreeMap::new();
    for (c, k) in cfgs.iter().zip(&ks) {
        let ours = mean_sem(k);
        stats.insert((c.name, c.bc), (ours, k.clone()));
        if let Some(r) = openmc_ref(c.name, c.bc) {
            let z = z_of(ours, r);
            worst = worst.max(z.abs());
            println!("{:5} {:10}  {:.5} +/- {:.5}   {:.5} +/- {:.5}  {:+.2}", c.name, c.bc, ours.0, ours.1, r.0, r.1, z);
        } else {
            println!("{:5} {:10}  {:.5} +/- {:.5}   (no OpenMC reference yet)", c.name, c.bc, ours.0, ours.1);
        }
    }
    for (case, other) in [("A0.5", "white"), ("A1.0", "white"), ("A3.0", "white"), ("B1", "periodic"), ("B0", "periodic")] {
        let a = &stats[&(case, "reflective")];
        let b = &stats[&(case, other)];
        let d = (b.0 .0 - a.0 .0, (a.0 .1 * a.0 .1 + b.0 .1 * b.0 .1).sqrt());
        if let (Some(ra), Some(rb)) = (openmc_ref(case, "reflective"), openmc_ref(case, other)) {
            let rd = (rb.0 - ra.0, (ra.1 * ra.1 + rb.1 * rb.1).sqrt());
            let z = z_of(d, rd);
            worst = worst.max(z.abs());
            println!("{case:5} d({other}-refl)  ours {:+.5} +/- {:.5}  openmc {:+.5} +/- {:.5}  z {:+.2}", d.0, d.1, rd.0, rd.1, z);
        } else {
            println!("{case:5} d({other}-refl)  ours {:+.5} +/- {:.5}", d.0, d.1);
        }
    }
    if !OPENMC.is_empty() {
        assert!(worst <= 2.0, "worst |z| = {worst:.2} against OpenMC exceeds the pre-declared gate of 2");
    }
}

/// Pictures of what the solver sees (AGENTS.md geometry rule): the xy slice
/// of the slab, of both rod boxes. `OUTRAM_PLOT_OUT=<dir>` writes PNGs.
#[test]
fn draw_the_models() {
    let out = std::env::var_os("OUTRAM_PLOT_OUT").map(std::path::PathBuf::from);
    if let Some(d) = &out {
        std::fs::create_dir_all(d).unwrap();
    }
    let fuel = Rgb::new(200, 90, 40);
    let moderator = Rgb::new(70, 130, 200);
    let rod_palette = [(fuel, "fuel2 rod"), (moderator, "mod")];
    let cases: Vec<(String, Geometry, Vec<(Rgb, &str)>, Position, [f64; 2])> = vec![
        ("slab_T1.0_xy".into(), slab(1.0, BoundaryType::White), vec![(fuel, "fuel (slab)")], Position::new(0.5, 0.0, 0.0), [3.0, 3.0]),
        ("B1_periodic_xy".into(), rod_box(1.2, 1.7, true), rod_palette.to_vec(), Position::new(3.0, 3.0, 0.0), [6.6, 6.6]),
        ("B0_periodic_xy".into(), rod_box(3.0, 3.0, true), rod_palette.to_vec(), Position::new(3.0, 3.0, 0.0), [6.6, 6.6]),
    ];
    for (name, geom, palette, origin, width) in cases {
        let plot = SlicePlot::new(PlotBasis::Xy, origin, width, [440, 440]);
        let (raw, img) = render_material_slice(&geom, &plot, &palette, &name);
        let n = raw.pixels.iter().filter(|c| **c == palette[0].0).count();
        assert!(n > 0, "{name}: no pixels of the first material");
        if palette.len() > 1 {
            assert!(raw.pixels.iter().any(|c| *c == palette[1].0), "{name}: no moderator pixels");
        }
        if let Some(d) = &out {
            img.write_png(d.join(format!("{name}.png"))).unwrap();
        }
    }
}
