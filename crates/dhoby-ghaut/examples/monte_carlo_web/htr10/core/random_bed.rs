//! **Which bed the live core runs on** (gh:#786, maintainer-approved on #787,
//! 2026-10-08): Şeker & Çolak (2003)'s lattice at N layers, as every recorded
//! k, or the DEM random bed.
//!
//! The pool passes the choice around as one number, the "bed code" (the
//! `layers` field of [`super::CoreReq`] and [`super::AssemblyReport`], and
//! the slice's raster parameter): N = 10..=20 is the lattice at N layers,
//! [`DEM_BED`] (0) is the random bed.
//!
//! **The random bed** is the #216 friction study's pour at µ = 0.1, µ_r = 0
//! (`reference-data/liggghts/htr10_conus_presettled_mu10_mur00.csv`, the same
//! file the beds view draws), cut to the lattice's [`DEM_CORE_BALLS`] core
//! pebbles at N = 12 exactly as `nee_soon/examples/htr10_dem_bed_images.rs`
//! cuts it: every pebble at or below the floor kept in file order, then the
//! lowest `16 681` above it by height. Fuel identity by
//! `paper_fuel_assignment` (57:43 above the floor, conus and tube all dummy),
//! the core by `assemble_explicit_triso_from_centres(.., 14, 0)`: the bed
//! delta-tracked against the same bed majorant, everything outside it the
//! same shell as the lattice core.
//!
//! The CSV is compiled in (2.2 MB, the exact `f64`s, not the beds view's
//! quantised bake), so a worker and the slice builder have it without a
//! download.

use nee_soon::htr10_rmc::core_model::{assemble_explicit_triso, AssembledCore};
use nee_soon::htr10_rmc::explicit_bed::{assemble_explicit_triso_from_centres, paper_fuel_assignment};
use uom::si::f64::Length;
use uom::si::length::meter;

/// The bed code of the DEM random bed.
pub const DEM_BED: usize = 0;
/// Core pebbles (centred above the floor) the random bed is cut to: the
/// lattice's count at N = 12 (`run_e8_N12.log`, "16681 balls").
pub const DEM_CORE_BALLS: usize = 16_681;

/// The #216 pour (`id,x,y,z,vx,vy,vz`, metres, DEM frame).
const DEM_CSV: &str =
    include_str!("../../../../../../reference-data/liggghts/htr10_conus_presettled_mu10_mur00.csv");

/// The random bed's centres, cut as the module docs say.
pub fn dem_centres() -> Result<Vec<[Length; 3]>, String> {
    let mut below = Vec::new();
    let mut above = Vec::new();
    for (i, line) in DEM_CSV.lines().enumerate().skip(1) {
        if line.trim().is_empty() {
            continue;
        }
        let f: Vec<f64> = line
            .split(',')
            .map(|v| v.trim().parse::<f64>())
            .collect::<Result<_, _>>()
            .map_err(|e| format!("DEM bed line {}: {e}", i + 1))?;
        let c = [
            *f.get(1).ok_or("short line")?,
            *f.get(2).ok_or("short line")?,
            *f.get(3).ok_or("short line")?,
        ];
        if c[2] <= 0.0 {
            below.push(c);
        } else {
            above.push(c);
        }
    }
    above.sort_by(|p, q| p[2].total_cmp(&q[2]));
    if above.len() < DEM_CORE_BALLS {
        return Err(format!(
            "the pour holds {} core pebbles, fewer than {DEM_CORE_BALLS}",
            above.len()
        ));
    }
    above.truncate(DEM_CORE_BALLS);
    below.extend(above);
    Ok(below.iter().map(|c| c.map(Length::new::<meter>)).collect())
}

/// The core for a bed code.
pub fn build_core(code: usize) -> Result<AssembledCore, String> {
    if code == DEM_BED {
        let c = dem_centres()?;
        let fuel = paper_fuel_assignment(&c);
        Ok(assemble_explicit_triso_from_centres(
            &c,
            &fuel,
            super::RINGS,
            0,
        ))
    } else {
        Ok(assemble_explicit_triso(super::RINGS, code.clamp(10, 20), 0))
    }
}

/// What the bed code means, for the page.
pub fn label(code: usize) -> String {
    if code == DEM_BED {
        format!("DEM random bed ({DEM_CORE_BALLS} core pebbles, as the lattice at N = 12)")
    } else {
        format!("Şeker lattice, N = {code}")
    }
}

/// The geometry review images of the random-bed core (the crate's drawing
/// rule), from the ASSEMBLED geometry: the whole model side-on, the bed
/// side-on, and a plan through the bed at mid-height.
#[cfg(not(target_arch = "wasm32"))]
pub fn render_review(dir: &std::path::Path) -> Result<(), String> {
    use outram_mc_libs::geometry::plot::{render_material_slice, PlotBasis, SlicePlot};
    use outram_mc_libs::geometry::position::Position;
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let core = build_core(DEM_BED)?;
    let pal = nee_soon::htr10_rmc::plots::palette();
    let (lo, hi) = (core.conus_floor, core.bed_half_height);
    let shots = [
        (
            "htr10_4_dem_core_xz.png",
            SlicePlot::new(
                PlotBasis::Xz,
                Position::new(0.0, 0.0, -15.0),
                [400.0, 620.0],
                [400, 620],
            ),
            "HTR-10 DEM BED (16681 CORE PEBBLES): X-Z, WHOLE MODEL",
        ),
        (
            "htr10_5_dem_bed_xz.png",
            SlicePlot::new(
                PlotBasis::Xz,
                Position::new(0.0, 0.0, 0.5 * (lo + hi)),
                [190.0, hi - lo + 10.0],
                [600, 600],
            ),
            "HTR-10 DEM BED: X-Z, THE BED",
        ),
        (
            "htr10_6_dem_bed_xy.png",
            SlicePlot::new(
                PlotBasis::Xy,
                Position::new(0.0, 0.0, 0.0),
                [190.0, 190.0],
                [600, 600],
            ),
            "HTR-10 DEM BED: X-Y, THE CORE MID-PLANE",
        ),
    ];
    for (name, plot, title) in shots {
        let (_raw, img) = render_material_slice(&core.geometry, &plot, &pal, title);
        img.write_png(&dir.join(name)).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;

    /// The random bed is cut to the lattice's core count, keeps its conus and
    /// tube, builds, and puts fuel in the bed; the lattice codes build the
    /// recorded core.
    #[test]
    fn the_random_bed_is_cut_to_the_lattice_count_and_builds() {
        let c = dem_centres().expect("centres");
        let core = c.iter().filter(|p| p[2].get::<meter>() > 0.0).count();
        assert_eq!(core, DEM_CORE_BALLS);
        assert!(c.len() > core, "the conus and tube pebbles are kept");
        let fuel = paper_fuel_assignment(&c);
        assert!(fuel.iter().filter(|&&f| f).count() > 9000);
        let dem = build_core(DEM_BED).expect("DEM core");
        let lat = build_core(12).expect("lattice core");
        assert!(dem.cells > lat.cells, "{} against {}", dem.cells, lat.cells);
        assert_eq!(
            build_core(99).expect("clamped").cells,
            build_core(20).expect("N = 20").cells
        );
        assert!(label(DEM_BED).contains("random") && label(12).contains("N = 12"));
    }
}
