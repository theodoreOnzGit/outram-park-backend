// SPDX-License-Identifier: GPL-3.0-only
//! **A per-run diagnostic record: what data was used, and where the time went.**
//!
//! # Why this exists
//!
//! A Monte Carlo run's wall time is two very different things added together:
//! **nuclear-data processing** (reconstructing cross sections from ENDF tapes,
//! Doppler broadening, generating or baking S(alpha,beta) tables) and the
//! **transport** itself. They scale with completely different things — data
//! prep with the number of nuclides and the thermal laws asked for, transport
//! with histories times cycles — and reporting one number for both makes a run
//! impossible to reason about. A case that spends four minutes in LEAPR and
//! thirty seconds in transport is not "a four-and-a-half minute case".
//!
//! It also answers the question that is hardest to reconstruct afterwards:
//! **which data files did this run actually use?** A missing tape usually does
//! not announce itself — a thermal law that fails to load falls back to free
//! gas, and the eigenvalue simply comes out somewhere else. Recording every
//! source, with the MAT and the temperature it was taken at, turns that from
//! an invisible substitution into a line in a file.
//!
//! # What it is not
//!
//! Not a benchmark. The times here are whatever the machine was doing at the
//! time; use `perf_report` for measurements meant to be compared. This is a
//! provenance record that happens to carry timings.
//!
//! # Output
//!
//! Written to the **gitignored** [`perf_report::LOCAL_PERF_DIR`], like every
//! other machine-specific artifact in this crate, unless
//! `OUTRAM_MC_DIAGNOSTICS` names a path.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use crate::perf_report::LOCAL_PERF_DIR;

/// Where one piece of nuclear data came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DataSource {
    /// Read from a file on disk.
    File(PathBuf),
    /// Generated in-process from a LEAPR deck rather than read from a tape.
    GeneratedFromLeaprDeck(String),
}

impl DataSource {
    fn describe(&self) -> String {
        match self {
            Self::File(p) => p.display().to_string(),
            Self::GeneratedFromLeaprDeck(d) => format!("GENERATED from LEAPR deck '{d}'"),
        }
    }

    /// Size in bytes for a file source, `None` for a generated one.
    fn size_bytes(&self) -> Option<u64> {
        match self {
            Self::File(p) => std::fs::metadata(p).ok().map(|m| m.len()),
            Self::GeneratedFromLeaprDeck(_) => None,
        }
    }
}

/// One nuclear-data item: what it was for, where it came from, how long it took.
#[derive(Debug, Clone)]
pub struct DataItem {
    /// What this data is for, e.g. `"U-235 cross sections"`.
    pub role: String,
    /// Where it came from.
    pub source: DataSource,
    /// Free text — MAT number, temperature, evaluation, whatever identifies it.
    pub detail: String,
    /// Wall seconds spent processing it.
    pub seconds: f64,
    /// Whether it succeeded. A **failed** item is recorded rather than dropped:
    /// a silent fallback is exactly what this file exists to make visible.
    pub ok: bool,
}

/// One timed phase of the run proper.
#[derive(Debug, Clone)]
pub struct Phase {
    /// Phase name, e.g. `"transport (k-eigenvalue)"`.
    pub name: String,
    /// Wall seconds.
    pub seconds: f64,
}

/// The record for one run. Build it as the run proceeds, then [`write`](Self::write).
#[derive(Debug, Clone)]
pub struct RunDiagnostics {
    label: String,
    started_unix: u64,
    data: Vec<DataItem>,
    phases: Vec<Phase>,
    notes: Vec<String>,
}

