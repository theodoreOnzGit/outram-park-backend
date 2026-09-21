//! Pipes tab: the same [`PipeVisual`] widget over all three flow backends,
//! stacked one above the other for direct comparison.
//!
//! This is the point of putting them on one tab. Each backend models a
//! different physical situation, and the studio is where you find out whether
//! the *same* widget renders all three honestly:
//!
//! | Row | Backend | Fluid | Models |
//! |---|---|---|---|
//! | top | [`PipeBackend::Lumped`] | molten salt (FLiBe) | single-phase liquid, TUAS Boussinesq |
//! | middle | [`PipeBackend::SteamHem`] | steam / water | two-phase HEM, IAPWS-IF97 |
//! | bottom | [`PipeBackend::Compressible`] | helium | single-phase compressible, CoolProp EOS |
//!
//! The middle row is the one that carries phase information; it is the
//! intended baseline that drift-flux and two-fluid models get measured against
//! (workspace beads `op-dt3.18`, `op-dt3.19`).
//!
//! **Offline demonstration only.** The geometry and initial states below are
//! illustrative round numbers chosen to make the widget legible — they are not
//! taken from any plant or design, per the workspace `RESPONSIBLE_USE.md` and
//! data policy.

use egui::{Pos2, RichText, Vec2};
use outram_park_digital_twin_engine::animation::{residence_time_from_velocity, TracerTrain};
use outram_park_digital_twin_engine::components::{
    CoaxialDuctGeometry, CoaxialDuctVisual, PipeComponent, PipeScalars,
};
use tampines::components::{Pipe, PipeBackend};
use tampines::compressible::{CompressibleFluidArray, CoolPropFluid};
use tampines::single_phase::LiquidMaterial;
use tuas_boussinesq_solver::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent;
use tampines_steam_tables::openfoam_algorithms::rhoPimpleFoam::TampinesSteamArray;
use tuas_boussinesq_solver::boussinesq_thermophysical_properties::SolidMaterial;
use uom::si::angle::degree;
use uom::si::area::square_meter;
use uom::si::f64::{
    Angle, Area, HeatTransfer, Length, MassDensity, MassRate, Pressure, Ratio,
    ThermodynamicTemperature, Time, Velocity,
};
use uom::si::heat_transfer::watt_per_square_meter_kelvin;
use uom::si::mass_density::kilogram_per_cubic_meter;
use uom::si::mass_rate::kilogram_per_second;
use uom::si::velocity::meter_per_second;
use uom::si::length::{meter, millimeter};
use uom::si::pressure::atmosphere;
use uom::si::ratio::ratio;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::time::second;

/// Number of finite-volume cells in each demonstration pipe.
///
/// Kept small deliberately: `PipeVisual` draws one coloured segment per cell,
/// so the cell count is directly visible as the number of colour bands. A
/// realistic count would render as a smooth gradient and hide the fact that
/// cell count drives the drawing at all.
const CELLS: i64 = 8;

/// Minimum seconds between tracer releases.
///
/// One mark at a time, released no more often than this. A train of marks on a
/// short or fast run strobes and is uncomfortable to watch; a single plug every
/// few seconds reads cleanly and still crosses in exactly the residence time.
const TRACER_INTERVAL_S: f64 = 2.5;

/// Illustrative metal temperature at which a pipe wall is drawn red.
///
/// **Not a code allowable.** A real limit depends on the material, the code of
/// construction and the duty. This is a demonstration threshold only, chosen
/// so the studio can show the alarm state; do not cite or re-use it.
const WALL_ALARM_K: f64 = 850.0;

/// One row of the tab: a pipe plus the label explaining what it is.
pub struct PipeRow {
    /// The persistent pipe: physics array, tracer and display settings.
    ///
    /// Lives across frames. The egui widget it mints each repaint does not —
    /// see `components::PipeComponent` for why the split exists.
    pub component: PipeComponent,
    /// Short name for the row.
    pub name: &'static str,
    /// What this backend models, and what it can and cannot represent.
    pub detail: &'static str,
}

