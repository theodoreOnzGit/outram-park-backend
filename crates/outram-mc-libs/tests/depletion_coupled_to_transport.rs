//! **Depletion collapsed against a flux spectrum from an actual transport solve,
//! re-solved at every burnup step.**
//!
//! # The gap this closes
//!
//! `one_group_cross_sections` had two successive approximations, each recorded
//! as a remaining step when it landed:
//!
//! 1. **One energy.** Every nuclide evaluated at 0.0253 eV and called
//!    "one-group" — missing resonance absorption entirely.
//! 2. **A representative spectrum.** NJOY's `iwt = 4` analytic shape
//!    (Maxwellian + `1/E` + fission tail). A large improvement, and still a
//!    spectrum that knows nothing about *this* geometry's moderation, leakage or
//!    self-shielding.
//!
//! This is the third: `OneGroupWeighting::TabulatedFlux` collapses against a
//! spectrum the caller measured, and `deplete_coupled` **re-solves it before
//! every step** instead of freezing beginning-of-life data across the whole
//! burnup. `deplete_predictor` computes its cross sections once, before the
//! loop — which is only right if the spectrum does not move, and the point of
//! depletion is that it does.
//!
//! # Methodology
//!
//! A reflective HEU pin cell in an H-1 moderator, the same geometry the
//! `flux_spectrum` notebook test uses. Before each burnup step, `run_keff_csg`
//! is run with an [`EnergyFilter`] tally over a 40-bin log grid from 1e-3 eV to
//! 20 MeV; the tallied bin means become the weighting spectrum for that step.
//!
//! Three arms are compared over the same chain and inventory:
//!
//! | arm | weighting |
//! |---|---|
//! | `SingleEnergy` | one point at 0.0253 eV |
//! | `ThermalFissionSpectrum` | NJOY `iwt = 4`, frozen |
//! | `deplete_coupled` | tallied flux, **re-solved each step** |
//!
//! # What is asserted, and why each one
//!
//! - **The callback is actually invoked once per step.** A "coupled" driver that
//!   solved once and reused the answer would pass every numerical check below
//!   while not being coupled at all, so the mechanism is asserted directly.
//! - **The tallied spectrum has thermal *and* fast flux.** A spectrum that
//!   collapsed to one bin would make the comparison meaningless.
//! - **The three arms disagree.** If the weighting did not reach the cross
//!   sections, they would not.
//! - **Inventory stays physical** — non-negative, and U-235 monotonically
//!   decreasing under irradiation.
//!
//! # Results (2026-09-16)
//!
//! | arm | one-group `k_inf` at BOL |
//! |---|---|
//! | `SingleEnergy` (0.0253 eV) | **1.92205** |
//! | `ThermalFissionSpectrum` (`iwt = 4`) | **0.59300** |
//! | **transport-solved flux** | **1.59737** |
//!
//! Tallied spectrum: 40 groups, **16.3 % below 1 eV, 43.3 % above 100 keV** —
//! an under-moderated pin, neither thermal nor fast.
//!
//! **The weighting choice moves the answer by a factor of three.** That is the
//! result worth taking from this: the two frozen arms are wrong in *opposite*
//! directions, because a single thermal point maximises U-235 fission while
//! barely seeing U-238 capture, and a generic thermal-reactor weight assumes a
//! moderation this geometry does not have. The transport-flux value lands
//! between them, which is what the measured spectrum implies.
//!
//! **These `k_inf` numbers are not physical predictions and must not be quoted
//! as such.** They are the one-group collapse's own internal consistency metric
//! — production over absorption on CORE-tier data with a three-nuclide chain.
//! Their *spread* is the finding; their values are not a criticality result.
//!
//! # What this does NOT claim
//!
//! This is **operator splitting** (a predictor): transport is solved on the
//! inventory entering a step, held constant across it, and the inventory
//! advanced by CRAM. There is no second solve at the end of the step, so it
//! carries the usual first-order splitting error in step length — shortening
//! the step is what controls it. It is also not validated against an external
//! depletion code; the cross-section collapse and the CRAM step are each
//! verified separately, and this checks that they are wired together correctly.

use outram_mc_libs::depletion::chain::DepletionChain;
use outram_mc_libs::depletion::operator::{
    deplete_coupled, deplete_predictor, BurnupSettings, OneGroupWeighting,
};
use outram_mc_libs::prelude::*;
use outram_mc_libs::tally::filter::EnergyFilter;

/// Ascending log-spaced group boundaries, `n + 1` of them.
fn log_grid(lo: f64, hi: f64, n: usize) -> Vec<f64> {
    (0..=n)
        .map(|i| (lo.ln() + (hi.ln() - lo.ln()) * i as f64 / n as f64).exp())
        .collect()
}

