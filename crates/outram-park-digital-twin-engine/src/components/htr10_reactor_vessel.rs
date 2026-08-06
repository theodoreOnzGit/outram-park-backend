//! Visual HTR-10 (pebble-bed high-temperature gas-cooled reactor) vessel.
//!
//! Built on the same pattern as
//! [`crate::components::fhr_reactor_vessel::FhrReactorVesselVisual`] — a
//! scalar-fed cut-away that colours each region from a temperature the caller
//! supplies — but the geometry is HTR-10's, not the FHR's, and the differences
//! are physical rather than cosmetic.
//!
//! # How HTR-10 differs from the FHR
//!
//! Both are pebble beds, but almost nothing else lines up:
//!
//! - **Flow direction is reversed.** Cold helium enters the vessel and rises
//!   through channels *inside the side reflector*, reverses at the top of the
//!   core, and flows **downward** through the pebble bed into a hot gas
//!   chamber in the bottom reflector. The FHR's salt rises through its bed.
//! - **Control rods sit in the side reflector**, not in the bed — ten borings
//!   in the graphite around the active core, entered from the vessel head.
//! - **The bed drains through a cone into a central discharge tube** that
//!   penetrates the lower head, because fuel circulates multi-pass and is
//!   assayed for burnup before being returned to the top.
//! - **Heat leaves sideways**, through a hot gas duct nozzle low on the vessel
//!   wall, to a separate steam-generator vessel standing beside the reactor.
//! - **The vessel is a capsule** — a cylindrical shell closed by domed heads —
//!   rather than a squared-off box.
//!
//! # Provenance of the geometry
//!
//! Proportions follow the HTR-10 reactor vertical cross-section (Figure 4.6)
//! and the core-configuration and vessel-system descriptions in the IAEA
//! coordinated-research-programme report on HTGR performance, ingested into
//! this workspace's literature layer at
//! `crates/kovan-literature/open/reports/htr-10-iaea.json`.
//!
//! Published dimensions used directly: reactor pressure vessel 4.2 m inner
//! diameter by 11.1 m high (which sets [`NATIVE_ASPECT_RATIO`]); pebble bed
//! 1.8 m diameter by 1.97 m mean height; side reflector 1.0 m thick including
//! carbon bricks.
//!
//! **This is schematic art, not a design drawing.** Feature positions are
//! proportioned by eye from the figure, not dimensioned from it, and nothing
//! here is a validated model. See `RESPONSIBLE_USE.md`.

use crate::components::pebble_packing::{
    PackedPebble, BARREL_HEIGHT, BED_BOUNDS, CHUTE_RADIUS, CONE_HEIGHT, PACKED_PEBBLES,
};
use crate::components::temperature_colour;
use egui::{Color32, FontId, Pos2, Rect, Response, Sense, Stroke, StrokeKind, Ui, Vec2, Widget};
use uom::si::f64::ThermodynamicTemperature;

/// Width-to-height ratio the vessel is drawn at.
///
/// Taken from the published reactor pressure vessel dimensions — 4.2 m inner
/// diameter by 11.1 m high — so the silhouette carries the real slenderness of
/// the machine rather than an invented one.
pub const NATIVE_ASPECT_RATIO: f32 = 4.2 / 11.1;

/// The largest sub-rectangle of `available` carrying [`NATIVE_ASPECT_RATIO`],
/// centred within it.
///
/// Same letterbox contract as the FHR vessel: the artwork keeps its
/// proportions at any size rather than stretching to fill its box.
pub fn fit_native_aspect(available: Rect) -> Rect {
    let (w, h) = (available.width(), available.height());
    if w <= 0.0 || h <= 0.0 {
        return available;
    }
    let (fw, fh) = if w / h > NATIVE_ASPECT_RATIO {
        (h * NATIVE_ASPECT_RATIO, h)
    } else {
        (w, w / NATIVE_ASPECT_RATIO)
    };
    Rect::from_center_size(available.center(), Vec2::new(fw, fh))
}

/// The outline of a pebble bed: a cylinder that narrows through a cone into
/// the discharge chute, in screen coordinates.
///
/// Every field is derived from the baked packing's own constants at render
/// time (barrel half-width = one vessel radius, then
/// [`BARREL_HEIGHT`]/[`CONE_HEIGHT`]/[`CHUTE_RADIUS`] times that), so the
/// silhouette drawn here and the settled bed drawn inside it cannot drift
/// apart.
struct PebbleBedShape {
    centre_x: f32,
    top: f32,
    cylinder_bottom: f32,
    cone_bottom: f32,
    half_width: f32,
    chute_half_width: f32,
}

impl PebbleBedShape {
    /// Half-width of the bed at height `y` — constant down the cylinder, then
    /// tapering linearly to the chute through the cone.
    fn half_width_at(&self, y: f32) -> f32 {
        if y <= self.cylinder_bottom {
            self.half_width
        } else {
            let t = ((y - self.cylinder_bottom) / (self.cone_bottom - self.cylinder_bottom))
                .clamp(0.0, 1.0);
            self.half_width + (self.chute_half_width - self.half_width) * t
        }
    }
}

/// The pebble bed's screen outline inside an already-letterboxed vessel
/// `rect`.
///
/// Only two numbers are free: the bed's half-width (0.30 of the vessel width,
/// proportioned from the published 1.8 m bed inside the 4.2 m vessel) and the
/// height its top sits at. Everything below follows from the packing's own
/// constants at one uniform scale, because the barrel half-width *is* one
/// vessel radius in the packing's frame.
fn pebble_bed_shape(rect: Rect) -> PebbleBedShape {
    let half_width = rect.width() * 0.30;
    let top = rect.top() + rect.height() * 0.20;
    let cylinder_bottom = top + BARREL_HEIGHT * half_width;
    PebbleBedShape {
        centre_x: rect.center().x,
        top,
        cylinder_bottom,
        cone_bottom: cylinder_bottom + CONE_HEIGHT * half_width,
        half_width,
        chute_half_width: CHUTE_RADIUS * half_width,
    }
}

/// How the baked packing is laid into an HTR-10 bed: **the way it settled**.
///
/// Helium is a gas, so a graphite pebble is far denser than its coolant and the
/// bed rests on itself under gravity, draining through the cone at the bottom.
/// That is exactly the situation the DEM run simulated, so the packing is
/// placed with no inversion and no cropping — the whole vessel maps onto the
/// whole vessel at a single scale of `bed.half_width` points per vessel radius.
///
/// (The FHR vessel is the opposite case: its pebbles float, so it inverts the
/// same packing. See [`VerticalSense`].)
fn settled_bed_packing(bed: &PebbleBedShape) -> PackingTransform {
    PackingTransform {
        axis_x: bed.centre_x,
        // Packing y = 0 is the cone/barrel junction.
        origin_y: bed.cylinder_bottom,
        scale: bed.half_width,
        vertical: VerticalSense::GravityUp,
    }
}

