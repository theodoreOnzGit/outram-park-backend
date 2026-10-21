// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of PETIR, a component of OUTRAM PARK.
//
// PETIR is free software: you can redistribute it and/or modify it under the
// terms of the GNU General Public License as published by the Free Software
// Foundation, version 3 of the License.
//
// PETIR is distributed in the hope that it will be useful, but WITHOUT ANY
// WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE.  See the GNU General Public License for more
// details.
//
// You should have received a copy of the GNU General Public License along
// with PETIR.  If not, see <https://www.gnu.org/licenses/>.
//
// ---------------------------------------------------------------------------
// PORTED from Clawpack / PyClaw (NOT from GSL, which has no SSP method).
//
//   Project : Clawpack -- PyClaw
//   File    : src/pyclaw/sharpclaw/solver.py
//             (`SharpClawSolver.step`, the 'SSP22' / 'SSP33' / 'SSP104'
//             branches, and the helpers `ssp22` and `ssp104`;
//             file header: "Created: 2010-03-20, Author: David Ketcheson")
//   Commit  : f522337ef75abef1153e2025a204b7ef4f7c5c9f (2026-06-25),
//             https://github.com/clawpack/pyclaw, read 2026-10-16
//   Licence : BSD-3-Clause. Upstream notice, reproduced in full as source
//             redistribution requires:
//
//   Copyright (c) 1994-2018, Clawpack Developers
//   All rights reserved.
//
//   Redistribution and use in source and binary forms, with or without
//   modification, are permitted provided that the following conditions are
//   met:
//
//   * Redistributions of source code must retain the above copyright notice,
//     this list of conditions and the following disclaimer.
//
//   * Redistributions in binary form must reproduce the above copyright
//     notice, this list of conditions and the following disclaimer in the
//     documentation and/or other materials provided with the distribution.
//
//   * Neither the name of the copyright holder nor the names of its
//     contributors may be used to endorse or promote products derived from
//     this software without specific prior written permission.
//
//   THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS
//   IS" AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO,
//   THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR
//   PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR
//   CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL,
//   EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO,
//   PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR
//   PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF
//   LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING
//   NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF THIS
//   SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
//
// BSD-3-Clause flows one way into GPL-3.0-only. Do not strip this notice into
// NOTICE only, and do not contribute this file back upstream under GPL.
// Maintainer direction 2026-10-16 (decision D3, gh:#896 / gh:#903): "port an
// SSP Runge-Kutta time integrator from OpenFOAM or wherever suitable".
// OpenFOAM-10, OpenFOAM-dev and (unreachable from the build container)
// OpenFOAM.com were checked first; OpenFOAM-10 and -dev have none -- see
// the module docs.
// ---------------------------------------------------------------------------

