// SPDX-License-Identifier: AGPL-3.0-only
//
// Attribution (RESEARCH_INTEGRITY_AND_PROVENANCE.md): port of EGSnrc.
//   Upstream project: EGSnrc, https://github.com/nrc-cnrc/EGSnrc
//   Upstream file:    HEN_HOUSE/src/EEMF_macros.mortran
//   Upstream commit:  bd49a47aae3938f49f4100b8e018091861408b19
//   Copyright:        (C) 2016 Victor N. Malkov, D. W. O. Rogers (contributors
//                     Reid Townson); the National Research Council of Canada
//   Upstream licence: AGPL-3.0-or-later; this port is AGPL-3.0-only.
//! `HEN_HOUSE/src/EEMF_macros.mortran` (EGSnrc, 2400 lines): the enhanced
//! electromagnetic-field transport macros of Malkov and Rogers, "Charged
//! particle transport in magnetic fields in EGSnrc", Med. Phys. 43, 4447
//! (2016). Upstream says that only the **magnetic** field is tested and
//! validated.
//!
//! **Status: first port, compared with a compiled EGSnrc (gh:#1697)**: 200
//! `tutor1` histories agree to ~1e-15 for B fields in the plate and to
//! round-off amplification in a layered vacuum-gap geometry; results and
//! method are in `docs/egsnrc_phase2_differential.md`, "EEMF (Malkov-Rogers)".
//!
//! **Scope of this port (maintainer decision, 2026-11-21).** The configuration
//! the file compiles by default: `$IntegrationAlg = 3`, `$ssFieldAlg = 2`,
//! `$orderBoundary`, `$orderAdaptive`, `$duAdaptive` true,
//! `$energyLossViaPotential` false, `$ss_em` true, `$changingEMField` false,
//! PRESTA-II (`$EMFIELD_PII` inside `msdist_pII`), the exact boundary-crossing
//! algorithm, **magnetic field only**. A non-zero electric field, PRESTA-I
//! (`$EMFIELD_PI`) and `exact_bca = false` return [`EgsError::Unsupported`]
//! from [`electr`](super::egsnrc_mortran::electr), not a result.
//!
//! | upstream (`EEMF_macros.mortran`) | here |
//! |---|---|
//! | `COMMON/EM/` (`:113-211`), with the initial values of a Fortran `COMMON` (zero) | [`EemfState`] |
//! | `$GET_EM_FIELD` (`:223`), the input-file definition | [`EemfField`], [`EemfState::get_em_field`] |
//! | `$EMFIELD_INITIATE_SET_TUSTEP` (`:267-380`) | [`EemfState::initiate_set_tustep`] |
//! | `$TUSTEP_RIST_B` (`:435-453`) | inside [`EemfState::initiate_set_tustep`] |
//! | `$EMFIELD_PII` (`:487-811`) | [`EemfState::emfield_pii`] |
//! | `$NORM_VEC_B` (`:814-854`), `$INTERACT_EM` (`:856-871`) | [`EemfState::norm_vec_b`], [`EemfState::interact_em`] |
//! | `$NORMALIZE`, `$SET_MAGNITUDE` (`:2328-2343`) | [`normalize`], [`set_magnitude`] |
//! | `$EM_FIELD_SS` (`:1083-1437`), the magnetic branch with `$ssFieldAlg = 2` | [`EemfState::em_field_ss`] |
//! | `$SS_MF_Transport`, `$SS_MF_directionsConstants`, `$CALC_SS_MAG` (`:1440-1613`) | [`EemfState::ss_mf_transport`], [`EemfState::ss_mf_directions_constants`], [`EemfState::calc_ss_mag`] |
//! | `$EMFieldInVacuum` (`:1615-1963`), the magnetic branch | [`EemfState::emfield_in_vacuum`] |
//! | `$CALC_analyticDeflection` (`:2345-2398`), `$MF_Transport` (`:1965-2021`) | [`EemfState::calc_analytic_deflection`], [`EemfState::mf_transport`] |
//! | `$ADD_WORK_EM_FIELD`, `$calculate_EchangeInEF` (`:2299-2320`) | [`EemfState::add_work_em_field`] |
//!
//! **Not ported, with the reason.**
//! - The electric-field macros (`$TUSTEP_RIST_E`, `$TUSTEP_ERIST`,
//!   `$EF_Transport`, `$EF_timeIntegral`, `$EF_setVelocities`,
//!   `$EF_setPositions`, `$SS_EF_Transport`, `$setEFPotential`,
//!   `$COMPUTE_PL_EM_LOSS`): the file says only the magnetic field is validated,
//!   and `$SS_EF_Transport` calls `$calculateDeltaT`, which no file defines.
//!   Nothing in them runs for a magnetic field alone.
//! - `$EMFIELD_PI` (PRESTA-I) and `$IntegrationAlg` 0, 1, 2: the
//!   `$IntegrationAlg` constant is 3, so the `= 0` branch of `$EMFIELD_PII` is
//!   compiled out (the generated Fortran has `IF (3 .EQ. 0)`); the `= 1` branch
//!   runs only for `tustep <= sMinP3` and is ported.
//! - The `$changingEMField` branches (`.false.`): the generated Fortran holds
//!   them as `IF (.false.)`.
//! - The branches of `$EM_FIELD_SS` after `ELSEIF(.true.)` (`:1294-1352`):
//!   `ELSEIF(.true.)` is taken whenever the chain reaches it, so the later
//!   branches (including the one that calls `$CALC_analyticDeflection`) are
//!   unreachable at run time. [`EemfState::calc_analytic_deflection`] and
//!   [`EemfState::mf_transport`] are ported and unit-tested for completeness;
//!   **no code path of the default configuration calls them** (`$MF_Transport`
//!   is defined and never expanded anywhere in the source tree).
//! - `EEMF_macros_beamnrc.mortran`: not the same algorithm plus plumbing (see
//!   `eemf_macros_beamnrc_mortran.rs`).
//!
//! **Faithfulness to the compiled build.** Three properties of the Fortran are
//! reproduced on purpose and matter to the result: (1) `COMMON/EM/` persists
//! across steps, calls and histories ([`EemfState`] lives in
//! `ElectrCounters`), and `tmax_CH` keeps its old value when a step is
//! parallel to `B`; (2) `ELECTR`'s locals `tperp` and `skindepth` are read by
//! the vacuum macros while they hold the value of the last step in a medium
//! (`electr` hoists them to function scope); (3) `$SS_MF_Transport` computes
//! the position from the **initial** position `xEMF` and the total `vstep`,
//! not segment by segment.

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;

use super::egsnrc_macros::{EgsError, Stack};
use super::egsnrc_mortran::{ElectronGeometry, Geometry, HowfarState};
use super::lit;
use petir::mathf::RealMath;
use petir::real::Real;

/// `$SL` (`:91`): speed of light, m/s.
pub const SL: f64 = lit(299_792_458.0);
/// `$ERM` (`:92`): electron rest energy for this file, MeV. **Not** `RM`
/// (`0.5109989461`): the file has its own, older value.
pub const ERM: f64 = lit(0.510_998_910);
/// `$epsilon_em` (`:96`).
pub const EPSILON_EM: f64 = lit(1e-14);
/// `$MINS_ANG` (`:54`).
pub const MINS_ANG: f64 = lit(0.05);
/// `$deltaUDefault` (`:57`).
pub const DELTA_U_DEFAULT: f64 = lit(0.02);

/// `$BCA_lin_buffer` (`:75`): `min(1e-6, skindepth)`.
#[inline]
#[must_use]
pub fn bca_lin_buffer(skindepth: f64) -> f64 {
    lit(1e-6).min(skindepth)
}

/// `$BCA_lin_dist` (`:76`): `min(1e-5, skindepth)`.
#[inline]
#[must_use]
pub fn bca_lin_dist(skindepth: f64) -> f64 {
    lit(1e-5).min(skindepth)
}

/// The field and limits the user gave (`BxIN, ByIN, BzIN`, `ExIN, EyIN, EzIN`,
/// `EMLMTIN` of `COMMON/EMF-INPUTS/`, and `ESTEPE` of `COMMON/ET-Control/`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EemfField {
    /// `ExIN`, `EyIN`, `EzIN`, V/cm. Must be zero: the electric field is not
    /// ported ([`EgsError::Unsupported`]).
    pub e_in: [f64; 3],
    /// `BxIN`, `ByIN`, `BzIN`, T.
    pub b_in: [f64; 3],
    /// `EMLMTIN` (`delta_u`, default 0.02).
    pub emlmt: f64,
    /// `estepe`: global energy-loss constraint, read by
    /// `$EMFIELD_INITIATE_SET_TUSTEP` (default `$MAX-ELOSS`, 0.25).
    pub estepe: f64,
}

impl EemfField {
    /// A magnetic field with the default limits (`EMLMTIN = 0.02`,
    /// `estepe = 0.25`).
    #[must_use]
    pub fn magnetic(b_in: [f64; 3]) -> Self {
        Self { e_in: [0.0; 3], b_in, emlmt: DELTA_U_DEFAULT, estepe: lit(0.25) }
    }

