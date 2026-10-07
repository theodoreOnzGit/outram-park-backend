//! The `htr10` rung's geometry review images (the crate's drawing rule):
//! the core the zoom ladder slices, at N = 12, drawn from the ASSEMBLED
//! geometry with the ported plotter and `nee_soon`'s palette. The full set,
//! checked by eye, is `nee_soon`'s `htr10_geometry_images/`; these three
//! show the ladder's own planes.

use super::{model, LADDER_N, LADDER_Z, PARTICLE_XY};
use outram_mc_libs::geometry::plot::{render_material_slice, PlotBasis, SlicePlot};
use outram_mc_libs::geometry::position::Position;

pub fn render_all(dir: &std::path::Path) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let core = model::core(LADDER_N as usize);
    let pal = nee_soon::htr10_rmc::plots::palette();
    let shots = [
        ("htr10_1_core_xz.png", SlicePlot::new(PlotBasis::Xz, Position::new(0.0, 0.0, -15.0), [400.0, 620.0], [400, 620]), "HTR-10 N=12: X-Z, WHOLE MODEL"),
        ("htr10_2_pebble_xy.png", SlicePlot::new(PlotBasis::Xy, Position::new(0.0, 0.0, LADDER_Z), [6.6, 6.6], [500, 500]), "HTR-10 N=12: X-Y, THE LADDER'S FUEL PEBBLE"),
        ("htr10_3_triso_xy.png", SlicePlot::new(PlotBasis::Xy, Position::new(PARTICLE_XY[0], PARTICLE_XY[1], LADDER_Z), [0.24, 0.24], [480, 480]), "HTR-10 N=12: X-Y, THE LADDER'S TRISO PARTICLE"),
    ];
    for (name, plot, title) in shots {
        let (_raw, img) = render_material_slice(&core.geometry, &plot, &pal, title);
        img.write_png(&dir.join(name)).map_err(|e| e.to_string())?;
    }
    // The random bed the core view can run on (gh:#786, #787).
    super::core::random_bed::render_review(dir)
}
