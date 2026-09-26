// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors

//! **The HTR-10 reflector as explicit 3-D geometry**: every boring at its own
//! position, in solid graphite, inside the benchmark's R-Z zone map.
//!
//! # Why this replaced the homogenised bands (2026-09-25)
//!
//! Maintainer direction: *"Don't average, just make channels explicit."* Until
//! then the reflector was TECDOC zone 22 almost everywhere, a full-height void
//! annulus at r 140.6-148.6 stood in for the twenty coolant channels, and the
//! control-rod, absorber-ball and irradiation borings were absent (their
//! smeared zones 31-40 existed only behind `OUTRAM_HTR10_BORINGS`).
//!
//! # The specification
//!
//! All inputs come from **IAEA-TECDOC-1382 part 2, § 4.1.2**, transcribed with
//! page references in `crates/kovan-literature/derived/tecdoc1382-htr10-mc-borings-and-zone-map.md`:
//!
//! - **p. 241-242, the borings to model in a Monte Carlo calculation:** count,
//!   diameter, radial centre and axial extent of the coolant, control-rod,
//!   irradiation and small-absorber-ball (KLAK) channels, and of the hot gas
//!   duct. p. 242 has no text layer in the PDF, which is why this never
//!   reached the workspace's Markdown copy.
//! - **p. 242, the density corrections** that go with them: once the borings
//!   are explicit, the zones that had homogenised them take the solid
//!   graphite (zone 22), boronated brick (17) or carbon brick (18) densities,
//!   and zones 29/42 and 60 are scaled back up. See
//!   [`super::core_model::mat::for_zone_mc`].
//! - **p. 241, Fig. 4.10, the zone map**, as [`FIG_4_10_BOXES`].
//! - **p. 234, Fig. 4.7**, for the KLAK slot's shape (100 mm straight + R30).
//!
//! Li, Yu & Wei (2014), the reference being compared against, states that its
//! top and side reflectors house the control rods, small absorber balls,
//! helium flow channels and irradiation channels, and refers every reflector
//! detail to that TECDOC. It never says any of them was homogenised.
//!
//! # What the specification does NOT give, and what is done instead
//!
//! **Channel azimuths.** The text gives counts and radii only, and Fig. 4.7 is
//! a 416 x 233 px raster (about 2 cm per pixel) that cannot resolve them. So
//! the placements below are a stated convention, NOT data: the 20 inner-ring
//! borings on an 18 degree pitch, the 20 coolant channels offset by 9
//! degrees, the hot gas duct along +x. The only constraint used is
//! geometric: TECDOC's own zone 44/62 densities are exactly additive in the
//! duct and channel voids, i.e. the duct overlaps none of them, and this
//! layout honours that. The assignment of the 20 inner-ring positions to 10
//! rods, 3 irradiation and 7 KLAK channels is likewise a convention. It
//! matters for the one-rod worth problems (B32, B42), not for B1.
//!
//! **Contents.** B1 is defined with no rod inserted (p. 242), and the rods'
//! withdrawn position is given (lower end at 119.2 cm), so the rods ARE in
//! their channels, in the top reflector, with their B4C, steel sleeves and
//! iron joints as explicit geometry. The absorber-ball system is a reserve
//! shutdown system, so its channels are empty. The irradiation channels are
//! empty. Nothing is said about either; both are open items.
//!
//! **Zones whose internal structure is unspecified** (the cold helium chamber,
//! zone 3; the bottom structures, zones 0 and 8-16; the partly-void layers 21,
//! 29, 48, 57): these are explicit REGIONS at their Fig. 4.10 positions, but
//! each carries the source's own Table 4-3 composition because no geometry
//! for their contents is given anywhere. That is the source's homogenisation,
//! not ours, and it is recorded as an open item.
//!
//! # Coordinates
//!
//! TECDOC's axial coordinate `z_T` runs **downward** from the model top
//! (0) to the bottom (610 cm). The model's local z runs upward with the origin
//! at bed mid-height, so `z_local = refl_top - z_T`. Every zone boundary here
//! is fixed hardware: nothing but the bed top moves with the loading.

use outram_mc_libs::geometry::cell::{Cell, CellFill, HalfSpaceSense, RegionToken};
use outram_mc_libs::geometry::position::Position;
use outram_mc_libs::geometry::surface::{
    BoundaryType, Plane, SurfaceKind, XCylinder, XPlane, ZCylinder, ZPlane,
};
use outram_mc_libs::geometry::universe::Universe;

use super::control_rod::{
    AXIAL_IS_B4C, AXIAL_SECTIONS_CM, LOWER_END_WITHDRAWN_CM, N_CONTROL_RODS,
};

/// First cell id of the reflector's cells, clear of the bed-tile id ranges.
pub const REFLECTOR_CELL_ID_BASE: i32 = 100_000;