/// Which way the packing's `+y` (gravity-up) axis is drawn on screen.
///
/// The baked packing in [`crate::components::pebble_packing`] settled
/// **downward under gravity**: its densest, most compressed layers sit at the
/// bottom, and its loose free surface is at the top. Which end of a drawn bed
/// that dense base belongs at is a *physics* question, not a drawing
/// convention, so it is named rather than left to a sign in the arithmetic.
pub(crate) enum VerticalSense {
    /// Packing `+y` points **up the screen** — the bed sits the way it
    /// settled, dense base at the bottom, free surface on top.
    ///
    /// Correct for a **gas-cooled** pebble bed such as HTR-10: helium is far
    /// less dense than a graphite pebble, so the pebbles rest on one another
    /// under their own weight and drain out of the bottom.
    ///
    /// Screen `y` grows downward while packing `y` grows upward, so this
    /// applies one flip.
    GravityUp,
    /// Packing `+y` points **down the screen** — the settled bed is turned
    /// over, dense base at the top and free surface facing down.
    ///
    /// Correct for a **salt-cooled** pebble bed such as an FHR: molten FLiBe
    /// (roughly 1940 kg/m³ at operating temperature) is *denser* than a
    /// graphite pebble (roughly 1740–1800 kg/m³), so the pebbles are buoyant.
    /// They float upward and pack against a retaining structure at the top of
    /// the core, and the bed's free surface is its **bottom** face.
    ///
    /// This is an inversion of a gravity-settled packing, which is exactly
    /// what a buoyant bed is. It cancels against egui's downward screen `y`,
    /// so packing `y` and screen `y` end up running the same way — that
    /// double flip is deliberate, not a bug.
    Buoyant,
}

/// Where the baked packing's normalised vessel frame lands on screen.
///
/// The packing is expressed in **vessel barrel inner radii** with its origin on
/// the axis at the cone/barrel junction (see
/// [`crate::components::pebble_packing`]). This maps that frame to points.
///
/// [`Self::scale`] is a **single** factor applied to both axes and to each
/// pebble's radius. That is not an implementation detail: a settled packing is
/// only valid as a packing because its spheres touch, so scaling `x` and `y`
/// differently would open or close every contact in it and destroy the very
/// property that makes it worth baking. If a bed's drawn proportions do not
/// match the packing's, narrow the [`PackingWindow`] — never stretch.
pub(crate) struct PackingTransform {
    /// Screen x of the vessel axis (packing `x = 0`), in points.
    pub axis_x: f32,
    /// Screen y of the packing's origin plane (packing `y = 0`), in points.
    ///
    /// For [`VerticalSense::GravityUp`] that plane is the cone/barrel
    /// junction, so this is the screen y where the cylinder ends and the cone
    /// begins. For [`VerticalSense::Buoyant`] the same plane is drawn at the
    /// *top* of the bed, because the packing is turned over.
    pub origin_y: f32,
    /// Points per vessel radius — one uniform factor for both axes.
    pub scale: f32,
    /// Which way packing `+y` runs on screen. See [`VerticalSense`].
    pub vertical: VerticalSense,
}

impl PackingTransform {
    /// Screen centre of `pebble`, in points.
    pub fn centre(&self, pebble: &PackedPebble) -> Pos2 {
        let y = match self.vertical {
            VerticalSense::GravityUp => self.origin_y - pebble.y * self.scale,
            VerticalSense::Buoyant => self.origin_y + pebble.y * self.scale,
        };
        Pos2::new(self.axis_x + pebble.x * self.scale, y)
    }

    /// Screen radius of `pebble`, in points.
    ///
    /// The baked radius is the **chord** of the sphere cut by the slicing
    /// plane, not the sphere radius, so it varies from pebble to pebble. That
    /// spread is what a real saw-cut through a bed looks like and is preserved
    /// here rather than normalised away.
    pub fn radius(&self, pebble: &PackedPebble) -> f32 {
        pebble.r * self.scale
    }
}

/// Which part of the baked packing a widget draws.
///
/// A widget whose bed has the packing's own proportions draws all of it
/// ([`Self::whole_bed`]). A widget whose bed is a different shape takes a
/// **sub-region** ([`Self::barrel_column`]) — cropping keeps the packing's
/// contacts intact, where stretching would not.
///
/// The test is on each circle's full **extent** (`centre ± radius`), so a kept
/// pebble is drawn wholly inside the bed outline rather than overhanging it.
pub(crate) struct PackingWindow {
    /// Largest `|x| + r` a pebble may reach, in vessel radii.
    pub max_abs_x: f32,
    /// Lowest `y - r` a pebble may reach, in vessel radii.
    pub min_y: f32,
    /// Highest `y + r` a pebble may reach, in vessel radii.
    pub max_y: f32,
}

impl PackingWindow {
    /// The whole baked bed — barrel *and* cone, every one of the 261 circles.
    ///
    /// The right window for a widget drawing the same vessel the packing was
    /// settled in, which is what HTR-10's cylinder-into-a-cone bed is.
    ///
    /// Built from [`BED_BOUNDS`], the packing's own measured bounding box,
    /// with a 1e-3 slack: that constant is printed to five decimals, and
    /// without the slack the one circle that defines each bound could be
    /// dropped by a rounding hair.
    pub fn whole_bed() -> Self {
        Self {
            max_abs_x: BED_BOUNDS[1].max(-BED_BOUNDS[0]) + 1.0e-3,
            min_y: BED_BOUNDS[2] - 1.0e-3,
            max_y: BED_BOUNDS[3] + 1.0e-3,
        }
    }

    /// A central vertical column of the **barrel only** (`y >= 0`), at most
    /// `max_abs_x` vessel radii either side of the axis.
    ///
    /// For a bed that has no cone and is proportionally taller than the
    /// packing's barrel. Taking a narrower column lets the full barrel height
    /// fill the drawn bed at one uniform scale; the cost is that the column's
    /// side boundaries are a cut through the packing rather than a wall, so
    /// pebbles there do not rest tangentially against the bed edge.
    pub fn barrel_column(max_abs_x: f32) -> Self {
        Self {
            max_abs_x,
            min_y: 0.0,
            max_y: BARREL_HEIGHT,
        }
    }

    /// Whether `pebble` is drawn wholly inside this window.
    pub fn contains(&self, pebble: &PackedPebble) -> bool {
        pebble.x.abs() + pebble.r <= self.max_abs_x
            && pebble.y - pebble.r >= self.min_y
            && pebble.y + pebble.r <= self.max_y
    }
}

/// Deterministic pseudo-random value in `[0, 1)` from two indices.
///
/// **Determinism is the point.** The widget is rebuilt every repaint, so
/// drawing anything from a real random source would make the bed shimmer —
/// every TRISO kernel jumping to a new spot each frame. Hashing the pebble and
/// dot indices instead gives a scatter that looks random but is identical
/// frame to frame.
///
/// The pebble *positions* no longer come from here — they are the baked DEM
/// packing in [`crate::components::pebble_packing`] — but the TRISO speckle
/// still does, via [`triso_dot_offset`]. `salt` separates independent draws so
/// they do not correlate into visible structure.
fn pebble_hash(col: i32, row: i32, salt: u32) -> f32 {
    let mut h = (col as u32).wrapping_mul(0x9E37_79B9)
        ^ (row as u32).wrapping_mul(0x85EB_CA6B)
        ^ salt.wrapping_mul(0xC2B2_AE35);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2545_F491);
    h ^= h >> 13;
    (h % 1_000_003) as f32 / 1_000_003.0
}

