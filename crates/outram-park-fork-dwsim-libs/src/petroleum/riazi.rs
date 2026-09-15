//! Riazi's **C7+ bulk distribution** method — split a single set of averaged
//! (C7+) properties into `n` pseudo-components without any distillation curve.
//!
//! # Provenance
//!
//! Faithful port of DWSIM (GPL-3.0),
//! `DWSIM.Thermodynamics/PetroleumCharacterization/Riazi.vb` (459 lines, whole
//! file — the single method `Distr_Riazi`, `:23-455`), from the pinned upstream
//! clone `/home/teddy0/Documents/research/dwsim-upstream`, branch `windows`,
//! commit `1abf72d1b6b41d3e9a8cc770d3cc4e8fc76e5766`. Upstream copyright: 2008
//! Daniel Wagner O. de Medeiros and the DWSIM contributors. GPL-3.0; this port
//! is GPL-3.0-only.
//!
//! Upstream's only caller is the Windows-Forms `C7+ Characterization` dialog
//! (`DWSIM/Tools/C7plusCharacterization/FormPCBulk.vb:41`, `:812`), which is
//! GUI and therefore out of scope. The **method** is ported here so the
//! capability is not lost with the form.
//!
//! # The method
//!
//! Riazi's generalised distribution model writes any bulk property `P` as
//!
//! ```text
//! P*  = (P − P0) / P0
//! P*  = [ (A/B)·ln(1/(1 − x)) ]^(1/B)
//! ```
//!
//! where `x` is the cumulative fraction, `P0` is a property-specific lower
//! bound (`M0 = 90 g/mol`, `SG0 = 0.7`, `Tb0` from the SCN relation) and
//! `(A, B)` are fixed per property (`B = 1` for `M`, `3` for `SG`, `1.5` for
//! `Tb`, `0.7` for viscosity). Inverting it on a 1000-point grid and
//! differencing between cut boundaries gives each cut's mole fraction and mean
//! property.
//!
//! Whichever of `M`, `SG` or `Kw` the caller supplies drives the distribution;
//! the missing properties are back-filled from the Riazi-Al-Sahhaf SCN
//! relations. Critical constants then follow from Lee-Kesler, and viscosities
//! either from the same distribution model or from Letsou-Stiel.
//!
//! # Relationship to [`crate::petroleum::generate_compounds`]
//!
//! Both distribute bulk properties, but they are **different algorithms**:
//! `Distr_Riazi` evaluates the cut means with a *fitted exponential surrogate*
//! (`g(q) = b − (b − a)·exp(−c·q^d)`, `Riazi.vb:74-77` and its three siblings),
//! whereas `GenerateCompounds.vb` evaluates them with the *exact* incomplete
//! gamma integral. `GenerateCompounds` is the path DWSIM's modern assay manager
//! uses; this one is the older C7+ dialog's.
//!
//! # Units
//!
//! `uom` on the public surface, with two honest caveats inherited from
//! upstream:
//!
//! - **`t1`/`t2` are treated as °C.** Upstream writes `viscl_letsti(T1 +
//!   273.15, …)` (`:302`, `:363`), so its `T1`/`T2` are Celsius. This port
//!   takes `ThermodynamicTemperature` and converts with `°C + 273.15`,
//!   reproducing upstream exactly for the same physical temperature.
//! - **`v1`/`v2` are unit-ambiguous upstream.** When supplied they are fed
//!   through the distribution unchanged (so they come back out in whatever unit
//!   went in); when *not* supplied, upstream substitutes
//!   `1000 × viscl_letsti(...)`, i.e. a **dynamic** viscosity in mPa·s (cP)
//!   where a **kinematic** viscosity was expected. Both behaviours are
//!   reproduced; the values are carried as documented raw `f64` rather than
//!   given a `uom` dimension they do not consistently have.
//!
//! # Excluded DWSIM behavior
//!
//! Nothing is excluded. The untyped `mat(n-1, 10)` return matrix (`:431-451`)
//! becomes the typed [`RiaziDistributionCut`].
//!
//! # Upstream quirks preserved (all documented at their call sites below)
//!
//! - The **molecular-weight** branch indexes its grid with `/(n)` while the
//!   SG/Tb/viscosity branches use `/(n+1)` (`:62` versus `:115`, `:171`,
//!   `:321`) — an inconsistency, reproduced.
//! - Grid indices are VB `Double`-to-`Integer` conversions, i.e. **banker's
//!   rounding** ([`vb_round_to_i32`]).
//! - The final `i = n` iteration of each mean-property loop reuses the
//!   *previous* pair of `q` values (`:85-86` and siblings), so the last cut's
//!   mean repeats the second-to-last cut's increment.
//! - The `MW = 0, SG = 0, WK ≠ 0` block appears **twice**, identically
//!   (`:261-270` and `:272-281`); it is idempotent, so it is implemented once.
//! - Inside that block, `SG_p(i)` is assigned the **boiling-point** expression
//!   `1080 − exp(6.97996 − 0.01964·M^(2/3))` (`:266`, `:277`) instead of the
//!   specific-gravity one — a clear upstream defect, reproduced and flagged.
//! - `Pc_p` is converted bar → atm by `×0.986923` (`:292`) and the reported
//!   value is then `×1e5` (`:448`), so the tabulated critical pressure is
//!   **1.3 % low** relative to a true bar → Pa conversion. Reproduced.

