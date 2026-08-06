//! Random-packed TRISO k∞ on **HIGH-fidelity net-fetched ENDF data**, with a
//! CPU-vs-GPU cross-section-kernel throughput benchmark.
//!
//! Run it (needs the `net-fetch` feature and a network connection on first run):
//! ```text
//! cargo run --release -p outram-mc-libs --features net-fetch --example triso_gpu_benchmark
//! ```
//!
//! # What this is, and how it differs from `triso_delta_tracking`
//!
//! This is the **HIGH-tier** sibling of [`triso_delta_tracking`](triso_delta_tracking),
//! which runs the same doubly-heterogeneous delta-tracking k∞ on **LOW-tier**
//! offline data (`Nuclide::from_core`, embedded WMP + fast MGXS). The transport
//! machinery is identical — random-packed HEU kernels in a reflective cube,
//! streamed by delta (Woodcock) tracking — but two things change here:
//!
//! 1. **The cross sections are HIGH-fidelity.** Every nuclide is reconstructed on
//!    device from raw ENDF/B-VII.1 tapes via [`Nuclide::from_endf`] (RECONR full
//!    resonance reconstruction at 0.1 % tolerance + BROADR Doppler to 293.6 K +
//!    energy-dependent ν̄ from MF=1/452 + MF=5 χ), instead of the LOW tier's
//!    group-collapsed WMP data. This is continuous-energy pointwise σ(E) with
//!    implicit self-shielding.
//! 2. **A distinct geometry.** To make this a genuinely different case (not a
//!    re-run of the LOW example) the packing is coarser and the kernel larger:
//!    packing fraction 0.25, kernel radius 0.05 cm, seed 20260717, in a 1.2 cm
//!    reflective cube. The LOW example uses pf 0.30, r 0.04 cm, seed 20240715, in
//!    a 1.0 cm cube.
//!
//! On top of the k∞ it runs an **isolated GPU acceleration benchmark** of the
//! batched macroscopic-total cross-section lookup kernel
//! ([`UnionTotalXs`](outram_mc_libs::gpu::union_grid::UnionTotalXs)) — CPU `f64`
//! reference vs GPU `f32` — on the HEU fuel material, at several batch sizes.
//!
//! # Honesty banner — what is and is NOT GPU-accelerated
//!
//! **The k∞ is computed entirely on the CPU in `f64`.** End-to-end GPU neutron
//! transport is *not* wired yet (that is beads op-u6s.4); the delta-tracking power
//! iteration is the trusted CPU reference. The GPU is exercised **only** on the
//! embarrassingly-parallel cross-section-interpolation sub-kernel, and its `f32`
//! results are judged *against* the CPU `f64` reference within a tolerance — never
//! trusted as the reference and never used to produce a k∞. **There is no GPU k∞
//! in this example, by design; fabricating one would be dishonest.** What is
//! measured here is (a) the CPU k∞ baseline on HIGH-fidelity ENDF data, and (b)
//! the isolated XS-kernel GPU throughput vs CPU.
//!
//! # Data provenance
//!
//! - **Geometry / material model:** the OpenMC `triso.ipynb` notebook
//!   (openmc-dev/openmc-notebooks) @ commit `cf1e5db` — random-packed TRISO fuel
//!   kernels in a moderator matrix, HEU kernel composition. MIT-licensed OpenMC
//!   project material; used here as a modelling reference, adapted to a tractable
//!   unit cell. Cited per `RESPONSIBLE_USE.md`.
//! - **Cross-section data:** ENDF/B-VII.1 evaluated neutron sublibrary, fetched
//!   from the IAEA Nuclear Data Services (NDS) `download-endf` mirror
//!   (`https://www-nds.iaea.org/public/download-endf`), **accessed 2026-07-17**,
//!   reconstructed on device by this workspace's RECONR + BROADR port. VII.1 (not
//!   VIII.0) is used because its U resonances are Reich-Moore (LRF=3), which the
//!   port reconstructs; VIII.0 U is LRF=7 (not yet ported).
//! - **Which nuclides are HIGH-fidelity ENDF vs any fallback:** U-234, U-235,
//!   U-238 **and** the H-1 moderator are all reconstructed from ENDF/B-VII.1 —
//!   **all four are HIGH-fidelity.** Graphite carbon was the preferred moderator
//!   (the notebook uses a carbonaceous matrix), but carbon (C-12 / C-nat) is
//!   **not** in the port's built-in ENDF MAT table (`well_known_mat` ships only
//!   H/O/Fe/Th/U/Pu), so `Nuclide::from_endf("C12", …)` cannot address it by name
//!   without an explicit-MAT path that this example does not add. The moderator
//!   therefore **falls back to hydrogen H-1** — itself still HIGH-fidelity ENDF,
//!   just a lighter, more strongly-moderating nuclide than graphite would be. The
//!   H-1 unit-cell k∞ is thus not a graphite-TRISO physics result; it is a
//!   HIGH-fidelity delta-tracking demonstration on an H-moderated HEU dispersion.
//!
//! # V&V — methodology and results
//!
//! **Methodology.**
//! - *k∞ baseline:* random-pack HEU kernels (pf 0.25, r 0.05 cm, seed 20260717)
//!   into a reflective 1.2 cm cube; build a bin-maximum majorant (4096 bins × 32
//!   subsamples, 10 % margin) that provably bounds Σ_t across the U resonances;
//!   run a delta-tracking fission-source power iteration (2000 histories ×
//!   [25 inactive + 75 active]) on the HIGH-fidelity ENDF nuclides. Report
//!   k∞ ± σ, wall-clock, and source-history throughput. There is no external
//!   benchmark k for this *specific* unit cell (it is not a standard case); the
//!   pass criterion is a converged, stationary, physical eigenvalue (k∞ ≫ 1 for a
//!   near-unmoderated fissile HEU medium, finite σ), and the k∞ is reported as a
//!   measured baseline, not a validated benchmark match.
//! - *XS-kernel GPU throughput:* tabulate the HEU fuel macroscopic total Σ_t on
//!   4096 log-spaced points (1e-3 .. 2e7 eV); form batches of representative query
//!   energies by Watt-spectrum sampling at 2^16, 2^18, 2^20; time `lookup_cpu`
//!   (`f64`) vs `lookup_gpu` (`f32`, warm-up dispatch first, host-visible time
//!   incl. upload + readback). Report queries/s each side, GPU speedup, and
//!   max/mean |gpu_f32 − cpu_f64| over Σ_t \[cm⁻¹\]. GPU is judged solely against
//!   the CPU reference; if no GPU adapter is present the GPU columns are skipped.
//!
//! **Results (2026-07-17, NVIDIA GeForce RTX 3050, Vulkan; warm OS disk cache).**
//!
//! > **SUPERSEDED AND PENDING A RE-RUN (flagged 2026-08-06, bead `op-jis`).**
//! > The k∞ below, and the XS-kernel error columns further down, were measured
//! > with the pre-`op-jis` `prn` output function (the raw top-52 bits of the LCG
//! > state). `op-jis` replaced it with OpenMC's PCG-RXS-M-XS output permutation,
//! > which changes every uniform: the eigenvalue, the sampled kernel positions,
//! > and the Watt-sampled query energies behind the error columns all move. This
//! > example **could not be re-run** on 2026-08-06 — it needs `--features
//! > net-fetch` to download the ENDF tapes and no local cache was present, so
//! > **no number below has been replaced or estimated**. The two tracked CSVs it
//! > writes, `verification_and_validation/gpu_benchmarks/triso_keff_convergence.csv`
//! > and `triso_xs_throughput.csv`, therefore also still hold pre-`op-jis`
//! > values. Re-run with
//! > `cargo run --release -p outram-mc-libs --features net-fetch --example triso_gpu_benchmark`
//! > before citing any of this as current. (The sibling `godiva_gpu_benchmark`,
//! > which needs no network data, *was* re-run and is current.)
//!
//! - **Nuclide fidelity:** U-234, U-235, U-238, H-1 all HIGH-fidelity ENDF/B-VII.1
//!   (carbon fell back to H-1 as documented above). Reconstruction wall-clock:
//!   U-234 0.4 s, U-235 12.4 s, U-238 28.4 s, H-1 0.9 s; total 42.1 s (tapes were
//!   already cached and the processed-cache warm; a cold first run downloads +
//!   reconstructs and is slower).
//! - **k∞ baseline (HIGH-fidelity ENDF, H-1 moderator):**
//!   **k∞ = 1.86062 ± 0.00268** over 100 generations of delta tracking; packed
//!   825 HEU kernels (realized pf 0.2500) into the 1.2 cm reflective cube.
//!   Transport wall-clock 15.3 s; source-history throughput =
//!   2000 × (25 + 75) / 15.3 s = **1.311 × 10⁴ histories/s**. Interpretation: a
//!   large, stationary, physical k∞ ≫ 1 — an HEU dispersion in an H-1 matrix is
//!   strongly supercritical at infinite-medium (no leakage), consistent with a
//!   hard-spectrum fissile cell; the eigenvalue converged and its
//!   active-generation spread is ~270 pcm (1σ). The per-generation trace (in the
//!   convergence CSV) is stationary from the first active generation.
//! - **XS-kernel GPU vs CPU throughput** (HEU fuel Σ_t, Watt-sampled queries):
//!
//! | batch | CPU Mq/s | GPU Mq/s | GPU speedup | max\|Δ\| cm⁻¹ | mean\|Δ\| cm⁻¹ |
//! |---|---|---|---|---|---|
//! | 65 536   | 30.5 | 57.3  | 1.88× | 1.374e-6 | 9.878e-9 |
//! | 262 144  | 32.3 | 186.7 | 5.77× | 1.104e-5 | 9.967e-9 |
//! | 1 048 576| 28.5 | 171.1 | 6.01× | 2.082e-5 | 9.965e-9 |
//!
//!   Interpretation: the GPU `f32` lookup reproduces the CPU `f64` reference to
//!   single-precision rounding — mean |Δ| ≈ 1e-8 cm⁻¹ and max |Δ| growing from
//!   1.4e-6 to 2.1e-5 cm⁻¹ as the larger batches sample more steep resonance
//!   flanks, all negligible against Σ_t values of O(0.1–40 cm⁻¹) (≲1e-6 relative)
//!   — so it is a sound accelerator. On this hardware the GPU **beat the CPU at
//!   every batch size tested**, including 2^16 (1.88×), and the speedup **grew
//!   with batch size and plateaued around ~6× by 2^18–2^20**: the fixed
//!   host-visible overhead (buffer upload + kernel launch + readback) is amortised
//!   as the batch grows, while the single-threaded `f64` CPU lookup holds a flat
//!   ~30 Mq/s. (End-to-end GPU transport, which would keep data resident and avoid
//!   per-batch transfers, is op-u6s.4 — not measured here.)
//!
//! These numbers are hardware- and run-dependent (a 6 GB laptop-class RTX 3050,
//! Vulkan backend, warm OS disk cache). CPU is `f64`, single-threaded lookup; GPU
//! is `f32`. Re-running regenerates the two CSVs named below.

