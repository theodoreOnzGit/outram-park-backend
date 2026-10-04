//! Geometry review images for the LCT-008 rung (the crate's drawing rule).
//!
//! Drawn with `outram_mc_libs::geometry::plot::render_material_slice` from
//! the ASSEMBLED core the Watch mode transports through
//! ([`super::model::build_geometry`]), so every pixel is the material the
//! solver finds there. Three levels:
//! 1. the whole core in x-y at z = 0, with the 7 × 7 tiles, the all-water
//!    tiles and the vacuum outside the r = 76.2 cm cylinder;
//! 2. one assembly near the centre in x-y;
//! 3. a few pins;
//!
//! plus an x-z slice through y = 0 showing the axial extent
//! (|z| < 81.662 cm).

use super::model;
use outram_mc_libs::geometry::plot::{render_material_slice, PlotBasis, Rgb, SlicePlot};
use outram_mc_libs::geometry::position::Position;
use std::path::Path;

pub fn render_all(dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let mats = model::materials()?;
    let geom = model::build_geometry(&mats);
    let pal = vec![
        (Rgb::new(70, 130, 220), "Water, 1511 ppm boron"),
        (Rgb::new(220, 60, 40), "UO2 2.459 w/o"),
        (Rgb::new(150, 150, 150), "Al-6061 clad"),
    ];
    let r = model::R_CORE;
    let views = [
        ("lct008_1_core_xy.png", PlotBasis::Xy, Position::new(0.0, 0.0, 0.0), 2.1 * r, 1000,
         "LCT-008 case 1, x-y at z = 0: 7 x 7 core lattice of 15 x 15 pin lattices, vacuum outside r = 76.2 cm".to_string()),
        ("lct008_2_assembly_xy.png", PlotBasis::Xy, Position::new(0.0, 0.0, 0.0), 26.0, 900,
         "LCT-008, x-y: the central core tile (15 x 15 pins, pitch 1.63576 cm)".to_string()),
        ("lct008_3_pins_xy.png", PlotBasis::Xy, Position::new(0.0, 0.0, 0.0), 5.0, 700,
         "LCT-008, x-y: pins at the core centre (fuel r 0.514858, clad 0.602996 cm)".to_string()),
        ("lct008_4_core_xz.png", PlotBasis::Xz, Position::new(0.0, 0.0, 0.0), 2.3 * model::Z_HI, 1000,
         "LCT-008, x-z at y = 0: -81.662 < z < 81.662 cm, vacuum above and below".to_string()),
    ];
    for (file, basis, origin, w, px, title) in views {
        let plot = SlicePlot::new(basis, origin, [w, w], [px, px]);
        let (_raw, img) = render_material_slice(&geom, &plot, &pal, &title);
        img.write_png(dir.join(file)).map_err(|e| e.to_string())?;
        println!("wrote {file}");
    }
    Ok(())
}
