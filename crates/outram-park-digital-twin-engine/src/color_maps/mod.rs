//! Colour-map functions for physics-state-driven rendering.
//!
//! Ported verbatim from the duplicated `hot_to_cold_colour`/
//! `steam_quality_colour` functions in `tampines-steam-tables`'s
//! `fhr_sim_v1`/`fhr_sim_v2` examples and `tuas_boussinesq_solver`'s
//! `ciet_educational_simulator/ciet_simulator_v1` example (confirmed
//! byte-identical between `fhr_sim_v1`/`v2`) -- real, already-validated code,
//! not new physics or a stub. Each function takes a plain `f32` already
//! computed by the caller (e.g. a normalised temperature or quality), the
//! same signature the existing call sites use.

/// Fabio Crameri's Scientific colour maps (MIT). Perceptually uniform,
/// CVD-friendly and greyscale-safe -- see the module docs for provenance and
/// for when to prefer them over the maps in this file.
pub mod crameri;

use egui::Color32;

/// Hot-to-cold colour map, variant 1: blue (cold) to red (hot), with a
/// green tint that fades out as `hotness` rises.
///
/// `hotness` is clamped to \[0, 1\] before mapping (out-of-range values
/// saturate at their nearest bound rather than producing an invalid
/// colour). This is the primary hot/cold colour map used across the
/// existing FHR/CIET simulator examples (e.g. reactor core/downcomer
/// temperature-driven fill colours).
pub fn hot_to_cold_colour_mark_1(hotness: f32) -> Color32 {
    let mut hotness_clone = hotness;

    if hotness_clone < 0.0 {
        hotness_clone = 0.0;
    } else if hotness_clone > 1.0 {
        hotness_clone = 1.0
    }

    let red: f32 = 255.0 * hotness_clone;
    let green: f32 = 135.0 * (1.0 - hotness_clone);
    let blue: f32 = 255.0 * (1.0 - hotness_clone);

    Color32::from_rgb(red as u8, green as u8, blue as u8)
}

/// Hot-to-cold colour map, variant 2: black (cold) to red (hot), no green
/// or blue tint.
///
/// `hotness` is clamped to \[0, 1\] before mapping, same convention as
/// [`hot_to_cold_colour_mark_1`]. Used for a subset of pipe segments in the
/// existing FHR simulator examples where the simpler black-to-red map was
/// preferred over `mark_1`.
pub fn hot_to_cold_colour_mark_2(hotness: f32) -> Color32 {
    let mut hotness_clone = hotness;

    if hotness_clone < 0.0 {
        hotness_clone = 0.0;
    } else if hotness_clone > 1.0 {
        hotness_clone = 1.0
    }

    let red: f32 = 255.0 * hotness_clone;
    let green: f32 = 0.0;
    let blue: f32 = 0.0;

    Color32::from_rgb(red as u8, green as u8, blue as u8)
}

/// Steam-quality colour map: blue (`quality = 0`, saturated liquid) to
/// white (`quality = 1`, saturated vapour).
///
/// `steam_quality` is clamped to \[0, 1\] before mapping, same convention as
/// [`hot_to_cold_colour_mark_1`].
pub fn steam_quality_colour_mark_1(steam_quality: f32) -> Color32 {
    let mut steam_quality_clone = steam_quality;

    if steam_quality_clone < 0.0 {
        steam_quality_clone = 0.0;
    } else if steam_quality_clone > 1.0 {
        steam_quality_clone = 1.0
    }

    let red: f32 = 255.0 * steam_quality_clone;
    let green: f32 = 255.0 * steam_quality_clone;
    let blue: f32 = 255.0;

    Color32::from_rgb(red as u8, green as u8, blue as u8)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hot_to_cold_mark_1_endpoints() {
        assert_eq!(hot_to_cold_colour_mark_1(0.0), Color32::from_rgb(0, 135, 255));
        assert_eq!(hot_to_cold_colour_mark_1(1.0), Color32::from_rgb(255, 0, 0));
    }

    #[test]
    fn hot_to_cold_mark_1_clamps_out_of_range() {
        assert_eq!(hot_to_cold_colour_mark_1(-1.0), hot_to_cold_colour_mark_1(0.0));
        assert_eq!(hot_to_cold_colour_mark_1(2.0), hot_to_cold_colour_mark_1(1.0));
    }

    #[test]
    fn hot_to_cold_mark_2_endpoints() {
        assert_eq!(hot_to_cold_colour_mark_2(0.0), Color32::from_rgb(0, 0, 0));
        assert_eq!(hot_to_cold_colour_mark_2(1.0), Color32::from_rgb(255, 0, 0));
    }

    #[test]
    fn steam_quality_mark_1_endpoints() {
        assert_eq!(steam_quality_colour_mark_1(0.0), Color32::from_rgb(0, 0, 255));
        assert_eq!(steam_quality_colour_mark_1(1.0), Color32::from_rgb(255, 255, 255));
    }

    #[test]
    fn steam_quality_mark_1_clamps_out_of_range() {
        assert_eq!(steam_quality_colour_mark_1(-1.0), steam_quality_colour_mark_1(0.0));
        assert_eq!(steam_quality_colour_mark_1(2.0), steam_quality_colour_mark_1(1.0));
    }
}
