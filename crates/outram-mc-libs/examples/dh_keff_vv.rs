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
//! # Data — HIGH tier, ENDF/B-VIII.0
//!
//! Continuous-energy cross sections reconstructed from the ENDF/B-VIII.0 tapes
//! in `reference-data/endf/` (RECONR + BROADR on device, 1e-3 tolerance), plus
//! the **ENDF/B-VIII.0 crystalline-graphite S(alpha,beta)** law on the moderator
//! carbon.
//!
//! The bound-carbon law is not optional for this problem. A pebble is
//! graphite-moderated and the spectrum is thermal, so below ~4 eV neutrons
//! scatter off the bound lattice — coherent Bragg plus incoherent inelastic —
//! not off a free carbon atom. This crate's own measurement is that omitting it
//! puts a graphite-moderated spectrum **~1700 pcm too high**
//! (`tests/htr10_graphite_thermal_scattering`). Running this V&V on free-gas
//! carbon would put a known 1700 pcm error into every arm.
//!
//! An earlier revision of this example used the LOW embedded tier
//! (`Nuclide::from_core`: coarse group data above the WMP range, Watt
//! fission-birth stand-in, no S(alpha,beta)). That is fine for a *timing*
//! comparison but is not defensible as V&V evidence, and it has been replaced.
//!
//! Setup cost: reconstructing six nuclides takes a few minutes before any
//! transport starts, and it is reported separately from the per-treatment
//! timings below.
//!
//! # Results — SUPERSEDED, RE-RUN IN FLIGHT
//!
//! **Do not cite the table below.** It was measured on the LOW embedded tier,
//! which this example no longer uses: coarse group data above the WMP range, a
//! Watt fission-birth stand-in in place of real MF=5 chi (worth ~500 pcm on
//! Godiva by this crate's own measurement), and no S(alpha,beta) at all
//! (~1700 pcm on a graphite-moderated spectrum). Those three omissions are
//! larger than some of the biases the table reports, so it cannot support the
//! conclusions drawn from it.
//!
//! Retained verbatim rather than deleted because it is the last measurement
//! actually taken, and because the *ranking* finding below — that both
//! approximations are slower for eigenvalues than the geometry benchmark
//! predicts — is about cost, not data, and is expected to survive. That
//! expectation is **not yet confirmed on HIGH tier.**
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
//! ## The accuracy result — LOW tier, superseded
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
use outram_mc_libs::material::thermal::ThermalScattering;
use njoy_outram_park_fork::reference_data::reference_endf;

/// Representative HALEU UCO TRISO compositions \[atoms/b-cm\], room temperature.
///
/// Illustrative of the material class, **not** a benchmark specification — this
/// example measures the *difference between treatments*, which is insensitive to
/// the exact densities so long as all three arms share them.
///
/// # Carbon bookkeeping
///
/// Two carbon entries, deliberately:
///
/// - **`C12` free gas** — the kernel's oxycarbide carbon and the SiC carbon.
///   Neither sits in a graphite lattice.
/// - **`C12G` graphite-bound** — buffer, IPyC, OPyC, fuel-zone matrix and the
///   outer shell, all of which are graphite and carry the S(alpha,beta) law.
///
/// C-13 (1.1 % of natural carbon) is neglected. That is a stated approximation:
/// it applies identically to all three arms, so it cannot bias the comparison
/// this example exists to make, though it would matter for an absolute k.
fn materials() -> Vec<Material> {
    // Nuclide indices into the vector returned by `nuclides()`.
    const U235: usize = 0;
    const U238: usize = 1;
    const O16: usize = 2;
    const C_FREE: usize = 3;
    const C_GRAPHITE: usize = 4;
    const SI28: usize = 5;

    let m = |id: i32, name: &str, comps: Vec<(usize, f64)>| Material {
        id,
        name: name.into(),
        temperature: TEMP_K,
        components: comps
            .into_iter()
            .map(|(nuclide_idx, atom_density)| NuclideComponent {
                nuclide_idx,
                atom_density,
            })
            .collect(),
    };

    vec![
        // 0 kernel — 19.9 % HALEU UCO. Kernel carbon is not graphite.
        m(0, "UCO kernel", vec![
            (U235, 4.40e-3), (U238, 1.77e-2), (O16, 2.27e-2), (C_FREE, 9.10e-3),
        ]),
        // 1 buffer — porous graphite
        m(1, "buffer", vec![(C_GRAPHITE, 5.02e-2)]),
        // 2 IPyC — pyrolytic graphite
        m(2, "IPyC", vec![(C_GRAPHITE, 9.53e-2)]),
        // 3 SiC — carbon here is silicon-bound, not graphite
        m(3, "SiC", vec![(SI28, 4.79e-2), (C_FREE, 4.79e-2)]),
        // 4 OPyC — pyrolytic graphite
        m(4, "OPyC", vec![(C_GRAPHITE, 9.53e-2)]),
        // 5 matrix — graphite binder in the fuel zone
        m(5, "matrix graphite", vec![(C_GRAPHITE, 8.53e-2)]),
        // 6 shell — fuel-free graphite outer shell
        m(6, "shell graphite", vec![(C_GRAPHITE, 8.78e-2)]),
    ]
}

