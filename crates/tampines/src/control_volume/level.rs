// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.
//! The volume–altitude table and the swollen pool level (gh:#1092).
//!
//! > Research, education and V&V only. AI draft, unreviewed.
//!
//! - [`VolumeAltitudeTable`]: MELCOR's description of a control volume's
//!   shape (CVH users' guide, the volume–altitude table; MELCOR 2.2 CVH/FL
//!   reference manual §2, cited, not quoted): the free volume below each
//!   altitude, piecewise linear between the points, so a non-prismatic vessel
//!   (a torus, a lower head, a vessel with internals) has a level that is not
//!   `V / A`. The inverse is exact on each linear piece (no iteration).
//! - [`swollen_level`] and [`swollen_level_from_vapour_flow`]: MELCOR's pool
//!   swell (CVH/FL RM §5.1.3, Eqs 5-26 to 5-30). **Ported from
//!   `sembawang::bwr_mark1::swollen_level`** (gh:#1036), with the vessel's
//!   `volume_below` and `height_of_volume` passed in as closures instead of
//!   `sembawang`'s BWR vessel; the operations and their order are kept, so
//!   on `sembawang`'s own vessel the result is bit-identical
//!   (`crates/sembawang/tests/swollen_level_vs_tampines.rs`).

use uom::si::area::square_meter;
use uom::si::f64::{Area, Length, Volume};
use uom::si::length::meter;
use uom::si::volume::cubic_meter;

use crate::TampinesError;

/// MELCOR's default bubble rise velocity in a pool, m/s (CVH/FL RM §5.1.3,
/// sensitivity coefficient C4407(1); `sembawang::bwr_mark1::params::BUBBLE_RISE_M_S`).
/// The manual says other assumptions change the answer by about a factor of 2.
pub const BUBBLE_RISE_M_S: f64 = 0.3;
/// MELCOR's default largest mean pool void (C4407(11), roughly the upper
/// limit of bubbly flow; `sembawang::bwr_mark1::params::MAX_POOL_VOID`).
pub const MAX_POOL_VOID: f64 = 0.4;

/// The swollen level, from the collapsed level `z` (same datum as the
/// closures) and the vapour volume flow `vapour_flow_m3_s` rising through the
/// pool top: the free cross-section at `z` by a centred difference of ±1 cm,
/// the superficial vapour velocity `J = Q_v / A`, the mean void `α = J / (2
/// v0)` capped at `max_void` (Eqs 5-26 to 5-30), and the swollen pool filling
/// `V(z) / (1 − α)`. No vapour or no cross-section: the collapsed level.
/// **Port of `sembawang::bwr_mark1::swollen_level`** with `Q_v = q / h_fg ·
/// v_g` factored out (the same operations in the same order).
pub fn swollen_level_from_vapour_flow<V, H>(volume_below: V, height_of_volume: H, z: f64, vapour_flow_m3_s: f64, bubble_rise_m_s: f64, max_void: f64) -> f64
where
    V: Fn(f64) -> f64,
    H: Fn(f64) -> f64,
{
    let dz = 0.01;
    let area = (volume_below(z + dz) - volume_below(z - dz)) / (2.0 * dz);
    if area <= 0.0 || vapour_flow_m3_s <= 0.0 {
        return z;
    }
    let j = vapour_flow_m3_s / area;
    let alpha = (j / (2.0 * bubble_rise_m_s)).min(max_void);
    height_of_volume(volume_below(z) / (1.0 - alpha))
}

/// `sembawang`'s form: the vapour raised by `q_pool_w` W of heat with latent
/// heat `h_fg` J/kg and vapour specific volume `v_g` m³/kg. With `z` =
/// `level_m + baf_m` and the same vessel closures this returns
/// `sembawang::bwr_mark1::swollen_level(..) + baf_m` bit for bit when
/// `q_pool_w > 0` (with no heat, `z` itself).
#[allow(clippy::too_many_arguments)]
pub fn swollen_level<V, H>(volume_below: V, height_of_volume: H, z: f64, q_pool_w: f64, h_fg: f64, v_g: f64, bubble_rise_m_s: f64, max_void: f64) -> f64
where
    V: Fn(f64) -> f64,
    H: Fn(f64) -> f64,
{
    if q_pool_w <= 0.0 {
        return z;
    }
    swollen_level_from_vapour_flow(volume_below, height_of_volume, z, q_pool_w / h_fg * v_g, bubble_rise_m_s, max_void)
}

