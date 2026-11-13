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

//! Spalart-Allmaras DES family (DES, DDES, IDDES), ported from OpenFOAM-v2506 (ESI)
//! `SpalartAllmarasDES.C`, `SpalartAllmarasDDES.C`, `SpalartAllmarasIDDES.C` on
//! `SpalartAllmarasBase.C` (gh:#1545, under #1377 and #1376).
//!
//! One struct, one enum ([`SaDesVariant`]) in place of upstream's class chain
//! (`IDDES : DDES : DES : SpalartAllmarasBase<DESModel<..>>`), because this crate
//! dispatches with enums, never `dyn`.
//!
//! The model differs from plain Spalart-Allmaras (`crate::spalart_allmaras`) only in the
//! length scale `dTilda` that replaces the wall distance in `Stilda`, `r`, `fw` and the
//! destruction term:
//!
//! * DES: `dTilda = min(psi CDES delta, y)`, `psi` the low-Re correction (`lowReCorrection`).
//! * DDES: `dTilda = max(y - fd max(y - lLES, 0), SMALL)`, `fd = 1 - tanh((Cd1 r)^Cd2)`,
//!   `r = min(nuEff / (max(|gradU|, SMALL) (kappa y)^2), 10)`.
//! * IDDES: Shur et al. (2008) with `fe` (default), Gritskevich et al. (2011) without.
//!
//! Defaults are upstream's: `CDES 0.65`, `fwStar 0.424`, `lowReCorrection true`, `Cd1 8`,
//! `Cd2 3`, `Cdt1 8`, `Cdt2 3`, `Cl 3.55`, `Ct 1.63`, `fe true`, `ft2` off, and the SA base
//! coefficients with `sigmaNut 0.66666`.
//!
//! **Status: AI draft, terms only.** [`SpalartAllmarasDes::source_terms`] is compared per
//! cell against the real ESI model on a frozen duct
//! (`verification_and_validation/upstream_turb_gh1362/README.md`, slice 4). There is NO
//! `correct()` (no transport solve) and no [`crate::traits::TurbulenceModel`] impl yet.
//! Not ported: `useSigma true` (the `Ssigma` invariants branch), `shielding ZDES2020`
//! (needs `wallDist::n()` and `fvc::curl`), `ft2 true`, and the `IDDESDelta` / `maxDeltaxyz`
//! classes themselves: the IDDES width `delta` and `hmax` are INPUTS (`delta`, `h_max`).

use crate::source_terms::{self, FrozenState, SourceTerms};
use crate::spalart_allmaras::compute_wall_distance;
use outram_foam_basic_lib::prelude::{FvMesh, VolScalarField};
use petir::mathf::RealMath;
use std::sync::Arc;

/// upstream double `SMALL`.
const U_SMALL: f64 = 1e-15;

/// Which member of the Spalart-Allmaras DES family.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaDesVariant {
    /// `SpalartAllmarasDES` (Spalart et al. 1997, with the low-Re `psi`).
    Des,
    /// `SpalartAllmarasDDES` (Spalart et al. 2006, standard shielding).
    Ddes,
    /// `SpalartAllmarasIDDES` (Shur et al. 2008).
    Iddes,
}

/// Upstream coefficients of the family, defaults as in the `.C` files.
#[derive(Debug, Clone, Copy)]
pub struct SaDesCoeffs {
    /// `sigmaNut`.
    pub sigma_nut: f64,
    /// `kappa`.
    pub kappa: f64,
    /// `Cb1`.
    pub cb1: f64,
    /// `Cb2`.
    pub cb2: f64,
    /// `Cw2`.
    pub cw2: f64,
    /// `Cw3`.
    pub cw3: f64,
    /// `Cv1`.
    pub cv1: f64,
    /// `Cs`.
    pub cs: f64,
    /// `CDES`.
    pub cdes: f64,
    /// `fwStar`.
    pub fw_star: f64,
    /// `lowReCorrection`.
    pub low_re_correction: bool,
    /// DDES `Cd1`.
    pub cd1: f64,
    /// DDES `Cd2`.
    pub cd2: f64,
    /// IDDES `Cdt1`.
    pub cdt1: f64,
    /// IDDES `Cdt2`.
    pub cdt2: f64,
    /// IDDES `Cl`.
    pub cl: f64,
    /// IDDES `Ct`.
    pub ct: f64,
    /// IDDES `fe` (Shur et al. 2008 elevating function).
    pub fe: bool,
}

