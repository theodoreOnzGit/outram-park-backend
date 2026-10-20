// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK.
//
// OUTRAM PARK is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the
// Free Software Foundation, either version 3 of the License, or (at your
// option) any later version.
//
// OUTRAM PARK is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
// General Public License for more details.
//
// You should have received a copy of the GNU General Public License along
// with OUTRAM PARK.  If not, see <https://www.gnu.org/licenses/>.

//! # The HTTR start-up core as CSG
//!
//! [`assemble_httr`] builds the fully-loaded 30-column core of R1 (see
//! [`super::spec`]) from the inside out:
//!
//! | level | what | how |
//! |---|---|---|
//! | TRISO | kernel, four coatings, overcoat; matrix beyond | spheres, one universe per enrichment |
//! | compact | 0.5-1.3 cm annulus, 54.6 cm | a `RectLattice` 27 x 27 x 546 (+ 1 matrix layer above and below), **delta-tracked** |
//! | fuel rod | helium hole, compacts, gap, IG-110 sleeve, coolant annulus | cylinders and planes |
//! | block | 33- or 31-hole pin lattice, BP pins, graphite | a 4-ring `HexLattice` (flats perpendicular to x), one per block kind and column rotation |
//! | column | 9 blocks of 58 cm, or a control / instrumentation / reflector column | hexagonal prism (apothem 18 cm) in a 18.1 cm tile; helium gap |
//! | core | 61 columns | a 5-ring `HexLattice` (flats perpendicular to y), pitch 36.2 cm, PGX outside |
//! | model | PGX cylinder r 212.5 cm, `|z|` < 261 cm | vacuum beyond |
//!
//! **Rotation.** The CSG supports translations only, so each column
//! rotation of R1's deck (`*trcl`, R1 Fig. 3.16) is built as its own
//! universe: the pin lattice's tiles are filled by looking up each tile's
//! position rotated back into the unrotated block, and a control column's
//! holes are placed at their rotated angles. The TRISO lattice inside each
//! compact is not rotated: it stands in for a random distribution (R1
//! §3.1.1), and rotating its square pattern by 60° would change nothing
//! physical.
//!
//! **Delta tracking.** Only the compact annulus is delta-tracked, against
//! majorant 0 of the caller's table: the region that holds 181 818 particles
//! per rod. Everything else is surface-tracked, so the majorant covers
//! only the compact's materials (`materials::mat::COMPACT`), as HTR-10's
//! covers only its bed.

use petir::mathf::RealMath;
use std::f64::consts::PI;

use outram_mc_libs::geometry::cell::{Cell, CellFill};
use outram_mc_libs::geometry::geometry::Geometry;
use outram_mc_libs::geometry::lattice::{HexLattice, HexOrientation, Lattice, RectLattice, HEX_NONE};
use outram_mc_libs::geometry::position::Position;
use outram_mc_libs::geometry::surface::{BoundaryType, Plane, Sphere, SurfaceKind, ZCylinder, ZPlane};
use outram_mc_libs::geometry::universe::Universe;

use super::materials::mat;
use super::spec::{
    self, core_map, fuel_block_slot, BpLoading, ColumnRole, FuelLayer, FuelZone, PinSlot,
    RodBank, RodPositions,
};
use crate::htr10_rmc::reflector_geometry::Rgn;

/// Index of the compact's majorant in the transport driver's table.
pub const COMPACT_MAJORANT: usize = 0;

/// What [`assemble_httr`] returns.
#[derive(Debug, Clone)]
pub struct AssembledHttr {
    /// The model.
    pub geometry: Geometry,
    /// The rod positions it was built with.
    pub rods: RodPositions,
    /// TRISO particles per fuel rod as built (counted from the lattice).
    pub particles_per_rod: usize,
    /// Fuel rods in the core as built.
    pub fuel_rods: usize,
}

/// The TRISO-region tracking method.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CompactTracking {
    /// Delta tracking inside the compact annulus (default).
    #[default]
    Delta,
    /// ABLATION: surface tracking everywhere.
    Surface,
}

/// Surfaces, cells, universes and lattices under construction.
#[derive(Default)]
struct B {
    surfaces: Vec<SurfaceKind>,
    cells: Vec<Cell>,
    universes: Vec<Universe>,
    lattices: Vec<Lattice>,
}

