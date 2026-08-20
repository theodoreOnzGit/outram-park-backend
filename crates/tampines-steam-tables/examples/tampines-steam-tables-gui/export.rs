//! Scene assembly and file export.
//!
//! # Output location
//!
//! Files land in `figures/property_validation/` **inside this crate**, resolved
//! from `CARGO_MANIFEST_DIR` at compile time rather than from the working
//! directory. That is the directory issue #26 suggests, and resolving it from
//! the manifest follows the convention `tests/edwards_blowdown.rs` already uses
//! for its own CSV output — so `cargo run --example tampines-steam-tables-gui` writes
//! to the same place whether it is launched from the workspace root or from the
//! crate directory. `--out-dir` overrides it.
//!
//! # What one export writes
//!
//! Per diagram, five files:
//!
//! ```text
//! <stem>.png            raster figure
//! <stem>.pdf            vector figure
//! <stem>.svg            vector figure
//! data/<stem>_curves.csv    every computed curve, full state per row
//! data/<stem>_points.csv    every reference point, full state per row
//! ```
//!
//! with `<stem>` from [`DiagramKind::file_stem`], matching the names the issue
//! lists (`ph_validation_coverage`, `mollier_validation_coverage`, …).

use std::path::{Path, PathBuf};

use crate::data::{LayerKind, PlotLayer};
use crate::diagram::DiagramKind;
use crate::figure::layout::PageSize;
use crate::figure::{csv_export, pdf, png, svg, AxisScale, FigurePalette, Scene, Series};
use crate::layers::LayerId;

/// Default output directory, inside the crate, as issue #26 suggests.
pub const DEFAULT_OUT_DIR: &str =
    concat!(env!("CARGO_MANIFEST_DIR"), "/figures/property_validation");

/// A file format the tool can export.
///
/// An enum rather than a trait object: the set is closed, and every consumer
/// should have to handle a new one explicitly.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExportFormat {
    /// Raster figure.
    Png,
    /// Vector figure.
    Pdf,
    /// Vector figure.
    Svg,
    /// Curve and reference-point data.
    Csv,
}

impl ExportFormat {
    /// All four, in the order the export buttons appear.
    pub const ALL: [ExportFormat; 4] = [
        ExportFormat::Png,
        ExportFormat::Pdf,
        ExportFormat::Svg,
        ExportFormat::Csv,
    ];

    /// Button / log label.
    pub fn label(self) -> &'static str {
        match self {
            Self::Png => "PNG",
            Self::Pdf => "PDF",
            Self::Svg => "SVG",
            Self::Csv => "CSV",
        }
    }
}

/// Assembles the [`Scene`] for one diagram from its visible layers.
///
/// `x_scale` and `y_scale` come from the GUI's axis controls. If the layers
/// contain nothing representable the scene keeps its default unit ranges rather
/// than producing an infinite one, and the caller can tell because
/// [`Scene::series`] is empty.
pub fn build_scene(
    diagram: DiagramKind,
    layers: &[PlotLayer],
    x_scale: AxisScale,
    y_scale: AxisScale,
    active: &[LayerId],
) -> Scene {
    let mut scene = Scene::new(diagram.title(), diagram.x_label(), diagram.y_label());
    scene.x_scale = x_scale;
    scene.y_scale = y_scale;

    for layer in layers {
        let mut points: Vec<[f64; 2]> =
            Vec::with_capacity(layer.point_count() + layer.segments.len());
        for (i, segment) in layer.segments.iter().enumerate() {
            if i > 0 {
                // Pen up between segments: a NaN, which the layout pass turns
                // into a break rather than a joining line.
                points.push([f64::NAN, f64::NAN]);
            }
            points.extend(segment.iter().map(|point| diagram.project(point)));
        }
        scene.series.push(Series {
            name: layer.label.clone(),
            style: layer.style,
            colour: layer.colour,
            points,
            show_in_legend: layer.show_in_legend,
        });
    }

    scene.notes = footnotes(diagram, active);
    scene.autoscale();
    scene
}

