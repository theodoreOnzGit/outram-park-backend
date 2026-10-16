// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// This file is part of OUTRAM PARK. See the module header of `htr10_rmc` for
// licence terms.

//! # HTR-10 cells for the MGXS gate (gh:#849)
//!
//! Two small CSG models built from the **same** particle, pebble and Şeker
//! tile as the full core, so multigroup constants condensed on them describe
//! the core's fuel:
//!
//! - [`Htr10Cell::single_pebble`]: one explicit-TRISO fuel pebble and helium
//!   in a reflective cube of side 6 cm, the simplest cell for the gate.
//! - [`Htr10Cell::seker_cluster`]: Şeker & Çolak (2003)'s 13-ball tiles at
//!   the core's packing, fuel and dummy balls by the core's rule, in a
//!   reflective hexagonal prism one tile high.
//!
//! The TRISO particle and its lattice are [`TrisoBuild`] and the pebbles of a
//! tile [`build_tile_universes`], both extracted from the core builder for
//! this module (2026-10-13); nothing is copied.
//!
//! # The single pebble's packing is 0.524, not 0.61
//!
//! One 6 cm ball in a 6 cm cube is simple cubic packing: `(pi/6) = 0.5236`
//! of the volume is pebble. The core's bed is 0.61 (Şeker's 0.6448 in the
//! interior). The single pebble is a deliberately simple cell. ~~Its helium
//! fraction is too high for the core, though helium barely interacts.~~
//! **CORRECTED 2026-10-13 (pilots, `verification_and_validation/htr10_mgxs_cells/`):**
//! the helium matters little, but every ball in this cell is fuel, while the
//! core's bed is 43 % dummy graphite balls. The cell is therefore
//! under-moderated. Its kernel's one-group Σa is 28 % below the Şeker
//! cluster's, and its `k_inf` is 6100 ± 536 pcm lower. Its constants do not
//! describe the core's fuel; it serves as the simplest MG-against-CE gate.
//!
//! # The Şeker cluster's boundary, and why it is a superlattice hexagon
//!
//! Top and bottom are exact under reflection: the planes cut the flower
//! balls through their centres, and the close-packed A-B-A stacking is
//! mirror-symmetric about an A plane. **The side faces are not**: a tile's
//! `Outer(k)` balls cross three of its faces and its neighbours' balls enter
//! through the other three, so a reflected tile face distorts those balls.
//! Periodic boundaries, which would be exact, are refused by
//! `outram-mc-libs` (gh:#259).
//!
//! So the side boundary is the Wigner–Seitz hexagon of a **hexagonal
//! superlattice** of the tile lattice, with superlattice vector
//! `L = i a1 + j a2` (`a1`, `a2` the tile lattice vectors of
//! `htr10_rmc::bed::tile_xy`): `(i, j) = (1, 0), (2, 1), (3, 2)` for
//! `rings = 1, 2, 3`, holding `N = i^2 + i j + j^2 = 1, 7, 19` tiles' worth of
//! bed. That hexagon is the exact periodic unit of the bed's geometry, so
//! under periodic boundaries it would be exact; with reflective ones it is an
//! approximation that **must be measured**, by the convergence of `k_inf` and
//! the constants over N = 1, 7, 19, not assumed away. For N = 1 it is the tile
//! itself; for N = 7 and 19 it is rotated against the tiles (by 19.1° and
//! 23.4°) and cuts through them. The ball fraction inside it is Şeker's
//! 0.6448 for every N, because the hexagon is a unit cell of the tiling.
//!
//! **What the convex hexagon replaces.** A boundary following the cluster's
//! own zig-zag outline would need reflective planes that also pass through
//! the cluster's interior, and a reflective surface reflects wherever it is
//! crossed. A convex prism is the only reflective boundary this CSG can
//! express without that error.
//!
//! # Fuel and dummy balls
//!
//! Every ball with a piece inside the prism takes part; they are ordered as
//! `SekerBed::assign` orders the core's (by layer, then `rho^2`, then angle,
//! then id), and the core's low-discrepancy rule makes a ball fuel when
//! `floor((n + 1) f) > floor(n f)`, with `f` the design's
//! `fuel_ball_fraction` (0.57 by default). A cell this small cannot hold
//! exactly 0.57 of its ball volume as fuel; [`Htr10Cell::fuel_balls`] and
//! [`Htr10Cell::dummy_balls`] report what it holds.
//!
//! **Research, education and V&V only** (`RESPONSIBLE_USE.md`).

