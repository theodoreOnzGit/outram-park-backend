//! **The SPATIAL half of self-shielding, against the exact homogeneous limit**
//! (gh:#178, bead op-qho6).
//!
//! Scales a Wigner-Seitz cell (fuel sphere + moderator shell) up and down at
//! fixed cell-averaged composition. The thin-lump limit must reproduce the exact
//! homogeneous slowing-down solution — there is no modelling freedom left there
//! — and the growth of the escape probability with scale is the lumping effect
//! itself.
//!
//! Together with `examples/slowing_down_oracle.rs`, which does the same against
//! the same oracle in *energy* only, this excluded spatial transport from the
//! ring-RPT residual.
//!
//! # Read `vv_gate`'s doc comment before changing the boundary condition
//!
//! This program was first written with a **specular** reflective outer sphere,
//! which produced a flat +39–42 % offset that looked exactly like the physics
//! defect being hunted. It was a harness bug: specular reflection off a
//! concentric sphere conserves the impact parameter, so near-tangential neutrons
//! are trapped on their chords and can never re-enter the lump. The correct
//! Wigner-Seitz condition is **white**. Both are still run, and the gate asserts
//! they disagree, so the counter-example stays executable.
use njoy_outram_park_fork::reference_data::reference_endf;
use outram_mc_libs::geometry::surface::BoundaryType;
use outram_mc_libs::material::material::{Material, NuclideComponent};
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::pebble_beds::fhr_pebble::fhr_pebble_geometry;
use outram_mc_libs::physics::slowing_down::*;
use std::time::Instant;

const TEMP: f64 = 293.6;
const SIGMA_P_C12: f64 = 4.7392;

