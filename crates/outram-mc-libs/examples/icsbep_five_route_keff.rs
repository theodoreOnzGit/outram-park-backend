// SPDX-License-Identifier: GPL-3.0

//! **Four ICSBEP cases on outram-mc, by nuclear-data route** — the outram-mc
//! half of the five-route study
//! (`verification_and_validation/icsbep/five_route_keff/`).
//!
//! | route | transport | nuclear data | this program's `--route` |
//! |---|---|---|---|
//! | 1 | OpenMC | NJOY2016 ACE -> HDF5 | — (`openmc_inputs/icsbep_openmc.py`) |
//! | 2 | OpenMC | Rust-NJOY ACE -> HDF5 | — (same script) |
//! | 3 | outram-mc | NJOY2016 ACE | `ace --ace-dir <njoy lib>` |
//! | 4 | outram-mc | Rust NJOY, ENDF read directly | `endf` |
//! | 5 | outram-mc | Rust-NJOY ACE | `ace --ace-dir <rust lib>` |
//!
//! # Why one driver and not a flag on each existing example
//!
//! The models are the ones in `godiva_keff_endf_local.rs`, `jemima_keff.rs`,
//! `hst009_keff.rs` and `lct008_ace_roundtrip.rs`, transcribed here with their
//! atom densities and radii unchanged (each constant block cites its source).
//! A route flag on four programs would have meant four copies of the ACE-loading
//! and per-seed CSV code; this way there is one, and every case × route row in
//! the study's CSV is written by the same lines. The per-case examples keep
//! their own ablation knobs and V&V gates, which this program does not repeat.
//!
//! # Physics carried, per route — the same default-on set everywhere it can be
//!
//! - **URR probability tables.** ENDF route: built at construction
//!   (`with_urr_probability_tables(.., 20, 16, 2000)`, the crate default). ACE
//!   routes: read from the table's UNR block — NJOY2016's `purr 20/64` or the
//!   port's `build_full_with_purr(20, 64, 10000)`. Same bins, **different ladder
//!   and sample counts**; stated, not hidden.
//! - **DBRC, E <= 1 keV, every nuclide with 0 K elastic.** ENDF route: from
//!   RECONR's 0 K grid. ACE routes: from the `0K/` companion table next to each
//!   `293.6K/` table, paired by `Nuclide::from_ace_file`.
//! - **S(a,b) H in H2O** on H-1 in water-bearing materials (HST-009, LCT-008s).
//!   ENDF route: `ThermalScattering::from_endf_file` on `tsl-HinH2O.endf`
//!   (this crate's own emission grid). ACE routes: `ThermalScattering::from_ace`
//!   on the library's `HinH2O.ace` (IFENG = 0, NJOY2016's grid and bin counts).
//!
//! Every run prints, and writes to the CSV, how many nuclides carry URR and
//! DBRC and whether S(a,b) is attached, so a route that silently lost a term
//! shows in the data rather than in the residual.
//!
//! # LCT-008 is the case-1 LATTICE, on an 11-nuclide tier
//!
//! `lct008` runs the model `lct008_keff.rs` runs (shared through
//! `common/lct008_model.rs`) on that example's `--cheap-nuclides` tier; see
//! [`lct008_lattice_case`]. A homogenised-sphere case (`lct008s`) existed briefly
//! on 2026-09-28/29 and was **deleted 2026-09-29 at the maintainer's direction**
//! because it was the wrong model.
//!
//! # Usage
//!
//! ```text
//! RAYON_NUM_THREADS=8 cargo run --release -p outram-mc-libs --features endf-pebble-cases \
//!     --example icsbep_five_route_keff -- --case godiva --route endf \
//!     --seeds 1,2,3 --csv out.csv --label route4 [--ace-dir DIR] [--threads 8] \
//!     [--particles 5000 --inactive 40 --active 120] [--commit SHA]
//! ```
//!
//! One CSV row is **appended per seed as soon as it finishes**, so a launcher
//! can resume by asking only for the seeds not yet in the file.

use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Instant;

