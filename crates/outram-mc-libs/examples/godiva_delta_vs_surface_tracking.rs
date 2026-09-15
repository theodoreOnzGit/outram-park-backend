//! **V&V: delta (Woodcock) tracking and surface tracking must give the same
//! Godiva eigenvalue.**
//!
//! # What this checks, and why it is worth checking
//!
//! Delta tracking and surface tracking are two *estimators of the same
//! quantity*. Woodcock tracking replaces the real `Sigma_t` with a majorant and
//! rejects the difference as virtual collisions; the rejection is exact, so the
//! collision density it produces is the same one ray-tracing to surfaces
//! produces. They consume randomness completely differently and therefore never
//! bit-match, but their eigenvalues must agree to within combined statistics.
//!
//! That makes this a **cross-check with no free parameters**: nothing can be
//! tuned to make it pass. If the two disagree by more than statistics allow, one
//! of them is wrong — a mis-sampled flight, a majorant that does not bound
//! `Sigma_t`, a boundary that leaks when it should reflect or the reverse, or a
//! reaction partition that differs between the two drivers.
//!
//! Both arms call the *same* secondary-physics helpers
//! (`continuum_inelastic_scatter_evaluated`, `two_body_scatter`,
//! `free_gas_elastic_scatter`) and the same reaction partition, so a
//! disagreement localises to transport, not to physics data.
//!
//! # Why this did not exist before
//!
//! `run_keff_delta*` was written for pebble-bed `k_inf`, where the cell boundary
//! *is* reflective, and every [`DeltaDomain`] variant reflected. A bare critical
//! assembly is the opposite problem — leakage is most of the physics, and
//! reflecting it computes a different eigenvalue entirely — so delta tracking
//! simply could not express Godiva, and the surface-tracking driver had no
//! counterpart to be checked against. [`DeltaDomain::SphereVacuum`] (added
//! 2026-09-15 alongside this program) closes that.
//!
//! # Methodology
//!
//! ICSBEP **HEU-MET-FAST-001** (Godiva): bare HEU sphere, `r = 8.7407 cm`, the
//! three ICSBEP nuclides at their specification densities, ENDF/B-VIII.0
//! reconstructed with RECONR + BROADR at 293.6 K. Identical material, radius,
//! temperature, source spectrum and generation counts in both arms; only the
//! transport differs.
//!
//! - **Surface arm** — [`run_keff`], which ray-traces to the sphere and leaks
//!   on crossing.
//! - **Delta arm** — [`run_keff_delta_in`] on [`DeltaDomain::SphereVacuum`],
//!   with a majorant built by [`Majorant::bounding`] (the bin-maximum
//!   constructor, not the point sampler — Godiva runs on reconstructed
//!   resonance data, where a peak between two grid points would otherwise slip
//!   under the bound and bias the real/virtual split).
//!
//! Each arm is run over `N` independent seeds (default 8, override with
//! `OUTRAM_GODIVA_SEEDS`) because Godiva's seed-to-seed spread at these settings
//! is **sd ~ 170 pcm** — a single pair of runs cannot resolve a difference
//! smaller than that, and reporting one would be meaningless.
//!
//! **Pass criterion.** `|surface − delta|` within **3 sigma** of the combined
//! standard error of the two pooled means. Three, not one: a V&V gate that fires
//! on ordinary statistical fluctuation trains people to ignore it.
//!
//! # Results
//!
//! Filled in by the run; see the printed table. This program asserts the
//! agreement rather than printing it, per this crate's "oracle examples assert
//! their comparison" rule.
//!
//! # Scope
//!
//! One geometry, one material, one temperature. This is **verification** that
//! two transport methods in this crate agree with each other — it is not
//! validation, and it says nothing about whether either agrees with the
//! experiment. That claim lives in `examples/godiva_keff_endf_local.rs`.
//!
//! ```text
//! cargo run --release -p outram-mc-libs --features endf-pebble-cases \
//!     --example godiva_delta_vs_surface_tracking
//! ```

#[cfg(target_os = "android")]
fn main() {
    println!("godiva_delta_vs_surface_tracking is desktop-only (reads reference-data/endf/).");
}

#[cfg(not(target_os = "android"))]
fn main() {
    desktop::run();
}

