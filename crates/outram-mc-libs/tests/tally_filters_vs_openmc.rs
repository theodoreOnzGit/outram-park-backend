// SPDX-License-Identifier: AGPL-3.0-only

//! **gh:#261 — Experiment A: bin-for-bin tally-filter comparison against OpenMC
//! 0.15.3, deterministic single-neutron beams.**
//!
//! # Methodology
//!
//! Eighteen mono-directional, mono-energetic neutrons (16 random sites from
//! `RandomState(261)`, plus an axis-parallel beam and a body-diagonal beam through
//! every tile centre) cross a 3x1x1 lattice (pitch 6 cm; tile = sphere r = 2 cm
//! of material 1 in material 2; vacuum outside). Both materials are O-16 at
//! 1e-10 / 2e-10 atoms/b-cm, so no collision is sampled and the track is fully
//! determined: the comparison is exact, not statistical, and is made **per site,
//! per bin**. The OpenMC side (`openmc_inputs/exp_a_beams.py`, OpenMC 0.15.3
//! commit 27e38e89, ENDF/B-VIII.0 O-16 via NJOY2016, CE mode, 1 particle x 1
//! batch per site) is committed as plain text in
//! `verification_and_validation/tally_filters_vs_openmc/openmc_reference/`.
//!
//! ~~Only `DistribcellFilter`, `CellInstanceFilter`, `EnergyFunction` and
//! `MeshMaterial` went through the real runner; the history-field filters read
//! fields the transport did not set and were scored through the
//! `tally::history_events` driver.~~ **CORRECTED 2026-11-18 (gh:#261,
//! maintainer-confirmed wiring):** every Experiment A filter except CELL_INSTANCE
//! also has a `real_*` test that runs the real fixed-source runner
//! (`run_fixed_source_traced`); the driver tests are kept as the oracle. ~~The
//! mesh-surface `real_*` tests pin the **count** of tie misses (16 and 1), they
//! do not assert zero.~~ **CHANGED 2026-11-22 (gh:#1709):** the tie misses came
//! from scoring mesh-surface currents per geometry segment with a re-derived
//! crossing count; upstream raytraces once from `r_last_current` with
//! `raytrace_mesh`. Both are now ported, and all four mesh-surface tests
//! (driver and real) assert zero misses and measure worst 0. Experiments B (COLLISION) and C (WEIGHT, survival
//! biasing) at the end of the file are statistical.
//!
//! Pass criterion (fixed before the run, `prediction_before_measuring.md`,
//! amendment 1): relative 2e-8, or absolute 2e-8 for bins under 1; integer-valued
//! currents must match exactly (1e-12).
//!
//! # Results
//!
//! Printed per filter at run time and recorded in
//! `verification_and_validation/tally_filters_vs_openmc/README.md`.

use outram_mc_libs::geometry::cell::{Cell, CellFill, HalfSpaceSense, RegionToken};
use outram_mc_libs::geometry::distribcell::DistribcellOffsets;
use outram_mc_libs::geometry::geometry::Geometry;
use outram_mc_libs::geometry::lattice::{Lattice, RectLattice};
use outram_mc_libs::geometry::position::{Direction, Position};
use outram_mc_libs::geometry::surface::{BoundaryType, Sphere, SurfaceKind, XPlane, YPlane, ZPlane};
use outram_mc_libs::geometry::universe::Universe;
use outram_mc_libs::material::material::{Material, NuclideComponent};
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::physics::fixed_source::{run_fixed_source_traced, FixedSource, FixedSourceSettings};
use outram_mc_libs::physics::transport_csg::trace_csg_history;
use outram_mc_libs::physics::tracking_trace::TraceEvent;
use outram_mc_libs::source::angle::{AngleKind, MonodirectionalAngle};
use outram_mc_libs::source::energy::{EnergyKind, Monoenergetic};
use outram_mc_libs::source::source::IndependentSource;
use outram_mc_libs::source::spatial::{PointSource, SpatialKind};
use outram_mc_libs::tally::filter::FilterKind;
use outram_mc_libs::tally::filter::SurfaceFilter;
use outram_mc_libs::tally::filter_extra::*;
use outram_mc_libs::tally::history_events::{history_events, score_history};
use outram_mc_libs::tally::mesh::{MeshKind, RegularMesh};
use outram_mc_libs::tally::tally::{ScoreType, Tally, TallyBin};
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

const TEMP_K: f64 = 293.6;

fn ref_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("verification_and_validation/tally_filters_vs_openmc/openmc_reference")
}

fn load_sites() -> Vec<[f64; 7]> {
    let txt = std::fs::read_to_string(ref_dir().join("sites.txt")).expect("sites.txt");
    txt.lines()
        .map(|l| {
            let v: Vec<f64> = l.split_whitespace().map(|x| x.parse().unwrap()).collect();
            [v[0], v[1], v[2], v[3], v[4], v[5], v[6]]
        })
        .collect()
}

/// `reference[site][bin]` from `expA_<name>.txt`.
fn load_ref(name: &str) -> Vec<Vec<f64>> {
    let txt = std::fs::read_to_string(ref_dir().join(format!("{name}.txt"))).expect("reference");
    let mut out: Vec<Vec<f64>> = Vec::new();
    for l in txt.lines() {
        let v: Vec<&str> = l.split_whitespace().collect();
        let (s, b, x): (usize, usize, f64) = (v[0].parse().unwrap(), v[1].parse().unwrap(), v[2].parse().unwrap());
        if out.len() <= s {
            out.resize(s + 1, Vec::new());
        }
        assert_eq!(out[s].len(), b);
        out[s].push(x);
    }
    out
}

struct Model {
    geom: Geometry,
    materials: Vec<Material>,
    nuclides: Vec<Nuclide>,
}

