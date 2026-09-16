//! **Tally Godiva's flux spectrum, to localise the ~69 pcm spectral residual.**
//!
//! # Why a spectrum and not another eigenvalue
//!
//! The Godiva discrepancy against OpenMC splits into ~181 pcm of leakage —
//! attributed to the inelastic angular distributions this crate does not sample
//! — and **~69 ± 23 pcm that survives with the boundary removed**, i.e. is
//! spectral. Cross sections (≤0.06 % flux-weighted), ν̄ (0.0002 %), elastic
//! `⟨μ⟩` (≤5.6e-4) and the fission spectrum (≤0.018 %) are all cleared against
//! OpenMC, so the residual is in how secondary energies are sampled after a
//! collision.
//!
//! **An eigenvalue cannot localise that. A spectrum can.** `k` is one number
//! integrated over everything; a flux-vs-energy tally shows *where* in energy
//! the two codes part company, which points at the reaction responsible.
//!
//! # Methodology
//!
//! ICSBEP HEU-MET-FAST-001 as CSG — one vacuum-bounded sphere, `r = 8.7407 cm`,
//! the three ICSBEP nuclides at their densities — run through
//! [`run_keff_csg`] with a 50-bin log-energy track-length flux tally over
//! `1e-3 … 2e7 eV`. That is the same grid and the same
//! [`EnergyFilter`]/[`ScoreType::Flux`] construction
//! `tests/openmc_notebooks/flux_spectrum.rs` and the TUI's spectrum overlay
//! already use, so the tally path is one that is exercised elsewhere rather
//! than written for this study.
//!
//! The OpenMC counterpart is `godiva.py --spectrum`, which tallies flux on the
//! **same 50 edges** and writes `openmc_spectrum.csv`. `compare_spectrum.py`
//! differences them.
//!
//! Both spectra are **normalised to unit integral** before comparison: neither
//! side carries a volume or power normalisation, and only the *shape* is
//! meaningful. A shape difference concentrated in one energy decade is the
//! signature being looked for.
//!
//! # Results (measured 2026-09-15, 4 seeds per side)
//!
//! Each run writes one CSV; the seed and the path are taken from `OURS_SEED`
//! and `OURS_SPECTRUM`. Four seeds per side, differenced with
//! `compare_spectrum.py`, using the **seed-to-seed** spread as the uncertainty
//! (not the per-run tally sigma — the question is whether the two codes
//! disagree, and re-randomisation is the dominant term at these statistics).
//!
//! **Our spectrum is harder than OpenMC's**, by a small but well-resolved
//! amount:
//!
//! | quantity | ours | OpenMC | difference | sigma |
//! |---|---|---|---|---|
//! | mean `E` \[eV\] | 1.47466e6 | 1.46804e6 | **+0.45 %** | 3.5 |
//! | mean `ln E` | 13.6841 | 13.6774 | **+0.05 %** | 4.9 |
//! | flux fraction below 300 keV | 0.154809 | 0.156861 | **−1.31 %** | 5.2 |
//! | flux fraction above 4.8 MeV | 0.040170 | 0.039690 | **+1.21 %** | 2.8 |
//!
//! Per bin, the difference is a **monotone deficit below ~300 keV** (−1.1 % to
//! −4.5 %, growing as energy falls) and a **monotone excess above ~700 keV**
//! (+0.3 % to +2.8 %, growing as energy rises), crossing zero near 300–450 keV.
//! The whole shape difference integrates to 0.26 % of the spectrum sitting in
//! different bins.
//!
//! **No single bin proves it.** The one bin past 3 sigma — 7.744e6 … 1.245e7 eV
//! at +2.84 %, 6.1 sigma — carries only 0.48 % of the flux, and the bins that
//! carry 7–16 % agree to ±0.45 %. It is the *aggregates* above that resolve the
//! difference, which is why `compare_spectrum.py` reports them: the per-bin
//! sigmas are individually weak and cannot be pooled (the spectra are
//! normalised, so a deficit anywhere forces an excess elsewhere).
//!
//! # Interpretation, and what it does not establish
//!
//! Too little down-scatter, in the right direction and the right place for the
//! leading hypothesis: for A ≈ 235 elastic scattering removes almost no energy
//! (`⟨E'/E⟩ ≈ 1 − 2A/(A+1)² ≈ 0.992`), so **inelastic scattering is the
//! dominant energy-loss mechanism**, and a spectrum that is too hard is what an
//! inelastic secondary-energy treatment that returns neutrons too high in
//! energy would produce. A harder spectrum raises `ν̄` and U-238 threshold
//! fission, which is the right sign for the +69 ± 23 pcm `k_inf` excess.
//!
//! **It corroborates; it does not prove.** This measurement shows the spectrum
//! differs and where, not which reaction causes it — the remaining candidates
//! (MT=91's continuum law shape, the discrete MT=51–90 levels, `(n,2n)`) all
//! produce a hardness difference of this sign and would need separate ablation
//! to separate. Three small-sample results reversed earlier in this
//! investigation, so the seed count is quoted wherever these numbers are.
//!
//! ```text
//! cargo run --release -p outram-mc-libs --features endf-pebble-cases \
//!     --example godiva_spectrum_vs_openmc
//! ```