/// The caveats printed under the axes.
///
/// These are not decoration. The quality-line note is required by issue #26
/// ("quality should not be presented as an independently validated property");
/// the Marviken and Edwards notes exist because a reader of the figure would
/// otherwise reasonably assume both datasets are validated measurements of the
/// plotted state, and neither claim is true.
pub fn footnotes(diagram: DiagramKind, active: &[LayerId]) -> Vec<String> {
    let mut notes = vec![
        "Curves computed live from tampines-steam-tables (IAPWS-IF97). \
         Scattered points are cited reference data, never computed by this tool."
            .to_string(),
    ];
    let has = |id: LayerId| active.contains(&id);

    if has(LayerId::QualityLines)
        && LayerId::QualityLines
            .availability_on(diagram)
            .is_available()
    {
        notes.push(
            "Quality lines use the Region-4 lever rule x = (h - h_f(p)) / (h_g(p) - h_f(p)). \
             Quality is a DERIVED quantity here and is not an independently validated property."
                .to_string(),
        );
    }
    if has(LayerId::MarvikenStates) {
        notes.push(
            "Marviken: test 23 is validated by this crate's V&V (mean deviation 12.6 %); \
             test 24 is NOT validated (mean -48.5 %) and is shown as characterisation only."
                .to_string(),
        );
    }
    if has(LayerId::EdwardsGs1PressureTrace)
        && LayerId::EdwardsGs1PressureTrace
            .availability_on(diagram)
            .is_available()
    {
        notes.push(
            "Edwards-O'Brien GS-1: the measured quantity is PRESSURE only. The plotted \
             temperature is the IF97 saturation temperature at each measured pressure, \
             not a measurement."
                .to_string(),
        );
    }
    if has(LayerId::MoodyStates) || has(LayerId::ZaloudekStates) {
        notes.push(
            "Moody and Zaloudek states are graph-read from published charts; \
             their mass-flux uncertainty is of order 15 %."
                .to_string(),
        );
    }
    notes
}

/// Writes every requested format for one diagram, in `palette`'s colours,
/// returning the paths written.
pub fn write_files(
    out_dir: &Path,
    diagram: DiagramKind,
    layers: &[PlotLayer],
    scene: &Scene,
    formats: &[ExportFormat],
    page: PageSize,
    pixels_per_point: f64,
    palette: FigurePalette,
) -> Result<Vec<PathBuf>, String> {
    std::fs::create_dir_all(out_dir)
        .map_err(|e| format!("could not create {}: {e}", out_dir.display()))?;
    let stem = diagram.file_stem();
    let mut written = Vec::new();

    for format in formats {
        match format {
            ExportFormat::Svg => {
                let path = out_dir.join(format!("{stem}.svg"));
                write(&path, svg::render(scene, page, palette).as_bytes())?;
                written.push(path);
            }
            ExportFormat::Pdf => {
                let path = out_dir.join(format!("{stem}.pdf"));
                write(&path, &pdf::render(scene, page, palette))?;
                written.push(path);
            }
            ExportFormat::Png => {
                let path = out_dir.join(format!("{stem}.png"));
                let bytes = png::render(scene, page, pixels_per_point, palette)?;
                write(&path, &bytes)?;
                written.push(path);
            }
            ExportFormat::Csv => {
                let data_dir = out_dir.join("data");
                std::fs::create_dir_all(&data_dir)
                    .map_err(|e| format!("could not create {}: {e}", data_dir.display()))?;
                let curves_path = data_dir.join(format!("{stem}_curves.csv"));
                write(
                    &curves_path,
                    csv_export::render(layers, diagram, LayerKind::ComputedCurve).as_bytes(),
                )?;
                written.push(curves_path);
                let points_path = data_dir.join(format!("{stem}_points.csv"));
                write(
                    &points_path,
                    csv_export::render(layers, diagram, LayerKind::ReferencePoints).as_bytes(),
                )?;
                written.push(points_path);
            }
        }
    }
    Ok(written)
}

/// Writes exactly one figure to the exact path a save dialog returned,
/// rather than deriving a filename from [`DiagramKind::file_stem`] under some
/// output directory the way [`write_files`] does.
///
/// This is the file-browser export path (issue #26): "clicking Export PNG
/// opens a save dialog" names one destination file, not a directory, so the
/// GUI needs a writer that respects whatever name the user actually chose in
/// the dialog. `format` must be [`ExportFormat::Png`], [`ExportFormat::Pdf`]
/// or [`ExportFormat::Svg`] — CSV export always writes two files (curves and
/// points) and has no single-file form, so it is not accepted here; the GUI
/// routes CSV (and "all formats") through a directory picker and
/// [`write_files`] instead.
///
/// # Errors
///
/// Returns `Err` if `format` is [`ExportFormat::Csv`], or if the write itself
/// fails.
pub fn write_single_file(
    path: &Path,
    scene: &Scene,
    format: ExportFormat,
    page: PageSize,
    pixels_per_point: f64,
    palette: FigurePalette,
) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("could not create {}: {e}", parent.display()))?;
    }
    match format {
        ExportFormat::Svg => write(path, svg::render(scene, page, palette).as_bytes()),
        ExportFormat::Pdf => write(path, &pdf::render(scene, page, palette)),
        ExportFormat::Png => {
            let bytes = png::render(scene, page, pixels_per_point, palette)?;
            write(path, &bytes)
        }
        ExportFormat::Csv => Err(
            "CSV export writes two files (curves and points) and has no single-file \
                 form -- use write_files with a destination directory instead"
                .to_string(),
        ),
    }
}