/// Surface order matches the OpenMC script's `surf_all`: sphere, x-9, x+9, y-3,
/// y+3, z-3, z+3.
fn model() -> Option<&'static Model> {
    static M: OnceLock<Option<Model>> = OnceLock::new();
    M.get_or_init(|| {
        let path = njoy_outram_park_fork::reference_data::reference_endf("n-008_O_016-ENDF8.0.endf")?;
        let o16 = Nuclide::from_endf_file(
            &path,
            "O16",
            uom::si::f64::ThermodynamicTemperature::new::<uom::si::thermodynamic_temperature::kelvin>(TEMP_K),
            1.0e-3,
        )
        .ok()?;
        let bc = BoundaryType::Vacuum;
        let surfaces = vec![
            SurfaceKind::Sphere(Sphere { x0: 0.0, y0: 0.0, z0: 0.0, r: 2.0, bc: BoundaryType::Transmissive }),
            SurfaceKind::XPlane(XPlane { x0: -9.0, bc }),
            SurfaceKind::XPlane(XPlane { x0: 9.0, bc }),
            SurfaceKind::YPlane(YPlane { y0: -3.0, bc }),
            SurfaceKind::YPlane(YPlane { y0: 3.0, bc }),
            SurfaceKind::ZPlane(ZPlane { z0: -3.0, bc }),
            SurfaceKind::ZPlane(ZPlane { z0: 3.0, bc }),
        ];
        use HalfSpaceSense::{Inside, Outside};
        let hs = |i: usize, s| RegionToken::HalfSpace { surface_idx: i, sense: s };
        let root = vec![
            hs(1, Outside), hs(2, Inside), RegionToken::Intersection,
            hs(3, Outside), RegionToken::Intersection,
            hs(4, Inside), RegionToken::Intersection,
            hs(5, Outside), RegionToken::Intersection,
            hs(6, Inside), RegionToken::Intersection,
        ];
        let geom = Geometry {
            surfaces,
            cells: vec![
                Cell::fill(100, root, CellFill::Lattice(0), Position::ZERO),
                Cell::material(1, vec![hs(0, Inside)], 0, None::<f64>),
                Cell::material(2, vec![hs(0, Outside)], 1, None::<f64>),
            ],
            universes: vec![
                Universe { id: 0, cell_indices: vec![0] },
                Universe { id: 1, cell_indices: vec![1, 2] },
            ],
            lattices: vec![Lattice::Rect(RectLattice {
                id: 5,
                n: [3, 1, 1],
                lower_left: Position::new(-9.0, -3.0, -3.0),
                pitch: [6.0, 6.0, 6.0],
                universes: vec![1; 3],
                outer: None,
            })],
            root_universe: 0,
        };
        let mat = |id: i32, rho: f64| Material {
            id,
            name: format!("dilute O16 {id}"),
            components: vec![NuclideComponent { nuclide_idx: 0, atom_density: rho }],
            temperature: TEMP_K,
        };
        Some(Model { geom, materials: vec![mat(1, 1.0e-10), mat(2, 2.0e-10)], nuclides: vec![o16] })
    })
    .as_ref()
}

fn mesh() -> RegularMesh {
    RegularMesh {
        lower_left: [-9.0, -3.0, -3.0],
        upper_right: [9.0, 3.0, 3.0],
        dimension: [6, 2, 1],
    }
}

fn tally(filters: Vec<FilterKind>, score: ScoreType) -> Tally {
    let n: usize = filters.iter().map(|f| f.n_bins()).product();
    Tally { id: 1, name: "t".into(), filters, scores: vec![score], bins: vec![TallyBin::default(); n], derivative: None }
}

/// Run one site through the REAL fixed-source runner; the tally's per-bin sums.
fn via_runner(m: &Model, proto: &Tally, site: &[f64; 7], tables: &[DistribcellOffsets], i: usize) -> Vec<f64> {
    let mut t = proto.clone();
    let src = FixedSource::Independent(Arc::new(IndependentSource {
        spatial: SpatialKind::Point(PointSource { r: Position::new(site[0], site[1], site[2]) }),
        energy: EnergyKind::Mono(Monoenergetic { e: site[6] }),
        angle: AngleKind::Monodirectional(MonodirectionalAngle {
            d: Direction { u: site[3], v: site[4], w: site[5] },
        }),
        strength: 1.0,
    }));
    run_fixed_source_traced(
        &m.geom,
        &m.materials,
        &m.nuclides,
        &src,
        &FixedSourceSettings { n_particles: 1, n_batches: 1, seed: 1 + i as u64, ..Default::default() },
        Some(&mut t),
        None,
        None,
        tables.first(),
    );
    t.bins.iter().map(|b| b.sum).collect()
}

/// Run one site through the history-event driver.
fn via_driver(m: &Model, t: &Tally, site: &[f64; 7], tables: &[DistribcellOffsets], i: usize) -> Vec<f64> {
    let mut trace: Vec<TraceEvent> = Vec::new();
    let mut seed = 1 + i as u64;
    trace_csg_history(
        Position::new(site[0], site[1], site[2]),
        Direction { u: site[3], v: site[4], w: site[5] },
        site[6],
        &m.geom,
        &m.materials,
        &m.nuclides,
        &mut seed,
        |e| trace.push(e),
    );
    let events = history_events(&m.geom, &trace, tables);
    let mut out = vec![0.0; t.bins.len()];
    score_history(t, &events, &mut out).expect("supported tally");
    out
}

struct Report {
    n_bins: usize,
    n_nonzero: usize,
    worst: f64,
    n_miss: usize,
}

fn compare(name: &str, ours: &[Vec<f64>], theirs: &[Vec<f64>], exact: bool, bins: Option<usize>) -> Report {
    let mut r = Report { n_bins: 0, n_nonzero: 0, worst: 0.0, n_miss: 0 };
    for (s, (a, b)) in ours.iter().zip(theirs).enumerate() {
        let nb = bins.unwrap_or(b.len());
        assert!(a.len() >= nb && b.len() >= nb, "{name}: bin counts differ at site {s}: {} vs {}", a.len(), b.len());
        for k in 0..nb {
            r.n_bins += 1;
            if b[k] != 0.0 {
                r.n_nonzero += 1;
            }
            let d = (a[k] - b[k]).abs();
            let tol = if exact { 1.0e-12 } else { 2.0e-8 * b[k].abs().max(1.0) };
            r.worst = r.worst.max(d / if exact { 1.0 } else { b[k].abs().max(1.0) });
            if d > tol {
                r.n_miss += 1;
                println!("  MISS {name} site {s} bin {k}: ours {:.17e} openmc {:.17e} diff {:.3e}", a[k], b[k], d);
            }
        }
    }
    println!(
        "[{name}] sites {} bins {} nonzero-reference {} worst scaled |diff| {:.3e} misses {}",
        ours.len(), r.n_bins, r.n_nonzero, r.worst, r.n_miss
    );
    r
}

