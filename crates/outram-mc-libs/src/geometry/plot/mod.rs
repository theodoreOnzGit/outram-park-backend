// SPDX-License-Identifier: GPL-3.0

//! **Geometry plotting** — OpenMC's plotter (`src/plot.cpp`), ported, writing
//! PNG natively from Rust. GitHub #268.
//!
//! # What changed, and why this module is shaped the way it is
//!
//! ~~"The native rasteriser is NOT ported ... emits a matplotlib script"~~ —
//! **REVERSED 2026-09-25 by maintainer direction**: *"make sure the plotting
//! capabilities of openmc are properly ported over (to jpg or PNG)"*. The
//! earlier decision (2026-09-22) kept this crate free of an image encoder and
//! emitted a Python script instead; that path still exists, unchanged in
//! behaviour, in [`script`] (re-exported here as [`sample_slice`],
//! [`emit_python`], [`ColourBy`], [`Slice`]).
//!
//! # What is ported (OpenMC d7d3284a1)
//!
//! | Here | Upstream | Lines |
//! |---|---|---|
//! | [`colour::random_colour`], [`colour::default_colours`] | `random_color`, `set_default_colors` | `plot.cpp:1179-1183`, `:580-596` |
//! | [`colour::ColourScheme`] builders | `set_user_colors`, `set_mask`, `set_bg_color`, `set_overlap_color` | `plot.cpp:598-633`, `:743-827`, `:466-477` |
//! | [`slice::SlicePlot::id_map`] | `SlicePlotBase::get_map<IdData>`, `IdData::set_value` | `plot.h:222-293`, `plot.cpp:47-79` |
//! | [`slice::SlicePlot::create_image`] | `Plot::create_image` | `plot.cpp:317-358` |
//! | [`slice::check_cell_overlap`] | `check_cell_overlap` | `geometry.cpp:38-90` |
//! | mesh lines (private) | `Plot::draw_mesh_lines`, `RegularMesh::plot` | `plot.cpp:941-1053`, `mesh.cpp:1628-1667` |
//! | [`raytrace::Camera`] | `RayTracePlot::update_view`, `get_pixel_ray` | `plot.cpp:1200-1219`, `:1326-1367` |
//! | ray tracer (private) | `Ray::trace`, `advance_to_boundary_from_void` | `ray.cpp:14-143`, `particle_data.cpp:59-84` |
//! | [`raytrace::WireframeRayTracePlot`] | `WireframeRayTracePlot::create_image`, `trackstack_equivalent`, `ProjectionRay` | `plot.cpp:1369-1529`, `:1265-1324`, `:1750-1763` |
//! | [`raytrace::SolidRayTracePlot`] | `SolidRayTracePlot::create_image`, `PhongRay` | `plot.cpp:1683-1701`, `:1765-1891` |
//! | [`image::ImageData::write_png`] / [`image::ImageData::write_ppm`] | `output_png` / `output_ppm` | `plot.cpp:887-935` / `:857-881` |
//!
//! Every function above carries its own upstream line range in its doc
//! comment.
//!
//! # What is NOT ported, and why
//!
//! - **Voxel plots** (`Plot::create_voxel`, `plot.cpp:1065-1177`). They write
//!   an HDF5 volume, not an image; HDF5 output belongs to
//!   `njoy-outram-park-fork` (gh:#270), and nothing here needs it.
//! - **`plots.xml` parsing** — the crate reads no XML (see its `CLAUDE.md`).
//!   Plots are built with Rust constructors whose fields name the XML elements.
//! - **`openmc_*` C API** entry points (`plot.cpp:1893-2621`) — the Python
//!   binding layer; this crate's API *is* the Rust types.
//! - **Property maps** (temperature/density, `PropertyData`) and **tally-filter
//!   bins** in `RasterData` — used by the interactive plotter, not by image
//!   output.
//! - **JPEG.** Lossy compression blurs the boundaries a geometry plot exists
//!   to show, and OpenMC itself writes only PNG/PPM; PNG is sufficient and
//!   smaller for flat-colour images. See [`image`].
//! - **Non-regular meshes for mesh lines** — the crate has only
//!   [`crate::tally::mesh::RegularMesh`].
//! - **Cell rotations** in the Phong normal (`plot.cpp:1828-1833`) — this
//!   crate's nested frames are pure translations, so there is nothing to undo.
//!
//! Known behavioural differences of the ray tracer are listed in
//! [`raytrace`]; measured agreement with `openmc --plot` is in
//! `verification_and_validation/geometry_plotting/README.md`.
//!
//! # Drawing a reactor model (the geometry-drawing HARD RULE)
//!
//! [`render_material_slice`] is the one-call path: slice the **assembled**
//! geometry, colour by material with a caller-chosen palette, and frame it
//! with a legend and dimensioned axes ([`annotate::annotate_slice`]).
//! `crates/nee_soon/examples/htr10_geometry_images.rs` uses it on the HTR-10
//! core.

