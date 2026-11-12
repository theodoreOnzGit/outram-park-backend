// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// Derived from OpenFOAM (www.openfoam.com), ESI/OpenCFD release v2506
// Copyright (C) 2011-2017 OpenFOAM Foundation
// Copyright (C) 2015-2024 OpenCFD Ltd.
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

//! # `esi` — sonicFoam as OpenFOAM-v2506 (ESI) writes it (gh:#1363, row 3b)
//!
//! A translation, operation for operation, of ESI v2506
//! `applications/solvers/compressible/sonicFoam` (`sonicFoam.C`,
//! `createFields.H`, `UEqn.H`, `EEqn.H`, `pEqn.H`) and of the finite-volume
//! machinery those files call, for the case the port can honestly claim:
//! a mesh that is one-dimensional along x (`empty` y and z patches), every
//! other patch `zeroGradient` for `U`, `p` and `T`, a gas with `mu = 0`.
//! Anything else is **refused** by name ([`AppBuilderError::UnsupportedScheme`]).
//!
//! The existing [`super::SonicFoam::step`] is not changed and stays callable;
//! it is a sketch of neither the ESI nor the Foundation line (no energy
//! equation).
//!
//! ## Upstream sources translated (all `OpenFOAM-v2506/src`)
//! * `finiteVolume/finiteVolume/ddtSchemes/EulerDdtScheme/EulerDdtScheme.C`
//!   (`fvmDdt(rho, vf)`, `fvmDdt(vf)`, `fvcDdt(rho, vf)`, `fvcDdtPhiCorr(rho, U, phi)`)
//!   and `ddtSchemes/ddtScheme/ddtScheme.C` (`fvcDdtPhiCoeff`, default branch
//!   because `experimentalDdtCorr 0` in `etc/controlDict`);
//! * `convectionSchemes/gaussConvectionScheme` (`fvmDiv`, `fvcDiv`, `flux`);
//! * `interpolation/surfaceInterpolation/limitedSchemes` (`LimitedScheme`,
//!   `limitedLinear`, `NVDTVD`, `limitedSurfaceInterpolationScheme::weights`);
//! * `fvMesh/fvGeometryScheme/basic/basicFvGeometryScheme.C` (weights,
//!   `deltaCoeffs`), `gradSchemes/gaussGrad`, `laplacianSchemes/gaussLaplacianScheme`;
//! * `fvMatrices/fvMatrix/fvMatrix.C` (`A`, `H`, `flux`, `solveSegregated`),
//!   `OpenFOAM/matrices/lduMatrix` (`Amul`, `sumA`, `residual`, `H`, `faceH`,
//!   `negSumDiag`, `operator+=`/`-=`, `smoothSolver`, `symGaussSeidelSmoother`,
//!   `solver::normFactor`, `SolverPerformance::checkConvergence`);
//! * `thermophysicalModels/basic/psiThermo/hePsiThermo.C` (`calculate`).
//!
//! ## What was verified, and what was not
//! See `verification_and_validation/sonic_foam_vs_openfoam.md`: the port is
//! compared with a driver that is `sonicFoam.C` on the unmodified ESI thermo
//! and turbulence libraries, per cell, bit for bit.

use crate::error::AppBuilderError;
use crate::solvers::rho_central_foam::PerfectGas;
use outram_foam_basic_lib::mesh::fv_mesh::{FvMesh, PatchKind};
use std::sync::Arc;

/// `SMALL` of `doubleScalar.H`.
const SMALL: f64 = 1.0e-15;
/// `solverPerformance::small_`.
const SOLVER_SMALL: f64 = 1.0e-20;

fn unsupported(scheme: &str, reason: &'static str) -> AppBuilderError {
    AppBuilderError::UnsupportedScheme {
        family: "sonicFoam (ESI v2506)",
        scheme: scheme.to_string(),
        reason,
    }
}

/// `Foam::sign`: `(s >= 0) ? 1 : -1`.
fn sign(s: f64) -> f64 {
    if s >= 0.0 {
        1.0
    } else {
        -1.0
    }
}

/// `Foam::pos0`: `(s >= 0) ? 1 : 0`.
fn pos0(s: f64) -> f64 {
    if s >= 0.0 {
        1.0
    } else {
        0.0
    }
}

/// Mesh data in the order OpenFOAM's `lduAddressing` and `surfaceInterpolation`
/// use it (x only: the mesh is one-dimensional along x).
#[derive(Debug, Clone)]
pub struct Topo {
    n: usize,
    nf: usize,
    owner: Vec<usize>,
    nei: Vec<usize>,
    own_start: Vec<usize>,
    v: Vec<f64>,
    cx: Vec<f64>,
    cy: Vec<f64>,
    cz: Vec<f64>,
    sfx: Vec<f64>,
    sfy: Vec<f64>,
    sfz: Vec<f64>,
    mag_sf: Vec<f64>,
    w: Vec<f64>,
    dc: Vec<f64>,
    /// `(owner cell, Sf)` of every face on a non-empty boundary patch.
    bfaces: Vec<(usize, [f64; 3])>,
}

impl Topo {
    /// Cell volumes `V`.
    #[must_use]
    pub fn volumes(&self) -> &[f64] {
        &self.v
    }
    /// Interpolation weights of the internal faces (`mesh.weights()`).
    #[must_use]
    pub fn weights(&self) -> &[f64] {
        &self.w
    }
    /// `mesh.deltaCoeffs()` of the internal faces.
    #[must_use]
    pub fn delta_coeffs(&self) -> &[f64] {
        &self.dc
    }
    /// `mag(Sf)` of the internal faces.
    #[must_use]
    pub fn mag_sf(&self) -> &[f64] {
        &self.mag_sf
    }
    /// `Sf_x` of the internal faces.
    #[must_use]
    pub fn sf_x(&self) -> &[f64] {
        &self.sfx
    }
}

