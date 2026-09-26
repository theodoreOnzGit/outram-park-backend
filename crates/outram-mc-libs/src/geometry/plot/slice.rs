// SPDX-License-Identifier: GPL-3.0
//
// Ported from OpenMC (MIT), commit d7d3284a1:
//   include/openmc/plot.h:199-293  SlicePlotBase, get_map<T>
//   src/plot.cpp:47-79             IdData::set_value / set_overlap
//   src/plot.cpp:317-358           Plot::create_image
//   src/plot.cpp:479-548           set_basis / set_origin / set_width
//   src/plot.cpp:550-561           set_universe (the `level`)
//   src/plot.cpp:941-1053          Plot::draw_mesh_lines
//   src/mesh.cpp:1628-1667         RegularMesh::plot
//   src/geometry.cpp:38-90         check_cell_overlap
// Copyright (c) 2011-2026 Massachusetts Institute of Technology, UChicago
// Argonne LLC, and OpenMC contributors. MIT notice in
// verification_and_validation/geometry_plotting/openmc_inputs/LICENSE.openmc.

//! **Slice plots** — OpenMC's `<plot type="slice">`, rasterised in Rust.

#[cfg(not(target_arch = "wasm32"))]
use rayon::prelude::*;

#[cfg(target_arch = "wasm32")]
use crate::wasm_par::prelude::*;

use super::colour::{ColourScheme, PlotColourBy, Rgb, BLACK, WHITE};
use super::image::ImageData;
use crate::geometry::cell::SurfaceToken;
use crate::geometry::geometry::{Geometry, GeometryPath};
use crate::geometry::position::{Direction, Position};
use crate::tally::mesh::RegularMesh;

/// Slice orientation. `SlicePlotBase::PlotBasis` (`include/openmc/plot.h:204`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlotBasis {
    /// Horizontal axis +x, vertical axis +y.
    Xy,
    /// Horizontal axis +x, vertical axis +z — an R-Z view of an axisymmetric core.
    Xz,
    /// Horizontal axis +y, vertical axis +z.
    Yz,
}

impl PlotBasis {
    /// Global axis indices `(horizontal, vertical)` — `ax1`, `ax2` in
    /// `draw_mesh_lines` (`src/plot.cpp:946-970`).
    #[must_use]
    pub fn axes(self) -> (usize, usize) {
        match self {
            Self::Xy => (0, 1),
            Self::Xz => (0, 2),
            Self::Yz => (1, 2),
        }
    }

    /// Short name, as OpenMC spells it.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Xy => "xy",
            Self::Xz => "xz",
            Self::Yz => "yz",
        }
    }
}

/// Mesh-line overlay. `<meshlines>` (`Plot::set_meshlines`, `src/plot.cpp:635-741`).
///
/// Upstream draws any structured mesh that implements `Mesh::plot` (regular,
/// rectilinear, cylindrical, spherical). This crate has **one** mesh type,
/// [`RegularMesh`], so that is the only one supported — which also means
/// `meshtype` (`ufs` / `entropy` / `tally`) has nothing to choose between here:
/// the caller hands over the mesh itself.
#[derive(Debug, Clone)]
pub struct MeshLines {
    /// The mesh to overlay.
    pub mesh: RegularMesh,
    /// `linewidth`: a line covers `2 * width + 1` pixels (`src/plot.cpp:1015`).
    pub width: i32,
    /// Line colour. Upstream's default is a value-initialised `RGBColor`, i.e.
    /// [`BLACK`] (`include/openmc/plot.h:50`, `:324`).
    pub colour: Rgb,
}

/// A slice plot: the geometry-side parameters of OpenMC's `Plot` with
/// `PlotType::slice`. Colours live separately in a [`ColourScheme`] so one
/// geometry pass ([`SlicePlot::id_map`]) can be coloured several ways.
#[derive(Debug, Clone)]
pub struct SlicePlot {
    /// Centre of the slice \[cm\]. `origin_`.
    pub origin: Position,
    /// Orientation. `basis_`.
    pub basis: PlotBasis,
    /// Full widths along the horizontal and vertical axes \[cm\]. `width_`.
    pub width: [f64; 2],
    /// Pixels across and down. `pixels_`.
    pub pixels: [usize; 2],
    /// Universe level whose cell is coloured under cell colouring; `None` is
    /// upstream's `PLOT_LEVEL_LOWEST` (the leaf cell). `level_` / `slice_level_`.
    /// Level 0 is the root universe. Ignored under material colouring, where
    /// the leaf material is always used — as upstream.
    pub level: Option<usize>,
    /// Paint overlapping cells in the scheme's overlap colour. `show_overlaps_`.
    pub show_overlaps: bool,
    /// Optional mesh-line overlay.
    pub meshlines: Option<MeshLines>,
}

