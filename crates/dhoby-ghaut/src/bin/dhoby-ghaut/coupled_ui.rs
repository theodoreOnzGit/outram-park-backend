//! Steps 9 and 10 in the window: the case setup panel, the run panel, the
//! r-z field in the main view, and the coupling console and plots.
//!
//! Everything this file computes is drawing; the run is on its own thread
//! (`coupled.rs`). The one exception is the Step 9 power-shape preview, a
//! closed-form evaluation (no march) redone only when a setting changes.

use dhoby_ghaut::web_demo::link::{start_native, Link};
use dhoby_ghaut::workbench::multiphysics::{FeedbackWeighting, MultiphysicsSetup, PowerShape};
use dhoby_ghaut::workbench::recipe::{Citation, ElementStatus};
use egui::{Color32, Pos2, Rect, RichText, Stroke, Vec2};
use egui_plot::{Legend, Line, Plot, PlotPoints};

use crate::app::App;
use crate::coupled::{MpEngine, MpEv, MpReq, StopFlag};
use crate::porous_core::{Fields, IterationReport, Summary};

/// Which field the main view shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldKind {
    Power,
    Helium,
    Surface,
    Centre,
    Kernel,
}

impl FieldKind {
    pub const ALL: [Self; 5] = [
        Self::Power,
        Self::Helium,
        Self::Surface,
        Self::Centre,
        Self::Kernel,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Power => "power density [W/cm³]",
            Self::Helium => "helium [°C]",
            Self::Surface => "fuel-pebble surface [°C]",
            Self::Centre => "pebble centre [°C]",
            Self::Kernel => "hottest kernel [°C]",
        }
    }

    fn values(self, f: &Fields) -> Vec<f64> {
        let c = |v: &[f64]| v.iter().map(|t| t - 273.15).collect();
        match self {
            Self::Power => f.q_w_m3.iter().map(|q| q * 1e-6).collect(),
            Self::Helium => c(&f.t_helium_k),
            Self::Surface => c(&f.t_surface_k),
            Self::Centre => c(&f.t_centre_k),
            Self::Kernel => c(&f.t_kernel_k),
        }
    }
}

/// Steps 9 and 10's state.
pub struct MpUi {
    pub link: Link<MpReq, MpEv>,
    /// The case (Step 9), saved in the recipe as `step-9`.
    pub setup: MultiphysicsSetup,
    /// The HTR-10 prefill, for "edited" and reset.
    pub preset: MultiphysicsSetup,
    pub running: bool,
    pub stop: StopFlag,
    pub history: Vec<IterationReport>,
    pub fields: Option<Fields>,
    pub summary: Option<Summary>,
    pub console: Vec<String>,
    pub field: FieldKind,
    pub zoom: f32,
    pub started_at: f64,
    pub elapsed: f64,
    /// The power shape of the current setup (Step 9 preview).
    preview: Option<(MultiphysicsSetup, Result<Fields, String>)>,
}

impl MpUi {
    pub fn new(ctx: egui::Context, reference_temperature_k: f64) -> Self {
        let link = start_native(MpEngine, move || ctx.request_repaint());
        let preset = crate::mp_preset::htr10(reference_temperature_k);
        Self {
            link,
            setup: preset.clone(),
            preset,
            running: false,
            stop: Default::default(),
            history: Vec::new(),
            fields: None,
            summary: None,
            console: Vec::new(),
            field: FieldKind::Kernel,
            zoom: 1.0,
            started_at: 0.0,
            elapsed: 0.0,
            preview: None,
        }
    }

    pub fn start(&mut self, now: f64) {
        if let Ok(mut s) = self.stop.write() {
            *s = false;
        }
        self.running = true;
        self.history.clear();
        self.summary = None;
        self.console = vec![
            "Step 10 coupled run, HTR-10 porous core (TENTATIVE).".into(),
            "SIMPLIFIED: prescribed power shape, lumped feedback, r-z rings without inter-ring conduction; see the element list.".into(),
            " iter  flow resid   max dT [K]  T_he,max [C]  T_kernel,max [C]  rho [pcm]  k(cold-crit)".into(),
        ];
        self.started_at = now;
        self.link.send(MpReq::Run {
            setup: self.setup.clone(),
            stop: self.stop.clone(),
        });
    }

    pub fn request_stop(&mut self) {
        if let Ok(mut s) = self.stop.write() {
            *s = true;
        }
    }