/// Build the three demonstration pipes, top to bottom.
///
/// Returns only the rows that could be constructed. Two of the three backends
/// return a `Result` (their meshes can fail to build), and a row that cannot be
/// built is **omitted with its error reported** rather than replaced by a
/// fabricated stand-in — an empty row is honest, a fake one is not.
pub fn build_rows() -> (Vec<PipeRow>, Vec<String>) {
    let mut rows = Vec::new();
    let mut errors = Vec::new();

    // Deliberately DIFFERENT bores and lengths per row. Length and thickness
    // are now derived from the real geometry, so identical pipes would hide
    // that fact — three different ones make the scaling visible and checkable
    // by eye: the helium line is the widest bore, as a gas duct would be.
    let roughness = Length::new::<millimeter>(0.045);
    let incline = Angle::new::<degree>(0.0);
    let dt = Time::new::<second>(0.01);

    let salt_length = Length::new::<meter>(3.0);
    let salt_bore = Length::new::<millimeter>(50.0);

    let steam_length = Length::new::<meter>(4.0);
    let steam_bore = Length::new::<millimeter>(80.0);
    let steam_area = Area::new::<square_meter>(std::f64::consts::FRAC_PI_4 * 0.08 * 0.08);

    // 12 m. At a 120 mm bore the original 2.5 m run drew almost square
    // (200 x 113 points), reading as a plenum rather than a pipe. A gas duct of
    // this bore would realistically be many metres long, so the fix is a longer
    // PIPE, never a fudged scale — length must stay a true proportion between
    // rows. At 80 points/m this draws 960 points, wider than the panel, which
    // is why the canvas scrolls horizontally.
    let helium_length = Length::new::<meter>(12.0);
    let helium_bore = Length::new::<millimeter>(120.0);
    let helium_area = Area::new::<square_meter>(std::f64::consts::FRAC_PI_4 * 0.12 * 0.12);

    // ── Molten salt: TUAS PRE-BUILT insulated pipe ────────────────────────
    // Uses the component TUAS already ships rather than assembling a bare
    // FluidArray and wiring lateral links by hand: it couples fluid array,
    // metal pipe shell and insulation, so it is the one row that can report a
    // real WALL temperature. 900 K because FLiBe melts near 732 K and TUAS
    // rejects an initial temperature below its valid range rather than
    // extrapolating its property correlations.
    let salt_area = Area::new::<square_meter>(std::f64::consts::FRAC_PI_4 * 0.05 * 0.05);
    let salt = InsulatedFluidComponent::new_insulated_pipe(
        ThermodynamicTemperature::new::<kelvin>(900.0),
        ThermodynamicTemperature::new::<kelvin>(300.0),
        Pressure::new::<atmosphere>(1.0),
        Pressure::new::<atmosphere>(1.0),
        salt_area,
        incline,
        Ratio::new::<ratio>(0.0),
        salt_bore,
        Length::new::<millimeter>(56.0),
        Length::new::<millimeter>(20.0),
        salt_length,
        salt_bore,
        SolidMaterial::SteelSS304L,
        // PyrogelHPS, NOT Fiberglass. FLiBe here sits at 900 K = 626.85 degC,
        // and TUAS's fibreglass correlations stop at 326.85 degC — it warned on
        // every step once the arrays started advancing. Pyrogel HPS is rated to
        // 650 degC, which is also what a molten-salt line would actually be
        // lagged with. Caught only because the pipes now step; a static array
        // never asked its insulation for a property.
        SolidMaterial::PyrogelHPS,
        LiquidMaterial::FLiBe,
        HeatTransfer::new::<watt_per_square_meter_kelvin>(20.0),
        CELLS as usize - 2,
        roughness,
    );
    rows.push(PipeRow {
        component: PipeComponent::new(
            Pipe::new(
                PipeBackend::InsulatedPipe(salt),
                salt_bore,
                salt_length,
                roughness,
                incline,
            ),
            ThermodynamicTemperature::new::<kelvin>(800.0),
            ThermodynamicTemperature::new::<kelvin>(1000.0),
            Velocity::new::<meter_per_second>(1.2),
            Time::new::<second>(TRACER_INTERVAL_S),
        )
        .with_wall_alarm(ThermodynamicTemperature::new::<kelvin>(WALL_ALARM_K)),
        name: "Molten salt (FLiBe) — TUAS",
        detail: "PipeBackend::InsulatedPipe · TUAS pre-built: fluid array + metal shell + \
                 insulation, thermally coupled. The only row reporting a WALL temperature.",
    });

    // ── Steam / water: two-phase HEM, IAPWS-IF97 ──────────────────────────
    match TampinesSteamArray::new(steam_length, steam_area, CELLS, dt) {
        Ok(steam) => rows.push(PipeRow {
            component: PipeComponent::new(
                Pipe::new(
                    PipeBackend::SteamHem(steam),
                    steam_bore,
                    steam_length,
                    roughness,
                    incline,
                ),
                ThermodynamicTemperature::new::<kelvin>(300.0),
                ThermodynamicTemperature::new::<kelvin>(600.0),
                Velocity::new::<meter_per_second>(6.0),
                Time::new::<second>(TRACER_INTERVAL_S),
            )
            .with_wall_alarm(ThermodynamicTemperature::new::<kelvin>(WALL_ALARM_K)),
            name: "Steam / water — TAMPINES HEM",
            detail: "PipeBackend::SteamHem · homogeneous-equilibrium two-phase, \
                     IAPWS-IF97. The only row carrying phase information, and the \
                     baseline drift-flux and two-fluid are measured against.",
        }),
        Err(e) => errors.push(format!("steam/water (TampinesSteamArray): {e:?}")),
    }

    // ── Helium: single-phase compressible, CoolProp EOS ───────────────────
    match CompressibleFluidArray::new(CoolPropFluid::Helium, helium_length, helium_area, CELLS, dt)
    {
        Ok(helium) => rows.push(PipeRow {
            component: PipeComponent::new(
                Pipe::new(
                    PipeBackend::Compressible(helium),
                    helium_bore,
                    helium_length,
                    roughness,
                    incline,
                ),
                ThermodynamicTemperature::new::<kelvin>(300.0),
                ThermodynamicTemperature::new::<kelvin>(1200.0),
                Velocity::new::<meter_per_second>(20.0),
                Time::new::<second>(TRACER_INTERVAL_S),
            )
            .with_wall_alarm(ThermodynamicTemperature::new::<kelvin>(WALL_ALARM_K)),
            name: "Helium gas — OPCP (CoolProp)",
            detail: "PipeBackend::Compressible · single-phase compressible, \
                     Helmholtz EOS. Gas-cooled reactor working fluid — drawn in \
                     LIGHTER shades because the backend carries a gas.",
        }),
        Err(e) => errors.push(format!("helium (OPCPFluidArray): {e:?}")),
    }

    (rows, errors)
}

