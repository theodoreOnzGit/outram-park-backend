// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// This file is part of OUTRAM PARK.

//! **A fuel pebble with the record's TRISO lattice and with random packing**
//! (gh:#1243, the drawing rule): one fuel ball of an explicit (DEM) bed,
//! built through `assemble_explicit_triso_from_centres_with` with
//! `TrisoPacking::Lattice` and with `TrisoPacking::DEM_DEFAULT`, sliced
//! through its centre from the assembled geometry.
//!
//! ```text
//! cargo run --release -p nee_soon --example htr10_random_triso_images
//! ```
//!
//! Writes PNGs to `verification_and_validation/htr10_random_triso/`. Image
//! glue (exempt from the reaching-test rule: the packing it draws is tested
//! by `explicit_bed::tests::random_triso_packing_holds_the_lattices_particles`).

use std::path::PathBuf;

use nee_soon::htr10_rmc::core_design::{Htr10CoreDesign, TrisoPacking};
use nee_soon::htr10_rmc::explicit_bed::assemble_explicit_triso_from_centres_with;
use nee_soon::htr10_rmc::plots::palette;
use outram_mc_libs::geometry::plot::{render_material_slice, PlotBasis, SlicePlot};
use outram_mc_libs::geometry::position::Position;
use uom::si::f64::Length;
use uom::si::length::centimeter;

fn main() -> Result<(), String> {
    let out = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("verification_and_validation/htr10_random_triso");
    std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
    let c = |v: f64| Length::new::<centimeter>(v);
    // Two separate fuel balls resting on the floor, 12 cm apart.
    let centres = vec![[c(0.0), c(0.0), c(3.0)], [c(12.0), c(0.0), c(3.0)]];
    let fuel = [true, true];
    let pal = palette();
    for (name, packing) in [
        ("lattice", TrisoPacking::Lattice),
        ("random", TrisoPacking::DEM_DEFAULT),
    ] {
        let design = Htr10CoreDesign {
            triso_packing: packing,
            ..Htr10CoreDesign::default()
        };
        let core = assemble_explicit_triso_from_centres_with(&centres, &fuel, 1, 0, &design);
        let Some(nee_soon::htr10_rmc::bed::PebbleBed::Explicit(eb)) = core.bed.as_ref() else {
            return Err("not an explicit bed".into());
        };
        for (k, b) in eb.centres.iter().enumerate().take(2) {
            for (basis, tag) in [(PlotBasis::Xy, "xy"), (PlotBasis::Xz, "xz")] {
                let plot = SlicePlot::new(basis, Position::new(b[0], b[1], b[2]), [6.4, 6.4], [1600, 1600]);
                let (_raw, img) = render_material_slice(
                    &core.geometry,
                    &plot,
                    &pal,
                    &format!("FUEL BALL {k}, {} TRISO, {} THROUGH ITS CENTRE", name.to_uppercase(), tag.to_uppercase()),
                );
                let path = out.join(format!("pebble{k}_{name}_{tag}.png"));
                img.write_png(&path).map_err(|e| e.to_string())?;
                println!("wrote {} ({} particles per fuel zone)", path.display(), core.triso_particles.unwrap_or(0));
            }
        }
    }
    Ok(())
}
