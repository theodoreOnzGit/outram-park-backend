//! Geometry review images for the `ugraphite` rung (the crate's drawing rule).
//!
//! Drawn with `outram_mc_libs::geometry::plot::render_material_slice` from
//! the ASSEMBLED cube the Watch mode transports through
//! ([`super::model::build_geometry`]): every pixel is the material the
//! solver finds there. An x-y slice at z = 0 and an x-z slice at y = 0,
//! each a little wider than the cube, so the reflective walls at
//! ±50 cm and the absence of any cell outside them show.

use super::model;
use outram_mc_libs::geometry::plot::{render_material_slice, PlotBasis, Rgb, SlicePlot};
use outram_mc_libs::geometry::position::Position;
use std::path::Path;

pub fn render_all(dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let geom = model::build_geometry();
    let h = model::HALF_CM;
    let pal = vec![(Rgb::new(110, 104, 120), "U (17 wt% U-235) + graphite, N_C/N_U 767.2")];
    let views = [
        ("ugraphite_1_xy.png", PlotBasis::Xy, format!("rung 2, x-y at z = 0: homogeneous U + graphite, reflective cube, walls at x, y = -{h} and +{h} cm")),
        ("ugraphite_2_xz.png", PlotBasis::Xz, format!("rung 2, x-z at y = 0: the same mixture, reflective walls at z = -{h} and +{h} cm")),
    ];
    for (file, basis, title) in views {
        let plot = SlicePlot::new(basis, Position::new(0.0, 0.0, 0.0), [2.3 * h, 2.3 * h], [700, 700]);
        let (_raw, img) = render_material_slice(&geom, &plot, &pal, &title);
        img.write_png(dir.join(file)).map_err(|e| e.to_string())?;
        println!("wrote {file}");
    }
    Ok(())
}
