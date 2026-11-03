// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// This file is part of OUTRAM PARK.

//! **The HTR-10 core coloured by temperature zone** (gh:#1138, the drawing
//! rule): the assembled geometry of `ParametricStudy`'s zoned design, each
//! cell coloured by the zone whose slider sets its temperature
//! (`zones::zone_palette`), so a reader can check which cells each of the
//! eight sliders moves.
//!
//! ```text
//! cargo run --release -p nee_soon --example htr10_zone_images
//! ```
//!
//! Writes PNGs to `verification_and_validation/htr10_zone_temperatures/images/`
//! (`OUTRAM_HTR10_IMG_OUT` overrides): the whole model in x-z at y = 0, x-y
//! slices at bed mid-height, through the conus and through the top
//! reflector, and an x-z close-up of the bed edge with its fuel and
//! moderator pebbles. Image glue (exempt from the reaching-test rule: the
//! palette and the geometry it draws are tested in the library).

use std::path::PathBuf;

use nee_soon::htr10_rmc::core_model::HTR10_REFLECTOR_OUTER_CM;
use nee_soon::htr10_rmc::parametric::{ParametricStudy, StudyBed};
use nee_soon::htr10_rmc::zones::zone_palette;
use outram_mc_libs::geometry::plot::{render_material_slice, PlotBasis, SlicePlot};
use outram_mc_libs::geometry::position::Position;

fn main() -> Result<(), String> {
    let out = std::env::var("OUTRAM_HTR10_IMG_OUT").map_or_else(
        |_| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("verification_and_validation/htr10_zone_temperatures/images")
        },
        PathBuf::from,
    );
    std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
    let study = ParametricStudy {
        bed: StudyBed::Seker { layers: 12 },
        ..ParametricStudy::default()
    };
    let plan = study.plan().map_err(|e| e.to_string())?;
    let core = study.core(&plan)?;
    let g = &core.geometry;
    let pal = zone_palette();
    let render = |name: &str, plot: SlicePlot, what: &str| -> Result<(), String> {
        let (_raw, img) = render_material_slice(g, &plot, &pal, &format!("HTR-10 N=12 BY ZONE: {what}"));
        let path = out.join(name);
        img.write_png(&path).map_err(|e| e.to_string())?;
        println!("wrote {}", path.display());
        Ok(())
    };
    let half_w = HTR10_REFLECTOR_OUTER_CM + 5.0;
    let (z_lo, z_hi) = (core.refl_bottom - 5.0, core.refl_top + 5.0);
    let px = |w: f64, cm: f64| (w / cm).round() as usize;
    let (w, h) = (2.0 * half_w, z_hi - z_lo);
    render(
        "htr10_zones_xz_full.png",
        SlicePlot::new(PlotBasis::Xz, Position::new(0.0, 0.0, 0.5 * (z_lo + z_hi)), [w, h], [px(w, 0.4), px(h, 0.4)]),
        "X-Z, Y = 0",
    )?;
    let xy = |z: f64| SlicePlot::new(PlotBasis::Xy, Position::new(0.0, 0.0, z), [w, w], [px(w, 0.4), px(w, 0.4)]);
    render("htr10_zones_xy_bed_mid.png", xy(0.0), "X-Y AT BED MID-HEIGHT")?;
    render(
        "htr10_zones_xy_conus.png",
        xy(0.5 * (core.conus_floor - core.bed_half_height)),
        "X-Y THROUGH THE CONUS",
    )?;
    render("htr10_zones_xy_top_reflector.png", xy(core.refl_top - 60.0), "X-Y, TOP REFLECTOR (60 CM BELOW THE TOP)")?;
    render(
        "htr10_zones_xz_bed_edge.png",
        SlicePlot::new(PlotBasis::Xz, Position::new(75.0, 0.0, 0.0), [40.0, 40.0], [800, 800]),
        "X-Z CLOSE-UP, BED EDGE (FUEL AND MODERATOR PEBBLES)",
    )?;
    Ok(())
}