impl PipeRow {
    /// Short label for a legend caption, where the full name will not fit.
    pub fn short_name(&self) -> &'static str {
        // First word of the row name is enough to tell the three apart.
        match self.name.split_whitespace().next() {
            Some(w) => w,
            None => "pipe",
        }
    }
}

/// Residence time of a row at its current velocity, `tau = L/u`.
pub fn residence_time(row: &PipeRow) -> Time {
    row.component.residence_time()
}

/// Advance every row's PHYSICS and tracer by one frame.
///
/// One call per row, into the component, so the fluid state and the animation
/// stay on the same clock. Errors are collected rather than swallowed: a
/// backend that fails to step must be visible, not silently frozen while its
/// tracer keeps moving and implies everything is fine.
pub fn step_rows(rows: &mut [PipeRow], dt: Time) -> Vec<String> {
    let mut errors = Vec::new();
    for row in rows.iter_mut() {
        if let Err(e) = row.component.step(dt) {
            errors.push(format!("{}: {e}", row.name));
        }
    }
    errors
}

/// Draw the stacked pipes and their labels, then the coaxial duct below them.
pub fn draw(
    ui: &mut egui::Ui,
    rows: &[PipeRow],
    errors: &[String],
    coax: &CoaxialDuctDemo,
) {
    ui.heading("Pipes — one widget, three flow backends");
    ui.label(
        RichText::new(
            "The same PipeVisual over each backend, stacked for comparison. One coloured \
             segment per finite-volume cell, so the cell count is directly visible.",
        )
        .small()
        .weak(),
    );
    ui.separator();

    for e in errors {
        ui.colored_label(
            egui::Color32::from_rgb(220, 120, 60),
            format!("⚠ backend unavailable, row omitted — {e}"),
        );
    }

    // Length is drawn to true scale, so a long run can exceed the panel width.
    // Scrolling is the honest response: shrinking the scale to fit would make
    // pipes of different lengths no longer comparable to each other, which is
    // the whole point of deriving length from geometry.
    let longest_pts = rows
        .iter()
        .map(|r| r.component.pipe.length.get::<meter>() as f32 * 80.0)
        .fold(0.0_f32, f32::max);

    // Reserve the full true-scale width so the scroll area knows how far the
    // longest pipe actually extends.
    ui.set_min_width(longest_pts + 40.0);

    let available = ui.available_rect_before_wrap();
    // Rows must clear the thickest pipe: thickness is derived from bore now,
    // so a fixed row height would clip the widest run.
    let row_height = 118.0_f32;

    for (i, row) in rows.iter().enumerate() {
        let top = available.top() + 12.0 + i as f32 * row_height;

        ui.scope_builder(
            egui::UiBuilder::new().max_rect(egui::Rect::from_min_size(
                Pos2::new(available.left(), top),
                Vec2::new(available.width(), row_height),
            )),
            |ui| {
                ui.label(RichText::new(row.name).strong());
                ui.label(RichText::new(row.detail).small().weak());
            },
        );

        // The pipe run itself, drawn below its label.
        let start = Pos2::new(available.left() + 8.0, top + row_height - 16.0);
        // Length, thickness and slope all come from the pipe's own geometry;
        // screen_vector is only the fallback direction for geometry-less runs.
        // The component mints this frame's widget; all persistent state
        // (physics array, tracer phase) stays in the component.
        ui.add(row.component.visual(start));
    }

    // ── The coaxial duct, below the three single-bore rows ──────────────────
    //
    // Deliberately last and visually separate: it is a different widget
    // (`CoaxialDuctVisual`), not a fourth backend behind the same one.
    let coax_top = available.top() + 12.0 + rows.len() as f32 * row_height;
    ui.scope_builder(
        egui::UiBuilder::new().max_rect(egui::Rect::from_min_size(
            Pos2::new(available.left(), coax_top),
            Vec2::new(available.width(), COAXIAL_ROW_HEIGHT),
        )),
        |ui| {
            ui.separator();
            ui.label(RichText::new("Coaxial duct — HTR-10 hot gas duct").strong());
            ui.label(
                RichText::new(
                    "A different widget: two streams in one duct body, drawn as a \
                     longitudinal section. Bores are the real 300 mm inner / 900 mm outer, \
                     so the drawn band ratio is the plant's.",
                )
                .small()
                .weak(),
            );
            // The derivation chain, printed. This is the studio's whole point:
            // a widget secretly ignoring its physics has nowhere to hide if the
            // numbers driving it are on screen beside it.
            ui.label(
                RichText::new(format!(
                    "inner: {:.2} kg/s over {:.4} m² at {:.2} kg/m³ → {:.1} m/s, τ = {:.2} s   |   \
                     annulus: {:.2} kg/s over {:.4} m² at {:.2} kg/m³ → {:.1} m/s, τ = {:.2} s",
                    coax.hot_mass_flow_kg_per_s,
                    coax.geometry.inner_flow_area().get::<square_meter>(),
                    coax.density_at(coax.hot_temp_k),
                    coax.hot_velocity().get::<meter_per_second>(),
                    coax.core_scalars().residence_time.get::<second>(),
                    coax.cold_mass_flow_kg_per_s,
                    coax.geometry.annulus_flow_area().get::<square_meter>(),
                    coax.density_at(coax.cold_temp_k),
                    coax.cold_velocity().get::<meter_per_second>(),
                    coax.annulus_scalars().residence_time.get::<second>(),
                ))
                .small()
                .monospace(),
            );
        },
    );

    let coax_start = Pos2::new(
        available.left() + 8.0,
        coax_top + COAXIAL_ROW_HEIGHT - 34.0,
    );
    ui.add(coax.visual(coax_start, DISPLAY_MIN_K, DISPLAY_MAX_K));
}

