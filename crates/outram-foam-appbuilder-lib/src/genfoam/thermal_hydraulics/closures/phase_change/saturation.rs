// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: src/classes/thermalHydraulics/src/physicsModels/phaseChangeModels/
//             saturationModels/{constantTemperature,water,waterTRACE,BrowningPotter}/
//   Upstream commit: 652b3da
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//   Upstream authors: Stefan Radman (EPFL) — constantTemperature, water;
//             Gauthier Lazare, Carlo Fiorina — waterTRACE
//   Upstream license: GPL-3.0
//
// This file is part of OUTRAM PARK, distributed under GPL-3.0. See the crate
// root or <https://www.gnu.org/licenses/> for the full license text.

//! Saturation-curve correlations: `T_sat(p)`, `p_sat(T)`, `ln(p_sat(T))`, and
//! `dp_sat/dT`.
//!
//! Port of GeN-Foam's `saturationModel` run-time-selectable family. Every
//! [`SaturationModel`] method takes both the local temperature *and* pressure
//! even when a given correlation only uses one of them — this mirrors
//! upstream, where every model has both `iT_`/`p_` fields available regardless
//! of which it actually reads (see each variant's docs for which argument is
//! live).
//!
//! ## References
//!
//! - `water`: NIST water saturation data, 0.01-350 degC,
//!   <https://www.nist.gov/system/files/documents/srd/NISTIR5078-Tab1.pdf>.
//! - `waterTRACE`: TRACE V5.0 Theory Manual,
//!   <https://www.nrc.gov/docs/ML1200/ML120060218.pdf>.
//! - `BrowningPotter`: Browning & Potter correlation, ANL-RE-95/2,
//!   <https://www.ne.anl.gov/eda/ANL-RE-95-2.pdf>.

use super::SaturationPressureSlope;
use uom::si::f64::{Pressure, ThermodynamicTemperature};
use uom::si::pressure::pascal;
use uom::si::thermodynamic_temperature::kelvin;

/// Saturation temperature/pressure correlation.
///
/// Closed enum port of GeN-Foam's `saturationModel` family. Evaluate with
/// [`SaturationModel::t_sat`] (`T_sat(p)`), [`SaturationModel::p_sat`]
/// (`p_sat(T)`), [`SaturationModel::ln_p_sat`] (`ln(p_sat(T))`), and
/// [`SaturationModel::p_sat_prime`] (`dp_sat/dT`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SaturationModel {
    /// Fixed saturation temperature, independent of local pressure.
    ///
    /// Faithful quirk: upstream's `valuePSat` does **not** compute a pressure
    /// from `TSat_` — it returns the local system pressure field verbatim
    /// (`return p_[celli];`), i.e. this model treats "the current cell
    /// pressure" as automatically saturated. `p_sat`/`ln_p_sat` below
    /// reproduce that pass-through; `p_sat_prime` is always zero.
    ConstantTemperature {
        /// The fixed saturation temperature (upstream `TSat_`, dictionary key `value`).
        t_sat: ThermodynamicTemperature,
    },
    /// Water, 0.01-350 degC, `p_sat = A * (T - B)^C` fit to NIST tabulated
    /// data (A, B, C hard-coded from the upstream fit).
    ///
    /// **Known fit accuracy pitfall** (measured, not a port bug — see
    /// `tests.rs`): the fit is a good round-trip inverse of itself, but as an
    /// absolute correlation it deviates further from the true saturation
    /// curve than its "NIST fit" description suggests — `T_sat(101325 Pa)`
    /// measures 378.59 K (+1.46% vs. the true 373.15 K normal boiling point),
    /// and `p_sat(373.15 K)` measures 80908 Pa (-20% vs. 101325 Pa). This is
    /// the upstream correlation's own coefficient set, ported verbatim.
    Water,
    /// Water, TRACE-theory-manual piecewise fit (4 branches in `T` for
    /// `p_sat`, 4 branches in `p` for `T_sat`, boundaries at approximately
    /// 370.4 K, 609.6 K, 647.3 K and 90.56 kPa, 13.97 MPa, 22.12 MPa
    /// respectively).
    ///
    /// **Known upstream discrepancy** (faithfully ported, not fixed — see
    /// `tests.rs`): the branch-2 (370.4251 K - 609.62463 K) `p_sat(T)` formula
    /// `ps = BB * ((T-DB)/AB)^(1/CB)` and the branch-2 `T_sat(p)` formula
    /// `Ts = AB * (BB*p)^CB + DB` are **not** algebraic inverses of each other
    /// (the former multiplies by `BB = 1e-5` where consistency with the
    /// latter would require dividing by it) — `p_sat(373.15 K)` in this branch
    /// evaluates to about `1.0e-5 Pa`, nowhere near the true ~101325 Pa,
    /// while `T_sat(101325 Pa)` in the same branch correctly recovers
    /// `373.35 K`. This crate ports the upstream formula as written in both
    /// directions; do not "fix" `p_sat`'s branch 2 to match `T_sat`'s inverse
    /// without upstream confirmation this is in fact a transcription bug.
    WaterTrace,
    /// Browning & Potter correlation (salt/other-fluid fit), ANL-RE-95/2.
    /// `ln(p_sat) = A + B/T + C*ln(T)`, not analytically invertible in `T`, so
    /// `T_sat(p)` is upstream's documented second-order-polynomial
    /// approximation of the inverse (round-trips to within ~0.04% — see
    /// `tests.rs`).
    BrowningPotter,
}