fn run_all(
    name: &str,
    proto: &Tally,
    tables: &[DistribcellOffsets],
    driver: bool,
    exact: bool,
    bins: Option<usize>,
) -> Option<Report> {
    let m = model()?;
    let sites = load_sites();
    let theirs = load_ref(&match name.strip_prefix("A2_") { Some(n) => format!("expA2_{n}"), None => format!("expA_{name}") });
    let ours: Vec<Vec<f64>> = sites
        .iter()
        .enumerate()
        .map(|(i, s)| if driver { via_driver(m, proto, s, tables, i) } else { via_runner(m, proto, s, tables, i) })
        .collect();
    Some(compare(name, &ours, &theirs, exact, bins))
}

fn tables_a(m: &Model) -> Vec<DistribcellOffsets> {
    vec![DistribcellOffsets::build(&m.geom, 1).unwrap(), DistribcellOffsets::build(&m.geom, 2).unwrap()]
}

#[test]
fn distribcell_vs_openmc() {
    let Some(m) = model() else { return };
    let tb = tables_a(m);
    let t = tally(vec![FilterKind::Distribcell(DistribcellFilter { cell_idx: 1, n_instances: 3 })], ScoreType::Flux);
    let r = run_all("distribcell_A", &t, &tb, false, false, None).unwrap();
    assert_eq!(r.n_miss, 0);
}

#[test]
fn cell_instance_vs_openmc() {
    let Some(m) = model() else { return };
    let tb = tables_a(m);
    let pairs = vec![(1, 0), (1, 1), (1, 2), (2, 0), (2, 1), (2, 2)];
    let t = tally(vec![FilterKind::CellInstance(CellInstanceFilter { pairs })], ScoreType::Flux);
    let r = run_all("cell_instance", &t, &tb, true, false, None).unwrap();
    assert_eq!(r.n_miss, 0);
}

#[test]
fn energy_function_vs_openmc() {
    let Some(m) = model() else { return };
    let f = EnergyFunctionFilter::new(
        vec![1.0e3, 1.0e5, 1.0e6, 1.0e7, 2.0e7],
        vec![0.5, 2.0, 3.0, 1.0, 0.2],
    )
    .unwrap();
    let t = tally(vec![FilterKind::EnergyFunction(f)], ScoreType::Flux);
    let r = run_all("energy_function", &t, &[], false, false, None).unwrap();
    assert_eq!(r.n_miss, 0);
    let _ = m;
}

#[test]
fn mesh_material_vs_openmc() {
    let Some(_m) = model() else { return };
    let pairs: Vec<(usize, usize)> = (0..12).flat_map(|e| [(e, 0usize), (e, 1usize)]).collect();
    let t = tally(
        vec![FilterKind::MeshMaterial(MeshMaterialFilter { mesh: MeshKind::Regular(mesh()), pairs })],
        ScoreType::Flux,
    );
    let r = run_all("mesh_material", &t, &[], false, false, None).unwrap();
    assert_eq!(r.n_miss, 0, "MeshMaterial: see the V&V README");
}

/// The same MeshMaterial comparison with the flight split among mesh elements
/// by `history_events::regular_mesh_split` (new behaviour beside the filter).
#[test]
fn mesh_material_split_vs_openmc() {
    let Some(_m) = model() else { return };
    let pairs: Vec<(usize, usize)> = (0..12).flat_map(|e| [(e, 0usize), (e, 1usize)]).collect();
    let t = tally(
        vec![FilterKind::MeshMaterial(MeshMaterialFilter { mesh: MeshKind::Regular(mesh()), pairs })],
        ScoreType::Flux,
    );
    let r = run_all("mesh_material", &t, &[], true, false, None).unwrap();
    assert_eq!(r.n_miss, 0);
}

// ── Experiment A2 (post-hoc, prediction_before_measuring.md amendment 2) ─────
// Same sites and geometry, mesh 5x3x1 so that no mesh plane lies on a tile
// edge or a sphere centre (the registered 6x2x1 mesh makes flights end exactly
// on mesh planes, which tests floating-point tie-breaking, not bin assignment).

fn mesh_a2() -> RegularMesh {
    RegularMesh { lower_left: [-9.0, -3.0, -3.0], upper_right: [9.0, 3.0, 3.0], dimension: [5, 3, 1] }
}

#[test]
fn a2_mesh_surface_vs_openmc() {
    let f = MeshSurfaceFilter::new(MeshKind::Regular(mesh_a2())).unwrap();
    let t = tally(vec![FilterKind::MeshSurface(f)], ScoreType::Current);
    if let Some(r) = run_all("A2_mesh_surface", &t, &[], true, true, None) {
        assert_eq!(r.n_miss, 0);
    }
}

#[test]
fn a2_mesh_born_vs_openmc() {
    let t = tally(vec![FilterKind::MeshBorn(MeshBornFilter { mesh: MeshKind::Regular(mesh_a2()) })], ScoreType::Flux);
    if let Some(r) = run_all("A2_mesh_born", &t, &[], true, false, None) {
        assert_eq!(r.n_miss, 0);
    }
}