/// Vertical space reserved for the coaxial-duct row, in points.
///
/// Taller than a pipe row: it carries three lines of caption plus a duct drawn
/// thicker than any single-bore run, and the stream labels sit outside the
/// duct body on both sides.
const COAXIAL_ROW_HEIGHT: f32 = 168.0;

/// Cold end of the coaxial duct's colour scale, K.
///
/// Set about the HTR-10 primary loop rather than to the extremes seen: the
/// midpoint of this range is the diverging map's neutral white point, so it
/// falls between the 523 K cold return and the 973 K hot supply and puts one
/// stream on each half of the scale.
const DISPLAY_MIN_K: f64 = 300.0;

/// Hot end of the coaxial duct's colour scale, K.
const DISPLAY_MAX_K: f64 = 1200.0;

// ── Coaxial duct (HTR-10 hot gas duct) ──────────────────────────────────────

/// Molar mass of helium, kg/mol (IUPAC standard atomic weight, 4.002602).
const HELIUM_MOLAR_MASS_KG_PER_MOL: f64 = 0.004_002_602;

/// Universal gas constant, J/(mol K).
const GAS_CONSTANT_J_PER_MOL_K: f64 = 8.314_462_618;

/// The coaxial-duct row: the HTR-10 hot gas duct, both streams live.
///
/// Exists to exercise [`CoaxialDuctVisual`] against real plant numbers rather
/// than invented ones. Every quantity that drives the drawing is **derived**
/// here, in the open, so the tab doubles as a check that the derivation chain
/// works:
///
/// ```text
///   mass flow ──┐
///   bore area ──┼─> velocity ──┐
///   density   ──┘              ├─> residence time ──> tracer SPEED
///   duct length ───────────────┘
///   sign of mass flow ─────────────────────────────> tracer DIRECTION
/// ```
///
/// Nothing about the motion is hardcoded — see the crate `CLAUDE.md`,
/// "ANIMATION IS DERIVED FROM PHYSICS, NEVER HARDCODED".
///
/// **The one thing that cannot be derived is the duct length**, which no
/// source in `docs/reactor-scoping/htr10-plant-data.md` states (section 4.1
/// records it as *Unknown*). It is therefore a slider, labelled as a display
/// choice, and it scales both residence times identically — so the *ratio* of
/// the two tracer speeds stays physical whatever it is set to.
pub struct CoaxialDuctDemo {
    /// Bore geometry — the real HTR-10 duct by default.
    pub geometry: CoaxialDuctGeometry,
    /// Hot helium temperature in the inner tube, K. Reactor outlet.
    pub hot_temp_k: f64,
    /// Cold helium temperature in the annulus, K. Circulator discharge.
    pub cold_temp_k: f64,
    /// Inner-tube mass flow, kg/s. Positive runs reactor -> SG.
    pub hot_mass_flow_kg_per_s: f64,
    /// Annulus mass flow, kg/s. **Negative** runs SG -> reactor, which is the
    /// real direction, and is what makes the duct counter-current.
    pub cold_mass_flow_kg_per_s: f64,
    /// Primary helium pressure, MPa. Sets both densities.
    pub pressure_mpa: f64,
    /// Duct length, m. **Not a plant dimension** — see the type docs.
    pub assumed_length_m: f64,
    /// Drawn run length in screen points. A layout choice, like the length.
    pub drawn_length_points: f32,
    /// Tracer train for the inner stream. Persists across frames.
    core_tracer: TracerTrain,
    /// Tracer train for the annulus stream.
    annulus_tracer: TracerTrain,
}

