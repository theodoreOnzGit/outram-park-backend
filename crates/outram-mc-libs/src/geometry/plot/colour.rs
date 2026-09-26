// SPDX-License-Identifier: GPL-3.0
//
// Ported from OpenMC (MIT), commit d7d3284a1:
//   include/openmc/plot.h  -- RGBColor, WHITE/RED/BLACK, PlottableInterface colours
//   src/plot.cpp           -- random_color, set_default_colors, set_user_colors,
//                             set_mask, set_bg_color, set_overlap_color
// Copyright (c) 2011-2026 Massachusetts Institute of Technology, UChicago
// Argonne LLC, and OpenMC contributors. MIT notice in
// verification_and_validation/geometry_plotting/openmc_inputs/LICENSE.openmc.

//! **Plot colours** — OpenMC's default colour stream, user colours, masks.
//!
//! The one thing that decides whether a plot here and a plot from
//! `openmc --plot` come out the *same colour* is [`random_colour`]: three
//! draws of the crate's PCG `prn` on a dedicated plotter seed, truncated to a
//! byte. It is ported bit-for-bit, so with the same seed and the same cell (or
//! material) ordering the two codes agree on every default colour — verified
//! pixel-for-pixel in `verification_and_validation/geometry_plotting/`.

use crate::rng::lcg::prn;

/// One 8-bit RGB colour. Maps to `openmc::RGBColor` (`include/openmc/plot.h:48-79`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Rgb {
    /// Red channel.
    pub r: u8,
    /// Green channel.
    pub g: u8,
    /// Blue channel.
    pub b: u8,
}

impl Rgb {
    /// A colour from its three channels.
    #[must_use]
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    /// Scale every channel by `x`, truncating back to a byte exactly as
    /// `RGBColor::operator*=` does (`include/openmc/plot.h:69-75`): the `uint8_t`
    /// channel is promoted to `double`, multiplied, and converted back by
    /// truncation.
    #[must_use]
    pub fn scaled(self, x: f64) -> Self {
        Self {
            r: (f64::from(self.r) * x) as u8,
            g: (f64::from(self.g) * x) as u8,
            b: (f64::from(self.b) * x) as u8,
        }
    }
}

/// `WHITE` (`include/openmc/plot.h:82`) — the default background, the colour of a
/// void material, and of a masked component with no mask background.
pub const WHITE: Rgb = Rgb::new(255, 255, 255);
/// `RED` (`include/openmc/plot.h:83`) — the default overlap colour.
pub const RED: Rgb = Rgb::new(255, 0, 0);
/// `BLACK` (`include/openmc/plot.h:84`) — the default wireframe colour.
pub const BLACK: Rgb = Rgb::new(0, 0, 0);

/// Initial value of OpenMC's `model::plotter_seed` (`src/plot.cpp:174`). The
/// `<plot_seed>` element of `settings.xml` overrides it (`src/settings.cpp:581-585`).
pub const DEFAULT_PLOTTER_SEED: u64 = 1;

/// One random colour from the plotter stream. Port of `random_color`
/// (`src/plot.cpp:1179-1183`): `int(prn(&seed) * 255)` per channel, red first.
pub fn random_colour(seed: &mut u64) -> Rgb {
    // Evaluation order matters: C++ brace-initialiser lists are evaluated
    // left to right, so red, green, blue draw in that order.
    let r = (prn(seed) * 255.0) as i32;
    let g = (prn(seed) * 255.0) as i32;
    let b = (prn(seed) * 255.0) as i32;
    Rgb::new(r as u8, g as u8, b as u8)
}

/// `n` default colours drawn from `seed`, rejecting [`RED`] and [`WHITE`].
/// Port of `PlottableInterface::set_default_colors` (`src/plot.cpp:580-596`).
///
/// # The seed is shared across plots
///
/// Upstream's `model::plotter_seed` is a single global that every plot in a
/// `plots.xml` draws from in turn, starting at [`DEFAULT_PLOTTER_SEED`]. So the
/// *second* plot's colours depend on how many cells or materials the first one
/// coloured. Pass the same `&mut u64` through a sequence of plots to reproduce
/// a multi-plot `openmc --plot` run; start a fresh one at
/// [`DEFAULT_PLOTTER_SEED`] to reproduce a single-plot run.
pub fn default_colours(n: usize, seed: &mut u64) -> Vec<Rgb> {
    (0..n)
        .map(|_| {
            let mut c = random_colour(seed);
            while c == RED || c == WHITE {
                c = random_colour(seed);
            }
            c
        })
        .collect()
}

/// What a plot colours by. Maps to `PlottableInterface::PlotColorBy`
/// (`include/openmc/plot.h:120`) — upstream offers exactly these two for image
/// output. (The matplotlib-script path's [`super::ColourBy`] additionally has a
/// universe mode; OpenMC's image plots do not.)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlotColourBy {
    /// By leaf cell (or the cell at the plot's universe level).
    Cell,
    /// By leaf material; a void cell draws [`WHITE`].
    Material,
}