    /// `emfield_on` (`egs_utilities.mortran:934`): any component non-zero.
    #[must_use]
    pub fn active(&self) -> bool {
        self.e_in.iter().chain(self.b_in.iter()).map(|v| v * v).sum::<f64>() > 0.0
    }

    /// Check the field is one this port handles.
    ///
    /// # Errors
    ///
    /// [`EgsError::Unsupported`] for a non-zero electric field.
    pub fn check_supported(&self) -> Result<(), EgsError> {
        if self.e_in.iter().any(|v| *v != 0.0) {
            return Err(EgsError::Unsupported(
                "EEMF_macros: a non-zero electric field is not ported (upstream validates the magnetic field only)",
            ));
        }
        Ok(())
    }
}

/// `$NORMALIZE(P1, P2, P3, P4)` (`:2328`): the length, and the vector divided
/// by it unless the length is zero.
#[must_use]
pub fn normalize(x: f64, y: f64, z: f64) -> (f64, f64, f64, f64) {
    let r = Real::sqrt(x * x + y * y + z * z);
    if r.abs() > 0.0 { (x / r, y / r, z / r, r) } else { (x, y, z, r) }
}

/// `$SET_MAGNITUDE(P1, P2, P3, P4)` (`:2341`).
#[must_use]
pub fn set_magnitude(x: f64, y: f64, z: f64) -> f64 {
    Real::sqrt(x * x + y * y + z * z)
}

/// The inputs of [`EemfState::initiate_set_tustep`] that are `ELECTR` locals.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InitiateInput {
    /// `eke`, kinetic energy, MeV.
    pub eke: f64,
    /// `tperp`, as `ELECTR` holds it (stale in vacuum).
    pub tperp: f64,
    /// `skindepth`, as `ELECTR` holds it (stale in vacuum).
    pub skindepth: f64,
    /// `exact_bca`.
    pub exact_bca: bool,
}

/// What `$EMFIELD_PII` reads from `msdist_pII` (all its locals).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PiiInput {
    /// Initial direction `u0, v0, w0`.
    pub u0: [f64; 3],
    /// Initial position `x0, y0, z0`.
    pub x0: [f64; 3],
    /// `tustep`.
    pub tustep: f64,
    /// `eloss`.
    pub eloss: f64,
    /// `qel` (0 for e-, 1 for e+).
    pub qel: usize,
    /// First sub-step: `sint1, cphi1, sphi1, w1`.
    pub s1: [f64; 4],
    /// Second sub-step: `sint2, cphi2, sphi2, w2`.
    pub s2: [f64; 4],
}

/// The scattering and transport directions `$EMFIELD_PII` changes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PiiDirections {
    /// `us, vs, ws`: scattering direction.
    pub us: [f64; 3],
    /// `ut, vt, wt`: transport direction.
    pub ut: [f64; 3],
}

/// The `ELECTR` variables `$EM_FIELD_SS` and `$EMFieldInVacuum` read and write.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct EemfStepVars {
    /// `callhowfar` (cleared by the single-scattering transport).
    pub callhowfar: bool,
    /// `callmsdist`.
    pub callmsdist: bool,
    /// `dosingle`.
    pub dosingle: bool,
    /// `ustep`.
    pub ustep: f64,
    /// `tustep`.
    pub tustep: f64,
    /// `vstep`.
    pub vstep: f64,
    /// `tuss`: sampled distance to a single scattering event.
    pub tuss: f64,
    /// `tperp`.
    pub tperp: f64,
    /// `skindepth`.
    pub skindepth: f64,
    /// `irl`.
    pub irl: usize,
    /// `irnew`.
    pub irnew: usize,
    /// `idisc`.
    pub idisc: i32,
    /// `lelec` (-1 for e-, +1 for e+).
    pub lelec: i32,
    /// `x_final`, `y_final`, `z_final`.
    pub final_pos: [f64; 3],
}

/// `COMMON/EM/` (`EEMF_macros.mortran:113-211`), the magnetic subset, with a
/// Fortran `COMMON`'s initial value (zero). Lives across steps, calls and
/// histories.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct EemfState {
    /// `Bx, By, Bz` after normalisation (unit vector).
    pub b: [f64; 3],
    /// `B_amp`, T.
    pub b_amp: f64,
    /// `Ex, Ey, Ez` after normalisation (zero: the electric field is refused).
    pub e: [f64; 3],
    /// `E_amp`, V/m.
    pub e_amp: f64,
    /// `beta_em`.
    pub beta_em: f64,
    /// `beta_em2`.
    pub beta_em2: f64,
    /// `Bvi`: `B_amp * v_i`.
    pub bvi: f64,
    /// `E_total`: total energy, MeV (with this file's `ERM`).
    pub e_total: f64,
    /// `EM_const`.
    pub em_const: f64,
    /// `EM_const0`.
    pub em_const0: f64,
    /// `EM_const2`.
    pub em_const2: f64,
    /// `v_i`, m/s.
    pub v_i: f64,
    /// `v_f`.
    pub v_f: f64,
    /// `tmax_CH`: the largest condensed-history step the field allows (kept
    /// from an earlier step when `EM_const = 0`).
    pub tmax_ch: f64,
    /// `eloss_corr`.
    pub eloss_corr: f64,
    /// `eloss_corr0`.
    pub eloss_corr0: f64,
    /// `UdotE1`.
    pub udot_e1: f64,
    /// `UXBx1, UXBy1, UXBz1`.
    pub uxb1: [f64; 3],
    /// `UdotB`.
    pub udot_b: f64,
    /// `r0_em`.
    pub r0_em: f64,
    /// `r02_em`.
    pub r02_em: f64,
    /// `delta_u`.
    pub delta_u: f64,
    /// `step_rist`.
    pub step_rist: f64,
    /// `u_em, v_em, w_em`.
    pub em: [f64; 3],
    /// `s_max_pii`.
    pub s_max_pii: f64,
    /// `tperp_max_EM`.
    pub tperp_max_em: f64,
    /// `tperp_em`: the distance to the nearest boundary from the macros.
    pub tperp_em: f64,
    /// `tperp0_em`.
    pub tperp0_em: f64,
    /// `beta_em0`.
    pub beta_em0: f64,
    /// `u_perp, v_perp, w_perp`.
    pub perp: [f64; 3],
    /// `u_perp2, v_perp2, w_perp2`.
    pub perp2: [f64; 3],
    /// `u_prll, v_prll, w_prll`.
    pub prll: [f64; 3],
    /// `SSc`.
    pub ssc: f64,
    /// `sign_omega`.
    pub sign_omega: f64,
    /// `omega_em`.
    pub omega_em: f64,
    /// `alpha_em`.
    pub alpha_em: f64,
    /// `sin_a`, `cos_a`.
    pub sin_cos_a: [f64; 2],
    /// `sMinP3`.
    pub smin_p3: f64,
    /// `duAdaptivEMFactor`.
    pub du_adaptiv_em_factor: f64,
    /// `u0_em, v0_em, w0_em`: direction saved at the start of the step.
    pub u0: [f64; 3],
    /// `x0_em, y0_em, z0_em`: position saved at the start of the step.
    pub x0: [f64; 3],
    /// `uEMF, vEMF, wEMF`.
    pub u_emf: [f64; 3],
    /// `xEMF, yEMF, zEMF`.
    pub x_emf: [f64; 3],
    /// `us_em, vs_em, ws_em`.
    pub us: [f64; 3],
    /// `xs_em, ys_em, zs_em`.
    pub xs: [f64; 3],
    /// `vstep0_mag`.
    pub vstep0_mag: f64,
    /// `show_B`: 1000 after the first call.
    pub show_b: i32,
    /// `EMtransportActive`.
    pub em_transport_active: bool,
    /// `BFieldTransport`.
    pub b_field_transport: bool,
    /// `EFieldTransport`.
    pub e_field_transport: bool,
    /// `deltaTCalculated`.
    pub delta_t_calculated: bool,
    /// `thirdOrder`.
    pub third_order: bool,
    /// `EMVacuumDone`.
    pub em_vacuum_done: bool,
}

/// `$CALL-HOWFAR-IN-ELECTR` (`egsnrc.macros:2418`): `IF(callhowfar | wt(np) <= 0) call howfar`.
fn call_howfar<G: Geometry>(geometry: &mut G, stack: &Stack, v: &mut EemfStepVars) {
    if v.callhowfar || stack.wt[stack.np] <= 0.0 {
        let mut st = HowfarState { ustep: v.ustep, irnew: v.irnew, idisc: v.idisc };
        geometry.howfar(stack, &mut st);
        v.ustep = st.ustep;
        v.irnew = st.irnew;
        v.idisc = st.idisc;
    }
}

impl EemfState {
    /// `$GET_EM_FIELD(x, y, z)` (`:223`): the field of the input file (constant).
    pub fn get_em_field(&mut self, f: &EemfField) {
        self.b = f.b_in;
        self.e = f.e_in;
        self.delta_u = f.emlmt;
    }

