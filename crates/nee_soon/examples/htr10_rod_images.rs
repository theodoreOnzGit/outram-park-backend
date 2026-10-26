// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// This file is part of OUTRAM PARK.

//! **HTR-10 control-rod review images** (gh:#1025; the crates' drawing rule).
//!
//! Draws what the solver sees, from the ASSEMBLED geometry
//! (`assemble_explicit_triso_with`, one `Geometry::locate` per pixel through
//! `outram_mc_libs::geometry::plot::render_material_slice`), for the rod
//! configurations the gh:#1025 runs use, at 14 x 12:
//!
//! - all ten withdrawn, half in and fully in (the bank);
//! - rod 3 ("S3", the channel at 81°) alone at the experiment's start
//!   (`z_T` 171.2 cm), at B42's 282.618 cm and fully in (394.2 cm).
//!
//! For each: a y-z slice through rod 3's axis (`x` = rod 3's; rod 3 at 81°
//! is nearer the y axis, so this vertical plane through its axis also cuts
//! the bed), the whole model height, 0.4 cm per pixel; and an x-y slice at
//! bed mid-height.
//!
//! ```text
//! OUTRAM_ROD_IMG_OUT=/some/dir cargo run --release -p nee_soon --example htr10_rod_images
//! ```
//!
//! Default output: `crates/dhoby-ghaut/verification_and_validation/htr10_rod_curve/geometry/`.
//!
//! `OUTRAM_ROD_IMG_STEEL_PIECES=1` (gh:#1027) draws instead the bank fully
//! in for each `RodSteelPieces` variant: the same y-z slice, and a 30 cm
//! wide strip centred on rod 3's axis at 0.1 cm per pixel, so the 2.3 cm top
//! end and the 3.6 cm joints are 23 and 36 pixels. Default output
//! `crates/nee_soon/verification_and_validation/htr10_rod_end_pieces/geometry/`.
//! CLI glue (exempt from the reaching-test rule): the builders it calls are
//! tested in `core_design` (`the_bank_is_ten_equal_rods`,
//! `inserted_rods_are_in_the_assembled_geometry`).

use std::path::PathBuf;

use nee_soon::htr10_rmc::control_rod::RodSteelPieces;
use nee_soon::htr10_rmc::core_design::Htr10CoreDesign;
use nee_soon::htr10_rmc::core_model::{assemble_explicit_triso_with, HTR10_REFLECTOR_OUTER_CM};
use nee_soon::htr10_rmc::reflector_geometry::{control_rod_azimuths_deg, ROD_CHANNEL_CENTRE_RADIUS_CM};
use outram_mc_libs::geometry::plot::{render_material_slice, PlotBasis, SlicePlot};
use outram_mc_libs::geometry::position::Position;
use petir::mathf::RealMath;

