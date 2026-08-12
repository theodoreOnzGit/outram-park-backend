//! Per-machine performance-report generator (hardware detection + markdown).
//!
//! # What this is for
//!
//! GPU and CPU timings differ from one computer to the next, so committing one
//! box's benchmark numbers into the repository would be misleading. Instead this
//! module lets a benchmark **generate a fresh markdown report on the machine it
//! runs on** — headed by that machine's hardware — and write it to a
//! **git-ignored** local path. The committed benchmark markdown then documents
//! only the methodology (a template); the actual timings live in each user's
//! local report.
//!
//! It is used by the `gpu_wmp_bench` example (the full-fidelity windowed-multipole
//! GPU-vs-CPU benchmark), but nothing here is WMP-specific — it is a small,
//! reusable formatting helper: pass it a title, a [`HardwareInfo`], and a slice of
//! [`PerfRow`] measurements and it returns a markdown string, or writes that
//! string to a local (git-ignored) directory.
//!
//! # No physics, pure `std`
//!
//! This module does not touch `wgpu` and has no GPU dependency, so it compiles on
//! every target (Android included). The GPU adapter label is passed in as a plain
//! string by the caller (obtained on desktop from
//! [`crate::gpu::GpuContext::adapter_label`]); on a machine with no GPU the caller
//! passes `"CPU only"`.

use std::io;
use std::path::{Path, PathBuf};

/// One measured benchmark point: a grid size and the CPU/GPU timings and
/// agreement observed at that size.
///
/// Units: `n_energy` is a count of incident-energy grid points; `cpu_ms` and
/// `gpu_ms` are wall-clock milliseconds for evaluating the whole grid on the CPU
/// (trusted `f64` reference) and GPU (`f32`) respectively; `speedup` is
/// `cpu_ms / gpu_ms` (dimensionless, `> 1` means the GPU is faster);
/// `max_rel_err` is the maximum relative error of the GPU result versus the CPU
/// reference over the grid (dimensionless).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PerfRow {
    /// Number of incident-energy grid points evaluated.
    pub n_energy: usize,
    /// CPU (trusted `f64`) wall-clock time for the whole grid \[ms\].
    pub cpu_ms: f64,
    /// GPU (`f32`) wall-clock time for the whole grid \[ms\].
    pub gpu_ms: f64,
    /// Speedup `cpu_ms / gpu_ms` (dimensionless; `> 1` ⇒ GPU faster).
    pub speedup: f64,
    /// Max relative error of the GPU result vs the CPU reference (dimensionless).
    pub max_rel_err: f64,
}

/// A description of the machine a benchmark ran on, for the report header.
///
/// `gpu_label` is a human GPU string such as `"NVIDIA GeForce RTX 3050 / Vulkan"`
/// (or `"CPU only"` when no GPU adapter is present); `cpu_cores` is the number of
/// logical CPU cores; `os` is the target operating system (`std::env::consts::OS`,
/// e.g. `"linux"`, `"windows"`, `"macos"`, `"android"`).
#[derive(Debug, Clone, PartialEq)]
pub struct HardwareInfo {
    /// GPU adapter label (name + backend), or `"CPU only"` if there is no GPU.
    pub gpu_label: String,
    /// Number of logical CPU cores.
    pub cpu_cores: usize,
    /// Operating system (`std::env::consts::OS`).
    pub os: String,
}

impl HardwareInfo {
    /// Detect the current machine's CPU-core count and OS, pairing them with a
    /// caller-supplied GPU label.
    ///
    /// The GPU cannot be detected here without a `wgpu` dependency, so the caller
    /// passes it in — on desktop, from [`crate::gpu::GpuContext::adapter_label`];
    /// when no GPU is present, pass `"CPU only"`. CPU cores come from
    /// [`std::thread::available_parallelism`] (falling back to `1` if it cannot be
    /// determined), and the OS from [`std::env::consts::OS`].
    pub fn detect(gpu_label: impl Into<String>) -> Self {
        let cpu_cores = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1);
        Self {
            gpu_label: gpu_label.into(),
            cpu_cores,
            os: std::env::consts::OS.to_string(),
        }
    }

    /// A one-line "Performance on your system: ..." header string naming the
    /// hardware, e.g. `"NVIDIA GeForce RTX 3050 / Vulkan, 12 cores, linux"`.
    pub fn summary_line(&self) -> String {
        format!("{}, {} cores, {}", self.gpu_label, self.cpu_cores, self.os)
    }
}

