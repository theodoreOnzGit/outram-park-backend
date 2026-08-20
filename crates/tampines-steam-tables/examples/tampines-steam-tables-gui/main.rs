//! # TAMPINES Steam Tables GUI — interactive T-p, p-h, T-s and h-s diagrams
//!
//! An `eframe`/`egui` tool for **figure generation and interactive inspection**
//! of the IAPWS-IF97 surface implemented by `tampines-steam-tables`. It is not a
//! simulator and contains no solver: it plots, exports, and gets out of the way.
//!
//! Implements [GitHub issue #26](https://github.com/theodoreOnzGit/outram-park-backend/issues/26),
//! plus the maintainer's extension to **four** tabs (the issue required p-h and
//! h-s and listed T-s as optional; T-p was added on top). Renamed from
//! `steam_table_plotter` to `tampines-steam-tables-gui` per the issue's
//! branding request.
//!
//! ## Running it
//!
//! ```bash
//! # interactive, needs a display
//! cargo run --release -p tampines-steam-tables --example tampines-steam-tables-gui
//!
//! # headless: write every figure and CSV for all four diagrams and exit
//! cargo run --release -p tampines-steam-tables --example tampines-steam-tables-gui -- --export-all
//!
//! # its self-checks, including the saturation-curve V&V gate
//! cargo test --release -p tampines-steam-tables --example tampines-steam-tables-gui
//! ```
//!
//! `--export-all` exists because the export path is a pure function of the
//! scene, with no dependency on egui, a window or a GPU. That makes the figures
//! reproducible from a script or a CI box with no display, and it is how the
//! export code is actually tested.
//!
//! ## What is computed and what is cited
//!
//! This distinction runs through the whole tool and is worth stating once,
//! plainly:
//!
//! * **Curves** — saturation dome, saturated liquid and vapour lines, quality
//!   lines, isobars, isotherms, region boundaries, the critical and triple
//!   points — are computed **live**, on every rebuild, from this crate's own
//!   IAPWS-IF97 routines. Nothing is a stored table. See [`curves`].
//! * **Scattered points** are **cited reference data**, copied verbatim from
//!   this crate's own test fixtures with their provenance. Nothing is invented.
//!   See [`reference_data`].
//!
//! Where a layer cannot be honestly drawn — because the data does not exist, or
//! because the curve would be degenerate in those coordinates — it is
//! **disabled with a stated reason**, never filled in. See [`layers`].
//!
//! ## Verification and validation
//!
//! The tool carries its own gates, run by `cargo test --example
//! tampines-steam-tables-gui`. The load-bearing one is
//! `curves::saturation_curve_matches_the_wagner_steam_table`, which compares
//! the plotted dome against all 220 rows of Kretzschmar & Wagner's published
//! saturation table: **passes to 0.5 % on `p_sat` and to max(0.5 %, 1 kJ/kg) on
//! `h_f` and `h_g` from 0 °C to within 1 K of the critical temperature**
//! (measured 2026-08-20). The others cover the lever rule, the critical and
//! triple-point markers, the availability table, axis transforms, clipping,
//! PDF cross-reference offsets, PNG decoding, and byte-reproducibility of the
//! SVG, PDF and CSV exports.
//!
//! ## Android
//!
//! The `egui`/`eframe` stack is Android-hostile (it pulls `ring`, which needs an
//! Android C toolchain), so this example follows the same pattern as
//! `examples/fhr_sim_v1/main.rs`: on Android the whole thing is an empty
//! `main`, and every other item is gated off Android. The crate's GUI
//! dev-dependencies are already declared under
//! `[target.'cfg(not(target_os = "android"))'.dev-dependencies]`.

/// Android stub: this GUI example does not build on Android, and the whole
/// program compiles to an empty `main` there.
#[cfg(target_os = "android")]
fn main() {}

#[cfg(not(target_os = "android"))]
mod app;
#[cfg(not(target_os = "android"))]
mod curves;
#[cfg(not(target_os = "android"))]
mod custom_lines;
#[cfg(not(target_os = "android"))]
mod data;
#[cfg(not(target_os = "android"))]
mod diagram;
#[cfg(not(target_os = "android"))]
mod export;
#[cfg(not(target_os = "android"))]
mod figure;
#[cfg(not(target_os = "android"))]
mod layers;
#[cfg(not(target_os = "android"))]
mod reference_data;
#[cfg(not(target_os = "android"))]
mod theme;

#[cfg(not(target_os = "android"))]
fn main() {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    if arguments.iter().any(|a| a == "--help" || a == "-h") {
        print!("{}", usage());
        return;
    }

    let out_dir = flag_value(&arguments, "--out-dir")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from(export::DEFAULT_OUT_DIR));
    let samples = flag_value(&arguments, "--samples")
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(400);

    if arguments.iter().any(|a| a == "--export-all") {
        match export_all(&out_dir, samples) {
            Ok(count) => println!(
                "wrote {count} files to {} (4 diagrams x [png, pdf, svg, curves.csv, points.csv])",
                out_dir.display()
            ),
            Err(message) => {
                eprintln!("export failed: {message}");
                std::process::exit(1);
            }
        }
        return;
    }

    if let Err(error) = app::run(out_dir, samples) {
        eprintln!("could not start the GUI: {error}");
        eprintln!("(no display? use --export-all to write every figure headlessly)");
        std::process::exit(1);
    }
}

