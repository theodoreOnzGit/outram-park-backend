// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// Derived from OpenFOAM (www.openfoam.com)
// Copyright (C) 2004-2023 OpenFOAM Foundation
// Copyright (C) 2016-2023 OpenCFD Ltd.
//
// This file is part of OUTRAM PARK.
//
// OUTRAM PARK is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the
// Free Software Foundation, either version 3 of the License, or (at your
// option) any later version.
//
// OUTRAM PARK is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
// General Public License for more details.
//
// You should have received a copy of the GNU General Public License along
// with OUTRAM PARK.  If not, see <https://www.gnu.org/licenses/>.

//! Specie reaction-rate classes of OpenFOAM-v2506 (ESI), gh:#1378 family 2:
//! `ArrheniusReactionRate`, `thirdBodyArrheniusReactionRate`,
//! `LandauTellerReactionRate`, `powerSeriesReactionRate`, `JanevReactionRate`,
//! `LangmuirHinshelwoodReactionRate`, `infiniteReactionRate`,
//! `thirdBodyEfficiencies`, the three fall-off functions (`Lindemann`, `Troe`,
//! `SRI`), `FallOffReactionRate` and `ChemicallyActivatedReactionRate`.
//!
//! Translated from `specie/reaction/reactionRate/**/*I.H` of ESI v2506 (read,
//! not recalled), keeping the operation order and the `mag(x) > VSMALL` guards
//! (a missing guard is the usual port defect). Concentrations `c` are in the
//! unit upstream uses (kmol/m^3) and rates in the reaction-order-dependent unit
//! of the pre-exponential factor `A`; neither is a fixed `uom` dimension, so
//! they stay `f64`, as upstream's `scalar`. Temperature and pressure are `uom`.
//!
//! No chemistry, species table or reaction-network layer is ported here (the
//! rate classes are self-contained, as upstream): `chemistryModel` and ODE
//! chemistry remain gh:#1378 deferred items.
//!
//! Differential against the real classes:
//! `verification_and_validation/upstream_thermo_gh1362/README.md`.

use crate::primitives::{SMALL, VSMALL};
use crate::thermophysics::imports::*;
use petir::mathf::RealMath;

fn mag(x: f64) -> f64 {
    x.abs()
}

/// `thirdBodyEfficiencies`: one efficiency per species, `M(c) = sum_i eff_i c_i`.
#[derive(Debug, Clone, PartialEq)]
pub struct ThirdBodyEfficiencies {
    efficiencies: Vec<f64>,
}

impl ThirdBodyEfficiencies {
    /// One efficiency per species, in species-table order (upstream `scalarList`).
    pub fn new(efficiencies: Vec<f64>) -> Self {
        Self { efficiencies }
    }

    /// The `defaultEfficiency` form: the same efficiency for `n_species` species.
    pub fn uniform(n_species: usize, efficiency: f64) -> Self {
        Self { efficiencies: vec![efficiency; n_species] }
    }

    /// Third-body concentration `M = sum_i eff_i c_i`, summed in species order.
    pub fn m(&self, c: &[f64]) -> f64 {
        let mut m = 0.0;
        for (i, e) in self.efficiencies.iter().enumerate() {
            m += e * c[i];
        }
        m
    }
}

/// `ArrheniusReactionRate`: `k = A T^beta exp(-Ta/T)`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ArrheniusReactionRate {
    a: f64,
    beta: f64,
    ta: f64,
}

impl ArrheniusReactionRate {
    /// `a` pre-exponential factor, `beta` temperature exponent `[-]`, `ta` activation temperature `[K]`.
    pub fn new(a: f64, beta: f64, ta: f64) -> Self {
        Self { a, beta, ta }
    }

    /// Rate `k(p, T, c)`; `p` and `c` are unused by this class, as upstream.
    pub fn rate(&self, _p: Pressure, t: ThermodynamicTemperature, _c: &[f64]) -> f64 {
        let t = t.get::<kelvin>();
        let mut ak = self.a;
        if mag(self.beta) > VSMALL {
            ak *= t.r_powf(self.beta);
        }
        if mag(self.ta) > VSMALL {
            ak *= (-self.ta / t).r_exp();
        }
        ak
    }
}

