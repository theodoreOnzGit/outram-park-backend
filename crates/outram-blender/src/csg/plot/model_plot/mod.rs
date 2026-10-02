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

//! **OpenMC's Python slice plot (`openmc.Model.plot`), as an emitted
//! matplotlib script.**
//!
//! `openmc.Model.plot` — and `Universe.plot`, `Cell.plot`, `Geometry.plot`
//! and `Region.plot`, which all build a throw-away model and call it — asks
//! the C++ library for an **id map** (cell id, cell instance, material id per
//! pixel), turns it into an RGB image in numpy (`_id_map_to_rgb`), and draws
//! it with `imshow` on a figure sized so the axes are exactly `pixels` big,
//! optionally with an outline (`contour`), a legend and sampled source points.
//!
//! Here the id map comes from this crate's own ported point location
//! ([`SlicePlot::id_map`], verified pixel-for-pixel against `openmc --plot`),
//! and **everything after it is emitted as Python** that is a line-for-line
//! transcription of upstream's: the same numpy calls in the same order, the
//! same `RandomState(1)` default colours, the same figure sizing, the same
//! legend and contour arguments. Running the script therefore draws the
//! picture upstream draws. Verified pixel for pixel against OpenMC 0.16.1.dev25
//! running `Model.plot` itself: see
//! `verification_and_validation/python_plotting_parity/README.md`.
//!
//! The script is standalone (`numpy` + `matplotlib`, no OpenMC, no Rust
//! binary): the id map is embedded as base64 of zlib-compressed
//! little-endian `int32`.
//!
//! # Mapping of the Python keyword arguments
//!
//! | `Model.plot(...)` | [`ModelPlot`] |
//! |---|---|
//! | `origin`, `width` (default: bounding box) | [`ModelPlot::origin`], [`ModelPlot::width`] |
//! | `pixels` (int total, or `(h, v)`) | [`Pixels`] |
//! | `basis` | [`ModelPlot::basis`] |
//! | `color_by` | [`ModelPlot::color_by`] |
//! | `colors` (dict of Cell/Material → RGB or SVG name) | [`ModelPlot::colors`] |
//! | `seed` (→ `SlicePlot.colorize`) | [`ModelPlot::seed`] |
//! | `legend`, `legend_kwargs` | [`ModelPlot::legend`], [`ModelPlot::legend_kwargs`] |
//! | `axis_units` | [`AxisUnits`] |
//! | `outline` (`False`/`True`/`'only'`), `contour_kwargs` | [`Outline`], [`ModelPlot::contour_kwargs`] |
//! | `show_overlaps`, `overlap_color` | [`ModelPlot::show_overlaps`], [`ModelPlot::overlap_color`] |
//! | `n_samples`, `plane_tolerance`, `source_kwargs` | [`ModelPlot::source_points`], [`ModelPlot::plane_tolerance`], [`ModelPlot::source_kwargs`] |
//! | `**kwargs` (to `imshow`) | [`ModelPlot::imshow_kwargs`] |
//! | `axes` (draw into existing axes) | not ported: the script owns its figure |
//!
//! `*_kwargs` entries are `(name, python_expression)` pairs written verbatim
//! into the call, so anything matplotlib accepts can be passed.
//!
//! # One input differs, by necessity: `n_samples`
//!
//! Upstream samples `n_samples` source particles through `openmc.lib` with
//! OpenMC's own random stream. This crate's source sampling is its own, so the
//! caller passes the sampled positions ([`ModelPlot::source_points`]); the
//! slab selection (`slice_value - tol < r[z] < slice_value + tol`), the unit
//! scaling and the `scatter` call are upstream's.
//!
//! # IDs
//!
//! Upstream's id map holds **ids**, not indices: `Cell::id` here, and
//! [`MaterialIdentity::material_id`] of the material the cell's index points to — which is why
//! [`ModelPlot::emit`] takes the material table. Void reads `-1`
//! (`MATERIAL_VOID`), "not found" `-2`, an overlap `-3` in every channel
//! (`src/plot.cpp:43-79`).

use std::fmt::Write as _;

use crate::csg::cell::{CellFill, RegionToken};
use crate::csg::position::Position;
use crate::csg::universe::Universe;
use crate::csg::geometry::Geometry;

use super::slice::{PlotBasis, SliceHit, SlicePlot};

mod bounds;
mod svg;

pub use bounds::*;
pub use svg::*;

/// What the plotter needs to know about a material: the user-facing
/// id (OpenMC's `Material.id`, which the id map and the colour keys
/// carry) and its name.
///
/// Added 2026-10-02 (GitHub #486) when the plotter moved here from
/// `outram-mc-libs`, which implements it for its `Material`. Static
/// dispatch: every function taking materials is generic over it.
pub trait MaterialIdentity {
    /// User-facing material id (`openmc.Material.id`).
    fn material_id(&self) -> i32;
    /// Material name (`openmc.Material.name`).
    fn material_name(&self) -> &str;
}