//! Strong-stability-preserving (SSP) Runge-Kutta steps for method-of-lines
//! semi-discretisations — PyClaw's `SSP22`, `SSP33` and `SSP104`.
//!
//! # What is here
//!
//! | function | upstream name | stages | order | SSP coefficient `C` |
//! |---|---|---|---|---|
//! | [`ssp22_step`] | `'SSP22'` (Shu–Osher) | 2 | 2 | 1 |
//! | [`ssp33_step`] | `'SSP33'` (Shu–Osher) | 3 | 3 | 1 |
//! | [`ssp104_step`] | `'SSP104'` (Ketcheson 2008) | 10 | 4 | 6 |
//!
//! [`ssp_step`] dispatches on [`SspMethod`]. The `C` column is upstream's own
//! `_sspcoeff` table (`solver.py:135-141`), not a value computed here.
//!
//! # What "strong stability preserving" buys you
//!
//! If forward Euler, `q + dt*L(q)`, is total-variation diminishing (or keeps a
//! density positive, or satisfies any other convex bound) for
//! `dt <= dt_FE`, then each of these methods keeps the same property for
//! `dt <= C * dt_FE`. Every stage is a convex combination of forward-Euler
//! steps, which is what the Shu–Osher form below makes visible. For
//! first-order upwind advection `dt_FE` is the CFL-1 step, so SSP22 and SSP33
//! are TVD at CFL <= 1 and SSP104 at CFL <= 6 (≈ 0.6 per stage — the effective
//! coefficient is `C / stages`, and SSP104's 0.6 beats SSP33's 0.33).
//!
//! What it does **not** buy: SSP is a property of the time integrator *given*
//! a spatial operator whose forward-Euler step already has the property. It
//! makes a central-difference flux no more TVD than forward Euler does.
//!
//! # Method-of-lines shape
//!
//! The caller owns the state `q` and advances it in place; the right-hand side
//! is a closure `rhs(t, q, dqdt)` that writes the time derivative `L(q)` into
//! `dqdt`, the same callback signature as [`crate::ode::rk4_step`]. Stage
//! storage lives in an [`SspWorkspace`] the caller allocates once and reuses,
//! so a step allocates nothing.
//!
//! # Where the port differs from upstream, and why it is invisible at a
//! power-of-two `dt`
//!
//! PyClaw's `dq(state)` returns `dt * L(q)`, and `step` caches the first stage
//! as `dq_dt = dq(state) / dt` (`solver.py:293`) so that a step rejected by
//! its CFL controller can be retried at a new `dt` without re-evaluating it.
//! The first stage then forms `q + dt * dq_dt`, i.e. `q + dt * ((dt*L)/dt)`.
//! This port has no step-rejection controller — it is a fixed-step kernel and
//! `dt` is the caller's — so the first stage uses `L` directly:
//! `q + dt * L`. Every other stage forms `dt * L` exactly as upstream's
//! `dq` does.
//!
//! The round trip `(dt*L)/dt` is exact when `dt` is a power of two, so there
//! the two are bit-identical; otherwise they can differ in the last ulp of the
//! first stage. Both are pinned by `tests/ssprk_vs_pyclaw.rs` against
//! upstream's own `step()` run under NumPy.
//!
//! The arithmetic of every other line is upstream's, in upstream's order —
//! `0.5*(q + r + dt*L)`, not `0.5*q + 0.5*r + ...`; `(1/3)*q + (2/3)*(...)`;
//! `q/25 + 0.36*s` — because a reordering changes the rounding.
//!
//! # Why not OpenFOAM, which the maintainer named first
//!
//! OpenFOAM-10 (`src/ODE/ODESolvers`: Euler, EulerSI, RKCK45, RKDP45, RKF45,
//! Rosenbrock12/23/34, rodas23/34, seulex, SIBS, Trapezoid;
//! `finiteVolume/ddtSchemes`: Euler, backward, CrankNicolson, localEuler,
//! SLTS, bounded, CoEuler, steadyState) and OpenFOAM-dev HEAD (tree listing,
//! 2026-10-16) contain no SSP / TVD Runge–Kutta. OpenFOAM.com could not be
//! reached from the build container and is **not re-checked**. PyClaw was
//! chosen over deal.II, SUNDIALS ARKODE and PETSc TSSSP because its three
//! methods are written out as explicit array statements in one file, which
//! makes a line-for-line port reviewable, and because it could be run here to
//! produce a code-to-code reference.
//!
//! # Units
//!
//! Bare `f64`, per the crate's units policy.

use alloc::vec;
use alloc::vec::Vec;

use crate::zip::zip_flat;
use crate::{PetirError, Result};

/// Which SSP Runge–Kutta scheme [`ssp_step`] takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SspMethod {
    /// Two-stage, second order, SSP coefficient 1 (upstream `'SSP22'`).
    Ssp22,
    /// Three-stage, third order, SSP coefficient 1 (upstream `'SSP33'`).
    Ssp33,
    /// Ten-stage, fourth order, SSP coefficient 6 (upstream `'SSP104'`).
    Ssp104,
}