#[test]
fn a2_mesh_material_split_vs_openmc() {
    let pairs: Vec<(usize, usize)> = (0..15).flat_map(|e| [(e, 0usize), (e, 1usize)]).collect();
    let t = tally(
        vec![FilterKind::MeshMaterial(MeshMaterialFilter { mesh: MeshKind::Regular(mesh_a2()), pairs: pairs.clone() })],
        ScoreType::Flux,
    );
    if let Some(r) = run_all("A2_mesh_material", &t, &[], true, false, None) {
        assert_eq!(r.n_miss, 0);
    }
    // And as shipped (the midpoint rule through the real runner), reported.
    let t2 = tally(
        vec![FilterKind::MeshMaterial(MeshMaterialFilter { mesh: MeshKind::Regular(mesh_a2()), pairs })],
        ScoreType::Flux,
    );
    if let Some(r) = run_all("A2_mesh_material", &t2, &[], false, false, None) {
        println!("[A2 mesh_material, as shipped through the runner] misses {}", r.n_miss);
    }
}

#[test]
fn cell_born_vs_openmc() {
    let t = tally(vec![FilterKind::CellBorn(CellBornFilter { cells: vec![1, 2] })], ScoreType::Flux);
    if let Some(r) = run_all("cell_born", &t, &[], true, false, None) {
        assert_eq!(r.n_miss, 0);
    }
}

fn surfaces_all() -> FilterKind {
    FilterKind::Surface(SurfaceFilter { surface_indices: vec![0, 1, 2, 3, 4, 5, 6] })
}

#[test]
fn cell_from_vs_openmc() {
    let t = tally(
        vec![surfaces_all(), FilterKind::CellFrom(CellFromFilter { cells: vec![1, 2] })],
        ScoreType::Current,
    );
    if let Some(r) = run_all("cell_from", &t, &[], true, true, None) {
        assert_eq!(r.n_miss, 0);
    }
}

#[test]
fn material_from_vs_openmc() {
    let t = tally(
        vec![surfaces_all(), FilterKind::MaterialFrom(MaterialFromFilter { materials: vec![0, 1] })],
        ScoreType::Current,
    );
    if let Some(r) = run_all("material_from", &t, &[], true, true, None) {
        assert_eq!(r.n_miss, 0);
    }
}

#[test]
fn mu_surface_vs_openmc() {
    let edges: Vec<f64> = (0..9).map(|i| -1.0 + 0.25 * i as f64).collect();
    let t = tally(
        vec![surfaces_all(), FilterKind::MuSurface(MuSurfaceFilter { bins: edges })],
        ScoreType::Current,
    );
    if let Some(r) = run_all("mu_surface", &t, &[], true, true, None) {
        assert_eq!(r.n_miss, 0);
    }
}

#[test]
fn mesh_surface_vs_openmc() {
    let f = MeshSurfaceFilter::new(MeshKind::Regular(mesh())).unwrap();
    let t = tally(vec![FilterKind::MeshSurface(f)], ScoreType::Current);
    if let Some(r) = run_all("mesh_surface", &t, &[], true, true, None) {
        assert_eq!(r.n_miss, 0);
    }
}

#[test]
fn mesh_born_vs_openmc() {
    let t = tally(vec![FilterKind::MeshBorn(MeshBornFilter { mesh: MeshKind::Regular(mesh()) })], ScoreType::Flux);
    if let Some(r) = run_all("mesh_born", &t, &[], true, false, None) {
        assert_eq!(r.n_miss, 0);
    }
}

// ── The same Experiment A bins THROUGH THE REAL TRANSPORT (gh:#261, 2026-11-18) ──
// After the maintainer-confirmed wiring of the event-history fields into
// `transport_history_vr_urr` and the MESH_MATERIAL splitting fix. Predictions:
// `prediction_before_measuring.md`, amendment 3. `driver = false` everywhere:
// `run_fixed_source_traced`, no `history_events` driver.

#[test]
fn real_cell_born_vs_openmc() {
    let t = tally(vec![FilterKind::CellBorn(CellBornFilter { cells: vec![1, 2] })], ScoreType::Flux);
    if let Some(r) = run_all("cell_born", &t, &[], false, false, None) {
        assert_eq!(r.n_miss, 0);
    }
}

#[test]
fn real_mesh_born_vs_openmc() {
    let t = tally(vec![FilterKind::MeshBorn(MeshBornFilter { mesh: MeshKind::Regular(mesh()) })], ScoreType::Flux);
    if let Some(r) = run_all("mesh_born", &t, &[], false, false, None) {
        assert_eq!(r.n_miss, 0);
    }
}

#[test]
fn real_a2_mesh_born_vs_openmc() {
    let t = tally(vec![FilterKind::MeshBorn(MeshBornFilter { mesh: MeshKind::Regular(mesh_a2()) })], ScoreType::Flux);
    if let Some(r) = run_all("A2_mesh_born", &t, &[], false, false, None) {
        assert_eq!(r.n_miss, 0);
    }
}

#[test]
fn real_a2_mesh_material_vs_openmc() {
    let pairs: Vec<(usize, usize)> = (0..15).flat_map(|e| [(e, 0usize), (e, 1usize)]).collect();
    let t = tally(
        vec![FilterKind::MeshMaterial(MeshMaterialFilter { mesh: MeshKind::Regular(mesh_a2()), pairs })],
        ScoreType::Flux,
    );
    if let Some(r) = run_all("A2_mesh_material", &t, &[], false, false, None) {
        assert_eq!(r.n_miss, 0);
    }
}

#[test]
fn real_cell_from_vs_openmc() {
    let t = tally(
        vec![surfaces_all(), FilterKind::CellFrom(CellFromFilter { cells: vec![1, 2] })],
        ScoreType::Current,
    );
    if let Some(r) = run_all("cell_from", &t, &[], false, true, None) {
        assert_eq!(r.n_miss, 0);
    }
}

#[test]
fn real_material_from_vs_openmc() {
    let t = tally(
        vec![surfaces_all(), FilterKind::MaterialFrom(MaterialFromFilter { materials: vec![0, 1] })],
        ScoreType::Current,
    );
    if let Some(r) = run_all("material_from", &t, &[], false, true, None) {
        assert_eq!(r.n_miss, 0);
    }
}