    /// `$INTERACT_EM(P1, P2, P3)` (`:856`): the Lorentz-force change of the
    /// direction `p`, into `u_em, v_em, w_em`.
    pub fn interact_em(&mut self, p: [f64; 3]) {
        let [ex, ey, ez] = self.e;
        let [bx, by, bz] = self.b;
        self.udot_e1 = p[0] * ex + p[1] * ey + p[2] * ez;
        self.uxb1 = [p[1] * bz - p[2] * by, p[2] * bx - p[0] * bz, p[0] * by - p[1] * bx];
        self.em = [
            self.em_const * (self.e_amp * (ex - p[0] * self.udot_e1) + self.bvi * self.uxb1[0]),
            self.em_const * (self.e_amp * (ey - p[1] * self.udot_e1) + self.bvi * self.uxb1[1]),
            self.em_const * (self.e_amp * (ez - p[2] * self.udot_e1) + self.bvi * self.uxb1[2]),
        ];
    }

    /// `$NORM_VEC_B(P1, P2, P3)` (`:814`): add the change `u_em, v_em, w_em`
    /// to the part of `p` perpendicular to `B` (the parallel part is kept) and
    /// renormalise. Returns the new vector (`p` unchanged when the new
    /// perpendicular part has zero length).
    pub fn norm_vec_b(&mut self, p: [f64; 3]) -> [f64; 3] {
        let [bx, by, bz] = self.b;
        self.udot_b = p[0] * bx + p[1] * by + p[2] * bz;
        self.prll = [self.udot_b * bx, self.udot_b * by, self.udot_b * bz];
        self.perp = [p[0] - self.prll[0], p[1] - self.prll[1], p[2] - self.prll[2]];
        self.r0_em = set_magnitude(self.perp[0], self.perp[1], self.perp[2]);
        self.perp = [self.perp[0] + self.em[0], self.perp[1] + self.em[1], self.perp[2] + self.em[2]];
        self.r02_em = set_magnitude(self.perp[0], self.perp[1], self.perp[2]);
        if self.r02_em > 0.0 {
            let a = [
                self.perp[0] * self.r0_em / self.r02_em + self.prll[0],
                self.perp[1] * self.r0_em / self.r02_em + self.prll[1],
                self.perp[2] * self.r0_em / self.r02_em + self.prll[2],
            ];
            let (x, y, z, r) = normalize(a[0], a[1], a[2]);
            self.r0_em = r;
            return [x, y, z];
        }
        p
    }

    /// `$EMFIELD_INITIATE_SET_TUSTEP` (`EEMF_macros.mortran:267-380`) with
    /// `$TUSTEP_RIST_B` (`:435-453`): read the field at the particle, set the
    /// particle constants of the step and shorten `tustep` to what the field
    /// allows. `ELECTR` calls it twice (vacuum, before `tstep`/`vacdst` are
    /// set so that it cannot change the step; medium, after the skin-depth
    /// restriction).
    pub fn initiate_set_tustep(&mut self, f: &EemfField, stack: &Stack, np: usize, inp: &InitiateInput, tustep: &mut f64) {
        let InitiateInput { eke, tperp, skindepth, exact_bca } = *inp;
        self.b = [0.0; 3];
        self.b_amp = 0.0;
        self.delta_u = DELTA_U_DEFAULT;
        self.e = [0.0; 3];
        self.e_amp = 0.0;
        self.get_em_field(f);
        let (bx, by, bz, r) = normalize(self.b[0], self.b[1], self.b[2]);
        self.b = [bx, by, bz];
        self.r0_em = r;
        self.b_amp = r;
        let (ex, ey, ez, r) = normalize(self.e[0], self.e[1], self.e[2]);
        self.e = [ex, ey, ez];
        self.r0_em = r;
        self.e_amp = r;
        self.e_amp *= lit(100.0);

        self.em_transport_active = true;
        self.b_field_transport = true;
        self.e_field_transport = true;
        if self.b_amp < lit(1e-16) && self.e_amp < lit(1e-16) {
            self.em_transport_active = false;
        }
        if self.b_amp < lit(1e-16) {
            self.b_field_transport = false;
        }
        if self.e_amp < lit(1e-16) {
            self.e_field_transport = false;
        }

        self.e_total = eke + ERM;
        let q = ERM / self.e_total;
        self.beta_em = Real::sqrt(1.0 - q * q);
        self.v_i = SL * self.beta_em;
        self.beta_em2 = self.beta_em * self.beta_em;
        self.bvi = self.v_i * self.b_amp;

        self.u0 = [stack.u[np], stack.v[np], stack.w[np]];
        self.x0 = [stack.x[np], stack.y[np], stack.z[np]];

        // EM_const = 1 since the step-size restriction needs the unscaled change
        self.em_const = 1.0;
        self.interact_em(self.u0);
        self.udot_b = self.u0[0] * self.b[0] + self.u0[1] * self.b[1] + self.u0[2] * self.b[2];

        // tmax_CH: velocity lower bound and the energy-loss correction of the time
        let q = ERM / (self.e_total - f.estepe * eke);
        self.v_f = SL * Real::sqrt(1.0 - q * q);
        if self.v_f > 0.0 {
            self.eloss_corr = 0.5 * (1.0 + self.v_i / self.v_f);
        } else {
            self.eloss_corr = 1.0;
        }
        self.tperp0_em = tperp;
        self.tperp_max_em = lit(1e-8) / ((eke + ERM) * self.beta_em2);
        self.tperp_max_em = (self.eloss_corr * self.eloss_corr) * self.tperp_max_em;
        self.r0_em = Real::sqrt(self.em[0] * self.em[0] + self.em[1] * self.em[1] + self.em[2] * self.em[2]);
        self.tperp_max_em = 0.5 * self.tperp_max_em * self.r0_em;
        // the first quadratic solution
        if self.tperp_max_em.abs() > 0.0 {
            self.s_max_pii = (-1.0 + Real::sqrt(1.0 + 4.0 * self.tperp_max_em * lit(0.99) * tperp)) / (2.0 * self.tperp_max_em);
        } else {
            self.s_max_pii = lit(1e30);
        }

        // the first call only: `show_B`, and the scaling of delta_u to 1.5 T
        if self.show_b != 1000 {
            self.show_b = 1000;
            self.delta_t_calculated = false;
            // $duAdaptive
            if self.b_field_transport {
                self.du_adaptiv_em_factor = (lit(1.5) / self.b_amp).r_powf(1.0 / 5.0);
            }
        }

        // $TUSTEP_RIST_B
        if self.b_field_transport {
            self.em_const = Real::sqrt(self.em[0] * self.em[0] + self.em[1] * self.em[1] + self.em[2] * self.em[2]);
            if self.em_const > 0.0 {
                // possible if no interaction; the restriction for 1 MeV in 1.5 T, scaled
                self.r0_em = self.delta_u * self.e_total * self.beta_em2 * self.b_amp / (lit(1e-8) * self.em_const * lit(1.5));
                self.step_rist = self.r0_em * self.du_adaptiv_em_factor;
                self.smin_p3 = MINS_ANG * self.e_total * self.beta_em2 / (lit(1e-8) * self.em_const);
                self.tmax_ch = self.step_rist.min(self.s_max_pii);
            }
        }

        // the final restrictions on the step size
        if self.em_transport_active {
            let buffer = bca_lin_buffer(skindepth);
            // $ss_em is true: the `~$ss_em` branches are dead
            if tperp <= buffer {
                // very close to the boundary: limit the step to a small value
                *tustep = tustep.min(bca_lin_dist(skindepth));
            } else if *tustep <= tperp && (!exact_bca || *tustep > skindepth) {
                // the condensed-history step conditions of EGSnrc
                *tustep = tustep.min(self.tmax_ch);
                // $ssFieldAlg = 2: a step shortened below the skin depth is sent to single scattering
                *tustep = tustep.max(skindepth);
            }
            // the `$ssFieldAlg = 1` and electric-field restriction is false for a magnetic field
        }
    }

