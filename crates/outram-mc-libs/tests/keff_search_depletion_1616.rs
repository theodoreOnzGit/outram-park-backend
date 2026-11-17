// SPDX-License-Identifier: GPL-3.0

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

use outram_mc_libs::depletion::chain::DepletionChain;
use outram_mc_libs::depletion::integrators::{deplete_independent, Integrator, MicroXs};
use outram_mc_libs::depletion::keff_search::{
    deplete_independent_with_keff_search, KeffSearchControl,
};
use outram_mc_libs::depletion::operator::BurnupSettings;
use outram_mc_libs::physics::search::{SearchMethod, SearchSettings};
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

/// The pin cell for one burnable inventory and one boron concentration.
fn pin_cell_k(ppm: f64, inventory: &[(String, f64)]) -> KeffResult {
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
        n_active: 25,
        ..KeffSettings::default()
    };
    let src = SourceBox {
        lower: Position::new(-R_FUEL, -R_FUEL, -1.0),
        upper: Position::new(R_FUEL, R_FUEL, 1.0),
    };
    run_keff_csg(&pincell(R_FUEL, HALF), &materials, &nuclides, src, &ks, None)
}

#[test]
fn critical_boron_falls_monotonically_with_burnup() {
    let chain = DepletionChain::simple();
    // 4.25 wt% UO2, 10.4 g/cm3: N(UO2) = 0.02320 /(b cm).
    let initial = vec![("U235".to_string(), 9.8e-4), ("U238".to_string(), 2.222e-2)];
    let settings = BurnupSettings { n_steps: 4, step_days: 50.0, ..BurnupSettings::default() };
    let micro = MicroXs::from_chain(&chain, &settings);
    let control = KeffSearchControl::new(0.0, 6000.0, (0.0, 6000.0))
        .unwrap()
        .with_search_settings(SearchSettings {
            target: 1.0,
            tol: TOL_PPM,
            k_tol: K_TOL,
            max_iterations: 12,
            method: SearchMethod::Secant,
        });
    let t0 = std::time::Instant::now();
    let run = deplete_independent_with_keff_search(
        &chain,
        &initial,
        &settings,
        &micro,
        Integrator::Predictor,
        &control,
        |p, inv| pin_cell_k(p, inv),
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
}
