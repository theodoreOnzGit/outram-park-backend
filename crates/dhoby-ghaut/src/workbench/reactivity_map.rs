//! # Step 6 (A): the reactivity map, a LOW-FIDELITY neutronics-only surrogate
//!
//! A Chebyshev fit of the reactivity `ρ(state)` over the Monte Carlo runs
//! saved in Step 5, exported as a **plain-TOML** config file for DOVER and the
//! digital-twin simulators (gh:#571; plain TOML for light data transfer is the
//! maintainer's decision of 2026-10-05, gh:#576).
//!
//! **What it is, and what it is not.** It is a fit to eigenvalue runs: no
//! thermal-hydraulics, no feedback model, no kinetics parameters, no spatial
//! shape. It interpolates `ρ` between the states that were actually run and
//! **refuses to extrapolate** outside them. It is a data-driven surrogate, so
//! by the workspace's model-hierarchy rule it sits below a physics-derived one:
//! its only claim is the held-out error it reports.
//!
//! ## The state axes
//!
//! | axis | in this build |
//! |---|---|
//! | temperature \[K\] | **isothermal**: Step 5 runs every material at one data temperature, so fuel and moderator temperature are ONE axis here, not two (separating them: gh:#590) |
//! | control-rod insertion | ~~the HTR-10 model has its rods WITHDRAWN only (gh:#580), so every run sits at 0~~ **UPDATED 2026-10-05:** all ten rods move together from 0 (withdrawn) to 1 (fully in) since gh:#580; each run records the insertion its core was built to. Runs at one insertion leave the axis *fixed*; the planner sweeps temperature at the recipe's current insertion |
//!
//! An axis whose runs all share one value is not fitted; it is written as a
//! `[[fixed]]` entry and the map refuses any other value on it.
//!
//! ## The fit (methodology)
//!
//! - **Quantity.** `ρ = (k − 1)/k` in pcm, with `σ_ρ = σ_k / k²` (first-order
//!   propagation of the run's own 1σ).
//! - **Basis.** A total-degree Chebyshev basis `Π_a T_{d_a}(s_a)`,
//!   `Σ d_a ≤ order`, in each varying axis' scaled coordinate
//!   `s = (2u − u_lo − u_hi)/(u_hi − u_lo)` on the runs' range. Temperature
//!   enters through `u = √T` by default: the textbook first-order Doppler
//!   result is a resonance absorption growing like `√T`, so `ρ` linear in `√T`
//!   is the physically expected leading term (`ln T` and `T` are offered).
//!   This is the choice of fitting VARIABLE, made before the fit, not a tuned
//!   parameter.
//! - **Weights.** Weighted least squares, each run weighted by `1/σ_ρ`: the
//!   instrument the Monte Carlo noise calls for (a run with twice the σ
//!   carries a quarter of the information). Solved by Householder QR,
//!   `petir::linalg::QrDecomposition` (the GSL port), the same solver
//!   `petir::ChebSeries::fit` uses for one axis.
//! - **Held-out error.** Leave-one-out: each run in turn is left out, the map
//!   refitted on the rest, and the left-out `ρ` predicted. The RMS of those
//!   misses is shown **beside the RMS Monte Carlo σ_ρ** of the runs. A miss
//!   contains the held-out run's own noise, so an adequate map has a held-out
//!   RMS near the MC σ; one well above it means the order (or the variable)
//!   cannot carry the shape. The in-sample χ²/dof is reported too, but it is
//!   not the number to quote: it cannot see over-fitting, the held-out error
//!   can.
//!
//! The order is the user's, set before the fit and never chosen by the
//! held-out error; the planner proposes enough runs for it.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uom::si::f64::ThermodynamicTemperature;
use uom::si::thermodynamic_temperature::kelvin;

use super::recipe::{Recipe, RunRecord};

/// The `format` string of an exported map.
pub const MAP_FORMAT: &str = "dhoby-ghaut-reactivity-map";
/// The map format version this build writes and reads.
pub const MAP_VERSION: u32 = 1;

/// The fidelity statement every exported map carries, word for word.
pub const FIDELITY: &str = "LOW FIDELITY: a neutronics-only surrogate (Chebyshev fit of rho(state) \
     over continuous-energy Monte Carlo eigenvalue runs). No thermal-hydraulics, no feedback model, \
     no kinetics, no spatial shape; interpolation only inside the run range. Research, education \
     and V&V only: not for facility operation, licensing or safety decisions.";

/// How the map is evaluated, written into every export so a reader needs no
/// code from this crate.
pub const EVALUATION: &str = "rho_pcm = sum over [[term]] of coefficient * product over [[axis]] \
     a of T_{degrees[a]}(s_a), with T_n the Chebyshev polynomials of the first kind, \
     s_a = (2 u - u_lo - u_hi) / (u_hi - u_lo), u = basis(x) (linear: x; sqrt: sqrt(x); \
     ln: ln(x)), u_lo = basis(lo), u_hi = basis(hi). Refuse x outside [lo, hi], and any value \
     other than the recorded one on a [[fixed]] axis. k = 1 / (1 - rho_pcm * 1e-5).";

