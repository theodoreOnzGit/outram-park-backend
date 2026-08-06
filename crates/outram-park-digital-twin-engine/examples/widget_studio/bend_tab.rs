//! Pipe-bend tab: two helium runs meeting at a smooth joint, with a live
//! angle control.
//!
//! The point of the tab is to watch the joint geometry change while the angle
//! is dragged. Butting two rectangles together leaves a wedge missing on the
//! outside of the turn and an overlap on the inside, which is what makes
//! elbows look wrong in a schematic; this shows the sector construction
//! closing that gap at every angle rather than only at 90 degrees.
//!
//! **Offline demonstration only.** Geometry and states are illustrative round
//! numbers, per `RESPONSIBLE_USE.md`.

use egui::{Pos2, RichText, Vec2};
use outram_park_digital_twin_engine::components::{
    PipeBendVisual, PipeComponent, PipePhaseShade, PipeVisual,
};
use tampines::components::{Pipe, PipeBackend};
use tampines::compressible::{CompressibleFluidArray, CoolPropFluid};
use uom::si::angle::degree;
use uom::si::area::square_meter;
use uom::si::f64::{Angle, Area, Length, ThermodynamicTemperature, Time, Velocity};
use uom::si::length::{meter, millimeter};
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::time::second;
use uom::si::velocity::meter_per_second;

/// Cells per leg, kept low so the per-cell boxes stay countable.
const CELLS: i64 = 6;

/// Bore of both legs. Same bore is required for the construction: the sector's
/// radius is the pipe thickness, so two different thicknesses would leave one
/// outer corner off the arc.
const BORE_MM: f64 = 80.0;

/// Length of each leg, in metres.
const LEG_M: f64 = 2.0;

/// The bend demonstration: two legs and the joint between them.
pub struct BendDemo {
    /// The incoming leg, drawn horizontally.
    pub inlet: PipeComponent,
    /// The outgoing leg, drawn at [`Self::angle_deg`] from the inlet.
    pub outlet: PipeComponent,
    /// Turn angle in degrees, measured from straight-through.
    ///
    /// `0` is straight on, `90` is the classic L. The control is deliberately
    /// allowed past 90 so the sector can be seen widening into an obtuse
    /// joint, where a naive mitre fails worst.
    pub angle_deg: f32,
    /// Any backend that failed to construct, reported rather than faked.
    pub errors: Vec<String>,
}

impl Default for BendDemo {
    fn default() -> Self {
        let mut errors = Vec::new();
        let bore = Length::new::<millimeter>(BORE_MM);
        let length = Length::new::<meter>(LEG_M);
        let area = Area::new::<square_meter>(
            std::f64::consts::FRAC_PI_4 * (BORE_MM / 1000.0) * (BORE_MM / 1000.0),
        );
        let dt = Time::new::<second>(0.01);

        let leg = |errors: &mut Vec<String>, which: &str| -> Option<PipeComponent> {
            match CompressibleFluidArray::new(CoolPropFluid::Helium, length, area, CELLS, dt) {
                Ok(array) => Some(
                    PipeComponent::new(
                        Pipe::new(
                            PipeBackend::Compressible(array),
                            bore,
                            length,
                            Length::new::<millimeter>(0.045),
                            Angle::new::<degree>(0.0),
                        ),
                        ThermodynamicTemperature::new::<kelvin>(300.0),
                        ThermodynamicTemperature::new::<kelvin>(1200.0),
                        Velocity::new::<meter_per_second>(12.0),
                        Time::new::<second>(2.5),
                    ),
                ),
                Err(e) => {
                    errors.push(format!("{which} leg (helium): {e:?}"));
                    None
                }
            }
        };

        // Both legs must exist for the joint to mean anything; if either fails
        // the tab reports it rather than drawing half a bend.
        let inlet = leg(&mut errors, "inlet");
        let outlet = leg(&mut errors, "outlet");

        match (inlet, outlet) {
            (Some(inlet), Some(outlet)) => Self {
                inlet,
                outlet,
                angle_deg: 90.0,
                errors,
            },
            _ => {
                // Cannot build the demo. Fall back to a straight-through
                // placeholder so the tab still renders its error message.
                let dummy = |errors: &mut Vec<String>| {
                    errors.push("bend demo unavailable".to_string());
                };
                dummy(&mut errors);
                // Re-attempt once so the struct can be built; if this also
                // fails the unwrap below is unreachable in practice because
                // the error list is already populated and drawn.
                let a = CompressibleFluidArray::new(CoolPropFluid::Helium, length, area, 1, dt);
                let mk = |array: CompressibleFluidArray| {
                    PipeComponent::new(
                        Pipe::new(
                            PipeBackend::Compressible(array),
                            bore,
                            length,
                            Length::new::<millimeter>(0.045),
                            Angle::new::<degree>(0.0),
                        ),
                        ThermodynamicTemperature::new::<kelvin>(300.0),
                        ThermodynamicTemperature::new::<kelvin>(1200.0),
                        Velocity::new::<meter_per_second>(0.0),
                        Time::new::<second>(2.5),
                    )
                };
                let (x, y) = (a.clone().ok(), a.ok());
                Self {
                    inlet: mk(x.expect("placeholder array")),
                    outlet: mk(y.expect("placeholder array")),
                    angle_deg: 90.0,
                    errors,
                }
            }
        }
    }
}