// ---------------------------------------------------------------------------
// The plot
// ---------------------------------------------------------------------------

/// `pixels`: a total count split by aspect ratio, or `(horizontal, vertical)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pixels {
    /// `pixels=int` (upstream default `40000`).
    Total(usize),
    /// `pixels=(h, v)`.
    Exact([usize; 2]),
}

/// `axis_units`, with upstream's scale factors (`model.py:1392`) — note
/// `'km': 0.00001`, which is upstream's number (1 cm = 1e-5 km), kept.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AxisUnits {
    /// Kilometres.
    Km,
    /// Metres.
    M,
    /// Centimetres (default).
    Cm,
    /// Millimetres.
    Mm,
}

impl AxisUnits {
    fn name(self) -> &'static str {
        match self {
            Self::Km => "km",
            Self::M => "m",
            Self::Cm => "cm",
            Self::Mm => "mm",
        }
    }
}

/// `outline`: `False`, `True` or `'only'`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outline {
    /// No outline (default).
    Off,
    /// Contour lines over the image.
    On,
    /// Contour lines only, no image.
    Only,
}

/// `color_by`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorBy {
    /// By cell id (id-map channel 0). Upstream default.
    Cell,
    /// By material id (channel 2).
    Material,
}

/// One colour value as upstream accepts it: an RGB triple (0-255) or an SVG
/// colour name (looked up in `_SVG_COLORS` for the image, passed to
/// matplotlib as the name for the legend patch, exactly as upstream does).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlotColour {
    /// `(r, g, b)`.
    Rgb([u8; 3]),
    /// An SVG name, e.g. `"red"`. Must be a key of [`SVG_COLOURS`].
    Named(String),
}

impl PlotColour {
    fn py(&self) -> String {
        match self {
            Self::Rgb([r, g, b]) => format!("({r}, {g}, {b})"),
            Self::Named(n) => format!("{:?}", n),
        }
    }
}

/// A `colors` dict entry: the domain's **id** (cell id or material id, per
/// [`ModelPlot::color_by`]), its legend label (`key.name`, or the id when the
/// name is empty — upstream's rule), and its colour.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomainColour {
    /// Cell or material id.
    pub id: i32,
    /// `name`; empty means "label with the id".
    pub name: String,
    /// The colour.
    pub colour: PlotColour,
}

/// Errors `Model.plot` raises before drawing, surfaced before emitting.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelPlotError {
    /// `_check_pixels`: a zero pixel count.
    Pixels,
    /// `plane_tolerance` must be > 0.
    PlaneTolerance,
    /// `legend=True` with no colours (`model.py:1478-1480`).
    LegendWithoutColours,
    /// A named colour that is not in `_SVG_COLORS` (upstream: `KeyError`).
    UnknownColourName(String),
    /// A cell fills a material index the material table does not have.
    MaterialIndex(usize),
}

impl std::fmt::Display for ModelPlotError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pixels => write!(f, "pixels must be > 0"),
            Self::PlaneTolerance => write!(f, "plane_tolerance must be > 0"),
            Self::LegendWithoutColours => write!(
                f,
                "Must pass 'colors' dictionary if you are adding a legend via legend=True."
            ),
            Self::UnknownColourName(n) => write!(f, "unknown SVG colour name {n:?}"),
            Self::MaterialIndex(m) => write!(f, "material index {m} not in the material table"),
        }
    }
}

impl std::error::Error for ModelPlotError {}