/// A state axis of the map.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AxisName {
    /// Every material's data temperature \[K\] (isothermal in this build).
    TemperatureK,
    /// Control-rod insertion, fraction (0 = withdrawn).
    RodInsertion,
}

impl AxisName {
    /// Both axes, in the order the map stores them.
    pub const ALL: [Self; 2] = [Self::TemperatureK, Self::RodInsertion];

    /// The axis' value in a run.
    #[must_use]
    pub fn of(self, r: &RunRecord) -> f64 {
        match self {
            Self::TemperatureK => r.temperature_k,
            Self::RodInsertion => r.rod_insertion,
        }
    }

    /// A label for tables.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::TemperatureK => "temperature [K] (isothermal)",
            Self::RodInsertion => "rod insertion [fraction]",
        }
    }
}

/// The variable an axis is fitted in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AxisBasis {
    /// `u = x`.
    Linear,
    /// `u = √x` (the default for temperature: first-order Doppler).
    #[default]
    Sqrt,
    /// `u = ln x`.
    Ln,
}

impl AxisBasis {
    /// Every basis, for a picker.
    pub const ALL: [Self; 3] = [Self::Sqrt, Self::Ln, Self::Linear];

    /// `u(x)`.
    #[must_use]
    pub fn u(self, x: f64) -> f64 {
        match self {
            Self::Linear => x,
            Self::Sqrt => x.sqrt(),
            Self::Ln => x.ln(),
        }
    }

    /// `x(u)`, the inverse of [`Self::u`].
    #[must_use]
    pub fn x(self, u: f64) -> f64 {
        match self {
            Self::Linear => u,
            Self::Sqrt => u * u,
            Self::Ln => u.exp(),
        }
    }

    /// A label for pickers.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Linear => "T",
            Self::Sqrt => "√T (first-order Doppler)",
            Self::Ln => "ln T",
        }
    }
}

/// The fit's settings, chosen before the fit (they are Step 6's recipe
/// settings).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct MapSettings {
    /// Highest total Chebyshev degree.
    pub order: usize,
    /// The variable temperature is fitted in.
    pub temperature_basis: AxisBasis,
}

impl Default for MapSettings {
    fn default() -> Self {
        Self {
            order: 2,
            temperature_basis: AxisBasis::Sqrt,
        }
    }
}

/// A fitted axis.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MapAxis {
    /// Which state.
    pub name: AxisName,
    /// The variable it is fitted in.
    pub basis: AxisBasis,
    /// Lowest value run, in the axis' own unit.
    pub lo: f64,
    /// Highest value run.
    pub hi: f64,
}

/// An axis every run held at one value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FixedAxis {
    /// Which state.
    pub name: AxisName,
    /// The one value run.
    pub value: f64,
    /// Why it is fixed.
    pub note: String,
}

/// One term of the series.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Term {
    /// Chebyshev degree per fitted axis, in `[[axis]]` order.
    pub degrees: Vec<usize>,
    /// Coefficient \[pcm\].
    pub coefficient: f64,
}

/// One run's leave-one-out check.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HeldOut {
    /// The run.
    pub label: String,
    /// Its `ρ` \[pcm\].
    pub rho_pcm: f64,
    /// Its Monte Carlo `σ_ρ` \[pcm\].
    pub sigma_rho_pcm: f64,
    /// The map refitted without it, evaluated at its state \[pcm\]; absent
    /// when the remaining runs cannot determine the series.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub predicted_pcm: Option<f64>,
}

impl HeldOut {
    /// `predicted − ρ` \[pcm\].
    #[must_use]
    pub fn error_pcm(&self) -> Option<f64> {
        self.predicted_pcm.map(|p| p - self.rho_pcm)
    }
}

/// What the fit reports.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FitSummary {
    /// How the coefficients were found.
    pub method: String,
    /// Highest total degree.
    pub order: usize,
    /// Runs fitted.
    pub n_runs: usize,
    /// Terms in the series.
    pub n_terms: usize,
    /// How the held-out error was measured.
    pub held_out_method: String,
    /// RMS leave-one-out miss \[pcm\] (absent when any run cannot be left out).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub held_out_rms_pcm: Option<f64>,
    /// Largest leave-one-out miss \[pcm\].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub held_out_max_pcm: Option<f64>,
    /// RMS Monte Carlo `σ_ρ` of the runs \[pcm\], the number to compare the
    /// held-out RMS with.
    pub mc_sigma_rms_pcm: f64,
    /// In-sample weighted χ² per degree of freedom (absent with no freedom).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chi2_per_dof: Option<f64>,
    /// The QR factor's smallest/largest |diagonal| ratio (near 0: the runs
    /// barely determine some term).
    pub conditioning: f64,
}