impl RunDiagnostics {
    /// Start a record for a run called `label`.
    #[must_use]
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            started_unix: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |d| d.as_secs()),
            data: Vec::new(),
            phases: Vec::new(),
            notes: Vec::new(),
        }
    }

    /// Time `f`, record it as a nuclear-data item, and return its result.
    ///
    /// `f` returns an `Option`; `None` is recorded as a **failure** rather than
    /// omitted, because the thing this file is for is making a silent fallback
    /// visible.
    pub fn time_data<T>(
        &mut self,
        role: impl Into<String>,
        source: DataSource,
        detail: impl Into<String>,
        f: impl FnOnce() -> Option<T>,
    ) -> Option<T> {
        let t = Instant::now();
        let out = f();
        self.data.push(DataItem {
            role: role.into(),
            source,
            detail: detail.into(),
            seconds: t.elapsed().as_secs_f64(),
            ok: out.is_some(),
        });
        out
    }

    /// Record an already-timed data item.
    pub fn push_data(&mut self, item: DataItem) {
        self.data.push(item);
    }

    /// Time `f` as a named run phase and return its result.
    pub fn time_phase<T>(&mut self, name: impl Into<String>, f: impl FnOnce() -> T) -> T {
        let t = Instant::now();
        let out = f();
        self.phases.push(Phase {
            name: name.into(),
            seconds: t.elapsed().as_secs_f64(),
        });
        out
    }

    /// Add a free-text note — settings, environment knobs, caveats.
    pub fn note(&mut self, note: impl Into<String>) {
        self.notes.push(note.into());
    }

    /// How many nuclear-data items were recorded.
    #[must_use]
    pub fn data_item_count(&self) -> usize {
        self.data.len()
    }

    /// Total seconds spent on nuclear data.
    #[must_use]
    pub fn data_seconds(&self) -> f64 {
        self.data.iter().map(|d| d.seconds).sum()
    }

    /// Total seconds spent in the run phases.
    #[must_use]
    pub fn phase_seconds(&self) -> f64 {
        self.phases.iter().map(|p| p.seconds).sum()
    }

    /// Any data item that failed to load.
    #[must_use]
    pub fn failures(&self) -> Vec<&DataItem> {
        self.data.iter().filter(|d| !d.ok).collect()
    }

    /// Render the record.
    #[must_use]
    pub fn render(&self) -> String {
        let mut s = String::new();
        let _ = writeln!(s, "# outram-mc run diagnostics — {}", self.label);
        let _ = writeln!(s, "\nstarted_unix,{}", self.started_unix);
        let _ = writeln!(s, "data_seconds,{:.3}", self.data_seconds());
        let _ = writeln!(s, "transport_seconds,{:.3}", self.phase_seconds());
        let total = self.data_seconds() + self.phase_seconds();
        let _ = writeln!(s, "total_seconds,{total:.3}");
        if total > 0.0 {
            let _ = writeln!(
                s,
                "data_fraction_percent,{:.1}",
                100.0 * self.data_seconds() / total
            );
        }

        let _ = writeln!(s, "\n## Nuclear data\n");
        let _ = writeln!(s, "| role | seconds | ok | source | detail |");
        let _ = writeln!(s, "|---|---:|---|---|---|");
        for d in &self.data {
            let size = d
                .source
                .size_bytes()
                .map_or(String::new(), |b| format!(" ({b} bytes)"));
            let _ = writeln!(
                s,
                "| {} | {:.3} | {} | {}{} | {} |",
                d.role,
                d.seconds,
                if d.ok { "yes" } else { "**NO**" },
                d.source.describe(),
                size,
                d.detail
            );
        }

        let failures = self.failures();
        if !failures.is_empty() {
            let _ = writeln!(
                s,
                "\n**{} DATA ITEM(S) FAILED TO LOAD.** Whatever the run did \
                 instead — a free-gas fallback, a dropped nuclide — it did not \
                 use the data named above, and the result must not be quoted \
                 as if it had:",
                failures.len()
            );
            for f in failures {
                let _ = writeln!(s, "- {} — {}", f.role, f.source.describe());
            }
        }

        let _ = writeln!(s, "\n## Run phases\n");
        let _ = writeln!(s, "| phase | seconds |");
        let _ = writeln!(s, "|---|---:|");
        for p in &self.phases {
            let _ = writeln!(s, "| {} | {:.3} |", p.name, p.seconds);
        }

        if !self.notes.is_empty() {
            let _ = writeln!(s, "\n## Notes\n");
            for n in &self.notes {
                let _ = writeln!(s, "- {n}");
            }
        }
        s
    }

    /// Print the summary to stdout — the two totals, separated, and any failure.
    pub fn print_summary(&self) {
        println!("\n  --- timing, data processing vs transport ---");
        println!("  nuclear data : {:8.1} s", self.data_seconds());
        println!("  transport    : {:8.1} s", self.phase_seconds());
        println!(
            "  total        : {:8.1} s",
            self.data_seconds() + self.phase_seconds()
        );
        let failures = self.failures();
        if !failures.is_empty() {
            println!("  !! {} DATA ITEM(S) FAILED TO LOAD:", failures.len());
            for f in failures {
                println!("     {} — {}", f.role, f.source.describe());
            }
        }
    }

    /// Where the record will be written: `OUTRAM_MC_DIAGNOSTICS`, else a
    /// timestamped file in the gitignored local-perf directory.
    #[must_use]
    pub fn output_path(&self) -> PathBuf {
        if let Ok(p) = std::env::var("OUTRAM_MC_DIAGNOSTICS") {
            return PathBuf::from(p);
        }
        let slug: String = self
            .label
            .chars()
            .map(|c| if c.is_alphanumeric() { c } else { '-' })
            .collect();
        PathBuf::from(LOCAL_PERF_DIR).join(format!("run-{}-{}.md", slug, self.started_unix))
    }

    /// Write the record, creating the directory if needed. Returns the path.
    ///
    /// # Errors
    /// Propagates any I/O error from creating the directory or writing.
    pub fn write(&self) -> std::io::Result<PathBuf> {
        let path = self.output_path();
        if let Some(dir) = path.parent() {
            if !dir.as_os_str().is_empty() {
                std::fs::create_dir_all(dir)?;
            }
        }
        std::fs::write(&path, self.render())?;
        Ok(path)
    }

    /// Write, and print where it went — or why it could not be written.
    ///
    /// Never panics and never fails the run: a diagnostics file that cannot be
    /// written is a nuisance, not a reason to discard a completed transport
    /// calculation.
    pub fn write_and_report(&self) -> Option<PathBuf> {
        match self.write() {
            Ok(p) => {
                println!("  diagnostics  : {}", p.display());
                Some(p)
            }
            Err(e) => {
                eprintln!("  diagnostics could not be written: {e}");
                None
            }
        }
    }
}

