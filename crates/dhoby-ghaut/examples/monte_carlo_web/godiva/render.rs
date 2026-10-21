//! Geometry review images for the Godiva rung — the crate's drawing rule
//! (dhoby-ghaut `CLAUDE.md`).
//!
//! Drawn with `outram_mc_libs::geometry::plot::render_material_slice` from
//! the ASSEMBLED geometry the Watch mode transports through
//! ([`super::model::build_geometry`]): every pixel is the material the solver
//! finds there. Two views: an x-y slice through the centre and an x-z slice
//! (the sphere is the same in every plane through its centre, so these two
//! show any defect in the surface or the cell). The legend lists
//! `OUTSIDE MODEL` for the vacuum outside, where no cell exists by design.
//!
//! (The Run k_eff mode does not use this geometry: `PowerIteration` builds its
//! own sphere from the same [`super::model::RADIUS_CM`].)

use super::model;
use outram_mc_libs::geometry::plot::{render_material_slice, PlotBasis, Rgb, SlicePlot};
use outram_mc_libs::geometry::position::Position;
use std::path::Path;

pub fn render_all(dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let geom = model::build_geometry();
    let r = model::RADIUS_CM;
    let pal = vec![(Rgb::new(150, 120, 90), "HEU metal (U-234/235/238)")];
    let w = 2.4 * r;
    let views = [
        ("godiva_1_xy.png", PlotBasis::Xy, format!("Godiva x-y, z = 0: HEU sphere r = {r} cm, vacuum outside")),
        ("godiva_2_xz.png", PlotBasis::Xz, format!("Godiva x-z, y = 0: HEU sphere r = {r} cm, vacuum outside")),
    ];
    for (file, basis, title) in views {
        let plot = SlicePlot::new(basis, Position::new(0.0, 0.0, 0.0), [w, w], [700, 700]);
        let (_raw, img) = render_material_slice(&geom, &plot, &pal, &title);
        img.write_png(dir.join(file)).map_err(|e| e.to_string())?;
        println!("wrote {file}");
    }
    Ok(())
}
