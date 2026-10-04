// SPDX-License-Identifier: GPL-3.0

//! **Natural uranium in lumps inside graphite: `k_inf` against lump size.**
//! Tutorial rung 3 (GitHub #525, epic #520), following rung 2's homogeneous
//! mixture (`ugraphite_four_factor.rs`, GitHub #524).
//!
//! > Research, education and V&V only. Nothing here is authoritative for the
//! > operation, licensing or safety of any reactor (`RESPONSIBLE_USE.md`).
//!
//! Rung 2 found how far a homogeneous mixture of natural uranium and graphite
//! gets (its recorded sweep is in `ugraphite_four_factor.rs`). This program
//! keeps the **same atoms in the same proportion** and gathers the uranium into
//! one sphere per cell, then asks how `k_inf` changes with the size of the
//! lump. Lumping lets the uranium shield itself in space: at a U-238 resonance
//! peak the lump's skin absorbs the neutrons arriving from the graphite, and
//! the interior sees few, so `p` rises; the price is that thermal neutrons must
//! now reach the lump too, so `f` falls a little.
//!
//! **Verification, not validation.** There is no experiment on this cell; the
//! checks are the homogeneous limit (rung 2's own result) and OpenMC.
//!
//! # The cell
//!
//! A **Wigner-Seitz sphere**: a uranium-metal sphere of radius `r` at the
//! centre of a graphite sphere of radius `R`, built with the same constructor
//! `lump_self_shielding_scan.rs` uses (`fhr_pebble_geometry`, a uranium ball,
//! two graphite shells), with a **white** outer boundary: a neutron reaching
//! `R` is sent back in with a cosine-law direction
//! (`SurfaceKindExt::diffuse_reflect`, a port of OpenMC's
//! `Surface::diffuse_reflect`, verified against OpenMC in
//! `verification_and_validation/white_boundary/`). **Not** a specular sphere:
//! read `lump_self_shielding_scan.rs`'s `vv_gate` doc first. A specular sphere
//! keeps each neutron's impact parameter for ever, so near-tangential neutrons
//! never re-enter the lump, and in that study it gave a flat +39-42 % error
//! that looked like physics. White is the standard Wigner-Seitz approximation
//! to an infinite lattice; it is an approximation of the real (cubic or
//! hexagonal) lattice, and is the same one on both sides of the OpenMC
//! comparison.
//!
//! - Lump: natural uranium metal at 19.05 g/cm3 (a handbook density, not
//!   page-checked here), U-234 / U-235 / U-238 in the IUPAC natural atom
//!   fractions of `common/ugraphite_common.rs`.
//! - Moderator: graphite at 1.73 g/cm3, the density rung 2 uses.
//! - Cell-average `N_C/N_U` fixed at **600** (env `CU`). Chosen **before** the
//!   rung-2 sweep had produced any number: it is one of that sweep's grid
//!   points, so the homogeneous limit can be compared with a recorded result
//!   directly, and it lies inside the 500-800 range where the hand estimate
//!   of rung 2 put the homogeneous optimum. It is not chosen to make `k_inf`
//!   reach any value. With the two densities above it fixes the uranium volume
//!   fraction `v`, and `R = r / v^(1/3)`.
//! - Everything else as rung 2: ENDF/B-VIII.0 at 296 K through the workspace
//!   NJOY port, URR and DBRC on, crystalline-graphite S(alpha,beta) on C-12 and
//!   C-13. With all the uranium in its own material, the library's
//!   material-based definition of "fuel" is the textbook `f`.
//!
//! # Checks
//!
//! 1. **Homogeneous limit (can fail).** The same Wigner-Seitz geometry with
//!    *both* regions filled with the cell-average mixture must give rung 2's
//!    homogeneous `k_inf` at `N_C/N_U = 600`: in an infinite homogeneous
//!    medium the white boundary is exact (the angular flux is isotropic), and
//!    `k_inf` depends only on the atom ratios, not on the absolute density. A
//!    lost or double-counted flight across the lump surface or the white
//!    boundary would show here. Shrinking the lump does **not** reach this
//!    limit at an affordable radius: U-238's 6.67 eV resonance has a mean free
//!    path of order 10 um in uranium metal, so even a 0.1 mm lump is black at
//!    the peak.
//! 2. **Telescoping**, as rung 2.
//! 3. **OpenMC code-to-code** at a few radii:
//!    `verification_and_validation/tutorial_rung3/openmc_inputs/lumped_openmc.py`.
//!
//! # Prediction, written 2026-10-04 BEFORE any run of this program
//!
//! (Committed after a 5000-history smoke test of the OpenMC deck at
//! `r = 2 cm`, which printed `k = 0.87, f = 0.63, p = 0.97` with no usable
//! statistics; the text below was not changed after it.)
//!
//! `k_inf` rises with `r` from the homogeneous value, passes a maximum and
//! falls (the issue's statement of the expected shape). Whether the maximum
//! exceeds 1 is the measured result. My expectation, from memory of
//! natural-uranium-graphite lattice physics and not from a page-checked
//! source: the gain from lumping is large (tens of per cent in `k_inf`, nearly
//! all in `p`), the maximum is **above 1, of order 1.05**, at a lump radius of
//! **order 1-3 cm**, and `f` falls by a few per cent over the same range.
//!
//! # Results
//!
//! Not yet run.
//!
//! # Running
//!
//! ```text
//! cargo run --release -p outram-mc-libs --features endf-pebble-cases \
//!     --example lumped_ugraphite_kinf                    # scan + homogeneous control
//! MODE=images cargo run --release -p outram-mc-libs --features endf-pebble-cases \
//!     --example lumped_ugraphite_kinf                    # draw the cells (PNG)
//! ```
//!
//! Environment (all optional): `PARTICLES` (20000), `INACTIVE` (20), `ACTIVE`
//! (100), `THREADS` (3), `SEED`, `CU` (600), `RADII` (comma list, cm),
//! `CONTROL` (0 skips the homogenised control).

