// SPDX-License-Identifier: GPL-3.0
//
// Ported from OpenMC (MIT), commit d7d3284a1 (0.16.1.dev25):
//   openmc/model/model.py:69-75      _check_pixels
//   openmc/model/model.py:1114-1148  Model._set_plot_defaults
//   openmc/model/model.py:1345-1543  Model.plot
//   openmc/plots.py:22-172           _BASIS_INDICES, _SVG_COLORS
//   openmc/plots.py:360-437          _id_map_to_rgb
//   openmc/plots.py:626-655          SlicePlot.colorize
//   openmc/universe.py:207-255,335-341,435-441  get_all_cells/materials, plot, bounding_box
//   openmc/cell.py:373-377,448-494,596-606      bounding_box, get_all_*, plot
//   openmc/lattice.py:110-136,161-207           get_unique_universes, get_all_*
//   openmc/geometry.py:70-71,753-770            bounding_box, plot
//   openmc/region.py:348-362,451-455,542-547,615-616  plot, bounding boxes
//   openmc/surface.py:243-266,537-581,1449-1655,1742-1760,2424-2583,2679-2680
//                                    Surface/Halfspace bounding_box
//   openmc/bounding_box.py:57-202    BoundingBox (&, |, center, width, infinite)
//   src/plot.cpp:47-79               IdData::set_value (id-map channel contents)
// Copyright (c) 2011-2026 Massachusetts Institute of Technology, UChicago
// Argonne LLC, and OpenMC contributors. MIT notice in
// verification_and_validation/geometry_plotting/openmc_inputs/LICENSE.openmc.
// Moved here from outram-mc-libs (src/geometry/plot/) on 2026-10-02,
// GitHub issue #486: the plotter draws the CSG description this crate owns,
// through the same locator transport uses. outram-mc-libs re-exports it as
// `outram_mc_libs::geometry::plot`.

//! Bounding boxes and domain order for [`super::ModelPlot`] (split out of
//! the model-plot module on 2026-10-02 to respect the 1000-line file cap).

use crate::csg::cell::{Cell, CellFill, HalfSpaceSense, RegionToken};
use crate::csg::geometry::Geometry;
use crate::csg::lattice::Lattice;
use crate::csg::surface::SurfaceKind;
#[allow(unused_imports)]
use crate::csg::position::Position;

// ---------------------------------------------------------------------------
// Bounding boxes (openmc/bounding_box.py, surface.py, region.py)
// ---------------------------------------------------------------------------

/// Axis-aligned bounding box. Port of `openmc.BoundingBox`
/// (`openmc/bounding_box.py`); infinite extents are `±f64::INFINITY`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoundingBox {
    /// Lower-left corner \[cm\].
    pub lower_left: [f64; 3],
    /// Upper-right corner \[cm\].
    pub upper_right: [f64; 3],
}

impl BoundingBox {
    /// `BoundingBox.infinite()` (`bounding_box.py:193-202`).
    #[must_use]
    pub const fn infinite() -> Self {
        Self {
            lower_left: [f64::NEG_INFINITY; 3],
            upper_right: [f64::INFINITY; 3],
        }
    }

    /// The empty box a `Union` starts from (`region.py:543-544`).
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            lower_left: [f64::INFINITY; 3],
            upper_right: [f64::NEG_INFINITY; 3],
        }
    }

    /// `self & other` (`bounding_box.py:57-76`): element-wise max / min.
    /// `np.maximum`/`np.minimum` propagate NaN; so does this.
    #[must_use]
    pub fn and(self, other: Self) -> Self {
        let mut b = self;
        for k in 0..3 {
            b.lower_left[k] = np_max(self.lower_left[k], other.lower_left[k]);
            b.upper_right[k] = np_min(self.upper_right[k], other.upper_right[k]);
        }
        b
    }

    /// `self | other` (`bounding_box.py:78-97`).
    #[must_use]
    pub fn or(self, other: Self) -> Self {
        let mut b = self;
        for k in 0..3 {
            b.lower_left[k] = np_min(self.lower_left[k], other.lower_left[k]);
            b.upper_right[k] = np_max(self.upper_right[k], other.upper_right[k]);
        }
        b
    }

    /// `BoundingBox.center` (`bounding_box.py:118-119`), `(ll + ur) / 2`.
    #[must_use]
    pub fn center(&self) -> [f64; 3] {
        [0, 1, 2].map(|k| (self.lower_left[k] + self.upper_right[k]) / 2.0)
    }

    /// `BoundingBox.width` (`bounding_box.py:167-168`).
    #[must_use]
    pub fn width(&self) -> [f64; 3] {
        [0, 1, 2].map(|k| self.upper_right[k] - self.lower_left[k])
    }
}