#[cfg(not(feature = "net-fetch"))]
fn main() {
    eprintln!(
        "this example needs the `net-fetch` feature:\n  \
         cargo run --release -p outram-mc-libs --features net-fetch --example triso_gpu_benchmark"
    );
    std::process::exit(2);
}

#[cfg(feature = "net-fetch")]
fn main() {
    use njoy_outram_park_fork::acquire::EndfLibrary;
    use outram_mc_libs::gpu::union_grid::UnionTotalXs;
    use outram_mc_libs::geometry::position::Position;
    use outram_mc_libs::material::material::{Material, NuclideComponent};
    use outram_mc_libs::material::nuclide::Nuclide;
    use outram_mc_libs::pebble_beds::delta_tracking::Majorant;
    use outram_mc_libs::pebble_beds::keff_delta::run_keff_delta;
    use outram_mc_libs::pebble_beds::sphere_packing::PackedSpheres;
    use outram_mc_libs::physics::keff::KeffSettings;
    use outram_mc_libs::rng::distributions::watt;
    use std::fs;
    use std::io::Write;
    use std::path::PathBuf;
    use std::time::Instant;

    let temp_k = 293.6_f64;
    let lib = EndfLibrary::EndfBVII1;

    println!("=====================================================================");
    println!(" TRISO delta-tracking k∞ on HIGH-fidelity net-fetched ENDF/B-VII.1");
    println!(" + isolated CPU-vs-GPU cross-section-kernel throughput benchmark");
    println!("=====================================================================");
    println!(" Provenance: OpenMC triso.ipynb @ cf1e5db (geometry/material model);");
    println!("             ENDF/B-VII.1 via IAEA NDS, accessed 2026-07-17 (XS data).");
    println!(" HONESTY: k∞ is CPU f64 (trusted). GPU f32 accelerates only the XS");
    println!("          lookup kernel — there is NO GPU k∞ here. End-to-end GPU");
    println!("          transport is pending beads op-u6s.4.");
    println!("---------------------------------------------------------------------\n");

    // ── 1. Reconstruct HIGH-fidelity nuclides from raw ENDF (per-nuclide timing).
    //
    // Moderator note: carbon (graphite) is the notebook's matrix, but C-12/C-nat is
    // not in the port's built-in ENDF MAT table, so `from_endf` cannot address it by
    // name. We fall back to H-1 — still HIGH-fidelity ENDF, just a lighter moderator.
    let names = ["U234", "U235", "U238", "H1"];
    println!(
        "Reconstructing {} isotopes from {} (RECONR 0.1% + BROADR @ {temp_k} K)…",
        names.len(),
        lib.label()
    );
    let t_data = Instant::now();
    let mut nuclides: Vec<Nuclide> = Vec::with_capacity(names.len());
    for name in names.iter() {
        let t = Instant::now();
        match Nuclide::from_endf(lib, name, temp_k, 1.0e-3) {
            Ok(n) => {
                println!("  {name}: reconstructed in {:.1} s", t.elapsed().as_secs_f64());
                nuclides.push(n);
            }
            // Fail gracefully rather than panicking with a backtrace: the usual
            // cause is no outbound network (offline / restrictive proxy). Print
            // an honest explanation and point at the offline CORE-data twin, then
            // exit non-zero cleanly.
            Err(e) => {
                eprintln!(
                    "\ncould not obtain ENDF data for {name} from ENDF/B-VII.1: {e}\n\n\
                     This HIGH-fidelity benchmark downloads the ENDF/B-VII.1 neutron tapes\n\
                     from the IAEA Nuclear Data Services and reconstructs them on device\n\
                     (RECONR + BROADR), so it needs outbound network access to\n\
                     https://www-nds.iaea.org. If you are offline or behind a restrictive\n\
                     proxy, run one of the offline CORE-data TRISO twins instead (embedded\n\
                     WMP data, no network):\n  \
                     cargo run --release -p outram-mc-libs --example triso_delta_tracking\n  \
                     cargo run --release -p outram-mc-libs --example triso_stochastic_media"
                );
                std::process::exit(1);
            }
        }
    }
    let data_secs = t_data.elapsed().as_secs_f64();
    println!("All four nuclides HIGH-fidelity ENDF/B-VII.1; data ready in {data_secs:.1} s.\n");

    // ── 2. Materials: HEU fuel kernel (index 0) + H-1 moderator matrix (index 1).
    // Fuel atom densities are the standard HEU-MET-FAST-001 (Godiva) values, i.e.
    // the same highly-enriched U metal the notebook packs into TRISO kernels.
    let fuel = Material {
        id: 1,
        name: "HEU kernel".into(),
        temperature: temp_k,
        components: vec![
            NuclideComponent { nuclide_idx: 0, atom_density: 4.9184e-4 }, // U-234
            NuclideComponent { nuclide_idx: 1, atom_density: 4.4994e-2 }, // U-235
            NuclideComponent { nuclide_idx: 2, atom_density: 2.4984e-3 }, // U-238
        ],
    };
    let moderator = Material {
        id: 2,
        name: "H-1 matrix".into(),
        temperature: temp_k,
        components: vec![NuclideComponent { nuclide_idx: 3, atom_density: 4.0e-2 }],
    };
    let materials = vec![fuel, moderator];

    // ── 3. Distinct HIGH-tier geometry: coarser packing, larger kernel, new seed.
    let half = 0.6_f64; // 1.2 cm reflective cube
    let radius = 0.05_f64; // kernel radius [cm]
    let pf = 0.25_f64; // target packing fraction
    let seed = 20260717_u64;
    let packed = PackedSpheres::pack(radius, half, pf, seed).expect("RSA packs at pf 0.25");
    println!(
        "Packed {} HEU kernels into a {:.2} cm³ reflective cube — realized pf {:.4}.",
        packed.len(),
        (2.0 * half).powi(3),
        packed.packing_fraction()
    );

    // ── 4. Majorant: a provable Σ_t bound across the U resonances (bin-maximum).
    let majorant = Majorant::bounding(&materials, &nuclides, 1.0e-3, 2.0e7, 4096, 32, 0.1);

    // Geometry lookup for delta tracking: kernel → fuel (0), matrix → moderator (1).
    let material_at = move |p: Position| Some(if packed.is_inside_kernel(p) { 0usize } else { 1usize });

    // ── 5. Delta-tracking fission-source power iteration (CPU f64 reference).
    let settings = KeffSettings {
        n_particles: 2000,
        n_inactive: 25,
        n_active: 75,
        temperature_k: temp_k,
        ..KeffSettings::default()
    };
    println!(
        "\nDelta-tracking k∞  —  {} histories/gen, {} inactive + {} active generations",
        settings.n_particles, settings.n_inactive, settings.n_active
    );
    let t_mc = Instant::now();
    let result = run_keff_delta(half, &materials, &nuclides, &majorant, material_at, &settings);
    let mc_secs = t_mc.elapsed().as_secs_f64();
    let histories = settings.n_particles * (settings.n_inactive + settings.n_active);
    let hist_per_s = histories as f64 / mc_secs;
    println!(
        "  k∞ = {:.5} ± {:.5}   ({} generations, delta tracking)",
        result.k_mean,
        result.k_std,
        result.k_by_generation.len()
    );
    println!("  transport wall-clock: {mc_secs:.1} s");
    println!(
        "  source-history throughput: {histories} histories / {mc_secs:.1} s = {hist_per_s:.3e} histories/s"
    );

    // ── 6. Isolated GPU-vs-CPU XS-kernel throughput on the HEU fuel Σ_t.
    println!("\nXS-kernel throughput benchmark (HEU fuel macroscopic total Σ_t):");
    let table = UnionTotalXs::tabulate(&materials[0], &nuclides, 1.0e-3, 2.0e7, 4096);
    println!("  tabulated Σ_t on {} log-spaced points (1e-3 .. 2e7 eV).", table.grid.len());

    let gpu = outram_mc_libs::gpu::probe();
    match &gpu {
        Some(ctx) => println!("  GPU adapter: {} ({:?}).", ctx.info.name, ctx.info.backend),
        None => println!("  No GPU adapter found — GPU columns skipped (CPU-only path)."),
    }

    // Representative query energies via Watt fission-spectrum sampling.
    let batch_sizes = [1usize << 16, 1usize << 18, 1usize << 20];
    let mut xs_rows: Vec<XsRow> = Vec::with_capacity(batch_sizes.len());
    for &n in &batch_sizes {
        let mut qseed = 0xC0FFEE_u64 ^ (n as u64);
        let queries: Vec<f64> = (0..n)
            .map(|_| watt(&mut qseed, settings.watt_a, settings.watt_b))
            .collect();

        // CPU reference (f64), timed.
        let t = Instant::now();
        let cpu = table.lookup_cpu(&queries);
        let cpu_ms = t.elapsed().as_secs_f64() * 1.0e3;
        let cpu_qps = n as f64 / (cpu_ms / 1.0e3);

        // GPU (f32): warm-up dispatch (discarded) then timed, host-visible.
        let (gpu_ms, gpu_qps, max_abs, mean_abs) = if let Some(ctx) = &gpu {
            let _warm = table.lookup_gpu(ctx, &queries[..queries.len().min(4096)]);
            let t = Instant::now();
            let g = table.lookup_gpu(ctx, &queries);
            let gpu_ms = t.elapsed().as_secs_f64() * 1.0e3;
            let gpu_qps = n as f64 / (gpu_ms / 1.0e3);
            let mut max_abs = 0.0_f64;
            let mut sum_abs = 0.0_f64;
            for (&gv, &cv) in g.iter().zip(cpu.iter()) {
                let d = (gv as f64 - cv).abs();
                max_abs = max_abs.max(d);
                sum_abs += d;
            }
            (gpu_ms, gpu_qps, max_abs, sum_abs / g.len() as f64)
        } else {
            (f64::NAN, f64::NAN, f64::NAN, f64::NAN)
        };

        let speedup = gpu_qps / cpu_qps;
        println!(
            "  batch {n:>9}: CPU {:.1} Mq/s ({cpu_ms:.2} ms) | GPU {:.1} Mq/s ({gpu_ms:.2} ms) | \
             speedup {speedup:.2}× | max|Δ| {max_abs:.3e} | mean|Δ| {mean_abs:.3e} cm⁻¹",
            cpu_qps / 1.0e6,
            gpu_qps / 1.0e6,
        );
        xs_rows.push(XsRow { n, cpu_ms, gpu_ms, cpu_qps, gpu_qps, speedup, max_abs, mean_abs });
    }

    // ── 7. Write the two plottable CSVs.
    let out_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("verification_and_validation")
        .join("gpu_benchmarks");
    fs::create_dir_all(&out_dir).expect("create gpu_benchmarks output dir");

    // 7a. k∞ convergence CSV.
    let conv_path = out_dir.join("triso_keff_convergence.csv");
    {
        let mut f = fs::File::create(&conv_path).expect("create keff convergence CSV");
        writeln!(f, "generation,k_generation,k_cumulative_mean,k_cumulative_sigma,phase").unwrap();
        let mut active: Vec<f64> = Vec::new();
        for (gen, &k) in result.k_by_generation.iter().enumerate() {
            let phase = if gen < settings.n_inactive { "inactive" } else { "active" };
            if gen >= settings.n_inactive {
                active.push(k);
                let (m, s) = mean_and_stderr(&active);
                writeln!(f, "{gen},{k:.6},{m:.6},{s:.6},{phase}").unwrap();
            } else {
                // Inactive generations are discarded from the tally — no cumulative.
                writeln!(f, "{gen},{k:.6},,,{phase}").unwrap();
            }
        }
    }

    // 7b. XS-kernel throughput CSV.
    let xs_path = out_dir.join("triso_xs_throughput.csv");
    {
        let mut f = fs::File::create(&xs_path).expect("create xs throughput CSV");
        writeln!(
            f,
            "batch_size,cpu_ms,gpu_ms,cpu_queries_per_s,gpu_queries_per_s,gpu_speedup,max_abs_err_cm^-1,mean_abs_err_cm^-1"
        )
        .unwrap();
        for r in &xs_rows {
            writeln!(
                f,
                "{},{:.4},{:.4},{:.1},{:.1},{:.4},{:.6e},{:.6e}",
                r.n, r.cpu_ms, r.gpu_ms, r.cpu_qps, r.gpu_qps, r.speedup, r.max_abs, r.mean_abs
            )
            .unwrap();
        }
    }

    println!("\nWrote CSVs:");
    println!("  {}", conv_path.display());
    println!("  {}", xs_path.display());

    // ── 8. Closing honesty banner.
    println!("\n---------------------------------------------------------------------");
    println!(" End-to-end GPU transport is pending beads op-u6s.4.");
    println!(" Measured here: (a) CPU k∞ baseline on HIGH-fidelity ENDF/B-VII.1,");
    println!("                (b) isolated XS-kernel GPU throughput vs CPU (f32 vs f64).");
    println!(" The GPU produced NO k∞ — it is acceleration only, judged against CPU.");
    println!("---------------------------------------------------------------------");
}

/// One row of the XS-kernel throughput table (per batch size).
#[cfg(feature = "net-fetch")]
struct XsRow {
    n: usize,
    cpu_ms: f64,
    gpu_ms: f64,
    cpu_qps: f64,
    gpu_qps: f64,
    speedup: f64,
    max_abs: f64,
    mean_abs: f64,
}

/// Mean and standard error of the mean (1σ) of a slice — matches the driver's own
/// active-generation statistic so the CSV's cumulative column agrees with the
/// reported k∞ at the final generation.
#[cfg(feature = "net-fetch")]
fn mean_and_stderr(k: &[f64]) -> (f64, f64) {
    let n = k.len();
    if n == 0 {
        return (0.0, 0.0);
    }
    let mean = k.iter().sum::<f64>() / n as f64;
    if n < 2 {
        return (mean, 0.0);
    }
    let var = k.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (n as f64 - 1.0);
    (mean, (var / n as f64).sqrt())
}
