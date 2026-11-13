//! The precomputed artefacts the twin shows, and the grid they live on.
//!
//! **Nothing here is computed in the browser.** The Kennedy–O'Hagan
//! calibration (gh:#1180, ~19 min) and the co-kriging fit (gh:#1190) are far
//! too slow for a page, so `--generate-artefacts` ([`crate::generate`]) runs
//! them natively, once, from the twin engine's own code, and writes:
//!
//! - [`GRID_CSV`]: on [`grid`] (9 × 9 × 3 × 5 = 1215 points), the 1-D UA at
//!   nominal θ and at the posterior-mean θ, the KOH predictive of UA (η + δ,
//!   with the CFD error ε, as gh:#1180's held-out check), δ(x) on ln UA, and
//!   the UA-only MFK mean and σ;
//! - [`THETA_CSV`]: every 10th of the 2 000 posterior θ samples, and the
//!   posterior mean.
//!
//! The CFD design points are the twin engine's committed dataset
//! (`dataset::load_committed`, gh:#1178) and the filtering traces are
//! gh:#1191's committed `traces.csv`; both are read as they are.
//!
//! The record (methodology, provenance, the regenerated held-out numbers
//! against gh:#1180's and gh:#1190's): ~~`crates/dhoby-ghaut/...`~~
//! `crates/outram-park-digital-twin-engine/verification_and_validation/
//! double_pipe_twin_web_gh1182/README.md` (moved with this example from
//! dhoby-ghaut, 2026-11-05).

// The generator's writers and the native tests' fields are unused in the
// browser build.
#![cfg_attr(target_arch = "wasm32", allow(dead_code))]

use outram_park_digital_twin_engine::multifidelity::double_pipe::design::Split;
use outram_park_digital_twin_engine::multifidelity::double_pipe::{
    fouling_si, lf_grid, DoublePipeInput, Theta,
};
use uom::si::f64::Ratio;
use uom::si::ratio::ratio;

/// The grid artefact (written by [`crate::generate`]).
pub const GRID_CSV: &str =
    include_str!("../../verification_and_validation/double_pipe_twin_web_gh1182/koh_mfk_grid.csv");
/// The θ posterior artefact (written by [`crate::generate`]).
pub const THETA_CSV: &str = include_str!(
    "../../verification_and_validation/double_pipe_twin_web_gh1182/koh_theta_samples.csv"
);
/// gh:#1191's committed filtering traces (the twin engine's V&V folder).
pub const TRACES_CSV: &str = include_str!("../../verification_and_validation/double_pipe_filtering_gh1191/traces.csv");

/// Grid points along each Reynolds number.
pub const N_RE: usize = 9;

/// Nine levels, geometric: `lf_grid`'s five (which hold the training and
/// held-out Reynolds numbers exactly) and the geometric midpoint of each
/// neighbouring pair between them.
fn nine(five: [f64; 5]) -> [f64; N_RE] {
    std::array::from_fn(|i| {
        if i % 2 == 0 {
            five[i / 2]
        } else {
            (five[i / 2] * five[i / 2 + 1]).sqrt()
        }
    })
}

/// Re_hot levels, 10 … 90.
#[must_use]
pub fn re_hot_levels() -> [f64; N_RE] {
    nine(lf_grid::re_hot_levels())
}

/// Re_cold levels, 5 … 45.
#[must_use]
pub fn re_cold_levels() -> [f64; N_RE] {
    nine(lf_grid::re_cold_levels())
}

/// k_wall levels \[W/(m K)\]: `lf_grid`'s (TUAS's wall levels without 16).
pub const K_LEVELS: [f64; 3] = lf_grid::K_WALL_LEVELS;
/// Length levels \[m\]: `lf_grid`'s.
pub const L_LEVELS: [f64; 5] = lf_grid::LENGTH_LEVELS;

/// Index of a grid point: Re_hot slowest, L fastest (as `lf_grid`).
#[must_use]
pub fn index(i_rh: usize, i_rc: usize, i_k: usize, i_l: usize) -> usize {
    ((i_rh * N_RE + i_rc) * K_LEVELS.len() + i_k) * L_LEVELS.len() + i_l
}

/// The 1215 grid inputs in [`index`] order: the twin engine's design-sweep
/// API (gh:#1181, `cokriging::SweepAxes::full_factorial`) on these levels.
#[must_use]
pub fn grid() -> Vec<DoublePipeInput> {
    sweep_axes().full_factorial()
}