use outram_mc_libs::geometry::cell::{Cell, CellFill, RegionToken};
use outram_mc_libs::geometry::geometry::Geometry;
use outram_mc_libs::geometry::lattice::{HexLattice, HexOrientation, Lattice};
use outram_mc_libs::geometry::position::Position;
use outram_mc_libs::geometry::surface::{
    BoundaryType, Plane, Sphere, SurfaceKind, XPlane, YPlane, ZPlane,
};
use outram_mc_libs::geometry::universe::Universe;
use outram_mc_libs::physics::transport_csg::SourceBox;
use outram_mc_libs::tally::mesh::RegularMesh;
use serde::{Deserialize, Serialize};

use crate::htr10_rmc::bed::{hex_ring, seker_tile_ball, tile_xy, SekerBallId, SekerCell, SekerSite};
use crate::htr10_rmc::core_design::Htr10CoreDesign;
use crate::htr10_rmc::core_model::{
    build_tile_universes, TRISO_MATRIX_UNIVERSE, TRISO_PARTICLE_UNIVERSE,
};
use crate::htr10_rmc::core_shell::{ins, out, TrisoBuild};

/// Which cell: the identity a case or a record names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CellKind {
    /// One fuel pebble and helium in a reflective 6 cm cube.
    SinglePebble,
    /// Şeker tiles in the reflective superlattice prism of `rings` = 1, 2 or 3
    /// (1, 7 or 19 tiles' worth of bed).
    SekerCluster {
        /// 1, 2 or 3.
        rings: usize,
    },
}

impl CellKind {
    /// A short description for logs and records.
    #[must_use]
    pub fn describe(&self) -> String {
        match self {
            Self::SinglePebble => {
                "HTR-10 single fuel pebble, explicit TRISO, helium, reflective 6 cm cube \
                 (simple cubic, ball fraction 0.524)"
                    .to_string()
            }
            Self::SekerCluster { rings } => format!(
                "HTR-10 Şeker cluster, {} tiles' worth (superlattice prism, rings {rings}), \
                 explicit TRISO, reflective, one tile high",
                superlattice(*rings).map_or(0, |(i, j)| i * i + i * j + j * j)
            ),
        }
    }
}

/// An assembled HTR-10 cell: the CSG geometry and what it holds.
///
/// Material indices are those of `htr10_rmc::materials::htr10_material_set`
/// (`htr10_rmc::core_model::mat`), so the same material and nuclide tables
/// as the core run on it. The root cell is delta-tracked with majorant
/// `majorant_index`, as the core's bed is, or surface-tracked when
/// `majorant_index == usize::MAX`.
#[derive(Debug, Clone)]
pub struct Htr10Cell {
    /// Which cell.
    pub kind: CellKind,
    /// The assembled geometry.
    pub geometry: Geometry,
    /// Half-extents of the cell's bounding box \[cm\], centred on the origin.
    pub half_extent: [f64; 3],
    /// Volume of the cell \[cm^3\].
    pub volume_cm3: f64,
    /// Whole TRISO particles per fuel pebble.
    pub triso_particles: usize,
    /// Fuel balls with a piece inside the cell.
    pub fuel_balls: usize,
    /// Dummy (all-graphite) balls with a piece inside the cell.
    pub dummy_balls: usize,
    /// Centre \[cm\] of every ball with a piece inside the cell, and whether
    /// it is fuel: the identities the geometry was built from, kept so a
    /// check can recount the cell's contents without going through the CSG.
    pub balls: Vec<([f64; 3], bool)>,
}

impl Htr10Cell {
    /// The single pebble at the default design, delta-tracked with majorant 0.
    #[must_use]
    pub fn single_pebble() -> Self {
        Self::single_pebble_with(&Htr10CoreDesign::default(), 0)
    }

