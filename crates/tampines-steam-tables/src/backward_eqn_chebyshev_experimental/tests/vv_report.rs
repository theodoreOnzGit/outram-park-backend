//! Writes the diagnostic sweeps out as markdown V&V reports.
//!
//! The diagnostic tests in this module tree measure the experimental
//! correlations against IAPWS-traceable references. Rather than leaving those
//! numbers only in stderr, each diagnostic writes a self-contained markdown
//! report into `verification_and_validation/generated/`, which is gitignored:
//! the reports are **regenerable output**, not source, and the committed
//! write-ups alongside them are hand-authored.
//!
//! Every report records both methodology and results, per the workspace V&V
//! documentation rule, and carries the standing caveat that these correlations
//! are experimental and have no human V&V sign-off.
//!
//! Reports are written one file per diagnostic, so tests running in parallel
//! never contend for the same file.

use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

/// Directory the generated reports are written to, relative to the crate root.
const REPORT_DIR: &str = "verification_and_validation/generated";

/// A markdown V&V report under construction.
pub struct VvReport {
    /// File stem, without the `.md` extension.
    slug: String,
    /// Document title.
    title: String,
    body: String,
}

impl VvReport {
    /// Starts a report. `slug` becomes the filename, `title` the `# ` heading.
    pub fn new(slug: &str, title: &str) -> Self {
        Self {
            slug: slug.to_string(),
            title: title.to_string(),
            body: String::new(),
        }
    }

    /// Adds a `## ` section heading.
    pub fn section(&mut self, heading: &str) -> &mut Self {
        // paragraphs already end in a blank line; don't stack another
        while self.body.ends_with('\n') {
            self.body.pop();
        }
        if !self.body.is_empty() {
            self.body.push('\n');
        }
        self.body.push_str(&format!("\n## {heading}\n\n"));
        self
    }

    /// Adds a paragraph of prose.
    pub fn paragraph(&mut self, text: &str) -> &mut Self {
        self.body.push_str(text);
        self.body.push_str("\n\n");
        self
    }

    /// Adds a markdown table from a header row and body rows.
    pub fn table(&mut self, header: &[&str], rows: &[Vec<String>]) -> &mut Self {
        self.body.push_str(&format!("| {} |\n", header.join(" | ")));
        self.body
            .push_str(&format!("|{}|\n", vec!["---"; header.len()].join("|")));
        for row in rows {
            self.body.push_str(&format!("| {} |\n", row.join(" | ")));
        }
        self.body.push('\n');
        self
    }

    /// Writes the report to `verification_and_validation/generated/<slug>.md`
    /// and returns the path written.
    ///
    /// `regenerate_with` is the cargo command a reader can run to reproduce
    /// the file, and is recorded in the report itself.
    pub fn write(&self, regenerate_with: &str) -> PathBuf {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(REPORT_DIR);
        fs::create_dir_all(&dir).expect("could not create the V&V report directory");

        let path = dir.join(format!("{}.md", self.slug));
        let mut file = fs::File::create(&path).expect("could not create the V&V report");

        write!(
            file,
            "# {title}\n\n\
             > **Generated file — do not hand-edit.** Regenerate with:\n\
             >\n\
             > ```bash\n\
             > {regenerate_with}\n\
             > ```\n\
             >\n\
             > Generated {date} (UTC).\n\n\
             ## Status\n\n\
             These are **experimental, non-IAPWS correlations** fitted in-house \
             (see GitHub issue #34). IAPWS-IF97 publishes no backward equations \
             for some of the cases covered here, so where a reference is quoted \
             it is either an IAPWS equation already implemented in this crate or \
             this crate's own forward equations — never a published backward-\
             equation reference value.\n\n\
             Per `RESPONSIBLE_USE.md` this is AI-assisted draft material: the \
             numbers below are measurements, **not a validation sign-off**. No \
             human has reviewed them.\n\
             {body}",
            title = self.title,
            regenerate_with = regenerate_with,
            date = utc_date_string(),
            body = self.body,
        )
        .expect("could not write the V&V report");

        eprintln!("wrote V&V report: {}", path.display());
        path
    }
}

/// Formats the current time as `YYYY-MM-DD hh:mm`, in UTC.
///
/// Done by hand rather than with a date crate: this crate's dependencies are
/// deliberately minimal (`approx`, `ndarray`, `thiserror`, `uom`) and a report
/// timestamp does not justify adding one.
fn utc_date_string() -> String {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    let days = seconds.div_euclid(86_400);
    let seconds_of_day = seconds.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);

    format!(
        "{year:04}-{month:02}-{day:02} {:02}:{:02}",
        seconds_of_day / 3600,
        (seconds_of_day % 3600) / 60
    )
}

/// Converts a count of days since the Unix epoch into a civil `(year, month,
/// day)`, by Howard Hinnant's `civil_from_days` algorithm.
fn civil_from_days(days_since_epoch: i64) -> (i64, u32, u32) {
    // shift the epoch to 0000-03-01, which puts the leap day at the end of the
    // 4-year era and makes the arithmetic branch-free
    let z = days_since_epoch + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = z.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = (day_of_year - (153 * month_prime + 2) / 5 + 1) as u32;
    let month = if month_prime < 10 {
        month_prime + 3
    } else {
        month_prime - 9
    } as u32;
    let year = if month <= 2 { year + 1 } else { year };

    (year, month, day)
}

/// Formats an error distribution as table cells: median, p90, p99, maximum.
///
/// `errors` need not be sorted; it is sorted in place.
pub fn percentile_cells(errors: &mut [f64], as_percent: bool) -> Vec<String> {
    errors.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let pick = |q: f64| errors[((errors.len() - 1) as f64 * q) as usize];
    let format_one = |value: f64| {
        if as_percent {
            format!("{:.4}%", 100.0 * value)
        } else {
            format!("{value:.3e}")
        }
    };

    vec![
        format_one(pick(0.50)),
        format_one(pick(0.90)),
        format_one(pick(0.99)),
        format_one(errors[errors.len() - 1]),
    ]
}