impl Default for SaDesCoeffs {
    fn default() -> Self {
        Self {
            sigma_nut: 0.66666,
            kappa: 0.41,
            cb1: 0.1355,
            cb2: 0.622,
            cw2: 0.3,
            cw3: 2.0,
            cv1: 7.1,
            cs: 0.3,
            cdes: 0.65,
            fw_star: 0.424,
            low_re_correction: true,
            cd1: 8.0,
            cd2: 3.0,
            cdt1: 8.0,
            cdt2: 3.0,
            cl: 3.55,
            ct: 1.63,
            fe: true,
        }
    }
}

fn sqr(a: f64) -> f64 {
    a * a
}

fn pow3(a: f64) -> f64 {
    a * sqr(a)
}

fn pow6(a: f64) -> f64 {
    pow3(sqr(a))
}

/// Upstream `r(nur, magGradU-or-Stilda, length)`: `min(nur/(max(s, SMALL) (kappa l)^2), 10)`.
pub fn sa_r(nur: f64, s: f64, length: f64, kappa: f64) -> f64 {
    source_terms::umin(
        nur / (source_terms::umax(s, U_SMALL) * sqr(kappa * length)),
        10.0,
    )
}

/// Spalart-Allmaras DES family on a frozen field (terms only).
pub struct SpalartAllmarasDes {
    pub mesh: Arc<FvMesh>,
    /// Which model of the family.
    pub variant: SaDesVariant,
    /// Coefficients (upstream defaults).
    pub coeffs: SaDesCoeffs,
    /// Working variable nuTilda [m^2/s].
    pub nu_tilde: VolScalarField,
    /// Molecular kinematic viscosity [m^2/s].
    pub nu: VolScalarField,
    /// Eddy viscosity `nut` the shielding uses (`nuEff = nut + nu`) [m^2/s].
    pub nu_t: VolScalarField,
    /// LES width `delta` [m] (`cubeRootVol`, or `IDDESDelta` for IDDES).
    pub delta: Vec<f64>,
    /// `hmax` [m] from `IDDESDelta` (IDDES only).
    pub h_max: Option<Vec<f64>>,
    y: Vec<f64>,
}

impl SpalartAllmarasDes {
    /// Upstream defaults; `delta = 1.0 cbrt(V)` (`cubeRootVol`, `deltaCoeff 1`), wall
    /// distance = nearest wall-face centre unless `FrozenState::wall_distance` overrides it.
    pub fn new(mesh: Arc<FvMesh>, variant: SaDesVariant) -> Self {
        let delta = (0..mesh.n_cells)
            .map(|c| 1.0 * mesh.cell_volumes[c].r_cbrt())
            .collect();
        let y = compute_wall_distance(&mesh);
        Self {
            variant,
            coeffs: SaDesCoeffs::default(),
            nu_tilde: VolScalarField::uniform("nuTilda", mesh.clone(), 1e-4),
            nu: VolScalarField::uniform("nu", mesh.clone(), 1e-5),
            nu_t: VolScalarField::zeros("nut", mesh.clone()),
            delta,
            h_max: None,
            y,
            mesh,
        }
    }

