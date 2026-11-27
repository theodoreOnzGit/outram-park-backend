// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: https://gitlab.com/foam-for-nuclear/GeN-Foam
//   Upstream commit: 652b3da
//   Files: src/classes/thermalHydraulics/solvers/twoPhase/include/alphaEqnsRoutines/myMULES/
//          {myMULES.C, myMULES.H, myMULESTemplates.C}
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//   Upstream license: GPL-3.0
// and from OpenFOAM v2506 (ESI), (C) 2011-2016 OpenFOAM Foundation,
// 2017-2025 OpenCFD Ltd., GPL-3.0:
//   finiteVolume/interpolation/surfaceInterpolation/limitedSchemes/{LimitedScheme,vanLeer,NVDTVD,
//   limitedSurfaceInterpolationScheme}, finiteVolume/gradSchemes/gaussGrad.
//
// This file is part of OUTRAM PARK, distributed under GPL-3.0. See the crate
// root or <https://www.gnu.org/licenses/> for the full license text.

//! `myMULES`: GeN-Foam's copy of OpenFOAM's MULES explicit solver and limiter
//! (gh:#1361 round 7), with the convection schemes its alpha fluxes need
//! (`Gauss vanLeer`, `Gauss upwind`, `Gauss linear`).
//!
//! Translated operation by operation, in upstream's order, so the result is
//! bit-identical on a mesh without coupled patches (no `cyclic`, no `wedge`;
//! `syncTools::syncFaceList` is the identity there). A surface field is a
//! `Vec<f64>` over all faces, zero on `empty` faces, as in [`super::fv`].
//! Only `rho = geometricOneField` is supported (the one GeN-Foam passes).

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;

use super::fv::{Bc, Geometry, Scalar};
use outram_foam_basic_lib::prelude::PatchKind;

/// `ROOTVSMALL` of OpenFOAM's `doubleScalar`.
const ROOT_V_SMALL: f64 = 1.0e-150;
/// `SMALL` of OpenFOAM's `doubleScalar`.
const SMALL: f64 = 1.0e-15;

/// OpenFOAM `min(a, b)` for scalars: `(a < b) ? a : b`.
#[inline]
fn fmin(a: f64, b: f64) -> f64 {
    if a < b {
        a
    } else {
        b
    }
}

/// OpenFOAM `max(a, b)` for scalars: `(a > b) ? a : b`.
#[inline]
fn fmax(a: f64, b: f64) -> f64 {
    if a > b {
        a
    } else {
        b
    }
}

/// OpenFOAM `sign(s)`: `(s >= 0) ? 1 : -1`.
#[inline]
fn sign(s: f64) -> f64 {
    if s >= 0.0 {
        1.0
    } else {
        -1.0
    }
}

/// The convection scheme of a `div(phi,alpha)`-type entry (`Gauss <scheme>`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlphaConvection {
    /// `Gauss upwind`.
    Upwind,
    /// `Gauss linear`.
    Linear,
    /// `Gauss vanLeer` (`LimitedScheme<scalar, vanLeerLimiter<NVDTVD>, magSqr>`).
    VanLeer,
}

impl AlphaConvection {
    /// Parse the scheme words of a `divSchemes` entry (`["Gauss", "vanLeer"]`).
    #[must_use]
    pub fn from_words(words: &[&str]) -> Option<Self> {
        match words {
            ["Gauss", "upwind"] => Some(Self::Upwind),
            ["Gauss", "linear"] => Some(Self::Linear),
            ["Gauss", "vanLeer"] => Some(Self::VanLeer),
            _ => None,
        }
    }
}