    /// Drain the engine. Returns a status message for the app's status line.
    pub fn handle(&mut self) -> Vec<(String, bool)> {
        let mut said = Vec::new();
        for ev in self.link.drain() {
            match ev {
                MpEv::Started { delta_note } => self.console.push(delta_note),
                MpEv::Iteration {
                    report: r,
                    fields,
                    seconds,
                } => {
                    self.console.push(format!(
                        "{:5}  {:10.3e}  {:11.4}  {:12.2}  {:16.2}  {:9.1}  {:.5}",
                        r.iteration,
                        r.flow_residual,
                        r.temperature_change_k,
                        r.max_helium_k - 273.15,
                        r.max_kernel_k - 273.15,
                        r.rho_pcm,
                        r.k_vs_cold_critical
                    ));
                    self.history.push(r);
                    self.fields = Some(fields);
                    self.elapsed = seconds;
                }
                MpEv::Done {
                    summary,
                    fields,
                    stopped,
                    seconds,
                } => {
                    self.running = false;
                    self.elapsed = seconds;
                    if stopped {
                        self.console
                            .push("STOPPED by the user before convergence.".into());
                    }
                    self.console
                        .extend(crate::mp_headless::summary_lines(&summary));
                    said.push((
                        format!(
                            "Coupled run {} in {:.1} s: peak fuel {:.0} °C, outlet {:.0} °C (TENTATIVE)",
                            if summary.converged { "converged" } else { "did not converge" },
                            seconds,
                            summary.max_kernel_c,
                            summary.vessel_outlet_c
                        ),
                        !summary.converged,
                    ));
                    self.summary = Some(summary);
                    self.fields = Some(fields);
                }
                MpEv::Error(e) => {
                    self.running = false;
                    self.console.push(format!("ERROR: {e}"));
                    said.push((e, true));
                }
            }
        }
        said
    }

    fn preview(&mut self) -> Result<&Fields, String> {
        let stale = self
            .preview
            .as_ref()
            .map_or(true, |(s, _)| *s != self.setup);
        if stale {
            let f = crate::porous_core::PorousCore::new(&self.setup).map(|c| c.fields().clone());
            self.preview = Some((self.setup.clone(), f));
        }
        match &self.preview {
            Some((_, Ok(f))) => Ok(f),
            Some((_, Err(e))) => Err(e.clone()),
            None => Err("no preview".into()),
        }
    }
}

/// The literature pane's citations for Steps 9 and 10.
pub fn cites(app: &App) -> Vec<Citation> {
    app.mp.setup.cites.clone()
}

fn num(
    ui: &mut egui::Ui,
    label: &str,
    v: &mut f64,
    speed: f64,
    range: std::ops::RangeInclusive<f64>,
) -> bool {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.add(egui::DragValue::new(v).speed(speed).range(range))
            .changed()
    })
    .inner
}

fn int(
    ui: &mut egui::Ui,
    label: &str,
    v: &mut usize,
    range: std::ops::RangeInclusive<usize>,
) -> bool {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.add(egui::DragValue::new(v).range(range)).changed()
    })
    .inner
}

fn elements(ui: &mut egui::Ui, s: &MultiphysicsSetup) {
    for e in &s.elements {
        let c = match e.status {
            ElementStatus::InModel => Color32::from_rgb(30, 120, 60),
            ElementStatus::Simplified => Color32::from_rgb(170, 110, 0),
            ElementStatus::NotInModel => Color32::from_rgb(190, 30, 30),
        };
        ui.horizontal_wrapped(|ui| {
            ui.colored_label(c, format!("[{}]", e.status.badge()));
            ui.label(RichText::new(&e.name).strong());
        });
        ui.small(&e.note);
    }
}