#[path = "common/ugraphite_common.rs"]
mod ugraphite_common;

use outram_mc_libs::geometry::geometry::Geometry;
use outram_mc_libs::geometry::plot::{render_material_slice, PlotBasis, Rgb, SlicePlot};
use outram_mc_libs::geometry::position::Position;
use outram_mc_libs::geometry::surface::BoundaryType;
use outram_mc_libs::pebble_beds::fhr_pebble::fhr_pebble_geometry;
use std::time::Instant;
use ugraphite_common::{
    env_or, graphite_density, load_nuclides, natural_mix, print_case, run_case, FuelSplit, Mix,
    M_U234, M_U235, M_U238, NAT_U, NA_B, TEMP_K,
};

/// Natural uranium metal density \[g/cm3\] (handbook value, not page-checked).
const RHO_U_METAL: f64 = 19.05;

/// The lump radii scanned by default \[cm\].
const RADII_CM: &[f64] = &[0.1, 0.3, 1.0, 2.0, 3.0, 4.0, 6.0];

/// Lump radius of the homogenised control cell \[cm\]. Any radius would do;
/// this one puts the internal surface where the scan's interesting rows are.
const CONTROL_R_CM: f64 = 2.0;

/// Natural uranium metal \[atoms/b-cm\].
fn uranium_metal() -> Mix {
    let m_u = NAT_U[0] * M_U234 + NAT_U[1] * M_U235 + NAT_U[2] * M_U238;
    let n_u = RHO_U_METAL * NA_B / m_u;
    Mix {
        u: [NAT_U[0] * n_u, NAT_U[1] * n_u, NAT_U[2] * n_u],
        c: 0.0,
    }
}

/// Uranium volume fraction that gives cell-average `c_per_u` with a pure
/// uranium-metal lump and pure graphite around it.
fn uranium_volume_fraction(c_per_u: f64) -> f64 {
    let n_u: f64 = uranium_metal().u.iter().sum();
    let n_c = graphite_density();
    // (1 - v) n_c / (v n_u) = c_per_u
    1.0 / (1.0 + c_per_u * n_u / n_c)
}

/// The Wigner-Seitz cell: lump (material 0) to `r`, graphite (material 1) to
/// `big_r`, white outer boundary. The middle sphere is only the constructor's
/// second graphite shell.
fn ws_cell(r: f64, big_r: f64) -> Geometry {
    fhr_pebble_geometry(
        0.0,
        r,
        0.5 * (r + big_r),
        big_r,
        0,
        1,
        1,
        BoundaryType::White,
        TEMP_K,
    )
}