/// Where the map came from.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Provenance {
    /// Program and version.
    pub generator: String,
    /// When the file was written (RFC 3339).
    pub created: String,
    /// `true` when the runs are SYNTHETIC (made up for a test), not Monte Carlo.
    pub synthetic: bool,
    /// The recipe's title.
    pub recipe_title: String,
    /// SHA-256 of the recipe's model (see [`recipe_hash`]).
    pub recipe_hash: String,
    /// Whether the recipe differs from its preset (no V&V standing if so).
    pub recipe_edited: bool,
    /// The preset's V&V status, as its record states it.
    pub vv_status: String,
    /// The evaluated library.
    pub endf_library: String,
    /// The tape folder the runs read.
    pub endf_dir: String,
    /// Modelling notes.
    #[serde(default)]
    pub notes: Vec<String>,
    /// Every run the map was fitted to.
    #[serde(default, rename = "run")]
    pub runs: Vec<RunRecord>,
}

impl Provenance {
    /// Provenance for a map fitted to `recipe`'s runs.
    #[must_use]
    pub fn from_recipe(recipe: &Recipe, created: &str, synthetic: bool) -> Self {
        Self {
            generator: format!("dhoby-ghaut {}", env!("CARGO_PKG_VERSION")),
            created: created.to_string(),
            synthetic,
            recipe_title: recipe.header.title.clone(),
            recipe_hash: recipe_hash(recipe),
            recipe_edited: recipe.header.edited,
            vv_status: recipe.header.vv_status.clone(),
            endf_library: recipe.nuclear_data.library.clone(),
            endf_dir: recipe.nuclear_data.endf_dir.clone(),
            notes: vec![
                "Temperature is isothermal: every material at the run's one data temperature \
                 (fuel/moderator separation: gh:#590)."
                    .into(),
                "Control rods: all ten move together (one insertion fraction per run, gh:#580); \
                 a single rod alone is not modelled."
                    .into(),
                "The runs do not record which model they were made on; the recipe hash names \
                 the model at export time."
                    .into(),
            ],
            runs: recipe.monte_carlo.runs.clone(),
        }
    }
}

/// The map: an exported plain-TOML file and the evaluator in one.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReactivityMap {
    /// Always [`MAP_FORMAT`].
    pub format: String,
    /// [`MAP_VERSION`].
    pub version: u32,
    /// [`FIDELITY`].
    pub fidelity: String,
    /// [`EVALUATION`].
    pub evaluation: String,
    /// Where it came from.
    pub provenance: Provenance,
    /// What the fit reports.
    pub fit: FitSummary,
    /// The fitted axes.
    #[serde(rename = "axis")]
    pub axes: Vec<MapAxis>,
    /// Axes held at one value.
    #[serde(default, rename = "fixed")]
    pub fixed: Vec<FixedAxis>,
    /// The series.
    #[serde(rename = "term")]
    pub terms: Vec<Term>,
    /// The leave-one-out check, run by run.
    #[serde(default, rename = "held_out")]
    pub held_out: Vec<HeldOut>,
}

/// Why a map could not be fitted, evaluated or read.
#[derive(Debug, Clone, PartialEq)]
pub enum MapError {
    /// Fewer runs than the order needs.
    TooFewRuns {
        /// Runs given.
        have: usize,
        /// Runs needed (terms + 1, so that a run can be held out).
        need: usize,
    },
    /// Every run is at the same state: there is nothing to map.
    NoVariation,
    /// A run with σ ≤ 0 or a non-finite k cannot be weighted.
    BadRun(String),
    /// The least-squares system could not be solved.
    Numerical(String),
    /// A state outside the fitted range (the map does not extrapolate).
    OutOfRange {
        /// Axis.
        axis: AxisName,
        /// Value asked for.
        value: f64,
        /// Range fitted.
        lo: f64,
        /// Range fitted.
        hi: f64,
    },
    /// A value other than the recorded one on a fixed axis.
    FixedAxis {
        /// Axis.
        axis: AxisName,
        /// Value asked for.
        value: f64,
        /// The only value run.
        fixed: f64,
    },
    /// The file is not a readable map.
    Format(String),
}

impl std::fmt::Display for MapError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooFewRuns { have, need } => write!(
                f,
                "{have} runs; this order needs at least {need} (one more than its terms, so a run can be held out)"
            ),
            Self::NoVariation => write!(f, "every run is at the same state: there is nothing to map"),
            Self::BadRun(m) => write!(f, "{m}"),
            Self::Numerical(m) => write!(f, "the fit could not be solved: {m}"),
            Self::OutOfRange { axis, value, lo, hi } => write!(
                f,
                "{} = {value} is outside the runs' range [{lo}, {hi}]; the map does not extrapolate",
                axis.label()
            ),
            Self::FixedAxis { axis, value, fixed } => write!(
                f,
                "{} = {value}: every run was at {fixed}, so the map knows nothing else",
                axis.label()
            ),
            Self::Format(m) => write!(f, "not a readable reactivity map: {m}"),
        }
    }
}