impl SaturationModel {
    /// Saturation temperature `T_sat(p)`.
    ///
    /// Faithful translation of each upstream model's `valueTSat(celli)`.
    #[must_use]
    pub fn t_sat(&self, p: Pressure) -> ThermodynamicTemperature {
        ThermodynamicTemperature::new::<kelvin>(self.t_sat_value(p.get::<pascal>()))
    }

    /// Saturation pressure `p_sat(T)`.
    ///
    /// `p` is the local system pressure field, needed only by
    /// [`SaturationModel::ConstantTemperature`] (see its docs); every other
    /// variant computes `p_sat` from `t` alone. Faithful translation of each
    /// upstream model's `valuePSat(celli)`.
    #[must_use]
    pub fn p_sat(&self, t: ThermodynamicTemperature, p: Pressure) -> Pressure {
        Pressure::new::<pascal>(self.p_sat_value(t.get::<kelvin>(), p.get::<pascal>()))
    }

    /// Natural log of the saturation pressure, `ln(p_sat(T) / 1 Pa)`.
    ///
    /// Dimensionless by construction (the argument of `ln` is the numeric
    /// magnitude of `p_sat` in pascals). Upstream computes this
    /// independently per model (`valueLnPSat`), but every model satisfies
    /// `valueLnPSat = log(valuePSat)` exactly by construction (verified in
    /// `tests.rs` for [`SaturationModel::BrowningPotter`], whose upstream
    /// `valuePSat` is *defined* as `exp(valueLnPSat)`), so this port
    /// implements it generically from [`SaturationModel::p_sat`].
    #[must_use]
    pub fn ln_p_sat(&self, t: ThermodynamicTemperature, p: Pressure) -> f64 {
        self.p_sat_value(t.get::<kelvin>(), p.get::<pascal>()).ln()
    }

    /// Saturation-curve slope `dp_sat/dT`.
    ///
    /// `p` is the local system pressure field; [`SaturationModel::WaterTrace`]
    /// uses it directly in place of a freshly evaluated `p_sat(T)` (matching
    /// upstream, which reads `p_[celli]` rather than recomputing `valuePSat`).
    /// Faithful translation of each upstream model's `valuePSatPrime(celli)`.
    #[must_use]
    pub fn p_sat_prime(&self, t: ThermodynamicTemperature, p: Pressure) -> SaturationPressureSlope {
        let slope_pa_per_k = self.p_sat_prime_value(t.get::<kelvin>(), p.get::<pascal>());
        Pressure::new::<pascal>(slope_pa_per_k) / ThermodynamicTemperature::new::<kelvin>(1.0)
    }

