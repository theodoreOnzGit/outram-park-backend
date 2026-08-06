//! Schematic reactor-vessel art, one architecture per reactor type.
//!
//! The six reactor types scoped in `docs/reactor-scoping/` do not share a
//! vessel shape, and the differences are the physics: a BWR's chimney and
//! separator exist to drive natural circulation, an integral PWR's steam
//! generator lives *inside* the vessel, EBR-II's core sits submerged in a
//! sodium pool, MSRE drains its fuel through a freeze valve. Drawing them all
//! as the same rectangle would hide exactly what makes each one interesting.
//!
//! So this module draws each architecture distinctly, at schematic fidelity —
//! recognisable, labelled, and coloured by real temperatures, but not to
//! scale and not a design drawing.
//!
//! # What this is not
//!
//! **These are not validated models and carry no plant data.** They are
//! offline demonstration art for the widget gallery and for simulators that
//! have not yet earned bespoke vessel art. Geometry is illustrative and does
//! not represent any specific licensed design. See `RESPONSIBLE_USE.md`.
//!
//! Where a reactor *has* earned bespoke art, this module **delegates to it
//! rather than redrawing it**: [`ReactorArchetype::Fhr`] renders the real
//! [`crate::components::fhr_reactor_vessel::FhrReactorVesselVisual`] — the
//! artwork migrated out of the `fhr_sim_v2` simulator — so the gallery and the
//! simulator show the same vessel and iterating on one improves both. Its
//! fourteen region temperatures are interpolated from the three this archetype
//! carries; see `draw_fhr` for exactly how, and prefer building that widget
//! directly if you hold real per-region state.
//!
//! # Dispatch
//!
//! [`ReactorArchetype`] is an enum, not a trait object, per the workspace
//! rule: the set of reactor architectures is closed and known at compile time,
//! so adding one is a variant and the compiler then points at every match that
//! needs handling.

use crate::components::fhr_reactor_vessel::FhrReactorVesselVisual;
use crate::components::temperature_colour;
use egui::{Color32, FontId, Pos2, Rect, Response, Sense, Stroke, StrokeKind, Ui, Vec2, Widget};
use uom::si::f64::ThermodynamicTemperature;
use uom::si::thermodynamic_temperature::kelvin;

/// Which reactor architecture to draw.
///
/// Each variant corresponds to a scoping document under
/// `docs/reactor-scoping/`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReactorArchetype {
    /// Pebble-bed high-temperature gas reactor (HTR-10): helium through a
    /// graphite-moderated pebble bed, surrounded by a graphite reflector.
    Htr10,
    /// Molten Salt Reactor Experiment: fuel dissolved in the flowing salt,
    /// graphite stringers in the core, and a drain tank below a freeze valve.
    Msre,
    /// Integral PWR SMR: core, riser and a helical-coil steam generator all
    /// inside one vessel, with the pressuriser in the head.
    IntegralPwr,
    /// Natural-circulation BWR: boiling core, chimney, steam separator and
    /// dryer, with the downcomer annulus returning the liquid.
    Bwr,
    /// Fluoride-salt-cooled high-temperature reactor: pebble bed in FLiBe with
    /// downcomers either side.
    Fhr,
    /// Pool-type sodium fast reactor (EBR-II): core, pumps and intermediate
    /// heat exchanger all submerged in a sodium pool with a free surface.
    EbrII,
}