/// `thirdBodyArrheniusReactionRate`: `M(c) * k_Arrhenius`.
#[derive(Debug, Clone, PartialEq)]
pub struct ThirdBodyArrheniusReactionRate {
    arrhenius: ArrheniusReactionRate,
    efficiencies: ThirdBodyEfficiencies,
}

impl ThirdBodyArrheniusReactionRate {
    /// Arrhenius parameters and the third-body efficiencies.
    pub fn new(a: f64, beta: f64, ta: f64, efficiencies: ThirdBodyEfficiencies) -> Self {
        Self { arrhenius: ArrheniusReactionRate::new(a, beta, ta), efficiencies }
    }

    /// Rate `M(c) * k(T)`.
    pub fn rate(&self, p: Pressure, t: ThermodynamicTemperature, c: &[f64]) -> f64 {
        self.efficiencies.m(c) * self.arrhenius.rate(p, t, c)
    }
}

/// `LandauTellerReactionRate`: `k = A T^beta exp(-Ta/T + B/T^(1/3) + C/T^(2/3))`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LandauTellerReactionRate {
    a: f64,
    beta: f64,
    ta: f64,
    b: f64,
    c: f64,
}

impl LandauTellerReactionRate {
    /// `a`, `beta`, `ta` as Arrhenius; `b` and `c` the cube-root and `T^(-2/3)` coefficients `[K^(1/3)]`, `[K^(2/3)]`.
    pub fn new(a: f64, beta: f64, ta: f64, b: f64, c: f64) -> Self {
        Self { a, beta, ta, b, c }
    }

    /// Rate `k(T)`.
    pub fn rate(&self, _p: Pressure, t: ThermodynamicTemperature, _c: &[f64]) -> f64 {
        let t = t.get::<kelvin>();
        let mut lta = self.a;
        if mag(self.beta) > VSMALL {
            lta *= t.r_powf(self.beta);
        }
        let mut exp_arg = 0.0;
        if mag(self.ta) > VSMALL {
            exp_arg -= self.ta / t;
        }
        if mag(self.b) > VSMALL {
            exp_arg += self.b / t.r_cbrt();
        }
        if mag(self.c) > VSMALL {
            exp_arg += self.c / t.r_powf(2.0 / 3.0);
        }
        if mag(exp_arg) > VSMALL {
            lta *= exp_arg.r_exp();
        }
        lta
    }
}

/// `powerSeriesReactionRate`: `k = A T^beta exp(sum_n coeffs[n] / T^(n+1))`
/// (`Ta` is read and stored upstream but not used by `operator()`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PowerSeriesReactionRate {
    a: f64,
    beta: f64,
    ta: f64,
    coeffs: [f64; 4],
}

impl PowerSeriesReactionRate {
    /// `a`, `beta`, `ta` (stored, unused by the rate, as upstream) and the four `coeffs`.
    pub fn new(a: f64, beta: f64, ta: f64, coeffs: [f64; 4]) -> Self {
        Self { a, beta, ta, coeffs }
    }

    /// The stored `Ta`, which upstream's rate does not use.
    pub fn ta(&self) -> f64 {
        self.ta
    }

    /// Rate `k(T)`.
    pub fn rate(&self, _p: Pressure, t: ThermodynamicTemperature, _c: &[f64]) -> f64 {
        let t = t.get::<kelvin>();
        let mut lta = self.a;
        if mag(self.beta) > VSMALL {
            lta *= t.r_powf(self.beta);
        }
        let mut exp_arg = 0.0;
        for (n, coeff) in self.coeffs.iter().enumerate() {
            exp_arg += coeff / t.r_powf((n + 1) as f64);
        }
        lta *= exp_arg.r_exp();
        lta
    }
}

/// `JanevReactionRate`: `k = A T^beta exp(-Ta/T + sum_{n<9} b[n] (ln T)^n)`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct JanevReactionRate {
    a: f64,
    beta: f64,
    ta: f64,
    b: [f64; 9],
}

