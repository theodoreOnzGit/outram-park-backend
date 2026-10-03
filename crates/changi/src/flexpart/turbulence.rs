// SPDX-License-Identifier: GPL-3.0
//
// FLEXPART port — provenance
// --------------------------
// Upstream project : FLEXPART (NILU) — https://github.com/flexpart/flexpart
// Upstream version : 10.4 (2019-11-12), commit 3d7eebf
// Upstream source  : src/hanna_mod.f90, src/hanna.f90, src/hanna1.f90,
//                    src/hanna_short.f90, src/windalign.f90
// Original licence : GPL-3.0-or-later — SPDX-FileCopyrightText: FLEXPART 1998-2019
// Ported into this GPL-3.0 work; see LICENSE.flexpart and NOTICE.flexpart.

//! Boundary-layer turbulence statistics after Hanna (1982): the velocity
//! standard deviations `sigma_u`, `sigma_v`, `sigma_w`, the vertical gradient of
//! `sigma_w`, and the Lagrangian time scales `T_Lu`, `T_Lv`, `T_Lw` that drive
//! FLEXPART's Langevin equation.
//!
//! # Three variants, and why
//!
//! | Routine | Used by `advance.f90` when | Gradient it returns |
//! |---|---|---|
//! | [`hanna`] | Gaussian turbulence (`turb_option` default) | `d sigma_w / dz` |
//! | [`hanna1`] | the well-mixed scheme with the density correction | `d sigma_w^2 / dz` |
//! | [`hanna_short`] | the short inner time step, `w` only | `d sigma_w / dz` |
//!
//! Each splits on stability in the same order: **neutral** when
//! `h / |L| < 1`, else **unstable** when `L < 0`, else **stable**.
//!
//! # State is shared, exactly as upstream shares it
//!
//! Upstream keeps all of these quantities in the module `hanna_mod`, which the
//! three routines read and write and `advance.f90` reads afterwards. Two outputs
//! depend on what an *earlier* call left there, and the port preserves that
//! rather than tidying it away, because `advance.f90` relies on the call order:
//!
//! * [`hanna_short`] applies `max(10, tlu)` and `max(10, tlv)` to time scales it
//!   never computes — they are whatever the last full [`hanna`] call produced.
//! * [`hanna1`] assigns no `sigma_w` or `d sigma_w^2/dz` for an unstable layer at
//!   `zeta >= 1`, so those keep their previous values (clamped to `>= 1e-6`).
//!
//! [`HannaState`] is that module, one struct per particle; the code-to-code
//! fixture sets it to sentinels before every call so both behaviours are
//! verified, not just tolerated.
//!
//! # NaN, and Fortran `MAX`
//!
//! In an unstable layer above the mixing height (`zeta > 1.11`), `hanna` and
//! `hanna_short` take the square root of a negative number, so `sigma_w` and
//! its gradient are NaN, and upstream does not guard against it. The port
//! returns the same NaN. The floors that follow (`max(30, tlw)` etc.) then see
//! a NaN. gfortran's `MAX` returns the non-NaN operand there, and so does
//! `f64::max`, so the floors are plain `f64::max`. That is exercised by the 20
//! fixture rows where this happens: `tlw` comes out exactly `30` in both
//! codes.
//!
//! # Units
//!
//! Bare `f64` in FLEXPART's units, as throughout this module (see
//! [`super::surface_layer`]): velocities m/s, lengths m, times s.

