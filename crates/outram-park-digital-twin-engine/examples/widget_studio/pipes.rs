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
use outram_park_digital_twin_engine::components::PipeVisual;
use tampines::components::{Pipe, PipeBackend};
use tampines::compressible::{CompressibleFluidArray, CoolPropFluid};
use tampines::single_phase::{LiquidMaterial, SinglePhaseFluidArray};
use tampines_steam_tables::openfoam_algorithms::rhoPimpleFoam::TampinesSteamArray;
use tuas_boussinesq_solver::boussinesq_thermophysical_properties::SolidMaterial;
use uom::si::angle::degree;
use uom::si::area::square_meter;
use uom::si::f64::{
    Angle, Area, Length, Pressure, Ratio, ThermodynamicTemperature, Time,
};
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

    let length = Length::new::<meter>(3.0);
    let diameter = Length::new::<millimeter>(50.0);
    let roughness = Length::new::<millimeter>(0.045);
    let incline = Angle::new::<degree>(0.0);
    let xs_area = Area::new::<square_meter>(0.002);
    let dt = Time::new::<second>(0.01);

    // ── Molten salt: single-phase liquid, TUAS Boussinesq ─────────────────
    // `new_cylinder` is infallible, so this row is always present.
    let salt = SinglePhaseFluidArray::new_cylinder(
        length,
        diameter,
        ThermodynamicTemperature::new::<kelvin>(900.0),
        Pressure::new::<atmosphere>(1.0),
        SolidMaterial::SteelSS304L,
        LiquidMaterial::FLiBe,
        Ratio::new::<ratio>(0.0),
        CELLS as usize - 2,
        incline,
    );
    rows.push(PipeRow {
        pipe: Pipe::new(
            PipeBackend::Lumped(salt),
            diameter,
            length,
            roughness,
            incline,
        ),
        name: "Molten salt (FLiBe) — TUAS",
        detail: "PipeBackend::Lumped · single-phase liquid, Boussinesq. \
                 No phase information: this backend cannot represent boiling.",
        min_temp: ThermodynamicTemperature::new::<kelvin>(800.0),
        max_temp: ThermodynamicTemperature::new::<kelvin>(1000.0),
    });

    // ── Steam / water: two-phase HEM, IAPWS-IF97 ──────────────────────────
    match TampinesSteamArray::new(length, xs_area, CELLS, dt) {
        Ok(steam) => rows.push(PipeRow {
            pipe: Pipe::new(
                PipeBackend::SteamHem(steam),
                diameter,
                length,
                roughness,
                incline,
            ),
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
    match CompressibleFluidArray::new(CoolPropFluid::Helium, length, xs_area, CELLS, dt) {
        Ok(helium) => rows.push(PipeRow {
            pipe: Pipe::new(
                PipeBackend::Compressible(helium),
                diameter,
                length,
                roughness,
                incline,
            ),
            name: "Helium gas — OPCP (CoolProp)",
            detail: "PipeBackend::Compressible · single-phase compressible, \
                     Helmholtz EOS. Gas-cooled reactor working fluid.",
            min_temp: ThermodynamicTemperature::new::<kelvin>(300.0),
            max_temp: ThermodynamicTemperature::new::<kelvin>(1200.0),
        }),
        Err(e) => errors.push(format!("helium (OPCPFluidArray): {e:?}")),
    }

    (rows, errors)
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
    let run_length = (available.width() - 260.0).max(120.0);
    let row_height = 78.0_f32;

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
        ui.add(PipeVisual::new(
            row.pipe.clone(),
            start,
            Vec2::new(run_length, 0.0),
            row.min_temp,
            row.max_temp,
        ));
    }
}
