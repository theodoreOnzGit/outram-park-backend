//! The v1.1 plant view: the whole HTR-10 plant, driven by this simulator's
//! running model.
//!
//! It replaced the v1 schematic on screen on 2026-09-22 (maintainer
//! direction). The drawing itself is the engine's
//! `components::htr10_plant::draw_htr10_plant`, the same one the widget
//! studio's HTR-10 page uses; this module only fills it from the
//! [`HtgrSnapshot`] each frame. So everything that moves or colours here is
//! the model's state:
//!
//! | on screen | from the snapshot |
//! |---|---|
//! | pebble bed | fuel temperature |
//! | helium in / out, vessel wall | core inlet / outlet temperature |
//! | reflector | bed (graphite) temperature |
//! | control rods | operator command, slewed like v1, with the scram floor |
//! | helium tracers | helium mass flow and residence time |
//! | main steam / feedwater / condensate | SG steam outlet; (p, h) flashes as in v1 |
//! | turbine rotor | shaft speed |
//! | condenser | exhaust quality, condensate temperature, cooling water |
//! | secondary tracers | secondary mass flow and residence time |
//!
//! Two things are not model state, and are stated rather than invented: the
//! feed pump draws stationary, because this plant has no pump shaft-speed
//! model (the same choice v1 made), and the bed height is the equilibrium
//! loading, since the model has no bed-height variable.
//!
//! **Offline demonstration only**, per the workspace `RESPONSIBLE_USE.md`.

use super::schematic::{feed_and_condensate_temps, SchematicTracers, DISPLAY_MAX_K, DISPLAY_MIN_K};
use super::state::HtgrSnapshot;
use crate::physics::secondary_loop::COOLING_WATER_INLET_K;
use egui::Ui;
use outram_park_digital_twin_engine::animation::control_rod_drive::ControlRodDrive;
use outram_park_digital_twin_engine::components::control_rod_drive::slewed_control_rod_insertion;
use outram_park_digital_twin_engine::components::htr10_plant::{
    draw_htr10_plant, SecondaryLoopView, SecondaryTracers,
};
use outram_park_digital_twin_engine::components::htr10_reactor_schematic::EQUILIBRIUM_BED_HEIGHT_CM;
use outram_park_digital_twin_engine::components::{Htr10ReactorSchematic, Htr10SteamGeneratorVisual};
use uom::si::angular_velocity::radian_per_second;
use uom::si::f64::{AngularVelocity, MassRate, ThermodynamicTemperature, Time};
use uom::si::mass_rate::kilogram_per_second;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::time::second;

fn k(value_k: f64) -> ThermodynamicTemperature {
    ThermodynamicTemperature::new::<kelvin>(value_k)
}

/// Vessel width, points, that the zoom readout calls 100 %.
const NOMINAL_VESSEL_WIDTH: f32 = 300.0;
/// Zoom limits, as drawn vessel width in points. Below the minimum the labels
/// crowd the artwork; above the maximum the canvas gets unwieldy to pan.
const MIN_VESSEL_WIDTH: f32 = 120.0;
const MAX_VESSEL_WIDTH: f32 = 1500.0;
/// Factor one press of the zoom buttons changes the scale by.
const ZOOM_STEP: f32 = 1.25;

/// First-frame guess at the plant canvas size per point of vessel width,
/// replaced by the measured value after the first draw. Only a starting point:
/// a wrong guess costs one frame at the wrong scale.
const FIRST_GUESS_CANVAS_PER_VESSEL_WIDTH: egui::Vec2 = egui::Vec2::new(5.0, 1.8);

/// How far past each edge of the plant the view can pan, as a fraction of the
/// viewport (maintainer direction, 2026-09-22): enough to bring any corner of
/// the plant towards the middle of the screen to focus on it. A viewing
/// margin, nothing physical.
const PAN_MARGIN_FRACTION: f32 = 0.4;

/// Frames to hold the view centred after it opens or Fit is pressed. The
/// first frame draws at the guessed canvas size and the second at the
/// measured one, so centring on one frame alone would centre the guess.
const RECENTRE_FRAMES: u8 = 2;

