//! **Diagnostic: Godiva's material as an INFINITE medium (`k_inf`), both
//! tracking methods, for comparison against OpenMC on the same material.**
//!
//! # Why `k_inf` splits the problem
//!
//! `outram-mc-libs` sits `+250 ± 51 pcm` above OpenMC on Godiva with identical
//! nuclear data (`verification_and_validation/openmc_godiva_cross_code/`), and
//! the data itself is now excluded: every reaction the kernel partitions on
//! agrees with the NJOY-produced ACE to **≤ 0.06 % flux-weighted**, U-235 —
//! 94 % of the material — to 0.0002 %. So the offset is in transport. This
//! program narrows *where*.
//!
//! Remove the boundary and the leakage term goes with it. `k_inf` still depends
//! on the cross sections, on ν̄, and on the **spectrum** (through which energies
//! the collisions happen at), but it does **not** depend on the angular
//! distribution, on the boundary treatment, or on anything geometric. So:
//!
//! - `k_inf` **differs** in the same direction ⇒ the fault is spectral —
//!   secondary energy sampling (χ, inelastic levels, the continuum law) or the
//!   reaction partition.
//! - `k_inf` **agrees** and only `k_eff` differs ⇒ the fault is in leakage —
//!   elastic angular distributions, free-gas treatment, or boundary crossing.
//!
//! That is a clean two-way split of the remaining hypothesis space, and it costs
//! one run per side.
//!
//! # Method
//!
//! The same ICSBEP HEU-MET-FAST-001 material, at the same radius and
//! temperature, in a **reflective** sphere — which is what
//! [`DeltaDomain::Sphere`] already provides, and what the delta path was
//! originally written for. The surface-tracking driver has no reflective mode,
//! so the two arms here are delta-tracked; the delta and surface paths were
//! shown to agree on the bare problem to `−10 ± 73 pcm`
//! (`examples/godiva_delta_vs_surface_tracking.rs`), so delta alone is an
//! adequate stand-in for both.
//!
//! The OpenMC counterpart is `godiva.py --kinf` in that V&V directory.
//!
//! ```text
//! cargo run --release -p outram-mc-libs --features endf-pebble-cases \
//!     --example godiva_kinf_vs_openmc
//! ```

#[cfg(target_os = "android")]
fn main() {
    println!("godiva_kinf_vs_openmc is desktop-only (reads reference-data/endf/).");
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
    use outram_mc_libs::physics::keff::KeffSettings;
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

    fn stats(x: &[f64]) -> (f64, f64, f64) {
        let n = x.len() as f64;
        let m = x.iter().sum::<f64>() / n;
        let var = x.iter().map(|v| (v - m).powi(2)).sum::<f64>() / (n - 1.0).max(1.0);
        (m, var.sqrt(), var.sqrt() / n.sqrt())
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
        println!("Nuclear data ready in {:.1} s.\n", t0.elapsed().as_secs_f64());

        let material = Material {
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
        };
        let materials = vec![material];
        let majorant = Majorant::bounding(&materials, &nuclides, 1.0e-5, 2.0e7, 4000, 24, 0.10);
        // Infinite medium: fuel everywhere inside the reflective boundary.
        let material_at = |_p: Position| -> Option<usize> { Some(0) };

        let n_seeds: usize = std::env::var("OUTRAM_GODIVA_SEEDS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(8);
        println!("{n_seeds} seeds, {HISTORIES} histories x [{INACTIVE} + {ACTIVE}], reflective sphere…");

        let mut ks = Vec::new();
        for seed in 1..=n_seeds as u64 {
            let s = KeffSettings {
                n_particles: HISTORIES,
                n_inactive: INACTIVE,
                n_active: ACTIVE,
                temperature_k: TEMP_K,
                seed,
                ..KeffSettings::default()
            };
            let t = Instant::now();
            let k = run_keff_delta_in(
                DeltaDomain::Sphere { radius: RADIUS_CM },
                &materials,
                &nuclides,
                &majorant,
                material_at,
                &s,
            );
            println!(
                "  seed {seed:>3}: k_inf = {:.5} ± {:.5}   ({:.1} s)",
                k.k_mean,
                k.k_std,
                t.elapsed().as_secs_f64()
            );
            ks.push(k.k_mean);
        }
        let (m, sd, sem) = stats(&ks);
        println!("\n  outram-mc-libs k_inf = {m:.5}  (sd {:.0} pcm, sem {:.0} pcm, n={})",
                 sd * 1e5, sem * 1e5, ks.len());
        println!("  Compare against: python3 godiva.py --kinf   in");
        println!("  verification_and_validation/openmc_godiva_cross_code/");
    }
}