/// Step 9's settings panel.
pub fn setup_settings(app: &mut App, ui: &mut egui::Ui) {
    let s = &mut app.mp.setup;
    let mut changed = false;
    ui.colored_label(
        Color32::from_rgb(170, 90, 0),
        "What runs at Step 10 is SIMPLIFIED: a prescribed power shape, lumped feedback and an r-z \
         multi-channel porous core. The parts below marked NOT in model do not run.",
    );
    egui::CollapsingHeader::new(
        RichText::new("OUTRAM-Foam side: porous-core thermal-hydraulics").strong(),
    )
    .default_open(true)
    .show(ui, |ui| {
        let f = &mut s.foam;
        ui.label(RichText::new("Boundary conditions").underline());
        changed |= num(
            ui,
            "Inlet helium [°C]",
            &mut f.inlet_temperature_c,
            1.0,
            20.0..=900.0,
        );
        changed |= num(
            ui,
            "Total helium flow [kg/s]",
            &mut f.total_mass_flow_kg_s,
            0.01,
            0.1..=50.0,
        );
        changed |= num(
            ui,
            "  of which through the bed [kg/s]",
            &mut f.core_mass_flow_kg_s,
            0.01,
            0.1..=50.0,
        );
        ui.small(format!(
            "bypass {:.2} kg/s ({:.1} %), mixed back at the inlet temperature",
            f.total_mass_flow_kg_s - f.core_mass_flow_kg_s,
            100.0 * (1.0 - f.core_mass_flow_kg_s / f.total_mass_flow_kg_s.max(1e-9))
        ));
        changed |= num(
            ui,
            "Outlet pressure [MPa]",
            &mut f.outlet_pressure_mpa,
            0.01,
            0.1..=10.0,
        );
        ui.label("Side wall: adiabatic (zero heat flux)");
        changed |= ui
            .checkbox(&mut f.flow_downward, "Flow downward (top inlet)")
            .changed();
        ui.label(RichText::new("Porous bed").underline());
        changed |= num(
            ui,
            "Bed radius [cm]",
            &mut f.core_radius_cm,
            0.5,
            10.0..=500.0,
        );
        changed |= num(
            ui,
            "Bed height [cm]",
            &mut f.core_height_cm,
            0.5,
            10.0..=1000.0,
        );
        changed |= num(
            ui,
            "Filling fraction",
            &mut f.filling_fraction,
            0.001,
            0.3..=0.74,
        );
        changed |= num(
            ui,
            "Fuel-pebble fraction",
            &mut f.fuel_pebble_fraction,
            0.01,
            0.01..=1.0,
        );
        changed |= num(
            ui,
            "Fast fluence [1e25 n/m²]",
            &mut f.fast_fluence_1e25_per_m2,
            0.01,
            0.0..=10.0,
        );
        ui.small(format!(
            "pebble diameter {} cm (pebble interior: tampines Pebble::htr10)",
            f.pebble_diameter_cm
        ));
        ui.label(RichText::new("Models").underline());
        for m in [
            &f.friction_model,
            &f.heat_transfer_model,
            &f.pebble_model,
            &f.helium_properties,
        ] {
            ui.small(format!("• {m}"));
        }
        ui.label(RichText::new("Solver (r-z multi-channel)").underline());
        changed |= int(ui, "Radial rings (equal area)", &mut f.radial_rings, 1..=40);
        changed |= int(ui, "Axial nodes", &mut f.axial_nodes, 2..=400);
    });
    egui::CollapsingHeader::new(RichText::new("Neutronics side").strong()).default_open(true).show(ui, |ui| {
        let n = &mut s.neutronics;
        changed |= num(ui, "Thermal power [MW]", &mut n.thermal_power_mw, 0.1, 0.01..=100.0);
        let mut j0 = matches!(n.shape, PowerShape::J0Cosine { .. });
        ui.horizontal(|ui| {
            changed |= ui.radio_value(&mut j0, true, "J0 x cosine").changed();
            changed |= ui.radio_value(&mut j0, false, "uniform (ablation)").changed();
        });
        match (&mut n.shape, j0) {
            (PowerShape::J0Cosine { peak_to_mean }, true) => {
                changed |= num(ui, "  peak / mean power density", peak_to_mean, 0.001, 1.001..=3.6);
            }
            (shape, true) => *shape = PowerShape::J0Cosine { peak_to_mean: 2.57 / 2.0 },
            (shape, false) => *shape = PowerShape::Uniform,
        }
        ui.small("The shape is PRESCRIBED, not solved: spatial diffusion on Step 8's cross sections is not wired (gh:#591).");
        changed |= num(ui, "Isothermal coefficient [1/K]", &mut n.isothermal_coefficient_per_k, 1e-6, -1e-3..=1e-3);
        changed |= num(ui, "Cold reference temperature [K]", &mut n.reference_temperature_k, 0.1, 1.0..=3000.0);
        ui.horizontal(|ui| {
            ui.label("Feedback weighting");
            changed |= ui.radio_value(&mut n.weighting, FeedbackWeighting::Power, "power").changed();
            changed |= ui.radio_value(&mut n.weighting, FeedbackWeighting::Volume, "volume").changed();
        });
    });
    egui::CollapsingHeader::new(RichText::new("Coupling loop").strong())
        .default_open(false)
        .show(ui, |ui| {
            let c = &mut s.coupling;
            changed |= int(ui, "Max iterations", &mut c.max_iterations, 1..=1000);
            changed |= num(
                ui,
                "Ring Δp tolerance (relative)",
                &mut c.pressure_tolerance,
                1e-5,
                1e-8..=0.1,
            );
            changed |= num(
                ui,
                "Temperature tolerance [K]",
                &mut c.temperature_tolerance_k,
                0.001,
                1e-4..=10.0,
            );
            changed |= num(
                ui,
                "Flow relaxation",
                &mut c.flow_relaxation,
                0.01,
                0.05..=1.0,
            );
        });
    egui::CollapsingHeader::new(RichText::new("farrer-park side: structural FEM").strong())
        .default_open(false)
        .show(ui, |ui| {
            let st = &s.structural;
            ui.colored_label(
                Color32::from_rgb(190, 30, 30),
                "NOT run in this build (gh:#593).",
            );
            ui.small(format!(
                "element: {}; model: {}; load: {}",
                st.element, st.material_model, st.load
            ));
        });
    ui.separator();
    egui::CollapsingHeader::new("What the run models (in / simplified / NOT)")
        .default_open(false)
        .show(ui, |ui| {
            elements(ui, &app.mp.setup);
        });
    if app.mp.setup != app.mp.preset {
        ui.colored_label(
            Color32::from_rgb(170, 90, 0),
            "Edited from the HTR-10 prefill.",
        );
        if ui.button("Reset Step 9 to the HTR-10 prefill").clicked() {
            app.mp.setup = app.mp.preset.clone();
            changed = true;
        }
    }
    if changed {
        // A new case: the last run no longer describes it.
        app.mp.summary = None;
    }
}