/// The plant view's zoom and pan, kept in egui memory between frames.
#[derive(Clone, Copy)]
struct PlantZoom {
    /// Drawn vessel width chosen by the operator, points; `None` fits the
    /// whole plant to the viewport (the starting view, and what the Fit
    /// button returns to).
    vessel_width: Option<f32>,
    /// Canvas size per point of vessel width, measured on the last draw.
    canvas_per_vessel_width: egui::Vec2,
    /// Frames left for which the view is scrolled to centre the plant.
    recentre_frames: u8,
    /// Vessel width, scroll offset and viewport of the last frame drawn, so a
    /// zoom can keep the point at the middle of the view where it is.
    last_vessel_width: f32,
    last_offset: egui::Vec2,
    last_viewport: egui::Vec2,
}

impl Default for PlantZoom {
    fn default() -> Self {
        Self {
            vessel_width: None,
            canvas_per_vessel_width: FIRST_GUESS_CANVAS_PER_VESSEL_WIDTH,
            recentre_frames: RECENTRE_FRAMES,
            last_vessel_width: 0.0,
            last_offset: egui::Vec2::ZERO,
            last_viewport: egui::Vec2::ZERO,
        }
    }
}

/// The vessel width that makes the whole plant fit inside `viewport`.
///
/// The layout is linear in vessel width (every dimension is a multiple of it),
/// so the canvas the last frame measured, divided by the vessel width it was
/// drawn at, gives the canvas per unit vessel width, and the fit follows from
/// the tighter of the two axes. This is a drawing scale, nothing physical.
fn fitted_vessel_width(viewport: egui::Vec2, canvas_per_vessel_width: egui::Vec2) -> f32 {
    let fit_x = viewport.x / canvas_per_vessel_width.x;
    let fit_y = viewport.y / canvas_per_vessel_width.y;
    let fit = fit_x.min(fit_y);
    if fit.is_finite() {
        fit.clamp(MIN_VESSEL_WIDTH, MAX_VESSEL_WIDTH)
    } else {
        NOMINAL_VESSEL_WIDTH
    }
}

/// Where the plant sits inside the scrollable content: a margin of
/// [`PAN_MARGIN_FRACTION`] of the viewport on every side, widened further to
/// centre a plant too small to fill the viewport even with that margin.
/// Returns (top-left margin, total content size).
fn content_layout(plant: egui::Vec2, viewport: egui::Vec2) -> (egui::Vec2, egui::Vec2) {
    let pad = PAN_MARGIN_FRACTION * viewport;
    let content = plant + 2.0 * pad;
    let extra = ((viewport - content) * 0.5).max(egui::Vec2::ZERO);
    (pad + extra, content + 2.0 * extra)
}

