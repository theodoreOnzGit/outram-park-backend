//! `expansion-filters` notebook -> outram-mc verification (LIVE, op-6tz.14).
//!
//! Notebook: `expansion-filters.ipynb`
//! (openmc-notebooks@`cf1e5db2cd77d53a4fa76ffd9af7ab638f468713`, MIT).
//!
//! **What the notebook does.** Tallies functional-expansion moments of the flux —
//! a Legendre expansion of the axial (z) flux via `SpatialLegendreFilter`, and a
//! Zernike (r-θ) expansion via `ZernikeFilter` / `ZernikeRadialFilter` — then
//! reconstructs the continuous flux shape from the stored moments.
//! OpenMC API exercised: `SpatialLegendreFilter`, `ZernikeFilter`,
//! `ZernikeRadialFilter`, `openmc.Legendre`, `legendre_from_expcoef`.
//!
//! **What outram-mc does here (this run wires it, op-6tz.14).** The CSG
//! k-eigenvalue loop ([`outram_mc_libs::physics::transport_csg::run_keff_csg`])
//! drives the **track-length flux estimator**
//! ([`outram_mc_libs::tally::scoring::score_track_length`]); a lone
//! [`outram_mc_libs::tally::filter::SpatialLegendreFilter`] on the **z axis**
//! deposits `w·d·P_n(ξ)` into moment bin `n` for every streamed segment
//! (`ξ = 2·(z − min)/(max − min) − 1`), the faithful analogue of OpenMC's
//! multi-`(bin, weight)` `get_all_bins`
//! (`src/tallies/filter_sptl_legendre.cpp:63`). So the notebook's **axial
//! Legendre flux moments** are real here.
//!
//! **Scope / gap.** Only the z-axis Legendre expansion is LIVE. The Zernike
//! (r-θ) expansion filters are **not** ported (bead op-6tz.14); the notebook's
//! *exact* moment magnitudes also need full continuous-energy data, so as in
//! `flux_spectrum.rs` this is a **shape** check, not a benchmark of the moment
//! values.
//!
//! # V&V — methodology and results
//!
//! **Methodology.** A fully-reflective HEU box (Godiva atom densities) — an HEU
//! fuel cylinder (r = 0.4 cm) in an H-1 moderator filling a 1.26 cm square pitch,
//! **reflective x/y/z planes** bounding a finite axial span z ∈ [−10, 10] cm.
//! Reflective boundaries on all six faces make the flux an *infinite-medium*
//! field — spatially uniform, in particular **uniform in z**. Analog transport,
//! LOW-tier embedded data (`Nuclide::from_core`: WMP + Watt-collapsed fast MGXS),
//! free-gas hydrogen. A single [`SpatialLegendreFilter`] of order 4 on the z axis
//! over `[−10, 10]` cm scores the track-length flux into moments `P_0 … P_4`.
//!
//! **Pass criterion (physical moment shape, analytic identity).** For a flux
//! `φ(z)` that is uniform over the full expansion interval, the raw Legendre
//! moments `m_n = ∫ φ P_n(ξ) dξ` reduce by orthogonality (`∫ P_n = 0` for
//! `n ≥ 1`) to: `m_0 = φ · L` (strongly positive, ∝ total flux) and
//! `m_n ≈ 0` for `n ≥ 1`, up to Monte-Carlo noise. So the check is:
//! - every moment finite,
//! - `m_0 > 0` and dominant (`|m_n| ≪ m_0` for `n ≥ 1`),
//! - the higher moments consistent with zero (`|m_n| / m_0` small).
//!
//! **Results (measured 2026-07-17, this harness; deterministic seed).** With 500
//! particles, 20 inactive + 35 active generations the run gives
//! **k_inf = 1.83164 ± 0.00763** (fast HEU + free-gas-H infinite medium; a high
//! k_inf is expected, and consistent with `flux_spectrum.rs`'s ~1.828). The axial
//! (z) Legendre moments (± batch σ, track-length units) are:
//!
//! - `m_0 = +3.2133e5 ± 3.17e4`   (P_0 — mean axial flux · interval length)
//! - `m_1 = +6.97e1  ± 4.95e1`    (P_1 — |m_1|/m_0 = 2.2e-4, statistically zero)
//! - `m_2 = -2.24e3  ± 1.21e3`    (P_2 — |m_2|/m_0 = 7.0e-3)
//! - `m_3 = +2.86e1  ± 1.65e1`    (P_3 — |m_3|/m_0 = 8.9e-5, statistically zero)
//! - `m_4 = -3.30e3  ± 1.14e3`    (P_4 — |m_4|/m_0 = 1.0e-2)
//!
//! So `m_0` dominates by ~10^2–10^4: the **odd** moments (m_1, m_3) are ~10^-4 of
//! m_0 — statistically indistinguishable from zero (< 2σ), the expected exact-zero
//! by up/down symmetry of a flat field. The **even** moments (m_2, m_4) sit at the
//! ~1% level and ~2–3σ from zero — a small residual axial non-uniformity from the
//! finite generation count (the source is seeded uniformly in z but the
//! fission-site distribution equilibrates over only 35 batches); they are still
//! ≪ m_0, so the flat-flux signature holds. Interpretation: the z-axis Legendre
//! expansion filter and the moment-deposition scoring path are wired correctly
//! and recover the analytic flat-flux shape (m_0 dominant, higher moments small).
//! Absolute magnitudes are track-length units (volume normalization is op-6tz.22),
//! so this guards the expansion-filter wiring, not moment accuracy. The measured
//! moments are also written to
//! `verification_and_validation/openmc_notebook_comparisons/expansion_filters.csv`.