    /// The single pebble of `design`: one fuel pebble (6 cm ball, the design's
    /// fuel zone and TRISO lattice) and helium, in a reflective cube of side
    /// 6 cm centred on the pebble. See the module docs for its 0.524 packing.
    ///
    /// # Panics
    /// If `design` fails `Htr10CoreDesign::check`.
    #[must_use]
    pub fn single_pebble_with(design: &Htr10CoreDesign, majorant_index: usize) -> Self {
        if let Err(e) = design.check() {
            panic!("HTR-10 design refused: {e}");
        }
        let r_pebble = 0.5 * SekerCell::from_paper().ball_diameter;
        let half = r_pebble;
        let triso = TrisoBuild::new(design);
        // 0..4 TRISO shells, 5 fuel zone, 6 pebble, 7..12 the cube.
        let mut surfaces = triso.shells();
        surfaces.push(sphere([0.0; 3], design.fuel_zone_radius_cm));
        surfaces.push(sphere([0.0; 3], r_pebble));
        let r = BoundaryType::Reflective;
        surfaces.extend([
            SurfaceKind::XPlane(XPlane { x0: -half, bc: r }),
            SurfaceKind::XPlane(XPlane { x0: half, bc: r }),
            SurfaceKind::YPlane(YPlane { y0: -half, bc: r }),
            SurfaceKind::YPlane(YPlane { y0: half, bc: r }),
            SurfaceKind::ZPlane(ZPlane { z0: -half, bc: r }),
            SurfaceKind::ZPlane(ZPlane { z0: half, bc: r }),
        ]);
        let region = intersection((7..13).map(|i| if i % 2 == 1 { out(i) } else { ins(i) }));
        // Universe 3 (built below) is the pebble.
        let mut cells = vec![root_cell(region, CellFill::Universe(3), majorant_index)];
        let triso_first = cells.len();
        cells.extend(TrisoBuild::cells(0));
        let [pu, mu] = TrisoBuild::universes(triso_first, TRISO_PARTICLE_UNIVERSE, TRISO_MATRIX_UNIVERSE);
        let mut universes = vec![
            Universe {
                id: 0,
                cell_indices: vec![0],
            },
            pu,
            mu,
        ];
        // One site, fuelled and present: the core's own pebble construction.
        let keys = [(1u32, 1u32)];
        let key_universe =
            build_tile_universes(&keys, &[[0.0; 3]], &[(5, 6)], 0, &mut cells, &mut universes);
        debug_assert_eq!(key_universe[&keys[0]], 3);
        let geometry = Geometry {
            surfaces,
            cells,
            universes,
            lattices: vec![Lattice::Rect(triso.lattice(
                1,
                TRISO_PARTICLE_UNIVERSE,
                TRISO_MATRIX_UNIVERSE,
            ))],
            root_universe: 0,
        };
        Self {
            kind: CellKind::SinglePebble,
            geometry,
            half_extent: [half; 3],
            volume_cm3: (2.0 * half).powi(3),
            triso_particles: triso.n_particles,
            fuel_balls: 1,
            dummy_balls: 0,
            balls: vec![([0.0; 3], true)],
        }
    }

    /// The Şeker cluster of `rings` (1, 2 or 3) at the default design,
    /// delta-tracked with majorant 0.
    ///
    /// # Panics
    /// If `rings` is not 1, 2 or 3.
    #[must_use]
    pub fn seker_cluster(rings: usize) -> Self {
        Self::seker_cluster_with(rings, &Htr10CoreDesign::default(), 0)
    }