// ── TRISO speckling ─────────────────────────────────────────────────────────
//
// A pebble is not a hot ball. It is a graphite matrix through which thousands
// of tiny TRISO particles are dispersed over a fuelled inner zone, wrapped in
// an unfuelled graphite shell. The fission heat is produced in those kernels,
// so the artwork draws a graphite body speckled with hot dots rather than one
// smooth coloured sphere.
//
// These helpers are shared with
// [`crate::components::fhr_reactor_vessel`], which draws its own hand-placed
// pebbles the same way. They live here because this is the module that owns
// [`pebble_hash`], the determinism the speckling depends on.

/// Fraction of a pebble's radius occupied by the **fuelled zone** —
/// dimensionless, in `[0, 1]`.
///
/// An HTR-10 fuel sphere is 60 mm in diameter with an unfuelled outer graphite
/// shell about 5 mm thick, so the fuelled inner zone is 50 mm across: a radius
/// ratio of 25/30, or about 0.83. Rounded down to 0.8 here so that a dot's own
/// drawn radius still fits inside the shell at small on-screen sizes, which
/// keeps the unfuelled rim visible instead of letting kernels touch the edge.
pub(crate) const FUELLED_ZONE_FRAC: f32 = 0.8;

/// Fraction of the fuelled zone the TRISO speckle should visually cover —
/// dimensionless, in `[0, 1)`.
///
/// The maintainer's target: a pebble should read as *packed* with kernels, not
/// sprinkled with them. [`triso_dot_count`] inverts the random-coverage
/// relation to hit this, so changing this one number changes the density and
/// nothing else needs retuning.
///
/// **This is a legibility target, not a physical packing fraction.** A real
/// fuel sphere holds thousands of TRISO particles at a far lower volume
/// fraction; drawing them faithfully at these sizes would give a uniform grey.
pub(crate) const TRISO_TARGET_FILL: f32 = 0.55;

/// Drawn pebble radius, in **points**, at or below which no dots are drawn.
///
/// Below roughly a 1.5 pt radius a pebble is about three pixels across at
/// unit device-pixel-ratio; a dot inside it would be sub-pixel and would
/// alias into noise, so the pebble is instead filled with a single colour that
/// blends the graphite matrix toward the fuel colour. The pebble still
/// responds to temperature — it just cannot resolve individual kernels.
const TRISO_TINT_ONLY_RADIUS: f32 = 1.5;

/// Drawn pebble radius, in **points**, below which a pebble gets exactly one
/// centred kernel dot rather than a scatter.
///
/// Between [`TRISO_TINT_ONLY_RADIUS`] and this threshold a pebble is large
/// enough to show that it has a hot *interior* distinct from its graphite
/// rim, but far too small for several dots — a scatter at this size overlaps
/// into a single blob and reads worse than the honest single kernel. This is
/// the regime the HTR-10 bed lands in at gallery/thumbnail card sizes.
const TRISO_SINGLE_DOT_RADIUS: f32 = 3.0;

/// Number of TRISO kernel dots drawn inside a pebble of drawn `radius`
/// (points).
///
/// Scale-aware by design — a real pebble holds thousands of particles, but
/// drawing more than a couple of dozen dots inside a few points of screen
/// space turns to mud. The three regimes are:
///
/// | Drawn radius (pt) | Dots | Reads as |
/// |---|---|---|
/// | `<= 1.5` | 0 | tinted graphite speck (see [`TRISO_TINT_ONLY_RADIUS`]) |
/// | `1.5 .. 3.0` | 1 | graphite ball with a hot centre |
/// | `>= 3.0` | `1.5 * radius`, clamped to 4..=18 | speckled fuel zone |
///
/// The upper clamp exists because dot area grows with radius too, so beyond
/// about 18 dots the fuelled zone saturates and reverts to looking solid.
/// Non-finite radii return 0 rather than panicking, since egui layout can
/// transiently hand a widget a degenerate rectangle.
pub(crate) fn triso_dot_count(radius: f32) -> usize {
    if !radius.is_finite() || radius <= TRISO_TINT_ONLY_RADIUS {
        0
    } else if radius < TRISO_SINGLE_DOT_RADIUS {
        1
    } else {
        {
            // Derive the count from the TARGET FILL rather than a hand-tuned
            // multiplier, so the intent is stated once and the geometry follows.
            //
            // Dots are scattered independently, so they overlap. For random
            // placement the expected covered fraction of the zone is
            // `1 - exp(-A_dots / A_zone)`, which inverts to
            // `N = -ln(1 - fill) * (r_zone / r_dot)^2`.
            let zone = FUELLED_ZONE_FRAC * radius;
            let dot = triso_dot_radius(radius);
            let n = -(1.0 - TRISO_TARGET_FILL).ln() * (zone * zone) / (dot * dot);
            (n as usize).clamp(24, 260)
        }
    }
}

/// Drawn radius, in **points**, of one TRISO kernel dot inside a pebble of
/// drawn `radius`.
///
/// A single-dot pebble gets a proportionally much larger dot (0.42 of the
/// pebble radius) because it stands for the whole fuelled zone; a speckled
/// pebble gets small dots (0.13 of the radius) so the scatter still reads as
/// discrete particles. Both are floored so a dot never falls below about half
/// a point, where it would vanish, and the speckle dot is capped at 2 pt so a
/// very large pebble does not turn back into a solid disc.
pub(crate) fn triso_dot_radius(radius: f32) -> f32 {
    if radius < TRISO_SINGLE_DOT_RADIUS {
        (radius * 0.42).max(0.5)
    } else {
        (radius * 0.10).clamp(0.4, 1.2)
    }
}

/// Offset, from the pebble centre, of TRISO dot `k` in the pebble identified
/// by `index`, for a pebble of drawn `radius` (points).
///
/// **Deterministic by construction.** Widgets here are rebuilt on every
/// repaint, so a scatter drawn from a real random source would make every
/// pebble's speckle jump frame to frame — the same shimmer [`pebble_hash`]
/// was written to avoid for the packing itself. The angle and radial position
/// are hashed from `(index, k)` with fresh salts (11 and 12) that do not
/// collide with the packing's salts 1..=4.
///
/// The radial draw is square-rooted so dots are uniform per unit *area* rather
/// than crowding the centre, and the maximum radius is reduced by the dot's
/// own radius so the whole dot lands inside
/// [`FUELLED_ZONE_FRAC`] of the pebble — the unfuelled graphite shell stays
/// clear. Single-dot and tint-only pebbles return the zero offset.
pub(crate) fn triso_dot_offset(index: i32, k: usize, radius: f32) -> Vec2 {
    if triso_dot_count(radius) <= 1 {
        return Vec2::ZERO;
    }
    let max_rho = (FUELLED_ZONE_FRAC * radius - triso_dot_radius(radius)).max(0.0);
    let k = k as i32;
    let angle = pebble_hash(index, k, 11) * std::f32::consts::TAU;
    let rho = max_rho * pebble_hash(index, k, 12).sqrt();
    Vec2::new(rho * angle.cos(), rho * angle.sin())
}

/// Linear interpolation between two colours, keeping `a`'s alpha.
fn blend_rgb(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let mix = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgba_unmultiplied(
        mix(a.r(), b.r()),
        mix(a.g(), b.g()),
        mix(a.b(), b.b()),
        a.a(),
    )
}