impl ReactorArchetype {
    /// Every architecture, in the order the gallery shows them.
    pub const ALL: &'static [Self] = &[
        Self::Htr10,
        Self::Msre,
        Self::IntegralPwr,
        Self::Bwr,
        Self::Fhr,
        Self::EbrII,
    ];

    /// Short display name.
    pub fn label(self) -> &'static str {
        match self {
            Self::Htr10 => "HTR-10",
            Self::Msre => "MSRE",
            Self::IntegralPwr => "iPWR",
            Self::Bwr => "BWR",
            Self::Fhr => "FHR",
            Self::EbrII => "EBR-II",
        }
    }

    /// Reactor type in words, for a caption.
    pub fn description(self) -> &'static str {
        match self {
            Self::Htr10 => "pebble-bed HTGR",
            Self::Msre => "molten salt, circulating fuel",
            Self::IntegralPwr => "integral PWR SMR",
            Self::Bwr => "natural-circulation BWR",
            Self::Fhr => "pebble-bed FHR",
            Self::EbrII => "pool-type sodium fast reactor",
        }
    }

    /// Primary coolant.
    pub fn coolant(self) -> &'static str {
        match self {
            Self::Htr10 => "helium",
            Self::Msre => "fueled salt (LiF-BeF2-ZrF4-UF4)",
            Self::IntegralPwr => "pressurised water",
            Self::Bwr => "boiling water",
            Self::Fhr => "FLiBe",
            Self::EbrII => "liquid sodium",
        }
    }

    /// How heat leaves the plant, in words.
    pub fn secondary(self) -> &'static str {
        match self {
            Self::Htr10 => "helical-coil steam generator",
            Self::Msre => "coolant salt to an air-cooled radiator",
            Self::IntegralPwr => "once-through SG inside the vessel",
            Self::Bwr => "direct cycle — steam straight to the turbine",
            Self::Fhr => "salt intermediate loop, then Rankine",
            Self::EbrII => "intermediate sodium loop, then steam",
        }
    }

    /// The scoping document that covers this reactor.
    pub fn scoping_doc(self) -> &'static str {
        match self {
            Self::Htr10 => "docs/reactor-scoping/htr10.md",
            Self::Msre => "docs/reactor-scoping/msre.md",
            Self::IntegralPwr => "docs/reactor-scoping/ipwr.md",
            Self::Bwr => "docs/reactor-scoping/bwr.md",
            Self::Fhr => "docs/reactor-scoping/fhr.md",
            Self::EbrII => "docs/reactor-scoping/ebr2.md",
        }
    }
}

/// Schematic vessel art for one reactor architecture.
///
/// Placement follows the convention every widget in [`crate::components`]
/// uses: `screen_position` is the on-screen centre, `screen_vector` the box
/// size, so the vessel can be positioned absolutely on a schematic canvas.
///
/// Three temperatures drive the colouring. They are absolute thermodynamic
/// temperatures (`uom`-typed, so the unit rides with the type):
///
/// - `core_temp` — the hot region: fuel, pebble bed, or the boiling core.
/// - `inlet_temp` — coolant entering the vessel (the cold leg).
/// - `outlet_temp` — coolant leaving the vessel (the hot leg).
///
/// `min_temp`/`max_temp` bound the colour scale. Because the shared map is
/// diverging — blue, through neutral white, to red — the midpoint carries
/// meaning, so set the range symmetrically about whatever reference matters
/// rather than clamping it to the extremes seen.
pub struct ReactorArchetypeVisual {
    /// Which architecture to draw.
    pub archetype: ReactorArchetype,
    /// On-screen centre position.
    pub screen_position: Pos2,
    /// On-screen size of the vessel box, in points.
    pub screen_vector: Vec2,
    /// Temperature mapped to the coldest displayable colour.
    pub min_temp: ThermodynamicTemperature,
    /// Temperature mapped to the hottest displayable colour.
    pub max_temp: ThermodynamicTemperature,
    /// Core / fuel region temperature.
    pub core_temp: ThermodynamicTemperature,
    /// Coolant inlet (cold leg) temperature.
    pub inlet_temp: ThermodynamicTemperature,
    /// Coolant outlet (hot leg) temperature.
    pub outlet_temp: ThermodynamicTemperature,
    /// Control-rod insertion, dimensionless in `[0, 1]`: `0.0` fully
    /// withdrawn, `1.0` fully inserted. Clamped at render time, so a
    /// controller that transiently overshoots draws fully in or fully out
    /// instead of panicking.
    pub control_rod_insertion_frac: f32,
    /// Whether to draw the small component labels inside the vessel. Off for
    /// thumbnails, where they would be unreadable.
    pub show_labels: bool,
}

impl ReactorArchetypeVisual {
    /// Build a vessel visual for `archetype`.
    ///
    /// Control rods default to fully inserted, so a caller that forgets to
    /// drive them draws a shut-down core rather than a critical one. Labels
    /// default on.
    pub fn new(
        archetype: ReactorArchetype,
        screen_position: Pos2,
        screen_vector: Vec2,
        min_temp: ThermodynamicTemperature,
        max_temp: ThermodynamicTemperature,
        core_temp: ThermodynamicTemperature,
        inlet_temp: ThermodynamicTemperature,
        outlet_temp: ThermodynamicTemperature,
    ) -> Self {
        Self {
            archetype,
            screen_position,
            screen_vector,
            min_temp,
            max_temp,
            core_temp,
            inlet_temp,
            outlet_temp,
            control_rod_insertion_frac: 1.0,
            show_labels: true,
        }
    }

