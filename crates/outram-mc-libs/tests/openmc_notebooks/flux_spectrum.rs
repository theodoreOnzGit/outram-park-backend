//! `flux-spectrum` notebook -> outram-mc verification (LIVE, op-6tz.9).
//!
//! Notebook: `flux-spectrum.ipynb`
//! (openmc-notebooks@`cf1e5db2cd77d53a4fa76ffd9af7ab638f468713`, MIT).
//!
//! **What the notebook does.** Runs a PWR pin cell and tallies the neutron flux
//! into a fine energy-group structure (CASMO/SHEM), then plots the spectrum.
//! OpenMC API exercised: `examples.pwr_pin_cell`, `EnergyFilter`,
//! `mgxs.GROUP_STRUCTURES`, `Tally`, `StatePoint`.
//!
//! **What outram-mc does here (this run wires it, op-6tz.9).** The CSG
//! k-eigenvalue loop ([`outram_mc_libs::physics::transport_csg::run_keff_csg`])
//! now drives a **track-length flux estimator**
//! ([`outram_mc_libs::tally::scoring::score_track_length`]): every streamed
//! free-flight segment deposits `w·d` into the energy bin
//! ([`outram_mc_libs::tally::filter::EnergyFilter`]) matching the particle's
//! current energy, accumulated per generation and flushed as one Monte-Carlo
//! realization per active batch. So the notebook's **energy-binned flux
//! spectrum** is real here — this LIVE test scores it and asserts physical
//! spectrum-shape properties.
//!
//! The notebook's data reference is a *shape* (a slowing-down spectrum), not a
//! benchmark k, so no reference eigenvalue is asserted.
//!
//! # V&V — methodology and results
//!
//! **Methodology.** A heterogeneous fast-driven, hydrogen-moderated infinite
//! medium: an HEU fuel cylinder (Godiva atom densities, r = 0.4 cm) inside a
//! light-hydrogen moderator region (H-1 at 6.6e-2 atoms/b·cm) filling a 1.26 cm
//! square pitch with **reflective** x/y planes, infinite in z (⇒ zero leakage,
//! k_inf). Analog transport, LOW-tier embedded data (`Nuclide::from_core`: WMP +
//! Watt-collapsed fast MGXS); **free-gas** hydrogen scattering (no S(α,β) table,
//! so the spectrum slows down and thermalizes only approximately — it is *not*
//! data-gated on the public `tsl-HinH2O.endf` file, unlike the thermal pincell).
//! A single `Tally` with one [`EnergyFilter`] over a 50-bin log grid from
//! 1e-3 eV to 20 MeV scores the **track-length flux**; the eigenvalue run seeds
//! the fast fission source.
//!
//! **Pass criterion (physical spectrum shape, no reference k).**
//! - Every energy bin is finite and non-negative (a valid flux estimate).
//! - Total track-length flux is strictly positive; the per-bin fractions
//!   normalize to 1.
//! - The fast group (E > 0.1 MeV) carries a substantial fraction of the flux
//!   (the fission source is fast) — the **fast tail** of the spectrum.
//! - Substantial flux appears *below* the fission-source energy (E < 0.1 MeV):
//!   hydrogen elastic scattering slows neutrons down — the **slowing-down /
//!   thermal** side of the spectrum. Without this the moderator would be inert.
//! - The batch-to-batch relative standard deviation of the total flux is small
//!   (a converged tally).
//!
//! **Results (measured 2026-08-06, this harness; deterministic seed).** With 400
//! particles, 15 inactive + 30 active generations the run gives
//! **k_inf = 1.82955 ± 0.01050** (fast HEU + free-gas-H infinite medium, so a
//! high k_inf is expected), a **fast (E > 0.1 MeV) fraction = 0.673** and a
//! **below-0.1-MeV (slowing-down + thermal) fraction = 0.327**, total track-length
//! flux = 6.7141e3 (arbitrary track-length units; volume normalization is op-6tz.22),
//! all 50 bins finite/non-negative, and the peak energy bin (bin 45,
//! 1.866–2.999 MeV, the fission-source region) resolved to a **batch relative
//! std-dev of 0.023** — a
//! physically sane fast-peaked, hydrogen-moderated slowing-down spectrum. The split
//! is printed at run time and asserted against broad physical bands below (not a
//! benchmark gate — it guards the track-length energy-binned tally wiring, not
//! spectral accuracy).
//!
//! **Supersedes (pre-`op-jis` figures, measured 2026-07-17).** The numbers above
//! replace values taken with the old `prn` output function (uniforms formed from
//! the raw top-52 state bits, before bead `op-jis` added OpenMC's PCG-RXS-M-XS
//! output permutation). The LCG *state* recurrence is unchanged, but every
//! sampled uniform moved, and with it every sampled statistic. Superseded:
//! **k_inf = 1.82844 ± 0.00917**, fast fraction **0.674**, below-0.1-MeV fraction
//! **0.326**, total track-length flux **6.794e3**, peak-bin (bin 45) batch
//! relative std-dev **0.027**. The peak bin index (45) and its energy range are
//! properties of the fixed 50-bin log grid, not of the RNG, and did not move.