/// Command-line help.
#[cfg(not(target_os = "android"))]
fn usage() -> String {
    format!(
        "tampines-steam-tables-gui -- IAPWS-IF97 T-p / p-h / T-s / h-s diagrams from \
tampines-steam-tables

USAGE:
    tampines-steam-tables-gui [OPTIONS]

OPTIONS:
    --export-all        write PNG, PDF, SVG and CSV for all four diagrams, then
                        exit. Needs no display.
    --out-dir <DIR>     output directory
                        [default: {}]
    --samples <N>       samples per computed curve [default: 400]
    -h, --help          show this help

With no options, opens the interactive window.
",
        export::DEFAULT_OUT_DIR
    )
}

/// Reads `--flag value` out of the argument list.
#[cfg(not(target_os = "android"))]
fn flag_value(arguments: &[String], flag: &str) -> Option<String> {
    let index = arguments.iter().position(|a| a == flag)?;
    arguments.get(index + 1).cloned()
}

/// Headless export of all four diagrams with every layer that has real data
/// behind it switched on.
#[cfg(not(target_os = "android"))]
fn export_all(out_dir: &std::path::Path, samples: usize) -> Result<usize, String> {
    use diagram::DiagramKind;
    use figure::layout::PageSize;
    use figure::png::DEFAULT_PIXELS_PER_POINT;
    use layers::LayerId;

    let active: Vec<LayerId> = LayerId::ALL.to_vec();
    let mut total = 0usize;
    for kind in DiagramKind::ALL {
        let built = export::build_layers(kind, &active, samples);
        let scene = export::build_scene(
            kind,
            &built,
            figure::AxisScale::Linear,
            kind.default_y_scale(),
            &active,
        );
        let written = export::write_files(
            out_dir,
            kind,
            &built,
            &scene,
            &export::ExportFormat::ALL,
            PageSize::DEFAULT,
            DEFAULT_PIXELS_PER_POINT,
            figure::FigurePalette::LIGHT_PUBLICATION,
        )?;
        for path in &written {
            println!("  {}", path.display());
        }
        total += written.len();
    }
    Ok(total)
}

/// End-to-end gate: the headless export really writes usable files.
///
/// # Methodology
///
/// Runs [`export_all`] into a temporary directory at a low sample count and
/// asserts, for each of the four diagrams: the PNG decodes and has the expected
/// pixel dimensions; the PDF begins `%PDF-` and ends `%%EOF`; the SVG is
/// well-formed enough to open and close its root element; and both CSVs have a
/// header plus at least one data row. This is the check that the whole pipeline
/// — live IF97 evaluation, layer assembly, scene layout, and all four
/// serialisers — actually works, rather than each stage working alone.
///
/// # Result (measured 2026-08-20)
///
/// Passes: 20 files written (4 diagrams x 5 files), all four PNGs decode at
/// 1440 x 1080 px, all four PDFs are structurally valid, and every CSV carries
/// data rows.
#[cfg(all(test, not(target_os = "android")))]
#[test]
fn headless_export_writes_valid_files_for_all_four_diagrams() {
    use diagram::DiagramKind;

    let dir = std::env::temp_dir().join(format!(
        "tampines_steam_tables_gui_export_gate_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    let count = export_all(&dir, 60).expect("headless export succeeds");
    assert_eq!(count, 20, "expected five files for each of four diagrams");

    for kind in DiagramKind::ALL {
        let stem = kind.file_stem();

        let png = std::fs::read(dir.join(format!("{stem}.png"))).expect("PNG written");
        let decoded = image::load_from_memory(&png).expect("PNG decodes");
        assert_eq!((decoded.width(), decoded.height()), (1440, 1080));

        let pdf = std::fs::read(dir.join(format!("{stem}.pdf"))).expect("PDF written");
        assert!(pdf.starts_with(b"%PDF-"), "{stem}.pdf lacks a PDF header");
        assert!(pdf.ends_with(b"%%EOF\n"), "{stem}.pdf lacks an EOF marker");

        let svg = std::fs::read_to_string(dir.join(format!("{stem}.svg"))).expect("SVG written");
        assert!(svg.starts_with("<svg"));
        assert!(svg.trim_end().ends_with("</svg>"));

        for suffix in ["curves", "points"] {
            let csv =
                std::fs::read_to_string(dir.join("data").join(format!("{stem}_{suffix}.csv")))
                    .expect("CSV written");
            assert!(csv.starts_with(figure::csv_export::HEADER));
            assert!(
                csv.lines().count() > 1,
                "{stem}_{suffix}.csv has no data rows"
            );
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
}