/// Model bottom, `z_T` \[cm\] (Fig. 4.10).
pub const MODEL_BOTTOM_ZT_CM: f64 = 610.0;

/// Coolant (cold helium flow) channels: count (p. 241).
pub const N_COOLANT_CHANNELS: usize = 20;
/// Coolant channel radius \[cm\]: 80 mm diameter (p. 241).
pub const COOLANT_RADIUS_CM: f64 = 4.0;
/// Coolant channel centre radius \[cm\]: 1446 mm (p. 241).
pub const COOLANT_CENTRE_RADIUS_CM: f64 = 144.6;
/// Coolant channel axial extent, `z_T` \[cm\]: 1050-6100 mm (p. 241).
pub const COOLANT_ZT_CM: (f64, f64) = (105.0, 610.0);

/// Control-rod and irradiation channel radius \[cm\]: 130 mm diameter (p. 242).
pub const ROD_CHANNEL_RADIUS_CM: f64 = 6.5;
/// Control-rod and irradiation channel centre radius \[cm\]: 1021 mm (p. 242).
pub const ROD_CHANNEL_CENTRE_RADIUS_CM: f64 = 102.1;
/// Control-rod and irradiation channel axial extent, `z_T` \[cm\]: 0-4500 mm
/// (p. 242).
pub const ROD_CHANNEL_ZT_CM: (f64, f64) = (0.0, 450.0);
/// Irradiation channels: count (p. 234, p. 242).
pub const N_IRRADIATION_CHANNELS: usize = 3;

/// Small-absorber-ball (KLAK) channels: count (p. 234, p. 242).
pub const N_KLAK_CHANNELS: usize = 7;
/// KLAK channel centre radius \[cm\]: 986 mm (p. 242; Fig. 4.7).
pub const KLAK_CENTRE_RADIUS_CM: f64 = 98.6;
/// KLAK channel radius \[cm\]: round, 60 mm diameter, above and below core
/// height (p. 242); also the R30 end radius of the slot (Fig. 4.7).
pub const KLAK_RADIUS_CM: f64 = 3.0;
/// KLAK slot straight length between the two end-arc centres \[cm\]: 100 mm
/// (Fig. 4.7). Slot area `pi 3^2 + 6 x 10 = 88.27 cm^2`.
pub const KLAK_SLOT_STRAIGHT_CM: f64 = 10.0;
/// KLAK channel axial extent, `z_T` \[cm\] (p. 242).
pub const KLAK_ZT_CM: (f64, f64) = (0.0, 610.0);
/// Where the KLAK channel is the slot rather than round, `z_T` \[cm\]:
/// 1300-3887.64 mm (p. 242).
pub const KLAK_SLOT_ZT_CM: (f64, f64) = (130.0, 388.764);

/// Hot gas duct radius \[cm\]: 300 mm diameter (p. 242).
pub const HOT_GAS_DUCT_RADIUS_CM: f64 = 15.0;
/// Hot gas duct axis, `z_T` \[cm\]: z = 4800 mm (p. 242).
pub const HOT_GAS_DUCT_AXIS_ZT_CM: f64 = 480.0;
/// Hot gas duct radial extent \[cm\]: R = 900-1900 mm (p. 242).
pub const HOT_GAS_DUCT_RHO_CM: (f64, f64) = (90.0, 190.0);

/// Angular pitch \[deg\] of the 20 inner-ring borings (10 rods + 3
/// irradiation + 7 KLAK). **A convention, not data**: see the module docs.
pub const INNER_RING_PITCH_DEG: f64 = 18.0;
/// Azimuthal offset \[deg\] of the coolant ring against the inner ring, so no
/// coolant channel meets the hot gas duct. **A convention, not data.**
pub const COOLANT_OFFSET_DEG: f64 = 9.0;
/// Inner-ring positions holding a KLAK channel. **A convention, not data.**
/// Spread as evenly as 7 in 20 allows, and never position 0, which is on the
/// hot gas duct's azimuth: a KLAK channel reaches the duct's height, a rod
/// channel stops above it.
pub const KLAK_POSITIONS: [usize; N_KLAK_CHANNELS] = [1, 4, 7, 10, 12, 15, 18];
/// Inner-ring positions holding an irradiation channel. **A convention, not
/// data.**
pub const IRRADIATION_POSITIONS: [usize; N_IRRADIATION_CHANNELS] = [3, 9, 16];

/// What a reflector channel is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelKind {
    /// Cold-helium coolant channel: empty.
    Coolant,
    /// Control-rod channel: holds a rod at its withdrawn position.
    ControlRod,
    /// Irradiation channel: empty.
    Irradiation,
    /// Small-absorber-ball (KLAK) channel: empty (reserve shutdown system).
    AbsorberBall,
}