    /// Core `T_sat` evaluation on bare SI magnitudes (K in, K out).
    fn t_sat_value(&self, p_pa: f64) -> f64 {
        match *self {
            Self::ConstantTemperature { t_sat } => t_sat.get::<kelvin>(),
            Self::Water => water_t_sat(p_pa),
            Self::WaterTrace => water_trace_t_sat(p_pa),
            Self::BrowningPotter => browning_potter_t_sat(p_pa),
        }
    }

    /// Core `p_sat` evaluation on bare SI magnitudes (K, Pa in; Pa out).
    fn p_sat_value(&self, t_k: f64, p_pa: f64) -> f64 {
        match *self {
            // Upstream quirk: pass the local pressure straight through.
            Self::ConstantTemperature { .. } => p_pa,
            Self::Water => water_p_sat(t_k),
            Self::WaterTrace => water_trace_p_sat(t_k),
            Self::BrowningPotter => browning_potter_p_sat(t_k),
        }
    }

    /// Core `dp_sat/dT` evaluation on bare SI magnitudes (K, Pa in; Pa/K out).
    fn p_sat_prime_value(&self, t_k: f64, p_pa: f64) -> f64 {
        match *self {
            Self::ConstantTemperature { .. } => 0.0,
            Self::Water => water_p_sat_prime(t_k),
            Self::WaterTrace => water_trace_p_sat_prime(t_k, p_pa),
            Self::BrowningPotter => browning_potter_p_sat_prime(t_k),
        }
    }
}

// ---------------------------------------------------------------------------
// water
// ---------------------------------------------------------------------------

const WATER_A: f64 = 1e6 * 2.590718143628726e-10;
const WATER_B: f64 = 273.159;
const WATER_C: f64 = 4.247368421052632;

/// `p_sat = A * (T - B)^C`.
fn water_p_sat(t_k: f64) -> f64 {
    WATER_A * (t_k - WATER_B).powf(WATER_C)
}

/// Upstream `valuePSatPrime`: `(C - 1) * A * (T - B)^(C - 1)`.
///
/// Faithful port of a likely upstream typo: the true derivative of
/// `A*(T-B)^C` with respect to `T` is `C*A*(T-B)^(C-1)`, not
/// `(C-1)*A*(T-B)^(C-1)`. Ported verbatim, not corrected (see module docs).
fn water_p_sat_prime(t_k: f64) -> f64 {
    (WATER_C - 1.0) * WATER_A * (t_k - WATER_B).powf(WATER_C - 1.0)
}

/// `T_sat = (p / A)^(1/C) + B`, the algebraic inverse of [`water_p_sat`].
fn water_t_sat(p_pa: f64) -> f64 {
    (p_pa / WATER_A).powf(1.0 / WATER_C) + WATER_B
}

// ---------------------------------------------------------------------------
// waterTRACE
// ---------------------------------------------------------------------------

/// Gas constant of water vapour, J/(kg K) — used by the branch-1 `T_sat` and
/// `p_sat_prime` Clausius-Clapeyron-style corrections.
const WATER_TRACE_RV: f64 = 461.4975;

fn water_trace_p_sat(t_k: f64) -> f64 {
    if t_k < 370.4251 {
        let ts = t_k.max(273.15);
        let (aap, bap, cap, dap) = (24821.0, 338.0, -5.3512, 20.387);
        aap * (ts / bap).powf(cap) * (dap * (ts - bap) / ts).exp()
    } else if t_k < 609.62462615967 {
        // Upstream quirk (see SaturationModel::WaterTrace docs): this branch
        // is not the algebraic inverse of water_trace_t_sat's matching branch.
        let (ab, bb, cb, db) = (117.8, 1e-5, 0.223, 255.2);
        bb * ((t_k - db) / ab).powf(1.0 / cb)
    } else if t_k < 647.3 {
        let (ac, bc, cc) = (7.2166948490268e11, -8529.6481905883, 1166669.3278328);
        ac * ((bc + cc / t_k) / t_k).exp()
    } else {
        let (ad, bd, cd) = (22.12e6, 7.6084087, 4924.9229);
        ad * (bd - cd / t_k).exp()
    }
}