    /// The Şeker cluster of `rings` for `design`. See the module docs for the
    /// boundary and the fuel rule.
    ///
    /// # Panics
    /// If `rings` is not 1, 2 or 3, or `design` fails
    /// `Htr10CoreDesign::check`.
    #[must_use]
    pub fn seker_cluster_with(rings: usize, design: &Htr10CoreDesign, majorant_index: usize) -> Self {
        let (i, j) = superlattice(rings).unwrap_or_else(|| {
            panic!("seker_cluster: rings must be 1, 2 or 3 (1, 7 or 19 tiles), got {rings}")
        });
        if let Err(e) = design.check() {
            panic!("HTR-10 design refused: {e}");
        }
        let cell = SekerCell::from_paper();
        let (pitch, h) = (cell.pitch(), cell.height);
        let r_ball = 0.5 * cell.ball_diameter;
        let prism = Prism::new(i, j, pitch);

        // Covering tiles: every tile whose hexagon can reach the prism.
        let ring_reach = 0.5 * 3.0_f64.sqrt() * pitch;
        let n_cover = ((prism.circumradius + pitch) / ring_reach).ceil() as usize + 1;
        let nr = n_cover as i32;
        let mut tiles = Vec::new();
        for a in -(nr - 1)..=(nr - 1) {
            for b in -(nr - 1)..=(nr - 1) {
                if hex_ring(a, b) <= n_cover - 1 {
                    tiles.push((a, b));
                }
            }
        }

        // The balls with a piece inside the prism, and their identities.
        let centre = |id: SekerBallId| ball_centre(&cell, id);
        let mut balls: Vec<SekerBallId> = Vec::new();
        for &(a, b) in &tiles {
            for site in SekerSite::ALL {
                let id = seker_tile_ball(a, b, 0, site);
                let [x, y, _] = centre(id);
                // A ball only TOUCHING a face (distance = r, e.g. a
                // neighbour's flower ball tangent to the tile's side) has no
                // piece inside; 1 um keeps rounding from counting it.
                if prism.distance(x, y) < r_ball - 1.0e-4 && !balls.contains(&id) {
                    balls.push(id);
                }
            }
        }
        // The core's order (`SekerBed::assign`): layer, rho^2, angle, id.
        balls.sort_by(|p, q| {
            let ([px, py, pz], [qx, qy, qz]) = (centre(*p), centre(*q));
            pz.total_cmp(&qz)
                .then((px * px + py * py).total_cmp(&(qx * qx + qy * qy)))
                .then(py.atan2(px).total_cmp(&qy.atan2(qx)))
                .then(p.cmp(q))
        });
        let f = design.fuel_ball_fraction;
        let fuel: Vec<SekerBallId> = balls
            .iter()
            .enumerate()
            .filter(|(n, _)| ((*n + 1) as f64 * f).floor() > (*n as f64 * f).floor())
            .map(|(_, id)| *id)
            .collect();
        let is_fuel = |id: SekerBallId| fuel.contains(&id);

        // Surfaces: 0..4 TRISO, 5/6 site 0's pair, 7..12 the prism, 13/14 the
        // slab, then the pairs of sites 1..22 (the core's layout, so site 0's
        // pair is 5/6 as `build_tile_universes` expects of any caller).
        let triso = TrisoBuild::new(design);
        let site_centres: Vec<[f64; 3]> = SekerSite::ALL.iter().map(|s| cell.site_centre(*s)).collect();
        let mut surfaces = triso.shells();
        surfaces.push(sphere(site_centres[0], design.fuel_zone_radius_cm));
        surfaces.push(sphere(site_centres[0], r_ball));
        for (nx, ny, d) in prism.faces() {
            surfaces.push(SurfaceKind::Plane(Plane {
                a: nx,
                b: ny,
                c: 0.0,
                d,
                bc: BoundaryType::Reflective,
            }));
        }
        surfaces.push(SurfaceKind::ZPlane(ZPlane {
            z0: -0.5 * h,
            bc: BoundaryType::Reflective,
        }));
        surfaces.push(SurfaceKind::ZPlane(ZPlane {
            z0: 0.5 * h,
            bc: BoundaryType::Reflective,
        }));
        let mut site_surfaces = vec![(5usize, 6usize); site_centres.len()];
        for (s, pair) in site_surfaces.iter_mut().enumerate().skip(1) {
            *pair = (surfaces.len(), surfaces.len() + 1);
            surfaces.push(sphere(site_centres[s], design.fuel_zone_radius_cm));
            surfaces.push(sphere(site_centres[s], r_ball));
        }
        let region = intersection((7..13).map(ins).chain([out(13), ins(14)]));

        let mut cells = vec![root_cell(region, CellFill::Lattice(0), majorant_index)];
        let triso_first = cells.len();
        cells.extend(TrisoBuild::cells(0));
        let [pu, mu] = TrisoBuild::universes(triso_first, TRISO_PARTICLE_UNIVERSE, TRISO_MATRIX_UNIVERSE);
        let mut universes = vec![
            Universe {
                id: 0,
                cell_indices: vec![0],
            },
            pu,
            mu,
        ];
        // Every ball present (nothing is rejected inside a cell); fuel by the
        // identities above. The all-absent (helium) key is the lattice outer,
        // as the core's for Şeker's bed.
        let all_present: u32 = (1u32 << SekerSite::ALL.len()) - 1;
        let tile_key = |a: i32, b: i32| -> (u32, u32) {
            let m = SekerSite::ALL.iter().enumerate().fold(0u32, |m, (s, site)| {
                m | (u32::from(is_fuel(seker_tile_ball(a, b, 0, *site))) << s)
            });
            (m, all_present)
        };
        let outer_key = (0u32, 0u32);
        let mut keys: Vec<(u32, u32)> = vec![outer_key];
        let tile_keys: Vec<(i32, i32, (u32, u32))> =
            tiles.iter().map(|&(a, b)| (a, b, tile_key(a, b))).collect();
        for &(_, _, k) in &tile_keys {
            if !keys.contains(&k) {
                keys.push(k);
            }
        }
        keys.sort_unstable();
        let key_universe = build_tile_universes(
            &keys,
            &site_centres,
            &site_surfaces,
            1,
            &mut cells,
            &mut universes,
        );
        let outer_universe = key_universe[&outer_key];
        let placeholder: Vec<Vec<Vec<usize>>> = vec![(0..n_cover)
            .rev()
            .map(|r| vec![outer_universe; if r == 0 { 1 } else { 6 * r }])
            .collect()];
        let mut bed = HexLattice::from_rings_3d(
            0,
            HexOrientation::Y,
            Position::ZERO,
            pitch,
            h,
            &placeholder,
            Some(outer_universe),
        );
        for &(a, b, key) in &tile_keys {
            let flat = bed.flat_index([a + nr - 1, b + nr - 1, 0]);
            bed.universes[flat] = key_universe[&key] as i32;
        }
        let geometry = Geometry {
            surfaces,
            cells,
            universes,
            lattices: vec![
                Lattice::Hex(bed),
                Lattice::Rect(triso.lattice(1, TRISO_PARTICLE_UNIVERSE, TRISO_MATRIX_UNIVERSE)),
            ],
            root_universe: 0,
        };
        let fuel_balls = fuel.len();
        let dummy_balls = balls.len() - fuel_balls;
        let balls = balls.iter().map(|&id| (centre(id), is_fuel(id))).collect();
        Self {
            kind: CellKind::SekerCluster { rings },
            geometry,
            half_extent: [prism.circumradius, prism.circumradius, 0.5 * h],
            volume_cm3: prism.area * h,
            triso_particles: triso.n_particles,
            fuel_balls,
            dummy_balls,
            balls,
        }
    }