/// The grid's levels as the twin engine's sweep axes.
#[must_use]
pub fn sweep_axes() -> outram_park_digital_twin_engine::multifidelity::double_pipe::cokriging::SweepAxes {
    use uom::si::f64::{Length, Ratio, ThermalConductivity};
    use uom::si::{length::meter, ratio::ratio, thermal_conductivity::watt_per_meter_kelvin};
    outram_park_digital_twin_engine::multifidelity::double_pipe::cokriging::SweepAxes {
        re_hot: re_hot_levels().iter().map(|&v| Ratio::new::<ratio>(v)).collect(),
        re_cold: re_cold_levels().iter().map(|&v| Ratio::new::<ratio>(v)).collect(),
        k_wall: K_LEVELS.iter().map(|&v| ThermalConductivity::new::<watt_per_meter_kelvin>(v)).collect(),
        length: L_LEVELS.iter().map(|&v| Length::new::<meter>(v)).collect(),
    }
}

/// One grid row. UA in W/K; the KOH mean and sd and δ on ln(UA / (W/K)).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GridRow {
    pub x: [f64; 4],
    pub ua_nominal: f64,
    pub ua_post_mean: f64,
    pub koh_median: f64,
    pub koh_lower95: f64,
    pub koh_upper95: f64,
    pub koh_mean_ln: f64,
    pub koh_sd_ln: f64,
    pub delta_mean: f64,
    pub delta_sd: f64,
    pub mfk_mean: f64,
    pub mfk_sd: f64,
}

/// [`GridRow`]'s columns, the header the generator writes.
pub const GRID_HEADER: &str = "re_hot,re_cold,k_wall_W_per_mK,length_m,ua_1d_nominal_W_per_K,ua_1d_post_mean_W_per_K,koh_median_W_per_K,koh_lower95_W_per_K,koh_upper95_W_per_K,koh_mean_lnUA,koh_sd_lnUA,delta_mean_lnUA,delta_sd_lnUA,mfk_mean_W_per_K,mfk_sd_W_per_K";

impl GridRow {
    /// The row as CSV, every number in Rust's shortest round-trip form.
    #[must_use]
    pub fn to_csv(&self) -> String {
        let v = self.values();
        v.iter()
            .map(|x| format!("{x}"))
            .collect::<Vec<_>>()
            .join(",")
    }

    fn values(&self) -> [f64; 15] {
        let [a, b, c, d] = self.x;
        [
            a,
            b,
            c,
            d,
            self.ua_nominal,
            self.ua_post_mean,
            self.koh_median,
            self.koh_lower95,
            self.koh_upper95,
            self.koh_mean_ln,
            self.koh_sd_ln,
            self.delta_mean,
            self.delta_sd,
            self.mfk_mean,
            self.mfk_sd,
        ]
    }

    fn from_values(v: &[f64]) -> Option<Self> {
        if v.len() != 15 {
            return None;
        }
        Some(Self {
            x: [v[0], v[1], v[2], v[3]],
            ua_nominal: v[4],
            ua_post_mean: v[5],
            koh_median: v[6],
            koh_lower95: v[7],
            koh_upper95: v[8],
            koh_mean_ln: v[9],
            koh_sd_ln: v[10],
            delta_mean: v[11],
            delta_sd: v[12],
            mfk_mean: v[13],
            mfk_sd: v[14],
        })
    }
}

/// Numeric rows of a CSV: `#` lines and the first non-comment line (the
/// header) skipped.
fn numeric_rows(csv: &str) -> Result<Vec<Vec<f64>>, String> {
    csv.lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
        .skip(1)
        .map(|l| {
            l.split(',')
                .map(|c| c.trim().parse::<f64>().map_err(|e| format!("{c:?}: {e}")))
                .collect()
        })
        .collect()
}

/// The `#` comment lines of a CSV, without the `# `.
#[must_use]
pub fn comments(csv: &str) -> Vec<&str> {
    csv.lines()
        .filter_map(|l| l.strip_prefix('#'))
        .map(str::trim)
        .collect()
}