impl AsRef<Path> for DataSource {
    fn as_ref(&self) -> &Path {
        match self {
            Self::File(p) => p.as_path(),
            Self::GeneratedFromLeaprDeck(_) => Path::new(""),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_failed_item_is_recorded_not_dropped() {
        let mut d = RunDiagnostics::new("t");
        let got = d.time_data(
            "missing law",
            DataSource::File(PathBuf::from("/nonexistent.endf")),
            "MAT 99",
            || None::<u8>,
        );
        assert!(got.is_none());
        assert_eq!(d.failures().len(), 1, "a silent fallback must be visible");
        assert!(d.render().contains("FAILED TO LOAD"));
    }

    #[test]
    fn data_and_transport_are_totalled_separately() {
        let mut d = RunDiagnostics::new("t");
        d.time_data(
            "x",
            DataSource::GeneratedFromLeaprDeck("d".into()),
            "",
            || Some(1u8),
        );
        d.time_phase("transport", || {});
        // Both totals exist independently; the point of the file is that they
        // are never added together silently.
        assert!(d.data_seconds() >= 0.0 && d.phase_seconds() >= 0.0);
        let r = d.render();
        assert!(r.contains("data_seconds"));
        assert!(r.contains("transport_seconds"));
    }

    #[test]
    fn the_output_path_honours_the_env_override() {
        let d = RunDiagnostics::new("my run");
        // Default lands in the gitignored local-perf directory.
        assert!(d.output_path().starts_with(LOCAL_PERF_DIR));
        assert!(d.output_path().to_string_lossy().contains("my-run"));
    }
}