    /// `$EMFIELD_PII` (`EEMF_macros.mortran:487-811`), called at the end of
    /// `msdist_pII` after the rotation into the lab frame: bend the scattering
    /// direction `us` and the transport direction `ut` by the field, with the
    /// 3-point integration (1-point when `tustep <= sMinP3`).
    pub fn emfield_pii(&mut self, f: &EemfField, inp: &PiiInput, d: &mut PiiDirections) {
        let _ = f;
        if !self.em_transport_active {
            return;
        }
        let qsign = inp.qel as f64 * 2.0 - 1.0;
        let (tustep, eloss) = (inp.tustep, inp.eloss);
        let mut u0 = inp.u0;
        let [x0, y0, z0] = inp.x0;
        let [sint1, cphi1, sphi1, w1] = inp.s1;
        let [sint2, cphi2, sphi2, w2] = inp.s2;

        // the final velocity and the energy-loss correction for the time
        let q = ERM / (self.e_total - eloss);
        self.v_f = SL * Real::sqrt(1.0 - q * q);
        if self.v_f > 0.0 {
            self.eloss_corr0 = 0.5 * (1.0 + self.v_i / self.v_f);
        }
        self.em_const0 = (qsign * tustep * lit(1e-8)) / (self.e_total * self.beta_em2);

        // `$IntegrationAlg = 3`: the `= 1` branch runs for a short step, with B
        if tustep <= self.smin_p3 && self.b_field_transport {
            // first-order integration with the correct normalisation
            self.em_const = (qsign * tustep * self.eloss_corr0 * lit(1e-8)) / (self.e_total * self.beta_em2);
            self.interact_em(u0);
            d.ut[0] += 0.5 * self.eloss_corr0 * self.em[0];
            d.ut[1] += 0.5 * self.eloss_corr0 * self.em[1];
            d.ut[2] += 0.5 * self.eloss_corr0 * self.em[2];
            d.us = self.norm_vec_b(d.us);
            return;
        }

        // third-order (Simpson) integration
        self.beta_em0 = self.beta_em;
        self.em_const = (qsign * tustep * self.eloss_corr0 * lit(1e-8)) / (self.e_total * self.beta_em2);
        self.interact_em(u0);
        self.u0 = u0;
        let u_tmp_em = self.em;
        let mut uem = self.em;
        let mut dem = self.em;
        // account for the half step
        self.em = [0.5 * self.em[0] / self.em_const, 0.5 * self.em[1] / self.em_const, 0.5 * self.em[2] / self.em_const];
        let q = ERM / (self.e_total - 0.5 * eloss);
        self.v_f = SL * Real::sqrt(1.0 - q * q);
        if self.v_f > 0.0 {
            self.eloss_corr = 0.5 * (1.0 + self.v_i / self.v_f);
        } else {
            self.eloss_corr = 1.0;
        }
        // evaluate at a quarter energy loss
        self.em_const2 = (qsign * tustep * self.eloss_corr * lit(1e-8)) / (self.e_total * self.beta_em2);
        self.em = [self.em[0] * self.em_const2, self.em[1] * self.em_const2, self.em[2] * self.em_const2];
        // the effect of the magnetic field on u0
        if self.b_amp.abs() > lit(1e-16) {
            u0 = self.norm_vec_b(u0);
        }

        // rotate using the first scattering angle and the initial direction with the field
        let sint02 = u0[0] * u0[0] + u0[1] * u0[1];
        let mut uem1 = [sint1 * cphi1, sint1 * sphi1, w1];
        if sint02 > lit(1e-20) {
            let sint0 = Real::sqrt(sint02);
            let sint0i = 1.0 / sint0;
            let cphi0 = sint0i * u0[0];
            let sphi0 = sint0i * u0[1];
            let u2p = u0[2] * uem1[0] + sint0 * uem1[2];
            uem1[2] = u0[2] * uem1[2] - sint0 * uem1[0];
            uem1[0] = u2p * cphi0 - uem1[1] * sphi0;
            uem1[1] = u2p * sphi0 + uem1[1] * cphi0;
        } else {
            uem1[2] = u0[2] * uem1[2];
        }

        // interact the scattered direction, at the velocity of half the energy loss
        let q = ERM / (self.e_total - 0.5 * eloss);
        self.v_i = SL * Real::sqrt(1.0 - q * q);
        if self.v_i > 0.0 {
            self.bvi = self.v_i * self.b_amp;
            let r = self.v_i / SL;
            self.beta_em2 = r * r;
            self.beta_em = Real::sqrt(self.beta_em2);
            self.em_const = (qsign * tustep * self.eloss_corr0 * lit(1e-8)) / (self.e_total - 0.5 * eloss);
            self.em_const /= self.beta_em2;
            self.interact_em(uem1);
        } else {
            self.em = [0.0; 3];
        }
        // add the interaction to the total for the integral sum
        for k in 0..3 {
            uem[k] += 4.0 * self.em[k];
            dem[k] += 4.0 * self.em[k] * self.beta_em / self.beta_em0;
        }

        // the effect of the field on the first scattered direction
        self.em = [self.em[0] / self.em_const, self.em[1] / self.em_const, self.em[2] / self.em_const];
        let q = ERM / (self.e_total - eloss);
        self.v_f = SL * Real::sqrt(1.0 - q * q);
        let q = ERM / (self.e_total - 0.5 * eloss);
        self.v_i = SL * Real::sqrt(1.0 - q * q);
        if self.v_f > 0.0 {
            self.eloss_corr = 0.5 * (1.0 + self.v_i / self.v_f);
        } else {
            self.eloss_corr = 1.0;
        }
        let r = self.v_i / SL;
        self.beta_em2 = r * r;
        // evaluate at a 3/4 energy loss (the first assignment of upstream is overwritten)
        self.em_const2 = self.em_const / ((self.e_total - 0.5 * eloss) * self.beta_em2);
        self.em = [self.em[0] * self.em_const2, self.em[1] * self.em_const2, self.em[2] * self.em_const2];
        if self.b_field_transport {
            uem1 = self.norm_vec_b(uem1);
        }

        // rotate using the second scattering direction
        let sint02 = uem1[0] * uem1[0] + uem1[1] * uem1[1];
        let mut uem2 = [sint2 * cphi2, sint2 * sphi2, w2];
        if sint02 > lit(1e-20) {
            let sint0 = Real::sqrt(sint02);
            let sint0i = 1.0 / sint0;
            let cphi0 = sint0i * uem1[0];
            let sphi0 = sint0i * uem1[1];
            let u2p = uem1[2] * uem2[0] + sint0 * uem2[2];
            uem2[2] = uem1[2] * uem2[2] - sint0 * uem2[0];
            uem2[0] = u2p * cphi0 - uem2[1] * sphi0;
            uem2[1] = u2p * sphi0 + uem2[1] * cphi0;
        } else {
            uem2[2] = uem1[2] * uem2[2];
        }

        // interact this direction with the field, accounting for the energy loss
        let q = ERM / (self.e_total - eloss);
        self.v_i = SL * Real::sqrt(1.0 - q * q);
        if self.v_i > 0.0 {
            let r = self.v_i / SL;
            self.beta_em2 = r * r;
            self.bvi = self.v_i * self.b_amp;
            self.beta_em = Real::sqrt(self.beta_em2);
            self.em_const = (qsign * tustep * self.eloss_corr0 * lit(1e-8)) / (self.e_total - eloss);
            self.em_const /= self.beta_em2;
            self.interact_em(uem2);
        } else {
            self.em = [0.0; 3];
        }
        for k in 0..3 {
            uem[k] += self.em[k];
            dem[k] += self.em[k] * self.beta_em / self.beta_em0;
            dem[k] /= 6.0;
            uem[k] /= 6.0;
        }
        self.em = uem;
        // the direction of motion from the field
        if self.b_field_transport {
            d.us = self.norm_vec_b(d.us);
        }

        // before applying the change to the position, test for a step over a
        // boundary and, if so, use the first-order distance
        self.third_order = true;
        // $orderBoundary
        let x_tmp = x0 + tustep * d.ut[0] + tustep * 0.5 * self.eloss_corr0 * dem[0] - x0;
        let y_tmp = y0 + tustep * d.ut[1] + tustep * 0.5 * self.eloss_corr0 * dem[1] - y0;
        let z_tmp = z0 + tustep * d.ut[2] + tustep * 0.5 * self.eloss_corr0 * dem[2] - z0;
        self.r0_em = set_magnitude(x_tmp, y_tmp, z_tmp);
        if (self.r0_em - self.tperp0_em) > -1.0 * EPSILON_EM {
            self.third_order = false;
        }
        if self.third_order {
            // the eloss_corr is here because the time variable appears again
            for k in 0..3 {
                d.ut[k] += 0.5 * self.eloss_corr0 * dem[k];
            }
        } else {
            // the first-order position
            for k in 0..3 {
                d.ut[k] += 0.5 * self.eloss_corr0 * u_tmp_em[k] * self.em_const0 / self.em_const;
            }
        }
    }

    /// `$SS_MF_directionsConstants` (`:1551-1587`): the constants of the helix
    /// for the direction `u_EMF`.
    pub fn ss_mf_directions_constants(&mut self, lelec: i32) {
        let [bx, by, bz] = self.b;
        self.sign_omega = -1.0 * f64::from(lelec) * self.b_amp / self.b_amp.abs();
        self.omega_em = (self.b_amp * SL * SL * lit(1e-6) / self.e_total).abs();
        let q = ERM / self.e_total;
        self.beta_em = Real::sqrt(1.0 - q * q);
        self.v_i = SL * self.beta_em;
        let u = self.u_emf;
        self.udot_b = u[0] * bx + u[1] * by + u[2] * bz;
        self.prll = [self.udot_b * bx, self.udot_b * by, self.udot_b * bz];
        let p = [u[0] - self.prll[0], u[1] - self.prll[1], u[2] - self.prll[2]];
        let (px, py, pz, r) = normalize(p[0], p[1], p[2]);
        self.perp = [px, py, pz];
        self.ssc = r;
        let s = self.sign_omega;
        self.perp2 = [
            -1.0 * s * (self.perp[1] * bz - self.perp[2] * by),
            -1.0 * s * (self.perp[2] * bx - self.perp[0] * bz),
            -1.0 * s * (self.perp[0] * by - self.perp[1] * bx),
        ];
    }

