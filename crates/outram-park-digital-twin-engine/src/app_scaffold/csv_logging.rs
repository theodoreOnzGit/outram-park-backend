//! Opt-in, real CSV file export for a real-time simulator.
//!
//! The 2026-08-17 survey behind bead `op-wqk.22.2` found that despite the
//! name, no simulator in this crate actually writes a CSV **file** today.
//! `ciet_educational_simulator_v2`'s "CSV" pages (e.g.
//! `ciet_simulator_v2/app/panels_and_pages/heater_page.rs`'s
//! `ciet_sim_heater_page_csv`) and `fhr_sim_v2`'s equivalent
//! (`examples/fhr_sim_v2/app/graph_pages/mod.rs`) render a scrollable
//! on-screen `ui.label(...)` of hand comma-joined text -- a copy-paste-from-
//! screen affordance, not a logging pipeline. `htgr_sim_v1` has no CSV
//! affordance of either kind.
//!
//! [`CsvLogger`] is a real file writer any simulator can opt into: create it
//! once with a header row and a minimum write interval, then call
//! [`CsvLogger::maybe_write_row`] from the same sampling loop that already
//! feeds a [`super::plot_history::PlotHistory`] (or any other per-tick
//! source) -- it silently no-ops on ticks that arrive before the interval
//! has elapsed, so the caller does not need its own throttling logic. The
//! interval mirrors the `graph_data_record_interval_seconds` convention
//! already used by the CIET v1-derived plot state (`ciet_data.rs`,
//! `fhr_sim_v2`'s `graph_data/mod.rs`), so a future migration of that field
//! onto this type is a narrow, mechanical change.
//!
//! Built on the `csv` crate, which is already a workspace dependency
//! (`csv = "1.4.0"`, root `Cargo.toml`, consumed today by
//! `tuas_boussinesq_solver`) -- this module adds `csv.workspace = true` to
//! this crate's own `Cargo.toml` rather than hand-joining strings with
//! commas the way the "fake CSV" display panels do, so a value that itself
//! contains a comma or a quote is escaped correctly instead of corrupting
//! the file.
//!
//! **This module is additive.** As of this pass no existing simulator has
//! been wired to it -- each simulator's own column set (which physical
//! quantities, in what units) is a separate follow-up decision left to that
//! simulator's own maintainer, not something this generic logger should
//! guess at.
//!
//! # What belongs in this module
//!
//! Generic, physics-free CSV row writing with interval throttling and
//! file-system error reporting. Deciding *which* columns a given simulator
//! logs, and *when* in its own loop to call [`maybe_write_row`], is that
//! simulator's job -- this module does not know what a "reactor power" or a
//! "fuel temperature" is.
//!
//! [`maybe_write_row`]: CsvLogger::maybe_write_row

use std::fs::File;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// Things that can go wrong creating or writing a [`CsvLogger`]'s file.
///
/// Every variant carries the file path so the message is actionable without
/// the caller having to remember which logger raised it (a simulator may run
/// several: heater trend, plot export, ...).
#[derive(Debug, thiserror::Error)]
pub enum CsvLoggerError {
    /// The file could not be created (bad path, no write permission, disk
    /// full) or the header row could not be written to it.
    #[error("could not create the CSV log file at {path}: {source}")]
    Create { path: PathBuf, source: csv::Error },

    /// A data row could not be written -- normally only possible if the file
    /// was removed or the disk filled up after [`CsvLogger::create`]
    /// succeeded.
    #[error("could not write a row to the CSV log at {path}: {source}")]
    Write { path: PathBuf, source: csv::Error },

    /// The row was handed to the writer's internal buffer but could not be
    /// flushed to disk.
    #[error("could not flush the CSV log at {path}: {source}")]
    Flush {
        path: PathBuf,
        source: std::io::Error,
    },
}

/// An open CSV file that writes at most one row per configured interval.
///
/// Every accepted row is flushed to disk immediately (not left buffered) --
/// a physics thread in this crate may be running under
/// [`spawn_physics_thread_monitored`](super::spawn_physics_thread_monitored)
/// and could panic on the very next tick; buffering rows in memory only to
/// lose them on a crash would defeat the point of logging to a file at all.
/// The interval throttling exists so that cost is paid at a sane cadence
/// (e.g. once a second) instead of once per physics tick.
pub struct CsvLogger {
    writer: csv::Writer<File>,
    path: PathBuf,
    interval: Duration,
    last_write: Option<Instant>,
}

impl CsvLogger {
    /// Create a new CSV log at `path`, writing `header` as the first row
    /// immediately. `interval` is the minimum wall-clock time between
    /// accepted rows -- pass [`Duration::ZERO`] to accept every row handed
    /// to [`maybe_write_row`](Self::maybe_write_row).
    ///
    /// Fails if the file cannot be created (bad path, no write permission)
    /// or the header row cannot be written and flushed.
    pub fn create(
        path: impl AsRef<Path>,
        header: &[&str],
        interval: Duration,
    ) -> Result<Self, CsvLoggerError> {
        let path = path.as_ref().to_path_buf();
        let mut writer =
            csv::Writer::from_path(&path).map_err(|source| CsvLoggerError::Create {
                path: path.clone(),
                source,
            })?;
        writer
            .write_record(header)
            .map_err(|source| CsvLoggerError::Create {
                path: path.clone(),
                source,
            })?;
        writer.flush().map_err(|source| CsvLoggerError::Flush {
            path: path.clone(),
            source,
        })?;
        Ok(Self {
            writer,
            path,
            interval,
            last_write: None,
        })
    }

