// SPDX-License-Identifier: AGPL-3.0-only
//! **Pictures of the MSRE N-1 model's new parameters** (gh:#1658, #1659),
//! drawn from the assembled `vv::msre_n1::Study` geometry, as the
//! outram-mc-libs drawing rule asks before any result on them:
//!
//! - `rods_xz_y_plus.png`: rods 2 and 1 (`OPENINGS[0]`, `[1]`) at insertion
//!   1 (driven lower limit, position 0) and 0 (upper limit, position 51 in);
//! - `rods_xz_y_minus.png`: rod 3 (`OPENINGS[2]`) at insertion 0.5;
//! - `rods_top_zoom.png`: the top of the graphite around the withdrawn rod,
//!   its 2.75 in of poison below the graphite's top (ORNL-4233 Fig. 2);
//! - `salt_level_100cm.png`, `salt_level_40cm.png`: the whole vessel with
//!   the salt surface at 100 and 40 cm above the stringer bottom.
//!
//! ```text
//! cargo run --release -p outram-mc-libs --example msre_n1_parameter_images -- <out dir>
//! ```
//!
//! CLI glue (writes PNGs); what it draws is the library's, reached by
//! `vv::msre_n1`'s tests (`partly_inserted_rods_sit_where_fig_2_puts_them`,
//! `the_salt_level_empties_everything_above_it`).

use outram_mc_libs::geometry::plot::{render_material_slice, PlotBasis, Rgb, SlicePlot};
use outram_mc_libs::geometry::position::Position;
use outram_mc_libs::vv::msre_n1::{self as m, Study};
use std::path::PathBuf;

const COLOURS: [Rgb; 15] = [
    Rgb::new(220, 60, 40),
    Rgb::new(70, 70, 70),
    Rgb::new(160, 160, 200),
    Rgb::new(240, 170, 40),
    Rgb::new(150, 90, 60),
    Rgb::new(240, 120, 90),
    Rgb::new(200, 100, 160),
    Rgb::new(90, 140, 90),
    Rgb::new(120, 60, 140),
    Rgb::new(110, 110, 120),
    Rgb::new(60, 130, 200),
    Rgb::new(30, 30, 30),
    Rgb::new(255, 255, 0),
    Rgb::new(0, 150, 150),
    Rgb::new(200, 200, 120),
];

fn draw(dir: &PathBuf, study: &Study, stem: &str, origin: Position, width: [f64; 2], px: usize, title: &str) {
    let plan = study.plan();
    let g = study.geometry();
    let palette: Vec<(Rgb, &str)> = plan.materials.iter().enumerate().map(|(i, (n, _))| (COLOURS[i % COLOURS.len()], *n)).collect();
    let plot = SlicePlot {
        origin,
        basis: PlotBasis::Xz,
        width,
        pixels: [px, (px as f64 * width[1] / width[0]).round() as usize],
        level: None,
        show_overlaps: true,
        meshlines: None,
    };
    let (_raw, img) = render_material_slice(&g, &plot, &palette, title);
    let path = dir.join(format!("{stem}.png"));
    img.write_png(&path).expect("write png");
    eprintln!("drew {}", path.display());
}

fn main() {
    let dir = PathBuf::from(std::env::args().nth(1).expect("output directory"));
    std::fs::create_dir_all(&dir).expect("output directory");
    let d = m::Dims::at(m::REFERENCE_TEMPERATURE_F, m::REFERENCE_TEMPERATURE_F);
    let h = m::H_CORE * d.graphite;
    let rods = Study { rods: Some([1.0, 0.0, 0.5]), ..Study::default() };
    let y_plus = m::OPENINGS[0].1 * d.graphite;
    draw(&dir, &rods, "rods_xz_y_plus", Position::new(0.0, y_plus, h / 2.0), [24.0, h + 60.0], 480, "MSRE N-1 X-Z AT Y = +2 IN: ROD 2 DRIVEN IN (POSITION 0, LEFT), ROD 1 AT ITS UPPER LIMIT (51 IN, RIGHT)");
    draw(&dir, &rods, "rods_xz_y_minus", Position::new(0.0, -y_plus, h / 2.0), [24.0, h + 60.0], 480, "MSRE N-1 X-Z AT Y = -2 IN: ROD 3 HALF WAY (POSITION 25.5 IN, LEFT)");
    let x1 = m::OPENINGS[1].0 * d.graphite;
    draw(&dir, &rods, "rods_top_zoom", Position::new(x1, y_plus, h - 2.0), [8.0, 16.0], 480, "MSRE N-1 TOP OF THE GRAPHITE AT ROD 1, WITHDRAWN: 2.75 IN OF POISON BELOW THE TOP (ORNL-4233 FIG. 2)");
    let w = 2.0 * m::R_VESSEL_OUT + 10.0;
    for level in [100.0, 40.0] {
        let s = Study { salt_level_cm: Some(level), ..Study::default() };
        draw(&dir, &s, &format!("salt_level_{level:.0}cm"), Position::new(0.0, 0.0, h / 2.0), [w, h + 140.0], 600, &format!("MSRE N-1 X-Z AT Y = 0: SALT SURFACE AT Z = {level:.0} CM (STRINGER BOTTOM AT 0)"));
    }
}
