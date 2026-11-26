//! **The simplified EBR-II full core, drawn** (gh:#1762; the drawing rule):
//! material slices of the ASSEMBLED geometry (`nee_soon::ebr2_core`), so a
//! human can check the layout against ARC-228 Fig. 3.1 before any `k` on it
//! is trusted. Also the native `k` run, with `--keff`.
//!
//! ```text
//! cargo run --release -p nee_soon --example ebr2_core_images            # the pictures
//! cargo run --release -p nee_soon --example ebr2_core_images -- --keff 4000 20 80 1
//! ```
//!
//! Writes into `crates/nee_soon/verification_and_validation/ebr2_core/`:
//! - `ebr2_core_xy_midplane.png`: x-y at the core mid-plane, the whole core
//!   (drivers, rods, blanket, reflector);
//! - `ebr2_core_xy_rows1to7.png`: the same, zoomed on rows 1-7;
//! - `ebr2_core_xz.png`: x-z through the axis (the columns, the fuel
//!   height, the rods' drop and their B4C);
//! - `ebr2_core_xz_rods.png`: x-z through a high-worth control rod.
//!
//! Research, education and V&V only. A SIMPLIFIED FRESH CORE, NOT VALIDATED.
//! CLI glue: exempt from a reaching test (it writes files); what it draws is
//! `Study::geometry`, tested in the library.

use nee_soon::ebr2_core::{self as c, Study};
use outram_mc_libs::geometry::plot::{render_material_slice, PlotBasis, Rgb, SlicePlot};
use outram_mc_libs::geometry::position::Position;
use std::path::PathBuf;

const COLOURS: [Rgb; 7] = [
    Rgb::new(220, 60, 40),
    Rgb::new(240, 150, 60),
    Rgb::new(200, 100, 160),
    Rgb::new(30, 30, 30),
    Rgb::new(90, 140, 90),
    Rgb::new(160, 160, 200),
    Rgb::new(60, 130, 200),
];

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().collect();
    let out = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("verification_and_validation/ebr2_core");
    std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
    let study = Study::default();
    if let Some(i) = args.iter().position(|a| a == "--keff") {
        let n = |k: usize, d: u64| args.get(i + k).and_then(|s| s.parse::<u64>().ok()).unwrap_or(d);
        let (p, ia, a, seed) = (n(1, 4000) as usize, n(2, 20) as usize, n(3, 80) as usize, n(4, 1));
        let endf = njoy_outram_park_fork::reference_data::reference_endf_dir();
        let t0 = std::time::Instant::now();
        let r = c::keff(&endf, &study, p, ia, a, seed, |l| eprintln!("{l}"))?;
        println!(
            "EBR-II simplified fresh core, Run 138B layout, {:.2} K, control insertion {:.4}: k = {:.5} +- {:.5} ({p} x [{ia} + {a}], seed {seed}, {:.0} s)",
            study.temperature_k,
            study.control_insertion,
            r.k_mean,
            r.k_std,
            t0.elapsed().as_secs_f64()
        );
        return Ok(());
    }
    let g = study.geometry();
    let palette: Vec<(Rgb, &str)> = c::MATERIALS.iter().enumerate().map(|(i, n)| (COLOURS[i], *n)).collect();
    let r = c::R_OUTER_CM + 4.0;
    let hex = |name: &str, origin: Position, basis: PlotBasis, width: [f64; 2], title: &str| -> Result<(), String> {
        let px = 900usize;
        let plot = SlicePlot {
            origin,
            basis,
            width,
            pixels: [px, ((px as f64) * width[1] / width[0]).round() as usize],
            level: None,
            show_overlaps: true,
            meshlines: None,
        };
        let (_raw, img) = render_material_slice(&g, &plot, &palette, title);
        img.write_png(out.join(name)).map_err(|e| e.to_string())?;
        println!("wrote {name}");
        Ok(())
    };
    hex("ebr2_core_xy_midplane.png", Position::new(0.0, 0.0, 0.0), PlotBasis::Xy, [2.0 * r, 2.0 * r], "EBR-II SIMPLIFIED FRESH CORE, RUN 138B LAYOUT: X-Y AT THE CORE MID-PLANE")?;
    let z = 7.5 * c::PITCH_CM;
    hex("ebr2_core_xy_rows1to7.png", Position::new(0.0, 0.0, 0.0), PlotBasis::Xy, [2.0 * z, 2.0 * z], "ROWS 1-7 AT THE MID-PLANE: DRIVERS, HALF-WORTH, RODS, XX09")?;
    hex("ebr2_core_xz.png", Position::new(0.0, 0.0, 0.0), PlotBasis::Xz, [2.0 * r, c::COLUMN_CM + 8.0], "X-Z THROUGH THE AXIS: 139.7 CM COLUMNS, 34.29 CM FUEL")?;
    // A high-worth control rod: the first 'C' in the table.
    let &(q, rr, _) = c::RUN_138B.iter().find(|e| e.2 == b'C').ok_or("no control rod")?;
    let (x, y) = c::centre_cm(q, rr);
    hex("ebr2_core_xz_rods.png", Position::new(x, y, 0.0), PlotBasis::Xz, [8.0 * c::PITCH_CM, c::COLUMN_CM + 8.0], "X-Z THROUGH A HIGH-WORTH CONTROL ROD: FUEL DROPPED 21.06 CM, B4C ABOVE")?;
    Ok(())
}