/// Writes bytes, turning an I/O failure into a message the GUI can show.
fn write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    std::fs::write(path, bytes).map_err(|e| format!("could not write {}: {e}", path.display()))
}

/// Builds every visible layer for one diagram.
pub fn build_layers(
    diagram: DiagramKind,
    active: &[LayerId],
    curve_samples: usize,
) -> Vec<PlotLayer> {
    active
        .iter()
        .filter(|id| id.availability_on(diagram).is_available())
        .flat_map(|id| id.build(diagram, curve_samples))
        .filter(|layer| layer.point_count() > 0)
        .collect()
}

/// Checks that a scene assembled from real layers is well-formed on every tab.
///
/// # Methodology
///
/// For each of the four diagrams, builds the default-visible layers plus every
/// reference-data layer, assembles the scene, and asserts: at least one series
/// survives; the autoscaled ranges are finite and strictly ordered; a log
/// pressure axis never autoscales to a non-positive lower bound (which would
/// silently blank the plot); and the required quality-line footnote is present
/// exactly on the tabs where quality lines are actually drawn.
///
/// # Result (measured 2026-08-20)
///
/// Passes on all four tabs. The quality-line caveat appears on p-h, T-s and
/// h-s, and is correctly absent on T-p, where the quality lines are disabled as
/// degenerate.
#[cfg(test)]
#[test]
fn scenes_assemble_with_finite_ranges_and_the_required_caveats() {
    let active: Vec<LayerId> = LayerId::ALL.to_vec();
    for diagram in DiagramKind::ALL {
        let layers = build_layers(diagram, &active, 60);
        let scene = build_scene(
            diagram,
            &layers,
            AxisScale::Linear,
            diagram.default_y_scale(),
            &active,
        );
        assert!(!scene.series.is_empty(), "{diagram:?} produced no series");
        for (lo, hi) in [scene.x_range, scene.y_range] {
            assert!(
                lo.is_finite() && hi.is_finite(),
                "{diagram:?} range not finite"
            );
            assert!(hi > lo, "{diagram:?} range not ordered");
        }
        if scene.y_scale == AxisScale::Log10 {
            assert!(
                scene.y_range.0 > 0.0,
                "{diagram:?} log axis went non-positive"
            );
        }
        let has_quality_note = scene.notes.iter().any(|note| note.contains("lever rule"));
        assert_eq!(
            has_quality_note,
            LayerId::QualityLines
                .availability_on(diagram)
                .is_available(),
            "quality caveat presence must track whether quality lines are drawn on {diagram:?}"
        );
    }
}