use njoy_outram_park_fork::reference_data::reference_endf;
use outram_mc_libs::geometry::cell::{Cell, HalfSpaceSense, RegionToken};
use outram_mc_libs::geometry::geometry::Geometry;
use outram_mc_libs::geometry::position::Position;
use outram_mc_libs::geometry::surface::{BoundaryType, Sphere, SurfaceKind, ZCylinder, ZPlane};
use outram_mc_libs::geometry::universe::Universe;
use outram_mc_libs::material::material::{Material, NuclideComponent};
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::material::thermal::ThermalScattering;
use outram_mc_libs::pebble_beds::fhr_pebble::fhr_pebble_geometry;
use outram_mc_libs::physics::compute::{ComputeType, ThreadCount};
use outram_mc_libs::physics::keff::{run_keff, KeffResult, KeffSettings};
use outram_mc_libs::physics::transport_csg::{run_keff_csg, SourceBox};

#[path = "common/lct008_model.rs"]
mod lct008_model;

const TEMP_K: f64 = 293.6;

/// `(name, ENDF tape)` — the same tapes every source example reads.
const TAPES: [(&str, &str); 12] = [
    ("U234", "n-092_U_234-ENDF8.0.endf"),
    ("U235", "n-092_U_235-ENDF8.0.endf"),
    ("U238", "n-092_U_238.endf"),
    ("F19", "n-009_F_019-ENDF8.0.endf"),
    ("O16", "n-008_O_016-ENDF8.0.endf"),
    ("H1", "n-001_H_001-ENDF8.0-Beta6.endf"),
    ("Al27", "n-013_Al_027-ENDF8.0.endf"),
    ("Si28", "n-014_Si_028-ENDF8.0.endf"),
    ("Si29", "n-014_Si_029-ENDF8.0.endf"),
    ("Si30", "n-014_Si_030-ENDF8.0.endf"),
    ("Mn55", "n-025_Mn_055-ENDF8.0.endf"),
    ("B10", "n-005_B_010-ENDF8.0.endf"),
];

/// One material: name and `(nuclide, atoms/b-cm)`.
type Mat = (&'static str, Vec<(&'static str, f64)>);

/// The geometry a case runs on.
enum Model {
    /// Bare homogeneous sphere, `run_keff` (as `godiva_keff_endf_local.rs`).
    Sphere(f64),
    /// CSG geometry with a source box, `run_keff_csg`.
    Csg(Geometry, SourceBox),
}

struct Case {
    materials: Vec<Mat>,
    /// Which nuclides take S(a,b) H in H2O (only H-1, and only where water is).
    sab_on_h1: bool,
    model: Model,
    defaults: (usize, usize, usize),
}

fn case(name: &str) -> Case {
    match name {
        // godiva_keff_endf_local.rs: HEU-MET-FAST-001, r = 8.7407 cm.
        "godiva" => Case {
            materials: vec![(
                "Godiva HEU",
                vec![
                    ("U234", 4.9184e-4),
                    ("U235", 4.4994e-2),
                    ("U238", 2.4984e-3),
                ],
            )],
            sab_on_h1: false,
            model: Model::Sphere(8.7407),
            defaults: (5000, 40, 120),
        },
        // jemima_keff.rs: IEU-MET-FAST-002, four-cell cylinder.
        "jemima" => Case {
            materials: vec![
                (
                    "oralloy core",
                    vec![
                        ("U234", 8.4430e-05),
                        ("U235", 7.7777e-03),
                        ("U238", 3.9671e-02),
                    ],
                ),
                (
                    "natural uranium reflector",
                    vec![
                        ("U234", 2.6433e-06),
                        ("U235", 3.4603e-04),
                        ("U238", 4.7711e-02),
                    ],
                ),
            ],
            sab_on_h1: false,
            model: jemima_model(),
            defaults: (5000, 40, 120),
        },
        // hst009_keff.rs: HEU-SOL-THERM-009 case 1, with its three stated
        // approximations (U-236 and Cu/Zn omitted, O-17 folded into O-16).
        "hst009" => Case {
            materials: vec![
                (
                    "uranium oxyfluoride solution",
                    vec![
                        ("U234", 1.7561e-05),
                        ("U235", 1.6626e-03),
                        ("U238", 9.4079e-05),
                        ("F19", 3.5663e-03),
                        ("O16", 3.334735656e-02 + 1.264344e-05),
                        ("H1", 5.9587e-02),
                    ],
                ),
                (
                    "1100 aluminium tank",
                    vec![
                        ("Al27", 5.9699e-02),
                        ("Si28", 5.09126279536e-04),
                        ("Si29", 2.5851979832e-05),
                        ("Si30", 1.7041740632e-05),
                        ("Mn55", 1.4853e-05),
                    ],
                ),
                (
                    "water reflector",
                    vec![
                        ("H1", 6.6659e-02),
                        ("O16", 3.3316368309e-02 + 1.2631691e-05),
                    ],
                ),
            ],
            sab_on_h1: true,
            model: {
                let (rs, rt, rr) = (11.5177, 11.6764, 35.0);
                let g = fhr_pebble_geometry(0.0, rs, rt, rr, 0, 1, 2, BoundaryType::Vacuum, TEMP_K);
                Model::Csg(
                    g,
                    SourceBox {
                        lower: Position::new(-rs, -rs, -rs),
                        upper: Position::new(rs, rs, rs),
                    },
                )
            },
            defaults: (5000, 40, 120),
        },
        "lct008" => lct008_lattice_case(),
        other => panic!("--case must be godiva|jemima|hst009|lct008, got {other}"),
    }
}