use outram_mc_libs::geometry::cell::{Cell, HalfSpaceSense, RegionToken};
use outram_mc_libs::geometry::geometry::Geometry;
use outram_mc_libs::geometry::position::Position;
use outram_mc_libs::geometry::surface::{BoundaryType, SurfaceKind, XPlane, YPlane, ZCylinder, ZPlane};
use outram_mc_libs::geometry::universe::Universe;
use outram_mc_libs::material::material::{Material, NuclideComponent};
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::physics::keff::KeffSettings;
use outram_mc_libs::physics::transport_csg::{run_keff_csg, SourceBox};
use outram_mc_libs::tally::filter::{LegendreAxis, SpatialLegendreFilter};
use outram_mc_libs::tally::tally::{ScoreType, Tally, TallyBin};
use std::io::Write;

/// Godiva HEU atom densities (HEU-MET-FAST-001), atoms/barn-cm.
fn heu_fuel() -> Material {
    Material {
        id: 1,
        name: "Godiva HEU".into(),
        temperature: 293.6,
        components: vec![
            NuclideComponent { nuclide_idx: 0, atom_density: 4.9184e-4 }, // U-234
            NuclideComponent { nuclide_idx: 1, atom_density: 4.4994e-2 }, // U-235
            NuclideComponent { nuclide_idx: 2, atom_density: 2.4984e-3 }, // U-238
        ],
    }
}

/// Nuclide array: `0 U234, 1 U235, 2 U238, 3 H1` (free-gas H, no S(α,β)).
fn nuclides() -> Vec<Nuclide> {
    vec![
        Nuclide::from_core("U234").expect("U234 in CORE WMP library"),
        Nuclide::from_core("U235").expect("U235 in CORE WMP library"),
        Nuclide::from_core("U238").expect("U238 in CORE WMP library"),
        Nuclide::from_core("H1").expect("H1 in CORE WMP library"),
    ]
}

/// Half-space tokens that intersect the reflective x/y/z box (surfaces 1..=6).
/// Prepending the cylinder half-space + one `Intersection` per added token builds
/// the fuel / moderator regions.
fn box_walls() -> Vec<RegionToken> {
    vec![
        RegionToken::HalfSpace { surface_idx: 1, sense: HalfSpaceSense::Outside }, // x > -half
        RegionToken::Intersection,
        RegionToken::HalfSpace { surface_idx: 2, sense: HalfSpaceSense::Inside }, // x < +half
        RegionToken::Intersection,
        RegionToken::HalfSpace { surface_idx: 3, sense: HalfSpaceSense::Outside }, // y > -half
        RegionToken::Intersection,
        RegionToken::HalfSpace { surface_idx: 4, sense: HalfSpaceSense::Inside }, // y < +half
        RegionToken::Intersection,
        RegionToken::HalfSpace { surface_idx: 5, sense: HalfSpaceSense::Outside }, // z > -zmax
        RegionToken::Intersection,
        RegionToken::HalfSpace { surface_idx: 6, sense: HalfSpaceSense::Inside }, // z < +zmax
        RegionToken::Intersection,
    ]
}