/// Checks [`write_single_file`] — the file-browser save-dialog path (issue
/// #26) — writes exactly the requested format to exactly the path given, and
/// rejects [`ExportFormat::Csv`] since CSV has no single-file form.
///
/// # Methodology
///
/// Builds one small scene, writes it as PNG, PDF and SVG to three arbitrary
/// (non-`file_stem`-shaped) filenames in a temp directory including a nested
/// path that does not yet exist, and asserts each file exists with the right
/// magic bytes. Separately asserts a `Csv` request returns `Err` rather than
/// silently writing something.
///
/// # Result (measured 2026-08-20)
///
/// Passes.
#[cfg(test)]
#[test]
fn write_single_file_writes_the_exact_path_and_rejects_csv() {
    let active: Vec<LayerId> = LayerId::ALL.to_vec();
    let layers = build_layers(DiagramKind::PressureEnthalpy, &active, 30);
    let scene = build_scene(
        DiagramKind::PressureEnthalpy,
        &layers,
        AxisScale::Linear,
        AxisScale::Log10,
        &active,
    );

    let dir = std::env::temp_dir().join(format!(
        "tampines_steam_tables_gui_single_file_gate_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);

    let palette = FigurePalette::LIGHT_PUBLICATION;
    let png_path = dir.join("nested").join("my_custom_name.png");
    write_single_file(
        &png_path,
        &scene,
        ExportFormat::Png,
        PageSize::DEFAULT,
        1.0,
        palette,
    )
    .expect("PNG write succeeds, including creating the nested parent directory");
    let png_bytes = std::fs::read(&png_path).expect("PNG file exists at the exact chosen path");
    assert!(png_bytes.starts_with(b"\x89PNG"), "not a PNG file");

    let pdf_path = dir.join("whatever.pdf");
    write_single_file(
        &pdf_path,
        &scene,
        ExportFormat::Pdf,
        PageSize::DEFAULT,
        1.0,
        palette,
    )
    .expect("PDF write succeeds");
    let pdf_bytes = std::fs::read(&pdf_path).expect("PDF file exists at the exact chosen path");
    assert!(pdf_bytes.starts_with(b"%PDF-"), "not a PDF file");

    let svg_path = dir.join("figure.svg");
    write_single_file(
        &svg_path,
        &scene,
        ExportFormat::Svg,
        PageSize::DEFAULT,
        1.0,
        palette,
    )
    .expect("SVG write succeeds");
    let svg_text = std::fs::read_to_string(&svg_path).expect("SVG file exists at the chosen path");
    assert!(svg_text.starts_with("<svg"), "not an SVG file");

    let csv_path = dir.join("should_not_be_written.csv");
    let result = write_single_file(
        &csv_path,
        &scene,
        ExportFormat::Csv,
        PageSize::DEFAULT,
        1.0,
        palette,
    );
    assert!(
        result.is_err(),
        "CSV has no single-file form and must be rejected"
    );
    assert!(
        !csv_path.exists(),
        "a rejected CSV request must not write anything"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

/// Checks that a non-default [`FigurePalette`] actually reaches the raster
/// output, end to end through [`write_single_file`] — the publication export
/// style selector (issue #26: "Export style: Current theme / Light
/// publication / Dark / Gruvbox") would be a silent no-op if any of the three
/// backends had kept a hardcoded background.
///
/// # Methodology
///
/// Writes the same scene to PNG twice, once with
/// [`FigurePalette::LIGHT_PUBLICATION`] and once with [`FigurePalette::DARK`],
/// and decodes the top-left corner pixel of each — a point far from any
/// plotted curve, so it reads pure page background. Asserts the two corner
/// pixels differ, and that each matches its requested palette's `background`
/// to within `+-2` (the PNG encoder is 8-bit sRGB; some paths round rather
/// than truncate).
///
/// # Result (measured 2026-08-20)
///
/// Passes: light-publication corner `(255,255,255)`, dark corner `(24,24,26)`.
#[cfg(test)]
#[test]
fn export_style_palette_reaches_the_rendered_png() {
    let active: Vec<LayerId> = LayerId::ALL.to_vec();
    let layers = build_layers(DiagramKind::PressureEnthalpy, &active, 30);
    let scene = build_scene(
        DiagramKind::PressureEnthalpy,
        &layers,
        AxisScale::Linear,
        AxisScale::Log10,
        &active,
    );

    let dir = std::env::temp_dir().join(format!(
        "tampines_steam_tables_gui_export_style_gate_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);

    let corner_pixel = |path: &std::path::Path, palette: FigurePalette| -> [u8; 3] {
        write_single_file(
            path,
            &scene,
            ExportFormat::Png,
            PageSize::DEFAULT,
            1.0,
            palette,
        )
        .expect("PNG write succeeds");
        let bytes = std::fs::read(path).expect("PNG file exists");
        let decoded = image::load_from_memory(&bytes)
            .expect("PNG decodes")
            .to_rgb8();
        let p = decoded.get_pixel(0, 0);
        [p.0[0], p.0[1], p.0[2]]
    };

    let close = |a: u8, b: u8| (i16::from(a) - i16::from(b)).abs() <= 2;

    let light_path = dir.join("light.png");
    let light_corner = corner_pixel(&light_path, FigurePalette::LIGHT_PUBLICATION);
    let light_bg = FigurePalette::LIGHT_PUBLICATION.background;
    assert!(
        close(light_corner[0], light_bg.r)
            && close(light_corner[1], light_bg.g)
            && close(light_corner[2], light_bg.b),
        "light-publication corner {light_corner:?} does not match its background {light_bg:?}"
    );

    let dark_path = dir.join("dark.png");
    let dark_corner = corner_pixel(&dark_path, FigurePalette::DARK);
    let dark_bg = FigurePalette::DARK.background;
    assert!(
        close(dark_corner[0], dark_bg.r)
            && close(dark_corner[1], dark_bg.g)
            && close(dark_corner[2], dark_bg.b),
        "dark corner {dark_corner:?} does not match its background {dark_bg:?}"
    );

    assert_ne!(
        light_corner, dark_corner,
        "the two export styles must actually produce different backgrounds"
    );

    let _ = std::fs::remove_dir_all(&dir);
}