fn dot3(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// Build [`Topo`] from `mesh`, refusing what the translation does not cover.
///
/// # Errors
/// [`AppBuilderError::UnsupportedScheme`] if the mesh is not one-dimensional in
/// x with `empty` y/z patches, has a patch other than `Patch`/`Wall`/`Empty`,
/// or its internal faces are not in upper-triangular order.
pub fn build_topo(mesh: &FvMesh) -> Result<Topo, AppBuilderError> {
    let n = mesh.n_cells;
    let nf = mesh.n_internal_faces;
    // upstream polyMesh::calcDirections: solved components
    let mut sum = [0.0_f64; 3];
    for patch in &mesh.patches {
        if patch.kind == PatchKind::Empty {
            for fi in 0..patch.size {
                let sf = mesh.face_area_vectors[patch.start + fi];
                sum[0] += sf.x.abs();
                sum[1] += sf.y.abs();
                sum[2] += sf.z.abs();
            }
        }
    }
    let m = (sum[0] * sum[0] + sum[1] * sum[1] + sum[2] * sum[2]).sqrt();
    // empty directions are y and z: their normalised summed |Sf| exceeds 1e-6
    let one_d_in_x = m > 0.0 && sum[0] / m <= 1e-6 && sum[1] / m > 1e-6 && sum[2] / m > 1e-6;
    if !one_d_in_x {
        return Err(unsupported("mesh", "needs empty patches in y and z only (a 1-D mesh along x)"));
    }
    let mut bfaces = Vec::new();
    for patch in &mesh.patches {
        match patch.kind {
            PatchKind::Empty => {}
            PatchKind::Patch | PatchKind::Wall => {
                for fi in 0..patch.size {
                    let f = patch.start + fi;
                    let sf = mesh.face_area_vectors[f];
                    if sf.y.abs() > 1e-12 * sf.x.abs() || sf.z.abs() > 1e-12 * sf.x.abs() {
                        return Err(unsupported("mesh", "boundary face normal not along x"));
                    }
                    bfaces.push((mesh.owner[f], [sf.x, sf.y, sf.z]));
                }
            }
            _ => return Err(unsupported("patch", "only zeroGradient patches (Patch/Wall) and empty are translated")),
        }
    }
    let mut own_start = vec![0usize; n + 1];
    for f in 0..nf {
        if f > 0 && (mesh.owner[f] < mesh.owner[f - 1]) {
            return Err(unsupported("mesh", "internal faces are not in upper-triangular order"));
        }
        own_start[mesh.owner[f] + 1] += 1;
    }
    for c in 0..n {
        own_start[c + 1] += own_start[c];
    }
    let mut sfx = Vec::with_capacity(nf);
    let mut sfy = Vec::with_capacity(nf);
    let mut sfz = Vec::with_capacity(nf);
    let mut mag_sf = Vec::with_capacity(nf);
    let mut w = Vec::with_capacity(nf);
    let mut dc = Vec::with_capacity(nf);
    for f in 0..nf {
        let sf = mesh.face_area_vectors[f];
        // y and z components are round-off of the face-area computation; they
        // are carried (upstream's gradients, the limiter's `d & grad` and
        // `Sf & v` are 3-D) because the limiter reacts to round-off
        if sf.y.abs() > 1e-12 * sf.x.abs() || sf.z.abs() > 1e-12 * sf.x.abs() {
            return Err(unsupported("mesh", "internal face normal not along x"));
        }
        let (o, nb) = (mesh.owner[f], mesh.neighbour[f]);
        let (cf, co, cn) = (mesh.face_centres[f], mesh.cell_centres[o], mesh.cell_centres[nb]);
        let sfv = [sf.x, sf.y, sf.z];
        let own_d = dot3(sfv, [cf.x - co.x, cf.y - co.y, cf.z - co.z]).abs();
        let nei_d = dot3(sfv, [cn.x - cf.x, cn.y - cf.y, cn.z - cf.z]).abs();
        // basicFvGeometryScheme::makeWeights; ROOTVSMALL = 1e-150
        let wf = if (own_d + nei_d).abs() > 1.0e-150 { nei_d / (own_d + nei_d) } else { 0.5 };
        w.push(wf);
        let dv = [cn.x - co.x, cn.y - co.y, cn.z - co.z];
        dc.push(1.0 / dot3(dv, dv).sqrt());
        sfx.push(sf.x);
        sfy.push(sf.y);
        sfz.push(sf.z);
        mag_sf.push(dot3(sfv, sfv).sqrt());
    }
    Ok(Topo {
        n,
        nf,
        owner: mesh.owner[..nf].to_vec(),
        nei: mesh.neighbour[..nf].to_vec(),
        own_start,
        v: mesh.cell_volumes.to_vec(),
        cx: mesh.cell_centres.iter().map(|c| c.x).collect(),
        cy: mesh.cell_centres.iter().map(|c| c.y).collect(),
        cz: mesh.cell_centres.iter().map(|c| c.z).collect(),
        sfx,
        sfy,
        sfz,
        mag_sf,
        w,
        dc,
        bfaces,
    })
}

// ---------------------------------------------------------------- operators

/// `surfaceInterpolationScheme::interpolate(vf, lambda)` on an internal face.
fn lin(w: f64, vf: &[f64], o: usize, nb: usize) -> f64 {
    w * (vf[o] - vf[nb]) + vf[nb]
}

/// `fvc::interpolate(vf)` with the `linear` scheme, internal faces.
fn interpolate_linear(t: &Topo, vf: &[f64]) -> Vec<f64> {
    (0..t.nf).map(|f| lin(t.w[f], vf, t.owner[f], t.nei[f])).collect()
}

/// `fvc::surfaceIntegrate(ssf)`; `bflux` is the flux through every boundary
/// face in `Topo::bfaces` order.
fn surface_integrate(t: &Topo, ssf: &[f64], bflux: &[f64]) -> Vec<f64> {
    let mut r = vec![0.0; t.n];
    for f in 0..t.nf {
        r[t.owner[f]] += ssf[f];
        r[t.nei[f]] -= ssf[f];
    }
    for (b, &(c, _)) in t.bfaces.iter().enumerate() {
        r[c] += bflux[b];
    }
    for c in 0..t.n {
        r[c] /= t.v[c];
    }
    r
}

/// `fvc::grad(vf)` (`Gauss linear`, `gaussGrad::calcGrad`). Boundary face
/// value is the adjacent cell value (`zeroGradient`).
fn grad3(t: &Topo, vf: &[f64]) -> Vec<[f64; 3]> {
    let mut g = vec![[0.0_f64; 3]; t.n];
    for f in 0..t.nf {
        let ssf = lin(t.w[f], vf, t.owner[f], t.nei[f]);
        let sfssf = [t.sfx[f] * ssf, t.sfy[f] * ssf, t.sfz[f] * ssf];
        for k in 0..3 {
            g[t.owner[f]][k] += sfssf[k];
            g[t.nei[f]][k] -= sfssf[k];
        }
    }
    for &(c, sf) in &t.bfaces {
        for k in 0..3 {
            g[c][k] += sf[k] * vf[c];
        }
    }
    for c in 0..t.n {
        for k in 0..3 {
            g[c][k] /= t.v[c];
        }
    }
    g
}

/// Weights of `limitedLinear 1` for flux `flux` and field `phi`:
/// `LimitedScheme::calcLimiter` followed by
/// `limitedSurfaceInterpolationScheme::weights`.
fn limited_linear_weights(t: &Topo, flux: &[f64], phi: &[f64]) -> Vec<f64> {
    let gradc = grad3(t, phi);
    let two_by_k = 2.0 / 1.0_f64.max(SMALL);
    (0..t.nf)
        .map(|f| {
            let (o, nb) = (t.owner[f], t.nei[f]);
            let d = [t.cx[nb] - t.cx[o], t.cy[nb] - t.cy[o], t.cz[nb] - t.cz[o]];
            let gradf = phi[nb] - phi[o];
            let gradcf = if flux[f] > 0.0 { dot3(d, gradc[o]) } else { dot3(d, gradc[nb]) };
            let r = if gradcf.abs() >= 1000.0 * gradf.abs() {
                2.0 * 1000.0 * sign(gradcf) * sign(gradf) - 1.0
            } else {
                2.0 * (gradcf / gradf) - 1.0
            };
            let x = two_by_k * r;
            let lim = if x < 0.0 {
                0.0
            } else if x > 1.0 {
                1.0
            } else {
                x
            };
            lim * t.w[f] + (1.0 - lim) * pos0(flux[f])
        })
        .collect()
}

/// `upwind` weights `pos0(flux)`.
fn upwind_weights(flux: &[f64]) -> Vec<f64> {
    flux.iter().map(|&q| pos0(q)).collect()
}

/// `fvc::div(flux, vf)` with face weights `w`: `surfaceIntegrate(flux*interpolate)`;
/// `bflux` is the flux through the boundary faces, whose `zeroGradient` value of
/// `vf` is the adjacent cell value.
fn fvc_div(t: &Topo, flux: &[f64], w: &[f64], vf: &[f64], bflux: &[f64]) -> Vec<f64> {
    let ssf: Vec<f64> = (0..t.nf)
        .map(|f| flux[f] * lin(w[f], vf, t.owner[f], t.nei[f]))
        .collect();
    let bssf: Vec<f64> = t.bfaces.iter().enumerate().map(|(b, &(c, _))| bflux[b] * vf[c]).collect();
    surface_integrate(t, &ssf, &bssf)
}

/// An `fvMatrix<scalar>` (one component) with internal-face coefficients.
#[derive(Debug, Clone)]
pub struct Matrix {
    diag: Vec<f64>,
    /// `None` for a diagonal matrix (upstream: neither `lower` nor `upper` allocated).
    lower: Option<Vec<f64>>,
    upper: Option<Vec<f64>>,
    source: Vec<f64>,
    /// `internalCoeffs` of the boundary faces (`Topo::bfaces` order): added to the
    /// diagonal by `addBoundaryDiag` when the matrix is solved.
    bcoef: Vec<f64>,
}

/// `fvm::ddt(rho, vf)` (Euler): `diag = rDeltaT*rho*V`, `source = rDeltaT*rho0*vf0*V`.
fn fvm_ddt_rho(t: &Topo, r_dt: f64, rho: &[f64], rho0: &[f64], vf0: &[f64]) -> Matrix {
    Matrix {
        diag: (0..t.n).map(|c| r_dt * rho[c] * t.v[c]).collect(),
        lower: None,
        upper: None,
        source: (0..t.n).map(|c| r_dt * rho0[c] * vf0[c] * t.v[c]).collect(),
        bcoef: vec![0.0; t.bfaces.len()],
    }
}

/// `fvm::ddt(vf)` (Euler): `diag = rDeltaT*V`, `source = rDeltaT*vf0*V`.
fn fvm_ddt(t: &Topo, r_dt: f64, vf0: &[f64]) -> Matrix {
    Matrix {
        diag: (0..t.n).map(|c| r_dt * t.v[c]).collect(),
        lower: None,
        upper: None,
        source: (0..t.n).map(|c| r_dt * vf0[c] * t.v[c]).collect(),
        bcoef: vec![0.0; t.bfaces.len()],
    }
}

/// `fvm::div(flux, vf)` with weights `w` (`gaussConvectionScheme::fvmDiv`).
fn fvm_div(t: &Topo, flux: &[f64], w: &[f64], bflux: &[f64]) -> Matrix {
    let lower: Vec<f64> = (0..t.nf).map(|f| -w[f] * flux[f]).collect();
    let upper: Vec<f64> = (0..t.nf).map(|f| lower[f] + flux[f]).collect();
    let mut diag = vec![0.0; t.n];
    // lduMatrix::negSumDiag
    for f in 0..t.nf {
        diag[t.owner[f]] -= lower[f];
        diag[t.nei[f]] -= upper[f];
    }
    // internalCoeffs = patchFlux*valueInternalCoeffs, which is 1 for zeroGradient
    // (and for the fixedGradient energy patch)
    let bcoef = bflux.iter().map(|q| q * 1.0).collect();
    Matrix { diag, lower: Some(lower), upper: Some(upper), source: vec![0.0; t.n], bcoef }
}

impl Matrix {
    /// `lduMatrix::operator+=` for `this` diagonal-or-asymmetric and `a`
    /// diagonal-or-asymmetric, plus `fvMatrix::operator+=` of the source.
    fn add(&mut self, a: &Matrix) {
        for (d, x) in self.diag.iter_mut().zip(&a.diag) {
            *d += *x;
        }
        if let (Some(al), Some(au)) = (&a.lower, &a.upper) {
            match (&mut self.lower, &mut self.upper) {
                (Some(l), Some(u)) => {
                    for (x, y) in l.iter_mut().zip(al) {
                        *x += *y;
                    }
                    for (x, y) in u.iter_mut().zip(au) {
                        *x += *y;
                    }
                }
                _ => {
                    // diagonal + asymmetric: lduMatrix::operator+= copies A's coefficients
                    self.lower = Some(al.clone());
                    self.upper = Some(au.clone());
                }
            }
        }
        for (s, x) in self.source.iter_mut().zip(&a.source) {
            *s += *x;
        }
        for (s, x) in self.bcoef.iter_mut().zip(&a.bcoef) {
            *s += *x;
        }
    }

    /// The diagonal after `addBoundaryDiag`.
    fn solve_diag(&self, t: &Topo) -> Vec<f64> {
        let mut d = self.diag.clone();
        for (b, &(c, _)) in t.bfaces.iter().enumerate() {
            d[c] += self.bcoef[b];
        }
        d
    }

    /// `A - B` for `B` a symmetric matrix (`upper` only, `lower == upper`).
    fn sub_symmetric(&mut self, b_diag: &[f64], b_upper: &[f64]) {
        for (d, x) in self.diag.iter_mut().zip(b_diag) {
            *d -= *x;
        }
        if let (Some(l), Some(u)) = (&mut self.lower, &mut self.upper) {
            for (x, y) in l.iter_mut().zip(b_upper) {
                *x -= *y;
            }
            for (x, y) in u.iter_mut().zip(b_upper) {
                *x -= *y;
            }
        }
    }

    /// `fvMatrix + volField`: `source -= V*field`.
    fn add_field(&mut self, t: &Topo, field: &[f64]) {
        for c in 0..t.n {
            self.source[c] -= t.v[c] * field[c];
        }
    }
}

// ------------------------------------------------------------- linear solver

/// Outcome of one `smoothSolver` solve, the fields of upstream's
/// `SolverPerformance` that are printed in its log line.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SolveLog {
    /// Initial residual.
    pub initial: f64,
    /// Final residual.
    pub final_: f64,
    /// Number of iterations (sweeps).
    pub iterations: usize,
}