    /// The bounding box, for the initial fission source (rejection-sampled
    /// into fuel by the driver).
    #[must_use]
    pub fn source_box(&self) -> SourceBox {
        let [x, y, z] = self.half_extent;
        SourceBox {
            lower: Position::new(-x, -y, -z),
            upper: Position::new(x, y, z),
        }
    }

    /// A 4 x 4 x 4 Shannon-entropy mesh over the bounding box.
    #[must_use]
    pub fn entropy_mesh(&self) -> RegularMesh {
        let [x, y, z] = self.half_extent;
        RegularMesh {
            lower_left: [-x, -y, -z],
            upper_right: [x, y, z],
            dimension: [4, 4, 4],
        }
    }
}

/// The superlattice `(i, j)` of `rings` (see the module docs).
fn superlattice(rings: usize) -> Option<(usize, usize)> {
    match rings {
        1 => Some((1, 0)),
        2 => Some((2, 1)),
        3 => Some((3, 2)),
        _ => None,
    }
}

/// The Wigner–Seitz hexagon of superlattice vector `i a1 + j a2`, centred on
/// the origin tile.
struct Prism {
    /// Unit normals and apothem of the six faces.
    normals: [(f64, f64); 6],
    apothem: f64,
    circumradius: f64,
    area: f64,
}

