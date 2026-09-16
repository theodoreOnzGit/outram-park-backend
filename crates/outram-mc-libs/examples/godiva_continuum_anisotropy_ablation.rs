//! **Price the MF=6 continuum angular law on Godiva** — a paired ablation of
//! ENDF MF=6 LAW=1's `f₁ … f_NA` coefficients on MT=91 and MT=16, bead
//! `op-og56`.
//!
//! # What is being ablated
//!
//! Until `op-og56`, `njoy-outram-park-fork`'s MF=6 parser read ENDF `NA` only to
//! compute the row stride, kept the energy density `f₀`, and discarded
//! `f₁ … f_NA`; `LANG` was never read. So every continuum-inelastic (MT=91) and
//! (n,2n) (MT=16) neutron left the collision isotropically in the frame the law
//! names. The evaluations say otherwise: 8652 of U-238's 8654 MT=91 rows carry
//! `NA > 0`, and 5294 of U-235's 5296.
//!
//! This is the **third and last** of the angular channels. Elastic (MF=4/MT=2)
//! was always sampled; the discrete inelastic levels (MF=4/MT=51…90) were wired
//! in by `op-tm9f` and priced at **−198 pcm** by
//! `examples/godiva_inelastic_anisotropy_ablation.rs`; the continuum is this
//! one.
//!
//! # Method
//!
//! Two arms over the **same seeds**, identical in every other respect —
//! geometry, the three ICSBEP nuclide densities, temperature, histories,
//! generation split, and the cross sections themselves. The ablation replaces
//! each branch's angular law with
//! [`ContinuumAngular::Ablated`](njoy_outram_park_fork::nuclear_data::secondary::ContinuumAngular::Ablated)
//! and touches nothing else — not `f₀(E→E')`, not the branch yields, not a
//! single cross section. `tests/continuum_angular_ablation_control.rs` asserts
//! that, including that the two arms consume identical RNG variates in identical
//! order.
//!
//! - **ANISO** — the evaluated per-row Legendre cosine law.
//! - **ISO** — [`Nuclide::with_isotropic_continuum_scattering`], i.e. exactly
//!   the behaviour before `op-og56`.
//!
//! `N` seeds per arm (default 16, override with `OUTRAM_GODIVA_SEEDS`) at
//! 5000 histories × [40 inactive + 120 active]. A single run cannot resolve an
//! effect of this size: one draw of this model carries seed-to-seed
//! `sd ≈ 173 pcm`, so two single runs differ by ~245 pcm from re-randomisation
//! alone. Both the unpaired and the paired difference are printed; read
//! whichever the run actually shows to be tighter rather than assuming.
//!
//! # Prediction, stated before the measurement
//!
//! **Expect a SMALL move DOWN — well under 50 pcm, plausibly under 20.**
//!
//! The direction is the same mechanism as `op-tm9f`: forward-peaked emission
//! raises `⟨μ⟩`, lowers `Σ_tr = Σ_t(1 − ⟨μ⟩)`, lengthens the transport mean free
//! path and so increases leakage. Godiva is 55.8 % leakage, so `k` falls.
//!
//! The *magnitude* is what separates this from `op-tm9f`, and it is why the
//! prediction is small rather than another ~200 pcm. Weighted by each row's own
//! emission probability `f₀` — the `⟨μ_cm⟩` a neutron actually experiences —
//! U-238's MT=91 law is **exactly isotropic near threshold** and only turns on
//! at several MeV (measured 2026-09-16):
//!
//! | `E_in` | pdf-weighted `⟨μ_cm⟩` |
//! |---|---|
//! | 0.4356 MeV (threshold) | 0.000000 |
//! | 1.02 MeV | 0.000000 |
//! | 3.00 MeV | +0.001684 |
//! | 8.50 MeV | +0.073219 |
//! | 14.0 MeV | +0.272349 |
//! | 20.0 MeV | +0.387079 |
//!
//! The discrete levels of `op-tm9f` are anisotropic from ~1 MeV — the bulk of a
//! fission spectrum. This law has structure only above ~8 MeV, where a fission
//! spectrum has a percent or two of its flux. The headline "peak `|⟨μ⟩|` =
//! 0.557" quoted from the raw coefficients is **not** the number to reason
//! from: it sits in the far tail of a high-energy table where `f₀` is
//! negligible.
//!
//! **If this comes back at ~200 pcm, the diagnosis is wrong** and the wiring
//! should be suspected before the physics — most likely the angular law being
//! applied at the wrong incident energy, which would import the 14 MeV cosines
//! into the 2 MeV flux.
//!
//! # A second effect, recorded because it pulls the other way
//!
//! For a centre-of-mass law (`LCT = 2`, which both uranium isotopes use) the lab
//! energy is `E' = E_cm + E_trans + 2·μ_cm·√(E_cm·E_trans)`, so a forward-peaked
//! cosine also **raises** `⟨E'_lab⟩` and hardens the spectrum. This crate's
//! spectrum is already 0.45 % too hard against OpenMC (`op-os8x`), so this
//! change moves that residual the **wrong way**. Same shape as gh:#192's
//! Q-value cap, which was correct and moved Godiva further from the experiment.
//! Correctness is not chosen for its direction, and the tension is stated rather
//! than smoothed.
//!
//! # Results
//!
//! **Not yet measured.** This example is the instrument; running it is the
//! measurement. When it has been run at a seed count that can resolve the
//! prediction above, record the table here with the date, the seed count, and
//! whether the prediction held — and if it did not, say so plainly rather than
//! revising the prediction after the fact.
//!
//! ```text
//! OUTRAM_GODIVA_SEEDS=32 cargo run --release -p outram-mc-libs \
//!     --features endf-pebble-cases --example godiva_continuum_anisotropy_ablation
//! ```

