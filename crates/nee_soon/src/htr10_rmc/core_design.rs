// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// This file is part of OUTRAM PARK. See the module header of `htr10_rmc` for
// licence terms.

//! # What the HTR-10 geometry builders take as a design
//!
//! Added 2026-10-05 (gh:#566, gh:#580) so that a caller (Dhoby Ghaut's
//! workbench) can change the parts of the model the builders are able to
//! represent, and the assembled geometry the solver transports changes with
//! them. [`Htr10CoreDesign::default`] is the model of the RMC code-to-code
//! record, value for value: every builder called with it builds the same
//! geometry, cell for cell, as before this type existed (pinned by
//! `the_default_design_builds_the_geometry_unchanged` and by the workbench's
//! geometry fixture).
//!
//! # What it can change
//!
//! | field | what it moves in the geometry | source of the default |
//! |---|---|---|
//! | [`fuel_ball_fraction`](Htr10CoreDesign::fuel_ball_fraction) | the fuel/dummy split over the balls above the bed floor (same low-discrepancy rule) | Li, Yu & Wei (2014) Table 1, 0.57 |
//! | [`fuel_zone_radius_cm`](Htr10CoreDesign::fuel_zone_radius_cm) | the fuelled-zone sphere of every fuel ball, and the TRISO lattice clipped to it | Li (2014) Table 2, 2.5 cm |
//! | [`triso_radii_cm`](Htr10CoreDesign::triso_radii_cm) | the five TRISO shells | TECDOC-1382 / Li Table 2, as adjudicated (op-867c.12) |
//! | [`particles_per_pebble`](Htr10CoreDesign::particles_per_pebble) | the TRISO lattice pitch, solved so the zone holds exactly this many whole particles | Li Table 2 and Şeker & Çolak (2003) p. 266, 8335 |
//! | [`rod_insertion`](Htr10CoreDesign::rod_insertion) | the axial position of the ten explicit control rods | TECDOC-1382 § 4.1.1.5: withdrawn (0) is the benchmark state |
//!
//! # What it cannot change, stated plainly
//!
//! - **The pebble outer radius.** Şeker's 13-ball lattice cell and the DEM
//!   bed's overlap splitting are built for 6 cm balls; a different ball needs a
//!   different lattice, which no builder here makes.
//! - **Fertile or poison pebbles.** Every ball is fuel or graphite (dummy).
//! - **Materials** (kernel composition, enrichment, matrix, coolant): those are
//!   [`super::materials`]' and [`super::data`]'s, fixed to the benchmark.
//! - **Rod insertion is one position for all ten rods.** A single rod (the
//!   B32 / B42 problems) is not representable.
//!
//! # Rod insertion, defined
//!
//! `rod_insertion` is the fraction of the published travel, linear in the
//! position of the rod's lower end: 0 puts it at
//! [`LOWER_END_WITHDRAWN_CM`] (`z_T` = 119.2 cm, in the top reflector, the
//! benchmark's state), 1 at [`LOWER_END_INSERTED_CM`] (`z_T` = 394.2 cm),
//! the two positions TECDOC-1382 § 4.1.1.5 states. It is a position, not a
//! worth fraction: the rod (264.7 cm) is shorter than its travel (275 cm), and
//! worth depends on where the absorber sits in the flux.
//!
//! # Units
//!
//! Lengths are plain `f64` in **centimetres**, named `_cm`, as every length of
//! `htr10_rmc`'s geometry is (`AssembledCore`, the `HTR10_*_CM` constants).
//! Not `uom`: a `Length` stores metres, and the round trip cm -> m -> cm moves
//! 119.2 cm to 119.19999999999999 cm, which would have changed the record's
//! geometry in its last bit (measured 2026-10-05 by
//! `the_default_design_builds_the_geometry_unchanged`).

use super::control_rod::{LOWER_END_INSERTED_CM, LOWER_END_WITHDRAWN_CM};
use super::explicit_bed::{FUEL_ZONE_RADIUS_CM, PEBBLE_RADIUS_CM};

/// The TRISO shell radii of the record's model \[cm\], kernel outward:
/// kernel, buffer, IPyC, SiC, OPyC. Adjudicated radii (op-867c.12):
/// TECDOC-1382, 90 µm buffer; equal to `TrisoSpec::HTR10_LI2014`.
pub const TRISO_RADII_CM: [f64; 5] = [0.0250, 0.0340, 0.0380, 0.0415, 0.0455];