fn amul(t: &Topo, m: &Matrix, psi: &[f64]) -> Vec<f64> {
    let (lo, up) = (m.lower.as_ref().expect("asymmetric"), m.upper.as_ref().expect("asymmetric"));
    let mut ap: Vec<f64> = (0..t.n).map(|c| m.diag[c] * psi[c]).collect();
    for f in 0..t.nf {
        ap[t.nei[f]] += lo[f] * psi[t.owner[f]];
        ap[t.owner[f]] += up[f] * psi[t.nei[f]];
    }
    ap
}

fn residual(t: &Topo, m: &Matrix, psi: &[f64], source: &[f64]) -> Vec<f64> {
    let (lo, up) = (m.lower.as_ref().expect("asymmetric"), m.upper.as_ref().expect("asymmetric"));
    let mut r: Vec<f64> = (0..t.n).map(|c| source[c] - m.diag[c] * psi[c]).collect();
    for f in 0..t.nf {
        r[t.nei[f]] -= lo[f] * psi[t.owner[f]];
        r[t.owner[f]] -= up[f] * psi[t.nei[f]];
    }
    r
}

fn sum_mag(v: &[f64]) -> f64 {
    let mut s = 0.0;
    for x in v {
        s += x.abs();
    }
    s
}

/// `symGaussSeidelSmoother::smooth`, one sweep, no interfaces.
fn sym_gauss_seidel_sweep(t: &Topo, m: &Matrix, psi: &mut [f64], source: &[f64]) {
    let (lo, up) = (m.lower.as_ref().expect("asymmetric"), m.upper.as_ref().expect("asymmetric"));
    let mut b_prime = source.to_vec();
    for c in 0..t.n {
        let (f_start, f_end) = (t.own_start[c], t.own_start[c + 1]);
        let mut psii = b_prime[c];
        for f in f_start..f_end {
            psii -= up[f] * psi[t.nei[f]];
        }
        psii /= m.diag[c];
        for f in f_start..f_end {
            b_prime[t.nei[f]] -= lo[f] * psii;
        }
        psi[c] = psii;
    }
    for c in (0..t.n).rev() {
        let (f_start, f_end) = (t.own_start[c], t.own_start[c + 1]);
        let mut psii = b_prime[c];
        for f in f_start..f_end {
            psii -= up[f] * psi[t.nei[f]];
        }
        psii /= m.diag[c];
        psi[c] = psii;
    }
}

