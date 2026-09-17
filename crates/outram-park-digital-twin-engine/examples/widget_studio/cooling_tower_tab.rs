//! Cooling-tower tab: both draught architectures under live psychrometric
//! controls.
//!
//! A cooling tower's interesting behaviour is **entirely psychrometric**, and
//! none of it varies at a fixed plant operating point — which is exactly why it
//! needs a studio tab rather than a corner of a simulator. The controls here
//! drive the two things that govern the machine:
//!
//! - the **approach to wet bulb**, `T_water,out - T_wb`, which evaporative
//!   cooling can shrink but never cross; and
//! - how far the **exit air** sits into saturation, which is the only thing
//!   that makes a plume visible.
//!
//! Dragging the exit relative humidity from 0.90 up to saturation walks the
//! plume from invisible to full, and dragging the cold-water temperature below
//! the wet bulb produces a **negative** approach — a state a real tower cannot
//! reach, which the readout prints rather than hides.
//!
//! ## The air states are real psychrometry, resolved here
//!
//! Both air states come from `tampines::humid_air::state_from_t_p_r`, a
//! `uom`-typed wrapper over this workspace's CoolProp `HAPropsSI` port
//! (ASHRAE RP-1485). That solve **can fail**, and the tab surfaces the failure
//! on screen instead of substituting a plausible state — the same way the pipes
//! tab surfaces a failed backend.
//!
//! **What actually fails, measured 2026-08-12 rather than assumed.** The port
//! covers the liquid-water branch only, `T > 273.16 K`, and on this call path
//! the restriction bites on the **dry-bulb temperature itself**: 0.02 degC
//! resolves, 0.00 degC returns `OutOfRange`. It does *not* bite on cool dry
//! air — 10 degC at 40 % RH resolves here, even though the CoolProp module's
//! own documentation gives that state as an example of the restriction. Both
//! are correct: that example is about the **dew-point and wet-bulb solves**,
//! and `state_from_t_p_r` never requests either, because
//! [`tampines::humid_air::HumidAirState`] has no field for them (the same gap
//! that makes the wet-bulb temperature a hand-supplied slider below). The air
//! temperature sliders therefore run down to -20 degC so the real boundary is
//! reachable on purpose.
//!
//! ## The clock is owned here, not by the widget
//!
//! The induced-draught fan turns at `theta = omega * t`. Visual components are
//! rebuilt every repaint, so this tab owns the [`uom::si::f64::Time`] and
//! advances it in [`CoolingTowerTab::step`], which the studio calls once per
//! frame — the same arrangement as `PumpTab` and the turbine's
//! `simulation_time`.
//!
//! **Offline demonstration art.** The operating point is a round illustrative
//! set of numbers chosen to exercise the drawing; nothing here is dimensioned
//! from, or represents, any specific cooling tower. Per `RESPONSIBLE_USE.md`
//! this is for education, research and V&V only.

use egui::{Color32, RichText, Vec2};
use outram_park_digital_twin_engine::components::cooling_tower::{
    approach_to_wet_bulb, cooling_range, plume_opacity, CoolingTowerKind, CoolingTowerScalars,
    CoolingTowerVisual, PLUME_VISIBLE_RH_MIN,
};
use tampines::components::CoolingTower;
use tampines::humid_air::{state_from_t_p_r, HumidAirState};
use uom::si::angular_velocity::revolution_per_minute;
use uom::si::f64::{
    AngularVelocity, Pressure, Ratio, TemperatureInterval, ThermodynamicTemperature, Time,
    VolumeRate,
};
use uom::si::pressure::kilopascal;
use uom::si::ratio::{percent, ratio};
use uom::si::temperature_interval::kelvin as kelvin_interval;
use uom::si::thermodynamic_temperature::degree_celsius;
use uom::si::time::second;
use uom::si::volume_rate::cubic_meter_per_second;
use uom::ConstZero;