#[test]
fn real_mu_surface_vs_openmc() {
    let edges: Vec<f64> = (0..9).map(|i| -1.0 + 0.25 * i as f64).collect();
    let t = tally(
        vec![surfaces_all(), FilterKind::MuSurface(MuSurfaceFilter { bins: edges })],
        ScoreType::Current,
    );
    if let Some(r) = run_all("mu_surface", &t, &[], false, true, None) {
        assert_eq!(r.n_miss, 0);
    }
}

/// Registered 6x2x1 mesh. ~~The 16 tie misses of the driver version are
/// predicted to recur here (amendment 3); the test pins the COUNT, not zero.~~
/// **CHANGED 2026-11-22 (gh:#1709):** with upstream's `raytrace_mesh` ported
/// and the raytrace spanning `r_last_current` to collision / leak / reflection
/// as upstream does (not one per geometry segment), 0 misses, worst 0: pinned
/// at zero, moved from 16.
#[test]
fn real_mesh_surface_vs_openmc() {
    let f = MeshSurfaceFilter::new(MeshKind::Regular(mesh())).unwrap();
    let t = tally(vec![FilterKind::MeshSurface(f)], ScoreType::Current);
    if let Some(r) = run_all("mesh_surface", &t, &[], false, true, None) {
        assert_eq!(r.n_miss, 0, "mesh-surface misses (gh:#1709; record: README)");
    }
}

/// A2 mesh. ~~The 1 box-edge tie recurs (amendment 2 and 3).~~ **CHANGED
/// 2026-11-22 (gh:#1709):** 0 misses with the `raytrace_mesh` port; pinned at
/// zero, moved from 1.
#[test]
fn real_a2_mesh_surface_vs_openmc() {
    let f = MeshSurfaceFilter::new(MeshKind::Regular(mesh_a2())).unwrap();
    let t = tally(vec![FilterKind::MeshSurface(f)], ScoreType::Current);
    if let Some(r) = run_all("A2_mesh_surface", &t, &[], false, true, None) {
        assert_eq!(r.n_miss, 0, "mesh-surface misses (gh:#1709; record: README)");
    }
}

/// **The geometry, drawn from the assembled model** (AGENTS.md drawing rule),
/// with the ported plotter (`geometry::plot`), PNGs committed beside the record:
/// the xy slice at z = 0 through the three sphere centres, the xz slice at
/// y = 0.5 (off the mesh plane y = 0), and the xy slice with the 6x2x1 tally
/// mesh overlaid. What to look for is written in the V&V README.
#[test]
fn draw_the_experiment_a_geometry() {
    use outram_mc_libs::geometry::plot::{render_material_slice, MeshLines, PlotBasis, Rgb, SlicePlot};
    let Some(m) = model() else { return };
    let out = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("verification_and_validation/tally_filters_vs_openmc/geometry");
    std::fs::create_dir_all(&out).unwrap();
    let pal = [
        (Rgb::new(214, 96, 77), "M1 dilute O-16 (cell A, sphere r=2)"),
        (Rgb::new(67, 147, 195), "M2 dilute O-16 (cell B)"),
    ];
    let mk = |basis, origin: Position, meshlines| SlicePlot {
        origin,
        basis,
        width: [20.0, 6.667],
        pixels: [1200, 400],
        level: None,
        show_overlaps: true,
        meshlines,
    };
    let cases = [
        ("xy_z0.png", mk(PlotBasis::Xy, Position::new(0.0, 0.0, 0.0), None), "xy at z=0"),
        ("xz_y05.png", mk(PlotBasis::Xz, Position::new(0.0, 0.5, 0.0), None), "xz at y=0.5"),
        (
            "xy_z0_mesh.png",
            mk(
                PlotBasis::Xy,
                Position::new(0.0, 0.0, 0.0),
                Some(MeshLines { mesh: mesh(), width: 1, colour: Rgb::new(0, 0, 0) }),
            ),
            "xy at z=0 with the 6x2x1 tally mesh",
        ),
        (
            "xy_z0_mesh_a2.png",
            mk(
                PlotBasis::Xy,
                Position::new(0.0, 0.0, 0.0),
                Some(MeshLines { mesh: mesh_a2(), width: 1, colour: Rgb::new(0, 0, 0) }),
            ),
            "xy at z=0 with the 5x3x1 tally mesh (A2)",
        ),
    ];
    for (name, plot, what) in cases {
        let (_raw, img) = render_material_slice(&m.geom, &plot, &pal, &format!("gh:#261 Experiment A: {what}"));
        img.write_png(out.join(name)).expect("png");
        assert!(out.join(name).exists());
    }
}

// ── Experiment B: statistical, CE O-16 gas sphere ────────────────────────────
//
// Sphere r = 10 cm, vacuum, O-16 0.03 atoms/b-cm, isotropic 1 MeV point source,
// analog; 20 batches x 50 000 histories per code. Pass: every |z| <= 2,
// z = (a-b)/sqrt(sa^2+sb^2), sa/sb the batch standard errors. Prediction:
// `prediction_before_measuring.md`, "Experiment B".

fn load_stat(name: &str) -> Vec<(f64, f64)> {
    let txt = std::fs::read_to_string(ref_dir().join(format!("expB_{name}.txt"))).expect("expB reference");
    txt.lines()
        .map(|l| {
            let v: Vec<f64> = l.split_whitespace().map(|x| x.parse().unwrap()).collect();
            (v[1], v[2])
        })
        .collect()
}

fn model_b() -> Option<(Geometry, Vec<Material>)> {
    model()?;
    let surfaces = vec![SurfaceKind::Sphere(Sphere { x0: 0.0, y0: 0.0, z0: 0.0, r: 10.0, bc: BoundaryType::Vacuum })];
    let geom = Geometry {
        surfaces,
        cells: vec![Cell::material(
            1,
            vec![RegionToken::HalfSpace { surface_idx: 0, sense: HalfSpaceSense::Inside }],
            0,
            None::<f64>,
        )],
        universes: vec![Universe { id: 0, cell_indices: vec![0] }],
        lattices: vec![],
        root_universe: 0,
    };
    let material = Material {
        id: 1,
        name: "O16 gas".into(),
        components: vec![NuclideComponent { nuclide_idx: 0, atom_density: 0.03 }],
        temperature: TEMP_K,
    };
    Some((geom, vec![material]))
}