/// `smoothSolver` with `smoother symGaussSeidel`, `nSweeps 1`, `maxIter 1000`,
/// `minIter 0`, `relTol 0` at absolute tolerance `tolerance`; or
/// `diagonalSolver` if the matrix has no off-diagonal coefficients (what
/// `lduMatrix::solver::New` selects whatever the dictionary says).
fn solve_matrix(t: &Topo, m_in: &Matrix, psi: &mut [f64], tolerance: f64) -> SolveLog {
    let mut m_owned = m_in.clone();
    m_owned.diag = m_in.solve_diag(t);
    let m = &m_owned;
    if m.lower.is_none() && m.upper.is_none() {
        for c in 0..t.n {
            psi[c] = m.source[c] / m.diag[c];
        }
        return SolveLog { initial: 0.0, final_: 0.0, iterations: 0 };
    }
    let source = &m.source;
    let apsi = amul(t, m, psi);
    // normFactor: sumA * gAverage(psi)
    let (lo, up) = (m.lower.as_ref().expect("asymmetric"), m.upper.as_ref().expect("asymmetric"));
    let mut sum_a = m.diag.clone();
    for f in 0..t.nf {
        sum_a[t.nei[f]] += lo[f];
        sum_a[t.owner[f]] += up[f];
    }
    let mut s = 0.0;
    for x in psi.iter() {
        s += *x;
    }
    let avg = s / t.n as f64;
    for x in &mut sum_a {
        *x *= avg;
    }
    let mut ns = 0.0;
    for c in 0..t.n {
        ns += (apsi[c] - sum_a[c]).abs() + (source[c] - sum_a[c]).abs();
    }
    let norm_factor = ns + SOLVER_SMALL;
    let res0: Vec<f64> = (0..t.n).map(|c| source[c] - apsi[c]).collect();
    let initial = sum_mag(&res0) / norm_factor;
    let mut log = SolveLog { initial, final_: initial, iterations: 0 };
    let max_iter = 1000;
    if !(log.final_ < tolerance) {
        loop {
            sym_gauss_seidel_sweep(t, m, psi, source);
            let r = residual(t, m, psi, source);
            log.final_ = sum_mag(&r) / norm_factor;
            log.iterations += 1;
            if !(log.iterations < max_iter && !(log.final_ < tolerance)) {
                break;
            }
        }
    }
    log
}