    /// Write `row` and flush it to disk, unless fewer than `interval` (see
    /// [`create`](Self::create)) has elapsed since the last accepted row --
    /// in which case this is a no-op and returns `Ok(false)`.
    ///
    /// `now` is supplied by the caller (rather than read internally via
    /// [`Instant::now`]) so the throttling can be driven deterministically
    /// in a test without a real sleep, and so a caller already carrying a
    /// tick timestamp does not pay for a second clock read.
    ///
    /// Returns `Ok(true)` if the row was written. The very first call always
    /// writes (there is no "last write" yet to measure against).
    pub fn maybe_write_row(
        &mut self,
        now: Instant,
        row: &[String],
    ) -> Result<bool, CsvLoggerError> {
        let should_write = match self.last_write {
            None => true,
            Some(last) => now.saturating_duration_since(last) >= self.interval,
        };
        if !should_write {
            return Ok(false);
        }

        self.writer
            .write_record(row)
            .map_err(|source| CsvLoggerError::Write {
                path: self.path.clone(),
                source,
            })?;
        self.writer.flush().map_err(|source| CsvLoggerError::Flush {
            path: self.path.clone(),
            source,
        })?;
        self.last_write = Some(now);
        Ok(true)
    }

    /// The path this logger is writing to.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The configured minimum interval between accepted rows.
    pub fn interval(&self) -> Duration {
        self.interval
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_csv_path(name: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!(
            "outram_park_csv_logging_test_{name}_{}.csv",
            std::process::id()
        ));
        path
    }

    #[test]
    fn create_writes_the_header_immediately() {
        let path = temp_csv_path("header");
        let _logger =
            CsvLogger::create(&path, &["t_seconds", "power_mw"], Duration::ZERO).unwrap();
        let contents = std::fs::read_to_string(&path).unwrap();
        assert_eq!(contents.trim(), "t_seconds,power_mw");
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn first_row_is_always_written_regardless_of_interval() {
        let path = temp_csv_path("first_row");
        let mut logger = CsvLogger::create(
            &path,
            &["t_seconds", "power_mw"],
            Duration::from_secs(60),
        )
        .unwrap();
        let wrote = logger
            .maybe_write_row(Instant::now(), &["0.0".to_string(), "10.0".to_string()])
            .unwrap();
        assert!(wrote);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn row_within_interval_is_throttled() {
        let path = temp_csv_path("throttled");
        let mut logger = CsvLogger::create(
            &path,
            &["t_seconds", "power_mw"],
            Duration::from_secs(60),
        )
        .unwrap();
        let t0 = Instant::now();
        assert!(logger
            .maybe_write_row(t0, &["0.0".to_string(), "10.0".to_string()])
            .unwrap());
        // Only 1 ms later -- well inside the 60 s interval -- should be dropped.
        let t1 = t0 + Duration::from_millis(1);
        let wrote = logger
            .maybe_write_row(t1, &["0.001".to_string(), "10.1".to_string()])
            .unwrap();
        assert!(!wrote);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn row_after_interval_elapses_is_written() {
        let path = temp_csv_path("elapsed");
        let mut logger =
            CsvLogger::create(&path, &["t_seconds", "power_mw"], Duration::from_secs(1)).unwrap();
        let t0 = Instant::now();
        assert!(logger
            .maybe_write_row(t0, &["0.0".to_string(), "10.0".to_string()])
            .unwrap());
        let t1 = t0 + Duration::from_secs(2);
        let wrote = logger
            .maybe_write_row(t1, &["2.0".to_string(), "11.0".to_string()])
            .unwrap();
        assert!(wrote);

        let contents = std::fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = contents.lines().collect();
        // header + 2 accepted rows, the throttled write from the interval
        // test above is a different file so no cross-test interference.
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0], "t_seconds,power_mw");
        assert_eq!(lines[1], "0.0,10.0");
        assert_eq!(lines[2], "2.0,11.0");
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn zero_interval_accepts_every_row() {
        let path = temp_csv_path("zero_interval");
        let mut logger =
            CsvLogger::create(&path, &["t_seconds", "power_mw"], Duration::ZERO).unwrap();
        let t0 = Instant::now();
        for i in 0..5 {
            let t = t0 + Duration::from_nanos(i);
            let wrote = logger
                .maybe_write_row(t, &[i.to_string(), (i * 2).to_string()])
                .unwrap();
            assert!(wrote, "row {i} should have been written");
        }
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn a_value_containing_a_comma_is_escaped_not_corrupted() {
        // The point of using the `csv` crate instead of hand comma-joining
        // (what the "fake CSV" display panels do) -- a value that itself
        // contains a comma must round-trip, not silently split a column.
        let path = temp_csv_path("escaping");
        let mut logger = CsvLogger::create(&path, &["note"], Duration::ZERO).unwrap();
        logger
            .maybe_write_row(Instant::now(), &["reading, unstable".to_string()])
            .unwrap();
        drop(logger);

        let mut reader = csv::Reader::from_path(&path).unwrap();
        let record = reader.records().next().unwrap().unwrap();
        assert_eq!(&record[0], "reading, unstable");
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn path_and_interval_accessors_report_what_was_configured() {
        let path = temp_csv_path("accessors");
        let logger = CsvLogger::create(&path, &["t_seconds"], Duration::from_millis(250)).unwrap();
        assert_eq!(logger.path(), path.as_path());
        assert_eq!(logger.interval(), Duration::from_millis(250));
        drop(logger);
        std::fs::remove_file(&path).ok();
    }
}