/// **`openmc.Model.plot`, ported.** Build with [`ModelPlot::new`] (upstream's
/// defaults), set fields, then [`ModelPlot::emit`] the script.
#[derive(Debug, Clone)]
pub struct ModelPlot {
    /// `origin`; `None` = bounding-box centre (`_set_plot_defaults`).
    pub origin: Option<Position>,
    /// `width`; `None` = bounding-box width along the basis.
    pub width: Option<[f64; 2]>,
    /// `pixels`, default `Total(40000)`.
    pub pixels: Pixels,
    /// `basis`, default `xy`.
    pub basis: PlotBasis,
    /// `color_by`, default cell.
    pub color_by: ColorBy,
    /// `colors`, in dict order; `None` = upstream's `None`.
    pub colors: Option<Vec<DomainColour>>,
    /// `seed`: when `colors` is `None`, `SlicePlot.colorize(geometry, seed)`
    /// assigns every cell (or material) of the geometry a colour.
    pub seed: Option<u32>,
    /// Legend labels for cells, by cell id, used by `seed` colouring (this
    /// crate's cells carry no name). Missing = label with the id.
    pub cell_names: Vec<(i32, String)>,
    /// `legend`.
    pub legend: bool,
    /// Extra `legend_kwargs`, `(name, python expression)`, applied after
    /// upstream's `setdefault`s (so they override them).
    pub legend_kwargs: Vec<(String, String)>,
    /// `axis_units`.
    pub axis_units: AxisUnits,
    /// `outline`.
    pub outline: Outline,
    /// `contour_kwargs`, overriding upstream's defaults.
    pub contour_kwargs: Vec<(String, String)>,
    /// `show_overlaps`.
    pub show_overlaps: bool,
    /// `overlap_color`, default `(255, 0, 0)`.
    pub overlap_color: PlotColour,
    /// Sampled source positions — see the module docs on `n_samples`.
    /// `None` (or empty) draws no scatter.
    pub source_points: Option<Vec<Position>>,
    /// `plane_tolerance` \[cm\], default 1.
    pub plane_tolerance: f64,
    /// `source_kwargs`, overriding upstream's `marker='x'`.
    pub source_kwargs: Vec<(String, String)>,
    /// `**kwargs` passed to `imshow`.
    pub imshow_kwargs: Vec<(String, String)>,
    /// Title set on the axes after upstream's drawing (`None` = no title,
    /// as upstream). An addition for annotated figures; not in `Model.plot`.
    pub title: Option<String>,
    /// PNG the script writes when run with no argument.
    pub default_output: String,
    /// Keyword arguments for the final `plt.savefig` — the caller's call, not
    /// part of `Model.plot`. Empty (matplotlib defaults) for the parity cases;
    /// `bbox_inches='tight'` keeps upstream's outside-the-axes legend
    /// (`bbox_to_anchor=(1.05, 1)`) in the file.
    pub savefig_kwargs: Vec<(String, String)>,
}

impl ModelPlot {
    /// Upstream's defaults (`model.py:1345-1366`).
    #[must_use]
    pub fn new() -> Self {
        Self {
            origin: None,
            width: None,
            pixels: Pixels::Total(40000),
            basis: PlotBasis::Xy,
            color_by: ColorBy::Cell,
            colors: None,
            seed: None,
            cell_names: Vec::new(),
            legend: false,
            legend_kwargs: Vec::new(),
            axis_units: AxisUnits::Cm,
            outline: Outline::Off,
            contour_kwargs: Vec::new(),
            show_overlaps: false,
            overlap_color: PlotColour::Rgb([255, 0, 0]),
            source_points: None,
            plane_tolerance: 1.0,
            source_kwargs: Vec::new(),
            imshow_kwargs: Vec::new(),
            title: None,
            default_output: "plot.png".into(),
            savefig_kwargs: Vec::new(),
        }
    }

    /// `_set_plot_defaults` (`model.py:1114-1148`): origin, width and pixel
    /// counts after defaulting, from the geometry's bounding box.
    #[must_use]
    pub fn resolved(&self, geom: &Geometry) -> ([f64; 3], [f64; 2], [usize; 2]) {
        let (x, y) = self.basis.axes();
        let bb = geometry_bounding_box(geom);
        let inf = [
            bb.lower_left[x],
            bb.upper_right[x],
            bb.lower_left[y],
            bb.upper_right[y],
        ]
        .iter()
        .any(|v| v.is_infinite());
        let (origin, width) = if inf {
            (
                self.origin.map_or([0.0; 3], |o| [o.x, o.y, o.z]),
                self.width.unwrap_or([10.0, 10.0]),
            )
        } else {
            // np.nan_to_num: NaN -> 0, +inf -> max float, -inf -> min float.
            let c = bb.center().map(|v| {
                if v.is_nan() {
                    0.0
                } else if v == f64::INFINITY {
                    f64::MAX
                } else if v == f64::NEG_INFINITY {
                    f64::MIN
                } else {
                    v
                }
            });
            let w = bb.width();
            (
                self.origin.map_or(c, |o| [o.x, o.y, o.z]),
                self.width.unwrap_or([w[x], w[y]]),
            )
        };
        let pixels = match self.pixels {
            Pixels::Exact(p) => p,
            Pixels::Total(p) => {
                let aspect_ratio = width[0] / width[1];
                let pixels_y = (p as f64 / aspect_ratio).sqrt();
                [(p as f64 / pixels_y) as usize, pixels_y as usize]
            }
        };
        (origin, width, pixels)
    }

