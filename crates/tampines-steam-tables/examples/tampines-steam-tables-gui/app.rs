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
use crate::custom_lines::{self, CustomLine, CustomLineType};
use crate::layers::{LayerId, LayerSelection};
use crate::theme::GuiTheme;

/// How the live canvas's legend is shown (issue #26: "Legend mode dropdown:
/// Off / Compact / Full. Default should be Compact.").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LegendMode {
    /// No legend widget at all.
    Off,
    /// One row per [`PlotLayer::legend_group`] — nine isotherms, five
    /// quality lines, or seven reference datasets each collapse to one row.
    Compact,
    /// One row per individual [`PlotLayer`] — every isotherm, isobar and
    /// quality line gets its own entry.
    Full,
}

/// What the file-browser dialog will do once the user picks a path,
/// remembered between the click that opened the dialog and the (possibly
/// later) frame the pick actually resolves on. `DiagramKind` and
/// [`crate::figure::FigurePalette`] are both pinned at dialog-open time — see
/// [`PlotterApp::export`].
enum PendingExport {
    /// A single PNG/PDF/SVG file, written to the exact path picked.
    SingleFile(DiagramKind, ExportFormat, crate::figure::FigurePalette),
    /// One or more formats, written into the picked directory using the
    /// usual `<stem>.<ext>` / `data/<stem>_*.csv` layout.
    Directory(DiagramKind, Vec<ExportFormat>, crate::figure::FigurePalette),
}

/// The palette an exported PNG/PDF/SVG figure uses (issue #26: "Export
/// style: Current theme / Light publication / Dark / Gruvbox").
///
/// This is independent of [`LegendMode`] and of [`GuiTheme`] — the GUI chrome
/// can be in Gruvbox Dark while the user still exports a Light-publication
/// figure for a paper, or vice versa.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExportStyle {
    /// White background, black ink (issue #26's own fallback default: "If
    /// only one export style is implemented first, choose Light
    /// publication"). Unchanged from this tool's original figure output.
    LightPublication,
    /// Mirrors whichever [`GuiTheme`] is currently applied to the GUI.
    CurrentTheme,
    /// A generic dark export palette, independent of the active `GuiTheme`.
    Dark,
    /// The Gruvbox Dark palette, regardless of which theme is active in the
    /// GUI chrome.
    Gruvbox,
}

impl ExportStyle {
    /// All four, in the order the selector shows them.
    pub const ALL: [ExportStyle; 4] = [
        ExportStyle::LightPublication,
        ExportStyle::CurrentTheme,
        ExportStyle::Dark,
        ExportStyle::Gruvbox,
    ];

    /// Selector label.
    pub fn label(self) -> &'static str {
        match self {
            Self::LightPublication => "Light publication",
            Self::CurrentTheme => "Current theme",
            Self::Dark => "Dark",
            Self::Gruvbox => "Gruvbox",
        }
    }

    /// Resolves this style to a concrete [`crate::figure::FigurePalette`].
    /// `app_theme` and `dark_mode` are only consulted for `CurrentTheme`.
    pub fn palette(self, app_theme: GuiTheme, dark_mode: bool) -> crate::figure::FigurePalette {
        match self {
            Self::LightPublication => crate::figure::FigurePalette::LIGHT_PUBLICATION,
            Self::CurrentTheme => app_theme.figure_palette(dark_mode),
            Self::Dark => crate::figure::FigurePalette::DARK,
            Self::Gruvbox => GuiTheme::GruvboxDark.figure_palette(false),
        }
    }
}

impl LegendMode {
    /// All three, in the order the selector shows them.
    pub const ALL: [LegendMode; 3] = [LegendMode::Off, LegendMode::Compact, LegendMode::Full];