impl std::error::Error for MapError {}

/// `ρ` \[pcm\] of a run.
#[must_use]
pub fn rho_pcm(k: f64) -> f64 {
    (k - 1.0) / k * 1.0e5
}

/// `σ_ρ` \[pcm\] of a run, first-order from its `σ_k`.
#[must_use]
pub fn sigma_rho_pcm(k: f64, sigma_k: f64) -> f64 {
    sigma_k / (k * k) * 1.0e5
}

/// SHA-256 of the recipe's **model**: the recipe rendered with every
/// timestamp fixed, and with the runs, the review stamp and the Step 6
/// settings removed (how the model was run, checked and mapped, not what it
/// is). Two recipes with the same hash describe the same model.
#[must_use]
pub fn recipe_hash(recipe: &Recipe) -> String {
    let mut r = recipe.clone();
    r.monte_carlo.runs.clear();
    r.review = Default::default();
    r.branch = Default::default();
    let text = r.to_markdown("1970-01-01T00:00:00Z").unwrap_or_default();
    let digest = Sha256::digest(text.as_bytes());
    let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
    format!("sha256:{hex}")
}

/// Total-degree multi-indices over `n_axes` axes with `Σ d ≤ order`, graded
/// (degree 0 first).
fn multi_indices(n_axes: usize, order: usize) -> Vec<Vec<usize>> {
    let mut out = Vec::new();
    for total in 0..=order {
        let mut cur = vec![0; n_axes];
        fill(&mut out, &mut cur, 0, total);
    }
    out
}

fn fill(out: &mut Vec<Vec<usize>>, cur: &mut Vec<usize>, axis: usize, left: usize) {
    if axis + 1 == cur.len() {
        cur[axis] = left;
        out.push(cur.clone());
        return;
    }
    for d in (0..=left).rev() {
        cur[axis] = d;
        fill(out, cur, axis + 1, left - d);
    }
}

/// One basis row: `Π_a T_{d_a}(s_a)` for every term.
fn basis_row(s: &[f64], terms: &[Vec<usize>], max_degree: usize) -> Vec<f64> {
    let t: Vec<Vec<f64>> = s
        .iter()
        .map(|&x| {
            let mut v = vec![0.0; max_degree + 1];
            petir::cheb_slice::basis_into(x, &mut v);
            v
        })
        .collect();
    terms
        .iter()
        .map(|d| d.iter().enumerate().map(|(a, &k)| t[a][k]).product())
        .collect()
}

/// Weighted least squares over `rows` (scaled coordinates, ρ, σ_ρ).
fn solve(
    rows: &[(Vec<f64>, f64, f64)],
    terms: &[Vec<usize>],
    order: usize,
) -> Result<(Vec<f64>, f64, f64), MapError> {
    let (m, n) = (rows.len(), terms.len());
    let mut a = petir::linalg::Matrix::zeros(m, n).map_err(|e| MapError::Numerical(e.to_string()))?;
    let mut b = Vec::with_capacity(m);
    for (i, (s, rho, sig)) in rows.iter().enumerate() {
        for (j, v) in basis_row(s, terms, order).into_iter().enumerate() {
            a.set(i, j, v / sig);
        }
        b.push(rho / sig);
    }
    let qr = petir::linalg::QrDecomposition::new(a).map_err(|e| MapError::Numerical(e.to_string()))?;
    let cond = qr.diagonal_ratio();
    let ls = qr
        .least_squares(&b)
        .map_err(|e| MapError::Numerical(e.to_string()))?;
    let chi2 = ls.residual.iter().map(|r| r * r).sum();
    Ok((ls.solution, chi2, cond))
}

/// Two values the same state? (relative 1e-9, absolute 1e-12).
fn same(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-9 * a.abs().max(b.abs()) + 1e-12
}

