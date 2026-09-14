//! **DH treatment V&V** — the eigenvalue each [`DhTreatment`] produces on one
//! identical pebble, so the speedups in `examples/dh_tracking_speedup.rs` can be
//! judged on accuracy rather than on cost alone.
//!
//! ```text
//! cargo run --release -p outram-mc-libs --example dh_keff_vv
//! ```
//!
//! # What this establishes, and what it does not
//!
//! **Establishes:** how far chord-length sampling and ring-RPT homogenisation
//! move k-infinity away from an exact delta-tracked answer on the same geometry,
//! same materials, same settings, same seed. That difference is the accuracy
//! cost of the corresponding speedup, and it is what a user choosing a treatment
//! actually needs to know.
//!
//! **Does not establish:** that any of these eigenvalues is right. The reference
//! here is this crate's own exact treatment, not an experiment and not another
//! code. It is a **verification** of the approximate treatments against the exact
//! one — "do the shortcuts agree with the long way?" — not a validation against
//! measured criticality. For validation against an experiment, see the ICSBEP
//! Godiva case in `examples/godiva_keff_endf_local.rs`; for code-to-code against
//! OpenMC on this very pebble, `examples/fhr_ring_rpt_endf.rs`.
//!
//! Reflective boundary at the pebble surface, so this is k-infinity for a lattice
//! of identical pebbles, not a critical system.
//!
//! LOW-tier embedded data (`Nuclide::from_core`), so it runs offline in seconds.
//! That fidelity is not enough to compare against a benchmark, but it is the
//! same data for all three arms — which is all a *relative* comparison needs.
//!
//! # Results — measured 2026-09-14
//!
//! 800 histories x [15 inactive + 40 active], LOW tier, single-threaded,
//! 25 856 explicit particles in the delta arm.
//!
//! | Treatment | k-infinity | Time | vs delta | Bias |
//! |---|---|---|---|---|
//! | delta tracking (exact) | 1.22645 +/- 0.00691 | 10.0 s | 1.00x | — |
//! | chord-length sampling | 1.18646 +/- 0.00573 | 26.3 s | **0.38x** | **-3999 pcm (-4.5 sigma)** |
//! | ring-RPT (homogenised) | 1.18303 +/- 0.00614 | 71.0 s | **0.14x** | **-4342 pcm (-4.7 sigma)** |
//!
//! ## Both approximations are SLOWER here, and that is the headline
//!
//! `examples/dh_tracking_speedup.rs` measures the same three treatments as
//! *geometry* and finds chord-length ~2.5-3x and ring-RPT ~8x **faster** than
//! delta tracking. In a real eigenvalue calculation the ranking **inverts**.
//! The speedup measured on a geometry-only benchmark does not transfer, and
//! anyone choosing a treatment on the strength of that benchmark alone would
//! choose wrongly.
//!
//! The likely mechanism, which the numbers are consistent with but which this
//! example does **not** isolate: homogenisation moves cost out of geometry and
//! into cross-section evaluation. Explicitly, most of the fuel zone by volume is
//! single-nuclide carbon — buffer, IPyC, OPyC, matrix — so a delta-tracking
//! point query there sums **one** nuclide. Smear the particles and every point
//! in the zone carries the union of all five (C, O16, Si28, U235, U238), so
//! every query sums **five**. The geometry lookup that was traded away was
//! already O(1) through the packing grid, so the trade is roughly 5x the
//! cross-section work for nearly nothing.
//!
//! Chord-length sampling pays a second, avoidable cost: its sampler is stateful
//! and the k-eff seam wants `Fn + Sync`, so every point query takes a mutex (see
//! [`DhUniverse::keff`](outram_mc_libs::dh_universe::DhUniverse::keff)). That is
//! an implementation artefact of this wiring, not a property of CLS, and it is
//! the first thing to attack if CLS is wanted for production.
//!
//! **Neither of these is established here.** Separating material cost from
//! geometry cost needs a profile or an A/B with matched nuclide counts, and that
//! has not been run. The ranking above is measured; the explanation is a
//! hypothesis consistent with it.
//!
//! ## The accuracy result
//!
//! Both approximations sit about **4000 pcm low**, resolved at 4.5-4.7 sigma.
//! The sign is physically right for ring-RPT: smearing destroys the spatial
//! self-shielding that shields U-238's resonances inside the kernels, so the
//! resonance absorber sees more flux, absorbs more, and k falls. A ~4000 pcm
//! penalty is why the ring-RPT inner radius is a *fitted* parameter rather than
//! a geometric one — the fit is what buys that back.
//!
//! At these settings the combined standard error is ~900 pcm, so this run can
//! resolve a bias of roughly 2000 pcm and no better. A treatment reading "not
//! resolved" has not been shown to be unbiased; it has been shown to be
//! **unmeasured**. Raise `OUTRAM_DH_VV_HISTORIES` to tighten it.

use std::time::Instant;

use outram_mc_libs::prelude::*;
use outram_mc_libs::material::material::NuclideComponent;