impl SspMethod {
    /// Upstream's SSP coefficient `C` for this method (`solver.py:135-141`):
    /// the method keeps forward Euler's strong-stability property up to
    /// `dt <= C * dt_FE`.
    #[must_use]
    pub fn ssp_coefficient(self) -> f64 {
        match self {
            SspMethod::Ssp22 | SspMethod::Ssp33 => 1.0,
            SspMethod::Ssp104 => 6.0,
        }
    }

    /// Number of right-hand-side evaluations per step.
    #[must_use]
    pub fn stages(self) -> usize {
        match self {
            SspMethod::Ssp22 => 2,
            SspMethod::Ssp33 => 3,
            SspMethod::Ssp104 => 10,
        }
    }
}

/// Stage storage for the SSP steps: one register (upstream's
/// `self._registers[0]`, all three methods need exactly one) and one buffer
/// for the right-hand side. Allocate once per system size and reuse.
#[derive(Debug, Clone, PartialEq)]
pub struct SspWorkspace {
    reg: Vec<f64>,
    k: Vec<f64>,
}

impl SspWorkspace {
    /// Workspace for a system of `n` unknowns.
    #[must_use]
    pub fn new(n: usize) -> Self {
        SspWorkspace { reg: vec![0.0; n], k: vec![0.0; n] }
    }

    /// The system size this workspace was built for.
    #[must_use]
    pub fn len(&self) -> usize {
        self.reg.len()
    }

    /// `true` for a zero-sized workspace.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.reg.is_empty()
    }

    fn check(&self, q: &[f64]) -> Result<()> {
        if q.is_empty() {
            return Err(PetirError::Invalid);
        }
        if self.reg.len() != q.len() || self.k.len() != q.len() {
            return Err(PetirError::LengthMismatch { expected: q.len(), found: self.reg.len() });
        }
        Ok(())
    }
}

/// One step of the chosen SSP scheme: advances `q` in place from `t` to
/// `t + dt`.
///
/// `rhs(t, q, dqdt)` must write `L(q)` — the time derivative, **not**
/// `dt * L` — into `dqdt`.
///
/// # Errors
///
/// [`PetirError::Invalid`] for an empty state; [`PetirError::LengthMismatch`]
/// if `work` was built for a different size.
pub fn ssp_step<F>(
    method: SspMethod,
    rhs: F,
    t: f64,
    q: &mut [f64],
    dt: f64,
    work: &mut SspWorkspace,
) -> Result<()>
where
    F: FnMut(f64, &[f64], &mut [f64]),
{
    match method {
        SspMethod::Ssp22 => ssp22_step(rhs, t, q, dt, work),
        SspMethod::Ssp33 => ssp33_step(rhs, t, q, dt, work),
        SspMethod::Ssp104 => ssp104_step(rhs, t, q, dt, work),
    }
}

/// Shu–Osher two-stage, second-order SSP step (upstream `ssp22`,
/// `solver.py:391-397`):
///
/// ```text
/// q1      = q + dt L(t, q)
/// q^{n+1} = 1/2 (q + q1 + dt L(t + dt, q1))
/// ```
///
/// # Errors
///
/// As [`ssp_step`].
///
/// # Example
///
/// ```
/// use petir::ssprk::{ssp22_step, SspWorkspace};
/// // dq/dt = -q, q(0) = 1
/// let mut q = [1.0_f64];
/// let mut w = SspWorkspace::new(1);
/// ssp22_step(|_t, q: &[f64], d: &mut [f64]| d[0] = -q[0], 0.0, &mut q, 0.125, &mut w).unwrap();
/// // one step of a 2nd-order method: 1 - h + h^2/2
/// assert_eq!(q[0], 1.0 - 0.125 + 0.125 * 0.125 / 2.0);
/// ```
pub fn ssp22_step<F>(mut rhs: F, t: f64, q: &mut [f64], dt: f64, work: &mut SspWorkspace) -> Result<()>
where
    F: FnMut(f64, &[f64], &mut [f64]),
{
    work.check(q)?;
    let SspWorkspace { reg, k } = work;

    // self.dq_dt = self.dq(state) / self.dt   (see module docs: used as L)
    rhs(t, q, k);
    // self._registers[0].q = state.q + self.dt*self.dq_dt
    for (r, &qi, &ki) in zip_flat!(reg.iter_mut(), q.iter(), k.iter()) {
        *r = qi + dt * ki;
    }
    // self._registers[0].t = state.t + self.dt
    rhs(t + dt, reg, k);
    // state.q = 0.5*(state.q + self._registers[0].q + self.dq(self._registers[0]))
    for (qi, &r, &ki) in zip_flat!(q.iter_mut(), reg.iter(), k.iter()) {
        *qi = 0.5 * (*qi + r + dt * ki);
    }
    Ok(())
}