/// The colour state every image plot carries: the per-cell or per-material
/// table plus the background and overlap colours.
///
/// Maps to the colour members of `PlottableInterface`
/// (`include/openmc/plot.h:144-149`): `color_by_`, `not_found_`,
/// `overlap_color_`, `colors_`. Build it with [`ColourScheme::new`] (which draws
/// the default colours) and then apply user colours and masks **in upstream's
/// order**, which is the order of the `PlottableInterface` constructor
/// (`src/plot.cpp:829-839`): background, default colours, user colours, mask,
/// overlap colour. The builder methods are order-independent except that
/// [`Self::with_mask`] must follow [`Self::with_colour`], as upstream's does,
/// because a mask overwrites whatever colour the component had.
#[derive(Debug, Clone, PartialEq)]
pub struct ColourScheme {
    /// Cell or material colouring.
    pub colour_by: PlotColourBy,
    /// One colour per cell index (in [`crate::geometry::geometry::Geometry::cells`]
    /// order) or per material index, as `colour_by` says.
    pub colours: Vec<Rgb>,
    /// Background: pixels in no cell, and pixels whose cell level is deeper than
    /// the geometry at that point. `not_found_`, default [`WHITE`].
    pub background: Rgb,
    /// Colour of an overlap when overlaps are shown. `overlap_color_`, default [`RED`].
    pub overlap_colour: Rgb,
}

impl ColourScheme {
    /// Default colours for `n_domains` cells or materials, drawn from `seed`.
    ///
    /// `n_domains` must be the **whole** cell count (`geom.cells.len()`) or the
    /// whole material count, not the number visible in the plot: upstream
    /// sizes the table to `model::cells.size()` / `model::materials.size()`
    /// (`src/plot.cpp:583-587`), and every entry consumes draws from the shared
    /// seed, so a wrong count shifts every later plot's colours.
    pub fn new(colour_by: PlotColourBy, n_domains: usize, seed: &mut u64) -> Self {
        Self {
            colour_by,
            colours: default_colours(n_domains, seed),
            background: WHITE,
            overlap_colour: RED,
        }
    }

    /// Set one cell's or material's colour, by index. `set_user_colors`
    /// (`src/plot.cpp:598-633`). Upstream takes ids and warns on an unknown
    /// one; this takes an index and ignores one that is out of range, which
    /// is the same outcome.
    #[must_use]
    pub fn with_colour(mut self, index: usize, colour: Rgb) -> Self {
        if let Some(c) = self.colours.get_mut(index) {
            *c = colour;
        }
        self
    }

    /// Mask: every listed component draws `mask_background`, or [`WHITE`] when
    /// that is `None`. `set_mask` (`src/plot.cpp:743-798`).
    #[must_use]
    pub fn with_mask(mut self, components: &[usize], mask_background: Option<Rgb>) -> Self {
        let bg = mask_background.unwrap_or(WHITE);
        for &j in components {
            if let Some(c) = self.colours.get_mut(j) {
                *c = bg;
            }
        }
        self
    }

    /// Background colour. `set_bg_color` (`src/plot.cpp:466-477`).
    #[must_use]
    pub fn with_background(mut self, colour: Rgb) -> Self {
        self.background = colour;
        self
    }

    /// Overlap colour. `set_overlap_color` (`src/plot.cpp:800-827`).
    #[must_use]
    pub fn with_overlap_colour(mut self, colour: Rgb) -> Self {
        self.overlap_colour = colour;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The first colours of the plotter stream from seed 1, as OpenMC draws
    /// them. The expected values are read back out of `openmc --plot` output
    /// (`godiva_xy_cell.png`, `lattice_xy_cell.png` in
    /// `verification_and_validation/geometry_plotting/openmc_reference/`):
    /// cell index 0 of any single plot starting at seed 1 is drawn in the
    /// first of these.
    #[test]
    fn default_colour_stream_matches_openmc() {
        let mut seed = DEFAULT_PLOTTER_SEED;
        let c = default_colours(3, &mut seed);
        // Recorded from OpenMC's own PNG (see the V&V README): Godiva's single
        // cell is drawn in exactly this colour.
        assert_eq!(
            c[0], GODIVA_CELL_COLOUR,
            "first default colour drifted from OpenMC"
        );
        assert_ne!(c[0], c[1]);
        assert_ne!(c[1], c[2]);
        // Every draw consumed exactly three prn calls unless a RED/WHITE
        // rejection happened; either way the stream is deterministic.
        let mut again = DEFAULT_PLOTTER_SEED;
        assert_eq!(default_colours(3, &mut again), c);
        assert_eq!(seed, again);
    }

    /// Godiva's cell colour in OpenMC's `godiva_xy_cell.png` (pixel at the
    /// centre), d7d3284a1.
    const GODIVA_CELL_COLOUR: Rgb = Rgb::new(GODIVA_R, GODIVA_G, GODIVA_B);
    const GODIVA_R: u8 = 181;
    const GODIVA_G: u8 = 192;
    const GODIVA_B: u8 = 84;

    #[test]
    fn scaling_truncates_like_uint8_times_double() {
        // 255 * 0.1 = 25.5 -> 25; 3 * 0.5 = 1.5 -> 1.
        assert_eq!(Rgb::new(255, 3, 0).scaled(0.1), Rgb::new(25, 0, 0));
        assert_eq!(Rgb::new(255, 3, 0).scaled(0.5), Rgb::new(127, 1, 0));
    }

    #[test]
    fn mask_overwrites_user_colour_and_defaults_to_white() {
        let mut seed = DEFAULT_PLOTTER_SEED;
        let s = ColourScheme::new(PlotColourBy::Cell, 3, &mut seed)
            .with_colour(1, Rgb::new(1, 2, 3))
            .with_mask(&[1, 2], None);
        assert_eq!(s.colours[1], WHITE);
        assert_eq!(s.colours[2], WHITE);
        let t = ColourScheme::new(PlotColourBy::Cell, 3, &mut seed)
            .with_mask(&[0], Some(Rgb::new(40, 40, 40)));
        assert_eq!(t.colours[0], Rgb::new(40, 40, 40));
    }
}
