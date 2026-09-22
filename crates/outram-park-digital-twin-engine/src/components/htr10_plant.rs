//! The whole HTR-10 plant on one canvas: the vessel, the coaxial duct, the
//! steam generator, and the secondary loop (turbine, condenser, feed pump),
//! joined by routed pipework.
//!
//! Moved here on 2026-09-22 from the widget studio's HTR-10 page, so the studio
//! and `htgr_sim_v1` draw the plant from one implementation. The two differ
//! only in where the numbers come from: the studio's display sliders, or the
//! simulator's running model. Presentation only: nothing here computes a
//! physical quantity, it lays out widgets and pipes from values the caller
//! supplies.
//!
//! Every connection runs between ports the widgets report from their own
//! drawing code ([`Htr10ReactorSchematic::duct_port`],
//! [`Htr10SteamGeneratorVisual::gas_port`], `steam_port`, `feedwater_port`,
//! [`TurbineVisual::ports`], [`CondenserVisual::ports`],
//! [`PumpVisual::centrifugal_ports`]), so a pipe lands on the nozzle actually
//! drawn.

use crate::animation::TracerTrain;
use crate::components::pipe_route::{route, PipeStream};
use crate::components::pump::PumpKind;
use crate::components::{
    CondenserDisplayRange, CondenserKind, CondenserScalars, CondenserVisual, Htr10ReactorSchematic,
    Htr10SteamGeneratorVisual, PumpVisual, TurbineFlowPath, TurbineVisual,
};
use egui::{Pos2, Rect, Ui, UiBuilder, Vec2};
use uom::si::f64::{AngularVelocity, MassRate, ThermodynamicTemperature, Time};

/// Horizontal gap between the vessel and the steam generator, as a multiple
/// of the vessel width: the length of duct left visible between them. A
/// drawing choice; no source gives the duct length.
const DUCT_GAP_FRACTION: f32 = 0.6;

/// Secondary-loop pipe thickness, as a fraction of the vessel width. A drawing
/// choice.
const STEAM_PIPE_FRACTION: f32 = 0.035;

/// The four secondary-loop pipe runs' tracer trains, owned and advanced by the
/// application.
#[derive(Debug, Clone, Copy)]
pub struct SecondaryTracers {
    /// Steam generator steam outlet to turbine inlet.
    pub main_steam: TracerTrain,
    /// Turbine exhaust down into the condenser.
    pub exhaust: TracerTrain,
    /// Condensate from the hotwell to the feed pump suction.
    pub condensate: TracerTrain,
    /// Feed pump discharge up to the steam generator feedwater inlet.
    pub feed: TracerTrain,
}

/// The secondary loop's state: everything its widgets and pipes need.
///
/// The caller supplies real values from its own model, or display choices on a
/// GUI test bench. Nothing is derived here.
#[derive(Debug, Clone, Copy)]
pub struct SecondaryLoopView {
    /// Main steam temperature, leaving the steam generator.
    pub steam_temp: ThermodynamicTemperature,
    /// Feedwater temperature, entering the steam generator.
    pub feedwater_temp: ThermodynamicTemperature,
    /// Condenser condensing temperature; also colours the exhaust and the
    /// condensate line.
    pub condensing_temp: ThermodynamicTemperature,
    /// Turbine exhaust steam quality, `[0, 1]`.
    pub exhaust_quality: f64,
    /// Condenser cooling water in and out.
    pub cooling_water_inlet_temp: ThermodynamicTemperature,
    pub cooling_water_outlet_temp: ThermodynamicTemperature,
    /// Secondary loop mass flow; its sign sets the tracer direction.
    pub mass_flow: MassRate,
    /// Residence time along each pipe run; sets the tracer speed.
    pub pipe_residence_time: Time,
    /// Turbine shaft speed; turns the rotor.
    pub turbine_speed: AngularVelocity,
    /// Feed pump shaft speed; turns the impeller. Zero draws it stationary.
    pub pump_speed: AngularVelocity,
    /// Application clock, for the rotor phases (phase = speed x time).
    pub simulation_time: Time,
    /// The pipe runs' tracer trains.
    pub tracers: SecondaryTracers,
    /// Colour scale shared by every widget and pipe on the canvas.
    pub min_temp: ThermodynamicTemperature,
    pub max_temp: ThermodynamicTemperature,
}

