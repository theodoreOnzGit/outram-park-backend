//! Geometry review images — the crate's drawing rule (dhoby-ghaut `CLAUDE.md`).
//!
//! Drawn with `outram_mc_libs::geometry::plot::render_material_slice`, the
//! workspace's port of OpenMC's plotter (verified pixel-for-pixel against
//! `openmc --plot`): every pixel is the material the SOLVER finds there, on
//! the assembled geometry — never the named constants. Five views: the whole
//! cell, a quadrant, one TRISO particle, and two axial (x-z) slices that show
//! the model is z-invariant and bounded by the reflective z-planes. The legend
//! lists `OUTSIDE MODEL` only if some pixel finds no cell; inside the
//! reflective cell, it must not.

use crate::model;
use outram_mc_libs::geometry::plot::{render_material_slice, PlotBasis, Rgb, SlicePlot};
use outram_mc_libs::geometry::position::Position;
use std::path::Path;

/// Same colours as the interactive view, except outer PyC is lighter here so
/// the two PyC layers can be told apart in a review image.
fn palette() -> Vec<(Rgb, &'static str)> {
    let c = [
        Rgb::new(214, 120, 46),
        Rgb::new(58, 58, 62),
        Rgb::new(150, 150, 154),
        Rgb::new(200, 176, 112),
        Rgb::new(196, 196, 200),
        Rgb::new(92, 94, 98),
        Rgb::new(74, 76, 80),
    ];
    c.into_iter().zip(model::MATERIAL_NAMES).collect()
}

pub fn render_all(dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let centres = model::particle_centres(model::LAYOUT_SEED);
    let geom = model::build_geometry(&centres);
    let p = model::half_pitch();
    // The particle nearest the centre: the single-particle and axial views cut it.
    let &(cx, cy) = centres
        .iter()
        .min_by(|a, b| a.0.hypot(a.1).total_cmp(&b.0.hypot(b.1)))
        .ok_or("no particles")?;
    let w = model::particle_r() * 1.25;
    let pal = palette();
    let views = [
        (
            "1_cell_xy.png",
            SlicePlot::new(PlotBasis::Xy, Position::new(0.0, 0.0, 0.0), [2.0 * p, 2.0 * p], [900, 900]),
            format!(
                "x-y, z = 0. Reflective cell half-pitch {p:.4} cm, pebble R {} cm, fuel zone R {} cm, {} TRISO rods",
                model::PEBBLE_R, model::FUEL_ZONE_R, centres.len()
            ),
        ),
        (
            "2_quadrant_xy.png",
            SlicePlot::new(PlotBasis::Xy, Position::new(1.6, 1.6, 0.0), [3.2, 3.2], [900, 900]),
            "x-y, z = 0, upper-right quadrant: fuel-zone edge, fuel-free shell, helium".to_string(),
        ),
        (
            "3_one_particle_xy.png",
            SlicePlot::new(PlotBasis::Xy, Position::new(cx, cy, 0.0), [2.0 * w, 2.0 * w], [900, 900]),
            format!(
                "x-y, one particle at ({cx:.4}, {cy:.4}). Outer radii cm: kernel {:.4} buffer {:.4} IPyC {:.4} SiC {:.4} OPyC {:.4}",
                model::KERNEL_R, model::buffer_r(), model::ipyc_r(), model::sic_r(), model::particle_r()
            ),
        ),
        (
            "4_axial_xz_near_z0.png",
            SlicePlot::new(PlotBasis::Xz, Position::new(0.0, cy, 0.0), [2.0 * p, 2.0 * p], [900, 900]),
            format!("x-z at y = {cy:.4} (through the centre-most particle): every object is a z-invariant rod"),
        ),
        (
            "5_axial_xz_full_height.png",
            SlicePlot::new(PlotBasis::Xz, Position::new(0.0, cy, 0.0), [2.0 * p, 110.0], [300, 900]),
            "x-z, full height: reflective z-planes at +/-50 cm bound the model".to_string(),
        ),
    ];
    for (file, plot, title) in views {
        let (_raw, img) = render_material_slice(&geom, &plot, &pal, &title);
        img.write_png(dir.join(file)).map_err(|e| e.to_string())?;
        println!("wrote {file}");
    }
    Ok(())
}
