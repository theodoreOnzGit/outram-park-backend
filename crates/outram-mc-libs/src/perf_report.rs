//! Per-machine performance-report generator — the self-service "what performance
//! is available on my PC" answer for the Monte Carlo compute backends.
//!
//! Benchmark timings differ from machine to machine, so rather than treating one
//! development box's static numbers as authoritative, this module **detects the
//! host hardware and renders a fresh markdown report from the timings measured on
//! *that* machine**. The machine-specific output is written to a **gitignored**
//! local path ([`write_local_report`]) so each user's numbers stay local and
//! never commit; only methodology / template docs are committed.
//!
//! ## What belongs here
//! - [`HardwareInfo`] — detect this host's GPU adapter (name + backend), CPU
//!   logical-core count, and OS.
//! - [`PerfReport`] / [`PerfRow`] — accumulate measured `(batch size, backend)`
//!   wall-clock + eigenvalue rows and render them to markdown or CSV, including
//!   the GPU-vs-`CpuMultiThread` speedup and an honest crossover verdict.
//! - [`write_local_report`] — persist a rendered report under the crate's
//!   gitignored `verification_and_validation/local_perf/` directory.
//!
//! ## What does NOT belong here
//! The transport physics or the benchmark loop itself (that lives in the relevant
//! `physics` / example driver); this module only *describes* measured results.
//! It is pure formatting + host introspection — no RNG, no transport.
//!
//! Builds on every target, Android included: GPU detection is target-gated so on
//! Android (no `wgpu`) the report simply records "CPU only".

// UNTRUSTED AI-DRAFT: drafted with AI assistance; verified by the unit tests in
// `mod tests`, but still requires human review before it is trusted.

use std::path::PathBuf;

/// A snapshot of the host machine's compute-relevant hardware, for the report
/// header. All fields are best-effort host introspection.
#[derive(Debug, Clone)]
pub struct HardwareInfo {
    /// GPU adapter as `"<name> / <backend>"` (e.g. `"NVIDIA GeForce RTX 3050 /
    /// Vulkan"`), or `None` when no usable GPU adapter is present (headless
    /// server, CI with no loader, or Android — where the report reads
    /// "CPU only").
    pub gpu: Option<String>,
    /// Logical CPU cores via [`std::thread::available_parallelism`] (the count
    /// [`crate::physics::compute::ThreadCount::Auto`] resolves to); `1` if the
    /// query fails.
    pub cpu_logical_cores: usize,
    /// Target OS string from [`std::env::consts::OS`] (e.g. `"linux"`,
    /// `"windows"`, `"macos"`, `"android"`).
    pub os: String,
}

/// Detect the GPU adapter (desktop path): probe `wgpu` and format the adapter
/// name + backend. Creating a probe device is cheap and headless.
#[cfg(not(target_os = "android"))]
fn detect_gpu() -> Option<String> {
    crate::gpu::probe().map(|ctx| format!("{} / {:?}", ctx.info.name, ctx.info.backend))
}

/// Android has no `wgpu` backend compiled in, so there is never a GPU adapter to
/// report — always `None` (the report then reads "CPU only").
#[cfg(target_os = "android")]
fn detect_gpu() -> Option<String> {
    None
}

impl HardwareInfo {
    /// Introspect the current host: GPU adapter (if any), logical CPU cores, OS.
    ///
    /// This may create a transient `wgpu` device to read the adapter name; it is
    /// released immediately. On a machine with no GPU it returns `gpu: None`
    /// without error.
    pub fn detect() -> Self {
        let cpu_logical_cores =
            std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1);
        Self { gpu: detect_gpu(), cpu_logical_cores, os: std::env::consts::OS.to_string() }
    }

    /// A one-line hardware headline, e.g.
    /// `"NVIDIA GeForce RTX 3050 / Vulkan, 12 cores, linux"` or
    /// `"CPU only, 8 cores, android"`.
    pub fn headline(&self) -> String {
        let gpu = self.gpu.as_deref().unwrap_or("CPU only");
        format!("{gpu}, {} cores, {}", self.cpu_logical_cores, self.os)
    }
}

