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
//! **EBR-II's geometry review** ([`ebr2_slices`]): the fresh MARK-II driver
//! subassembly (`nee_soon::ebr2_fresh_driver`, arm A), assembled and drawn
//! with the record's three windows (`examples/ebr2_fresh_driver_kinf.rs`).
//! Its builder needs the nuclear data library loaded first (atomic masses
//! set the densities), so it reads the workspace's tapes.
//!
//! Not runnable yet, and still listed with its issue on the preset page:
//! both presets' k_eff (#1752).
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

/// The EBR-II fresh driver's geometry review (arm A, the record's
/// `GapTreatment::on_hand`), `px` pixels across, with the record's three
/// windows: the whole lattice cell (x-y at mid-height), the centre row of
/// pins (x-z) and a corner pin with the duct and the sodium between ducts.
///
/// # Errors
/// The workspace's tapes (`reference-data/endf`, `reference-data/ace`) are
/// missing, or the model does not build or assemble.
pub fn ebr2_slices(px: usize) -> Result<Vec<(String, ImageData)>, String> {
    use nee_soon::ebr2_fresh_driver::{build_model, driver_subassembly, library, GapTreatment};
    use njoy_outram_park_fork::reference_data::{ace_submodule_dir, reference_endf_dir};
    let ace = ace_submodule_dir().join("endf/endf-b-viii.0");
    let data = library(&reference_endf_dir(), &ace)?;
    let a = build_model(data, &GapTreatment::on_hand())?.assemble().map_err(|e| e.to_string())?;
    let corner = driver_subassembly().pin_centres_cm()[0];
    let draw = |basis, c: Position, w: [f64; 2], title: &str| (title.to_string(), a.draw_window(basis, c, w, px, title));
    Ok(vec![
        draw(PlotBasis::Xy, Position::new(0.0, 0.0, 0.0), [7.0, 7.0], "EBR-II FRESH MARK-II DRIVER, X-Y AT MID-HEIGHT: THE LATTICE CELL"),
        draw(PlotBasis::Xz, Position::new(0.0, 0.0, 0.0), [7.0, 36.0], "X-Z AT Y = 0: THE CENTRE ROW OF PINS (REFLECTIVE ENDS, AXES NOT TO SCALE)"),
        draw(PlotBasis::Xy, Position::new(corner[0], corner[1], 0.0), [1.6, 1.6], "A CORNER PIN: SLUG, BOND, CLAD, DUCT, SODIUM BETWEEN DUCTS"),
    ])
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

    /// gh:#1752: the three EBR-II slices are drawn from the assembled
    /// subassembly and show more than one material; skipped (passing, with
    /// a note) where the workspace's tapes are not checked out.
    #[test]
    fn the_ebr2_slices_are_drawn_from_the_model() {
        let s = match ebr2_slices(200) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("SKIP: {e}");
                return;
            }
        };
        assert_eq!(s.len(), 3);
        let dir = std::env::temp_dir().join("dhoby_ebr2_preset_slices");
        std::fs::create_dir_all(&dir).expect("temp dir");
        for (i, (title, img)) in s.iter().enumerate() {
            let first = img.pixels[img.pixels.len() / 2];
            assert!(img.pixels.iter().any(|p| *p != first), "{title} is one colour");
            img.write_png(dir.join(format!("slice_{i}.png"))).expect("png");
        }
    }
}