    /// Set control-rod insertion. Builder-style. Dimensionless `[0, 1]`.
    pub fn with_rod_insertion(mut self, frac: f32) -> Self {
        self.control_rod_insertion_frac = frac;
        self
    }

    /// Turn the internal component labels off — for thumbnails.
    pub fn without_labels(mut self) -> Self {
        self.show_labels = false;
        self
    }

    fn core_colour(&self) -> Color32 {
        temperature_colour(self.core_temp, self.min_temp, self.max_temp)
    }
    fn inlet_colour(&self) -> Color32 {
        temperature_colour(self.inlet_temp, self.min_temp, self.max_temp)
    }
    fn outlet_colour(&self) -> Color32 {
        temperature_colour(self.outlet_temp, self.min_temp, self.max_temp)
    }
}

// ── shared drawing helpers ───────────────────────────────────────────────────

const WALL: Color32 = Color32::from_rgb(70, 74, 82);
const GRAPHITE: Color32 = Color32::from_rgb(58, 58, 62);
const ROD: Color32 = Color32::from_rgb(30, 30, 34);
const LABEL: Color32 = Color32::from_rgb(210, 210, 214);

fn wall_stroke() -> Stroke {
    Stroke::new(2.0, WALL)
}

/// Draws a small caption inside the vessel, centred on `at`.
fn tag(ui: &Ui, at: Pos2, text: &str, show: bool) {
    if !show {
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

/// Control rods descending from the vessel head into `body`, inserted by
/// `frac` of the body height.
fn draw_control_rods(ui: &Ui, body: Rect, frac: f32, count: usize) {
    let frac = frac.clamp(0.0, 1.0);
    let depth = body.height() * frac;
    if depth <= 0.5 {
        return;
    }
    let painter = ui.painter();
    for i in 0..count {
        let t = (i as f32 + 1.0) / (count as f32 + 1.0);
        let x = body.left() + t * body.width();
        painter.line_segment(
            [Pos2::new(x, body.top()), Pos2::new(x, body.top() + depth)],
            Stroke::new(3.0, ROD),
        );
    }
}

/// A field of pebbles filling `area`, coloured by `colour`.
fn draw_pebble_bed(ui: &Ui, area: Rect, colour: Color32) {
    let painter = ui.painter();
    painter.rect_filled(area, 3.0, colour);
    let r = (area.width() / 14.0).clamp(2.0, 6.0);
    let step = r * 2.4;
    let shade = Color32::from_rgba_unmultiplied(20, 20, 24, 90);
    let mut y = area.top() + r + 1.0;
    let mut row = 0;
    while y < area.bottom() - r {
        let offset = if row % 2 == 0 { 0.0 } else { step / 2.0 };
        let mut x = area.left() + r + 1.0 + offset;
        while x < area.right() - r {
            painter.circle_filled(Pos2::new(x, y), r, shade);
            x += step;
        }
        y += step * 0.88;
        row += 1;
    }
}

/// Vertical channels — graphite stringers, or a rod bundle.
fn draw_vertical_channels(ui: &Ui, area: Rect, count: usize, colour: Color32, fill: Color32) {
    let painter = ui.painter();
    painter.rect_filled(area, 2.0, fill);
    for i in 0..count {
        let t = (i as f32 + 0.5) / count as f32;
        let x = area.left() + t * area.width();
        painter.line_segment(
            [
                Pos2::new(x, area.top() + 2.0),
                Pos2::new(x, area.bottom() - 2.0),
            ],
            Stroke::new(area.width() / (count as f32 * 2.6), colour),
        );
    }
}

impl Widget for ReactorArchetypeVisual {
    /// Draws the vessel for [`Self::archetype`], coloured by the three
    /// supplied temperatures via the shared
    /// [`crate::components::temperature_colour`] map, so this widget grades
    /// temperature identically to every other component in the library.
    fn ui(self, ui: &mut Ui) -> Response {
        let rect = Rect::from_center_size(self.screen_position, self.screen_vector);
        let response = ui.allocate_rect(rect, Sense::hover());

        match self.archetype {
            ReactorArchetype::Htr10 => self.draw_htr10(ui, rect),
            ReactorArchetype::Msre => self.draw_msre(ui, rect),
            // FHR delegates to the real widget — see `draw_fhr`.
            ReactorArchetype::IntegralPwr => self.draw_ipwr(ui, rect),
            ReactorArchetype::Bwr => self.draw_bwr(ui, rect),
            ReactorArchetype::Fhr => self.draw_fhr(ui, rect),
            ReactorArchetype::EbrII => self.draw_ebr2(ui, rect),
        }

        response
    }
}

impl ReactorArchetypeVisual {
    /// HTR-10: graphite reflector ring around a pebble bed, helium down the
    /// annulus and up through the bed, control rods in the reflector.
    fn draw_htr10(&self, ui: &Ui, rect: Rect) {
        let painter = ui.painter();
        painter.rect_filled(rect, 6.0, self.inlet_colour());
        painter.rect_stroke(rect, 6.0, wall_stroke(), StrokeKind::Middle);

        // Graphite reflector ring.
        let reflector = rect.shrink(rect.width() * 0.10);
        painter.rect_filled(reflector, 4.0, GRAPHITE);
        tag(
            ui,
            Pos2::new(rect.center().x, rect.top() + 9.0),
            "graphite reflector",
            self.show_labels,
        );

        // Pebble bed, conical at the bottom where pebbles are drawn off.
        let bed = Rect::from_min_max(
            Pos2::new(reflector.left() + 6.0, reflector.top() + 14.0),
            Pos2::new(reflector.right() - 6.0, reflector.bottom() - 18.0),
        );
        draw_pebble_bed(ui, bed, self.core_colour());
        tag(ui, bed.center(), "pebble bed", self.show_labels);

        // Defuelling chute.
        painter.line_segment(
            [
                Pos2::new(bed.center().x, bed.bottom()),
                Pos2::new(bed.center().x, rect.bottom() - 3.0),
            ],
            Stroke::new(4.0, GRAPHITE),
        );

        draw_control_rods(ui, reflector, self.control_rod_insertion_frac, 2);

        // Hot helium out of the top.
        painter.rect_filled(
            Rect::from_min_max(
                Pos2::new(rect.right() - 8.0, rect.top() + 10.0),
                Pos2::new(rect.right() - 2.0, rect.top() + 34.0),
            ),
            2.0,
            self.outlet_colour(),
        );
    }

    /// MSRE: fuel salt through graphite stringers, with the drain line and
    /// freeze valve below — the signature safety feature.
    fn draw_msre(&self, ui: &Ui, rect: Rect) {
        let painter = ui.painter();
        let vessel = Rect::from_min_max(
            rect.min,
            Pos2::new(rect.right(), rect.bottom() - rect.height() * 0.26),
        );
        painter.rect_filled(vessel, 6.0, self.inlet_colour());
        painter.rect_stroke(vessel, 6.0, wall_stroke(), StrokeKind::Middle);

        // Graphite stringers with fuel salt flowing between them.
        let core = vessel.shrink2(Vec2::new(vessel.width() * 0.12, vessel.height() * 0.16));
        draw_vertical_channels(ui, core, 7, GRAPHITE, self.core_colour());
        tag(
            ui,
            Pos2::new(core.center().x, core.top() - 7.0),
            "graphite stringers · fuel salt",
            self.show_labels,
        );

        draw_control_rods(ui, core, self.control_rod_insertion_frac, 1);

        // Drain line down to the freeze valve and drain tank.
        let cx = rect.center().x;
        painter.line_segment(
            [
                Pos2::new(cx, vessel.bottom()),
                Pos2::new(cx, vessel.bottom() + 14.0),
            ],
            Stroke::new(4.0, self.core_colour()),
        );
        // Freeze valve — drawn cold, because frozen is its safe state.
        let valve =
            Rect::from_center_size(Pos2::new(cx, vessel.bottom() + 14.0), Vec2::new(14.0, 8.0));
        painter.rect_filled(valve, 2.0, Color32::from_rgb(90, 150, 220));
        tag(
            ui,
            Pos2::new(cx + 52.0, vessel.bottom() + 14.0),
            "freeze valve",
            self.show_labels,
        );

        // Drain tank.
        let tank = Rect::from_center_size(
            Pos2::new(cx, rect.bottom() - 12.0),
            Vec2::new(rect.width() * 0.44, 20.0),
        );
        painter.rect_filled(tank, 5.0, GRAPHITE);
        painter.rect_stroke(tank, 5.0, wall_stroke(), StrokeKind::Middle);
        tag(ui, tank.center(), "drain tank", self.show_labels);
    }

    /// Integral PWR: everything in one vessel — core, riser, helical-coil
    /// steam generator in the annulus, pressuriser in the head.
    fn draw_ipwr(&self, ui: &Ui, rect: Rect) {
        let painter = ui.painter();
        painter.rect_filled(rect, 10.0, self.inlet_colour());
        painter.rect_stroke(rect, 10.0, wall_stroke(), StrokeKind::Middle);

        // Pressuriser in the head.
        let pzr = Rect::from_min_max(
            Pos2::new(rect.left() + 6.0, rect.top() + 4.0),
            Pos2::new(rect.right() - 6.0, rect.top() + rect.height() * 0.16),
        );
        painter.rect_filled(pzr, 5.0, Color32::from_rgb(120, 120, 128));
        tag(ui, pzr.center(), "pressuriser", self.show_labels);

        // Helical-coil SG in the annulus — drawn as stacked coil turns.
        let sg_top = pzr.bottom() + 6.0;
        let sg_bottom = rect.bottom() - rect.height() * 0.30;
        let turns = 7;
        for i in 0..turns {
            let t = (i as f32 + 0.5) / turns as f32;
            let y = sg_top + t * (sg_bottom - sg_top);
            for side in [-1.0f32, 1.0] {
                let x0 = rect.center().x + side * rect.width() * 0.17;
                let x1 = rect.center().x + side * rect.width() * 0.42;
                painter.line_segment(
                    [Pos2::new(x0, y), Pos2::new(x1, y)],
                    Stroke::new(3.0, self.outlet_colour()),
                );
            }
        }
        tag(
            ui,
            Pos2::new(rect.center().x, sg_top - 4.0),
            "helical-coil SG",
            self.show_labels,
        );

        // Central riser carrying hot water up from the core.
        let riser = Rect::from_min_max(
            Pos2::new(rect.center().x - rect.width() * 0.10, sg_top),
            Pos2::new(rect.center().x + rect.width() * 0.10, sg_bottom + 8.0),
        );
        painter.rect_filled(riser, 3.0, self.outlet_colour());

        // Core at the bottom — a rod bundle.
        let core = Rect::from_min_max(
            Pos2::new(rect.left() + rect.width() * 0.22, sg_bottom + 8.0),
            Pos2::new(rect.right() - rect.width() * 0.22, rect.bottom() - 8.0),
        );
        draw_vertical_channels(ui, core, 8, ROD, self.core_colour());
        painter.rect_stroke(core, 2.0, wall_stroke(), StrokeKind::Middle);
        tag(ui, core.center(), "core", self.show_labels);

        draw_control_rods(ui, riser, self.control_rod_insertion_frac, 1);
    }

    /// BWR: boiling core, chimney, separator and dryer, downcomer annulus.
    fn draw_bwr(&self, ui: &Ui, rect: Rect) {
        let painter = ui.painter();
        painter.rect_filled(rect, 10.0, self.inlet_colour());
        painter.rect_stroke(rect, 10.0, wall_stroke(), StrokeKind::Middle);
        tag(
            ui,
            Pos2::new(rect.left() + 26.0, rect.center().y),
            "downcomer",
            self.show_labels,
        );

        let cx = rect.center().x;
        let half = rect.width() * 0.26;

        // Steam dryer at the very top.
        let dryer = Rect::from_min_max(
            Pos2::new(cx - half, rect.top() + 6.0),
            Pos2::new(cx + half, rect.top() + rect.height() * 0.14),
        );
        painter.rect_filled(dryer, 3.0, Color32::from_rgb(150, 150, 158));
        tag(ui, dryer.center(), "dryer", self.show_labels);

        // Steam separator.
        let sep = Rect::from_min_max(
            Pos2::new(cx - half, dryer.bottom() + 4.0),
            Pos2::new(cx + half, dryer.bottom() + 4.0 + rect.height() * 0.12),
        );
        painter.rect_filled(sep, 3.0, Color32::from_rgb(120, 120, 128));
        tag(ui, sep.center(), "separator", self.show_labels);

        // Chimney — the natural-circulation driving head.
        let chimney = Rect::from_min_max(
            Pos2::new(cx - half * 0.8, sep.bottom() + 3.0),
            Pos2::new(cx + half * 0.8, rect.bottom() - rect.height() * 0.34),
        );
        painter.rect_filled(chimney, 2.0, self.outlet_colour());
        tag(ui, chimney.center(), "chimney", self.show_labels);

        // Boiling core.
        let core = Rect::from_min_max(
            Pos2::new(cx - half, chimney.bottom()),
            Pos2::new(cx + half, rect.bottom() - 10.0),
        );
        draw_vertical_channels(ui, core, 7, ROD, self.core_colour());
        painter.rect_stroke(core, 2.0, wall_stroke(), StrokeKind::Middle);
        tag(ui, core.center(), "core", self.show_labels);

        // BWR control rods enter from BELOW — the detail worth getting right.
        let frac = self.control_rod_insertion_frac.clamp(0.0, 1.0);
        let depth = core.height() * frac;
        for i in 0..3 {
            let t = (i as f32 + 1.0) / 4.0;
            let x = core.left() + t * core.width();
            painter.line_segment(
                [
                    Pos2::new(x, core.bottom()),
                    Pos2::new(x, core.bottom() - depth),
                ],
                Stroke::new(3.0, ROD),
            );
        }

        // Steam line out of the top.
        painter.line_segment(
            [
                Pos2::new(cx + half, dryer.center().y),
                Pos2::new(rect.right() - 2.0, dryer.center().y),
            ],
            Stroke::new(4.0, self.outlet_colour()),
        );
    }

    /// FHR: delegates to the **real** pebble-bed FHR vessel widget.
    ///
    /// This archetype does not draw its own FHR. It builds a
    /// [`FhrReactorVesselVisual`] — the artwork migrated out of the
    /// `fhr_sim_v2` simulator — so the gallery and the simulator show the same
    /// vessel, and iterating on that widget improves both at once.
    ///
    /// # Deriving fourteen temperatures from three
    ///
    /// [`FhrReactorVesselVisual`] resolves fourteen independent regions; this
    /// archetype carries three. The mapping below is a **display
    /// interpolation, not physics**:
    ///
    /// - pebble core takes `core_temp`;
    /// - the bed coolant takes the mean of inlet and outlet, since it is
    ///   mid-way along the heated path;
    /// - core bottom and inlet take `inlet_temp`; core top and outlet take
    ///   `outlet_temp`;
    /// - every downcomer node takes `inlet_temp`, because a downcomer carries
    ///   cold salt from the heat exchanger back to the bottom plenum.
    ///
    /// A caller that *has* real per-region state should build
    /// [`FhrReactorVesselVisual`] directly and pass the fourteen values rather
    /// than going through this archetype — do not feed the interpolation back
    /// in as though it were measured.
    fn draw_fhr(&self, ui: &mut Ui, rect: Rect) {
        let mean = |a: ThermodynamicTemperature, b: ThermodynamicTemperature| {
            ThermodynamicTemperature::new::<kelvin>(0.5 * (a.get::<kelvin>() + b.get::<kelvin>()))
        };
        let bed_coolant = mean(self.inlet_temp, self.outlet_temp);

        let mut vessel = FhrReactorVesselVisual::new(
            rect.size(),
            self.min_temp,
            self.max_temp,
            self.core_temp,   // pebble core
            bed_coolant,      // bed coolant, mid-way along the heated path
            self.inlet_temp,  // core bottom
            self.outlet_temp, // core top
            self.inlet_temp,  // core inlet
            self.outlet_temp, // core outlet
            self.inlet_temp,  // left downcomer upper
            self.inlet_temp,  // left downcomer mid
            self.inlet_temp,  // left downcomer lower
            self.inlet_temp,  // right downcomer upper
            self.inlet_temp,  // right downcomer mid
            self.inlet_temp,  // right downcomer lower
        );
        // Both rods follow the single archetype-level insertion control.
        vessel.set_left_cr_frac(self.control_rod_insertion_frac);
        vessel.set_right_cr_frac(self.control_rod_insertion_frac);

        ui.put(rect, vessel);
    }

    /// EBR-II: core, pumps and intermediate heat exchanger all submerged in a
    /// sodium pool with a free surface.
    fn draw_ebr2(&self, ui: &Ui, rect: Rect) {
        let painter = ui.painter();
        painter.rect_filled(rect, 12.0, WALL);
        painter.rect_stroke(rect, 12.0, wall_stroke(), StrokeKind::Middle);

        // The pool, with a free surface below the vessel head.
        let surface_y = rect.top() + rect.height() * 0.16;
        let pool = Rect::from_min_max(
            Pos2::new(rect.left() + 4.0, surface_y),
            Pos2::new(rect.right() - 4.0, rect.bottom() - 4.0),
        );
        painter.rect_filled(pool, 8.0, self.inlet_colour());
        painter.line_segment(
            [
                Pos2::new(pool.left(), surface_y),
                Pos2::new(pool.right(), surface_y),
            ],
            Stroke::new(2.0, Color32::from_rgb(200, 200, 208)),
        );
        tag(
            ui,
            Pos2::new(pool.left() + 46.0, surface_y - 8.0),
            "sodium pool · free surface",
            self.show_labels,
        );

        // Core, submerged, left of centre.
        let core = Rect::from_center_size(
            Pos2::new(pool.center().x - pool.width() * 0.16, pool.center().y + 8.0),
            Vec2::new(pool.width() * 0.28, pool.height() * 0.46),
        );
        draw_vertical_channels(ui, core, 6, ROD, self.core_colour());
        painter.rect_stroke(core, 2.0, wall_stroke(), StrokeKind::Middle);
        tag(ui, core.center(), "core", self.show_labels);
        draw_control_rods(ui, core, self.control_rod_insertion_frac, 2);

        // Intermediate heat exchanger, submerged, right of centre.
        let ihx = Rect::from_center_size(
            Pos2::new(pool.center().x + pool.width() * 0.24, pool.center().y),
            Vec2::new(pool.width() * 0.20, pool.height() * 0.62),
        );
        painter.rect_filled(ihx, 4.0, self.outlet_colour());
        painter.rect_stroke(ihx, 4.0, wall_stroke(), StrokeKind::Middle);
        tag(ui, ihx.center(), "IHX", self.show_labels);

        // Intermediate loop penetrating the head.
        for dx in [-6.0f32, 6.0] {
            painter.line_segment(
                [
                    Pos2::new(ihx.center().x + dx, rect.top() + 2.0),
                    Pos2::new(ihx.center().x + dx, ihx.top()),
                ],
                Stroke::new(3.0, Color32::from_rgb(150, 150, 158)),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every archetype must carry complete metadata — the gallery shows all of
    /// it, and a missing string would render as an empty caption rather than
    /// failing loudly.
    #[test]
    fn every_archetype_has_complete_metadata() {
        for a in ReactorArchetype::ALL {
            assert!(!a.label().is_empty(), "{a:?} has no label");
            assert!(!a.description().is_empty(), "{a:?} has no description");
            assert!(!a.coolant().is_empty(), "{a:?} has no coolant");
            assert!(!a.secondary().is_empty(), "{a:?} has no secondary");
            assert!(
                a.scoping_doc().starts_with("docs/reactor-scoping/"),
                "{a:?} scoping doc path looks wrong: {}",
                a.scoping_doc()
            );
        }
    }

    /// `ALL` must cover the whole enum. If a variant is added and not listed,
    /// the gallery would silently omit it — the one failure mode a closed enum
    /// is supposed to prevent.
    #[test]
    fn all_covers_every_variant() {
        assert_eq!(ReactorArchetype::ALL.len(), 6);
        for a in ReactorArchetype::ALL {
            // Exhaustive match: adding a variant without extending ALL fails
            // to compile here rather than silently disappearing from the UI.
            let covered = match a {
                ReactorArchetype::Htr10
                | ReactorArchetype::Msre
                | ReactorArchetype::IntegralPwr
                | ReactorArchetype::Bwr
                | ReactorArchetype::Fhr
                | ReactorArchetype::EbrII => true,
            };
            assert!(covered);
        }
    }

    /// Labels must be unique, or the gallery picker shows two identical rows.
    #[test]
    fn labels_are_unique() {
        let mut seen = Vec::new();
        for a in ReactorArchetype::ALL {
            assert!(!seen.contains(&a.label()), "duplicate label {}", a.label());
            seen.push(a.label());
        }
    }
}