    /// The id map upstream's `Model.slice_data` returns — shape
    /// `(v, h, [cell id, material id])` flattened row-major, row 0 the top —
    /// computed with this crate's point location.
    ///
    /// # Errors
    /// [`ModelPlotError::MaterialIndex`] if a cell's material index is not in
    /// `materials`.
    pub fn id_map<M: MaterialIdentity>(
        &self,
        geom: &Geometry,
        materials: &[M],
    ) -> Result<(Vec<i32>, Vec<i32>, [usize; 2]), ModelPlotError> {
        let (origin, width, pixels) = self.resolved(geom);
        let sp = SlicePlot::new(
            self.basis,
            Position::new(origin[0], origin[1], origin[2]),
            width,
            pixels,
        );
        if self.show_overlaps {
            return overlap_id_map(geom, &sp, materials).map(|(c, m)| (c, m, pixels));
        }
        let ids = sp.id_map(geom);
        let mut cells = Vec::with_capacity(ids.hits.len());
        let mut mats = Vec::with_capacity(ids.hits.len());
        for h in &ids.hits {
            let (c, m) = match *h {
                SliceHit::NotFound => (-2, -2),
                SliceHit::Overlap => unreachable!("overlaps are not checked on this path"),
                SliceHit::Found { cell, material } => (
                    cell.map_or(-2, |c| geom.cells[c].id),
                    material_id(material, materials)?,
                ),
            };
            cells.push(c);
            mats.push(m);
        }
        Ok((cells, mats, pixels))
    }

    fn check_colour(c: &PlotColour) -> Result<(), ModelPlotError> {
        match c {
            PlotColour::Named(n) if svg_colour(n).is_none() => {
                Err(ModelPlotError::UnknownColourName(n.clone()))
            }
            _ => Ok(()),
        }
    }

    /// Emit the standalone matplotlib script.
    ///
    /// # Errors
    /// The checks `Model.plot` makes before it draws ([`ModelPlotError`]).
    pub fn emit<M: MaterialIdentity>(
        &self,
        geom: &Geometry,
        materials: &[M],
    ) -> Result<String, ModelPlotError> {
        self.validate()?;
        let (cells, mats, _) = self.id_map(geom, materials)?;
        self.emit_with_id_map(geom, materials, &cells, &mats)
    }

    fn validate(&self) -> Result<(), ModelPlotError> {
        if matches!(self.pixels, Pixels::Total(0))
            || matches!(self.pixels, Pixels::Exact([0, _] | [_, 0]))
        {
            return Err(ModelPlotError::Pixels);
        }
        if !(self.plane_tolerance > 0.0) {
            return Err(ModelPlotError::PlaneTolerance);
        }
        Self::check_colour(&self.overlap_color)?;
        for d in self.colors.iter().flatten() {
            Self::check_colour(&d.colour)?;
        }
        Ok(())
    }