/// Material / data temperature \[K\].
const TEMP_K: f64 = 293.6;

/// Reconstruct the six nuclides from the ENDF/B-VIII.0 reference tapes.
///
/// Index order must match the constants in [`materials`].
fn nuclides() -> Vec<Nuclide> {
    let load = |name: &str, file: &str| -> Nuclide {
        let path = reference_endf(file)
            .unwrap_or_else(|| panic!("missing reference tape {file} in reference-data/endf/"));
        eprint!("  reconstructing {name:<6} from {file} … ");
        let t0 = Instant::now();
        let n = Nuclide::from_endf_file(&path, name, TEMP_K, 1.0e-3)
            .unwrap_or_else(|e| panic!("from_endf_file({}): {e}", path.display()));
        eprintln!("{:.1?}", t0.elapsed());
        n
    };

    let sab = ThermalScattering::from_endf_file(
        reference_endf("tsl-crystalline-graphite.endf")
            .expect("missing tsl-crystalline-graphite.endf")
            .to_str()
            .expect("valid UTF-8 path"),
        30, // MAT 30 — C in crystalline graphite (ENDF/B-VIII.0)
        TEMP_K,
        "c_Graphite",
    )
    .expect("crystalline-graphite S(alpha,beta)");
    eprintln!("  graphite S(alpha,beta): ENDF/B-VIII.0 tsl-crystalline-graphite … ok");

    vec![
        load("U235", "n-092_U_235-ENDF8.0.endf"),
        load("U238", "n-092_U_238.endf"),
        load("O16", "n-008_O_016-ENDF8.0.endf"),
        // free-gas carbon: kernel oxycarbide and SiC
        load("C12", "n-006_C_012-ENDF8.0.endf"),
        // graphite-bound carbon: buffer, PyC, matrix, shell
        load("C12", "n-006_C_012-ENDF8.0.endf").with_thermal_scattering(sab),
        load("Si28", "n-014_Si_028-ENDF8.0.endf"),
    ]
}

fn main() {
    println!("Reconstructing ENDF/B-VIII.0 cross sections (this is setup, not transport):");
    let t_data = Instant::now();
    let nucs = nuclides();
    let data_secs = t_data.elapsed().as_secs_f64();
    println!("  data ready in {data_secs:.1} s\n");
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
        temperature_k: TEMP_K,
        ..KeffSettings::default()
    };

    println!("DH treatment V&V — k-infinity of one FHR pebble, three treatments");
    println!("=================================================================");
    println!("  histories  : {} x [{} inactive + {} active]",
        settings.n_particles, settings.n_inactive, settings.n_active);
    println!("  data       : HIGH tier, ENDF/B-VIII.0 + crystalline-graphite S(a,b)");
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