/// Step 10's settings panel: Run / Stop and the results.
pub fn run_settings(app: &mut App, ui: &mut egui::Ui) {
    let now = app.now();
    ui.colored_label(
        Color32::from_rgb(170, 90, 0),
        "SIMPLIFIED coupled run: TH march on tampines correlations with a PRESCRIBED power shape and \
         lumped temperature feedback. Not a GeN-Foam/farrer-park run (gh:#591, #592, #593).",
    );
    ui.horizontal(|ui| {
        if ui
            .add_enabled(!app.mp.running, egui::Button::new("Run coupled case"))
            .clicked()
        {
            app.mp.start(now);
            app.say("Coupled run started", false);
        }
        if ui
            .add_enabled(app.mp.running, egui::Button::new("Stop"))
            .clicked()
        {
            app.mp.request_stop();
        }
        if app.mp.running {
            ui.spinner();
            ui.label(format!(
                "{} iterations, {:.1} s",
                app.mp.history.len(),
                now - app.mp.started_at
            ));
        }
    });
    ui.separator();
    ui.label(RichText::new("Main view field").strong());
    ui.horizontal_wrapped(|ui| {
        for k in FieldKind::ALL {
            ui.selectable_value(&mut app.mp.field, k, k.label());
        }
    });
    if let Some(r) = app.mp.history.last() {
        ui.separator();
        let (lo, hi) = r
            .ring_flow_kg_s
            .iter()
            .fold((f64::MAX, f64::MIN), |(l, h), &v| (l.min(v), h.max(v)));
        ui.label(RichText::new("Flow split (the coupling loop)").strong());
        ui.small(format!(
            "{} rings: {:.4}-{:.4} kg/s each (centre lowest), Δp spread {:.2e}",
            r.ring_flow_kg_s.len(),
            lo,
            hi,
            r.flow_residual
        ));
        egui::CollapsingHeader::new("Per ring")
            .default_open(false)
            .show(ui, |ui| {
                egui::Grid::new("mp_rings")
                    .striped(true)
                    .num_columns(3)
                    .show(ui, |ui| {
                        for h in ["ring", "flow [kg/s]", "Δp [Pa]"] {
                            ui.label(RichText::new(h).strong());
                        }
                        ui.end_row();
                        for (i, (m, dp)) in r.ring_flow_kg_s.iter().zip(&r.ring_dp_pa).enumerate() {
                            ui.label(format!("{i}"));
                            ui.label(format!("{m:.4}"));
                            ui.label(format!("{dp:.1}"));
                            ui.end_row();
                        }
                    })
            });
        ui.small(format!(
            "feedback temperature {:.1} °C",
            r.t_feedback_k - 273.15
        ));
    }
    if let Some(s) = app.mp.summary.clone() {
        ui.separator();
        ui.label(RichText::new("Result (TENTATIVE)").strong());
        egui::Grid::new("mp_compare")
            .striped(true)
            .num_columns(4)
            .show(ui, |ui| {
                for h in ["", "ours", "Gao & Shi", "diff"] {
                    ui.label(RichText::new(h).strong());
                }
                ui.end_row();
                for (what, ours, publ, note) in crate::mp_headless::comparison(&s) {
                    ui.label(what).on_hover_text(note);
                    ui.label(format!("{ours:.1}"));
                    ui.label(format!("{publ:.1}"));
                    ui.label(format!("{:+.1}", ours - publ));
                    ui.end_row();
                }
            });
        ui.small("Gao & Shi's temperatures include their §4.1 uncertainty factors (burnup peaking 1.2, hot spot 1.05, heat transfer 1.2); ours are nominal. Their pressure drop includes the bottom reflector.");
        ui.label(format!("k = {:.5} relative to a cold-critical reference ({:+.0} pcm at {:.0} °C feedback temperature): LUMPED, not an eigenvalue.", s.k_vs_cold_critical, s.rho_pcm, s.t_feedback_c));
        if let Some(run) = app.recipe.monte_carlo.runs.last() {
            let rho_ref = 1.0 - 1.0 / run.k;
            let k_hot = 1.0 / (1.0 - rho_ref - s.rho_pcm * 1e-5);
            ui.label(format!(
                "Applied to Step 5's {} (k = {:.5} ± {:.5}, cold): k_hot ≈ {:.5}.",
                run.label, run.k, run.sigma, k_hot
            ));
            ui.small("Caution: Step 5 ran the recipe's bed loading at cold zero power; the TH run is the full 197 cm core at power. The two geometries differ.");
        }
    }
}

