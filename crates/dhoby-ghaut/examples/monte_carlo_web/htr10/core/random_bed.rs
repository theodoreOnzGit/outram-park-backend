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

use nee_soon::htr10_rmc::control_rod::N_CONTROL_RODS;
use nee_soon::htr10_rmc::core_design::Htr10CoreDesign;
use nee_soon::htr10_rmc::core_model::{assemble_explicit_triso, assemble_explicit_triso_with, AssembledCore};
use nee_soon::htr10_rmc::explicit_bed::{assemble_explicit_triso_from_centres_with, paper_fuel_assignment};
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

/// The core for a bed code, rods withdrawn (the record's state).
pub fn build_core(code: usize) -> Result<AssembledCore, String> {
    build_core_with(code, &WITHDRAWN)
}

/// Number of control rods (`nee_soon`'s).
pub const N_RODS: usize = N_CONTROL_RODS;

/// Every rod withdrawn: the record's state and the default.
pub const WITHDRAWN: [f64; N_CONTROL_RODS] = [0.0; N_CONTROL_RODS];

/// The core for a bed code with each control rod at its own insertion
/// (`rods[i]` in `[0, 1]`, rod `i` as `Htr10CoreDesign::rod_insertions`
/// numbers them; gh:#1025). With [`WITHDRAWN`] it is [`build_core`]'s core
/// exactly, since `Htr10CoreDesign::default()` is the record's design.
///
/// # Errors
/// An insertion outside `[0, 1]`, or the DEM pour cannot be read.
pub fn build_core_with(code: usize, rods: &[f64; N_CONTROL_RODS]) -> Result<AssembledCore, String> {
    let mut design = Htr10CoreDesign::default();
    design.rod_insertions = *rods;
    design.check()?;
    if code == DEM_BED {
        let c = dem_centres()?;
        let fuel = paper_fuel_assignment(&c);
        Ok(assemble_explicit_triso_from_centres_with(&c, &fuel, super::RINGS, 0, &design))
    } else if rods == &WITHDRAWN {
        Ok(assemble_explicit_triso(super::RINGS, code.clamp(10, 20), 0))
    } else {
        Ok(assemble_explicit_triso_with(super::RINGS, code.clamp(10, 20), 0, &design))
    }
}

/// Core pebbles (centred above the bed floor, `PebbleBed::core_balls`) of
/// the lattice the core view builds, `assemble_explicit_triso(14, N, 0)`, for
/// N = 10 ..= 20. Quoted so the page can show the count without building the
/// bed; pinned to the model by `the_pebble_counts_are_the_beds`.
pub const LATTICE_CORE_BALLS: [usize; 11] = [14_021, 15_351, 16_681, 18_011, 19_341, 20_671, 22_001, 23_331, 24_661, 25_991, 27_321];

/// The core pebbles of a bed code: the DEM bed's, or the lattice's at N.
pub fn core_pebbles(code: usize) -> Option<usize> {
    if code == DEM_BED {
        Some(DEM_CORE_BALLS)
    } else {
        code.checked_sub(10).and_then(|i| LATTICE_CORE_BALLS.get(i)).copied()
    }
}

/// A count with thin-space thousands, as the page prints it (16 681).
pub fn grouped(n: usize) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push(' ');
        }
        out.push(c);
    }
    out
}

/// What the bed code means, for the page: the bed and its core pebbles.
pub fn label(code: usize) -> String {
    let pebbles = core_pebbles(code).map_or(String::new(), |n| format!(" · {} pebbles", grouped(n)));
    if code == DEM_BED {
        format!("DEM random bed{pebbles} (as the lattice at N = 12)")
    } else {
        format!("Şeker lattice, N = {code}{pebbles}")
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

    /// The page's pebble counts are the beds the workers build (2026-10-26,
    /// the main view shows them), and the label carries them.
    #[test]
    fn the_pebble_counts_are_the_beds() {
        let built: Vec<usize> = (10..=20)
            .map(|n| assemble_explicit_triso(super::super::RINGS, n, 0).bed.as_ref().and_then(|b| b.core_balls()).expect("Şeker bed reports its balls"))
            .collect();
        println!("core balls N = 10..=20: {built:?}");
        assert_eq!(built.as_slice(), LATTICE_CORE_BALLS.as_slice());
        assert_eq!(LATTICE_CORE_BALLS[2], DEM_CORE_BALLS, "the DEM bed is cut to the lattice at N = 12");
        assert_eq!(label(12), "Şeker lattice, N = 12 · 16 681 pebbles");
        assert_eq!(grouped(999), "999");
        assert_eq!(grouped(1000), "1 000");
        assert_eq!(core_pebbles(9), None);
    }

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
