// SPDX-License-Identifier: GPL-3.0-only
//! Meteorological-data processing as pyDOSEIA does it: hourly records to a
//! **triple joint frequency distribution** (TJFD: stability class x wind-speed
//! class x wind-direction sector), the missing-data correction, the calm
//! correction and the per-class speed distribution.
//!
//! # Provenance
//!
//! Ported from pyDOSEIA `metfunc.py` (`MetFunc.file_preprocessing`,
//! `met_data_to_tjfd`, `missing_correction`, `calm_correction_factor_calc`,
//! `speed_distribution_list`), upstream
//! <https://github.com/BiswajitSadhu/pyDOSEIA> at commit
//! `dca4cdc3bb0bef7f7e692c8991cf536c91e7a4ce`. Copyright (c) 2024 Dr. Biswajit
//! Sadhu; MIT licence (full notice in `crates/buangkok/NOTICE`). Upstream cites
//! the Hukkoo-Bapat BARC manual for the calm correction.
//!
//! Upstream reads an Excel workbook (one sheet per year). This port takes the
//! records already parsed ([`RawMetRecord`]); reading a spreadsheet is left to
//! the caller. Everything from the gap filling onward is ported.
//!
//! # Units
//!
//! Wind speeds are **km/h** here, as in upstream's input column
//! (`WS 10m(kmph)`) and its speed-class edges. They are converted to m/s only
//! inside the dilution factor (`/ 3.6`). Directions are degrees (the direction
//! the wind blows *from*, as recorded). Records are plain `f64` because they
//! mirror a spreadsheet row, including upstream's `999` / `9` missing-value
//! sentinels.

/// Upstream's wind-speed class edges, km/h (10 classes; class 0 is calm).
pub const WSRANGE_KMPH: [f64; 11] = [0.0, 1.8, 3.0, 5.5, 11.5, 19.5, 29.5, 38.5, 50.5, 61.5, 74.5];

/// Upstream's wind-direction bin edges, degrees (17 bins; the first and last
/// are folded into sector 0).
pub const WDRANGE_DEG: [f64; 18] = [
    0.0, 11.25, 33.75, 56.25, 78.75, 101.25, 123.75, 146.25, 168.75, 191.25, 213.75, 236.25,
    258.75, 281.25, 303.75, 326.25, 348.75, 360.0,
];

/// Representative speed of each speed class, km/h (upstream `WSPEED_K` before
/// its `/ 3.6`). Class 0 (calm) is never used in a dilution factor.
pub const WSPEED_K_KMPH: [f64; 10] = [0.9, 2.4, 4.25, 8.5, 15.5, 24.5, 34.0, 44.5, 56.0, 68.0];

/// Number of wind-speed classes.
pub const SPEED_CLASS_COUNT: usize = 10;
/// Number of direction sectors.
pub const SECTOR_COUNT: usize = 16;
/// Index of the calm speed class.
pub const CALM_SPEED_CLASS: usize = 0;

/// Upstream's fill value for a missing speed or direction.
pub const MISSING_VALUE: f64 = 999.0;
/// Upstream's code for a missing stability class (`'I'` = `ord('I') - 64`).
pub const MISSING_STABILITY: f64 = 9.0;

/// One stability class's joint frequency table: `[speed class][sector]`.
pub type FrequencyTable<T> = [[T; SECTOR_COUNT]; SPEED_CLASS_COUNT];

/// One hourly record as read from upstream's spreadsheet, before gap filling.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RawMetRecord {
    /// Hour of day (upstream's first column), used only for the operating-hours
    /// filter.
    pub hour: f64,
    /// Wind speed at the measurement height, km/h; `None` if blank.
    pub speed_kmph: Option<f64>,
    /// Wind direction, degrees; `None` if blank.
    pub direction_deg: Option<f64>,
    /// Stability class code: `1..=6`, or a letter's `ord - 64` (`'A'` = 1);
    /// `None` if blank.
    pub stability_code: Option<f64>,
}

/// A record after upstream's gap filling: `[speed, direction, stability]`
/// with `999` for a missing speed/direction and `9` for a missing class.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MetRecord {
    /// Wind speed, km/h (or [`MISSING_VALUE`]).
    pub speed_kmph: f64,
    /// Wind direction, degrees (or [`MISSING_VALUE`]).
    pub direction_deg: f64,
    /// Stability code (or [`MISSING_STABILITY`]).
    pub stability_code: f64,
}

