// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK.

//! **Draw the TRISO particle the random-walk solver sees** (crate `CLAUDE.md`,
//! "Reactor geometry is DRAWN for a human to check"; lesson rung 1, gh:#531).
//!
//! Builds two assembled [`TrisoCell`]s, the CRP-6 cell
//! ([`TrisoCell::new_crp6_geometry`]) and an HTR-10 cell from the published
//! radii (IAEA-TECDOC-1382 part 2 Table 4-17, as carried by
//! `boon_lay::fuel_failure::htr10` and `tampines::pebble_bed::triso`), and for
//! each one:
//!
//! 1. **Draws a z = 0 slice** by asking the assembled cell which region every
//!    pixel centre lies in ([`TrisoCell::get_triso_region`]), never from the
//!    named constants. Rows are run-length encoded into an SVG with a legend
//!    and a 100 µm scale bar, written beside the lesson.
//! 2. **Recovers every interface radius from the geometry** by bisection on
//!    the region lookup along a fixed oblique direction, and compares it with
//!    the radius the cell reports (`get_*_radius`). A mismatch above 1 nm
//!    fails the run.
//!
//! No physics runs here; it is a geometry check. Pure `std`, so it builds on
//! every target the library does.
//!
//! ```bash
//! cargo run --release -p boon-lay --example triso_cell_slice
//! ```
//!
//! # Results (2026-10-04, `develop` atop `5e802df3a4`)
//!
//! Recorded in the lesson page `docs/tutorial/src/triso.md` ("The check"),
//! with the printed table; images in `docs/tutorial/src/img/`.

use std::fmt::Write as _;
use std::path::PathBuf;

use boon_lay::lagrangian_decay_simulator::lagrangian_diffusion::single_particle_simulator::constructive_solid_geometry::{
    TrisoCell, TrisoRegion,
};
use uom::si::f64::Length;
use uom::si::length::{meter, micrometer};

/// Pixels per side of the slice.
const PIXELS: usize = 400;

fn um(x: f64) -> Length {
    Length::new::<micrometer>(x)
}

/// Colour and label for each region (also the legend order).
fn style(region: TrisoRegion) -> (&'static str, &'static str) {
    match region {
        TrisoRegion::Fuel => ("#c0392b", "kernel (UO2)"),
        TrisoRegion::Buffer => ("#d9c38c", "buffer (porous C)"),
        TrisoRegion::IPyC => ("#5d6d7e", "IPyC"),
        TrisoRegion::SiC => ("#2e86c1", "SiC"),
        TrisoRegion::OPyC => ("#2c3e50", "OPyC"),
        TrisoRegion::Outside => ("#ffffff", "outside (matrix)"),
    }
}

