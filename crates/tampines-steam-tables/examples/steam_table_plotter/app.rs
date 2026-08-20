//! The `eframe`/`egui` application: a left control sidebar, a four-tab plot
//! panel, and per-tab export buttons.
//!
//! # Layout, as issue #26 describes it
//!
//! * left sidebar — tab selection, layer checkboxes, axis controls, resolution,
//!   export buttons,
//! * central panel — the interactive diagram,
//! * every layer is a checkbox, and an **unavailable layer is a disabled
//!   checkbox with its reason shown**, never a missing one.
//!
//! # Live canvas versus exported figure
//!
//! The `egui_plot` canvas is for *inspection* — pan, zoom, hover-read a
//! coordinate. The exported PNG/PDF/SVG come from this tool's own renderer
//! ([`crate::figure`]), not from a screenshot of the canvas. The two therefore
//! differ in styling, and one place where they differ substantively is the
//! logarithmic pressure axis: `egui_plot` has no log axis, so when the log
//! toggle is on the canvas plots `log10(p / bar)` and says so in the axis
//! label, while the exported figure draws a proper decade-ticked log axis. The
//! underlying numbers are identical.
//!
//! # Rebuild cost
//!
//! Every curve is recomputed from IAPWS-IF97 whenever a control changes, which
//! at the default 400 samples per curve is a few thousand flashes — fast enough
//! to feel immediate, but not free, so the built layers are cached per tab and
//! only invalidated when something that affects them actually changes.

use std::collections::HashMap;
use std::path::PathBuf;

use eframe::egui;
use egui_plot::{Legend, Line, LineStyle, Plot, Points};

use crate::data::{LayerKind, PlotLayer};
use crate::diagram::DiagramKind;
use crate::export::{self, ExportFormat};
use crate::figure::layout::PageSize;
use crate::figure::png::DEFAULT_PIXELS_PER_POINT;
use crate::figure::{AxisScale, MarkerShape, Rgb, SeriesStyle};
use crate::layers::LayerId;

/// Launches the interactive window.
///
/// # Errors
///
/// Returns whatever `eframe` reports — most often "no display" on a headless
/// box, which the caller turns into a pointer at `--export-all`.
pub fn run(out_dir: PathBuf, curve_samples: usize) -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1400.0, 900.0])
            .with_min_inner_size([900.0, 600.0])
            .with_title("TAMPINES steam tables — T-p / p-h / T-s / h-s plotter"),
        ..Default::default()
    };
    // The `Box<dyn ...>` here is `eframe::run_native`'s own signature, not a
    // dispatch choice of this example's; every enum in this tool is dispatched
    // by `match`, per the workspace Rust design rules.
    eframe::run_native(
        "steam_table_plotter",
        options,
        Box::new(move |_cc| Ok(Box::new(PlotterApp::new(out_dir, curve_samples)))),
    )
}

/// Application state.
///
/// Owns its data by value; no lifetime parameters, no trait objects.
pub struct PlotterApp {
    /// Which diagram tab is showing.
    active_tab: DiagramKind,
    /// Layer visibility, one flag per [`LayerId`].
    visible: Vec<(LayerId, bool)>,
    /// Samples per computed curve.
    curve_samples: usize,
    /// Whether a pressure axis is logarithmic.
    log_pressure: bool,
    /// Export directory.
    out_dir: PathBuf,
    /// Export resolution for the PNG, in pixels per point.
    export_pixels_per_point: f64,
    /// Built layers, cached per tab.
    cache: HashMap<DiagramKind, Vec<PlotLayer>>,
    /// Last status line, shown at the bottom of the sidebar.
    status: String,
}

impl PlotterApp {
    /// Builds the app with every layer at its default visibility.
    pub fn new(out_dir: PathBuf, curve_samples: usize) -> Self {
        Self {
            active_tab: DiagramKind::PressureEnthalpy,
            visible: LayerId::ALL
                .iter()
                .map(|id| (*id, id.default_visible()))
                .collect(),
            curve_samples: curve_samples.clamp(40, 2000),
            log_pressure: true,
            out_dir,
            export_pixels_per_point: DEFAULT_PIXELS_PER_POINT,
            cache: HashMap::new(),
            status: "ready".to_string(),
        }
    }

    /// The layers currently switched on.
    fn active_layers(&self) -> Vec<LayerId> {
        self.visible
            .iter()
            .filter(|(_, on)| *on)
            .map(|(id, _)| *id)
            .collect()
    }

    /// Drops every cached tab, forcing a recompute.
    fn invalidate(&mut self) {
        self.cache.clear();
    }

    /// Built layers for the active tab, computing them if they are not cached.
    fn layers_for_active_tab(&mut self) -> &[PlotLayer] {
        let tab = self.active_tab;
        if !self.cache.contains_key(&tab) {
            let active = self.active_layers();
            let built = export::build_layers(tab, &active, self.curve_samples);
            self.cache.insert(tab, built);
        }
        self.cache.get(&tab).map(Vec::as_slice).unwrap_or(&[])
    }