/// Stability letter to upstream's code, `ord(letter) - 64` (`'A'` = 1).
#[must_use]
pub fn stability_code_from_letter(letter: char) -> f64 {
    f64::from(u32::from(letter)) - 64.0
}

/// Upstream's `file_preprocessing` for one sheet: keep the hours in
/// `[start, end]` (inclusive), fill blanks with `999` (speed, direction) or
/// `9` (stability).
#[must_use]
pub fn preprocess_records(raw: &[RawMetRecord], start_hour: f64, end_hour: f64) -> Vec<MetRecord> {
    raw.iter()
        .filter(|r| r.hour >= start_hour && r.hour <= end_hour)
        .map(|r| MetRecord {
            speed_kmph: r.speed_kmph.unwrap_or(MISSING_VALUE),
            direction_deg: r.direction_deg.unwrap_or(MISSING_VALUE),
            stability_code: r.stability_code.unwrap_or(MISSING_STABILITY),
        })
        .collect()
}

/// Upstream's count of **invalid** records: direction above 360 degrees or
/// stability code above 6.
///
/// Note (upstream behaviour, kept): a record with a missing *speed* (999) but
/// a valid direction and class is counted as **valid**, yet it lands in no
/// speed class (999 km/h is beyond the last edge) and so is silently dropped
/// from the TJFD without being redistributed by the missing correction.
#[must_use]
pub fn invalid_record_count(records: &[MetRecord]) -> usize {
    records
        .iter()
        .filter(|r| r.direction_deg > 360.0 || r.stability_code > 6.0)
        .count()
}

/// `numpy.histogram2d` bin index for explicit edges: `edges[i] <= v <
/// edges[i+1]`, the last bin closed on the right; `None` outside.
fn bin_index(edges: &[f64], v: f64) -> Option<usize> {
    let n = edges.len();
    if !(v >= edges[0] && v <= edges[n - 1]) {
        return None;
    }
    if v == edges[n - 1] {
        return Some(n - 2);
    }
    // searchsorted(side='right') - 1
    let idx = edges.partition_point(|e| *e <= v);
    Some(idx - 1)
}

/// Upstream's `met_data_to_tjfd` for one year: a per-class histogram of speed
/// class against direction bin, with the first (`[0, 11.25)`) and last
/// (`[348.75, 360]`) direction bins folded into sector 0.
///
/// Counts are `f64` because upstream's histogram is floating point.
#[must_use]
pub fn tjfd_from_records(records: &[MetRecord]) -> [FrequencyTable<f64>; 6] {
    let mut out = [[[0.0; SECTOR_COUNT]; SPEED_CLASS_COUNT]; 6];
    for (s, table) in out.iter_mut().enumerate() {
        let code = (s + 1) as f64;
        let mut raw = [[0.0_f64; 17]; SPEED_CLASS_COUNT];
        for r in records.iter().filter(|r| r.stability_code == code) {
            if let (Some(i), Some(j)) = (
                bin_index(&WSRANGE_KMPH, r.speed_kmph),
                bin_index(&WDRANGE_DEG, r.direction_deg),
            ) {
                raw[i][j] += 1.0;
            }
        }
        for (k, row) in raw.iter().enumerate() {
            table[k][0] = row[0] + row[16];
            table[k][1..SECTOR_COUNT].copy_from_slice(&row[1..SECTOR_COUNT]);
        }
    }
    out
}

/// Upstream's `missing_correction` for one year: each count is scaled by
/// `1 + invalid / valid` and **truncated to an integer** (`astype(int)`), as
/// upstream does.
#[must_use]
pub fn missing_correction(
    tjfd: &[FrequencyTable<f64>; 6],
    invalid: usize,
    total: usize,
) -> [FrequencyTable<i64>; 6] {
    let valid = total - invalid;
    let factor = invalid as f64 / valid as f64;
    let mut out = [[[0_i64; SECTOR_COUNT]; SPEED_CLASS_COUNT]; 6];
    for s in 0..6 {
        for k in 0..SPEED_CLASS_COUNT {
            for j in 0..SECTOR_COUNT {
                let t = tjfd[s][k][j];
                out[s][k][j] = (t + (t * factor)) as i64;
            }
        }
    }
    out
}