/// One vertical channel in the reflector.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReflectorChannel {
    /// What it is.
    pub kind: ChannelKind,
    /// Azimuth of its centre \[deg\] from +x. A convention: see the module docs.
    pub azimuth_deg: f64,
}

impl ReflectorChannel {
    /// Radius \[cm\] of the channel centre from the core axis.
    #[must_use]
    pub fn centre_radius_cm(&self) -> f64 {
        match self.kind {
            ChannelKind::Coolant => COOLANT_CENTRE_RADIUS_CM,
            ChannelKind::ControlRod | ChannelKind::Irradiation => ROD_CHANNEL_CENTRE_RADIUS_CM,
            ChannelKind::AbsorberBall => KLAK_CENTRE_RADIUS_CM,
        }
    }

    /// Channel centre `(x, y)` \[cm\].
    #[must_use]
    pub fn centre_xy(&self) -> [f64; 2] {
        let (s, c) = self.azimuth_deg.to_radians().sin_cos();
        let r = self.centre_radius_cm();
        [r * c, r * s]
    }

    /// Axial extent, `(z_T top, z_T bottom)` \[cm\].
    #[must_use]
    pub fn zt_range(&self) -> (f64, f64) {
        match self.kind {
            ChannelKind::Coolant => COOLANT_ZT_CM,
            ChannelKind::ControlRod | ChannelKind::Irradiation => ROD_CHANNEL_ZT_CM,
            ChannelKind::AbsorberBall => KLAK_ZT_CM,
        }
    }

    /// Radial extent `(r_min, r_max)` \[cm\] from the core axis at `z_T`
    /// (the KLAK slot is wider than its round sections). Exact for the round
    /// channels; for the slot, the corners of its straight section.
    #[must_use]
    pub fn radial_extent_cm(&self, zt: f64) -> (f64, f64) {
        let rc = self.centre_radius_cm();
        let a = match self.kind {
            ChannelKind::Coolant => COOLANT_RADIUS_CM,
            ChannelKind::ControlRod | ChannelKind::Irradiation => ROD_CHANNEL_RADIUS_CM,
            ChannelKind::AbsorberBall => KLAK_RADIUS_CM,
        };
        if self.kind == ChannelKind::AbsorberBall && is_klak_slot_zt(zt) {
            // A tangential slot: nearest point is the straight flank's middle,
            // farthest its outer corners.
            let half = 0.5 * KLAK_SLOT_STRAIGHT_CM;
            ((rc - a), ((rc + a).powi(2) + half * half).sqrt().max(rc + a))
        } else {
            (rc - a, rc + a)
        }
    }
}

/// Whether `z_T` lies in the KLAK channel's slot-shaped section.
#[must_use]
pub fn is_klak_slot_zt(zt: f64) -> bool {
    zt > KLAK_SLOT_ZT_CM.0 && zt < KLAK_SLOT_ZT_CM.1
}

/// Every vertical channel in the reflector: 20 coolant, 10 control-rod,
/// 3 irradiation, 7 KLAK. Positions follow the conventions in the module docs.
#[must_use]
pub fn reflector_channels() -> Vec<ReflectorChannel> {
    let mut v = Vec::with_capacity(40);
    for k in 0..N_COOLANT_CHANNELS {
        v.push(ReflectorChannel {
            kind: ChannelKind::Coolant,
            azimuth_deg: COOLANT_OFFSET_DEG + INNER_RING_PITCH_DEG * k as f64,
        });
    }
    let n_inner = N_CONTROL_RODS + N_IRRADIATION_CHANNELS + N_KLAK_CHANNELS;
    for k in 0..n_inner {
        let kind = if KLAK_POSITIONS.contains(&k) {
            ChannelKind::AbsorberBall
        } else if IRRADIATION_POSITIONS.contains(&k) {
            ChannelKind::Irradiation
        } else {
            ChannelKind::ControlRod
        };
        v.push(ReflectorChannel {
            kind,
            azimuth_deg: INNER_RING_PITCH_DEG * k as f64,
        });
    }
    v
}

/// One rectangle of the Fig. 4.10 zone map, in `(r, z_T)`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Fig410Box {
    /// Table 4-3 zone number, as printed in the figure.
    pub zone: usize,
    /// Inner radius \[cm\].
    pub r_in: f64,
    /// Outer radius \[cm\].
    pub r_out: f64,
    /// Top, `z_T` \[cm\] (smaller number: `z_T` runs downward).
    pub zt_top: f64,
    /// Bottom, `z_T` \[cm\].
    pub zt_bot: f64,
}

const fn bx(zone: usize, r_in: f64, r_out: f64, zt_top: f64, zt_bot: f64) -> Fig410Box {
    Fig410Box {
        zone,
        r_in,
        r_out,
        zt_top,
        zt_bot,
    }
}