/// The `hanna_mod` turbulence state for one particle.
///
/// Inputs the caller sets before a call: `ust`, `wst`, `ol`, `h`, `zeta`.
/// Everything else is output. Field names follow upstream so the port can be
/// read side by side with `hanna*.f90` and `advance.f90`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct HannaState {
    /// Friction velocity `u*`, m/s. Clamped to `>= 1e-4` in the neutral branch,
    /// and the clamped value is written back (upstream mutates the module).
    pub ust: f64,
    /// Convective velocity scale `w*`, m/s.
    pub wst: f64,
    /// Obukhov length `L`, m (negative = unstable).
    pub ol: f64,
    /// Mixing height `h`, m.
    pub h: f64,
    /// `z / h`, dimensionless. Set by the caller, not by these routines.
    pub zeta: f64,
    /// Along-wind velocity standard deviation, m/s.
    pub sigu: f64,
    /// Cross-wind velocity standard deviation, m/s.
    pub sigv: f64,
    /// Vertical velocity standard deviation, m/s.
    pub sigw: f64,
    /// `d sigma_w / dz`, 1/s (set by [`hanna`] and [`hanna_short`]).
    pub dsigwdz: f64,
    /// `d sigma_w^2 / dz`, m/s² (set by [`hanna1`]).
    pub dsigw2dz: f64,
    /// Lagrangian time scale for `u`, s (floor 10 s).
    pub tlu: f64,
    /// Lagrangian time scale for `v`, s (floor 10 s).
    pub tlv: f64,
    /// Lagrangian time scale for `w`, s (floor 30 s).
    pub tlw: f64,
}

impl HannaState {
    /// True when upstream treats the layer as neutral: `h / |L| < 1`.
    fn is_neutral(&self) -> bool {
        self.h / self.ol.abs() < 1.0
    }

    /// Unstable-branch `sigma_w` and its gradient, shared verbatim by
    /// `hanna.f90` and `hanna_short.f90`.
    fn unstable_sigw(&mut self) {
        let zeta = self.zeta;
        let ust2 = self.ust * self.ust;
        let wst2 = self.wst * self.wst;
        self.sigw = (1.2 * wst2 * (1.0 - 0.9 * zeta) * zeta.powf(0.66666)
            + (1.8 - 1.4 * zeta) * ust2)
            .sqrt()
            + 1.0e-2;
        self.dsigwdz = 0.5 / self.sigw / self.h
            * (-(1.4 * ust2)
                + wst2 * (0.8 * zeta.max(1.0e-3).powf(-0.33333) - 1.8 * zeta.powf(0.66666)));
    }

    /// Unstable-branch `T_Lw`, shared verbatim by all three routines.
    fn unstable_tlw(&mut self, z: f64) {
        self.tlw = if z < self.ol.abs() {
            0.1 * z / (self.sigw * (0.55 - 0.38 * (z / self.ol).abs()))
        } else if self.zeta < 0.1 {
            0.59 * z / self.sigw
        } else {
            0.15 * self.h / self.sigw * (1.0 - (-(5.0 * self.zeta)).exp())
        };
    }
}

/// Hanna (1982) turbulence for the Gaussian scheme (`hanna.f90`).
///
/// # Arguments
/// - `state` — the per-particle `hanna_mod` state; `ust`, `wst`, `ol`, `h` and
///   `zeta` must be set. Outputs are written into it.
/// - `z` — particle height above ground, m.
///
/// # Notes
/// `d sigma_w / dz` is never left at exactly zero: upstream replaces `0` by
/// `1e-10`, because `advance.f90` divides by it.
pub fn hanna(state: &mut HannaState, z: f64) {
    let s = state;
    if s.is_neutral() {
        s.ust = s.ust.max(1.0e-4);
        let corr = z / s.ust;
        s.sigu = 1.0e-2 + 2.0 * s.ust * (-3.0e-4 * corr).exp();
        s.sigw = 1.3 * s.ust * (-2.0e-4 * corr).exp();
        s.dsigwdz = -2.0e-4 * s.sigw;
        s.sigw += 1.0e-2;
        s.sigv = s.sigw;
        s.tlu = 0.5 * z / s.sigw / (1.0 + 1.5e-3 * corr);
        s.tlv = s.tlu;
        s.tlw = s.tlu;
    } else if s.ol < 0.0 {
        s.sigu = 1.0e-2 + s.ust * (12.0 - 0.5 * s.h / s.ol).powf(0.33333);
        s.sigv = s.sigu;
        s.unstable_sigw();
        s.tlu = 0.15 * s.h / s.sigu;
        s.tlv = s.tlu;
        s.unstable_tlw(z);
    } else {
        s.sigu = 1.0e-2 + 2.0 * s.ust * (1.0 - s.zeta);
        s.sigv = 1.0e-2 + 1.3 * s.ust * (1.0 - s.zeta);
        s.sigw = s.sigv;
        s.dsigwdz = -1.3 * s.ust / s.h;
        s.tlu = 0.15 * s.h / s.sigu * s.zeta.sqrt();
        s.tlv = 0.467 * s.tlu;
        s.tlw = 0.1 * s.h / s.sigw * s.zeta.powf(0.8);
    }
    s.tlu = 10.0_f64.max(s.tlu);
    s.tlv = 10.0_f64.max(s.tlv);
    s.tlw = 30.0_f64.max(s.tlw);
    if s.dsigwdz == 0.0 {
        s.dsigwdz = 1.0e-10;
    }
}