/// Step 10's bottom panel: console and convergence plots.
pub fn results(app: &mut App, ui: &mut egui::Ui) {
    let mp = &app.mp;
    let total = ui.available_width();
    // The panel's own height, not what the plots could ask for (which
    // would grow the panel to its maximum and squeeze the main view).
    let h = ui.available_height().clamp(140.0, 250.0);
    let cw = (total * 0.4).max(260.0);
    let pw = ((total - cw - 40.0) / 3.0).max(120.0);
    let pts = |f: fn(&IterationReport) -> f64| series(&mp.history, f);
    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            ui.set_width(cw);
            ui.label(RichText::new("Console").strong());
            egui::ScrollArea::both()
                .max_width(cw)
                .max_height(h - 24.0)
                .stick_to_bottom(true)
                .id_salt("mp_console")
                .show(ui, |ui| {
                    for l in &mp.console {
                        ui.add(
                            egui::Label::new(RichText::new(l).monospace().size(crate::style::Text::Small.size())).extend(),
                        );
                    }
                });
        });
        ui.separator();
        ui.vertical(|ui| {
            ui.set_width(pw);
            ui.label("Residuals (log10)");
            Plot::new("mp_resid")
                .width(pw)
                .height(h - 24.0)
                .legend(Legend::default())
                .show(ui, |p| {
                    p.line(Line::new(
                        "ring Δp spread",
                        pts(|r| r.flow_residual.max(1e-16).log10()),
                    ));
                    p.line(Line::new(
                        "max ΔT [K]",
                        pts(|r| r.temperature_change_k.max(1e-16).log10()),
                    ));
                });
        });
        ui.vertical(|ui| {
            ui.set_width(pw);
            ui.label("Temperatures [°C]");
            Plot::new("mp_temps")
                .width(pw)
                .height(h - 24.0)
                .legend(Legend::default())
                .show(ui, |p| {
                    p.line(Line::new("max kernel", pts(|r| r.max_kernel_k - 273.15)));
                    p.line(Line::new("max helium", pts(|r| r.max_helium_k - 273.15)));
                    p.line(Line::new("bed exit", pts(|r| r.core_exit_k - 273.15)));
                });
        });
        ui.vertical(|ui| {
            ui.set_width(pw);
            ui.label("k (lumped, vs cold-critical)");
            Plot::new("mp_k").width(pw).height(h - 24.0).show(ui, |p| {
                p.line(Line::new("k", pts(|r| r.k_vs_cold_critical)));
            });
        });
    });
}

fn series(h: &[IterationReport], f: impl Fn(&IterationReport) -> f64) -> PlotPoints<'static> {
    h.iter()
        .map(|r| [r.iteration as f64, f(r)])
        .collect::<Vec<[f64; 2]>>()
        .into()
}