#[cfg(not(target_os = "android"))]
mod desktop {
    use njoy_outram_park_fork::reference_data::reference_endf;
    use outram_mc_libs::material::material::{Material, NuclideComponent};
    use outram_mc_libs::material::nuclide::Nuclide;
    use outram_mc_libs::pebble_beds::delta_tracking::Majorant;
    use outram_mc_libs::pebble_beds::keff_delta::{run_keff_delta_in, DeltaDomain};
    use outram_mc_libs::physics::keff::{run_keff, KeffSettings};
    use outram_mc_libs::geometry::position::Position;
    use std::time::Instant;

    /// Godiva's fuel temperature \[K\] — the tapes are broadened to it.
    const TEMP_K: f64 = 293.6;
    /// ICSBEP HEU-MET-FAST-001 sphere radius \[cm\].
    const RADIUS_CM: f64 = 8.7407;
    /// Histories per generation, and the generation split.
    const HISTORIES: usize = 5000;
    const INACTIVE: usize = 40;
    const ACTIVE: usize = 120;

    /// `(tape file, nuclide name, atom density \[atoms/barn·cm\])` — the three
    /// ICSBEP nuclides, same numbers as `godiva_keff_endf_local`.
    const NUCLIDES: &[(&str, &str, f64)] = &[
        ("n-092_U_234-ENDF8.0.endf", "U234", 4.9184e-4),
        ("n-092_U_235-ENDF8.0.endf", "U235", 4.4994e-2),
        ("n-092_U_238.endf", "U238", 2.4984e-3),
    ];

    /// Energy span the majorant must bound \[eV\] — birth energy down to the
    /// lowest energy any history reaches.
    const E_MIN: f64 = 1.0e-5;
    const E_MAX: f64 = 2.0e7;
    /// Log bins and per-bin subsamples for [`Majorant::bounding`].
    const MAJ_BINS: usize = 4000;
    const MAJ_SUBSAMPLES: usize = 24;
    /// Safety cushion on the majorant envelope (fraction).
    const MAJ_MARGIN: f64 = 0.10;

    /// Mean, sample standard deviation and standard error of a sample.
    fn stats(x: &[f64]) -> (f64, f64, f64) {
        let n = x.len() as f64;
        let mean = x.iter().sum::<f64>() / n;
        let var = x.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / (n - 1.0).max(1.0);
        let sd = var.sqrt();
        (mean, sd, sd / n.sqrt())
    }

    fn settings(seed: u64) -> KeffSettings {
        KeffSettings {
            n_particles: HISTORIES,
            n_inactive: INACTIVE,
            n_active: ACTIVE,
            temperature_k: TEMP_K,
            seed,
            ..KeffSettings::default()
        }
    }

