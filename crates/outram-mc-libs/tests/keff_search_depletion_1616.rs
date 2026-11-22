// SPDX-License-Identifier: AGPL-3.0-only

//! **Critical boron versus burnup — the k search coupled to depletion**
//! (GitHub #1616; epic #257). Hook: `depletion::keff_search`, after OpenMC's
//! `openmc/deplete/keff_search_control.py` at `8f79d9d`.
//!
//! # Methodology
//!
//! A reflective PWR-like pin cell (fuel radius 0.4096 cm, half-pitch 0.63 cm,
//! no clad, water at 1.0 g/cm3, 293.6 K), run on `run_keff_csg` with the
//! embedded CORE data, 2500 histories x (8 inactive + 25 active) per solve,
//! fixed seed (so every solve of one search shares random numbers, which makes
//! `k(p)` smooth in `p`). The fuel is 4.25 wt% UO2 at 10.4 g/cm3. The burnable
//! inventory is the 9-nuclide `DepletionChain::simple()`; nuclides the chain
//! lacks (O-16, H-1, B-10) are held by the model. Depletion is the
//! **independent** (frozen one-group cross section) operator at 0.0253 eV,
//! Predictor, CRAM16, 174 W in 0.5542 cm3, 4 steps x 50 d, so the burnup
//! history is the `deplete_independent` one. The search parameter is the
//! natural-boron concentration of the water in ppm by mass, converted to B-10
//! atoms/(b cm) with the natural abundance 19.9 % (B-11 is not carried: it is
//! not in CORE, and its thermal absorption is ~4 orders below B-10). The
//! search is `search_for_keff` (false position) on `[0, 6000]` ppm, initial
//! guesses `(0, 6000)`, target 1, residual tolerance `k_tol = 0.005`, parameter
//! tolerance 25 ppm. A search runs before each of the 4 steps and once on the
//! final inventory.
//!
//! # Predictions, written BEFORE the first run (2026-11-17)
//!
//! 1. Every search converges inside `[0, 4000]` ppm (needs `k(0 ppm) > 1 >
//!    k(6000 ppm)` at BOL). This can fail: a CORE-tier, H-1-free-gas pin cell
//!    with no S(a,b) need not be supercritical unborated. If it fails the run
//!    stops with `KeffSearchDepletionError::Bracket`, and that is the finding.
//! 2. At every step the search's k is within `k_tol` of 1 or the bracket has
//!    shrunk below the 25 ppm tolerance.
//! 3. The boron trajectory is **non-increasing** with burnup (fissile burns,
//!    Xe-135 and fission products build in), within the MC noise of one
//!    search. Boron does not change the one-group depletion, so the history is
//!    bit-identical to the plain one (asserted below), and the parameter
//!    moves because the *transport* k at fixed boron falls.
//! 4. Magnitude: the BOL-to-final drop is several hundred ppm (Xe-135 at
//!    ~2.5 % k plus ~3 % k from 80 d at ~2.7 GWd/tU, over a boron worth of
//!    order 10 pcm/ppm gives 300 to 900 ppm), with the largest single step
//!    being step 0 to 1, where Xe-135 first appears.
//!
//! # Results
//!
//! See `verification_and_validation/keff_search_depletion_1616/README.md`.
//!
//! # 2026-11-17 addendum (GitHub #1618): GRsecant is now the default search
//!
//! The text above describes the original (bracketed false-position) run and is
//! kept as `critical_boron_falls_monotonically_with_burnup_bracketed`, selected
//! by name. `critical_boron_falls_monotonically_with_burnup` now runs the
//! hook's **default**, GRsecant (`model.py:3248-3405`), on the same case.
//!
//! ## Predictions for the GRsecant run, written BEFORE it was run
//!
//! Settings fixed in advance, not tuned: seeds `(0, 6000)` ppm (the old
//! bracket's ends), clamp `[0, 6000]`, `k_tol = 0.005` (as before),
//! `sigma_final = 0.004` (the size of the old runs' `sigma_k`, 0.0033 to
//! 0.0043), `b0 = 25` active (the old count), `b_min = 20` (upstream's), `b_max
//! = 100` (to protect a shared 4-core box), 8 inactive.
//!
//! 1. Every search converges (`|k - 1| <= 0.005`, `sigma <= 0.004`) in at most
//!    8 evaluations: the seeds' chord overshoots on a convex `k(p)`, then the
//!    secant closes in a few steps.
//! 2. The boron trajectory agrees with the recorded bracketed one (4014.8,
//!    3642.2, 3454.0, 3270.0, 3029.2 ppm) at every search to within 250 ppm
//!    (about 2.5 combined sigma: each run is about 65 to 75 ppm at the local
//!    5.4e-5 per ppm), and mostly within 100 ppm.
//! 3. Boron non-increasing with burnup within noise; total drop 986 +- 150 ppm.
//! 4. Total transport solves 20 to 35 (the bracketed run spent 32), each a bit
//!    longer than the old 25 active where the adaptive count rises.
//!
//! ## Outcome (2026-11-17, one run, fixed seed; README has the table)
//!
//! 1. Converged and `sigma <= 0.004` at every search: **met**. "At most 8
//!    evaluations": **missed**, 7, 9, 8, 9, 8.
//! 2. Agreement with the bracketed trajectory: **met**, the differences are
//!    -24, -20, -30, -26, -10 ppm (all inside 100 ppm, all the same sign).
//! 3. Monotone, total drop 971 ppm against 986 +- 150: **met**.
//! 4. Total solves 41 against 20 to 35: **missed**, and more than the bracketed
//!    run's 32. The 4-point fit window keeps the two far seeds (0 and 6000 ppm)
//!    for the first iterations, and `k(p)` is convex there, so early proposals
//!    are biased.

