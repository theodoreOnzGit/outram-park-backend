//! **Tutorial 8's model is the CSG it claims to be** (gh #943): the Şeker
//! bed of HTR-10, stacked in a hopper with whole-ball rejection, built
//! through the beginner API (`examples/tutorial_seker_bed.rs`, statements in
//! `examples/common/seker_bed_model.rs`) lowers to the hexagonal-lattice CSG
//! a hand-written model has.
//!
//! # Methodology
//!
//! The hand-built side is written here from **nee_soon's formulas**
//! (`htr10_rmc::bed`: `SekerCell::site_centre` and the 23 sites,
//! `seker_tile_ball`, `tile_xy`, `SekerBed::new`, `containment`, `reject`,
//! `assign`; `core_shell`: the cone's apex and slope, the three-part bed
//! region) with the low-level types only (spheres, planes, a cone,
//! `HexLattice::from_rings_3d`), independent of `model::bed::BedLayout` and
//! `Hopper::contains_ball`. nee_soon cannot be called from this crate (it
//! depends on it), so its own numbers are pinned as constants, printed by
//! `SekerBed::new(.., 14 rings, N, 36.946, 90, 25, Some(221.236), Paper)` on
//! 2026-10-18.
//!
//! 1. **Counts against nee_soon** at N = 3, 10, 12, 16, 20 on the real
//!    HTR-10 cavity: balls above the floor (Şeker Table 3's count), fuel
//!    balls, balls rejected, balls kept anywhere. Assembled from the ENDF tape
//!    headers, smeared balls (the count does not depend on what is in a ball).
//! 2. **The hand-built layout agrees** with the beginner census on the same
//!    numbers, ball for ball (every kept centre and every rejected centre).
//! 3. **Lookups.** A small bed (hopper radius 30 cm) of smeared balls on
//!    LOW-tier materials: 300 000 random points give the same material in
//!    both geometries.
//! 4. **Transport.** The CSG power iteration on the hand-built geometry and
//!    the beginner run at the same seed, source box and data: `k` in every
//!    generation to the last bit, on both backends.
//! 5. **Refusals and the small pieces** (the hopper, the surface sharing, the
//!    bad inputs).
//!
//! # Results (2026-10-18)
//!
//! All pass; counts and `k` values are printed by the tests (not physics
//! results: LOW tier, tiny runs).

use petir::mathf::RealMath;
#[path = "../examples/common/seker_bed_model.rs"]
#[allow(dead_code)]
mod seker_bed_model;

use std::collections::{HashMap, HashSet};

use outram_mc_libs::geometry::cell::{Cell, CellFill, HalfSpaceSense, RegionToken, SurfaceToken};
use outram_mc_libs::geometry::geometry::Geometry;
use outram_mc_libs::geometry::lattice::{HexLattice, HexOrientation, Lattice as CsgLattice};
use outram_mc_libs::geometry::position::{Direction, Position};
use outram_mc_libs::geometry::surface::{
    BoundaryType, Sphere as CsgSphere, SurfaceKind, XPlane, ZCone, ZCylinder, ZPlane,
};
use outram_mc_libs::geometry::universe::Universe;
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::model::prelude::*;
use outram_mc_libs::model::{Arrangement, RowKind, Wall};
use outram_mc_libs::physics::keff::KeffSettings as LowKeffSettings;
use outram_mc_libs::physics::keff::{ComputeType, ThreadCount};
use outram_mc_libs::physics::transport_csg::run_keff_csg;
use seker_bed_model as bed;
use bed::layer as seker;
use uom::si::f64::{Length, Ratio};
use uom::si::length::centimeter;
use uom::si::ratio::ratio;

const T: f64 = 300.15;

fn cm(x: f64) -> Length {
    Length::new::<centimeter>(x)
}

// ── nee_soon's formulas, written out again ─────────────────────────────────

/// One of nee_soon's 23 ball sites (`SekerSite::ALL`).
#[derive(Clone, Copy, Debug)]
enum Site {
    Bottom(u8),
    Top(u8),
    Inner(u8),
    Outer(u8),
    Entering(u8),
}

/// `SekerCell::site_centre`, for 6 cm balls (`bed.rs`).
fn site_centre(site: Site) -> [f64; 3] {
    let d = 6.0_f64;
    let apothem = d * (std::f64::consts::PI / 6.0).r_cos() + 0.5 * d;
    let pitch = 2.0 * apothem;
    let h = 2.0 * (d * (2.0_f64 / 3.0).sqrt());
    let polar = |rho: f64, deg: f64, z: f64| {
        let t = deg.to_radians();
        [rho * t.r_cos(), rho * t.r_sin(), z]
    };
    let flower = |k: u8, z: f64| {
        if k == 0 {
            [0.0, 0.0, z]
        } else {
            polar(d, 60.0 * f64::from(k - 1), z)
        }
    };
    let s3 = 3.0_f64.sqrt();
    match site {
        Site::Bottom(k) => flower(k, -0.5 * h),
        Site::Top(k) => flower(k, 0.5 * h),
        Site::Inner(k) => polar(d / s3, 90.0 + 120.0 * f64::from(k), 0.0),
        Site::Outer(k) => polar(2.0 * d / s3, 270.0 + 120.0 * f64::from(k), 0.0),
        Site::Entering(k) => polar(pitch - 2.0 * d / s3, 90.0 + 120.0 * f64::from(k), 0.0),
    }
}

/// The 23 sites in `SekerSite::ALL` order.
fn sites() -> Vec<Site> {
    let mut v: Vec<Site> = (0..7).map(Site::Bottom).collect();
    v.extend((0..7).map(Site::Top));
    v.extend((0..3).map(Site::Inner));
    v.extend((0..3).map(Site::Outer));
    v.extend((0..3).map(Site::Entering));
    v
}

/// A ball named by its owner (`SekerBallId`); `Flower < Central` as in
/// nee_soon's derived order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum Ball {
    Flower { a: i32, b: i32, face: i32, k: u8 },
    Central { a: i32, b: i32, level: i32, k: u8 },
}