use uom::si::f64::{MolarMass, Pressure, Ratio, ThermodynamicTemperature};
use uom::si::molar_mass::gram_per_mole;
use uom::si::pressure::pascal;
use uom::si::ratio::ratio;
use uom::si::thermodynamic_temperature::{degree_celsius, kelvin};

use crate::thermo::component::Component;
use crate::thermo::transport::liquid_viscosity_letsou_stiel;

use super::curve_conversion::vb_round_to_i32;
use super::property_methods::SpecificGravity;

/// Size of the internal inverse-distribution grid — upstream's fixed
/// `Dim MW_d(999)` … arrays, filled by `For i = 0 To 999` (`Riazi.vb:29`,
/// `:55-58`).
const GRID: usize = 1000;

/// One pseudo-component produced by [`distribute_riazi`] — a typed row of
/// upstream's `mat(n-1, 10)` matrix (`Riazi.vb:431-451`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RiaziDistributionCut {
    /// 1-based cut index — `mat(i, 0)`.
    pub index: usize,
    /// Normalised fraction of the stream in this cut [-] — `mat(i, 1)`.
    /// Whether this is a mole or volume fraction depends on which property
    /// drove the distribution (see the module docs); upstream calls the
    /// underlying array `Vx_dist`.
    pub fraction: Ratio,
    /// Running cumulative fraction through this cut [-] — `mat(i, 2)`.
    pub cumulative_fraction: Ratio,
    /// Mean normal boiling point of the cut [K] — `mat(i, 3)`.
    pub boiling_point: ThermodynamicTemperature,
    /// Mean molecular weight of the cut — `mat(i, 4)`, upstream in g/mol.
    pub molar_mass: MolarMass,
    /// Mean specific gravity of the cut [-] — `mat(i, 5)`, which upstream
    /// stores multiplied by 1000 (i.e. as kg/m³); this field carries the
    /// dimensionless gravity itself.
    pub specific_gravity: SpecificGravity,
    /// Cut viscosity at `t1` — `mat(i, 6)`. See the module "Units" caveat:
    /// this is the caller's own unit when `v1` was supplied, or **mPa·s** when
    /// upstream fell back to Letsou-Stiel.
    pub viscosity_1: f64,
    /// Cut viscosity at `t2` — `mat(i, 7)`. Same caveat as
    /// [`Self::viscosity_1`].
    pub viscosity_2: f64,
    /// Critical temperature by Lee-Kesler [K] — `mat(i, 8)`.
    pub critical_temperature: ThermodynamicTemperature,
    /// Critical pressure [Pa] — `mat(i, 9)`. Carries upstream's `×0.986923`
    /// bar→atm step followed by `×1e5`, so it reads **1.3 % low**; see the
    /// module docs.
    pub critical_pressure: Pressure,
    /// Acentric factor by Lee-Kesler [-] — `mat(i, 10)`.
    pub acentric_factor: Ratio,
}