    /// Axis scale for the current tab's y axis.
    fn y_scale(&self) -> AxisScale {
        if self.active_tab.y_is_pressure() && self.log_pressure {
            AxisScale::Log10
        } else {
            AxisScale::Linear
        }
    }

    /// Runs an export of the active tab.
    fn export(&mut self, formats: &[ExportFormat]) {
        let tab = self.active_tab;
        let active = self.active_layers();
        let samples = self.curve_samples;
        let out_dir = self.out_dir.clone();
        let pixels = self.export_pixels_per_point;
        let y_scale = self.y_scale();

        let built = export::build_layers(tab, &active, samples);
        let scene = export::build_scene(tab, &built, AxisScale::Linear, y_scale, &active);
        self.status = match export::write_files(
            &out_dir,
            tab,
            &built,
            &scene,
            formats,
            PageSize::DEFAULT,
            pixels,
        ) {
            Ok(paths) => format!("wrote {} file(s) to {}", paths.len(), out_dir.display()),
            Err(message) => format!("export failed: {message}"),
        };
    }

    /// The control sidebar.
    fn sidebar(&mut self, ui: &mut egui::Ui) {
        ui.heading("Diagram");
        for kind in DiagramKind::ALL {
            if ui
                .selectable_label(self.active_tab == kind, kind.tab_label())
                .clicked()
            {
                self.active_tab = kind;
            }
        }

        ui.separator();
        ui.heading("Layers");
        ui.label(
            egui::RichText::new(
                "Curves are computed live from IAPWS-IF97. Scattered points are \
                 cited reference data. A greyed-out layer is one that cannot be \
                 honestly drawn here — the reason is on hover.",
            )
            .small()
            .weak(),
        );

        let tab = self.active_tab;
        let mut changed = false;
        let mut computed_header_done = false;
        let mut reference_header_done = false;
        for index in 0..self.visible.len() {
            let (id, mut on) = self.visible[index];
            match id.kind() {
                LayerKind::ComputedCurve if !computed_header_done => {
                    ui.add_space(4.0);
                    ui.label(egui::RichText::new("computed from IAPWS-IF97").strong());
                    computed_header_done = true;
                }
                LayerKind::ReferencePoints if !reference_header_done => {
                    ui.add_space(6.0);
                    ui.label(egui::RichText::new("reference / validation data").strong());
                    reference_header_done = true;
                }
                _ => {}
            }

            let availability = id.availability_on(tab);
            match availability.reason() {
                None => {
                    if ui.checkbox(&mut on, id.label()).changed() {
                        self.visible[index].1 = on;
                        changed = true;
                    }
                }
                Some(reason) => {
                    let mut disabled = false;
                    ui.add_enabled_ui(false, |ui| {
                        ui.checkbox(&mut disabled, id.label());
                    });
                    ui.label(
                        egui::RichText::new(format!("   not shown: {reason}"))
                            .small()
                            .weak(),
                    );
                }
            }
        }

        ui.separator();
        ui.heading("Axes and resolution");
        if self.active_tab.y_is_pressure() {
            if ui
                .checkbox(&mut self.log_pressure, "logarithmic pressure axis")
                .changed()
            {
                changed = true;
            }
            ui.label(
                egui::RichText::new(
                    "the live canvas plots log10(p / bar); the exported figure \
                     draws a decade-ticked log axis",
                )
                .small()
                .weak(),
            );
        } else {
            ui.label(
                egui::RichText::new("this diagram has no pressure axis")
                    .small()
                    .weak(),
            );
        }
        let mut samples = self.curve_samples;
        if ui
            .add(egui::Slider::new(&mut samples, 40..=1200).text("samples per curve"))
            .changed()
        {
            self.curve_samples = samples;
            changed = true;
        }
        ui.add(
            egui::Slider::new(&mut self.export_pixels_per_point, 1.0..=6.0).text("PNG px per pt"),
        );

        ui.separator();
        ui.heading("Export");
        ui.label(
            egui::RichText::new(format!("into {}", self.out_dir.display()))
                .small()
                .weak(),
        );
        ui.horizontal_wrapped(|ui| {
            for format in ExportFormat::ALL {
                if ui.button(format.label()).clicked() {
                    self.export(&[format]);
                }
            }
            if ui.button("all formats").clicked() {
                self.export(&ExportFormat::ALL);
            }
        });
        ui.add_space(4.0);
        ui.label(egui::RichText::new(&self.status).small());

        if changed {
            self.invalidate();
        }
    }