/// **IAEA-TECDOC-1382 Fig. 4.10**, every zone outside the pebble-filled
/// interior, as printed (transcription and cross-checks in
/// `kovan-literature/derived/tecdoc1382-htr10-mc-borings-and-zone-map.md`).
///
/// Not listed, because they are built from the bed's own surfaces: zone 5
/// (the cavity), the bed, the conus, zone 0 (between the cone and r = 90) and
/// the discharge tube, zones 6, 7 and 81, which Li (2014) fills with graphite
/// balls. Zones 31-40 are one box: the figure does not label their internal
/// boundaries and all ten share one density. Zone 66 is L-shaped and appears
/// twice.
pub const FIG_4_10_BOXES: &[Fig410Box] = &[
    // Top reflector over the core.
    bx(1, 0.0, 90.0, 0.0, 40.0),
    bx(2, 0.0, 90.0, 40.0, 95.0),
    bx(3, 0.0, 90.0, 95.0, 105.0),
    bx(4, 0.0, 90.0, 105.0, 130.0),
    // r 90 - 95.6
    bx(19, 90.0, 95.6, 0.0, 40.0),
    bx(20, 90.0, 95.6, 40.0, 95.0),
    bx(21, 90.0, 95.6, 95.0, 105.0),
    bx(22, 90.0, 95.6, 105.0, 388.764),
    bx(23, 90.0, 95.6, 388.764, 430.0),
    bx(24, 90.0, 95.6, 430.0, 450.0),
    bx(25, 90.0, 95.6, 450.0, 465.0),
    bx(26, 90.0, 95.6, 465.0, 495.0),
    bx(80, 90.0, 95.6, 495.0, 510.0),
    // r 95.6 - 108.6: the control-rod / KLAK / irradiation band
    bx(27, 95.6, 108.6, 0.0, 40.0),
    bx(28, 95.6, 108.6, 40.0, 95.0),
    bx(29, 95.6, 108.6, 95.0, 105.0),
    bx(82, 95.6, 108.6, 105.0, 114.7),
    bx(30, 95.6, 108.6, 114.7, 130.0),
    bx(31, 95.6, 108.6, 130.0, 388.764),
    bx(41, 95.6, 108.6, 388.764, 430.0),
    bx(42, 95.6, 108.6, 430.0, 450.0),
    bx(43, 95.6, 108.6, 450.0, 465.0),
    bx(44, 95.6, 108.6, 465.0, 495.0),
    bx(45, 95.6, 108.6, 495.0, 510.0),
    bx(46, 95.6, 108.6, 510.0, 540.0),
    bx(47, 95.6, 108.6, 540.0, 610.0),
    // r 108.6 - 167.793, above z_T 40
    bx(74, 108.6, 167.793, 0.0, 40.0),
    bx(66, 108.6, 148.6, 40.0, 95.0),
    // r 108.6 - 140.6
    bx(48, 108.6, 140.6, 95.0, 105.0),
    bx(49, 108.6, 140.6, 105.0, 388.764),
    bx(50, 108.6, 140.6, 388.764, 430.0),
    bx(51, 108.6, 140.6, 430.0, 450.0),
    bx(52, 108.6, 140.6, 450.0, 465.0),
    bx(53, 108.6, 140.6, 465.0, 495.0),
    bx(54, 108.6, 140.6, 495.0, 510.0),
    bx(55, 108.6, 140.6, 510.0, 540.0),
    bx(56, 108.6, 140.6, 540.0, 610.0),
    // r 140.6 - 148.6: the coolant band
    bx(57, 140.6, 148.6, 95.0, 105.0),
    bx(58, 140.6, 148.6, 105.0, 388.764),
    bx(59, 140.6, 148.6, 388.764, 430.0),
    bx(60, 140.6, 148.6, 430.0, 450.0),
    bx(61, 140.6, 148.6, 450.0, 465.0),
    bx(62, 140.6, 148.6, 465.0, 495.0),
    bx(63, 140.6, 148.6, 495.0, 510.0),
    bx(64, 140.6, 148.6, 510.0, 540.0),
    bx(65, 140.6, 148.6, 540.0, 610.0),
    // r 148.6 - 167.793
    bx(66, 148.6, 167.793, 40.0, 388.764),
    bx(67, 148.6, 167.793, 388.764, 430.0),
    bx(68, 148.6, 167.793, 430.0, 450.0),
    bx(69, 148.6, 167.793, 450.0, 465.0),
    bx(70, 148.6, 167.793, 465.0, 495.0),
    bx(71, 148.6, 167.793, 495.0, 510.0),
    bx(72, 148.6, 167.793, 510.0, 540.0),
    bx(73, 148.6, 167.793, 540.0, 610.0),
    // r 167.793 - 190: boronated carbon bricks
    bx(75, 167.793, 190.0, 0.0, 40.0),
    bx(76, 167.793, 190.0, 40.0, 465.0),
    bx(77, 167.793, 190.0, 465.0, 495.0),
    bx(78, 167.793, 190.0, 495.0, 540.0),
    bx(79, 167.793, 190.0, 540.0, 610.0),
    // Bottom structures around the discharge tube
    bx(8, 25.0, 90.0, 388.764, 402.0),
    bx(9, 25.0, 90.0, 402.0, 430.0),
    bx(10, 25.0, 41.75, 430.0, 450.0),
    bx(11, 41.75, 90.0, 430.0, 450.0),
    bx(12, 25.0, 41.75, 450.0, 510.0),
    bx(13, 41.75, 90.0, 450.0, 465.0),
    bx(14, 41.75, 70.75, 465.0, 495.0),
    bx(15, 70.75, 90.0, 465.0, 495.0),
    bx(16, 41.75, 90.0, 495.0, 510.0),
    bx(17, 25.0, 95.6, 510.0, 540.0),
    bx(18, 25.0, 95.6, 540.0, 610.0),
];