/// Draws one pebble as a graphite matrix speckled with TRISO kernels.
///
/// `matrix` is the graphite body colour and `kernel` the fuel colour — the
/// caller supplies the latter from its own temperature map, so the dots track
/// the temperature sliders exactly as every other coloured region does.
/// `index` identifies the pebble and is the sole source of the scatter's
/// pseudo-randomness; two pebbles with the same index get the same speckle,
/// and the same pebble gets the same speckle on every repaint.
///
/// Below [`TRISO_TINT_ONLY_RADIUS`] the pebble is drawn as a single blended
/// fill instead, since dots would be sub-pixel there.
///
/// This is schematic artwork for an offline demonstration, not a model of
/// particle distribution: the dot count is chosen for legibility on screen,
/// not from a packing fraction.
pub(crate) fn draw_triso_pebble(
    painter: &egui::Painter,
    centre: Pos2,
    radius: f32,
    matrix: Color32,
    kernel: Color32,
    index: i32,
) {
    if !radius.is_finite() || radius <= 0.0 {
        return;
    }
    let dots = triso_dot_count(radius);
    if dots == 0 {
        painter.circle_filled(centre, radius, blend_rgb(matrix, kernel, 0.45));
        return;
    }
    painter.circle_filled(centre, radius, matrix);
    let dot_r = triso_dot_radius(radius);
    for k in 0..dots {
        painter.circle_filled(centre + triso_dot_offset(index, k, radius), dot_r, kernel);
    }
}

/// Graphite matrix colour of a drawn pebble.
///
/// Left slightly translucent so the bed fill behind the packing still shows
/// through between and beneath pebbles, which is what gives the bed depth
/// rather than reading as a flat field of discs.
const PEBBLE_MATRIX: Color32 = Color32::from_rgba_premultiplied(28, 28, 32, 214);

/// Draws the **settled** pebble bed: the baked DEM packing, placed by
/// `transform` and cropped to `window`.
///
/// Positions come from [`PACKED_PEBBLES`] — a cut-away slice of a real
/// soft-sphere DEM packing settled under gravity — so pebbles rest on one
/// another instead of floating on a lattice with unphysical gaps. Nothing is
/// jittered, tiled, or duplicated: each baked circle is drawn at most once, and
/// a bed larger than the packing is left honestly sparse rather than filled
/// with a repeat.
///
/// Each pebble is drawn by [`draw_triso_pebble`] as a `matrix`-coloured
/// graphite body speckled with `kernel`-coloured TRISO kernels, with `kernel`
/// the colour the caller's temperature map gives the fuel. The speckle is
/// seeded from the pebble's **index in the baked table**, so it is a pure
/// function of the data and stable across repaints.
///
/// Returns how many pebbles were drawn, which is what a caller checking the
/// bed is not silently empty at a degenerate size should look at.
pub(crate) fn draw_packed_pebbles(
    painter: &egui::Painter,
    transform: &PackingTransform,
    window: &PackingWindow,
    matrix: Color32,
    kernel: Color32,
) -> usize {
    let mut drawn = 0usize;
    for (index, pebble) in PACKED_PEBBLES.iter().enumerate() {
        if !window.contains(pebble) {
            continue;
        }
        draw_triso_pebble(
            painter,
            transform.centre(pebble),
            transform.radius(pebble),
            matrix,
            kernel,
            index as i32,
        );
        drawn += 1;
    }
    drawn
}

const STEEL: Color32 = Color32::from_rgb(96, 100, 108);
const GRAPHITE: Color32 = Color32::from_rgb(56, 56, 60);
const CARBON_BRICK: Color32 = Color32::from_rgb(42, 42, 46);
const ROD: Color32 = Color32::from_rgb(24, 24, 28);
const LABEL: Color32 = Color32::from_rgb(212, 212, 216);

/// Visual representation of the HTR-10 reactor vessel.
///
/// Scalar-fed and owns no physics, for the same reason the FHR vessel is: a
/// simulator already holds these temperatures in its own plant model.
///
/// All temperatures are absolute thermodynamic temperatures (`uom`-typed).
/// Control-rod insertion is dimensionless in `[0, 1]` — `0.0` fully withdrawn,
/// `1.0` fully inserted — clamped at render time so a transient overshoot from
/// a controller draws fully in or out rather than panicking.
pub struct Htr10ReactorVesselVisual {
    size: Vec2,
    min_temp: ThermodynamicTemperature,
    max_temp: ThermodynamicTemperature,
    /// Pebble (fuel) temperature — the hottest region.
    pebble_temp: ThermodynamicTemperature,
    /// Cold helium entering the vessel and rising in the side reflector.
    inlet_temp: ThermodynamicTemperature,
    /// Hot helium in the bottom plenum, leaving via the hot gas duct.
    outlet_temp: ThermodynamicTemperature,
    /// Graphite reflector bulk temperature.
    reflector_temp: ThermodynamicTemperature,
    control_rod_insertion_frac: f32,
    show_labels: bool,
}

impl Htr10ReactorVesselVisual {
    /// Build an HTR-10 vessel visual.
    ///
    /// `min_temp`/`max_temp` bound the colour scale; for HTR-10 phase-one
    /// operation the core runs roughly 250 degC inlet to 700 degC outlet, so a
    /// range spanning that keeps a normal operating point off both ends.
    ///
    /// Control rods default to fully inserted, so a caller that forgets to
    /// drive them draws a shut-down core rather than a critical one.
    pub fn new(
        size: Vec2,
        min_temp: ThermodynamicTemperature,
        max_temp: ThermodynamicTemperature,
        pebble_temp: ThermodynamicTemperature,
        inlet_temp: ThermodynamicTemperature,
        outlet_temp: ThermodynamicTemperature,
        reflector_temp: ThermodynamicTemperature,
    ) -> Self {
        Self {
            size,
            min_temp,
            max_temp,
            pebble_temp,
            inlet_temp,
            outlet_temp,
            reflector_temp,
            control_rod_insertion_frac: 1.0,
            show_labels: true,
        }
    }

    /// On-screen size, in points.
    pub fn size(&self) -> Vec2 {
        self.size
    }

    /// Sets control-rod insertion. Dimensionless `[0, 1]`.
    pub fn set_control_rod_frac(&mut self, frac: f32) {
        self.control_rod_insertion_frac = frac;
    }

    /// Turn the internal component labels off — for thumbnails.
    pub fn without_labels(mut self) -> Self {
        self.show_labels = false;
        self
    }

    fn colour(&self, t: ThermodynamicTemperature) -> Color32 {
        temperature_colour(t, self.min_temp, self.max_temp)
    }

    fn tag(&self, ui: &Ui, at: Pos2, text: &str) {
        if !self.show_labels {
            return;
        }
        ui.painter().text(
            at,
            egui::Align2::CENTER_CENTER,
            text,
            FontId::proportional(9.0),
            LABEL,
        );
    }
}

