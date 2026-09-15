//! Pump gallery tab: the three pump architectures side by side, on one shaft
//! speed and one temperature.
//!
//! The point of the tab is to watch the **rotating element turn at
//! `theta = omega * t`** while the shaft-speed slider is dragged, and to check
//! that the three silhouettes stay their own shapes when the box they are given
//! is deliberately mis-proportioned. A "box stretch" control exists for exactly
//! that: it squashes and stretches the allocated card, and the artwork should
//! letterbox to its own native aspect rather than shearing with it.
//!
//! ## The clock is owned here, not by the widget
//!
//! Visual components are rebuilt every repaint, so a clock owned by a widget
//! would reset to zero each frame and no pump would ever turn. This tab owns
//! the [`uom::si::f64::Time`] and advances it in [`step`], which the studio
//! calls once per frame from real elapsed time — the same arrangement
//! `studio.rs` already uses for the turbine's `simulation_time` and for the
//! pipe tracers.
//!
//! Stopping the clock **freezes** the impeller where it is, which is what a
//! paused simulation should look like. Dragging the shaft speed to zero is a
//! different thing: it is a genuinely stopped pump, and it must still draw a
//! complete, stationary impeller rather than an empty casing. Both are worth
//! looking at, so both controls are here.
//!
//! **Offline demonstration art.** Shaft speeds and temperatures are round
//! numbers chosen to exercise the drawing; the geometry is illustrative and
//! does not represent any specific pump. Per `RESPONSIBLE_USE.md`, this is for
//! education, research and V&V only.

use egui::{RichText, Vec2};
use outram_park_digital_twin_engine::components::pump::{PumpKind, PumpVisual};
use uom::si::angle::{degree, revolution};
use uom::si::angular_velocity::revolution_per_minute;
use uom::si::f64::{AngularVelocity, ThermodynamicTemperature, Time};
use uom::si::thermodynamic_temperature::degree_celsius;
use uom::si::time::second;
use uom::ConstZero;

/// Studio state for the pump gallery.
pub struct PumpTab {
    /// Shaft speed, revolutions per minute. This is the **physical** shaft
    /// speed handed to the widget; zero draws a stopped pump.
    pub shaft_speed_rpm: f64,
    /// Whether the tab's simulation clock is advancing. Pausing it freezes the
    /// impeller in place; it does not stop the pump.
    pub running: bool,
    /// Elapsed simulation time. Owned here, advanced in [`step`].
    pub simulation_time: Time,
    /// Whether a fluid temperature is known at all.
    ///
    /// Unchecking it passes `None` to the widget, which draws the passages
    /// neutral grey — the honest rendering of "not known", and worth being able
    /// to see next to a graded one.
    pub temperature_known: bool,
    /// Pumped-fluid temperature, degrees Celsius.
    pub fluid_temp_degc: f64,
    /// Cold end of the colour scale, degrees Celsius.
    pub min_temp_degc: f64,
    /// Hot end of the colour scale, degrees Celsius.
    pub max_temp_degc: f64,
    /// Height of one card, in points. Each card's width follows from its
    /// kind's native aspect ratio, times [`PumpTab::box_stretch`].
    pub card_height: f32,
    /// Deliberate distortion of the allocated box, dimensionless.
    ///
    /// `1.0` hands each pump a box already at its native proportions. Anything
    /// else mis-proportions it on purpose, so the letterbox can be checked: the
    /// artwork must keep its own shape and simply sit centred in the wrong-sized
    /// box.
    pub box_stretch: f32,
}

impl Default for PumpTab {
    /// Defaults put the shaft at 1500 rpm — slow enough that the white marker
    /// vane stays readable at a 60 Hz repaint, fast enough that the machine
    /// obviously turns — and the fluid at 290 degC on a 20-400 degC scale,
    /// which is a plausible feedwater band and sits off the middle of the map
    /// so the colour is not accidentally neutral.
    fn default() -> Self {
        Self {
            shaft_speed_rpm: 1500.0,
            running: true,
            simulation_time: Time::ZERO,
            temperature_known: true,
            fluid_temp_degc: 290.0,
            min_temp_degc: 20.0,
            max_temp_degc: 400.0,
            card_height: 300.0,
            box_stretch: 1.0,
        }
    }
}