fn main() {
    let hist: usize = std::env::var("HIST")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(50_000);
    let sigma_b: f64 = std::env::var("SIGMA_B")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(300.0);
    let vfrac: f64 = std::env::var("VFRAC")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0.30);

    let t0 = Instant::now();
    let c12 = load("C12", "n-006_C_012-ENDF8.0.endf");
    let u238 = load("U238", "n-092_U_238.endf");
    eprintln!("data ready in {:.1} s", t0.elapsed().as_secs_f64());
    let nuclides = vec![c12, u238];

    let band = SlowingDownBand {
        e_top: 10.0e3,
        e_bot: 1.0,
    };
    let mut grid = nuclides[1].native_energy_grid(band.e_bot * 0.9, band.e_top * 1.1);
    grid.extend_from_slice(&nuclides[0].native_energy_grid(band.e_bot * 0.9, band.e_top * 1.1));
    grid.sort_by(|a, b| a.partial_cmp(b).unwrap());
    grid.dedup_by(|a, b| (*a - *b).abs() <= 1e-12 * b.abs());

    // Cell-averaged composition: identical to the homogeneous case, so the
    // thin-lump limit is a like-for-like comparison.
    let n_u8_avg = 1.0e-3;
    let n_c_avg = sigma_b / SIGMA_P_C12 * n_u8_avg;
    let mix = vec![
        MixComponent {
            nuclide_idx: 0,
            atom_density: n_c_avg,
        },
        MixComponent {
            nuclide_idx: 1,
            atom_density: n_u8_avg,
        },
    ];
    let det = solve_deterministic(&nuclides, &mix, band, TEMP, &grid);
    // Rows of the lump-size scan, kept for the V&V gate at the bottom.
    let mut scan: Vec<ScanRow> = Vec::new();
    let hom_mc = InfiniteMediumMc {
        histories: hist,
        seed: 0xABCD_0001,
        kernel: ScatterKernel::IsotropicCmAtRest,
        max_collisions: 200_000,
    }
    .run(&nuclides, &mix, band, TEMP);
    eprintln!(
        "\nhomogeneous reference: deterministic p_esc {:.5}, 0-D MC {:.5} ± {:.5}",
        det.escaped,
        hom_mc.escaped,
        InfiniteMediumMc {
            histories: hist,
            ..Default::default()
        }
        .stderr_of(hom_mc.escaped)
    );

    // Two-region materials: volume-weighted back to the average above.
    let comp = |v: &[(usize, f64)]| -> Vec<NuclideComponent> {
        v.iter()
            .map(|&(nuclide_idx, atom_density)| NuclideComponent {
                nuclide_idx,
                atom_density,
            })
            .collect()
    };
    let fuel = Material {
        id: 1,
        name: "U-238 lump".into(),
        temperature: TEMP,
        components: comp(&[(1, n_u8_avg / vfrac)]),
    };
    let moderator = Material {
        id: 2,
        name: "graphite".into(),
        temperature: TEMP,
        components: comp(&[(0, n_c_avg / (1.0 - vfrac))]),
    };
    let homogeneous = Material {
        id: 3,
        name: "homogenised".into(),
        temperature: TEMP,
        components: comp(&[(0, n_c_avg), (1, n_u8_avg)]),
    };

    // ── Control ────────────────────────────────────────────────────────────
    // The SAME walker on a cell whose two regions carry the HOMOGENISED
    // material, so the exact answer is known at every size. The cell boundary is
    // then fictitious to the physics but entirely real to the code, and the
    // number of reflective crossings per collision runs from ~1 at R = 3 cm to
    // several hundred at R = 0.01 cm (the moderator mean free path here is
    // 2.3 cm). If `locate` / `distance_to_boundary` / `cross_surface` lost or
    // double-counted any part of a flight, a several-hundred-crossing walk would
    // not give the same answer as a one-crossing walk — and a bias that grows
    // with crossing frequency is exactly the kind that would hit a lumped
    // geometry harder than a homogeneous one.
    println!(
        "\nCONTROL - both regions homogenised, so the exact answer is {:.5} at every size",
        det.escaped
    );
    println!(
        "{:>12}  {:>11}  {:>10}  {:>9}  {:>10}",
        "R_cell [cm]", "p_esc", "1 sigma", "vs exact", "lost"
    );
    for &r_cell in &[0.03_f64, 0.1, 0.3, 1.0, 3.0] {
        let mats = vec![homogeneous.clone(), homogeneous.clone()];
        let geom = fhr_pebble_geometry(
            0.0,
            0.5 * r_cell,
            0.75 * r_cell,
            r_cell,
            0,
            1,
            1,
            BoundaryType::Vacuum,
            TEMP,
        );
        let n = if r_cell < 0.05 { hist / 5 } else { hist };
        let t = Instant::now();
        let r = LumpCellMc {
            histories: n,
            seed: 0xABCD_0002,
            kernel: ScatterKernel::IsotropicCmAtRest,
            boundary: CellBoundary::White,
            max_events: 20_000_000,
        }
        .run(&geom, &mats, &nuclides, band, TEMP, r_cell);
        println!(
            "{r_cell:>12.2}  {:>11.5}  {:>10.5}  {:>+8.2} %  {:>10.1e}   ({n} histories, {:.1} s)",
            r.escaped,
            r.stderr_of(r.escaped),
            100.0 * (r.escaped / det.escaped - 1.0),
            r.lost,
            t.elapsed().as_secs_f64()
        );
    }

    println!(
        "\nsigma_b = {sigma_b} b, fuel volume fraction {vfrac}, U-238 in the lump {:.5} /b·cm",
        n_u8_avg / vfrac
    );
    println!(
        "{:>12}  {:>10}  {:>9}  {:>11}  {:>10}  {:>9}  {:>11}  {:>9}",
        "R_cell [cm]",
        "R_fuel",
        "R/mfp*",
        "WHITE p_esc",
        "1 sigma",
        "vs hom.",
        "SPECULAR",
        "vs hom."
    );
    // Σ_t of the lump at the 6.674 eV resonance peak, for an optical-thickness
    // scale — measured, not recalled.
    let mut sigma_peak: f64 = 0.0;
    let mut e = 6.0_f64;
    while e <= 7.5 {
        sigma_peak = sigma_peak.max(fuel.macro_xs_total(e, &nuclides));
        e += 5.0e-4;
    }
    eprintln!(
        "lump Σ_t at the 6.67 eV peak: {sigma_peak:.1} cm⁻¹ (mfp {:.1} µm)",
        1.0e4 / sigma_peak
    );

    // The scan cannot reach R_cell → 0 by brute force: the MODERATOR's mean free
    // path is ~2.3 cm here, so a 1 µm cell costs ~23 000 reflective boundary
    // crossings per collision. It does not need to. What the thin-lump anchor
    // requires is that the LUMP be optically thin at the resonance peak, and at
    // R_cell = 0.01 cm the lump is 0.17 mfp across — transparent — while the
    // cell still only costs a few hundred crossings per collision.
    for &r_cell in &[0.006_f64, 0.02, 0.05, 0.15, 0.5, 1.5] {
        let r_fuel = r_cell * vfrac.cbrt();
        let mats = vec![fuel.clone(), moderator.clone()];
        let geom = fhr_pebble_geometry(
            0.0,
            r_fuel,
            0.5 * (r_fuel + r_cell),
            r_cell,
            0,
            1,
            1,
            BoundaryType::Vacuum,
            TEMP,
        );
        let geom_spec = fhr_pebble_geometry(
            0.0,
            r_fuel,
            0.5 * (r_fuel + r_cell),
            r_cell,
            0,
            1,
            1,
            BoundaryType::Reflective,
            TEMP,
        );
        // The tiny cells cost hundreds of reflective crossings per collision, so
        // they get fewer histories; 3000 still separates 0.39 from 0.55 by 18 sigma.
        let n = if r_cell < 0.01 {
            (hist / 10).max(3000)
        } else {
            hist
        };
        let t = Instant::now();
        let mc = |g: &_, b| {
            LumpCellMc {
                histories: n,
                seed: 0xABCD_0003,
                kernel: ScatterKernel::IsotropicCmAtRest,
                boundary: b,
                max_events: 40_000_000,
            }
            .run(g, &mats, &nuclides, band, TEMP, r_cell)
        };
        let r = mc(&geom, CellBoundary::White);
        let sp = mc(&geom_spec, CellBoundary::Specular);
        scan.push(ScanRow {
            r_cell,
            r_over_mfp: r_fuel * sigma_peak,
            white: r.escaped,
            white_stderr: r.stderr_of(r.escaped),
            specular: sp.escaped,
        });
        println!(
            "{r_cell:>12.1e}  {r_fuel:>10.3e}  {:>9.3}  {:>11.5}  {:>10.5}  {:>+8.2} %  \
             {:>11.5}  {:>+8.2} %   ({:.0} s)",
            r_fuel * sigma_peak,
            r.escaped,
            r.stderr_of(r.escaped),
            100.0 * (r.escaped / det.escaped - 1.0),
            sp.escaped,
            100.0 * (sp.escaped / det.escaped - 1.0),
            t.elapsed().as_secs_f64()
        );
    }
    println!(
        "\n* R/mfp is the lump radius in mean free paths at the 6.674 eV resonance peak.\n  \
         The first row must reproduce the homogeneous answer; the rise with scale is\n  \
         the lumping effect."
    );

    vv_gate(&scan, det.escaped);
}