/// Errors rejecting an unusable Riazi distribution request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum RiaziError {
    /// Fewer than two cuts were requested. Upstream would silently produce
    /// degenerate arrays; this port refuses.
    #[error("Riazi distribution needs at least 2 cuts, got {requested}")]
    TooFewCuts {
        /// The number requested.
        requested: usize,
    },
    /// None of molecular weight, specific gravity or Watson `K` was supplied,
    /// so there is nothing to distribute.
    #[error("Riazi distribution needs at least one of molar mass, specific gravity, or Watson K")]
    NoDrivingProperty,
}

/// Distribute a set of bulk C7+ properties into `cut_count` pseudo-components
/// by Riazi's generalised distribution model.
///
/// Ported from `Riazi.vb:23-455` (`Distr_Riazi`).
///
/// # Inputs
///
/// - `cut_count` — number of pseudo-components `n` (upstream's `n`), ≥ 2.
/// - `molar_mass` — bulk C7+ molecular weight. `None` maps to upstream's
///   `MW = 0` sentinel.
/// - `specific_gravity` — bulk C7+ specific gravity at 15.6/15.6 °C [-].
///   `None` maps to `SG = 0`.
/// - `watson_k` — bulk Watson characterisation factor [-]. `None` maps to
///   `WK = 0`. When supplied *with* a specific gravity it fixes the mean
///   boiling point through `Tb = (SG·Kw)³ × 0.55556` (`:157`).
/// - `t1`, `t2` — the two viscosity reference temperatures. **Read as °C** by
///   upstream; see the module "Units" note.
/// - `v1`, `v2` — bulk viscosities at `t1`, `t2`. `None` maps to upstream's
///   `V1 = 0` / `V2 = 0` sentinel, which switches to the Letsou-Stiel estimate.
///
/// # Valid range
///
/// The distribution is meaningful only for a genuine C7+ fraction:
/// `M > 90 g/mol`, `SG > 0.7`, `Tb > Tb0` — the model's `P*` becomes negative
/// otherwise and the grid inversion produces `NaN`. Upstream has no such guard;
/// this port likewise does not clamp, so `NaN` cuts are possible for
/// out-of-range bulk properties and the caller should check.
///
/// # Errors
///
/// [`RiaziError::TooFewCuts`] or [`RiaziError::NoDrivingProperty`].
#[allow(clippy::too_many_arguments)]
pub fn distribute_riazi(
    cut_count: usize,
    molar_mass: Option<MolarMass>,
    specific_gravity: Option<SpecificGravity>,
    watson_k: Option<Ratio>,
    t1: ThermodynamicTemperature,
    t2: ThermodynamicTemperature,
    v1: Option<f64>,
    v2: Option<f64>,
) -> Result<Vec<RiaziDistributionCut>, RiaziError> {
    let n = cut_count;
    if n < 2 {
        return Err(RiaziError::TooFewCuts { requested: n });
    }
    // Upstream's `<> 0` sentinels.
    let mw = molar_mass.map_or(0.0, |m| m.get::<gram_per_mole>());
    let sg = specific_gravity.map_or(0.0, |s| s.get::<ratio>());
    let wk = watson_k.map_or(0.0, |k| k.get::<ratio>());
    if mw == 0.0 && sg == 0.0 && wk == 0.0 {
        return Err(RiaziError::NoDrivingProperty);
    }
    let vv1 = v1.unwrap_or(0.0);
    let vv2 = v2.unwrap_or(0.0);

    let mut mw_p = vec![0.0_f64; n];
    let mut sg_p = vec![0.0_f64; n];
    let mut tb_p = vec![0.0_f64; n];
    let mut v1_p = vec![0.0_f64; n];
    let mut v2_p = vec![0.0_f64; n];
    let mut vx_dist = vec![0.0_f64; n];

    // ---- molecular-weight distribution (`:47-98`) -------------------------
    if mw != 0.0 {
        let mw0 = 90.0;
        let a = mw / mw0 - 1.0;
        let b = 1.0;
        let d = inverse_distribution_grid(a, b);
        // NOTE: this branch divides by `n`, while every sibling divides by
        // `n + 1` (`:62` versus `:115`, `:171`, `:321`). Upstream inconsistency.
        let z = cut_fractions(&d, a, b, n, n);
        vx_dist.copy_from_slice(&z);
        let q = cut_q_values(&d, a, b, n);
        let p = cut_means_surrogate(
            &z,
            &q,
            a,
            b,
            n,
            (
                1.0047729176981601,
                0.00836759925436675,
                0.31579045418979956,
                1.4969389791593306,
            ),
        );
        for i in 0..n {
            mw_p[i] = mw0 * (1.0 + p[i]);
        }
    }

    // ---- specific-gravity distribution (`:100-151`) -----------------------
    if sg != 0.0 {
        let sg0 = 0.7;
        let avx = sg / sg0 - 1.0;
        let a = (avx / 0.619).powi(3);
        let b = 3.0;
        let d = inverse_distribution_grid(a, b);
        let z = cut_fractions(&d, a, b, n, n + 1);
        vx_dist.copy_from_slice(&z);
        let q = cut_q_values(&d, a, b, n);
        let p = cut_means_surrogate(
            &z,
            &q,
            a,
            b,
            n,
            (
                0.90042013692039768,
                -0.0049102522593429,
                0.76738843430465553,
                0.98214367448668149,
            ),
        );
        for i in 0..n {
            sg_p[i] = sg0 * (1.0 + p[i]);
        }
    }

    // ---- boiling-point distribution (`:153-207`) --------------------------
    if wk != 0.0 {
        let tb0 = 1080.0 - (6.97996 - 0.01964 * 90.0_f64.powf(2.0 / 3.0)).exp();
        let tb = (sg * wk).powi(3) * 0.55556;
        let avx = tb / tb0 - 1.0;
        let a = (avx / 0.689).powf(1.5);
        let b = 1.5;
        let d = inverse_distribution_grid(a, b);
        let z = cut_fractions(&d, a, b, n, n + 1);
        vx_dist.copy_from_slice(&z);
        let q = cut_q_values(&d, a, b, n);
        let p = cut_means_surrogate(
            &z,
            &q,
            a,
            b,
            n,
            (
                0.9055123773075332,
                0.023814486817079358,
                0.28514645349487877,
                1.9566747404253064,
            ),
        );
        for i in 0..n {
            tb_p[i] = tb0 * (1.0 + p[i]);
        }
    }

    // ---- back-fill the properties the caller did not supply (`:209-281`) --
    let tb_from_mw = |m: f64| 1080.0 - (6.97996 - 0.01964 * m.powf(2.0 / 3.0)).exp();
    let sg_from_mw = |m: f64| 1.07 - (3.56073 - 2.93886 * m.powf(0.1)).exp();
    let mw_from_sg = |s: f64| (((1.07 - s).ln() - 3.56073) / -2.93886).powi(10);
    let mw_from_tb = |t: f64| (1.0 / 0.01964 * (6.97996 - (1080.0 - t).ln())).powf(1.5);

    if mw != 0.0 && sg == 0.0 && wk == 0.0 {
        for i in 0..n {
            tb_p[i] = tb_from_mw(mw_p[i]);
            sg_p[i] = sg_from_mw(mw_p[i]);
        }
    }
    if mw != 0.0 && sg != 0.0 && wk == 0.0 {
        for i in 0..n {
            tb_p[i] = tb_from_mw(mw_p[i]);
        }
    }
    if mw != 0.0 && sg == 0.0 && wk != 0.0 {
        for i in 0..n {
            sg_p[i] = sg_from_mw(mw_p[i]);
        }
    }
    if mw == 0.0 && sg != 0.0 && wk == 0.0 {
        for i in 0..n {
            mw_p[i] = mw_from_sg(sg_p[i]);
            tb_p[i] = tb_from_mw(mw_p[i]);
        }
    }
    if mw == 0.0 && sg != 0.0 && wk != 0.0 {
        for i in 0..n {
            mw_p[i] = mw_from_sg(sg_p[i]);
        }
    }
    if mw == 0.0 && sg == 0.0 && wk != 0.0 {
        // Upstream repeats this identical block twice (`:261-270`, `:272-281`);
        // it is idempotent, so it runs once here.
        for i in 0..n {
            mw_p[i] = mw_from_tb(tb_p[i]);
            // UPSTREAM DEFECT, reproduced: this assigns the *boiling-point*
            // expression to the specific gravity (`:266`, `:277`).
            sg_p[i] = tb_from_mw(mw_p[i]);
        }
    }

    // ---- critical constants (`:283-294`) ----------------------------------
    let mut tc_p = vec![0.0_f64; n];
    let mut pc_p = vec![0.0_f64; n];
    let mut w_p = vec![0.0_f64; n];
    for i in 0..n {
        let s = sg_p[i];
        let t = tb_p[i];
        tc_p[i] =
            189.8 + 450.6 * s + (0.4244 + 0.1174 * s) * t + (0.1441 - 1.0069 * s) * 100_000.0 / t;
        pc_p[i] = (5.689 - 0.0566 / s - (0.43639 + 4.1216 / s + 0.21343 / s.powi(2)) * 0.001 * t
            + (0.47579 + 1.182 / s + 0.15302 / s.powi(2)) * 0.000_001 * t.powi(2)
            - (2.4505 + 9.9099 / s.powi(2)) * 0.000_000_000_1 * t.powi(3))
        .exp();
        let tbr = t / tc_p[i];
        w_p[i] = (-(pc_p[i] / 1.01325).ln() - 5.92714 + 6.09648 / tbr + 1.28862 * tbr.ln()
            - 0.169347 * tbr.powi(6))
            / (15.2518 - 15.6875 / tbr - 13.4721 * tbr.ln() + 0.43577 * tbr.powi(6));
        // bar -> atm (`:292`); the reported value is later multiplied by 1e5.
        pc_p[i] *= 0.986923;
    }

    // ---- viscosities (`:296-419`) -----------------------------------------
    let t1_c = t1.get::<degree_celsius>();
    let t2_c = t2.get::<degree_celsius>();
    // Riazi's viscosity surrogate coefficients are the same for both points
    // (`:333-336`, `:395-398`).
    const VISC_SURROGATE: (f64, f64, f64, f64) = (
        1.2779512667511526,
        0.0097469995695727268,
        0.19222719403601637,
        1.6783369425946355,
    );

    if vv1 == 0.0 {
        for i in 0..n {
            v1_p[i] = 1000.0
                * letsou_stiel_mpa_s(t1_c + 273.15, tc_p[i], pc_p[i] * 100_000.0, w_p[i], mw_p[i]);
        }
    } else {
        let v0 = 0.1 * vv1;
        let avx = vv1 / v0 - 1.0;
        let b = 0.7_f64;
        let a = (avx
            / (0.992814 - 0.504242 * b.powi(-1) + 0.696215 * b.powi(-2) - 0.272936 * b.powi(-3)
                + 0.088362 * b.powi(-4)))
        .powf(b)
            * b;
        let d = inverse_distribution_grid(a, b);
        let z = cut_fractions(&d, a, b, n, n + 1);
        vx_dist.copy_from_slice(&z);
        let q = cut_q_values(&d, a, b, n);
        let p = cut_means_surrogate(&z, &q, a, b, n, VISC_SURROGATE);
        for i in 0..n {
            v1_p[i] = v0 * (1.0 + p[i]);
        }
    }

    if vv2 == 0.0 {
        for i in 0..n {
            v2_p[i] = 1000.0
                * letsou_stiel_mpa_s(t2_c + 273.15, tc_p[i], pc_p[i] * 100_000.0, w_p[i], mw_p[i]);
        }
    } else {
        let v0 = 0.1 * vv2;
        let avx = vv2 / v0 - 1.0;
        let b = 0.7_f64;
        let a = (avx
            / (0.992814 - 0.504242 * b.powi(-1) + 0.696215 * b.powi(-2) - 0.272936 * b.powi(-3)
                + 0.088362 * b.powi(-4)))
        .powf(b)
            * b;
        let d = inverse_distribution_grid(a, b);
        let z = cut_fractions(&d, a, b, n, n + 1);
        vx_dist.copy_from_slice(&z);
        let q = cut_q_values(&d, a, b, n);
        let p = cut_means_surrogate(&z, &q, a, b, n, VISC_SURROGATE);
        for i in 0..n {
            v2_p[i] = v0 * (1.0 + p[i]);
        }
    }

    // ---- assemble the result matrix (`:421-453`) --------------------------
    let total: f64 = vx_dist.iter().sum();
    let mut cumulative = 0.0;
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let fraction = vx_dist[i] / total;
        cumulative += fraction;
        out.push(RiaziDistributionCut {
            index: i + 1,
            fraction: Ratio::new::<ratio>(fraction),
            cumulative_fraction: Ratio::new::<ratio>(cumulative),
            boiling_point: ThermodynamicTemperature::new::<kelvin>(tb_p[i]),
            molar_mass: MolarMass::new::<gram_per_mole>(mw_p[i]),
            specific_gravity: Ratio::new::<ratio>(sg_p[i]),
            viscosity_1: v1_p[i],
            viscosity_2: v2_p[i],
            critical_temperature: ThermodynamicTemperature::new::<kelvin>(tc_p[i]),
            critical_pressure: Pressure::new::<pascal>(pc_p[i] * 100_000.0),
            acentric_factor: Ratio::new::<ratio>(w_p[i]),
        });
    }
    Ok(out)
}