// ---------------------------------------------------------------------------
// Region building
// ---------------------------------------------------------------------------

/// A CSG region as a postfix token stream (the convention `Cell` evaluates).
#[derive(Debug, Clone)]
pub(super) struct Rgn(pub Vec<RegionToken>);

impl Rgn {
    pub(super) fn ins(i: usize) -> Self {
        Self(vec![RegionToken::HalfSpace {
            surface_idx: i,
            sense: HalfSpaceSense::Inside,
        }])
    }
    pub(super) fn out(i: usize) -> Self {
        Self(vec![RegionToken::HalfSpace {
            surface_idx: i,
            sense: HalfSpaceSense::Outside,
        }])
    }
    pub(super) fn and(mut self, o: Self) -> Self {
        self.0.extend(o.0);
        self.0.push(RegionToken::Intersection);
        self
    }
    pub(super) fn or(mut self, o: Self) -> Self {
        self.0.extend(o.0);
        self.0.push(RegionToken::Union);
        self
    }
    pub(super) fn not(mut self) -> Self {
        self.0.push(RegionToken::Complement);
        self
    }
}

/// Surface list with de-duplication of coincident z-planes and z-cylinders.
///
/// Two surfaces at the same place with different indices are a tracking
/// hazard: a particle recorded as ON one is re-judged against the other by a
/// sign that round-off owns. So every z-plane and z-cylinder is looked up
/// before it is created, and an existing one (including an outer-boundary
/// one, whose boundary condition then applies) is reused.
pub(super) fn zplane(surfaces: &mut Vec<SurfaceKind>, z0: f64) -> usize {
    for (i, s) in surfaces.iter().enumerate() {
        if let SurfaceKind::ZPlane(p) = s {
            if (p.z0 - z0).abs() < 1.0e-7 {
                return i;
            }
        }
    }
    surfaces.push(SurfaceKind::ZPlane(ZPlane {
        z0,
        bc: BoundaryType::Transmissive,
    }));
    surfaces.len() - 1
}

/// See [`zplane`].
pub(super) fn zcyl(surfaces: &mut Vec<SurfaceKind>, x0: f64, y0: f64, r: f64) -> usize {
    for (i, s) in surfaces.iter().enumerate() {
        if let SurfaceKind::ZCylinder(c) = s {
            if (c.x0 - x0).abs() < 1.0e-9 && (c.y0 - y0).abs() < 1.0e-9 && (c.r - r).abs() < 1.0e-9
            {
                return i;
            }
        }
    }
    surfaces.push(SurfaceKind::ZCylinder(ZCylinder {
        x0,
        y0,
        r,
        bc: BoundaryType::Transmissive,
    }));
    surfaces.len() - 1
}

/// Where the reflector sits, and what it is made of.
#[derive(Debug, Clone, Copy)]
pub(super) struct ReflectorFrame {
    /// Local z of the model top (`z_T = 0`) \[cm\].
    pub refl_top: f64,
}

impl ReflectorFrame {
    /// Local z of a TECDOC axial coordinate.
    pub(super) fn lz(&self, zt: f64) -> f64 {
        self.refl_top - zt
    }
    /// The slab `z_T` in `[top, bot]`.
    pub(super) fn slab(&self, s: &mut Vec<SurfaceKind>, zt_top: f64, zt_bot: f64) -> Rgn {
        let lo = zplane(s, self.lz(zt_bot));
        let hi = zplane(s, self.lz(zt_top));
        Rgn::out(lo).and(Rgn::ins(hi))
    }
    /// The annulus `r in [r_in, r_out]` about the core axis.
    pub(super) fn annulus(&self, s: &mut Vec<SurfaceKind>, r_in: f64, r_out: f64) -> Rgn {
        let o = Rgn::ins(zcyl(s, 0.0, 0.0, r_out));
        if r_in > 0.0 {
            Rgn::out(zcyl(s, 0.0, 0.0, r_in)).and(o)
        } else {
            o
        }
    }
}