use outram_mc_libs::depletion::chain::DepletionChain;
use outram_mc_libs::depletion::integrators::{deplete_independent, Integrator, MicroXs};
use outram_mc_libs::depletion::keff_search::{
    deplete_independent_with_keff_search, KeffSearchControl, KeffSearchMethod, KeffSearchStep,
};
use outram_mc_libs::depletion::operator::BurnupSettings;
use outram_mc_libs::physics::search::{SearchMethod, SearchSettings};
use outram_mc_libs::physics::search_grsecant::GrSecantSettings;
use outram_mc_libs::prelude::*;

const R_FUEL: f64 = 0.4096;
const HALF: f64 = 0.63;
/// B-10 atoms/(b cm) per ppm (by mass) of natural boron in water at
/// 1.0 g/cm3: 1e-6 * rho * N_A / M_B * abundance * 1e-24.
const B10_PER_PPM: f64 = 1.0e-6 * 6.022_140_76e23 / 10.811 * 0.199 * 1.0e-24;
const K_TOL: f64 = 0.005;
const TOL_PPM: f64 = 25.0;

fn half_space(surface_idx: usize, sense: HalfSpaceSense) -> RegionToken {
    RegionToken::HalfSpace { surface_idx, sense }
}

/// Reflective square cell with a fuel cylinder (cell 0) in water (cell 1).
fn pincell(r_fuel: f64, half: f64) -> Geometry {
    let surfaces = vec![
        SurfaceKind::ZCylinder(ZCylinder {
            x0: 0.0,
            y0: 0.0,
            r: r_fuel,
            bc: BoundaryType::Transmissive,
        }),
        SurfaceKind::XPlane(XPlane { x0: -half, bc: BoundaryType::Reflective }),
        SurfaceKind::XPlane(XPlane { x0: half, bc: BoundaryType::Reflective }),
        SurfaceKind::YPlane(YPlane { y0: -half, bc: BoundaryType::Reflective }),
        SurfaceKind::YPlane(YPlane { y0: half, bc: BoundaryType::Reflective }),
    ];
    let fuel = Cell::material(1, vec![half_space(0, HalfSpaceSense::Inside)], 0, 293.6);
    let moder = Cell::material(
        2,
        vec![
            half_space(0, HalfSpaceSense::Outside),
            half_space(1, HalfSpaceSense::Outside),
            RegionToken::Intersection,
            half_space(2, HalfSpaceSense::Inside),
            RegionToken::Intersection,
            half_space(3, HalfSpaceSense::Outside),
            RegionToken::Intersection,
            half_space(4, HalfSpaceSense::Inside),
            RegionToken::Intersection,
        ],
        1,
        293.6,
    );
    Geometry {
        surfaces,
        cells: vec![fuel, moder],
        universes: vec![Universe { id: 0, cell_indices: vec![0, 1] }],
        lattices: vec![],
        root_universe: 0,
    }
}

