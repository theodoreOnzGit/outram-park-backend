// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// This file is part of OUTRAM PARK. See `src/htr10_rmc/mod.rs` for licence terms.

//! Prints the HTR-10 code-to-code verification report against the RMC paper.
//!
//! ```bash
//! cargo run --release -p nee_soon --example htr10_rmc_verification
//! ```
//!
//! Reference: Li Wanlin, Yu Ganglin & Wei Chunlin, *"Research on Benchmark
//! Calculation and Analysis of HTR-10 with RMC Code"*, HTR 2014, Weihai.

use nee_soon::htr10_rmc::{bed, geometry_closures, table1, RMC_KEFF_VS_HEIGHT};

fn main() {
    println!("HTR-10 code-to-code verification against Li, Yu & Wei (2014), HTR 2014 Weihai");
    println!("{}", "=".repeat(78));

    let cell = bed::HexBedCell::from_paper();
    println!("\nRECONSTRUCTED HEX BED CELL (the paper never states a pitch)");
    println!("  pitch                 {:.4} cm", cell.pitch);
    println!(
        "  height                {:.4} cm   (= 2 x {:.4} close-packed layer spacing)",
        cell.height,
        bed::close_packed_layer_spacing(cell.ball_diameter)
    );
    println!("  balls per cell        {}", cell.balls);
    println!("  packing fraction      {:.4}", cell.packing_fraction());
    println!(
        "  in-plane spacing      {:.4} cm  (balls are {:.1} cm -> {})",
        cell.in_plane_spacing(),
        cell.ball_diameter,
        if cell.in_plane_spacing() > cell.ball_diameter {
            "no overlap"
        } else {
            "OVERLAP"
        }
    );
    println!(
        "  interlayer spacing    {:.4} cm  ({})",
        cell.interlayer_spacing(),
        if cell.interlayer_spacing() > cell.ball_diameter {
            "no overlap"
        } else {
            "OVERLAP"
        }
    );
    println!(
        "  ordered close packing would be {:.4} -> the lattice is DILUTED",
        bed::close_packed_fraction()
    );

    println!("\nCLOSURES — quantities the paper states AND we can derive independently");
    println!(
        "{:<30} {:>12} {:>14} {:>11}   {}",
        "quantity", "paper", "reconstruction", "relative", "derived from"
    );
    println!("{}", "-".repeat(110));
    for c in geometry_closures() {
        println!(
            "{:<30} {:>12.6} {:>14.6} {:>10.4} %   {}",
            c.quantity,
            c.stated,
            c.derived,
            c.relative() * 100.0,
            c.derived_from
        );
    }

    let (fuel, moderator) =
        cell.fuel_and_moderator_balls(table1::CORE_DIAMETER_CM, table1::CORE_HEIGHT_CM);
    println!(
        "\n  core inventory at the 0.57/0.43 split: {fuel:.0} fuel + {moderator:.0} moderator balls"
    );

    println!("\nREFERENCE CURVE — the paper's RMC k_eff vs fuel-loading height");
    let c = RMC_KEFF_VS_HEIGHT;
    let i = c
        .windows(2)
        .position(|w| w[0].1 < 1.0 && w[1].1 >= 1.0)
        .unwrap();
    let (h0, k0) = c[i];
    let (h1, k1) = c[i + 1];
    let hc = h0 + (1.0 - k0) * (h1 - h0) / (k1 - k0);
    println!(
        "  {} points, {:.3} to {:.3} cm in {} cm steps",
        c.len(),
        c[0].0,
        c[c.len() - 1].0,
        table1::LAYER_HEIGHT_CM
    );
    println!("  k_eff {:.6} .. {:.6}", c[0].1, c[c.len() - 1].1);
    println!(
        "  crosses k = 1 between {h0} and {h1} cm -> critical height {hc:.2} cm (interpolated)"
    );

    println!("\nNOT VERIFIED, AND WHY");
    println!("  The k_eff curve above is NOT reproduced here. The paper defers the reflector:");
    println!("    \"Modeling details of reflector and structural material are referred to");
    println!("     paper released by IAEA which is listed in reference.\"  [IAEA-TECDOC-1382]");
    println!("  A 180 cm core inside ~1 m of graphite reflector cannot be modelled without it,");
    println!("  and the reflector houses the control rods and absorber-ball channels too.");
    println!("\n  Reference quality: the paper's own RMC-vs-MCNP relative differences reach");
    println!("  ~0.9 %, and it states the model was \"constructed relatively independently\".");
    println!("  Treat ~500 pcm as success here; 50 pcm would be suspicious.");
    println!("\n  Also: its Tables 3 and 4 are both captioned \"(vacuum)\" but share a");
    println!("  byte-identical RMC column (11/11 rows) with differing MCNP columns (0/11).");
    println!("  There is ONE RMC curve, not a vacuum/helium pair.");
}