/// Draw the whole plant: `reactor` (already built, with its tracers), a steam
/// generator from `make_sg`, and the secondary loop from `secondary`.
///
/// `make_sg` is called with the size the steam generator is drawn at (the
/// vessel's own height scale; both vessels are 11 m on their data sheets) and
/// returns it built with the caller's temperatures and tracers. The layout then
/// connects the duct to it (`with_duct_inlet`), so the duct's three streams
/// bend up inside it at the duct's own band heights.
///
/// Allocates one canvas for everything and draws inside a child area, so the
/// routed pipes and placed widgets cannot push the surrounding layout around.
pub fn draw_htr10_plant(
    ui: &mut Ui,
    reactor: Htr10ReactorSchematic,
    make_sg: impl FnOnce(Vec2) -> Htr10SteamGeneratorVisual,
    secondary: &SecondaryLoopView,
) {
    let reactor_size = reactor.size();
    let vw = reactor.vessel_width();
    let vessel_h = vw / crate::components::htr10_reactor_schematic::DRAWN_ASPECT_RATIO;
    let sg_size = Vec2::new(
        vessel_h * crate::components::htr10_steam_generator::HTR10_SG_ASPECT_RATIO,
        vessel_h,
    );
    let (min_t, max_t) = (secondary.min_temp, secondary.max_temp);

    // ── Primary side: reactor, duct and steam generator ────────────────────
    let reactor_local = Rect::from_min_size(Pos2::ZERO, reactor_size);
    let duct = reactor.duct_port(reactor_local);
    let sg = make_sg(sg_size).with_duct_inlet(duct.outer_height, duct.inner_height);
    let gas = sg.gas_port(Rect::from_min_size(Pos2::ZERO, sg_size));
    let sg_min = Pos2::new(
        reactor_local.right() + DUCT_GAP_FRACTION * vw,
        duct.end.y - gas.y,
    );
    let sg_local = Rect::from_min_size(sg_min, sg_size);
    let extension = (sg_min.x + gas.x) - duct.end.x;

    // ── Secondary side: turbine, condenser, feed pump ──────────────────────
    // Each is placed from its own reported ports, so the pipes land on the
    // nozzles the widgets actually draw. Sizes and gaps are drawing choices.
    let steam_port = sg.steam_port(sg_local);
    let feed_port = sg.feedwater_port(sg_local);

    let turbine_size = Vec2::new(0.8 * vw, 0.3 * vw);
    let turbine_centre = Pos2::new(
        steam_port.x + 0.3 * vw + 0.5 * turbine_size.x,
        steam_port.y + 0.15 * vw + 0.5 * turbine_size.y,
    );
    let turbine_ports =
        TurbineVisual::ports(turbine_centre, turbine_size, TurbineFlowPath::SingleFlow);

    let condenser_kind = CondenserKind::TwoPass;
    let condenser_size = Vec2::new(0.6 * vw, 0.5 * vw);
    let condenser_box = Rect::from_min_size(
        Pos2::new(
            turbine_ports.exhaust_out.x - 0.5 * condenser_size.x,
            turbine_ports.exhaust_out.y + 0.12 * vw,
        ),
        condenser_size,
    );
    let condenser_ports = CondenserVisual::ports(condenser_kind, condenser_box);

    // The pump sits below the feedwater nozzle (its discharge rises to it)
    // and left of the condensate line (its suction faces right).
    let pump_size = Vec2::new(0.32 * vw, 0.32 * vw);
    let probe = PumpVisual::centrifugal_ports(Rect::from_min_size(Pos2::ZERO, pump_size));
    let pump_top = (feed_port.y + 0.10 * vw).max(condenser_ports.condensate_out.y + 0.10 * vw);
    let pump_box = Rect::from_min_size(
        Pos2::new(
            condenser_ports.condensate_out.x - 0.2 * vw - probe.suction.x,
            pump_top,
        ),
        pump_size,
    );
    let pump_ports = PumpVisual::centrifugal_ports(pump_box);

    // Pipe centrelines, corner to corner.
    let main_steam_path = [
        steam_port,
        Pos2::new(turbine_ports.steam_in.x, steam_port.y),
        turbine_ports.steam_in,
    ];
    let exhaust_path = [turbine_ports.exhaust_out, condenser_ports.steam_in];
    let condensate_path = [
        condenser_ports.condensate_out,
        Pos2::new(condenser_ports.condensate_out.x, pump_ports.suction.y),
        pump_ports.suction,
    ];
    let feed_path = [
        pump_ports.discharge,
        Pos2::new(pump_ports.discharge.x, feed_port.y),
        feed_port,
    ];

    // ── Canvas: the bounding box of everything, then shift it into place ───
    let turbine_rect = Rect::from_center_size(turbine_centre, turbine_size);
    let bounds = [
        reactor_local,
        sg_local,
        turbine_rect,
        condenser_box,
        pump_box,
    ]
    .into_iter()
    .reduce(|a, b| a.union(b))
    .expect("five rects");
    let top = bounds.top().min(0.0);
    let (canvas, _response) = ui.allocate_exact_size(
        Vec2::new(bounds.right(), bounds.bottom() - top),
        egui::Sense::hover(),
    );
    let shift = canvas.min.to_vec2() - Vec2::new(0.0, top);
    let at = |p: Pos2| p + shift;
    let mut canvas_ui = ui.new_child(UiBuilder::new().max_rect(canvas));
    let ui = &mut canvas_ui;

    // Pipes first, so each widget's nozzle is painted over the pipe's end.
    let pipe = |temperature, tracer| PipeStream {
        temperature,
        mass_flow: secondary.mass_flow,
        residence_time: secondary.pipe_residence_time,
        thickness: STEAM_PIPE_FRACTION * vw,
        tracer,
        min_temp: min_t,
        max_temp: max_t,
    };
    let condensing = secondary.condensing_temp;
    let t = secondary.tracers;
    for (stream, path) in [
        (pipe(secondary.steam_temp, t.main_steam), &main_steam_path[..]),
        (pipe(condensing, t.exhaust), &exhaust_path[..]),
        (pipe(condensing, t.condensate), &condensate_path[..]),
        (pipe(secondary.feedwater_temp, t.feed), &feed_path[..]),
    ] {
        let path: Vec<Pos2> = path.iter().map(|p| at(*p)).collect();
        route(ui, &stream, &path, false, false);
    }

    // The duct stops at the steam generator's left wall (its gas port);
    // inside, the steam generator draws the bends. Painted first, so the
    // duct's end sits over the wall.
    ui.put(sg_local.translate(shift), sg);
    // The extended reactor is wider (its box includes the duct), but its
    // vessel is anchored at the left of the box, so the origin is unchanged.
    let reactor = reactor.with_duct_extension(extension);
    let reactor_rect = Rect::from_min_size(Pos2::ZERO, reactor.size());
    ui.put(reactor_rect.translate(shift), reactor);

    ui.add(
        TurbineVisual::from_scalars(
            secondary.turbine_speed,
            Some(secondary.steam_temp),
            at(turbine_centre),
            turbine_size,
            min_t,
            max_t,
        )
        .with_flow_path(TurbineFlowPath::SingleFlow)
        .at_time(secondary.simulation_time),
    );
    // No labels on this condenser: on the plant canvas they crowd the loop
    // (maintainer direction, 2026-09-22).
    ui.add(
        CondenserVisual::from_scalars(
            condenser_kind,
            at(condenser_box.center()),
            condenser_size,
            CondenserDisplayRange {
                min_temp: min_t,
                max_temp: max_t,
            },
            CondenserScalars {
                exhaust_quality: secondary.exhaust_quality,
                condensing_temp: condensing,
                condensate_temp: condensing,
                cooling_water_inlet_temp: secondary.cooling_water_inlet_temp,
                cooling_water_outlet_temp: secondary.cooling_water_outlet_temp,
                // No model supplies a hotwell level; `None` draws it hatched
                // rather than inventing one.
                hotwell_level_frac: None,
            },
        )
        .without_labels(),
    );
    ui.add(PumpVisual::from_scalars(
        PumpKind::Centrifugal,
        at(pump_box.center()),
        pump_size,
        secondary.pump_speed,
        secondary.simulation_time,
        Some(secondary.feedwater_temp),
        min_t,
        max_t,
    ));
}