/// The pin cell for one burnable inventory, one boron concentration and a
/// number of active generations.
fn pin_cell_k(ppm: f64, inventory: &[(String, f64)], n_active: usize) -> KeffResult {
    let mut nuclides: Vec<Nuclide> = Vec::new();
    let mut fuel: Vec<NuclideComponent> = Vec::new();
    let mut n_heavy = 0.0;
    for (name, dens) in inventory {
        if *dens <= 0.0 {
            continue;
        }
        if let Ok(nuc) = Nuclide::from_core(name) {
            if name.starts_with('U') {
                n_heavy += dens;
            }
            fuel.push(NuclideComponent { nuclide_idx: nuclides.len(), atom_density: *dens });
            nuclides.push(nuc);
        }
    }
    fuel.push(NuclideComponent { nuclide_idx: nuclides.len(), atom_density: 2.0 * n_heavy });
    let i_o = nuclides.len();
    nuclides.push(Nuclide::from_core("O16").expect("O16 in CORE"));
    let i_h = nuclides.len();
    nuclides.push(Nuclide::from_core("H1").expect("H1 in CORE"));
    let i_b = nuclides.len();
    nuclides.push(Nuclide::from_core("B10").expect("B10 in CORE"));
    let materials = vec![
        Material { id: 1, name: "fuel".into(), temperature: 293.6, components: fuel },
        Material {
            id: 2,
            name: "water".into(),
            temperature: 293.6,
            components: vec![
                NuclideComponent { nuclide_idx: i_h, atom_density: 6.69e-2 },
                NuclideComponent { nuclide_idx: i_o, atom_density: 3.345e-2 },
                NuclideComponent { nuclide_idx: i_b, atom_density: ppm * B10_PER_PPM },
            ],
        },
    ];
    let ks = KeffSettings {
        n_particles: 2500,
        n_inactive: 8,
        n_active,
        ..KeffSettings::default()
    };
    let src = SourceBox {
        lower: Position::new(-R_FUEL, -R_FUEL, -1.0),
        upper: Position::new(R_FUEL, R_FUEL, 1.0),
    };
    run_keff_csg(&pincell(R_FUEL, HALF), &materials, &nuclides, src, &ks, None)
}