/// One row of the lump-size scan, kept for the V&V gate.
struct ScanRow {
    /// Wigner-Seitz cell radius, cm.
    r_cell: f64,
    /// Lump radius in mean free paths at the 6.674 eV resonance peak.
    r_over_mfp: f64,
    /// Escape probability with a **white** (isotropic re-entry) cell boundary.
    white: f64,
    /// 1 sigma on `white`.
    white_stderr: f64,
    /// Escape probability with a **specular** cell boundary — kept as the
    /// counter-example, not as a result. See [`vv_gate`].
    specular: f64,
}

/// V&V gate: the spatial half of self-shielding, against the exact homogeneous
/// limit — and against the boundary condition that looks right and is not.
///
/// # The oracle
///
/// As the lump shrinks at fixed cell-averaged composition, the two-region
/// problem must converge onto the **homogeneous** one, whose answer
/// `solve_deterministic` gives exactly (a Volterra quadrature of the
/// infinite-medium slowing-down equation on the same nuclear data). There is no
/// modelling freedom left in that limit: the thin-lump row and the homogeneous
/// row are the same physics, so they must agree to counting statistics.
///
/// # THE BOUNDARY CONDITION IS THE LESSON HERE
///
/// This program was written with a **specular** reflective outer sphere, which
/// is what "reflective boundary" usually means in a CSG code. On a *sphere* it
/// is wrong, and wrong in a way that mimics the defect being hunted.
///
/// Specular reflection off a concentric sphere conserves the impact parameter
/// `b = r sin(theta)`. A neutron launched on a near-tangential path is therefore
/// trapped on that chord forever and **can never re-enter the fuel lump**;
/// one launched through the centre keeps hitting it. The cell stops being a
/// stand-in for an infinite lattice and becomes a set of disconnected orbits.
///
/// The result was a flat **+39 % to +42 %** offset in escape probability across
/// the whole scan — large, one-signed, present even in the thin-lump limit where
/// the answer is known exactly, and indistinguishable at a glance from "the
/// transport is not self-shielding properly", which is what the ring-RPT hunt
/// was looking for at the time. Days can go into a harness bug that presents as
/// a physics result.
///
/// The correct Wigner-Seitz condition is **white**: on escape, re-enter at a
/// uniformly random point on the sphere with a cosine-distributed inward
/// direction, which is what a neutron leaving one cell of an infinite lattice
/// actually does.
///
/// Both are still run, and the gate asserts that they **disagree**. That is
/// deliberate: it keeps the counter-example executable, so the next person to
/// reach for a specular boundary here finds the reason in a failing assertion
/// rather than in a comment nobody read.
///
/// # Results
///
/// Printed in full by the scan above, with the date on the run. The gate derives
/// its tolerances from that run's own counting statistics rather than from
/// numbers written here.
fn vv_gate(scan: &[ScanRow], det: f64) {
    use outram_mc_libs::vv::{assert_absolute, assert_monotone};

    println!("\n=== V&V gate: lumped self-shielding against the homogeneous limit ===");
    assert!(
        scan.len() >= 3,
        "the lump scan produced {} rows; the gate needs at least three to see a trend",
        scan.len()
    );

    // 1. The thin-lump limit IS the homogeneous problem.
    let thin = &scan[0];
    assert!(
        thin.r_over_mfp < 0.2,
        "the thinnest lump in the scan is {:.3} mean free paths across at the \
         6.674 eV peak (cell radius {:.3e} cm). That is not a thin lump, so the \
         homogeneous limit is not being tested at all and the first assertion \
         below would be checking nothing.",
        thin.r_over_mfp,
        thin.r_cell,
    );
    assert_absolute(
        "thin-lump p_esc vs the exact homogeneous solution (white boundary)",
        thin.white,
        det,
        (4.0 * thin.white_stderr).max(0.005 * det),
    );

    // 2. Lumping raises the escape probability. This is the effect itself; a
    //    flat curve would pass claim 1 and mean the geometry is doing nothing.
    let white_curve: Vec<f64> = scan.iter().map(|r| r.white).collect();
    assert_monotone(
        "p_esc rises with lump size (spatial self-shielding)",
        &white_curve,
        true,
        0.02,
    );
    let (first, last) = (white_curve[0], white_curve[white_curve.len() - 1]);
    assert!(
        last - first > 0.01,
        "p_esc moved only {:.4} across the whole scan ({first:.5} -> {last:.5}), \
         from {:.3} to {:.3} mean free paths. Spatial self-shielding is barely \
         being modelled; a flat curve passes the thin-lump agreement above.",
        last - first,
        scan[0].r_over_mfp,
        scan[scan.len() - 1].r_over_mfp,
    );

    // 3. The counter-example. See this function's doc comment.
    let worst_spec = scan
        .iter()
        .map(|r| (r.specular / r.white - 1.0).abs())
        .fold(0.0_f64, f64::max);
    println!(
        "  specular vs white boundary: worst departure {:.1} % across the scan",
        100.0 * worst_spec
    );
    assert!(
        worst_spec > 0.10,
        "the specular and white cell boundaries now agree to {:.1} %. They must \
         not: specular reflection off a concentric sphere conserves the impact \
         parameter b = r sin(theta), so a near-tangential neutron is trapped on \
         its chord and can never re-enter the lump. That produced a flat +39 % \
         to +42 % offset here and looked exactly like the self-shielding defect \
         being hunted.\n\
         If they now agree, either `CellBoundary::Specular` has stopped being \
         specular or `White` has stopped being white — and the counter-example \
         this gate keeps executable has been silently removed.",
        100.0 * worst_spec,
    );
    println!(
        "  [PASS] specular and white disagree, as they must — the impact-parameter \
         trap is still reproducible"
    );
}

fn load(name: &str, file: &str) -> Nuclide {
    let p = reference_endf(file).unwrap_or_else(|| panic!("missing {file}"));
    Nuclide::from_endf_file(&p, name, TEMP, 1.0e-3).expect("reconstruct")
}