    /// `$CALC_SS_MAG(P1)` (`:1589-1613`) with `vstep` the total step: the
    /// analytic helix over `p1`, into `xs`, `us`. The position uses `vstep`
    /// for the parallel displacement and `xEMF` as the start, as upstream.
    pub fn calc_ss_mag(&mut self, p1: f64, vstep: f64) {
        self.alpha_em = self.omega_em * p1 / (lit(100.0) * self.v_i);
        let sin_a = self.alpha_em.r_sin();
        let cos_a = self.alpha_em.r_cos();
        self.sin_cos_a = [sin_a, cos_a];
        let c = lit(100.0) * self.ssc * self.v_i;
        let mut xs = [0.0; 3];
        for k in 0..3 {
            xs[k] = c * (self.perp[k] * sin_a + self.perp2[k] * (1.0 - cos_a)) / self.omega_em;
        }
        for k in 0..3 {
            xs[k] = self.x_emf[k] + vstep * self.prll[k] + xs[k];
        }
        self.xs = xs;
        for k in 0..3 {
            self.us[k] = self.prll[k] + self.ssc * (self.perp2[k] * sin_a + self.perp[k] * cos_a);
        }
    }

    /// `$SS_MF_Transport(P1, P2, P3)` (`:1440-1489`) with `$changingEMField`
    /// false: for a magnetic field, `P3 = 1` first computes the constants for
    /// the direction `u_EMF`, then the helix over the total step `vstep`
    /// (`P2`). `P1` is unused for a constant field.
    pub fn ss_mf_transport(&mut self, vstep: f64, p3: f64, lelec: i32) {
        if self.b_amp.abs() > 0.0 {
            if p3 == 1.0 {
                self.ss_mf_directions_constants(lelec);
            }
            self.calc_ss_mag(vstep, vstep);
        } else {
            // $EMF_linearTransport
            self.us = self.u_emf;
            for k in 0..3 {
                self.xs[k] = self.u_emf[k] * vstep + self.xs[k];
            }
        }
    }

    /// `$CALC_analyticDeflection(P1)` (`:2345-2398`): the deflection of the
    /// direction `u_EMF` over a distance `p1`: `u_tmp` (the displacement
    /// relative to a straight line) and `u_em` (the change of direction).
    /// Returns `u_tmp, v_tmp, w_tmp`. Not reached by the default
    /// configuration (see the module notes).
    pub fn calc_analytic_deflection(&mut self, p1: f64, lelec: i32) -> [f64; 3] {
        let [bx, by, bz] = self.b;
        self.sign_omega = -1.0 * f64::from(lelec) * self.b_amp / self.b_amp.abs();
        let u = self.u_emf;
        self.udot_b = u[0] * bx + u[1] * by + u[2] * bz;
        self.prll = [self.udot_b * bx, self.udot_b * by, self.udot_b * bz];
        let p = [u[0] - self.prll[0], u[1] - self.prll[1], u[2] - self.prll[2]];
        let (px, py, pz, r) = normalize(p[0], p[1], p[2]);
        self.perp = [px, py, pz];
        self.ssc = r;
        let s = self.sign_omega;
        self.perp2 = [
            -1.0 * s * (self.perp[1] * bz - self.perp[2] * by),
            -1.0 * s * (self.perp[2] * bx - self.perp[0] * bz),
            -1.0 * s * (self.perp[0] * by - self.perp[1] * bx),
        ];
        self.omega_em = (self.b_amp * SL * SL * lit(1e-6) / self.e_total).abs();
        let q = ERM / self.e_total;
        self.beta_em = Real::sqrt(1.0 - q * q);
        self.v_i = SL * self.beta_em;
        self.alpha_em = self.omega_em * p1 / (lit(100.0) * self.v_i);
        let sin_a = self.alpha_em.r_sin();
        let cos_a = self.alpha_em.r_cos();
        self.sin_cos_a = [sin_a, cos_a];
        self.ssc *= lit(100.0);
        let mut t = [0.0; 3];
        for k in 0..3 {
            t[k] = self.ssc * self.v_i * (self.perp[k] * sin_a + self.perp2[k] * (1.0 - cos_a)) / self.omega_em;
        }
        for k in 0..3 {
            t[k] = -p1 * self.perp[k] * self.ssc / lit(100.0) + t[k];
        }
        self.ssc /= lit(100.0);
        for k in 0..3 {
            self.em[k] = self.ssc * (self.perp2[k] * sin_a + self.perp[k] * (cos_a - 1.0));
        }
        t
    }

    /// `$MF_Transport(P1, P2)` (`:1965-2021`): after a straight-line move of
    /// `p1` along `u_EMF`, the helix correction. `p2 = 0` leaves the position,
    /// `1` sets it without a check, `2` sets it after a boundary check. Defined
    /// upstream and never expanded; ported for completeness.
    pub fn mf_transport<G: ElectronGeometry>(&mut self, geometry: &mut G, stack: &mut Stack, p1: f64, p2: f64, lelec: i32) {
        let np = stack.np;
        let t = self.calc_analytic_deflection(p1, lelec);
        if p2 == 2.0 {
            let s_em = set_magnitude(t[0], t[1], t[2]);
            self.xs = [stack.x[np], stack.y[np], stack.z[np]];
            stack.x[np] = self.x_emf[0];
            stack.y[np] = self.x_emf[1];
            stack.z[np] = self.x_emf[2];
            self.tperp_em = geometry.hownear(stack);
            stack.x[np] = self.xs[0];
            stack.y[np] = self.xs[1];
            stack.z[np] = self.xs[2];
            if s_em < self.tperp_em && self.tperp_em > EPSILON_EM {
                self.xs = [self.x_emf[0] + t[0], self.x_emf[1] + t[1], self.x_emf[2] + t[2]];
            } else {
                self.xs = self.x_emf;
            }
        } else if p2 == 1.0 {
            self.xs = [self.x_emf[0] + t[0], self.x_emf[1] + t[1], self.x_emf[2] + t[2]];
        }
        self.us = [self.u_emf[0] + self.em[0], self.u_emf[1] + self.em[1], self.u_emf[2] + self.em[2]];
    }

    /// `$ADD_WORK_EM_FIELD` with `$calculate_EchangeInEF` (`:2299-2320`): the
    /// energy change over the step from the electric field, to add to `de`.
    /// `final_pos` is `x_final, y_final, z_final` as `ELECTR` holds them at
    /// that point. Zero for a magnetic field alone (`E_amp = 0`).
    #[must_use]
    pub fn add_work_em_field(&self, lelec: i32, final_pos: [f64; 3]) -> f64 {
        let [ex, ey, ez] = self.e;
        let mut de_em = ex * (self.x0[0] - final_pos[0]) + ey * (self.x0[1] - final_pos[1]) + ez * (self.x0[2] - final_pos[2]);
        de_em = f64::from(lelec) * self.e_amp * lit(1e-8) * de_em;
        de_em
    }