const T: f64 = 300.0;

impl B {
    fn surf(&mut self, s: SurfaceKind) -> usize {
        self.surfaces.push(s);
        self.surfaces.len() - 1
    }
    fn zcyl(&mut self, x0: f64, y0: f64, r: f64) -> usize {
        self.surf(SurfaceKind::ZCylinder(ZCylinder {
            x0,
            y0,
            r,
            bc: BoundaryType::Transmissive,
        }))
    }
    fn zplane(&mut self, z0: f64, bc: BoundaryType) -> usize {
        self.surf(SurfaceKind::ZPlane(ZPlane { z0, bc }))
    }
    fn sphere(&mut self, r: f64) -> usize {
        self.surf(SurfaceKind::Sphere(Sphere {
            x0: 0.0,
            y0: 0.0,
            z0: 0.0,
            r,
            bc: BoundaryType::Transmissive,
        }))
    }
    /// The six planes of a hexagonal prism of apothem `a` with flats
    /// perpendicular to y (outward normals at 30, 90, ..., 330 degrees);
    /// the inside of all six is the hexagon.
    fn hexagon(&mut self, a: f64) -> [usize; 6] {
        let mut out = [0; 6];
        for (k, o) in out.iter_mut().enumerate() {
            let phi = (30.0 + 60.0 * k as f64) * PI / 180.0;
            *o = self.surf(SurfaceKind::Plane(Plane {
                a: phi.r_cos(),
                b: phi.r_sin(),
                c: 0.0,
                d: a,
                bc: BoundaryType::Transmissive,
            }));
        }
        out
    }
    fn cell(&mut self, id: i32, r: Rgn, fill: CellFill, translation: Position) -> usize {
        let c = match fill {
            CellFill::Material(m) => Cell::material(id, r.0, m, T),
            f => Cell::fill(id, r.0, f, translation),
        };
        self.cells.push(c);
        self.cells.len() - 1
    }
    fn mat_cell(&mut self, id: i32, r: Rgn, m: usize) -> usize {
        self.cell(id, r, CellFill::Material(m), Position::ZERO)
    }
    fn universe(&mut self, cells: Vec<usize>) -> usize {
        let id = self.universes.len() as i32 + 1;
        self.universes.push(Universe {
            id,
            cell_indices: cells,
        });
        self.universes.len() - 1
    }
    fn lattice(&mut self, l: Lattice) -> usize {
        self.lattices.push(l);
        self.lattices.len() - 1
    }
    /// A universe that is material `m` everywhere: two cells, either side of
    /// surface `s` (a cell region cannot be "everything").
    fn uniform(&mut self, s: usize, m: usize) -> usize {
        let a = self.mat_cell(0, Rgn::ins(s), m);
        let b = self.mat_cell(0, Rgn::out(s), m);
        self.universe(vec![a, b])
    }
}

/// `ins(s[0]) and ... and ins(s[5])`: the inside of a hexagon.
fn hex_in(h: [usize; 6]) -> Rgn {
    h[1..]
        .iter()
        .fold(Rgn::ins(h[0]), |r, &s| r.and(Rgn::ins(s)))
}

/// Inside cylinder `c` and between planes `lo` and `hi`.
fn can(c: usize, lo: usize, hi: usize) -> Rgn {
    Rgn::ins(c).and(Rgn::out(lo)).and(Rgn::ins(hi))
}

/// Block-lattice index `(i, j)` (the deck's `lat=2`, `a1 = (5.15, 0)`,
/// `a2 = 5.15 (cos 60, sin 60)`) of the point `(x, y)`, rounded.
fn pin_index(x: f64, y: f64) -> (i32, i32) {
    let p = spec::PIN_PITCH;
    let j = y / (p * 3.0_f64.sqrt() / 2.0);
    let i = (x - j * p / 2.0) / p;
    (i.round() as i32, j.round() as i32)
}

/// `(x, y)` rotated by `deg` degrees.
fn rotate(x: f64, y: f64, deg: f64) -> (f64, f64) {
    let (s, c) = (deg * PI / 180.0).sin_cos();
    (c * x - s * y, s * x + c * y)
}