// -------------------------------------------------------------------- state

/// Per-solve log of one time step, in the order upstream prints them.
#[derive(Debug, Clone, PartialEq)]
pub struct StepLog {
    /// `(field name, performance)` per linear solve.
    pub solves: Vec<(&'static str, SolveLog)>,
    /// `(sumLocalContErr, globalContErr)` per `compressibleContinuityErrs.H` call.
    pub continuity: Vec<(f64, f64)>,
}

/// Dumps of intermediate fields (upstream driver's `dumpStages`): `(tag, name, values)`.
pub type StageDump = Vec<(&'static str, &'static str, Vec<f64>)>;

/// sonicFoam (ESI v2506) on a 1-D closed tube. See the module documentation.
#[derive(Debug, Clone)]
pub struct SonicFoamEsi {
    topo: Topo,
    gas: PerfectGas,
    dt: f64,
    n_outer: usize,
    tolerance: f64,
    /// Density [kg/m3].
    pub rho: Vec<f64>,
    /// x velocity [m/s].
    pub ux: Vec<f64>,
    /// y velocity [m/s]: not solved (empty direction), set by `U = HbyA - rAU grad(p)`.
    uy: Vec<f64>,
    /// z velocity [m/s], as `uy`.
    uz: Vec<f64>,
    uy0: Vec<f64>,
    uz0: Vec<f64>,
    /// Pressure [Pa].
    pub p: Vec<f64>,
    /// Temperature [K].
    pub t: Vec<f64>,
    /// Specific internal energy [J/kg].
    pub e: Vec<f64>,
    /// Compressibility psi = 1/(R T).
    pub psi: Vec<f64>,
    /// Mass flux through the internal faces [kg/s].
    pub phi: Vec<f64>,
    k: Vec<f64>,
    /// Mass flux through the boundary faces (`Topo::bfaces` order).
    phi_b: Vec<f64>,
    phi0_b: Vec<f64>,
    /// Boundary density `p_b psi_b` as last assigned by `rho = thermo.rho()`.
    rho_b: Vec<f64>,
    rho0_b: Vec<f64>,
    rho0: Vec<f64>,
    ux0: Vec<f64>,
    k0: Vec<f64>,
    e0: Vec<f64>,
    p0: Vec<f64>,
    phi0: Vec<f64>,
    /// `psi.oldTime()`; `None` until upstream first asks for it (step 1, in `pEqn`).
    psi0: Option<Vec<f64>>,
    step_index: usize,
    cumulative_cont_err: f64,
    /// When set, intermediate fields of the steps up to this index are recorded.
    pub dump_stage_steps: usize,
    /// First step whose intermediate fields are recorded (default 1).
    pub dump_stage_from: usize,
    /// Recorded intermediate fields (see [`Self::dump_stage_steps`]).
    pub stages: StageDump,
}

impl SonicFoamEsi {
    /// Upstream `createFields.H` + the `hePsiThermo` constructor: from `p`, `T`
    /// and `U_x` per cell build `e = HE(p, T)`, `T = THE(e, p, T)`, `psi`,
    /// `rho = p psi`, `phi = linearInterpolate(rho U) & Sf` and `K`.
    ///
    /// # Errors
    /// Refusals of [`build_topo`]; [`AppBuilderError::UnsupportedScheme`] for a
    /// viscous gas (`mu != 0`); [`AppBuilderError::Diverged`] from the
    /// temperature iteration.
    pub fn new(
        mesh: &Arc<FvMesh>,
        gas: PerfectGas,
        mu: f64,
        dt: f64,
        p: &[f64],
        t: &[f64],
        ux: &[f64],
    ) -> Result<Self, AppBuilderError> {
        if mu != 0.0 {
            return Err(unsupported("mu", "a viscous gas needs divDevRhoReff and alphaEff, not translated"));
        }
        let topo = build_topo(mesh)?;
        let n = topo.n;
        let mut e = Vec::with_capacity(n);
        for c in 0..n {
            e.push(gas.es(p[c], t[c]));
        }
        let mut tt = t.to_vec();
        let mut psi = vec![0.0; n];
        for c in 0..n {
            tt[c] = gas.the(e[c], p[c], tt[c])?;
            psi[c] = gas.psi(tt[c]);
        }
        let rho: Vec<f64> = (0..n).map(|c| p[c] * psi[c]).collect();
        let rho_u: Vec<f64> = (0..n).map(|c| rho[c] * ux[c]).collect();
        let phi: Vec<f64> = (0..topo.nf)
            .map(|f| lin(topo.w[f], &rho_u, topo.owner[f], topo.nei[f]) * topo.sfx[f])
            .collect();
        let rho_b: Vec<f64> = topo.bfaces.iter().map(|&(c, _)| p[c] * psi[c]).collect();
        let phi_b: Vec<f64> = topo
            .bfaces
            .iter()
            .enumerate()
            .map(|(b, &(c, sf))| {
                let (rux, ruy, ruz) = (rho_b[b] * ux[c], rho_b[b] * 0.0, rho_b[b] * 0.0);
                sf[0] * rux + sf[1] * ruy + sf[2] * ruz
            })
            .collect();
        let k: Vec<f64> = (0..n).map(|c| 0.5 * (ux[c] * ux[c] + 0.0 * 0.0 + 0.0 * 0.0)).collect();
        Ok(Self {
            gas,
            dt,
            n_outer: 2,
            tolerance: 1.0e-15,
            rho0: rho.clone(),
            ux0: ux.to_vec(),
            uy: vec![0.0; n],
            uz: vec![0.0; n],
            uy0: vec![0.0; n],
            uz0: vec![0.0; n],
            k0: k.clone(),
            e0: e.clone(),
            p0: p.to_vec(),
            phi0: phi.clone(),
            psi0: None,
            rho,
            ux: ux.to_vec(),
            p: p.to_vec(),
            t: tt,
            e,
            psi,
            phi,
            k,
            phi0_b: phi_b.clone(),
            phi_b,
            rho0_b: rho_b.clone(),
            rho_b,
            topo,
            step_index: 0,
            cumulative_cont_err: 0.0,
            dump_stage_steps: 0,
            dump_stage_from: 1,
            stages: Vec::new(),
        })
    }