use outram_mc_libs::geometry::cell::{Cell, HalfSpaceSense, RegionToken};
use outram_mc_libs::geometry::geometry::Geometry;
use outram_mc_libs::geometry::position::Position;
use outram_mc_libs::geometry::surface::{BoundaryType, SurfaceKind, XPlane, YPlane, ZCylinder};
use outram_mc_libs::geometry::universe::Universe;
use outram_mc_libs::material::material::{Material, NuclideComponent};
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::physics::keff::KeffSettings;
use outram_mc_libs::physics::transport_csg::{run_keff_csg, SourceBox};
use outram_mc_libs::tally::filter::EnergyFilter;
use outram_mc_libs::tally::tally::{ScoreType, Tally, TallyBin};

/// Godiva atom densities (HEU-MET-FAST-001), atoms/barn-cm, as the fissile fuel.
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
fn spectrum_nuclides() -> Vec<Nuclide> {
    vec![
        Nuclide::from_core("U234").expect("U234 in CORE WMP library"),
        Nuclide::from_core("U235").expect("U235 in CORE WMP library"),
        Nuclide::from_core("U238").expect("U238 in CORE WMP library"),
        Nuclide::from_core("H1").expect("H1 in CORE WMP library"),
    ]
}

/// Square reflective pin cell: an HEU fuel cylinder (radius `r_fuel`) in an H-1
/// moderator region, `2·half`-wide, reflective x/y planes, infinite in z.
/// Cell 0 = fuel (material 0), cell 1 = moderator (material 1).
fn pincell_geometry(r_fuel: f64, half: f64) -> Geometry {
    let surfaces = vec![
        SurfaceKind::ZCylinder(ZCylinder { x0: 0.0, y0: 0.0, r: r_fuel, bc: BoundaryType::Transmissive }),
        SurfaceKind::XPlane(XPlane { x0: -half, bc: BoundaryType::Reflective }),
        SurfaceKind::XPlane(XPlane { x0: half, bc: BoundaryType::Reflective }),
        SurfaceKind::YPlane(YPlane { y0: -half, bc: BoundaryType::Reflective }),
        SurfaceKind::YPlane(YPlane { y0: half, bc: BoundaryType::Reflective }),
    ];
    let fuel = Cell::material(
        1,
        vec![RegionToken::HalfSpace { surface_idx: 0, sense: HalfSpaceSense::Inside }],
        0,
        293.6,
    );
    let moder = Cell::material(
        2,
        vec![
            RegionToken::HalfSpace { surface_idx: 0, sense: HalfSpaceSense::Outside },
            RegionToken::HalfSpace { surface_idx: 1, sense: HalfSpaceSense::Outside }, // x > -half
            RegionToken::Intersection,
            RegionToken::HalfSpace { surface_idx: 2, sense: HalfSpaceSense::Inside }, // x < +half
            RegionToken::Intersection,
            RegionToken::HalfSpace { surface_idx: 3, sense: HalfSpaceSense::Outside }, // y > -half
            RegionToken::Intersection,
            RegionToken::HalfSpace { surface_idx: 4, sense: HalfSpaceSense::Inside }, // y < +half
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

/// `n`-bin log-spaced energy grid from `e_lo` to `e_hi` \[eV\] (returns `n+1`
/// ascending edges).
fn log_energy_grid(e_lo: f64, e_hi: f64, n: usize) -> Vec<f64> {
    let (l0, l1) = (e_lo.ln(), e_hi.ln());
    (0..=n).map(|i| (l0 + (l1 - l0) * i as f64 / n as f64).exp()).collect()
}

/// LIVE (op-6tz.9): energy-binned **track-length flux spectrum** of a fast-driven,
/// hydrogen-moderated infinite pin cell. Asserts physical spectrum-shape
/// properties (fast tail + slowing-down side, finite/normalized, converged). See
/// the module V&V note for methodology and measured results.
#[test]
fn flux_energy_spectrum() {
    let nuclides = spectrum_nuclides();
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
    let geom = pincell_geometry(r_fuel, half);

    // Energy filter: 50 log bins, 1e-3 eV .. 20 MeV (spans thermal → fast).
    let n_bins = 50usize;
    let edges = log_energy_grid(1.0e-3, 2.0e7, n_bins);
    let filter = EnergyFilter { bins: edges.clone() };
    let mut tally = Tally {
        id: 1,
        name: "flux spectrum".into(),
        filters: vec![Box::new(filter)],
        scores: vec![ScoreType::Flux],
        bins: vec![TallyBin::default(); n_bins],
    };

    let settings = KeffSettings { n_particles: 400, n_inactive: 15, n_active: 30, ..KeffSettings::default() };
    let src = SourceBox {
        lower: Position::new(-r_fuel, -r_fuel, -1.0),
        upper: Position::new(r_fuel, r_fuel, 1.0),
    };

    let t0 = std::time::Instant::now();
    let result = run_keff_csg(&geom, &materials, &nuclides, src, &settings, Some(&mut tally));
    let dt = t0.elapsed();

    let n_active = settings.n_active as u64;

    // ── Validity: every bin finite and non-negative ────────────────────────────
    for (i, b) in tally.bins.iter().enumerate() {
        let m = b.mean(n_active);
        assert!(
            m.is_finite() && m >= 0.0,
            "energy bin {i} ({:.3e}..{:.3e} eV) flux mean {m} is not finite/non-negative",
            edges[i], edges[i + 1]
        );
    }

    // ── Total flux and per-bin fractions ───────────────────────────────────────
    let means: Vec<f64> = tally.bins.iter().map(|b| b.mean(n_active)).collect();
    let total: f64 = means.iter().sum();
    assert!(total > 0.0, "total track-length flux must be positive, got {total}");
    let frac: Vec<f64> = means.iter().map(|m| m / total).collect();
    let frac_sum: f64 = frac.iter().sum();
    assert!((frac_sum - 1.0).abs() < 1.0e-9, "normalized fractions must sum to 1, got {frac_sum}");

    // ── Coarse-group split: fast (>0.1 MeV) vs slowing-down/thermal (<0.1 MeV) ──
    const FAST_CUT: f64 = 1.0e5; // 0.1 MeV
    let fast: f64 = tally.bins.iter().enumerate()
        .filter(|(i, _)| edges[*i] >= FAST_CUT)
        .map(|(_, b)| b.mean(n_active))
        .sum();
    let slow: f64 = total - fast;
    let fast_frac = fast / total;
    let slow_frac = slow / total;

    // Convergence: the peak (largest-mean) energy bin is the best-sampled, so its
    // batch-to-batch relative standard deviation is a fair convergence proxy for
    // the tally (sparse high-energy bins near 20 MeV are noisy by construction and
    // not indicative). The eigenvalue's own std-error `k_std` cross-checks it.
    let peak_idx = means
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
        .map(|(i, _)| i)
        .unwrap();
    let peak_rel_sd = tally.bins[peak_idx].rel_std_dev(n_active);

    eprintln!(
        "[flux spectrum] k_inf = {:.5} ± {:.5}  |  fast(>0.1MeV) frac = {:.3}, \
         slowing-down/thermal(<0.1MeV) frac = {:.3}  |  total TL-flux = {:.4e}  |  \
         peak bin {} ({:.3e}..{:.3e} eV) rel-sd = {:.3}  (npart={}, {}+{} gen, {:.1?})",
        result.k_mean, result.k_std, fast_frac, slow_frac, total,
        peak_idx, edges[peak_idx], edges[peak_idx + 1], peak_rel_sd,
        settings.n_particles, settings.n_inactive, settings.n_active, dt
    );

    assert!(result.k_mean.is_finite() && result.k_mean > 0.0, "k_inf must be finite & positive, got {}", result.k_mean);

    // Fast tail present: the fission source is fast, so a substantial fraction of
    // the flux sits above 0.1 MeV.
    assert!(
        fast_frac > 0.10,
        "expected a substantial fast tail (>0.1 MeV), got fast fraction {fast_frac}"
    );
    // Slowing-down side present: hydrogen elastic scattering must moderate a
    // substantial fraction of the flux below the fission-source energy.
    assert!(
        slow_frac > 0.10,
        "expected substantial slowing-down/thermal flux (<0.1 MeV) from H moderation, \
         got below-cut fraction {slow_frac}"
    );
    // Converged tally: the eigenvalue is well-resolved and the best-sampled
    // (peak) energy bin has a small batch-to-batch relative std-dev.
    assert!(result.k_std < 0.03, "eigenvalue under-converged: k_std {}", result.k_std);
    assert!(
        peak_rel_sd.is_finite() && peak_rel_sd < 0.25,
        "flux spectrum tally under-converged: peak-bin rel std-dev {peak_rel_sd}"
    );
}
