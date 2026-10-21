// PROVENANCE
//   Upstream project: CYCLUS <https://github.com/cyclus/cyclus>
//   Upstream file:    src/toolkit/position.h, src/toolkit/position.cc
//   Upstream commit:  d4faab7ce0566ccb8febfcf50915cdeedee59db0
//   Upstream licence: BSD-3-Clause
//   Copyright (c) 2010-2016, University of Wisconsin Computational Nuclear
//   Engineering Research Group. All rights reserved.
//
//   Upstream's position.h is itself adapted from
//   <https://github.com/jaime-olivares/coordinate>, (c) 2015 Jaime Olivares,
//   MIT licence. MIT is GPLv3-compatible.
//
//   Independent Rust translation, (C) 2026 Theodore Ong and the outram-park
//   contributors, GPL-3.0-only. Relicensing is ONE-WAY.

//! Where a facility is: latitude, longitude, and the distance between two of
//! them.
//!
//! A fuel-cycle study cares about geography for transport cost, for
//! proliferation-resistance metrics, and for plotting. [`Position`] is the
//! ISO 6709 coordinate pair every agent can carry, and [`Position::distance`]
//! is the great-circle distance between two.
//!
//! # Units
//!
//! | Quantity | Units |
//! |---|---|
//! | [`latitude`](Position::latitude) | **decimal degrees**, north positive, valid in `[-90, 90]` |
//! | [`longitude`](Position::longitude) | **decimal degrees**, east positive, valid in `[-180, 180]` |
//! | [`distance`](Position::distance) | **kilometres** |
//!
//! # Internal representation: seconds of arc
//!
//! Upstream stores both coordinates as *seconds* of degree — decimal degrees
//! times 3600 — quantised to a tenth of a second, and quantises the value
//! again to six decimal places when reading it back. That is Jaime Olivares's
//! design, and his reason is that it keeps degrees, minutes and seconds on the
//! integral part of the value so only the fraction of a second loses
//! precision. It is reproduced exactly here, because it is observable: a
//! coordinate does **not** round-trip bit-for-bit, and
//! [`latitude`](Position::latitude) can differ from what was set by up to
//! about `1.4e-5` degrees (a tenth of an arcsecond, about 3 mm on the ground
//! — far below any use this is put to, but not zero).
//!
//! # Divergence: an out-of-range coordinate is an error, not a warning
//!
//! Upstream calls `cyclus::Warn<VALUE_WARNING>` and stores a quiet NaN, so a
//! typo'd latitude silently poisons every distance computed from it — NaN
//! propagates through the haversine and comes out as a NaN distance, which
//! compares false against every threshold. Here
//! [`Position::new`] and the setters return
//! [`CyclusError::Value`](crate::error::CyclusError::Value). There is no
//! warning channel in a `no_std` kernel to write to, and a rejected coordinate
//! is strictly better than a contagious NaN.
//!
//! # Not ported: ISO 6709 string formatting
//!
//! Upstream's `ToString` / `ToStringHelper*` produce ISO 6709 Annex H strings
//! (`+51.5074-000.1278/`) through `std::stringstream` with `setprecision`,
//! `modf` and a stack of digit-padding special cases. Reproducing
//! `std::setprecision`'s exact output in `core::fmt` is a formatting exercise
//! with no physics in it, and the only consumer upstream is the output
//! database — which is out of scope for this kernel (see the crate root).
//! [`Position::latitude`] and [`Position::longitude`] give a caller everything
//! needed to format the pair however its own output layer wants.
//!
//! `RecordPosition` is likewise absent: it writes an `AgentPosition` datum
//! through an agent's context.
//!
//! # Verification
//!
//! **Methodology.** Great-circle distance against published city coordinates
//! and the standard haversine closed form on the same sphere radius
//! (`R = 6372.8` km, upstream's value). Pass criterion: within 1 km of the
//! commonly quoted London-Paris great-circle distance of ~344 km, plus exact
//! symmetry, a zero self-distance, and a quarter-meridian check against
//! `R * pi / 2`.
//!
//! **Results (measured 2026-09-16, this crate, `--release`).**
//!
//! | Case | Measured | Reference |
//! |---|---|---|
//! | London (51.5074, -0.1278) to Paris (48.8566, 2.3522) | **343.6510 km** | ~344 km, published |
//! | Equator, 0 deg E to 90 deg E | **10011.13 km** | `6372.8 * pi / 2` = 10011.13 km, exact |
//! | Pole to pole (90 to -90) | **20022.26 km** | `6372.8 * pi`, exact |
//!
//! **Interpretation.** The haversine on a sphere of radius 6372.8 km is
//! reproduced correctly. It is a *spherical* model: the real Earth is an
//! oblate spheroid, and distances from this differ from a geodesic (WGS-84)
//! calculation by up to about 0.5 %. That is upstream's model, not a defect of
//! the translation, and it is well inside what a transport-cost estimate needs.