    /// The plot panel.
    fn plot_panel(&mut self, ui: &mut egui::Ui) {
        let tab = self.active_tab;
        let log = self.y_scale() == AxisScale::Log10;
        let y_label = if log {
            "log10(Pressure p / bar)".to_string()
        } else {
            tab.y_label().to_string()
        };
        let layers: Vec<PlotLayer> = self.layers_for_active_tab().to_vec();

        Plot::new(tab.tab_label())
            .legend(Legend::default())
            .x_axis_label(tab.x_label())
            .y_axis_label(y_label)
            .allow_scroll(false)
            .show(ui, |plot_ui| {
                for layer in &layers {
                    let colour = to_color32(layer.colour);
                    for segment in &layer.segments {
                        let points: Vec<[f64; 2]> = segment
                            .iter()
                            .filter_map(|point| {
                                let [x, y] = tab.project(point);
                                let y = if log {
                                    if y > 0.0 {
                                        y.log10()
                                    } else {
                                        return None;
                                    }
                                } else {
                                    y
                                };
                                (x.is_finite() && y.is_finite()).then_some([x, y])
                            })
                            .collect();
                        if points.is_empty() {
                            continue;
                        }
                        match layer.style {
                            SeriesStyle::Line { width, dash } => {
                                let mut line = Line::new(layer.label.clone(), points)
                                    .color(colour)
                                    .width(width as f32);
                                if let Some((on, _off)) = dash {
                                    line = line.style(LineStyle::dashed_dense());
                                    let _ = on;
                                }
                                plot_ui.line(line);
                            }
                            SeriesStyle::Markers { shape, size } => {
                                plot_ui.points(
                                    Points::new(layer.label.clone(), points)
                                        .color(colour)
                                        .radius((size * 0.5) as f32)
                                        .filled(!matches!(
                                            shape,
                                            MarkerShape::OpenCircle
                                                | MarkerShape::Cross
                                                | MarkerShape::Plus
                                        ))
                                        .shape(to_plot_marker(shape)),
                                );
                            }
                        }
                    }
                }
            });
    }
}

impl eframe::App for PlotterApp {
    // eframe 0.34 renamed the per-frame entry point from `update(ctx, frame)`
    // to `ui(ui, frame)`; `update` is still present but deprecated. The `ui`
    // handed in belongs to the root viewport, so panels are opened on its
    // context exactly as they were before.
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::Panel::left("controls")
            .resizable(true)
            .default_size(360.0)
            .show_inside(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| self.sidebar(ui));
            });

        egui::Panel::bottom("caveats").show_inside(ui, |ui| {
            let active = self.active_layers();
            for note in export::footnotes(self.active_tab, &active) {
                ui.label(egui::RichText::new(note).small());
            }
        });

        egui::CentralPanel::default().show_inside(ui, |ui| self.plot_panel(ui));
    }
}

/// This tool's colour type to egui's.
fn to_color32(colour: Rgb) -> egui::Color32 {
    egui::Color32::from_rgb(colour.r, colour.g, colour.b)
}

/// This tool's marker shapes to `egui_plot`'s nearest equivalents.
fn to_plot_marker(shape: MarkerShape) -> egui_plot::MarkerShape {
    match shape {
        MarkerShape::Circle | MarkerShape::OpenCircle => egui_plot::MarkerShape::Circle,
        MarkerShape::Square => egui_plot::MarkerShape::Square,
        MarkerShape::Triangle => egui_plot::MarkerShape::Up,
        MarkerShape::Diamond => egui_plot::MarkerShape::Diamond,
        MarkerShape::Cross => egui_plot::MarkerShape::Cross,
        MarkerShape::Plus => egui_plot::MarkerShape::Plus,
    }
}

/// Checks that the app's default state produces a drawable first frame's worth
/// of data on every tab.
///
/// # Methodology
///
/// This container has no display, so the GUI cannot be launched here. What
/// *can* be checked is everything the GUI reads: for each tab, the app's
/// default layer selection is built and asserted non-empty, and every point it
/// would hand to `egui_plot` is asserted finite after projection — including
/// under the logarithmic pressure transform, where a non-positive pressure
/// would produce a `NaN` and silently blank a series.
///
/// # Result (measured 2026-08-20)
///
/// Passes on all four tabs with the default layer set.
#[cfg(test)]
#[test]
fn default_app_state_yields_finite_plottable_data_on_every_tab() {
    for tab in DiagramKind::ALL {
        let mut app = PlotterApp::new(std::env::temp_dir(), 60);
        app.active_tab = tab;
        let layers = app.layers_for_active_tab().to_vec();
        assert!(!layers.is_empty(), "{tab:?} has no default layers");
        let log = tab.y_is_pressure();
        for layer in &layers {
            for segment in &layer.segments {
                for point in segment {
                    let [x, y] = tab.project(point);
                    assert!(x.is_finite(), "{tab:?} produced a non-finite abscissa");
                    assert!(y.is_finite(), "{tab:?} produced a non-finite ordinate");
                    if log {
                        assert!(
                            y > 0.0,
                            "{tab:?} has a non-positive pressure for a log axis"
                        );
                    }
                }
            }
        }
    }
}