pub(crate) fn np_max(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        f64::NAN
    } else {
        a.max(b)
    }
}

pub(crate) fn np_min(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        f64::NAN
    } else {
        a.min(b)
    }
}

/// `np.isclose(a, b, rtol=0, atol=1e-12)` — `Surface._atol` (`surface.py:162`).
pub(crate) fn close(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1.0e-12
}

/// Axis-aligned-plane half-space box (`PlaneMixin.bounding_box`,
/// `surface.py:537-581`) for base coefficients `a x + b y + c z = d`.
pub(crate) fn plane_box(a: f64, b: f64, c: f64, d: f64, negative: bool) -> BoundingBox {
    let norm = (a * a + b * b + c * c).sqrt();
    let nhat = [a / norm, b / norm, c / norm];
    let mut bb = BoundingBox::infinite();
    if nhat.iter().any(|n| close(n.abs(), 1.0)) {
        let sign = nhat[0] + nhat[1] + nhat[2];
        let vals = [a, b, c].map(|v| if close(v, 0.0) { f64::NAN } else { d / v });
        let fill = |inf: f64| vals.map(|v| if v.is_nan() { inf } else { v });
        if negative == (sign > 0.0) {
            bb.upper_right = fill(f64::INFINITY);
        } else {
            bb.lower_left = fill(f64::NEG_INFINITY);
        }
    }
    bb
}

/// `Surface.bounding_box(side)` for every surface kind of this crate.
/// `negative` is `side == '-'` (the [`HalfSpaceSense::Inside`] half-space).
///
/// Cones, general quadrics and the `+` side of every closed surface are
/// infinite, as upstream (`surface.py:243-266`).
#[must_use]
pub fn surface_bounding_box(s: &SurfaceKind, negative: bool) -> BoundingBox {
    let bbox = |ll: [f64; 3], ur: [f64; 3]| {
        if negative {
            BoundingBox {
                lower_left: ll,
                upper_right: ur,
            }
        } else {
            BoundingBox::infinite()
        }
    };
    let (ni, pi) = (f64::NEG_INFINITY, f64::INFINITY);
    match s {
        SurfaceKind::XPlane(p) => plane_box(1.0, 0.0, 0.0, p.x0, negative),
        SurfaceKind::YPlane(p) => plane_box(0.0, 1.0, 0.0, p.y0, negative),
        SurfaceKind::ZPlane(p) => plane_box(0.0, 0.0, 1.0, p.z0, negative),
        SurfaceKind::Plane(p) => plane_box(p.a, p.b, p.c, p.d, negative),
        SurfaceKind::Sphere(s) => bbox(
            [s.x0 - s.r, s.y0 - s.r, s.z0 - s.r],
            [s.x0 + s.r, s.y0 + s.r, s.z0 + s.r],
        ),
        SurfaceKind::XCylinder(c) => {
            bbox([ni, c.y0 - c.r, c.z0 - c.r], [pi, c.y0 + c.r, c.z0 + c.r])
        }
        SurfaceKind::YCylinder(c) => {
            bbox([c.x0 - c.r, ni, c.z0 - c.r], [c.x0 + c.r, pi, c.z0 + c.r])
        }
        SurfaceKind::ZCylinder(c) => {
            bbox([c.x0 - c.r, c.y0 - c.r, ni], [c.x0 + c.r, c.y0 + c.r, pi])
        }
        SurfaceKind::XTorus(t) => bbox(
            [t.x0 - t.b, t.y0 - t.a - t.c, t.z0 - t.a - t.c],
            [t.x0 + t.b, t.y0 + t.a + t.c, t.z0 + t.a + t.c],
        ),
        SurfaceKind::YTorus(t) => bbox(
            [t.x0 - t.a - t.c, t.y0 - t.b, t.z0 - t.a - t.c],
            [t.x0 + t.a + t.c, t.y0 + t.b, t.z0 + t.a + t.c],
        ),
        SurfaceKind::ZTorus(t) => bbox(
            [t.x0 - t.a - t.c, t.y0 - t.a - t.c, t.z0 - t.b],
            [t.x0 + t.a + t.c, t.y0 + t.a + t.c, t.z0 + t.b],
        ),
        SurfaceKind::XCone(_)
        | SurfaceKind::YCone(_)
        | SurfaceKind::ZCone(_)
        | SurfaceKind::Quadric(_) => BoundingBox::infinite(),
    }
}