    /// The mesh data the translation uses.
    #[must_use]
    pub fn topo(&self) -> &Topo {
        &self.topo
    }

    /// Set `nOuterCorrectors` (tutorial: 2).
    pub fn set_n_outer_correctors(&mut self, n: usize) {
        self.n_outer = n.max(1);
    }

    fn record_matrix(&mut self, tag: &'static str, m: &Matrix) {
        self.record(tag, "diag", &m.diag.clone());
        self.record(tag, "source", &m.source.clone());
        self.record(tag, "lower", &m.lower.clone().unwrap_or_default());
        self.record(tag, "upper", &m.upper.clone().unwrap_or_default());
    }

    fn record(&mut self, tag: &'static str, name: &'static str, v: &[f64]) {
        if self.step_index >= self.dump_stage_from && self.step_index <= self.dump_stage_steps {
            self.stages.push((tag, name, v.to_vec()));
        }
    }

    /// `thermo.correct()`: `T = THE(e, p, T)`, `psi = 1/(R T)`.
    fn thermo_correct(&mut self) -> Result<(), AppBuilderError> {
        for c in 0..self.topo.n {
            self.t[c] = self.gas.the(self.e[c], self.p[c], self.t[c])?;
            self.psi[c] = self.gas.psi(self.t[c]);
        }
        Ok(())
    }

    /// `rhoEqn.H` without `fvOptions`: `solve(ddt(rho) + div(phi))`.
    fn rho_eqn(&mut self, r_dt: f64) -> SolveLog {
        let t = &self.topo;
        let mut m = fvm_ddt(t, r_dt, &self.rho0);
        let divphi = surface_integrate(t, &self.phi, &self.phi_b);
        m.add_field(t, &divphi);
        let mut rho = self.rho.clone();
        let log = solve_matrix(t, &m, &mut rho, self.tolerance);
        self.rho = rho;
        log
    }

    /// `compressibleContinuityErrs.H`.
    fn continuity_errors(&mut self) -> (f64, f64) {
        let t = &self.topo;
        let mut total_mass = 0.0;
        for c in 0..t.n {
            total_mass += t.v[c] * self.rho[c];
        }
        let mut local = 0.0;
        let mut global = 0.0;
        for c in 0..t.n {
            let d = self.rho[c] - self.p[c] * self.psi[c];
            local += t.v[c] * d.abs();
        }
        for c in 0..t.n {
            let d = self.rho[c] - self.p[c] * self.psi[c];
            global += t.v[c] * d;
        }
        let (a, b) = (local / total_mass, global / total_mass);
        self.cumulative_cont_err += b;
        (a, b)
    }