impl PumpTab {
    /// Advance the tab's simulation clock by `dt`.
    ///
    /// Called once per frame by the studio from real elapsed time, exactly as
    /// `BendDemo::step` and the turbine's `simulation_time` are. Does nothing
    /// while paused, which freezes the impeller rather than snapping it back to
    /// zero phase.
    pub fn step(&mut self, dt: Time) {
        if self.running {
            self.simulation_time += dt;
        }
    }

    /// Restart the clock from zero, leaving the shaft speed alone.
    pub fn reset(&mut self) {
        self.simulation_time = Time::ZERO;
    }

    /// Shaft angular velocity handed to every pump in the gallery.
    pub fn shaft_speed(&self) -> AngularVelocity {
        AngularVelocity::new::<revolution_per_minute>(self.shaft_speed_rpm)
    }

    /// Fluid temperature, or `None` when the caller has declared it unknown.
    pub fn fluid_temperature(&self) -> Option<ThermodynamicTemperature> {
        self.temperature_known.then(|| degc(self.fluid_temp_degc))
    }

    /// Build the widget for one kind, at the given screen box.
    ///
    /// Shared by [`draw`] and the readout so the angle shown and the angle
    /// drawn cannot drift apart.
    pub fn visual(&self, kind: PumpKind, centre: egui::Pos2, size: Vec2) -> PumpVisual {
        PumpVisual::from_scalars(
            kind,
            centre,
            size,
            self.shaft_speed(),
            self.simulation_time,
            self.fluid_temperature(),
            degc(self.min_temp_degc),
            degc(self.max_temp_degc),
        )
    }
}

fn degc(value: f64) -> ThermodynamicTemperature {
    ThermodynamicTemperature::new::<degree_celsius>(value)
}

/// Right-panel controls for the gallery.
pub fn controls(ui: &mut egui::Ui, state: &mut PumpTab) {
    ui.heading("Pumps");
    ui.label(
        RichText::new(
            "Three pump architectures, one shaft speed, one temperature. \
             Illustrative schematic art — not a validated model and not any \
             specific pump design.",
        )
        .small()
        .weak(),
    );
    ui.separator();

    ui.label(RichText::new("Shaft").strong());
    ui.add(
        egui::Slider::new(&mut state.shaft_speed_rpm, -3600.0..=3600.0)
            .text("shaft speed [rpm]")
            .fixed_decimals(0),
    );
    ui.label(
        RichText::new(
            "θ = ω·t, so this really is the shaft speed and not an animation \
             rate. Zero is a STOPPED pump: the impeller must still be drawn, \
             complete and stationary. Negative runs it backwards.",
        )
        .small()
        .weak(),
    );
    ui.horizontal(|ui| {
        if ui
            .button(if state.running {
                "⏸ pause"
            } else {
                "▶ run"
            })
            .clicked()
        {
            state.running = !state.running;
        }
        if ui.button("↺ reset clock").clicked() {
            state.reset();
        }
        if ui.button("0 rpm").clicked() {
            state.shaft_speed_rpm = 0.0;
        }
    });
    ui.label(
        RichText::new(
            "Pausing freezes the clock — the impeller holds its angle. That is \
             a different thing from a stopped pump.",
        )
        .small()
        .weak(),
    );

    ui.separator();
    ui.label(RichText::new("Pumped fluid").strong());
    ui.checkbox(&mut state.temperature_known, "temperature is known");
    ui.add_enabled(
        state.temperature_known,
        egui::Slider::new(&mut state.fluid_temp_degc, 0.0..=800.0).text("fluid [°C]"),
    );
    if !state.temperature_known {
        ui.label(
            RichText::new(
                "Unknown → neutral grey. The widget will not invent a point on \
                 the colour scale.",
            )
            .small()
            .weak(),
        );
    }

    ui.separator();
    ui.label(RichText::new("Colour scale [°C]").strong());
    ui.label(
        RichText::new(
            "Diverging map: blue at min, neutral white at the MIDPOINT, red at \
             max. Set the range about a reference rather than clamping to the \
             extremes seen.",
        )
        .small()
        .weak(),
    );
    ui.add(egui::Slider::new(&mut state.min_temp_degc, -50.0..=400.0).text("min"));
    ui.add(egui::Slider::new(&mut state.max_temp_degc, 100.0..=900.0).text("max"));
    if state.max_temp_degc <= state.min_temp_degc {
        state.max_temp_degc = state.min_temp_degc + 1.0;
    }

    ui.separator();
    ui.label(RichText::new("Layout").strong());
    ui.add(egui::Slider::new(&mut state.card_height, 160.0..=560.0).text("card height [pt]"));
    ui.add(
        egui::Slider::new(&mut state.box_stretch, 0.35..=2.5)
            .text("box stretch")
            .fixed_decimals(2),
    );
    ui.label(
        RichText::new(
            "1.00 gives each pump a box already at its native aspect. Move it \
             and the box is mis-proportioned on purpose: the artwork should \
             letterbox and keep its own shape, not shear.",
        )
        .small()
        .weak(),
    );

    ui.separator();
    ui.label(RichText::new("Readout").strong());
    let omega = state.shaft_speed();
    let angle = PumpVisual::from_scalars(
        PumpKind::Centrifugal,
        egui::Pos2::ZERO,
        Vec2::splat(1.0),
        omega,
        state.simulation_time,
        None,
        degc(0.0),
        degc(1.0),
    )
    .rotor_angle();
    egui::Grid::new("pump_readout")
        .num_columns(2)
        .striped(true)
        .show(ui, |ui| {
            ui.label("simulation time");
            ui.label(format!("{:.2} s", state.simulation_time.get::<second>()));
            ui.end_row();
            ui.label("shaft speed ω");
            ui.label(format!("{:.0} rpm", omega.get::<revolution_per_minute>()));
            ui.end_row();
            ui.label("rotor phase θ = ω·t");
            ui.label(format!(
                "{:.1}° ({:.2} rev)",
                angle.get::<degree>() % 360.0,
                angle.get::<revolution>()
            ));
            ui.end_row();
            ui.label("clock");
            ui.label(if state.running { "running" } else { "paused" });
            ui.end_row();
        });
}