/// Studio state for the cooling-tower gallery.
pub struct CoolingTowerTab {
    /// Ambient (inlet) air dry-bulb temperature, degrees Celsius.
    pub air_in_dry_bulb_degc: f64,
    /// Ambient (inlet) air relative humidity, dimensionless `[0, 1]`.
    pub air_in_rh: f64,
    /// Exit air dry-bulb temperature, degrees Celsius.
    pub air_out_dry_bulb_degc: f64,
    /// Exit air relative humidity, dimensionless `[0, 1]`.
    ///
    /// **The plume control.** Below [`PLUME_VISIBLE_RH_MIN`] nothing is drawn;
    /// at saturation the plume is at full opacity.
    pub air_out_rh: f64,
    /// Barometric pressure, kilopascals. Both air states are resolved at this
    /// pressure; a tower at altitude runs at a lower one.
    pub barometric_kpa: f64,
    /// Wet-bulb temperature of the entering air, degrees Celsius.
    ///
    /// Supplied by hand because `tampines::humid_air::HumidAirState` carries no
    /// wet-bulb field — see the widget's module documentation and workspace
    /// bead `op-s2es`.
    pub wet_bulb_degc: f64,
    /// Warm water returning to the distribution deck, degrees Celsius.
    pub water_in_degc: f64,
    /// Cooled water leaving the basin, degrees Celsius.
    pub water_out_degc: f64,
    /// Circulating-water volumetric flow, cubic metres per second. Zero draws
    /// no spray and no rain — a tower with nothing circulating is not cooling
    /// anything.
    pub water_flow_m3s: f64,
    /// Target approach handed to the physics-backed card, kelvin. A set-point:
    /// reported as a target, never used to colour anything.
    pub target_approach_k: f64,
    /// Cold end of the colour scale, degrees Celsius.
    pub min_temp_degc: f64,
    /// Hot end of the colour scale, degrees Celsius.
    pub max_temp_degc: f64,
    /// Fan shaft speed, revolutions per minute. Only the induced-draught cell
    /// has a fan; the natural-draught tower ignores it.
    pub fan_speed_rpm: f64,
    /// Whether the tab's simulation clock is advancing. Pausing freezes the fan
    /// where it is; it does not stop the fan.
    pub running: bool,
    /// Elapsed simulation time. Owned here, advanced in
    /// [`CoolingTowerTab::step`].
    pub simulation_time: Time,
    /// Whether to draw the physics-backed card alongside the driven ones.
    pub show_physics_card: bool,
    /// Height of one card, in points. Width follows from each kind's own
    /// aspect ratio.
    pub card_height: f32,
    /// Whether to draw the internal component labels and readouts.
    pub show_labels: bool,
}

impl Default for CoolingTowerTab {
    /// Defaults sit at a warm, humid operating point — 32 degC / 60 % ambient
    /// air, saturated exit air at 38 degC, water cooled 40 to 30 degC against a
    /// 25.4 degC wet bulb, giving a 4.6 K approach and a 10 K range. Deliberately
    /// tropical: a cooler, drier ambient would put the dew point below 273.16 K
    /// and the CoolProp port would (correctly) refuse to resolve it, which is
    /// worth reaching on purpose but is a poor first impression.
    fn default() -> Self {
        Self {
            air_in_dry_bulb_degc: 32.0,
            air_in_rh: 0.60,
            air_out_dry_bulb_degc: 38.0,
            air_out_rh: 1.00,
            barometric_kpa: 101.325,
            wet_bulb_degc: 25.4,
            water_in_degc: 40.0,
            water_out_degc: 30.0,
            water_flow_m3s: 12.0,
            target_approach_k: 4.0,
            min_temp_degc: 0.0,
            max_temp_degc: 100.0,
            fan_speed_rpm: 120.0,
            running: true,
            simulation_time: Time::ZERO,
            show_physics_card: true,
            card_height: 340.0,
            show_labels: true,
        }
    }
}

impl CoolingTowerTab {
    /// Advance the tab's simulation clock by `dt`.
    ///
    /// Called once per frame by the studio from real elapsed time, exactly as
    /// `PumpTab::step` is. Does nothing while paused, which freezes the fan
    /// rather than snapping it back to zero phase.
    pub fn step(&mut self, dt: Time) {
        if self.running {
            self.simulation_time += dt;
        }
    }

    /// Restart the clock from zero, leaving the fan speed alone.
    pub fn reset(&mut self) {
        self.simulation_time = Time::ZERO;
    }

    /// Resolve the inlet air state through the CoolProp-backed psychrometrics.
    ///
    /// Returns the backend's own failure as a message rather than substituting
    /// anything: the port covers the liquid-water branch only, so a cool dry
    /// ambient legitimately has no state on it.
    pub fn air_inlet(&self) -> Result<HumidAirState, String> {
        self.resolve(self.air_in_dry_bulb_degc, self.air_in_rh, "inlet air")
    }

    /// Resolve the exit air state. Same contract as [`Self::air_inlet`].
    pub fn air_outlet(&self) -> Result<HumidAirState, String> {
        self.resolve(self.air_out_dry_bulb_degc, self.air_out_rh, "exit air")
    }

    fn resolve(&self, dry_bulb_degc: f64, rh: f64, what: &str) -> Result<HumidAirState, String> {
        state_from_t_p_r(
            degc(dry_bulb_degc),
            Pressure::new::<kilopascal>(self.barometric_kpa),
            Ratio::new::<ratio>(rh),
        )
        .map_err(|e| {
            format!(
                "{what}: {dry_bulb_degc:.1} °C at {:.0} % RH, {:.3} kPa did not resolve ({e:?}). \
                 The CoolProp port covers the liquid-water branch only, T > 273.16 K \
                 (0.01 °C), so a state at or below the water triple point is refused rather \
                 than extrapolated into the unported ice-sublimation branch.",
                rh * 100.0,
                self.barometric_kpa
            )
        })
    }