    /// Advance one time step (`while (runTime.loop())` body of `sonicFoam.C`).
    ///
    /// # Errors
    /// [`AppBuilderError::Diverged`] from the temperature iteration;
    /// [`AppBuilderError::UnsupportedScheme`] if a boundary mass flux becomes
    /// non-zero (waves reached a closed end, not translated).
    pub fn step(&mut self) -> Result<StepLog, AppBuilderError> {
        self.step_index += 1;
        let mut log = StepLog { solves: Vec::new(), continuity: Vec::new() };
        let r_dt = 1.0 / self.dt;
        let n = self.topo.n;
        // ++runTime: oldTime fields of rho, U, K, e, p, phi are the values at the
        // end of the previous step (see the module documentation of the
        // oldTime semantics); psi0 follows the rule in `pEqn`.
        if self.step_index > 1 {
            self.rho0 = self.rho.clone();
            self.ux0 = self.ux.clone();
            self.uy0 = self.uy.clone();
            self.uz0 = self.uz.clone();
            self.k0 = self.k.clone();
            self.e0 = self.e.clone();
            self.p0 = self.p.clone();
            self.phi0 = self.phi.clone();
            self.phi0_b = self.phi_b.clone();
            self.rho0_b = self.rho_b.clone();
            self.psi0 = Some(self.psi.clone());
        }

        // rhoEqn.H
        log.solves.push(("rho", self.rho_eqn(r_dt)));
        let rho_dump = self.rho.clone();
        self.record("rhoEqn", "rho", &rho_dump);

        for outer in 0..self.n_outer {
            let t = self.topo.clone();
            // ---- UEqn.H
            let w_u = upwind_weights(&self.phi);
            let mut u_eqn = fvm_ddt_rho(&t, r_dt, &self.rho, &self.rho0, &self.ux0);
            u_eqn.add(&fvm_div(&t, &self.phi, &w_u, &self.phi_b));
            self.record_matrix("UEqn:m", &u_eqn);
            // solve(UEqn == -fvc::grad(p)): source += V*(-grad p)
            let gradp = grad3(&t, &self.p);
            let mut pred = u_eqn.clone();
            for c in 0..n {
                pred.source[c] += t.v[c] * (-gradp[c][0]);
            }
            let mut ux = self.ux.clone();
            log.solves.push(("Ux", solve_matrix(&t, &pred, &mut ux, self.tolerance)));
            self.ux = ux;
            for c in 0..n {
                self.k[c] = 0.5 * (self.ux[c] * self.ux[c] + self.uy[c] * self.uy[c] + self.uz[c] * self.uz[c]);
            }
            if self.step_index == 1 && outer == 0 {
                // createK.H creates K's old-time field only `if (U.nOldTimes())`,
                // false at start-up, so K.oldTime() is first requested by
                // fvc::ddt(rho, K) in the first EEqn, as a copy of the K just set
                // by the momentum predictor: ddt(rho, K) = 0 in the first iteration.
                self.k0 = self.k.clone();
            }
            let ux_dump = self.ux.clone();
            self.record("UEqn", "Ux", &ux_dump);

            // ---- EEqn.H
            {
                let (pb, rb) = (self.phi_b.clone(), self.rho_b.clone());
                self.record("EEqn:phib", "phib", &pb);
                self.record("EEqn:rhob", "rhob", &rb);
                let w_e = limited_linear_weights(&t, &self.phi, &self.e);
                let mut e_eqn = fvm_ddt_rho(&t, r_dt, &self.rho, &self.rho0, &self.e0);
                e_eqn.add(&fvm_div(&t, &self.phi, &w_e, &self.phi_b));
                // + fvc::ddt(rho, K)
                let ddt_k: Vec<f64> = (0..n)
                    .map(|c| r_dt * (self.rho[c] * self.k[c] - self.rho0[c] * self.k0[c]))
                    .collect();
                self.record("EEqn:ddtK", "v", &ddt_k);
                e_eqn.add_field(&t, &ddt_k);
                // + fvc::div(phi, K)
                let w_k = limited_linear_weights(&t, &self.phi, &self.k);
                let div_k = fvc_div(&t, &self.phi, &w_k, &self.k, &self.phi_b);
                self.record("EEqn:divK", "v", &div_k);
                e_eqn.add_field(&t, &div_k);
                // + fvc::div(phiv, p, "v"), phiv = phi/interpolate(rho)
                let rho_f = interpolate_linear(&t, &self.rho);
                let phiv: Vec<f64> = (0..t.nf).map(|f| self.phi[f] / rho_f[f]).collect();
                let phiv_b: Vec<f64> = (0..t.bfaces.len()).map(|b| self.phi_b[b] / self.rho_b[b]).collect();
                let w_p = limited_linear_weights(&t, &phiv, &self.p);
                let div_p = fvc_div(&t, &phiv, &w_p, &self.p, &phiv_b);
                self.record("EEqn:divp", "v", &div_p);
                e_eqn.add_field(&t, &div_p);
                self.record_matrix("EEqn:m", &e_eqn);
                let mut e = self.e.clone();
                log.solves.push(("e", solve_matrix(&t, &e_eqn, &mut e, self.tolerance)));
                self.e = e;
                self.thermo_correct()?;
            }
            let (e_dump, t_dump) = (self.e.clone(), self.t.clone());
            self.record("EEqn", "e", &e_dump);
            self.record("EEqn", "T", &t_dump);

            // ---- pEqn.H (nCorrectors 1)
            {
                // rho = thermo.rho()
                for c in 0..n {
                    self.rho[c] = self.p[c] * self.psi[c];
                }
                for (b, &(c, _)) in t.bfaces.iter().enumerate() {
                    self.rho_b[b] = self.p[c] * self.psi[c];
                }
                // D() = diag + cmptAv(internalCoeffs); internalCoeffs = flux*(1,1,1)
                let mut d_diag = u_eqn.diag.clone();
                let mut b_term = vec![0.0_f64; n];
                for (b, &(c, _)) in t.bfaces.iter().enumerate() {
                    let ic = u_eqn.bcoef[b];
                    let cmpt_av = (ic + ic + ic) / 3.0;
                    d_diag[c] += cmpt_av;
                    // H(): boundaryDiag = -(internalCoeffs) + cmptAv(internalCoeffs)
                    b_term[c] = -(0.0 + ic) + cmpt_av;
                }
                let a: Vec<f64> = (0..n).map(|c| d_diag[c] / t.v[c]).collect();
                let r_au: Vec<f64> = a.iter().map(|x| 1.0 / *x).collect();
                let rho_rau: Vec<f64> = (0..n).map(|c| self.rho[c] * r_au[c]).collect();
                let rhor_auf = interpolate_linear(&t, &rho_rau);
                // UEqn.H(): -(lower*psi_l ...) + source, / V
                let (lo, up) = (
                    u_eqn.lower.as_ref().expect("asymmetric"),
                    u_eqn.upper.as_ref().expect("asymmetric"),
                );
                let mut h = vec![0.0; n];
                for f in 0..t.nf {
                    h[t.nei[f]] -= lo[f] * self.ux[t.owner[f]];
                    h[t.owner[f]] -= up[f] * self.ux[t.nei[f]];
                }
                let hbya: Vec<f64> = (0..n)
                    .map(|c| {
                        let hc = (b_term[c] * self.ux[c] + (h[c] + u_eqn.source[c])) / t.v[c];
                        r_au[c] * hc
                    })
                    .collect();
                // ddtCorr(rho, U, phi)
                let rho_u0: Vec<f64> = (0..n).map(|c| self.rho0[c] * self.ux0[c]).collect();
                let rho_u0y: Vec<f64> = (0..n).map(|c| self.rho0[c] * self.uy0[c]).collect();
                let rho_u0z: Vec<f64> = (0..n).map(|c| self.rho0[c] * self.uz0[c]).collect();
                let rho_f0 = interpolate_linear(&t, &rho_u0);
                let rho_f0y = interpolate_linear(&t, &rho_u0y);
                let rho_f0z = interpolate_linear(&t, &rho_u0z);
                let rho_f = interpolate_linear(&t, &self.rho);
                let psi_f = interpolate_linear(&t, &self.psi);
                let hbya_f = interpolate_linear(&t, &hbya);
                let mut phid = Vec::with_capacity(t.nf);
                for f in 0..t.nf {
                    let phi_corr = self.phi0[f]
                        - (t.sfx[f] * rho_f0[f] + t.sfy[f] * rho_f0y[f] + t.sfz[f] * rho_f0z[f]);
                    let m = (phi_corr.abs() / (self.phi0[f].abs() + SMALL)).min(1.0);
                    let coeff = 1.0 - m;
                    let ddt_corr = coeff * r_dt * phi_corr;
                    // flux(HbyA) = Sf & interpolate(HbyA), HbyA_y = HbyA_z = 0
                    let flux_hbya = t.sfx[f] * hbya_f[f] + t.sfy[f] * 0.0 + t.sfz[f] * 0.0;
                    phid.push(psi_f[f] * (flux_hbya + rhor_auf[f] * ddt_corr / rho_f[f]));
                }
                // boundary faces: phid_b = psi_b (Sf & HbyA_b + rhorAUf_b ddtCorr_b / rho_b)
                let mut phid_b = Vec::with_capacity(t.bfaces.len());
                let (mut dbg_flux, mut dbg_ddt, mut dbg_hb, mut dbg_rau, mut dbg_rrf) =
                    (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new());
                for (b, &(c, sf)) in t.bfaces.iter().enumerate() {
                    let flux_h = sf[0] * hbya[c] + sf[1] * 0.0 + sf[2] * 0.0;
                    let rhor_auf_b = self.rho_b[b] * r_au[c];
                    let rho_u0 = [self.rho0_b[b] * self.ux0[c], self.rho0_b[b] * self.uy0[c], self.rho0_b[b] * self.uz0[c]];
                    let phi_corr = self.phi0_b[b] - (sf[0] * rho_u0[0] + sf[1] * rho_u0[1] + sf[2] * rho_u0[2]);
                    // fvcDdtPhiCoeff zeroes the coefficient on patches whose `U` (here
                    // rhoU0 = rho0*U0, a field with `calculated` patches, which
                    // `fixesValue()`) fixes the value: the boundary ddtCorr is 0
                    let ddt_corr = 0.0 * r_dt * phi_corr;
                    phid_b.push(self.psi[c] * (flux_h + rhor_auf_b * ddt_corr / self.rho_b[b]));
                    dbg_flux.push(flux_h);
                    dbg_ddt.push(rhor_auf_b * ddt_corr / self.rho_b[b]);
                    dbg_hb.push(hbya[c]);
                    dbg_rau.push(r_au[c]);
                    dbg_rrf.push(rhor_auf_b);
                }
                self.record("pEqn:phidb", "v", &phid_b.clone());
                self.record("pEqn:fluxHbyAb", "v", &dbg_flux);
                self.record("pEqn:ddtCorrb", "v", &dbg_ddt);
                self.record("pEqn:HbyAb", "v", &dbg_hb);
                self.record("pEqn:rAUb", "v", &dbg_rau);
                let (a0, b0) = (self.phi0_b.clone(), self.rho0_b.clone());
                let c0: Vec<f64> = t.bfaces.iter().map(|&(c, _)| self.ux0[c]).collect();
                self.record("pEqn:phi0b", "v", &a0);
                self.record("pEqn:rho0b", "v", &b0);
                self.record("pEqn:U0c", "v", &c0);
                self.record("pEqn:rhorAUfb", "v", &dbg_rrf);
                self.record("pEqn:rAU", "rAU", &r_au);
                self.record("pEqn:HbyA", "Ux", &hbya);
                self.record("pEqn:phid", "phid", &phid);
                self.record("pEqn:rhorAUf", "rhorAUf", &rhor_auf);

                // fvm::ddt(psi, p) + fvm::div(phid, p) - fvm::laplacian(rhorAUf, p)
                let psi0 = match &self.psi0 {
                    Some(v) => v.clone(),
                    None => {
                        // psi.oldTime() is created on first use, here, as a copy
                        // of the current psi (after this iteration's thermo.correct())
                        self.psi0 = Some(self.psi.clone());
                        self.psi.clone()
                    }
                };
                let mut p_eqn = fvm_ddt_rho(&t, r_dt, &self.psi, &psi0, &self.p0);
                let w_pd = limited_linear_weights(&t, &phid, &self.p);
                p_eqn.add(&fvm_div(&t, &phid, &w_pd, &phid_b));
                let lap_upper: Vec<f64> =
                    (0..t.nf).map(|f| t.dc[f] * (rhor_auf[f] * t.mag_sf[f])).collect();
                let mut lap_diag = vec![0.0; n];
                for f in 0..t.nf {
                    lap_diag[t.owner[f]] -= lap_upper[f];
                    lap_diag[t.nei[f]] -= lap_upper[f];
                }
                p_eqn.sub_symmetric(&lap_diag, &lap_upper);
                self.record_matrix("pEqn:m", &p_eqn);
                let mut p = self.p.clone();
                log.solves.push(("p", solve_matrix(&t, &p_eqn, &mut p, self.tolerance)));
                self.p = p;
                // phi = pEqn.flux(): faceH
                let (lo, up) = (
                    p_eqn.lower.as_ref().expect("asymmetric"),
                    p_eqn.upper.as_ref().expect("asymmetric"),
                );
                self.phi = (0..t.nf)
                    .map(|f| up[f] * self.p[t.nei[f]] - lo[f] * self.p[t.owner[f]])
                    .collect();
                // boundary flux: internalCoeffs*p_c (boundaryCoeffs are zero)
                for (b, &(c, _)) in t.bfaces.iter().enumerate() {
                    self.phi_b[b] = p_eqn.bcoef[b] * self.p[c];
                }

                // rhoEqn.H, continuity errors
                log.solves.push(("rho", self.rho_eqn(r_dt)));
                log.continuity.push(self.continuity_errors());

                // U = HbyA - rAU*fvc::grad(p); K
                let gradp = grad3(&t, &self.p);
                for c in 0..n {
                    self.ux[c] = hbya[c] - r_au[c] * gradp[c][0];
                    self.uy[c] = 0.0 - r_au[c] * gradp[c][1];
                    self.uz[c] = 0.0 - r_au[c] * gradp[c][2];
                    self.k[c] = 0.5 * (self.ux[c] * self.ux[c] + self.uy[c] * self.uy[c] + self.uz[c] * self.uz[c]);
                }
                let (p_dump, phi_dump, u_dump) = (self.p.clone(), self.phi.clone(), self.ux.clone());
                self.record("pEqn", "p", &p_dump);
                self.record("pEqn", "phi", &phi_dump);
                self.record("pEqn", "Ux", &u_dump);
            }
        }
        // rho = thermo.rho()
        for c in 0..n {
            self.rho[c] = self.p[c] * self.psi[c];
        }
        for (b, &(c, _)) in self.topo.bfaces.iter().enumerate() {
            self.rho_b[b] = self.p[c] * self.psi[c];
        }
        Ok(log)
    }
}