/// Shu–Osher three-stage, third-order SSP step (upstream `'SSP33'` branch of
/// `step`, `solver.py:312-323`):
///
/// ```text
/// q1      = q + dt L(t, q)
/// q2      = 3/4 q + 1/4 (q1 + dt L(t + dt, q1))
/// q^{n+1} = 1/3 q + 2/3 (q2 + dt L(t + dt/2, q2))
/// ```
///
/// # Errors
///
/// As [`ssp_step`].
pub fn ssp33_step<F>(mut rhs: F, t: f64, q: &mut [f64], dt: f64, work: &mut SspWorkspace) -> Result<()>
where
    F: FnMut(f64, &[f64], &mut [f64]),
{
    work.check(q)?;
    let SspWorkspace { reg, k } = work;

    rhs(t, q, k);
    // self._registers[0].q = state.q + self.dt*self.dq_dt
    for (r, &qi, &ki) in zip_flat!(reg.iter_mut(), q.iter(), k.iter()) {
        *r = qi + dt * ki;
    }
    // self._registers[0].t = state.t + self.dt
    rhs(t + dt, reg, k);
    // self._registers[0].q = 0.75*state.q + 0.25*(self._registers[0].q + self.dq(self._registers[0]))
    for (r, &qi, &ki) in zip_flat!(reg.iter_mut(), q.iter(), k.iter()) {
        *r = 0.75 * qi + 0.25 * (*r + dt * ki);
    }
    // self._registers[0].t = state.t + 0.5*self.dt
    rhs(t + 0.5 * dt, reg, k);
    // state.q = 1./3.*state.q + 2./3.*(self._registers[0].q + self.dq(self._registers[0]))
    for (qi, &r, &ki) in zip_flat!(q.iter_mut(), reg.iter(), k.iter()) {
        *qi = 1.0 / 3.0 * *qi + 2.0 / 3.0 * (r + dt * ki);
    }
    Ok(())
}

