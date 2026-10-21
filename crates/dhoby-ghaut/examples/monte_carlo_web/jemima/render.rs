//! Geometry review images for the Jemima rung — the crate's drawing rule
//! (dhoby-ghaut `CLAUDE.md`).
//!
//! Drawn with `outram_mc_libs::geometry::plot::render_material_slice` from
//! the ASSEMBLED geometry the rung transports through
//! (`outram_mc_libs::vv::jemima::geometry`): every pixel is the material the
//! solver finds there. Two views: the side view (x-z through the axis), which
//! shows the core inside its bottom, radial and top reflectors, and the top
//! view (x-y at the core's mid-height). The legend lists `OUTSIDE MODEL` for
//! the vacuum outside, where no cell exists by design.

use outram_mc_libs::geometry::plot::{render_material_slice, PlotBasis, Rgb, SlicePlot};
use outram_mc_libs::geometry::position::Position;
use outram_mc_libs::vv::jemima as model;
use std::path::Path;

pub fn render_all(dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let geom = model::geometry();
    let pal = vec![(Rgb::new(196, 92, 60), "Core: U metal, 16 % U-235"), (Rgb::new(96, 84, 128), "Reflector: natural U")];
    let zc = 0.5 * (model::Z_BOT + model::Z_TOP);
    let zm = 0.5 * (model::Z_CORE_LO + model::Z_CORE_HI);
    let w = 2.3 * model::R_OUT;
    let views = [
        (
            "jemima_1_xz.png",
            PlotBasis::Xz,
            Position::new(0.0, 0.0, zc),
            format!("Jemima x-z, y = 0: core r = {} cm, z {}-{} cm, reflector to r = {}, z {}-{}", model::R_CORE, model::Z_CORE_LO, model::Z_CORE_HI, model::R_OUT, model::Z_BOT, model::Z_TOP),
        ),
        ("jemima_2_xy.png", PlotBasis::Xy, Position::new(0.0, 0.0, zm), format!("Jemima x-y, z = {zm:.2} cm (core mid-height): core r = {} cm in the reflector to {} cm", model::R_CORE, model::R_OUT)),
    ];
    for (file, basis, origin, title) in views {
        let plot = SlicePlot::new(basis, origin, [w, w], [700, 700]);
        let (_raw, img) = render_material_slice(&geom, &plot, &pal, &title);
        img.write_png(dir.join(file)).map_err(|e| e.to_string())?;
        println!("wrote {file}");
    }
    Ok(())
}