/// `fvc::grad(vsf)` with `Gauss linear`: per-cell gradient, three components
/// (`gaussGrad::calcGrad`: `Sf*ssf` summed over the faces, divided by `V`).
#[must_use]
pub fn gauss_linear_grad(g: &Geometry, s: &Scalar) -> Vec<[f64; 3]> {
    let m = &g.mesh;
    let ni = g.n_internal();
    let mut grad = vec![[0.0_f64; 3]; g.n_cells()];
    for f in 0..ni {
        let w = g.weights[f];
        let (o, n) = (m.owner[f], m.neighbour[f]);
        let ssf = w * s.c[o] + (1.0 - w) * s.c[n];
        let sf = m.face_area_vectors[f];
        let v = [sf.x * ssf, sf.y * ssf, sf.z * ssf];
        for k in 0..3 {
            grad[o][k] += v[k];
            grad[n][k] -= v[k];
        }
    }
    for f in ni..g.n_faces() {
        if !g.active[f] {
            continue;
        }
        let sf = m.face_area_vectors[f];
        let ssf = s.b[f - ni];
        let o = m.owner[f];
        grad[o][0] += sf.x * ssf;
        grad[o][1] += sf.y * ssf;
        grad[o][2] += sf.z * ssf;
    }
    for (c, gc) in grad.iter_mut().enumerate() {
        let v = m.cell_volumes[c];
        for k in gc.iter_mut() {
            *k /= v;
        }
    }
    grad
}

/// `NVDTVD::r` followed by `vanLeerLimiter::limiter`.
fn van_leer_limiter(face_flux: f64, phi_p: f64, phi_n: f64, grad_p: &[f64; 3], grad_n: &[f64; 3], d: [f64; 3]) -> f64 {
    let gradf = phi_n - phi_p;
    let gc = if face_flux > 0.0 { grad_p } else { grad_n };
    let gradcf = d[0] * gc[0] + d[1] * gc[1] + d[2] * gc[2];
    let r = if gradcf.abs() >= 1000.0 * gradf.abs() {
        2.0 * 1000.0 * sign(gradcf) * sign(gradf) - 1.0
    } else {
        2.0 * (gradcf / gradf) - 1.0
    };
    (r + r.abs()) / (1.0 + r.abs())
}

/// `scheme.interpolate(vf)` with the face flux `face_flux` selecting the
/// upwind direction: internal faces `w vf[P] + (1 - w) vf[N]` with the weights
/// of `limitedSurfaceInterpolationScheme::weights`; boundary faces take the
/// boundary value (no coupled patches).
#[must_use]
pub fn interpolate(g: &Geometry, scheme: AlphaConvection, face_flux: &[f64], vf: &Scalar) -> Vec<f64> {
    let m = &g.mesh;
    let ni = g.n_internal();
    let mut out = g.surface_zeros();
    let limiters: Vec<f64> = match scheme {
        AlphaConvection::Upwind => vec![0.0; ni],
        AlphaConvection::Linear => vec![1.0; ni],
        AlphaConvection::VanLeer => {
            let grad = gauss_linear_grad(g, vf);
            (0..ni)
                .map(|f| {
                    let (o, n) = (m.owner[f], m.neighbour[f]);
                    let d = m.cell_centres[n] - m.cell_centres[o];
                    van_leer_limiter(face_flux[f], vf.c[o], vf.c[n], &grad[o], &grad[n], [d.x, d.y, d.z])
                })
                .collect()
        }
    };
    for f in 0..ni {
        let pos0 = if face_flux[f] >= 0.0 { 1.0 } else { 0.0 };
        let w = limiters[f] * g.weights[f] + (1.0 - limiters[f]) * pos0;
        out[f] = w * vf.c[m.owner[f]] + (1.0 - w) * vf.c[m.neighbour[f]];
    }
    for f in ni..g.n_faces() {
        if g.active[f] {
            out[f] = vf.b[f - ni];
        }
    }
    out
}

/// `fvc::flux(face_flux, vf, scheme)` = `face_flux * interpolate(vf)` on every face.
#[must_use]
pub fn flux(g: &Geometry, scheme: AlphaConvection, face_flux: &[f64], vf: &Scalar) -> Vec<f64> {
    let i = interpolate(g, scheme, face_flux, vf);
    (0..g.n_faces()).map(|f| face_flux[f] * i[f]).collect()
}

/// Limiter controls read from the `alpha` dictionary of `fvSolution`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LimiterControls {
    /// `nLimiterIter` (default 3).
    pub n_limiter_iter: usize,
    /// `smoothLimiter` (default 0).
    pub smooth_limiter: f64,
    /// `extremaCoeff` (default 0).
    pub extrema_coeff: f64,
}

impl Default for LimiterControls {
    fn default() -> Self {
        Self { n_limiter_iter: 3, smooth_limiter: 0.0, extrema_coeff: 0.0 }
    }
}

