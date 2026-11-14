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

//! `kOmegaSSTLM` (Langtry-Menter gamma-ReTheta transition on SST), ported from OpenFOAM-v2506
//! (ESI) `kOmegaSSTLM.C` on `kOmegaSSTBase.C` (gh:#1544, under #1377 and #1376).
//!
//! The SST terms the LM model does not change (`y`, `S2`, `CDkOmega`, the SST `F1`, `F23`,
//! `nut`, `GbyNu0`, `G`, `GbyNu`, the SST `Pk`) are those of [`crate::k_omega_sst::KOmegaSST`]
//! (ESI arithmetic), REUSED through [`KOmegaSstLm::sst`]. LM overrides three SST hooks:
//! `F1 = max(F1_SST, F3)` with `F3 = exp(-(y sqrt(k)/(120 nu))^8)`, `Pk = gammaIntEff Pk_SST`
//! (UNCLAMPED `gammaIntEff`), and `epsilonByk = clamp(gammaIntEff, 0.1, 1) betaStar omega`
//! (CLAMPED); and adds the `ReThetat` and `gammaInt` transport equations with the helper
//! functions `Fthetat`, `ReThetac`, `Flength`, `ReThetat0` (a fixed-point iteration on the
//! pressure-gradient parameter `lambda`), `Fonset`, `Fturb`, `Freattach`, `gammaSep`.
//!
//! **Status: AI draft, terms only.** [`KOmegaSstLm::source_terms`] is compared per cell against
//! the real ESI model on a frozen duct (README slice 8 of the V&V record). There is no
//! `correct()` (the two new transport solves, `bound`, and updating `gammaIntEff` between steps),
//! and no [`crate::traits::TurbulenceModel`] impl. Not ported: `fvOptions`, `omegaInf`/`kInf`
//! sources, `Qsas`, SST `F3` and decay control (all off in the v2506 defaults).

use crate::k_omega_sst::KOmegaSST;
use crate::source_terms::{self, umax, umin, FrozenState, SourceForm, SourceTerms};
use outram_foam_basic_lib::prelude::FvMesh;
use petir::mathf::RealMath;
use std::sync::Arc;

/// Upstream `SMALL` for double precision.
const SMALL: f64 = 1e-15;

/// Upstream `pow4(s) = sqr(sqr(s))`.
fn pow4(s: f64) -> f64 {
    let q = s * s;
    q * q
}

/// Upstream `pow3(s) = s*sqr(s)`.
fn pow3(s: f64) -> f64 {
    s * (s * s)
}

/// Upstream `clamp(v, scalarMinMax(lo, hi))` for finite values.
fn clamp(v: f64, lo: f64, hi: f64) -> f64 {
    umin(umax(v, lo), hi)
}

/// `kOmegaSSTLM` on a frozen field (terms only).
pub struct KOmegaSstLm {
    /// The k-omega SST model whose ESI-form terms this model reuses; set `k`, `omega`, `nu`,
    /// `u` on it.
    pub sst: KOmegaSST,
    /// `ReThetat` per cell (the transported momentum-thickness Reynolds number).
    pub re_thetat: Vec<f64>,
    /// `gammaInt` per cell (the transported intermittency).
    pub gamma_int: Vec<f64>,
    /// `gammaIntEff` per cell: the value `Pk` and `epsilonByk` use (upstream carries it from the
    /// previous `correct()`; zero at construction).
    pub gamma_int_eff: Vec<f64>,
    /// `ca1`, upstream 2.
    pub ca1: f64,
    /// `ca2`, upstream 0.06.
    pub ca2: f64,
    /// `ce1`, upstream 1.
    pub ce1: f64,
    /// `ce2`, upstream 50.
    pub ce2: f64,
    /// `cThetat`, upstream 0.03.
    pub c_thetat: f64,
    /// `sigmaThetat`, upstream 2.
    pub sigma_thetat: f64,
    /// Convergence tolerance of the `lambda` iteration, upstream 1e-6.
    pub lambda_err: f64,
}