/// Upstream's calm-correction factors, one per sector:
/// `1 + N_0 N_JL / (N_L N_J)`, with `N_0` the calm count, `N_J` the non-calm
/// count in sector `J`, `N_JL` the count in the lowest non-calm speed class in
/// sector `J` (all over the six classes).
///
/// **Upstream defect D1, reproduced here:** upstream computes `N_L` as
/// `TJFD.reshape(6,10,16).sum(axis=1).sum(axis=0)[1]`, which is the total count
/// in **direction sector 1 over all speed classes** (including calm), not the
/// total in the lowest speed class over all sectors that the formula calls
/// for. [`calm_correction_factors_lowest_speed_class`] is the corrected
/// variant. A sector with `N_J = 0` gives `inf`/NaN, as upstream.
#[must_use]
pub fn calm_correction_factors(tjfd: &[FrequencyTable<i64>; 6]) -> [f64; SECTOR_COUNT] {
    let n_l: i64 = tjfd
        .iter()
        .map(|t| t.iter().map(|row| row[1]).sum::<i64>())
        .sum();
    calm_factors_with_n_l(tjfd, n_l)
}

/// **Divergence from upstream (D1 corrected).** As
/// [`calm_correction_factors`] but with `N_L` = the total count in the lowest
/// non-calm speed class (class 1) over every sector and stability class. Not
/// verified against the Hukkoo-Bapat manual itself, which was not available;
/// the correction follows from the formula's own definition of `N_L`.
#[must_use]
pub fn calm_correction_factors_lowest_speed_class(
    tjfd: &[FrequencyTable<i64>; 6],
) -> [f64; SECTOR_COUNT] {
    let n_l: i64 = tjfd.iter().map(|t| t[1].iter().sum::<i64>()).sum();
    calm_factors_with_n_l(tjfd, n_l)
}

fn calm_factors_with_n_l(tjfd: &[FrequencyTable<i64>; 6], n_l: i64) -> [f64; SECTOR_COUNT] {
    let n_0: i64 = tjfd
        .iter()
        .map(|t| t[CALM_SPEED_CLASS].iter().sum::<i64>())
        .sum();
    let mut out = [0.0; SECTOR_COUNT];
    for (direc, o) in out.iter_mut().enumerate() {
        let n_j: i64 = tjfd
            .iter()
            .map(|t| (1..SPEED_CLASS_COUNT).map(|k| t[k][direc]).sum::<i64>())
            .sum();
        let n_jl: i64 = tjfd.iter().map(|t| t[1][direc]).sum();
        *o = 1.0 + ((n_0 * n_jl) as f64 / (n_l * n_j) as f64);
    }
    out
}

/// `numpy.quantile(..., method='linear')` on unsorted data (numpy's `_lerp`).
fn numpy_linear_quantile(values: &[f64], q: f64) -> f64 {
    let mut v = values.to_vec();
    v.sort_by(f64::total_cmp);
    let n = v.len();
    if n == 0 {
        return f64::NAN;
    }
    let virtual_index = (n as f64 - 1.0) * q;
    let prev = virtual_index.floor();
    let lo = (prev as usize).min(n - 1);
    let hi = (lo + 1).min(n - 1);
    let gamma = virtual_index - prev;
    let (a, b) = (v[lo], v[hi]);
    let diff = b - a;
    if gamma >= 0.5 {
        b - diff * (1.0 - gamma)
    } else {
        a + diff * gamma
    }
}

/// Upstream's `speed_distribution_list`: for each stability class, the speeds
/// strictly below the class's `quantile` (default 0.90), and their mean, km/h.
///
/// Records with a missing speed (999 km/h) are **included** in the quantile,
/// as upstream does; with fewer than 10 % missing they are then cut by the
/// 0.9 quantile. A class with no records gives a NaN mean.
///
/// Upstream uses these means (km/h) to divide a single-plume dilution factor
/// computed at a 1 m/s reference speed, but that path fails its own shape
/// assertion (defect D3). ~~so nothing in this port consumes them~~
/// **CHANGED 2026-09-28:** the labelled divergence
/// [`super::dispersion::dilution_single_plume_with_met_speeds`] uses them
/// (converted to m/s).
#[must_use]
pub fn speed_distribution(records: &[MetRecord], quantile: f64) -> ([Vec<f64>; 6], [f64; 6]) {
    let mut kept: [Vec<f64>; 6] = Default::default();
    let mut means = [0.0; 6];
    for s in 0..6 {
        let code = (s + 1) as f64;
        let speeds: Vec<f64> = records
            .iter()
            .filter(|r| r.stability_code == code)
            .map(|r| r.speed_kmph)
            .collect();
        let cut = numpy_linear_quantile(&speeds, quantile);
        kept[s] = speeds.into_iter().filter(|v| *v < cut).collect();
        means[s] = if kept[s].is_empty() {
            f64::NAN
        } else {
            kept[s].iter().sum::<f64>() / kept[s].len() as f64
        };
    }
    (kept, means)
}