/// Ketcheson's ten-stage, fourth-order SSP step, SSP coefficient 6 (upstream
/// `ssp104`, `solver.py:400-430`; D. I. Ketcheson, SIAM J. Sci. Comput. 30(4),
/// 2008, the low-storage form).
///
/// **It does not hold a steady state bit for bit.** With `L = 0`, the
/// recombination `q/25 + 0.36 s`, then `15 q - 5 s`, then `q + 0.6 s` turns
/// `1.0` into `0.9999999999999993`. Upstream does the same; the value was
/// checked against NumPy on 2026-10-16. A caller who needs an exact
/// fixed point should use [`ssp33_step`].
///
/// # Errors
///
/// As [`ssp_step`].
pub fn ssp104_step<F>(mut rhs: F, t: f64, q: &mut [f64], dt: f64, work: &mut SspWorkspace) -> Result<()>
where
    F: FnMut(f64, &[f64], &mut [f64]),
{
    work.check(q)?;
    let SspWorkspace { reg: s1, k } = work;

    rhs(t, q, k);
    // s1.q = state.q + self.dt*self.dq_dt/6.
    for (s, &qi, &ki) in zip_flat!(s1.iter_mut(), q.iter(), k.iter()) {
        *s = qi + dt * ki / 6.0;
    }
    // s1.t = state.t + self.dt/6.
    let mut ts = t + dt / 6.0;

    for _ in 0..4 {
        rhs(ts, s1, k);
        // s1.q = s1.q + self.dq(s1)/6.
        for (s, &ki) in zip_flat!(s1.iter_mut(), k.iter()) {
            *s += dt * ki / 6.0;
        }
        // s1.t = s1.t + self.dt/6.
        ts += dt / 6.0;
    }

    // state.q = state.q/25. + 0.36 * s1.q
    // s1.q = 15. * state.q - 5. * s1.q
    for (qi, s) in zip_flat!(q.iter_mut(), s1.iter_mut()) {
        *qi = *qi / 25.0 + 0.36 * *s;
        *s = 15.0 * *qi - 5.0 * *s;
    }
    // s1.t = state.t + self.dt/3.
    ts = t + dt / 3.0;

    for _ in 0..4 {
        rhs(ts, s1, k);
        for (s, &ki) in zip_flat!(s1.iter_mut(), k.iter()) {
            *s += dt * ki / 6.0;
        }
        ts += dt / 6.0;
    }

    rhs(ts, s1, k);
    // state.q += 0.6 * s1.q + 0.1 * self.dq(s1)
    for (qi, &s, &ki) in zip_flat!(q.iter_mut(), s1.iter(), k.iter()) {
        *qi += 0.6 * s + 0.1 * (dt * ki);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_and_mismatched_states_are_errors_not_panics() {
        let mut w = SspWorkspace::new(0);
        let mut q: [f64; 0] = [];
        let f = |_t: f64, _q: &[f64], _d: &mut [f64]| {};
        assert_eq!(ssp33_step(f, 0.0, &mut q, 0.1, &mut w), Err(PetirError::Invalid));
        let mut w = SspWorkspace::new(2);
        let mut q = [1.0_f64; 3];
        assert_eq!(
            ssp_step(SspMethod::Ssp104, f, 0.0, &mut q, 0.1, &mut w),
            Err(PetirError::LengthMismatch { expected: 3, found: 2 })
        );
    }

    /// Each scheme evaluates the right-hand side exactly `stages()` times, at
    /// upstream's stage times.
    #[test]
    fn stage_counts_and_times_match_upstream() {
        let mut w = SspWorkspace::new(1);
        for (m, want) in [
            (SspMethod::Ssp22, vec![0.0, 1.0]),
            (SspMethod::Ssp33, vec![0.0, 1.0, 0.5]),
        ] {
            let mut seen = Vec::new();
            let mut q = [1.0_f64];
            ssp_step(m, |t, _q: &[f64], d: &mut [f64]| { seen.push(t); d.fill(0.0); }, 0.0, &mut q, 1.0, &mut w)
                .unwrap();
            assert_eq!(seen, want);
            assert_eq!(seen.len(), m.stages());
        }
        let mut seen = Vec::new();
        let mut q = [1.0_f64];
        ssp104_step(|t, _q: &[f64], d: &mut [f64]| { seen.push(t); d.fill(0.0); }, 0.0, &mut q, 6.0, &mut w).unwrap();
        assert_eq!(seen, vec![0.0, 1.0, 2.0, 3.0, 4.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
        // A zero right-hand side does NOT leave the state bit-identical in
        // SSP104: the low-storage recombination `q/25 + 0.36 s`, `15 q - 5 s`,
        // `q + 0.6 s` rounds 1.0 to 0.9999999999999993. Upstream's NumPy gives
        // the same value (checked 2026-10-16), so this is the method's
        // arithmetic, not a port defect. SSP22 and SSP33 do preserve it.
        assert_eq!(q[0], 0.9999999999999993);
        for m in [SspMethod::Ssp22, SspMethod::Ssp33] {
            let mut q = [1.0_f64];
            ssp_step(m, |_t, _q: &[f64], d: &mut [f64]| d.fill(0.0), 0.0, &mut q, 1.0, &mut w).unwrap();
            assert_eq!(q[0], 1.0);
        }
    }
}
