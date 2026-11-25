//! # The historical presets' runnable steps (gh:#1752)
//!
//! The steps a [`super::catalogue::HistoricalPreset`] marks available, made
//! runnable from the workbench rather than only listed. So far: **MSRE's
//! geometry review**, drawn live from the ASSEMBLED full-core model
//! (`nee_soon::msre_n1::Study::default().geometry()`) by the material slice
//! plotter, exactly as the N-1 record's images are drawn
//! (`nee_soon/examples/msre_n1_parameter_images.rs`). The drawing rule
//! binds: what is drawn is what the solver tracks, never the constants.
//!
//! Not runnable yet, and still listed with its issue on the preset page:
//! - EBR-II's geometry. Its builder needs the nuclear data loaded first
//!   (`ebr2_fresh_driver::build_model`), so its committed images stand.
//! - Both presets' k_eff (#1752).
//!
//! Research, education and V&V only.

use nee_soon::msre_n1::{self as m, Study};
use outram_mc_libs::geometry::plot::{render_material_slice, ImageData, PlotBasis, Rgb, SlicePlot};
use outram_mc_libs::geometry::position::Position;

/// Material colours, as the N-1 record's images use.
const COLOURS: [Rgb; 15] = [
    Rgb::new(220, 60, 40),
    Rgb::new(70, 70, 70),
    Rgb::new(160, 160, 200),
    Rgb::new(240, 170, 40),
    Rgb::new(150, 90, 60),
    Rgb::new(240, 120, 90),
    Rgb::new(200, 100, 160),
    Rgb::new(90, 140, 90),
    Rgb::new(120, 60, 140),
    Rgb::new(110, 110, 120),
    Rgb::new(60, 130, 200),
    Rgb::new(30, 30, 30),
    Rgb::new(255, 255, 0),
    Rgb::new(0, 150, 150),
    Rgb::new(200, 200, 120),
];

/// The MSRE N-1 default model's geometry review, each slice `px` pixels
/// across with a legend and dimensioned axes, `(title, image)` pairs:
///
/// 1. x-z through the axis, the whole model: the full TM-0728 shield (base,
///    cylinder, cover) around the vessel, the record's frame
///    (`msre_n1_first_criticality --draw`'s `msre_n1_fullshield_xz`);
/// 2. x-y at the core's mid-height, out to the shield's outer plate;
/// 3. x-z of the vessel alone, zoomed (core, plenums, heads).
///
/// The frames are sized from the shield's and the vessel's own constants
/// (`SHIELD_R_OUT`, `SHIELD_HEIGHT`, the plenums and `T_HEAD`, at the
/// record's cold dimensions), so nothing the model holds falls outside
/// them.
pub fn msre_slices(px: usize) -> Vec<(String, ImageData)> {
    let study = Study::default();
    let plan = study.plan();
    let g = study.geometry();
    let palette: Vec<(Rgb, &str)> = plan.materials.iter().enumerate().map(|(i, (n, _))| (COLOURS[i % COLOURS.len()], *n)).collect();
    let z_top = m::H_CORE + m::upper_plenum_height() + m::T_HEAD;
    let z_bot = -m::lower_plenum_height() - m::T_HEAD;
    let w_shield = 2.0 * m::SHIELD_R_OUT + 10.0;
    let zc0 = (z_top + z_bot) / 2.0 - m::FULL_SHIELD_VESSEL_MID_ABOVE_BOTTOM;
    let zc1 = zc0 + m::SHIELD_HEIGHT;
    let zb = zc0 - m::SHIELD_BASE_THICKNESS;
    let w_vessel = 2.0 * m::R_VESSEL_OUT + 10.0;
    let slice = |basis, origin, width: [f64; 2], title: &str| {
        let plot = SlicePlot {
            origin,
            basis,
            width,
            pixels: [px, ((px as f64) * width[1] / width[0]).round().max(1.0) as usize],
            level: None,
            show_overlaps: true,
            meshlines: None,
        };
        (title.to_string(), render_material_slice(&g, &plot, &palette, title).1)
    };
    vec![
        slice(
            PlotBasis::Xz,
            Position::new(0.0, 0.3, (zb + zc1 + m::SHIELD_COVER_THICKNESS) / 2.0),
            [w_shield, zc1 + m::SHIELD_COVER_THICKNESS - zb + 20.0],
            "MSRE N-1 X-Z, THE WHOLE MODEL: FULL TM-0728 SHIELD AROUND THE VESSEL",
        ),
        slice(PlotBasis::Xy, Position::new(0.0, 0.0, m::H_CORE / 2.0), [w_shield, w_shield], "MSRE N-1 X-Y AT THE CORE MID-HEIGHT, OUT TO THE SHIELD"),
        slice(PlotBasis::Xz, Position::new(0.0, 0.3, (z_top + z_bot) / 2.0), [w_vessel, z_top - z_bot + 20.0], "MSRE N-1 X-Z, THE VESSEL: CORE, PLENUMS, HEADS"),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// gh:#1752: the three MSRE slices are drawn from the assembled model,
    /// the requested width, and show more than one material.
    #[test]
    fn the_msre_slices_are_drawn_from_the_model() {
        let s = msre_slices(160);
        assert_eq!(s.len(), 3);
        for (title, img) in &s {
            assert!(title.starts_with("MSRE N-1"));
            assert!(img.width >= 160, "{title}: {} px", img.width);
            let first = img.pixels[img.pixels.len() / 2];
            assert!(img.pixels.iter().any(|p| *p != first), "{title} is one colour");
        }
        // For a human to look at (the drawing rule): the temp folder.
        let dir = std::env::temp_dir().join("dhoby_msre_preset_slices");
        std::fs::create_dir_all(&dir).expect("temp dir");
        for (i, (_, img)) in s.iter().enumerate() {
            img.write_png(dir.join(format!("slice_{i}.png"))).expect("png");
        }
    }
}