#[cfg(target_os = "android")]
fn main() {
    println!("godiva_spectrum_vs_openmc is desktop-only (reads reference-data/endf/).");
}

#[cfg(not(target_os = "android"))]
fn main() {
    desktop::run();
}

#[cfg(not(target_os = "android"))]
mod desktop {
    use njoy_outram_park_fork::reference_data::reference_endf;
    use outram_mc_libs::geometry::cell::{Cell, HalfSpaceSense, RegionToken};
    use outram_mc_libs::geometry::geometry::Geometry;
    use outram_mc_libs::geometry::position::Position;
    use outram_mc_libs::geometry::surface::{BoundaryType, Sphere, SurfaceKind};
    use outram_mc_libs::geometry::universe::Universe;
    use outram_mc_libs::material::material::{Material, NuclideComponent};
    use outram_mc_libs::material::nuclide::Nuclide;
    use outram_mc_libs::physics::keff::KeffSettings;
    use outram_mc_libs::physics::transport_csg::{run_keff_csg, SourceBox};
    use outram_mc_libs::tally::filter::EnergyFilter;
    use outram_mc_libs::tally::tally::{ScoreType, Tally, TallyBin};
    use std::time::Instant;

    const TEMP_K: f64 = 293.6;
    const RADIUS_CM: f64 = 8.7407;
    const HISTORIES: usize = 5000;
    const INACTIVE: usize = 40;
    const ACTIVE: usize = 120;
    /// 50 log bins over 1e-3 .. 2e7 eV — the same grid `flux_spectrum.rs` and
    /// the TUI spectrum overlay use.
    const N_BINS: usize = 50;
    const E_LO: f64 = 1.0e-3;
    const E_HI: f64 = 2.0e7;

    const NUCLIDES: &[(&str, &str, f64)] = &[
        ("n-092_U_234-ENDF8.0.endf", "U234", 4.9184e-4),
        ("n-092_U_235-ENDF8.0.endf", "U235", 4.4994e-2),
        ("n-092_U_238.endf", "U238", 2.4984e-3),
    ];

    fn log_grid(n: usize) -> Vec<f64> {
        let (l0, l1) = (E_LO.ln(), E_HI.ln());
        (0..=n)
            .map(|i| (l0 + (l1 - l0) * i as f64 / n as f64).exp())
            .collect()
    }

    /// A bare sphere of fuel: everything inside the surface is material 0, and
    /// the surface itself leaks.
    fn godiva_geometry() -> Geometry {
        Geometry {
            surfaces: vec![SurfaceKind::Sphere(Sphere {
                x0: 0.0,
                y0: 0.0,
                z0: 0.0,
                r: RADIUS_CM,
                bc: BoundaryType::Vacuum,
            })],
            cells: vec![Cell::material(
                1,
                vec![RegionToken::HalfSpace {
                    surface_idx: 0,
                    sense: HalfSpaceSense::Inside,
                }],
                0,
                TEMP_K,
            )],
            universes: vec![Universe {
                id: 0,
                cell_indices: vec![0],
            }],
            lattices: vec![],
            root_universe: 0,
        }
    }