fn run_and_gate(control: KeffSearchControl) -> Vec<KeffSearchStep> {
    let chain = DepletionChain::simple();
    // 4.25 wt% UO2, 10.4 g/cm3: N(UO2) = 0.02320 /(b cm).
    let initial = vec![("U235".to_string(), 9.8e-4), ("U238".to_string(), 2.222e-2)];
    let settings = BurnupSettings { n_steps: 4, step_days: 50.0, ..BurnupSettings::default() };
    let micro = MicroXs::from_chain(&chain, &settings);
    let t0 = std::time::Instant::now();
    let run = deplete_independent_with_keff_search(
        &chain,
        &initial,
        &settings,
        &micro,
        Integrator::Predictor,
        &control,
        |p, inv, b| pin_cell_k(p, inv, b),
        |_, _| {},
    )
    .expect("k search coupled to depletion");
    println!("wall time {:.1} s", t0.elapsed().as_secs_f64());

    println!("step  t[d]  boron[ppm]  k  sigma  evals");
    for s in &run.searches {
        println!(
            "{:>4} {:>5.0} {:>10.1} {:.5} {:.5} {:>3} trials {:?}",
            s.step,
            s.time_days,
            s.parameter,
            s.keff,
            s.keff_std,
            s.n_evaluations,
            s.iterations
                .iter()
                .map(|i| (i.parameter.round(), (i.keff * 1e5).round() / 1e5))
                .collect::<Vec<_>>()
        );
    }

    // The burnup is the plain independent one, bit for bit: boron changes no
    // burnable nuclide.
    let plain =
        deplete_independent(&chain, &initial, &settings, &micro, Integrator::Predictor).unwrap();
    for (a, b) in plain.steps.iter().zip(&run.burnup.steps) {
        assert_eq!(a.k_inf.to_bits(), b.k_inf.to_bits());
        assert_eq!(a.density("U235").to_bits(), b.density("U235").to_bits());
    }

    // Gate 1: a search before each step and one at the end.
    assert_eq!(run.searches.len(), settings.n_steps + 1);
    // Gate 2: k within the search tolerance of the target at every step. The
    // search stops on `|k - 1| <= K_TOL`, or on the 25 ppm bracket, where k is
    // still within K_TOL plus a few sigma.
    for s in &run.searches {
        assert!(
            (s.keff - 1.0).abs() <= K_TOL + 3.0 * s.keff_std,
            "step {}: k = {} +- {}",
            s.step,
            s.keff,
            s.keff_std
        );
        assert!(!s.used_absolute_bracket);
    }
    // Gate 3: boron non-increasing with burnup, to within the noise of the two
    // searches compared. The ppm noise of one search is sigma_k / |dk/dp|, with
    // dk/dp read from that search's own two bracket endpoints: a chord over the
    // whole bracket. k(p) is convex here, so the chord OVERstates the local
    // slope near the root (measured: chord 1.0e-4 per ppm, local 5.4e-5), which
    // understates the ppm noise by about 2x. The gate is therefore tighter on
    // the noise than the truth; the measured drop still clears it (README).
    let noise_ppm = |s: &outram_mc_libs::depletion::keff_search::KeffSearchStep| {
        let (a, b) = (s.iterations[0], s.iterations[1]);
        let slope = ((b.keff - a.keff) / (b.parameter - a.parameter)).abs();
        s.keff_std / slope
    };
    for w in run.searches.windows(2) {
        let allowed =
            3.0 * (noise_ppm(&w[0]).powi(2) + noise_ppm(&w[1]).powi(2)).sqrt() + 2.0 * TOL_PPM;
        assert!(
            w[1].parameter <= w[0].parameter + allowed,
            "boron rose {} -> {} (allowed {allowed})",
            w[0].parameter,
            w[1].parameter
        );
    }
    // The overall drop exceeds 3 sigma of the two end searches.
    let first = &run.searches[0];
    let last = run.searches.last().unwrap();
    let drop_noise = (noise_ppm(first).powi(2) + noise_ppm(last).powi(2)).sqrt();
    assert!(
        first.parameter - last.parameter > 3.0 * drop_noise,
        "boron {} -> {}, noise {drop_noise}",
        first.parameter,
        last.parameter
    );
    run.searches
}

/// The recorded bracketed trajectory (README, 2026-11-17): boron at each search.
const BRACKETED_PPM: [f64; 5] = [4014.8, 3642.2, 3454.0, 3270.0, 3029.2];
/// Local worth near the root, per ppm (README): 5.4e-5.
const LOCAL_SLOPE: f64 = 5.4e-5;

