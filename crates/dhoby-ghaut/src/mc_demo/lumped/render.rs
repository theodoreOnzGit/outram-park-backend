//! Geometry review images for the `lumped` rung (the crate's drawing rule).
//!
//! Drawn with `outram_mc_libs::geometry::plot::render_material_slice` from
//! the ASSEMBLED cell the Watch mode transports through
//! ([`super::model::build_geometry`]): the whole cell in x-y and x-z through
//! its centre (the white boundary at `R`, nothing outside it), and a zoom on
//! the lump.

use super::model;
use outram_mc_libs::geometry::plot::{render_material_slice, PlotBasis, Rgb, SlicePlot};
use outram_mc_libs::geometry::position::Position;
use std::path::Path;

pub fn render_all(dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let geom = model::build_geometry();
    let (r, big_r) = (model::LUMP_R_CM, model::cell_r());
    let pal = vec![(Rgb::new(214, 120, 46), "natural U metal (lump)"), (Rgb::new(92, 94, 98), "graphite")];
    let views = [
        ("lumped_1_cell_xy.png", PlotBasis::Xy, 2.2 * big_r,
         format!("rung 3, x-y at z = 0: U lump r = {r} cm in graphite, white boundary R = {big_r:.3} cm")),
        ("lumped_2_cell_xz.png", PlotBasis::Xz, 2.2 * big_r,
         format!("rung 3, x-z at y = 0: the same cell (N_C/N_U = {} cell average)", model::C_PER_U)),
        ("lumped_3_lump_xy.png", PlotBasis::Xy, 3.2 * r, format!("rung 3, x-y at z = 0: the lump, r = {r} cm")),
    ];
    for (file, basis, w, title) in views {
        let plot = SlicePlot::new(basis, Position::new(0.0, 0.0, 0.0), [w, w], [700, 700]);
        let (_raw, img) = render_material_slice(&geom, &plot, &pal, &title);
        img.write_png(dir.join(file)).map_err(|e| e.to_string())?;
        println!("wrote {file}");
    }
    Ok(())
}