/// Draws every pump kind as a card in one row (wrapping if the panel is
/// narrow).
pub fn draw(ui: &mut egui::Ui, state: &PumpTab) {
    ui.heading("Widget under test");
    ui.label(
        RichText::new(
            "The white vane is the marker — watch it to read speed and \
             direction. Each pump keeps its own proportions inside whatever box \
             it is given.",
        )
        .small()
        .weak(),
    );
    ui.separator();

    egui::ScrollArea::both()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let caption_h = 92.0;
            let gap = 16.0;

            ui.horizontal_top(|ui| {
                for kind in PumpKind::ALL {
                    // Each card is sized from its OWN native aspect, then
                    // deliberately mis-proportioned by `box_stretch` so the
                    // letterbox can be seen doing its job.
                    let h = state.card_height;
                    let w = (h * kind.native_aspect_ratio() * state.box_stretch).clamp(48.0, 640.0);

                    ui.vertical(|ui| {
                        ui.set_width(w);
                        let (rect, _response) =
                            ui.allocate_exact_size(Vec2::new(w, h), egui::Sense::hover());
                        ui.add(state.visual(kind, rect.center(), Vec2::new(w, h)));

                        ui.allocate_ui(Vec2::new(w, caption_h), |ui| {
                            ui.label(RichText::new(kind.label()).strong());
                            ui.label(RichText::new(kind.description()).small().weak());
                            ui.label(
                                RichText::new(format!("service: {}", kind.typical_service()))
                                    .small()
                                    .weak(),
                            );
                            ui.label(
                                RichText::new(format!(
                                    "native aspect {:.3} : 1",
                                    kind.native_aspect_ratio()
                                ))
                                .small()
                                .weak(),
                            );
                        });
                    });
                    ui.add_space(gap);
                }
            });

            ui.separator();
            ui.label(
                RichText::new(
                    "Offline demonstration art. Not for nuclear facility operation, \
                     reactor control, safety-critical decision-making, or licensing.",
                )
                .small()
                .weak(),
            );
        });
}
