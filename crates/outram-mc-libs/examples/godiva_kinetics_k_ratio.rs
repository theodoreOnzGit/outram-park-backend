// SPDX-License-Identifier: GPL-3.0

//! **β_eff and Λ for Godiva by the k-ratio route** — GitHub #262 scope item 3.
//!
//! # Methodology
//!
//! Bare HEU sphere at Godiva's critical radius (8.7407 cm), U-235 + U-238 from
//! `reference-data/endf/` at 293.6 K. Two **paired** eigenvalue runs over the
//! same seeds, differing only in the nuclides' yield:
//!
//! - `k` — the ordinary run.
//! - `k_p` — every nuclide through [`Nuclide::with_prompt_only_nubar`], so
//!   production is `ν̄ − ν̄_d(E)` with ν̄_d read from ENDF MF=1/455.
//!
//! Then `β_eff ≈ 1 − k_p/k`. Λ comes from a whole-geometry tally of
//! [`ScoreType::InverseVelocity`] and [`ScoreType::NuFission`] on the `k` arm:
//! `Λ = ∫(φ/v) / ∫(νΣ_f φ)`.
//!
//! Pairing matters: β_eff on a fast metal system is a few hundred pcm and
//! seed-to-seed scatter on this model is ~180 pcm, so an unpaired pair of runs
//! measures noise. The per-seed differences are what is averaged.
//!
//! # What this is NOT
//!
//! The **adjoint-weighted** β_eff. The k-ratio definition is the "prompt-`k`"
//! one and is biased; the IFP route (`src/ifp.cpp`) is the defensible one and
//! is not ported. See `physics::kinetics` for the full statement. A β_eff
//! quoted from this without that qualifier would be misused.
//!
//! It is also a **two-nuclide** Godiva: U-234 (4.9184e-4 /b·cm in the ICSBEP
//! specification) is absent, as it is in every two-nuclide case in this crate.
//!
//! # Results
//!
//! `verification_and_validation/kinetics/godiva_k_ratio_2026_09_22.md`.
//!
//! ```text
//! cargo run --release -p outram-mc-libs --example godiva_kinetics_k_ratio -- 24
//! ```

use outram_mc_libs::geometry::cell::{Cell, HalfSpaceSense, RegionToken};
use outram_mc_libs::geometry::geometry::Geometry;
use outram_mc_libs::geometry::position::Position;
use outram_mc_libs::geometry::surface::{BoundaryType, Sphere, SurfaceKind};
use outram_mc_libs::geometry::universe::Universe;
use outram_mc_libs::material::material::{Material, NuclideComponent};
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::physics::keff::{ComputeType, KeffSettings};
use outram_mc_libs::physics::kinetics;
use outram_mc_libs::physics::transport_csg::{run_keff_csg, SourceBox};
use outram_mc_libs::tally::filter::{CellFilter, FilterKind};
use outram_mc_libs::tally::tally::{ScoreType, Tally, TallyBin};

const R_FUEL: f64 = 8.7407;
const TEMP: f64 = 293.6;

fn heu() -> Option<Vec<Nuclide>> {
    let base =
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../reference-data/endf");
    let load = |name: &str, f: &str| -> Option<Nuclide> {
        let p = base.join(f);
        p.exists().then_some(())?;
        Nuclide::from_endf_file(&p, name, TEMP, 1.0e-3).ok()
    };
    Some(vec![
        load("U235", "n-092_U_235-ENDF8.0.endf")?,
        load("U238", "n-092_U_238.endf")?,
    ])
}

fn model() -> (Geometry, Vec<Material>) {
    let geom = Geometry {
        surfaces: vec![SurfaceKind::Sphere(Sphere {
            x0: 0.0,
            y0: 0.0,
            z0: 0.0,
            r: R_FUEL,
            bc: BoundaryType::Vacuum,
        })],
        cells: vec![Cell::material(
            1,
            vec![RegionToken::HalfSpace {
                surface_idx: 0,
                sense: HalfSpaceSense::Inside,
            }],
            0,
            TEMP,
        )],
        universes: vec![Universe {
            id: 0,
            cell_indices: vec![0],
        }],
        lattices: vec![],
        root_universe: 0,
    };
    let mats = vec![Material {
        id: 1,
        name: "HEU".into(),
        components: vec![
            NuclideComponent {
                nuclide_idx: 0,
                atom_density: 4.4994e-2,
            },
            NuclideComponent {
                nuclide_idx: 1,
                atom_density: 2.4984e-3,
            },
        ],
        temperature: TEMP,
    }];
    (geom, mats)
}

fn settings(seed: u64) -> KeffSettings {
    KeffSettings {
        n_particles: 4000,
        n_inactive: 20,
        n_active: 60,
        temperature_k: TEMP,
        seed,
        compute: ComputeType::CpuSingleThread,
        ..KeffSettings::default()
    }
}