impl JanevReactionRate {
    /// `a`, `beta`, `ta` as Arrhenius and the nine `b` coefficients of the `ln T` series.
    pub fn new(a: f64, beta: f64, ta: f64, b: [f64; 9]) -> Self {
        Self { a, beta, ta, b }
    }

    /// Rate `k(T)`.
    pub fn rate(&self, _p: Pressure, t: ThermodynamicTemperature, _c: &[f64]) -> f64 {
        let t = t.get::<kelvin>();
        let mut lta = self.a;
        if mag(self.beta) > VSMALL {
            lta *= t.r_powf(self.beta);
        }
        let mut exp_arg = 0.0;
        if mag(self.ta) > VSMALL {
            exp_arg -= self.ta / t;
        }
        let ln_t = t.r_ln();
        for (n, b) in self.b.iter().enumerate() {
            exp_arg += b * ln_t.r_powf(n as f64);
        }
        lta *= exp_arg.r_exp();
        lta
    }
}

/// `LangmuirHinshelwoodReactionRate` with the three species indices upstream
/// looks up by name (`CO`, `C3H6`, `NO`) given directly.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LangmuirHinshelwoodReactionRate {
    a: [f64; 5],
    ta: [f64; 5],
    co: usize,
    c3h6: usize,
    no: usize,
}

impl LangmuirHinshelwoodReactionRate {
    /// `a[i]`, `ta[i]` are the five `(A, Ta)` pairs of the `coeffs` entry; `co`, `c3h6`, `no`
    /// the indices of those species in the concentration array.
    pub fn new(a: [f64; 5], ta: [f64; 5], co: usize, c3h6: usize, no: usize) -> Self {
        Self { a, ta, co, c3h6, no }
    }

    /// Rate `k(T, c)`.
    pub fn rate(&self, _p: Pressure, t: ThermodynamicTemperature, c: &[f64]) -> f64 {
        let t = t.get::<kelvin>();
        let (a, ta) = (&self.a, &self.ta);
        let sqr = |x: f64| x * x;
        a[0] * (-ta[0] / t).r_exp()
            / (t * sqr(
                1.0 + a[1] * (-ta[1] / t).r_exp() * c[self.co]
                    + a[2] * (-ta[2] / t).r_exp() * c[self.c3h6],
            ) * (1.0 + a[3] * (-ta[3] / t).r_exp() * sqr(c[self.co]) * sqr(c[self.c3h6]))
                * (1.0 + a[4] * (-ta[4] / t).r_exp() * c[self.no].r_powf(0.7)))
    }
}

/// `infiniteReactionRate`: `k = 1`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct InfiniteReactionRate;

impl InfiniteReactionRate {
    /// Rate, always `1`.
    pub fn rate(&self, _p: Pressure, _t: ThermodynamicTemperature, _c: &[f64]) -> f64 {
        1.0
    }
}

/// Fall-off blending functions `F(T, Pr)`: `Lindemann`, `Troe`, `SRI`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FallOffFunction {
    /// `F = 1`.
    Lindemann,
    /// Troe form with `alpha`, `Tsss`, `Ts`, `Tss`.
    Troe {
        /// `alpha`.
        alpha: f64,
        /// `Tsss` `[K]`.
        tsss: f64,
        /// `Ts` `[K]`.
        ts: f64,
        /// `Tss` `[K]`.
        tss: f64,
    },
    /// SRI form with `a`, `b`, `c`, `d`, `e`.
    Sri {
        /// `a`.
        a: f64,
        /// `b` `[K]`.
        b: f64,
        /// `c` `[K]`.
        c: f64,
        /// `d`.
        d: f64,
        /// `e`.
        e: f64,
    },
}

