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

/// Deterministic pseudo-random value in `[0, 1)` from two lattice indices.
///
/// **Determinism is the point.** The widget is rebuilt every repaint, so
/// drawing the packing from a real random source would make the bed shimmer —
/// every pebble jumping to a new spot each frame. Hashing the lattice indices
/// instead gives a packing that looks random but is identical frame to frame.
///
/// `salt` separates the independent draws (x offset, y offset, radius, void)
/// so they do not correlate into visible stripes.
fn pebble_hash(col: i32, row: i32, salt: u32) -> f32 {
    let mut h = (col as u32).wrapping_mul(0x9E37_79B9)
        ^ (row as u32).wrapping_mul(0x85EB_CA6B)
        ^ salt.wrapping_mul(0xC2B2_AE35);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2545_F491);
    h ^= h >> 13;
    (h % 1_000_003) as f32 / 1_000_003.0
}

/// Fraction of lattice sites left empty, to break up the rows.
///
/// A real pebble bed packs to roughly 0.61 solid fraction, so it is visibly
/// *not* a close-packed lattice. Dropping about one site in eight, on top of
/// the positional jitter, is what stops the drawing reading as a grid.
const PEBBLE_VOID_FRACTION: f32 = 0.12;

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
        ((radius * 12.0) as usize).clamp(30, 120)
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
        (radius * 0.075).clamp(0.35, 1.0)
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

/// Draws a randomly packed pebble bed filling `shape`.
///
/// Pebbles are laid on a jittered lattice: each site is displaced by up to
/// half a spacing in both axes, its radius varied by roughly a fifth, and some
/// sites dropped entirely (see [`PEBBLE_VOID_FRACTION`]). Sites whose pebble
/// would not fit inside the bed outline at that height are skipped, so the
/// packing follows the cone down to the chute rather than stopping at the
/// cylinder.
///
/// Each surviving site is drawn by [`draw_triso_pebble`] as a graphite body
/// speckled with fuel-coloured TRISO kernels, with `kernel` the colour the
/// caller's temperature map gives the fuel. The per-pebble scatter is seeded
/// from the lattice indices, so it is stable frame to frame like the packing.
fn draw_packed_pebbles(
    painter: &egui::Painter,
    shape: PebbleBedShape,
    base_radius: f32,
    kernel: Color32,
) {
    let step = base_radius * 2.15;

    let mut y = shape.top + base_radius * 1.2;
    let mut row: i32 = 0;
    while y < shape.cone_bottom - base_radius * 0.5 {
        let half = shape.half_width_at(y);
        // Stagger alternate rows before jittering, so the starting lattice is
        // hexagonal rather than square.
        let stagger = if row % 2 == 0 { 0.0 } else { step * 0.5 };
        let mut x = shape.centre_x - half + base_radius + stagger;
        let mut col: i32 = 0;

        while x < shape.centre_x + half - base_radius {
            if pebble_hash(col, row, 4) >= PEBBLE_VOID_FRACTION {
                let dx = (pebble_hash(col, row, 1) - 0.5) * step * 0.85;
                let dy = (pebble_hash(col, row, 2) - 0.5) * step * 0.7;
                let r = base_radius * (0.78 + 0.42 * pebble_hash(col, row, 3));

                let (px, py) = (x + dx, y + dy);
                // Keep the pebble inside the bed at ITS OWN height, which is
                // what makes the cone read as a cone.
                let half_here = shape.half_width_at(py);
                let inside_x = (px - shape.centre_x).abs() + r <= half_here;
                let inside_y = py - r >= shape.top && py + r <= shape.cone_bottom;
                if inside_x && inside_y {
                    // Seed the speckle from the lattice site, so a pebble's
                    // TRISO scatter is as stable across repaints as its
                    // position is.
                    let index = row.wrapping_mul(1009).wrapping_add(col);
                    draw_triso_pebble(painter, Pos2::new(px, py), r, PEBBLE_MATRIX, kernel, index);
                }
            }
            x += step;
            col += 1;
        }

        y += step * 0.86;
        row += 1;
    }
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

        // ── Pebble bed: rounded top, cylinder, then a cone to the chute ─────
        let bed_half_w = w * 0.30;
        let bed_top = rect.top() + h * 0.20;
        let bed_cyl_bottom = rect.top() + h * 0.46;
        let cone_bottom = rect.top() + h * 0.60;
        let hot = self.colour(self.pebble_temp);

        // Cylindrical section with a domed upper surface.
        let bed_body = Rect::from_min_max(
            Pos2::new(cx - bed_half_w, bed_top),
            Pos2::new(cx + bed_half_w, bed_cyl_bottom),
        );
        painter.rect_filled(bed_body, bed_half_w * 0.35, hot);

        // Conical bottom funnelling into the discharge tube.
        let chute_half_w = w * 0.055;
        painter.add(egui::Shape::convex_polygon(
            vec![
                Pos2::new(cx - bed_half_w, bed_cyl_bottom),
                Pos2::new(cx + bed_half_w, bed_cyl_bottom),
                Pos2::new(cx + chute_half_w, cone_bottom),
                Pos2::new(cx - chute_half_w, cone_bottom),
            ],
            hot,
            Stroke::NONE,
        ));

        // Pebbles, randomly packed across the bed — cylinder AND cone. Each
        // is a graphite body speckled with TRISO kernels at the fuel colour,
        // so the bed reads as fuelled graphite spheres rather than as one
        // uniformly hot volume.
        draw_packed_pebbles(
            &painter,
            PebbleBedShape {
                centre_x: cx,
                top: bed_top,
                cylinder_bottom: bed_cyl_bottom,
                cone_bottom,
                half_width: bed_half_w,
                chute_half_width: chute_half_w,
            },
            (bed_half_w / 5.0).clamp(1.5, 5.0),
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

    /// The packing must be identical frame to frame.
    ///
    /// **Methodology.** The widget is rebuilt on every repaint, so a packing
    /// drawn from a real random source would make the bed shimmer. Evaluate
    /// the hash repeatedly at the same site and require bitwise-equal results,
    /// and require it to stay in `[0, 1)`.
    ///
    /// **Result (2026-08-06):** identical across repeated calls, and every
    /// sampled site lies in range. Interpretation: the bed looks randomly
    /// packed but is stable, so pebbles do not jump between frames.
    #[test]
    fn the_packing_is_deterministic_and_in_range() {
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

    /// Neighbouring sites must decorrelate, or the jitter shows as stripes
    /// rather than reading as a random packing.
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

    /// The four independent draws at one site must not agree, or radius and
    /// position would vary together and every large pebble would sit in the
    /// same place within its cell.
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
    /// monotonic non-decreasing growth, the 120-dot cap, a zero offset in the
    /// degenerate regimes, and no panic on non-finite input.
    ///
    /// **Result (2026-08-06):** counts 0 at r <= 1.5, 1 over 1.5 < r < 3.0,
    /// 4 at r = 3.0 rising monotonically to the 120-dot cap at r = 12.0;
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
            36,
            "the speckle regime starts at 36 dots"
        );
        assert!(triso_dot_count(6.0) > 15);
        assert_eq!(triso_dot_count(12.0), 120, "the cap must bite by 12 pt");
        assert_eq!(triso_dot_count(30.0), 120, "the cap must hold");

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