/// **LEU-COMP-THERM-008 case 1, the real lattice**, from the SAME model
/// `lct008_keff.rs` runs (`common/lct008_model.rs`: the committed
/// `mit-crpg/benchmarks` OpenMC cards parsed at run time, CSG lattice, geometry
/// self-check), restricted to that example's `--cheap-nuclides` tier
/// (`TAPES_CHEAP`, 11 nuclides). Model nuclides outside the tier are **dropped,
/// not renormalised**, exactly as `lct008_keff.rs --cheap-nuclides` does; the
/// list is printed on every run and recorded in the five-route V&V record.
fn lct008_lattice_case() -> Case {
    let _ = lct008_model::ACTIVE_CASE.set(1);
    let spec = lct008_model::parse_materials(lct008_model::materials_xml());
    let keep = |n: &str| lct008_model::TAPES_CHEAP.iter().any(|(m, _)| *m == n);
    let mut slots: BTreeMap<String, usize> = BTreeMap::new();
    let mut omitted: BTreeMap<String, f64> = BTreeMap::new();
    for m in &spec {
        for (n, ao) in &m.nuclides {
            if keep(n) {
                let k = slots.len();
                slots.entry(n.clone()).or_insert(k);
            } else {
                *omitted.entry(n.clone()).or_insert(0.0) += ao;
            }
        }
    }
    let (mats, _) = lct008_model::build_materials(&spec, &slots, &omitted, false);
    lct008_model::report_omissions(&spec, &omitted);
    let ids: Vec<i32> = mats.iter().map(|m| m.id).collect();
    assert_eq!(
        ids,
        (1..=ids.len() as i32).collect::<Vec<_>>(),
        "material ids must be 1..n"
    );
    let geom = lct008_model::build_geometry(&mats, true);
    let _ = lct008_model::check_geometry(&geom, &mats);
    let sab_on_h1 = spec.iter().any(|m| m.sab.as_deref() == Some("c_H_in_H2O"));
    // Back to (name, [(nuclide, density)]) in the model's own order, so the
    // generic loader below sees the lattice like any other case. `String::leak`
    // gives the 'static names the case table uses; a handful of short strings
    // per process.
    let by_slot: BTreeMap<usize, String> = slots.iter().map(|(n, &i)| (i, n.clone())).collect();
    let materials: Vec<Mat> = mats
        .iter()
        .map(|m| {
            let comps = m
                .components
                .iter()
                .map(|c| (&*by_slot[&c.nuclide_idx].clone().leak(), c.atom_density))
                .collect();
            (&*m.name.clone().leak(), comps)
        })
        .collect();
    Case {
        materials,
        sab_on_h1,
        model: Model::Csg(
            geom,
            SourceBox {
                lower: Position::new(
                    -lct008_model::R_CORE,
                    -lct008_model::R_CORE,
                    lct008_model::Z_LO,
                ),
                upper: Position::new(
                    lct008_model::R_CORE,
                    lct008_model::R_CORE,
                    lct008_model::Z_HI,
                ),
            },
        ),
        defaults: (10_000, 250, 400),
    }
}

