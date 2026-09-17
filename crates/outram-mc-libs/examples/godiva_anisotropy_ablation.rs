//! **Ablation: what is elastic angular anisotropy worth on Godiva, and does it
//! behave the way the transport diagnosis says it should?**
//!
//! # Why ablate
//!
//! `verification_and_validation/openmc_godiva_cross_code/` attributes most of
//! this crate's `+250 ± 51 pcm` offset against OpenMC to **inelastic** angular
//! distributions, which this crate does not yet sample — they are isotropic in
//! the centre of mass, where the evaluation is not. The argument is:
//!
//! > isotropic scattering ⇒ `⟨μ⟩` too small ⇒ `Σ_tr = Σ_t(1 − ⟨μ⟩)` too large
//! > ⇒ diffusion coefficient too small ⇒ leakage too low ⇒ **k too high**.
//!
//! That is a *prediction about a mechanism*, and it is testable on the part of
//! the mechanism this crate **has** implemented. Elastic scattering is ~85 % of
//! all scattering on Godiva with an evaluated `⟨μ_lab⟩ ≈ 0.276`, against
//! inelastic's ~15 % at `⟨μ_lab⟩ ≈ 0.025`. So ablating elastic anisotropy should
//! push `k` **up**, by *much more* than the inelastic share is worth.
//!
//! If it pushes `k` down, or barely moves it, the diagnosis in that study is
//! wrong and must be revisited rather than tuned around.
//!
//! # What is ablated, and how
//!
//! [`Nuclide::with_isotropic_elastic_scattering`] empties the MF=4 table. The
//! transport kernel is **not** modified: `sample_elastic_mu_cm` already returns
//! `None` for a nuclide with no anisotropic data and every driver already falls
//! back to isotropic CM. The two arms therefore differ in exactly one input and
//! run identical code.
//!
//! Both tracking methods are ablated, because they are independent
//! implementations of the collision loop and a physics knob must move them
//! together. If it does not, the two kernels have drifted apart.
//!
//! # What this does NOT ablate
//!
//! **Unresolved-resonance self-shielding cannot be ablated here, because this
//! crate has none to switch off.** Its chain is RECONR + BROADR only, so the
//! unresolved range is at infinite dilution always. That ablation was done on
//! the OpenMC side instead (`godiva.py --ptables`), and measured
//! **+43 ± 38 pcm over 16 seeds per arm — 1.1 sigma, consistent with ZERO.**
//! An earlier 8/12-seed read gave `+79 ± 40` and is superseded; do not quote it.
//! URR self-shielding is a small term on Godiva, which is physically
//! unsurprising — a bare fast metal sphere has little flux in the unresolved
//! range. Pairing the arms by seed does not help: OpenMC's RNG stream diverges
//! as soon as the physics differs, so same-seed runs are uncorrelated (measured:
//! the paired difference has sd 170 pcm, larger than either arm's).
//!
//! It remains a real gap to close for correctness, and its sign is *opposite*
//! to the anisotropy fix, so the two must be priced independently and never
//! netted.
//!
//! # Results (2026-09-15, 8 seeds per arm)
//!
//! | arm | evaluated | ablated | worth of elastic anisotropy |
//! |---|---|---|---|
//! | surface | 1.00273 ± 50 pcm | 1.10785 ± 44 pcm | **+10511 ± 67 pcm** |
//! | delta | 1.00283 ± 52 pcm | 1.10812 ± 72 pcm | **+10528 ± 89 pcm** |
//!
//! The two tracking methods price it identically (differing by 17 ± 111 pcm),
//! and the sign is the predicted one: removing anisotropy **raises** k, by 10 %.
//!
//! **Calibration.** Elastic carries `0.852 × 0.2758 = 0.23498` of the `⟨μ⟩`
//! budget and inelastic `0.148 × 0.0254 = 0.00376`, a ratio of 0.0160. Scaling
//! this ablation by that ratio predicts the *missing* inelastic anisotropy at
//! **+168 pcm**, against **+181 pcm** measured from the `k_eff`/`k_inf` split
//! and **+219 pcm** from one-group diffusion. Three independent routes agreeing
//! to ~25 % — corroboration, not precision, since linear scaling across a
//! 10 000 pcm ablation is crude.
//!
//! The program asserts rather than prints, per this crate's "oracle examples
//! assert their comparison" rule.
//!
//! ```text
//! cargo run --release -p outram-mc-libs --features endf-pebble-cases \
//!     --example godiva_anisotropy_ablation
//! ```