impl KOmegaSstLm {
    /// Upstream defaults; `ReThetat` and `gammaInt` start at zero, `gammaIntEff` at zero as the
    /// upstream constructor leaves it.
    pub fn new(mesh: Arc<FvMesh>) -> Self {
        let n = mesh.n_cells;
        Self {
            sst: KOmegaSST::new(mesh),
            re_thetat: vec![0.0; n],
            gamma_int: vec![0.0; n],
            gamma_int_eff: vec![0.0; n],
            ca1: 2.0,
            ca2: 0.06,
            ce1: 1.0,
            ce2: 50.0,
            c_thetat: 0.03,
            sigma_thetat: 2.0,
            lambda_err: 1e-6,
        }
    }

    /// Upstream `ReThetac()` for one `ReThetat`.
    pub fn re_thetac(re_thetat: f64) -> f64 {
        if re_thetat <= 1870.0 {
            re_thetat - 396.035e-2 + 120.656e-4 * re_thetat - 868.230e-6 * (re_thetat * re_thetat)
                + 696.506e-9 * pow3(re_thetat)
                - 174.105e-12 * pow4(re_thetat)
        } else {
            re_thetat - 593.11 - 0.482 * (re_thetat - 1870.0)
        }
    }

    /// Upstream `Flength()` for one cell (`y`, `omega`, `nu` of that cell).
    pub fn f_length(re_thetat: f64, y: f64, omega: f64, nu: f64) -> f64 {
        let f = if re_thetat < 400.0 {
            398.189e-1 - 119.270e-4 * re_thetat - 132.567e-6 * (re_thetat * re_thetat)
        } else if re_thetat < 596.0 {
            263.404 - 123.939e-2 * re_thetat + 194.548e-5 * (re_thetat * re_thetat)
                - 101.695e-8 * pow3(re_thetat)
        } else if re_thetat < 1200.0 {
            0.5 - 3e-4 * (re_thetat - 596.0)
        } else {
            0.3188
        };
        let a = ((y * y) * omega) / (200.0 * nu);
        let f_sublayer = (-(a * a)).r_exp();
        f * (1.0 - f_sublayer) + 40.0 * f_sublayer
    }

    /// Upstream `ReThetat0()` for one cell: the transition-onset `ReThetat` from the freestream
    /// turbulence intensity `Tu` and the pressure-gradient parameter `lambda`, found by the
    /// upstream fixed-point iteration. Returns `(ReThetat0, iterations)`.
    ///
    /// Like upstream the loop has NO iteration cap (`maxLambdaIter` only warns there); a
    /// guard of 1e6 turns a non-converging cell into a panic instead of a silent hang.
    pub fn re_thetat0(&self, k: f64, us: f64, d_us_ds: f64, nu: f64) -> (f64, u32) {
        let tu = umax(100.0 * ((2.0 / 3.0) * k).sqrt() / us, 0.027);
        let mut lambda = 0.0_f64;
        let mut thetat;
        let mut iter = 0_u32;
        loop {
            let lambda0 = lambda;
            let neg_poly = -12.986 * lambda - 123.66 * (lambda * lambda) - 405.689 * pow3(lambda);
            if tu <= 1.3 {
                let f_lambda = if d_us_ds <= 0.0 {
                    1.0 - neg_poly * (-((tu / 1.5).r_powf(1.5))).r_exp()
                } else {
                    1.0 + 0.275 * (1.0 - (-35.0 * lambda).r_exp()) * (-tu / 0.5).r_exp()
                };
                thetat = (1173.51 - 589.428 * tu + 0.2196 / (tu * tu)) * f_lambda * nu / us;
            } else {
                let f_lambda = if d_us_ds <= 0.0 {
                    1.0 - neg_poly * (-((tu / 1.5).r_powf(1.5))).r_exp()
                } else {
                    1.0 + 0.275 * (1.0 - (-35.0 * lambda).r_exp()) * (-2.0 * tu).r_exp()
                };
                thetat = 331.50 * (tu - 0.5658).r_powf(-0.671) * f_lambda * nu / us;
            }
            lambda = (thetat * thetat) / nu * d_us_ds;
            lambda = clamp(lambda, -0.1, 0.1);
            let err = (lambda - lambda0).abs();
            iter += 1;
            assert!(iter < 1_000_000, "ReThetat0 lambda iteration does not converge");
            if err <= self.lambda_err {
                break;
            }
        }
        (umax(thetat * us / nu, 20.0), iter)
    }