    /// `$EM_FIELD_SS` (`:1083-1437`): single-scatter transport in a magnetic
    /// field with the analytic helix (`$ssFieldAlg = 2`); only called when
    /// `callmsdist` is false. Sets `v.final_pos` (and the direction on the
    /// stack); with `callhowfar` it walks to the boundary in steps of the
    /// distance to the nearest boundary (the `:BCA4_tperpCheck:` loop).
    pub fn em_field_ss<G: ElectronGeometry>(&mut self, stack: &mut Stack, geometry: &mut G, v: &mut EemfStepVars) {
        let np = stack.np;
        if !self.b_field_transport {
            // no field: linear transport
            v.final_pos = [self.x0[0] + self.u0[0] * v.vstep, self.x0[1] + self.u0[1] * v.vstep, self.x0[2] + self.u0[2] * v.vstep];
            return;
        }
        let buffer = bca_lin_buffer(v.skindepth);
        let dist = bca_lin_dist(v.skindepth);
        if v.vstep <= dist && v.tperp <= buffer {
            // linear transport near the boundary
            v.final_pos = [self.x0[0] + self.u0[0] * v.vstep, self.x0[1] + self.u0[1] * v.vstep, self.x0[2] + self.u0[2] * v.vstep];
            return;
        }
        // $ssFieldAlg = 2: save the initial step parameters
        self.em_const0 = v.vstep;
        self.r02_em = v.tustep;
        // limit the step size if howfar is called
        if v.callhowfar {
            v.vstep = v.tperp;
        }
        self.u_emf = [stack.u[np], stack.v[np], stack.w[np]];
        self.x_emf = [stack.x[np], stack.y[np], stack.z[np]];
        self.ss_mf_transport(v.vstep, 1.0, v.lelec);
        v.final_pos = self.xs;
        stack.u[np] = self.us[0];
        stack.v[np] = self.us[1];
        stack.w[np] = self.us[2];

        // the step is over, unless howfar needs to be called
        if v.callhowfar {
            v.irl = stack.ir[np] as usize;
            v.irnew = v.irl;
            // :BCA4_tperpCheck:
            loop {
                stack.x[np] = self.xs[0];
                stack.y[np] = self.xs[1];
                stack.z[np] = self.xs[2];
                self.tperp_em = geometry.hownear(stack);
                if (v.vstep + self.tperp_em) >= v.tustep {
                    // tustep is reached and we cannot go further: no boundary is crossed
                    v.dosingle = true;
                    if v.tustep < v.tuss {
                        v.dosingle = false;
                    }
                    v.vstep = v.tustep;
                    self.ss_mf_transport(v.vstep, 0.0, v.lelec);
                    v.final_pos = self.xs;
                    stack.u[np] = self.us[0];
                    stack.v[np] = self.us[1];
                    stack.w[np] = self.us[2];
                } else if self.tperp_em <= buffer {
                    // a small jump to the boundary, inside the linear buffer
                    v.dosingle = false;
                    v.ustep = v.tustep - v.vstep;
                    stack.x[np] = self.xs[0];
                    stack.y[np] = self.xs[1];
                    stack.z[np] = self.xs[2];
                    call_howfar(geometry, stack, v);
                    stack.x[np] = self.x0[0];
                    stack.y[np] = self.x0[1];
                    stack.z[np] = self.x0[2];
                    if (v.vstep + v.ustep) <= v.tustep && v.ustep <= dist {
                        // a linear transport; the regional change stands
                        v.final_pos[0] += stack.u[np] * v.ustep;
                        v.final_pos[1] += stack.v[np] * v.ustep;
                        v.final_pos[2] += stack.w[np] * v.ustep;
                        v.vstep += v.ustep;
                    } else {
                        // `ELSEIF(.true.)`: no regional change; the branches after it are unreachable
                        v.irl = stack.ir[np] as usize;
                        v.irnew = v.irl;
                    }
                } else {
                    v.vstep += self.tperp_em;
                    self.ss_mf_transport(v.vstep, 0.0, v.lelec);
                    v.final_pos = self.xs;
                    stack.u[np] = self.us[0];
                    stack.v[np] = self.us[1];
                    stack.w[np] = self.us[2];
                    continue;
                }
                break;
            }
            v.callhowfar = false;
            v.ustep = v.vstep;
            v.tustep = v.vstep;
            // return the original position
            stack.x[np] = self.x0[0];
            stack.y[np] = self.x0[1];
            stack.z[np] = self.x0[2];
        }
    }