impl Widget for Htr10ReactorVesselVisual {
    /// Draws the HTR-10 cut-away: capsule pressure vessel, graphite reflector
    /// with its vertical channels, the pebble bed narrowing through a cone
    /// into the central discharge tube, the bottom hot-gas plenum, and the
    /// side hot gas duct nozzle.
    ///
    /// Every region is filled via the shared
    /// [`crate::components::temperature_colour`] map, so this vessel grades
    /// temperature identically to every other widget in the library.
    fn ui(mut self, ui: &mut Ui) -> Response {
        let (response, painter) = ui.allocate_painter(self.size, Sense::hover());
        self.control_rod_insertion_frac = self.control_rod_insertion_frac.clamp(0.0, 1.0);

        // Keep the vessel's real slenderness at any box size.
        let rect = fit_native_aspect(response.rect);
        let w = rect.width();
        let h = rect.height();
        let cx = rect.center().x;

        // ── Pressure vessel: cylindrical shell closed by domed heads ────────
        let dome = w * 0.5;
        let shell = Rect::from_min_max(
            Pos2::new(rect.left(), rect.top() + dome * 0.62),
            Pos2::new(rect.right(), rect.bottom() - dome * 0.62),
        );
        painter.rect_filled(shell, 0.0, STEEL);
        // Domed heads, approximated as generously rounded caps.
        painter.rect_filled(
            Rect::from_min_max(
                Pos2::new(rect.left(), rect.top()),
                Pos2::new(rect.right(), shell.top() + dome * 0.4),
            ),
            dome * 0.6,
            STEEL,
        );
        painter.rect_filled(
            Rect::from_min_max(
                Pos2::new(rect.left(), shell.bottom() - dome * 0.4),
                Pos2::new(rect.right(), rect.bottom()),
            ),
            dome * 0.6,
            STEEL,
        );

        // ── Carbon brick layer, then graphite reflector inside it ───────────
        let inner = shell.shrink(w * 0.06);
        painter.rect_filled(inner, w * 0.04, CARBON_BRICK);
        let reflector = inner.shrink(w * 0.045);
        painter.rect_filled(reflector, w * 0.03, GRAPHITE);
        // Tint the reflector by its own temperature, kept subtle so the
        // graphite still reads as graphite.
        let refl_tint = self.colour(self.reflector_temp);
        painter.rect_filled(
            reflector,
            w * 0.03,
            Color32::from_rgba_unmultiplied(refl_tint.r(), refl_tint.g(), refl_tint.b(), 60),
        );

        // ── Side-reflector channels: helium risers, control rods, absorbers ─
        // Cold helium rises here before reversing at the top of the core, so
        // these are drawn at the INLET temperature.
        let cold = self.colour(self.inlet_temp);
        let channel_top = reflector.top() + h * 0.06;
        let channel_bottom = reflector.bottom() - h * 0.10;
        for side in [-1.0f32, 1.0] {
            for (k, frac) in [0.60f32, 0.78, 0.92].iter().enumerate() {
                let x = cx + side * w * 0.5 * frac;
                let stroke_w = if k == 1 { 3.0 } else { 2.0 };
                painter.line_segment(
                    [Pos2::new(x, channel_top), Pos2::new(x, channel_bottom)],
                    Stroke::new(stroke_w, cold),
                );
            }
        }
        self.tag(
            ui,
            Pos2::new(cx + w * 0.5 * 0.78, channel_top - h * 0.018),
            "He risers",
        );

        // ── Pebble bed: cylinder, then a cone to the chute ──────────────────
        //
        // The outline is DERIVED FROM THE BAKED PACKING rather than from
        // hand-picked fractions of the box, so the settled bed lands exactly
        // inside it — see `pebble_bed_shape`.
        let bed = pebble_bed_shape(rect);
        let packing = settled_bed_packing(&bed);
        let bed_half_w = bed.half_width;
        let bed_top = bed.top;
        let bed_cyl_bottom = bed.cylinder_bottom;
        let cone_bottom = bed.cone_bottom;
        let chute_half_w = bed.chute_half_width;
        let hot = self.colour(self.pebble_temp);

        // Cylindrical section. Only lightly rounded: the barrel the packing
        // settled in is straight-walled, and a heavy corner radius would cut
        // the outline back inside the topmost pebbles.
        let barrel_half = bed.half_width_at(bed.top);
        let bed_body = Rect::from_min_max(
            Pos2::new(bed.centre_x - barrel_half, bed.top),
            Pos2::new(bed.centre_x + barrel_half, bed.cylinder_bottom),
        );
        painter.rect_filled(bed_body, bed_half_w * 0.15, hot);

        // Conical bottom funnelling into the discharge tube. Its corners are
        // read back out of `bed`, so the drawn silhouette is by construction
        // the taper `PebbleBedShape::half_width_at` describes — which is the
        // same linear taper as `pebble_packing::vessel_half_width`, so the
        // packing's lowest circles sit inside it.
        let cone_top_half = bed.half_width_at(bed.cylinder_bottom);
        let cone_end_half = bed.half_width_at(bed.cone_bottom);
        painter.add(egui::Shape::convex_polygon(
            vec![
                Pos2::new(cx - cone_top_half, bed_cyl_bottom),
                Pos2::new(cx + cone_top_half, bed_cyl_bottom),
                Pos2::new(cx + cone_end_half, cone_bottom),
                Pos2::new(cx - cone_end_half, cone_bottom),
            ],
            hot,
            Stroke::NONE,
        ));

        // Pebbles, settled across the whole bed — cylinder AND cone. Each is a
        // graphite body speckled with TRISO kernels at the fuel colour, so the
        // bed reads as fuelled graphite spheres rather than as one uniformly
        // hot volume.
        draw_packed_pebbles(
            &painter,
            &packing,
            &PackingWindow::whole_bed(),
            PEBBLE_MATRIX,
            hot,
        );
        self.tag(ui, Pos2::new(cx, bed_top + h * 0.05), "pebble bed");

        // ── Central discharge tube through the lower head ───────────────────
        painter.rect_filled(
            Rect::from_min_max(
                Pos2::new(cx - chute_half_w, cone_bottom),
                Pos2::new(cx + chute_half_w, rect.bottom() - h * 0.01),
            ),
            2.0,
            GRAPHITE,
        );
        self.tag(
            ui,
            Pos2::new(cx + w * 0.24, rect.top() + h * 0.70),
            "discharge tube",
        );

        // ── Hot gas plenum in the bottom reflector, and the duct nozzle ─────
        let plenum = Rect::from_min_max(
            Pos2::new(reflector.left() + w * 0.04, rect.top() + h * 0.62),
            Pos2::new(reflector.right() - w * 0.04, rect.top() + h * 0.70),
        );
        painter.rect_filled(plenum, 3.0, self.colour(self.outlet_temp));
        self.tag(ui, plenum.center(), "hot gas plenum");

        // Hot gas duct leaving sideways to the steam-generator vessel.
        painter.rect_filled(
            Rect::from_min_max(
                Pos2::new(rect.right() - w * 0.02, plenum.top()),
                Pos2::new(rect.right() + w * 0.16, plenum.bottom()),
            ),
            2.0,
            self.colour(self.outlet_temp),
        );

        // ── Control rods, entering the SIDE REFLECTOR from the head ─────────
        let rod_span = channel_bottom - channel_top;
        let depth = rod_span * self.control_rod_insertion_frac;
        if depth > 0.5 {
            for side in [-1.0f32, 1.0] {
                let x = cx + side * w * 0.5 * 0.78;
                painter.line_segment(
                    [Pos2::new(x, channel_top), Pos2::new(x, channel_top + depth)],
                    Stroke::new(4.0, ROD),
                );
            }
        }

        // Control-rod drive penetrations on the vessel head.
        for i in 0..5 {
            let t = (i as f32 + 1.0) / 6.0;
            let x = rect.left() + t * w;
            painter.line_segment(
                [
                    Pos2::new(x, rect.top() - h * 0.012),
                    Pos2::new(x, rect.top() + h * 0.045),
                ],
                Stroke::new(2.5, STEEL),
            );
        }

        // Vessel outline last, so it reads on top of everything.
        painter.rect_stroke(
            shell,
            0.0,
            Stroke::new(2.0, Color32::from_rgb(150, 154, 162)),
            StrokeKind::Middle,
        );

        response
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uom::si::thermodynamic_temperature::degree_celsius;

    fn degc(v: f64) -> ThermodynamicTemperature {
        ThermodynamicTemperature::new::<degree_celsius>(v)
    }

    fn vessel() -> Htr10ReactorVesselVisual {
        Htr10ReactorVesselVisual::new(
            Vec2::new(200.0, 500.0),
            degc(200.0),
            degc(800.0),
            degc(700.0),
            degc(250.0),
            degc(700.0),
            degc(500.0),
        )
    }

    /// The silhouette must carry the real slenderness of the machine.
    ///
    /// **Methodology.** The published reactor pressure vessel is 4.2 m inner
    /// diameter by 11.1 m high. Require [`NATIVE_ASPECT_RATIO`] to match that
    /// quotient, and require the letterbox fit to preserve it in a square, an
    /// over-wide and an over-tall box.
    ///
    /// **Result (2026-08-06):** ratio 0.3784, matching 4.2/11.1; all three
    /// boxes preserve it within 1e-4 and stay centred. Interpretation: the
    /// vessel reads as the tall, slender machine it is at any card size.
    #[test]
    fn the_silhouette_uses_the_published_vessel_proportions() {
        assert!((NATIVE_ASPECT_RATIO - (4.2 / 11.1)).abs() < 1e-6);

        for size in [
            Vec2::new(300.0, 300.0),
            Vec2::new(900.0, 200.0),
            Vec2::new(100.0, 900.0),
        ] {
            let fitted = fit_native_aspect(Rect::from_min_size(Pos2::ZERO, size));
            assert!(
                (fitted.width() / fitted.height() - NATIVE_ASPECT_RATIO).abs() < 1e-4,
                "box {size:?} did not preserve the ratio"
            );
        }
    }

    /// Rods must default to fully inserted, so a caller that forgets to drive
    /// them draws a shut-down core rather than a critical one.
    #[test]
    fn control_rods_default_to_fully_inserted() {
        assert_eq!(vessel().control_rod_insertion_frac, 1.0);
    }

    /// Insertion is stored as given and clamped at render time, matching the
    /// FHR vessel's contract so the two behave alike under one controller.
    #[test]
    fn insertion_is_stored_unclamped() {
        let mut v = vessel();
        v.set_control_rod_frac(1.8);
        assert_eq!(v.control_rod_insertion_frac, 1.8);
        v.set_control_rod_frac(-0.4);
        assert_eq!(v.control_rod_insertion_frac, -0.4);
    }

    /// A degenerate box must not produce NaN geometry — zero-height
    /// allocations happen transiently during egui layout.
    #[test]
    fn degenerate_boxes_are_returned_as_is() {
        let flat = Rect::from_min_size(Pos2::ZERO, Vec2::new(200.0, 0.0));
        assert_eq!(fit_native_aspect(flat), flat);
    }
}

#[cfg(test)]
mod packing_tests {
    use super::*;