/// Hanna (1982) turbulence for the density-corrected well-mixed scheme
/// (`hanna1.f90`). Returns `d sigma_w^2 / dz` in `state.dsigw2dz` rather than
/// `d sigma_w / dz`.
///
/// # Arguments
/// As [`hanna`].
///
/// # The `zeta >= 1` carry-over
/// In an unstable layer the `sigma_w` profile is defined piecewise for
/// `zeta < 0.03`, `< 0.4`, `< 0.96` and `< 1.0`. Above that, upstream assigns
/// nothing, so `sigw` and `dsigw2dz` keep the values the state already held —
/// then `sigw` is clamped to `>= 1e-6`. Ported as is; see the module docs.
pub fn hanna1(state: &mut HannaState, z: f64) {
    let s = state;
    if s.is_neutral() {
        s.ust = s.ust.max(1.0e-4);
        s.sigu = 2.0 * s.ust * (-(3.0e-4 * z / s.ust)).exp();
        s.sigu = s.sigu.max(1.0e-5);
        s.sigv = 1.3 * s.ust * (-(2.0e-4 * z / s.ust)).exp();
        s.sigv = s.sigv.max(1.0e-5);
        s.sigw = s.sigv;
        s.dsigw2dz = -6.76e-4 * s.ust * (-(4.0e-4 * z / s.ust)).exp();
        s.tlu = 0.5 * z / s.sigw / (1.0 + 1.5e-3 * z / s.ust);
        s.tlv = s.tlu;
        s.tlw = s.tlu;
    } else if s.ol < 0.0 {
        s.sigu = s.ust * (12.0 - 0.5 * s.h / s.ol).powf(0.33333);
        s.sigu = s.sigu.max(1.0e-6);
        s.sigv = s.sigu;
        let zeta = s.zeta;
        let (wst, h, ol) = (s.wst, s.h, s.ol);
        if zeta < 0.03 {
            s.sigw = 0.96 * wst * (3.0 * zeta - ol / h).powf(0.33333);
            s.dsigw2dz = 1.8432 * wst * wst / h * (3.0 * zeta - ol / h).powf(-0.33333);
        } else if zeta < 0.4 {
            let s1 = 0.96 * (3.0 * zeta - ol / h).powf(0.33333);
            let s2 = 0.763 * zeta.powf(0.175);
            if s1 < s2 {
                s.sigw = wst * s1;
                s.dsigw2dz = 1.8432 * wst * wst / h * (3.0 * zeta - ol / h).powf(-0.33333);
            } else {
                s.sigw = wst * s2;
                s.dsigw2dz = 0.203_759 * wst * wst / h * zeta.powf(-0.65);
            }
        } else if zeta < 0.96 {
            s.sigw = 0.722 * wst * (1.0 - zeta).powf(0.207);
            s.dsigw2dz = -0.215_812 * wst * wst / h * (1.0 - zeta).powf(-0.586);
        } else if zeta < 1.00 {
            s.sigw = 0.37 * wst;
            s.dsigw2dz = 0.0;
        }
        s.sigw = s.sigw.max(1.0e-6);
        s.tlu = 0.15 * s.h / s.sigu;
        s.tlv = s.tlu;
        s.unstable_tlw(z);
    } else {
        s.sigu = 2.0 * s.ust * (1.0 - s.zeta);
        s.sigv = 1.3 * s.ust * (1.0 - s.zeta);
        s.sigu = s.sigu.max(1.0e-6);
        s.sigv = s.sigv.max(1.0e-6);
        s.sigw = s.sigv;
        s.dsigw2dz = 3.38 * s.ust * s.ust * (s.zeta - 1.0) / s.h;
        s.tlu = 0.15 * s.h / s.sigu * s.zeta.sqrt();
        s.tlv = 0.467 * s.tlu;
        s.tlw = 0.1 * s.h / s.sigw * s.zeta.powf(0.8);
    }
    s.tlu = 10.0_f64.max(s.tlu);
    s.tlv = 10.0_f64.max(s.tlv);
    s.tlw = 30.0_f64.max(s.tlw);
}

