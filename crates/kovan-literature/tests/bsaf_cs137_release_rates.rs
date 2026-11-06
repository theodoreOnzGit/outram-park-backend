//! Regression guard on the digitised BSAF Cs-137 release-rate histories
//! (`derived/nea7525-bsaf-cs137-release-rates/`, GitHub #1028).
//!
//! **Why this exists.** That folder holds 10 403 committed data points read
//! off Figures 5.1-5.4 of OECD/NEA No. 7525, and the Fukushima simulator is
//! meant to be driven by them. Before this file nothing checked any of it: a
//! mangled CSV, a dropped series, a shifted column or a quietly edited number
//! would have been found by whoever next ran a plume calculation, if at all.
//!
//! **What it checks, in rising order of strength:**
//!
//! 1. **Structure** — the data parses, every series is time-ascending, and
//!    every point lies inside the axes it was read off (0-20 days,
//!    1e7-1e15 Bq/hr). A point outside the frame is a digitisation that
//!    escaped the plot.
//! 2. **Internal consistency** — the per-series CSVs and the combined
//!    `all-series.csv` are two copies of the same numbers, so they are
//!    checked against each other. Duplicated data drifts; this is the guard.
//! 3. **The published quality table is parsed out of the README and checked
//!    against the data.** The point count, the integral and the ratio printed
//!    there must be what the committed CSVs actually produce. A doc claim the
//!    data contradicts is a defect under the workspace rule, and this makes
//!    that rule executable rather than aspirational.
//! 4. **The independent V&V** — each digitised curve is integrated and
//!    compared against the integral release the report tabulates in its
//!    Table 5.1, transcribed by hand from a different page. The curves come
//!    from pixels and the targets from a table, so this is the check that
//!    actually says the right curve was traced. It is the check that caught
//!    the SNL/KAERI confusion during the digitisation.
//!
//! **Result (2026-10-27).** 21 series, 10 403 points. Median integral ratio
//! 0.928; 17 of 21 within a factor of two; 7 within 5 % (9 graded A, a wider
//! band — the README said "9 within 5 %" until this test was written and
//! disagreed). Four series under-read
//! because a column-wise trace cannot integrate a near-vertical spike; they
//! are graded C or D in the README and their ratios are pinned here as they
//! stand, **not** corrected toward the table.
//!
//! **This is not a validation of BSAF or of the simulator.** It checks that
//! the committed numbers are the numbers that were measured and published in
//! the README beside them. Acknowledgement: OECD Nuclear Energy Agency,
//! NEA No. 7525, (c) OECD 2021.

use std::collections::BTreeMap;
use std::path::PathBuf;

/// Where the dataset lives, relative to this crate.
fn data_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("derived/nea7525-bsaf-cs137-release-rates")
}

/// The axes the curves were read off: 0-20 days, 1e7-1e15 Bq/hr.
const T_MIN: f64 = 0.0;
const T_MAX: f64 = 20.0;
const Q_MIN: f64 = 1.0e7;
const Q_MAX: f64 = 1.0e15;

/// One digitised point.
#[derive(Debug, Clone, Copy)]
struct Point {
    t_day: f64,
    q_bq_per_hr: f64,
}

/// Every series, keyed by `(figure, series)`.
type Series = BTreeMap<(String, String), Vec<Point>>;

fn load_combined() -> Series {
    let path = data_dir().join("all-series.csv");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
    let mut lines = text.lines();
    let header = lines.next().expect("all-series.csv is empty");
    assert_eq!(
        header, "figure,unit,series,time_day,release_rate_bq_per_hr",
        "all-series.csv header changed; downstream readers index by position"
    );
    let mut out: Series = BTreeMap::new();
    for (i, line) in lines.enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let f: Vec<&str> = line.split(',').collect();
        assert_eq!(f.len(), 5, "row {} has {} fields: {line}", i + 2, f.len());
        let t: f64 = f[3]
            .parse()
            .unwrap_or_else(|e| panic!("row {}: bad time {:?}: {e}", i + 2, f[3]));
        let q: f64 = f[4]
            .parse()
            .unwrap_or_else(|e| panic!("row {}: bad rate {:?}: {e}", i + 2, f[4]));
        out.entry((f[0].to_string(), f[2].to_string()))
            .or_default()
            .push(Point {
                t_day: t,
                q_bq_per_hr: q,
            });
    }
    out
}

/// Trapezoidal integral \[PBq\], not bridging gaps longer than `MAX_GAP_DAYS`.
///
/// A long gap is a real absence in the figure — a curve drawn in fragments, or
/// one that left the axes — and bridging it would invent release that the
/// published figure does not show.
const MAX_GAP_DAYS: f64 = 0.5;