    /// The scalars the state-driven cards are drawn from, or the psychrometric
    /// failure that prevented them.
    pub fn scalars(&self) -> Result<CoolingTowerScalars, String> {
        Ok(CoolingTowerScalars {
            air_inlet: self.air_inlet()?,
            air_outlet: self.air_outlet()?,
            inlet_wet_bulb: degc(self.wet_bulb_degc),
            water_inlet_temp: degc(self.water_in_degc),
            water_outlet_temp: degc(self.water_out_degc),
            water_flow_rate: VolumeRate::new::<cubic_meter_per_second>(self.water_flow_m3s),
        })
    }

    /// The physics component the neutral card wraps: a real inlet air state, a
    /// real water inlet temperature and flow, and a **target** approach.
    ///
    /// `CoolingTower::evaluate` is not implemented, so nothing downstream of
    /// the fill exists on it — no exit air, no cold water, no plume.
    pub fn physics(&self) -> Result<CoolingTower, String> {
        Ok(CoolingTower::new(
            self.air_inlet()?,
            degc(self.water_in_degc),
            VolumeRate::new::<cubic_meter_per_second>(self.water_flow_m3s),
            TemperatureInterval::new::<kelvin_interval>(self.target_approach_k),
        ))
    }

    /// Fan shaft angular velocity handed to every tower in the gallery.
    pub fn fan_speed(&self) -> AngularVelocity {
        AngularVelocity::new::<revolution_per_minute>(self.fan_speed_rpm)
    }

    /// A state-driven card for `kind`, at the given screen box.
    pub fn driven(
        &self,
        kind: CoolingTowerKind,
        scalars: CoolingTowerScalars,
        centre: egui::Pos2,
        size: Vec2,
    ) -> CoolingTowerVisual {
        let visual = CoolingTowerVisual::from_scalars(
            kind,
            centre,
            size,
            degc(self.min_temp_degc),
            degc(self.max_temp_degc),
            scalars,
        )
        .with_fan_speed(self.fan_speed())
        .at_time(self.simulation_time);
        if self.show_labels {
            visual
        } else {
            visual.without_labels()
        }
    }

    /// The physics-backed card, at the given screen box.
    ///
    /// Built through the preserved five-argument [`CoolingTowerVisual::new`],
    /// so this is exactly what any existing call site gets.
    pub fn neutral(
        &self,
        physics: CoolingTower,
        kind: CoolingTowerKind,
        centre: egui::Pos2,
        size: Vec2,
    ) -> CoolingTowerVisual {
        let visual = CoolingTowerVisual::new(
            physics,
            centre,
            size,
            degc(self.min_temp_degc),
            degc(self.max_temp_degc),
        )
        .with_kind(kind)
        .with_fan_speed(self.fan_speed())
        .at_time(self.simulation_time);
        if self.show_labels {
            visual
        } else {
            visual.without_labels()
        }
    }

    /// The achieved approach implied by the sliders.
    pub fn approach(&self) -> TemperatureInterval {
        approach_to_wet_bulb(degc(self.water_out_degc), degc(self.wet_bulb_degc))
    }

    /// The cooling range implied by the sliders.
    pub fn range(&self) -> TemperatureInterval {
        cooling_range(degc(self.water_in_degc), degc(self.water_out_degc))
    }
}

/// Vertical space reserved under each card for its caption, in points.
const CAPTION_H: f32 = 92.0;

/// Gap between cards, in points.
const GAP: f32 = 16.0;

fn degc(value: f64) -> ThermodynamicTemperature {
    ThermodynamicTemperature::new::<degree_celsius>(value)
}