/// `jemima_keff.rs`'s `build_geometry`, unchanged: bottom reflector, core,
/// radial reflector, top reflector; material 0 = core, 1 = reflector.
fn jemima_model() -> Model {
    const Z_BOT: f64 = 0.0;
    const Z_CORE_LO: f64 = 7.62;
    const Z_CORE_HI: f64 = 39.571;
    const Z_TOP: f64 = 47.0894;
    const R_CORE: f64 = 19.05;
    const R_OUT: f64 = 26.6446;
    let zp = |z0: f64, bc: BoundaryType| SurfaceKind::ZPlane(ZPlane { z0, bc });
    let zc = |r: f64, bc: BoundaryType| {
        SurfaceKind::ZCylinder(ZCylinder {
            x0: 0.0,
            y0: 0.0,
            r,
            bc,
        })
    };
    let surfaces = vec![
        zp(Z_BOT, BoundaryType::Vacuum),
        zp(Z_CORE_LO, BoundaryType::Transmissive),
        zp(Z_CORE_HI, BoundaryType::Transmissive),
        zp(Z_TOP, BoundaryType::Vacuum),
        zc(R_CORE, BoundaryType::Transmissive),
        zc(R_OUT, BoundaryType::Vacuum),
    ];
    let hs =
        |surface_idx: usize, sense: HalfSpaceSense| RegionToken::HalfSpace { surface_idx, sense };
    let out = |i: usize| hs(i, HalfSpaceSense::Outside);
    let ins = |i: usize| hs(i, HalfSpaceSense::Inside);
    let and = RegionToken::Intersection;
    let cells = vec![
        Cell::material(1, vec![out(0), ins(1), and, ins(5), and], 1, TEMP_K),
        Cell::material(2, vec![out(1), ins(2), and, ins(4), and], 0, TEMP_K),
        Cell::material(
            3,
            vec![out(1), ins(2), and, out(4), and, ins(5), and],
            1,
            TEMP_K,
        ),
        Cell::material(4, vec![out(2), ins(3), and, ins(5), and], 1, TEMP_K),
    ];
    let cell_indices = (0..cells.len()).collect();
    let g = Geometry {
        surfaces,
        cells,
        universes: vec![Universe {
            id: 0,
            cell_indices,
        }],
        lattices: vec![],
        root_universe: 0,
    };
    Model::Csg(
        g,
        SourceBox {
            lower: Position::new(-R_CORE, -R_CORE, Z_CORE_LO),
            upper: Position::new(R_CORE, R_CORE, Z_CORE_HI),
        },
    )
}