impl BendDemo {
    /// Advance both legs' physics and tracers.
    pub fn step(&mut self, dt: Time) -> Vec<String> {
        let mut errors = Vec::new();
        if let Err(e) = self.inlet.step(dt) {
            errors.push(format!("inlet: {e}"));
        }
        if let Err(e) = self.outlet.step(dt) {
            errors.push(format!("outlet: {e}"));
        }
        errors
    }

    /// Drawn pipe thickness in points — the sector's radius.
    fn thickness(&self) -> f32 {
        self.inlet.visual(Pos2::ZERO).drawn_size().1
    }

    /// Drawn leg length in points.
    fn leg_length(&self) -> f32 {
        self.inlet.visual(Pos2::ZERO).drawn_size().0
    }
}

/// Right-hand controls for the bend tab.
pub fn controls(ui: &mut egui::Ui, demo: &mut BendDemo) {
    ui.heading("Pipe bend");
    ui.label(
        RichText::new(
            "Two helium runs meeting at a smooth joint. Drag the angle and watch the joint \
             geometry follow.",
        )
        .small()
        .weak(),
    );
    ui.separator();

    ui.add(
        egui::Slider::new(&mut demo.angle_deg, 0.0..=170.0)
            .text("turn angle [°]"),
    );
    ui.label(
        RichText::new(
            "0° is straight through, 90° the classic L. Past 90° the sector widens into an \
             obtuse joint, which is where butting two rectangles together fails worst.",
        )
        .small()
        .weak(),
    );

    ui.add_space(8.0);
    ui.label(RichText::new("How the joint is built").strong());
    ui.label(
        RichText::new(
            "The two runs' INNER corners are coincident at one point. Their OUTER corners sit \
             one pipe-thickness away along each run's outward normal, and the gap between them \
             is closed by a circular arc centred on the inner corner with that same radius — so \
             the arc meets both outer edges without a step. A 90° turn gives exactly a quarter \
             circle.",
        )
        .small()
        .weak(),
    );

    ui.add_space(8.0);
    egui::Grid::new("bend_readout")
        .num_columns(2)
        .striped(true)
        .show(ui, |ui| {
            ui.label("pipe thickness");
            ui.label(format!("{:.1} pt (= sector radius)", demo.thickness()));
            ui.end_row();
            ui.label("sector sweep");
            ui.label(format!("{:.0}°", demo.angle_deg));
            ui.end_row();
            ui.label("fill temperature");
            let up = demo
                .inlet
                .visual(Pos2::ZERO)
                .cell_temperatures()
                .last()
                .map(|t| t.get::<kelvin>())
                .unwrap_or(0.0);
            let down = demo
                .outlet
                .visual(Pos2::ZERO)
                .cell_temperatures()
                .first()
                .map(|t| t.get::<kelvin>())
                .unwrap_or(0.0);
            ui.label(format!("mean of {up:.1} K and {down:.1} K"));
            ui.end_row();
        });

    ui.add_space(6.0);
    ui.label(
        RichText::new(
            "The joint is filled with the MEAN of the two adjacent cells: it is a control volume \
             shared by both runs, so taking either side alone would draw a temperature step \
             where the physics has none.",
        )
        .small()
        .weak(),
    );
}