    /// `$EMFieldInVacuum` (`:1615-1963`): a step in vacuum in a magnetic
    /// field. A short linear step near a boundary (inside the linear buffer),
    /// otherwise helix segments of the distance to the nearest boundary until
    /// the buffer, then a `howfar` to the boundary. Sets `v.vstep`, `v.ustep`
    /// and `dnear`, moves the particle and rotates its direction. `v.tperp`
    /// and `v.skindepth` are the (possibly stale) `ELECTR` locals.
    pub fn emfield_in_vacuum<G: ElectronGeometry>(&mut self, stack: &mut Stack, geometry: &mut G, v: &mut EemfStepVars) {
        let np = stack.np;
        let buffer = bca_lin_buffer(v.skindepth);
        let dist = bca_lin_dist(v.skindepth);
        if !self.em_transport_active {
            // a step in vacuum without a field
            v.vstep = v.ustep;
            stack.x[np] += stack.u[np] * v.vstep;
            stack.y[np] += stack.v[np] * v.vstep;
            stack.z[np] += stack.w[np] * v.vstep;
            stack.dnear[np] -= v.vstep;
            return;
        }
        if (v.ustep <= dist && v.tperp <= buffer) && v.irl != stack.ir[np] as usize {
            // a short step near the boundary that moves the particle into the next region
            stack.x[np] += self.u0[0] * v.ustep;
            stack.y[np] += self.u0[1] * v.ustep;
            stack.z[np] += self.u0[2] * v.ustep;
            v.vstep = v.ustep;
        } else {
            // the vacuum EM transport is not complete yet
            self.em_vacuum_done = false;
            v.vstep = 0.0;
            if v.tperp <= buffer {
                stack.ir[np] = v.irl as i32;
                // close to the boundary: a short linear step
                v.vstep = v.ustep.min(dist);
                v.final_pos = [stack.x[np] + stack.u[np] * v.vstep, stack.y[np] + stack.v[np] * v.vstep, stack.z[np] + stack.w[np] * v.vstep];
                // :EMFieldVacuumHFCheck_linear:
                loop {
                    self.tperp_em = geometry.hownear(stack);
                    if self.tperp_em <= buffer {
                        if v.vstep == dist {
                            // still in the linear buffer: one large howfar check
                            v.ustep = lit(1e20);
                            call_howfar(geometry, stack, v);
                        }
                        if v.ustep > dist {
                            v.ustep -= dist;
                            v.vstep += dist;
                            v.final_pos[0] += stack.u[np] * dist;
                            v.final_pos[1] += stack.v[np] * dist;
                            v.final_pos[2] += stack.w[np] * dist;
                            continue;
                        }
                        v.vstep += v.ustep;
                        v.final_pos[0] += stack.u[np] * v.ustep;
                        v.final_pos[1] += stack.v[np] * v.ustep;
                        v.final_pos[2] += stack.w[np] * v.ustep;
                        // transport in vacuum is finished: we reached a boundary
                        self.em_vacuum_done = true;
                    }
                    break;
                }
                // the electric-field energy change is zero here
                self.xs = v.final_pos;
                stack.x[np] = v.final_pos[0];
                stack.y[np] = v.final_pos[1];
                stack.z[np] = v.final_pos[2];
                self.us = [stack.u[np], stack.v[np], stack.w[np]];
            }

            // continue with the field transport only if a boundary was not reached
            if !self.em_vacuum_done {
                self.vstep0_mag = v.vstep;
                self.xs = [stack.x[np], stack.y[np], stack.z[np]];
                self.us = [stack.u[np], stack.v[np], stack.w[np]];
                // initialise the B-field transport (the field is on, or the step is linear)
                if self.b_field_transport {
                    self.u_emf = self.us;
                    self.x_emf = self.xs;
                    self.ss_mf_directions_constants(v.lelec);
                }
                // :EMFieldVacuumHFCheck:
                loop {
                    stack.x[np] = self.xs[0];
                    stack.y[np] = self.xs[1];
                    stack.z[np] = self.xs[2];
                    self.tperp_em = geometry.hownear(stack);
                    if self.tperp_em <= buffer {
                        // within the linear buffer: the distance to the boundary
                        v.ustep = lit(1e20);
                        stack.x[np] = self.xs[0];
                        stack.y[np] = self.xs[1];
                        stack.z[np] = self.xs[2];
                        stack.u[np] = self.us[0];
                        stack.v[np] = self.us[1];
                        stack.w[np] = self.us[2];
                        call_howfar(geometry, stack, v);
                        if v.ustep <= dist {
                            self.xs[0] += v.ustep * self.us[0];
                            self.xs[1] += v.ustep * self.us[1];
                            self.xs[2] += v.ustep * self.us[2];
                            v.vstep += v.ustep;
                            // transport ends here
                        } else {
                            // a step of BCA_lin_dist
                            self.xs[0] += dist * self.us[0];
                            self.xs[1] += dist * self.us[1];
                            self.xs[2] += dist * self.us[2];
                            v.vstep += dist;
                            if self.b_field_transport {
                                self.u_emf = self.us;
                                self.x_emf = self.xs;
                                self.ss_mf_directions_constants(v.lelec);
                                self.vstep0_mag = v.vstep;
                            }
                            // no transition from this step
                            v.irl = stack.ir[np] as usize;
                            continue;
                        }
                    } else {
                        // vacuum B-field transport by the distance to the nearest boundary
                        v.vstep += self.tperp_em - self.vstep0_mag;
                        self.ss_mf_transport(v.vstep, 2.0, v.lelec);
                        v.vstep += self.vstep0_mag;
                        continue;
                    }
                    break;
                }
                // the position and the direction after the transport
                stack.x[np] = self.xs[0];
                stack.y[np] = self.xs[1];
                stack.z[np] = self.xs[2];
                stack.u[np] = self.us[0];
                stack.v[np] = self.us[1];
                stack.w[np] = self.us[2];
            }
        }
        v.ustep = v.vstep;
        stack.dnear[np] -= v.vstep;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A stack with one electron.
    fn one_electron(pos: [f64; 3], dir: [f64; 3], region: i32) -> Stack {
        let mut s = Stack::new(5);
        s.np = 1;
        s.x[1] = pos[0];
        s.y[1] = pos[1];
        s.z[1] = pos[2];
        s.u[1] = dir[0];
        s.v[1] = dir[1];
        s.w[1] = dir[2];
        s.ir[1] = region;
        s.iq[1] = -1;
        s.e[1] = 5.0 + 0.5109989461;
        s.wt[1] = 1.0;
        s
    }

    /// Vacuum in `z > 0` bounded by planes at `z = 0` (region 1, back) and
    /// `z = 1` (region 3, front); the particle is in region 2 (vacuum).
    struct Gap;

    impl Geometry for Gap {
        fn howfar(&mut self, stack: &Stack, st: &mut HowfarState) {
            let (z, w) = (stack.z[stack.np], stack.w[stack.np]);
            if w > 0.0 {
                let tval = (1.0 - z) / w;
                if tval <= st.ustep {
                    st.ustep = tval;
                    st.irnew = 3;
                }
            } else if w < 0.0 {
                let tval = -z / w;
                if tval <= st.ustep {
                    st.ustep = tval;
                    st.irnew = 1;
                }
            }
        }
    }

    impl ElectronGeometry for Gap {
        fn hownear(&mut self, stack: &Stack) -> f64 {
            let z = stack.z[stack.np];
            z.min(1.0 - z)
        }
    }

    /// An active state for a 1 T field along x, a 5 MeV electron.
    fn started(b: [f64; 3]) -> (EemfField, EemfState, Stack) {
        let f = EemfField::magnetic(b);
        let st = one_electron([0.0, 0.0, 0.5], [0.0, 0.0, 1.0], 2);
        let mut s = EemfState::default();
        let mut tustep = 1.0;
        s.initiate_set_tustep(&f, &st, 1, &InitiateInput { eke: 5.0, tperp: 0.5, skindepth: 1.0e-3, exact_bca: true }, &mut tustep);
        (f, s, st)
    }

    /// `$NORMALIZE`, `$SET_MAGNITUDE`: a unit vector comes out of any non-zero
    /// vector with its length reported; the zero vector is left alone.
    #[test]
    fn normalize_and_magnitude() {
        let (x, y, z, r) = normalize(3.0, 0.0, 4.0);
        assert_eq!(r, 5.0);
        assert!((x * x + y * y + z * z - 1.0).abs() < 1e-15);
        assert_eq!(set_magnitude(3.0, 0.0, 4.0), 5.0);
        assert_eq!(normalize(0.0, 0.0, 0.0), (0.0, 0.0, 0.0, 0.0));
    }

    /// The field is refused when electric, and `emfield_on` follows any component.
    #[test]
    fn field_checks() {
        assert!(EemfField::magnetic([0.0, 0.5, 0.8]).active());
        assert!(!EemfField::magnetic([0.0; 3]).active());
        assert!(EemfField::magnetic([0.0, 0.5, 0.8]).check_supported().is_ok());
        let mut e = EemfField::magnetic([0.0, 0.5, 0.8]);
        e.e_in = [1.0, 0.0, 0.0];
        assert!(matches!(e.check_supported(), Err(EgsError::Unsupported(_))));
    }

    /// `$INITIATE`: sets the unit field vector, `B_amp`, the particle constants
    /// of the step and a finite step limit for a perpendicular direction; the
    /// limit is kept (not recomputed) for a direction parallel to `B`.
    #[test]
    fn initiate_sets_the_step_limit_and_keeps_it_for_a_parallel_step() {
        let (f, mut s, st) = started([0.0, 0.0, 2.0]);
        assert_eq!(s.b_amp, 2.0);
        assert_eq!(s.b, [0.0, 0.0, 1.0]);
        assert!(s.em_transport_active && s.b_field_transport && !s.e_field_transport);
        assert_eq!(s.show_b, 1000);
        // direction (0,0,1) is parallel to B = (0,0,1): EM_const = 0, tmax_CH untouched (0, a COMMON's initial value)
        assert_eq!(s.em_const, 0.0);
        assert_eq!(s.tmax_ch, 0.0);
        // a perpendicular direction gives a finite positive limit
        let mut st2 = st.clone();
        st2.u[1] = 1.0;
        st2.w[1] = 0.0;
        let mut tustep = 0.4; // inside tperp: a condensed-history step
        s.initiate_set_tustep(&f, &st2, 1, &InitiateInput { eke: 5.0, tperp: 0.5, skindepth: 1.0e-3, exact_bca: true }, &mut tustep);
        assert!(s.tmax_ch > 0.0 && s.tmax_ch.is_finite());
        assert!(tustep <= 0.4 && tustep <= s.tmax_ch.max(1.0e-3));
        // and a parallel step afterwards keeps that limit
        let keep = s.tmax_ch;
        let mut tustep = 10.0;
        s.initiate_set_tustep(&f, &st, 1, &InitiateInput { eke: 5.0, tperp: 0.5, skindepth: 1.0e-3, exact_bca: true }, &mut tustep);
        assert_eq!(s.tmax_ch, keep);
    }

    /// `$NORM_VEC_B`: the parallel component is kept and the result is a unit vector.
    #[test]
    fn norm_vec_b_keeps_the_parallel_part_and_unit_length() {
        let (_, mut s, _) = started([0.0, 0.0, 1.0]);
        s.em = [1.0e-3, -2.0e-3, 5.0e-4];
        let p = [0.6, 0.0, 0.8];
        let r = s.norm_vec_b(p);
        let len = (r[0] * r[0] + r[1] * r[1] + r[2] * r[2]).sqrt();
        assert!((len - 1.0).abs() < 1e-15, "{len}");
        // B = z: the parallel part is p.z, kept; r.z = parallel part + perp.z (perp.z = 0), then renormalised
        assert!((r[2] - 0.8).abs() < 1e-3);
        // a zero change leaves the vector unchanged (up to the renormalisation)
        s.em = [0.0; 3];
        let r0 = s.norm_vec_b(p);
        for k in 0..3 {
            assert!((r0[k] - p[k]).abs() < 1e-15);
        }
    }

    /// `$INTERACT_EM`: the change is the force `v x B` scaled, perpendicular to both.
    #[test]
    fn interact_em_is_perpendicular_to_direction_and_field() {
        let (_, mut s, _) = started([1.0, 0.0, 0.0]);
        s.em_const = 1.0;
        s.interact_em([0.0, 0.0, 1.0]);
        let dot_u = s.em[2];
        let dot_b = s.em[0];
        assert!(dot_u.abs() < 1e-30 && dot_b.abs() < 1e-30, "{:?}", s.em);
        assert!(s.em[1].abs() > 0.0);
    }

    /// `$CALC_SS_MAG`: the helix turns the direction through `alpha`, keeps the
    /// length of the direction (1), the parallel component, and the chord of the
    /// displacement is `2 R sin(alpha/2)` for a perpendicular start.
    #[test]
    fn calc_ss_mag_follows_the_analytic_helix() {
        let (_, mut s, st) = started([1.0, 0.0, 0.0]);
        s.u_emf = [0.0, 0.0, 1.0];
        s.x_emf = [0.0, 0.0, 0.0];
        let _ = st;
        s.ss_mf_transport(0.2, 1.0, -1);
        let u = s.us;
        assert!(((u[0] * u[0] + u[1] * u[1] + u[2] * u[2]) - 1.0).abs() < 1e-14);
        assert!(u[0].abs() < 1e-14, "no change parallel to B");
        // radius in cm: v_i/omega * 100
        let r = s.v_i / s.omega_em * 100.0;
        let alpha = 0.2 / r;
        assert!((s.alpha_em - alpha).abs() / alpha < 1e-14);
        let chord = (s.xs[0] * s.xs[0] + s.xs[1] * s.xs[1] + s.xs[2] * s.xs[2]).sqrt();
        assert!((chord - 2.0 * r * (alpha / 2.0).sin()).abs() / chord < 1e-12, "{chord}");
        // the direction turned by alpha: u.w = cos(alpha)
        assert!((u[2] - alpha.cos()).abs() < 1e-14);
    }

    /// `$CALC_analyticDeflection` and `$MF_Transport` (not reached by the
    /// default configuration): the deflection of a straight step has length
    /// `~R alpha^2 / 2`, and `$MF_Transport(.., 1)` sets the position to the
    /// start plus it.
    #[test]
    fn analytic_deflection_and_mf_transport() {
        let (_, mut s, mut st) = started([1.0, 0.0, 0.0]);
        s.u_emf = [0.0, 0.0, 1.0];
        s.x_emf = [0.0, 0.0, 0.5];
        let t = s.calc_analytic_deflection(0.2, -1);
        let r = s.v_i / s.omega_em * 100.0;
        let alpha = 0.2 / r;
        let mag = (t[0] * t[0] + t[1] * t[1] + t[2] * t[2]).sqrt();
        assert!((mag - r * alpha * alpha / 2.0).abs() / mag < 5e-3, "{mag}");
        let mut g = Gap;
        s.mf_transport(&mut g, &mut st, 0.2, 1.0, -1);
        assert!((s.xs[1] - (0.0 + t[1])).abs() < 1e-18);
        // p2 = 2 with the boundary far away takes the same position
        let before = s.xs;
        s.mf_transport(&mut g, &mut st, 0.2, 2.0, -1);
        assert_eq!(s.xs, before);
        // p2 = 0 leaves the position
        s.xs = [9.0; 3];
        s.mf_transport(&mut g, &mut st, 0.2, 0.0, -1);
        assert_eq!(s.xs, [9.0; 3]);
    }

    /// `$ADD_WORK_EM_FIELD`: zero without an electric field.
    #[test]
    fn no_work_without_an_electric_field() {
        let (_, s, _) = started([0.0, 0.5, 0.8]);
        assert_eq!(s.add_work_em_field(-1, [1.0, 2.0, 3.0]), 0.0);
    }

    /// `$EMFIELD_PII`: both integrations (a short step takes the first-order
    /// one) return unit scattering directions and bend `ut` by an amount that
    /// grows with the step; with the field off nothing changes.
    #[test]
    fn emfield_pii_bends_the_directions_and_keeps_unit_length() {
        let (f, mut s, _) = started([1.0, 0.0, 0.0]);
        // make the field act on a perpendicular direction so sMinP3 is set
        let mut st = one_electron([0.0, 0.0, 0.5], [0.0, 0.0, 1.0], 2);
        st.u[1] = 0.0;
        let mut tustep = 1.0;
        s.initiate_set_tustep(&f, &st, 1, &InitiateInput { eke: 5.0, tperp: 0.5, skindepth: 1.0e-3, exact_bca: true }, &mut tustep);
        assert!(s.smin_p3 > 0.0);
        let mk = |ts: f64| PiiInput { u0: [0.0, 0.0, 1.0], x0: [0.0, 0.0, 0.5], tustep: ts, eloss: 1.0e-3, qel: 0, s1: [0.01, 1.0, 0.0, 0.99995], s2: [0.01, 0.0, 1.0, 0.99995] };
        let dirs = |s: &mut EemfState, ts: f64| {
            let mut d = PiiDirections { us: [0.0, 0.0, 1.0], ut: [0.0, 0.0, 1.0] };
            s.emfield_pii(&f, &mk(ts), &mut d);
            d
        };
        // short step: first order (tustep <= sMinP3); long step: Simpson
        let short = s.smin_p3 * 0.5;
        let long = s.smin_p3 * 4.0;
        let d1 = dirs(&mut s, short);
        let d3 = dirs(&mut s, long);
        for d in [d1, d3] {
            let n = (d.us[0] * d.us[0] + d.us[1] * d.us[1] + d.us[2] * d.us[2]).sqrt();
            assert!((n - 1.0).abs() < 1e-12, "{n}");
            assert!(d.us[1].abs() > 0.0, "the field bends the direction: {:?}", d.us);
        }
        assert!(d3.us[1].abs() > d1.us[1].abs());
        // field off: no change
        let mut off = EemfState::default();
        let mut d = PiiDirections { us: [0.0, 0.0, 1.0], ut: [0.0, 0.0, 1.0] };
        off.emfield_pii(&f, &mk(0.1), &mut d);
        assert_eq!(d.us, [0.0, 0.0, 1.0]);
    }

    /// `$EM_FIELD_SS`: a long step towards a boundary walks the helix in
    /// segments of the distance to the nearest boundary and stops at
    /// `tustep` or inside the linear buffer; the position stays inside the
    /// geometry and the direction keeps unit length.
    #[test]
    fn em_field_ss_walks_to_tustep_or_the_boundary() {
        let (_, mut s, mut st) = started([1.0, 0.0, 0.0]);
        let mut g = Gap;
        // tustep far smaller than the distance to the boundary (0.5): one transport, no howfar loop
        let mut v = EemfStepVars { callhowfar: false, vstep: 0.1, ustep: 0.1, tustep: 0.1, tperp: 0.5, skindepth: 1.0e-3, irl: 2, irnew: 2, lelec: -1, ..Default::default() };
        s.em_field_ss(&mut st, &mut g, &mut v);
        let np = st.np;
        let n = (st.u[np] * st.u[np] + st.v[np] * st.v[np] + st.w[np] * st.w[np]).sqrt();
        assert!((n - 1.0).abs() < 1e-14);
        assert!(v.final_pos[1].abs() > 0.0, "the helix moved the particle off the straight line");
        // a step that reaches the boundary: callhowfar, tustep beyond tperp
        let (_, mut s2, mut st2) = started([1.0, 0.0, 0.0]);
        let mut v2 = EemfStepVars { callhowfar: true, vstep: 5.0, ustep: 5.0, tustep: 5.0, tperp: 0.5, skindepth: 1.0e-3, irl: 2, irnew: 2, lelec: -1, ..Default::default() };
        s2.em_field_ss(&mut st2, &mut g, &mut v2);
        assert!(!v2.callhowfar);
        assert_eq!(v2.ustep, v2.vstep);
        assert!(v2.vstep > 0.0 && v2.vstep <= 5.0);
        // no field: the linear branch
        let mut s3 = EemfState { x0: [0.0, 0.0, 0.5], u0: [0.0, 0.0, 1.0], ..Default::default() };
        let mut st3 = one_electron([0.0, 0.0, 0.5], [0.0, 0.0, 1.0], 2);
        let mut v3 = EemfStepVars { vstep: 0.2, ..Default::default() };
        s3.em_field_ss(&mut st3, &mut g, &mut v3);
        assert_eq!(v3.final_pos, [0.0, 0.0, 0.7]);
    }

    /// `$EMFieldInVacuum`: in vacuum with a field, the electron curves in the
    /// gap and ends on a boundary; with a stale `tperp` inside the linear
    /// buffer the macro takes its short linear step first; without a field
    /// the step is a straight line.
    #[test]
    fn emfield_in_vacuum_curves_and_reaches_a_boundary() {
        let f = EemfField::magnetic([1.0, 0.0, 0.0]);
        let mut g = Gap;
        // low energy (radius 2.9 mm in 1 T), started 1 mm from the wall at z = 1 so that the circle crosses it
        let mut st = one_electron([0.0, 0.0, 0.999], [0.0, 0.0, 1.0], 2);
        st.e[1] = 0.5 + 0.5109989461;
        let mut s = EemfState::default();
        let mut tustep = 1.0e8;
        s.initiate_set_tustep(&f, &st, 1, &InitiateInput { eke: 0.5, tperp: 0.001, skindepth: 1.0e-3, exact_bca: true }, &mut tustep);
        let mut v = EemfStepVars { callhowfar: true, ustep: 1.0e8, tustep: 1.0e8, vstep: 0.0, tperp: 0.001, skindepth: 1.0e-3, irl: 2, irnew: 2, lelec: -1, ..Default::default() };
        // howfar of the straight line first, as ELECTR does
        call_howfar(&mut g, &st, &mut v);
        s.emfield_in_vacuum(&mut st, &mut g, &mut v);
        assert!(v.vstep > 0.0, "{:?}", v);
        assert_eq!(v.ustep, v.vstep);
        let np = st.np;
        // it stopped within the linear buffer of a boundary (z = 0 or z = 1)
        let z = st.z[np];
        assert!(z.min(1.0 - z) <= bca_lin_buffer(1.0e-3) + 1.0e-5, "z = {z}");
        let n = (st.u[np] * st.u[np] + st.v[np] * st.v[np] + st.w[np] * st.w[np]).sqrt();
        assert!((n - 1.0).abs() < 1e-9, "{n}");
        // the path is longer than the straight line to the nearest wall allows: it curved (y moved)
        assert!((st.y[np].abs() - 1.7e-6).abs() < 3.0e-7, "sagitta d^2/(2R) = 1.7e-6 cm, y = {}", st.y[np]);

        // short step near the boundary into the next region (stale tperp small, irl differs)
        let mut s2 = s;
        let mut st2 = one_electron([0.0, 0.0, 0.999999], [0.0, 0.0, 1.0], 2);
        s2.u0 = [0.0, 0.0, 1.0];
        let mut v2 = EemfStepVars { callhowfar: true, ustep: 5.0e-6, tustep: 5.0e-6, tperp: 1.0e-7, skindepth: 1.0e-3, irl: 3, irnew: 3, lelec: -1, ..Default::default() };
        s2.emfield_in_vacuum(&mut st2, &mut g, &mut v2);
        assert_eq!(v2.vstep, 5.0e-6);
        assert!((st2.z[1] - (0.999999 + 5.0e-6)).abs() < 1e-15);

        // field off: a straight step
        let mut s3 = EemfState::default();
        let mut st3 = one_electron([0.0, 0.0, 0.5], [0.0, 0.0, 1.0], 2);
        let mut v3 = EemfStepVars { ustep: 0.25, ..Default::default() };
        s3.emfield_in_vacuum(&mut st3, &mut g, &mut v3);
        assert_eq!(st3.z[1], 0.75);
        assert_eq!(v3.vstep, 0.25);
    }
}
