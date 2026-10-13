// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors

//! # Draw the gh:#849 HTR-10 cells (crate HARD RULE: geometry is drawn)
//!
//! Renders material slices of the ASSEMBLED geometry of
//! `nee_soon::htr10_mgxs::cells::Htr10Cell` with
//! `outram_mc_libs::geometry::plot::render_material_slice`, coloured by the
//! HTR-10 palette, into
//! `crates/nee_soon/verification_and_validation/htr10_mgxs_cells/`:
//!
//! | cell | images |
//! |---|---|
//! | single pebble | x-y and x-z through a TRISO layer near the pebble's centre, one TRISO particle |
//! | Şeker cluster, rings 1, 2, 3 | x-y at the basal plane (just inside the bottom face) and the central plane, x-z through the centre, one fuel pebble (through a TRISO layer), one TRISO particle |
//!
//! ```text
//! cargo run --release -p nee_soon --example htr10_mgxs_cell_images
//! ```
//!
//! **Research, education and V&V only** (`RESPONSIBLE_USE.md`).

use std::path::PathBuf;

use nee_soon::htr10_mgxs::cells::Htr10Cell;
use nee_soon::htr10_rmc::core_model::mat;
use outram_mc_libs::geometry::cell::SurfaceToken;
use outram_mc_libs::geometry::plot::{render_material_slice, PlotBasis, SlicePlot};
use outram_mc_libs::geometry::position::{Direction, Position};

/// The centre of one TRISO particle near `near`, read off the located path:
/// the leaf level is the particle universe, entered through the TRISO
/// lattice, so its offset is the particle centre.
fn find_triso(c: &Htr10Cell, near: [f64; 3]) -> Option<Position> {
    let g = &c.geometry;
    let u = Direction::new(0.0, 0.0, 1.0);
    for kz in 0..20 {
        for j in 0..60 {
            for i in 0..60 {
                let p = Position::new(
                    near[0] + 0.01 * i as f64,
                    near[1] + 0.01 * j as f64,
                    near[2] + 0.01 * kz as f64,
                );
                if let Some(path) = g.locate(p, u, SurfaceToken::NONE) {
                    if path.material == Some(mat::KERNEL) {
                        return path.levels.last().map(|l| l.offset);
                    }
                }
            }
        }
    }
    None
}

fn main() {
    let out = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("verification_and_validation/htr10_mgxs_cells");
    std::fs::create_dir_all(&out).expect("create output directory");
    let pal = nee_soon::htr10_rmc::plots::palette();

    let mut cells = vec![("single_pebble".to_string(), Htr10Cell::single_pebble())];
    for rings in 1..=3 {
        cells.push((format!("seker_cluster_r{rings}"), Htr10Cell::seker_cluster(rings)));
    }
    for (tag, c) in &cells {
        let g = &c.geometry;
        println!(
            "{tag}: {} | volume {:.3} cm^3, {} fuel + {} dummy balls, {} TRISO per pebble, \
             {} cells, {} universes",
            c.kind.describe(),
            c.volume_cm3,
            c.fuel_balls,
            c.dummy_balls,
            c.triso_particles,
            g.cells.len(),
            g.universes.len()
        );
        let render = |name: &str, plot: SlicePlot, what: &str| {
            let (_raw, img) = render_material_slice(g, &plot, &pal, &format!("{tag}: {what}"));
            let path = out.join(format!("{tag}_{name}"));
            img.write_png(&path).expect("write png");
            println!("  wrote {}", path.display());
        };
        let [hx, _, hz] = c.half_extent;
        // One fuel pebble (the one nearest the centre), and one TRISO
        // particle in it. The particle fixes the pebble-level slice planes:
        // the TRISO lattice has an even count per axis, so a plane through
        // the pebble's centre runs BETWEEN particle layers and shows plain
        // matrix (the same graphite colour as the shell).
        let Some(&(peb, _)) = c.balls.iter().filter(|(_, fuel)| *fuel).min_by(|a, b| {
            let d = |p: [f64; 3]| p[0] * p[0] + p[1] * p[1] + p[2] * p[2];
            d(a.0).total_cmp(&d(b.0))
        }) else {
            println!("  no fuel ball in {tag}");
            continue;
        };
        let probe = [peb[0] - 0.3, peb[1] - 0.3, peb[2].clamp(-hz + 0.2, hz - 0.4)];
        let Some(part) = find_triso(c, probe) else {
            println!("  no kernel found near {probe:?} in {tag}");
            continue;
        };
        println!(
            "  fuel pebble at ({:.3}, {:.3}, {:.3}) cm, TRISO at ({:.4}, {:.4}, {:.4}) cm",
            peb[0], peb[1], peb[2], part.x, part.y, part.z
        );
        // A margin round the cell, so its boundary is visible against the
        // plotter's background.
        let w = 2.0 * hx + 2.0;
        let px = ((w / 0.02).round() as usize).clamp(600, 1600);
        let xy = |z: f64| SlicePlot::new(PlotBasis::Xy, Position::new(0.0, 0.0, z), [w, w], [px, px]);
        let hgt = 2.0 * hz + 2.0;
        let xz = |y: f64| {
            SlicePlot::new(
                PlotBasis::Xz,
                Position::new(0.0, y, 0.0),
                [w, hgt],
                [px, ((hgt / w) * px as f64).round() as usize],
            )
        };
        if matches!(c.kind, nee_soon::htr10_mgxs::cells::CellKind::SinglePebble) {
            render("xy_layer.png", xy(part.z), "X-Y THROUGH A TRISO LAYER");
            render("xz_layer.png", xz(part.y), "X-Z THROUGH A TRISO LAYER");
        } else {
            render("xy_basal.png", xy(-hz + 0.05), "X-Y, BASAL PLANE (BOTTOM FACE + 0.05 CM)");
            render("xy_central.png", xy(0.0), "X-Y, CENTRAL PLANE");
            render("xz.png", xz(0.0), "X-Z THROUGH THE CENTRE");
            render(
                "pebble_xy.png",
                SlicePlot::new(PlotBasis::Xy, Position::new(peb[0], peb[1], part.z), [7.0, 7.0], [1000, 1000]),
                "X-Y, 7 CM ROUND ONE FUEL PEBBLE, THROUGH A TRISO LAYER",
            );
        }
        render(
            "triso_xy.png",
            SlicePlot::new(PlotBasis::Xy, part, [0.8, 0.8], [800, 800]),
            "X-Y, 0.8 CM ROUND ONE TRISO PARTICLE",
        );
    }
}
