//! Reactor-physics capture: run a k-eigenvalue calculation and pull out, in one
//! pass, the quantities a reactor physicist reads **alongside** k_eff —
//!
//! 1. the **six-factor formula** (η, f, p, ε, P_FNL, P_TNL) with 1σ
//!    uncertainties, on a three-energy-group (fast / resonance / thermal) split,
//!    and
//! 2. the **lethargy-normalised neutron flux spectrum** over the whole material
//!    domain of the geometry.
//!
//! Both come from a **single** combined track-length [`Tally`] scored by one
//! [`run_keff_csg_reactor_physics`] call, plus the per-energy leakage spectrum
//! that same driver accumulates (see [`crate::physics::transport_csg`]). There
//! is no second transport run.
//!
//! # Three groups, not two
//!
//! The classical six-factor formula is written for two groups (thermal vs.
//! "fast"), which collapses the *resonance escape probability* p into a bare
//! ratio with no resonance region to escape from. This module keeps a genuine
//! **resonance group** between two user-set boundaries, so p is
//! `A_thermal / (A_thermal + A_resonance)` (leakage-corrected) — the textbook
//! meaning. Fast absorption is folded into the fast-fission factor ε.
//!
//! # What the factors mean here (A_g, P_g, L_g = group absorption / ν-fission
//! production / leakage rate; subscript `fuel` = the uranium-bearing materials)
//!
//! ```text
//! η      = P_thermal / A_thermal,fuel
//! f      = A_thermal,fuel / A_thermal
//! P_TNL  = A_thermal / (A_thermal + L_thermal)
//! p      = (A_thermal + L_thermal) / (A_thermal + L_thermal + A_res + L_res)
//! P_FNL  = (A_thermal + L_thermal + A_res + L_res)
//!          / (A_thermal + L_thermal + A_res + L_res + L_fast)
//! ε      = (P_total / P_thermal) · (A_total + L_total − A_fast) / (A_total + L_total)
//! ```
//!
//! These telescope **exactly**: `η·f·p·ε·P_FNL·P_TNL = P_total / (A_total +
//! L_total)`, which is k_eff (production over losses). For a reflective /
//! infinite geometry every `L_g = 0`, so `P_FNL = P_TNL = 1` and the product is
//! `k_inf = P_total / A_total`.
//!
//! # Caveats (see [`ReactorPhysicsReport`])
//!
//! - **Absorption is `Σ_a` = capture + fission** ([`ScoreType::Absorption`] via
//!   [`crate::material::material::MacroXs::absorption`]) — the OpenMC MT=27
//!   quantity, the sum of the non-redundant disappearance reactions plus
//!   fission. It excludes inelastic scatter and (n,2n)/(n,3n). The remaining
//!   `consistency_gap` against `k_eff` is the small `(n,2n)` multiplication that
//!   ν-fission does not count.
//! - **Void-gap flux is not scored.** The combined tally's [`MaterialFilter`]
//!   rejects segments in a void (`material_idx == usize::MAX`), so the spectrum
//!   and its lethargy normalisation cover the material domain only.
//! - **Ratio uncertainties** use the delta method assuming the tally bins are
//!   independent. For the six factors a numerator group is a subset of its
//!   denominator, so the neglected covariance is negative and the quoted `std`
//!   is **conservative** (larger than the true correlated 1σ).

use crate::geometry::geometry::Geometry;
use crate::material::material::Material;
use crate::material::nuclide::Nuclide;
use crate::physics::keff::{KeffResult, KeffSettings};
use crate::physics::transport_csg::{run_keff_csg_reactor_physics, SourceBox};
use crate::tally::filter::{EnergyFilter, MaterialFilter};
use crate::tally::tally::{ScoreType, Tally, TallyBin};

/// Accepted band for [`ReactorPhysicsReport::consistency_gap`]
/// `(k_eff − k_from_factors) / k_eff`.
///
/// Matches the reference pipeline. The four factors telescope to
/// `P_total / (A_total + L_total)` exactly; the residual gap against the
/// power-iteration `k_eff` is the physics that ν-fission and MT=27 absorption
/// together do not capture — chiefly `(n,2n)`/`(n,3n)`, which add a neutron
/// without a fission and are not a disappearance. That runs the product a
/// little **below** `k_eff` (positive gap), by up to a percent or two in a
/// graphite/heavy system. A gap outside this band, or a large negative one,
/// means a mis-specified tally.
pub const CONSISTENCY_BAND: (f64, f64) = (-0.01, 0.05);

// ─────────────────────────────────────────────────────────────────────────────
// Estimate + delta-method propagation
// ─────────────────────────────────────────────────────────────────────────────

/// A scalar Monte-Carlo estimate: a mean and its 1σ standard error, in the same
/// (unstated) units as the quantity being estimated.
///
/// Sums, differences, ratios and products of `Estimate`s propagate by the delta
/// method **assuming the operands are statistically independent**. For the six
/// factors that assumption is false — a numerator group is a subset of its
/// denominator — which makes the quoted `std` conservative (see the module docs).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Estimate {
    /// Mean over the active generations.
    pub mean: f64,
    /// 1σ standard error of the mean, in absolute units.
    pub std: f64,
}

impl Estimate {
    /// A known-exact value (zero uncertainty).
    pub const fn exact(v: f64) -> Self {
        Self { mean: v, std: 0.0 }
    }

    /// Relative standard error `std / |mean|` (0 when `mean == 0`).
    fn rel(self) -> f64 {
        if self.mean == 0.0 {
            0.0
        } else {
            self.std / self.mean.abs()
        }
    }