fn integral_pbq(points: &[Point]) -> f64 {
    let mut total = 0.0;
    for w in points.windows(2) {
        let dt = w[1].t_day - w[0].t_day;
        if dt <= 0.0 || dt > MAX_GAP_DAYS {
            continue;
        }
        total += 0.5 * (w[0].q_bq_per_hr + w[1].q_bq_per_hr) * dt * 24.0;
    }
    total / 1.0e15
}

/// Table 5.1 of NEA No. 7525 — integral Cs-137 release \[PBq\] per
/// participant and unit. Transcribed by hand from the rendered page, and the
/// independent target the digitised curves are judged against.
fn table_5_1(figure: &str, series: &str) -> Option<f64> {
    let v = match (figure, series) {
        ("5.1", "SNL MELCOR") => 2.8,
        ("5.1", "IAE SAMPSON") => 4.4,
        ("5.1", "IRSN ASTEC") => 0.1,
        ("5.1", "KAERI MELCOR") => 18.9,
        ("5.1", "CIEMAT MELCOR") => 5.6,
        ("5.2", "SNL MELCOR") => 12.3,
        ("5.2", "IRSN ASTEC") => 0.1,
        ("5.2", "KAERI MELCOR") => 5.8,
        ("5.2", "CNL MELCOR") => 8.1,
        ("5.2", "CRIEPI MAAP") => 6.7,
        ("5.3", "SNL MELCOR") => 10.6,
        ("5.3", "IAE SAMPSON") => 1.1,
        ("5.3", "IRSN ASTEC") => 43.1,
        ("5.3", "KAERI MELCOR") => 10.9,
        ("5.3", "CRIEPI MAAP") => 0.8,
        ("5.3", "PSI MELCOR") => 0.2,
        ("5.4", "SNL MELCOR") => 25.7,
        ("5.4", "GRS Backwards Calculation") => 13.1,
        ("5.4", "IAE SAMPSON") => 5.5,
        ("5.4", "IRSN ASTEC") => 43.2,
        ("5.4", "KAERI MELCOR") => 35.5,
        _ => return None,
    };
    Some(v)
}

#[test]
fn the_dataset_has_the_series_and_point_count_it_claims() {
    let data = load_combined();
    assert_eq!(data.len(), 21, "expected 21 series, got {}", data.len());
    let total: usize = data.values().map(Vec::len).sum();
    assert_eq!(total, 10_403, "expected 10 403 points, got {total}");

    // Every series must be one the report actually has.
    for (figure, series) in data.keys() {
        assert!(
            table_5_1(figure, series).is_some(),
            "series ({figure}, {series}) is not in Table 5.1"
        );
    }
    // IAE on unit 2 must NOT be present: the report says 0.0 PBq and the
    // figure carries no curve. An empty file would read as a failed
    // extraction, so the absence is deliberate and is pinned here.
    assert!(
        !data.contains_key(&("5.2".to_string(), "IAE SAMPSON".to_string())),
        "IAE SAMPSON on unit 2 should be absent, not empty"
    );
}

#[test]
fn every_point_lies_inside_the_axes_it_was_read_off() {
    for ((figure, series), points) in load_combined() {
        assert!(!points.is_empty(), "({figure}, {series}) is empty");
        for p in &points {
            assert!(
                (T_MIN..=T_MAX).contains(&p.t_day),
                "({figure}, {series}) t={} outside 0-20 day axis",
                p.t_day
            );
            assert!(
                p.q_bq_per_hr.is_finite() && (Q_MIN..=Q_MAX).contains(&p.q_bq_per_hr),
                "({figure}, {series}) q={:.4e} outside the 1e7-1e15 Bq/hr axis",
                p.q_bq_per_hr
            );
        }
        // Strictly ascending in time: a digitiser emits one value per pixel
        // column, so a repeat or a reversal means the rows were disturbed.
        for w in points.windows(2) {
            assert!(
                w[1].t_day > w[0].t_day,
                "({figure}, {series}) not ascending at t={}",
                w[0].t_day
            );
        }
    }
}

#[test]
fn the_per_series_files_and_the_combined_file_are_the_same_numbers() {
    let combined = load_combined();
    for ((figure, series), points) in &combined {
        let slug: String = format!("fig{figure}-{}", series.to_lowercase().replace(' ', "-"))
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || *c == '.' || *c == '-')
            .collect();
        let path = data_dir().join(format!("{slug}.csv"));
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
        let mut lines = text.lines();
        assert_eq!(
            lines.next().unwrap(),
            "time_day,release_rate_bq_per_hr",
            "{slug}: header changed"
        );
        let rows: Vec<&str> = lines.filter(|l| !l.trim().is_empty()).collect();
        assert_eq!(
            rows.len(),
            points.len(),
            "{slug}: {} rows against {} in all-series.csv",
            rows.len(),
            points.len()
        );
        for (i, (row, p)) in rows.iter().zip(points).enumerate() {
            let f: Vec<&str> = row.split(',').collect();
            let (t, q): (f64, f64) = (f[0].parse().unwrap(), f[1].parse().unwrap());
            assert_eq!(t, p.t_day, "{slug} row {i}: time differs from all-series");
            assert_eq!(
                q, p.q_bq_per_hr,
                "{slug} row {i}: rate differs from all-series"
            );
        }
    }
}