    /// [`Self::emit`] with an id map the caller already has from
    /// [`Self::id_map`] (same plot settings), so a caller that inspects the
    /// map first — e.g. to list only the materials present — does not locate
    /// every pixel twice.
    ///
    /// # Errors
    /// As [`Self::emit`].
    pub fn emit_with_id_map<M: MaterialIdentity>(
        &self,
        geom: &Geometry,
        materials: &[M],
        cells: &[i32],
        mats: &[i32],
    ) -> Result<String, ModelPlotError> {
        self.validate()?;
        let (origin, width, pixels) = self.resolved(geom);
        assert_eq!(
            cells.len(),
            pixels[0] * pixels[1],
            "id map does not match the plot settings"
        );

        // `colorize` domains, in get_all_cells / get_all_materials order.
        let seeded: Option<Vec<(i32, String)>> = match (&self.colors, self.seed) {
            (None, Some(_)) => Some(match self.color_by {
                ColorBy::Cell => all_cells_order(geom, geom.root_universe)
                    .into_iter()
                    .map(|c| {
                        let id = geom.cells[c].id;
                        let name = self
                            .cell_names
                            .iter()
                            .find(|(i, _)| *i == id)
                            .map_or(String::new(), |(_, n)| n.clone());
                        (id, name)
                    })
                    .collect(),
                ColorBy::Material => all_materials_order(geom, geom.root_universe)
                    .into_iter()
                    .map(|m| {
                        materials
                            .get(m)
                            .map(|mt| (mt.material_id(), mt.material_name().to_string()))
                            .ok_or(ModelPlotError::MaterialIndex(m))
                    })
                    .collect::<Result<_, _>>()?,
            }),
            _ => None,
        };
        let has_colours = self.colors.as_ref().is_some_and(|c| !c.is_empty())
            || seeded.as_ref().is_some_and(|c| !c.is_empty());
        if self.legend && !has_colours {
            return Err(ModelPlotError::LegendWithoutColours);
        }

        let (xi, yi) = self.basis.axes();
        let zi = 3 - xi - yi;
        let basis = self.basis.name();
        let mut s = String::new();
        let w = &mut s;
        let _ = writeln!(w, "#!/usr/bin/env python3");
        let _ = writeln!(
            w,
            "# Generated by outram_mc_libs::geometry::plot::model_plot -- a transcription of\n\
             # openmc.Model.plot (openmc/model/model.py:1345-1543) and _id_map_to_rgb\n\
             # (openmc/plots.py:360-437), OpenMC d7d3284a1 (MIT). The id map was computed by\n\
             # outram-mc-libs' ported point location. Needs numpy + matplotlib only.\n\
             # Usage: python3 <this> [output.png]"
        );
        let _ = writeln!(w, "import base64, sys, zlib\nimport numpy as np\nimport matplotlib\nmatplotlib.use('Agg')\nimport matplotlib.patches as mpatches\nimport matplotlib.pyplot as plt\n");
        let _ = writeln!(w, "_SVG_COLORS = {{");
        for (n, [r, g, b]) in SVG_COLOURS {
            let _ = writeln!(w, "    '{n}': ({r}, {g}, {b}),");
        }
        let _ = writeln!(w, "}}\n");
        let _ = writeln!(
            w,
            "class _Domain:\n    \"\"\"Stands in for openmc.Cell / openmc.Material: an id and a name.\"\"\"\n    def __init__(self, id, name):\n        self.id = id\n        self.name = name\n"
        );
        let _ = writeln!(
            w,
            "def _arr(s):\n    return np.frombuffer(zlib.decompress(base64.b64decode(s)), dtype='<i4')\n"
        );
        let _ = writeln!(w, "V_PIX, H_PIX = {}, {}", pixels[1], pixels[0]);
        let _ = writeln!(w, "_CELLS = '{}'", b64_zlib_i32(cells));
        let _ = writeln!(w, "_MATS = '{}'", b64_zlib_i32(mats));
        let _ = writeln!(
            w,
            "id_map = np.zeros((V_PIX, H_PIX, 3), dtype=np.int32)\n\
             id_map[:, :, 0] = _arr(_CELLS).reshape(V_PIX, H_PIX)\n\
             id_map[:, :, 1] = np.where(id_map[:, :, 0] < 0, id_map[:, :, 0], 0)  # instance: not used by the plot\n\
             id_map[:, :, 2] = _arr(_MATS).reshape(V_PIX, H_PIX)\n\
             import os\n\
             if os.environ.get('OUTRAM_IDMAP_OUT'):  # verification hook, draws nothing\n\
             \x20   np.save(os.environ['OUTRAM_IDMAP_OUT'], id_map)\n"
        );
        let _ = writeln!(w, "basis = '{basis}'");
        let _ = writeln!(w, "x, y, z = {xi}, {yi}, {zi}");
        let _ = writeln!(
            w,
            "origin = ({}, {}, {})",
            py_f(origin[0]),
            py_f(origin[1]),
            py_f(origin[2])
        );
        let _ = writeln!(w, "width = ({}, {})", py_f(width[0]), py_f(width[1]));
        let _ = writeln!(w, "pixels = ({}, {})", pixels[0], pixels[1]);
        let _ = writeln!(
            w,
            "color_by = '{}'",
            match self.color_by {
                ColorBy::Cell => "cell",
                ColorBy::Material => "material",
            }
        );
        let _ = writeln!(w, "axis_units = '{}'", self.axis_units.name());
        let _ = writeln!(w, "legend = {}", py_bool(self.legend));
        let _ = writeln!(
            w,
            "outline = {}",
            match self.outline {
                Outline::Off => "False",
                Outline::On => "True",
                Outline::Only => "'only'",
            }
        );
        let _ = writeln!(w, "overlap_color = {}", self.overlap_color.py());
        let _ = writeln!(w, "plane_tolerance = {}", py_f(self.plane_tolerance));
        match &self.colors {
            Some(cs) => {
                let _ = writeln!(w, "colors = {{");
                for d in cs {
                    let _ = writeln!(w, "    _Domain({}, {:?}): {},", d.id, d.name, d.colour.py());
                }
                let _ = writeln!(w, "}}");
            }
            None => {
                let _ = writeln!(w, "colors = None");
            }
        }
        match (&seeded, self.seed) {
            (Some(domains), Some(seed)) => {
                // SlicePlot.colorize (plots.py:626-655).
                let _ = writeln!(w, "_domains = [");
                for (id, name) in domains {
                    let _ = writeln!(w, "    _Domain({id}, {name:?}),");
                }
                let _ = writeln!(
                    w,
                    "]\nif colors is None:\n    colors = {{}}\n    rng = np.random.RandomState({seed})\n    for domain in _domains:\n        colors[domain] = rng.randint(0, 256, (3,))"
                );
            }
            _ => {}
        }
        let kw = |v: &[(String, String)]| {
            v.iter()
                .map(|(k, e)| format!("{k:?}: {e}"))
                .collect::<Vec<_>>()
                .join(", ")
        };
        let _ = writeln!(w, "legend_kwargs = {{{}}}", kw(&self.legend_kwargs));
        let _ = writeln!(w, "source_kwargs = {{{}}}", kw(&self.source_kwargs));
        let _ = writeln!(
            w,
            "contour_kwargs = {}",
            if self.contour_kwargs.is_empty() {
                "None".to_string()
            } else {
                format!("{{{}}}", kw(&self.contour_kwargs))
            }
        );
        let _ = writeln!(w, "kwargs = {{{}}}", kw(&self.imshow_kwargs));
        match self.source_points.as_ref().filter(|p| !p.is_empty()) {
            Some(pts) => {
                let flat: Vec<f64> = pts.iter().flat_map(|p| [p.x, p.y, p.z]).collect();
                let _ = writeln!(
                    w,
                    "_SRC = '{}'\nsource_r = np.frombuffer(zlib.decompress(base64.b64decode(_SRC)), dtype='<f8').reshape(-1, 3)",
                    b64_zlib_f64(&flat)
                );
            }
            None => {
                let _ = writeln!(w, "source_r = None");
            }
        }
        s.push_str(PY_BODY);
        if let Some(t) = &self.title {
            let _ = writeln!(s, "axes.set_title({t:?})");
        }
        let extra: String = self
            .savefig_kwargs
            .iter()
            .map(|(k, v)| format!(", {k}={v}"))
            .collect();
        let _ = writeln!(
            s,
            "plt.savefig(sys.argv[1] if len(sys.argv) > 1 else {:?}{extra})",
            self.default_output
        );
        Ok(s)
    }