fn pincell(r_fuel: f64, half: f64) -> Geometry {
    let surfaces = vec![
        SurfaceKind::ZCylinder(ZCylinder {
            x0: 0.0,
            y0: 0.0,
            r: r_fuel,
            bc: BoundaryType::Transmissive,
        }),
        SurfaceKind::XPlane(XPlane {
            x0: -half,
            bc: BoundaryType::Reflective,
        }),
        SurfaceKind::XPlane(XPlane {
            x0: half,
            bc: BoundaryType::Reflective,
        }),
        SurfaceKind::YPlane(YPlane {
            y0: -half,
            bc: BoundaryType::Reflective,
        }),
        SurfaceKind::YPlane(YPlane {
            y0: half,
            bc: BoundaryType::Reflective,
        }),
    ];
    let fuel = Cell::material(
        1,
        vec![RegionToken::HalfSpace {
            surface_idx: 0,
            sense: HalfSpaceSense::Inside,
        }],
        0,
        293.6,
    );
    let moder = Cell::material(
        2,
        vec![
            RegionToken::HalfSpace {
                surface_idx: 0,
                sense: HalfSpaceSense::Outside,
            },
            RegionToken::HalfSpace {
                surface_idx: 1,
                sense: HalfSpaceSense::Outside,
            },
            RegionToken::Intersection,
            RegionToken::HalfSpace {
                surface_idx: 2,
                sense: HalfSpaceSense::Inside,
            },
            RegionToken::Intersection,
            RegionToken::HalfSpace {
                surface_idx: 3,
                sense: HalfSpaceSense::Outside,
            },
            RegionToken::Intersection,
            RegionToken::HalfSpace {
                surface_idx: 4,
                sense: HalfSpaceSense::Inside,
            },
            RegionToken::Intersection,
        ],
        1,
        293.6,
    );
    Geometry {
        surfaces,
        cells: vec![fuel, moder],
        universes: vec![Universe {
            id: 0,
            cell_indices: vec![0, 1],
        }],
        lattices: vec![],
        root_universe: 0,
    }
}