/// Right-panel controls for the gallery.
pub fn controls(ui: &mut egui::Ui, state: &mut CoolingTowerTab) {
    ui.heading("Cooling towers");
    ui.label(
        RichText::new(
            "Natural-draught hyperbolic and mechanical induced-draught, on one \
             set of psychrometric conditions. Illustrative schematic art — not \
             a validated model and not any specific tower design.",
        )
        .small()
        .weak(),
    );
    ui.separator();

    ui.label(RichText::new("Inlet air").strong());
    ui.add(egui::Slider::new(&mut state.air_in_dry_bulb_degc, -20.0..=50.0).text("dry bulb [°C]"));
    ui.add(
        egui::Slider::new(&mut state.air_in_rh, 0.05..=1.0)
            .text("relative humidity")
            .fixed_decimals(2),
    );
    ui.add(
        egui::Slider::new(&mut state.barometric_kpa, 70.0..=105.0)
            .text("barometric [kPa]")
            .fixed_decimals(3),
    );
    ui.label(
        RichText::new(
            "Resolved through tampines::humid_air (the CoolProp HAPropsSI port, \
             ASHRAE RP-1485). The port covers the liquid-water branch only, so a \
             dry bulb at or below 0.01 °C is REFUSED — drag either air \
             temperature below freezing to see the failure reported on the \
             canvas rather than papered over.",
        )
        .small()
        .weak(),
    );

    ui.separator();
    ui.label(RichText::new("Exit air — the plume control").strong());
    ui.add(egui::Slider::new(&mut state.air_out_dry_bulb_degc, -20.0..=60.0).text("dry bulb [°C]"));
    ui.add(
        egui::Slider::new(&mut state.air_out_rh, 0.05..=1.0)
            .text("relative humidity")
            .fixed_decimals(3),
    );
    ui.horizontal(|ui| {
        if ui.button("no plume (0.85)").clicked() {
            state.air_out_rh = 0.85;
        }
        if ui.button("threshold (0.90)").clicked() {
            state.air_out_rh = PLUME_VISIBLE_RH_MIN as f64;
        }
        if ui.button("half (0.95)").clicked() {
            state.air_out_rh = 0.95;
        }
        if ui.button("saturated (1.00)").clicked() {
            state.air_out_rh = 1.0;
        }
    });
    ui.label(
        RichText::new(
            "A plume is condensed water, so it needs the exit air at or very \
             near saturation. Opacity ramps from 0 at RH 0.90 to full at RH \
             1.00. That is a DISPLAY mapping of a supplied property, not a \
             plume-formation calculation.",
        )
        .small()
        .weak(),
    );

    ui.separator();
    ui.label(RichText::new("Water side").strong());
    ui.add(egui::Slider::new(&mut state.wet_bulb_degc, 0.0..=40.0).text("inlet wet bulb [°C]"));
    ui.add(egui::Slider::new(&mut state.water_in_degc, 10.0..=70.0).text("water in (deck) [°C]"));
    ui.add(egui::Slider::new(&mut state.water_out_degc, 5.0..=60.0).text("water out (basin) [°C]"));
    ui.add(
        egui::Slider::new(&mut state.water_flow_m3s, 0.0..=40.0)
            .text("circulating flow [m³/s]")
            .fixed_decimals(1),
    );
    if state.water_flow_m3s == 0.0 {
        ui.label(
            RichText::new(
                "Zero flow: no spray and no rain are drawn. A tower with nothing \
                 circulating is not cooling anything.",
            )
            .small()
            .weak(),
        );
    }
    ui.label(
        RichText::new(
            "Evaporation drives the water towards the wet bulb and cannot pass \
             it. Drag the basin temperature below the wet bulb and the approach \
             goes negative — a state no real tower reaches, printed rather than \
             clamped away.",
        )
        .small()
        .weak(),
    );

    ui.separator();
    ui.label(RichText::new("Fan").strong());
    ui.add(
        egui::Slider::new(&mut state.fan_speed_rpm, -300.0..=300.0)
            .text("fan speed [rpm]")
            .fixed_decimals(0),
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
            state.fan_speed_rpm = 0.0;
        }
    });
    ui.label(
        RichText::new(
            "θ = ω·t on the studio's own clock. Only the induced-draught cell \
             has a fan; the natural-draught tower ignores this slider entirely, \
             because its draught is buoyancy.",
        )
        .small()
        .weak(),
    );

    ui.separator();
    ui.label(RichText::new("Colour scale [°C]").strong());
    ui.add(egui::Slider::new(&mut state.min_temp_degc, -20.0..=40.0).text("min"));
    ui.add(egui::Slider::new(&mut state.max_temp_degc, 20.0..=200.0).text("max"));
    if state.max_temp_degc <= state.min_temp_degc {
        state.max_temp_degc = state.min_temp_degc + 1.0;
    }

    ui.separator();
    ui.label(RichText::new("Physics-backed card").strong());
    ui.checkbox(&mut state.show_physics_card, "show CoolingTowerVisual::new");
    ui.add(
        egui::Slider::new(&mut state.target_approach_k, 0.5..=15.0)
            .text("target approach [K]")
            .fixed_decimals(1),
    );
    ui.label(
        RichText::new(
            "tampines::components::CoolingTower holds a real inlet air state, \
             water inlet temperature and flow, plus this TARGET approach — a \
             set-point. Its evaluate() is unimplemented, so that card has no \
             exit air, no cold water and NO PLUME.",
        )
        .small()
        .weak(),
    );

    ui.separator();
    ui.label(RichText::new("Layout").strong());
    ui.add(egui::Slider::new(&mut state.card_height, 200.0..=620.0).text("card height [pt]"));
    ui.checkbox(&mut state.show_labels, "show internal labels");

    ui.separator();
    ui.label(RichText::new("What drives what").strong());
    egui::Grid::new("cooling_tower_readout")
        .num_columns(2)
        .striped(true)
        .show(ui, |ui| {
            let approach = state.approach().get::<kelvin_interval>();
            ui.label("approach to wet bulb");
            if approach > 0.0 {
                ui.label(format!("{approach:.2} K"));
            } else {
                ui.label(
                    RichText::new(format!("{approach:.2} K — below wet bulb, check the model"))
                        .italics()
                        .color(Color32::from_rgb(220, 80, 60)),
                );
            }
            ui.end_row();

            ui.label("cooling range");
            ui.label(format!("{:.2} K", state.range().get::<kelvin_interval>()));
            ui.end_row();

            ui.label("plume opacity");
            let opacity = plume_opacity(Ratio::new::<ratio>(state.air_out_rh));
            if opacity > 0.0 {
                ui.label(format!("{opacity:.2} (exit RH {:.3})", state.air_out_rh));
            } else {
                ui.label(
                    RichText::new(format!(
                        "0.00 — exit RH {:.3} is below {PLUME_VISIBLE_RH_MIN:.2}, no plume",
                        state.air_out_rh
                    ))
                    .italics()
                    .color(Color32::from_rgb(200, 140, 60)),
                );
            }
            ui.end_row();

            // Read off the widget itself, so the angle reported and the angle
            // drawn cannot drift apart. Only available when the psychrometrics
            // resolved — the card does not exist otherwise.
            ui.label("fan phase θ = ω·t");
            match state.scalars().ok().and_then(|scalars| {
                state
                    .driven(
                        CoolingTowerKind::InducedDraught,
                        scalars,
                        egui::Pos2::ZERO,
                        Vec2::splat(1.0),
                    )
                    .fan_angle()
            }) {
                Some(a) => ui.label(format!("{:.1}°", a.get::<uom::si::angle::degree>() % 360.0)),
                None => ui.label("n/a — no card drawn"),
            };
            ui.end_row();

            ui.label("simulation time");
            ui.label(format!(
                "{:.2} s ({})",
                state.simulation_time.get::<second>(),
                if state.running { "running" } else { "paused" }
            ));
            ui.end_row();

            match state.air_inlet() {
                Ok(air) => {
                    ui.label("inlet air (CoolProp)");
                    ui.label(format!(
                        "W {:.5} kg/kg · RH {:.0} %",
                        air.humidity_ratio.get::<ratio>(),
                        air.relative_humidity.get::<percent>()
                    ));
                }
                Err(_) => {
                    ui.label("inlet air (CoolProp)");
                    ui.label(
                        RichText::new("did not resolve — see canvas")
                            .italics()
                            .color(Color32::from_rgb(220, 80, 60)),
                    );
                }
            }
            ui.end_row();

            match state.air_outlet() {
                Ok(air) => {
                    ui.label("exit air (CoolProp)");
                    ui.label(format!(
                        "W {:.5} kg/kg · RH {:.0} %",
                        air.humidity_ratio.get::<ratio>(),
                        air.relative_humidity.get::<percent>()
                    ));
                }
                Err(_) => {
                    ui.label("exit air (CoolProp)");
                    ui.label(
                        RichText::new("did not resolve — see canvas")
                            .italics()
                            .color(Color32::from_rgb(220, 80, 60)),
                    );
                }
            }
            ui.end_row();
        });
}