/// Which pieces of the explicit reflector to build. Every flag is `true` in
/// the model the benchmark runs; each `false` is an ABLATION, named by the
/// environment knob `assemble_explicit_triso` reads.
#[derive(Debug, Clone, Copy)]
pub(super) struct ReflectorOptions {
    /// The ten rods at their withdrawn position (`OUTRAM_HTR10_NO_WITHDRAWN_RODS`
    /// leaves the rod channels empty).
    pub withdrawn_rods: bool,
}

/// Material slots the reflector needs, supplied by `core_model` so this module
/// never hard-codes an index into the material set.
#[derive(Debug, Clone, Copy)]
pub(super) struct ReflectorMaterials {
    /// Helium (near void).
    pub helium: usize,
    /// B4C of the rod absorber rings.
    pub b4c: usize,
    /// Rod sleeve stainless steel.
    pub steel: usize,
    /// Iron of the rod joints and ends.
    pub iron: usize,
}

/// The pieces of one channel's cross-section, as regions, and where a zone box
/// that contains it must cut it out.
struct BuiltChannel {
    ch: ReflectorChannel,
    /// Round cross-section (every channel; for KLAK, the round sections).
    round: usize,
    /// KLAK slot: its region, when this is a KLAK channel.
    slot: Option<Rgn>,
}