    pub fn run() {
        let t0 = Instant::now();
        println!("Reconstructing HEU isotopes from ENDF/B-VIII.0 (RECONR + BROADR @ {TEMP_K} K)…");
        let nuclides: Vec<Nuclide> = NUCLIDES
            .iter()
            .map(|(f, n, _)| {
                let p = reference_endf(f).unwrap_or_else(|| panic!("missing tape {f}"));
                Nuclide::from_endf_file(&p, n, TEMP_K, 1.0e-3)
                    .unwrap_or_else(|e| panic!("from_endf_file: {e}"))
            })
            .collect();
        println!(
            "Nuclear data ready in {:.1} s.\n",
            t0.elapsed().as_secs_f64()
        );

        let materials = vec![Material {
            id: 1,
            name: "Godiva HEU".into(),
            temperature: TEMP_K,
            components: NUCLIDES
                .iter()
                .enumerate()
                .map(|(i, (_, _, d))| NuclideComponent {
                    nuclide_idx: i,
                    atom_density: *d,
                })
                .collect(),
        }];

        let edges = log_grid(N_BINS);
        let mut tally = Tally {
            id: 1,
            name: "godiva flux spectrum".into(),
            filters: vec![Box::new(EnergyFilter {
                bins: edges.clone(),
            })],
            scores: vec![ScoreType::Flux],
            bins: vec![TallyBin::default(); N_BINS],
        };

        let settings = KeffSettings {
            n_particles: HISTORIES,
            n_inactive: INACTIVE,
            n_active: ACTIVE,
            temperature_k: TEMP_K,
            seed: std::env::var("OURS_SEED")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(1),
            ..KeffSettings::default()
        };
        let src = SourceBox {
            lower: Position::new(-RADIUS_CM, -RADIUS_CM, -RADIUS_CM),
            upper: Position::new(RADIUS_CM, RADIUS_CM, RADIUS_CM),
        };

        println!(
            "{HISTORIES} histories x [{INACTIVE} + {ACTIVE}], tallying flux in {N_BINS} log bins…"
        );
        let t = Instant::now();
        let k = run_keff_csg(
            &godiva_geometry(),
            &materials,
            &nuclides,
            src,
            &settings,
            Some(&mut tally),
        );
        println!(
            "  k_eff = {:.5} ± {:.5}   ({:.1} s)",
            k.k_mean,
            k.k_std,
            t.elapsed().as_secs_f64()
        );

        let n_active = ACTIVE as u64;
        let total: f64 = tally.bins.iter().map(|b| b.mean(n_active)).sum();
        assert!(
            total > 0.0,
            "the flux tally is empty -- nothing was scored, so the comparison would be vacuous"
        );
        let path =
            std::env::var("OURS_SPECTRUM").unwrap_or_else(|_| "ours_spectrum.csv".to_string());
        let mut out = String::from("e_lo,e_hi,flux_norm\n");
        for (i, b) in tally.bins.iter().enumerate() {
            out.push_str(&format!(
                "{:.6e},{:.6e},{:.6e}\n",
                edges[i],
                edges[i + 1],
                b.mean(n_active) / total
            ));
        }
        std::fs::write(&path, out).expect("write spectrum csv");
        println!("  wrote {path} ({N_BINS} bins, normalised to unit integral)");
        println!("  Run at least 3 seeds a side (OURS_SEED / OURS_SPECTRUM) — one pair");
        println!("  cannot separate a spectral difference from re-randomisation. Then, in");
        println!("  verification_and_validation/openmc_godiva_cross_code/:");
        println!("    python3 compare_spectrum.py --ours ours_spec_*.csv --openmc omc_spec_*.csv");
    }
}