fn colour(t: f64) -> Color32 {
    // A perceptually ordered dark-blue -> red -> yellow ramp.
    let stops = [
        (0.0, [20, 20, 90]),
        (0.3, [60, 70, 200]),
        (0.55, [200, 60, 90]),
        (0.8, [240, 140, 30]),
        (1.0, [250, 240, 120]),
    ];
    let t = t.clamp(0.0, 1.0);
    for w in stops.windows(2) {
        let (a, ca) = w[0];
        let (b, cb) = w[1];
        if t <= b {
            let s = (t - a) / (b - a);
            let m = |i: usize| (f64::from(ca[i]) + s * (f64::from(cb[i]) - f64::from(ca[i]))) as u8;
            return Color32::from_rgb(m(0), m(1), m(2));
        }
    }
    Color32::from_rgb(250, 240, 120)
}

/// Draw `values` on the r-z grid, mirrored about the axis (an axial
/// section), into `rect`. Returns the colour range.
fn draw_rz(
    painter: &egui::Painter,
    rect: Rect,
    f: &Fields,
    values: &[f64],
    zoom: f32,
) -> (f64, f64) {
    let (lo, hi) = values
        .iter()
        .fold((f64::MAX, f64::MIN), |(l, h), &v| (l.min(v), h.max(v)));
    let span = (hi - lo).max(1e-12);
    let r = f.ring_outer_m.last().copied().unwrap_or(1.0);
    let h = f.height_m;
    let scale =
        (rect.width() / (2.0 * r as f32 * 1.25)).min(rect.height() / (h as f32 * 1.2)) * zoom;
    let c = rect.center();
    let to = |x: f64, z: f64| Pos2::new(c.x + x as f32 * scale, c.y + (z - 0.5 * h) as f32 * scale);
    let dz = h / f.n_z as f64;
    for i in 0..f.n_r {
        let r_in = if i == 0 { 0.0 } else { f.ring_outer_m[i - 1] };
        let r_out = f.ring_outer_m[i];
        for j in 0..f.n_z {
            let col = colour((values[i * f.n_z + j] - lo) / span);
            for side in [-1.0, 1.0] {
                let a = to(side * r_in, j as f64 * dz);
                let b = to(side * r_out, (j + 1) as f64 * dz);
                painter.rect_filled(Rect::from_two_pos(a, b), 0.0, col);
            }
        }
        // Ring lines only where they stay at least 6 px apart.
        if (r_out - r_in) as f32 * scale >= 6.0 || i + 1 == f.n_r {
            for side in [-1.0, 1.0] {
                painter.line_segment(
                    [to(side * r_out, 0.0), to(side * r_out, h)],
                    Stroke::new(0.5, Color32::from_gray(30)),
                );
            }
        }
    }
    painter.line_segment(
        [to(0.0, -0.05 * h), to(0.0, 1.05 * h)],
        Stroke::new(1.0, Color32::from_gray(200)),
    );
    painter.rect_stroke(
        Rect::from_two_pos(to(-r, 0.0), to(r, h)),
        0.0,
        Stroke::new(1.5, Color32::from_gray(220)),
        egui::StrokeKind::Outside,
    );
    // Colour bar.
    let bar = Rect::from_min_size(
        Pos2::new(rect.right() - 40.0, rect.top() + 60.0),
        Vec2::new(16.0, rect.height() - 120.0),
    );
    let n = 60;
    for k in 0..n {
        let t0 = k as f32 / n as f32;
        let y1 = bar.bottom() - t0 * bar.height();
        let y0 = bar.bottom() - (k + 1) as f32 / n as f32 * bar.height();
        painter.rect_filled(
            Rect::from_min_max(Pos2::new(bar.left(), y0), Pos2::new(bar.right(), y1)),
            0.0,
            colour(f64::from(t0)),
        );
    }
    let font = crate::style::Text::Small.font();
    painter.text(
        Pos2::new(bar.left() - 4.0, bar.top()),
        egui::Align2::RIGHT_CENTER,
        format!("{hi:.1}"),
        font.clone(),
        Color32::from_gray(230),
    );
    painter.text(
        Pos2::new(bar.left() - 4.0, bar.bottom()),
        egui::Align2::RIGHT_CENTER,
        format!("{lo:.1}"),
        font,
        Color32::from_gray(230),
    );
    (lo, hi)
}

