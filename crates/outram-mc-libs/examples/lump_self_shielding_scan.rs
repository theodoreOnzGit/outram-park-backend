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
//! # The thick lump now has an oracle too (2026-09-12, bead `op-t9cr`)
//!
//! The transparent limit was the only anchored point: past it the scan reported
//! a monotone rise with nothing to check it against, and the FHR ring-RPT fuel
//! annulus sits at **6.6 mean free paths** at the 6.674 eV U-238 peak, where
//! that concentration of the heavy-metal inventory is worth +3327 pcm over naive
//! homogenisation. Every row now carries a **deterministic reference** as well:
//! [`solve_deterministic_multiregion`] solves the same slowing-down equation on
//! the same cell with **exact first-flight collision probabilities** from
//! `physics::collision_probability`, and no Monte Carlo anywhere in it. See
//! [`vv_gate_thick_lump`] for what that measured.
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
    // Flat-flux sub-shells inside the FUEL lump for the deterministic reference,
    // and the Gauss-Legendre order of its impact-parameter quadrature. Both are
    // convergence levers, not tuning parameters — see `vv_gate_thick_lump`.
    let fuel_sub: usize = std::env::var("FUEL_SUB")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(12);
    let cp_nodes: usize = std::env::var("CP_NODES")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(16);
    // `MODE=thick` runs ONLY the optically thick rows, the deterministic
    // reference and the ring-RPT annulus -- i.e. `vv_gate_thick_lump` and
    // `ring_rpt_annulus` and nothing else.
    //
    // The reason is cost, and the cost runs the opposite way to the physics.
    // The CONTROL section and the thin rows exist to anchor the TRANSPARENT
    // limit and to keep the specular counter-example executable, and a
    // transparent cell is the expensive one: the moderator's mean free path is
    // ~2.3 cm here, so an R = 0.03 cm cell costs ~80 reflective boundary
    // crossings per collision and a single control row is minutes where a thick
    // row is seconds. Those claims are settled and their recorded tables are in
    // `vv_gate`; when what is being measured is the THICK lump, paying for them
    // again buys nothing. `MODE=full` (the default) runs everything.
    let thick_only = std::env::var("MODE").map(|m| m == "thick").unwrap_or(false);

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
    let control_sizes: &[f64] = if thick_only {
        println!("  (skipped: MODE=thick)");
        &[]
    } else {
        &[0.03_f64, 0.1, 0.3, 1.0, 3.0]
    };
    println!(
        "{:>12}  {:>11}  {:>10}  {:>9}  {:>10}",
        "R_cell [cm]", "p_esc", "1 sigma", "vs exact", "lost"
    );
    for &r_cell in control_sizes {
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
        "{:>12}  {:>10}  {:>9}  {:>11}  {:>10}  {:>9}  {:>11}  {:>9}  {:>11}  {:>9}",
        "R_cell [cm]",
        "R_fuel",
        "R/mfp*",
        "WHITE p_esc",
        "1 sigma",
        "vs hom.",
        "ORACLE",
        "MC-ORACLE",
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
    // Index 0 is the fuel, index 1 the moderator, for every lumped run below
    // and for `ring_rpt_annulus`.
    let mats_lumped = vec![fuel.clone(), moderator.clone()];
    let scan_sizes: &[f64] = if thick_only {
        &[0.15_f64, 0.5, 1.5]
    } else {
        &[0.006_f64, 0.02, 0.05, 0.15, 0.5, 1.5]
    };
    for &r_cell in scan_sizes {
        let r_fuel = r_cell * vfrac.cbrt();
        let mats = &mats_lumped;
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
        #[allow(clippy::needless_late_init)]
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
        // History budget follows the cost, which runs the opposite way to the
        // physics of interest: a TINY cell costs hundreds of reflective boundary
        // crossings per collision (3000 histories there still separate 0.39 from
        // 0.55 by 18 sigma, which is all the specular counter-example needs),
        // while the THICK rows are both the cheapest per history and the only
        // ones `vv_gate_thick_lump` can judge against the deterministic
        // reference — so they get four times the budget, where it buys
        // something.
        let n = if r_cell < 0.01 {
            (hist / 10).max(3000)
        } else if r_fuel * sigma_peak >= 2.0 {
            hist * 4
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
            .run(g, mats, &nuclides, band, TEMP, r_cell)
        };
        let r = mc(&geom, CellBoundary::White);
        // The specular counter-example belongs to `vv_gate`, which `MODE=thick`
        // does not run; it costs as much as the measurement itself, so it is
        // skipped there and the field is left NaN rather than quietly filled
        // with the white answer (which would let `vv_gate` pass on a row it
        // never measured).
        let sp_escaped = if thick_only {
            f64::NAN
        } else {
            mc(&geom_spec, CellBoundary::Specular).escaped
        };
        // The deterministic reference on the SAME cell: exact first-flight
        // collision probabilities at every lethargy point, the fuel lump split
        // into `fuel_sub` equal-volume sub-shells so the flat-flux
        // discretisation can be refined away.
        let oracle = solve_deterministic_multiregion(
            &ShellCell {
                radii: vec![r_fuel, 0.5 * (r_fuel + r_cell), r_cell],
                material: vec![0, 1, 1],
            }
            .subdivide_each(&[fuel_sub, 1, 1]),
            mats,
            &nuclides,
            band,
            TEMP,
            &grid,
            cp_nodes,
            20,
        );
        scan.push(ScanRow {
            r_cell,
            r_over_mfp: r_fuel * sigma_peak,
            white: r.escaped,
            white_stderr: r.stderr_of(r.escaped),
            specular: sp_escaped,
            oracle: oracle.escaped,
        });
        println!(
            "{r_cell:>12.1e}  {r_fuel:>10.3e}  {:>9.3}  {:>11.5}  {:>10.5}  {:>+8.2} %  \
             {:>11.5}  {:>+8.2} %  {:>11.5}  {:>+8.2} %   ({:.0} s)",
            r_fuel * sigma_peak,
            r.escaped,
            r.stderr_of(r.escaped),
            100.0 * (r.escaped / det.escaped - 1.0),
            oracle.escaped,
            100.0 * (r.escaped / oracle.escaped - 1.0),
            sp_escaped,
            100.0 * (sp_escaped / det.escaped - 1.0),
            t.elapsed().as_secs_f64()
        );
    }
    println!(
        "\n* R/mfp is the lump radius in mean free paths at the 6.674 eV resonance peak.\n  \
         The first row must reproduce the homogeneous answer; the rise with scale is\n  \
         the lumping effect."
    );

    if thick_only {
        println!(
            "\n(MODE=thick: the transparent-limit and specular-boundary gate `vv_gate` \n              needs the thin rows and was not run. Run without MODE to get it.)"
        );
    } else {
        vv_gate(&scan, det.escaped);
    }
    vv_gate_thick_lump(&scan, det.escaped, fuel_sub, cp_nodes);
    ring_rpt_annulus(
        &nuclides,
        &mats_lumped,
        &grid,
        band,
        sigma_peak,
        hist * 4,
        fuel_sub,
        cp_nodes,
    );
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
    /// The **deterministic** escape probability for the same cell, from
    /// `solve_deterministic_multiregion`. No Monte Carlo in it. See
    /// [`vv_gate_thick_lump`].
    oracle: f64,
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
/// # Results (2026-09-11, ENDF/B-VIII.0 @ 293.6 K, sigma_b = 300 b, vfrac 0.30,
/// HIST=4000, seed 0xABCD_0003)
///
/// Exact homogeneous solution: `p_esc = 0.38763` (deterministic).
///
/// ```text
///   R_cell [cm]   R/mfp*   WHITE p_esc   1 sigma   vs hom.   SPECULAR   vs hom.
///     6.0e-3       0.103     0.38567     0.00889   -0.51 %    0.54067   +39.48 %
///     2.0e-2       0.343     0.40425     0.00776   +4.29 %    0.55650   +43.57 %
///     5.0e-2       0.856     0.38700     0.00770   -0.16 %    0.55200   +42.40 %
///     1.5e-1       2.569     0.38900     0.00771   +0.35 %    0.56100   +44.73 %
///     5.0e-1       8.564     0.42900     0.00783  +10.67 %    0.55450   +43.05 %
///     1.5e0       25.693     0.50175     0.00791  +29.44 %    0.58000   +49.63 %
/// ```
///
/// \* lump radius in mean free paths at the 6.674 eV resonance peak.
///
/// **The thin-lump limit reproduces the exact homogeneous answer to −0.51 %**,
/// well inside its own counting statistics, and `p_esc` then rises to +29.4 %
/// as the lump grows to 25.7 mean free paths — that rise *is* the spatial
/// self-shielding effect. Spatial transport is therefore excluded from the
/// ring-RPT residual, the same way `examples/slowing_down_oracle.rs` excludes
/// the energy treatment.
///
/// **The specular boundary is wrong by +39.5 % to +49.6 % at every size**,
/// including the thin-lump limit where the answer is known exactly. That is the
/// signature to recognise: an offset that is large, one-signed, and present even
/// where the physics is trivial, is a harness bug, not a physics result.
///
/// The gate derives its tolerances from each run's own counting statistics
/// rather than from the numbers above, so they are a record and cannot go stale
/// into a false pass.
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
    // The slack is 4 sigma of the WORST row's own counting statistics, not a
    // fixed percentage. `HIST` is configurable, so a fixed slack would make this
    // gate's verdict depend on how many histories the harness was given rather
    // than on the physics -- passing at HIST=50000 and failing at HIST=4000 for
    // no reason anyone should have to know about.
    let worst_stderr = scan
        .iter()
        .map(|r| r.white_stderr / r.white)
        .fold(0.0_f64, f64::max);
    let slack = 4.0 * worst_stderr;
    println!(
        "  monotonicity slack from this run's own statistics: {:.1} % \
         (4 sigma of the noisiest row)",
        100.0 * slack
    );
    assert_monotone(
        "p_esc rises with lump size (spatial self-shielding)",
        &white_curve,
        true,
        slack,
    );
    let (first, last) = (white_curve[0], white_curve[white_curve.len() - 1]);
    assert!(
        last - first > (0.01_f64).max(4.0 * worst_stderr * last),
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

/// The FHR ring-RPT fuel annulus **at its own radii**, Monte Carlo against the
/// deterministic reference.
///
/// # Why a second geometry
///
/// The scan above grows a fuel **ball**. The ring-RPT pebble's fuel is a
/// **shell** — 1.4934 to 1.7531 cm around an inner graphite ball — and a shell
/// has a different chord-length distribution from a ball at the same optical
/// thickness: every chord that would have passed through the middle of a ball
/// instead crosses the annulus twice with a transparent gap between, which is
/// precisely the geometry that decides how much of the lump's interior the flux
/// reaches. Running the same comparison on the real radii removes the last step
/// of inference between the scan and the pebble.
///
/// The materials are the scan's own, so the annulus is exactly the working point
/// the residual lives at: `Sigma_t = 25.59 cm-1` at the 6.674 eV U-238 peak over
/// a 0.2597 cm annulus is **6.65 mean free paths**.
///
/// The outer zones are graphite out to the ring-RPT root radius of 3 cm, and the
/// boundary is white rather than the pebble's own specular sphere — a specularly
/// reflective sphere conserves the impact parameter and is not a valid cell
/// closure (see [`vv_gate`]), and the deterministic reference is built on the
/// white one.
///
/// # Results (2026-09-12, ENDF/B-VIII.0 @ 293.6 K, `MODE=thick HIST=50000` so
/// 200 000 histories, seed 0xABCD_0004, the fuel annulus split into 12
/// equal-volume sub-shells, 16 Gauss nodes)
///
/// ```text
///   fuel shell 1.4934-1.7531 cm = 0.2597 cm = 6.65 mfp at the 6.674 eV peak
///   fuel volume fraction of the cell                     0.07620
///   volume-homogenised exact p_esc                       0.673208
///
///   MONTE CARLO    p_esc 0.797250 +/- 0.000899   fuel absorption 0.201380
///   DETERMINISTIC  p_esc 0.798467                fuel absorption 0.200160
///   MC - oracle    -0.152 % on p_esc, +0.61 % on the fuel absorption rate,
///                  -1.35 sigma; no history left the cell; worst white-closure
///                  conservation defect over the sweep 5.9e-12
///
///   lumping effect L = p_esc(annulus) - p_esc(homogenised)
///     MC 0.124042, oracle 0.125259, MC/oracle - 1 = -0.97 % +/- 0.72 %
/// ```
///
/// **On the annulus the residual actually lives in, the Monte Carlo's lumping
/// effect is right to -0.97 % +/- 0.72 %** — that is **-32 +/- 24 pcm** of the
/// ring-RPT pebble's +3327 pcm of lumping reactivity, against a residual of
/// +4004 pcm.
///
/// The sign is worth stating separately, because the hypothesis this was built
/// to test predicts a specific one. "Under-absorbing in the resonance range"
/// means the lump is too self-shielded, which shows up as `p_esc` too HIGH and
/// the fuel absorption rate too LOW. What is measured is the opposite: the
/// Monte Carlo absorbs **0.61 %** *more* in the fuel than the reference, at
/// 1.3 sigma. Whatever is worth +4004 pcm on this pebble, it is not the way
/// this code self-shields a thick annulus.
#[allow(clippy::too_many_arguments)]
fn ring_rpt_annulus(
    nuclides: &[Nuclide],
    mats: &[Material],
    grid: &[f64],
    band: SlowingDownBand,
    sigma_peak: f64,
    hist: usize,
    fuel_sub: usize,
    cp_nodes: usize,
) {
    /// Inner graphite ball of the ring-RPT pebble \[cm\] (`fhr_ring_rpt_endf`'s
    /// `R_RPT_INNER`; a fitted parameter of that deck, used here only as a
    /// length).
    const R_RPT_INNER: f64 = 1.493_359_375;
    /// Outer radius of the homogenised fuel shell \[cm\].
    const R_RPT_FUEL: f64 = 1.7531;
    /// Graphite shell outer radius \[cm\].
    const R_PEBBLE: f64 = 2.0;
    /// The ring-RPT root radius \[cm\].
    const R_ROOT: f64 = 3.0;

    println!("\n\n=== THE RING-RPT ANNULUS AT ITS OWN RADII ===");
    println!(
        "  fuel shell {R_RPT_INNER:.4}-{R_RPT_FUEL:.4} cm = {:.4} cm thick = {:.2} mean free \n  \
         paths at the 6.674 eV U-238 peak (Sigma_t = {sigma_peak:.2} /cm); graphite to \
         {R_PEBBLE} cm\n  and on to the {R_ROOT} cm root, white boundary.",
        R_RPT_FUEL - R_RPT_INNER,
        (R_RPT_FUEL - R_RPT_INNER) * sigma_peak,
    );

    // The exact homogeneous answer for THIS cell: the same inventory smeared
    // over the whole 3 cm ball. `solve_deterministic` gives it exactly, and the
    // multi-region solver reproduces it identically when every shell carries one
    // material (tests/lump_collision_probability.rs), so the two sides of the
    // lumping effect below are on the same footing.
    let vol = |r_out: f64, r_in: f64| r_out.powi(3) - r_in.powi(3);
    let v_fuel = vol(R_RPT_FUEL, R_RPT_INNER);
    let v_cell = vol(R_ROOT, 0.0);
    let f_fuel = v_fuel / v_cell;
    let smear = |m: &Material, f: f64| -> Vec<MixComponent> {
        m.components
            .iter()
            .map(|c| MixComponent {
                nuclide_idx: c.nuclide_idx,
                atom_density: c.atom_density * f,
            })
            .collect()
    };
    let mut mix = smear(&mats[0], f_fuel);
    mix.extend(smear(&mats[1], 1.0 - f_fuel));
    let hom = solve_deterministic(nuclides, &mix, band, TEMP, grid);
    println!(
        "  fuel volume fraction of the cell {f_fuel:.5}; volume-homogenised exact \
         p_esc = {:.6}",
        hom.escaped
    );

    let geom = fhr_pebble_geometry(
        R_RPT_INNER,
        R_RPT_FUEL,
        R_PEBBLE,
        R_ROOT,
        0,
        1,
        1,
        BoundaryType::Vacuum,
        TEMP,
    );
    let t = Instant::now();
    let mc = LumpCellMc {
        histories: hist,
        seed: 0xABCD_0004,
        kernel: ScatterKernel::IsotropicCmAtRest,
        boundary: CellBoundary::White,
        max_events: 40_000_000,
    }
    .run(&geom, mats, nuclides, band, TEMP, R_ROOT);
    let se = mc.stderr_of(mc.escaped);
    println!(
        "  MONTE CARLO  p_esc = {:.6} +/- {:.6}, fuel absorption {:.6}, lost {:.1e}  \
         ({hist} histories, {:.0} s)",
        mc.escaped,
        se,
        mc.absorbed_by[0],
        mc.lost,
        t.elapsed().as_secs_f64()
    );

    let t = Instant::now();
    let oracle = solve_deterministic_multiregion(
        &ShellCell {
            radii: vec![R_RPT_INNER, R_RPT_FUEL, R_PEBBLE, R_ROOT],
            material: vec![1, 0, 1, 1],
        }
        .subdivide_each(&[1, fuel_sub, 1, 1]),
        mats,
        nuclides,
        band,
        TEMP,
        grid,
        cp_nodes,
        20,
    );
    println!(
        "  DETERMINISTIC p_esc = {:.6}, fuel absorption {:.6}  \
         (fuel split into {fuel_sub} shells, conservation {:.1e}, {:.0} s)",
        oracle.escaped,
        oracle.absorbed_by_material[0],
        oracle.worst_conservation_defect,
        t.elapsed().as_secs_f64()
    );

    let z = (mc.escaped - oracle.escaped) / se;
    println!(
        "  MC - oracle: {:+.3} % on p_esc, {:+.2} % on the fuel absorption rate, {z:+.2} sigma",
        100.0 * (mc.escaped / oracle.escaped - 1.0),
        100.0 * (mc.absorbed_by[0] / oracle.absorbed_by_material[0] - 1.0),
    );

    let (l_mc, l_or) = (mc.escaped - hom.escaped, oracle.escaped - hom.escaped);
    let frac = l_mc / l_or - 1.0;
    let frac_sigma = se / l_or;
    const RING_RPT_LUMPING_PCM: f64 = 3327.0;
    println!(
        "  lumping effect L = p_esc(annulus) - p_esc(homogenised): \
         MC {l_mc:.6}, oracle {l_or:.6}\n  \
         MC/oracle - 1 = {:+.2} % +/- {:.2} %  ->  {:+.0} +/- {:.0} pcm of the ring-RPT \
         pebble's\n  +3327 pcm lumping reactivity (naive 1.37187 +/- 0.00232 vs explicit \
         1.40514 +/- 0.00204)",
        100.0 * frac,
        100.0 * frac_sigma,
        frac * RING_RPT_LUMPING_PCM,
        frac_sigma * RING_RPT_LUMPING_PCM,
    );

    assert!(
        (R_RPT_FUEL - R_RPT_INNER) * sigma_peak > 6.0,
        "the annulus is only {:.2} mean free paths thick at the resonance peak; the \
         whole point of this section is the deeply self-shielded regime",
        (R_RPT_FUEL - R_RPT_INNER) * sigma_peak
    );
    assert_eq!(
        mc.lost, 0.0,
        "{:.1e} of the histories left the cell; the white boundary is meant to return \
         every one of them, and the deterministic reference assumes a closed cell",
        mc.lost
    );
    // The same statistics-derived slack as `vv_gate_thick_lump`. If this fires,
    // the sign is the finding: MC above the reference means the annulus
    // under-absorbs, which is the direction the +4004 pcm ring-RPT residual
    // needs; MC below means the opposite and the residual is not here.
    outram_mc_libs::vv::assert_absolute(
        "the REAL ring-RPT annulus (6.65 mfp at the 6.674 eV peak): MC vs the          deterministic collision-probability solution",
        mc.escaped,
        oracle.escaped,
        4.0 * se + 0.002 * oracle.escaped,
    );
    println!(
        "  [PASS] the annulus geometry self-shields the 6.674 eV resonance the way an\n  \
         independent deterministic solution says it should."
    );
}

/// V&V gate: **the optically thick lump against a deterministic oracle** — the
/// one regime the scan above could not judge.
///
/// # What was missing, and what this closes
///
/// [`vv_gate`] anchors the **transparent** limit, where the two-region cell is
/// provably the homogeneous mixture and `solve_deterministic` gives the exact
/// answer. Past that it can only check that the curve *rises*. The FHR ring-RPT
/// fuel annulus (1.4934-1.7531 cm) is **6.6 mean free paths thick at the
/// 6.674 eV U-238 resonance peak** and carries the entire heavy-metal
/// inventory; that concentration is worth **+3327 pcm** over naive
/// homogenisation on this code's own numbers (naive 1.37187 +/- 0.00232 vs
/// explicit 1.40514 +/- 0.00204). Nothing checked whether it was the *right*
/// +3327 pcm, and "under-absorbing in the resonance range" is exactly what the
/// pebble's six-factor signature shows (p +8.5 %, eps -5.0 %, eta and f exact).
///
/// # The oracle, and why it is independent
///
/// `solve_deterministic_multiregion` solves the same slowing-down equation on
/// the same cell with **no Monte Carlo in it at all**:
///
/// - the spatial coupling is the **exact** first-flight collision probability
///   matrix `P_{i->j}(E)`, from an impact-parameter track quadrature of the
///   analytic six-fold collision integral
///   (`physics::collision_probability::first_flight`), recomputed at **every**
///   lethargy point so a resonance is self-shielded in space as well as in
///   energy, and closed with a white outer boundary;
/// - the energy treatment is `solve_on_grid`'s own Volterra march, which
///   `examples/slowing_down_oracle.rs` already showed reproduces the Monte Carlo
///   at every dilution from sigma_b = 30 b to 10 000 b.
///
/// It shares the reconstructed cross sections with the Monte Carlo and nothing
/// else — not the tracking, not the geometry traversal, not the random number
/// stream, not the collision sampling.
///
/// The geometric half is verified against closed forms in
/// `tests/lump_collision_probability.rs`: the bare-sphere escape probability to
/// 1.5e-13, surface reciprocity `4 V_i Sigma_i P_iS = A P_Si` to 3.7e-16,
/// white-closure conservation to 8.9e-16, and invariance under sub-shelling to
/// 4.4e-16. The coupled solve is verified by the one configuration whose answer
/// is known independently: give every shell the **same** material and it must
/// return the infinite-medium solution identically, which it does to **1.1e-12**
/// at cell radii from 0.01 cm to 100 cm and 3 to 24 shells.
///
/// # The one approximation, and its measured size
///
/// Flat flux within a sub-shell. It is removed by refining, and the refinement
/// was measured rather than assumed. The lumping effect
/// `L = p_esc(lump) - p_esc(homogeneous)` against the number of equal-volume
/// sub-shells inside the fuel (2026-09-12, ENDF/B-VIII.0 @ 293.6 K, sigma_b =
/// 300 b, vfrac 0.30; the percentage is the change from the row above):
///
/// ```text
///   fuel        R/mfp = 2.57          R/mfp = 8.56          R/mfp = 25.69
///   sub-shells   L         d          L         d           L         d
///      1      0.012373      -      0.043344      -       0.110698      -
///      2      0.012460  +0.701 %   0.043836  +1.134 %    0.111774  +0.972 %
///      4      0.012488  +0.224 %   0.044065  +0.522 %    0.112455  +0.609 %
///      8      0.012496  +0.066 %   0.044152  +0.198 %    0.112838  +0.341 %
///     12      0.012497  +0.013 %   0.044172  +0.046 %    0.112958  +0.106 %
///     16      0.012498  +0.005 %   0.044180  +0.018 %    0.113012  +0.047 %
/// ```
///
/// Geometric, so at the default `FUEL_SUB=12` the discretisation residual on
/// `L` is **below 0.02 % at 2.6 mean free paths and below 0.15 % at 25.7** —
/// an order of magnitude or more under the Monte Carlo's counting statistics,
/// which is why the gate's tolerances are drawn from the latter.
///
/// Two knobs that turn out **not** to matter, checked at all three sizes:
/// sub-dividing the MODERATOR as well moves `L` by at most 0.026 % (it is
/// 0.15 mean free paths thick at the resonance peak, so its flux really is
/// flat), and raising the impact-parameter quadrature from 16 to 32 Gauss nodes
/// reproduces `L` to all six printed decimals.
///
/// # Results (2026-09-12, ENDF/B-VIII.0 @ 293.6 K, sigma_b = 300 b, vfrac 0.30,
/// `MODE=thick HIST=50000` so each row carries 200 000 histories, seed
/// 0xABCD_0003, FUEL_SUB=12, CP_NODES=16)
///
/// Exact homogeneous solution `p_esc = 0.38763`.
///
/// ```text
///   R/mfp    MC p_esc    1 sigma     ORACLE    MC - ORACLE      z
///    2.57     0.39811    0.00109    0.40013      -0.50 %     -1.84
///    8.56     0.43320    0.00111    0.43180      +0.32 %     +1.26
///   25.69     0.50082    0.00112    0.50059      +0.05 %     +0.20
/// ```
///
/// **The Monte Carlo reproduces the deterministic reference at every optical
/// thickness, worst 1.8 sigma**, over a range where the lumping effect itself
/// grows from +2.7 % to +29.2 % of the homogeneous answer. There is no trend
/// with thickness and no consistent sign.
///
/// As the lumping effect `L = p_esc(lump) - p_esc(homogeneous)` — the quantity
/// that carries the reactivity, and a small difference of two larger numbers,
/// so the demanding comparison:
///
/// ```text
///   R/mfp    L (MC)     L (oracle)   MC/oracle - 1        pcm of +3327
///    2.57    0.01048     0.01250    -16.14 % +/- 8.76     -537 +/- 291
///    8.56    0.04557     0.04417     +3.15 % +/- 2.51     +105 +/-  83
///   25.69    0.11319     0.11296     +0.20 % +/- 0.99       +7 +/-  33
/// ```
///
/// The 2.57 mfp row is the weak one, and for a statistical reason rather than a
/// physical one: `L` is only 2.7 % of `p_esc` there, so the Monte Carlo's own
/// 0.28 % on `p_esc` becomes 8.8 % on `L`. It is 1.8 sigma from zero. The rows
/// that bracket the **ring-RPT annulus's own 6.65 mean free paths** are the
/// 8.56 mfp row (+105 +/- 83 pcm) and [`ring_rpt_annulus`] itself
/// (-32 +/- 24 pcm), and they agree with each other.
///
/// **Read against the +4004 pcm the ring-RPT pebble sits above its OpenMC
/// reference, that is an exclusion.** Reproducing the residual through this
/// mechanism would need the lumping effect to be wrong by more than 100 %; it
/// is right to a few per cent, and the sign of what is left is *more*
/// absorption in the lump, not less. Spatial self-shielding in an optically
/// thick lump joins the list of things the residual is not.
fn vv_gate_thick_lump(scan: &[ScanRow], det: f64, fuel_sub: usize, cp_nodes: usize) {
    use outram_mc_libs::vv::assert_absolute;

    println!(
        "\n=== V&V gate: the THICK lump against a deterministic collision-probability oracle ==="
    );
    assert!(
        fuel_sub >= 4,
        "the deterministic reference was run with {fuel_sub} sub-shell(s) in the fuel. \
         Below 4 the flat-flux discretisation is worth more than the Monte Carlo's \
         counting statistics (1 -> 2 sub-shells moves the lumping effect 0.70 %), so \
         the comparison below would be measuring the reference's own mesh."
    );
    assert!(
        cp_nodes >= 16,
        "the collision-probability quadrature was run with {cp_nodes} Gauss nodes; \
         8 nodes is 1.1e-4 off the closed-form sphere escape probability and 16 is \
         6.6e-6 (tests/lump_collision_probability.rs)"
    );

    let thick: Vec<&ScanRow> = scan.iter().filter(|r| r.r_over_mfp >= 2.0).collect();
    assert!(
        !thick.is_empty(),
        "no row of the scan reaches 2 mean free paths across the lump, so the thick-lump \
         regime -- the only one left unchecked -- is not being tested at all"
    );

    println!(
        "{:>12}  {:>9}  {:>11}  {:>10}  {:>11}  {:>10}  {:>8}",
        "R_cell [cm]", "R/mfp", "MC p_esc", "1 sigma", "ORACLE", "MC - ORACLE", "z"
    );
    let mut worst_z = 0.0_f64;
    for r in &thick {
        let z = (r.white - r.oracle) / r.white_stderr;
        worst_z = worst_z.max(z.abs());
        println!(
            "{:>12.1e}  {:>9.2}  {:>11.5}  {:>10.5}  {:>11.5}  {:>+10.2} %  {z:>8.2}",
            r.r_cell,
            r.r_over_mfp,
            r.white,
            r.white_stderr,
            r.oracle,
            100.0 * (r.white / r.oracle - 1.0),
        );
    }

    // 1. Every thick row agrees with the reference to counting statistics.
    //    The slack is 4 sigma of that row's own statistics plus 0.2 % of the
    //    answer for the reference's own discretisation -- ten times the measured
    //    0.02 % residual at FUEL_SUB = 12, and still far below the noise.
    for r in &thick {
        // 4 sigma of this row's own counting statistics, plus 0.2 % of the
        // answer for the reference's own flat-flux discretisation -- an order of
        // magnitude above the measured residual at FUEL_SUB = 12, and still far
        // below the noise. Drawing the slack from the run rather than from a
        // fixed percentage is what keeps the verdict independent of HIST.
        //
        // If this fires, the SIGN is the finding: MC above the reference is too
        // MUCH self-shielding (the lump under-absorbs, which is the direction
        // the ring-RPT residual needs), MC below is too little.
        assert_absolute(
            &format!(
                "MC vs the deterministic collision-probability solution at \
                 {:.2} mean free paths (R_cell = {:.1e} cm)",
                r.r_over_mfp, r.r_cell
            ),
            r.white,
            r.oracle,
            4.0 * r.white_stderr + 0.002 * r.oracle,
        );
    }

    // 2. The LUMPING EFFECT itself -- the difference from the homogeneous
    //    answer -- is what carries the reactivity, and it is a small difference
    //    of two larger numbers, so it is the demanding comparison. Only rows
    //    where the Monte Carlo resolves it are judged.
    println!(
        "\n  lumping effect L = p_esc(lump) - p_esc(homogeneous = {det:.5}), \
         and the reactivity it implies"
    );
    println!(
        "{:>12}  {:>9}  {:>11}  {:>11}  {:>12}  {:>14}",
        "R_cell [cm]", "R/mfp", "L (MC)", "L (oracle)", "MC/oracle-1", "pcm on +3327"
    );
    let mut judged = 0usize;
    let mut worst_bound_pcm = 0.0_f64;
    for r in &thick {
        let l_mc = r.white - det;
        let l_or = r.oracle - det;
        if l_or <= 0.0 || l_mc <= 4.0 * r.white_stderr {
            continue;
        }
        judged += 1;
        let frac = l_mc / l_or - 1.0;
        let frac_sigma = r.white_stderr / l_or;
        // What the lump concentration is worth on the FHR ring-RPT pebble, on
        // this code's own measurement (naive 1.37187 +/- 0.00232 vs explicit
        // 1.40514 +/- 0.00204). A fractional error in the lumping effect is
        // that fraction of it.
        const RING_RPT_LUMPING_PCM: f64 = 3327.0;
        let bound = (frac.abs() + 2.0 * frac_sigma) * RING_RPT_LUMPING_PCM;
        worst_bound_pcm = worst_bound_pcm.max(bound);
        println!(
            "{:>12.1e}  {:>9.2}  {:>11.5}  {:>11.5}  {:>+9.2} % +/- {:.2}   \
             {:>+8.0} +/- {:.0}",
            r.r_cell,
            r.r_over_mfp,
            l_mc,
            l_or,
            100.0 * frac,
            100.0 * frac_sigma,
            frac * RING_RPT_LUMPING_PCM,
            frac_sigma * RING_RPT_LUMPING_PCM,
        );
    }
    assert!(
        judged > 0,
        "no row resolved the lumping effect above 4 sigma of its own counting \
         statistics, so the comparison that matters was not made. Raise HIST or add \
         a thicker row."
    );
    println!(
        "\n  worst |z| against the deterministic reference over {} thick rows: {worst_z:.2}",
        thick.len()
    );
    println!(
        "  worst 2-sigma bound on the ring-RPT lumping reactivity: +/- {worst_bound_pcm:.0} pcm"
    );
    println!(
        "  [PASS] the thick-lump resonance treatment reproduces an independent\n  \
         deterministic solution of the same equation on the same geometry."
    );
}

fn load(name: &str, file: &str) -> Nuclide {
    let p = reference_endf(file).unwrap_or_else(|| panic!("missing {file}"));
    Nuclide::from_endf_file(&p, name, TEMP, 1.0e-3).expect("reconstruct")
}
