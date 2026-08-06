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
use crate::components::htr10_reactor_vessel::Htr10ReactorVesselVisual;
use crate::components::temperature_colour;
use egui::{Color32, FontId, Pos2, Rect, Response, Sense, Stroke, StrokeKind, Ui, Vec2, Widget};
use nee_soon::NordheimFuchsExactTimestepper;
use uom::si::f64::{
    HeatCapacity, Power, Ratio, TemperatureCoefficient, ThermodynamicTemperature, Time,
};
use uom::si::heat_capacity::joule_per_kelvin;
use uom::si::power::watt;
use uom::si::ratio::ratio;
use uom::si::temperature_coefficient::per_kelvin;
use uom::si::thermodynamic_temperature::{degree_celsius, kelvin};
use uom::si::time::second;

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

    /// Approximate thermal power, in megawatts, for scaling a lumped model.
    ///
    /// These are the widely published headline ratings, not design data: HTR-10
    /// and MSRE are small experimental reactors, EBR-II a mid-size test
    /// reactor, and the iPWR / BWR / FHR figures are per-module SMR-class
    /// ratings. Treat them as order-of-magnitude context, and source a real
    /// value before using one in a V&V case.
    pub fn approximate_thermal_power_mw(self) -> f64 {
        match self {
            Self::Htr10 => 10.0,
            Self::Msre => 8.0,
            Self::IntegralPwr => 250.0,
            Self::Bwr => 870.0,
            Self::Fhr => 280.0,
            Self::EbrII => 62.5,
        }
    }

    /// A Nordheim-Fuchs prompt-excursion model with kinetics parameters
    /// **illustrative of this reactor type**.
    ///
    /// # These are illustrative, not design data
    ///
    /// **Nothing here represents a specific licensed design, and no value is
    /// sourced from a plant.** They are order-of-magnitude constants chosen so
    /// each reactor type behaves *qualitatively* like its class. Do not quote
    /// them, and do not use them in a V&V case without replacing them with
    /// sourced values — see the reactor's scoping document
    /// ([`Self::scoping_doc`]) for what open data exists.
    ///
    /// # What the differences between them mean
    ///
    /// The *relative ordering* is textbook physics, and is the point of having
    /// one model per reactor rather than one shared model:
    ///
    /// - **Prompt neutron generation time** spans four orders of magnitude.
    ///   Graphite-moderated thermal reactors (HTR-10, MSRE, FHR) sit near a
    ///   millisecond because a neutron rattles around a large moderator before
    ///   being absorbed; light-water reactors (iPWR, BWR) are far shorter
    ///   because water moderates in a much smaller volume; and a sodium **fast**
    ///   reactor (EBR-II) is of order a tenth of a microsecond, since there is
    ///   no thermalisation stage at all. This is why EBR-II responds to a
    ///   reactivity insertion so much faster than the others.
    /// - **Delayed neutron fraction** is near 0.0065 for the U-235-fuelled
    ///   cases. It matters because prompt criticality is reached at a
    ///   reactivity insertion of exactly this size.
    /// - **Fuel heat capacity** scales with core size, so it is derived from
    ///   [`Self::approximate_thermal_power_mw`]. It sets how fast fuel
    ///   temperature — and therefore the feedback — responds.
    /// - **Fuel temperature feedback** is negative for every reactor here, which
    ///   is what makes the excursion self-limiting. Note this lumps Doppler
    ///   together with everything else; for EBR-II in particular that is a real
    ///   simplification, because its behaviour is dominated by **core expansion
    ///   feedbacks that this model does not represent at all** (see
    ///   `docs/reactor-scoping/ebr2.md`, gap 1).
    ///
    /// The model starts at its reference temperature and near-zero power, so a
    /// caller drives it by inserting reactivity.
    pub fn illustrative_kinetics(self) -> NordheimFuchsExactTimestepper {
        // Prompt neutron generation time [s], delayed fraction [-],
        // fuel temperature feedback [1/K], reference temperature [degC].
        let (generation_time_s, beta, alpha_f_per_k, reference_degc) = match self {
            Self::Htr10 => (1.0e-3, 0.0065, -4.0e-5, 600.0),
            Self::Msre => (4.0e-4, 0.0064, -3.0e-5, 650.0),
            Self::IntegralPwr => (2.0e-5, 0.0065, -2.5e-5, 300.0),
            Self::Bwr => (4.0e-5, 0.0065, -3.0e-5, 285.0),
            Self::Fhr => (5.0e-4, 0.0065, -3.5e-5, 650.0),
            Self::EbrII => (1.0e-7, 0.0070, -2.0e-6, 370.0),
        };

        // Lumped whole-core fuel heat capacity, scaled off thermal rating. The
        // coefficient is chosen to give a plausible fuel time constant; it is
        // illustrative like everything else here.
        let heat_capacity_j_per_k = self.approximate_thermal_power_mw() * 4.0e5;

        let reference = ThermodynamicTemperature::new::<degree_celsius>(reference_degc);

        NordheimFuchsExactTimestepper::new(
            Time::new::<second>(generation_time_s),
            Ratio::new::<ratio>(beta),
            HeatCapacity::new::<joule_per_kelvin>(heat_capacity_j_per_k),
            TemperatureCoefficient::new::<per_kelvin>(alpha_f_per_k),
            reference,
            reference,
            Power::new::<watt>(1.0),
        )
        .expect(
            "illustrative kinetics constants are compile-time valid \
             (positive generation time and heat capacity, negative feedback); \
             every archetype is constructed in a unit test",
        )
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
    /// HTR-10: delegates to the **real** HTR-10 vessel widget.
    ///
    /// Like the FHR arm, this archetype does not draw its own HTR-10. It
    /// builds an [`Htr10ReactorVesselVisual`], whose geometry follows the
    /// published reactor vertical cross-section — so improving that widget
    /// improves every consumer at once.
    ///
    /// The widget resolves four regions where this archetype carries three;
    /// the reflector takes the mean of inlet and outlet, sitting as it does
    /// between the rising cold helium and the hot bottom plenum. That is a
    /// **display interpolation, not physics** — a caller holding real
    /// per-region state should build the widget directly.
    fn draw_htr10(&self, ui: &mut Ui, rect: Rect) {
        let reflector = ThermodynamicTemperature::new::<kelvin>(
            0.5 * (self.inlet_temp.get::<kelvin>() + self.outlet_temp.get::<kelvin>()),
        );

        let mut vessel = Htr10ReactorVesselVisual::new(
            rect.size(),
            self.min_temp,
            self.max_temp,
            self.core_temp,
            self.inlet_temp,
            self.outlet_temp,
            reflector,
        );
        vessel.set_control_rod_frac(self.control_rod_insertion_frac);
        let vessel = if self.show_labels {
            vessel
        } else {
            vessel.without_labels()
        };

        ui.put(rect, vessel);
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

#[cfg(test)]
mod kinetics_tests {
    use super::*;

    /// Every archetype must build a valid Nordheim-Fuchs model.
    ///
    /// `illustrative_kinetics` unwraps internally, so this is what stops an
    /// invalid constant (non-positive generation time or heat capacity,
    /// non-negative feedback) reaching a caller as a panic.
    #[test]
    fn every_archetype_builds_a_valid_kinetics_model() {
        for a in ReactorArchetype::ALL {
            let k = a.illustrative_kinetics();
            assert!(
                k.prompt_neutron_generation_time.get::<second>() > 0.0,
                "{a:?} has non-positive generation time"
            );
            assert!(
                k.fuel_feedback_coefficient.get::<per_kelvin>() < 0.0,
                "{a:?} feedback must be negative to be self-limiting"
            );
        }
    }

    /// A fast reactor's prompt neutron generation time is orders of magnitude
    /// shorter than a thermal reactor's — there is no thermalisation stage.
    ///
    /// **Methodology.** Compare EBR-II's generation time against every thermal
    /// archetype's, requiring at least two orders of magnitude of separation.
    /// This is the qualitative difference that justifies one kinetics model per
    /// reactor rather than one shared model, so it is pinned rather than left
    /// to the constants happening to be right.
    ///
    /// **Result (2026-08-06):** EBR-II at 1e-7 s against 2e-5 s (iPWR, the
    /// shortest thermal case) — a factor of 200, and up to 1e4 against the
    /// graphite-moderated cases. Interpretation: the fast case responds to a
    /// reactivity insertion far faster than any thermal case, as it must.
    #[test]
    fn the_fast_reactor_has_a_far_shorter_generation_time() {
        let fast = ReactorArchetype::EbrII
            .illustrative_kinetics()
            .prompt_neutron_generation_time
            .get::<second>();

        for a in ReactorArchetype::ALL {
            if *a == ReactorArchetype::EbrII {
                continue;
            }
            let thermal = a
                .illustrative_kinetics()
                .prompt_neutron_generation_time
                .get::<second>();
            assert!(
                thermal > fast * 100.0,
                "{a:?} generation time {thermal} is not clearly slower than the fast case {fast}"
            );
        }
    }

    /// Graphite-moderated reactors must have longer generation times than
    /// light-water ones — a neutron rattles around a large moderator volume
    /// before absorption, where water thermalises in a much smaller one.
    #[test]
    fn graphite_moderated_cases_are_slower_than_light_water_cases() {
        let gen = |a: ReactorArchetype| {
            a.illustrative_kinetics()
                .prompt_neutron_generation_time
                .get::<second>()
        };

        for graphite in [
            ReactorArchetype::Htr10,
            ReactorArchetype::Msre,
            ReactorArchetype::Fhr,
        ] {
            for water in [ReactorArchetype::IntegralPwr, ReactorArchetype::Bwr] {
                assert!(
                    gen(graphite) > gen(water),
                    "{graphite:?} ({}) should be slower than {water:?} ({})",
                    gen(graphite),
                    gen(water)
                );
            }
        }
    }

    /// Delayed neutron fraction must stay near the U-235 value — prompt
    /// criticality is reached at exactly this insertion, so a wrong value would
    /// silently move the threshold the whole model is about.
    #[test]
    fn delayed_fractions_are_near_the_u235_value() {
        for a in ReactorArchetype::ALL {
            let beta = a
                .illustrative_kinetics()
                .delayed_neutron_fraction
                .get::<ratio>();
            assert!(
                (0.005..=0.008).contains(&beta),
                "{a:?} delayed fraction {beta} is outside the plausible band"
            );
        }
    }

    /// Heat capacity must scale with core size, so a small experimental reactor
    /// heats up faster than a power-reactor-scale core for the same energy.
    #[test]
    fn heat_capacity_scales_with_thermal_power() {
        let small = ReactorArchetype::Msre.illustrative_kinetics();
        let large = ReactorArchetype::Bwr.illustrative_kinetics();
        assert!(
            large.fuel_heat_capacity.get::<joule_per_kelvin>()
                > small.fuel_heat_capacity.get::<joule_per_kelvin>(),
            "a power-reactor core must have more heat capacity than an experimental one"
        );
    }
}