    pub fn run() {
        let t0 = Instant::now();
        println!("Reconstructing HEU isotopes from ENDF/B-VIII.0 (RECONR + BROADR @ {TEMP_K} K)…");
        let nuclides: Vec<Nuclide> = NUCLIDES
            .iter()
            .map(|(file, name, _)| {
                let p = reference_endf(file)
                    .unwrap_or_else(|| panic!("missing reference tape {file}"));
                Nuclide::from_endf_file(&p, name, TEMP_K, 1.0e-3)
                    .unwrap_or_else(|e| panic!("from_endf_file({}): {e}", p.display()))
            })
            .collect();
        println!("Nuclear data ready in {:.1} s.\n", t0.elapsed().as_secs_f64());

        let material = Material {
            id: 1,
            name: "Godiva HEU".into(),
            temperature: TEMP_K,
            components: NUCLIDES
                .iter()
                .enumerate()
                .map(|(i, (_, _, dens))| NuclideComponent {
                    nuclide_idx: i,
                    atom_density: *dens,
                })
                .collect(),
        };
        let materials = vec![material.clone()];

        // The majorant must bound Sigma_t EVERYWHERE, not just at grid points --
        // Godiva runs on reconstructed resonance data. `bounding` takes the
        // maximum over a dense sub-sample of each bin, so a peak between grid
        // points cannot slip under it.
        let t_maj = Instant::now();
        let majorant = Majorant::bounding(
            &materials,
            &nuclides,
            E_MIN,
            E_MAX,
            MAJ_BINS,
            MAJ_SUBSAMPLES,
            MAJ_MARGIN,
        );
        println!(
            "Majorant built in {:.1} s ({MAJ_BINS} bins x {MAJ_SUBSAMPLES} subsamples, \
             +{:.0}% margin)",
            t_maj.elapsed().as_secs_f64(),
            MAJ_MARGIN * 100.0
        );

        // Bare sphere: fuel inside, void outside. Returning None outside is what
        // makes the history leak.
        let r2 = RADIUS_CM * RADIUS_CM;
        let material_at = move |p: Position| -> Option<usize> {
            if p.norm_sqr() <= r2 {
                Some(0)
            } else {
                None
            }
        };

        let n_seeds: usize = std::env::var("OUTRAM_GODIVA_SEEDS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(8);
        println!(
            "\n{n_seeds} seeds per arm, {HISTORIES} histories x [{INACTIVE} inactive + \
             {ACTIVE} active]…"
        );

        let (mut surf, mut delta) = (Vec::new(), Vec::new());
        for seed in 1..=n_seeds as u64 {
            let s = settings(seed);

            let t = Instant::now();
            let k_s = run_keff(RADIUS_CM, &material, &nuclides, &s);
            let dt_s = t.elapsed().as_secs_f64();

            let t = Instant::now();
            let k_d = run_keff_delta_in(
                DeltaDomain::SphereVacuum { radius: RADIUS_CM },
                &materials,
                &nuclides,
                &majorant,
                material_at,
                &s,
            );
            let dt_d = t.elapsed().as_secs_f64();

            surf.push(k_s.k_mean);
            delta.push(k_d.k_mean);
            println!(
                "  seed {seed:>3}: surface {:.5} ({dt_s:>5.1} s)   delta {:.5} ({dt_d:>5.1} s)   \
                 Δ = {:+.0} pcm",
                k_s.k_mean,
                k_d.k_mean,
                (k_s.k_mean - k_d.k_mean) * 1.0e5
            );
        }

        let (ms, sds, sems) = stats(&surf);
        let (md, sdd, semd) = stats(&delta);
        let diff_pcm = (ms - md) * 1.0e5;
        let comb_sem_pcm = (sems * sems + semd * semd).sqrt() * 1.0e5;

        println!("\n  arm        n      k_mean       sd(pcm)   sem(pcm)   Δk vs ICSBEP");
        println!(
            "  surface  {:>3}   {ms:.5}   {:>7.0}   {:>8.0}   {:+.0} pcm",
            surf.len(),
            sds * 1.0e5,
            sems * 1.0e5,
            (ms - 1.0) * 1.0e5
        );
        println!(
            "  delta    {:>3}   {md:.5}   {:>7.0}   {:>8.0}   {:+.0} pcm",
            delta.len(),
            sdd * 1.0e5,
            semd * 1.0e5,
            (md - 1.0) * 1.0e5
        );
        println!(
            "\n  surface − delta = {diff_pcm:+.0} ± {comb_sem_pcm:.0} pcm  ({:.1} sigma)",
            if comb_sem_pcm > 0.0 {
                (diff_pcm / comb_sem_pcm).abs()
            } else {
                0.0
            }
        );

        println!("\n=== V&V gate: delta tracking reproduces surface tracking ===");
        let gate = 3.0 * comb_sem_pcm;
        assert!(
            diff_pcm.abs() <= gate,
            "delta and surface tracking disagree by {diff_pcm:+.0} pcm, outside the \
             {gate:.0} pcm (3 sigma) gate.\n\
             These are two estimators of the SAME eigenvalue and nothing here is \
             tunable, so this is a defect, not a tolerance to widen. Look at: the \
             majorant actually bounding Sigma_t everywhere (raise MAJ_BINS / \
             MAJ_SUBSAMPLES and re-run — if the gap moves, the majorant was the \
             problem); the SphereVacuum boundary leaking exactly at r = {RADIUS_CM}; \
             and whether the two drivers' reaction partitions have drifted apart."
        );
        println!(
            "  [PASS] surface {ms:.5} vs delta {md:.5}: {diff_pcm:+.0} pcm, \
             gate ±{gate:.0} pcm (3 sigma of {n_seeds} seeds/arm)"
        );
    }
}