    /// `Universe.plot` (`universe.py:335-341`): the same plot of a model
    /// whose root is universe `u`.
    ///
    /// # Errors
    /// As [`Self::emit`].
    pub fn emit_universe<M: MaterialIdentity>(
        &self,
        geom: &Geometry,
        u: usize,
        materials: &[M],
    ) -> Result<String, ModelPlotError> {
        let mut g = geom.clone();
        g.root_universe = u;
        self.emit(&g, materials)
    }

    /// `Cell.plot` (`cell.py:596-606`): a throw-away universe holding only
    /// cell `c`.
    ///
    /// # Errors
    /// As [`Self::emit`].
    pub fn emit_cell<M: MaterialIdentity>(
        &self,
        geom: &Geometry,
        c: usize,
        materials: &[M],
    ) -> Result<String, ModelPlotError> {
        let mut g = geom.clone();
        g.universes.push(Universe {
            id: i32::MAX,
            cell_indices: vec![c],
        });
        g.root_universe = g.universes.len() - 1;
        self.emit(&g, materials)
    }

    /// `Region.plot` (`region.py:348-362`): a void cell with this region
    /// (auto id: one past the largest cell id), plotted as [`Self::emit_cell`].
    /// Upstream ignores `color_by`, `colors`, `legend`, `legend_kwargs` here
    /// (with a warning); so does this.
    ///
    /// # Errors
    /// As [`Self::emit`].
    pub fn emit_region<M: MaterialIdentity>(
        &self,
        geom: &Geometry,
        region: Vec<RegionToken>,
        materials: &[M],
    ) -> Result<String, ModelPlotError> {
        let mut g = geom.clone();
        let id = g.cells.iter().map(|c| c.id).max().unwrap_or(0) + 1;
        let mut cell = g.cells.first().cloned().expect("geometry has a cell");
        cell.id = id;
        cell.region = region;
        cell.fill = CellFill::Void;
        cell.translation = Position::new(0.0, 0.0, 0.0);
        g.cells.push(cell);
        let mut p = self.clone();
        p.color_by = ColorBy::Cell;
        p.colors = None;
        p.legend = false;
        p.legend_kwargs.clear();
        p.emit_cell(&g, g.cells.len() - 1, materials)
    }
}

impl Default for ModelPlot {
    fn default() -> Self {
        Self::new()
    }
}

fn material_id<M: MaterialIdentity>(
    material: Option<usize>,
    materials: &[M],
) -> Result<i32, ModelPlotError> {
    match material {
        None => Ok(-1),
        Some(m) => Ok(materials
            .get(m)
            .ok_or(ModelPlotError::MaterialIndex(m))?
            .material_id()),
    }
}

/// The overlap key `{universe id, min cell id, max cell id}` upstream's
/// `check_cell_overlap` (`src/geometry.cpp:38-90`, `error = false`) records
/// for a located point: at each level the first *other* cell of that level's
/// universe that contains the point, the deepest such level winning (each
/// level overwrites `overlap_index`).
fn overlap_key(
    geom: &Geometry,
    path: &crate::csg::geometry::GeometryPath,
) -> Option<(i32, i32, i32)> {
    let mut key = None;
    for coord in &path.levels {
        let univ = &geom.universes[coord.universe];
        for &i_cell in &univ.cell_indices {
            if geom.cells[i_cell].contains(coord.r, coord.u, &geom.surfaces, path.on_surface)
                && i_cell != coord.cell
            {
                let (a, b) = (geom.cells[i_cell].id, geom.cells[coord.cell].id);
                key = Some((univ.id, a.min(b), a.max(b)));
                break;
            }
        }
    }
    key
}