/// A region's RPN, rebuilt as a flat expression tree (indices, no `Box`).
pub(crate) enum Node {
    Half(usize, HalfSpaceSense),
    And(usize, usize),
    Or(usize, usize),
    Not(usize),
}

pub(crate) fn region_tree(region: &[RegionToken]) -> Option<(Vec<Node>, usize)> {
    let mut nodes = Vec::new();
    let mut stack: Vec<usize> = Vec::new();
    for t in region {
        let n = match *t {
            RegionToken::HalfSpace { surface_idx, sense } => Node::Half(surface_idx, sense),
            RegionToken::Intersection => {
                let b = stack.pop()?;
                Node::And(stack.pop()?, b)
            }
            RegionToken::Union => {
                let b = stack.pop()?;
                Node::Or(stack.pop()?, b)
            }
            RegionToken::Complement => Node::Not(stack.pop()?),
        };
        nodes.push(n);
        stack.push(nodes.len() - 1);
    }
    let root = stack.pop()?;
    Some((nodes, root))
}

/// `Region.bounding_box`. `Complement` is `(~node).bounding_box` (De Morgan,
/// `region.py:412-413,503-504,598-599,615-616`), carried here as `negate`.
pub(crate) fn node_box(geom: &Geometry, nodes: &[Node], i: usize, negate: bool) -> BoundingBox {
    match nodes[i] {
        Node::Half(s, sense) => {
            let negative = matches!(sense, HalfSpaceSense::Inside) != negate;
            surface_bounding_box(&geom.surfaces[s], negative)
        }
        Node::And(a, b) | Node::Or(a, b) => {
            let intersect = matches!(nodes[i], Node::And(..)) != negate;
            let (ba, bb) = (
                node_box(geom, nodes, a, negate),
                node_box(geom, nodes, b, negate),
            );
            // Intersection starts from infinite and `&=`s each node; Union
            // starts from the empty box and `|=`s (region.py:451-455,542-547).
            if intersect {
                BoundingBox::infinite().and(ba).and(bb)
            } else {
                BoundingBox::empty().or(ba).or(bb)
            }
        }
        Node::Not(a) => node_box(geom, nodes, a, !negate),
    }
}

/// `Cell.bounding_box` (`cell.py:373-377`): the region's box, or infinite for
/// a cell with no region.
#[must_use]
pub fn cell_bounding_box(geom: &Geometry, cell: &Cell) -> BoundingBox {
    match region_tree(&cell.region) {
        Some((nodes, root)) => node_box(geom, &nodes, root, false),
        None => BoundingBox::infinite(),
    }
}

/// `Universe.bounding_box` (`universe.py:435-441`): the `Union` of its cells'
/// regions, or infinite when no cell has one.
#[must_use]
pub fn universe_bounding_box(geom: &Geometry, universe: usize) -> BoundingBox {
    let boxes: Vec<BoundingBox> = geom.universes[universe]
        .cell_indices
        .iter()
        .map(|&c| &geom.cells[c])
        .filter(|c| !c.region.is_empty())
        .map(|c| cell_bounding_box(geom, c))
        .collect();
    if boxes.is_empty() {
        BoundingBox::infinite()
    } else {
        boxes
            .into_iter()
            .fold(BoundingBox::empty(), BoundingBox::or)
    }
}

/// `Geometry.bounding_box` = the root universe's (`geometry.py:70-71`).
#[must_use]
pub fn geometry_bounding_box(geom: &Geometry) -> BoundingBox {
    universe_bounding_box(geom, geom.root_universe)
}

// ---------------------------------------------------------------------------
// Domain order (get_all_cells / get_all_materials)
// ---------------------------------------------------------------------------

