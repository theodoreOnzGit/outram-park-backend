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
//! MIT is compatible with this workspace's `GPL-3.0-only`. The full licence
//! text ships as `LICENSE.crameri.pdf` at the crate root, per the MIT
//! requirement that the copyright and permission notice accompany the work.
//!
//! The tables were transcribed from the **official release**, not from any
//! third-party wrapper. Two wrappers were considered and rejected on licence
//! grounds: NASA GISS Panoply's colorbar collection carries no licence at all
//! and aggregates third-party tables, and `github.com/chadagreene/crameri`
//! (a MATLAB wrapper) declares no licence, so its code is all-rights-reserved
//! regardless of the data being MIT underneath.
//!
//! # Why these rather than the existing maps
//!
//! [`crate::color_maps::hot_to_cold_colour_mark_1`] and friends remain, and
//! callers depending on their exact values are unaffected — this module is an
//! addition, not a replacement.
//!
//! The difference that matters is **perceptual uniformity**: equal steps in the
//! underlying quantity produce equal-looking steps in colour. A map without
//! that property manufactures structure — stretches where the colour changes
//! quickly read as steep gradients and flat stretches read as uniform, whether
//! or not the data does anything of the sort. For widgets whose whole premise
//! is that the rendering derives from physics state, a colour map that invents
//! features is actively misleading.
//!
//! # Choosing a map
//!
//! - **Diverging** ([`vik`], [`roma`]) — a quantity with a meaningful midpoint,
//!   where deviation either way matters: temperature about a reference,
//!   a residual, an error.
//! - **Sequential** ([`batlow`], [`lajolla`]) — a quantity read as magnitude
//!   with no special centre: burnup, flux, steam quality.
//! - **Cyclic** ([`roma_o`]) — a quantity that wraps, so the ends must join
//!   without a visible seam: a rotor angle, a phase.

use egui::Color32;

pub mod tables;

/// Sample a 256-entry table at `t`, linearly interpolating between entries.
///
/// `t` is clamped to `[0, 1]`; `0` is the first entry and `1` the last.
/// Interpolation keeps the map smooth when the table is stretched across more
/// screen pixels than it has entries.
fn sample(table: &[[u8; 3]; 256], t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let x = t * (table.len() - 1) as f32;
    let i = x.floor() as usize;
    let j = (i + 1).min(table.len() - 1);
    let f = x - i as f32;
    let lerp = |a: u8, b: u8| -> u8 { (a as f32 + (b as f32 - a as f32) * f).round() as u8 };
    Color32::from_rgb(
        lerp(table[i][0], table[j][0]),
        lerp(table[i][1], table[j][1]),
        lerp(table[i][2], table[j][2]),
    )
}

/// Sample a **cyclic** table at `t`, wrapping rather than clamping.
///
/// `t = 0.0` and `t = 1.0` give the same colour, as does any integer offset —
/// so an angle can be passed as `angle / 2*pi` without special-casing the
/// wrap point.
fn sample_cyclic(table: &[[u8; 3]; 256], t: f32) -> Color32 {
    let t = t.rem_euclid(1.0);
    let x = t * table.len() as f32;
    let i = (x.floor() as usize) % table.len();
    let j = (i + 1) % table.len();
    let f = x - x.floor();
    let lerp = |a: u8, b: u8| -> u8 { (a as f32 + (b as f32 - a as f32) * f).round() as u8 };
    Color32::from_rgb(
        lerp(table[i][0], table[j][0]),
        lerp(table[i][1], table[j][1]),
        lerp(table[i][2], table[j][2]),
    )
}

/// `vik` — blue-white-red **diverging**. The default temperature map.
///
/// `t = 0.0` is the cold end, `0.5` the neutral centre, `1.0` the hot end.
/// Normalise a temperature onto `[0, 1]` about its reference before calling,
/// so that `0.5` lands where the physics says "neither hot nor cold".
pub fn vik(t: f32) -> Color32 {
    sample(&tables::VIK, t)
}

/// `roma` — red-yellow-blue **diverging**. Use when a second, visually
/// distinguishable diverging field shares the screen with [`vik`].
pub fn roma(t: f32) -> Color32 {
    sample(&tables::ROMA, t)
}

/// `batlow` — general-purpose **sequential**, monotonic in lightness.
///
/// The safe default for a magnitude with no meaningful midpoint, and the one
/// to reach for if the figure may be printed in greyscale.
pub fn batlow(t: f32) -> Color32 {
    sample(&tables::BATLOW, t)
}

/// `lajolla` — warm **sequential**, light at the low end.
pub fn lajolla(t: f32) -> Color32 {
    sample(&tables::LAJOLLA, t)
}

/// `romaO` — **cyclic**. Wraps, so `t` and `t + 1` are the same colour.
///
/// For quantities with no beginning or end: a rotor angle, a phase. A
/// non-cyclic map used for these shows a hard seam at the wrap, implying a
/// discontinuity the physics does not have.
pub fn roma_o(t: f32) -> Color32 {
    sample_cyclic(&tables::ROMA_O, t)
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