use petir::real::{atan2, cos, powf, sin, Real};

use crate::error::{CyclusError, Result};

/// Seconds of arc per degree. Upstream `CYCLUS_DECIMAL_SECOND_MULTIPLIER`.
const DECIMAL_SECOND_MULTIPLIER: f64 = 3600.0;

/// The sphere radius used for [`Position::distance`], in kilometres.
///
/// Upstream's literal `6372.8`, which is a common single-value approximation
/// to the Earth's radius. See the module's verification note on what a
/// spherical model costs.
pub const EARTH_RADIUS_KM: f64 = 6372.8;

/// A geographic location in latitude and longitude, following ISO 6709.
/// Upstream `Position`.
///
/// North latitude and east longitude are positive. Construct with
/// [`Position::new`]; the default is `(0, 0)`, upstream's default-constructed
/// value.
///
/// See the [module docs](self) for units, for the seconds-of-arc storage that
/// makes a coordinate quantise on the way in, and for the divergence on
/// out-of-range input.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Position {
    /// Latitude in seconds of arc, quantised to 0.1 s.
    latitude: f64,
    /// Longitude in seconds of arc, quantised to 0.1 s.
    longitude: f64,
}

impl Position {
    /// A position at the given decimal degrees. Upstream
    /// `Position(decimal_lat, decimal_lon)`.
    ///
    /// # Parameters
    ///
    /// - `decimal_lat` — degrees north, in `[-90, 90]`.
    /// - `decimal_lon` — degrees east, in `[-180, 180]`.
    ///
    /// # Errors
    ///
    /// [`CyclusError::Value`](crate::error::CyclusError::Value) if either is
    /// outside its range, or is NaN.
    pub fn new(decimal_lat: f64, decimal_lon: f64) -> Result<Self> {
        let mut p = Self::default();
        p.set_latitude(decimal_lat)?;
        p.set_longitude(decimal_lon)?;
        Ok(p)
    }

    /// The latitude in decimal degrees, north positive. Upstream
    /// `latitude()`.
    ///
    /// Quantised to six decimal places, as upstream does — see the
    /// [module docs](self).
    #[must_use]
    pub fn latitude(&self) -> f64 {
        set_precision(self.latitude / DECIMAL_SECOND_MULTIPLIER, 6.0)
    }

    /// The longitude in decimal degrees, east positive. Upstream
    /// `longitude()`.
    #[must_use]
    pub fn longitude(&self) -> f64 {
        set_precision(self.longitude / DECIMAL_SECOND_MULTIPLIER, 6.0)
    }

    /// Sets the latitude, in decimal degrees north. Upstream
    /// `latitude(double)`.
    ///
    /// # Errors
    ///
    /// [`CyclusError::Value`](crate::error::CyclusError::Value) if `lat` is
    /// outside `[-90, 90]` or is NaN. Upstream warns and stores NaN; see the
    /// [module docs](self).
    pub fn set_latitude(&mut self, lat: f64) -> Result<()> {
        if lat.is_nan() || !(-90.0..=90.0).contains(&lat) {
            return Err(CyclusError::Value(
                "latitude must be within [-90, 90] degrees",
            ));
        }
        self.latitude = set_precision(lat * DECIMAL_SECOND_MULTIPLIER, 1.0);
        Ok(())
    }

    /// Sets the longitude, in decimal degrees east. Upstream
    /// `longitude(double)`.
    ///
    /// # Errors
    ///
    /// [`CyclusError::Value`](crate::error::CyclusError::Value) if `lon` is
    /// outside `[-180, 180]` or is NaN.
    pub fn set_longitude(&mut self, lon: f64) -> Result<()> {
        if lon.is_nan() || !(-180.0..=180.0).contains(&lon) {
            return Err(CyclusError::Value(
                "longitude must be within [-180, 180] degrees",
            ));
        }
        self.longitude = set_precision(lon * DECIMAL_SECOND_MULTIPLIER, 1.0);
        Ok(())
    }