/// What one pixel of a slice found. The information of one `IdData` entry
/// (`src/plot.cpp:47-79`) in a type rather than in magic negative integers
/// (`NOT_FOUND = -2`, `OVERLAP = -3`, `MATERIAL_VOID = -1`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SliceHit {
    /// The point is in no cell (outside the model, or in a hole at some level).
    NotFound,
    /// The point is claimed by two or more cells of one universe
    /// (only reported when [`SlicePlot::show_overlaps`] is set).
    Overlap,
    /// A cell was found.
    Found {
        /// Cell index at the plot's level, or `None` when that level is deeper
        /// than the geometry here (upstream writes `NOT_FOUND`, drawn as background).
        cell: Option<usize>,
        /// Leaf material index, or `None` for a void cell (`MATERIAL_VOID`).
        material: Option<usize>,
    },
}

/// The per-pixel result of a slice — `get_map<IdData>`. Row 0 is the top of
/// the image; `hits[y * width + x]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdMap {
    /// Pixels across.
    pub width: usize,
    /// Pixels down.
    pub height: usize,
    /// One entry per pixel.
    pub hits: Vec<SliceHit>,
}

impl IdMap {
    /// The hit at column `x`, row `y`.
    #[must_use]
    pub fn get(&self, x: usize, y: usize) -> SliceHit {
        self.hits[y * self.width + x]
    }
}

/// The tie-breaking direction upstream locates every pixel with
/// (`include/openmc/plot.h:255`): not aligned with any axis, so a pixel centre
/// lying exactly on a plane resolves the same way everywhere.
pub(crate) fn plot_direction() -> Direction {
    let s = 1.0 / 2.0_f64.sqrt();
    Direction::new(s, s, 0.0)
}

impl SlicePlot {
    /// A slice with upstream's defaults: leaf level, no overlaps, no mesh lines.
    #[must_use]
    pub fn new(basis: PlotBasis, origin: Position, width: [f64; 2], pixels: [usize; 2]) -> Self {
        Self {
            origin,
            basis,
            width,
            pixels,
            level: None,
            show_overlaps: false,
            meshlines: None,
        }
    }

    /// Plot a specific universe level (0 = root). `<level>`.
    #[must_use]
    pub fn at_level(mut self, level: usize) -> Self {
        self.level = Some(level);
        self
    }

    /// Show overlapping cells. `<show_overlaps>`.
    #[must_use]
    pub fn showing_overlaps(mut self) -> Self {
        self.show_overlaps = true;
        self
    }

    /// Overlay a regular mesh's lines. `<meshlines>`.
    #[must_use]
    pub fn with_meshlines(mut self, mesh: RegularMesh, width: i32, colour: Rgb) -> Self {
        self.meshlines = Some(MeshLines {
            mesh,
            width,
            colour,
        });
        self
    }

    /// `u_span_` / `v_span_` for this basis (`Plot::set_width`, `src/plot.cpp:511-548`).
    fn spans(&self) -> (Position, Position) {
        let (wx, wy) = (self.width[0], self.width[1]);
        match self.basis {
            PlotBasis::Xy => (Position::new(wx, 0.0, 0.0), Position::new(0.0, wy, 0.0)),
            PlotBasis::Xz => (Position::new(wx, 0.0, 0.0), Position::new(0.0, 0.0, wy)),
            PlotBasis::Yz => (Position::new(0.0, wx, 0.0), Position::new(0.0, 0.0, wy)),
        }
    }

    /// Global position of the centre of pixel `(x, y)`, with exactly the
    /// floating-point operations of `get_map` (`include/openmc/plot.h:240-271`)
    /// so that a pixel centre lying on a surface rounds the same way.
    #[must_use]
    pub fn pixel_centre(&self, x: usize, y: usize) -> Position {
        let (u_span, v_span) = self.spans();
        let u_step = u_span / self.pixels[0] as f64;
        let v_step = v_span / self.pixels[1] as f64;
        let start = self.origin - 0.5 * u_span + 0.5 * v_span + 0.5 * u_step - 0.5 * v_step;
        let row = start - v_step * y as f64;
        row + u_step * x as f64
    }