/// A block's pin-lattice contents, by kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum BlockKind {
    /// A fuel block of enrichment index `e` and BP loading `bp`.
    Fuel { e: usize, bp: BpLoading, holes_31: bool },
    /// A reflector block with coolant channels at the fuel positions.
    Reflector { holes_31: bool },
}

/// Pin universes shared by every block.
struct Pins {
    fuel: [usize; 12],
    bp: [usize; 2],
    empty_bp: usize,
    graphite: usize,
    channel: usize,
}

/// Build the HTTR fully-loaded core with its rods at `rods`.
///
/// # Panics
/// If the built TRISO lattice does not hold [`spec::TRISO_PER_LAYER`]
/// particles per layer, or the core map is not 61 columns (both checked
/// against the spec, as the HTR-10 model checks its own counts, gh:#316).
#[must_use]
pub fn assemble_httr(rods: RodPositions, tracking: CompactTracking) -> AssembledHttr {
    let mut b = B::default();
    let vac = BoundaryType::Vacuum;
    let tr = BoundaryType::Transmissive;

    // --- Root boundary: these three surfaces carry the vacuum condition
    // and are reused wherever the core's top and bottom are needed.
    let pr = b.surf(SurfaceKind::ZCylinder(ZCylinder {
        x0: 0.0,
        y0: 0.0,
        r: spec::PERMANENT_REFLECTOR_R,
        bc: vac,
    }));
    let z_bot = b.zplane(-spec::CORE_HALF_HEIGHT, vac);
    let z_top = b.zplane(spec::CORE_HALF_HEIGHT, vac);

    // --- TRISO particle (R1 Fig. 3.1).
    let sph: Vec<usize> = spec::TRISO_RADII_CM.iter().map(|&r| b.sphere(r)).collect();
    let coat = [mat::BUFFER, mat::IPYC, mat::SIC, mat::OPYC, mat::OVERCOAT];
    let matrix_u = b.uniform(sph[5], mat::MATRIX);
    let mut particle_u = [0usize; 12];
    for (e, pu) in particle_u.iter_mut().enumerate() {
        let mut cells = vec![b.mat_cell(1, Rgn::ins(sph[0]), mat::KERNEL0 + e)];
        for (k, &m) in coat.iter().enumerate() {
            cells.push(b.mat_cell(2 + k as i32, Rgn::out(sph[k]).and(Rgn::ins(sph[k + 1])), m));
        }
        cells.push(b.mat_cell(7, Rgn::out(sph[5]), mat::MATRIX));
        *pu = b.universe(cells);
    }

    // --- The compact's TRISO lattice: the deck's layer pattern, 546 layers,
    // plus one layer of matrix above and below so the lattice covers the
    // annulus it fills with a margin (an uncovered sliver gives negative
    // distances, as HTR-10 found).
    let [px, py, pz] = spec::TRISO_PITCH_CM;
    let nz = spec::TRISO_LAYERS + 2;
    let mut per_layer = 0;
    let layer_is_particle: Vec<bool> = spec::TRISO_LAYER
        .iter()
        .flat_map(|row| row.bytes().map(|c| c == b'o'))
        .collect();
    for &p in &layer_is_particle {
        per_layer += usize::from(p);
    }
    assert_eq!(per_layer, spec::TRISO_PER_LAYER, "the deck's layer pattern");
    let mut triso_lattice = [0usize; 12];
    for (e, tl) in triso_lattice.iter_mut().enumerate() {
        let mut v = Vec::with_capacity(27 * 27 * nz);
        for k in 0..nz {
            let inner = k > 0 && k < nz - 1;
            v.extend(
                layer_is_particle
                    .iter()
                    .map(|&p| if p && inner { particle_u[e] } else { matrix_u }),
            );
        }
        *tl = b.lattice(Lattice::Rect(RectLattice {
            id: 100 + e as i32,
            n: [27, 27, nz],
            lower_left: Position::new(
                -13.5 * px,
                -13.5 * py,
                -spec::COMPACT_HALF_HEIGHT - pz,
            ),
            pitch: [px, py, pz],
            universes: v,
            outer: Some(matrix_u),
        }));
    }

    // --- Fuel rod (R1 Fig. 3.3; deck cells 185-309).
    let c_hole = b.zcyl(0.0, 0.0, spec::COMPACT_INNER_R);
    let c_comp = b.zcyl(0.0, 0.0, spec::COMPACT_OUTER_R);
    let c_slv_in = b.zcyl(0.0, 0.0, spec::SLEEVE_INNER_R);
    let c_slv_out = b.zcyl(0.0, 0.0, spec::SLEEVE_OUTER_R);
    let c_chan = b.zcyl(0.0, 0.0, spec::FUEL_HOLE_R);
    let zc = (b.zplane(-spec::COMPACT_HALF_HEIGHT, tr), b.zplane(spec::COMPACT_HALF_HEIGHT, tr));
    let zs_in = (
        b.zplane(-spec::SLEEVE_INNER_HALF_HEIGHT, tr),
        b.zplane(spec::SLEEVE_INNER_HALF_HEIGHT, tr),
    );
    let zs_out = (
        b.zplane(-spec::SLEEVE_HALF_HEIGHT, tr),
        b.zplane(spec::SLEEVE_HALF_HEIGHT, tr),
    );
    let mut fuel_pin = [0usize; 12];
    for (e, fp) in fuel_pin.iter_mut().enumerate() {
        let compact_region = Rgn::out(c_hole).and(can(c_comp, zc.0, zc.1));
        let compact = b.cell(
            10,
            compact_region,
            CellFill::Lattice(triso_lattice[e]),
            Position::ZERO,
        );
        if tracking == CompactTracking::Delta {
            let c = b.cells[compact].clone().delta_tracked(COMPACT_MAJORANT);
            b.cells[compact] = c;
        }
        let hole = b.mat_cell(11, can(c_hole, zc.0, zc.1), mat::HELIUM);
        let gap = b.mat_cell(
            12,
            can(c_slv_in, zs_in.0, zs_in.1).and(can(c_comp, zc.0, zc.1).not()),
            mat::HELIUM,
        );
        let sleeve = b.mat_cell(
            13,
            can(c_slv_out, zs_out.0, zs_out.1).and(can(c_slv_in, zs_in.0, zs_in.1).not()),
            mat::SLEEVE,
        );
        let coolant = b.mat_cell(
            14,
            Rgn::ins(c_chan).and(can(c_slv_out, zs_out.0, zs_out.1).not()),
            mat::HELIUM,
        );
        let block = b.mat_cell(15, Rgn::out(c_chan), mat::BLOCK);
        *fp = b.universe(vec![compact, hole, gap, sleeve, coolant, block]);
    }

    // --- Burnable-poison pins (R1 Figs. 3.4-3.5; deck cells 351-362).
    let c_bp = b.zcyl(0.0, 0.0, spec::BP_R);
    let c_bph = b.zcyl(0.0, 0.0, spec::BP_HOLE_R);
    let zb: Vec<usize> = spec::BP_Z.iter().map(|&z| b.zplane(z, tr)).collect();
    let bp_hole = || can(c_bph, zb[0], zb[3]);
    let mut bp = [0usize; 2];
    for (k, m) in [mat::BP_2_0, mat::BP_2_5].into_iter().enumerate() {
        let lo = b.mat_cell(20, can(c_bp, zb[0], zb[1]), m);
        let disks = b.mat_cell(21, can(c_bp, zb[1], zb[2]), mat::DISKS);
        let hi = b.mat_cell(22, can(c_bp, zb[2], zb[3]), m);
        let gap = b.mat_cell(23, Rgn::out(c_bp).and(bp_hole()), mat::HELIUM);
        let blk = b.mat_cell(24, bp_hole().not(), mat::BLOCK);
        bp[k] = b.universe(vec![lo, disks, hi, gap, blk]);
    }
    let empty_bp = {
        let he = b.mat_cell(25, bp_hole(), mat::HELIUM);
        let blk = b.mat_cell(26, bp_hole().not(), mat::BLOCK);
        b.universe(vec![he, blk])
    };
    let graphite = b.uniform(c_bph, mat::BLOCK);
    let c_rch = b.zcyl(0.0, 0.0, spec::REFLECTOR_CHANNEL_R);
    let channel = {
        let he = b.mat_cell(27, Rgn::ins(c_rch), mat::HELIUM);
        let blk = b.mat_cell(28, Rgn::out(c_rch), mat::BLOCK);
        b.universe(vec![he, blk])
    };
    let pins = Pins {
        fuel: fuel_pin,
        bp,
        empty_bp,
        graphite,
        channel,
    };

    // --- Column-level surfaces, shared by every column (tile-local frame).
    let hex = b.hexagon(spec::BLOCK_APOTHEM);
    let mut z_layer = vec![z_bot];
    for m in 1..spec::LAYERS_PER_COLUMN {
        let z = -spec::CORE_HALF_HEIGHT + spec::BLOCK_HEIGHT * f64::from(m);
        z_layer.push(b.zplane(z, tr));
    }
    z_layer.push(z_top);

    // Block pin lattices, built once per (kind, rotation).
    let mut block_lattices: Vec<((BlockKind, i32), usize)> = Vec::new();
    let mut block_lattice = |b: &mut B, kind: BlockKind, rot: f64| -> usize {
        let key = (kind, rot.round() as i32);
        if let Some(&(_, l)) = block_lattices.iter().find(|(k, _)| *k == key) {
            return l;
        }
        let l = build_block_lattice(b, kind, rot, &pins);
        block_lattices.push((key, l));
        l
    };

    // Control-rod holes: six possible centres at 60-degree steps.
    let hole_cyl: Vec<usize> = (0..6)
        .map(|k| {
            let a = f64::from(k) * 60.0 * PI / 180.0;
            b.zcyl(
                spec::CR_HOLE_OFFSET * a.r_cos(),
                spec::CR_HOLE_OFFSET * a.r_sin(),
                spec::CR_HOLE_R,
            )
        })
        .collect();
    let z_hole = b.zplane(spec::CR_HOLE_BOTTOM_Z, tr);
    let hole_at = |deg: f64| -> (usize, f64, f64) {
        let k = ((deg.rem_euclid(360.0) / 60.0).round() as usize) % 6;
        let a = k as f64 * 60.0 * PI / 180.0;
        (k, spec::CR_HOLE_OFFSET * a.r_cos(), spec::CR_HOLE_OFFSET * a.r_sin())
    };
    let rod_u = build_rod(&mut b);

    // --- Columns.
    let pgx_u = b.uniform(pr, mat::PGX);
    let mut column_universes: Vec<(usize, i32, i32)> = Vec::new(); // (u, i, j)
    let mut fuel_rods = 0usize;
    let map = core_map();
    assert_eq!(map.len(), 61, "the HTTR core has 61 columns");
    for col in &map {
        let rot = col.rotation_deg;
        let mut cells = Vec::new();
        match col.role {
            ColumnRole::Fuel(zone) => {
                for (m, k) in (-4..=4).enumerate() {
                    let kind = match FuelLayer::at(k) {
                        Some(layer) => BlockKind::Fuel {
                            e: zone.enrichment_index(layer),
                            bp: layer.bp(),
                            holes_31: zone.has_31_holes(),
                        },
                        // The deck's reflector blocks: 33 holes at the bottom
                        // of every column; 31 at the top of zones 3-4 (deck
                        // cells 503-504; R1 §3.1.2.9's text says 31 top and
                        // bottom for zones 3-4 -- the deck is followed).
                        None => BlockKind::Reflector {
                            holes_31: k > 0 && zone.has_31_holes(),
                        },
                    };
                    if let BlockKind::Fuel { holes_31, .. } = kind {
                        fuel_rods += if holes_31 { 31 } else { 33 };
                    }
                    let l = block_lattice(&mut b, kind, rot);
                    let region = hex_in(hex)
                        .and(Rgn::out(z_layer[m]))
                        .and(Rgn::ins(z_layer[m + 1]));
                    let z = spec::BLOCK_HEIGHT * f64::from(k);
                    cells.push(b.cell(30, region, CellFill::Lattice(l), Position::new(0.0, 0.0, z)));
                }
            }
            ColumnRole::Control(bank) => {
                let zb = spec::FUEL_BOTTOM_Z + rods.of(bank);
                let mut holes = Vec::new();
                for (h, &a) in spec::CR_HOLE_ANGLES_DEG.iter().enumerate() {
                    let (k, x, y) = hole_at(a + rot);
                    let region = Rgn::ins(hole_cyl[k]).and(Rgn::out(z_hole));
                    holes.push(k);
                    cells.push(if h < 2 {
                        b.cell(31, region, CellFill::Universe(rod_u), Position::new(x, y, zb))
                    } else {
                        b.mat_cell(32, region, mat::HELIUM)
                    });
                }
                cells.push(graphite_with_holes(&mut b, hex, &holes, &hole_cyl, z_hole));
            }
            ColumnRole::Instrumentation => {
                let mut holes = Vec::new();
                for &a in &spec::CR_HOLE_ANGLES_DEG {
                    let (k, _, _) = hole_at(a + rot);
                    holes.push(k);
                    let region = Rgn::ins(hole_cyl[k]).and(Rgn::out(z_hole));
                    cells.push(b.mat_cell(33, region, mat::HELIUM));
                }
                cells.push(graphite_with_holes(&mut b, hex, &holes, &hole_cyl, z_hole));
            }
            ColumnRole::Reflector => {
                cells.push(b.mat_cell(34, hex_in(hex), mat::BLOCK));
            }
        }
        cells.push(b.mat_cell(35, hex_in(hex).not(), mat::HELIUM));
        let u = b.universe(cells);
        column_universes.push((u, col.i, col.j));
    }

    // --- Core lattice: tiles filled by position (deck core map, cell 701).
    let core = {
        let n_rings = 5usize;
        let n_side = 2 * n_rings - 1;
        let mut lat = HexLattice {
            id: 1,
            orientation: HexOrientation::Y,
            n_rings,
            n_axial: 1,
            center: Position::ZERO,
            pitch: [spec::COLUMN_PITCH, 0.0],
            universes: vec![HEX_NONE; n_side * n_side],
            outer: Some(pgx_u),
        };
        let mut placed = 0;
        for iy in 0..n_side as i32 {
            for ix in 0..n_side as i32 {
                let idx = [ix, iy, 0];
                if !lat.are_valid_indices(idx) {
                    continue;
                }
                let c = lat.tile_center(idx);
                let (i, j) = column_index(c.x, c.y);
                let u = column_universes
                    .iter()
                    .find(|&&(_, ci, cj)| (ci, cj) == (i, j))
                    .map(|&(u, _, _)| u)
                    .unwrap_or_else(|| panic!("no column at deck index ({i}, {j})"));
                let f = lat.flat_index(idx);
                lat.universes[f] = u as i32;
                placed += 1;
            }
        }
        assert_eq!(placed, 61, "every tile of the 5-ring core lattice is a column");
        b.lattice(Lattice::Hex(lat))
    };

    // --- Root: the core lattice inside the permanent-reflector cylinder.
    let root_cell = b.cell(
        1,
        can(pr, z_bot, z_top),
        CellFill::Lattice(core),
        Position::ZERO,
    );
    let root = b.universe(vec![root_cell]);

    let B {
        surfaces,
        cells,
        universes,
        lattices,
    } = b;
    AssembledHttr {
        geometry: Geometry {
            surfaces,
            cells,
            universes,
            lattices,
            root_universe: root,
        },
        rods,
        particles_per_rod: per_layer * spec::TRISO_LAYERS,
        fuel_rods,
    }
}