/// The main view for Steps 9 (the case: mesh, BCs, power shape) and 10
/// (the chosen field, live).
pub fn main_view(app: &mut App, ui: &mut egui::Ui, run_step: bool) {
    let (rect, resp) = ui.allocate_exact_size(ui.available_size(), egui::Sense::click_and_drag());
    if resp.hovered() {
        let (scroll, zoom) = ui.input(|i| (i.smooth_scroll_delta.y, i.zoom_delta()));
        app.mp.zoom = (app.mp.zoom * zoom * (scroll * 0.002).exp()).clamp(0.3, 20.0);
    }
    if resp.double_clicked() {
        app.mp.zoom = 1.0;
    }
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, Color32::from_rgb(40, 40, 46));
    let font = crate::style::Text::Body.font();
    let text = Color32::from_gray(230);
    let zoom = app.mp.zoom;
    let mut lines: Vec<String> = Vec::new();
    if run_step {
        let kind = app.mp.field;
        if let Some(f) = app.mp.fields.clone() {
            let v = kind.values(&f);
            draw_rz(&painter, rect, &f, &v, zoom);
            lines.push(format!(
                "{} on the TH mesh ({} rings x {} nodes), axial section",
                kind.label(),
                f.n_r,
                f.n_z
            ));
            if let Some(r) = app.mp.history.last() {
                lines.push(format!(
                    "iteration {}  ring Δp spread {:.2e}  max ΔT {:.3} K",
                    r.iteration, r.flow_residual, r.temperature_change_k
                ));
            }
        } else {
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "Press \"Run coupled case\" in the panel.",
                font.clone(),
                text,
            );
        }
    } else {
        match app.mp.preview() {
            Ok(f) => {
                let f = f.clone();
                let v = FieldKind::Power.values(&f);
                draw_rz(&painter, rect, &f, &v, zoom);
                lines.push(format!(
                    "Prescribed power density [W/cm³] on the TH mesh ({} rings x {} nodes)",
                    f.n_r, f.n_z
                ));
                let fo = &app.mp.setup.foam;
                let r = f.ring_outer_m.last().copied().unwrap_or(1.0);
                let scale = (rect.width() / (2.0 * r as f32 * 1.25))
                    .min(rect.height() / (f.height_m as f32 * 1.2))
                    * zoom;
                let c = rect.center();
                let (top, bottom) = (
                    c.y - 0.5 * f.height_m as f32 * scale,
                    c.y + 0.5 * f.height_m as f32 * scale,
                );
                let (inlet_y, outlet_y) = if fo.flow_downward {
                    (top, bottom)
                } else {
                    (bottom, top)
                };
                painter.text(
                    Pos2::new(c.x, inlet_y - 18.0),
                    egui::Align2::CENTER_CENTER,
                    format!(
                        "INLET: helium {:.0} °C, {:.2} kg/s through the bed",
                        fo.inlet_temperature_c, fo.core_mass_flow_kg_s
                    ),
                    font.clone(),
                    text,
                );
                painter.text(
                    Pos2::new(c.x, outlet_y + 18.0),
                    egui::Align2::CENTER_CENTER,
                    format!(
                        "OUTLET: {:.2} MPa (bypass {:.2} kg/s mixed here)",
                        fo.outlet_pressure_mpa,
                        fo.total_mass_flow_kg_s - fo.core_mass_flow_kg_s
                    ),
                    font.clone(),
                    text,
                );
                painter.text(
                    Pos2::new(c.x - r as f32 * scale - 6.0, c.y),
                    egui::Align2::RIGHT_CENTER,
                    "adiabatic\nwall",
                    font.clone(),
                    text,
                );
            }
            Err(e) => {
                painter.text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    format!("Set-up refused: {e}"),
                    font.clone(),
                    Color32::from_rgb(230, 90, 90),
                );
            }
        }
    }
    lines.push(
        "SIMPLIFIED: prescribed power shape; no inter-ring conduction; lumped feedback".into(),
    );
    painter.text(
        rect.left_bottom() + Vec2::new(10.0, -10.0),
        egui::Align2::LEFT_BOTTOM,
        lines.join("\n"),
        crate::style::Text::Small.font(),
        text,
    );
    // + / − / reset.
    let h = crate::style::button_height();
    let w = crate::style::Text::Body.size() * 2.4;
    let mut x = rect.right() - 8.0 - 3.0 * (w + 6.0) - 50.0;
    for (label, f) in [("+", 1.3f32), ("−", 1.0 / 1.3), ("⟲", 0.0)] {
        if ui
            .put(
                Rect::from_min_size(Pos2::new(x, rect.top() + 8.0), Vec2::new(w, h)),
                egui::Button::new(RichText::new(label).size(crate::style::Text::Emphasis.size())),
            )
            .clicked()
        {
            app.mp.zoom = if f == 0.0 {
                1.0
            } else {
                (app.mp.zoom * f).clamp(0.3, 20.0)
            };
        }
        x += w + 6.0;
    }
    app.main_rect = rect;
}