/// The original #1616 run, the bracketed false-position search selected by
/// name (GRsecant has been the default since #1618).
#[test]
fn critical_boron_falls_monotonically_with_burnup_bracketed() {
    // The bracketed search hands the model `GrSecantSettings::b0` active
    // generations (it has no batch adaptation); the original run used 25, so
    // set it, then select the bracketed search by name. (Left at the default
    // 50 the run is a different, twice-as-long design: measured 2026-11-17,
    // 4004.6, 3692.1, 3456.5, 3283.1, 3051.6 ppm.)
    let control = KeffSearchControl::new(0.0, 6000.0, (0.0, 6000.0))
        .unwrap()
        .with_grsecant_settings(GrSecantSettings { b0: 25, ..GrSecantSettings::default() })
        .with_search_settings(SearchSettings {
            target: 1.0,
            tol: TOL_PPM,
            k_tol: K_TOL,
            max_iterations: 12,
            method: SearchMethod::Secant,
        });
    assert_eq!(control.method(), KeffSearchMethod::Bracketed);
    run_and_gate(control);
}

/// The same case under the hook's default search, GRsecant (#1618).
#[test]
fn critical_boron_falls_monotonically_with_burnup() {
    let control = KeffSearchControl::new(0.0, 6000.0, (0.0, 6000.0))
        .unwrap()
        .with_grsecant_settings(GrSecantSettings {
            k_tol: K_TOL,
            sigma_final: 0.004,
            b0: 25,
            inactive: 8,
            b_min: 20,
            b_max: Some(100),
            maxiter: 12,
            ..GrSecantSettings::default()
        });
    assert_eq!(control.method(), KeffSearchMethod::GrSecant);
    let searches = run_and_gate(control);
    // Prediction 1 (converged, at the stated sigma): `Ok` means the search met
    // `k_tol`; the sigma is checked here. The "at most 8 evaluations" part of
    // the prediction was MISSED (measured 7, 9, 8, 9, 8) and is recorded in the
    // README, not gated: the count is a measurement, not a correctness bound.
    for s in &searches {
        assert!(s.keff_std <= 0.004, "step {}: sigma {}", s.step, s.keff_std);
        assert!(!s.root_outside_bracket);
    }
    // Prediction 2: agreement with the recorded bracketed trajectory, within
    // 3 combined sigma at the local worth.
    for (s, old) in searches.iter().zip(BRACKETED_PPM) {
        let sigma_new = s.keff_std / LOCAL_SLOPE;
        let sigma_old = 0.0035 / LOCAL_SLOPE;
        let allowed = 3.0 * (sigma_new.powi(2) + sigma_old.powi(2)).sqrt();
        println!(
            "step {}: GRsecant {:.1} ppm vs bracketed {old} ppm, diff {:.1}, allowed {allowed:.0}",
            s.step,
            s.parameter,
            s.parameter - old
        );
        assert!((s.parameter - old).abs() <= allowed, "step {}", s.step);
    }
}