/// The deck core-lattice index `(i, j)` (`a1 = (0, 36.2)`,
/// `a2 = 36.2 (cos 150, sin 150)`) of the point `(x, y)`, rounded.
fn column_index(x: f64, y: f64) -> (i32, i32) {
    let p = spec::COLUMN_PITCH;
    let j = x / (p * (150.0 * PI / 180.0).r_cos());
    let i = (y - j * p * (150.0 * PI / 180.0).r_sin()) / p;
    (i.round() as i32, j.round() as i32)
}

/// Graphite of a holed column: inside the hexagon, outside each hole.
fn graphite_with_holes(b: &mut B, hex: [usize; 6], holes: &[usize], cyl: &[usize], z_hole: usize) -> usize {
    let mut r = hex_in(hex);
    for &k in holes {
        r = r.and(Rgn::ins(cyl[k]).and(Rgn::out(z_hole)).not());
    }
    b.mat_cell(36, r, mat::BLOCK)
}

/// A block's pin lattice for `kind`, rotated by `rot` degrees.
fn build_block_lattice(b: &mut B, kind: BlockKind, rot: f64, pins: &Pins) -> usize {
    let n_rings = 4usize;
    let n_side = 2 * n_rings - 1;
    let mut lat = HexLattice {
        id: 10,
        orientation: HexOrientation::X,
        n_rings,
        n_axial: 1,
        center: Position::ZERO,
        pitch: [spec::PIN_PITCH, 0.0],
        universes: vec![HEX_NONE; n_side * n_side],
        outer: Some(pins.graphite),
    };
    for iy in 0..n_side as i32 {
        for ix in 0..n_side as i32 {
            let idx = [ix, iy, 0];
            if !lat.are_valid_indices(idx) {
                continue;
            }
            let c = lat.tile_center(idx);
            let (x0, y0) = rotate(c.x, c.y, -rot);
            let (i, j) = pin_index(x0, y0);
            let holes_31 = match kind {
                BlockKind::Fuel { holes_31, .. } | BlockKind::Reflector { holes_31 } => holes_31,
            };
            let slot = fuel_block_slot(i, j, holes_31).expect("a 4-ring lattice is within 3 rings");
            let u = match (kind, slot) {
                (_, PinSlot::Graphite) => pins.graphite,
                (BlockKind::Fuel { e, .. }, PinSlot::Fuel) => pins.fuel[e],
                (BlockKind::Fuel { bp, .. }, PinSlot::Bp) => match bp {
                    BpLoading::Two => pins.bp[0],
                    BpLoading::TwoAndHalf => pins.bp[1],
                },
                (BlockKind::Fuel { .. }, PinSlot::EmptyBp) => pins.empty_bp,
                // Reflector blocks: channels at the fuel positions only; the
                // BP positions are solid (deck cells 485-486).
                (BlockKind::Reflector { .. }, PinSlot::Fuel) => pins.channel,
                (BlockKind::Reflector { .. }, PinSlot::Bp | PinSlot::EmptyBp) => pins.graphite,
            };
            let f = lat.flat_index(idx);
            lat.universes[f] = u as i32;
        }
    }
    b.lattice(Lattice::Hex(lat))
}

