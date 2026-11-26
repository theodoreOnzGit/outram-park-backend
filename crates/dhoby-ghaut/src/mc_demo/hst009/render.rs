//! Geometry review images for the HEU-SOL-THERM-009 rung — the crate's
//! drawing rule (dhoby-ghaut `CLAUDE.md`).
//!
//! Drawn with `outram_mc_libs::geometry::plot::render_material_slice` from
//! the ASSEMBLED geometry the rung transports through
//! (`outram_mc_libs::vv::hst009::geometry`): every pixel is the material the
//! solver finds there. Two views through the centre: the whole model (the
//! solution, the tank and the water reflector to 35 cm), and a zoom on the
//! tank wall, which is 0.1587 cm thick and a line at the first view's scale.
//! The model is the same in every plane through its centre.

use outram_mc_libs::geometry::plot::{render_material_slice, PlotBasis, Rgb, SlicePlot};
use outram_mc_libs::geometry::position::Position;
use outram_mc_libs::vv::hst009 as model;
use std::path::Path;

pub fn render_all(dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let geom = model::geometry();
    let pal = vec![
        (Rgb::new(214, 170, 60), "UO2F2 solution (HEU)"),
        (Rgb::new(170, 170, 176), "1100 aluminium tank"),
        (Rgb::new(40, 90, 170), "Water reflector"),
    ];
    let whole = 2.2 * model::R_REFLECTOR;
    let wall = 1.0;
    let views = [
        (
            "hst009_1_xz.png",
            Position::new(0.0, 0.0, 0.0),
            whole,
            format!("HEU-SOL-THERM-009 x-z, y = 0: solution r = {} cm, Al tank to {} cm, water to {} cm", model::R_SOLUTION, model::R_TANK, model::R_REFLECTOR),
        ),
        (
            "hst009_2_tank_wall_xz.png",
            Position::new(0.5 * (model::R_SOLUTION + model::R_TANK), 0.0, 0.0),
            wall,
            format!("HEU-SOL-THERM-009 tank wall, {wall} cm wide view at x = {} to {} cm: solution, aluminium, water", model::R_SOLUTION, model::R_TANK),
        ),
    ];
    for (file, origin, w, title) in views {
        let plot = SlicePlot::new(PlotBasis::Xz, origin, [w, w], [700, 700]);
        let (_raw, img) = render_material_slice(&geom, &plot, &pal, &title);
        img.write_png(dir.join(file)).map_err(|e| e.to_string())?;
        println!("wrote {file}");
    }
    Ok(())
}