fn water_trace_p_sat_prime(t_k: f64, p_pa: f64) -> f64 {
    if t_k < 370.4251 {
        let ts = t_k.max(273.15);
        let (aa, ba) = (3180619.59, 2470.2120);
        let hs = aa - ba * ts;
        hs * p_pa / WATER_TRACE_RV / (ts * ts)
    } else if t_k < 609.62462615967 {
        let (ab, bb) = (0.223, 255.2);
        p_pa / (ab * (t_k - bb))
    } else if t_k < 647.3 {
        let (ac, bc) = (-8529.6481905883, 2333338.6556656);
        -p_pa * (ac + bc / t_k) / (t_k * t_k)
    } else {
        let ad = 2.0304886238506e-4;
        p_pa / (ad * t_k * t_k)
    }
}

fn water_trace_t_sat(p_pa: f64) -> f64 {
    if p_pa < 90.56466e3 {
        let ps = p_pa.max(610.8);
        let (aa, ba, ca, da) = (-2263.0, 0.434, 100000.0, 6.064);
        let (aap, bap, cap, dap) = (24821.0, 338.0, -5.3512, 20.387);
        let (aah, bah) = (3180619.59, 2470.2120);
        let mut ts = aa / (ba * (ps / ca).ln() - da);
        for _ in 0..2 {
            let ts_approx = ts;
            let ps_approx =
                aap * (ts_approx / bap).powf(cap) * (dap * (ts_approx - bap) / ts_approx).exp();
            let hs_approx = aah - bah * ts_approx;
            ts = ts_approx / (1.0 - WATER_TRACE_RV * ts_approx / hs_approx * (ps / ps_approx).ln());
        }
        ts
    } else if p_pa < 13.969971285053e6 {
        let (ab, bb, cb, db) = (117.8, 1e-5, 0.223, 255.2);
        ab * (bb * p_pa).powf(cb) + db
    } else if p_pa < 22.12e6 {
        let (ac, bc, cc, dc) = (
            4264.8240952941,
            13666986.708428,
            1166669.3278328,
            27.304833093884,
        );
        (ac + (-bc + cc * p_pa.ln()).sqrt()) / (dc - p_pa.ln())
    } else {
        let (ad, bd) = (4924.9229, 24.520401);
        ad / (bd - p_pa.ln())
    }
}

// ---------------------------------------------------------------------------
// BrowningPotter
// ---------------------------------------------------------------------------

/// `ln(p_sat) = A + B/T + C*ln(T)`, plus `ln(1e6)` to convert the upstream
/// MPa-based fit constants to a Pa-based result.
fn browning_potter_ln_p_sat(t_k: f64) -> f64 {
    (11.9463 - 12633.37 / t_k - 0.4672 * t_k.ln()) + 1.0e6_f64.ln()
}

fn browning_potter_p_sat(t_k: f64) -> f64 {
    browning_potter_ln_p_sat(t_k).exp()
}

fn browning_potter_p_sat_prime(t_k: f64) -> f64 {
    browning_potter_p_sat(t_k) * (-0.4672 / t_k + 12633.37 / (t_k * t_k))
}

/// Upstream's documented second-order-polynomial approximate inverse of
/// [`browning_potter_ln_p_sat`] (not exact — `ln(p_sat)` is not analytically
/// invertible in `T`; see [`SaturationModel::BrowningPotter`] docs).
fn browning_potter_t_sat(p_pa: f64) -> f64 {
    923840.0 / (-11275.0 + (127125625.0 + 1847680.0 * (7.8270 - (p_pa / 1.0e6).ln())).sqrt())
}