#[cfg(target_os = "android")]
fn main() {
    println!("godiva_continuum_anisotropy_ablation is desktop-only (reads reference-data/endf/).");
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
    use outram_mc_libs::physics::keff::{run_keff, KeffSettings};
    use std::sync::Arc;
    use std::time::Instant;

    /// Godiva's fuel temperature \[K\] — the tapes are broadened to it.
    const TEMP_K: f64 = 293.6;
    /// ICSBEP HEU-MET-FAST-001 sphere radius \[cm\].
    const RADIUS_CM: f64 = 8.7407;
    /// Worker threads. Each runs whole seeds; transport inside a seed is serial.
    const WORKERS: usize = 4;

    /// `(tape file, nuclide name, atom density \[atoms/barn·cm\])` — the three
    /// ICSBEP nuclides, same numbers as `godiva_keff_endf_local`.
    const NUCLIDES: &[(&str, &str, f64)] = &[
        ("n-092_U_234-ENDF8.0.endf", "U234", 4.9184e-4),
        ("n-092_U_235-ENDF8.0.endf", "U235", 4.4994e-2),
        ("n-092_U_238.endf", "U238", 2.4984e-3),
    ];

    /// Mean, sample standard deviation and standard error of a sample.
    fn stats(x: &[f64]) -> (f64, f64, f64) {
        let n = x.len() as f64;
        let mean = x.iter().sum::<f64>() / n;
        let var = x.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / (n - 1.0).max(1.0);
        let sd = var.sqrt();
        (mean, sd, sd / n.sqrt())
    }

    /// Run one arm over `seeds`, returning Δk from the benchmark in pcm per seed.
    fn run_arm(nuclides: &Arc<Vec<Nuclide>>, material: &Material, seeds: &[u64]) -> Vec<f64> {
        let mut out = vec![0.0; seeds.len()];
        let chunk = seeds.len().div_ceil(WORKERS);
        std::thread::scope(|s| {
            for (sd_chunk, out_chunk) in seeds.chunks(chunk).zip(out.chunks_mut(chunk)) {
                let nuclides = Arc::clone(nuclides);
                s.spawn(move || {
                    for (k, &seed) in sd_chunk.iter().enumerate() {
                        let settings = KeffSettings {
                            n_particles: 5000,
                            n_inactive: 40,
                            n_active: 120,
                            temperature_k: TEMP_K,
                            seed,
                            ..KeffSettings::default()
                        };
                        let r = run_keff(RADIUS_CM, material, &nuclides, &settings);
                        out_chunk[k] = (r.k_mean - 1.0) * 1.0e5;
                    }
                });
            }
        });
        out
    }

    pub fn run() {
        let n_seeds: usize = std::env::var("OUTRAM_GODIVA_SEEDS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(16);

        println!("Reconstructing HEU isotopes from ENDF/B-VIII.0 (RECONR + BROADR @ 293.6 K)…");
        let t0 = Instant::now();
        let mut aniso = Vec::new();
        for &(file, name, _) in NUCLIDES {
            let Some(p) = reference_endf(file) else {
                println!(
                    "  missing {file} — set OUTRAM_PARK_ENDF_DIR or fetch the tape; skipping."
                );
                return;
            };
            aniso.push(
                Nuclide::from_endf_file(&p, name, TEMP_K, 1.0e-3)
                    .unwrap_or_else(|e| panic!("from_endf_file({}): {e}", p.display())),
            );
        }

        // The control, checked before spending an hour of transport on two arms
        // that might be identical: at least one nuclide must actually carry a
        // continuum angular law, or the "ablation" removes nothing and the run
        // reports a null result that reads as "this physics does not matter".
        let carriers: Vec<&str> = aniso
            .iter()
            .filter(|n| n.has_continuum_anisotropy())
            .map(|n| n.name.as_str())
            .collect();
        assert!(
            !carriers.is_empty(),
            "not one of the three ICSBEP nuclides carries a continuum angular law, so this \
             ablation would remove nothing and report a meaningless null. Either the MF=6 \
             coefficients stopped being read (bead op-og56) or the tapes changed."
        );
        println!("  continuum angular law present on: {}", carriers.join(", "));

        let iso: Vec<Nuclide> = aniso
            .iter()
            .cloned()
            .map(Nuclide::with_isotropic_continuum_scattering)
            .collect();
        assert!(
            iso.iter().all(|n| !n.has_continuum_anisotropy()),
            "the ablation left a continuum angular law in place on at least one nuclide -- it is \
             a no-op, and any difference measured below would be an artefact"
        );
        println!(
            "Nuclear data ready in {:.1} s.\n",
            t0.elapsed().as_secs_f64()
        );

        let material = Material {
            id: 1,
            name: "Godiva HEU".into(),
            temperature: TEMP_K,
            components: NUCLIDES
                .iter()
                .enumerate()
                .map(|(i, &(_, _, rho))| NuclideComponent {
                    nuclide_idx: i,
                    atom_density: rho,
                })
                .collect(),
        };

        let seeds: Vec<u64> = (1..=n_seeds as u64).collect();
        let aniso = Arc::new(aniso);
        let iso = Arc::new(iso);

        println!("{n_seeds} seeds per arm, 5000 histories × [40 inactive + 120 active]…");
        let t = Instant::now();
        let a = run_arm(&aniso, &material, &seeds);
        println!("  ANISO arm done in {:.1} s", t.elapsed().as_secs_f64());
        let t = Instant::now();
        let b = run_arm(&iso, &material, &seeds);
        println!("  ISO   arm done in {:.1} s\n", t.elapsed().as_secs_f64());

        let (ma, sa, ea) = stats(&a);
        let (mb, sb, eb) = stats(&b);
        let diff = ma - mb;
        let ediff = (ea * ea + eb * eb).sqrt();

        // Paired: the arms share seeds. Unlike the discrete-level ablation, the
        // two arms here consume the SAME number of RNG draws per collision (one
        // variate, spent either inverting a cosine CDF or mapping it linearly),
        // so the streams do not diverge at the first continuum collision and the
        // pairing has a real chance of helping. Whether it does is measured, not
        // assumed — both figures are printed and the run says which to quote.
        let d: Vec<f64> = a.iter().zip(&b).map(|(x, y)| x - y).collect();
        let (md, sdp, edp) = stats(&d);

        println!("Δk from ICSBEP HEU-MET-FAST-001 = 1.0000, pcm");
        println!("  arm       n     mean      sd     sem");
        println!("  ANISO   {:>3}   {:+7.0}  {:>6.0}  {:>6.0}", a.len(), ma, sa, ea);
        println!("  ISO     {:>3}   {:+7.0}  {:>6.0}  {:>6.0}", b.len(), mb, sb, eb);
        println!();
        println!(
            "  difference (ANISO − ISO), unpaired = {:+.0} ± {:.0} pcm  ({:.1} sigma)",
            diff,
            ediff,
            (diff / ediff).abs()
        );
        println!(
            "  difference (ANISO − ISO), paired   = {:+.0} ± {:.0} pcm  ({:.1} sigma), paired sd {:.0}",
            md,
            edp,
            (md / edp).abs(),
            sdp
        );
        if sdp > sa.max(sb) {
            println!(
                "  -> paired sd ({sdp:.0}) EXCEEDS either arm's ({:.0}); the seeds do not pair, \
                 so quote the unpaired figure.",
                sa.max(sb)
            );
        } else {
            println!(
                "  -> paired sd ({sdp:.0}) is below either arm's ({:.0}); the pairing helps, \
                 so quote the paired figure.",
                sa.max(sb)
            );
        }

        println!(
            "\n  Prediction on op-og56, recorded before this ran: ANISO − ISO is SMALL and\n  \
             NEGATIVE — well under 50 pcm, plausibly under 20. U-238's MT=91 law is exactly\n  \
             isotropic below 1.2 MeV and only reaches <mu_cm> = +0.27 at 14 MeV, where a\n  \
             fission spectrum has little flux. A result near -200 pcm would mean the law is\n  \
             being applied at the wrong incident energy, not that it is worth that much."
        );
        let resolvable = 3.0 * ediff;
        if diff.abs() < resolvable {
            println!(
                "  NOTE: |{diff:+.0}| is inside 3 sigma ({resolvable:.0} pcm) of this run's own\n  \
                 statistics, so it is consistent with zero AND with the prediction. That is a\n  \
                 bound, not a measurement — raise OUTRAM_GODIVA_SEEDS to resolve it."
            );
        }
    }
}