/// Representative HALEU UCO TRISO compositions \[atoms/b-cm\], room temperature.
///
/// Illustrative of the material class, **not** a benchmark specification — the
/// point of this example is the difference between treatments, which is
/// insensitive to the exact densities so long as all three arms share them.
fn materials() -> Vec<Material> {
    // Nuclide indices into the vector returned by `nuclides()`.
    const U235: usize = 0;
    const U238: usize = 1;
    const O16: usize = 2;
    const C: usize = 3;
    const SI28: usize = 4;

    let m = |id: i32, name: &str, comps: Vec<(usize, f64)>| Material {
        id,
        name: name.into(),
        temperature: 293.6,
        components: comps
            .into_iter()
            .map(|(nuclide_idx, atom_density)| NuclideComponent {
                nuclide_idx,
                atom_density,
            })
            .collect(),
    };

    vec![
        // 0 kernel — 19.9 % HALEU UCO
        m(0, "UCO kernel", vec![
            (U235, 4.40e-3), (U238, 1.77e-2), (O16, 2.27e-2), (C, 9.10e-3),
        ]),
        // 1 buffer — porous carbon
        m(1, "buffer", vec![(C, 5.02e-2)]),
        // 2 IPyC
        m(2, "IPyC", vec![(C, 9.53e-2)]),
        // 3 SiC
        m(3, "SiC", vec![(SI28, 4.79e-2), (C, 4.79e-2)]),
        // 4 OPyC
        m(4, "OPyC", vec![(C, 9.53e-2)]),
        // 5 matrix — graphite binder in the fuel zone
        m(5, "matrix graphite", vec![(C, 8.53e-2)]),
        // 6 shell — fuel-free graphite outer shell
        m(6, "shell graphite", vec![(C, 8.78e-2)]),
    ]
}

fn nuclides() -> Vec<Nuclide> {
    ["U235", "U238", "O16", "C0", "Si28"]
        .iter()
        .map(|n| Nuclide::from_core(n).unwrap_or_else(|e| panic!("core nuclide {n}: {e:?}")))
        .collect()
}

fn main() {
    let nucs = nuclides();
    let mats = materials();

    // Deliberately modest so the example finishes in a couple of minutes. The
    // delta-tracked arm resolves ~51k explicit particles with continuous-energy
    // data and is the expensive one; at 3000 x [30 + 90] it ran past 15 minutes,
    // which makes an example nobody runs twice. Raise it with:
    //
    //     OUTRAM_DH_VV_HISTORIES=3000 cargo run --release --example dh_keff_vv
    //
    // Statistics scale as usual: the default resolves a bias of roughly 700 pcm,
    // so a smaller real bias will read as "not resolved" rather than as zero.
    let n_particles: usize = std::env::var("OUTRAM_DH_VV_HISTORIES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(800);
    let settings = KeffSettings {
        n_particles,
        n_inactive: 15,
        n_active: 40,
        temperature_k: 293.6,
        ..KeffSettings::default()
    };

    println!("DH treatment V&V — k-infinity of one FHR pebble, three treatments");
    println!("=================================================================");
    println!("  histories  : {} x [{} inactive + {} active]",
        settings.n_particles, settings.n_inactive, settings.n_active);
    println!("  data       : LOW tier (embedded WMP + fast MGXS), offline");
    println!("  boundary   : reflective at the pebble surface -> k-infinity\n");

    let mut rows = Vec::new();
    for treatment in DhTreatment::ALL {
        let params = PebbleParams::fhr_reference().with_materials(mats.clone());
        let universe = match DhUniverse::pebble(params, treatment) {
            Ok(u) => u,
            Err(e) => {
                println!("  {:<24} BUILD FAILED: {e}", treatment.name());
                continue;
            }
        };
        let n_particles = universe.particle_count();
        let t0 = Instant::now();
        let result = universe.keff(&nucs, &settings);
        let secs = t0.elapsed().as_secs_f64();
        println!(
            "  {:<24} k = {:.5} +/- {:.5}   {:>7.1} s   {} particles stored",
            treatment.name(),
            result.k_mean,
            result.k_std,
            secs,
            n_particles
        );
        rows.push((treatment, result.k_mean, result.k_std, secs));
    }

    // ── V&V comparison against the exact arm ──────────────────────────────
    let Some(&(_, k_ref, s_ref, t_ref)) = rows.iter().find(|(t, ..)| t.is_exact()) else {
        println!("\n  No exact arm ran — nothing to verify against.");
        return;
    };

    println!("\n=== Bias vs the exact (delta-tracked) treatment ===");
    println!("  Reference: k = {k_ref:.5} +/- {s_ref:.5}  (delta tracking is exact by construction)\n");
    println!("  {:<24} {:>10} {:>12} {:>10} {:>14}", "treatment", "dk [pcm]", "combined sd", "sigma", "verdict");
    println!("  {}", "-".repeat(76));
    for &(t, k, s, _) in &rows {
        if t.is_exact() {
            continue;
        }
        let dk_pcm = (k - k_ref) * 1.0e5;
        let sd_pcm = (s * s + s_ref * s_ref).sqrt() * 1.0e5;
        let n_sigma = dk_pcm / sd_pcm;
        let verdict = if n_sigma.abs() < 2.0 {
            "not resolved"
        } else {
            "RESOLVED bias"
        };
        println!("  {:<24} {:>+10.0} {:>12.0} {:>+10.1} {:>14}", t.name(), dk_pcm, sd_pcm, n_sigma, verdict);
    }

    println!("\n=== Accuracy bought per unit speed ===");
    for &(t, k, _, secs) in &rows {
        let speedup = t_ref / secs;
        let dk_pcm = (k - k_ref) * 1.0e5;
        if t.is_exact() {
            println!("  {:<24} 1.00x   (reference)", t.name());
        } else {
            println!("  {:<24} {:>4.2}x   costs {:+.0} pcm", t.name(), speedup, dk_pcm);
        }
    }

    println!("\n  A treatment is worth taking only if its bias is small against the");
    println!("  tolerance of the question being asked. A bias that is 'not resolved'");
    println!("  here means this run could not measure it -- NOT that it is zero.");
}