/// Vertical-only Hanna turbulence for the short inner time step
/// (`hanna_short.f90`): `sigma_w`, `d sigma_w/dz` and `T_Lw`.
///
/// # Arguments
/// As [`hanna`].
///
/// # Stale `tlu` / `tlv`
/// Upstream ends with `tlu = max(10, tlu)` and `tlv = max(10, tlv)` although it
/// computes neither, so they keep — floored — whatever the state held. The port
/// reproduces this; see the module docs.
pub fn hanna_short(state: &mut HannaState, z: f64) {
    let s = state;
    if s.is_neutral() {
        s.ust = s.ust.max(1.0e-4);
        s.sigw = 1.3 * (-(2.0e-4 * z / s.ust)).exp();
        s.dsigwdz = -2.0e-4 * s.sigw;
        s.sigw = s.sigw * s.ust + 1.0e-2;
        s.tlw = 0.5 * z / s.sigw / (1.0 + 1.5e-3 * z / s.ust);
    } else if s.ol < 0.0 {
        s.unstable_sigw();
        s.unstable_tlw(z);
    } else {
        s.sigw = 1.0e-2 + 1.3 * s.ust * (1.0 - s.zeta);
        s.dsigwdz = -1.3 * s.ust / s.h;
        s.tlw = 0.1 * s.h / s.sigw * s.zeta.powf(0.8);
    }
    s.tlu = 10.0_f64.max(s.tlu);
    s.tlv = 10.0_f64.max(s.tlv);
    s.tlw = 30.0_f64.max(s.tlw);
    if s.dsigwdz == 0.0 {
        s.dsigwdz = 1.0e-10;
    }
}

/// Rotate along-wind and cross-wind turbulent velocities into the grid's `u`
/// and `v` directions (`windalign.f90`).
///
/// # Arguments
/// - `u`, `v` — the mean wind components, m/s, which define the rotation.
/// - `ffap` — the along-wind turbulent component, m/s.
/// - `ffcp` — the cross-wind turbulent component, m/s.
///
/// # Returns
/// `(ux, vy)`, the turbulent components on the grid axes, m/s. A calm wind
/// (`|V| < 1e-30`) is floored to `1e-30` before dividing, so the direction is
/// then that of the (tiny) wind rather than undefined.
#[must_use]
pub fn windalign(u: f64, v: f64, ffap: f64, ffcp: f64) -> (f64, f64) {
    const EPS: f64 = 1.0e-30;
    let ffinv = 1.0 / (u * u + v * v).sqrt().max(EPS);
    let sinphi = v * ffinv;
    let vy1 = sinphi * ffap;
    let cosphi = u * ffinv;
    let ux1 = cosphi * ffap;
    let ux2 = -sinphi * ffcp;
    let vy2 = cosphi * ffcp;
    (ux1 + ux2, vy1 + vy2)
}