    /// Locate every pixel. Port of `SlicePlotBase::get_map<IdData>`
    /// (`include/openmc/plot.h:222-293`) with `IdData::set_value`
    /// (`src/plot.cpp:51-73`) and, when [`Self::show_overlaps`] is set,
    /// `check_cell_overlap` (`src/geometry.cpp:38-90`).
    ///
    /// # One divergence from upstream, stated
    ///
    /// Upstream runs the overlap check even when the locate *failed* part-way
    /// down (on the coordinate levels it did reach). [`Geometry::locate`]
    /// returns no partial path, so here an unlocated pixel is `NotFound`
    /// without an overlap check. It matters only for an overlap in an upper
    /// universe directly above a hole in a lower one.
    #[must_use]
    pub fn id_map(&self, geom: &Geometry) -> IdMap {
        let (w, h) = (self.pixels[0], self.pixels[1]);
        let dir = plot_direction();
        let rows: Vec<Vec<SliceHit>> = (0..h)
            .into_par_iter()
            .map(|y| {
                (0..w)
                    .map(|x| {
                        let r = self.pixel_centre(x, y);
                        let Some(path) = geom.locate(r, dir, SurfaceToken::NONE) else {
                            return SliceHit::NotFound;
                        };
                        if self.show_overlaps && check_cell_overlap(geom, &path) {
                            return SliceHit::Overlap;
                        }
                        // `j = level >= 0 ? level : n_coord - 1`, and a level
                        // at or past n_coord reads NOT_FOUND (plot.cpp:55-57).
                        let cell = match self.level {
                            None => path.levels.last().map(|c| c.cell),
                            Some(l) => path.levels.get(l).map(|c| c.cell),
                        };
                        SliceHit::Found {
                            cell,
                            material: path.material,
                        }
                    })
                    .collect()
            })
            .collect();
        IdMap {
            width: w,
            height: h,
            hits: rows.into_iter().flatten().collect(),
        }
    }

    /// Render the slice. Port of `Plot::create_image` (`src/plot.cpp:317-358`),
    /// including the mesh-line overlay.
    #[must_use]
    pub fn create_image(&self, geom: &Geometry, scheme: &ColourScheme) -> ImageData {
        let ids = self.id_map(geom);
        self.colour_id_map(&ids, scheme)
    }

    /// Colour an already-computed [`IdMap`] — the colouring half of
    /// `create_image`, so one geometry pass can be drawn by cell and by
    /// material without locating every pixel twice.
    #[must_use]
    pub fn colour_id_map(&self, ids: &IdMap, scheme: &ColourScheme) -> ImageData {
        let mut data = ImageData::filled(ids.width, ids.height, scheme.background);
        for y in 0..ids.height {
            for x in 0..ids.width {
                let c = match ids.get(x, y) {
                    SliceHit::NotFound => continue,
                    SliceHit::Overlap => scheme.overlap_colour,
                    SliceHit::Found { cell, material } => match scheme.colour_by {
                        PlotColourBy::Cell => match cell {
                            None => continue,
                            Some(c) => scheme.colours[c],
                        },
                        PlotColourBy::Material => match material {
                            None => WHITE,
                            Some(m) => scheme.colours[m],
                        },
                    },
                };
                data.set(x, y, c);
            }
        }
        if let Some(ml) = &self.meshlines {
            self.draw_mesh_lines(&mut data, ml);
        }
        data
    }