/// Draw the v1.1 plant from this frame's `snapshot`, with the application's
/// tracer trains, in a pan-and-zoom viewport.
///
/// Panning follows the CIET educational simulator's main page: a
/// `ScrollArea::both` with always-visible bars that also pans on drag. The
/// plant carries a margin of [`PAN_MARGIN_FRACTION`] of the viewport on every
/// side, so any part of it can be brought towards the middle of the screen.
/// It opens centred. Zoom redraws the plant at a different vessel width, so it
/// stays crisp at any scale, and keeps the middle of the view where it was:
/// the -/+ buttons, Ctrl + scroll over the drawing, Fit (the whole plant in
/// view, centred) and 100 %. Labels keep a constant size at every zoom.
pub fn draw_plant_v1_1(ui: &mut Ui, snapshot: &HtgrSnapshot, tracers: &SchematicTracers) {
    let zoom_id = ui.id().with("htr10_plant_zoom");
    let mut zoom: PlantZoom = ui.ctx().data(|d| d.get_temp(zoom_id)).unwrap_or_default();

    // The controls go above the viewport, so the viewport is what is left.
    let current = zoom.vessel_width.unwrap_or_else(|| {
        // Estimated before the controls are drawn, then corrected below.
        fitted_vessel_width(ui.available_size(), zoom.canvas_per_vessel_width)
    });
    ui.horizontal(|ui| {
        ui.label("Plant zoom:");
        if ui.button(" - ").clicked() {
            zoom.vessel_width = Some(current / ZOOM_STEP);
        }
        if ui.button(" + ").clicked() {
            zoom.vessel_width = Some(current * ZOOM_STEP);
        }
        if ui.button("Fit").clicked() {
            zoom.vessel_width = None;
            zoom.recentre_frames = RECENTRE_FRAMES;
        }
        if ui.button("100 %").clicked() {
            zoom.vessel_width = Some(NOMINAL_VESSEL_WIDTH);
        }
        ui.label(format!("{:.0} %", 100.0 * current / NOMINAL_VESSEL_WIDTH));
        ui.separator();
        ui.small("Drag or use the scroll bars to pan; Ctrl + scroll over the plant to zoom.");
    });

    let viewport = ui.available_size();
    // A resized window re-centres a fitted view; a zoomed one keeps its place.
    if zoom.vessel_width.is_none() && viewport != zoom.last_viewport {
        zoom.recentre_frames = zoom.recentre_frames.max(1);
    }
    let vessel_width = zoom
        .vessel_width
        .unwrap_or_else(|| fitted_vessel_width(viewport, zoom.canvas_per_vessel_width))
        .clamp(MIN_VESSEL_WIDTH, MAX_VESSEL_WIDTH);
    let plant = zoom.canvas_per_vessel_width * vessel_width;
    let (margin, content) = content_layout(plant, viewport);

    // The scroll offset to force this frame, if any: centred on open and on
    // Fit; otherwise, after a zoom, the offset that keeps the plant point at
    // the middle of the view where it was.
    let offset = if zoom.recentre_frames > 0 {
        zoom.recentre_frames -= 1;
        Some(((content - viewport) * 0.5).max(egui::Vec2::ZERO))
    } else if zoom.last_vessel_width > 0.0 && vessel_width != zoom.last_vessel_width {
        let (last_margin, _) = content_layout(
            zoom.canvas_per_vessel_width * zoom.last_vessel_width,
            zoom.last_viewport,
        );
        let middle = zoom.last_offset + 0.5 * zoom.last_viewport;
        let on_plant = (middle - last_margin) * (vessel_width / zoom.last_vessel_width);
        Some((margin + on_plant - 0.5 * viewport).max(egui::Vec2::ZERO))
    } else {
        None
    };

    let mut area = egui::ScrollArea::both()
        .id_salt("htr10_plant_viewport")
        .auto_shrink([false, false])
        .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysVisible)
        .scroll_source(egui::scroll_area::ScrollSource::ALL);
    if let Some(offset) = offset {
        area = area.scroll_offset(offset);
    }
    let output = area.show(ui, |ui| {
        ui.spacing_mut().item_spacing = egui::Vec2::ZERO;
        ui.add_space(margin.y);
        let canvas = ui
            .horizontal(|ui| {
                ui.add_space(margin.x);
                let canvas = draw_plant_at(ui, snapshot, tracers, vessel_width);
                ui.add_space(content.x - margin.x - canvas.width());
                canvas
            })
            .inner;
        ui.add_space(content.y - margin.y - canvas.height());
        canvas
    });
    zoom.canvas_per_vessel_width = output.inner.size() / vessel_width;
    zoom.last_vessel_width = vessel_width;
    zoom.last_offset = output.state.offset;
    zoom.last_viewport = viewport;

    // Ctrl + scroll over the viewport zooms the plant (egui reports it as a
    // zoom delta, not a scroll, so it does not also pan).
    if ui.rect_contains_pointer(output.inner_rect) {
        let factor = ui.input(|i| i.zoom_delta());
        if factor != 1.0 {
            zoom.vessel_width =
                Some((vessel_width * factor).clamp(MIN_VESSEL_WIDTH, MAX_VESSEL_WIDTH));
        }
    }
    if let Some(width) = zoom.vessel_width.as_mut() {
        *width = width.clamp(MIN_VESSEL_WIDTH, MAX_VESSEL_WIDTH);
    }
    ui.ctx().data_mut(|d| d.insert_temp(zoom_id, zoom));
}