/// Draws both draught architectures, plus the physics-backed card.
pub fn draw(ui: &mut egui::Ui, state: &CoolingTowerTab) {
    ui.heading("Widget under test");
    ui.label(
        RichText::new(
            "Warm water is sprayed over the fill and falls through rising air. \
             Watch the plume as the exit air approaches saturation, and the \
             approach readout as the basin temperature nears the wet bulb.",
        )
        .small()
        .weak(),
    );
    ui.separator();

    egui::ScrollArea::both()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            match state.scalars() {
                Ok(scalars) => {
                    ui.label(
                        RichText::new("State-driven — CoolingTowerVisual::from_scalars").strong(),
                    );
                    ui.horizontal_top(|ui| {
                        for kind in CoolingTowerKind::ALL {
                            card(
                                ui,
                                state,
                                |ui, rect, size| {
                                    ui.put(rect, state.driven(*kind, scalars, rect.center(), size));
                                },
                                *kind,
                                true,
                            );
                        }
                    });
                }
                Err(message) => {
                    ui.colored_label(
                        Color32::from_rgb(220, 80, 60),
                        format!("⚠ psychrometric state unavailable — {message}"),
                    );
                    ui.label(
                        RichText::new(
                            "No state-driven card is drawn. Substituting a nearby state that DOES \
                             resolve would put a number on screen that the backend refused to \
                             produce.",
                        )
                        .small()
                        .weak(),
                    );
                }
            }

            if state.show_physics_card {
                ui.add_space(GAP);
                ui.separator();
                ui.label(RichText::new("Physics-backed — CoolingTowerVisual::new").strong());
                ui.label(
                    RichText::new(
                        "Real inlet air, water inlet temperature and flow; a TARGET approach \
                         set-point. CoolingTower::evaluate is unimplemented, so there is no exit \
                         air state and no cold water — the basin stays grey, the approach reads \
                         'not evaluated', and no plume is drawn. This is the honest rendering.",
                    )
                    .small()
                    .weak(),
                );
                match state.physics() {
                    Ok(physics) => {
                        ui.horizontal_top(|ui| {
                            for kind in CoolingTowerKind::ALL {
                                card(
                                    ui,
                                    state,
                                    |ui, rect, size| {
                                        ui.put(
                                            rect,
                                            state.neutral(physics, *kind, rect.center(), size),
                                        );
                                    },
                                    *kind,
                                    false,
                                );
                            }
                        });
                    }
                    Err(message) => {
                        ui.colored_label(
                            Color32::from_rgb(220, 80, 60),
                            format!("⚠ inlet air unavailable — {message}"),
                        );
                    }
                }
            }

            ui.add_space(GAP);
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

/// Lays out one card — box, artwork, caption — for `kind`.
///
/// `paint` receives the reserved rectangle and the size to hand the widget, so
/// the state-driven and physics-backed rows share one layout and cannot drift
/// apart.
fn card(
    ui: &mut egui::Ui,
    state: &CoolingTowerTab,
    paint: impl FnOnce(&mut egui::Ui, egui::Rect, Vec2),
    kind: CoolingTowerKind,
    driven: bool,
) {
    let h = state.card_height;
    let w = (h * kind.native_aspect_ratio()).clamp(80.0, 720.0);

    ui.vertical(|ui| {
        ui.set_width(w);
        let (rect, _response) = ui.allocate_exact_size(Vec2::new(w, h), egui::Sense::hover());
        paint(ui, rect, Vec2::new(w, h));

        ui.allocate_ui(Vec2::new(w, CAPTION_H), |ui| {
            ui.label(RichText::new(kind.label()).strong());
            ui.label(RichText::new(kind.description()).small().weak());
            ui.label(
                RichText::new(format!("draught: {}", kind.draught()))
                    .small()
                    .weak(),
            );
            if driven {
                let opacity = plume_opacity(Ratio::new::<ratio>(state.air_out_rh));
                ui.label(
                    RichText::new(format!(
                        "plume {opacity:.2} · approach {:.1} K",
                        state.approach().get::<kelvin_interval>()
                    ))
                    .small()
                    .weak(),
                );
            } else {
                ui.label(
                    RichText::new("no exit air — no plume, no approach")
                        .small()
                        .italics()
                        .color(Color32::from_rgb(200, 140, 60)),
                );
            }
        });
    });
    ui.add_space(GAP);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The exit-humidity slider must walk the plume across its whole visible
    /// range, and the tab's readout must agree with what the widget draws.
    ///
    /// **Methodology.** The plume is the tab's headline behaviour, so sweep
    /// [`CoolingTowerTab::air_out_rh`] from 0.50 to 1.00 in steps of 0.01,
    /// resolve the exit air state through the CoolProp port at each step, and
    /// require the opacity computed from the **resolved state's** relative
    /// humidity to be zero at and below [`PLUME_VISIBLE_RH_MIN`], to rise
    /// monotonically above it, and to reach 1.0 at saturation. Comparing
    /// against the resolved state, not against the slider value, is the point:
    /// it checks the number that actually reaches the widget.
    ///
    /// **Result (2026-08-12):** 51 sampled humidities at 38.0 degC and
    /// 101.325 kPa, all resolving; opacity 0.00 for every RH at or below 0.90,
    /// 0.50 at RH 0.95 and 1.00 at RH 1.00, monotonic throughout, and the
    /// resolved relative humidity matched the slider to better than 1e-6 at
    /// every step. Interpretation: dragging the slider genuinely walks the
    /// plume from invisible to full, and the studio readout cannot disagree
    /// with the drawing.
    #[test]
    fn the_exit_humidity_slider_walks_the_plume_from_invisible_to_full() {
        let mut tab = CoolingTowerTab::default();
        let mut previous = 0.0f32;
        let mut sampled = 0usize;
        for step in 50..=100 {
            tab.air_out_rh = step as f64 / 100.0;
            let air = tab
                .air_outlet()
                .expect("38 degC exit air resolves at every humidity tested");
            assert!(
                (air.relative_humidity.get::<ratio>() - tab.air_out_rh).abs() < 1e-6,
                "the resolved state did not carry the requested humidity"
            );
            let opacity = plume_opacity(air.relative_humidity);
            assert!(
                opacity >= previous - 1e-6,
                "opacity fell at RH {}",
                tab.air_out_rh
            );
            if tab.air_out_rh <= PLUME_VISIBLE_RH_MIN as f64 {
                assert_eq!(opacity, 0.0, "a plume appeared at RH {}", tab.air_out_rh);
            }
            previous = opacity;
            sampled += 1;
        }
        println!("{sampled} exit humidities swept");
        assert_eq!(previous, 1.0, "saturated exit air must plume fully");

        tab.air_out_rh = 0.95;
        let half = plume_opacity(tab.air_outlet().unwrap().relative_humidity);
        println!("RH 0.95 -> opacity {half:.4}");
        assert!((half - 0.5).abs() < 1e-3);
    }

    /// A psychrometric state the backend refuses must be reported, never
    /// substituted — and the boundary must be **where it actually is**, not
    /// where it was assumed to be.
    ///
    /// **Methodology.** The CoolProp port covers the liquid-water branch only,
    /// `T > 273.16 K`. Where that bites on *this* call path was measured rather
    /// than assumed: probe [`CoolingTowerTab::air_inlet`] across a spread of
    /// dry-bulb temperatures, humidities and barometric pressures reachable
    /// from the sliders, and record which resolve. Then require the failing
    /// side to propagate through [`CoolingTowerTab::scalars`] *and*
    /// [`CoolingTowerTab::physics`] with a message naming the input and the
    /// reason, and require the resolvable side to succeed.
    ///
    /// **Result (2026-08-12):** the restriction bites on the **dry-bulb
    /// temperature itself** — 0.02 degC resolved (W = 0.001513 kg/kg) and
    /// 0.00 degC returned `OutOfRange`, as did -5 and -20 degC. It did **not**
    /// bite on cool dry air: 10 degC at 40 % RH resolved fine
    /// (W = 0.003043 kg/kg), even though the CoolProp module's own docs give
    /// that state as an example of the restriction — correctly, because that
    /// example concerns the dew-point and wet-bulb solves and
    /// `state_from_t_p_r` requests neither. Low pressure (70 kPa), bone-dry
    /// air (5 % RH) and hot saturated air (60 degC, RH 1.0) all resolved.
    /// `scalars()` and `physics()` both propagated the failure, and the message
    /// named "inlet air" and the liquid-water restriction. Interpretation: the
    /// canvas draws no card below the triple point, so a state the backend
    /// would not produce never appears on screen — and the studio's slider
    /// range reaches that boundary deliberately.
    #[test]
    fn an_unresolvable_air_state_is_reported_not_substituted() {
        // Measured boundary, not assumed: everything above the water triple
        // point resolves, everything at or below it is refused.
        for (dry_bulb, rh, barometric, expect_ok) in [
            (10.0, 0.40, 101.325, true),
            (5.0, 0.20, 101.325, true),
            (0.02, 0.40, 101.325, true),
            (0.0, 0.40, 101.325, false),
            (-5.0, 0.40, 101.325, false),
            (-20.0, 0.40, 101.325, false),
            (32.0, 0.60, 70.0, true),
            (32.0, 0.05, 101.325, true),
            (60.0, 1.0, 101.325, true),
        ] {
            let mut tab = CoolingTowerTab::default();
            tab.air_in_dry_bulb_degc = dry_bulb;
            tab.air_in_rh = rh;
            tab.barometric_kpa = barometric;
            let resolved = tab.air_inlet();
            println!(
                "{dry_bulb} °C / {rh} RH / {barometric} kPa -> {}",
                if resolved.is_ok() { "ok" } else { "refused" }
            );
            assert_eq!(
                resolved.is_ok(),
                expect_ok,
                "{dry_bulb} °C / {rh} RH / {barometric} kPa"
            );
        }

        // The refusal must propagate, with a message a reader can act on.
        let mut tab = CoolingTowerTab::default();
        tab.air_in_dry_bulb_degc = -5.0;
        let failure = tab
            .scalars()
            .expect_err("a state below the water triple point must not resolve");
        println!("{failure}");
        assert!(
            failure.contains("inlet air"),
            "the message must name the input"
        );
        assert!(
            failure.contains("liquid-water branch"),
            "the message must give the reason"
        );
        assert!(
            tab.physics().is_err(),
            "no component may be built around a fabricated state"
        );

        let ok = CoolingTowerTab::default();
        assert!(ok.scalars().is_ok());
        assert!(ok.physics().is_ok());
    }

    /// The approach and range readouts must be the real differences of the
    /// slider values, including when the model is driven somewhere impossible.
    ///
    /// **Methodology.** Evaporative cooling cannot take the water below the
    /// wet-bulb temperature, so a basin temperature under it is an
    /// out-of-range model, not a very good tower. Require the tab's approach
    /// and range to match hand-computed values at the default operating point,
    /// then drag the basin 1 K below the wet bulb and require a negative
    /// approach to be reported rather than clamped.
    ///
    /// **Result (2026-08-12):** at 40 -> 30 degC water against a 25.4 degC wet
    /// bulb, approach 4.600 K and range 10.000 K; with the basin at 24.4 degC
    /// the approach read -1.000 K. Interpretation: the studio can be driven
    /// into a physically impossible state on purpose, and it says so.
    #[test]
    fn the_approach_readout_reports_impossible_states_rather_than_hiding_them() {
        let mut tab = CoolingTowerTab::default();
        assert!((tab.approach().get::<kelvin_interval>() - 4.6).abs() < 1e-9);
        assert!((tab.range().get::<kelvin_interval>() - 10.0).abs() < 1e-9);

        tab.water_out_degc = tab.wet_bulb_degc - 1.0;
        let impossible = tab.approach().get::<kelvin_interval>();
        println!("basin 1 K under the wet bulb -> approach {impossible:.3} K");
        assert!((impossible + 1.0).abs() < 1e-9);
    }

    /// The clock must be owned by the tab, so the fan turns across repaints and
    /// freezes when the studio is paused.
    ///
    /// **Methodology.** Widgets are rebuilt every frame, so a widget-owned
    /// clock would reset the fan phase to zero each repaint. Advance
    /// [`CoolingTowerTab::step`] in 0.1 s increments and require the elapsed
    /// time to accumulate; pause and require it to hold; reset and require it
    /// to return to zero. Then require the induced-draught card to report a
    /// non-zero fan angle at a non-zero speed, and the natural-draught card to
    /// report none at any speed.
    ///
    /// **Result (2026-08-12):** 10 steps of 0.1 s gave 1.000 s; a further 5
    /// steps while paused left it at 1.000 s; reset gave 0.000 s. At 120 rpm
    /// and 1.0 s the induced-draught fan angle was 12.566 rad (2.00 rev) and
    /// the natural-draught tower reported `None`. Interpretation: the fan
    /// animation survives being rebuilt every frame, and a tower with no fan
    /// cannot be made to grow one.
    #[test]
    fn the_tab_owns_the_clock_that_turns_the_fan() {
        let mut tab = CoolingTowerTab::default();
        for _ in 0..10 {
            tab.step(Time::new::<second>(0.1));
        }
        assert!((tab.simulation_time.get::<second>() - 1.0).abs() < 1e-9);

        tab.running = false;
        for _ in 0..5 {
            tab.step(Time::new::<second>(0.1));
        }
        assert!(
            (tab.simulation_time.get::<second>() - 1.0).abs() < 1e-9,
            "a paused clock must hold"
        );

        let scalars = tab.scalars().expect("the default state resolves");
        let induced = tab
            .driven(
                CoolingTowerKind::InducedDraught,
                scalars,
                egui::Pos2::ZERO,
                Vec2::splat(100.0),
            )
            .fan_angle()
            .expect("an induced-draught cell has a fan");
        println!(
            "120 rpm for 1 s -> {:.3} rad",
            induced.get::<uom::si::angle::radian>()
        );
        assert!((induced.get::<uom::si::angle::revolution>() - 2.0).abs() < 1e-9);

        assert!(tab
            .driven(
                CoolingTowerKind::NaturalDraught,
                scalars,
                egui::Pos2::ZERO,
                Vec2::splat(100.0),
            )
            .fan_angle()
            .is_none());

        tab.reset();
        assert_eq!(tab.simulation_time, Time::ZERO);
    }

    /// The physics-backed card must carry the component and none of the
    /// state-driven sliders.
    #[test]
    fn the_physics_card_reports_only_what_the_component_holds() {
        let tab = CoolingTowerTab::default();
        let physics = tab.physics().expect("the default inlet air resolves");
        let card = tab.neutral(
            physics,
            CoolingTowerKind::NaturalDraught,
            egui::Pos2::ZERO,
            Vec2::splat(100.0),
        );
        assert!(card.scalars().is_none(), "no scalars on the physics path");
        assert_eq!(card.physics(), Some(physics));
        assert_eq!(card.approach(), None, "evaluate() is unimplemented");
        assert_eq!(card.cooling_range(), None);
        assert_eq!(
            card.target_approach(),
            Some(TemperatureInterval::new::<kelvin_interval>(
                tab.target_approach_k
            ))
        );
    }
}
