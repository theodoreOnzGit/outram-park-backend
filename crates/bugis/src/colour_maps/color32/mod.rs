//! Colour-map functions for physics-state-driven rendering.
//!
//! ~~Ported verbatim from the duplicated `hot_to_cold_colour`/
//! `steam_quality_colour` functions in the FHR/CIET simulator examples.~~
//! **MOVED 2026-11-02:** the maps now live, byte for byte, in the shared
//! colour-theming crate `bugis` (`bugis::colour_maps`, returning
//! `bugis::Rgb`). These are thin wrappers that keep the public names and the
//! `egui::Color32` return type, so no call site changes; the tests below
//! still pin the exact colours. Each function takes a plain `f32` already
//! computed by the caller (e.g. a normalised temperature or quality).
//!
//! **MOVED AGAIN 2026-11-02 (gh:#1112):** these `Color32` wrappers moved
//! from `outram-park-digital-twin-engine`'s `src/color_maps/` into
//! `bugis::colour_maps::color32` (feature `egui`), because the widget library
//! that calls them moved here too. The engine re-exports this module as its
//! `color_maps`, so its call sites still did not change.

/// Fabio Crameri's Scientific colour maps (MIT), wrapping
/// `crate::colour_maps::crameri`. Perceptually uniform, CVD-friendly and
/// greyscale-safe.
pub mod crameri;

use egui::Color32;

/// Hot-to-cold colour map, variant 1: blue (cold) to red (hot), with a
/// green tint that fades out as `hotness` rises. `hotness` is clamped to
/// \[0, 1\]. Wraps [`crate::colour_maps::hot_to_cold_colour_mark_1`].
pub fn hot_to_cold_colour_mark_1(hotness: f32) -> Color32 {
    crate::colour_maps::hot_to_cold_colour_mark_1(hotness).into()
}

/// Hot-to-cold colour map, variant 2: black (cold) to red (hot). `hotness`
/// is clamped to \[0, 1\]. Wraps
/// [`crate::colour_maps::hot_to_cold_colour_mark_2`].
pub fn hot_to_cold_colour_mark_2(hotness: f32) -> Color32 {
    crate::colour_maps::hot_to_cold_colour_mark_2(hotness).into()
}

/// Steam-quality colour map: blue (`quality = 0`, saturated liquid) to
/// white (`quality = 1`, saturated vapour), clamped to \[0, 1\]. Wraps
/// [`crate::colour_maps::steam_quality_colour_mark_1`].
pub fn steam_quality_colour_mark_1(steam_quality: f32) -> Color32 {
    crate::colour_maps::steam_quality_colour_mark_1(steam_quality).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hot_to_cold_mark_1_endpoints() {
        assert_eq!(
            hot_to_cold_colour_mark_1(0.0),
            Color32::from_rgb(0, 135, 255)
        );
        assert_eq!(hot_to_cold_colour_mark_1(1.0), Color32::from_rgb(255, 0, 0));
    }

    #[test]
    fn hot_to_cold_mark_1_clamps_out_of_range() {
        assert_eq!(
            hot_to_cold_colour_mark_1(-1.0),
            hot_to_cold_colour_mark_1(0.0)
        );
        assert_eq!(
            hot_to_cold_colour_mark_1(2.0),
            hot_to_cold_colour_mark_1(1.0)
        );
    }

    #[test]
    fn hot_to_cold_mark_2_endpoints() {
        assert_eq!(hot_to_cold_colour_mark_2(0.0), Color32::from_rgb(0, 0, 0));
        assert_eq!(hot_to_cold_colour_mark_2(1.0), Color32::from_rgb(255, 0, 0));
    }

    #[test]
    fn steam_quality_mark_1_endpoints() {
        assert_eq!(
            steam_quality_colour_mark_1(0.0),
            Color32::from_rgb(0, 0, 255)
        );
        assert_eq!(
            steam_quality_colour_mark_1(1.0),
            Color32::from_rgb(255, 255, 255)
        );
    }

    #[test]
    fn steam_quality_mark_1_clamps_out_of_range() {
        assert_eq!(
            steam_quality_colour_mark_1(-1.0),
            steam_quality_colour_mark_1(0.0)
        );
        assert_eq!(
            steam_quality_colour_mark_1(2.0),
            steam_quality_colour_mark_1(1.0)
        );
    }
}