impl FallOffFunction {
    /// `F(T, Pr)`.
    pub fn value(&self, t: ThermodynamicTemperature, pr: f64) -> f64 {
        let t = t.get::<kelvin>();
        match *self {
            FallOffFunction::Lindemann => 1.0,
            FallOffFunction::Troe { alpha, tsss, ts, tss } => {
                let arg = (1.0 - alpha) * (-t / tsss).r_exp()
                    + alpha * (-t / ts).r_exp()
                    + (-tss / t).r_exp();
                let log_fcent = arg.max(SMALL).r_log10();
                let c = -0.4 - 0.67 * log_fcent;
                let d = 0.14;
                let n = 0.75 - 1.27 * log_fcent;
                let log_pr = pr.max(SMALL).r_log10();
                let x = (log_pr + c) / (n - d * (log_pr + c));
                10.0f64.r_powf(log_fcent / (1.0 + x * x))
            }
            FallOffFunction::Sri { a, b, c, d, e } => {
                let l = pr.max(SMALL).r_log10();
                let x = 1.0 / (1.0 + l * l);
                d * (a * (-b / t).r_exp() + (-t / c).r_exp()).r_powf(x) * t.r_powf(e)
            }
        }
    }
}

/// `FallOffReactionRate<ArrheniusReactionRate, F>`:
/// `k = kInf (Pr/(1+Pr)) F(T, Pr)`, `Pr = k0 M(c) / kInf`.
#[derive(Debug, Clone, PartialEq)]
pub struct FallOffReactionRate {
    k0: ArrheniusReactionRate,
    k_inf: ArrheniusReactionRate,
    f: FallOffFunction,
    efficiencies: ThirdBodyEfficiencies,
}

impl FallOffReactionRate {
    /// Low- and high-pressure limit rates, blending function and third-body efficiencies.
    pub fn new(
        k0: ArrheniusReactionRate,
        k_inf: ArrheniusReactionRate,
        f: FallOffFunction,
        efficiencies: ThirdBodyEfficiencies,
    ) -> Self {
        Self { k0, k_inf, f, efficiencies }
    }

    /// Rate `k(p, T, c)`.
    pub fn rate(&self, p: Pressure, t: ThermodynamicTemperature, c: &[f64]) -> f64 {
        let k0 = self.k0.rate(p, t, c);
        let k_inf = self.k_inf.rate(p, t, c);
        let pr = k0 * self.efficiencies.m(c) / k_inf;
        k_inf * (pr / (1.0 + pr)) * self.f.value(t, pr)
    }
}

/// `ChemicallyActivatedReactionRate<ArrheniusReactionRate, F>`:
/// `k = k0 (1/(1+Pr)) F(T, Pr)`, `Pr = k0 M(c) / kInf`.
#[derive(Debug, Clone, PartialEq)]
pub struct ChemicallyActivatedReactionRate {
    k0: ArrheniusReactionRate,
    k_inf: ArrheniusReactionRate,
    f: FallOffFunction,
    efficiencies: ThirdBodyEfficiencies,
}

impl ChemicallyActivatedReactionRate {
    /// Low- and high-pressure limit rates, activation function and third-body efficiencies.
    pub fn new(
        k0: ArrheniusReactionRate,
        k_inf: ArrheniusReactionRate,
        f: FallOffFunction,
        efficiencies: ThirdBodyEfficiencies,
    ) -> Self {
        Self { k0, k_inf, f, efficiencies }
    }

    /// Rate `k(p, T, c)`.
    pub fn rate(&self, p: Pressure, t: ThermodynamicTemperature, c: &[f64]) -> f64 {
        let k0 = self.k0.rate(p, t, c);
        let k_inf = self.k_inf.rate(p, t, c);
        let pr = k0 * self.efficiencies.m(c) / k_inf;
        k0 * (1.0 / (1.0 + pr)) * self.f.value(t, pr)
    }
}

/// Closed set of reaction-rate models (enum dispatch, no trait objects).
#[derive(Debug, Clone, PartialEq)]
pub enum ReactionRate {
    /// [`ArrheniusReactionRate`].
    Arrhenius(ArrheniusReactionRate),
    /// [`ThirdBodyArrheniusReactionRate`].
    ThirdBodyArrhenius(ThirdBodyArrheniusReactionRate),
    /// [`LandauTellerReactionRate`].
    LandauTeller(LandauTellerReactionRate),
    /// [`PowerSeriesReactionRate`].
    PowerSeries(PowerSeriesReactionRate),
    /// [`JanevReactionRate`].
    Janev(JanevReactionRate),
    /// [`LangmuirHinshelwoodReactionRate`].
    LangmuirHinshelwood(LangmuirHinshelwoodReactionRate),
    /// [`InfiniteReactionRate`].
    Infinite(InfiniteReactionRate),
    /// [`FallOffReactionRate`].
    FallOff(FallOffReactionRate),
    /// [`ChemicallyActivatedReactionRate`].
    ChemicallyActivated(ChemicallyActivatedReactionRate),
}