/// Build the 1000-point inverse-distribution grid
/// `d(i) = [ (A/B)·ln(1/(1 − i/1000)) ]^(1/B)` — `Riazi.vb:55-58` and its three
/// siblings. `d(0) = 0` and the grid rises monotonically.
fn inverse_distribution_grid(a: f64, b: f64) -> Vec<f64> {
    (0..GRID)
        .map(|i| (a / b * (1.0 / (1.0 - i as f64 / 1000.0)).ln()).powf(1.0 / b))
        .collect()
}

/// Cut fractions `z(i) = exp(−B/A·d[lo]^B) − exp(−B/A·d[hi]^B)` — `Riazi.vb:60-64`
/// and siblings. `divisor` is upstream's per-branch denominator (`n` for the
/// molecular-weight branch, `n + 1` everywhere else — see the module docs).
///
/// Grid indices are VB `Double`-to-`Integer` conversions, i.e. banker's
/// rounding, reproduced through [`vb_round_to_i32`].
fn cut_fractions(d: &[f64], a: f64, b: f64, n: usize, divisor: usize) -> Vec<f64> {
    (1..=n)
        .map(|i| {
            let lo = grid_index(d, (i as f64 - 1.0) * 999.0 / divisor as f64);
            let hi = grid_index(d, i as f64 * 999.0 / divisor as f64);
            (-b / a * lo.powf(b)).exp() - (-b / a * hi.powf(b)).exp()
        })
        .collect()
}

