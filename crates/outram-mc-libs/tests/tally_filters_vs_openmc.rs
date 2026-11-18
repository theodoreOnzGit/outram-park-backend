// SPDX-License-Identifier: GPL-3.0

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
//! Wired filters (`DistribcellFilter`, `CellInstanceFilter`, `EnergyFunction`,
//! `MeshMaterial`) are scored through the real fixed-source runner
//! (`run_fixed_source_traced`). The other filters read event-history fields that
//! the transport does not set (see `tally::history_events`), so they are scored
//! through that driver, fed by the reviewed transport's `TraceEvent` stream.
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
    Tally { id: 1, name: "t".into(), filters, scores: vec![score], bins: vec![TallyBin::default(); n] }
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
    ];
    for (name, plot, what) in cases {
        let (_raw, img) = render_material_slice(&m.geom, &plot, &pal, &format!("gh:#261 Experiment A: {what}"));
        img.write_png(out.join(name)).expect("png");
        assert!(out.join(name).exists());
    }
}