/// The interfaces, innermost first: (name, radius the cell reports).
fn reported_radii(cell: &TrisoCell) -> [(&'static str, Length); 5] {
    [
        ("kernel/buffer", cell.get_fuel_radius()),
        ("buffer/IPyC", cell.get_buffer_radius()),
        ("IPyC/SiC", cell.get_ipyc_radius()),
        ("SiC/OPyC", cell.get_sic_radius()),
        ("OPyC/outside", cell.get_opyc_radius()),
    ]
}

/// Region index along the ladder, so "inside" means a smaller index.
fn rank(region: TrisoRegion) -> usize {
    match region {
        TrisoRegion::Fuel => 0,
        TrisoRegion::Buffer => 1,
        TrisoRegion::IPyC => 2,
        TrisoRegion::SiC => 3,
        TrisoRegion::OPyC => 4,
        TrisoRegion::Outside => 5,
    }
}

/// Bisect, along the unit direction `dir`, for the radius where the region
/// lookup changes from rank `k` to rank `k + 1`. Uses only the lookup.
fn recover_interface(cell: &TrisoCell, dir: [f64; 3], k: usize) -> Length {
    let point = |r: f64| [Length::new::<meter>(r * dir[0]), Length::new::<meter>(r * dir[1]), Length::new::<meter>(r * dir[2])];
    let (mut lo, mut hi) = (0.0_f64, 1.0e-3_f64); // 0 to 1 mm, beyond any particle
    for _ in 0..200 {
        let mid = 0.5 * (lo + hi);
        if rank(cell.get_triso_region(point(mid))) <= k {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    Length::new::<meter>(0.5 * (lo + hi))
}

/// Draw the z = 0 slice as an SVG, from per-pixel region lookups.
fn slice_svg(cell: &TrisoCell, title: &str) -> String {
    let r_out = cell.get_opyc_radius().get::<micrometer>();
    let half = 1.15 * r_out; // half-width of the view, µm
    let px = 2.0 * half / PIXELS as f64; // µm per pixel
    let legend_w = 190.0;
    let mut svg = String::new();
    let (w, h) = (PIXELS as f64 + legend_w, PIXELS as f64 + 40.0);
    writeln!(svg, r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {w} {h}" width="{w}" height="{h}" font-family="sans-serif" font-size="12">"##).unwrap();
    writeln!(svg, r##"<rect width="{w}" height="{h}" fill="#ffffff"/>"##).unwrap();
    writeln!(svg, r##"<text x="6" y="16" font-size="13" font-weight="bold">{title}</text>"##).unwrap();
    writeln!(svg, r##"<g transform="translate(0,24)" shape-rendering="crispEdges">"##).unwrap();
    for j in 0..PIXELS {
        let y = half - (j as f64 + 0.5) * px;
        let mut run_start = 0usize;
        let mut run_region: Option<TrisoRegion> = None;
        for i in 0..=PIXELS {
            let region = if i < PIXELS {
                let x = -half + (i as f64 + 0.5) * px;
                Some(cell.get_triso_region([um(x), um(y), um(0.0)]))
            } else {
                None
            };
            if region != run_region {
                if let Some(r) = run_region {
                    if r != TrisoRegion::Outside {
                        let (fill, _) = style(r);
                        writeln!(svg, r##"<rect x="{run_start}" y="{j}" width="{}" height="1" fill="{fill}"/>"##, i - run_start).unwrap();
                    }
                }
                run_start = i;
                run_region = region;
            }
        }
    }
    // 100 µm scale bar, bottom left.
    let bar = 100.0 / px;
    let yb = PIXELS as f64 - 14.0;
    writeln!(svg, r##"<line x1="12" y1="{yb}" x2="{}" y2="{yb}" stroke="#000" stroke-width="3"/>"##, 12.0 + bar).unwrap();
    writeln!(svg, r##"<text x="12" y="{}">100 µm</text>"##, yb - 6.0).unwrap();
    writeln!(svg, "</g>").unwrap();
    // Legend with the radii the geometry reports.
    let lx = PIXELS as f64 + 10.0;
    let radii = reported_radii(cell);
    for (n, region) in [TrisoRegion::Fuel, TrisoRegion::Buffer, TrisoRegion::IPyC, TrisoRegion::SiC, TrisoRegion::OPyC]
        .into_iter()
        .enumerate()
    {
        let (fill, label) = style(region);
        let y = 44.0 + 40.0 * n as f64;
        writeln!(svg, r##"<rect x="{lx}" y="{y}" width="16" height="16" fill="{fill}" stroke="#000"/>"##).unwrap();
        writeln!(svg, r##"<text x="{}" y="{}">{label}</text>"##, lx + 22.0, y + 12.0).unwrap();
        writeln!(svg, r##"<text x="{}" y="{}" fill="#444">r ≤ {:.1} µm</text>"##, lx + 22.0, y + 27.0, radii[n].1.get::<micrometer>()).unwrap();
    }
    writeln!(svg, r##"<text x="{lx}" y="{}" fill="#444">slice z = 0, drawn from</text>"##, h - 40.0).unwrap();
    writeln!(svg, r##"<text x="{lx}" y="{}" fill="#444">TrisoCell::get_triso_region</text>"##, h - 24.0).unwrap();
    writeln!(svg, "</svg>").unwrap();
    svg
}

fn main() {
    let crp6 = TrisoCell::new_crp6_geometry();
    // HTR-10, IAEA-TECDOC-1382 pt 2 Table 4-17 (the same five radii as
    // tampines::pebble_bed::triso::TrisoParticle::htr10; the kernel, buffer and
    // SiC radii are also boon_lay::fuel_failure::htr10's constants).
    let htr10 = TrisoCell::new(um(250.0), um(340.0), um(380.0), um(415.0), um(455.0));

    let out_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("docs/tutorial/src/img");
    std::fs::create_dir_all(&out_dir).expect("create docs/tutorial/src/img");

    // An oblique, irrational direction, so the bisection does not ride an axis.
    let d = [1.0_f64, 2.0_f64.sqrt(), 3.0_f64.sqrt()];
    let n = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
    let dir = [d[0] / n, d[1] / n, d[2] / n];

    let mut worst_nm = 0.0_f64;
    for (name, cell, file) in [
        ("CRP-6 (new_crp6_geometry)", &crp6, "triso_crp6_slice.svg"),
        ("HTR-10 (TECDOC-1382 Table 4-17)", &htr10, "triso_htr10_slice.svg"),
    ] {
        println!("{name}");
        println!("  {:<14} {:>14} {:>16} {:>10}", "interface", "reported (µm)", "recovered (µm)", "diff (nm)");
        for (k, (iface, reported)) in reported_radii(cell).into_iter().enumerate() {
            let recovered = recover_interface(cell, dir, k);
            let diff_nm = (recovered - reported).get::<micrometer>().abs() * 1e3;
            worst_nm = worst_nm.max(diff_nm);
            println!(
                "  {:<14} {:>14.4} {:>16.4} {:>10.2e}",
                iface,
                reported.get::<micrometer>(),
                recovered.get::<micrometer>(),
                diff_nm
            );
        }
        let path = out_dir.join(file);
        std::fs::write(&path, slice_svg(cell, name)).expect("write slice SVG");
        println!("  slice written to {}", path.display());
    }
    println!("worst interface mismatch: {worst_nm:.2e} nm (pass if < 1 nm)");
    assert!(worst_nm < 1.0, "assembled geometry disagrees with its reported radii");
}