/// Whole TRISO particles per fuel pebble: Li (2014) Table 2, and Şeker &
/// Çolak (2003) p. 266 (*"verified to be 8335"*).
pub const PARTICLES_PER_PEBBLE: usize = 8335;

/// The parts of the HTR-10 model a caller may change, and the builders
/// represent. See the module docs for what is and is not covered.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Htr10CoreDesign {
    /// Fraction \[-\] of the balls centred above the bed floor that are fuel;
    /// the rest are graphite dummies. In `[0, 1]`. The conus and the tube stay
    /// all dummy whatever this is (Terry 2005 sec. 2). Default 0.57.
    pub fuel_ball_fraction: f64,
    /// Radius \[cm\] of a fuel ball's fuelled zone. Must be smaller than the
    /// 3 cm ball and larger than a TRISO particle. Default 2.5 cm.
    pub fuel_zone_radius_cm: f64,
    /// TRISO shell radii \[cm\], kernel outward (kernel, buffer, IPyC, SiC,
    /// OPyC), strictly increasing. Default [`TRISO_RADII_CM`].
    pub triso_radii_cm: [f64; 5],
    /// Whole TRISO particles in each fuel zone; the lattice pitch is solved
    /// to give exactly this count. Default [`PARTICLES_PER_PEBBLE`].
    pub particles_per_pebble: usize,
    /// Control-rod insertion, fraction \[-\] of the published travel, `[0, 1]`:
    /// 0 withdrawn (the benchmark), 1 fully inserted. All ten rods together.
    pub rod_insertion: f64,
}

impl Default for Htr10CoreDesign {
    /// The model of the RMC code-to-code record (see the module table).
    fn default() -> Self {
        Self {
            fuel_ball_fraction: super::table1::FUEL_BALL_FRACTION,
            fuel_zone_radius_cm: FUEL_ZONE_RADIUS_CM,
            triso_radii_cm: TRISO_RADII_CM,
            particles_per_pebble: PARTICLES_PER_PEBBLE,
            rod_insertion: 0.0,
        }
    }
}

impl Htr10CoreDesign {
    /// TECDOC axial coordinate `z_T` \[cm\] of the rods' lower end (it runs
    /// downward from the model top), for [`Self::rod_insertion`] clamped to
    /// `[0, 1]`: 119.2 cm withdrawn to 394.2 cm fully inserted.
    #[must_use]
    pub fn rod_lower_end_zt_cm(&self) -> f64 {
        let f = self.rod_insertion.clamp(0.0, 1.0);
        LOWER_END_WITHDRAWN_CM + f * (LOWER_END_INSERTED_CM - LOWER_END_WITHDRAWN_CM)
    }