    /// Per-cell terms on the CURRENT fields (`sst.k`, `sst.omega`, `sst.nu`, `sst.u`,
    /// `re_thetat`, `gamma_int`, `gamma_int_eff`), named as the driver's `T_*` keys:
    /// the SST set `S2`, `CDkOmega`, `F1sst`, `F3`, `F1`, `F23`, `GbyNu0`, `G`, `gamma`, `beta`,
    /// `GbyNu`, `PkSst`, `Pk`, `omega_E`, `omega_SuSp1`, `omega_Sp`, `omega_SuSp2`, `k_E`,
    /// `k_SuSp`, `k_Sp`; and `Omega`, `S`, `Us`, `dUsds`, `delta`, `Fwake`, `Fthetat`,
    /// `ReThetac`, `Flength`, `ReThetat0`, `lambdaIters`, `Pthetat`, `ReThetat_E`, `Rev`, `RT`,
    /// `Fonset`, `Pgamma`, `Fturb`, `Egamma`, `Freattach`, `gammaSep`, `gammaIntEff` (the value
    /// `correct()` would store for the next step). `state` needs `grad_u`, `div_u`, `grad_k`,
    /// `grad_omega`; `wall_distance` overrides `y`.
    pub fn source_terms(&self, state: &FrozenState) -> SourceTerms {
        let base = self.sst.source_terms(SourceForm::EsiV2506, state);
        let n = self.sst.mesh.n_cells;
        let k = self.sst.k.internal.as_slice();
        let w = self.sst.omega.internal.as_slice();
        let nu = self.sst.nu.internal.as_slice();
        let y = base.get("y").unwrap();
        let cd = base.get("CDkOmega").unwrap();
        let f1sst = base.get("F1").unwrap();
        let gbn = base.get("GbyNu").unwrap();
        let pk_sst = base.get("Pk").unwrap();

        let names = [
            "F3", "F1", "gamma", "beta", "Pk", "omega_E", "omega_SuSp1", "omega_Sp",
            "omega_SuSp2", "k_Sp", "Omega", "S", "Us", "dUsds", "delta", "Fwake", "Fthetat",
            "ReThetac", "Flength", "ReThetat0", "lambdaIters", "Pthetat", "ReThetat_E", "Rev",
            "RT", "Fonset", "Pgamma", "Fturb", "Egamma", "Freattach", "gammaSep", "gammaIntEff",
        ];
        let mut t: Vec<Vec<f64>> = (0..names.len()).map(|_| Vec::with_capacity(n)).collect();
        for c in 0..n {
            let gr = &state.grad_u[c];
            let u = self.sst.u.internal[c];

            // ---- SST hooks overridden by LM ----
            let ry = (y[c] * k[c].sqrt()) / nu[c];
            let r120 = ry / 120.0;
            let f3 = (-(r120.r_powf(8.0))).r_exp();
            let f1 = umax(f1sst[c], f3);
            let gamma = f1 * (5.0 / 9.0 - 0.44) + 0.44;
            let beta = f1 * (0.075 - 0.0828) + 0.0828;
            let pk = self.gamma_int_eff[c] * pk_sst[c];
            let eps_by_k = clamp(self.gamma_int_eff[c], 0.1, 1.0) * (0.09 * w[c]);
            let omega_e = gamma * gbn[c];
            let su_sp1 = ((2.0 / 3.0) * gamma) * state.div_u[c];
            let omega_sp = beta * w[c];
            let su_sp2 = ((f1 - 1.0) * cd[c]) / w[c];

            // ---- velocity-gradient fields ----
            let omega_big = (2.0 * source_terms::mag_sqr_skew(gr)).sqrt();
            let s = source_terms::two_mag_sqr_symm(gr).sqrt();
            let mag_u = (u.x * u.x + u.y * u.y + u.z * u.z).sqrt();
            let us = umax(mag_u, SMALL);
            let vg_x = u.x * gr.xx + u.y * gr.yx + u.z * gr.zx;
            let vg_y = u.x * gr.xy + u.y * gr.yy + u.z * gr.zy;
            let vg_z = u.x * gr.xz + u.y * gr.yz + u.z * gr.zz;
            let d_us_ds = (u.x * vg_x + u.y * vg_y + u.z * vg_z) / (us * us);

            // ---- Fthetat ----
            let re_t = self.re_thetat[c];
            let gi = self.gamma_int[c];
            let delta = umax(
                ((((375.0 * omega_big) * nu[c]) * re_t) * y[c]) / (us * us),
                SMALL,
            );
            let re_omega = ((y[c] * y[c]) * w[c]) / nu[c];
            let q = re_omega / 1e5;
            let fwake = (-(q * q)).r_exp();
            let a = 1.0 / self.ce2;
            let r = (gi - a) / (1.0 - a);
            let f_thetat = umin(
                umax(fwake * (-pow4(y[c] / delta)).r_exp(), 1.0 - r * r),
                1.0,
            );

            // ---- ReThetat equation ----
            let re_thetac = Self::re_thetac(re_t);
            let flength = Self::f_length(re_t, y[c], w[c], nu[c]);
            let (re_thetat0, iters) = self.re_thetat0(k[c], us, d_us_ds, nu[c]);
            let tt = (500.0 * nu[c]) / (us * us);
            let p_thetat = (self.c_thetat / tt) * (1.0 - f_thetat);
            let re_thetat_e = p_thetat * re_thetat0;

            // ---- gammaInt equation ----
            let rev = ((y[c] * y[c]) * s) / nu[c];
            let rt = k[c] / (nu[c] * w[c]);
            let fonset1 = rev / (2.193 * re_thetac);
            let fonset2 = umin(umax(fonset1, pow4(fonset1)), 2.0);
            let fonset3 = umax(1.0 - pow3(rt / 2.5), 0.0);
            let fonset = umax(fonset2 - fonset3, 0.0);
            let pgamma = ((self.ca1 * flength) * s) * (gi * fonset).sqrt();
            let fturb = (-pow4(0.25 * rt)).r_exp();
            let egamma = ((self.ca2 * omega_big) * fturb) * gi;
            let freattach = (-pow4(rt / 20.0)).r_exp();
            let gamma_sep = umin(
                (2.0 * umax(rev / (3.235 * re_thetac) - 1.0, 0.0)) * freattach,
                2.0,
            ) * f_thetat;
            let g_eff = umax(gi, gamma_sep);

            let row = [
                f3, f1, gamma, beta, pk, omega_e, su_sp1, omega_sp, su_sp2, eps_by_k, omega_big, s,
                us, d_us_ds, delta, fwake, f_thetat, re_thetac, flength, re_thetat0, iters as f64,
                p_thetat, re_thetat_e, rev, rt, fonset, pgamma, fturb, egamma, freattach,
                gamma_sep, g_eff,
            ];
            for (col, v) in t.iter_mut().zip(row) {
                col.push(v);
            }
        }
        let mut out = SourceTerms::default();
        for (name, col) in names.iter().zip(t) {
            out.push(name, col);
        }
        // terms that pass through unchanged from the SST evaluation
        for name in ["S2", "CDkOmega", "F23", "GbyNu0", "G", "GbyNu", "nut"] {
            out.push(name, base.get(name).unwrap().to_vec());
        }
        out.push("F1sst", f1sst.to_vec());
        out.push("PkSst", pk_sst.to_vec());
        let pk_lm = out.get("Pk").unwrap().to_vec();
        out.push("k_E", pk_lm);
        out.push("k_SuSp", base.get("k_SuSp").unwrap().to_vec());
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `ReThetac` second branch is exact; the first stays near `ReThetat`.
    #[test]
    fn re_thetac_branches() {
        assert_eq!(KOmegaSstLm::re_thetac(2000.0), 2000.0 - 593.11 - 0.482 * (2000.0 - 1870.0));
        let r = KOmegaSstLm::re_thetac(1000.0);
        assert!(r > 900.0 && r < 1100.0, "{r}");
    }

    /// `Flength` takes the sub-layer value 40 where `y^2 omega/(200 nu)` is small and the
    /// correlation value (here the constant 0.3188) where it is large.
    #[test]
    fn f_length_limits() {
        assert_eq!(KOmegaSstLm::f_length(1500.0, 1.0, 1e9, 1e-5), 0.3188);
        let near = KOmegaSstLm::f_length(1500.0, 1e-9, 1.0, 1e-5);
        assert!((near - 40.0).abs() < 1e-9, "{near}");
    }
}