fn lambda_tally() -> Tally {
    Tally {
        id: 7,
        name: "kinetics".into(),
        filters: vec![FilterKind::Cell(CellFilter {
            cell_indices: vec![0],
        })],
        scores: vec![ScoreType::InverseVelocity, ScoreType::NuFission],
        bins: vec![TallyBin::default(); 2],
    }
}

fn mean_sem(v: &[f64]) -> (f64, f64) {
    let n = v.len() as f64;
    let mean = v.iter().sum::<f64>() / n;
    if v.len() < 2 {
        return (mean, f64::NAN);
    }
    let var = v.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (n - 1.0);
    (mean, (var / n).sqrt())
}

fn main() {
    let n_seeds: u64 = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(16);

    let Some(nucs) = heu() else {
        eprintln!("SKIP: ENDF tapes not in this checkout");
        return;
    };
    let (geom, mats) = model();
    let src = SourceBox {
        lower: Position::new(-R_FUEL, -R_FUEL, -R_FUEL),
        upper: Position::new(R_FUEL, R_FUEL, R_FUEL),
    };

    // The check that has to come FIRST: a zero beta on this path means the
    // evaluations carry no MT=455, not that the system has no delayed
    // neutrons.
    let complete = kinetics::delayed_data_is_complete(&mats, &nucs);
    println!("GitHub #262 — Godiva beta_eff and Lambda by the k-ratio route");
    println!("  delayed data complete for every fissionable nuclide: {complete}");
    for (i, n) in nucs.iter().enumerate() {
        match n.delayed() {
            Some(d) => println!(
                "  nuclide {i}: {} groups, lambda {:?} 1/s, evaluated group split: {}",
                d.n_groups(),
                d.lambda,
                d.has_evaluated_group_split()
            ),
            None => println!("  nuclide {i}: NO MF=1/455"),
        }
    }
    if !complete {
        eprintln!(
            "REFUSING: at least one fissionable nuclide has no delayed data, so any \
             beta from this model is a lower bound rather than a measurement."
        );
        return;
    }
    println!(
        "  nu_bar(1 MeV): U235 total {:.5} prompt {:.5} (beta {:.5}), \
         U238 total {:.5} prompt {:.5} (beta {:.5})",
        nucs[0].nu_bar(1.0e6),
        nucs[0].nu_prompt(1.0e6),
        nucs[0].delayed_fraction(1.0e6),
        nucs[1].nu_bar(1.0e6),
        nucs[1].nu_prompt(1.0e6),
        nucs[1].delayed_fraction(1.0e6),
    );

    let prompt_nucs = kinetics::prompt_only(&nucs);
    let mut k_tot = Vec::new();
    let mut k_pro = Vec::new();
    let mut betas = Vec::new();
    let mut lambdas = Vec::new();

    for s in 0..n_seeds {
        let seed = 20_260_922 + s * 7919;
        let st = settings(seed);
        let mut t = lambda_tally();
        let a = run_keff_csg(&geom, &mats, &nucs, src, &st, Some(&mut t));
        let b = run_keff_csg(&geom, &mats, &prompt_nucs, src, &st, None);

        let n_real = st.n_active as u64;
        let inv_v = t.bins[0].mean(n_real);
        let nu_f = t.bins[1].mean(n_real);
        match kinetics::generation_time(inv_v, nu_f) {
            Ok(l) => lambdas.push(l),
            Err(e) => eprintln!("  seed {seed}: Lambda unavailable ({e})"),
        }

        k_tot.push(a.k_mean);
        k_pro.push(b.k_mean);
        betas.push(1.0 - b.k_mean / a.k_mean);
        println!(
            "  seed {seed}: k {:.5}  k_p {:.5}  beta {:.1} pcm",
            a.k_mean,
            b.k_mean,
            1.0e5 * (1.0 - b.k_mean / a.k_mean)
        );
    }

    let (mk, sk) = mean_sem(&k_tot);
    let (mp, sp) = mean_sem(&k_pro);
    let (mb, sb) = mean_sem(&betas);
    let (ml, sl) = mean_sem(&lambdas);

    println!("\n=== RESULTS ({n_seeds} paired seeds) ===");
    println!("  k       = {mk:.5} +/- {sk:.5}");
    println!("  k_p     = {mp:.5} +/- {sp:.5}");
    println!(
        "  beta_eff (k-ratio, PAIRED) = {:.1} +/- {:.1} pcm",
        1.0e5 * mb,
        1.0e5 * sb
    );
    println!("  Lambda  = {:.3e} +/- {:.1e} s  ({:.2} ns)", ml, sl, ml * 1.0e9);

    // The unpaired arithmetic, for comparison — this is what quoting the two
    // ensemble means separately would give, and the point of showing it is
    // that it is a worse measurement of the same quantity.
    match kinetics::from_k_ratio(mk, sk, mp, sp, ml, 1.0) {
        Ok(p) => println!(
            "  beta_eff (k-ratio, UNPAIRED means) = {:.1} +/- {:.1} pcm",
            1.0e5 * p.beta_eff,
            1.0e5 * p.beta_eff_sigma
        ),
        Err(e) => println!("  unpaired ratio refused: {e}"),
    }
}