    /// Port of `Plot::draw_mesh_lines` (`src/plot.cpp:941-1053`), integer
    /// truncations and all.
    fn draw_mesh_lines(&self, data: &mut ImageData, ml: &MeshLines) {
        let (ax1, ax2) = self.basis.axes();
        let px = [self.pixels[0] as i64, self.pixels[1] as i64];
        let o = [self.origin.x, self.origin.y, self.origin.z];
        let mut ll = o;
        let mut ur = o;
        ll[ax1] -= self.width[0] / 2.0;
        ll[ax2] -= self.width[1] / 2.0;
        ur[ax1] += self.width[0] / 2.0;
        ur[ax2] += self.width[1] / 2.0;
        let width = [ur[0] - ll[0], ur[1] - ll[1], ur[2] - ll[2]];

        let (first, second) = regular_mesh_plot_lines(&ml.mesh, ll, ur);
        let rgb = ml.colour;
        let mut put = |x: i64, y: i64| {
            data.set(x as usize, y as usize, rgb);
        };

        let (ax2_min, ax2_max) =
            if let (Some(&back), Some(&front)) = (second.last(), second.first()) {
                let frac = (back - ll[ax2]) / width[ax2];
                let lo = (((1.0 - frac) * px[1] as f64) as i64).max(0);
                let frac = (front - ll[ax2]) / width[ax2];
                let hi = (((1.0 - frac) * px[1] as f64) as i64).min(px[1]);
                (lo, hi)
            } else {
                (0, px[1])
            };
        for &v in &first {
            let frac = (v - ll[ax1]) / width[ax1];
            let ax1_ind = (frac * px[0] as f64) as i64;
            for ax2_ind in ax2_min..ax2_max {
                for plus in 0..=i64::from(ml.width) {
                    if ax1_ind + plus >= 0 && ax1_ind + plus < px[0] {
                        put(ax1_ind + plus, ax2_ind);
                    }
                    if ax1_ind - plus >= 0 && ax1_ind - plus < px[0] {
                        put(ax1_ind - plus, ax2_ind);
                    }
                }
            }
        }

        let (ax1_min, ax1_max) = if let (Some(&front), Some(&back)) = (first.first(), first.last())
        {
            let frac = (front - ll[ax1]) / width[ax1];
            let lo = ((frac * px[0] as f64) as i64).max(0);
            let frac = (back - ll[ax1]) / width[ax1];
            let hi = ((frac * px[0] as f64) as i64).min(px[0]);
            (lo, hi)
        } else {
            (0, px[0])
        };
        for &v in &second {
            let frac = (v - ll[ax2]) / width[ax2];
            let ax2_ind = ((1.0 - frac) * px[1] as f64) as i64;
            for ax1_ind in ax1_min..ax1_max {
                for plus in 0..=i64::from(ml.width) {
                    if ax2_ind + plus >= 0 && ax2_ind + plus < px[1] {
                        put(ax1_ind, ax2_ind + plus);
                    }
                    if ax2_ind - plus >= 0 && ax2_ind - plus < px[1] {
                        put(ax1_ind, ax2_ind - plus);
                    }
                }
            }
        }
    }
}

/// Mesh lines of a regular mesh that fall inside an axis-aligned plot window.
/// Port of `RegularMesh::plot` (`src/mesh.cpp:1628-1667`), which accumulates
/// `coord += width` rather than computing `lower_left + i * width` — kept, so
/// the same lines fall on the same pixels. This crate's mesh is always 3-D.
fn regular_mesh_plot_lines(mesh: &RegularMesh, ll: [f64; 3], ur: [f64; 3]) -> (Vec<f64>, Vec<f64>) {
    let n_dimension = 3;
    // `plot_ur.z == plot_ll.z` etc.: the axis with zero extent is the normal.
    let axes: [Option<usize>; 2] = if ur[2] == ll[2] {
        [Some(0), if n_dimension > 1 { Some(1) } else { None }]
    } else if ur[1] == ll[1] {
        [Some(0), if n_dimension > 2 { Some(2) } else { None }]
    } else {
        [
            if n_dimension > 1 { Some(1) } else { None },
            if n_dimension > 2 { Some(2) } else { None },
        ]
    };
    let width = mesh.width();
    let mut out: [Vec<f64>; 2] = [Vec::new(), Vec::new()];
    for (i_ax, axis) in axes.iter().enumerate() {
        let Some(axis) = *axis else { continue };
        let mut coord = mesh.lower_left[axis];
        for _ in 0..=mesh.dimension[axis] {
            if coord >= ll[axis] && coord <= ur[axis] {
                out[i_ax].push(coord);
            }
            coord += width[axis];
        }
    }
    let [a, b] = out;
    (a, b)
}