#[cfg(target_os = "android")]
fn main() {
    println!("godiva_anisotropy_ablation is desktop-only (reads reference-data/endf/).");
}

#[cfg(not(target_os = "android"))]
fn main() {
    desktop::run();
}

#[cfg(not(target_os = "android"))]
mod desktop {
    use njoy_outram_park_fork::reference_data::reference_endf;
    use outram_mc_libs::geometry::position::Position;
    use outram_mc_libs::material::material::{Material, NuclideComponent};
    use outram_mc_libs::material::nuclide::Nuclide;
    use outram_mc_libs::pebble_beds::delta_tracking::Majorant;
    use outram_mc_libs::pebble_beds::keff_delta::{run_keff_delta_in, DeltaDomain};
    use outram_mc_libs::physics::keff::{run_keff, KeffSettings};
    use std::time::Instant;

    const TEMP_K: f64 = 293.6;
    const RADIUS_CM: f64 = 8.7407;
    const HISTORIES: usize = 5000;
    const INACTIVE: usize = 40;
    const ACTIVE: usize = 120;
    const NUCLIDES: &[(&str, &str, f64)] = &[
        ("n-092_U_234-ENDF8.0.endf", "U234", 4.9184e-4),
        ("n-092_U_235-ENDF8.0.endf", "U235", 4.4994e-2),
        ("n-092_U_238.endf", "U238", 2.4984e-3),
    ];

    /// The measured worth of elastic anisotropy on Godiva, in pcm.
    ///
    /// **+10511 ± 67 pcm (surface), +10528 ± 89 pcm (delta), 8 seeds per arm,
    /// 2026-09-15.** Recorded so that a change to the MF=4 reading or to the
    /// elastic scatter kinematics fails loudly instead of drifting: this is a
    /// 10 % effect on k, and anything that moves it is either a real physics
    /// change or a bug.
    const RECORDED_ELASTIC_WORTH_PCM: f64 = 10511.0;

    /// Flux-weighted contributions to `⟨μ⟩` over all scattering, measured in
    /// `verification_and_validation/openmc_godiva_cross_code/transport_decomposition.py`:
    /// elastic `0.852 × 0.2758 = 0.23498`, inelastic `0.148 × 0.0254 = 0.00376`.
    /// The ratio scales this ablation into a prediction for the missing
    /// inelastic anisotropy.
    const INELASTIC_OVER_ELASTIC_MU: f64 = 0.00376 / 0.23498;