impl Prism {
    fn new(i: usize, j: usize, pitch: f64) -> Self {
        // `tile_xy(a, b, p) = [sqrt(3)/2 a p, (b + a/2) p]`: a1 = (a = 1),
        // a2 = (b = 1), 60 degrees apart.
        let [a1x, a1y] = tile_xy(1, 0, pitch);
        let [a2x, a2y] = tile_xy(0, 1, pitch);
        let (lx, ly) = (i as f64 * a1x + j as f64 * a2x, i as f64 * a1y + j as f64 * a2y);
        let len = lx.hypot(ly);
        let theta = ly.atan2(lx);
        let normals = std::array::from_fn(|k| {
            let t = theta + std::f64::consts::FRAC_PI_3 * k as f64;
            (t.cos(), t.sin())
        });
        let apothem = 0.5 * len;
        Self {
            normals,
            apothem,
            circumradius: apothem / (std::f64::consts::PI / 6.0).cos(),
            area: 2.0 * 3.0_f64.sqrt() * apothem * apothem,
        }
    }

    /// `(nx, ny, d)` of each face, inside where `nx x + ny y < d`.
    fn faces(&self) -> [(f64, f64, f64); 6] {
        self.normals.map(|(nx, ny)| (nx, ny, self.apothem))
    }

    /// Distance \[cm\] from `(x, y)` to the hexagon (0 inside).
    fn distance(&self, x: f64, y: f64) -> f64 {
        if self.normals.iter().all(|&(nx, ny)| nx * x + ny * y <= self.apothem) {
            return 0.0;
        }
        // Outside: the nearest point is on an edge; check every edge segment.
        let verts: Vec<(f64, f64)> = (0..6)
            .map(|k| {
                let (n0x, n0y) = self.normals[k];
                let t = n0y.atan2(n0x) + std::f64::consts::FRAC_PI_6;
                (self.circumradius * t.cos(), self.circumradius * t.sin())
            })
            .collect();
        (0..6)
            .map(|k| {
                let (ax, ay) = verts[(k + 5) % 6];
                let (bx, by) = verts[k];
                let (dx, dy) = (bx - ax, by - ay);
                let t = (((x - ax) * dx + (y - ay) * dy) / (dx * dx + dy * dy)).clamp(0.0, 1.0);
                (x - ax - t * dx).hypot(y - ay - t * dy)
            })
            .fold(f64::INFINITY, f64::min)
    }
}

/// Centre \[cm\] of a ball of a one-level Şeker lattice centred on `z = 0`
/// (bottom face `-h/2`), as `SekerBed::centre` computes it for the core.
fn ball_centre(cell: &SekerCell, id: SekerBallId) -> [f64; 3] {
    let h = cell.height;
    let (a, b, zc, site) = match id {
        SekerBallId::Flower { a, b, face, k } => {
            (a, b, -0.5 * h + f64::from(face) * h, SekerSite::BottomFlower(k))
        }
        SekerBallId::Central { a, b, level, k } => (
            a,
            b,
            f64::from(level) * h,
            if k < 3 {
                SekerSite::Inner(k)
            } else {
                SekerSite::Outer(k - 3)
            },
        ),
    };
    let [tx, ty] = tile_xy(a, b, cell.pitch());
    let [lx, ly, _] = cell.site_centre(site);
    [tx + lx, ty + ly, zc]
}

fn sphere(c: [f64; 3], r: f64) -> SurfaceKind {
    SurfaceKind::Sphere(Sphere {
        x0: c[0],
        y0: c[1],
        z0: c[2],
        r,
        bc: BoundaryType::Transmissive,
    })
}

/// The intersection of half-spaces, in RPN.
fn intersection(tokens: impl IntoIterator<Item = RegionToken>) -> Vec<RegionToken> {
    let mut v = Vec::new();
    for (n, t) in tokens.into_iter().enumerate() {
        v.push(t);
        if n > 0 {
            v.push(RegionToken::Intersection);
        }
    }
    v
}

fn root_cell(region: Vec<RegionToken>, fill: CellFill, majorant_index: usize) -> Cell {
    let c = Cell::fill(1, region, fill, Position::ZERO);
    if majorant_index == usize::MAX {
        c
    } else {
        c.delta_tracked(majorant_index)
    }
}

#[cfg(test)]
mod tests;