fn main() {
    let cu: f64 = env_or("CU", 600.0);
    let v = uranium_volume_fraction(cu);
    let radii: Vec<f64> = std::env::var("RADII")
        .ok()
        .map(|s| s.split(',').map(|x| x.trim().parse().expect("RADII")).collect())
        .unwrap_or_else(|| RADII_CM.to_vec());
    let cell_r = |r: f64| r / v.cbrt();

    if std::env::var("MODE").as_deref() == Ok("images") {
        draw(&radii, cell_r);
        return;
    }

    let seed: u64 = env_or("SEED", 20_261_005);
    eprintln!("Reconstructing nuclides (ENDF/B-VIII.0, {TEMP_K} K):");
    let t_load = Instant::now();
    let nuclides = load_nuclides();
    eprintln!("data ready in {:.1} s", t_load.elapsed().as_secs_f64());
    let lump = uranium_metal();
    let graphite = Mix {
        u: [0.0; 3],
        c: graphite_density(),
    };
    println!(
        "settings: PARTICLES {} x [INACTIVE {} + ACTIVE {}], THREADS {}, SEED {seed}, {TEMP_K} K; \
         cell-average N_C/N_U = {cu}, uranium volume fraction v = {v:.6e}, R/r = {:.4}",
        env_or::<usize>("PARTICLES", 20_000),
        env_or::<usize>("INACTIVE", 20),
        env_or::<usize>("ACTIVE", 100),
        env_or::<usize>("THREADS", 3),
        1.0 / v.cbrt()
    );
    println!(
        "lump: U metal {RHO_U_METAL} g/cm3, N_U = {:.6e} /b-cm; moderator: graphite N_C = {:.6e} /b-cm",
        lump.u.iter().sum::<f64>(),
        graphite.c
    );

    // ── Check 1: the homogenised cell must give rung 2's homogeneous k_inf ───
    let mut rows = Vec::new();
    if env_or::<u32>("CONTROL", 1) != 0 {
        let hom = Mix {
            u: [lump.u[0] * v, lump.u[1] * v, lump.u[2] * v],
            c: graphite.c * (1.0 - v),
        };
        let r = CONTROL_R_CM;
        let geom = ws_cell(r, cell_r(r));
        let mats = vec![hom.material(1, "homogenised"), hom.material(2, "homogenised")];
        let res = run_case(&geom, &mats, &nuclides, seed, r, FuelSplit::ByNuclide(hom));
        print_case(
            &format!("CONTROL: Wigner-Seitz cell r = {r} cm, both regions homogenised"),
            hom.c_per_u(),
            &res,
        );
        let ref_mix = natural_mix(cu);
        println!(
            "  same ratio as rung 2's natural_mix({cu}): N_C/N_U {:.4} vs {:.4}",
            hom.c_per_u(),
            ref_mix.c_per_u()
        );
        rows.push((0.0, res));
    }

    // ── The scan ────────────────────────────────────────────────────────────
    for (i, &r) in radii.iter().enumerate() {
        let big_r = cell_r(r);
        let geom = ws_cell(r, big_r);
        let mats = vec![lump.material(1, "U metal"), graphite.material(2, "graphite")];
        let res = run_case(&geom, &mats, &nuclides, seed + 1 + i as u64, r, FuelSplit::ByMaterial);
        print_case(
            &format!("lump r = {r} cm, cell R = {big_r:.4} cm (white)"),
            cu,
            &res,
        );
        rows.push((r, res));
    }

    println!("\n=== Rung 3: natural U metal lumps in graphite, N_C/N_U = {cu}, {TEMP_K} K ===");
    println!(
        "{:>8} {:>9} {:>9} {:>8} {:>8} {:>8} {:>8} {:>8} {:>9} {:>8}",
        "r [cm]", "R [cm]", "k_inf", "sigma", "eta", "f", "p", "eps", "k_factors", "wall s"
    );
    for (r, res) in &rows {
        let f = &res.factors;
        let label = if *r == 0.0 {
            "homog.".to_string()
        } else {
            format!("{r:.2}")
        };
        println!(
            "{label:>8} {:>9.4} {:>9.5} {:>8.5} {:>8.5} {:>8.5} {:>8.5} {:>8.5} {:>9.5} {:>8.1}",
            if *r == 0.0 { cell_r(CONTROL_R_CM) } else { cell_r(*r) },
            res.report.keff.k_mean,
            res.report.keff.k_std,
            f.eta.mean,
            f.f.mean,
            f.p.mean,
            f.epsilon.mean,
            f.k_from_factors.mean,
            res.wall_s
        );
    }
}

/// Draw what the solver sees: an x-y slice through the centre of each cell,
/// coloured by material, from the assembled geometry.
fn draw(radii: &[f64], cell_r: impl Fn(f64) -> f64) {
    let out = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("verification_and_validation/tutorial_rung3/geometry");
    std::fs::create_dir_all(&out).expect("create image directory");
    let pal = vec![
        (Rgb::new(40, 90, 200), "natural U metal (lump)"),
        (Rgb::new(90, 90, 90), "graphite"),
    ];
    for &r in radii {
        let big_r = cell_r(r);
        let geom = ws_cell(r, big_r);
        // Whole cell, and a zoom on the lump.
        for (tag, half) in [("cell", 1.05 * big_r), ("lump", 1.6 * r)] {
            let plot = SlicePlot::new(
                PlotBasis::Xy,
                Position::new(0.0, 0.0, 0.0),
                [2.0 * half, 2.0 * half],
                [600, 600],
            );
            let (_raw, img) = render_material_slice(
                &geom,
                &plot,
                &pal,
                &format!("rung 3: lump radius {r} cm, cell radius {big_r:.3} cm, white"),
            );
            let path = out.join(format!("ws_cell_r{:05.2}cm_{tag}.png", r));
            img.write_png(&path).expect("write png");
            println!("wrote {}", path.display());
        }
    }
}