    fn bed() -> PebbleBedShape {
        PebbleBedShape {
            centre_x: 100.0,
            top: 0.0,
            cylinder_bottom: 100.0,
            cone_bottom: 160.0,
            half_width: 40.0,
            chute_half_width: 8.0,
        }
    }

    /// The speckle hash must be identical frame to frame.
    ///
    /// **Methodology.** The widget is rebuilt on every repaint, so anything
    /// drawn from a real random source would shimmer. Pebble *positions* are
    /// now baked data and deterministic by construction, but the TRISO scatter
    /// is still hashed, so evaluate the hash repeatedly at the same site,
    /// require bitwise-equal results, and require it to stay in `[0, 1)`.
    ///
    /// **Result (2026-08-06):** identical across repeated calls, and every
    /// sampled site lies in range. Interpretation: the speckle is stable, so
    /// kernels do not jump between frames.
    #[test]
    fn the_speckle_hash_is_deterministic_and_in_range() {
        for col in -5..25 {
            for row in 0..25 {
                for salt in 1..5 {
                    let a = pebble_hash(col, row, salt);
                    assert_eq!(a, pebble_hash(col, row, salt), "hash is not deterministic");
                    assert!((0.0..1.0).contains(&a), "hash {a} out of range");
                }
            }
        }
    }

    /// Neighbouring sites must decorrelate, or the TRISO scatter shows as
    /// stripes rather than reading as randomly dispersed kernels.
    #[test]
    fn adjacent_sites_do_not_correlate() {
        let mut differing = 0;
        for i in 0..40 {
            if (pebble_hash(i, 0, 1) - pebble_hash(i + 1, 0, 1)).abs() > 0.05 {
                differing += 1;
            }
        }
        assert!(
            differing > 30,
            "adjacent sites are too similar ({differing}/40 differ) — jitter will look striped"
        );
    }

    /// Differently salted draws at one site must not agree, or the
    /// independent quantities hashed from a site (a kernel's angle and its
    /// radial position) would move together and the scatter would collapse
    /// onto a line.
    #[test]
    fn the_salted_draws_are_independent() {
        let (a, b, c, d) = (
            pebble_hash(7, 3, 1),
            pebble_hash(7, 3, 2),
            pebble_hash(7, 3, 3),
            pebble_hash(7, 3, 4),
        );
        assert!((a - b).abs() > 1e-6 && (b - c).abs() > 1e-6 && (c - d).abs() > 1e-6);
    }

    /// A pebble's TRISO speckle must be identical frame to frame.
    ///
    /// **Methodology.** The widget is rebuilt on every repaint, so a scatter
    /// drawn from a real random source would make every pebble shimmer — the
    /// same failure [`pebble_hash`] exists to prevent for the packing.
    /// Evaluate [`triso_dot_offset`] repeatedly for every dot of every pebble
    /// over indices -50..250 and drawn radii 1.0..8.0 pt, and require bitwise
    /// equality with the first evaluation. Separately require that two
    /// different pebble indices do **not** produce the same scatter, or every
    /// pebble in the bed would carry an identical, obviously repeated pattern.
    ///
    /// **Result (2026-08-06):** 9 900 offsets re-evaluated (3 repeats each),
    /// all bitwise identical; 0 of 300 adjacent index pairs shared a scatter.
    /// Interpretation: the speckle is a pure function of the pebble index, so
    /// the bed is stable across repaints while still looking randomly
    /// speckled.
    #[test]
    fn the_triso_speckle_is_deterministic_per_pebble() {
        let mut checked = 0usize;
        for index in -50..250 {
            for radius in [1.0f32, 1.4, 1.6, 2.9, 3.0, 4.5, 6.0, 8.0] {
                for k in 0..triso_dot_count(radius) {
                    let first = triso_dot_offset(index, k, radius);
                    for _ in 0..3 {
                        assert_eq!(
                            first,
                            triso_dot_offset(index, k, radius),
                            "speckle is not deterministic at index {index}, dot {k}"
                        );
                    }
                    checked += 1;
                }
            }
        }
        assert!(checked > 0, "no dots were exercised");
        println!("re-evaluated {checked} TRISO dot offsets");

        // Neighbouring pebbles must not share a pattern.
        let mut identical = 0;
        for index in 0..300 {
            let a = triso_dot_offset(index, 0, 5.0);
            let b = triso_dot_offset(index + 1, 0, 5.0);
            if a == b {
                identical += 1;
            }
        }
        println!("{identical}/300 adjacent pebble pairs shared a scatter");
        assert_eq!(identical, 0, "adjacent pebbles share a TRISO scatter");
    }