pub mod annotate;
pub mod colour;
pub mod image;
pub mod raytrace;
pub mod script;
pub mod slice;

pub use annotate::{annotate_image, annotate_slice, LegendEntry};
pub use colour::{
    default_colours, random_colour, ColourScheme, PlotColourBy, Rgb, BLACK, DEFAULT_PLOTTER_SEED,
    RED, WHITE,
};
pub use image::{decode_png, ImageData, PngDecodeError};
pub use raytrace::{Camera, Projection, SolidRayTracePlot, WireframeRayTracePlot};
pub use script::{emit_python, sample_slice, ColourBy, Slice};
pub use slice::{IdMap, MeshLines, PlotBasis, SliceHit, SlicePlot};

use crate::geometry::geometry::Geometry;

/// Highest material index any cell fills with, plus one — the smallest
/// material table a [`ColourScheme`] for this geometry can use. (OpenMC sizes
/// its table to the full materials list, which may be longer; pass that
/// length instead when reproducing an OpenMC run's colours.)
#[must_use]
pub fn material_count(geom: &Geometry) -> usize {
    geom.cells
        .iter()
        .filter_map(|c| match c.fill {
            crate::geometry::cell::CellFill::Material(m) => Some(m + 1),
            _ => None,
        })
        .max()
        .unwrap_or(0)
}

/// **Draw a slice of an assembled geometry by material, with a legend and
/// dimensioned axes** — the geometry-drawing rule's minimum, in one call.
///
/// `palette[i]` is `(colour, label)` for material index `i`; materials past
/// the end of `palette` get OpenMC's default colours (seed
/// [`DEFAULT_PLOTTER_SEED`]), labelled `MATERIAL <i>`. Only materials that
/// actually appear in the slice are listed in the legend, plus `VOID` and
/// `OUTSIDE MODEL` when present. Returns `(raw, annotated)`: the raw image is
/// OpenMC-parity pixels, the annotated one is for a human.
#[must_use]
pub fn render_material_slice(
    geom: &Geometry,
    plot: &SlicePlot,
    palette: &[(Rgb, &str)],
    title: &str,
) -> (ImageData, ImageData) {
    let n = material_count(geom).max(palette.len());
    let mut seed = DEFAULT_PLOTTER_SEED;
    let mut scheme = ColourScheme::new(PlotColourBy::Material, n, &mut seed)
        .with_background(Rgb::new(235, 235, 235));
    for (i, (c, _)) in palette.iter().enumerate() {
        scheme = scheme.with_colour(i, *c);
    }
    let ids = plot.id_map(geom);
    let raw = plot.colour_id_map(&ids, &scheme);

    let mut present = vec![false; n];
    let mut void = false;
    let mut outside = false;
    for h in &ids.hits {
        match *h {
            SliceHit::Found {
                material: Some(m), ..
            } => present[m] = true,
            SliceHit::Found { material: None, .. } => void = true,
            SliceHit::NotFound => outside = true,
            SliceHit::Overlap => {}
        }
    }
    let mut legend: Vec<LegendEntry> = present
        .iter()
        .enumerate()
        .filter(|(_, p)| **p)
        .map(|(i, _)| {
            let label = palette
                .get(i)
                .map_or_else(|| format!("MATERIAL {i}"), |(_, l)| (*l).to_string());
            LegendEntry::new(scheme.colours[i], label)
        })
        .collect();
    if void {
        legend.push(LegendEntry::new(WHITE, "VOID"));
    }
    if outside {
        legend.push(LegendEntry::new(scheme.background, "OUTSIDE MODEL"));
    }
    let annotated = annotate_slice(&raw, plot, title, &legend);
    (raw, annotated)
}