/// One measured timing row: a full benchmark run of `backend` at a given batch
/// size. Wall-clock in seconds; `k_mean`/`k_std` the eigenvalue and its 1-sigma
/// standard error; `histories_total` the total neutron histories transported
/// (`batch_size * generations`) used for the throughput figure.
#[derive(Debug, Clone)]
pub struct PerfRow {
    /// Neutron histories per generation for this run.
    pub batch_size: usize,
    /// Compute backend name, e.g. `"CpuSingleThread"`, `"CpuMultiThread"`, `"Gpu"`.
    pub backend: String,
    /// Measured wall-clock of the full `run_keff`, seconds.
    pub wall_time_s: f64,
    /// Reported mean eigenvalue.
    pub k_mean: f64,
    /// 1-sigma standard error of `k_mean`.
    pub k_std: f64,
    /// Total histories transported over the whole run (`batch_size * n_gen`),
    /// the numerator of the histories/second throughput.
    pub histories_total: usize,
}

impl PerfRow {
    /// Throughput in **histories per second** (`histories_total / wall_time_s`);
    /// `0.0` for a non-positive wall time.
    pub fn throughput_hps(&self) -> f64 {
        if self.wall_time_s > 0.0 {
            self.histories_total as f64 / self.wall_time_s
        } else {
            0.0
        }
    }
}

/// An accumulated set of benchmark rows plus the host hardware, renderable to a
/// per-machine markdown report or CSV.
///
/// Build it with [`PerfReport::new`], [`push`](PerfReport::push) each measured
/// [`PerfRow`], then [`render_markdown`](PerfReport::render_markdown) /
/// [`to_csv`](PerfReport::to_csv). The markdown includes a hardware headline, a
/// per-backend timing + throughput table, the GPU-vs-`CpuMultiThread` speedup at
/// each batch size, and an honest crossover verdict.
#[derive(Debug, Clone)]
pub struct PerfReport {
    /// Free-text title shown at the top of the report.
    pub title: String,
    /// The host this report was generated on.
    pub hardware: HardwareInfo,
    /// The measured rows, in insertion order.
    pub rows: Vec<PerfRow>,
    /// ISO date (`YYYY-MM-DD`) the run was taken; free-text, caller-supplied.
    pub date: String,
}