impl ReactionRate {
    /// Rate `k(p, T, c)` of the wrapped model.
    pub fn rate(&self, p: Pressure, t: ThermodynamicTemperature, c: &[f64]) -> f64 {
        match self {
            ReactionRate::Arrhenius(r) => r.rate(p, t, c),
            ReactionRate::ThirdBodyArrhenius(r) => r.rate(p, t, c),
            ReactionRate::LandauTeller(r) => r.rate(p, t, c),
            ReactionRate::PowerSeries(r) => r.rate(p, t, c),
            ReactionRate::Janev(r) => r.rate(p, t, c),
            ReactionRate::LangmuirHinshelwood(r) => r.rate(p, t, c),
            ReactionRate::Infinite(r) => r.rate(p, t, c),
            ReactionRate::FallOff(r) => r.rate(p, t, c),
            ReactionRate::ChemicallyActivated(r) => r.rate(p, t, c),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p() -> Pressure {
        Pressure::new::<pascal>(1.0e5)
    }
    fn t(v: f64) -> ThermodynamicTemperature {
        ThermodynamicTemperature::new::<kelvin>(v)
    }

    #[test]
    fn arrhenius_guards_and_closed_form() {
        let c = [1.0, 2.0];
        assert_eq!(ArrheniusReactionRate::new(3.0, 0.0, 0.0).rate(p(), t(500.0), &c), 3.0);
        let k = ArrheniusReactionRate::new(3.0, 0.5, 1000.0).rate(p(), t(500.0), &c);
        assert!((k - 3.0 * 500f64.sqrt() * (-2.0f64).exp()).abs() / k < 1e-14);
    }

    #[test]
    fn third_body_and_fall_off_limits() {
        let eff = ThirdBodyEfficiencies::new(vec![1.0, 2.5, 0.4]);
        let c = [0.01, 0.02, 0.005];
        assert_eq!(eff.m(&c), 0.01 + 2.5 * 0.02 + 0.4 * 0.005);
        assert_eq!(ThirdBodyEfficiencies::uniform(3, 1.0).m(&c), 0.01 + 0.02 + 0.005);
        let tb = ThirdBodyArrheniusReactionRate::new(2.0, 0.0, 0.0, eff.clone());
        assert_eq!(tb.rate(p(), t(300.0), &c), eff.m(&c) * 2.0);
        let k0 = ArrheniusReactionRate::new(1.0e3, 0.0, 0.0);
        let ki = ArrheniusReactionRate::new(1.0e6, 0.0, 0.0);
        // Lindemann: low M -> k ~ k0 M, high M -> kInf
        let fo = FallOffReactionRate::new(k0, ki, FallOffFunction::Lindemann, eff.clone());
        let low = fo.rate(p(), t(300.0), &[1e-9, 0.0, 0.0]);
        assert!((low - 1.0e3 * 1e-9).abs() / low < 1e-5);
        let high = fo.rate(p(), t(300.0), &[1e9, 0.0, 0.0]);
        assert!((high - 1.0e6).abs() / high < 1e-5);
        // chemically activated: k0 at low M, 0 at high M
        let ca = ChemicallyActivatedReactionRate::new(k0, ki, FallOffFunction::Lindemann, eff);
        assert!((ca.rate(p(), t(300.0), &[1e-9, 0.0, 0.0]) - 1.0e3).abs() < 1e-3);
        assert!(ca.rate(p(), t(300.0), &[1e9, 0.0, 0.0]) < 1e-3);
    }

    #[test]
    fn troe_and_sri_are_bounded_and_dispatch_matches() {
        let troe = FallOffFunction::Troe { alpha: 0.5, tsss: 1e-30, ts: 1e30, tss: 1e100 };
        // Fcent = 0.5 exactly here, F(Pr = 1) = Fcent for Troe at logPr = -c: F <= 1 and > 0
        for pr in [1e-6, 1e-2, 1.0, 1e2, 1e6] {
            let f = troe.value(t(1000.0), pr);
            assert!(f > 0.0 && f <= 1.0, "{f}");
        }
        let sri = FallOffFunction::Sri { a: 0.5, b: 300.0, c: 1000.0, d: 1.2, e: 0.1 };
        let f = sri.value(t(1000.0), 1.0);
        assert!((f - 1.2 * (0.5 * (-0.3f64).exp() + (-1.0f64).exp()) * 1000f64.powf(0.1)).abs() / f < 1e-12);
        let a = ArrheniusReactionRate::new(1.0, 1.0, 0.0);
        let e = ReactionRate::Arrhenius(a);
        assert_eq!(e.rate(p(), t(400.0), &[]), a.rate(p(), t(400.0), &[]));
    }

    #[test]
    fn series_rates_reduce_to_arrhenius_and_dispatch() {
        let c = [1.0e-2, 1.0e-3, 1.0e-4];
        let lt = LandauTellerReactionRate::new(2.0, 0.0, 500.0, 0.0, 0.0);
        let ar = ArrheniusReactionRate::new(2.0, 0.0, 500.0);
        assert_eq!(lt.rate(p(), t(600.0), &c), ar.rate(p(), t(600.0), &c));
        let lt2 = LandauTellerReactionRate::new(2.0, 0.0, 0.0, 3.0, 4.0);
        let want = 2.0 * (3.0 / 600f64.cbrt() + 4.0 / 600f64.powf(2.0 / 3.0)).exp();
        assert!((lt2.rate(p(), t(600.0), &c) - want).abs() / want < 1e-13);
        let ps = PowerSeriesReactionRate::new(2.0, 0.0, 99.0, [100.0, 0.0, 0.0, 0.0]);
        assert_eq!(ps.ta(), 99.0);
        let want = 2.0 * (100.0 / 600.0f64).exp();
        assert!((ps.rate(p(), t(600.0), &c) - want).abs() / want < 1e-13);
        // Janev with only b[0] = 0 and Ta = 0 is A T^beta
        let jv = JanevReactionRate::new(2.0, 1.0, 0.0, [0.0; 9]);
        assert!((jv.rate(p(), t(600.0), &c) - 1200.0).abs() < 1e-9);
        // b[1] = 1: exp(ln T) = T
        let mut b = [0.0; 9];
        b[1] = 1.0;
        let jv = JanevReactionRate::new(1.0, 0.0, 0.0, b);
        assert!((jv.rate(p(), t(600.0), &c) - 600.0).abs() < 1e-9);
        assert_eq!(InfiniteReactionRate.rate(p(), t(300.0), &c), 1.0);
        let lh = LangmuirHinshelwoodReactionRate::new([1.0; 5], [0.0; 5], 0, 1, 2);
        let k = lh.rate(p(), t(500.0), &c);
        assert!(k > 0.0 && k.is_finite());
        for r in [
            ReactionRate::LandauTeller(lt),
            ReactionRate::PowerSeries(ps),
            ReactionRate::Janev(jv),
            ReactionRate::LangmuirHinshelwood(lh),
            ReactionRate::Infinite(InfiniteReactionRate),
            ReactionRate::ThirdBodyArrhenius(ThirdBodyArrheniusReactionRate::new(
                1.0,
                0.0,
                0.0,
                ThirdBodyEfficiencies::uniform(3, 1.0),
            )),
            ReactionRate::FallOff(FallOffReactionRate::new(
                ar,
                ar,
                FallOffFunction::Lindemann,
                ThirdBodyEfficiencies::uniform(3, 1.0),
            )),
            ReactionRate::ChemicallyActivated(ChemicallyActivatedReactionRate::new(
                ar,
                ar,
                FallOffFunction::Lindemann,
                ThirdBodyEfficiencies::uniform(3, 1.0),
            )),
        ] {
            assert!(r.rate(p(), t(500.0), &c).is_finite());
        }
    }
}