/// **Build the reflector**: append its surfaces and cells, and return the root
/// cells in search order.
///
/// `zone_material(zone)` maps a Fig. 4.10 zone to a material slot, with the
/// p. 242 corrections already applied (see `core_model::mat::for_zone_mc`).
///
/// The rod universe (when rods are built) is appended to `universes` and every
/// rod channel is a translated fill of it.
///
/// # Panics
///
/// If a channel crosses a zone box's radial boundary, or starts or ends
/// inside a box it overlaps. Either would mean the box's cut-out is wrong;
/// the Fig. 4.10 map was drawn around the borings, and this asserts it.
pub(super) fn build_reflector(
    surfaces: &mut Vec<SurfaceKind>,
    cells: &mut Vec<Cell>,
    universes: &mut Vec<Universe>,
    frame: ReflectorFrame,
    opts: ReflectorOptions,
    mats: ReflectorMaterials,
    zone_material: impl Fn(usize) -> usize,
) -> Vec<usize> {
    let s = surfaces;
    let mut root: Vec<usize> = Vec::new();
    // Reflector cell ids start here, clear of the bed-tile id ranges
    // (`core_model::tile_cell_role`).
    let mut next_id = REFLECTOR_CELL_ID_BASE;
    let push = |cells: &mut Vec<Cell>, c: Cell| -> usize {
        cells.push(c);
        cells.len() - 1
    };
    let mut new_id = || {
        next_id += 1;
        next_id
    };

    // --- channels -------------------------------------------------------
    let channels = reflector_channels();
    let mut built: Vec<BuiltChannel> = Vec::with_capacity(channels.len());
    for ch in &channels {
        let [x, y] = ch.centre_xy();
        let r = match ch.kind {
            ChannelKind::Coolant => COOLANT_RADIUS_CM,
            ChannelKind::ControlRod | ChannelKind::Irradiation => ROD_CHANNEL_RADIUS_CM,
            ChannelKind::AbsorberBall => KLAK_RADIUS_CM,
        };
        let round = zcyl(s, x, y, r);
        let slot = (ch.kind == ChannelKind::AbsorberBall).then(|| {
            // Tangential slot: two R30 end cylinders 100 mm apart along the
            // tangent, joined by a 60 mm wide box (Fig. 4.7).
            let th = ch.azimuth_deg.to_radians();
            let (er, et) = ([th.cos(), th.sin()], [-th.sin(), th.cos()]);
            let half = 0.5 * KLAK_SLOT_STRAIGHT_CM;
            let rc = KLAK_CENTRE_RADIUS_CM;
            let end = |sgn: f64| [x + sgn * half * et[0], y + sgn * half * et[1]];
            let [ax, ay] = end(1.0);
            let [bx_, by] = end(-1.0);
            let ca = zcyl(s, ax, ay, KLAK_RADIUS_CM);
            let cb = zcyl(s, bx_, by, KLAK_RADIUS_CM);
            let mut plane = |n: [f64; 2], d: f64| {
                s.push(SurfaceKind::Plane(Plane {
                    a: n[0],
                    b: n[1],
                    c: 0.0,
                    d,
                    bc: BoundaryType::Transmissive,
                }));
                s.len() - 1
            };
            let r_hi = plane(er, rc + KLAK_RADIUS_CM);
            let r_lo = plane(er, rc - KLAK_RADIUS_CM);
            // e_t . (x, y) of the centre is 0, so the flanks sit at +/- half.
            let t_hi = plane(et, half);
            let t_lo = plane(et, -half);
            let rect = Rgn::ins(r_hi)
                .and(Rgn::out(r_lo))
                .and(Rgn::ins(t_hi))
                .and(Rgn::out(t_lo));
            Rgn::ins(ca).or(Rgn::ins(cb)).or(rect)
        });
        built.push(BuiltChannel {
            ch: *ch,
            round,
            slot,
        });
    }

    // Hot gas duct, horizontal along +x, rho 90 -> 190 (p. 242).
    s.push(SurfaceKind::XCylinder(XCylinder {
        y0: 0.0,
        z0: frame.lz(HOT_GAS_DUCT_AXIS_ZT_CM),
        r: HOT_GAS_DUCT_RADIUS_CM,
        bc: BoundaryType::Transmissive,
    }));
    let duct_cyl = s.len() - 1;
    s.push(SurfaceKind::XPlane(XPlane {
        x0: 0.0,
        bc: BoundaryType::Transmissive,
    }));
    let x_pos = s.len() - 1;
    let duct = || Rgn::ins(duct_cyl).and(Rgn::out(x_pos));
    let duct_zt = (
        HOT_GAS_DUCT_AXIS_ZT_CM - HOT_GAS_DUCT_RADIUS_CM,
        HOT_GAS_DUCT_AXIS_ZT_CM + HOT_GAS_DUCT_RADIUS_CM,
    );

    // --- the rod universe ----------------------------------------------
    // Rod-local frame: the rod axis at the origin, z as the model's (the
    // translation is purely lateral). TECDOC § 4.1.2 / control_rod.rs:
    // radial 27.5 void / 2 ss / 0.5 void / 22.5 B4C / 0.5 void / 2 ss (mm);
    // axial, lower end upward, 45/487/36/487/36/487/36/487/36/487/23 mm;
    // joints and ends are Fe only in 27.5 < R < 55 mm. Withdrawn, the lower
    // end is at z_T = 119.2 cm and the rod runs up out of the model top.
    let rod_universe = if opts.withdrawn_rods {
        // Surfaces pushed fresh, never shared with the root frame: a
        // rod-local cylinder is a different physical surface for every rod.
        let mut cyl = |r: f64| {
            s.push(SurfaceKind::ZCylinder(ZCylinder {
                x0: 0.0,
                y0: 0.0,
                r,
                bc: BoundaryType::Transmissive,
            }));
            s.len() - 1
        };
        let [c275, c295, c300, c525, c530, c550] =
            [2.75, 2.95, 3.00, 5.25, 5.30, 5.50].map(&mut cyl);
        let mut idx = Vec::new();
        let he = mats.helium;
        // Everything outside the outer sleeve (the channel gap and beyond),
        // and the rod bore.
        idx.push(push(cells, Cell::material(new_id(), Rgn::out(c550).0, he, 293.6)));
        idx.push(push(cells, Cell::material(new_id(), Rgn::ins(c275).0, he, 293.6)));
        // Sections from the lower end up: z_T of each section's lower end.
        let mut zt_lo = LOWER_END_WITHDRAWN_CM;
        let wall = Rgn::out(c275).and(Rgn::ins(c550));
        // Below the rod: empty channel.
        let p_end = zplane(s, frame.lz(zt_lo));
        idx.push(push(
            cells,
            Cell::material(new_id(), wall.clone().and(Rgn::ins(p_end)).0, he, 293.6),
        ));
        for (i, (&len, &is_b4c)) in AXIAL_SECTIONS_CM.iter().zip(AXIAL_IS_B4C.iter()).enumerate() {
            let zt_hi = zt_lo - len;
            let lo = zplane(s, frame.lz(zt_lo));
            // The last section inside the model runs on up, unbounded: the
            // channel cell's own top (z_T = 0) closes it.
            let last = zt_hi <= 0.0 || i + 1 == AXIAL_SECTIONS_CM.len();
            let slab = if last {
                Rgn::out(lo)
            } else {
                Rgn::out(lo).and(Rgn::ins(zplane(s, frame.lz(zt_hi))))
            };
            if is_b4c {
                let shells = [
                    (c275, c295, mats.steel),
                    (c295, c300, he),
                    (c300, c525, mats.b4c),
                    (c525, c530, he),
                    (c530, c550, mats.steel),
                ];
                for (a, b, m) in shells {
                    let reg = Rgn::out(a).and(Rgn::ins(b)).and(slab.clone());
                    idx.push(push(cells, Cell::material(new_id(), reg.0, m, 293.6)));
                }
            } else {
                let reg = wall.clone().and(slab);
                idx.push(push(cells, Cell::material(new_id(), reg.0, mats.iron, 293.6)));
            }
            if last {
                break;
            }
            zt_lo = zt_hi;
        }
        universes.push(Universe {
            id: universes.len() as i32,
            cell_indices: idx,
        });
        Some(universes.len() - 1)
    } else {
        None
    };

    // --- channel cells --------------------------------------------------
    for b in &built {
        let [x, y] = b.ch.centre_xy();
        let (zt0, zt1) = b.ch.zt_range();
        match b.ch.kind {
            ChannelKind::AbsorberBall => {
                let slot = b.slot.clone().expect("KLAK has a slot");
                for (a, c, reg) in [
                    (zt0, KLAK_SLOT_ZT_CM.0, Rgn::ins(b.round)),
                    (KLAK_SLOT_ZT_CM.0, KLAK_SLOT_ZT_CM.1, slot),
                    (KLAK_SLOT_ZT_CM.1, zt1, Rgn::ins(b.round)),
                ] {
                    let r = reg.and(frame.slab(s, a, c));
                    root.push(push(cells, Cell::material(new_id(), r.0, mats.helium, 293.6)));
                }
            }
            ChannelKind::ControlRod if rod_universe.is_some() => {
                let r = Rgn::ins(b.round).and(frame.slab(s, zt0, zt1));
                root.push(push(
                    cells,
                    Cell::fill(
                        new_id(),
                        r.0,
                        CellFill::Universe(rod_universe.expect("checked")),
                        Position::new(x, y, 0.0),
                    ),
                ));
            }
            _ => {
                let r = Rgn::ins(b.round).and(frame.slab(s, zt0, zt1));
                root.push(push(cells, Cell::material(new_id(), r.0, mats.helium, 293.6)));
            }
        }
    }
    // The duct.
    {
        let r = duct()
            .and(Rgn::out(zcyl(s, 0.0, 0.0, HOT_GAS_DUCT_RHO_CM.0)))
            .and(Rgn::ins(zcyl(s, 0.0, 0.0, HOT_GAS_DUCT_RHO_CM.1)));
        root.push(push(cells, Cell::material(new_id(), r.0, mats.helium, 293.6)));
    }

    // --- zone boxes, each minus the borings it contains -------------------
    const TOL: f64 = 1.0e-6;
    for zb in FIG_4_10_BOXES {
        let mut reg = frame
            .annulus(s, zb.r_in, zb.r_out)
            .and(frame.slab(s, zb.zt_top, zb.zt_bot));
        let zt_mid = 0.5 * (zb.zt_top + zb.zt_bot);
        for b in &built {
            let (c0, c1) = b.ch.zt_range();
            // Axial overlap with the box, and the KLAK piece in this box.
            if c1 <= zb.zt_top + TOL || c0 >= zb.zt_bot - TOL {
                continue;
            }
            let (rmin, rmax) = b.ch.radial_extent_cm(zt_mid);
            if rmax <= zb.r_in + TOL || rmin >= zb.r_out - TOL {
                continue;
            }
            assert!(
                rmin >= zb.r_in - TOL && rmax <= zb.r_out + TOL,
                "{:?} at {} deg crosses zone {}'s radial boundary",
                b.ch.kind,
                b.ch.azimuth_deg,
                zb.zone
            );
            assert!(
                c0 <= zb.zt_top + TOL && c1 >= zb.zt_bot - TOL,
                "{:?} starts or ends inside zone {}",
                b.ch.kind,
                zb.zone
            );
            if b.ch.kind == ChannelKind::AbsorberBall {
                let in_slot = is_klak_slot_zt(zt_mid);
                // A box must not straddle the slot/round transition.
                assert!(
                    in_slot == is_klak_slot_zt(zb.zt_top + TOL)
                        && in_slot == is_klak_slot_zt(zb.zt_bot - TOL),
                    "zone {} straddles the KLAK slot transition",
                    zb.zone
                );
                if in_slot {
                    reg = reg.and(b.slot.clone().expect("KLAK has a slot").not());
                    continue;
                }
            }
            reg = reg.and(Rgn::out(b.round));
        }
        if zb.r_out > HOT_GAS_DUCT_RHO_CM.0 + TOL
            && zb.zt_top < duct_zt.1 - TOL
            && zb.zt_bot > duct_zt.0 + TOL
        {
            assert!(
                zb.r_in >= HOT_GAS_DUCT_RHO_CM.0 - TOL
                    && zb.zt_top <= duct_zt.0 + TOL
                    && zb.zt_bot >= duct_zt.1 - TOL,
                "zone {} cuts the hot gas duct",
                zb.zone
            );
            reg = reg.and(duct().not());
        }
        let m = zone_material(zb.zone);
        root.push(push(cells, Cell::material(new_id(), reg.0, m, 293.6)));
    }
    root
}