/// Fully-reflective HEU box: fuel cylinder (radius `r_fuel`) in an H-1 moderator,
/// `2·half` wide in x/y with reflective planes and reflective z planes at `±zmax`
/// (a finite axial span so the Legendre expansion has a bounded interval). All six
/// faces reflective ⇒ an infinite-medium, spatially uniform flux. Both cells carry
/// the z-plane half-spaces so the particle reflects off them.
/// Cell 0 = fuel (material 0), cell 1 = moderator (material 1).
fn reflective_box(r_fuel: f64, half: f64, zmax: f64) -> Geometry {
    let surfaces = vec![
        SurfaceKind::ZCylinder(ZCylinder { x0: 0.0, y0: 0.0, r: r_fuel, bc: BoundaryType::Transmissive }),
        SurfaceKind::XPlane(XPlane { x0: -half, bc: BoundaryType::Reflective }),
        SurfaceKind::XPlane(XPlane { x0: half, bc: BoundaryType::Reflective }),
        SurfaceKind::YPlane(YPlane { y0: -half, bc: BoundaryType::Reflective }),
        SurfaceKind::YPlane(YPlane { y0: half, bc: BoundaryType::Reflective }),
        SurfaceKind::ZPlane(ZPlane { z0: -zmax, bc: BoundaryType::Reflective }),
        SurfaceKind::ZPlane(ZPlane { z0: zmax, bc: BoundaryType::Reflective }),
    ];

    // Fuel: inside the cylinder, capped in z by the reflective z planes.
    let mut fuel_region = vec![RegionToken::HalfSpace { surface_idx: 0, sense: HalfSpaceSense::Inside }];
    fuel_region.push(RegionToken::HalfSpace { surface_idx: 5, sense: HalfSpaceSense::Outside });
    fuel_region.push(RegionToken::Intersection);
    fuel_region.push(RegionToken::HalfSpace { surface_idx: 6, sense: HalfSpaceSense::Inside });
    fuel_region.push(RegionToken::Intersection);
    let fuel = Cell::material(1, fuel_region, 0, 293.6);

    // Moderator: outside the cylinder AND inside the reflective x/y/z box.
    let mut moder_region = vec![RegionToken::HalfSpace { surface_idx: 0, sense: HalfSpaceSense::Outside }];
    moder_region.extend(box_walls());
    let moder = Cell::material(2, moder_region, 1, 293.6);

    Geometry {
        surfaces,
        cells: vec![fuel, moder],
        universes: vec![Universe { id: 0, cell_indices: vec![0, 1] }],
        lattices: vec![],
        root_universe: 0,
    }
}

/// Write the notebook-comparison CSV (gitignored dir).
fn write_csv(rows: &[String]) {
    let dir = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/verification_and_validation/openmc_notebook_comparisons"
    );
    let _ = std::fs::create_dir_all(dir);
    let path = format!("{dir}/expansion_filters.csv");
    if let Ok(mut f) = std::fs::File::create(&path) {
        let _ = writeln!(f, "date,notebook,case,our_value±σ,reference,delta,note");
        for r in rows {
            let _ = writeln!(f, "{r}");
        }
    }
}