fn flag(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

fn load_endf(name: &str) -> Nuclide {
    let file = TAPES
        .iter()
        .find(|(n, _)| *n == name)
        .expect("known nuclide")
        .1;
    let p = reference_endf(file).unwrap_or_else(|| panic!("missing reference tape {file}"));
    Nuclide::from_endf_file_with_speed(&p, name, TEMP_K, outram_mc_libs::vv::bench_speed())
        .unwrap_or_else(|e| panic!("from_endf_file_with_speed({name}): {e}"))
}

fn load_ace(dir: &Path, name: &str) -> Nuclide {
    let p = dir.join("293.6K").join(format!("{name}.ace"));
    let n = Nuclide::from_ace_file(&p, name)
        .unwrap_or_else(|e| panic!("from_ace_file({}): {e}", p.display()));
    if let Some(why) = n.dbrc_unavailable_reason() {
        eprintln!(
            "    {name}: DBRC OFF -- {why}\n{}",
            Nuclide::zero_kelvin_companion_report(&p)
        );
    }
    n
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let case_name = flag(&args, "--case").expect("--case");
    let route = flag(&args, "--route").expect("--route endf|ace");
    let label = flag(&args, "--label").unwrap_or_else(|| route.clone());
    let csv = PathBuf::from(flag(&args, "--csv").expect("--csv <path>"));
    let commit = flag(&args, "--commit").unwrap_or_else(|| "unknown".into());
    let threads: usize = flag(&args, "--threads").map_or(8, |s| s.parse().expect("--threads"));
    let seeds: Vec<u64> = flag(&args, "--seeds")
        .expect("--seeds 1,2,3")
        .split(',')
        .filter(|s| !s.is_empty())
        .map(|s| s.parse().expect("seed"))
        .collect();
    let mut c = case(&case_name);
    // `--variant solution-inf|solution-bare` (diagnostic A/B, GitHub #367, HST-009
    // only): the solution sphere alone, with a REFLECTIVE boundary (its k_inf) or
    // with vacuum outside (no tank, no water). Splits a residual between the
    // multiplying medium and the leakage/reflector return.
    if let Some(v) = flag(&args, "--variant") {
        assert_eq!(case_name, "hst009", "--variant is defined for hst009 only");
        let bc = match v.as_str() {
            "solution-inf" => BoundaryType::Reflective,
            "solution-bare" => BoundaryType::Vacuum,
            other => panic!("unknown --variant {other}"),
        };
        eprintln!("  VARIANT: {v} (solution sphere alone, {bc:?} boundary)");
        let r = 11.5177;
        let g = Geometry {
            surfaces: vec![SurfaceKind::Sphere(Sphere {
                x0: 0.0,
                y0: 0.0,
                z0: 0.0,
                r,
                bc,
            })],
            cells: vec![Cell::material(
                1,
                vec![RegionToken::HalfSpace {
                    surface_idx: 0,
                    sense: HalfSpaceSense::Inside,
                }],
                0,
                TEMP_K,
            )],
            universes: vec![Universe {
                id: 0,
                cell_indices: vec![0],
            }],
            lattices: vec![],
            root_universe: 0,
        };
        c.materials.truncate(1);
        c.model = Model::Csg(
            g,
            SourceBox {
                lower: Position::new(-r, -r, -r),
                upper: Position::new(r, r, r),
            },
        );
    }
    // `--drop-nuclide N` (diagnostic A/B, GitHub #367): remove one nuclide from
    // every material, to localise a residual to it. Not renormalised.
    if let Some(drop) = flag(&args, "--drop-nuclide") {
        eprintln!("  ABLATION: {drop} removed from every material (--drop-nuclide)");
        for (_, comps) in c.materials.iter_mut() {
            comps.retain(|(n, _)| *n != drop);
        }
    }
    let get = |f: &str, d: usize| flag(&args, f).map_or(d, |s| s.parse().expect(f));
    let (np, ni, na) = (
        get("--particles", c.defaults.0),
        get("--inactive", c.defaults.1),
        get("--active", c.defaults.2),
    );

    // Nuclide list in first-use order across the case's materials.
    let mut names: Vec<&str> = Vec::new();
    for (_, comps) in &c.materials {
        for (n, _) in comps {
            if !names.contains(n) {
                names.push(n);
            }
        }
    }

    eprintln!("== {case_name} / route {route} ({label}) ==");
    let t0 = Instant::now();
    let ace_dir = flag(&args, "--ace-dir").map(PathBuf::from);
    let (mut nuclides, sab): (Vec<Nuclide>, Option<ThermalScattering>) = match route.as_str() {
        "endf" => {
            let mut n: Vec<Nuclide> = names.iter().map(|n| load_endf(n)).collect();
            // `--urr-ladders N [--urr-samples M]` (diagnostic A/B): rebuild the
            // ENDF route's PURR tables with the ACE libraries' ladder/sample
            // counts (20 bins / 64 ladders / 10 000 samples) instead of the
            // construction default (20 / 16 / 2000). Same routine, same bins;
            // only the Monte Carlo resolution of the tables changes.
            if let Some(nl) = flag(&args, "--urr-ladders") {
                let nl: usize = nl.parse().expect("--urr-ladders");
                let ns: usize = flag(&args, "--urr-samples")
                    .map_or(10_000, |v| v.parse().expect("--urr-samples"));
                n = n
                    .into_iter()
                    .zip(names.iter())
                    .map(|(nuc, name)| {
                        if !nuc.has_urr_probability_tables() {
                            return nuc;
                        }
                        let file = TAPES.iter().find(|(m, _)| m == name).expect("tape").1;
                        let p = reference_endf(file).expect("tape");
                        let tape = njoy_outram_park_fork::endf::tape::Tape::read_file(&p).expect("tape");
                        let mat = tape.materials()[0];
                        eprintln!("    {name}: URR tables rebuilt with 20 bins / {nl} ladders / {ns} samples");
                        nuc.without_urr_probability_tables()
                            .with_urr_probability_tables(&tape, mat, TEMP_K, 20, nl, ns)
                            .expect("PURR")
                    })
                    .collect();
            }
            // `--sab-ace PATH` (diagnostic A/B): the ENDF route with H(H2O) taken
            // from a thermal ACE table instead of this crate's own ENDF kernel.
            if let Some(p) = flag(&args, "--sab-ace") {
                let raw = njoy_outram_park_fork::acer::read::read(&p)
                    .unwrap_or_else(|e| panic!("read {p}: {e}"));
                eprintln!("    H1: S(a,b) from {p} (--sab-ace)");
                let sab = c.sab_on_h1.then(|| {
                    ThermalScattering::from_ace(&raw, "c_H_in_H2O").expect("S(a,b) from ACE")
                });
                (n, sab)
            } else {
                let sab = c.sab_on_h1.then(|| {
                    let p = reference_endf("tsl-HinH2O.endf").expect("H(H2O) tape");
                    ThermalScattering::from_endf_file(
                        p.to_str().expect("path"),
                        1,
                        TEMP_K,
                        "c_H_in_H2O",
                    )
                    .expect("H(H2O) S(a,b) from ENDF")
                });
                (n, sab)
            }
        }
        "ace" => {
            let dir = ace_dir.clone().expect("--route ace needs --ace-dir");
            // `--endf-nuclides U238,...` (diagnostic): load the listed nuclides
            // from ENDF instead, so a route difference can be localised to one
            // nuclide's ACE table. Unset = every nuclide from ACE, the route.
            let from_endf: Vec<String> = flag(&args, "--endf-nuclides")
                .map(|v| v.split(',').map(str::to_string).collect())
                .unwrap_or_default();
            let n = names
                .iter()
                .map(|n| {
                    if from_endf.iter().any(|m| m == n) {
                        eprintln!("    {n}: from ENDF (--endf-nuclides)");
                        load_endf(n)
                    } else {
                        load_ace(&dir, n)
                    }
                })
                .collect();
            let sab = c.sab_on_h1.then(|| {
                let p = dir.join("293.6K").join("HinH2O.ace");
                let raw = njoy_outram_park_fork::acer::read::read(&p)
                    .unwrap_or_else(|e| panic!("read {}: {e}", p.display()));
                ThermalScattering::from_ace(&raw, "c_H_in_H2O").expect("H(H2O) S(a,b) from ACE")
            });
            (n, sab)
        }
        other => panic!("--route must be endf|ace, got {other}"),
    };
    // `--no-sab` (diagnostic A/B, GitHub #367): H-1 stays free gas even where
    // the case carries S(a,b), to localise a thermal-scattering residual.
    let sab = if args.iter().any(|a| a == "--no-sab") {
        eprintln!("  ABLATION: no S(a,b) (--no-sab); H-1 is free gas");
        None
    } else {
        sab
    };
    if let Some(s) = sab {
        let i = names
            .iter()
            .position(|n| *n == "H1")
            .expect("S(a,b) needs H1 in the case");
        let h = nuclides.remove(i);
        nuclides.insert(i, h.with_thermal_scattering(s));
    }
    // `--ablate a,b` (diagnostic): explicit, visible ablations applied to every
    // nuclide, for localising a route difference. Recorded in the CSV `route`
    // label by the caller; the campaign itself never passes this flag.
    if let Some(list) = flag(&args, "--ablate") {
        for a in list.split(',') {
            eprintln!("  ABLATION: {a}");
            nuclides = nuclides
                .into_iter()
                .map(|n| match a {
                    "iso-elastic" => n.with_isotropic_elastic_scattering(),
                    "iso-inelastic" => n.with_isotropic_inelastic_scattering(),
                    "iso-continuum" => n.with_isotropic_continuum_scattering(),
                    "frozen-nubar" => n.with_frozen_nubar(1.0e6),
                    "frozen-chi" => n.with_frozen_fission_spectrum(1.0e6),
                    "no-urr" => n.without_urr_probability_tables(),
                    "no-dbrc" => n.without_dbrc(),
                    "unit-n2n" => n.with_unit_n2n_multiplicity(),
                    "no-inelastic" => n.without_inelastic(),
                    other => panic!("unknown --ablate {other}"),
                })
                .collect();
        }
    }
    let load_s = t0.elapsed().as_secs_f64();

    let n_urr = nuclides
        .iter()
        .filter(|n| n.has_urr_probability_tables())
        .count();
    let n_dbrc = nuclides.iter().filter(|n| n.has_dbrc()).count();
    eprintln!("  nuclear data ready in {load_s:.1} s");
    for (name, n) in names.iter().zip(&nuclides) {
        eprintln!(
            "    {name:<5} URR {:<5} DBRC {:<5} {}",
            n.has_urr_probability_tables(),
            n.has_dbrc(),
            n.urr_range_ev().map_or(String::new(), |(lo, hi)| format!(
                "URR [{lo:.3e}, {hi:.3e}] eV"
            ))
        );
    }
    eprintln!(
        "  URR on {n_urr}/{} nuclides, DBRC on {n_dbrc}/{}, S(a,b) {}",
        nuclides.len(),
        nuclides.len(),
        c.sab_on_h1
    );

    let idx = |n: &str| names.iter().position(|m| *m == n).expect("nuclide index");
    let materials: Vec<Material> = c
        .materials
        .iter()
        .enumerate()
        .map(|(i, (mname, comps))| Material {
            id: i as i32 + 1,
            name: (*mname).into(),
            temperature: TEMP_K,
            components: comps
                .iter()
                .map(|(n, d)| NuclideComponent {
                    nuclide_idx: idx(n),
                    atom_density: *d,
                })
                .collect(),
        })
        .collect();

    let base = KeffSettings {
        n_particles: np,
        n_inactive: ni,
        n_active: na,
        temperature_k: TEMP_K,
        compute: ComputeType::CpuMultiThread(ThreadCount::Fixed(threads)),
        ..KeffSettings::default()
    };
    let new_file = !csv.is_file();
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&csv)
        .expect("open csv");
    if new_file {
        writeln!(
            f,
            "case,route,code,seed,k,k_std_internal,particles,inactive,active,threads,wall_s,load_s,n_nuclides,n_urr,n_dbrc,sab,commit,k_alt"
        )
        .expect("csv header");
    }
    for seed in seeds {
        let s = KeffSettings {
            seed,
            ..base.clone()
        };
        let t = Instant::now();
        let r: KeffResult = match &c.model {
            Model::Sphere(radius) => run_keff(*radius, &materials[0], &nuclides, &s),
            Model::Csg(g, src) => run_keff_csg(g, &materials, &nuclides, *src, &s, None),
        };
        let wall = t.elapsed().as_secs_f64();
        eprintln!(
            "  seed {seed}: k = {:.5} +/- {:.5}   ({wall:.1} s)",
            r.k_mean, r.k_std
        );
        writeln!(
            f,
            "{case_name},{label},outram-mc,{seed},{:.6},{:.6},{np},{ni},{na},{threads},{wall:.1},{load_s:.1},{},{n_urr},{n_dbrc},{},{commit},",
            r.k_mean,
            r.k_std,
            nuclides.len(),
            c.sab_on_h1
        )
        .expect("csv row");
        f.flush().expect("flush csv");
    }
}