/// `seker_tile_ball`.
fn tile_ball(a: i32, b: i32, level: i32, site: Site) -> Ball {
    match site {
        Site::Bottom(k) => Ball::Flower {
            a,
            b,
            face: level,
            k,
        },
        Site::Top(k) => Ball::Flower {
            a,
            b,
            face: level + 1,
            k,
        },
        Site::Inner(k) => Ball::Central { a, b, level, k },
        Site::Outer(k) => Ball::Central {
            a,
            b,
            level,
            k: 3 + k,
        },
        Site::Entering(k) => {
            let (da, db) = [(0, 1), (-1, 0), (1, -1)][usize::from(k)];
            Ball::Central {
                a: a + da,
                b: b + db,
                level,
                k: 3 + k,
            }
        }
    }
}

fn tile_xy(a: i32, b: i32, pitch: f64) -> [f64; 2] {
    let (a, b) = (f64::from(a), f64::from(b));
    [0.5 * 3.0_f64.sqrt() * a * pitch, (b + 0.5 * a) * pitch]
}

fn hex_ring(a: i32, b: i32) -> usize {
    ((a.abs() + b.abs() + (a + b).abs()) / 2) as usize
}

fn segment_distance(px: f64, pz: f64, a: [f64; 2], b: [f64; 2]) -> f64 {
    let (dx, dz) = (b[0] - a[0], b[1] - a[1]);
    let l2 = dx * dx + dz * dz;
    let t = if l2 > 0.0 {
        (((px - a[0]) * dx + (pz - a[1]) * dz) / l2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    (px - a[0] - t * dx).r_hypot(pz - a[1] - t * dz)
}

/// The cavity the hand-built bed fills.
#[derive(Clone, Copy)]
struct Cavity {
    radius: f64,
    height: f64,
    cone: f64,
    outlet: f64,
    tube: f64,
}

impl Cavity {
    fn htr10() -> Self {
        Self {
            radius: bed::CORE_RADIUS_CM,
            height: bed::CAVITY_HEIGHT_CM,
            cone: bed::CONE_HEIGHT_CM,
            outlet: bed::TUBE_RADIUS_CM,
            tube: bed::TUBE_DEPTH_CM,
        }
    }
    fn small() -> Self {
        Self {
            radius: 30.0,
            height: 40.0,
            cone: 10.0,
            outlet: 14.0,
            tube: 30.0,
        }
    }
}

/// nee_soon's bed (`SekerBed`), as a plain struct of maps.
struct Hand {
    cavity: Cavity,
    top: f64,
    pitch: f64,
    h: f64,
    z_bottom: f64,
    n_rings: i32,
    n_levels: i32,
    present: HashMap<Ball, bool>,
    fuel: HashSet<Ball>,
    kept_in_bed: usize,
    kept_all: usize,
    rejected: usize,
    rejected_centres: Vec<[f64; 3]>,
    kept_centres: Vec<([f64; 3], bool)>,
}

impl Hand {
    fn centre(&self, id: Ball) -> [f64; 3] {
        let (a, b, zc, site) = match id {
            Ball::Flower { a, b, face, k } => (
                a,
                b,
                self.z_bottom + f64::from(face) * self.h,
                Site::Bottom(k),
            ),
            Ball::Central { a, b, level, k } => (
                a,
                b,
                self.z_bottom + (f64::from(level) + 0.5) * self.h,
                if k < 3 {
                    Site::Inner(k)
                } else {
                    Site::Outer(k - 3)
                },
            ),
        };
        let [tx, ty] = tile_xy(a, b, self.pitch);
        let [lx, ly, _] = site_centre(site);
        [tx + lx, ty + ly, zc]
    }

    /// `SekerBed::containment`: whole, and centre inside.
    fn containment(&self, c: [f64; 3]) -> (bool, bool) {
        let cv = self.cavity;
        let (rho, z) = (c[0].r_hypot(c[1]), c[2]);
        let cone_bottom = -cv.cone;
        let tube_bottom = cone_bottom - cv.tube;
        let p = [
            [0.0, self.top],
            [cv.radius, self.top],
            [cv.radius, 0.0],
            [cv.outlet, cone_bottom],
            [cv.outlet, tube_bottom],
            [0.0, tube_bottom],
        ];
        let mut inside = false;
        let mut dmin = f64::INFINITY;
        for i in 0..p.len() {
            let (u, v) = (p[i], p[(i + 1) % p.len()]);
            if (u[1] > z) != (v[1] > z) && rho < u[0] + (z - u[1]) / (v[1] - u[1]) * (v[0] - u[0]) {
                inside = !inside;
            }
            if i + 1 < p.len() {
                dmin = dmin.min(segment_distance(rho, z, u, v));
            }
        }
        (inside && dmin >= 3.0 - 1e-9, inside)
    }

    fn new(cavity: Cavity, top: f64, fraction: f64) -> Self {
        let h = 2.0 * (6.0 * (2.0_f64 / 3.0).sqrt());
        let apothem = 6.0 * (std::f64::consts::PI / 6.0).r_cos() + 3.0;
        let pitch = 2.0 * apothem;
        let face0 = 0.0 + 3.0;
        let bottom = -cavity.cone - cavity.tube;
        let m_lo = ((bottom - face0) / h).floor() as i32 - 1;
        let m_hi = (((cavity.height) - face0) / h).floor() as i32 + 2;
        let n_rings = (cavity.radius / ((3.0_f64.sqrt() / 2.0) * pitch)).ceil() as i32 + 1;
        let mut hand = Self {
            cavity,
            top,
            pitch,
            h,
            z_bottom: face0 + f64::from(m_lo) * h,
            n_rings,
            n_levels: m_hi - m_lo,
            present: HashMap::new(),
            fuel: HashSet::new(),
            kept_in_bed: 0,
            kept_all: 0,
            rejected: 0,
            rejected_centres: Vec::new(),
            kept_centres: Vec::new(),
        };
        // all_balls: owned balls of the tiles within the rings.
        let nr = hand.n_rings;
        let mut cols = Vec::new();
        for a in -(nr - 1)..=(nr - 1) {
            for b in -(nr - 1)..=(nr - 1) {
                if hex_ring(a, b) <= (nr - 1) as usize {
                    cols.push((a, b));
                }
            }
        }
        let mut all = Vec::new();
        for face in 0..=hand.n_levels {
            for &(a, b) in &cols {
                all.extend((0..7).map(|k| Ball::Flower { a, b, face, k }));
            }
        }
        for level in 0..hand.n_levels {
            for &(a, b) in &cols {
                all.extend((0..6).map(|k| Ball::Central { a, b, level, k }));
            }
        }
        for &id in &all {
            let c = hand.centre(id);
            let (whole, centre_in) = hand.containment(c);
            hand.present.insert(id, whole);
            if !whole && centre_in {
                hand.rejected += 1;
                hand.rejected_centres.push(c);
            }
        }
        // assign: the low-discrepancy rule over kept balls above the floor.
        let layer = |id: Ball| match id {
            Ball::Flower { face, .. } => 2 * face,
            Ball::Central { level, .. } => 2 * level + 1,
        };
        let mut eligible: Vec<(i32, i64, f64, Ball)> = Vec::new();
        for &id in &all {
            let [x, y, z] = hand.centre(id);
            if z > 0.0 && hand.present[&id] {
                eligible.push((
                    layer(id),
                    ((x * x + y * y) * 1.0e6).round() as i64,
                    y.r_atan2(x),
                    id,
                ));
            }
        }
        eligible.sort_by(|p, q| {
            p.0.cmp(&q.0)
                .then(p.1.cmp(&q.1))
                .then(p.2.total_cmp(&q.2))
                .then(p.3.cmp(&q.3))
        });
        for (n, &(_, _, _, id)) in eligible.iter().enumerate() {
            if ((n + 1) as f64 * fraction).floor() > (n as f64 * fraction).floor() {
                hand.fuel.insert(id);
            }
        }
        for &id in &all {
            if hand.present[&id] {
                hand.kept_all += 1;
                let c = hand.centre(id);
                hand.kept_centres.push((c, hand.fuel.contains(&id)));
                hand.kept_in_bed += usize::from(c[2] > 0.0);
            }
        }
        hand
    }

    fn present(&self, id: Ball) -> bool {
        self.present.get(&id).copied().unwrap_or_else(|| {
            let c = self.centre(id);
            self.containment(c).0
        })
    }

    fn state(&self, a: i32, b: i32, level: i32, site: Site) -> u8 {
        let id = tile_ball(a, b, level, site);
        if !self.present(id) {
            0
        } else if self.fuel.contains(&id) {
            2
        } else {
            1
        }
    }
}

/// nee_soon's counts, `SekerBed::new` run on 2026-10-18 (14 rings, 36.946 cm
/// cone, r 90, tube r 25 cm 221.236 cm deep, `FuelAssignment::Paper`): `(N,
/// balls above the floor, fuel, rejected (boundary_rejected), kept anywhere)`.
const NEE_SOON: [(usize, usize, usize, usize, usize); 5] = [
    (3, 4711, 2685, 1116, 8712),
    (10, 14021, 7991, 1725, 18022),
    (12, 16681, 9508, 1899, 20682),
    (16, 22001, 12540, 2247, 26002),
    (20, 27321, 15572, 2595, 31322),
];

/// A library of incident-neutron headers (masses only).
fn header_only_library() -> NuclearData {
    let endf = |x: f64| {
        let e = x.abs().r_log10().floor() as i32;
        format!("{:.6}{e:+}", x / 10f64.r_powi(e))
    };
    let tape = |za: f64, awr: f64| {
        format!(
            " TPID\n{:>11}{:>11}{:>11}{:>11}{:>11}{:>11}\n{:>11}{:>11}{:>11}{:>11}{:>11}{:>11}\n{:>11}{:>11}{:>11}{:>11}{:>11}{:>11}\n",
            endf(za),
            endf(awr),
            0, 0, 0, 0, "0.0", "0.0", 0, 0, 0, 6, "1.0", "2.0+7", 0, 0, 10, 8
        )
        .into_bytes()
    };
    NuclearData::from_endf_bytes(
        "headers only",
        [
            ("u235".to_string(), tape(92235.0, 233.0248)),
            ("u238".to_string(), tape(92238.0, 236.0058)),
            ("o16".to_string(), tape(8016.0, 15.857510)),
            ("c12".to_string(), tape(6012.0, 11.896910)),
        ],
    )
    .expect("four incident-neutron headers")
}

/// LOW-tier materials (as tutorial 7's test).
fn small_materials() -> (seker::Materials, Material) {
    let d = atoms_per_barn_cm;
    let mix = Material::from_atom_densities([
        (U235, d(0.0016)),
        (U238, d(0.0064)),
        (O16, d(0.016)),
        (C12, d(0.05)),
    ])
    .named("fuel mix");
    let shell = Material::from_atom_densities([(C12, d(0.0870))]).named("shell graphite");
    let gas = Material::from_atom_densities([(C12, d(1.0e-6))]).named("gas");
    (
        seker::Materials {
            uo2: mix.clone(),
            buffer: shell.clone(),
            pyc: shell.clone(),
            sic: shell.clone(),
            matrix: shell.clone(),
            shell,
            helium: gas,
        },
        mix,
    )
}

fn components(model: &mut Model) -> bed::Components {
    bed::Components {
        layer: seker::Components {
            kernel: model.component("Kernel"),
            coatings: model.component("Coatings"),
            matrix: model.component("Matrix"),
            shell: model.component("Fuel ball shell"),
            moderator: model.component("Moderator ball"),
            helium: model.component("Helium"),
        },
        reflector: model.component("Graphite reflector"),
        bricks: model.component("Boronated bricks"),
    }
}

/// A hopper holding only the bed, vacuum outside: the model the bit-for-bit
/// test runs.
fn hopper_model(
    data: NuclearData,
    m: &seker::Materials,
    ball: &seker::FuelBall,
    cv: Cavity,
    top: f64,
) -> (Model, bed::Components) {
    let mut model = Model::new(data);
    let c = components(&mut model);
    let lattice = Lattice::hex_bed(
        bed::tile(m, &c, ball),
        bed::moderator_ball(m, &c),
        Ratio::new::<ratio>(0.57),
        cm(top),
    );
    let hopper = Hopper::new(cm(cv.radius), cm(cv.height))
        .conical_floor(cm(cv.cone), cm(cv.outlet))
        .outlet_tube(cm(cv.tube))
        .filled_with(m.helium.clone())
        .in_component(c.layer.helium)
        .containing_lattice(lattice);
    model.add(hopper);
    model.set_temperature(Temperature::Uniform(
        uom::si::f64::ThermodynamicTemperature::new::<uom::si::thermodynamic_temperature::kelvin>(
            T,
        ),
    ));
    (model, c)
}

fn low_tier(a: &AssembledModel) -> Vec<Nuclide> {
    a.nuclides
        .iter()
        .map(|k| {
            let name = match k.isotope.name().as_str() {
                "C12" => "C0".to_string(),
                other => other.to_string(),
            };
            Nuclide::from_core(&name).expect("CORE nuclide")
        })
        .collect()
}

fn half(s: usize, sense: HalfSpaceSense) -> RegionToken {
    RegionToken::HalfSpace {
        surface_idx: s,
        sense,
    }
}

/// The hand-built geometry of the small bed: far plane, 23 site spheres, the
/// hopper's seven surfaces; two ball universes, one tile universe per state,
/// the outer universe, the lattice, the root.
fn hand_built(a: &AssembledModel, hand: &Hand, fuel_ball_is_smeared: bool) -> Geometry {
    assert!(fuel_ball_is_smeared);
    let m = |name: &str| {
        a.materials
            .iter()
            .position(|x| x.name == name)
            .unwrap_or_else(|| panic!("no material {name}"))
    };
    let (fuel_m, mod_m, gas_m) = (m("fuel mix"), m("shell graphite"), m("gas"));
    let cv = hand.cavity;
    let (inside, outside) = (HalfSpaceSense::Inside, HalfSpaceSense::Outside);
    let x = RegionToken::Intersection;
    let mut surfaces: Vec<SurfaceKind> = vec![SurfaceKind::XPlane(XPlane {
        x0: 1.0e30,
        bc: BoundaryType::Transmissive,
    })];
    let site_list = sites();
    for s in &site_list {
        let c = site_centre(*s);
        surfaces.push(SurfaceKind::Sphere(CsgSphere {
            x0: c[0],
            y0: c[1],
            z0: c[2],
            r: 3.0,
            bc: BoundaryType::Transmissive,
        }));
    }
    let first_site = 1;
    let vac = BoundaryType::Vacuum;
    let (zf, zt, cyl, cone, zc, tube, zb) = (24, 25, 26, 27, 28, 29, 30);
    let slope = (cv.radius - cv.outlet) / cv.cone;
    surfaces.push(SurfaceKind::ZPlane(ZPlane { z0: 0.0, bc: vac }));
    surfaces.push(SurfaceKind::ZPlane(ZPlane {
        z0: cv.height,
        bc: vac,
    }));
    surfaces.push(SurfaceKind::ZCylinder(ZCylinder {
        x0: 0.0,
        y0: 0.0,
        r: cv.radius,
        bc: vac,
    }));
    surfaces.push(SurfaceKind::ZCone(ZCone {
        x0: 0.0,
        y0: 0.0,
        z0: -cv.radius / slope,
        r_sq: slope * slope,
        bc: vac,
    }));
    surfaces.push(SurfaceKind::ZPlane(ZPlane {
        z0: -cv.cone,
        bc: vac,
    }));
    surfaces.push(SurfaceKind::ZCylinder(ZCylinder {
        x0: 0.0,
        y0: 0.0,
        r: cv.outlet,
        bc: vac,
    }));
    surfaces.push(SurfaceKind::ZPlane(ZPlane {
        z0: -cv.cone - cv.tube,
        bc: vac,
    }));

    let everywhere = vec![half(0, inside), half(0, outside), RegionToken::Union];
    let mut cells: Vec<Cell> = Vec::new();
    let mut universes: Vec<Universe> = Vec::new();
    let add_cell = |cells: &mut Vec<Cell>, mk: &dyn Fn(i32) -> Cell| -> usize {
        cells.push(mk(cells.len() as i32 + 1));
        cells.len() - 1
    };
    // Ball universes: fuel, moderator.
    for mat in [fuel_m, mod_m] {
        let c = add_cell(&mut cells, &|id| {
            Cell::material(id, everywhere.clone(), mat, T)
        });
        universes.push(Universe {
            id: universes.len() as i32,
            cell_indices: vec![c],
        });
    }
    // Tile variants.
    let nr = hand.n_rings;
    let mut variants: HashMap<Vec<u8>, usize> = HashMap::new();
    let mut tiles: Vec<(i32, i32, i32, Vec<u8>)> = Vec::new();
    let mut order: Vec<Vec<u8>> = vec![vec![0; 23]];
    for level in 0..hand.n_levels {
        for a_ in -(nr - 1)..=(nr - 1) {
            for b in -(nr - 1)..=(nr - 1) {
                if hex_ring(a_, b) > (nr - 1) as usize {
                    continue;
                }
                let key: Vec<u8> = site_list
                    .iter()
                    .map(|s| hand.state(a_, b, level, *s))
                    .collect();
                if !order.contains(&key) {
                    order.push(key.clone());
                }
                tiles.push((a_, b, level, key));
            }
        }
    }
    for key in &order {
        let mut idx = Vec::new();
        let mut region: Vec<RegionToken> = Vec::new();
        for (i, (&state, s)) in key.iter().zip(&site_list).enumerate() {
            if state == 0 {
                continue;
            }
            let c = site_centre(*s);
            let ci = add_cell(&mut cells, &|id| {
                Cell::fill(
                    id,
                    vec![half(first_site + i, inside)],
                    CellFill::Universe(usize::from(state) - 1 + 0),
                    Position::new(c[0], c[1], c[2]),
                )
            });
            // state 1 = moderator = universe 1; state 2 = fuel = universe 0.
            cells[ci].fill = CellFill::Universe(if state == 2 { 0 } else { 1 });
            idx.push(ci);
            if region.is_empty() {
                region.push(half(first_site + i, outside));
            } else {
                region.push(half(first_site + i, outside));
                region.push(x);
            }
        }
        if region.is_empty() {
            region = everywhere.clone();
        }
        let bg = add_cell(&mut cells, &|id| {
            Cell::material(id, region.clone(), gas_m, T)
        });
        idx.push(bg);
        universes.push(Universe {
            id: universes.len() as i32,
            cell_indices: idx,
        });
        variants.insert(key.clone(), universes.len() - 1);
    }
    // Outer universe, never reached.
    let oc = add_cell(&mut cells, &|id| {
        Cell::material(
            id,
            vec![half(cyl, inside), half(cyl, outside), RegionToken::Union],
            gas_m,
            T,
        )
    });
    universes.push(Universe {
        id: universes.len() as i32,
        cell_indices: vec![oc],
    });
    let outer = universes.len() - 1;
    let outer_tile = variants[&order[0]];
    let placeholder: Vec<Vec<Vec<usize>>> = (0..hand.n_levels)
        .map(|_| {
            (0..nr as usize)
                .rev()
                .map(|r| vec![outer_tile; if r == 0 { 1 } else { 6 * r }])
                .collect()
        })
        .collect();
    let mut lat = HexLattice::from_rings_3d(
        1,
        HexOrientation::Y,
        Position::new(
            0.0,
            0.0,
            hand.z_bottom + 0.5 * f64::from(hand.n_levels) * hand.h,
        ),
        hand.pitch,
        hand.h,
        &placeholder,
        Some(outer),
    );
    for (a_, b, level, key) in &tiles {
        let i = [a_ + nr - 1, b + nr - 1, *level];
        let flat = lat.flat_index(i);
        lat.universes[flat] = variants[key] as i32;
    }
    // The root: the hopper's three-part region, filled with the lattice.
    let region = vec![
        half(cyl, inside),
        half(zf, outside),
        x,
        half(zt, inside),
        x,
        half(cone, inside),
        half(zf, inside),
        x,
        half(zc, outside),
        x,
        RegionToken::Union,
        half(tube, inside),
        half(zc, inside),
        x,
        half(zb, outside),
        x,
        RegionToken::Union,
    ];
    // The union is written (cylinder part) (cone part) U (tube part) U.
    let region = {
        let mut r = vec![half(cyl, inside), half(zf, outside), x, half(zt, inside), x];
        r.extend([
            half(cone, inside),
            half(zf, inside),
            x,
            half(zc, outside),
            x,
            RegionToken::Union,
        ]);
        r.extend([
            half(tube, inside),
            half(zc, inside),
            x,
            half(zb, outside),
            x,
            RegionToken::Union,
        ]);
        assert_eq!(r.len(), region.len());
        r
    };
    let fill = add_cell(&mut cells, &|id| {
        Cell::fill(id, region.clone(), CellFill::Lattice(0), Position::ZERO)
    });
    universes.push(Universe {
        id: universes.len() as i32,
        cell_indices: vec![fill],
    });
    let root = universes.len() - 1;
    let _ = zb;
    Geometry {
        surfaces,
        cells,
        universes,
        lattices: vec![CsgLattice::Hex(lat)],
        root_universe: root,
    }
}

// ── 1 and 2: the counts, against nee_soon and the hand-built layout ────────

#[test]
fn the_real_cavity_keeps_nee_soons_balls_and_rejects_nee_soons_rejects() {
    let data =
        NuclearData::from_folder(&njoy_outram_park_fork::reference_data::reference_endf_dir())
            .expect("reference-data/endf is tracked");
    let mats = seker::materials(T);
    let refl = bed::reflector_materials();
    for (n, in_bed, fuel, rejected, kept_all) in NEE_SOON {
        let core = bed::build(
            data_clone(&data),
            &mats,
            &refl,
            &seker::FuelBall::Smeared(mats.matrix.clone()),
            n,
            T,
        );
        let a = core.model.assemble().expect("assembles");
        let c = a.bed_census().expect("a bed");
        println!(
            "N = {n}: bed {} fuel {} rejected {} kept {} (nee_soon {in_bed} {fuel} {rejected} {kept_all}); height {:.4}, filling {:.4}",
            c.bed_balls, c.fuel_balls, c.rejected_balls, c.loaded_balls, c.bed_height_cm, c.filling_fraction
        );
        assert_eq!(
            (c.bed_balls, c.fuel_balls, c.rejected_balls, c.loaded_balls),
            (in_bed, fuel, rejected, kept_all),
            "N = {n}"
        );
        // The independent hand-built layout gives the same, ball for ball.
        let hand = Hand::new(Cavity::htr10(), bed::loading_height_cm(n), 0.57);
        assert_eq!(
            (
                hand.kept_in_bed,
                hand.fuel.len(),
                hand.rejected,
                hand.kept_all
            ),
            (in_bed, fuel, rejected, kept_all)
        );
        // Places compared on a 1e-3 cm grid (balls are 6 cm apart).
        let key = |p: [f64; 3]| p.map(|v| (v * 1e3).round() as i64);
        let ours: HashMap<[i64; 3], bool> =
            c.kept.iter().map(|k| (key(k.centre_cm), k.fuel)).collect();
        let theirs: HashMap<[i64; 3], bool> = hand
            .kept_centres
            .iter()
            .map(|(p, f)| (key(*p), *f))
            .collect();
        assert_eq!(ours.len(), c.kept.len(), "no two kept balls share a place");
        let only_ours = ours.keys().filter(|k| !theirs.contains_key(*k)).count();
        let only_theirs = theirs.keys().filter(|k| !ours.contains_key(*k)).count();
        let kinds_differ = ours
            .iter()
            .filter(|(k, f)| theirs.get(*k).is_some_and(|g| g != *f))
            .count();
        let mut shown = 0;
        for (k, f) in &ours {
            if theirs.get(k).is_some_and(|g| g != f) && shown < 6 {
                println!(
                    "   differs at {:?} cm: here fuel = {f}",
                    k.map(|v| v as f64 / 1e3)
                );
                shown += 1;
            }
        }
        println!("  places only here {only_ours}, only in the hand-built {only_theirs}, kinds differing {kinds_differ}");
        assert_eq!(
            (only_ours, only_theirs, kinds_differ),
            (0, 0, 0),
            "every kept ball, its place and its kind"
        );
        let r_ours: HashSet<[i64; 3]> = c.rejected.iter().map(|k| key(k.centre_cm)).collect();
        let r_theirs: HashSet<[i64; 3]> = hand.rejected_centres.iter().map(|p| key(*p)).collect();
        assert_eq!(r_ours, r_theirs, "every rejected ball");
        // Şeker's height, whole balls, and the rejection breakdown adds up.
        assert!((c.bed_height_cm - (9.797_959 * n as f64 + 6.0)).abs() < 1e-3);
        assert_eq!(
            c.rejected_at_side_wall
                + c.rejected_at_cone
                + c.rejected_at_tube_wall
                + c.rejected_at_tube_bottom
                + c.rejected_at_top,
            c.rejected_balls
        );
        assert_eq!(c.rejected_at_top, 0, "the top plane of balls just fits");
        // Per plane: 721 balls per basal plane, 609 per central plane (the
        // whole-ball rule's 721 + 609; Seker's table has 733 + 613).
        assert_eq!(c.planes.len(), 2 * n + 1);
        for (i, p) in c.planes.iter().enumerate() {
            assert_eq!(p.1, if i % 2 == 0 { 721 } else { 609 }, "plane {i}");
        }
        // Against Seker's own count: fewer, as the page says.
        assert_eq!(bed::seker_reference_balls(n), 1346 * n + 733);
        assert!(c.bed_balls_against(bed::seker_reference_balls(n)) < 0);
        assert!((c.filling_fraction - 0.5999).abs() < 0.002 || n < 5);
        assert_eq!(c.fuel_balls + c.moderator_balls_in_bed, c.bed_balls);
        assert_eq!(c.bed_balls + c.cone_and_tube_balls, c.loaded_balls);
        assert_eq!(c.kept.len(), c.loaded_balls);
        assert_eq!(c.rejected.len(), c.rejected_balls);
        let text = format!("{c}");
        assert!(text.contains("rejected whole") && text.contains("filling fraction"));
        assert!(c.rejected.iter().any(|r| r.wall == Wall::Cone));
    }
}

/// `NuclearData` is built once and each model takes its own copy of the
/// folder's tapes; `workspace`-style loading is cheap on headers.
fn data_clone(_template: &NuclearData) -> NuclearData {
    NuclearData::from_folder(&njoy_outram_park_fork::reference_data::reference_endf_dir())
        .expect("reference-data/endf is tracked")
}

#[test]
fn the_geometry_census_counts_the_balls_once_each() {
    let data =
        NuclearData::from_folder(&njoy_outram_park_fork::reference_data::reference_endf_dir())
            .expect("reference-data/endf is tracked");
    let mats = seker::materials(T);
    let refl = bed::reflector_materials();
    let core = bed::build(
        data,
        &mats,
        &refl,
        &seker::FuelBall::Smeared(mats.matrix.clone()),
        3,
        T,
    );
    let a = core.model.assemble().expect("assembles");
    let b = a.bed_census().expect("a bed");
    let census = a.census().expect("census");
    let c = core.components;
    // Each ball is one place however many tiles draw a piece of it.
    assert_eq!(census.copies(c.layer.shell), b.fuel_balls);
    assert_eq!(
        census.copies(c.layer.moderator),
        b.moderator_balls_in_bed + b.cone_and_tube_balls
    );
    let row = census
        .packings
        .iter()
        .find(|p| matches!(p.arrangement, Arrangement::HexBed { .. }))
        .expect("the bed's packing row");
    assert_eq!(row.particles, b.loaded_balls);
    let text = format!("{census}");
    assert!(text.contains("hexagonal bed of balls"));
    // The bed's helium is one pool: hopper volume less the kept balls.
    let hopper = Hopper::new(cm(bed::CORE_RADIUS_CM), cm(bed::CAVITY_HEIGHT_CM))
        .conical_floor(cm(bed::CONE_HEIGHT_CM), cm(bed::TUBE_RADIUS_CM))
        .outlet_tube(cm(bed::TUBE_DEPTH_CM));
    let ball_v = 4.0 / 3.0 * std::f64::consts::PI * 27.0;
    let want = hopper.volume_cm3() - b.loaded_balls as f64 * ball_v;
    let pool = census
        .rows
        .iter()
        .find(|r| r.kind == RowKind::Pool && r.component == Some(c.layer.helium))
        .expect("the helium pool");
    let got = pool.volume().get::<uom::si::volume::cubic_centimeter>();
    assert!((got / want - 1.0).abs() < 1e-12, "{got} against {want}");
    assert!(a.bed_censuses().len() == 1);
}

// ── 3 and 4: the small bed, lookups and transport ──────────────────────────

#[test]
fn the_beginner_bed_lowers_to_the_hand_built_hexagonal_lattice() {
    let (mats, mix) = small_materials();
    let cv = Cavity::small();
    let top = bed::loading_height_cm(2);
    let (model, _) = hopper_model(
        header_only_library(),
        &mats,
        &seker::FuelBall::Smeared(mix),
        cv,
        top,
    );
    let a = model.assemble().expect("assembles");
    let hand = Hand::new(cv, top, 0.57);
    let census = a.bed_census().expect("a bed");
    println!(
        "small bed: {} balls in the bed, {} fuel, {} rejected, {} kept in all",
        census.bed_balls, census.fuel_balls, census.rejected_balls, census.loaded_balls
    );
    assert_eq!(
        (
            census.bed_balls,
            census.fuel_balls,
            census.rejected_balls,
            census.loaded_balls
        ),
        (
            hand.kept_in_bed,
            hand.fuel.len(),
            hand.rejected,
            hand.kept_all
        )
    );
    assert!(census.rejected_balls > 10 && census.fuel_balls > 10);
    let g = hand_built(&a, &hand, true);

    // Structure.
    assert_eq!(a.geometry.surfaces.len(), g.surfaces.len(), "surfaces");
    assert_eq!(a.geometry.cells.len(), g.cells.len(), "cells");
    assert_eq!(a.geometry.universes.len(), g.universes.len(), "universes");
    assert_eq!(a.geometry.lattices.len(), 1);
    match (&a.geometry.lattices[0], &g.lattices[0]) {
        (CsgLattice::Hex(l), CsgLattice::Hex(m)) => {
            assert_eq!((l.n_rings, l.n_axial), (m.n_rings, m.n_axial));
            assert_eq!(l.orientation, HexOrientation::Y);
            assert_eq!(l.pitch[0].to_bits(), m.pitch[0].to_bits());
            assert_eq!(l.pitch[1].to_bits(), m.pitch[1].to_bits());
            assert_eq!(
                l.center.z.to_bits(),
                m.center.z.to_bits(),
                "lattice centre z"
            );
        }
        _ => panic!("hexagonal lattices"),
    }
    // The hopper's surfaces carry the vacuum boundary, the cone too.
    let vacuum = a
        .geometry
        .surfaces
        .iter()
        .filter(|s| match s {
            SurfaceKind::ZPlane(p) => p.bc == BoundaryType::Vacuum,
            SurfaceKind::ZCylinder(p) => p.bc == BoundaryType::Vacuum,
            SurfaceKind::ZCone(p) => p.bc == BoundaryType::Vacuum,
            _ => false,
        })
        .count();
    assert_eq!(vacuum, 7, "three planes... the hopper's seven surfaces");

    // Lookups.
    let names: Vec<String> = a.materials.iter().map(|m| m.name.clone()).collect();
    let find = |g: &Geometry, p: Position| {
        g.locate(p, Direction::new(0.0, 0.0, 1.0), SurfaceToken::NONE)
            .and_then(|path| path.material)
            .map(|m| names[m].clone())
    };
    let mut seed = 20_261_018_u64;
    let mut prn = move || {
        seed = seed
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        ((seed >> 11) as f64) / ((1u64 << 53) as f64)
    };
    let (mut inside, mut in_ball) = (0, 0);
    for i in 0..300_000 {
        let p = Position::new(
            (2.0 * prn() - 1.0) * 34.0,
            (2.0 * prn() - 1.0) * 34.0,
            -cv.cone - cv.tube - 3.0 + prn() * (cv.cone + cv.tube + cv.height + 6.0),
        );
        let (l, r) = (find(&a.geometry, p), find(&g, p));
        assert_eq!(l, r, "point {i} ({}, {}, {})", p.x, p.y, p.z);
        if l.is_some() {
            inside += 1;
            in_ball += usize::from(l.as_deref() != Some("gas"));
        }
    }
    println!("lookups: 300000 points agree ({inside} in the hopper, {in_ball} in a ball)");
    assert!(inside > 100_000 && in_ball > 30_000);
    // Every kept ball's centre is a ball of its kind, every rejected one gas.
    for k in &census.kept {
        let want = if k.fuel { "fuel mix" } else { "shell graphite" };
        let p = Position::new(k.centre_cm[0], k.centre_cm[1], k.centre_cm[2]);
        assert_eq!(find(&a.geometry, p).as_deref(), Some(want));
    }
    for r in &census.rejected {
        let p = Position::new(r.centre_cm[0], r.centre_cm[1], r.centre_cm[2]);
        assert_eq!(find(&a.geometry, p).as_deref(), Some("gas"));
    }

    // Transport: k in every generation, bit for bit.
    let nuclides = low_tier(&a);
    let (n, inactive, active, seed) = (300, 4, 8, 11);
    let low = LowKeffSettings {
        n_particles: n,
        n_inactive: inactive,
        n_active: active,
        temperature_k: T,
        seed,
        ..LowKeffSettings::default()
    };
    let reference_seq = run_keff_csg(&g, &a.materials, &nuclides, a.source_box, &low, None);
    let reference = run_keff_csg(
        &g,
        &a.materials,
        &nuclides,
        a.source_box,
        &LowKeffSettings {
            compute: ComputeType::CpuMultiThread(ThreadCount::Fixed(3)),
            ..low.clone()
        },
        None,
    );
    let custom = KeffSettings::new(
        MCSettings::Custom(CustomRun {
            particles: n,
            inactive,
            active,
            seed,
        }),
        None,
    );
    let run = a.keff_with(&nuclides, &custom).expect("runs");
    println!(
        "k per generation: beginner {:?}\n                  hand-built {:?}",
        run.k_by_cycle, reference.k_by_generation
    );
    assert_eq!(
        run.k_by_cycle, reference.k_by_generation,
        "per-generation k, bit for bit"
    );
    let single = a
        .keff_with(&nuclides, &custom.clone().single_thread())
        .expect("runs on one thread");
    assert_eq!(
        single.k_by_cycle, reference_seq.k_by_generation,
        "single thread"
    );
    assert!(run.k_by_cycle.iter().all(|k| k.is_finite() && *k > 0.0));
}

// ── 5: the pieces ──────────────────────────────────────────────────────────

/// Two coaxial cylinders of one height meet on one plane each, and the
/// outermost's boundary reaches both (a reflector of brick around graphite).
#[test]
fn coaxial_cylinders_of_one_height_share_their_planes() {
    let (mats, _) = small_materials();
    let mut model = Model::new(header_only_library());
    let inner = Cylinder::radius(cm(2.0))
        .height(cm(10.0))
        .centred_at(cm(0.0), cm(0.0), cm(5.0))
        .filled_with(mats.shell.clone());
    let outer = Cylinder::radius(cm(4.0))
        .height(cm(10.0))
        .centred_at(cm(0.0), cm(0.0), cm(5.0))
        .filled_with(mats.matrix.clone())
        .containing(inner);
    model.add(outer);
    model.set_temperature(Temperature::Uniform(
        uom::si::f64::ThermodynamicTemperature::new::<uom::si::thermodynamic_temperature::kelvin>(
            T,
        ),
    ));
    let a = model.assemble().expect("assembles");
    let planes: Vec<&ZPlane> = a
        .geometry
        .surfaces
        .iter()
        .filter_map(|s| match s {
            SurfaceKind::ZPlane(p) => Some(p),
            _ => None,
        })
        .collect();
    assert_eq!(planes.len(), 2, "one plane at each end, not two");
    assert!(planes.iter().all(|p| p.bc == BoundaryType::Vacuum));
}

/// The bad inputs, each named.
#[test]
fn a_bad_bed_is_refused_in_words() {
    let (mats, mix) = small_materials();
    let cv = Cavity::small();
    let ball = seker::FuelBall::Smeared(mix.clone());
    let build = |f: &dyn Fn(&mut Model, &bed::Components) -> Shape| {
        let mut model = Model::new(header_only_library());
        let c = components(&mut model);
        let s = f(&mut model, &c);
        model.add(s);
        model.set_temperature(Temperature::Uniform(
            uom::si::f64::ThermodynamicTemperature::new::<uom::si::thermodynamic_temperature::kelvin>(T),
        ));
        model.assemble().err().map(|e| e.to_string())
    };
    let hopper = |c: &bed::Components| {
        Hopper::new(cm(cv.radius), cm(cv.height))
            .conical_floor(cm(cv.cone), cm(cv.outlet))
            .outlet_tube(cm(cv.tube))
            .filled_with(mats.helium.clone())
            .in_component(c.layer.helium)
    };
    let lattice = |c: &bed::Components, top: f64, f: f64| {
        Lattice::hex_bed(
            bed::tile(&mats, c, &ball),
            bed::moderator_ball(&mats, c),
            Ratio::new::<ratio>(f),
            cm(top),
        )
    };
    // The good one assembles.
    assert!(build(&|_, c| hopper(c).containing_lattice(lattice(c, 25.0, 0.57))).is_none());
    let said = |why: Option<String>, needle: &str| {
        let w = why.expect("refused");
        assert!(w.contains(needle), "{w}");
    };
    said(
        build(&|_, c| hopper(c).containing_lattice(lattice(c, 25.0, 1.5))),
        "fuel fraction",
    );
    said(
        build(&|_, c| hopper(c).containing_lattice(lattice(c, 45.0, 0.57))),
        "loaded to",
    );
    said(
        build(&|_, c| hopper(c).containing_lattice(lattice(c, -1.0, 0.57))),
        "loaded to",
    );
    said(
        build(&|_, c| {
            Cylinder::radius(cm(30.0))
                .height(cm(40.0))
                .filled_with(mats.helium.clone())
                .containing_lattice(lattice(c, 25.0, 0.57))
        }),
        "fills a Hopper",
    );
    said(
        build(&|_, c| {
            Hopper::new(cm(cv.radius), cm(cv.height))
                .conical_floor(cm(cv.cone), cm(cv.outlet))
                .filled_with(mats.shell.clone())
                .containing_lattice(lattice(c, 25.0, 0.57))
        }),
        "hopper's material",
    );
    said(
        build(&|_, c| {
            Hopper::new(cm(cv.radius), cm(cv.height))
                .conical_floor(cm(cv.cone), cm(cv.cone * 10.0))
                .filled_with(mats.helium.clone())
                .containing_lattice(lattice(c, 25.0, 0.57))
        }),
        "every size must be positive",
    );
    // A moderator ball of another radius.
    said(
        build(&|_, c| {
            let l = Lattice::hex_bed(
                bed::tile(&mats, c, &ball),
                Sphere::radius(cm(2.0)).filled_with(mats.shell.clone()),
                Ratio::new::<ratio>(0.57),
                cm(25.0),
            );
            hopper(c).containing_lattice(l)
        }),
        "fuel ball's radius",
    );
    // A tile whose balls differ.
    said(
        build(&|_, c| {
            let mut t = HexPrism::flat_to_flat(cm(seker::pitch_cm()), cm(seker::height_cm()))
                .filled_with(mats.helium.clone())
                .in_component(c.layer.helium);
            let all = seker::own_sites();
            for (i, site) in all.iter().enumerate() {
                let at = seker::site_centre(*site);
                t = if i + 1 == all.len() {
                    t.containing(
                        Sphere::radius(cm(3.0))
                            .centred_at(cm(at[0]), cm(at[1]), cm(at[2]))
                            .filled_with(mats.shell.clone()),
                    )
                } else {
                    t.containing(seker::fuel_ball(&mats, &c.layer, &ball, at))
                };
            }
            let l = Lattice::hex_bed(
                t,
                bed::moderator_ball(&mats, c),
                Ratio::new::<ratio>(0.57),
                cm(25.0),
            );
            hopper(c).containing_lattice(l)
        }),
        "same fuel ball",
    );
    // The model of no bed says so.
    let mut model = Model::new(header_only_library());
    model.add(Sphere::radius(cm(1.0)).filled_with(mats.shell.clone()));
    model.set_temperature(Temperature::Uniform(
        uom::si::f64::ThermodynamicTemperature::new::<uom::si::thermodynamic_temperature::kelvin>(
            T,
        ),
    ));
    let a = model.assemble().expect("assembles");
    assert!(a
        .bed_census()
        .unwrap_err()
        .to_string()
        .contains("no stacked bed"));
    assert!(a.bed_censuses().is_empty());
}