impl ReactivityMap {
    /// Fit the map to `runs` with `settings`. The provenance is left empty
    /// (synthetic = false, no recipe); fill it with
    /// [`Provenance::from_recipe`].
    ///
    /// # Errors
    ///
    /// [`MapError::BadRun`] for a run with σ ≤ 0 or non-finite k,
    /// [`MapError::NoVariation`] when every run is at one state,
    /// [`MapError::TooFewRuns`] when the order needs more runs, and
    /// [`MapError::Numerical`] when the runs do not determine the series
    /// (e.g. too few DISTINCT states for the order).
    pub fn fit(runs: &[RunRecord], settings: MapSettings) -> Result<Self, MapError> {
        for r in runs {
            if !(r.sigma > 0.0) || !r.k.is_finite() || !(r.k > 0.0) {
                return Err(MapError::BadRun(format!(
                    "{}: k = {} ± {} cannot be weighted (σ must be > 0)",
                    r.label, r.k, r.sigma
                )));
            }
        }
        let mut axes = Vec::new();
        let mut fixed = Vec::new();
        for name in AxisName::ALL {
            let vals: Vec<f64> = runs.iter().map(|r| name.of(r)).collect();
            let lo = vals.iter().copied().fold(f64::INFINITY, f64::min);
            let hi = vals.iter().copied().fold(f64::NEG_INFINITY, f64::max);
            if runs.is_empty() {
                continue;
            }
            if same(lo, hi) {
                fixed.push(FixedAxis {
                    name,
                    value: lo,
                    note: match name {
                        AxisName::TemperatureK => "every run at this temperature".into(),
                        AxisName::RodInsertion => {
                            "every run at this insertion".into()
                        }
                    },
                });
            } else {
                let basis = match name {
                    AxisName::TemperatureK => settings.temperature_basis,
                    AxisName::RodInsertion => AxisBasis::Linear,
                };
                if basis != AxisBasis::Linear && lo <= 0.0 {
                    return Err(MapError::BadRun(format!(
                        "{} reaches {lo}, where the {basis:?} basis is undefined",
                        name.label()
                    )));
                }
                axes.push(MapAxis { name, basis, lo, hi });
            }
        }
        if axes.is_empty() {
            return Err(MapError::NoVariation);
        }
        let terms = multi_indices(axes.len(), settings.order);
        let need = terms.len() + 1;
        if runs.len() < need {
            return Err(MapError::TooFewRuns {
                have: runs.len(),
                need,
            });
        }
        let scaled = |r: &RunRecord| -> Vec<f64> {
            axes.iter()
                .map(|a| {
                    petir::cheb_slice::scale(a.basis.u(a.name.of(r)), a.basis.u(a.lo), a.basis.u(a.hi))
                })
                .collect()
        };
        let rows: Vec<(Vec<f64>, f64, f64)> = runs
            .iter()
            .map(|r| (scaled(r), rho_pcm(r.k), sigma_rho_pcm(r.k, r.sigma)))
            .collect();
        let (coeffs, chi2, conditioning) = solve(&rows, &terms, settings.order)?;
        let dof = runs.len() - terms.len();
        // Leave-one-out.
        let mut held_out = Vec::with_capacity(runs.len());
        for (i, r) in runs.iter().enumerate() {
            let rest: Vec<(Vec<f64>, f64, f64)> = rows
                .iter()
                .enumerate()
                .filter(|(j, _)| *j != i)
                .map(|(_, x)| x.clone())
                .collect();
            let predicted_pcm = solve(&rest, &terms, settings.order).ok().map(|(c, _, _)| {
                basis_row(&rows[i].0, &terms, settings.order)
                    .iter()
                    .zip(&c)
                    .map(|(t, c)| t * c)
                    .sum()
            });
            held_out.push(HeldOut {
                label: r.label.clone(),
                rho_pcm: rows[i].1,
                sigma_rho_pcm: rows[i].2,
                predicted_pcm,
            });
        }
        let errs: Option<Vec<f64>> = held_out.iter().map(HeldOut::error_pcm).collect();
        let (held_out_rms_pcm, held_out_max_pcm) = match errs {
            Some(e) if !e.is_empty() => (
                Some((e.iter().map(|x| x * x).sum::<f64>() / e.len() as f64).sqrt()),
                Some(e.iter().fold(0.0_f64, |m, x| m.max(x.abs()))),
            ),
            _ => (None, None),
        };
        let mc_sigma_rms_pcm =
            (rows.iter().map(|x| x.2 * x.2).sum::<f64>() / rows.len() as f64).sqrt();
        Ok(Self {
            format: MAP_FORMAT.into(),
            version: MAP_VERSION,
            fidelity: FIDELITY.into(),
            evaluation: EVALUATION.into(),
            provenance: Provenance::default(),
            fit: FitSummary {
                method: "weighted (1/sigma_rho) least squares, total-degree Chebyshev basis, \
                         Householder QR (petir::linalg, GSL port)"
                    .into(),
                order: settings.order,
                n_runs: runs.len(),
                n_terms: terms.len(),
                held_out_method: "leave-one-out: refit without each run, predict it".into(),
                held_out_rms_pcm,
                held_out_max_pcm,
                mc_sigma_rms_pcm,
                chi2_per_dof: (dof > 0).then(|| chi2 / dof as f64),
                conditioning,
            },
            axes,
            fixed,
            terms: terms
                .into_iter()
                .zip(coeffs)
                .map(|(degrees, coefficient)| Term {
                    degrees,
                    coefficient,
                })
                .collect(),
            held_out,
        })
    }