    /// Per-cell terms of the nuTilda equation on the CURRENT fields, upstream arithmetic.
    ///
    /// `state` carries `grad_u` and `grad_nu_tilda`; `wall_distance` and `cell_length`
    /// override the model's `y` and `delta`. Terms, in order: `delta`, `chi`, `fv1`, `fv2`,
    /// `psi`, `lLES`, then DDES `r_fd`; IDDES `alpha`, `fB`, `fdt`, `ft`, `fl`, `fe1`, `fe`;
    /// then `fd` (0 for DES, `fd` for DDES, `fdTilda` for IDDES), `dTilda`, `Omega`,
    /// `Stilda`, `r`, `fw`, `gradSqr`, `nuTilda_E`, `nuTilda_Sp`, `LESRegion`, `nut`
    /// (= `nuTilda fv1`).
    pub fn source_terms(&self, state: &FrozenState) -> SourceTerms {
        let n = self.mesh.n_cells;
        let cf = &self.coeffs;
        let nt = self.nu_tilde.internal.as_slice();
        let nu = self.nu.internal.as_slice();
        let nut = self.nu_t.internal.as_slice();
        let y: &[f64] = match &state.wall_distance {
            Some(v) => v.as_slice(),
            None => &self.y,
        };
        let delta: &[f64] = match &state.cell_length {
            Some(v) => v.as_slice(),
            None => &self.delta,
        };
        let cw1 = cf.cb1 / (cf.kappa * cf.kappa) + (1.0 + cf.cb2) / cf.sigma_nut;
        let mut names: Vec<&str> = vec!["delta", "chi", "fv1", "fv2", "psi", "lLES"];
        match self.variant {
            SaDesVariant::Des => {}
            SaDesVariant::Ddes => names.push("r_fd"),
            SaDesVariant::Iddes => {
                names.extend(["alpha", "fB", "fdt", "ft", "fl", "fe1", "fe"]);
            }
        }
        names.extend([
            "fd",
            "dTilda",
            "Omega",
            "Stilda",
            "r",
            "fw",
            "gradSqr",
            "nuTilda_E",
            "nuTilda_Sp",
            "LESRegion",
            "nut",
        ]);
        let mut cols: Vec<Vec<f64>> = names.iter().map(|_| Vec::with_capacity(n)).collect();
        for c in 0..n {
            let mut row: Vec<f64> = Vec::with_capacity(names.len());
            let x = nt[c] / nu[c];
            let chi3 = pow3(x);
            let f1 = chi3 / (chi3 + pow3(cf.cv1));
            let f2 = 1.0 - x / (1.0 + x * f1);
            // psi: upstream `psi(chi, fv1)` with `ft2 = 0` (ft2 off):
            // sqrt(min(100, (1 - Cb1/(Cw1 sqr(kappa) fwStar) (ft2 + (1 - ft2) fv2))
            //                / max(SMALL, fv1 max(1e-10, 1 - ft2))))
            let ft2 = 0.0_f64;
            let psi = if cf.low_re_correction {
                let a = cf.cb1 / ((cw1 * sqr(cf.kappa)) * cf.fw_star);
                let num = 1.0 - a * (ft2 + (1.0 - ft2) * f2);
                let den = source_terms::umax(U_SMALL, f1 * source_terms::umax(1e-10, 1.0 - ft2));
                source_terms::umin(100.0, num / den).sqrt()
            } else {
                0.0
            };
            let l_les = (psi * cf.cdes) * delta[c];
            let gr = &state.grad_u[c];
            let mag_grad_u = source_terms::mag_tensor(gr);
            let yc = y[c];
            let mut extra: Vec<f64> = Vec::new();
            let fd_out;
            let d_tilda;
            match self.variant {
                SaDesVariant::Des => {
                    fd_out = 0.0;
                    d_tilda = source_terms::umin(l_les, yc);
                }
                SaDesVariant::Ddes => {
                    let r_fd = sa_r(nut[c] + nu[c], mag_grad_u, yc, cf.kappa);
                    let fd = 1.0 - (cf.cd1 * r_fd).r_powf(cf.cd2).r_tanh();
                    fd_out = fd;
                    d_tilda = source_terms::umax(
                        yc - fd * source_terms::umax(yc - l_les, 0.0),
                        U_SMALL,
                    );
                    extra.push(r_fd);
                }
                SaDesVariant::Iddes => {
                    let hmax = self.h_max.as_ref().expect("IDDES needs h_max")[c];
                    let alpha = source_terms::umax(0.25 - yc / hmax, -5.0);
                    let exp_term = sqr(alpha).r_exp();
                    let fb = source_terms::umin(2.0 * exp_term.r_powf(-9.0), 1.0);
                    let rt = sa_r(nut[c], mag_grad_u, yc, cf.kappa);
                    let rl = sa_r(nu[c], mag_grad_u, yc, cf.kappa);
                    let fdt = 1.0 - (cf.cdt1 * rt).r_powf(cf.cdt2).r_tanh();
                    let fd_tilda = source_terms::umax(1.0 - fdt, fb);
                    let ft = pow3(sqr(cf.ct) * rt).r_tanh();
                    let fl = (sqr(cf.cl) * rl).r_powf(10.0).r_tanh();
                    let fe1 = 2.0
                        * source_terms::lerp(
                            exp_term.r_powf(-9.0),
                            exp_term.r_powf(-11.09),
                            source_terms::pos0(alpha),
                        );
                    let fe2 = 1.0 - source_terms::umax(ft, fl);
                    let fe = (source_terms::umax(fe1 - 1.0, 0.0) * psi) * fe2;
                    fd_out = fd_tilda;
                    d_tilda = if cf.fe {
                        source_terms::umax(
                            ((fd_tilda * (1.0 + fe)) * yc) + ((1.0 - fd_tilda) * l_les),
                            U_SMALL,
                        )
                    } else {
                        source_terms::umax(source_terms::lerp(l_les, yc, fd_tilda), U_SMALL)
                    };
                    extra.extend([alpha, fb, fdt, ft, fl, fe1, fe]);
                }
            }
            let om = std::f64::consts::SQRT_2 * source_terms::mag_skew(gr);
            let st = source_terms::umax(
                om + (f2 * nt[c]) / sqr(cf.kappa * d_tilda),
                cf.cs * om,
            );
            let r = source_terms::umin(
                nt[c] / (source_terms::umax(st, U_SMALL) * sqr(cf.kappa * d_tilda)),
                10.0,
            );
            let g = r + cf.cw2 * (pow6(r) - r);
            let fw = g
                * ((1.0 + pow6(cf.cw3)) / (pow6(g) + pow6(cf.cw3))).r_powf(1.0 / 6.0);
            let grad_sqr = source_terms::mag_sqr(state.grad_nu_tilda[c]);
            let prod = ((cf.cb1 * st) * nt[c]) * (1.0 - 0.0);
            let sp = ((cw1 * fw - (cf.cb1 / sqr(cf.kappa)) * 0.0) * nt[c]) / sqr(d_tilda);
            row.extend([delta[c], x, f1, f2, psi, l_les]);
            row.extend(extra);
            row.extend([
                fd_out,
                d_tilda,
                om,
                st,
                r,
                fw,
                grad_sqr,
                prod,
                sp,
                source_terms::neg(d_tilda - yc),
                nt[c] * f1,
            ]);
            for (col, v) in cols.iter_mut().zip(row) {
                col.push(v);
            }
        }
        let mut out = SourceTerms::default();
        for (name, col) in names.iter().zip(cols) {
            out.push(name, col);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use outram_foam_basic_lib::prelude::{
        BoundaryPatch, FvMeshBuilder, PatchKind, Tensor, Vector3,
    };

    fn vx(x: f64) -> Vector3 {
        Vector3::new(x, 0.0, 0.0)
    }

    fn line_mesh() -> Arc<FvMesh> {
        Arc::new(
            FvMeshBuilder::new()
                .n_cells(3)
                .n_internal_faces(2)
                .owner(vec![0, 1, 0, 2])
                .neighbour(vec![1, 2])
                .patches(vec![
                    BoundaryPatch::new("left", 2, 1, PatchKind::Wall),
                    BoundaryPatch::new("right", 3, 1, PatchKind::Patch),
                ])
                .cell_volumes(vec![8.0, 8.0, 8.0])
                .cell_centres(vec![vx(0.5), vx(1.5), vx(2.5)])
                .face_area_vectors(vec![vx(1.0), vx(1.0), vx(-1.0), vx(1.0)])
                .face_centres(vec![vx(1.0), vx(2.0), vx(0.0), vx(3.0)])
                .build()
                .unwrap(),
        )
    }

    fn state() -> FrozenState {
        let g = Tensor::new(0.0, 2.0, 0.0, 0.1, 0.0, 0.0, 0.0, 0.0, 0.0);
        FrozenState {
            grad_u: vec![g; 3],
            grad_nu_tilda: vec![Vector3::new(1e-3, 0.0, 0.0); 3],
            wall_distance: Some(vec![0.5, 1.5, 2.5]),
            cell_length: Some(vec![0.2, 0.2, 0.2]),
            ..FrozenState::default()
        }
    }

    fn model(v: SaDesVariant) -> SpalartAllmarasDes {
        let mut m = SpalartAllmarasDes::new(line_mesh(), v);
        m.nu_tilde = VolScalarField::uniform("nuTilda", m.mesh.clone(), 3e-5);
        m.nu_t = VolScalarField::uniform("nut", m.mesh.clone(), 1e-5);
        m.h_max = Some(vec![1.0, 1.0, 1.0]);
        m
    }

    /// DES takes the smaller of the wall distance and the LES length: the near-wall cell
    /// (y = 0.5 > lLES) is in the LES region, so dTilda <= y in every cell.
    #[test]
    fn des_length_scale_is_min_of_les_and_wall_distance() {
        let t = model(SaDesVariant::Des).source_terms(&state());
        let d = t.get("dTilda").unwrap();
        let y = [0.5, 1.5, 2.5];
        let l = t.get("lLES").unwrap();
        for c in 0..3 {
            assert_eq!(d[c], l[c].min(y[c]));
        }
        assert!(t.get("psi").unwrap().iter().all(|p| p.is_finite() && *p > 0.0));
        assert!(t.get("LESRegion").unwrap().iter().all(|r| *r == 1.0 || *r == 0.0));
    }

    /// DDES with the wall far away keeps fd near 0 and so dTilda near y = RAS; with the
    /// shielding saturated at 1 it falls back to the LES length. Both bounds hold.
    #[test]
    fn ddes_length_scale_lies_between_les_and_ras() {
        let t = model(SaDesVariant::Ddes).source_terms(&state());
        let (d, l) = (t.get("dTilda").unwrap(), t.get("lLES").unwrap());
        let fd = t.get("fd").unwrap();
        for c in 0..3 {
            assert!((0.0..=1.0).contains(&fd[c]));
            assert!(d[c] <= [0.5, 1.5, 2.5][c] + 1e-15);
            assert!(d[c] >= l[c].min([0.5, 1.5, 2.5][c]) - 1e-15);
        }
        assert!(t.get("r_fd").is_some());
    }

    /// IDDES: both the Shur (fe on) and Gritskevich (fe off) forms give a finite
    /// positive length scale, and the elevating function vanishes with fe off.
    #[test]
    fn iddes_forms_are_finite_and_positive() {
        let mut m = model(SaDesVariant::Iddes);
        let on = m.source_terms(&state());
        m.coeffs.fe = false;
        let off = m.source_terms(&state());
        for t in [&on, &off] {
            assert!(t.get("dTilda").unwrap().iter().all(|d| d.is_finite() && *d > 0.0));
            assert!(t.get("alpha").unwrap().iter().all(|a| *a >= -5.0));
        }
        assert!(on.get("fe").is_some() && off.get("nuTilda_Sp").is_some());
    }

    /// Without the low-Re correction psi is 0, so lLES is 0 and the DES length is 0
    /// (documents the switch; upstream default is on).
    #[test]
    fn low_re_correction_switch_zeroes_psi() {
        let mut m = model(SaDesVariant::Des);
        m.coeffs.low_re_correction = false;
        let t = m.source_terms(&state());
        assert!(t.get("psi").unwrap().iter().all(|p| *p == 0.0));
    }

    /// The model owns a wall distance when no override is given.
    #[test]
    fn own_wall_distance_is_used_without_override() {
        let mut s = state();
        s.wall_distance = None;
        let t = model(SaDesVariant::Des).source_terms(&s);
        assert_eq!(t.get("dTilda").unwrap().len(), 3);
    }
}