/// The `q(i) = B/A·d[(i+1)·1000/(n+1)]^B` sequence — `Riazi.vb:68-72` and
/// siblings.
fn cut_q_values(d: &[f64], a: f64, b: f64, n: usize) -> Vec<f64> {
    (0..n)
        .map(|i| {
            let x = grid_index(d, (i as f64 + 1.0) * 1000.0 / (n as f64 + 1.0));
            b / a * x.powf(b)
        })
        .collect()
}

/// Cut mean properties from Riazi's fitted exponential surrogate
/// `g(q) = b − (b − a)·exp(−c·q^d)`:
/// `p(i) = 1/z(i)·(A/B)^(1/B)·(g(q_{i−1}) − g(q_i))` — `Riazi.vb:79-90` and
/// siblings.
///
/// The final iteration (`i == n`) deliberately reuses `q(n−2)`/`q(n−1)` rather
/// than `q(n−1)`/`q(n)` (`:85-86`), so the last cut repeats the previous
/// increment. Reproduced.
fn cut_means_surrogate(
    z: &[f64],
    q: &[f64],
    a_param: f64,
    b_param: f64,
    n: usize,
    surrogate: (f64, f64, f64, f64),
) -> Vec<f64> {
    let (a, b, c, d) = surrogate;
    let g = |x: f64| b - (b - a) * (-c * x.powf(d)).exp();
    (1..=n)
        .map(|i| {
            let (g1, g2) = if i != n {
                (g(q[i - 1]), g(q[i]))
            } else {
                (g(q[i - 2]), g(q[i - 1]))
            };
            1.0 / z[i - 1] * (a_param / b_param).powf(1.0 / b_param) * (g1 - g2)
        })
        .collect()
}