    /// `ρ` \[pcm\] at a state given as axis values (any order, every fitted
    /// axis present; a fixed axis may be given at its value or left out).
    ///
    /// # Errors
    ///
    /// [`MapError::OutOfRange`] outside the fitted range,
    /// [`MapError::FixedAxis`] off a fixed axis' value, and
    /// [`MapError::Format`] when a fitted axis is missing.
    pub fn rho_pcm_at(&self, state: &[(AxisName, f64)]) -> Result<f64, MapError> {
        for f in &self.fixed {
            if let Some((_, v)) = state.iter().find(|(n, _)| *n == f.name) {
                if !same(*v, f.value) {
                    return Err(MapError::FixedAxis {
                        axis: f.name,
                        value: *v,
                        fixed: f.value,
                    });
                }
            }
        }
        let mut s = Vec::with_capacity(self.axes.len());
        for a in &self.axes {
            let v = state
                .iter()
                .find(|(n, _)| *n == a.name)
                .map(|x| x.1)
                .ok_or_else(|| MapError::Format(format!("no value given for {}", a.name.label())))?;
            let tol = 1e-9 * (a.hi - a.lo).abs();
            if v < a.lo - tol || v > a.hi + tol {
                return Err(MapError::OutOfRange {
                    axis: a.name,
                    value: v,
                    lo: a.lo,
                    hi: a.hi,
                });
            }
            s.push(petir::cheb_slice::scale(a.basis.u(v), a.basis.u(a.lo), a.basis.u(a.hi)).clamp(-1.0, 1.0));
        }
        let degrees: Vec<Vec<usize>> = self.terms.iter().map(|t| t.degrees.clone()).collect();
        let max_degree = degrees.iter().flatten().copied().max().unwrap_or(0);
        Ok(basis_row(&s, &degrees, max_degree)
            .iter()
            .zip(&self.terms)
            .map(|(b, t)| b * t.coefficient)
            .sum())
    }

    /// [`Self::rho_pcm_at`] with the temperature as a `uom` quantity and the
    /// rod insertion as a fraction.
    ///
    /// # Errors
    ///
    /// As [`Self::rho_pcm_at`].
    pub fn rho_pcm(&self, temperature: ThermodynamicTemperature, rod_insertion: f64) -> Result<f64, MapError> {
        self.rho_pcm_at(&[
            (AxisName::TemperatureK, temperature.get::<kelvin>()),
            (AxisName::RodInsertion, rod_insertion),
        ])
    }

    /// The map as plain TOML.
    ///
    /// # Errors
    ///
    /// [`MapError::Format`] if serialisation fails.
    pub fn to_toml(&self) -> Result<String, MapError> {
        let body = toml::to_string_pretty(self).map_err(|e| MapError::Format(e.to_string()))?;
        Ok(format!(
            "# Dhoby Ghaut reactivity map (gh:#571). {FIDELITY}\n# Read `evaluation` before using the coefficients.\n\n{body}"
        ))
    }

    /// Read a map written by [`Self::to_toml`].
    ///
    /// # Errors
    ///
    /// [`MapError::Format`] for a malformed file, another format, or a newer
    /// version.
    pub fn from_toml(text: &str) -> Result<Self, MapError> {
        let m: Self = toml::from_str(text).map_err(|e| MapError::Format(e.to_string()))?;
        if m.format != MAP_FORMAT {
            return Err(MapError::Format(format!("format is {:?}", m.format)));
        }
        if m.version > MAP_VERSION {
            return Err(MapError::Format(format!(
                "version {} is newer than this build reads ({MAP_VERSION})",
                m.version
            )));
        }
        Ok(m)
    }

    /// The map on a grid, as CSV (`temperature_k,rod_insertion,rho_pcm,k`):
    /// `n` points per fitted axis, evenly spaced in the axis' own unit.
    #[must_use]
    pub fn grid_csv(&self, n: usize) -> String {
        let n = n.max(2);
        let mut out = String::from("temperature_k,rod_insertion,rho_pcm,k\n");
        let value = |name: AxisName, i: usize| -> Option<f64> {
            if let Some(a) = self.axes.iter().find(|a| a.name == name) {
                return Some(a.lo + (a.hi - a.lo) * i as f64 / (n - 1) as f64);
            }
            self.fixed.iter().find(|f| f.name == name).map(|f| f.value)
        };
        let n_t = if self.axes.iter().any(|a| a.name == AxisName::TemperatureK) { n } else { 1 };
        let n_r = if self.axes.iter().any(|a| a.name == AxisName::RodInsertion) { n } else { 1 };
        for i in 0..n_t {
            for j in 0..n_r {
                let (Some(t), Some(r)) = (value(AxisName::TemperatureK, i), value(AxisName::RodInsertion, j)) else {
                    continue;
                };
                if let Ok(rho) = self.rho_pcm_at(&[(AxisName::TemperatureK, t), (AxisName::RodInsertion, r)]) {
                    out.push_str(&format!("{t:.4},{r:.4},{rho:.3},{:.6}\n", 1.0 / (1.0 - rho * 1.0e-5)));
                }
            }
        }
        out
    }
}