/// Parse [`GRID_CSV`]-format text, checking that it is [`grid`] in order.
///
/// # Errors
/// A malformed row, the wrong count, or a row off the grid.
pub fn parse_grid(csv: &str) -> Result<Vec<GridRow>, String> {
    let rows: Vec<GridRow> = numeric_rows(csv)?
        .iter()
        .map(|v| GridRow::from_values(v).ok_or_else(|| format!("{} columns, want 15", v.len())))
        .collect::<Result<_, _>>()?;
    let g = grid();
    if rows.len() != g.len() {
        return Err(format!("{} rows, want {}", rows.len(), g.len()));
    }
    for (r, x) in rows.iter().zip(&g) {
        if r.x != x.to_si() {
            return Err(format!("row {:?} is not grid point {:?}", r.x, x.to_si()));
        }
    }
    Ok(rows)
}

/// The θ posterior: the mean the generator read off raffles
/// (`ThetaEstimate::point`) and the thinned samples, each
/// `[ln θ_h, ln θ_c, r_f (m² K/W)]`.
#[derive(Debug, Clone, PartialEq)]
pub struct ThetaPosterior {
    /// `Theta`'s SI coordinates: θ_h, θ_c, r_f, friction multipliers.
    pub mean_si: [f64; 5],
    pub samples: Vec<[f64; 3]>,
}

impl ThetaPosterior {
    /// The posterior-mean θ as the twin engine's [`Theta`].
    #[must_use]
    pub fn mean_theta(&self) -> Theta {
        theta_from(self.mean_si[0], self.mean_si[1], self.mean_si[2])
    }

    /// Sample `i` as a [`Theta`] (friction at nominal).
    #[must_use]
    pub fn sample_theta(&self, i: usize) -> Theta {
        let [lh, lc, rf] = self.samples[i];
        theta_from(lh.exp(), lc.exp(), rf)
    }
}

/// A [`Theta`] from θ_h, θ_c and r_f (SI), friction at nominal.
#[must_use]
pub fn theta_from(nu_hot: f64, nu_cold: f64, r_f: f64) -> Theta {
    let mut t = Theta::nominal();
    t.nu_mult_hot = Ratio::new::<ratio>(nu_hot);
    t.nu_mult_cold = Ratio::new::<ratio>(nu_cold);
    t.wall_resistance = fouling_si(r_f);
    t
}

/// The comment line that carries the posterior mean.
pub const MEAN_TAG: &str = "posterior_mean_theta_si:";

/// Parse [`THETA_CSV`]-format text.
///
/// # Errors
/// No mean line, or a malformed row.
pub fn parse_theta(csv: &str) -> Result<ThetaPosterior, String> {
    let mean_line = comments(csv)
        .into_iter()
        .find_map(|c| c.strip_prefix(MEAN_TAG))
        .ok_or("no posterior-mean line")?;
    let m: Vec<f64> = mean_line
        .split(',')
        .map(|c| c.trim().parse::<f64>().map_err(|e| e.to_string()))
        .collect::<Result<_, _>>()?;
    let mean_si: [f64; 5] = m
        .try_into()
        .map_err(|_| "posterior mean needs 5 numbers".to_string())?;
    let samples = numeric_rows(csv)?
        .into_iter()
        .map(|v| <[f64; 3]>::try_from(v).map_err(|_| "theta row needs 3 numbers".to_string()))
        .collect::<Result<Vec<_>, _>>()?;
    if samples.is_empty() {
        return Err("no theta samples".into());
    }
    Ok(ThetaPosterior { mean_si, samples })
}

/// One CFD design point, as the twin shows it.
#[derive(Debug, Clone, PartialEq)]
pub struct CfdPoint {
    pub id: String,
    pub training: bool,
    pub x: [f64; 4],
    pub ua: f64,
}

/// The 44 committed CFD rows (gh:#1178), UA only.
#[must_use]
pub fn cfd_points() -> Vec<CfdPoint> {
    use uom::si::thermal_conductance::watt_per_kelvin;
    outram_park_digital_twin_engine::multifidelity::double_pipe::dataset::load_committed()
        .into_iter()
        .map(|r| CfdPoint {
            id: r.id,
            training: r.split == Split::Training,
            x: r.input.to_si(),
            ua: r.output.ua.get::<watt_per_kelvin>(),
        })
        .collect()
}

/// One step of a committed filtering trace (gh:#1191), r_f in m² K/W.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TraceStep {
    pub step: usize,
    pub truth: f64,
    pub estimate: f64,
    pub sigma: f64,
    pub nees: f64,
    pub nis: f64,
}