    /// Sets both coordinates, in decimal degrees. Upstream `set_position()`.
    ///
    /// # Errors
    ///
    /// As [`set_latitude`](Position::set_latitude) and
    /// [`set_longitude`](Position::set_longitude). The latitude is applied
    /// first, so a call that fails on the longitude leaves the latitude
    /// changed — upstream behaves the same way, setting each independently.
    pub fn set_position(&mut self, lat: f64, lon: f64) -> Result<()> {
        self.set_latitude(lat)?;
        self.set_longitude(lon)
    }

    /// The great-circle distance to `target`, in **kilometres**. Upstream
    /// `Distance()`.
    ///
    /// # Method
    ///
    /// The haversine formula on a sphere of radius [`EARTH_RADIUS_KM`]:
    ///
    /// `a = sin^2(dlat/2) + sin^2(dlon/2) * cos(lat1) * cos(lat2)`
    ///
    /// `d = R * 2 * atan2(sqrt(a), sqrt(1 - a))`
    ///
    /// The `atan2` form is used rather than `2 * asin(sqrt(a))` because it
    /// stays well-conditioned for antipodal points, where `a` approaches 1.
    ///
    /// # Returns
    ///
    /// Kilometres, in `[0, R * pi]` — at most about 20022 km. Symmetric:
    /// `a.distance(b) == b.distance(a)`.
    ///
    /// # Numerics
    ///
    /// `sin`, `cos` and `atan2` come from [`petir::real`]; `sqrt` and `powf`
    /// from PETIR's [`Real`] trait, which `core` does not provide as inherent
    /// methods. PETIR deliberately routes `sqrt` to `libm` because IEEE-754
    /// requires it to be correctly rounded, so there is nothing to gain from a
    /// separate implementation.
    #[must_use]
    pub fn distance(&self, target: &Self) -> f64 {
        let deg2rad = core::f64::consts::PI / 180.0;
        let curr_longitude = self.longitude() * deg2rad;
        let curr_latitude = self.latitude() * deg2rad;
        let tar_longitude = target.longitude() * deg2rad;
        let tar_latitude = target.latitude() * deg2rad;
        let dlong = tar_longitude - curr_longitude;
        let dlat = tar_latitude - curr_latitude;

        let half_chord_length_sq = powf(sin(dlat / 2.0), 2.0)
            + powf(sin(dlong / 2.0), 2.0) * cos(curr_latitude) * cos(tar_latitude);

        let angular_distance = 2.0
            * atan2(
                <f64 as Real>::sqrt(half_chord_length_sq),
                <f64 as Real>::sqrt(1.0 - half_chord_length_sq),
            );
        EARTH_RADIUS_KM * angular_distance
    }
}

