//! Fabio Crameri's **Scientific colour maps** — perceptually uniform,
//! colour-vision-deficiency friendly, greyscale-safe.
//!
//! # Provenance
//!
//! | | |
//! |---|---|
//! | Project | Scientific colour maps |
//! | Author | Fabio Crameri |
//! | Version | 8.0.1 |
//! | DOI | [10.5281/zenodo.1243862](https://doi.org/10.5281/zenodo.1243862) |
//! | Home | <https://www.fabiocrameri.ch/colourmaps/> |
//! | Licence | **MIT**, Copyright (c) 2023, Fabio Crameri |
//! | Retrieved | 2026-08-06, from the official `ScientificColourMaps8.zip` |
//!
//! Cite as: Crameri, F. (2018). *Scientific colour maps.* Zenodo.
//! <https://doi.org/10.5281/zenodo.1243862>
//!
//! The tables, the sampling code and the full licence text
//! (`LICENSE.crameri.pdf`) now live in the `bugis` crate.
//!
//! The tables were transcribed from the **official release**, not from any
//! third-party wrapper. Two wrappers were considered and rejected on licence
//! grounds: NASA GISS Panoply's colorbar collection carries no licence at all
//! and aggregates third-party tables, and `github.com/chadagreene/crameri`
//! (a MATLAB wrapper) declares no licence, so its code is all-rights-reserved
//! regardless of the data being MIT underneath.
//!
//! **MOVED 2026-11-02:** the tables and the samplers moved, byte for byte, to
//! `crate::colour_maps::crameri` (returning `bugis::Rgb`). This module keeps
//! the public names and the `egui::Color32` return type, so no call site
//! changes; the tests below still pin the exact colours.
//!
//! **MOVED AGAIN 2026-11-02 (gh:#1112):** this wrapper moved with its parent
//! to `bugis::colour_maps::color32::crameri`; the engine re-exports it at
//! `color_maps::crameri`.
//!
//! # Choosing a map
//!
//! - **Diverging** ([`vik`], [`roma`]) — a quantity with a meaningful midpoint.
//! - **Sequential** ([`batlow`], [`lajolla`]) — a magnitude with no centre.
//! - **Cyclic** ([`roma_o`]) — a quantity that wraps, without a visible seam.

use egui::Color32;

/// The 256-entry tables, from `bugis`.
pub use crate::colour_maps::crameri::tables;

/// *vik*, diverging (blue → neutral → red); `t` clamped to `[0, 1]`.
pub fn vik(t: f32) -> Color32 {
    crate::colour_maps::crameri::vik(t).into()
}

/// *roma*, diverging; `t` clamped to `[0, 1]`.
pub fn roma(t: f32) -> Color32 {
    crate::colour_maps::crameri::roma(t).into()
}

/// *batlow*, sequential; `t` clamped to `[0, 1]`.
pub fn batlow(t: f32) -> Color32 {
    crate::colour_maps::crameri::batlow(t).into()
}

/// *lajolla*, sequential; `t` clamped to `[0, 1]`.
pub fn lajolla(t: f32) -> Color32 {
    crate::colour_maps::crameri::lajolla(t).into()
}

/// *romaO*, cyclic: `t` wraps, so `0` and `1` are the same colour.
pub fn roma_o(t: f32) -> Color32 {
    crate::colour_maps::crameri::roma_o(t).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every table must be the full 256 entries. A truncated table would still
    /// compile and still render, just with a silently distorted map.
    #[test]
    fn tables_are_complete() {
        assert_eq!(tables::VIK.len(), 256);
        assert_eq!(tables::ROMA.len(), 256);
        assert_eq!(tables::BATLOW.len(), 256);
        assert_eq!(tables::LAJOLLA.len(), 256);
        assert_eq!(tables::ROMA_O.len(), 256);
    }

    /// The endpoints must be the table's own first and last entries, not an
    /// interpolated near-miss.
    #[test]
    fn endpoints_are_exact() {
        let first = tables::VIK[0];
        let last = tables::VIK[255];
        assert_eq!(vik(0.0), Color32::from_rgb(first[0], first[1], first[2]));
        assert_eq!(vik(1.0), Color32::from_rgb(last[0], last[1], last[2]));
    }

    /// Out-of-range input must clamp, never wrap or panic — a temperature
    /// outside the display range should saturate at the end of the scale.
    #[test]
    fn non_cyclic_maps_clamp() {
        assert_eq!(vik(-5.0), vik(0.0));
        assert_eq!(vik(5.0), vik(1.0));
        assert_eq!(batlow(f32::NEG_INFINITY), batlow(0.0));
    }

    /// A cyclic map must join seamlessly: `t = 0` and `t = 1` are the same
    /// point on the cycle, and any integer offset must agree.
    ///
    /// **Methodology:** compare `roma_o` at 0.0, 1.0, 2.0 and -1.0, and check
    /// the table's own first and last entries are close enough that no visible
    /// seam appears at the wrap.
    ///
    /// **Result (2026-08-06):** all four sample points return an identical
    /// colour, and the first/last table entries differ by at most 2/255 per
    /// channel — below the threshold of visible banding.
    #[test]
    fn cyclic_map_wraps_without_a_seam() {
        assert_eq!(roma_o(0.0), roma_o(1.0));
        assert_eq!(roma_o(0.0), roma_o(2.0));
        assert_eq!(roma_o(0.0), roma_o(-1.0));

        let first = tables::ROMA_O[0];
        let last = tables::ROMA_O[255];
        for c in 0..3 {
            let d = (first[c] as i16 - last[c] as i16).abs();
            assert!(d <= 2, "cyclic seam of {d}/255 on channel {c}");
        }
    }

    /// `batlow` is monotonic in lightness, which is what makes it survive
    /// greyscale printing. Checked on the standard luma weighting.
    ///
    /// **Methodology:** compute Rec. 601 luma at each of the 256 entries and
    /// require it to be non-decreasing, allowing a small tolerance for the
    /// 8-bit quantisation.
    ///
    /// **Result (2026-08-06):** monotonic across all 256 entries with a
    /// tolerance of 2/255; the largest single backward step is within that.
    #[test]
    fn batlow_is_monotonic_in_lightness() {
        let luma =
            |c: [u8; 3]| -> f32 { 0.299 * c[0] as f32 + 0.587 * c[1] as f32 + 0.114 * c[2] as f32 };
        let mut prev = luma(tables::BATLOW[0]);
        for (i, entry) in tables::BATLOW.iter().enumerate().skip(1) {
            let l = luma(*entry);
            assert!(
                l >= prev - 2.0,
                "lightness dropped at entry {i}: {prev} -> {l}"
            );
            prev = l;
        }
    }

    /// A diverging map's centre must actually be neutral — that is what makes
    /// "no deviation" read as "no colour". If the midpoint were saturated, a
    /// value sitting exactly at the reference would look like an excursion.
    #[test]
    fn vik_midpoint_is_near_neutral() {
        let c = vik(0.5);
        let spread = c.r().max(c.g()).max(c.b()) as i16 - c.r().min(c.g()).min(c.b()) as i16;
        assert!(spread < 40, "vik midpoint is not neutral: spread {spread}");
    }
}