/// 20 batches of `per` histories; per-batch tally sums divided by `per`.
fn run_b(geom: &Geometry, materials: &[Material], nuclides: &[Nuclide], tallies: &[Tally], per: usize, seed0: u64) -> Vec<Vec<Vec<f64>>> {
    use outram_mc_libs::rng::lcg::prn;
    let mut seed: u64 = seed0;
    let mut batches: Vec<Vec<Vec<f64>>> = Vec::new();
    for _ in 0..20 {
        let mut acc: Vec<Vec<f64>> = tallies.iter().map(|t| vec![0.0; t.bins.len()]).collect();
        for _ in 0..per {
            let mu = 2.0 * prn(&mut seed) - 1.0;
            let phi = 2.0 * std::f64::consts::PI * prn(&mut seed);
            let s = (1.0 - mu * mu).sqrt();
            let mut trace: Vec<TraceEvent> = Vec::new();
            trace_csg_history(
                Position::ZERO,
                Direction { u: s * phi.cos(), v: s * phi.sin(), w: mu },
                1.0e6,
                geom,
                materials,
                nuclides,
                &mut seed,
                |e| trace.push(e),
            );
            let events = history_events(geom, &trace, &[]);
            for (t, a) in tallies.iter().zip(acc.iter_mut()) {
                score_history(t, &events, a).expect("supported");
            }
        }
        for a in acc.iter_mut() {
            for v in a.iter_mut() {
                *v /= per as f64;
            }
        }
        batches.push(acc);
    }
    batches
}

fn z_report(name: &str, batches: &[Vec<Vec<f64>>], ti: usize, theirs: &[(f64, f64)]) -> (usize, f64) {
    let n = batches.len() as f64;
    let mut worst = 0.0f64;
    for (b, &(rm, rs)) in theirs.iter().enumerate() {
        let xs: Vec<f64> = batches.iter().map(|bt| bt[ti][b]).collect();
        let mean = xs.iter().sum::<f64>() / n;
        let var = xs.iter().map(|x| (x - mean) * (x - mean)).sum::<f64>() / (n - 1.0);
        let se = (var / n).sqrt();
        let z = (mean - rm) / (se * se + rs * rs).sqrt();
        println!("[B {name}] bin {b}: ours {mean:.6} +- {se:.6}  openmc {rm:.6} +- {rs:.6}  z {z:+.2}");
        worst = worst.max(z.abs());
    }
    (theirs.len(), worst)
}

/// First run of Experiment B, gate |z| <= 2 on every bin.
///
/// ~~A miss leaves the test red.~~ **CHANGED 2026-11-22 (gh:#1709, maintainer
/// decision: "Follow registered plan"):** one run gated bin by bin on 10 bins
/// fails a correct code about 37 % of the time (1 - 0.9545^10). The
/// registered protocol reports a first-run miss and decides it with the 8-seed
/// pooled replication at the same |z| <= 2, so the test now does exactly that.
/// The threshold is unchanged. Recorded: first run 2.87 (collision bin 4),
/// pooled 1.54 (collision) and 1.19 (legendre); the README has the numbers.
#[test]
fn b_collision_and_legendre_vs_openmc() {
    let Some((geom, mats)) = model_b() else { return };
    let nuc = &model().unwrap().nuclides;
    let tc = tally(vec![FilterKind::Collision(CollisionFilter { bins: vec![0, 1, 2, 3, 4, 5] })], ScoreType::Flux);
    let tl = tally(vec![FilterKind::Legendre(LegendreFilter { order: 3 })], ScoreType::Scatter);
    let t0 = std::time::Instant::now();
    let batches = run_b(&geom, &mats, nuc, &[tc, tl], 50_000, 261);
    println!("[B] 1e6 histories in {:.1} s (4 logical cores, single thread, shared host)", t0.elapsed().as_secs_f64());
    let (n1, w1) = z_report("collision", &batches, 0, &load_stat("collision"));
    let (n2, w2) = z_report("legendre", &batches, 1, &load_stat("legendre"));
    println!("[B] collision: {n1} bins worst |z| {w1:.2}; legendre: {n2} bins worst |z| {w2:.2}");
    if w1 <= 2.0 && w2 <= 2.0 {
        return;
    }
    // The registered protocol (`prediction_before_measuring.md`, amendment 2,
    // fixed before the replication was run): a first-run miss is reported and
    // decided by the 8-seed pooled replication at the same |z| <= 2 (gh:#1709).
    let pooled = b_pooled_worst(&geom, &mats, nuc);
    println!("[B] first run missed (collision {w1:.2}, legendre {w2:.2}); pooled replication {pooled:?}");
    assert!(
        pooled[0] <= 2.0 && pooled[1] <= 2.0,
        "first run worst |z| collision {w1} legendre {w2}, and the registered pooled replication {pooled:?}"
    );
}

fn load_stat_file(stem: &str) -> Vec<f64> {
    let txt = std::fs::read_to_string(ref_dir().join(format!("{stem}.txt"))).expect("replica reference");
    txt.lines().map(|l| l.split_whitespace().nth(1).unwrap().parse().unwrap()).collect()
}

/// Replication of Experiment B (plan fixed in `prediction_before_measuring.md`
/// before it was run): 8 further independent runs of each code, seeds
/// 1001..1008, pooled per bin from the scatter of the 8 run means.
#[test]
fn b_replicates_pooled_vs_openmc() {
    let Some((geom, mats)) = model_b() else { return };
    let nuc = &model().unwrap().nuclides;
    let worst = b_pooled_worst(&geom, &mats, nuc);
    assert!(worst[0] <= 2.0 && worst[1] <= 2.0, "pooled worst |z| {worst:?}");
}