/// One year (one upstream sheet) of processed met data.
#[derive(Debug, Clone, PartialEq)]
pub struct MetYear {
    /// Number of days the year's data covers (upstream `num_days`).
    pub num_days: i64,
    /// The gap-filled records inside the operating hours.
    pub records: Vec<MetRecord>,
    /// The raw TJFD, per stability class.
    pub tjfd: [FrequencyTable<f64>; 6],
    /// The missing-corrected, integer TJFD, per stability class.
    pub missing_corrected: [FrequencyTable<i64>; 6],
}

/// Several years of met data processed as upstream does, ready for
/// `dispersion::dilution_long_term_with_met`.
#[derive(Debug, Clone, PartialEq)]
pub struct MetClimatology {
    /// First operating hour kept (upstream `start_operation_time`).
    pub start_hour: i64,
    /// Last operating hour kept (upstream `end_operation_time`).
    pub end_hour: i64,
    /// One entry per year (upstream sheet), in input order.
    pub years: Vec<MetYear>,
}

impl MetClimatology {
    /// Process raw records, one slice per year with its day count, through
    /// gap filling, the TJFD and the missing correction.
    #[must_use]
    pub fn from_raw_years(
        years: &[(&[RawMetRecord], i64)],
        start_hour: i64,
        end_hour: i64,
    ) -> Self {
        let years = years
            .iter()
            .map(|(raw, num_days)| {
                let records = preprocess_records(raw, start_hour as f64, end_hour as f64);
                let tjfd = tjfd_from_records(&records);
                let missing_corrected =
                    missing_correction(&tjfd, invalid_record_count(&records), records.len());
                MetYear {
                    num_days: *num_days,
                    records,
                    tjfd,
                    missing_corrected,
                }
            })
            .collect();
        Self {
            start_hour,
            end_hour,
            years,
        }
    }

    /// Operating hours per day, `|start - end|` (upstream's definition; note
    /// that the hour filter itself is inclusive at both ends).
    #[must_use]
    pub fn operation_hours_per_day(&self) -> i64 {
        (self.start_hour - self.end_hour).abs()
    }

    /// All years' records concatenated (upstream's input to the speed
    /// distribution).
    #[must_use]
    pub fn all_records(&self) -> Vec<MetRecord> {
        self.years
            .iter()
            .flat_map(|y| y.records.iter().copied())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn histogram_edges_follow_numpy() {
        assert_eq!(bin_index(&WDRANGE_DEG, 0.0), Some(0));
        assert_eq!(bin_index(&WDRANGE_DEG, 11.25), Some(1));
        assert_eq!(bin_index(&WDRANGE_DEG, 360.0), Some(16));
        assert_eq!(bin_index(&WDRANGE_DEG, 360.1), None);
        assert_eq!(bin_index(&WSRANGE_KMPH, 74.5), Some(9));
        assert_eq!(bin_index(&WSRANGE_KMPH, 999.0), None);
    }

    #[test]
    fn linear_quantile_matches_numpy_on_a_small_case() {
        // numpy.quantile([1, 2, 3, 999], 0.9) = 700.2 (checked in the venv).
        let q = numpy_linear_quantile(&[1.0, 2.0, 3.0, 999.0], 0.9);
        assert!((q - 700.2).abs() < 1e-9, "{q}");
    }

    #[test]
    fn letter_codes_match_upstream() {
        assert_eq!(stability_code_from_letter('A'), 1.0);
        assert_eq!(stability_code_from_letter('F'), 6.0);
        assert_eq!(stability_code_from_letter('I'), MISSING_STABILITY);
    }
}