// ---------------------------------------------------------------------------
// GitHub #1618 item 4: a parameter that writes a BURNABLE nuclide, with transport
// ---------------------------------------------------------------------------
//
// Methodology. The same unborated pin cell, one burnup step (20 d, the
// independent operator), one GRsecant search before it, none after. The
// searched parameter is the U-235 atom fraction of the heavy metal, `p`
// (enrichment in atom percent / 100); `adjust` sets U-235 to `p * N_heavy` and
// U-238 to `(1 - p) * N_heavy` with `N_heavy = 9.8e-4 + 2.222e-2 = 2.32e-2`
// atoms/(b cm) held fixed. Both are chain nuclides, so `write_back` carries the
// found enrichment into the depletion vector, which a boron search never does.
// Transport per solve: 2500 histories x (8 inactive + `n_active`), fixed seed.
// GRsecant: seeds (0.015, 0.030), clamp [0.007, 0.06], `k_tol` 0.01,
// `sigma_final` 0.008, `b0` 15, `b_min` 10, `b_max` 40, at most 10 solves.
//
// Predictions, written BEFORE the first run (2026-11-17):
//
// 1. The search converges inside the clamp.
// 2. The critical enrichment lies between natural uranium (0.0072, where light
//    water without heavy-water moderation cannot be critical) and the case's own
//    4.23 %, where the boron search found k(0 ppm) = 1.49 (README). Those two
//    are physics bounds and are the gate.
// 3. My estimate of where in that range, NOT gated: 0.010 to 0.022, central
//    0.015, from a rough scaling of k_inf with enrichment in a tight-pitch
//    cold pin cell. CORE-tier H-1 has no S(a,b), which lowers k and would push
//    the root up.
// 4. The BOL inventory is the written-back one, `U235 = p * N_heavy` to the
//    bit, and differs from the initial 9.8e-4; U-235 then falls over the step.
//
// Results: README, section "2026-11-17, #1618 item 4".
#[test]
fn a_critical_enrichment_search_writes_a_burnable_nuclide_with_transport() {
    const N_HEAVY: f64 = 9.8e-4 + 2.222e-2;
    let chain = DepletionChain::simple();
    let initial = vec![("U235".to_string(), 9.8e-4), ("U238".to_string(), 2.222e-2)];
    let settings = BurnupSettings { n_steps: 1, step_days: 20.0, ..BurnupSettings::default() };
    let micro = MicroXs::from_chain(&chain, &settings);
    let control = KeffSearchControl::new(0.015, 0.030, (0.007, 0.06))
        .unwrap()
        .with_search_at_end(false)
        .with_grsecant_settings(GrSecantSettings {
            k_tol: 0.01,
            sigma_final: 0.008,
            b0: 15,
            inactive: 8,
            b_min: 10,
            b_max: Some(40),
            maxiter: 10,
            ..GrSecantSettings::default()
        });
    let t0 = std::time::Instant::now();
    let run = deplete_independent_with_keff_search(
        &chain,
        &initial,
        &settings,
        &micro,
        Integrator::Predictor,
        &control,
        // The model reads the inventory the search adjusted; no boron.
        |_p, inv, n_active| pin_cell_k(0.0, inv, n_active),
        |p, inv| {
            for e in inv.iter_mut() {
                if e.0 == "U235" {
                    e.1 = p * N_HEAVY;
                } else if e.0 == "U238" {
                    e.1 = (1.0 - p) * N_HEAVY;
                }
            }
        },
    )
    .expect("enrichment search");
    println!("wall time {:.1} s", t0.elapsed().as_secs_f64());
    assert_eq!(run.searches.len(), 1);
    let s = &run.searches[0];
    println!(
        "root U-235 fraction {:.5} (k {:.5} +- {:.5}, {} solves) trials {:?} batches {:?}",
        s.parameter,
        s.keff,
        s.keff_std,
        s.n_evaluations,
        s.iterations
            .iter()
            .map(|i| ((i.parameter * 1e5).round() / 1e5, (i.keff * 1e5).round() / 1e5))
            .collect::<Vec<_>>(),
        s.batches
    );
    // Prediction 1.
    assert!(s.parameter >= 0.007 && s.parameter <= 0.06);
    assert!((s.keff - 1.0).abs() <= 0.01 && s.keff_std <= 0.008);
    // Prediction 2: the physics bounds.
    assert!(s.parameter > 0.0072, "below natural uranium: {}", s.parameter);
    assert!(s.parameter < 9.8e-4 / N_HEAVY, "above the 4.23 % case: {}", s.parameter);
    // Prediction 4: the written-back inventory, to the bit, and burnup after it.
    let bol = &run.burnup.steps[0];
    assert_eq!(bol.density("U235").to_bits(), (s.parameter * N_HEAVY).to_bits());
    assert_ne!(bol.density("U235").to_bits(), 9.8e-4_f64.to_bits());
    assert!(run.burnup.steps[1].density("U235") < bol.density("U235"));
}