/// Whether any cell other than the one found claims the point, at any level.
/// Port of `check_cell_overlap` (`src/geometry.cpp:38-90`) with `error =
/// false`, minus its bookkeeping of which pair overlaps (the image only needs
/// to know that one does).
#[must_use]
pub fn check_cell_overlap(geom: &Geometry, path: &GeometryPath) -> bool {
    for coord in &path.levels {
        for &i_cell in &geom.universes[coord.universe].cell_indices {
            if i_cell != coord.cell
                && geom.cells[i_cell].contains(coord.r, coord.u, &geom.surfaces, path.on_surface)
            {
                return true;
            }
        }
    }
    false
}

/// Default mesh-line colour, spelled out: upstream's `meshlines_color_` is a
/// default-constructed `RGBColor`, which is black.
pub const DEFAULT_MESHLINE_COLOUR: Rgb = BLACK;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::cell::{Cell, HalfSpaceSense, RegionToken};
    use crate::geometry::surface::{BoundaryType, SurfaceKind, XPlane};
    use crate::geometry::universe::Universe;

    /// Split at x = 0.5: cell 0 (material 0) for x < 0.5, cell 1 (void) for
    /// 0.5 < x < 2, nothing beyond x = 2.
    fn split() -> Geometry {
        let p = |x0, bc| SurfaceKind::XPlane(XPlane { x0, bc });
        let hs = |surface_idx, sense| RegionToken::HalfSpace { surface_idx, sense };
        Geometry {
            surfaces: vec![
                p(0.5, BoundaryType::Transmissive),
                p(2.0, BoundaryType::Vacuum),
            ],
            cells: vec![
                Cell::material(1, vec![hs(0, HalfSpaceSense::Inside)], 0, 293.6),
                Cell::fill(
                    2,
                    vec![
                        hs(0, HalfSpaceSense::Outside),
                        hs(1, HalfSpaceSense::Inside),
                        RegionToken::Intersection,
                    ],
                    crate::geometry::cell::CellFill::Void,
                    Position::ZERO,
                ),
            ],
            universes: vec![Universe {
                id: 0,
                cell_indices: vec![0, 1],
            }],
            lattices: vec![],
            root_universe: 0,
        }
    }

    /// Pixel centres follow `get_map`: 4 pixels over [-2, 2] sit at -1.5, -0.5,
    /// 0.5, 1.5, and row 0 is the TOP of the image.
    #[test]
    fn pixel_centres_and_orientation() {
        let s = SlicePlot::new(PlotBasis::Xy, Position::ZERO, [4.0, 4.0], [4, 4]);
        let c = s.pixel_centre(0, 0);
        assert_eq!((c.x, c.y), (-1.5, 1.5));
        let c = s.pixel_centre(3, 3);
        assert_eq!((c.x, c.y), (1.5, -1.5));
        let s = SlicePlot::new(
            PlotBasis::Xz,
            Position::new(0.0, 7.0, 0.0),
            [4.0, 2.0],
            [4, 2],
        );
        let c = s.pixel_centre(1, 1);
        assert_eq!((c.x, c.y, c.z), (-0.5, 7.0, -0.5));
    }

    /// Material, void and outside all land where they should; x = 0.5 lies
    /// exactly on the plane and resolves along +x (the tie direction), into
    /// the void cell, as upstream's `(1/sqrt 2, 1/sqrt 2, 0)` does.
    #[test]
    fn slice_classifies_material_void_and_outside() {
        let g = split();
        let s = SlicePlot::new(
            PlotBasis::Xy,
            Position::new(0.5, 0.0, 0.0),
            [5.0, 1.0],
            [5, 1],
        );
        let ids = s.id_map(&g);
        let got: Vec<SliceHit> = (0..5).map(|x| ids.get(x, 0)).collect();
        assert_eq!(
            got[0],
            SliceHit::Found {
                cell: Some(0),
                material: Some(0)
            }
        ); // x = -1.5
        assert_eq!(
            got[1],
            SliceHit::Found {
                cell: Some(0),
                material: Some(0)
            }
        ); // x = -0.5
        assert_eq!(
            got[2],
            SliceHit::Found {
                cell: Some(1),
                material: None
            }
        ); // x = 0.5 (on plane)
        assert_eq!(
            got[3],
            SliceHit::Found {
                cell: Some(1),
                material: None
            }
        ); // x = 1.5
        assert_eq!(got[4], SliceHit::NotFound); // x = 2.5
    }
}