/// `myMULES::limiter` for `rho = one`, no moving mesh, `Sp = Su = 0`: the
/// per-face limiter `lambda` (1 on boundary faces of non-coupled patches).
/// The zero-source case of [`limiter_with_sources`].
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn limiter(
    g: &Geometry,
    ctl: &LimiterControls,
    r_delta_t: f64,
    psi: &Scalar,
    psi0: &[f64],
    phi_bd: &[f64],
    phi_corr: &[f64],
    psi_max: &[f64],
    psi_min: &[f64],
) -> Vec<f64> {
    let zero = vec![0.0; g.n_cells()];
    limiter_with_sources(g, ctl, r_delta_t, psi, psi0, phi_bd, phi_corr, &zero, &zero, psi_max, psi_min)
}

/// `myMULES::limiter` (myMULESTemplates.C:241-610) for `rho = geometricOneField`, no moving mesh, with the
/// implicit and explicit sources `Sp`, `Su` (`rho rDeltaT - Sp` and `Su` in the `psiMaxn`/`psiMinn` bounds,
/// lines 440-452). Boundary faces of non-coupled patches keep `lambda = 1`; faces of a `wedge` patch are set
/// to 0 at the end of every iteration (lines 561-566). Coupled patches (`cyclic`) are not in the mesh type
/// of this port, so `psiPf.coupled()` is never true here.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn limiter_with_sources(
    g: &Geometry,
    ctl: &LimiterControls,
    r_delta_t: f64,
    psi: &Scalar,
    psi0: &[f64],
    phi_bd: &[f64],
    phi_corr: &[f64],
    sp: &[f64],
    su: &[f64],
    psi_max: &[f64],
    psi_min: &[f64],
) -> Vec<f64> {
    let m = &g.mesh;
    let ni = g.n_internal();
    let nf = g.n_faces();
    let nc = g.n_cells();
    let psi_if = &psi.c;
    let mut lambda = vec![1.0_f64; nf];

    let psi_max_field: Vec<f64> = psi_max.to_vec();
    let psi_min_field: Vec<f64> = psi_min.to_vec();
    let mut psi_maxn: Vec<f64> = psi_min.to_vec();
    let mut psi_minn: Vec<f64> = psi_max.to_vec();
    let mut sum_phi_bd = vec![0.0; nc];
    let mut sum_phip = vec![0.0; nc];
    let mut m_sum_phim = vec![0.0; nc];

    for f in 0..ni {
        let (own, nei) = (m.owner[f], m.neighbour[f]);
        psi_maxn[own] = fmax(psi_maxn[own], psi_if[nei]);
        psi_minn[own] = fmin(psi_minn[own], psi_if[nei]);
        psi_maxn[nei] = fmax(psi_maxn[nei], psi_if[own]);
        psi_minn[nei] = fmin(psi_minn[nei], psi_if[own]);
        sum_phi_bd[own] += phi_bd[f];
        sum_phi_bd[nei] -= phi_bd[f];
        let phi_corrf = phi_corr[f];
        if phi_corrf > 0.0 {
            sum_phip[own] += phi_corrf;
            m_sum_phim[nei] += phi_corrf;
        } else {
            m_sum_phim[own] -= phi_corrf;
            sum_phip[nei] -= phi_corrf;
        }
    }
    for f in ni..nf {
        if !g.active[f] {
            continue;
        }
        let c = m.owner[f];
        let fixes = matches!(psi.bc[g.patch_of[f - ni]], Bc::Fixed(_));
        if fixes {
            let pv = psi.b[f - ni];
            psi_maxn[c] = fmax(psi_maxn[c], pv);
            psi_minn[c] = fmin(psi_minn[c], pv);
        } else {
            psi_maxn[c] = fmax(psi_maxn[c], psi_max[c]);
            psi_minn[c] = fmin(psi_minn[c], psi_min[c]);
        }
        sum_phi_bd[c] += phi_bd[f];
        let phi_corrf = phi_corr[f];
        if phi_corrf > 0.0 {
            sum_phip[c] += phi_corrf;
        } else {
            m_sum_phim[c] -= phi_corrf;
        }
    }

    for c in 0..nc {
        let span = psi_max_field[c] - psi_min_field[c];
        psi_maxn[c] = fmin(psi_maxn[c] + ctl.extrema_coeff * span, psi_max_field[c]);
        psi_minn[c] = fmax(psi_minn[c] - ctl.extrema_coeff * span, psi_min_field[c]);
    }
    if ctl.smooth_limiter > SMALL {
        let s = ctl.smooth_limiter;
        for c in 0..nc {
            psi_maxn[c] = fmin(s * psi_if[c] + (1.0 - s) * psi_maxn[c], psi_max_field[c]);
            psi_minn[c] = fmax(s * psi_if[c] + (1.0 - s) * psi_minn[c], psi_min_field[c]);
        }
    }
    for c in 0..nc {
        let v = m.cell_volumes[c];
        // V*((rho rDeltaT - Sp) psiMaxn - Su - (rho0 rDeltaT) psi0) + sumPhiBD ; V*(Su - (rho rDeltaT - Sp) psiMinn + (rho0 rDeltaT) psi0) - sumPhiBD
        psi_maxn[c] = v * ((((r_delta_t - sp[c]) * psi_maxn[c]) - su[c]) - (r_delta_t * psi0[c])) + sum_phi_bd[c];
        psi_minn[c] = v * ((su[c] - ((r_delta_t - sp[c]) * psi_minn[c])) + (r_delta_t * psi0[c])) - sum_phi_bd[c];
    }

    let mut sum_lphip = vec![0.0; nc];
    let mut m_sum_lphim = vec![0.0; nc];
    for _ in 0..ctl.n_limiter_iter {
        sum_lphip.iter_mut().for_each(|x| *x = 0.0);
        m_sum_lphim.iter_mut().for_each(|x| *x = 0.0);
        for f in 0..ni {
            let (own, nei) = (m.owner[f], m.neighbour[f]);
            let lpc = lambda[f] * phi_corr[f];
            if lpc > 0.0 {
                sum_lphip[own] += lpc;
                m_sum_lphim[nei] += lpc;
            } else {
                m_sum_lphim[own] -= lpc;
                sum_lphip[nei] -= lpc;
            }
        }
        for f in ni..nf {
            if !g.active[f] {
                continue;
            }
            let c = m.owner[f];
            let lpc = lambda[f] * phi_corr[f];
            if lpc > 0.0 {
                sum_lphip[c] += lpc;
            } else {
                m_sum_lphim[c] -= lpc;
            }
        }
        for c in 0..nc {
            sum_lphip[c] = fmax(fmin((sum_lphip[c] + psi_maxn[c]) / (m_sum_phim[c] + ROOT_V_SMALL), 1.0), 0.0);
            m_sum_lphim[c] = fmax(fmin((m_sum_lphim[c] + psi_minn[c]) / (sum_phip[c] + ROOT_V_SMALL), 1.0), 0.0);
        }
        let lambdam = &sum_lphip;
        let lambdap = &m_sum_lphim;
        for f in 0..ni {
            let (own, nei) = (m.owner[f], m.neighbour[f]);
            lambda[f] = if phi_corr[f] > 0.0 {
                fmin(lambda[f], fmin(lambdap[own], lambdam[nei]))
            } else {
                fmin(lambda[f], fmin(lambdam[own], lambdap[nei]))
            };
        }
        // Boundary faces of non-coupled, non-wedge patches keep lambda = 1; a `wedge` patch is set to 0.
        for f in ni..nf {
            if g.active[f] && m.patches[g.patch_of[f - ni]].kind == PatchKind::Wedge {
                lambda[f] = 0.0;
            }
        }
    }
    lambda
}