impl PerfReport {
    /// Start an empty report for `title`, taken on `date` (`YYYY-MM-DD`), with the
    /// host hardware detected now via [`HardwareInfo::detect`].
    pub fn new(title: impl Into<String>, date: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            hardware: HardwareInfo::detect(),
            rows: Vec::new(),
            date: date.into(),
        }
    }

    /// Append one measured [`PerfRow`].
    pub fn push(&mut self, row: PerfRow) {
        self.rows.push(row);
    }

    /// The distinct batch sizes present, ascending.
    fn batch_sizes(&self) -> Vec<usize> {
        let mut v: Vec<usize> = self.rows.iter().map(|r| r.batch_size).collect();
        v.sort_unstable();
        v.dedup();
        v
    }

    /// The wall time for `backend` at `batch_size`, if measured.
    fn time_of(&self, batch_size: usize, backend: &str) -> Option<f64> {
        self.rows
            .iter()
            .find(|r| r.batch_size == batch_size && r.backend == backend)
            .map(|r| r.wall_time_s)
    }

    /// Render the machine-specific report to GitHub-flavoured markdown.
    ///
    /// Sections: a hardware headline ("Performance on your system: …"), a
    /// per-row timing + throughput + eigenvalue table, a GPU-vs-`CpuMultiThread`
    /// speedup column per batch size, and a crossover verdict (the smallest batch
    /// size at which `Gpu` beats `CpuMultiThread`, or a plain statement that there
    /// is no crossover on this machine). The `CpuSingleThread` `f64` path is noted
    /// as the trusted reference; GPU numbers are `f32` acceleration only.
    pub fn render_markdown(&self) -> String {
        let mut s = String::new();
        s.push_str(&format!("# {}\n\n", self.title));
        s.push_str(&format!(
            "> Auto-generated per-machine performance report ({}). Your hardware's \
             numbers will differ from any other machine's — this file is local and \
             gitignored.\n\n",
            self.date
        ));
        s.push_str(&format!("**Performance on your system:** {}\n\n", self.hardware.headline()));

        // Timing / throughput / eigenvalue table.
        s.push_str("## Measured runs\n\n");
        s.push_str("| Histories/gen | Backend | Wall time (s) | Throughput (hist/s) | k_mean | k_std |\n");
        s.push_str("|---|---|---:|---:|---:|---:|\n");
        for r in &self.rows {
            s.push_str(&format!(
                "| {} | {} | {:.4} | {:.3e} | {:.5} | {:.5} |\n",
                r.batch_size, r.backend, r.wall_time_s, r.throughput_hps(), r.k_mean, r.k_std
            ));
        }
        s.push('\n');

        // GPU vs CpuMultiThread speedup + crossover.
        s.push_str("## Gpu vs CpuMultiThread\n\n");
        s.push_str("Speedup = CpuMultiThread wall time / Gpu wall time (> 1 means the GPU is faster).\n\n");
        s.push_str("| Histories/gen | CpuMultiThread (s) | Gpu (s) | Speedup | GPU faster? |\n");
        s.push_str("|---|---:|---:|---:|---|\n");
        let mut crossover: Option<usize> = None;
        for bs in self.batch_sizes() {
            let (Some(multi), Some(gpu)) = (self.time_of(bs, "CpuMultiThread"), self.time_of(bs, "Gpu"))
            else {
                continue;
            };
            let speedup = if gpu > 0.0 { multi / gpu } else { 0.0 };
            let faster = speedup > 1.0;
            if faster && crossover.is_none() {
                crossover = Some(bs);
            }
            s.push_str(&format!(
                "| {} | {:.4} | {:.4} | {:.2}x | {} |\n",
                bs,
                multi,
                gpu,
                speedup,
                if faster { "yes" } else { "no" }
            ));
        }
        s.push('\n');

        s.push_str("### Crossover verdict\n\n");
        match crossover {
            Some(bs) => s.push_str(&format!(
                "On this machine the batched **Gpu** path first beats \
                 **CpuMultiThread** at **{bs} histories/gen** and above.\n\n"
            )),
            None => s.push_str(
                "On this machine the batched **Gpu** path does **not** beat \
                 **CpuMultiThread** at any measured batch size — there is no \
                 crossover. The event-based path round-trips the CPU/GPU boundary \
                 once per event (collision physics stays on the CPU), so it is \
                 memory-transfer / launch bound rather than compute bound.\n\n",
            ),
        }

        s.push_str(
            "> The `CpuSingleThread` `f64` path is the trusted, bit-reproducible \
             reference. `CpuMultiThread` and `Gpu` are acceleration paths validated \
             against it within combined statistical uncertainty; the `Gpu` path is \
             `f32` and not bit-identical to the reference.\n",
        );
        s
    }

    /// Render the rows to CSV (same columns as the committed methodology doc
    /// documents), for plotting the timing-vs-batch-size curve locally.
    pub fn to_csv(&self) -> String {
        let mut s = String::new();
        s.push_str("date,gpu,cpu_cores,os,batch_size,backend,wall_time_s,throughput_hps,k_mean,k_std,histories_total\n");
        let gpu = self.hardware.gpu.as_deref().unwrap_or("CPU only");
        for r in &self.rows {
            s.push_str(&format!(
                "{},{},{},{},{},{},{:.4},{:.4},{:.6},{:.6},{}\n",
                self.date,
                gpu,
                self.hardware.cpu_logical_cores,
                self.hardware.os,
                r.batch_size,
                r.backend,
                r.wall_time_s,
                r.throughput_hps(),
                r.k_mean,
                r.k_std,
                r.histories_total
            ));
        }
        s
    }
}

/// The crate-local, **gitignored** directory machine-specific reports are written
/// to: `verification_and_validation/local_perf/` (relative to the crate root, the
/// usual working directory for `cargo test`/example runs).
pub const LOCAL_PERF_DIR: &str = "verification_and_validation/local_perf";