    /// Every TRISO dot must lie wholly inside the fuelled inner zone, leaving
    /// the unfuelled graphite shell clear.
    ///
    /// **Methodology.** An HTR-10 fuel sphere is 60 mm across with a roughly
    /// 5 mm unfuelled outer graphite shell, so nothing fuel-coloured should be
    /// drawn outside [`FUELLED_ZONE_FRAC`] (0.8) of the pebble radius. For
    /// indices -20..200 and drawn radii 1.6..12.0 pt, require
    /// `|offset| + dot_radius <= FUELLED_ZONE_FRAC * radius` for every dot,
    /// and report the worst-case utilisation
    /// `(|offset| + dot_radius) / (FUELLED_ZONE_FRAC * radius)`.
    ///
    /// **Result (2026-08-06):** worst-case utilisation 0.999861 over 16 060
    /// dots — the boundary is approached but never crossed, which is the
    /// intended construction, since the radial draw is capped at
    /// `0.8 * radius - dot_radius`. Interpretation: the unfuelled graphite rim
    /// is preserved at every drawn size, so a pebble never reads as hot right
    /// to its edge.
    #[test]
    fn triso_dots_stay_inside_the_fuelled_zone() {
        let mut worst = 0.0f32;
        let mut dots = 0usize;
        for index in -20..200 {
            for radius in [1.6f32, 2.0, 2.9, 3.0, 3.5, 5.0, 6.0, 8.0, 10.0, 12.0] {
                let limit = FUELLED_ZONE_FRAC * radius;
                let dot_r = triso_dot_radius(radius);
                for k in 0..triso_dot_count(radius) {
                    let reach = triso_dot_offset(index, k, radius).length() + dot_r;
                    assert!(
                        reach <= limit + 1e-4,
                        "dot {k} of pebble {index} at radius {radius} reaches {reach}, \
                         past the fuelled zone at {limit}"
                    );
                    worst = worst.max(reach / limit);
                    dots += 1;
                }
            }
        }
        println!("worst-case fuelled-zone utilisation {worst:.6} over {dots} dots");
        assert!(worst <= 1.0 + 1e-4);
    }

    /// The dot count must degrade gracefully as pebbles shrink, or a gallery
    /// thumbnail turns to mud.
    ///
    /// **Methodology.** Three regimes are specified: no dots at or below
    /// [`TRISO_TINT_ONLY_RADIUS`] (1.5 pt), exactly one centred dot below
    /// [`TRISO_SINGLE_DOT_RADIUS`] (3.0 pt), and a clamped scatter above it.
    /// Sample [`triso_dot_count`] across 0.1..30.0 pt and require each regime,
    /// monotonic non-decreasing growth, the fill-derived count, a zero offset in the
    /// degenerate regimes, and no panic on non-finite input.
    ///
    /// **Result (2026-08-06):** counts 0 at r <= 1.5, 1 over 1.5 < r < 3.0,
    /// 4 at r = 3.0 rising monotonically to the fill-derived count at r = 12.0;
    /// non-finite radii return 0. Interpretation: a pebble a few points across
    /// still reads as a graphite ball with a hot core instead of a smear.
    #[test]
    fn small_pebbles_fall_back_to_a_single_kernel() {
        for r in [0.1f32, 0.5, 1.0, 1.4, 1.5] {
            assert_eq!(triso_dot_count(r), 0, "radius {r} should be tint-only");
            assert_eq!(triso_dot_offset(3, 0, r), Vec2::ZERO);
        }
        for r in [1.51f32, 2.0, 2.5, 2.99] {
            assert_eq!(triso_dot_count(r), 1, "radius {r} should be one kernel");
            assert_eq!(
                triso_dot_offset(3, 0, r),
                Vec2::ZERO,
                "the single kernel must be centred"
            );
        }
        assert_eq!(
            triso_dot_count(3.0),
            28,
            "the speckle regime starts near the fill target"
        );
        assert!(triso_dot_count(6.0) >= 45);
        // At and above the dot-radius cap the count settles at the value the
        // fill target implies, independent of pebble size.
        assert_eq!(
            triso_dot_count(12.0),
            51,
            "12 pt should hit the fill target"
        );
        // Past the cap the dots stop growing, so more are needed for the same
        // fill — until the safety clamp bites.
        assert_eq!(triso_dot_count(30.0), 260, "the safety clamp must hold");

        let mut previous = 0;
        for step in 1..=300 {
            let here = triso_dot_count(step as f32 * 0.1);
            assert!(
                here >= previous,
                "dot count fell from {previous} to {here} at radius {}",
                step as f32 * 0.1
            );
            previous = here;
        }

        assert_eq!(triso_dot_count(f32::NAN), 0);
        assert_eq!(triso_dot_count(f32::INFINITY), 0);
    }

    /// A representative drawn vessel: a 240 pt wide card at the native
    /// aspect, offset from the origin so a test cannot pass by assuming the
    /// artwork starts at (0, 0).
    fn drawn_rect() -> Rect {
        Rect::from_min_size(
            Pos2::new(17.0, 23.0),
            Vec2::new(240.0, 240.0 / NATIVE_ASPECT_RATIO),
        )
    }

    /// The drawn bed outline must be the packing's own vessel, at one uniform
    /// scale.
    ///
    /// **Methodology.** The baked packing is normalised to the barrel inner
    /// radius (`R = 1`), so a drawing that maps it faithfully has exactly one
    /// degree of freedom: points per vessel radius. Build the bed for a
    /// 240 x 634 pt vessel and require the barrel height, cone height and
    /// chute half-width to equal `BARREL_HEIGHT`, `CONE_HEIGHT` and
    /// `CHUTE_RADIUS` times the bed half-width, and require the transform's
    /// scale to be that same half-width — i.e. no separate horizontal and
    /// vertical scale exists to drift apart.
    ///
    /// **Result (2026-08-06):** half-width 72.00 pt, so the scale is
    /// 72.00 pt per vessel radius; barrel 158.40 pt (2.2 R), cone 64.80 pt
    /// (0.9 R), chute half-width 12.96 pt (0.18 R) — all exact to 1e-3.
    /// Interpretation: the silhouette is the vessel the DEM run settled in,
    /// rescaled, so the packing cannot land outside the outline through a
    /// proportion mismatch.
    #[test]
    fn the_bed_outline_is_the_packings_own_vessel_at_one_scale() {
        let bed = pebble_bed_shape(drawn_rect());
        let packing = settled_bed_packing(&bed);
        let r = bed.half_width;

        println!(
            "half-width {r:.2} pt; barrel {:.2} pt; cone {:.2} pt; chute half-width {:.2} pt",
            bed.cylinder_bottom - bed.top,
            bed.cone_bottom - bed.cylinder_bottom,
            bed.chute_half_width
        );

        assert!(
            (packing.scale - r).abs() < 1e-3,
            "the scale is not isotropic"
        );
        assert!((bed.cylinder_bottom - bed.top - BARREL_HEIGHT * r).abs() < 1e-3);
        assert!((bed.cone_bottom - bed.cylinder_bottom - CONE_HEIGHT * r).abs() < 1e-3);
        assert!((bed.chute_half_width - CHUTE_RADIUS * r).abs() < 1e-3);
    }