    /// Selector label.
    pub fn label(self) -> &'static str {
        match self {
            Self::Off => "Off",
            Self::Compact => "Compact",
            Self::Full => "Full",
        }
    }
}

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
            .with_title("TAMPINES Steam Tables GUI — T-p / p-h / T-s / h-s plotter"),
        ..Default::default()
    };
    // The `Box<dyn ...>` here is `eframe::run_native`'s own signature, not a
    // dispatch choice of this example's; every enum in this tool is dispatched
    // by `match`, per the workspace Rust design rules.
    eframe::run_native(
        "tampines-steam-tables-gui",
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
    /// Which of [`crate::curves::DEFAULT_ISOBARS_BAR`] the
    /// [`LayerId::Isobars`] checkbox actually draws when it is on, one flag
    /// per entry in the same order. Issue #26's follow-up: the checkbox used
    /// to be all-or-nothing; this is what the sidebar's multi-select dropdown
    /// edits instead of it drawing every default value.
    isobar_selected: Vec<bool>,
    /// Same as [`PlotterApp::isobar_selected`], for
    /// [`crate::curves::DEFAULT_ISOTHERMS_DEGC`] / [`LayerId::Isotherms`].
    isotherm_selected: Vec<bool>,
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
    /// The selected GUI theme.
    theme: GuiTheme,
    /// The theme actually applied to the `egui::Context` so far, so
    /// [`GuiTheme::apply`] runs once at startup and again only on change,
    /// rather than every frame.
    applied_theme: Option<GuiTheme>,
    /// The live canvas's legend mode.
    legend_mode: LegendMode,
    /// The in-app export-path picker (issue #26's file-browser export
    /// request). Pure `egui`, no GTK/native-dialog backend — see this
    /// example's `Cargo.toml` for why `egui-file-dialog` 0.13.0 specifically.
    file_dialog: egui_file_dialog::FileDialog,
    /// What to do once [`PlotterApp::file_dialog`] resolves to a picked path.
    pending_export: Option<PendingExport>,
    /// The palette exported figures use.
    export_style: ExportStyle,
    /// User-added custom thermodynamic lines.
    custom_lines: Vec<CustomLine>,
    /// The "Add line" control's currently selected type.
    custom_line_type: CustomLineType,
    /// The "Add line" control's currently selected value, in
    /// `custom_line_type.unit_display()`.
    custom_line_value: f64,
    /// Set by the "Reset plot" button; consumed (and cleared) by the next
    /// frame's plot panel.
    reset_view_requested: bool,
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
            isobar_selected: vec![true; crate::curves::DEFAULT_ISOBARS_BAR.len()],
            isotherm_selected: vec![true; crate::curves::DEFAULT_ISOTHERMS_DEGC.len()],
            curve_samples: curve_samples.clamp(40, 2000),
            log_pressure: true,
            out_dir,
            export_pixels_per_point: DEFAULT_PIXELS_PER_POINT,
            cache: HashMap::new(),
            status: "ready".to_string(),
            theme: GuiTheme::System,
            applied_theme: None,
            legend_mode: LegendMode::Compact,
            file_dialog: egui_file_dialog::FileDialog::new(),
            pending_export: None,
            export_style: ExportStyle::LightPublication,
            custom_lines: Vec::new(),
            custom_line_type: CustomLineType::Isobar,
            custom_line_value: CustomLineType::Isobar.default_value(),
            reset_view_requested: false,
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

    /// Which isobars/isotherms [`LayerId::Isobars`] and [`LayerId::Isotherms`]
    /// draw right now, per [`PlotterApp::isobar_selected`] /
    /// [`PlotterApp::isotherm_selected`].
    fn layer_selection(&self) -> LayerSelection {
        LayerSelection {
            isobars_bar: crate::curves::DEFAULT_ISOBARS_BAR
                .iter()
                .zip(&self.isobar_selected)
                .filter_map(|(value, on)| on.then_some(*value))
                .collect(),
            isotherms_degc: crate::curves::DEFAULT_ISOTHERMS_DEGC
                .iter()
                .zip(&self.isotherm_selected)
                .filter_map(|(value, on)| on.then_some(*value))
                .collect(),
        }
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
            let built =
                export::build_layers(tab, &active, self.curve_samples, &self.layer_selection());
            self.cache.insert(tab, built);
        }
        self.cache.get(&tab).map(Vec::as_slice).unwrap_or(&[])
    }

    /// Every user-added custom line, built into [`PlotLayer`]s. Not cached
    /// like [`PlotterApp::layers_for_active_tab`]'s built-in layers — there
    /// are typically only a handful, so rebuilding every frame is cheap, and
    /// it means a line drawn on one diagram is drawn identically (same
    /// underlying state, just a different projection) on every other.
    fn custom_layers(&self) -> Vec<PlotLayer> {
        self.custom_lines
            .iter()
            .enumerate()
            .filter_map(|(index, line)| {
                line.build(self.curve_samples, custom_lines::colour_for_index(index))
            })
            .collect()
    }

    /// Axis scale for the current tab's y axis.
    fn y_scale(&self) -> AxisScale {
        if self.active_tab.y_is_pressure() && self.log_pressure {
            AxisScale::Log10
        } else {
            AxisScale::Linear
        }
    }

    /// Runs an export of `tab` into `dir`, in `palette`'s colours, remembering
    /// `dir` as the export directory the file-browser dialog opens in next
    /// time. `tab` and `palette` are whichever diagram and export style were
    /// active when the export was *requested* — pinned explicitly rather than
    /// re-read from `self.active_tab`/`self.export_style`, since a
    /// save/directory dialog can stay open across frames in which the user
    /// changes either.
    fn export(
        &mut self,
        tab: DiagramKind,
        dir: &std::path::Path,
        formats: &[ExportFormat],
        palette: crate::figure::FigurePalette,
    ) {
        let active = self.active_layers();
        let samples = self.curve_samples;
        let pixels = self.export_pixels_per_point;
        let y_scale = self.y_scale();

        let mut built = export::build_layers(tab, &active, samples, &self.layer_selection());
        built.extend(self.custom_layers());
        let scene = export::build_scene(tab, &built, AxisScale::Linear, y_scale, &active);
        self.status = match export::write_files(
            dir,
            tab,
            &built,
            &scene,
            formats,
            PageSize::DEFAULT,
            pixels,
            palette,
        ) {
            Ok(paths) => format!("wrote {} file(s) to {}", paths.len(), dir.display()),
            Err(message) => format!("export failed: {message}"),
        };
        self.out_dir = dir.to_path_buf();
    }

    /// Runs a single-format export of `tab` to an exact file path a save
    /// dialog returned (issue #26's file-browser export request — PNG, PDF
    /// and SVG only; CSV always writes two files and goes through
    /// [`PlotterApp::export`] with a directory instead). See [`PlotterApp::export`]
    /// on why `tab` is pinned rather than read from `self.active_tab`.
    fn export_single(
        &mut self,
        tab: DiagramKind,
        path: &std::path::Path,
        format: ExportFormat,
        palette: crate::figure::FigurePalette,
    ) {
        let active = self.active_layers();
        let samples = self.curve_samples;
        let pixels = self.export_pixels_per_point;
        let y_scale = self.y_scale();

        let mut built = export::build_layers(tab, &active, samples, &self.layer_selection());
        built.extend(self.custom_layers());
        let scene = export::build_scene(tab, &built, AxisScale::Linear, y_scale, &active);
        self.status = match export::write_single_file(
            path,
            &scene,
            format,
            PageSize::DEFAULT,
            pixels,
            palette,
        ) {
            Ok(()) => format!("wrote {}", path.display()),
            Err(message) => format!("export failed: {message}"),
        };
        if let Some(parent) = path.parent() {
            self.out_dir = parent.to_path_buf();
        }
    }

    /// Draws one layer's checkbox row — or, when unavailable, its greyed-out
    /// row with the reason shown alongside it, per this tool's "never
    /// silently absent, never filled in" rule. On a real toggle, updates
    /// `self.visible[index]` and the status line, and returns whether
    /// visibility changed (the caller uses this to decide whether to
    /// invalidate the layer cache).
    fn layer_row(
        &mut self,
        ui: &mut egui::Ui,
        id: LayerId,
        tab: DiagramKind,
        index: usize,
        on: &mut bool,
    ) -> bool {
        let availability = id.availability_on(tab);
        match availability.reason() {
            None => {
                if ui.checkbox(on, id.label()).changed() {
                    self.visible[index].1 = *on;
                    self.status = format!(
                        "{} {}",
                        if *on { "enabled" } else { "disabled" },
                        id.label()
                    );
                    true
                } else {
                    false
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
                false
            }
        }
    }

    /// Draws a multi-select dropdown next to [`LayerId::Isobars`] /
    /// [`LayerId::Isotherms`]'s checkbox row (issue #26's follow-up: *"the
    /// isotherm checkbox switches all of the isotherms on... give me a
    /// drop-down menu to select which isotherms I want to add and plot"*).
    /// The dropdown popup stays open across clicks, so multiple boxes can be
    /// (un)checked in one interaction rather than reopening it each time.
    /// Returns whether the selection changed, so the caller knows to
    /// invalidate the layer cache.
    fn multi_select_row(
        ui: &mut egui::Ui,
        id_salt: &str,
        values: &[f64],
        unit: &str,
        selected: &mut [bool],
    ) -> bool {
        let mut changed = false;
        let n_selected = selected.iter().filter(|on| **on).count();
        ui.horizontal(|ui| {
            ui.add_space(18.0);
            ui.label("select:");
            egui::ComboBox::from_id_salt(id_salt)
                .selected_text(format!("{n_selected}/{} shown", values.len()))
                .show_ui(ui, |ui| {
                    for (value, on) in values.iter().zip(selected.iter_mut()) {
                        if ui.checkbox(on, format!("{value} {unit}")).changed() {
                            changed = true;
                        }
                    }
                });
            if ui.small_button("all").clicked() {
                selected.iter_mut().for_each(|on| *on = true);
                changed = true;
            }
            if ui.small_button("none").clicked() {
                selected.iter_mut().for_each(|on| *on = false);
                changed = true;
            }
        });
        changed
    }

    /// The control sidebar.
    ///
    /// Sectioned per issue #26's suggested order: Diagram, Theme, Legend,
    /// Computed curves, Custom lines, Reference / validation data, Axes and
    /// resolution, Export, Status. Long explanatory prose is kept out of the
    /// section bodies and attached as a hover tooltip on the heading instead
    /// (issue: "Move long explanatory text into tooltips where possible").
    fn sidebar(&mut self, ui: &mut egui::Ui) {
        ui.heading("Diagram");
        for kind in DiagramKind::ALL {
            if ui
                .selectable_label(self.active_tab == kind, kind.tab_label())
                .clicked()
            {
                self.active_tab = kind;
                self.status = format!("switched to {}", kind.tab_label());
            }
        }

        ui.separator();
        ui.heading("Theme");
        ui.horizontal_wrapped(|ui| {
            for candidate in GuiTheme::ALL {
                ui.selectable_value(&mut self.theme, candidate, candidate.label());
            }
        });

        ui.separator();
        ui.heading("Legend").on_hover_text(
            "Off: no legend. Compact (default): a whole family of curves \
                 -- e.g. every isotherm -- shares one row. Full: one row per curve.",
        );
        ui.horizontal_wrapped(|ui| {
            for candidate in LegendMode::ALL {
                ui.selectable_value(&mut self.legend_mode, candidate, candidate.label());
            }
        });

        ui.separator();
        ui.heading("Computed curves").on_hover_text(
            "Computed live from IAPWS-IF97 on every rebuild -- never a stored table.",
        );
        let tab = self.active_tab;
        let mut changed = false;
        for index in 0..self.visible.len() {
            let (id, mut on) = self.visible[index];
            if id.kind() != LayerKind::ComputedCurve {
                continue;
            }
            if self.layer_row(ui, id, tab, index, &mut on) {
                changed = true;
            }
            if !id.availability_on(tab).is_available() {
                continue;
            }
            match id {
                LayerId::Isobars => {
                    if Self::multi_select_row(
                        ui,
                        "isobar_multiselect",
                        &crate::curves::DEFAULT_ISOBARS_BAR,
                        "bar",
                        &mut self.isobar_selected,
                    ) {
                        changed = true;
                    }
                }
                LayerId::Isotherms => {
                    if Self::multi_select_row(
                        ui,
                        "isotherm_multiselect",
                        &crate::curves::DEFAULT_ISOTHERMS_DEGC,
                        "\u{00B0}C",
                        &mut self.isotherm_selected,
                    ) {
                        changed = true;
                    }
                }
                _ => {}
            }
        }

        ui.separator();
        ui.heading("Custom lines").on_hover_text(
            "Add an isobar/isotherm at any value, not just the defaults above, \
                 or an isentrope/isenthalp/isochore. Each is computed live and included in \
                 CSV export.",
        );
        if let Some(line) = custom_lines::add_line_controls(
            ui,
            &mut self.custom_line_type,
            &mut self.custom_line_value,
        ) {
            self.status = format!(
                "added custom {} = {:.4} {}",
                line.line_type.label(),
                line.value,
                line.line_type.unit_display()
            );
            self.custom_lines.push(line);
        }
        if !self.custom_lines.is_empty() {
            let mut remove_index = None;
            for (index, line) in self.custom_lines.iter().enumerate() {
                ui.horizontal(|ui| {
                    ui.label(format!(
                        "{} {} = {:.4} {}",
                        index + 1,
                        line.line_type.label(),
                        line.value,
                        line.line_type.unit_display()
                    ));
                    if ui.small_button("remove").clicked() {
                        remove_index = Some(index);
                    }
                });
            }
            if let Some(index) = remove_index {
                let removed = self.custom_lines.remove(index);
                self.status = format!(
                    "removed custom {} = {:.4} {}",
                    removed.line_type.label(),
                    removed.value,
                    removed.line_type.unit_display()
                );
            }
        }
        ui.horizontal_wrapped(|ui| {
            if ui.button("Clear custom lines").clicked() {
                let count = self.custom_lines.len();
                self.custom_lines.clear();
                self.status = format!("cleared {count} custom line(s)");
            }
            if ui.button("Clear reference overlays").clicked() {
                for (id, on) in &mut self.visible {
                    if id.kind() == LayerKind::ReferencePoints {
                        *on = false;
                    }
                }
                self.status = "cleared reference / validation overlays".to_string();
                changed = true;
            }
            if ui.button("Clear all overlays").clicked() {
                self.custom_lines.clear();
                for (id, on) in &mut self.visible {
                    *on = matches!(
                        id,
                        LayerId::SaturationDome
                            | LayerId::SaturatedLiquidLine
                            | LayerId::SaturatedVapourLine
                            | LayerId::CriticalPoint
                            | LayerId::TriplePoint
                    );
                }
                self.status = "cleared custom lines and optional overlays".to_string();
                changed = true;
            }
            if ui.button("Reset plot").clicked() {
                self.reset_view_requested = true;
                self.status = "reset plot view".to_string();
            }
        });

        ui.separator();
        ui.heading("Reference / validation data").on_hover_text(
            "Cited measurements, digitised or published, never computed by \
                 this tool. A greyed-out layer is one that cannot be honestly drawn here -- \
                 the reason is on hover. Missing data is disabled, never invented.",
        );
        for index in 0..self.visible.len() {
            let (id, mut on) = self.visible[index];
            if id.kind() != LayerKind::ReferencePoints {
                continue;
            }
            if self.layer_row(ui, id, tab, index, &mut on) {
                changed = true;
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
        ui.heading("Export").on_hover_text(
            "PNG/PDF/SVG open a save-file dialog; CSV and \"all formats\" open \
                 a directory picker, since CSV always writes two files.",
        );
        ui.label(egui::RichText::new("Export style").strong())
            .on_hover_text(
                "the exported figure's background/ink/grid; every plotted curve \
                 keeps its own colour regardless of style",
            );
        ui.horizontal_wrapped(|ui| {
            for candidate in ExportStyle::ALL {
                ui.selectable_value(&mut self.export_style, candidate, candidate.label());
            }
        });
        ui.label(
            egui::RichText::new(format!("default location: {}", self.out_dir.display()))
                .small()
                .weak(),
        );
        let tab = self.active_tab;
        let palette = self
            .export_style
            .palette(self.theme, ui.visuals().dark_mode);
        ui.horizontal_wrapped(|ui| {
            for format in ExportFormat::ALL {
                if ui.button(format.label()).clicked() {
                    match format {
                        ExportFormat::Csv => {
                            self.file_dialog.config_mut().initial_directory = self.out_dir.clone();
                            self.file_dialog.pick_directory();
                            self.pending_export = Some(PendingExport::Directory(
                                tab,
                                vec![ExportFormat::Csv],
                                palette,
                            ));
                        }
                        ExportFormat::Png | ExportFormat::Pdf | ExportFormat::Svg => {
                            let ext = match format {
                                ExportFormat::Png => "png",
                                ExportFormat::Pdf => "pdf",
                                ExportFormat::Svg => "svg",
                                ExportFormat::Csv => unreachable!(),
                            };
                            self.file_dialog.config_mut().initial_directory = self.out_dir.clone();
                            self.file_dialog.config_mut().default_file_name =
                                format!("{}.{ext}", tab.file_stem());
                            self.file_dialog.save_file();
                            self.pending_export =
                                Some(PendingExport::SingleFile(tab, format, palette));
                        }
                    }
                }
            }
            if ui.button("all formats").clicked() {
                self.file_dialog.config_mut().initial_directory = self.out_dir.clone();
                self.file_dialog.pick_directory();
                self.pending_export = Some(PendingExport::Directory(
                    tab,
                    ExportFormat::ALL.to_vec(),
                    palette,
                ));
            }
        });
        ui.separator();
        ui.heading("Status");
        ui.label(egui::RichText::new(&self.status).small());

        if changed {
            self.invalidate();
        }
    }

    /// The plot panel.
    fn plot_panel(&mut self, ui: &mut egui::Ui) {
        let tab = self.active_tab;
        let log = self.y_scale() == AxisScale::Log10;
        let legend_mode = self.legend_mode;
        let y_label = if log {
            "log10(Pressure p / bar)".to_string()
        } else {
            tab.y_label().to_string()
        };
        let mut layers: Vec<PlotLayer> = self.layers_for_active_tab().to_vec();
        let custom_built = self.custom_layers();
        // Issue #26: "The GUI should not panic if property calls fail... It
        // should skip invalid points and show a warning." A custom line whose
        // value cannot be evaluated anywhere along its sweep (out of range,
        // or landing entirely in a gap this crate declines) is dropped by
        // `CustomLine::build` rather than fabricated -- surface that here so
        // the drop is visible, not silent.
        let skipped = self.custom_lines.len() - custom_built.len();
        if skipped > 0 {
            self.status =
                format!("skipped {skipped} custom line(s) -- no evaluable point in range");
        }
        layers.extend(custom_built);
        // `crate::figure::INK` is fixed black — right for the exported figure
        // (always paper-white), but black-on-dark-background is close to
        // invisible on the live canvas in a dark theme. Substitute a
        // theme-aware ink colour there so the saturation dome, critical/triple
        // points and region boundaries stay the dominant, high-contrast
        // curves issue #26 asks for in *every* theme, not just light ones.
        let live_ink = crate::theme::live_ink_colour(ui.visuals().dark_mode);

        let mut plot = Plot::new(tab.tab_label());
        // "Legend mode dropdown: Off / Compact / Full" (issue #26). `Off`
        // never attaches a legend at all; `Compact` (the default) and `Full`
        // both attach egui_plot's own legend widget, which merges
        // same-named series into one row -- the difference between the two
        // is entirely in which name each layer is given below.
        if legend_mode != LegendMode::Off {
            plot = plot.legend(Legend::default());
        }
        // "Reset plot" clear button (issue #26): forces egui_plot to
        // recompute auto-bounds this frame, the same effect a double-click on
        // the canvas has.
        if self.reset_view_requested {
            plot = plot.reset();
            self.reset_view_requested = false;
        }
        plot.x_axis_label(tab.x_label())
            .y_axis_label(y_label)
            .allow_scroll(false)
            // Hover coordinates with units, on every diagram (issue #26):
            // `label_formatter` customises the tooltip egui_plot already
            // shows when the pointer is near a plotted line/point;
            // `coordinates_formatter` adds a corner-anchored readout that
            // tracks the pointer everywhere in the plot area, not just near
            // data, since the request was "hover my cursor over the graph",
            // not "hover over a curve".
            .label_formatter(move |name, value| {
                let coords = format!("{}\n{}", tab.x_hover(value.x), tab.y_hover(value.y, log));
                if name.is_empty() {
                    coords
                } else {
                    format!("{name}\n{coords}")
                }
            })
            .coordinates_formatter(
                egui_plot::Corner::LeftTop,
                egui_plot::CoordinatesFormatter::new(move |point, _bounds| {
                    format!("{}\n{}", tab.x_hover(point.x), tab.y_hover(point.y, log))
                }),
            )
            .show(ui, |plot_ui| {
                for layer in &layers {
                    // Compact groups a whole family of curves (isotherms,
                    // quality lines, the seven reference datasets, …) under
                    // one legend row by giving every member of the family the
                    // same egui_plot series name (LegendGrouping::ByName, the
                    // default, then merges them); Off and Full both keep each
                    // layer's own distinct name.
                    let series_name = if legend_mode == LegendMode::Compact {
                        layer.legend_group.to_string()
                    } else {
                        layer.label.clone()
                    };
                    let colour = if layer.colour == crate::figure::INK {
                        live_ink
                    } else {
                        to_color32(layer.colour)
                    };
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
                                let mut line = Line::new(series_name.clone(), points)
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
                                    Points::new(series_name.clone(), points)
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
        if self.applied_theme != Some(self.theme) {
            self.theme.apply(ui.ctx());
            self.applied_theme = Some(self.theme);
        }

        // Drives the file-browser export dialog (issue #26). `update` both
        // advances its state machine and draws it when open; once it
        // resolves to a picked path, dispatch on what the click that opened
        // it asked for.
        self.file_dialog.update(ui.ctx());
        if let Some(path) = self.file_dialog.take_picked() {
            match self.pending_export.take() {
                Some(PendingExport::SingleFile(tab, format, palette)) => {
                    self.export_single(tab, &path, format, palette);
                }
                Some(PendingExport::Directory(tab, formats, palette)) => {
                    self.export(tab, &path, &formats, palette);
                }
                None => {}
            }
        }

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