/// `myMULES::limit(rDeltaT, one, psi, phi, phiPsi, zero, zero, psiMax, zero,
/// returnCorr = true)`: on return `phi_psi` holds the LIMITED CORRECTION
/// `lambda (phiPsi - phiBD)`, not the full flux (upstream's call in
/// `solveAlphaMULES.H`).
#[allow(clippy::too_many_arguments)]
pub fn limit_return_corr(
    g: &Geometry,
    ctl: &LimiterControls,
    r_delta_t: f64,
    psi: &Scalar,
    psi0: &[f64],
    phi: &[f64],
    phi_psi: &mut [f64],
    psi_max: &[f64],
) {
    let zero = vec![0.0; g.n_cells()];
    limit_core(g, ctl, r_delta_t, psi, psi0, phi, phi_psi, &zero, &zero, psi_max, &zero, true);
}

/// `myMULES::limit(rDeltaT, one, psi, phi, phiPsi, Sp, Su, psiMax, psiMin, returnCorr = false)`
/// (myMULESTemplates.C:622-697): on return `phi_psi` holds the FULL limited flux
/// `phiBD + lambda (phiPsi - phiBD)` (the form `solveAlpha` stores as `alphaPhi`).
#[allow(clippy::too_many_arguments)]
pub fn limit_full_flux(
    g: &Geometry,
    ctl: &LimiterControls,
    r_delta_t: f64,
    psi: &Scalar,
    psi0: &[f64],
    phi: &[f64],
    phi_psi: &mut [f64],
    sp: &[f64],
    su: &[f64],
    psi_max: &[f64],
    psi_min: &[f64],
) {
    limit_core(g, ctl, r_delta_t, psi, psi0, phi, phi_psi, sp, su, psi_max, psi_min, false);
}