/// The `(experiment, filter)` trace of [`TRACES_CSV`]-format text.
///
/// # Errors
/// A malformed row, or no row for that pair.
pub fn trace(csv: &str, experiment: &str, filter: &str) -> Result<Vec<TraceStep>, String> {
    let mut out = Vec::new();
    for line in csv.lines().skip(1) {
        let c: Vec<&str> = line.split(',').collect();
        if c.len() != 8 || c[0] != experiment || c[1] != filter {
            continue;
        }
        let f = |i: usize| c[i].parse::<f64>().map_err(|e| format!("{line}: {e}"));
        out.push(TraceStep {
            step: c[2].parse().map_err(|e| format!("{line}: {e}"))?,
            truth: f(3)?,
            estimate: f(4)?,
            sigma: f(5)?,
            nees: f(6)?,
            nis: f(7)?,
        });
    }
    if out.is_empty() {
        return Err(format!("no trace for {experiment} / {filter}"));
    }
    Ok(out)
}

/// Mean of `f` over the steps after the filter's burn-in (gh:#1191 scores
/// steps 31–200).
#[must_use]
pub fn mean_after_burn_in(t: &[TraceStep], f: impl Fn(&TraceStep) -> f64) -> f64 {
    let after: Vec<f64> = t
        .iter()
        .filter(|s| {
            s.step > outram_park_digital_twin_engine::multifidelity::double_pipe::filtering::BURN_IN
        })
        .map(f)
        .collect();
    after.iter().sum::<f64>() / after.len().max(1) as f64
}

/// Everything the twin draws from committed files, parsed once.
#[derive(Debug, Clone)]
pub struct Artefacts {
    pub grid: Vec<GridRow>,
    pub theta: ThetaPosterior,
    pub cfd: Vec<CfdPoint>,
    /// gh:#1191's UKF on P-A (closures known) and P-B (textbook closures).
    pub trace_pa: Vec<TraceStep>,
    pub trace_pb: Vec<TraceStep>,
    /// The grid file's provenance comment lines.
    pub provenance: Vec<String>,
}

impl Artefacts {
    /// Parse the compiled-in files.
    ///
    /// # Errors
    /// Any parse error (pinned by the tests, so never at run time).
    pub fn load() -> Result<Self, String> {
        Ok(Self {
            grid: parse_grid(GRID_CSV)?,
            theta: parse_theta(THETA_CSV)?,
            cfd: cfd_points(),
            trace_pa: trace(TRACES_CSV, "P-A", "Unscented")?,
            trace_pb: trace(TRACES_CSV, "P-B", "Unscented")?,
            provenance: comments(GRID_CSV).into_iter().map(str::to_string).collect(),
        })
    }

    /// The grid row at an index tuple.
    #[must_use]
    pub fn at(&self, i_rh: usize, i_rc: usize, i_k: usize, i_l: usize) -> &GridRow {
        &self.grid[index(i_rh, i_rc, i_k, i_l)]
    }

    /// The CFD points on the slice (Re_cold, k_wall, L) = grid levels
    /// `(i_rc, i_k, i_l)`, to 1e-9 relative.
    #[must_use]
    pub fn cfd_on_slice(&self, i_rc: usize, i_k: usize, i_l: usize) -> Vec<&CfdPoint> {
        let want = [re_cold_levels()[i_rc], K_LEVELS[i_k], L_LEVELS[i_l]];
        let near = |a: f64, b: f64| (a / b - 1.0).abs() < 1e-9;
        self.cfd
            .iter()
            .filter(|p| near(p.x[1], want[0]) && near(p.x[2], want[1]) && near(p.x[3], want[2]))
            .collect()
    }
}