    fn stats(x: &[f64]) -> (f64, f64, f64) {
        let n = x.len() as f64;
        let m = x.iter().sum::<f64>() / n;
        let var = x.iter().map(|v| (v - m).powi(2)).sum::<f64>() / (n - 1.0).max(1.0);
        (m, var.sqrt(), var.sqrt() / n.sqrt())
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

    fn load() -> Vec<Nuclide> {
        NUCLIDES
            .iter()
            .map(|(f, n, _)| {
                let p = reference_endf(f).unwrap_or_else(|| panic!("missing tape {f}"));
                Nuclide::from_endf_file(&p, n, TEMP_K, 1.0e-3)
                    .unwrap_or_else(|e| panic!("from_endf_file: {e}"))
            })
            .collect()
    }

    fn material() -> Material {
        Material {
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
        }
    }

    pub fn run() {
        let t0 = Instant::now();
        println!("Reconstructing HEU isotopes from ENDF/B-VIII.0 (RECONR + BROADR @ {TEMP_K} K)…");
        let evaluated = load();
        let ablated: Vec<Nuclide> = evaluated
            .iter()
            .cloned()
            .map(Nuclide::with_isotropic_elastic_scattering)
            .collect();
        println!(
            "Nuclear data ready in {:.1} s.\n",
            t0.elapsed().as_secs_f64()
        );

        // The ablation must actually have changed something. A silently no-op
        // ablation would "pass" every gate below while measuring nothing.
        let probe_e = 2.0e6;
        let mut s1 = 12345u64;
        let mut s2 = 12345u64;
        let anis_before = (0..2000)
            .filter_map(|_| evaluated[1].sample_elastic_mu_cm(probe_e, &mut s1))
            .count();
        let anis_after = (0..2000)
            .filter_map(|_| ablated[1].sample_elastic_mu_cm(probe_e, &mut s2))
            .count();
        println!(
            "  ablation check: U-235 returns an evaluated μ_cm {anis_before}/2000 times before, \
             {anis_after}/2000 after"
        );
        assert!(
            anis_before > 0,
            "the EVALUATED arm never returned an anisotropic cosine at {probe_e:.1e} eV -- \
             there is no anisotropy to ablate and this whole study is vacuous"
        );
        assert_eq!(
            anis_after, 0,
            "with_isotropic_elastic_scattering did not ablate: still returning evaluated cosines"
        );

        let mat = material();
        let mats = vec![mat.clone()];
        let maj_eval = Majorant::bounding(&mats, &evaluated, 1.0e-5, 2.0e7, 4000, 24, 0.10);
        let maj_abl = Majorant::bounding(&mats, &ablated, 1.0e-5, 2.0e7, 4000, 24, 0.10);
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
        println!("\n{n_seeds} seeds per arm, {HISTORIES} histories x [{INACTIVE} + {ACTIVE}]…");

        let (mut se, mut sa, mut de, mut da) = (vec![], vec![], vec![], vec![]);
        for seed in 1..=n_seeds as u64 {
            let s = settings(seed);
            let k_se = run_keff(RADIUS_CM, &mat, &evaluated, &s).k_mean;
            let k_sa = run_keff(RADIUS_CM, &mat, &ablated, &s).k_mean;
            let k_de = run_keff_delta_in(
                DeltaDomain::SphereVacuum { radius: RADIUS_CM },
                &mats,
                &evaluated,
                &maj_eval,
                material_at,
                &s,
            )
            .k_mean;
            let k_da = run_keff_delta_in(
                DeltaDomain::SphereVacuum { radius: RADIUS_CM },
                &mats,
                &ablated,
                &maj_abl,
                material_at,
                &s,
            )
            .k_mean;
            println!(
                "  seed {seed:>3}: surface {k_se:.5} / {k_sa:.5} (Δ{:+5.0})   \
                 delta {k_de:.5} / {k_da:.5} (Δ{:+5.0})  [evaluated / ablated, pcm]",
                (k_sa - k_se) * 1e5,
                (k_da - k_de) * 1e5
            );
            se.push(k_se);
            sa.push(k_sa);
            de.push(k_de);
            da.push(k_da);
        }

        let report = |label: &str, ev: &[f64], ab: &[f64]| -> (f64, f64) {
            let (me, _, seme) = stats(ev);
            let (ma, _, sema) = stats(ab);
            let d = (ma - me) * 1e5;
            let s = (seme * seme + sema * sema).sqrt() * 1e5;
            println!(
                "  {label:<8} evaluated {me:.5} ± {:.0} pcm   ablated {ma:.5} ± {:.0} pcm   \
                 worth {d:+.0} ± {s:.0} pcm",
                seme * 1e5,
                sema * 1e5
            );
            (d, s)
        };
        println!();
        let (d_surf, s_surf) = report("surface", &se, &sa);
        let (d_delta, s_delta) = report("delta", &de, &da);

        println!("\n=== Gate 1: ablating elastic anisotropy must RAISE k ===");
        for (label, d, s) in [("surface", d_surf, s_surf), ("delta", d_delta, s_delta)] {
            assert!(
                d > 3.0 * s,
                "{label}: removing elastic anisotropy moved k by {d:+.0} ± {s:.0} pcm, which is \
                 not a significant INCREASE.\n\
                 The transport diagnosis in verification_and_validation/\
                 openmc_godiva_cross_code/ predicts isotropic scattering lowers ⟨μ⟩, raises \
                 Σ_tr, cuts leakage and RAISES k. If this fires, that diagnosis is wrong and \
                 must be revisited -- do not widen this gate."
            );
            println!("  [PASS] {label}: {d:+.0} ± {s:.0} pcm, a significant increase");
        }

        println!("\n=== Gate 2: both tracking methods must price it the same ===");
        let diff = (d_surf - d_delta).abs();
        let comb = (s_surf * s_surf + s_delta * s_delta).sqrt();
        assert!(
            diff <= 3.0 * comb,
            "surface prices elastic anisotropy at {d_surf:+.0} pcm but delta at {d_delta:+.0} \
             pcm, differing by {diff:.0} pcm against a combined {comb:.0} pcm.\n\
             A physics knob must move two independent collision loops together; if it does \
             not, the kernels have drifted apart."
        );
        println!(
            "  [PASS] surface {d_surf:+.0} vs delta {d_delta:+.0} pcm, \
             differ by {diff:.0} ± {comb:.0} pcm"
        );

        println!("\n=== Gate 3: the measured worth must reproduce the recorded value ===");
        let drift = d_surf - RECORDED_ELASTIC_WORTH_PCM;
        let drift_gate = 4.0 * s_surf;
        assert!(
            drift.abs() <= drift_gate,
            "elastic anisotropy now prices at {d_surf:+.0} pcm against the recorded \
             {RECORDED_ELASTIC_WORTH_PCM:+.0} pcm -- a drift of {drift:+.0} pcm, outside the \
             {drift_gate:.0} pcm (4 sigma) gate. Either the MF=4 reading or the elastic \
             kinematics changed. That may be a fix; it is not a tolerance to widen."
        );
        println!(
            "  [PASS] {d_surf:+.0} pcm vs recorded {RECORDED_ELASTIC_WORTH_PCM:+.0} pcm \
             (drift {drift:+.0}, gate ±{drift_gate:.0})"
        );

        // Calibration: scale this ablation by the mu-bar-weighted share to predict
        // what the MISSING inelastic anisotropy is worth.
        let scaled = INELASTIC_OVER_ELASTIC_MU * d_surf;
        println!("\n  CALIBRATION -- what the missing inelastic anisotropy should be worth:");
        println!(
            "    scaled from this ablation ({:.4} of the mu-bar budget): {scaled:+.0} pcm",
            INELASTIC_OVER_ELASTIC_MU
        );
        println!("    measured, from the k_eff/k_inf split:                  ~+181 pcm");
        println!("    predicted, one-group diffusion:                         +219 pcm");
        println!("    Three independent routes, agreeing to ~25%. Linear scaling across a");
        println!("    10000 pcm ablation is crude, so this is corroboration, not precision.");

        println!("\n  URR: this crate has NO probability tables, so there is nothing to ablate");
        println!("  here. Measured on the OpenMC side instead (godiva.py --ptables), 16 seeds");
        println!("  per arm: +43 ± 38 pcm, 1.1 sigma -- CONSISTENT WITH ZERO, sign unresolved.");
        println!("  An earlier 8/12-seed read gave +79 ± 40 and is superseded. URR self-shielding");
        println!("  is a small term on Godiva, which is physically unsurprising: a bare fast");
        println!("  metal sphere has little flux in the unresolved range.");
    }
}
