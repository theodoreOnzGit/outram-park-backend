//! **Which bed the live core runs on** (gh:#786, maintainer-approved on #787,
//! 2026-10-08): Şeker & Çolak (2003)'s lattice at N layers, as every recorded
//! k, or the DEM random bed.
//!
//! The pool passes the choice around as one number, the "bed code" (the
//! `layers` field of [`super::CoreReq`] and [`super::AssemblyReport`], and
//! the slice's raster parameter): N = 10..=20 is the lattice at N layers,
//! [`DEM_BED`] (0) is the random bed, [`USER_POUR`] (1) the reader's own
//! pour (gh:#1244), which each worker holds once the page has sent it.
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

use nee_soon::mc_rungs::htr10_core as model;
use nee_soon::htr10_rmc::core_model::AssembledCore;
use uom::si::f64::Length;

// The bed codes, the DEM cut, the core builders and the lattice counts moved
// to `nee_soon::mc_rungs::htr10_core` (gh:#1742, 2026-11-25); the DEM cut
// there is `htr10_rmc::explicit_bed`'s, which this file duplicated (same
// centres bit for bit, `the_dem_cut_is_the_rungs_cut`). What stays: the
// compiled-in pour and the worker's own copy of the reader's pour, which the
// wrappers below pass in.
#[allow(unused_imports)] // the counts are read by this file's tests
pub use model::{is_dem, DEM_BED, DEM_CORE_BALLS, LATTICE_CORE_BALLS, N_RODS, USER_POUR, WITHDRAWN};
use model::N_RODS as N_CONTROL_RODS;

/// The reader's pour this worker builds [`USER_POUR`] on. One per worker
/// (each Web Worker has its own; natively the pool's threads share it, and
/// the page sends every one the same bed).
static POURED: std::sync::RwLock<Option<std::sync::Arc<super::pour::PouredBed>>> = std::sync::RwLock::new(None);

/// Hand this worker the reader's pour.
pub fn set_poured(b: super::pour::PouredBed) {
    if let Ok(mut g) = POURED.write() {
        *g = Some(std::sync::Arc::new(b));
    }
}

/// The reader's pour, if one has been handed over.
pub fn poured() -> Option<std::sync::Arc<super::pour::PouredBed>> {
    POURED.read().ok().and_then(|g| g.clone())
}

/// The version of the pour [`USER_POUR`] means here (0: none yet).
pub fn poured_version() -> u32 {
    poured().map_or(0, |b| b.version)
}

/// The centres and fuel flags of an explicit bed code (this worker's pour
/// for [`USER_POUR`]).
///
/// # Errors
/// Not an explicit bed, the baked pour cannot be read, or no pour has been
/// handed over.
pub fn explicit_bed(code: usize) -> Result<(Vec<[Length; 3]>, Vec<bool>), String> {
    model::explicit_bed(code, DEM_CSV, poured().as_deref())
}

/// The #216 pour (`id,x,y,z,vx,vy,vz`, metres, DEM frame).
const DEM_CSV: &str =
    include_str!("../../../../../../reference-data/liggghts/htr10_conus_presettled_mu10_mur00.csv");

/// The random bed's centres, cut as the module docs say.
#[cfg_attr(not(test), allow(dead_code))] // read by this file's tests
pub fn dem_centres() -> Result<Vec<[Length; 3]>, String> {
    model::dem_centres(DEM_CSV)
}

/// The core for a bed code, rods withdrawn (the record's state).
pub fn build_core(code: usize) -> Result<AssembledCore, String> {
    model::build_core(code, DEM_CSV, poured().as_deref())
}

/// The core for a bed code with each control rod at its own insertion
/// (`nee_soon::mc_rungs::htr10_core::build_core_with`, this worker's pour).
///
/// # Errors
/// An insertion outside `[0, 1]`, or the DEM pour cannot be read.
pub fn build_core_with(code: usize, rods: &[f64; N_CONTROL_RODS]) -> Result<AssembledCore, String> {
    model::build_core_with(code, rods, DEM_CSV, poured().as_deref())
}

/// The core pebbles of a bed code: the DEM bed's, or the lattice's at N.
pub fn core_pebbles(code: usize) -> Option<usize> {
    model::core_pebbles(code, poured().as_deref())
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
    } else if code == USER_POUR {
        match poured() {
            Some(b) => format!("your DEM pour #{}{pebbles}, {:.0} % fuel", b.version, 100.0 * b.fuel_fraction),
            None => "your DEM pour (not poured yet)".to_string(),
        }
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

    /// The reader's pour (gh:#1244): once handed over, the bed code builds
    /// it exactly as the baked bed builds the same balls, and the label and
    /// count follow it. (The only test that sets this worker's pour.)
    #[test]
    fn a_handed_over_pour_builds_as_the_baked_bed() {
        let c = dem_centres().expect("centres");
        let fuel = paper_fuel_assignment(&c);
        let b = super::super::pour::PouredBed { centres: c, is_fuel: fuel, version: 4, cut: 0, fuel_fraction: 0.57 };
        // Through the worker's message, as the page sends it.
        let req = super::super::CoreReq::pour_bed(&b);
        let mut w = super::super::CoreWorker::default();
        super::super::serve_native(&mut w, req, &mut |_| {});
        assert_eq!(poured_version(), 4);
        assert_eq!(core_pebbles(USER_POUR), Some(DEM_CORE_BALLS));
        assert!(label(USER_POUR).contains("#4"), "{}", label(USER_POUR));
        let mine = build_core(USER_POUR).expect("pour core");
        let baked = build_core(DEM_BED).expect("baked core");
        assert_eq!(mine.cells, baked.cells);
        assert_eq!(mine.geometry.lattices.len(), baked.geometry.lattices.len());
        match super::super::study(USER_POUR, &WITHDRAWN, &nee_soon::htr10_rmc::zones::ZoneTemperatures::record()).expect("study").bed {
            nee_soon::htr10_rmc::parametric::StudyBed::Explicit { centres, .. } => assert_eq!(centres.len(), b.centres.len()),
            other => panic!("{other:?}"),
        }
        assert!(explicit_bed(12).is_err());
        // The pour itself goes through the same worker (dem_web's engine).
        let mut got = 0;
        let start = super::super::pour::engine::Request::Start { id: 1, n: 3_000, seed: 1 };
        super::super::serve_native(&mut w, super::super::CoreReq::Pour(start), &mut |e| {
            if matches!(e, crate::engine::Event::Core(super::super::CoreEv::Pour(super::super::pour::engine::Event::Progress(_)))) {
                got += 1;
            }
        });
        assert_eq!(got, 1);
        // The review images of a poured core, drawn from the same CSV
        // (here the baked pour; `--render-pour` on a fresh one).
        let dir = std::env::temp_dir().join("htr10_render_pour_test");
        let text = super::super::pour::render_review(DEM_CSV, 0.57, &dir).expect("images");
        assert!(text.contains("core pebbles"), "{text}");
        for f in ["pour_core_xz.png", "pour_bed_xz.png", "pour_bed_xy.png", "pour_fuel_pebble_xy.png"] {
            assert!(dir.join(f).metadata().is_ok_and(|m| m.len() > 1000), "{f}");
        }
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