    /// Sum of independent estimates: means add, variances add.
    fn sum(parts: &[Estimate]) -> Estimate {
        let mean = parts.iter().map(|e| e.mean).sum();
        let var: f64 = parts.iter().map(|e| e.std * e.std).sum();
        Estimate {
            mean,
            std: var.sqrt(),
        }
    }

    /// Difference `a − b` of independent estimates: variances add.
    fn diff(a: Estimate, b: Estimate) -> Estimate {
        Estimate {
            mean: a.mean - b.mean,
            std: (a.std * a.std + b.std * b.std).sqrt(),
        }
    }

    /// Ratio `num / den` of independent estimates: relative variances add.
    /// Returns `{0, 0}` if `den.mean == 0` (a degenerate group — e.g. the
    /// thermal group of an unmoderated system).
    fn ratio(num: Estimate, den: Estimate) -> Estimate {
        if den.mean == 0.0 {
            return Estimate { mean: 0.0, std: 0.0 };
        }
        let mean = num.mean / den.mean;
        let rel = (num.rel().powi(2) + den.rel().powi(2)).sqrt();
        Estimate {
            mean,
            std: mean.abs() * rel,
        }
    }

    /// Product of independent estimates: relative variances add.
    fn product(parts: &[Estimate]) -> Estimate {
        let mean: f64 = parts.iter().map(|e| e.mean).product();
        let rel2: f64 = parts.iter().map(|e| e.rel().powi(2)).sum();
        Estimate {
            mean,
            std: mean.abs() * rel2.sqrt(),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Config
// ─────────────────────────────────────────────────────────────────────────────

/// Inputs to [`run_keff_reactor_physics`].
#[derive(Debug, Clone)]
pub struct ReactorPhysicsConfig {
    /// Power-iteration controls (histories/generation, inactive/active counts,
    /// seed, temperature \[K\], compute backend).
    pub keff: KeffSettings,
    /// Box the initial fission source is rejection-sampled into \[cm\]. Almost
    /// always overridden — the [`Default`] box is a unit cube at the origin.
    pub source_box: SourceBox,
    /// Thermal-group upper edge / resonance-group lower edge \[eV\]. Default
    /// `0.625` (the cadmium cutoff).
    pub thermal_cutoff_ev: f64,
    /// Resonance-group upper edge / fast-group lower edge \[eV\]. Default
    /// `1.0e5` (0.1 MeV) — the textbook fast/epithermal divide, the `FAST_CUT`
    /// already used by the crate's `flux_spectrum` case, and roughly where
    /// U-238 fast fission turns on. The true resolved-resonance ceiling is
    /// nuclide-dependent (tens of keV for the actinides); set this deliberately
    /// for an epithermal-heavy spectrum.
    pub resonance_upper_ev: f64,
    /// Number of log-spaced fine energy bins spanning
    /// `[energy_min_ev, energy_max_ev]` **before** the two group boundaries are
    /// forced onto the grid. Default `500`.
    pub n_fine_bins: usize,
    /// Fine-grid low edge \[eV\]. Default `1.0e-3`.
    pub energy_min_ev: f64,
    /// Fine-grid high edge \[eV\]. Default `2.0e7`.
    pub energy_max_ev: f64,
}

impl Default for ReactorPhysicsConfig {
    fn default() -> Self {
        Self {
            keff: KeffSettings::default(),
            source_box: SourceBox {
                lower: crate::geometry::position::Position::new(-0.5, -0.5, -0.5),
                upper: crate::geometry::position::Position::new(0.5, 0.5, 0.5),
            },
            thermal_cutoff_ev: 0.625,
            resonance_upper_ev: 1.0e5,
            n_fine_bins: 500,
            energy_min_ev: 1.0e-3,
            energy_max_ev: 2.0e7,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Results
// ─────────────────────────────────────────────────────────────────────────────

/// Energy-group index for the three-group decomposition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Group {
    /// `E < thermal_cutoff_ev`.
    Thermal = 0,
    /// `thermal_cutoff_ev <= E < resonance_upper_ev`.
    Resonance = 1,
    /// `E >= resonance_upper_ev`.
    Fast = 2,
}

/// Three-group six-factor decomposition with the per-group leakages that make
/// the product telescope to k_eff. Group arrays are indexed
/// `[thermal, resonance, fast]`.
#[derive(Debug, Clone)]
pub struct SixFactors {
    /// Reproduction factor η = P_thermal / A_thermal,fuel \[dimensionless\].
    pub eta: Estimate,
    /// Thermal utilisation f = A_thermal,fuel / A_thermal.
    pub f: Estimate,
    /// Resonance escape probability p (3-group, leakage-corrected).
    pub p: Estimate,
    /// Fast fission factor ε — carries the fast-absorption term.
    pub epsilon: Estimate,
    /// Fast (+ epithermal) non-leakage probability P_FNL.
    pub p_fnl: Estimate,
    /// Thermal non-leakage probability P_TNL.
    pub p_tnl: Estimate,
    /// η·f·p·ε·P_FNL·P_TNL — telescopes to `P_total / (A_total + L_total)`.
    pub k_from_factors: Estimate,
    /// Per-group absorption rate \[neutrons per source neutron per generation\].
    pub absorption_by_group: [Estimate; 3],
    /// Per-group ν-fission production rate \[same units\].
    pub production_by_group: [Estimate; 3],
    /// Per-group leakage rate \[same units\].
    pub leakage_by_group: [Estimate; 3],
    /// Thermal absorption restricted to the fuel materials \[same units\].
    pub thermal_absorption_fuel: Estimate,
    /// Group boundaries actually used \[eV\]: `(thermal_cutoff, resonance_upper)`.
    pub group_bounds_ev: (f64, f64),
}

/// Lethargy-normalised neutron flux spectrum over the material domain.
#[derive(Debug, Clone)]
pub struct LethargySpectrum {
    /// `n + 1` ascending fine energy bin edges \[eV\]; the two group boundaries
    /// are among them.
    pub energy_edges_ev: Vec<f64>,
    /// Per-bin flux per unit lethargy `ψ_i = φ_i / ln(E_hi/E_lo)`, normalised so
    /// `Σ_i ψ_i · Δu_i = 1`. Length `energy_edges_ev.len() - 1`.
    pub flux_per_lethargy: Vec<Estimate>,
    /// Raw per-bin track-length flux means (arbitrary units, pre-lethargy,
    /// pre-normalisation) — so a caller can renormalise differently.
    pub flux_raw: Vec<f64>,
    /// `Σ_i φ_i`, the normalisation constant (arbitrary track-length units).
    pub flux_total: f64,
    /// Indices into `energy_edges_ev` of `(thermal/resonance edge,
    /// resonance/fast edge)`.
    pub group_edge_indices: (usize, usize),
}

/// Everything [`run_keff_reactor_physics`] captures alongside k_eff.
#[derive(Debug, Clone)]
pub struct ReactorPhysicsReport {
    /// The eigenvalue result (same type the plain drivers return).
    pub keff: KeffResult,
    /// Three-group, leakage-resolved six-factor decomposition.
    pub six_factors: SixFactors,
    /// Lethargy-normalised flux spectrum over the material domain.
    pub spectrum: LethargySpectrum,
    /// Total leakage rate \[neutrons per source neutron per generation\].
    pub leakage_total: Estimate,
    /// Per-fine-bin leakage spectrum, same grid as `spectrum.energy_edges_ev`.
    pub leakage_spectrum: Vec<Estimate>,
    /// `(k_eff − k_from_factors) / k_eff`. Positive ⇒ the factors under-predict
    /// (expected: ν-fission does not count the extra neutron from (n,2n)).
    pub consistency_gap: f64,
    /// Whether `consistency_gap` lies inside [`CONSISTENCY_BAND`].
    pub consistent: bool,
}

/// Why [`run_keff_reactor_physics`] could not produce a report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReactorPhysicsError {
    /// No material contains a nuclide whose name starts with `"U23"`, so the
    /// fuel cannot be identified and the thermal-utilisation factor f is
    /// undefined. (Mirrors the reference `_fuel_materials`; MOX/Pu fuel and
    /// U-233 are not detected.)
    NoFuelMaterial,
    /// The energy grid is ill-posed:
    /// `energy_min_ev < thermal_cutoff_ev < resonance_upper_ev < energy_max_ev`
    /// is violated.
    BadEnergyGrid,
}

impl std::fmt::Display for ReactorPhysicsError {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoFuelMaterial => write!(
                fmt,
                "no uranium-bearing material found (need a nuclide named 'U23*'); \
                 cannot identify the fuel for the thermal-utilisation factor f"
            ),
            Self::BadEnergyGrid => write!(
                fmt,
                "energy grid must satisfy energy_min < thermal_cutoff < resonance_upper < energy_max"
            ),
        }
    }
}

impl std::error::Error for ReactorPhysicsError {}

// ─────────────────────────────────────────────────────────────────────────────
// Energy grid
// ─────────────────────────────────────────────────────────────────────────────

/// `n`-bin log-spaced grid from `e_lo` to `e_hi` \[eV\] (`n + 1` ascending edges).
fn log_energy_grid(e_lo: f64, e_hi: f64, n: usize) -> Vec<f64> {
    let (l0, l1) = (e_lo.ln(), e_hi.ln());
    (0..=n)
        .map(|i| (l0 + (l1 - l0) * i as f64 / n as f64).exp())
        .collect()
}

/// Insert `target` into the ascending `edges`, or — if an existing edge is
/// within `1e-9` relative of it — snap that edge exactly to `target` (so the
/// group-membership comparisons below are exact). Keeps `edges` strictly
/// ascending.
fn insert_edge(edges: &mut Vec<f64>, target: f64) {
    match edges.binary_search_by(|e| e.partial_cmp(&target).unwrap()) {
        Ok(_) => {} // already present exactly
        Err(pos) => {
            let near_lo = pos > 0 && (edges[pos - 1] - target).abs() <= 1e-9 * target;
            let near_hi = pos < edges.len() && (edges[pos] - target).abs() <= 1e-9 * target;
            if near_lo {
                edges[pos - 1] = target;
            } else if near_hi {
                edges[pos] = target;
            } else {
                edges.insert(pos, target);
            }
        }
    }
}

/// The fine grid with the two group boundaries forced onto edges.
fn fine_energy_grid(cfg: &ReactorPhysicsConfig) -> Result<Vec<f64>, ReactorPhysicsError> {
    if !(cfg.energy_min_ev < cfg.thermal_cutoff_ev
        && cfg.thermal_cutoff_ev < cfg.resonance_upper_ev
        && cfg.resonance_upper_ev < cfg.energy_max_ev
        && cfg.n_fine_bins >= 3)
    {
        return Err(ReactorPhysicsError::BadEnergyGrid);
    }
    let mut edges = log_energy_grid(cfg.energy_min_ev, cfg.energy_max_ev, cfg.n_fine_bins);
    insert_edge(&mut edges, cfg.thermal_cutoff_ev);
    insert_edge(&mut edges, cfg.resonance_upper_ev);
    Ok(edges)
}

/// Which group a fine bin `[e_lo, e_hi]` belongs to, given the two boundaries.
/// Every fine bin lies wholly in one group because the boundaries are edges.
fn group_of(e_hi: f64, cut_t: f64, cut_r: f64) -> Group {
    if e_hi <= cut_t * (1.0 + 1e-9) {
        Group::Thermal
    } else if e_hi <= cut_r * (1.0 + 1e-9) {
        Group::Resonance
    } else {
        Group::Fast
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Six-factor assembly (pure — unit-tested directly)
// ─────────────────────────────────────────────────────────────────────────────

/// Assemble the six factors from the group-resolved rates. `a`, `p`, `l` are
/// indexed `[thermal, resonance, fast]`; `a_thermal_fuel` is thermal absorption
/// in the fuel materials. See the module docs for the formulas — they telescope
/// exactly to `η·f·p·ε·P_FNL·P_TNL = P_total / (A_total + L_total)`.
fn assemble_six_factors(
    a: [Estimate; 3],
    a_thermal_fuel: Estimate,
    p: [Estimate; 3],
    l: [Estimate; 3],
    bounds: (f64, f64),
) -> SixFactors {
    let a_tot = Estimate::sum(&a);
    let p_tot = Estimate::sum(&p);
    let l_tot = Estimate::sum(&l);

    // Running denominators, low energy → high.
    let s_t = Estimate::sum(&[a[0], l[0]]); // A_t + L_t
    let s_tr = Estimate::sum(&[a[0], l[0], a[1], l[1]]); // + A_r + L_r
    let s_trf = Estimate::sum(&[s_tr, l[2]]); // + L_f

    let eta = Estimate::ratio(p[0], a_thermal_fuel);
    let f = Estimate::ratio(a_thermal_fuel, a[0]);
    let p_tnl = Estimate::ratio(a[0], s_t);
    let p_esc = Estimate::ratio(s_t, s_tr);
    let p_fnl = Estimate::ratio(s_tr, s_trf);

    // ε = (P_tot / P_t) · (A_tot + L_tot − A_f) / (A_tot + L_tot)
    let a_plus_l = Estimate::sum(&[a_tot, l_tot]);
    let epsilon = Estimate::product(&[
        Estimate::ratio(p_tot, p[0]),
        Estimate::ratio(Estimate::diff(a_plus_l, a[2]), a_plus_l),
    ]);

    let k_from_factors = Estimate::product(&[eta, f, p_esc, epsilon, p_fnl, p_tnl]);

    SixFactors {
        eta,
        f,
        p: p_esc,
        epsilon,
        p_fnl,
        p_tnl,
        k_from_factors,
        absorption_by_group: a,
        production_by_group: p,
        leakage_by_group: l,
        thermal_absorption_fuel: a_thermal_fuel,
        group_bounds_ev: bounds,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Entry point
// ─────────────────────────────────────────────────────────────────────────────

/// One tally bin → an [`Estimate`], as a **per-source-neutron** rate: the
/// per-generation mean over `n` active generations, divided by `per_source`
/// (the histories per generation). So absorption / production / leakage all
/// come out on the `[0, ~1]` scale where `P_total ≈ k` and
/// `A_total + L_total ≈ 1`.
fn bin_estimate(bin: &TallyBin, n: u64, per_source: f64) -> Estimate {
    let mean = bin.mean(n) / per_source;
    if mean == 0.0 {
        return Estimate { mean: 0.0, std: 0.0 };
    }
    let rsd = bin.rel_std_dev(n);
    let rsd = if rsd.is_finite() { rsd } else { 0.0 };
    Estimate {
        mean,
        std: mean.abs() * rsd,
    }
}

/// Run a k-eigenvalue power iteration over `geom` and capture the six-factor
/// decomposition and the lethargy-normalised flux spectrum from one combined
/// track-length tally plus explicit leakage accounting.
///
/// `materials` is the global material array the geometry's cells index into;
/// `nuclides` is the global nuclide array the materials index into.
///
/// # Errors
/// - [`ReactorPhysicsError::NoFuelMaterial`] — no `U23*` material is present.
/// - [`ReactorPhysicsError::BadEnergyGrid`] — the energy grid is ill-posed.
///
/// # Caveats
/// See the module docs: unscored void-gap flux, and conservative delta-method
/// ratio uncertainties.
pub fn run_keff_reactor_physics(
    geom: &Geometry,
    materials: &[Material],
    nuclides: &[Nuclide],
    config: &ReactorPhysicsConfig,
) -> Result<ReactorPhysicsReport, ReactorPhysicsError> {
    // ── Fuel materials (any nuclide named "U23*") ─────────────────────────────
    let fuel_mask: Vec<bool> = materials
        .iter()
        .map(|m| {
            m.components.iter().any(|c| {
                nuclides
                    .get(c.nuclide_idx)
                    .map(|n| n.name.starts_with("U23"))
                    .unwrap_or(false)
            })
        })
        .collect();
    if !fuel_mask.iter().any(|&b| b) {
        return Err(ReactorPhysicsError::NoFuelMaterial);
    }

    // ── Energy grid + combined tally ─────────────────────────────────────────
    let edges = fine_energy_grid(config)?;
    let n_e = edges.len() - 1;
    let n_mat = materials.len().max(1);
    let all_mats: Vec<usize> = (0..materials.len()).collect();

    // scores: 0 = Flux, 1 = Absorption, 2 = NuFission
    const N_SCORES: usize = 3;
    let mut tally = Tally {
        id: 0,
        name: "reactor_physics".into(),
        filters: vec![
            Box::new(EnergyFilter {
                bins: edges.clone(),
            }),
            Box::new(MaterialFilter {
                material_indices: all_mats,
            }),
        ],
        scores: vec![ScoreType::Flux, ScoreType::Absorption, ScoreType::NuFission],
        bins: vec![TallyBin::default(); n_e * n_mat * N_SCORES],
    };
    let mut leak_bins: Vec<TallyBin> = vec![TallyBin::default(); n_e];

    // ── Transport ────────────────────────────────────────────────────────────
    let keff = run_keff_csg_reactor_physics(
        geom,
        materials,
        nuclides,
        config.source_box,
        &config.keff,
        &mut tally,
        &edges,
        &mut leak_bins,
    );
    let n_active = config.keff.n_active as u64;
    let per_source = config.keff.n_particles.max(1) as f64;

    // ── Pull per-fine-bin rates out of the flat tally ────────────────────────
    // idx(e, m, s) = (e * n_mat + m) * N_SCORES + s
    let cut_t = config.thermal_cutoff_ev;
    let cut_r = config.resonance_upper_ev;

    let mut flux_bin = vec![Estimate::exact(0.0); n_e];
    let mut flux_raw = vec![0.0_f64; n_e];
    // group-summed rates
    let mut a_group = [Vec::<Estimate>::new(), Vec::new(), Vec::new()];
    let mut p_group = [Vec::<Estimate>::new(), Vec::new(), Vec::new()];
    let mut a_fuel_group = [Vec::<Estimate>::new(), Vec::new(), Vec::new()];

    for e in 0..n_e {
        let g = group_of(edges[e + 1], cut_t, cut_r) as usize;
        let mut flux_parts = Vec::with_capacity(n_mat);
        let mut a_parts = Vec::with_capacity(n_mat);
        let mut p_parts = Vec::with_capacity(n_mat);
        let mut a_fuel_parts = Vec::new();
        for m in 0..materials.len() {
            let base = (e * n_mat + m) * N_SCORES;
            let fl = bin_estimate(&tally.bins[base], n_active, per_source);
            let ab = bin_estimate(&tally.bins[base + 1], n_active, per_source);
            let pr = bin_estimate(&tally.bins[base + 2], n_active, per_source);
            flux_raw[e] += fl.mean;
            flux_parts.push(fl);
            a_parts.push(ab);
            p_parts.push(pr);
            if fuel_mask[m] {
                a_fuel_parts.push(ab);
            }
        }
        flux_bin[e] = Estimate::sum(&flux_parts);
        a_group[g].push(Estimate::sum(&a_parts));
        p_group[g].push(Estimate::sum(&p_parts));
        if !a_fuel_parts.is_empty() {
            a_fuel_group[g].push(Estimate::sum(&a_fuel_parts));
        }
    }

    // ── Leakage: per-bin spectrum, group sums, total ─────────────────────────
    let leak_spec: Vec<Estimate> = (0..n_e)
        .map(|e| bin_estimate(&leak_bins[e], n_active, per_source))
        .collect();
    let mut l_group = [Vec::<Estimate>::new(), Vec::new(), Vec::new()];
    for e in 0..n_e {
        let g = group_of(edges[e + 1], cut_t, cut_r) as usize;
        l_group[g].push(leak_spec[e]);
    }
    let leakage_total = Estimate::sum(&leak_spec);

    let a = [
        Estimate::sum(&a_group[0]),
        Estimate::sum(&a_group[1]),
        Estimate::sum(&a_group[2]),
    ];
    let p = [
        Estimate::sum(&p_group[0]),
        Estimate::sum(&p_group[1]),
        Estimate::sum(&p_group[2]),
    ];
    let l = [
        Estimate::sum(&l_group[0]),
        Estimate::sum(&l_group[1]),
        Estimate::sum(&l_group[2]),
    ];
    let a_thermal_fuel = Estimate::sum(&a_fuel_group[0]);

    let six_factors = assemble_six_factors(a, a_thermal_fuel, p, l, (cut_t, cut_r));

    // ── Lethargy-normalised spectrum ────────────────────────────────────────
    let total_flux: f64 = flux_raw.iter().sum();
    let flux_per_lethargy: Vec<Estimate> = (0..n_e)
        .map(|e| {
            let du = (edges[e + 1] / edges[e]).ln();
            if du <= 0.0 || total_flux <= 0.0 || flux_raw[e] == 0.0 {
                return Estimate { mean: 0.0, std: 0.0 };
            }
            let psi = flux_raw[e] / (du * total_flux);
            // du and the common normalisation S are treated as exact for the
            // per-bin shape (see module docs); the raw arrays are exposed for
            // full propagation.
            Estimate {
                mean: psi,
                std: psi * flux_bin[e].rel(),
            }
        })
        .collect();

    let ti = edges
        .iter()
        .position(|&x| (x - cut_t).abs() <= 1e-6 * cut_t)
        .unwrap_or(0);
    let ri = edges
        .iter()
        .position(|&x| (x - cut_r).abs() <= 1e-6 * cut_r)
        .unwrap_or(0);

    let spectrum = LethargySpectrum {
        energy_edges_ev: edges,
        flux_per_lethargy,
        flux_raw,
        flux_total: total_flux,
        group_edge_indices: (ti, ri),
    };

    // ── Consistency ────────────────────────────────────────────────────────
    let consistency_gap = if keff.k_mean != 0.0 {
        (keff.k_mean - six_factors.k_from_factors.mean) / keff.k_mean
    } else {
        f64::NAN
    };
    let consistent =
        consistency_gap.is_finite() && consistency_gap > CONSISTENCY_BAND.0 && consistency_gap < CONSISTENCY_BAND.1;

    Ok(ReactorPhysicsReport {
        keff,
        six_factors,
        spectrum,
        leakage_total,
        leakage_spectrum: leak_spec,
        consistency_gap,
        consistent,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::cell::{Cell, HalfSpaceSense, RegionToken};
    use crate::geometry::geometry::Geometry;
    use crate::geometry::position::Position;
    use crate::geometry::surface::{BoundaryType, SurfaceKind, XPlane, YPlane, ZCylinder};
    use crate::geometry::universe::Universe;
    use crate::material::material::{Material, NuclideComponent};
    use crate::material::nuclide::Nuclide;

    /// Godiva HEU (HEU-MET-FAST-001) atom densities as the fissile fuel.
    fn heu_fuel() -> Material {
        Material {
            id: 1,
            name: "Godiva HEU".into(),
            temperature: 293.6,
            components: vec![
                NuclideComponent { nuclide_idx: 0, atom_density: 4.9184e-4 },
                NuclideComponent { nuclide_idx: 1, atom_density: 4.4994e-2 },
                NuclideComponent { nuclide_idx: 2, atom_density: 2.4984e-3 },
            ],
        }
    }

    /// `0 U234, 1 U235, 2 U238, 3 H1` (free-gas H).
    fn nuclides() -> Vec<Nuclide> {
        vec![
            Nuclide::from_core("U234").unwrap(),
            Nuclide::from_core("U235").unwrap(),
            Nuclide::from_core("U238").unwrap(),
            Nuclide::from_core("H1").unwrap(),
        ]
    }

    /// Square pin cell: an HEU fuel cylinder of radius `r_fuel` in an H-1
    /// moderator of half-width `half`, infinite in z, x/y planes with boundary
    /// `bc`. Cell 0 = fuel (material 0), cell 1 = moderator (material 1).
    /// Mirrors `tests/openmc_notebooks/flux_spectrum.rs`; a large `half`
    /// (thick moderator) softens the spectrum toward thermal.
    fn pincell_sized(bc: BoundaryType, r_fuel: f64, half: f64) -> Geometry {
        let surfaces = vec![
            SurfaceKind::ZCylinder(ZCylinder { x0: 0.0, y0: 0.0, r: r_fuel, bc: BoundaryType::Transmissive }),
            SurfaceKind::XPlane(XPlane { x0: -half, bc }),
            SurfaceKind::XPlane(XPlane { x0: half, bc }),
            SurfaceKind::YPlane(YPlane { y0: -half, bc }),
            SurfaceKind::YPlane(YPlane { y0: half, bc }),
        ];
        let fuel = Cell::material(
            1,
            vec![RegionToken::HalfSpace { surface_idx: 0, sense: HalfSpaceSense::Inside }],
            0,
            293.6,
        );
        let moder = Cell::material(
            2,
            vec![
                RegionToken::HalfSpace { surface_idx: 0, sense: HalfSpaceSense::Outside },
                RegionToken::HalfSpace { surface_idx: 1, sense: HalfSpaceSense::Outside },
                RegionToken::Intersection,
                RegionToken::HalfSpace { surface_idx: 2, sense: HalfSpaceSense::Inside },
                RegionToken::Intersection,
                RegionToken::HalfSpace { surface_idx: 3, sense: HalfSpaceSense::Outside },
                RegionToken::Intersection,
                RegionToken::HalfSpace { surface_idx: 4, sense: HalfSpaceSense::Inside },
                RegionToken::Intersection,
            ],
            1,
            293.6,
        );
        Geometry {
            surfaces,
            cells: vec![fuel, moder],
            universes: vec![Universe { id: 0, cell_indices: vec![0, 1] }],
            lattices: vec![],
            root_universe: 0,
        }
    }

    /// The tight fast pin cell from `flux_spectrum.rs` (r = 0.4, half = 0.63).
    fn pincell(bc: BoundaryType) -> Geometry {
        pincell_sized(bc, 0.4, 0.63)
    }

    fn materials() -> Vec<Material> {
        vec![
            heu_fuel(),
            Material {
                id: 2,
                name: "H moderator".into(),
                temperature: 293.6,
                components: vec![NuclideComponent { nuclide_idx: 3, atom_density: 6.6e-2 }],
            },
        ]
    }

    fn cheap_keff() -> KeffSettings {
        KeffSettings {
            n_particles: 250,
            n_inactive: 8,
            n_active: 12,
            ..KeffSettings::default()
        }
    }

    fn config_for(r_fuel: f64) -> ReactorPhysicsConfig {
        ReactorPhysicsConfig {
            keff: cheap_keff(),
            source_box: SourceBox {
                lower: Position::new(-r_fuel, -r_fuel, -1.0),
                upper: Position::new(r_fuel, r_fuel, 1.0),
            },
            n_fine_bins: 80,
            ..Default::default()
        }
    }

    /// Fraction of the track-length flux below the thermal cutoff.
    fn thermal_flux_fraction(report: &ReactorPhysicsReport) -> f64 {
        let sp = &report.spectrum;
        let cut = report.six_factors.group_bounds_ev.0;
        let thermal: f64 = sp
            .flux_raw
            .iter()
            .zip(sp.energy_edges_ev.windows(2))
            .filter(|(_, w)| w[1] <= cut * (1.0 + 1e-9))
            .map(|(f, _)| *f)
            .sum();
        thermal / sp.flux_total.max(1e-30)
    }

    fn log_report(tag: &str, report: &ReactorPhysicsReport) {
        let sf = &report.six_factors;
        eprintln!(
            "[reactor_physics {tag}] k = {:.5} ± {:.5} | eta {:.4} f {:.4} p {:.4} eps {:.4} \
             P_FNL {:.4} P_TNL {:.4} | k_4f {:.5} leak {:.4} gap {:+.2}% consistent={}",
            report.keff.k_mean,
            report.keff.k_std,
            sf.eta.mean,
            sf.f.mean,
            sf.p.mean,
            sf.epsilon.mean,
            sf.p_fnl.mean,
            sf.p_tnl.mean,
            sf.k_from_factors.mean,
            report.leakage_total.mean,
            report.consistency_gap * 100.0,
            report.consistent
        );
    }

    /// Machinery invariants that hold on the tight fast pin cell (a stress case
    /// for the thermal/resonance/fast split) under both boundary conditions:
    ///
    /// - every factor is finite and non-negative;
    /// - the lethargy spectrum is non-negative and integrates to 1;
    /// - the **direct** eigenvalue `P_total / (A_total + L_total)` — which the
    ///   factors telescope to when no group is degenerate — matches `k_mean`
    ///   within a small band of `k_mean` (the residual is (n,2n));
    /// - leakage has the right sign and the non-leakage factors respond.
    #[test]
    fn machinery_invariants_fast_pincell() {
        for bc in [BoundaryType::Reflective, BoundaryType::Vacuum] {
            let report =
                run_keff_reactor_physics(&pincell(bc), &materials(), &nuclides(), &config_for(0.4))
                    .expect("Ok");
            log_report(
                if bc == BoundaryType::Reflective { "fast/refl" } else { "fast/vac" },
                &report,
            );
            let sf = &report.six_factors;

            for v in [
                sf.eta.mean, sf.f.mean, sf.p.mean, sf.epsilon.mean, sf.p_fnl.mean, sf.p_tnl.mean,
            ] {
                assert!(v.is_finite() && v >= 0.0, "factor {v} not finite/non-negative ({bc:?})");
            }

            let a_tot: f64 = sf.absorption_by_group.iter().map(|e| e.mean).sum();
            let p_tot: f64 = sf.production_by_group.iter().map(|e| e.mean).sum();
            let l_tot: f64 = sf.leakage_by_group.iter().map(|e| e.mean).sum();
            // Rates are per source neutron: every history dies once, so
            // A_total + L_total ≈ 1 (Σ_a = capture + fission; scatter conserves).
            assert!(
                a_tot + l_tot > 0.8,
                "A+L = {} per source neutron, expected ~1 ({bc:?})",
                a_tot + l_tot
            );
            let direct_k = p_tot / (a_tot + l_tot);
            let gap = (report.keff.k_mean - direct_k) / report.keff.k_mean;
            // With real Σ_a the gap is small; the margin here absorbs the
            // 250-history statistical noise and the LOW-tier free-gas treatment
            // on this hard spectrum. A tight gate lives in the ENDF+S(α,β)
            // Phase-C V&V.
            assert!(
                gap > -0.10 && gap < 0.10,
                "direct-k gap {gap:+.3} unexpectedly large with real Σ_a ({bc:?})"
            );

            let sp = &report.spectrum;
            let edges = &sp.energy_edges_ev;
            let mut integral = 0.0;
            for (i, psi) in sp.flux_per_lethargy.iter().enumerate() {
                assert!(psi.mean >= 0.0 && psi.mean.is_finite(), "psi[{i}] = {}", psi.mean);
                integral += psi.mean * (edges[i + 1] / edges[i]).ln();
            }
            assert!((integral - 1.0).abs() < 1e-9, "lethargy integral {integral} ({bc:?})");

            match bc {
                BoundaryType::Reflective => {
                    assert!(
                        report.leakage_total.mean < 1e-3,
                        "reflective leak {}",
                        report.leakage_total.mean
                    );
                    assert!(
                        (sf.p_fnl.mean - 1.0).abs() < 1e-9 && (sf.p_tnl.mean - 1.0).abs() < 1e-9
                    );
                }
                BoundaryType::Vacuum => {
                    assert!(
                        report.leakage_total.mean > 0.0,
                        "vacuum leak {}",
                        report.leakage_total.mean
                    );
                    assert!(sf.leakage_by_group[2].mean > 0.0, "fast-group leakage");
                    assert!(sf.p_fnl.mean < 1.0, "P_FNL {} not < 1", sf.p_fnl.mean);
                }
                _ => unreachable!(),
            }
        }
    }

    /// Thickening the moderator softens the spectrum: a wider water region moves
    /// more flux below the thermal cutoff and populates the thermal group's
    /// absorption. Proves the geometry → spectrum → group-rate chain responds to
    /// moderation (the LOW-tier free-gas-H data thermalises only approximately,
    /// so this is a *direction* check, not a spectral-accuracy one).
    ///
    /// `#[ignore]`d by default: a thick free-gas-H water region is many scatter
    /// events per history and slow. Run explicitly with
    /// `cargo test -p outram-mc-libs --release -- --ignored thicker_moderator`.
    #[test]
    #[ignore = "slow: thick free-gas-H moderator; a spectrum-direction diagnostic, not a gate"]
    fn thicker_moderator_softens_spectrum() {
        let tight =
            run_keff_reactor_physics(&pincell(BoundaryType::Reflective), &materials(), &nuclides(), &config_for(0.4))
                .expect("Ok");
        let wide = run_keff_reactor_physics(
            &pincell_sized(BoundaryType::Reflective, 0.4, 1.4),
            &materials(),
            &nuclides(),
            &config_for(0.4),
        )
        .expect("Ok");
        log_report("mod/tight", &tight);
        log_report("mod/wide", &wide);

        let ft = thermal_flux_fraction(&tight);
        let fw = thermal_flux_fraction(&wide);
        assert!(
            fw > ft,
            "wider moderator should raise the thermal flux fraction: tight {ft:.4} vs wide {fw:.4}"
        );
        assert!(
            wide.six_factors.absorption_by_group[0].mean > 0.0,
            "thermal-group absorption must be populated with a thick moderator"
        );
    }

    /// A geometry with only a moderator material errors rather than dividing by
    /// a zero fuel absorption.
    #[test]
    fn no_fuel_material_errors() {
        let moderator = Material {
            id: 1,
            name: "graphite".into(),
            temperature: 293.6,
            components: vec![NuclideComponent { nuclide_idx: 0, atom_density: 8.0e-2 }],
        };
        let nucs = vec![Nuclide::from_core("C0").unwrap()];
        let geom = pincell(BoundaryType::Reflective);
        let err = run_keff_reactor_physics(&geom, &[moderator], &nucs, &config_for(0.4))
            .unwrap_err();
        assert_eq!(err, ReactorPhysicsError::NoFuelMaterial);
    }

    /// Feed synthetic group rates through `assemble_six_factors` and check the
    /// product telescopes to `P_total / (A_total + L_total)` and that a
    /// non-leaking system has `P_FNL = P_TNL = 1` exactly.
    #[test]
    fn pure_algebra_telescopes() {
        let e = Estimate::exact;
        // [thermal, resonance, fast]
        let a = [e(0.60), e(0.10), e(0.05)];
        let p = [e(0.55), e(0.15), e(0.05)];
        let a_tf = e(0.50);

        // Non-leaking.
        let sf = assemble_six_factors(a, a_tf, p, [e(0.0); 3], (0.625, 1.0e5));
        assert!((sf.p_fnl.mean - 1.0).abs() < 1e-12, "P_FNL {}", sf.p_fnl.mean);
        assert!((sf.p_tnl.mean - 1.0).abs() < 1e-12, "P_TNL {}", sf.p_tnl.mean);
        let a_tot = 0.60 + 0.10 + 0.05;
        let p_tot = 0.55 + 0.15 + 0.05;
        assert!(
            (sf.k_from_factors.mean - p_tot / a_tot).abs() < 1e-12,
            "k_from_factors {} vs {}",
            sf.k_from_factors.mean,
            p_tot / a_tot
        );

        // Leaking.
        let l = [e(0.02), e(0.03), e(0.08)];
        let sf = assemble_six_factors(a, a_tf, p, l, (0.625, 1.0e5));
        assert!(sf.p_fnl.mean < 1.0 && sf.p_fnl.mean > 0.0);
        assert!(sf.p_tnl.mean < 1.0 && sf.p_tnl.mean > 0.0);
        let l_tot = 0.02 + 0.03 + 0.08;
        assert!(
            (sf.k_from_factors.mean - p_tot / (a_tot + l_tot)).abs() < 1e-12,
            "k_from_factors {} vs {}",
            sf.k_from_factors.mean,
            p_tot / (a_tot + l_tot)
        );
    }

    #[test]
    fn bad_energy_grid_errors() {
        let mut cfg = ReactorPhysicsConfig {
            thermal_cutoff_ev: 1.0e-4, // below energy_min_ev
            ..Default::default()
        };
        assert_eq!(
            fine_energy_grid(&cfg).unwrap_err(),
            ReactorPhysicsError::BadEnergyGrid
        );
        cfg.thermal_cutoff_ev = 1.0e6;
        cfg.resonance_upper_ev = 1.0e3; // out of order
        assert_eq!(
            fine_energy_grid(&cfg).unwrap_err(),
            ReactorPhysicsError::BadEnergyGrid
        );
    }

    #[test]
    fn fine_grid_forces_boundary_edges() {
        let cfg = ReactorPhysicsConfig::default();
        let edges = fine_energy_grid(&cfg).unwrap();
        assert!(edges.windows(2).all(|w| w[0] < w[1]), "strictly ascending");
        assert!(
            edges.iter().any(|&x| (x - 0.625).abs() < 1e-9),
            "thermal cutoff is an edge"
        );
        assert!(
            edges.iter().any(|&x| (x - 1.0e5).abs() < 1e-3),
            "resonance upper is an edge"
        );
        // no fine bin straddles a group boundary
        for w in edges.windows(2) {
            for &b in &[0.625_f64, 1.0e5] {
                assert!(
                    !(w[0] < b && b < w[1]),
                    "bin [{}, {}] straddles boundary {}",
                    w[0],
                    w[1],
                    b
                );
            }
        }
    }
}