/// The state-point planner: the temperatures to run for a map over
/// `[lo, hi]` with `n` points, at the Chebyshev–Lobatto nodes in the fitting
/// variable (`cos(π j/(n−1))`, endpoints included, so the map's range is the
/// planned range and the nodes cluster where a polynomial fit is most
/// sensitive). A node within `tol_k` of an existing run's temperature is
/// dropped (that run already covers it). Ascending.
#[must_use]
pub fn plan_temperatures(lo: f64, hi: f64, n: usize, basis: AxisBasis, existing: &[f64], tol_k: f64) -> Vec<f64> {
    let (lo, hi) = (lo.min(hi), lo.max(hi));
    let nodes: Vec<f64> = if n <= 1 || same(lo, hi) {
        vec![lo]
    } else {
        let (ul, uh) = (basis.u(lo), basis.u(hi));
        (0..n)
            .map(|j| {
                let c = -(std::f64::consts::PI * j as f64 / (n - 1) as f64).cos();
                basis.x(0.5 * (ul + uh) + 0.5 * (uh - ul) * c)
            })
            .collect()
    };
    let mut out: Vec<f64> = nodes
        .into_iter()
        .map(|t| (t * 100.0).round() / 100.0)
        .filter(|t| !existing.iter().any(|e| (e - t).abs() <= tol_k))
        .collect();
    out.sort_by(f64::total_cmp);
    out
}