/// LIVE (op-6tz.14): axial (z) **Legendre flux moments** of a fully-reflective
/// (infinite-medium) HEU box. Asserts the analytic flat-flux moment signature —
/// `m_0` dominant and positive, higher moments consistent with zero. See the
/// module V&V note for methodology and measured results.
#[test]
fn expansion_moment_filters() {
    let nuclides = nuclides();
    let fuel = heu_fuel();
    let moderator = Material {
        id: 2,
        name: "H moderator".into(),
        temperature: 293.6,
        components: vec![NuclideComponent { nuclide_idx: 3, atom_density: 6.6e-2 }],
    };
    let materials = vec![fuel, moderator];

    let half = 0.63;
    let r_fuel = 0.4;
    let zmax = 10.0;
    let geom = reflective_box(r_fuel, half, zmax);

    // Legendre expansion of the axial flux: order 4, z axis, over [-zmax, zmax].
    let order = 4usize;
    let filter = SpatialLegendreFilter { order, axis: LegendreAxis::Z, min: -zmax, max: zmax };
    let n_moments = order + 1;
    let mut tally = Tally {
        id: 1,
        name: "axial Legendre flux".into(),
        filters: vec![Box::new(filter)],
        scores: vec![ScoreType::Flux],
        bins: vec![TallyBin::default(); n_moments],
    };

    let settings = KeffSettings { n_particles: 500, n_inactive: 20, n_active: 35, ..KeffSettings::default() };
    let src = SourceBox {
        lower: Position::new(-r_fuel, -r_fuel, -zmax),
        upper: Position::new(r_fuel, r_fuel, zmax),
    };

    let t0 = std::time::Instant::now();
    let result = run_keff_csg(&geom, &materials, &nuclides, src, &settings, Some(&mut tally));
    let dt = t0.elapsed();

    let n_active = settings.n_active as u64;
    let moments: Vec<f64> = tally.bins.iter().map(|b| b.mean(n_active)).collect();
    let sigmas: Vec<f64> = tally
        .bins
        .iter()
        .map(|b| {
            // Absolute batch std-dev of the moment = rel_std_dev · |mean|.
            let rsd = b.rel_std_dev(n_active);
            if rsd.is_finite() { rsd * b.mean(n_active).abs() } else { f64::NAN }
        })
        .collect();

    // ── Every moment finite ────────────────────────────────────────────────────
    for (n, m) in moments.iter().enumerate() {
        assert!(m.is_finite(), "Legendre moment m_{n} = {m} not finite");
    }

    let m0 = moments[0];
    eprintln!(
        "[expansion-filters] k_inf = {:.5} ± {:.5}  |  axial Legendre moments (± batch σ):",
        result.k_mean, result.k_std
    );
    for n in 0..n_moments {
        eprintln!(
            "    m_{n} = {:+.4e} ± {:.2e}   (|m_{n}|/m_0 = {:.2e})",
            moments[n], sigmas[n], (moments[n] / m0).abs()
        );
    }
    eprintln!(
        "    (npart={}, {}+{} gen, {:.1?})",
        settings.n_particles, settings.n_inactive, settings.n_active, dt
    );

    // ── m_0 dominant and strictly positive (∝ total flux) ──────────────────────
    assert!(m0 > 0.0, "P_0 moment (mean axial flux) must be positive, got {m0}");
    assert!(result.k_mean.is_finite() && result.k_mean > 0.0, "k_inf must be finite & positive");
    assert!(result.k_std < 0.03, "eigenvalue under-converged: k_std {}", result.k_std);

    // ── Higher moments consistent with zero for a flat axial flux ──────────────
    // Each |m_n| (n ≥ 1) must be ≪ m_0 (flat-flux orthogonality). A generous 5%
    // bound tolerates Monte-Carlo noise while still failing loudly if the axis
    // normalization or moment deposition is wrong (which would put O(1)·m_0 into a
    // higher moment).
    for n in 1..n_moments {
        let rel = (moments[n] / m0).abs();
        assert!(
            rel < 0.05,
            "axial flux is ~uniform ⇒ moment m_{n} should be ≪ m_0, but |m_{n}|/m_0 = {rel:.3}"
        );
    }

    // CSV: one row per moment (reference = analytic flat-flux value).
    let rows: Vec<String> = (0..n_moments)
        .map(|n| {
            let reference = if n == 0 { "m_0 > 0 (prop. to total flux)".to_string() } else { "0 (flat flux)".to_string() };
            let delta = if n == 0 { "n/a".to_string() } else { format!("{:.2e}", (moments[n] / m0).abs()) };
            format!(
                "2026-07-17,expansion-filters,P_{n} (z Legendre),{:+.4e}+/-{:.2e},{},{},{}",
                moments[n], sigmas[n], reference, delta,
                "shape check (flat-flux moments); abs magnitude = track-length units op-6tz.22"
            )
        })
        .collect();
    write_csv(&rows);
}