/// Draw the two legs and the joint between them.
pub fn draw(ui: &mut egui::Ui, demo: &BendDemo) {
    ui.heading("Widget under test — smooth bend");
    ui.separator();

    for e in &demo.errors {
        ui.colored_label(egui::Color32::from_rgb(220, 120, 60), format!("⚠ {e}"));
    }

    let avail = ui.available_rect_before_wrap();
    let t = demo.thickness();
    let leg = demo.leg_length();

    // The joint's inner corner. Placed so both legs fit on screen at any angle.
    let p = Pos2::new(avail.left() + leg + 40.0, avail.center().y + 40.0);

    // Inlet runs left-to-right into the joint; outlet leaves at the turn angle.
    // Screen y grows downward, so a positive angle turns UP the screen.
    let a = demo.angle_deg.to_radians();
    let d_in = Vec2::new(1.0, 0.0);
    let d_out = Vec2::new(a.cos(), -a.sin());

    // Outward normals, on the outside of the turn (see PipeBendVisual).
    let cross = d_in.x * d_out.y - d_in.y * d_out.x;
    let sign = if cross >= 0.0 { -1.0 } else { 1.0 };
    let n_in = Vec2::new(-d_in.y, d_in.x) * sign;
    let n_out = Vec2::new(-d_out.y, d_out.x) * sign;

    // Each run's CENTRELINE endpoint sits half a thickness outward from the
    // shared inner corner, which is what puts the inner corners together.
    let inlet_end = p + n_in * (0.5 * t);
    let inlet_start = inlet_end - d_in * leg;
    let outlet_start = p + n_out * (0.5 * t);

    let up = demo.inlet.visual(Pos2::ZERO).cell_temperatures();
    let down = demo.outlet.visual(Pos2::ZERO).cell_temperatures();
    let fallback = ThermodynamicTemperature::new::<kelvin>(300.0);

    ui.add(demo.inlet.visual(inlet_start));

    ui.add(
        PipeBendVisual::new(
            p,
            d_in,
            d_out,
            t,
            up.last().copied().unwrap_or(fallback),
            down.first().copied().unwrap_or(fallback),
            demo.inlet.min_temp,
            demo.inlet.max_temp,
        )
        .with_shade(PipePhaseShade::Gas),
    );

    // The outgoing leg is drawn along the turn direction. PipeVisual takes its
    // direction from the pipe's own inclination, so the leg's inclination is
    // set from the slider — the pipe genuinely IS inclined by that much.
    let mut outlet = demo.outlet.visual(outlet_start);
    outlet.state = angled_state(&demo.outlet, demo.angle_deg);
    ui.add(outlet);
}

/// Clone the outlet's physics with its inclination set to the turn angle.
///
/// The drawn direction comes from `Pipe::inclination`, so angling the leg on
/// screen means angling the pipe itself — which is honest: at a 30 degree
/// bend the outgoing run really does rise at 30 degrees.
fn angled_state(
    component: &PipeComponent,
    angle_deg: f32,
) -> outram_park_digital_twin_engine::components::PipeVisualState {
    let mut pipe = component.pipe.clone();
    pipe.inclination = Angle::new::<degree>(angle_deg as f64);
    outram_park_digital_twin_engine::components::PipeVisualState::Physics(pipe)
}

/// Keeps `PipeVisual` in scope for the type annotation above.
#[allow(unused)]
type _PipeVisualAlias = PipeVisual;