impl Default for CoaxialDuctDemo {
    /// HTR-10 normal full-power operation, from
    /// `docs/reactor-scoping/htr10-plant-data.md`: helium 250 degC in /
    /// 700 degC out (section 6), 4.32 kg/s (section 6, *Quoted*), 3.0 MPa
    /// primary pressure (section 6, *Quoted*, all three sources agree).
    ///
    /// The annulus flow is seeded **negative** because the cold return runs
    /// SG-to-reactor, against the inner stream. That single sign is what makes
    /// the two tracer trains separate on screen.
    fn default() -> Self {
        Self {
            geometry: CoaxialDuctGeometry::htr10_hot_gas_duct(),
            hot_temp_k: 973.15,
            cold_temp_k: 523.15,
            hot_mass_flow_kg_per_s: 4.32,
            cold_mass_flow_kg_per_s: -4.32,
            pressure_mpa: 3.0,
            assumed_length_m: 5.0,
            drawn_length_points: 520.0,
            core_tracer: TracerTrain::new(5),
            annulus_tracer: TracerTrain::new(5),
        }
    }
}

impl CoaxialDuctDemo {
    /// Helium density at this pressure and temperature, kg/m3, from the ideal
    /// gas law `rho = p M / (R T)`.
    ///
    /// Helium at 3.0 MPa and 250-700 degC is close to ideal (compressibility
    /// factor within a couple of percent of unity), so this is good enough to
    /// set a tracer speed and is stated rather than hidden. It is **not** good
    /// enough for a heat balance — use `outram-park-fork-coolprop` for that.
    fn density(&self, temperature_k: f64) -> MassDensity {
        let p = self.pressure_mpa * 1.0e6;
        let rho = if temperature_k > 0.0 {
            p * HELIUM_MOLAR_MASS_KG_PER_MOL / (GAS_CONSTANT_J_PER_MOL_K * temperature_k)
        } else {
            0.0
        };
        MassDensity::new::<kilogram_per_cubic_meter>(rho)
    }