/// The nearest level to `v` on a log axis.
#[must_use]
pub fn nearest_level(levels: &[f64], v: f64) -> usize {
    let mut best = 0;
    for (i, l) in levels.iter().enumerate() {
        if (l.ln() - v.ln()).abs() < (levels[best].ln() - v.ln()).abs() {
            best = i;
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The grid holds lf_grid's levels exactly (so the training and held-out
    /// Reynolds numbers), and every CFD point lies on it.
    #[test]
    fn the_grid_holds_every_design_point() {
        let (rh, rc) = (re_hot_levels(), re_cold_levels());
        for (i, v) in lf_grid::re_hot_levels().iter().enumerate() {
            assert_eq!(rh[2 * i], *v);
        }
        assert!(rh.windows(2).all(|w| w[1] > w[0]) && rc.windows(2).all(|w| w[1] > w[0]));
        let g = grid();
        assert_eq!(g.len(), 1215);
        assert_eq!(g[index(8, 3, 2, 4)].to_si(), [90.0, rc[3], 100.0, 0.3]);
        let cfd = cfd_points();
        assert_eq!(cfd.len(), 44);
        assert_eq!(cfd.iter().filter(|p| p.training).count(), 36);
        for p in &cfd {
            assert!(
                g.iter().any(|x| x.to_si() == p.x),
                "{} {:?} off the grid",
                p.id,
                p.x
            );
        }
        assert_eq!(nearest_level(&rh, 29.0), 4);
        assert_eq!(nearest_level(&rh, 1.0), 0);
        assert_eq!(nearest_level(&rh, 1e4), 8);
    }

    /// The committed artefacts parse, are on the grid, and are physically
    /// ordered (a harness check, not V&V): positive UA, a 95 % interval that
    /// holds the median, finite δ.
    #[test]
    fn the_committed_artefacts_parse_and_are_sane() {
        let a = Artefacts::load().expect("artefacts");
        assert_eq!(a.grid.len(), 1215);
        assert!(a.theta.samples.len() >= 100);
        for r in &a.grid {
            assert!(
                r.ua_nominal > 0.0 && r.ua_post_mean > 0.0 && r.mfk_mean > 0.0 && r.mfk_sd >= 0.0
            );
            assert!(
                r.koh_lower95 < r.koh_median && r.koh_median < r.koh_upper95,
                "{r:?}"
            );
            assert!(r.delta_mean.is_finite() && r.delta_sd > 0.0 && r.koh_sd_ln > 0.0);
        }
        assert!(a.provenance.iter().any(|l| l.contains("gh:#1182")));
        assert_eq!(a.trace_pa.len(), 200);
        assert_eq!(a.trace_pb.len(), 200);
        // Slices that carry CFD points: the training corner and H1's.
        let rc = re_cold_levels();
        assert_eq!(
            a.cfd_on_slice(0, 0, 0).len(),
            3,
            "Re_cold 5, k 1, L 0.1: Re_hot 10, 30, 90"
        );
        let h1 = a.cfd.iter().find(|p| p.id == "H1").unwrap();
        let i_rc = nearest_level(&rc, h1.x[1]);
        assert!(a.cfd_on_slice(i_rc, 1, 2).iter().any(|p| p.id == "H1"));
        assert!(trace(TRACES_CSV, "P-Z", "Unscented").is_err());
        assert!(parse_grid("a\n1,2\n").is_err());
        assert!(parse_theta("# nothing\nh\n1,2,3\n").is_err());
        assert_eq!(
            a.theta.mean_theta().nu_mult_hot.get::<ratio>(),
            a.theta.mean_si[0]
        );
        let s0 = a.theta.sample_theta(0);
        assert!((s0.nu_mult_cold.get::<ratio>().ln() - a.theta.samples[0][1]).abs() < 1e-12);
    }

    /// gh:#1191's P-B is the "confidently wrong" case: its NIS is as normal
    /// as P-A's while its r_f is biased by about −4.4e-4 m² K/W (the record's
    /// numbers, read off the committed trace here).
    #[test]
    fn p_b_is_confidently_wrong_in_the_committed_trace() {
        let a = Artefacts::load().expect("artefacts");
        let bias_a = mean_after_burn_in(&a.trace_pa, |s| s.estimate - s.truth);
        let bias_b = mean_after_burn_in(&a.trace_pb, |s| s.estimate - s.truth);
        let (nis_a, nis_b) = (
            mean_after_burn_in(&a.trace_pa, |s| s.nis),
            mean_after_burn_in(&a.trace_pb, |s| s.nis),
        );
        assert!(bias_a.abs() < 3e-5, "{bias_a}");
        assert!((bias_b + 4.4e-4).abs() < 0.3e-4, "{bias_b}");
        assert!(
            (nis_a - nis_b).abs() < 0.5 && nis_b < 3.0,
            "{nis_a} {nis_b}"
        );
        assert!(mean_after_burn_in(&a.trace_pb, |s| s.nees) > 100.0);
    }
}