/// Parse the README's quality table and hold the data to it.
///
/// Columns: `| Fig | Series | N | INT | TBL | Ratio | Spikes | Grade |`.
#[test]
fn the_published_quality_table_matches_the_committed_data() {
    let readme = std::fs::read_to_string(data_dir().join("README.md")).expect("README.md");
    let data = load_combined();

    let mut checked = 0;
    for line in readme.lines() {
        let line = line.trim();
        if !line.starts_with("| 5.") {
            continue;
        }
        let cells: Vec<String> = line
            .trim_matches('|')
            .split('|')
            .map(|c| c.trim().replace("**", ""))
            .collect();
        assert!(cells.len() >= 7, "unexpected table row: {line}");
        let (figure, series) = (cells[0].clone(), cells[1].clone());
        let n: usize = cells[2].parse().unwrap_or_else(|e| panic!("{line}: N {e}"));

        let Some(points) = data.get(&(figure.clone(), series.clone())) else {
            // The one row with no file: IAE on unit 2, N = 0.
            assert_eq!(n, 0, "{figure} {series}: row claims {n} points but no data");
            checked += 1;
            continue;
        };

        assert_eq!(
            points.len(),
            n,
            "{figure} {series}: README says {n} points, data has {}",
            points.len()
        );

        let got = integral_pbq(points);
        let printed: f64 = cells[3].parse().unwrap();
        assert!(
            (got - printed).abs() <= 1.0e-4 + 0.002 * printed,
            "{figure} {series}: README integral {printed} PBq, data gives {got:.4}"
        );

        let tbl: f64 = cells[4].parse().unwrap();
        assert_eq!(
            Some(tbl),
            table_5_1(&figure, &series),
            "{figure} {series}: README Table 5.1 value disagrees with this test's copy"
        );

        if tbl > 0.0 {
            let ratio: f64 = cells[5].parse().unwrap();
            assert!(
                (got / tbl - ratio).abs() < 0.002,
                "{figure} {series}: README ratio {ratio}, data gives {:.3}",
                got / tbl
            );
        }
        checked += 1;
    }
    assert_eq!(checked, 22, "expected 22 README table rows, parsed {checked}");
}

/// **The independent check.** Curves from pixels against a hand-transcribed
/// table, with the measured agreement pinned as it stands.
///
/// **Result (2026-10-27).** Median ratio 0.928, 17 of 21 series within a
/// factor of two, 7 within 5 %. The four that miss badly do so because a
/// column-wise
/// trace cannot integrate a near-vertical spike — PSI on unit 3 is 13 points
/// of pure spikes — and are reported as lower bounds, **not** rescaled.
#[test]
fn the_digitised_integrals_agree_with_table_5_1_as_measured() {
    let data = load_combined();
    let mut ratios = Vec::new();
    for ((figure, series), points) in &data {
        let tbl = table_5_1(figure, series).expect("series not in Table 5.1");
        if tbl <= 0.0 {
            continue;
        }
        ratios.push(integral_pbq(points) / tbl);
    }
    ratios.sort_by(|a, b| a.partial_cmp(b).unwrap());

    let median = ratios[ratios.len() / 2];
    assert!(
        (median - 0.928).abs() < 0.01,
        "median ratio moved: {median:.3} (was 0.928)"
    );
    let within_2x = ratios.iter().copied().filter(|r| (0.5..=2.0).contains(r)).count();
    assert_eq!(within_2x, 17, "series within a factor of two moved");
    let within_5pc = ratios.iter().copied().filter(|r| (0.95..=1.05).contains(r)).count();
    assert_eq!(
        within_5pc, 7,
        "series within 5 % moved; the README said 9 until 2026-10-27, which \
         confused the A-grade count with a 5 % band"
    );

    // Nothing may read as more than 3x the table: that would mean the trace
    // picked up a different, higher curve, which is how the SNL/KAERI
    // confusion showed itself during the digitisation.
    for ((figure, series), points) in &data {
        let tbl = table_5_1(figure, series).unwrap();
        if tbl <= 0.0 {
            continue;
        }
        let r = integral_pbq(points) / tbl;
        assert!(
            r < 3.0,
            "({figure}, {series}) integrates to {r:.2}x Table 5.1 — likely tracing another curve"
        );
    }
}