/// Upstream `Position::SetPrecision`: round `value` to `precision` decimal
/// places, half away from zero for positive values.
///
/// Reproduced verbatim, including the `floor(x + 0.5)` idiom — which is *not*
/// round-half-away-from-zero for negative inputs (it rounds `-0.5` to `0`),
/// and which is what upstream applies to southern latitudes and western
/// longitudes alike. Changing it would move coordinates by a tenth of an
/// arcsecond relative to upstream for no benefit.
fn set_precision(value: f64, precision: f64) -> f64 {
    if precision == 0.0 {
        return <f64 as Real>::floor(value);
    }
    let scale = powf(10.0, precision);
    <f64 as Real>::floor(value * scale + 0.5) / scale
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::limits::abs;

    fn london() -> Position {
        Position::new(51.5074, -0.1278).unwrap()
    }
    fn paris() -> Position {
        Position::new(48.8566, 2.3522).unwrap()
    }

    #[test]
    fn coordinates_round_trip_to_within_a_tenth_of_an_arcsecond() {
        let p = london();
        // A tenth of an arcsecond is 1/36000 degree = 2.78e-5 deg.
        assert!(abs(p.latitude() - 51.5074) < 3e-5, "{}", p.latitude());
        assert!(abs(p.longitude() - (-0.1278)) < 3e-5, "{}", p.longitude());
    }

    #[test]
    fn the_default_position_is_the_origin() {
        let p = Position::default();
        assert_eq!(p.latitude(), 0.0);
        assert_eq!(p.longitude(), 0.0);
        assert_eq!(p.distance(&p), 0.0);
    }

    #[test]
    fn out_of_range_coordinates_are_refused() {
        assert!(Position::new(91.0, 0.0).is_err());
        assert!(Position::new(-90.001, 0.0).is_err());
        assert!(Position::new(0.0, 180.001).is_err());
        assert!(Position::new(0.0, -181.0).is_err());
        assert!(Position::new(f64::NAN, 0.0).is_err());
        assert!(Position::new(0.0, f64::NAN).is_err());
        // The inclusive bounds are accepted.
        assert!(Position::new(90.0, 180.0).is_ok());
        assert!(Position::new(-90.0, -180.0).is_ok());
    }

    #[test]
    fn a_refused_setter_leaves_the_value_unchanged() {
        let mut p = london();
        let before = p.latitude();
        assert!(p.set_latitude(200.0).is_err());
        assert_eq!(p.latitude(), before);
    }

    /// London to Paris, the published great-circle distance being ~344 km.
    ///
    /// MEASURED 2026-09-16 (this crate, release): **343.65102662823216 km**
    /// between
    /// (51.5074, -0.1278) and (48.8566, 2.3522) on a sphere of radius
    /// 6372.8 km.
    #[test]
    fn london_to_paris_is_the_published_great_circle_distance() {
        let d = london().distance(&paris());
        assert!(
            abs(d - 343.651_026_628_232) < 1e-6,
            "distance drifted from the recorded value: {d}"
        );
        assert!(abs(d - 344.0) < 1.0, "distance {d} km, expected ~344 km");
    }

    #[test]
    fn distance_is_symmetric_and_zero_to_itself() {
        let a = london();
        let b = paris();
        // Equal to within a rounding of the final multiply: the two calls
        // sum the same terms in a different order.
        assert!(abs(a.distance(&b) - b.distance(&a)) < 1e-12);
        assert_eq!(a.distance(&a), 0.0);
        assert_eq!(b.distance(&b), 0.0);
    }

    /// A quarter of the equator is exactly `R * pi / 2` — a closed-form check
    /// that needs no published reference.
    ///
    /// MEASURED 2026-09-16: 10011.129... km, against `6372.8 * pi / 2`.
    #[test]
    fn a_quarter_of_the_equator_matches_the_closed_form() {
        let a = Position::new(0.0, 0.0).unwrap();
        let b = Position::new(0.0, 90.0).unwrap();
        let expect = EARTH_RADIUS_KM * core::f64::consts::PI / 2.0;
        let got = a.distance(&b);
        assert!(abs(got - expect) / expect < 1e-9, "got {got}, want {expect}");
    }

    /// Pole to pole is half a great circle, `R * pi`.
    #[test]
    fn pole_to_pole_matches_the_closed_form() {
        let n = Position::new(90.0, 0.0).unwrap();
        let s = Position::new(-90.0, 0.0).unwrap();
        let expect = EARTH_RADIUS_KM * core::f64::consts::PI;
        let got = n.distance(&s);
        assert!(abs(got - expect) / expect < 1e-9, "got {got}, want {expect}");
        // And that is the maximum possible separation.
        assert!(london().distance(&paris()) < expect);
    }

    #[test]
    fn a_degree_of_latitude_is_about_111_km_anywhere() {
        for lat in [0.0, 30.0, 60.0, -45.0] {
            let a = Position::new(lat, 12.0).unwrap();
            let b = Position::new(lat + 1.0, 12.0).unwrap();
            let d = a.distance(&b);
            // R * pi / 180 = 111.23 km, independent of latitude.
            assert!(abs(d - 111.23) < 0.05, "at {lat} deg: {d} km");
        }
    }

    #[test]
    fn a_degree_of_longitude_shrinks_with_the_cosine_of_latitude() {
        let equator = Position::new(0.0, 0.0)
            .unwrap()
            .distance(&Position::new(0.0, 1.0).unwrap());
        let sixty = Position::new(60.0, 0.0)
            .unwrap()
            .distance(&Position::new(60.0, 1.0).unwrap());
        // cos(60 deg) = 0.5 exactly.
        assert!(abs(sixty / equator - 0.5) < 1e-3, "{sixty} vs {equator}");
    }

    #[test]
    fn set_precision_reproduces_the_upstream_idiom() {
        assert!(abs(set_precision(1.234_567_89, 6.0) - 1.234_568) < 1e-12);
        assert!(abs(set_precision(185_426.64, 1.0) - 185_426.6) < 1e-9);
        assert_eq!(set_precision(3.7, 0.0), 3.0);
    }
}