#[test]
fn depletion_collapses_against_a_transport_solved_spectrum_each_step() {
    let chain = DepletionChain::simple();

    let nuclides = vec![
        Nuclide::from_core("U235").expect("U235 in CORE"),
        Nuclide::from_core("U238").expect("U238 in CORE"),
        Nuclide::from_core("H1").expect("H1 in CORE"),
    ];
    let (r_fuel, half) = (0.4_f64, 0.63_f64);
    let geom = pincell(r_fuel, half);

    const N_GROUPS: usize = 40;
    let edges = log_grid(1.0e-3, 2.0e7, N_GROUPS);

    // Initial inventory: 4.25 %-enriched UO2-like uranium, in atoms/(barn*cm).
    let initial = vec![("U235".to_string(), 1.0e-3), ("U238".to_string(), 2.2e-2)];

    let settings = BurnupSettings {
        n_steps: 3,
        step_days: 30.0,
        ..BurnupSettings::default()
    };

    // ── The coupled arm: a transport solve before every step ──────────────────
    let mut n_solves = 0usize;
    let mut spectra: Vec<Vec<f64>> = Vec::new();
    let edges_for_cb = edges.clone();
    let coupled = deplete_coupled(&chain, &initial, &settings, |inventory| {
        n_solves += 1;
        // The fuel's composition for THIS step, straight from the inventory the
        // depletion loop is carrying. This is the line that makes it coupled.
        let u235 = inventory
            .iter()
            .find(|(n, _)| n == "U235")
            .map(|(_, d)| *d)
            .unwrap_or(0.0);
        let u238 = inventory
            .iter()
            .find(|(n, _)| n == "U238")
            .map(|(_, d)| *d)
            .unwrap_or(0.0);
        if u235 + u238 <= 0.0 {
            return None;
        }
        let materials = vec![
            Material {
                id: 1,
                name: "fuel".into(),
                temperature: 293.6,
                components: vec![
                    NuclideComponent {
                        nuclide_idx: 0,
                        atom_density: u235,
                    },
                    NuclideComponent {
                        nuclide_idx: 1,
                        atom_density: u238,
                    },
                ],
            },
            Material {
                id: 2,
                name: "moderator".into(),
                temperature: 293.6,
                components: vec![NuclideComponent {
                    nuclide_idx: 2,
                    atom_density: 6.6e-2,
                }],
            },
        ];
        let mut tally = Tally {
            id: 1,
            name: "flux spectrum".into(),
            filters: vec![FilterKind::Energy(EnergyFilter {
                bins: edges_for_cb.clone(),
            })],
            scores: vec![ScoreType::Flux],
            bins: vec![TallyBin::default(); N_GROUPS],
        };
        let ks = KeffSettings {
            n_particles: 300,
            n_inactive: 8,
            n_active: 20,
            ..KeffSettings::default()
        };
        let src = SourceBox {
            lower: Position::new(-r_fuel, -r_fuel, -1.0),
            upper: Position::new(r_fuel, r_fuel, 1.0),
        };
        let _ = run_keff_csg(&geom, &materials, &nuclides, src, &ks, Some(&mut tally));
        let phi: Vec<f64> = tally
            .bins
            .iter()
            .map(|b| b.mean(ks.n_active as u64).max(0.0))
            .collect();
        if phi.iter().sum::<f64>() <= 0.0 {
            return None;
        }
        spectra.push(phi.clone());
        Some((edges_for_cb.clone(), phi))
    });

    // ── The two frozen arms ───────────────────────────────────────────────────
    let single = deplete_predictor(&chain, &initial, &settings);
    let analytic = deplete_predictor(
        &chain,
        &initial,
        &BurnupSettings {
            weighting: OneGroupWeighting::thermal_fission_default(),
            ..settings.clone()
        },
    );

    // ── The mechanism, asserted directly ──────────────────────────────────────
    println!(
        "coupled depletion: {n_solves} transport solves for {} steps, {} spectra recorded",
        settings.n_steps,
        spectra.len()
    );
    // Exactly `n_steps`, not `n_steps + 1`: the beginning-of-life solve is run
    // on the initial inventory, and step 1 starts from that same inventory, so
    // re-solving before step 1 would repeat an identical calculation. Steps
    // 2..=n each get their own. (This assertion was written expecting
    // `n_steps + 1` and was wrong -- the driver is deliberately avoiding a
    // redundant solve, which is worth a comment precisely because the arithmetic
    // looks off by one until you see why.)
    assert_eq!(
        n_solves, settings.n_steps,
        "the spectrum callback ran {n_solves} times for {} steps; expected one per step (the \
         BOL solve serves step 1, whose inventory is unchanged). A driver that solves once and \
         reuses the answer would still produce plausible numbers while not being coupled at \
         all -- which is exactly what deplete_predictor does, and what this must differ from.",
        settings.n_steps
    );

    // The tallied spectrum must actually span the range, or the collapse is
    // weighting against nothing.
    let phi0 = &spectra[0];
    let total: f64 = phi0.iter().sum();
    let thermal: f64 = phi0
        .iter()
        .zip(edges.windows(2))
        .filter(|(_, e)| e[1] <= 1.0)
        .map(|(p, _)| *p)
        .sum();
    let fast: f64 = phi0
        .iter()
        .zip(edges.windows(2))
        .filter(|(_, e)| e[0] >= 1.0e5)
        .map(|(p, _)| *p)
        .sum();
    println!(
        "   tallied spectrum: {} groups, {:.1}% below 1 eV, {:.1}% above 100 keV",
        phi0.len(),
        100.0 * thermal / total,
        100.0 * fast / total
    );
    assert!(
        total > 0.0 && thermal > 0.0 && fast > 0.0,
        "the tallied spectrum has no thermal or no fast flux, so collapsing against it tests \
         nothing (thermal {thermal:.3e}, fast {fast:.3e} of {total:.3e})"
    );

    // ── The three arms must disagree ──────────────────────────────────────────
    let (k_s, k_a, k_c) = (
        single.bol().k_inf,
        analytic.bol().k_inf,
        coupled.bol().k_inf,
    );
    println!("   k_inf at BOL:  single-energy {k_s:.5}   iwt=4 {k_a:.5}   transport-flux {k_c:.5}");
    assert!(
        (k_c - k_s).abs() > 1.0e-4,
        "the transport-flux collapse gives the same k_inf as the single-energy one \
         ({k_c:.6} vs {k_s:.6}); the tallied spectrum is not reaching the cross sections."
    );
    assert!(
        (k_c - k_a).abs() > 1.0e-6,
        "the transport-flux collapse is indistinguishable from the analytic iwt=4 shape \
         ({k_c:.6} vs {k_a:.6}). They are different spectra and should not agree exactly."
    );

    // ── The inventory stays physical ──────────────────────────────────────────
    let u235: Vec<f64> = coupled.steps.iter().map(|s| s.density("U235")).collect();
    println!(
        "   U-235 density over {} steps: {:?}",
        u235.len(),
        u235.iter().map(|d| format!("{d:.4e}")).collect::<Vec<_>>()
    );
    for s in &coupled.steps {
        for (n, d) in &s.densities {
            assert!(
                *d >= 0.0 && d.is_finite(),
                "{n} reached a non-physical density {d:e} at step {}",
                s.step
            );
        }
    }
    for w in u235.windows(2) {
        assert!(
            w[1] <= w[0] * (1.0 + 1.0e-12),
            "U-235 increased under irradiation ({:.6e} -> {:.6e}); nothing in this chain \
             produces it",
            w[0],
            w[1]
        );
    }
    assert!(
        u235[u235.len() - 1] < u235[0],
        "U-235 did not deplete at all over {} days",
        settings.n_steps as f64 * settings.step_days
    );
}