/// A control rod in its own frame: axis on the origin, bottom at `z = 0`,
/// ten 31 cm sections (R1 Figs. 3.7-3.8; deck cells 601-609). Outside the
/// rod is helium (the rest of its hole).
fn build_rod(b: &mut B) -> usize {
    let tr = BoundaryType::Transmissive;
    let r = [
        spec::ROD_SPINE_R,
        spec::ROD_INNER_CLAD_IN_R,
        spec::ROD_ABSORBER_IN_R,
        spec::ROD_ABSORBER_OUT_R,
        spec::ROD_OUTER_R,
    ];
    let c: Vec<usize> = r.iter().map(|&r| b.zcyl(0.0, 0.0, r)).collect();
    // Radial materials of a section body, inside out.
    let body = [mat::ALLOY_800H, mat::HELIUM, mat::ALLOY_800H, mat::ABSORBER, mat::ALLOY_800H];
    let mut cells = Vec::new();
    let mut top = b.zplane(0.0, tr);
    let bottom = top;
    for m in 0..spec::ROD_SECTIONS {
        let z0 = spec::ROD_SECTION * m as f64;
        let lo = top;
        let p1 = b.zplane(z0 + spec::ROD_END_PLATE, tr);
        let p2 = b.zplane(z0 + spec::ROD_SECTION - spec::ROD_END_PLATE, tr);
        top = b.zplane(z0 + spec::ROD_SECTION, tr);
        cells.push(b.mat_cell(40, can(c[4], lo, p1), mat::ALLOY_800H));
        cells.push(b.mat_cell(40, can(c[4], p2, top), mat::ALLOY_800H));
        for (k, &m) in body.iter().enumerate() {
            let ring = if k == 0 {
                can(c[0], p1, p2)
            } else {
                Rgn::out(c[k - 1]).and(can(c[k], p1, p2))
            };
            cells.push(b.mat_cell(41 + k as i32, ring, m));
        }
    }
    cells.push(b.mat_cell(46, can(c[4], bottom, top).not(), mat::HELIUM));
    b.universe(cells)
}

/// Every fuel zone, for iteration in tests and reports.
pub const ZONES: [FuelZone; 4] = [FuelZone::Z1, FuelZone::Z2, FuelZone::Z3, FuelZone::Z4];

/// Every rod bank.
pub const BANKS: [RodBank; 4] = [RodBank::C, RodBank::R1, RodBank::R2, RodBank::R3];