/// Universes a lattice holds, in `Lattice.get_unique_universes` order
/// (`lattice.py:110-136`): the Python `universes` nesting walked outer to
/// inner, first occurrence kept, then `outer`.
///
/// For a rectangular lattice the Python nesting is `[z][row][col]` with rows
/// listed **top first** (`lattice.py` `RectLattice.universes`); this crate
/// stores `ix + nx*iy + nx*ny*iz` with `iy = 0` at the bottom, so rows are
/// walked downwards. For a hex lattice the nesting is `[z][ring][position]`,
/// outermost ring first, positions clockwise from the top; the skewed index
/// of each (ring, position) is recovered by filling a probe array through the
/// same `fill_level` walk `HexLattice::from_rings` uses.
pub(crate) fn lattice_unique_universes(lat: &Lattice) -> Vec<usize> {
    let mut out: Vec<usize> = Vec::new();
    let add = |u: usize, out: &mut Vec<usize>| {
        if !out.contains(&u) {
            out.push(u);
        }
    };
    match lat {
        Lattice::Rect(r) => {
            let [nx, ny, nz] = r.n;
            for iz in 0..nz {
                for iy in (0..ny).rev() {
                    for ix in 0..nx {
                        add(r.universes[nx * ny * iz + nx * iy + ix], &mut out);
                    }
                }
            }
            if let Some(o) = r.outer {
                add(o, &mut out);
            }
        }
        Lattice::Hex(h) => {
            let order = h.ring_order_flat_indices();
            let n2 = h.n_side() * h.n_side();
            // Python's `universes[z]` is written to XML for z = 0, 1, ... and
            // the C++ reads the first level as the bottom (`fill_lattice_*`'s
            // `m` loop), so z = 0 is the bottom level, as `iz` is here.
            for iz in 0..h.n_axial {
                for &flat in &order {
                    let u = h.universes[n2 * iz + flat];
                    if u >= 0 {
                        add(u as usize, &mut out);
                    }
                }
            }
            if let Some(o) = h.outer {
                add(o, &mut out);
            }
        }
    }
    out
}

/// Cell indices in `Universe.get_all_cells` order (`universe.py:207-232`,
/// `cell.py:448-469`, `lattice.py:161-184`): the universe's own cells first,
/// then, cell by cell, everything nested in each fill; one shared memo so a
/// universe or lattice is expanded once. Dict insertion semantics: a key
/// already present keeps its first position.
#[must_use]
pub fn all_cells_order(geom: &Geometry, universe: usize) -> Vec<usize> {
    #[derive(Default)]
    struct Memo {
        universes: Vec<usize>,
        lattices: Vec<usize>,
        cells: Vec<usize>,
    }
    fn push_unique(v: &mut Vec<usize>, x: usize) {
        if !v.contains(&x) {
            v.push(x);
        }
    }
    fn universe_cells(geom: &Geometry, u: usize, memo: &mut Memo, first: bool) -> Vec<usize> {
        if !first && memo.universes.contains(&u) {
            return Vec::new();
        }
        memo.universes.push(u);
        let own = &geom.universes[u].cell_indices;
        let mut cells: Vec<usize> = Vec::new();
        for &c in own {
            push_unique(&mut cells, c);
        }
        for &c in own {
            for n in cell_cells(geom, c, memo) {
                push_unique(&mut cells, n);
            }
        }
        cells
    }
    fn cell_cells(geom: &Geometry, c: usize, memo: &mut Memo) -> Vec<usize> {
        if memo.cells.contains(&c) {
            return Vec::new();
        }
        memo.cells.push(c);
        match geom.cells[c].fill {
            CellFill::Universe(u) => universe_cells(geom, u, memo, false),
            CellFill::Lattice(l) => {
                if memo.lattices.contains(&l) {
                    return Vec::new();
                }
                memo.lattices.push(l);
                let mut cells = Vec::new();
                for u in lattice_unique_universes(&geom.lattices[l]) {
                    for n in universe_cells(geom, u, memo, false) {
                        push_unique(&mut cells, n);
                    }
                }
                cells
            }
            CellFill::Material(_) | CellFill::Void => Vec::new(),
        }
    }
    universe_cells(geom, universe, &mut Memo::default(), true)
}

/// Material indices in `Universe.get_all_materials` order
/// (`universe.py:234-255`): the materials of [`all_cells_order`]'s cells, in
/// that order, first occurrence kept.
#[must_use]
pub fn all_materials_order(geom: &Geometry, universe: usize) -> Vec<usize> {
    let mut out = Vec::new();
    for c in all_cells_order(geom, universe) {
        if let CellFill::Material(m) = geom.cells[c].fill {
            if !out.contains(&m) {
                out.push(m);
            }
        }
    }
    out
}