/// Write `content` to `LOCAL_PERF_DIR/<filename>`, creating the directory if
/// needed, and return the path written. This directory is gitignored so each
/// user's machine-specific numbers stay local.
///
/// Returns any [`std::io::Error`] from creating the directory or file (e.g. the
/// process has no write access to the crate tree — harmless for a benchmark).
pub fn write_local_report(filename: &str, content: &str) -> std::io::Result<PathBuf> {
    let dir = PathBuf::from(LOCAL_PERF_DIR);
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(filename);
    std::fs::write(&path, content)?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// V&V (formatting/logic): the report renders a hardware headline, a row
    /// table, and a correct GPU-vs-multi crossover verdict from synthetic rows.
    ///
    /// **Methodology.** Build a [`PerfReport`] with hand-set rows where `Gpu` is
    /// slower than `CpuMultiThread` at small batch sizes but faster at the largest
    /// (a synthetic crossover), render markdown, and assert: the headline appears,
    /// every backend/row appears, and the crossover verdict names the correct
    /// batch size. A second report with `Gpu` slower everywhere must render the
    /// explicit "no crossover" verdict. This checks the reporting logic
    /// independent of any real hardware timing.
    ///
    /// **Results (2026-07-17).** Both reports render as specified: the crossover
    /// report names the first batch size where `Gpu` wins; the no-crossover report
    /// emits the "does not beat" statement. Throughput = histories/wall_time is
    /// exercised via `throughput_hps`.
    #[test]
    fn report_renders_and_finds_crossover() {
        let mut rep = PerfReport::new("Test report", "2026-07-17");
        // Force a deterministic hardware line so the assertion is host-independent.
        rep.hardware = HardwareInfo {
            gpu: Some("Test GPU / Vulkan".into()),
            cpu_logical_cores: 8,
            os: "linux".into(),
        };
        // Small batch: GPU slower. Large batch: GPU faster (synthetic crossover).
        for (bs, multi, gpu) in [(1_000usize, 0.10f64, 1.00f64), (1_000_000, 5.00, 2.00)] {
            rep.push(PerfRow {
                batch_size: bs,
                backend: "CpuMultiThread".into(),
                wall_time_s: multi,
                k_mean: 1.0,
                k_std: 0.01,
                histories_total: bs * 10,
            });
            rep.push(PerfRow {
                batch_size: bs,
                backend: "Gpu".into(),
                wall_time_s: gpu,
                k_mean: 1.0,
                k_std: 0.01,
                histories_total: bs * 10,
            });
        }
        let md = rep.render_markdown();
        assert!(md.contains("Test GPU / Vulkan, 8 cores, linux"), "headline missing:\n{md}");
        assert!(md.contains("CpuMultiThread") && md.contains("Gpu"), "backends missing");
        assert!(md.contains("first beats") && md.contains("1000000"), "crossover verdict wrong:\n{md}");

        // A no-crossover report: GPU slower at every size.
        let mut rep2 = PerfReport::new("No crossover", "2026-07-17");
        rep2.hardware = HardwareInfo { gpu: None, cpu_logical_cores: 4, os: "linux".into() };
        rep2.push(PerfRow { batch_size: 1000, backend: "CpuMultiThread".into(), wall_time_s: 0.1, k_mean: 1.0, k_std: 0.0, histories_total: 10_000 });
        rep2.push(PerfRow { batch_size: 1000, backend: "Gpu".into(), wall_time_s: 1.0, k_mean: 1.0, k_std: 0.0, histories_total: 10_000 });
        let md2 = rep2.render_markdown();
        assert!(md2.contains("does **not** beat"), "no-crossover verdict missing:\n{md2}");
        assert!(md2.contains("CPU only, 4 cores, linux"), "CPU-only headline missing");
    }

    /// V&V: throughput is histories/second and CSV carries the hardware + rows.
    #[test]
    fn throughput_and_csv() {
        let row = PerfRow {
            batch_size: 1000,
            backend: "Gpu".into(),
            wall_time_s: 2.0,
            k_mean: 1.0,
            k_std: 0.0,
            histories_total: 10_000,
        };
        assert!((row.throughput_hps() - 5000.0).abs() < 1e-9, "throughput = hist/s");

        let mut rep = PerfReport::new("csv", "2026-07-17");
        rep.hardware = HardwareInfo { gpu: None, cpu_logical_cores: 2, os: "linux".into() };
        rep.push(row);
        let csv = rep.to_csv();
        assert!(csv.lines().next().unwrap().starts_with("date,gpu,cpu_cores,os"), "csv header");
        assert!(csv.contains("CPU only,2,linux"), "csv hardware row: {csv}");
    }
}