/// The id map `openmc_slice_data` returns with `color_overlaps = true`
/// (`src/plot.cpp:1947-2010`): it fills a `RasterData`, whose
/// `set_overlap` (`src/plot.cpp:153-164`) writes `OVERLAP - overlap_idx - 1`
/// (i.e. `-4, -5, ...`) into the **cell** channel and `OVERLAP = -3` into
/// the material channel. `overlap_idx` numbers distinct overlap keys in the
/// order they are first met. Upstream meets them inside an OpenMP loop over
/// rows; this port meets them in row-major order, which is what upstream does
/// on one thread, and what it does on any thread count when the slice holds a
/// single overlapping pair.
///
/// A consequence of upstream's own code, kept: `_id_map_to_rgb` colours only
/// `-3`, so with `color_by='cell'` an overlap is drawn **white**, not in
/// `overlap_color`; with `color_by='material'` it gets `overlap_color`.
fn overlap_id_map<M: MaterialIdentity>(
    geom: &Geometry,
    sp: &SlicePlot,
    materials: &[M],
) -> Result<(Vec<i32>, Vec<i32>), ModelPlotError> {
    #[cfg(not(target_arch = "wasm32"))]
    use rayon::prelude::*;
    #[cfg(target_arch = "wasm32")]
    use crate::wasm_par::prelude::*;
    use crate::csg::cell::SurfaceToken;

    let (w, h) = (sp.pixels[0], sp.pixels[1]);
    let dir = super::slice::plot_direction();
    // (cell id, material, overlap key) per pixel, computed in parallel.
    type Px = (i32, Option<Option<usize>>, Option<(i32, i32, i32)>);
    let rows: Vec<Vec<Px>> = (0..h)
        .into_par_iter()
        .map(|y| {
            (0..w)
                .map(|x| {
                    let r = sp.pixel_centre(x, y);
                    match geom.locate(r, dir, SurfaceToken::NONE) {
                        None => (-2, None, None),
                        Some(path) => {
                            if let Some(k) = overlap_key(geom, &path) {
                                return (-3, None, Some(k));
                            }
                            let cell = path.levels.last().map_or(-2, |c| geom.cells[c.cell].id);
                            (cell, Some(path.material), None)
                        }
                    }
                })
                .collect()
        })
        .collect();
    let mut keys: Vec<(i32, i32, i32)> = Vec::new();
    let mut cells = Vec::with_capacity(w * h);
    let mut mats = Vec::with_capacity(w * h);
    for (cell, material, key) in rows.into_iter().flatten() {
        if let Some(k) = key {
            let idx = keys.iter().position(|q| *q == k).unwrap_or_else(|| {
                keys.push(k);
                keys.len() - 1
            });
            cells.push(-3 - idx as i32 - 1);
            mats.push(-3);
        } else {
            cells.push(cell);
            mats.push(match material {
                None => -2,
                Some(m) => material_id(m, materials)?,
            });
        }
    }
    Ok((cells, mats))
}

/// The body of `Model.plot` after the id map (`model.py:1374-1543`),
/// transcribed. Lines that only validate input or talk to `openmc.lib` are
/// done in Rust before emission; everything that shapes pixels is here.
const PY_BODY: &str = r#"
# ---- openmc/model/model.py:1374-1382 ------------------------------------------
if legend_kwargs is None:
    legend_kwargs = {}
legend_kwargs.setdefault('bbox_to_anchor', (1.05, 1))
legend_kwargs.setdefault('loc', 2)
legend_kwargs.setdefault('borderaxespad', 0.0)
if source_kwargs is None:
    source_kwargs = {}
source_kwargs.setdefault('marker', 'x')

xlabel, ylabel = f'{basis[0]} [{axis_units}]', f'{basis[1]} [{axis_units}]'

axis_scaling_factor = {'km': 0.00001, 'm': 0.01, 'cm': 1, 'mm': 10}

x_min = (origin[x] - 0.5*width[0]) * axis_scaling_factor[axis_units]
x_max = (origin[x] + 0.5*width[0]) * axis_scaling_factor[axis_units]
y_min = (origin[y] - 0.5*width[1]) * axis_scaling_factor[axis_units]
y_max = (origin[y] + 0.5*width[1]) * axis_scaling_factor[axis_units]