/// The pooled worst |z| of the Experiment B replication (collision, legendre).
fn b_pooled_worst(geom: &Geometry, mats: &[Material], nuc: &[Nuclide]) -> [f64; 2] {
    let tc = tally(vec![FilterKind::Collision(CollisionFilter { bins: vec![0, 1, 2, 3, 4, 5] })], ScoreType::Flux);
    let tl = tally(vec![FilterKind::Legendre(LegendreFilter { order: 3 })], ScoreType::Scatter);
    let names = ["collision", "legendre"];
    let nb = [6usize, 4usize];
    let mut ours: Vec<Vec<Vec<f64>>> = vec![vec![Vec::new(); 6], vec![Vec::new(); 4]];
    let mut theirs: Vec<Vec<Vec<f64>>> = vec![vec![Vec::new(); 6], vec![Vec::new(); 4]];
    for k in 1..=8u64 {
        let seed = 1000 + k;
        let batches = run_b(geom, mats, nuc, &[tc.clone(), tl.clone()], 50_000, seed);
        for ti in 0..2 {
            let om = load_stat_file(&format!("expB_{}_s{seed}", names[ti]));
            for b in 0..nb[ti] {
                let m = batches.iter().map(|bt| bt[ti][b]).sum::<f64>() / batches.len() as f64;
                ours[ti][b].push(m);
                theirs[ti][b].push(om[b]);
            }
        }
    }
    let stats = |v: &[f64]| {
        let n = v.len() as f64;
        let m = v.iter().sum::<f64>() / n;
        let var = v.iter().map(|x| (x - m) * (x - m)).sum::<f64>() / (n - 1.0);
        (m, (var / n).sqrt())
    };
    let mut worst = [0.0f64; 2];
    for ti in 0..2 {
        for b in 0..nb[ti] {
            let (a, sa) = stats(&ours[ti][b]);
            let (o, so) = stats(&theirs[ti][b]);
            let z = (a - o) / (sa * sa + so * so).sqrt();
            println!("[B pooled {}] bin {b}: ours {a:.6} +- {sa:.6}  openmc {o:.6} +- {so:.6}  z {z:+.2}", names[ti]);
            worst[ti] = worst[ti].max(z.abs());
        }
    }
    println!("[B pooled] collision worst |z| {:.2}, legendre worst |z| {:.2}", worst[0], worst[1]);
    worst
}

// ── Experiments B and C THROUGH THE REAL TRANSPORT (gh:#261, 2026-11-18) ──────
// Predictions: `prediction_before_measuring.md`, amendments 3 and 4.

/// Run the fixed-source runner for 20 batches of `per` histories from one
/// isotropic point source at the origin, and return per bin `(mean, standard
/// error)` of the batch values per source particle.
#[allow(clippy::too_many_arguments)]
fn real_stats(
    geom: &Geometry,
    materials: &[Material],
    nuclides: &[Nuclide],
    proto: &Tally,
    vr: outram_mc_libs::physics::variance_reduction::VarianceReduction,
    e_ev: f64,
    per: usize,
    seed: u64,
) -> Vec<(f64, f64)> {
    use outram_mc_libs::source::angle::IsotropicAngle;
    let mut t = proto.clone();
    let src = FixedSource::Independent(Arc::new(IndependentSource {
        spatial: SpatialKind::Point(PointSource { r: Position::ZERO }),
        energy: EnergyKind::Mono(Monoenergetic { e: e_ev }),
        angle: AngleKind::Isotropic(IsotropicAngle),
        strength: 1.0,
    }));
    run_fixed_source_traced(
        geom,
        materials,
        nuclides,
        &src,
        &FixedSourceSettings { n_particles: 20 * per, n_batches: 20, seed, variance_reduction: vr, ..Default::default() },
        Some(&mut t),
        None,
        None,
        None,
    );
    t.bins
        .iter()
        .map(|b| {
            let n = b.count as f64;
            let mean = b.sum / n;
            let var = ((b.sum_sq - n * mean * mean) / (n - 1.0)).max(0.0);
            // gh:#675: batches are already per source particle (the default), so no
            // second division by `per`.
            (mean, (var / n).sqrt())
        })
        .collect()
}

fn z_pairs(name: &str, ours: &[(f64, f64)], theirs: &[(f64, f64)]) -> f64 {
    let mut worst = 0.0f64;
    for (b, (&(a, sa), &(o, so))) in ours.iter().zip(theirs).enumerate() {
        let d = (sa * sa + so * so).sqrt();
        let z = if d > 0.0 { (a - o) / d } else if a == o { 0.0 } else { f64::INFINITY };
        println!("[real {name}] bin {b}: ours {a:.6} +- {sa:.6}  openmc {o:.6} +- {so:.6}  z {z:+.2}");
        worst = worst.max(z.abs());
    }
    worst
}

/// First run of Experiment B through the real runner, gate |z| <= 2 on every
/// bin. **CHANGED 2026-11-22 (gh:#1709, maintainer decision):** a miss falls
/// back to the replication registered in amendment 5 (8 seed pairs pooled,
/// same |z| <= 2), as for [`b_collision_and_legendre_vs_openmc`]; 6 bins fail
/// a correct code about 24 % of the time in one run. Recorded: first run
/// 2.56, pooled 1.39.
#[test]
fn real_b_collision_vs_openmc() {
    let Some((geom, mats)) = model_b() else { return };
    let nuc = &model().unwrap().nuclides;
    let tc = tally(vec![FilterKind::Collision(CollisionFilter { bins: vec![0, 1, 2, 3, 4, 5] })], ScoreType::Flux);
    let t0 = std::time::Instant::now();
    let ours = real_stats(&geom, &mats, nuc, &tc, Default::default(), 1.0e6, 50_000, 261);
    println!("[real B] 1e6 histories in {:.1} s (4 logical cores, single thread, shared host)", t0.elapsed().as_secs_f64());
    let w = z_pairs("collision", &ours, &load_stat("collision"));
    println!("[real B] collision worst |z| {w:.2}");
    if w <= 2.0 {
        return;
    }
    // The registered protocol (amendment 5, fixed before the replication was
    // run): a first-run miss is decided by the 8-seed pooled replication at the
    // same |z| <= 2 (gh:#1709).
    let pooled = real_b_pooled_worst(&geom, &mats, nuc);
    println!("[real B] first run missed ({w:.2}); pooled replication {pooled:.2}");
    assert!(pooled <= 2.0, "first run worst |z| {w}, and the registered pooled replication {pooled}");
}