    /// Why the builders would refuse this design, if they would: a fraction
    /// outside `[0, 1]`, radii that do not nest (kernel < ... < OPyC <
    /// fuel zone < ball), or no particles.
    ///
    /// # Errors
    /// A sentence naming the first problem found.
    pub fn check(&self) -> Result<(), String> {
        if !(0.0..=1.0).contains(&self.fuel_ball_fraction) {
            return Err(format!("fuel ball fraction {} is not in [0, 1]", self.fuel_ball_fraction));
        }
        if !(0.0..=1.0).contains(&self.rod_insertion) {
            return Err(format!("rod insertion {} is not in [0, 1]", self.rod_insertion));
        }
        let r = self.triso_radii_cm;
        if r[0] <= 0.0 || r.windows(2).any(|w| w[1] <= w[0]) {
            return Err(format!("TRISO radii {r:?} cm must be positive and strictly increasing"));
        }
        let fz = self.fuel_zone_radius_cm;
        if fz <= r[4] || fz >= PEBBLE_RADIUS_CM {
            return Err(format!(
                "fuel zone radius {fz} cm must lie between the TRISO outer radius {} cm and the {PEBBLE_RADIUS_CM} cm ball",
                r[4]
            ));
        }
        if self.particles_per_pebble == 0 {
            return Err("a fuel pebble needs at least one TRISO particle".into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The default is the record's model, and the rod travel is TECDOC's.
    #[test]
    fn the_default_design_is_the_record_model() {
        let d = Htr10CoreDesign::default();
        assert!(d.check().is_ok());
        assert_eq!(d.fuel_ball_fraction, 0.57);
        assert_eq!(d.particles_per_pebble, 8335);
        assert_eq!(d.rod_lower_end_zt_cm(), 119.2);
        let full = Htr10CoreDesign { rod_insertion: 1.0, ..d };
        assert!((full.rod_lower_end_zt_cm() - 394.2).abs() < 1e-12);
        let half = Htr10CoreDesign { rod_insertion: 0.5, ..d };
        assert!((half.rod_lower_end_zt_cm() - 256.7).abs() < 1e-12);
        // TrisoSpec::HTR10_LI2014 is the same particle.
        let s = outram_mc_libs::pebble_beds::fhr_pebble::TrisoSpec::HTR10_LI2014;
        assert_eq!(TRISO_RADII_CM, [s.kernel, s.buffer, s.ipyc, s.sic, s.opyc]);
    }

    #[test]
    fn a_design_that_does_not_nest_is_refused() {
        let d = Htr10CoreDesign::default();
        let big_zone = Htr10CoreDesign { fuel_zone_radius_cm: 3.0, ..d };
        assert!(big_zone.check().is_err());
        let mut radii = d.triso_radii_cm;
        radii.swap(1, 2);
        assert!(Htr10CoreDesign { triso_radii_cm: radii, ..d }.check().is_err());
        assert!(Htr10CoreDesign { fuel_ball_fraction: 1.2, ..d }.check().is_err());
        assert!(Htr10CoreDesign { rod_insertion: -0.1, ..d }.check().is_err());
    }

    /// The default design builds the record's geometry: the counts the
    /// workbench fixture (`crates/dhoby-ghaut/tests/fixtures/
    /// dhoby_ghaut_geometry.csv`) pinned before this type existed, 14 x 12:
    /// 43 445 cells, 22 974 tiles, 1 502 universes, 8335 particles per fuel
    /// zone, rods withdrawn at `z_T` = 119.2 cm.
    #[test]
    fn the_default_design_builds_the_geometry_unchanged() {
        use crate::htr10_rmc::core_model::{assemble_explicit_triso, assemble_explicit_triso_with};
        let a = assemble_explicit_triso(14, 12, 0);
        let b = assemble_explicit_triso_with(14, 12, 0, &Htr10CoreDesign::default());
        for c in [&a, &b] {
            assert_eq!((c.cells, c.tiles, c.universes), (43_445, 22_974, 1_502));
            assert_eq!(c.triso_particles, Some(8335));
            assert_eq!(c.rod_lower_end_zt_cm, Some(119.2));
        }
        assert_eq!(format!("{:?}", a.geometry), format!("{:?}", b.geometry));
    }

    /// The material on the rod's B4C ring, sampled up the whole height of
    /// one control-rod channel of a built core: `(z, material)` pairs.
    fn rod_column(c: &crate::htr10_rmc::core_model::AssembledCore) -> Vec<(f64, Option<usize>)> {
        use crate::htr10_rmc::reflector_geometry::{reflector_channels, ChannelKind};
        use outram_mc_libs::geometry::cell::SurfaceToken;
        use outram_mc_libs::geometry::position::{Direction, Position};
        let ch = reflector_channels()
            .into_iter()
            .find(|c| c.kind == ChannelKind::ControlRod)
            .expect("a control-rod channel");
        let [x, y] = ch.centre_xy();
        // 4.1 cm from the rod axis: inside the B4C ring (3.00 - 5.25 cm).
        let (px, py) = (x + 4.1, y);
        let u = Direction::new(0.0, 0.0, 1.0);
        let n = 600;
        (0..n)
            .map(|i| {
                let z = c.refl_bottom + (c.refl_top - c.refl_bottom) * (i as f64 + 0.5) / n as f64;
                let m = c
                    .geometry
                    .locate(Position::new(px, py, z), u, SurfaceToken::NONE)
                    .and_then(|p| p.material);
                (z, m)
            })
            .collect()
    }

    /// **Inserted rods are in the geometry the solver transports** (gh:#580).
    ///
    /// # Methodology
    /// Build the 14 x 12 core (bed 123.6 cm, the RMC critical loading) at
    /// insertion 0 and 1, and locate 600 points up one rod channel, on the
    /// B4C ring (4.1 cm off the rod axis). The B4C must sit where the rod's
    /// published axial sequence puts it: withdrawn, only above the lower end
    /// at `z_T` 119.2 cm (in the top reflector, above the bed); fully
    /// inserted, from `z_T` 394.2 cm (below the bed floor) up to the rod top
    /// 264.7 cm higher, with helium above it.
    ///
    /// # Results (2026-10-05)
    /// Withdrawn: no B4C below `z_T` 119.2 cm; the bed height carries none.
    /// Inserted: B4C across the whole bed height (bed floor to top), none
    /// above `z_T` 129.5 cm. The B4C sampled column length is 243.5 cm of
    /// absorber in a 264.7 cm rod at full insertion, to the sampling step.
    #[test]
    fn inserted_rods_are_in_the_assembled_geometry() {
        use crate::htr10_rmc::core_model::{assemble_explicit_triso_with, mat};
        let d = Htr10CoreDesign::default();
        let out = assemble_explicit_triso_with(14, 12, 0, &d);
        let inn = assemble_explicit_triso_with(14, 12, 0, &Htr10CoreDesign { rod_insertion: 1.0, ..d });
        assert!((inn.rod_lower_end_zt_cm.expect("rods") - 394.2).abs() < 1e-12);
        let zt = |c: &crate::htr10_rmc::core_model::AssembledCore, z: f64| c.refl_top - z;
        let step = (out.refl_top - out.refl_bottom) / 600.0;
        // Withdrawn: B4C only above the lower end, never at bed height.
        let col = rod_column(&out);
        assert!(col.iter().any(|&(_, m)| m == Some(mat::ROD_B4C)), "no B4C at all withdrawn");
        for &(z, m) in &col {
            if m == Some(mat::ROD_B4C) {
                assert!(zt(&out, z) < 119.2, "withdrawn B4C at z_T {:.1}", zt(&out, z));
            }
        }
        // Inserted: B4C through the bed height, nothing above the rod top.
        let col = rod_column(&inn);
        let b4c: Vec<f64> = col.iter().filter(|p| p.1 == Some(mat::ROD_B4C)).map(|p| p.0).collect();
        let lo = b4c.iter().copied().fold(f64::INFINITY, f64::min);
        let hi = b4c.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        assert!(lo < -inn.bed_half_height && hi > inn.bed_half_height, "B4C spans {lo:.1}..{hi:.1} cm");
        for &(z, m) in &col {
            let t = zt(&inn, z);
            if m == Some(mat::ROD_B4C) {
                assert!(t > 129.5 && t < 394.2, "inserted B4C at z_T {t:.1}");
            }
            if t < 129.5 - step && t > 0.0 {
                assert_eq!(m, Some(mat::HELIUM), "above the inserted rod at z_T {t:.1}");
            }
        }
        let length = b4c.len() as f64 * step;
        assert!((length - 243.5).abs() < 6.0 * step, "B4C length {length:.1} cm");
    }

    /// The design's fuel fraction and TRISO count are what gets built
    /// (gh:#566): a 0.30 bed has 30 % fuel balls above the floor (none in
    /// the conus), and a 4000-particle zone holds the count closest to 4000
    /// the pitch search reaches.
    #[test]
    fn the_design_fuel_fraction_and_particle_count_are_built() {
        use crate::htr10_rmc::core_model::assemble_explicit_triso_with;
        use crate::htr10_rmc::tests::{built_ball_pieces, built_balls};
        let d = Htr10CoreDesign {
            fuel_ball_fraction: 0.30,
            particles_per_pebble: 4000,
            ..Htr10CoreDesign::default()
        };
        let c = assemble_explicit_triso_with(14, 5, 0, &d);
        let (mut n, mut fuel, mut conus_fuel) = (0usize, 0usize, 0usize);
        for (p, flags) in built_balls(&built_ball_pieces(&c)) {
            if p[0].hypot(p[1]) > c.bed_radius {
                continue;
            }
            if p[2] > -c.bed_half_height && p[2] < c.bed_half_height {
                n += 1;
                fuel += usize::from(flags[0]);
            } else if p[2] < -c.bed_half_height {
                conus_fuel += usize::from(flags[0]);
            }
        }
        let f = fuel as f64 / n as f64;
        assert!((f - 0.30).abs() < 2.0e-3, "fuel fraction {f:.4} of {n} balls");
        assert_eq!(conus_fuel, 0);
        let got = c.triso_particles.expect("TRISO lattice built");
        assert!(got.abs_diff(4000) <= 10, "{got} particles");
    }
}