/// A vessel's free volume below each altitude, piecewise linear between the
/// points (MELCOR's volume–altitude table).
#[derive(Clone, Debug, PartialEq)]
pub struct VolumeAltitudeTable {
    z: Vec<f64>,
    v: Vec<f64>,
}

impl VolumeAltitudeTable {
    /// A table from `(altitude, volume below it)` points. The first volume is
    /// zero (the vessel's bottom), and both columns strictly increase.
    ///
    /// # Errors
    ///
    /// [`TampinesError::InvalidInput`] for fewer than two points, a non-zero
    /// first volume, a non-finite value, or a column that does not strictly
    /// increase.
    pub fn new(points: &[(Length, Volume)]) -> Result<Self, TampinesError> {
        let z: Vec<f64> = points.iter().map(|p| p.0.get::<meter>()).collect();
        let v: Vec<f64> = points.iter().map(|p| p.1.get::<cubic_meter>()).collect();
        if z.len() < 2 {
            return Err(TampinesError::InvalidInput("a volume-altitude table needs two points".into()));
        }
        if v[0] != 0.0 {
            return Err(TampinesError::InvalidInput(format!("the first volume is {} m3, not 0", v[0])));
        }
        if z.iter().chain(v.iter()).any(|x| !x.is_finite()) || z.windows(2).any(|w| w[1] <= w[0]) || v.windows(2).any(|w| w[1] <= w[0]) {
            return Err(TampinesError::InvalidInput(format!("volume-altitude table not strictly increasing: z {z:?}, V {v:?}")));
        }
        Ok(Self { z, v })
    }

    /// A prismatic vessel of cross-section `area` from altitude 0 to `height`.
    ///
    /// # Errors
    ///
    /// As [`Self::new`] (a non-positive area or height).
    pub fn prismatic(area: Area, height: Length) -> Result<Self, TampinesError> {
        let vol = area * height;
        Self::new(&[(Length::new::<meter>(0.0), Volume::new::<cubic_meter>(0.0)), (height, vol)])
    }

    /// The bottom and top altitudes, m.
    pub(crate) fn bottom_m(&self) -> f64 {
        self.z[0]
    }
    pub(crate) fn top_m(&self) -> f64 {
        self.z[self.z.len() - 1]
    }
    /// The total volume, m³.
    pub(crate) fn total_m3(&self) -> f64 {
        self.v[self.v.len() - 1]
    }

    /// Volume below altitude `z`, m³, clamped to the vessel.
    pub(crate) fn volume_below_m3(&self, z: f64) -> f64 {
        if z <= self.z[0] {
            return 0.0;
        }
        for i in 1..self.z.len() {
            if z <= self.z[i] {
                return self.v[i - 1] + (z - self.z[i - 1]) / (self.z[i] - self.z[i - 1]) * (self.v[i] - self.v[i - 1]);
            }
        }
        self.total_m3()
    }

    /// Altitude below which the volume is `v`, m, clamped to the vessel.
    pub(crate) fn height_of_volume_m(&self, v: f64) -> f64 {
        if v <= 0.0 {
            return self.z[0];
        }
        for i in 1..self.v.len() {
            if v <= self.v[i] {
                return self.z[i - 1] + (v - self.v[i - 1]) / (self.v[i] - self.v[i - 1]) * (self.z[i] - self.z[i - 1]);
            }
        }
        self.top_m()
    }

    /// The free volume below altitude `z`.
    pub fn volume_below(&self, z: Length) -> Volume {
        Volume::new::<cubic_meter>(self.volume_below_m3(z.get::<meter>()))
    }
    /// The altitude below which the free volume is `v`.
    pub fn height_of_volume(&self, v: Volume) -> Length {
        Length::new::<meter>(self.height_of_volume_m(v.get::<cubic_meter>()))
    }
    /// The free cross-section at altitude `z` (the slope of the piece holding
    /// it; at a point, the piece below).
    pub fn area_at(&self, z: Length) -> Area {
        let z = z.get::<meter>();
        let n = self.z.len();
        let mut i = 1;
        while i < n - 1 && z > self.z[i] {
            i += 1;
        }
        Area::new::<square_meter>((self.v[i] - self.v[i - 1]) / (self.z[i] - self.z[i - 1]))
    }
    /// The vessel's total free volume.
    pub fn total_volume(&self) -> Volume {
        Volume::new::<cubic_meter>(self.total_m3())
    }
}