/// Index the 1000-point grid the way VB does: convert the `Double` subscript to
/// an `Integer` with banker's rounding, then clamp into range (upstream would
/// throw on an out-of-range subscript; the arithmetic above never produces one
/// for `n >= 2`, so the clamp is defensive only).
fn grid_index(d: &[f64], subscript: f64) -> f64 {
    let i = vb_round_to_i32(subscript).clamp(0, (GRID - 1) as i32) as usize;
    d[i]
}

/// Letsou-Stiel saturated-liquid dynamic viscosity in **Pa·s**, evaluated
/// through [`crate::thermo::transport::liquid_viscosity_letsou_stiel`] — the
/// crate's existing port of the same DWSIM routine (`FluidProperties.vb:142`
/// `viscl_letsti`) that `Riazi.vb:302`/`:363` calls.
///
/// `tc` [K], `pc` [Pa], `w` [-], `mw` [g/mol]. Returns `0.0` for non-physical
/// constants, where upstream would produce `NaN`.
fn letsou_stiel_mpa_s(t: f64, tc: f64, pc: f64, w: f64, mw: f64) -> f64 {
    if !(tc > 0.0) || !(pc > 0.0) || !(mw > 0.0) {
        return 0.0;
    }
    let component = Component {
        name: String::new(),
        molar_mass: mw / 1000.0,
        critical_temperature: tc,
        critical_pressure: pc,
        critical_volume: f64::NAN,
        acentric_factor: w,
        normal_boiling_point: f64::NAN,
        cp_ig_a: 0.0,
        cp_ig_b: 0.0,
        cp_ig_c: 0.0,
        cp_ig_d: 0.0,
        cp_ig_e: 0.0,
        ig_entropy_formation_25c: f64::NAN,
    };
    liquid_viscosity_letsou_stiel(&component, ThermodynamicTemperature::new::<kelvin>(t))
        .get::<uom::si::dynamic_viscosity::pascal_second>()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tk(v: f64) -> ThermodynamicTemperature {
        ThermodynamicTemperature::new::<kelvin>(v)
    }

    /// **Methodology.** Distribute a C7+ fraction of `M = 200 g/mol`,
    /// `SG = 0.82` into 8 cuts with no viscosity data. A physically sensible
    /// distribution must: (a) have fractions summing to exactly 1, (b) rise
    /// monotonically in molecular weight, boiling point and critical
    /// temperature across the cuts (the model is a *heavy-tail* distribution,
    /// so later cuts are heavier), and (c) produce finite positive constants
    /// throughout.
    ///
    /// **Results (2026-08-11, this port).** Reported by the assertions below;
    /// the numeric values are printed on failure. Fractions sum to 1 to within
    /// 1e-12; `M`, `Tb` and `Tc` are all strictly increasing across the 8 cuts.
    /// Test passes.
    #[test]
    fn c7plus_distribution_is_monotone_and_closes() {
        let cuts = distribute_riazi(
            8,
            Some(MolarMass::new::<gram_per_mole>(200.0)),
            Some(Ratio::new::<ratio>(0.82)),
            None,
            tk(37.8 + 273.15),
            tk(98.9 + 273.15),
            None,
            None,
        )
        .expect("a C7+ fraction with M and SG is a valid request");
        assert_eq!(cuts.len(), 8);

        let sum: f64 = cuts.iter().map(|c| c.fraction.get::<ratio>()).sum();
        assert!((sum - 1.0).abs() < 1.0e-12, "fractions sum to {sum}");
        assert!(
            (cuts[7].cumulative_fraction.get::<ratio>() - 1.0).abs() < 1.0e-12,
            "cumulative does not reach 1: {}",
            cuts[7].cumulative_fraction.get::<ratio>()
        );

        for w in cuts.windows(2) {
            assert!(
                w[1].molar_mass.get::<gram_per_mole>() > w[0].molar_mass.get::<gram_per_mole>(),
                "M not increasing: {:?} then {:?}",
                w[0].molar_mass.get::<gram_per_mole>(),
                w[1].molar_mass.get::<gram_per_mole>()
            );
            assert!(
                w[1].boiling_point.get::<kelvin>() > w[0].boiling_point.get::<kelvin>(),
                "Tb not increasing"
            );
            assert!(
                w[1].critical_temperature.get::<kelvin>()
                    > w[0].critical_temperature.get::<kelvin>(),
                "Tc not increasing"
            );
        }
        for c in &cuts {
            assert!(c.critical_pressure.get::<pascal>() > 0.0, "{c:?}");
            assert!(c.specific_gravity.get::<ratio>() > 0.0, "{c:?}");
            assert!(c.viscosity_1.is_finite(), "{c:?}");
            assert!(c.viscosity_2.is_finite(), "{c:?}");
        }
    }

    /// **Methodology.** The mole-weighted average molecular weight of the
    /// generated cuts should recover the bulk molecular weight that drove the
    /// distribution. This is the distribution's own closure condition. Riazi's
    /// exponential *surrogate* for the cut means is an approximation (see the
    /// module docs), so the gate is a loose 20 %.
    ///
    /// **Results (2026-08-11, this port).** Bulk `M = 200 g/mol` in; mole-
    /// weighted average of the 8 generated cuts comes back within the 20 %
    /// gate. The residual is the surrogate's own error, inherited from
    /// upstream, not a porting defect — DWSIM's own
    /// [`crate::petroleum::generate_compounds`] path uses the exact incomplete
    /// gamma integral instead and closes much more tightly. Test passes.
    #[test]
    fn mole_weighted_molar_mass_recovers_the_bulk_value() {
        let cuts = distribute_riazi(
            8,
            Some(MolarMass::new::<gram_per_mole>(200.0)),
            Some(Ratio::new::<ratio>(0.82)),
            None,
            tk(37.8 + 273.15),
            tk(98.9 + 273.15),
            None,
            None,
        )
        .expect("valid request");
        let average: f64 = cuts
            .iter()
            .map(|c| c.fraction.get::<ratio>() * c.molar_mass.get::<gram_per_mole>())
            .sum();
        assert!(
            ((average - 200.0) / 200.0).abs() < 0.20,
            "mole-weighted M = {average} g/mol versus bulk 200"
        );
    }

    /// **Methodology.** Bad requests must be rejected rather than producing
    /// degenerate arrays: fewer than two cuts, and no driving property at all.
    ///
    /// **Results (2026-08-11, this port).** `cut_count = 1` returns
    /// `TooFewCuts`; all-`None` properties return `NoDrivingProperty`. Test
    /// passes.
    #[test]
    fn bad_requests_are_rejected() {
        assert!(matches!(
            distribute_riazi(
                1,
                Some(MolarMass::new::<gram_per_mole>(200.0)),
                None,
                None,
                tk(311.0),
                tk(372.0),
                None,
                None
            ),
            Err(RiaziError::TooFewCuts { .. })
        ));
        assert!(matches!(
            distribute_riazi(8, None, None, None, tk(311.0), tk(372.0), None, None),
            Err(RiaziError::NoDrivingProperty)
        ));
    }
}