/// Format a per-machine GPU-vs-CPU performance report as GitHub-Flavored
/// Markdown.
///
/// The report names the hardware ([`HardwareInfo::summary_line`]) and tabulates
/// the measured [`PerfRow`]s (grid size, CPU/GPU ms, speedup, agreement), then
/// states the crossover grid size where the GPU first overtakes the CPU and the
/// peak speedup. `title` heads the document; `methodology` is a short prose
/// paragraph describing what was measured (units, reference, precision note).
///
/// The returned string is self-contained markdown (no external assets) safe to
/// write to a `.md` file.
pub fn format_perf_report(
    title: &str,
    methodology: &str,
    hw: &HardwareInfo,
    rows: &[PerfRow],
) -> String {
    let mut s = String::new();
    s.push_str(&format!("# {title}\n\n"));
    s.push_str(&format!(
        "**Performance on your system:** {}\n\n",
        hw.summary_line()
    ));
    s.push_str(
        "> This report was generated on **your** machine. GPU/CPU timings differ \
         per machine, so these numbers are local to this computer and are not \
         committed to the repository. The committed benchmark markdown documents \
         only the methodology.\n\n",
    );
    s.push_str("## Methodology\n\n");
    s.push_str(methodology);
    s.push_str("\n\n## Results\n\n");
    s.push_str("| n_energy | cpu_ms | gpu_ms | speedup | max_rel_err_total |\n");
    s.push_str("|---|---|---|---|---|\n");
    for r in rows {
        s.push_str(&format!(
            "| {} | {:.3} | {:.3} | {:.3} | {:.3e} |\n",
            r.n_energy, r.cpu_ms, r.gpu_ms, r.speedup, r.max_rel_err
        ));
    }
    s.push('\n');

    // Verdict: first crossover (speedup > 1) and the peak speedup.
    let crossover = rows.iter().find(|r| r.speedup > 1.0);
    let peak = rows.iter().max_by(|a, b| {
        a.speedup
            .partial_cmp(&b.speedup)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    match (crossover, peak) {
        (Some(c), Some(p)) => s.push_str(&format!(
            "**Verdict:** GPU overtakes CPU (speedup > 1) at N = {}; peak speedup {:.3}x at N = {}.\n",
            c.n_energy, p.speedup, p.n_energy
        )),
        (None, Some(p)) => s.push_str(&format!(
            "**Verdict:** GPU did not overtake CPU at any tested grid size (peak speedup {:.3}x at N = {}); \
             the CPU wins here — launch/transfer overhead dominates on this machine.\n",
            p.speedup, p.n_energy
        )),
        _ => s.push_str("**Verdict:** no measurements.\n"),
    }
    s
}

/// Write a generated report string to a **git-ignored** local directory, creating
/// the directory if needed, and return the full path written.
///
/// `crate_dir` is typically `env!("CARGO_MANIFEST_DIR")`; the report lands in
/// `crate_dir/verification_and_validation/local_perf/<filename>`. That
/// `local_perf/` directory is git-ignored (see the crate `.gitignore`) so each
/// user's machine-specific numbers stay local and never commit.
///
/// # Errors
/// Returns any [`std::io::Error`] from creating the directory or writing the file.
pub fn write_local_perf_report(
    crate_dir: impl AsRef<Path>,
    filename: &str,
    contents: &str,
) -> io::Result<PathBuf> {
    let dir = crate_dir
        .as_ref()
        .join("verification_and_validation")
        .join("local_perf");
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(filename);
    std::fs::write(&path, contents)?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The report formatter names the hardware, tabulates rows, and computes the
    /// crossover + peak-speedup verdict from the data (not hard-coded).
    #[test]
    fn format_report_has_header_table_and_verdict() {
        let hw = HardwareInfo {
            gpu_label: "Test GPU / Vulkan".to_string(),
            cpu_cores: 8,
            os: "linux".to_string(),
        };
        let rows = [
            PerfRow {
                n_energy: 1_000,
                cpu_ms: 0.5,
                gpu_ms: 1.6,
                speedup: 0.31,
                max_rel_err: 2.7e-3,
            },
            PerfRow {
                n_energy: 10_000,
                cpu_ms: 5.0,
                gpu_ms: 1.7,
                speedup: 2.94,
                max_rel_err: 4.6e-3,
            },
            PerfRow {
                n_energy: 100_000,
                cpu_ms: 50.0,
                gpu_ms: 2.2,
                speedup: 22.7,
                max_rel_err: 9.0e-3,
            },
        ];
        let md = format_perf_report("Test report", "Some methodology.", &hw, &rows);
        assert!(md.contains("Test GPU / Vulkan, 8 cores, linux"));
        assert!(md.contains("| n_energy | cpu_ms | gpu_ms | speedup | max_rel_err_total |"));
        // Crossover is the first row with speedup > 1 (N = 10000); peak is N = 100000.
        assert!(md.contains("overtakes CPU (speedup > 1) at N = 10000"));
        assert!(md.contains("peak speedup 22.700x at N = 100000"));
    }

    /// `HardwareInfo::detect` fills real CPU-core and OS values.
    #[test]
    fn detect_fills_cores_and_os() {
        let hw = HardwareInfo::detect("CPU only");
        assert!(hw.cpu_cores >= 1);
        assert_eq!(hw.os, std::env::consts::OS);
        assert_eq!(hw.gpu_label, "CPU only");
    }

    /// A run where the GPU never wins produces an honest CPU-wins verdict.
    #[test]
    fn verdict_reports_no_gpu_win() {
        let hw = HardwareInfo::detect("CPU only");
        let rows = [PerfRow {
            n_energy: 1_000,
            cpu_ms: 0.5,
            gpu_ms: 2.0,
            speedup: 0.25,
            max_rel_err: 1e-3,
        }];
        let md = format_perf_report("t", "m", &hw, &rows);
        assert!(md.contains("did not overtake CPU"));
    }
}