    /// Helium density at `temperature_k` and the current pressure, kg/m3.
    ///
    /// Exposed so the tab can print the number that is actually driving the
    /// velocity, rather than the reader having to trust that it was computed.
    pub fn density_at(&self, temperature_k: f64) -> f64 {
        self.density(temperature_k)
            .get::<kilogram_per_cubic_meter>()
    }

    /// Bulk velocity in the inner tube, `u = m_dot / (rho A)`.
    pub fn hot_velocity(&self) -> Velocity {
        Self::velocity(
            self.hot_mass_flow_kg_per_s,
            self.density(self.hot_temp_k),
            self.geometry.inner_flow_area(),
        )
    }

    /// Bulk velocity in the annulus, on the gross annular area.
    pub fn cold_velocity(&self) -> Velocity {
        Self::velocity(
            self.cold_mass_flow_kg_per_s,
            self.density(self.cold_temp_k),
            self.geometry.annulus_flow_area(),
        )
    }

    fn velocity(mass_flow_kg_per_s: f64, density: MassDensity, area: Area) -> Velocity {
        let denominator =
            density.get::<kilogram_per_cubic_meter>() * area.get::<square_meter>();
        let u = if denominator > 0.0 {
            mass_flow_kg_per_s / denominator
        } else {
            0.0
        };
        Velocity::new::<meter_per_second>(u)
    }

    fn length(&self) -> Length {
        Length::new::<meter>(self.assumed_length_m)
    }

    /// Inner-tube state handed to the widget.
    pub fn core_scalars(&self) -> PipeScalars {
        PipeScalars {
            temperature: ThermodynamicTemperature::new::<kelvin>(self.hot_temp_k),
            mass_flow: MassRate::new::<kilogram_per_second>(self.hot_mass_flow_kg_per_s),
            residence_time: residence_time_from_velocity(self.length(), self.hot_velocity()),
        }
    }

    /// Annulus state handed to the widget.
    pub fn annulus_scalars(&self) -> PipeScalars {
        PipeScalars {
            temperature: ThermodynamicTemperature::new::<kelvin>(self.cold_temp_k),
            mass_flow: MassRate::new::<kilogram_per_second>(self.cold_mass_flow_kg_per_s),
            residence_time: residence_time_from_velocity(self.length(), self.cold_velocity()),
        }
    }

    /// Advance both tracer trains by `dt`.
    ///
    /// Each train is given **its own** residence time and mass flow, so it
    /// takes both its speed and its direction from that stream's state. Set
    /// either flow to zero and that train freezes; flip a sign and it runs the
    /// other way. Neither behaviour is coded for here — it falls out of
    /// `TracerTrain::advance`.
    pub fn step(&mut self, dt: Time) {
        let core = self.core_scalars();
        let annulus = self.annulus_scalars();
        self.core_tracer
            .advance(dt, core.residence_time, core.mass_flow);
        self.annulus_tracer
            .advance(dt, annulus.residence_time, annulus.mass_flow);
    }

    /// Mint this frame's widget, anchored at `at`.
    ///
    /// The trains are **copied in**, not owned by the widget: widgets are
    /// rebuilt every repaint, so a train living in one would reset its phase
    /// each frame. See `crate::animation` in the engine crate.
    pub fn visual(&self, at: Pos2, min_temp_k: f64, max_temp_k: f64) -> CoaxialDuctVisual {
        CoaxialDuctVisual::new(
            self.geometry,
            at,
            Vec2::new(self.drawn_length_points, 0.0),
            self.core_scalars(),
            self.annulus_scalars(),
            ThermodynamicTemperature::new::<kelvin>(min_temp_k),
            ThermodynamicTemperature::new::<kelvin>(max_temp_k),
        )
        .with_drawn_thickness(54.0)
        .with_core_tracer(self.core_tracer.clone())
        .with_annulus_tracer(self.annulus_tracer.clone())
    }
}
