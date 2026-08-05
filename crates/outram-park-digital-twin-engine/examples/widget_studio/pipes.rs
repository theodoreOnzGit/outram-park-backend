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
use outram_park_digital_twin_engine::animation::{residence_time_from_velocity, TracerPulse};
use outram_park_digital_twin_engine::components::PipeVisual;
use tampines::components::{Pipe, PipeBackend};
use tampines::compressible::{CompressibleFluidArray, CoolPropFluid};
use tampines::single_phase::LiquidMaterial;
use tuas_boussinesq_solver::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent;
use tampines_steam_tables::openfoam_algorithms::rhoPimpleFoam::TampinesSteamArray;
use tuas_boussinesq_solver::boussinesq_thermophysical_properties::SolidMaterial;
use uom::si::angle::degree;
use uom::si::area::square_meter;
use uom::si::f64::{
    Angle, Area, HeatTransfer, Length, MassRate, Pressure, Ratio, ThermodynamicTemperature, Time,
    Velocity,
};
use uom::si::heat_transfer::watt_per_square_meter_kelvin;
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
    /// The physics-backed pipe.
    pub pipe: Pipe,
    /// Short name for the row.
    pub name: &'static str,
    /// What this backend models, and what it can and cannot represent.
    pub detail: &'static str,
    /// Temperature mapped to the coldest displayable colour.
    pub min_temp: ThermodynamicTemperature,
    /// Temperature mapped to the hottest displayable colour.
    pub max_temp: ThermodynamicTemperature,
    /// The single tracer mark for this run.
    ///
    /// **Application-owned and advanced once per frame**, then copied into the
    /// widget at build time — widgets are rebuilt every repaint, so a train
    /// owned by the widget would reset its phase to zero each frame and never
    /// appear to move. See `crate::animation`.
    pub tracer: TracerPulse,
    /// Bulk flow velocity, the studio's control over this run.
    pub velocity_m_s: f64,
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
    let steam_area = Area::new::<square_meter>(
        std::f64::consts::FRAC_PI_4 * 0.08 * 0.08,
    );

    let helium_length = Length::new::<meter>(2.5);
    let helium_bore = Length::new::<millimeter>(120.0);
    let helium_area = Area::new::<square_meter>(
        std::f64::consts::FRAC_PI_4 * 0.12 * 0.12,
    );

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
        SolidMaterial::Fiberglass,
        LiquidMaterial::FLiBe,
        HeatTransfer::new::<watt_per_square_meter_kelvin>(20.0),
        CELLS as usize - 2,
        roughness,
    );
    rows.push(PipeRow {
        pipe: Pipe::new(
            PipeBackend::InsulatedPipe(salt),
            salt_bore,
            salt_length,
            roughness,
            incline,
        ),
        tracer: TracerPulse::new(Time::new::<second>(TRACER_INTERVAL_S)),
        velocity_m_s: 1.2,
        name: "Molten salt (FLiBe) — TUAS",
        detail: "PipeBackend::InsulatedPipe · TUAS pre-built: fluid array + metal shell + \
                 insulation, thermally coupled. The only row reporting a WALL temperature.",
        min_temp: ThermodynamicTemperature::new::<kelvin>(800.0),
        max_temp: ThermodynamicTemperature::new::<kelvin>(1000.0),
    });

    // ── Steam / water: two-phase HEM, IAPWS-IF97 ──────────────────────────
    match TampinesSteamArray::new(steam_length, steam_area, CELLS, dt) {
        Ok(steam) => rows.push(PipeRow {
            pipe: Pipe::new(
                PipeBackend::SteamHem(steam),
                steam_bore,
                steam_length,
                roughness,
                incline,
            ),
            tracer: TracerPulse::new(Time::new::<second>(TRACER_INTERVAL_S)),
            velocity_m_s: 6.0,
            name: "Steam / water — TAMPINES HEM",
            detail: "PipeBackend::SteamHem · homogeneous-equilibrium two-phase, \
                     IAPWS-IF97. The only row carrying phase information, and the \
                     baseline drift-flux and two-fluid are measured against.",
            min_temp: ThermodynamicTemperature::new::<kelvin>(300.0),
            max_temp: ThermodynamicTemperature::new::<kelvin>(600.0),
        }),
        Err(e) => errors.push(format!("steam/water (TampinesSteamArray): {e:?}")),
    }

    // ── Helium: single-phase compressible, CoolProp EOS ───────────────────
    match CompressibleFluidArray::new(CoolPropFluid::Helium, helium_length, helium_area, CELLS, dt) {
        Ok(helium) => rows.push(PipeRow {
            pipe: Pipe::new(
                PipeBackend::Compressible(helium),
                helium_bore,
                helium_length,
                roughness,
                incline,
            ),
            tracer: TracerPulse::new(Time::new::<second>(TRACER_INTERVAL_S)),
            velocity_m_s: 20.0,
            name: "Helium gas — OPCP (CoolProp)",
            detail: "PipeBackend::Compressible · single-phase compressible, \
                     Helmholtz EOS. Gas-cooled reactor working fluid — drawn in \
                     LIGHTER shades because the backend carries a gas.",
            min_temp: ThermodynamicTemperature::new::<kelvin>(300.0),
            max_temp: ThermodynamicTemperature::new::<kelvin>(1200.0),
        }),
        Err(e) => errors.push(format!("helium (OPCPFluidArray): {e:?}")),
    }

    (rows, errors)
}

/// Residence time of a row at its current velocity, `tau = L/u`.
pub fn residence_time(row: &PipeRow) -> Time {
    residence_time_from_velocity(
        row.pipe.length,
        Velocity::new::<meter_per_second>(row.velocity_m_s),
    )
}

/// Advance every row's tracer train by one frame.
///
/// Each mark crosses the whole run in exactly one residence time, and the sign
/// of the velocity sets the direction — `TracerTrain::advance` takes direction
/// from the sign of its mass-flow argument and speed only through the
/// residence time, so a unit-magnitude rate carrying the right sign is the
/// documented way to drive it from a velocity.
pub fn advance_tracers(rows: &mut [PipeRow], dt: Time) {
    for row in rows.iter_mut() {
        let tau = residence_time(row);
        let direction =
            MassRate::new::<kilogram_per_second>(if row.velocity_m_s >= 0.0 { 1.0 } else { -1.0 });
        row.tracer.advance(dt, tau, direction);
    }
}

/// Draw the stacked pipes and their labels.
pub fn draw(ui: &mut egui::Ui, rows: &[PipeRow], errors: &[String]) {
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
        let mut widget = PipeVisual::new(
            row.pipe.clone(),
            start,
            Vec2::new(1.0, 0.0),
            row.min_temp,
            row.max_temp,
        )
        .with_wall_alarm(ThermodynamicTemperature::new::<kelvin>(WALL_ALARM_K));

        // One mark at a time, and only while it is actually in flight — the
        // pulse reports None during the gap between releases.
        if let Some(x) = row.tracer.position(residence_time(row)) {
            widget = widget.with_mark_at(x);
        }
        ui.add(widget);
    }
}