/// Replication plan of amendment 5 (fixed before it was run): 8 seed pairs,
/// 1001..1008, pooled per bin from the scatter of the 8 run means.
#[test]
fn real_b_replicates_pooled_vs_openmc() {
    let Some((geom, mats)) = model_b() else { return };
    let nuc = &model().unwrap().nuclides;
    let worst = real_b_pooled_worst(&geom, &mats, nuc);
    assert!(worst <= 2.0, "pooled worst |z| {worst}");
}

/// The pooled worst |z| of the real-transport Experiment B replication.
fn real_b_pooled_worst(geom: &Geometry, mats: &[Material], nuc: &[Nuclide]) -> f64 {
    let tc = tally(vec![FilterKind::Collision(CollisionFilter { bins: vec![0, 1, 2, 3, 4, 5] })], ScoreType::Flux);
    let mut ours: Vec<Vec<f64>> = vec![Vec::new(); 6];
    let mut theirs: Vec<Vec<f64>> = vec![Vec::new(); 6];
    for k in 1..=8u64 {
        let seed = 1000 + k;
        let run = real_stats(geom, mats, nuc, &tc, Default::default(), 1.0e6, 50_000, seed);
        let om = load_stat_file(&format!("expB_collision_s{seed}"));
        for b in 0..6 {
            ours[b].push(run[b].0);
            theirs[b].push(om[b]);
        }
    }
    let stats = |v: &[f64]| {
        let n = v.len() as f64;
        let m = v.iter().sum::<f64>() / n;
        let var = v.iter().map(|x| (x - m) * (x - m)).sum::<f64>() / (n - 1.0);
        (m, (var / n).sqrt())
    };
    let mut worst = 0.0f64;
    for b in 0..6 {
        let (a, sa) = stats(&ours[b]);
        let (o, so) = stats(&theirs[b]);
        let z = (a - o) / (sa * sa + so * so).sqrt();
        println!("[real B pooled collision] bin {b}: ours {a:.6} +- {sa:.6}  openmc {o:.6} +- {so:.6}  z {z:+.2}");
        worst = worst.max(z.abs());
    }
    println!("[real B pooled] collision worst |z| {worst:.2}");
    worst
}

fn model_c() -> Option<(Geometry, Vec<Material>, Vec<Nuclide>)> {
    let (geom, _) = model_b()?;
    let path = njoy_outram_park_fork::reference_data::reference_endf("n-003_Li_006-ENDF8.0.endf")?;
    let li6 = Nuclide::from_endf_file(
        &path,
        "Li6",
        uom::si::f64::ThermodynamicTemperature::new::<uom::si::thermodynamic_temperature::kelvin>(TEMP_K),
        1.0e-3,
    )
    .ok()?;
    let material = Material {
        id: 1,
        name: "Li6 gas".into(),
        components: vec![NuclideComponent { nuclide_idx: 0, atom_density: 0.05 }],
        temperature: TEMP_K,
    };
    Some((geom, vec![material], vec![li6]))
}

fn survival_biasing() -> outram_mc_libs::physics::variance_reduction::VarianceReduction {
    outram_mc_libs::physics::variance_reduction::VarianceReduction {
        survival_biasing: true,
        weight_cutoff: 0.25,
        weight_survive: 1.0,
        ..Default::default()
    }
}

fn weight_tally() -> Tally {
    let f = WeightFilter { bins: vec![0.0, 0.1, 0.25, 0.5, 0.75, 1.0] };
    tally(vec![FilterKind::Weight(f)], ScoreType::Flux)
}

fn load_stat_c(name: &str) -> Vec<(f64, f64)> {
    let txt = std::fs::read_to_string(ref_dir().join(format!("expC_{name}.txt"))).expect("expC reference");
    txt.lines()
        .map(|l| {
            let v: Vec<f64> = l.split_whitespace().map(|x| x.parse().unwrap()).collect();
            (v[1], v[2])
        })
        .collect()
}

/// Experiment C: WeightFilter under survival biasing (Li-6, 300 keV).
#[test]
fn real_c_weight_vs_openmc() {
    let Some((geom, mats, nuc)) = model_c() else { return };
    let ours = real_stats(&geom, &mats, &nuc, &weight_tally(), survival_biasing(), 3.0e5, 50_000, 261);
    let w = z_pairs("weight", &ours, &load_stat_c("weight"));
    println!("[real C] weight worst |z| {w:.2}");
    assert!(w <= 2.0, "weight filter through real transport: worst |z| {w}");
}

/// Wiring the history fields must not perturb the transport: a tally binned by
/// `CellBorn` (one cell, so every event lands in bin 0) holds the same total as
/// an unbinned flux tally from the same seed, bit for bit, and a collision
/// filter covering 0..100 collisions sums to it.
#[test]
fn history_tallies_do_not_perturb_the_run() {
    let Some((geom, mats)) = model_b() else { return };
    let nuc = &model().unwrap().nuclides;
    let plain = tally(vec![], ScoreType::Flux);
    let born = tally(vec![FilterKind::CellBorn(CellBornFilter { cells: vec![0] })], ScoreType::Flux);
    let a = real_stats(&geom, &mats, nuc, &plain, Default::default(), 1.0e6, 500, 7);
    let b = real_stats(&geom, &mats, nuc, &born, Default::default(), 1.0e6, 500, 7);
    assert_eq!(a[0].0.to_bits(), b[0].0.to_bits());
    assert_eq!(a[0].1.to_bits(), b[0].1.to_bits());
    let bins: Vec<u32> = (0..100).collect();
    let coll = tally(vec![FilterKind::Collision(CollisionFilter { bins })], ScoreType::Flux);
    let c = real_stats(&geom, &mats, nuc, &coll, Default::default(), 1.0e6, 500, 7);
    let total: f64 = c.iter().map(|x| x.0).sum();
    assert!((total - a[0].0).abs() <= 1.0e-12 * a[0].0, "collision bins sum {total} vs {}", a[0].0);
}