/// The body of `myMULES::limit`, upstream's order: `phiBD = upwind flux` with the non-coupled boundary set to
/// `phiPsi`, `phiCorr = phiPsi - phiBD`, `lambda` from the limiter, then `phiCorr *= lambda` (`return_corr`) or
/// `phiPsi = phiBD + lambda phiCorr`.
#[allow(clippy::too_many_arguments)]
fn limit_core(
    g: &Geometry,
    ctl: &LimiterControls,
    r_delta_t: f64,
    psi: &Scalar,
    psi0: &[f64],
    phi: &[f64],
    phi_psi: &mut [f64],
    sp: &[f64],
    su: &[f64],
    psi_max: &[f64],
    psi_min: &[f64],
    return_corr: bool,
) {
    let nf = g.n_faces();
    let ni = g.n_internal();
    // phiBD = upwind<scalar>(mesh, phi).flux(psi); non-coupled boundary := phiPsi.
    let up = interpolate(g, AlphaConvection::Upwind, phi, psi);
    let mut phi_bd: Vec<f64> = (0..nf).map(|f| phi[f] * up[f]).collect();
    for f in ni..nf {
        if g.active[f] {
            phi_bd[f] = phi_psi[f];
        }
    }
    let mut phi_corr: Vec<f64> = (0..nf).map(|f| phi_psi[f] - phi_bd[f]).collect();
    let lambda = limiter_with_sources(g, ctl, r_delta_t, psi, psi0, &phi_bd, &phi_corr, sp, su, psi_max, psi_min);
    if return_corr {
        for f in 0..nf {
            phi_corr[f] *= lambda[f];
        }
        phi_psi.copy_from_slice(&phi_corr);
    } else {
        for f in 0..nf {
            phi_psi[f] = phi_bd[f] + lambda[f] * phi_corr[f];
        }
    }
}

/// `myMULES::explicitSolve(rDeltaT, one, psi, phiPsi, Sp, Su)` WITHOUT the
/// limiter: `psi = (psi0 rDeltaT + Su - div(phiPsi)) / (rDeltaT - Sp)` on the
/// cells (relaxation and `correctBoundaryConditions` are the caller's).
pub fn explicit_update(g: &Geometry, r_delta_t: f64, psi: &mut [f64], psi0: &[f64], phi_psi: &[f64], sp: &[f64], su: &[f64]) {
    let m = &g.mesh;
    let ni = g.n_internal();
    let nc = g.n_cells();
    let mut div = vec![0.0; nc];
    for f in 0..ni {
        div[m.owner[f]] += phi_psi[f];
        div[m.neighbour[f]] -= phi_psi[f];
    }
    for f in ni..g.n_faces() {
        if g.active[f] {
            div[m.owner[f]] += phi_psi[f];
        }
    }
    for c in 0..nc {
        div[c] /= m.cell_volumes[c];
    }
    for c in 0..nc {
        psi[c] = (psi0[c] * r_delta_t + su[c] - div[c]) / (r_delta_t - sp[c]);
    }
}