/// A PNG of a field on the TH mesh, with the mesh lines drawn (the
/// crate's drawing rule, for meshes: plot the mesh itself), a title and a
/// colour legend.
///
/// # Errors
///
/// A file error.
pub fn write_field_png(f: &Fields, kind: FieldKind, path: &std::path::Path) -> Result<(), String> {
    use outram_mc_libs::geometry::plot::{annotate_image, ImageData, LegendEntry, Rgb};
    let px_per_m = 300.0;
    let r = f.ring_outer_m.last().copied().unwrap_or(1.0);
    let (w, h) = (
        ((2.0 * r) * px_per_m) as usize + 1,
        (f.height_m * px_per_m) as usize + 1,
    );
    let mut img = ImageData::filled(
        w,
        h,
        Rgb {
            r: 255,
            g: 255,
            b: 255,
        },
    );
    let v = kind.values(f);
    let (lo, hi) = v
        .iter()
        .fold((f64::MAX, f64::MIN), |(l, h), &x| (l.min(x), h.max(x)));
    let span = (hi - lo).max(1e-12);
    let dz = f.height_m / f.n_z as f64;
    for y in 0..h {
        let z = (y as f64 + 0.5) / px_per_m;
        let j = ((z / dz) as usize).min(f.n_z - 1);
        for x in 0..w {
            let rr = ((x as f64 + 0.5) / px_per_m - r).abs();
            let i = f
                .ring_outer_m
                .iter()
                .position(|&ro| rr <= ro)
                .unwrap_or(f.n_r - 1);
            let c = colour((v[i * f.n_z + j] - lo) / span);
            img.set(
                x,
                y,
                Rgb {
                    r: c.r(),
                    g: c.g(),
                    b: c.b(),
                },
            );
        }
    }
    // Mesh lines: ring and axial node boundaries, every `stride`-th so
    // lines stay at least 6 px apart (stated in the title).
    let black = Rgb { r: 0, g: 0, b: 0 };
    let min_ring = f
        .ring_outer_m
        .iter()
        .scan(0.0, |prev, &ro| {
            let d = ro - *prev;
            *prev = ro;
            Some(d)
        })
        .fold(f64::MAX, f64::min);
    let ring_stride = ((6.0 / (min_ring * px_per_m)).ceil() as usize).max(1);
    let axial_stride = ((6.0 / (dz * px_per_m)).ceil() as usize).max(1);
    for (k, ro) in f.ring_outer_m.iter().enumerate() {
        if (k + 1) % ring_stride != 0 && k + 1 != f.n_r {
            continue;
        }
        for side in [-1.0, 1.0] {
            let x = ((r + side * ro) * px_per_m).round() as usize;
            for y in 0..h {
                img.set(x.min(w - 1), y, black);
            }
        }
    }
    for j in (0..=f.n_z).filter(|j| j % axial_stride == 0 || *j == f.n_z) {
        let y = ((j as f64 * dz) * px_per_m).round() as usize;
        for x in 0..w {
            img.set(x, y.min(h - 1), black);
        }
    }
    let legend: Vec<LegendEntry> = (0..8)
        .rev()
        .map(|k| {
            let t = (k as f64 + 0.5) / 8.0;
            let c = colour(t);
            LegendEntry {
                colour: Rgb {
                    r: c.r(),
                    g: c.g(),
                    b: c.b(),
                },
                label: format!("{:.2}", lo + t * span),
            }
        })
        .collect();
    let title = format!(
        "HTR-10 TH MESH, R-Z ({} RINGS X {} NODES, LINES EVERY {} RING, {} NODE, {:.0} X {:.0} CM): {}",
        f.n_r,
        f.n_z,
        ring_stride,
        axial_stride,
        2.0 * r * 100.0,
        f.height_m * 100.0,
        kind.label().replace('°', "DEG ").replace('³', "3").to_uppercase()
    );
    let out = annotate_image(&img, &title, &legend);
    if let Some(p) = path.parent() {
        let _ = std::fs::create_dir_all(p);
    }
    out.write_png(path)
        .map_err(|e| format!("{}: {e}", path.display()))
}