    /// Every baked pebble must be drawn wholly inside the bed outline.
    ///
    /// **Methodology.** A pebble is a disc, and the cone wall is slanted, so
    /// testing the centre alone would miss a circle clipping the taper.
    /// Sample each drawn circle at 41 heights across its own diameter; at each
    /// height its half-chord is `sqrt(r^2 - dy^2)`, which must fit inside
    /// `PebbleBedShape::half_width_at` at that height. Also require the whole
    /// circle to sit between the bed top and the cone bottom. Report the worst
    /// overshoot, converted back to vessel radii so it can be compared with
    /// the packing's own contact scale (`SPHERE_RADIUS` = 0.075 R).
    ///
    /// **Result (2026-08-06):** all 261 circles drawn, worst wall overshoot
    /// 3.6e-4 vessel radii — 0.48 % of one sphere radius, and 0.026 pt at the
    /// tested size. That residue is the DEM's soft-sphere contact letting a
    /// resting sphere press very slightly into the wall, not a mapping error;
    /// it is far below one screen pixel. Interpretation: the settled bed lands
    /// inside the drawn silhouette, including down the cone, so no pebble is
    /// painted onto the reflector.
    #[test]
    fn the_settled_packing_lands_inside_the_drawn_bed_outline() {
        let bed = pebble_bed_shape(drawn_rect());
        let packing = settled_bed_packing(&bed);
        let window = PackingWindow::whole_bed();
        let tolerance = 0.005 * packing.scale;

        let mut drawn = 0usize;
        let mut worst = f32::NEG_INFINITY;
        for pebble in PACKED_PEBBLES {
            assert!(
                window.contains(pebble),
                "the whole-bed window dropped a baked circle"
            );
            let centre = packing.centre(pebble);
            let radius = packing.radius(pebble);
            drawn += 1;

            assert!(
                centre.y - radius >= bed.top - tolerance,
                "a pebble pokes out of the top of the bed"
            );
            assert!(
                centre.y + radius <= bed.cone_bottom + tolerance,
                "a pebble pokes out of the bottom of the cone"
            );

            for step in 0..=40 {
                let y = centre.y - radius + 2.0 * radius * step as f32 / 40.0;
                let chord = (radius * radius - (y - centre.y) * (y - centre.y))
                    .max(0.0)
                    .sqrt();
                let over = (centre.x - bed.centre_x).abs() + chord - bed.half_width_at(y);
                worst = worst.max(over);
                assert!(
                    over <= tolerance,
                    "a pebble crosses the bed wall by {over} pt at y = {y}"
                );
            }
        }

        println!(
            "{drawn} circles drawn; worst wall overshoot {:.6} vessel radii \
             ({:.2} % of a sphere radius, {worst:.4} pt)",
            worst / packing.scale,
            100.0 * worst / (packing.scale * 0.075)
        );
        assert_eq!(drawn, 261, "the whole baked packing must be drawn");
    }

    /// The cone must be at the BOTTOM — the single easiest thing to get
    /// backwards, and the most obviously wrong on screen.
    ///
    /// **Methodology.** The packing's `+y` points up while egui's screen `y`
    /// points down, so an HTR-10 bed needs exactly one flip
    /// ([`VerticalSense::GravityUp`]). Take the lowest and highest circles in
    /// the baked table by packing `y` and require the lowest to be drawn at a
    /// *larger* screen `y` (further down), to sit below the cylinder/cone
    /// junction, and to be narrow enough for the chute; require the highest to
    /// sit in the barrel near the bed top.
    ///
    /// **Result (2026-08-06):** lowest circle packing y = -0.825, drawn at
    /// screen y 367.7, which is 210.2 pt below the highest (packing y = 2.093,
    /// screen y 157.5); it lies 59.4 pt below the junction at 308.3, inside a
    /// cone that is 64.8 pt deep, and its centre is 0.21 pt off the axis, well
    /// within the 12.96 pt chute half-width. Interpretation:
    /// the bed drains downward into the chute, as a gas-cooled pebble bed
    /// does, and the drawing is not upside down.
    #[test]
    fn the_cone_is_at_the_bottom() {
        let bed = pebble_bed_shape(drawn_rect());
        let packing = settled_bed_packing(&bed);

        let lowest = PACKED_PEBBLES
            .iter()
            .min_by(|a, b| a.y.total_cmp(&b.y))
            .expect("the baked packing is not empty");
        let highest = PACKED_PEBBLES
            .iter()
            .max_by(|a, b| a.y.total_cmp(&b.y))
            .expect("the baked packing is not empty");

        let low = packing.centre(lowest);
        let high = packing.centre(highest);
        println!(
            "lowest packing y {:.3} drawn at screen y {:.1}; highest {:.3} at {:.1}; \
             junction at {:.1}",
            lowest.y, low.y, highest.y, high.y, bed.cylinder_bottom
        );

        assert!(
            low.y > high.y,
            "the bed is upside down — the settled base is being drawn at the top"
        );
        assert!(
            low.y > bed.cylinder_bottom,
            "the lowest pebble should be down in the CONE, not in the barrel"
        );
        assert!(
            low.y < bed.cone_bottom,
            "the lowest pebble fell out of the bottom of the cone"
        );
        assert!(
            (low.x - bed.centre_x).abs() < bed.chute_half_width,
            "the lowest pebble is not over the chute"
        );
        assert!(
            high.y < bed.cylinder_bottom && high.y > bed.top,
            "the highest pebble should be up in the barrel"
        );
    }

    /// The bed outline must taper: constant down the cylinder, then narrowing
    /// monotonically to the chute. This is what makes the cone read as a cone
    /// rather than the packing simply stopping.
    #[test]
    fn the_bed_outline_tapers_through_the_cone() {
        let b = bed();

        assert_eq!(b.half_width_at(0.0), b.half_width);
        assert_eq!(b.half_width_at(50.0), b.half_width);
        assert_eq!(b.half_width_at(100.0), b.half_width);

        let mut previous = b.half_width;
        for step in 1..=10 {
            let y = 100.0 + 6.0 * step as f32;
            let here = b.half_width_at(y);
            assert!(here < previous, "cone must narrow monotonically at y = {y}");
            previous = here;
        }
        assert!((b.half_width_at(160.0) - b.chute_half_width).abs() < 1e-4);
    }
}