fn main() {
    if std::env::var("OUTRAM_ROD_IMG_STEEL_PIECES").is_ok() {
        steel_pieces();
        return;
    }
    let out = std::env::var("OUTRAM_ROD_IMG_OUT").map_or_else(
        |_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../dhoby-ghaut/verification_and_validation/htr10_rod_curve/geometry"),
        PathBuf::from,
    );
    std::fs::create_dir_all(&out).expect("create the output directory");
    let s3 = 2usize;
    let (_, cs) = control_rod_azimuths_deg()[s3].to_radians().r_sin_cos();
    let x3 = ROD_CHANNEL_CENTRE_RADIUS_CM * cs;
    let ins = Htr10CoreDesign::rod_insertion_for_lower_end_zt_cm;
    let d = Htr10CoreDesign::default();
    let cases: [(&str, &str, Htr10CoreDesign); 6] = [
        ("bank_out", "ALL TEN WITHDRAWN (Z_T 119.2)", d),
        ("bank_half", "ALL TEN HALF IN (Z_T 256.7)", d.with_bank(0.5)),
        ("bank_in", "ALL TEN FULLY IN (Z_T 394.2)", d.with_bank(1.0)),
        ("s3_171", "ROD 3 S3 ALONE AT Z_T 171.2", d.with_rod(s3, ins(171.2))),
        ("s3_282", "ROD 3 S3 ALONE AT Z_T 282.618", d.with_rod(s3, ins(282.618))),
        ("s3_in", "ROD 3 S3 ALONE FULLY IN (Z_T 394.2)", d.with_rod(s3, 1.0)),
    ];
    let pal = nee_soon::htr10_rmc::plots::palette();
    for (name, what, design) in cases {
        let core = assemble_explicit_triso_with(14, 12, 0, &design);
        println!(
            "{name}: {} cells, {} universes, rod lower ends z_T {:?}",
            core.cells, core.universes, core.rod_lower_ends_zt_cm
        );
        let half_w = HTR10_REFLECTOR_OUTER_CM + 5.0;
        let (z_lo, z_hi) = (core.refl_bottom - 5.0, core.refl_top + 5.0);
        let w = 2.0 * half_w;
        let h = z_hi - z_lo;
        let px = |l: f64| (l / 0.4).round() as usize;
        let yz = SlicePlot::new(PlotBasis::Yz, Position::new(x3, 0.0, 0.5 * (z_lo + z_hi)), [w, h], [px(w), px(h)]);
        let (_, img) = render_material_slice(&core.geometry, &yz, &pal, &format!("HTR-10 14X12 {what}: Y-Z AT X = {x3:.2} (ROD 3 AXIS)"));
        img.write_png(out.join(format!("rods_{name}_yz_through_rod3.png"))).expect("write png");
        let xy = SlicePlot::new(PlotBasis::Xy, Position::new(0.0, 0.0, 0.0), [w, w], [px(w), px(w)]);
        let (_, img) = render_material_slice(&core.geometry, &xy, &pal, &format!("HTR-10 14X12 {what}: X-Y AT BED MID-HEIGHT"));
        img.write_png(out.join(format!("rods_{name}_xy_bed_mid.png"))).expect("write png");
    }
    println!("wrote {}", out.display());
}

/// gh:#1027: the bank fully in, once per steel-piece variant.
fn steel_pieces() {
    let out = std::env::var("OUTRAM_ROD_IMG_OUT").map_or_else(
        |_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("verification_and_validation/htr10_rod_end_pieces/geometry"),
        PathBuf::from,
    );
    std::fs::create_dir_all(&out).expect("create the output directory");
    let (sn, cs) = control_rod_azimuths_deg()[2].to_radians().r_sin_cos();
    let (x3, y3) = (ROD_CHANNEL_CENTRE_RADIUS_CM * cs, ROD_CHANNEL_CENTRE_RADIUS_CM * sn);
    let pal = nee_soon::htr10_rmc::plots::palette();
    for (name, v) in [
        ("published", RodSteelPieces::AsPublished),
        ("top_end_b4c", RodSteelPieces::TopEndB4c),
        ("all_b4c", RodSteelPieces::AllB4c),
    ] {
        let design = Htr10CoreDesign { rod_steel_pieces: v, ..Htr10CoreDesign::default() }.with_bank(1.0);
        let core = assemble_explicit_triso_with(14, 12, 0, &design);
        println!("{name}: {} cells, rod lower ends z_T {:?}", core.cells, core.rod_lower_ends_zt_cm);
        let half_w = HTR10_REFLECTOR_OUTER_CM + 5.0;
        let (z_lo, z_hi) = (core.refl_bottom - 5.0, core.refl_top + 5.0);
        let (w, h) = (2.0 * half_w, z_hi - z_lo);
        let px = |l: f64, d: f64| (l / d).round() as usize;
        let zc = 0.5 * (z_lo + z_hi);
        let yz = SlicePlot::new(PlotBasis::Yz, Position::new(x3, 0.0, zc), [w, h], [px(w, 0.4), px(h, 0.4)]);
        let (_, img) = render_material_slice(&core.geometry, &yz, &pal, &format!("HTR-10 14X12 BANK FULLY IN, {v:?}: Y-Z AT X = {x3:.2} (ROD 3 AXIS)"));
        img.write_png(out.join(format!("{name}_bank_in_yz.png"))).expect("write png");
        let strip = SlicePlot::new(PlotBasis::Yz, Position::new(x3, y3, zc), [30.0, h], [px(30.0, 0.1), px(h, 0.1)]);
        let (_, img) = render_material_slice(&core.geometry, &strip, &pal, &format!("ROD 3, BANK FULLY IN, {v:?}"));
        img.write_png(out.join(format!("{name}_bank_in_rod3_strip.png"))).expect("write png");
    }
    println!("wrote {}", out.display());
}