/// Runs a map of `order` over `n_axes` varying axes needs: one more than its
/// terms, so a run can be held out.
#[must_use]
pub fn runs_needed(order: usize, n_axes: usize) -> usize {
    multi_indices(n_axes.max(1), order).len() + 1
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A SYNTHETIC run (made up for a test, not Monte Carlo): `k` from a
    /// chosen `ρ` in pcm.
    fn synthetic(label: &str, t: f64, rod: f64, rho: f64, sigma_k: f64) -> RunRecord {
        RunRecord {
            label: label.into(),
            rod_insertion: rod,
            temperature_k: t,
            particles: 0,
            inactive: 0,
            active: 0,
            seed: 0,
            k: 1.0 / (1.0 - rho * 1.0e-5),
            sigma: sigma_k,
            transport_s: 0.0,
        }
    }

    /// SYNTHETIC: ρ = 3000 − 120 √T pcm exactly (linear in √T). An order-1
    /// fit in √T recovers it to rounding, at a point between the runs too.
    #[test]
    fn an_exact_sqrt_t_law_is_recovered() {
        let law = |t: f64| 3000.0 - 120.0 * t.sqrt();
        let runs: Vec<RunRecord> = [300.0, 450.0, 600.0, 900.0, 1200.0]
            .iter()
            .enumerate()
            .map(|(i, &t)| synthetic(&format!("S{i}"), t, 0.0, law(t), 1e-4))
            .collect();
        let m = ReactivityMap::fit(&runs, MapSettings { order: 1, temperature_basis: AxisBasis::Sqrt }).unwrap();
        assert_eq!(m.axes.len(), 1);
        assert_eq!(m.fixed.len(), 1, "rods are fixed");
        let got = m.rho_pcm(ThermodynamicTemperature::new::<kelvin>(750.0), 0.0).unwrap();
        assert!((got - law(750.0)).abs() < 1e-6, "{got} vs {}", law(750.0));
        assert!(m.fit.held_out_rms_pcm.unwrap() < 1e-6);
    }

    /// SYNTHETIC with deterministic ±σ noise: the held-out RMS is of the
    /// order of the MC σ for an adequate order, and far above it for an order
    /// that cannot carry the shape (order 0 on a sloped law).
    #[test]
    fn the_held_out_error_separates_an_adequate_order_from_an_inadequate_one() {
        let law = |t: f64| 3000.0 - 120.0 * t.sqrt();
        let temps = [300.0, 400.0, 500.0, 600.0, 700.0, 800.0, 900.0, 1000.0, 1100.0, 1200.0];
        let sigma_k = 5e-4; // ~50 pcm
        let runs: Vec<RunRecord> = temps
            .iter()
            .enumerate()
            .map(|(i, &t)| {
                let noise = if i % 2 == 0 { 50.0 } else { -50.0 };
                synthetic(&format!("S{i}"), t, 0.0, law(t) + noise, sigma_k)
            })
            .collect();
        let good = ReactivityMap::fit(&runs, MapSettings { order: 1, temperature_basis: AxisBasis::Sqrt }).unwrap();
        let bad = ReactivityMap::fit(&runs, MapSettings { order: 0, temperature_basis: AxisBasis::Sqrt }).unwrap();
        let (g, b) = (good.fit.held_out_rms_pcm.unwrap(), bad.fit.held_out_rms_pcm.unwrap());
        let s = good.fit.mc_sigma_rms_pcm;
        assert!(g < 2.0 * s, "adequate order: held-out {g} vs σ {s}");
        assert!(b > 10.0 * s, "order 0 on a sloped law: held-out {b} vs σ {s}");
    }

    /// SYNTHETIC two-axis map (rod insertion is not modelled in HTR-10, so
    /// this exercises the code path only): a bilinear law is recovered.
    #[test]
    fn a_two_axis_map_recovers_a_bilinear_law() {
        let law = |t: f64, r: f64| 2000.0 - 1.5 * t - 4000.0 * r;
        let mut runs = Vec::new();
        for (i, t) in [300.0, 600.0, 900.0].iter().enumerate() {
            for (j, r) in [0.0, 0.5, 1.0].iter().enumerate() {
                runs.push(synthetic(&format!("S{i}{j}"), *t, *r, law(*t, *r), 1e-4));
            }
        }
        let m = ReactivityMap::fit(&runs, MapSettings { order: 1, temperature_basis: AxisBasis::Linear }).unwrap();
        assert_eq!(m.axes.len(), 2);
        let v = m.rho_pcm_at(&[(AxisName::TemperatureK, 450.0), (AxisName::RodInsertion, 0.25)]).unwrap();
        assert!((v - law(450.0, 0.25)).abs() < 1e-6);
    }

    /// The map does not extrapolate, and refuses a fixed axis' other values.
    #[test]
    fn the_map_refuses_to_extrapolate() {
        let runs: Vec<RunRecord> = [300.0, 600.0, 900.0]
            .iter()
            .map(|&t| synthetic("S", t, 0.0, -t, 1e-4))
            .collect();
        let m = ReactivityMap::fit(&runs, MapSettings { order: 1, temperature_basis: AxisBasis::Sqrt }).unwrap();
        assert!(matches!(
            m.rho_pcm_at(&[(AxisName::TemperatureK, 1000.0)]),
            Err(MapError::OutOfRange { .. })
        ));
        assert!(matches!(
            m.rho_pcm_at(&[(AxisName::TemperatureK, 500.0), (AxisName::RodInsertion, 0.5)]),
            Err(MapError::FixedAxis { .. })
        ));
    }

    #[test]
    fn too_few_runs_or_one_state_is_refused() {
        let one = vec![synthetic("a", 300.0, 0.0, 0.0, 1e-4), synthetic("b", 300.0, 0.0, 1.0, 1e-4)];
        assert_eq!(ReactivityMap::fit(&one, MapSettings::default()), Err(MapError::NoVariation));
        let two = vec![synthetic("a", 300.0, 0.0, 0.0, 1e-4), synthetic("b", 600.0, 0.0, 1.0, 1e-4)];
        assert!(matches!(
            ReactivityMap::fit(&two, MapSettings::default()),
            Err(MapError::TooFewRuns { have: 2, need: 4 })
        ));
        let zero = vec![synthetic("a", 300.0, 0.0, 0.0, 0.0)];
        assert!(matches!(ReactivityMap::fit(&zero, MapSettings::default()), Err(MapError::BadRun(_))));
    }

    /// Write → read gives back the same map, provenance included.
    #[test]
    fn a_map_round_trips_through_plain_toml() {
        let runs: Vec<RunRecord> = [300.0, 500.0, 700.0, 900.0]
            .iter()
            .map(|&t| synthetic("S", t, 0.0, 1000.0 - 0.5 * t, 2e-4))
            .collect();
        let mut m = ReactivityMap::fit(&runs, MapSettings { order: 1, temperature_basis: AxisBasis::Ln }).unwrap();
        m.provenance.synthetic = true;
        m.provenance.runs = runs;
        m.provenance.recipe_hash = "sha256:00".into();
        let text = m.to_toml().unwrap();
        assert!(text.contains("[provenance]") && text.contains("[[provenance.run]]"), "{text}");
        assert!(text.contains("[[held_out]]") && text.contains("[[term]]"));
        let back = ReactivityMap::from_toml(&text).unwrap();
        assert_eq!(back, m);
    }

    #[test]
    fn the_planner_puts_lobatto_nodes_in_the_fitting_variable_and_skips_runs_made() {
        let p = plan_temperatures(300.0, 1200.0, 5, AxisBasis::Sqrt, &[], 1.0);
        assert_eq!(p.len(), 5);
        assert!((p[0] - 300.0).abs() < 1e-9 && (p[4] - 1200.0).abs() < 1e-9);
        // The middle node is the √T midpoint, (√300 + √1200)²/4 = 675.
        assert!((p[2] - 675.0).abs() < 1e-6, "{p:?}");
        let q = plan_temperatures(300.0, 1200.0, 5, AxisBasis::Sqrt, &[300.15, 675.0], 1.0);
        assert_eq!(q.len(), 3, "{q:?}");
        assert_eq!(runs_needed(2, 1), 4);
        assert_eq!(runs_needed(1, 2), 4);
    }
}