# ---- openmc/plots.py:360-437 (_id_map_to_rgb) ---------------------------------
def _id_map_to_rgb(id_map, color_by='cell', colors=None, overlap_color=(255, 0, 0)):
    img = np.ones(id_map.shape, dtype=float)
    if color_by == 'cell':
        id_index = 0
    elif color_by == 'material':
        id_index = 2
    else:
        raise ValueError("color_by must be either 'cell' or 'material'")
    unique_ids = np.unique(id_map[:, :, id_index])
    if colors is None:
        colors = {}
    color_map = {}
    for key, color in colors.items():
        if isinstance(key, _Domain):
            color_map[key.id] = color
        else:
            color_map[key] = color
    rng = np.random.RandomState(1)
    for uid in unique_ids:
        if uid > 0 and uid not in color_map:
            color_map[uid] = rng.randint(0, 256, (3,))
    for uid in unique_ids:
        if uid == -1:
            continue
        elif uid == -3:
            if isinstance(overlap_color, str):
                rgb = _SVG_COLORS[overlap_color.lower()]
            else:
                rgb = overlap_color
            mask = id_map[:, :, id_index] == uid
            img[mask] = np.array(rgb) / 255.0
        elif uid in color_map:
            color = color_map[uid]
            if isinstance(color, str):
                rgb = _SVG_COLORS[color.lower()]
            else:
                rgb = color
            mask = id_map[:, :, id_index] == uid
            img[mask] = np.array(rgb) / 255.0
    return img

# ---- openmc/model/model.py:1420-1543 ------------------------------------------
img = _id_map_to_rgb(
    id_map=id_map,
    color_by=color_by,
    colors=colors,
    overlap_color=overlap_color
)

px = 1/plt.rcParams['figure.dpi']
fig, axes = plt.subplots()
axes.set_xlabel(xlabel)
axes.set_ylabel(ylabel)
params = fig.subplotpars
width_px = pixels[0]*px/(params.right - params.left)
height_px = pixels[1]*px/(params.top - params.bottom)
fig.set_size_inches(width_px, height_px)

if outline:
    rgb = (img * 256).astype(int)
    image_value = (rgb[..., 0] << 16) + \
        (rgb[..., 1] << 8) + (rgb[..., 2])
    if contour_kwargs is None:
        contour_kwargs = {}
    contour_kwargs.setdefault('colors', 'k')
    contour_kwargs.setdefault('linestyles', 'solid')
    contour_kwargs.setdefault('algorithm', 'serial')
    axes.contour(
        image_value,
        origin="upper",
        levels=np.unique(image_value),
        extent=(x_min, x_max, y_min, y_max),
        **contour_kwargs
    )
    if outline == 'only':
        axes.set_xlim(x_min, x_max)
        axes.set_ylim(y_min, y_max)
        axes.set_aspect('equal')

if legend:
    if colors is None or len(colors) == 0:
        raise ValueError("Must pass 'colors' dictionary if you "
                         "are adding a legend via legend=True.")
    patches = []
    for key, color in colors.items():
        label = key.name if key.name != '' else key.id
        if len(color) == 3 and not isinstance(color, str):
            scaled_color = (
                color[0]/255, color[1]/255, color[2]/255)
        else:
            scaled_color = color
        key_patch = mpatches.Patch(color=scaled_color, label=label)
        patches.append(key_patch)
    axes.legend(handles=patches, **legend_kwargs)

if outline != 'only':
    axes.imshow(img, extent=(x_min, x_max, y_min, y_max), **kwargs)

if source_r is not None:
    slice_value = origin[z]
    xs = []
    ys = []
    tol = plane_tolerance
    for r in source_r:
        if (slice_value - tol < r[z] < slice_value + tol):
            xs.append(r[x] * axis_scaling_factor[axis_units])
            ys.append(r[y] * axis_scaling_factor[axis_units])
    axes.scatter(xs, ys, **source_kwargs)
"#;

fn py_bool(b: bool) -> &'static str {
    if b {
        "True"
    } else {
        "False"
    }
}

/// A float as a Python literal that parses to the same double (Rust's `{:?}`
/// is the shortest round-trip form; infinities spelled out).
fn py_f(v: f64) -> String {
    if v == f64::INFINITY {
        "float('inf')".into()
    } else if v == f64::NEG_INFINITY {
        "float('-inf')".into()
    } else if v.is_nan() {
        "float('nan')".into()
    } else {
        format!("{v:?}")
    }
}

fn b64_zlib_i32(v: &[i32]) -> String {
    let bytes: Vec<u8> = v.iter().flat_map(|x| x.to_le_bytes()).collect();
    base64(&miniz_oxide::deflate::compress_to_vec_zlib(&bytes, 9))
}

fn b64_zlib_f64(v: &[f64]) -> String {
    let bytes: Vec<u8> = v.iter().flat_map(|x| x.to_le_bytes()).collect();
    base64(&miniz_oxide::deflate::compress_to_vec_zlib(&bytes, 9))
}

/// Standard base64 (RFC 4648, with padding) — what `base64.b64decode` reads.
fn base64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            T[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            T[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}