/// Draw the plant at a drawn vessel width of `vessel_width` points, returning
/// the canvas it allocated.
fn draw_plant_at(
    ui: &mut Ui,
    snapshot: &HtgrSnapshot,
    tracers: &SchematicTracers,
    vessel_width: f32,
) -> egui::Rect {
    let (min_t, max_t) = (k(DISPLAY_MIN_K), k(DISPLAY_MAX_K));
    let primary = tracers.primary;
    let secondary_train = tracers.secondary;

    // The rods: the operator's command slewed at the drive speed, with the
    // protection system's scram insertion as a floor, exactly as v1 draws them
    // (see that module's section 4 for why the scram is not slewed twice).
    let commanded = snapshot.control_rod_insertion_fraction;
    let slewed = slewed_control_rod_insertion(
        ui.ctx(),
        ui.id().with("htr10_control_rod_bank"),
        commanded,
        ControlRodDrive::htr10(commanded),
    );
    let rods = slewed.max(snapshot.scram_insertion_fraction as f32);

    // Every helium pass carries the primary train, as every v1 helium run
    // does: one loop, one flow, one residence time.
    let reactor = Htr10ReactorSchematic::new(
        Htr10ReactorSchematic::native_size(vessel_width),
        min_t,
        max_t,
        k(snapshot.fuel_temperature_k),
        k(snapshot.core_inlet_temp_k),
        k(snapshot.core_outlet_temp_k),
        k(snapshot.bed_temperature_k),
        k(snapshot.core_inlet_temp_k),
    )
    .with_control_rod_frac(rods)
    .with_bed_height_cm(EQUILIBRIUM_BED_HEIGHT_CM)
    .with_downcomer_tracer(primary)
    .with_riser_tracer(primary)
    .with_plenum_tracer(primary)
    .with_cold_plenum_tracer(primary)
    .with_hot_duct_tracer(primary)
    .with_cold_duct_tracer(primary);

    let (feedwater_temp, condensate_temp) = feed_and_condensate_temps(snapshot);
    let steam_temp = k(snapshot.sg_steam_outlet_temp_k);
    let make_sg = |size| {
        Htr10SteamGeneratorVisual::new(
            size,
            min_t,
            max_t,
            k(snapshot.core_outlet_temp_k),
            k(snapshot.ihx_outlet_temp_k),
            feedwater_temp,
            steam_temp,
        )
        .with_riser_tracer(primary)
        .with_shell_gas_tracer(primary)
        .with_coil_water_tracer(secondary_train)
        .with_feedwater_tracer(secondary_train)
        .with_steam_tracer(secondary_train)
        .with_duct_inlet_tracers(primary, primary)
    };

    let secondary = SecondaryLoopView {
        steam_temp,
        feedwater_temp,
        condensing_temp: condensate_temp,
        exhaust_quality: snapshot.steam_quality_after_turbine,
        cooling_water_inlet_temp: k(COOLING_WATER_INLET_K),
        cooling_water_outlet_temp: k(snapshot.cooling_water_outlet_temp_k),
        mass_flow: MassRate::new::<kilogram_per_second>(snapshot.secondary_mass_flow_kg_per_s),
        pipe_residence_time: Time::new::<second>(snapshot.secondary_residence_time_s),
        turbine_speed: AngularVelocity::new::<radian_per_second>(snapshot.shaft_speed_rad_per_s),
        // No pump shaft-speed model in this plant: drawn stationary, as in v1.
        pump_speed: AngularVelocity::new::<radian_per_second>(0.0),
        simulation_time: Time::new::<second>(snapshot.sim_time_s),
        tracers: SecondaryTracers {
            main_steam: secondary_train,
            exhaust: secondary_train,
            condensate: secondary_train,
            feed: secondary_train,
        },
        min_temp: min_t,
        max_temp: max_t,
    };

    draw_htr10_plant(ui, reactor, make_sg, &secondary)
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::Vec2;

    /// The plant can be panned 40 % of the viewport past each edge, and a
    /// plant too small to fill the viewport even with that margin is centred.
    #[test]
    fn the_pan_margin_is_forty_percent_of_the_viewport_and_centres_small_plants() {
        let viewport = Vec2::new(1000.0, 600.0);

        // A plant that just fills the viewport: 400 x 240 of margin a side.
        let (margin, content) = content_layout(viewport, viewport);
        assert_eq!(margin, Vec2::new(400.0, 240.0));
        assert_eq!(content, Vec2::new(1800.0, 1080.0));

        // A plant much smaller than the viewport sits in the middle of it.
        let plant = Vec2::new(100.0, 60.0);
        let (margin, content) = content_layout(plant, viewport);
        assert_eq!(content, viewport, "no scroll range for a tiny plant");
        assert_eq!(margin + 0.5 * plant, 0.5 * viewport, "centred");
    }
}
