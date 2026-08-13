// Ported from NJOY2016 `src/groupr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! Center-of-mass → lab kinematics for the GROUPR scatter-matrix feed functions.
//!
//! The GROUPR group-to-group **matrix** path (`mfd = 6`) needs the
//! secondary-particle emission spectrum transformed from the center-of-mass
//! frame (where ENDF File 6 laws are given) into the lab frame, then binned into
//! secondary groups to form the feed function the panel integrator deposits.
//! This module ports the **continuum** core of that transform: the Kalbach-Mann
//! and Legendre-in-CM double-differential evaluations, the Kalbach slope, and the
//! adaptive CM→lab integrator.
//!
//! # Fortran routine → Rust item map (with `groupr.f90` line cites)
//!
//! | Fortran routine | lines | Rust item | Status |
//! |---|---|---|---|
//! | `legndr`  | `mathm.f90` | [`legndr`] | **DONE** (helper) |
//! | `bach`    | 8812–8932 | [`bach`] | **DONE** |
//! | `f6ddx`   | 8520–8717 | [`Cm6Emission::f6ddx`] | **DONE** for `lang=1,2` continuum; `lang=0` (phase space) and `lang>=11` (tabulated) → `NotPorted` |
//! | `f6cm`    | 8260–8518 | [`Cm6Emission::f6cm`] | **DONE** for the `ND=0` continuum; the discrete-delta branch (330, 8467–8501) is not reached (input carries no discrete lines) |
//! | `cm2lab`  | 8135–8258 | [`cm2lab`] | **DONE** |
//! | `f6dis`   | 8719–8810 | [`f6dis`] | **NotPorted** stub |
//! | `ll2lab`  | 8934–9061 | [`ll2lab`] | **NotPorted** stub |
//! | `f6lab`   | 9063–9338 | [`f6lab`] | **NotPorted** stub |
//!
//! # Scope of the ported path (honest limits)
//!
//! [`cm2lab`] handles ENDF File 6 **LAW 1** (continuum energy-angle) in the
//! **CM frame** with **LANG = 1** (Legendre-in-CM) or **LANG = 2** (Kalbach-Mann
//! systematics), and **no discrete emission lines** (ND = 0). This is the
//! tractable, hand-checkable core called out for this porting pass. The discrete
//! delta-function path (`f6dis`), the law-7 angle-energy transform (`ll2lab`),
//! the lab-frame LAW 1 evaluator (`f6lab`), and the phase-space law
//! (`f6psp`, LANG = 0) are **not ported** and return
//! [`crate::NjoyError::NotPorted`]. See the per-item docs for exact line ranges.
//!
//! This is untrusted AI draft material: verify against upstream NJOY golden
//! output before trusting numerically.

use crate::common::phys::{AMASSN_AMU, AMU_G, CLIGHT_CM_S, EV_ERG};
use crate::endf::interp::{terp1, IntLaw};
use crate::NjoyError;

/// Sentinel "no further break point" energy \[eV\] — NJOY's `emax = 1.e10`.
const EMAX: f64 = 1.0e10;
/// Relative smallness guard — NJOY's `small = 1.e-10`.
const SMALL: f64 = 1.0e-10;
/// NJOY's `tiny = 1.e-4`, used to size the `f6cm` step floor `eps = tiny/elmax`.
const TINY: f64 = 1.0e-4;

/// Legendre polynomials `P_0(x) .. P_np(x)` by upward recursion.
///
/// Faithful port of `legndr` (`mathm.f90`): returns a vector `p` of length
/// `np + 1` with `p[l] = P_l(x)` for `l = 0 ..= np`. `x` is a direction cosine
/// in `[-1, 1]` (dimensionless); the polynomials are dimensionless.
///
/// Uses the same three-term recurrence NJOY uses,
/// `P_{i+1} = ((2i+1) x P_i - i P_{i-1}) / (i+1)`, written in NJOY's
/// difference form to match its round-off behaviour bit-for-bit.
pub fn legndr(x: f64, np: usize) -> Vec<f64> {
    let mut p = vec![0.0_f64; np + 1];
    p[0] = 1.0;
    if np >= 1 {
        p[1] = x;
    }
    if np < 2 {
        return p;
    }
    for i in 1..=(np - 1) {
        let g = x * p[i];
        let h = g - p[i - 1];
        p[i + 1] = h + g - h / ((i + 1) as f64);
    }
    p
}

/// Lab secondary energy for a two-body-like CM→lab emission \[eV\].
///
/// This is the kinematic relation the [`cm2lab`] integrator inverts: a particle
/// emitted with center-of-mass energy `ep_cm` \[eV\] at CM cosine `mu_cm`
/// (dimensionless, `[-1, 1]`) from a reaction at incident energy `e_in` \[eV\]
/// appears in the lab at
///
/// `E_lab = ep_cm + xc * e_in + 2 * mu_cm * sqrt(xc * e_in * ep_cm)`,
///
/// where the CM-motion factor is `xc = A' / (A + 1)^2` — `A'` the emitted
/// particle's atomic-weight ratio to the neutron and `A` the target's
/// (dimensionless). At `mu_cm = +1` this gives the forward edge
/// `(sqrt(ep_cm) + sqrt(xc*e_in))^2`; at `mu_cm = -1` the backward edge
/// `(sqrt(ep_cm) - sqrt(xc*e_in))^2` (the `elmax`/`elmin` limits used in `f6cm`,
/// `groupr.f90:8316,8330-8333`).
///
/// Valid for `ep_cm >= 0`, `e_in > 0`, `xc >= 0`.
pub fn lab_energy_from_cm(e_in: f64, ep_cm: f64, mu_cm: f64, xc: f64) -> f64 {
    ep_cm + xc * e_in + 2.0 * mu_cm * (xc * e_in * ep_cm).sqrt()
}

/// The CM angular representation of a File-6 LAW-1 continuum emission.
///
/// Selects how the per-secondary-energy coefficient list in a [`Cm6Point`] is
/// interpreted, mirroring the ENDF `LANG` flag NJOY dispatches on in `f6ddx`
/// (`groupr.f90:8605,8627`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cm6Lang {
    /// `LANG = 1`: the coefficient list is `[b_0, b_1, .., b_NA]`, the Legendre
    /// expansion of the CM angular distribution,
    /// `f(mu) = sum_L (2L+1)/2 * b_L * P_L(mu)`.
    LegendreInCm,
    /// `LANG = 2`: Kalbach-Mann systematics. The coefficient list is `[f_0, r]`
    /// (slope `a` computed from [`bach`]) or `[f_0, r, a]` (slope tabulated),
    /// with `f(mu) = a (cosh(a mu) + r sinh(a mu)) / (2 sinh a) * f_0`.
    Kalbach,
}

/// One secondary-energy record of a File-6 LAW-1 CM continuum distribution.
///
/// Mirrors one `(E', b_0, b_1, ...)` entry of NJOY's `cnow` list
/// (`groupr.f90:8609-8658`). `ep` is the CM secondary energy \[eV\]; `coeffs`
/// is the `LANG`-dependent coefficient list (see [`Cm6Lang`]). All `coeffs`
/// entries across the points of one [`Cm6Emission`] must have the same length.
#[derive(Debug, Clone)]
pub struct Cm6Point {
    /// CM secondary (outgoing-particle) energy `E'` \[eV\].
    pub ep: f64,
    /// `LANG`-dependent coefficient list (Legendre `b_L`, or Kalbach `f_0, r[, a]`).
    pub coeffs: Vec<f64>,
}

/// An owned File-6 LAW-1 CM-frame continuum emission block (one incident energy).
///
/// This is the Rust analogue of NJOY's `cnow` array for a single incident-energy
/// subsection (`groupr.f90` `f6cm`/`f6ddx` consume `cnow(1..)`), restricted to
/// the ported path: **LAW 1, CM frame, LANG 1 or 2, no discrete lines
/// (ND = 0)**. [`Cm6Emission::to_lab`] / [`cm2lab`] transform it to the lab
/// frame.
///
/// # Fields and units
/// - `e_in` — incident (projectile) energy \[eV\] (`cnow(2)`).
/// - `lang` — CM angular representation ([`Cm6Lang`]).
/// - `lep` — ENDF secondary-energy interpolation law for `E'` between points
///   (the `LEP` flag; [`IntLaw::LinLin`] is the usual choice).
/// - `awr_target` — target atomic-weight ratio `A = m_target / m_neutron`
///   (dimensionless; NJOY `awr`).
/// - `awp_emitted` — emitted-particle atomic-weight ratio
///   `A' = m_product / m_neutron` (dimensionless; NJOY `aprime`).
/// - `za_projectile`, `za_target`, `za_emitted` — ENDF `ZA = 1000 Z + A`
///   integers for the incident particle, target, and emitted particle
///   (used by [`bach`] for the Kalbach slope; `za_projectile = 1` for a neutron).
/// - `points` — the continuum records, **ascending** in `ep`.
#[derive(Debug, Clone)]
pub struct Cm6Emission {
    /// Incident energy `E` \[eV\].
    pub e_in: f64,
    /// CM angular representation.
    pub lang: Cm6Lang,
    /// Secondary-energy interpolation law (ENDF `LEP`).
    pub lep: IntLaw,
    /// Target atomic-weight ratio `A` (dimensionless).
    pub awr_target: f64,
    /// Emitted-particle atomic-weight ratio `A'` (dimensionless).
    pub awp_emitted: f64,
    /// Projectile `ZA` (e.g. `1` for a neutron).
    pub za_projectile: i32,
    /// Target `ZA` (`1000 Z + A`).
    pub za_target: i32,
    /// Emitted-particle `ZA`.
    pub za_emitted: i32,
    /// Continuum secondary-energy records, ascending in `ep`.
    pub points: Vec<Cm6Point>,
}

/// The lab-frame Legendre distribution produced by [`cm2lab`].
///
/// `points` are `(E'_lab \[eV\], Legendre coefficients)` pairs in **ascending**
/// lab secondary energy, each coefficient vector of length `nl`
/// (`coeffs[l] = ` the lab `P_l` moment of the double-differential emission at
/// that `E'`). `p0_integral` is the integral of the lab `P_0` moment over `E'`
/// — NJOY's `cm2lab` `sum` normalization check (`groupr.f90:8233,8244`), which
/// should be `≈ 1` for a normalized CM emission.
#[derive(Debug, Clone)]
pub struct LabDistribution {
    /// Incident energy `E` \[eV\] this distribution is for.
    pub e_in: f64,
    /// Number of lab Legendre coefficients per point (`P_0 .. P_{nl-1}`).
    pub nl: usize,
    /// `(E'_lab \[eV\], [P_0, .., P_{nl-1}])`, ascending in `E'_lab`.
    pub points: Vec<(f64, Vec<f64>)>,
    /// Integral of the lab `P_0` moment over `E'` (the `sum ≈ 1` check).
    pub p0_integral: f64,
}

impl Cm6Emission {
    /// Number of CM angular parameters `NA` per record (`ncnow - 2` in NJOY).
    ///
    /// For `LANG = 1` this is the top Legendre order; for `LANG = 2` it is 1
    /// (slope from [`bach`]) or 2 (slope tabulated).
    fn na(&self) -> usize {
        self.points[0].coeffs.len().saturating_sub(1)
    }

    /// f6ddx **initialization** (`groupr.f90:8552-8571`, `ep == 0` branch).
    ///
    /// Returns `(epnext0, epmax)`: the first CM secondary-energy break point
    /// (clamped to `0.05 * epmax`, NJOY's `step`) and the maximum CM secondary
    /// energy \[eV\].
    fn f6ddx_init(&self) -> (f64, f64) {
        let cp = &self.points;
        let epmax = cp[cp.len() - 1].ep;
        // Skip a leading zero-energy point, exactly as NJOY's
        // `if (cnow(lnow).le.zero) lnow=lnow+ncnow`.
        let idx = if cp[0].ep <= 0.0 && cp.len() > 1 {
            1
        } else {
            0
        };
        let mut epnext = cp[idx].ep;
        let step = 0.05;
        if epnext > step * epmax {
            epnext = step * epmax;
        }
        (epnext, epmax)
    }

    /// CM double-differential cross section at CM secondary energy `ep_cm` \[eV\]
    /// and CM cosine `w` (dimensionless) — faithful port of the continuum branch
    /// of `f6ddx` (`groupr.f90:8579-8716`).
    ///
    /// Returns `(t, epnext)`: the double-differential value `t`
    /// (barn / eV / steradian-equivalent, `>= 0`) and the next CM secondary
    /// energy break above `ep_cm` \[eV\] (or [`EMAX`]). Only the continuum part
    /// is returned; discrete deltas are the caller's responsibility (and are not
    /// present on the ported path).
    ///
    /// `LANG = 1` interpolates the Legendre `b_L` between the bracketing points
    /// then evaluates `sum_L (2L+1)/2 b_L P_L(w)`; `LANG = 2` interpolates
    /// `f_0, r` (and `a`, or computes `a` via [`bach`]) then evaluates the
    /// Kalbach form.
    fn f6ddx(&self, ep_cm: f64, w: f64) -> Result<(f64, f64), NjoyError> {
        // Interpolation nudge constants (groupr.f90:8543-8547).
        const UP: f64 = 1.000_01;
        const DN: f64 = 0.999_99;
        const OFF: f64 = 0.999_995;

        let cp = &self.points;
        let npc = cp.len();
        let na = self.na();
        let nl = na + 1; // ncnow - 1
        let ep_first = cp[0].ep;
        let ep_last = cp[npc - 1].ep;

        // --Locate the bracketing interval [k-1, k] (groupr.f90:8582-8602).
        let mut k = 1usize;
        let mut epnext;
        loop {
            epnext = cp[k].ep;
            if ep_cm < OFF * epnext {
                let eplast = cp[k - 1].ep;
                if ep_cm >= OFF * eplast || k <= 1 {
                    break;
                }
                k -= 1;
            } else if k >= npc - 1 {
                epnext = EMAX;
                break;
            } else {
                k += 1;
            }
        }

        let in_range = ep_cm >= ep_first * (1.0 - SMALL) && ep_cm <= ep_last * (1.0 + SMALL);
        let (x1, x2) = (cp[k - 1].ep, cp[k].ep);

        let mut t;
        match self.lang {
            Cm6Lang::LegendreInCm => {
                let p = legndr(w, na);
                t = 0.0;
                if in_range {
                    for l in 1..=nl {
                        // coeffs[l-1] = b_{l-1} (offset l in NJOY's entry).
                        let y1 = cp[k - 1].coeffs[l - 1];
                        let y2 = cp[k].coeffs[l - 1];
                        let tt = if x1 == x2 {
                            y1
                        } else {
                            terp1(x1, y1, x2, y2, ep_cm, self.lep)?
                        };
                        t += (2.0 * l as f64 - 1.0) * tt * p[l - 1] / 2.0;
                    }
                }
                if t < 0.0 {
                    t = 0.0;
                }
            }
            Cm6Lang::Kalbach => {
                let mut s = 0.0;
                let mut r = 0.0;
                if in_range {
                    let (y1, y2) = (cp[k - 1].coeffs[0], cp[k].coeffs[0]);
                    s = if x1 == x2 {
                        y1
                    } else {
                        terp1(x1, y1, x2, y2, ep_cm, self.lep)?
                    };
                    let (y1, y2) = (cp[k - 1].coeffs[1], cp[k].coeffs[1]);
                    r = if x1 == x2 {
                        y1
                    } else {
                        terp1(x1, y1, x2, y2, ep_cm, self.lep)?
                    };
                }
                // Kalbach slope: tabulated (na==2) or from the systematics.
                let aa = if na == 2 {
                    let (y1, y2) = (cp[k - 1].coeffs[2], cp[k].coeffs[2]);
                    if x1 == x2 || !in_range {
                        y1
                    } else {
                        terp1(x1, y1, x2, y2, ep_cm, self.lep)?
                    }
                } else {
                    bach(
                        self.za_projectile,
                        self.za_emitted,
                        self.za_target,
                        self.e_in,
                        ep_cm,
                    )?
                };
                t = aa * ((aa * w).cosh() + r * (aa * w).sinh()) / (2.0 * aa.sinh());
                t *= s;
                if t < 0.0 {
                    t = 0.0;
                }
            }
        }

        // --Nudge epnext for histogram (lep==1) secondary interpolation
        //   (groupr.f90:8705-8714).
        if self.lep != IntLaw::Histogram {
            return Ok((t, epnext));
        }
        if (epnext - EMAX).abs() < EMAX * SMALL {
            Ok((t, epnext))
        } else if ep_cm >= DN * DN * epnext {
            Ok((t, UP * epnext))
        } else {
            Ok((t, DN * epnext))
        }
    }

    /// Lab-frame Legendre coefficients at lab secondary energy `ep` \[eV\] —
    /// faithful port of the continuum branch of `f6cm` (`groupr.f90:8343-8514`).
    ///
    /// Returns `(term, epnext)`: `term[l] = ` lab `P_l` moment
    /// (`l = 0 .. nl_lab-1`) of the double-differential emission at `ep`, and the
    /// next lab secondary-energy grid point \[eV\]. `prev_epnext` is the previous
    /// grid point (only used by the `c > 1` low-energy branch, NJOY line 8463).
    /// `xc`, `epmax`, `elmax`, and `eps` are the initialization quantities
    /// computed once by [`Cm6Emission::to_lab`].
    ///
    /// The physics: for the fixed lab energy `ep`, integrate the CM
    /// double-differential ([`Cm6Emission::f6ddx`]) over lab cosine `u` from
    /// `umin` to `1`, weighting by `P_{l-1}(u)` and the CM→lab Jacobian
    /// `1 / sqrt(yy)`, with adaptive refinement (NJOY's `imax = 10` cosine stack).
    #[allow(clippy::too_many_arguments)]
    fn f6cm(
        &self,
        ep: f64,
        prev_epnext: f64,
        nl_lab: usize,
        xc: f64,
        epmax: f64,
        elmax: f64,
        eps: f64,
    ) -> Result<(Vec<f64>, f64), NjoyError> {
        // Local constants (groupr.f90:8283-8296).
        const TOL: f64 = 0.005;
        const CHECK: f64 = 0.999_99;
        const AMIL: f64 = 1.0e-6;
        const ATHOU: f64 = 1.0e-3;
        const TINY_C: f64 = 1.0e-4;
        const ONE: f64 = 1.0;
        const IMAX: usize = 10;
        let e = self.e_in;
        let na_lab = nl_lab - 1;

        let mut term = vec![0.0_f64; nl_lab];

        let xx = ep / e;
        let cc0 = xc / xx;
        let c0 = cc0.sqrt();
        let umin0 = (1.0 + cc0 - epmax / ep) / (2.0 * c0);

        // --Branch selection (groupr.f90:8352-8354).
        if umin0 >= CHECK {
            if c0 <= ONE {
                // 420: above the lab kinematic maximum — no contribution.
                return Ok((term, EMAX));
            }
            // 320: c > 1 low-energy fold — step forward, no contribution.
            let epnxt = (prev_epnext + elmax) / 2.0;
            remove_small_moments(&mut term, nl_lab, TOL);
            return Ok((term, epnxt));
        }

        // --215 continuum: adaptive integration over lab cosine.
        let mut umin = umin0;
        if umin < -ONE {
            umin = -ONE;
        }
        let dm = (1.0 - umin) * ATHOU + AMIL;
        let mut epnn = EMAX;

        // `c`/`cc` persist and may be mutated by the yy<amil guard, exactly as
        // NJOY carries them across the panel loop.
        let mut c = c0;
        let mut cc = cc0;

        let umax = ONE;
        let mut un = umax;

        // Cosine panel stack (1-based to mirror NJOY; index 0 unused).
        let mut x = vec![0.0_f64; IMAX + 2];
        let mut y = vec![vec![0.0_f64; nl_lab]; IMAX + 2];
        x[2] = un;

        let mut guard: u64 = 0;
        while x[2] > umin {
            // --Load the two boundaries of this cosine panel (8370-8393).
            let mut load_again = true;
            while load_again {
                let u = un;
                let mut yy = 1.0 + cc - 2.0 * c * u;
                if yy < AMIL {
                    yy = AMIL;
                    c = u - ATHOU;
                    cc = c * c;
                }
                let epp = yy * ep;
                let w = (u - c) / yy.sqrt();
                let (s, epn) = self.f6ddx(epp, w)?;
                if u == umax && epn < epnn {
                    epnn = epn;
                }
                let p = legndr(u, na_lab);
                let jj = if u == umax { 2 } else { 1 };
                x[jj] = u;
                for l in 1..=nl_lab {
                    y[jj][l - 1] = p[l - 1] * s / yy.sqrt();
                }
                un = u - (epn - epp) / (2.0 * c * ep);
                if un > u - TINY_C {
                    un = u - TINY_C;
                }
                if un < umin + TINY_C / 10.0 {
                    un = umin;
                }
                load_again = u == umax;
            }

            // --Adaptive reconstruction of this panel (8396-8446).
            let mut i = 2usize;
            while i > 1 {
                let mut iflag = 0;
                let mut u_mid = 0.0;
                let mut yt = vec![0.0_f64; nl_lab];
                if i > 1 && i < IMAX {
                    let dx = x[i] - x[i - 1];
                    if dx >= dm {
                        let da = dx * (y[i][0] + y[i - 1][0]) / 2.0;
                        if da >= eps {
                            u_mid = (x[i - 1] + x[i]) / 2.0;
                            let yy = 1.0 + cc - 2.0 * c * u_mid;
                            let epp = yy * ep;
                            let w = (u_mid - c) / yy.sqrt();
                            let (s, _epn) = self.f6ddx(epp, w)?;
                            let p = legndr(u_mid, na_lab);
                            for l in 1..=nl_lab {
                                yt[l - 1] = p[l - 1] * s / yy.sqrt();
                            }
                            for l in 1..=nl_lab {
                                let ym = (y[i - 1][l - 1] + y[i][l - 1]) / 2.0;
                                let dy = (yt[l - 1] - ym).abs();
                                if dy > (l as f64) * TOL * ym.abs() + eps {
                                    iflag = 1;
                                }
                            }
                        }
                    }
                }
                if iflag == 1 {
                    i += 1;
                    x[i] = x[i - 1];
                    y[i] = y[i - 1].clone();
                    x[i - 1] = u_mid;
                    y[i - 1] = yt;
                } else {
                    for l in 1..=nl_lab {
                        term[l - 1] += (x[i] - x[i - 1]) * (y[i][l - 1] + y[i - 1][l - 1]) / 2.0;
                    }
                    i -= 1;
                }
            }

            // --Continue to the next cosine panel (8449-8452).
            x[2] = x[1];
            y[2] = y[1].clone();

            guard += 1;
            if guard > 10_000_000 {
                return Err(NjoyError::EndfParse(
                    "f6cm: cosine panel loop failed to advance".into(),
                ));
            }
        }

        // --Select the next lab energy grid point (8455-8461).
        let mut epnxt = e * ((epnn / e).sqrt() - xc.sqrt()).powi(2);
        if epnxt <= ep * (1.0 + SMALL) {
            epnxt = e * ((epnn / e).sqrt() + xc.sqrt()).powi(2);
        }
        if epnxt <= ep * (1.0 + SMALL) {
            epnxt = 3.0 * ep / 2.0;
        }
        if epnxt > elmax * (1.0 + SMALL) && elmax > ep * (1.0 + SMALL) {
            epnxt = elmax;
        }

        remove_small_moments(&mut term, nl_lab, TOL);
        Ok((term, epnxt))
    }

    /// Transform this CM emission block into the lab frame — the public entry
    /// point, equivalent to NJOY's `cm2lab` (see [`cm2lab`], which forwards here).
    ///
    /// `nl` is the number of lab Legendre coefficients to produce per point
    /// (`P_0 .. P_{nl-1}`, so `nl = 1` is `P_0` only). Returns the
    /// [`LabDistribution`] with points ascending in lab secondary energy and the
    /// `sum ≈ 1` normalization integral.
    ///
    /// # Errors
    /// - [`NjoyError::EndfParse`] if `points` is empty, not ascending, `nl == 0`,
    ///   or the coefficient lists are inconsistent / too short for `lang`.
    pub fn to_lab(&self, nl: usize) -> Result<LabDistribution, NjoyError> {
        // Local constants (groupr.f90:8152-8156, 8286-8296).
        const TOL: f64 = 0.01;
        const EPS: f64 = 1.0e-8;
        const UP: f64 = 1.000_001;
        const DN: f64 = 0.999_999;
        const STEP: f64 = 0.05;
        const IMAX: usize = 15;
        const MAX_POINTS: usize = 2_000_000;

        self.validate(nl)?;

        let e = self.e_in;
        let xc = self.awp_emitted / (self.awr_target + 1.0).powi(2);

        // --f6cm initialization (groupr.f90:8299-8340, npnow!=ndnow branch).
        let (epn0, epmax) = self.f6ddx_init();
        let epnn = EMAX.min(epn0);
        let epmax_c = epmax.max(0.0);
        let mut elmax = e * ((epmax_c / e).sqrt() + xc.sqrt()).powi(2);
        elmax *= UP;
        let mut epn = STEP * elmax;
        if epmax_c * UP < xc * e {
            epn = e * ((epmax_c / e).sqrt() - xc.sqrt()).powi(2);
        }
        let _ = epnn; // epnn only feeds the (skipped) discrete path in init.
        let mut epnext = EMAX.min(epn);
        let eps = TINY / elmax;
        epnext *= DN;

        // --cm2lab main march (groupr.f90:8158-8248).
        let mut points_out: Vec<(f64, Vec<f64>)> = Vec::new();
        let mut sum = 0.0_f64;

        let mut x = vec![0.0_f64; IMAX + 2];
        let mut y = vec![vec![0.0_f64; nl]; IMAX + 2];
        x[2] = 0.0; // primed with the init point (term == 0)

        let mut guard: u64 = 0;
        while epnext < EMAX * (1.0 - SMALL) {
            let ep = epnext;
            x[1] = ep;
            let (term, next) = self.f6cm(ep, epnext, nl, xc, epmax_c, elmax, eps)?;
            epnext = next;
            y[1] = term;

            // --Adaptive integration over the lab-energy panel [x(2), x(1)]
            //   with x(2) < x(1) (groupr.f90:8183-8236).
            let mut i = 2usize;
            while i > 1 {
                let mut iflag = 0;
                let mut xm = 0.0;
                let mut term_mid: Vec<f64> = vec![0.0; nl];
                if i > 1 && i < IMAX {
                    let da = (x[i - 1] - x[i]) * (y[i - 1][0] + y[i][0]) / 2.0;
                    let rat = if x[i] != 0.0 { x[i - 1] / x[i] } else { 2.0 };
                    if rat > 1.0 + TOL && da >= EPS {
                        xm = (x[i - 1] + x[i]) / 2.0;
                        if xm != x[i - 1] && xm != x[i] {
                            let (t2, _n2) = self.f6cm(xm, x[i], nl, xc, epmax_c, elmax, eps)?;
                            term_mid = t2;
                            for l in 1..=nl {
                                let ym = (y[i - 1][l - 1] + y[i][l - 1]) / 2.0;
                                let test = (l as f64) * TOL * ym.abs();
                                let dely = (term_mid[l - 1] - ym).abs();
                                if dely > test {
                                    iflag = 1;
                                }
                            }
                        }
                    }
                }
                if iflag == 1 {
                    i += 1;
                    x[i] = x[i - 1];
                    y[i] = y[i - 1].clone();
                    x[i - 1] = xm;
                    y[i - 1] = term_mid;
                } else {
                    points_out.push((x[i], y[i].clone()));
                    sum += (x[i - 1] - x[i]) * (y[i - 1][0] + y[i][0]) / 2.0;
                    i -= 1;
                    if points_out.len() > MAX_POINTS {
                        return Err(NjoyError::EndfParse(
                            "cm2lab: lab grid exceeded the 2M-point safety cap".into(),
                        ));
                    }
                }
            }

            x[2] = x[1];
            y[2] = y[1].clone();

            guard += 1;
            if guard > 10_000_000 {
                return Err(NjoyError::EndfParse(
                    "cm2lab: lab-energy march failed to advance".into(),
                ));
            }
        }

        Ok(LabDistribution {
            e_in: e,
            nl,
            points: points_out,
            p0_integral: sum,
        })
    }

    /// Validate the input record shape for the ported `LANG 1 / 2`, `ND = 0` path.
    fn validate(&self, nl: usize) -> Result<(), NjoyError> {
        if nl == 0 {
            return Err(NjoyError::EndfParse("cm2lab: nl must be >= 1".into()));
        }
        if self.points.is_empty() {
            return Err(NjoyError::EndfParse("cm2lab: no CM emission points".into()));
        }
        let width = self.points[0].coeffs.len();
        let min_width = match self.lang {
            Cm6Lang::LegendreInCm => 1,
            Cm6Lang::Kalbach => 2,
        };
        if width < min_width {
            return Err(NjoyError::EndfParse(format!(
                "cm2lab: coeff list of width {width} too short for {:?} (need >= {min_width})",
                self.lang
            )));
        }
        let mut prev = f64::NEG_INFINITY;
        for pt in &self.points {
            if pt.coeffs.len() != width {
                return Err(NjoyError::EndfParse(
                    "cm2lab: inconsistent coeff-list widths across points".into(),
                ));
            }
            if pt.ep < prev {
                return Err(NjoyError::EndfParse(
                    "cm2lab: CM secondary energies must be ascending".into(),
                ));
            }
            prev = pt.ep;
        }
        if self.e_in <= 0.0 {
            return Err(NjoyError::EndfParse(
                "cm2lab: incident energy must be > 0".into(),
            ));
        }
        Ok(())
    }
}

/// Zero-out lab Legendre moments that are negligible vs `P_0`
/// (`groupr.f90:8503-8510`): for `l >= 1`, if `|term[l]| < tol*|term[0]|`, set 0.
fn remove_small_moments(term: &mut [f64], nl: usize, tol: f64) {
    if nl > 1 {
        let test = tol * term[0].abs();
        for t in term.iter_mut().take(nl).skip(1) {
            if t.abs() < test {
                *t = 0.0;
            }
        }
    }
}

/// Transform a CM-frame File-6 LAW-1 continuum emission into the lab frame.
///
/// Faithful port of NJOY2016 `cm2lab` (`groupr.f90:8135-8258`) for the ported
/// scope (LAW 1, CM frame, LANG 1 or 2, no discrete lines). This is the public
/// entry point of the module; it forwards to [`Cm6Emission::to_lab`].
///
/// # Parameters
/// - `emission` — the CM emission block at one incident energy ([`Cm6Emission`]).
/// - `nl` — number of lab Legendre coefficients per point (`P_0 .. P_{nl-1}`).
///
/// # Returns
/// A [`LabDistribution`] (ascending lab `E'`, Legendre coefficients, and the
/// `sum ≈ 1` normalization integral), or [`NjoyError`] on malformed input.
///
/// # Example
/// ```
/// use njoy_outram_park_fork::endf::interp::IntLaw;
/// use njoy_outram_park_fork::groupr::kinematics::{
///     cm2lab, Cm6Emission, Cm6Lang, Cm6Point,
/// };
///
/// // An isotropic (P0-only) normalized triangular CM emission off a heavy
/// // target: the lab distribution integrates to ~1.
/// let emission = Cm6Emission {
///     e_in: 1.0e6,
///     lang: Cm6Lang::LegendreInCm,
///     lep: IntLaw::LinLin,
///     awr_target: 50.0,
///     awp_emitted: 1.0,
///     za_projectile: 1,
///     za_target: 26056,
///     za_emitted: 1,
///     points: vec![
///         Cm6Point { ep: 0.0,     coeffs: vec![0.0] },
///         Cm6Point { ep: 2.5e5,   coeffs: vec![4.0e-6] },
///         Cm6Point { ep: 5.0e5,   coeffs: vec![0.0] },
///     ],
/// };
/// let lab = cm2lab(&emission, 1).unwrap();
/// assert!((lab.p0_integral - 1.0).abs() < 0.01);
/// ```
pub fn cm2lab(emission: &Cm6Emission, nl: usize) -> Result<LabDistribution, NjoyError> {
    emission.to_lab(nl)
}

/// Kalbach-Mann `a(E, E')` slope parameter (dimensionless).
///
/// Faithful port of NJOY2016 `bach` (`groupr.f90:8812-8932`), the Kalbach-86
/// systematics. Given the projectile, emitted-particle, and target `ZA` codes
/// (`ZA = 1000 Z + A`; `iza1i = 0` is treated as an incident neutron), the
/// incident energy `e` \[eV\], and the CM secondary energy `ep` \[eV\], returns
/// the slope `a` of the Kalbach angular form
/// `f(mu) ~ a (cosh(a mu) + r sinh(a mu))`.
///
/// Natural-element target codes (`Z000`) are mapped to their dominant isotope,
/// as NJOY does. Separation energies use the Kalbach mass formula (constants
/// `c1..c6`, `s2..s5`). For an incident neutron (`iza1i = 0`) the extra
/// low-energy `d1 / sqrt(E')` enhancement factor is applied.
///
/// # Errors
/// [`NjoyError::EndfParse`] if the target's dominant isotope is unknown
/// (mirrors NJOY's `error('bach', 'dominant isotope not known ...')`).
///
/// Valid for `e > 0`, `ep > 0`.
pub fn bach(iza1i: i32, iza2: i32, izat: i32, e: f64, ep: f64) -> Result<f64, NjoyError> {
    // Kalbach-86 constants (groupr.f90:8826-8847).
    const THIRD: f64 = 0.333_333_333;
    const TWOTH: f64 = 0.666_666_667;
    const FOURTH: f64 = 1.333_333_33;
    const C1: f64 = 15.68;
    const C2: f64 = -28.07;
    const C3: f64 = -18.56;
    const C4: f64 = 33.22;
    const C5: f64 = -0.717;
    const C6: f64 = 1.211;
    const S2: f64 = 2.22;
    const S3: f64 = 8.48;
    const S4: f64 = 7.72;
    const S5: f64 = 28.3;
    const BRK1: f64 = 130.0;
    const BRK2: f64 = 41.0;
    const HALF: f64 = 0.5;
    const B1: f64 = 0.04;
    const B2: f64 = 1.8e-6;
    const B3: f64 = 6.7e-7;
    const D1: f64 = 9.3;
    const TOMEV: f64 = 1.0e-6;

    // Neutron rest-mass energy in MeV (NJOY `emc2 = tomev*amassn*amu*c^2/ev`).
    let emc2 = TOMEV * AMASSN_AMU * AMU_G * CLIGHT_CM_S * CLIGHT_CM_S / EV_ERG;

    let iza1 = if iza1i == 0 { 1 } else { iza1i };

    // Map a natural-element target to its dominant isotope (groupr.f90:8853-8875).
    let iza = match izat {
        6000 => 6012,
        12000 => 12024,
        14000 => 14028,
        16000 => 16032,
        17000 => 17035,
        19000 => 19039,
        20000 => 20040,
        22000 => 22048,
        23000 => 23051,
        24000 => 24052,
        26000 => 26056,
        28000 => 28058,
        29000 => 29063,
        31000 => 31069,
        40000 => 40090,
        42000 => 42096,
        48000 => 48112,
        49000 => 49115,
        50000 => 50120,
        63000 => 63151,
        72000 => 72178,
        74000 => 74184,
        82000 => 82208,
        other => other,
    };

    let aa = (iza.rem_euclid(1000)) as f64;
    if aa == 0.0 {
        return Err(NjoyError::EndfParse(format!(
            "bach: dominant isotope not known for {iza}"
        )));
    }
    let za = (iza / 1000) as f64;
    let ac = aa + (iza1.rem_euclid(1000)) as f64;
    let zc = za + (iza1 / 1000) as f64;
    let ab = ac - (iza2.rem_euclid(1000)) as f64;
    let zb = zc - (iza2 / 1000) as f64;
    let na = (aa - za).round();
    let nb = (ab - zb).round();
    let nc = (ac - zc).round();

    let mut sa = C1 * (ac - aa)
        + C2 * ((nc - zc).powi(2) / ac - (na - za).powi(2) / aa)
        + C3 * (ac.powf(TWOTH) - aa.powf(TWOTH))
        + C4 * ((nc - zc).powi(2) / ac.powf(FOURTH) - (na - za).powi(2) / aa.powf(FOURTH))
        + C5 * (zc.powi(2) / ac.powf(THIRD) - za.powi(2) / aa.powf(THIRD))
        + C6 * (zc.powi(2) / ac - za.powi(2) / aa);
    match iza1 {
        1002 => sa -= S2,
        1003 => sa -= S3,
        2003 => sa -= S4,
        2004 => sa -= S5,
        _ => {}
    }

    let mut sb = C1 * (ac - ab)
        + C2 * ((nc - zc).powi(2) / ac - (nb - zb).powi(2) / ab)
        + C3 * (ac.powf(TWOTH) - ab.powf(TWOTH))
        + C4 * ((nc - zc).powi(2) / ac.powf(FOURTH) - (nb - zb).powi(2) / ab.powf(FOURTH))
        + C5 * (zc.powi(2) / ac.powf(THIRD) - zb.powi(2) / ab.powf(THIRD))
        + C6 * (zc.powi(2) / ac - zb.powi(2) / ab);
    match iza2 {
        1002 => sb -= S2,
        1003 => sb -= S3,
        2003 => sb -= S4,
        2004 => sb -= S5,
        _ => {}
    }

    let ecm = aa * e / ac;
    let ea = ecm * TOMEV + sa;
    let eb = TOMEV * ep * ac / ab + sb;
    let mut x1 = eb;
    if ea > BRK1 {
        x1 = BRK1 * eb / ea;
    }
    let mut x3 = eb;
    if ea > BRK2 {
        x3 = BRK2 * eb / ea;
    }
    let fa = if iza1 == 2004 { 0.0 } else { 1.0 };
    let fb = if iza2 == 1 {
        HALF
    } else if iza2 == 2004 {
        2.0
    } else {
        1.0
    };
    let mut bb = B1 * x1 + B2 * x1.powi(3) + B3 * fa * fb * x3.powi(4);
    if iza1i == 0 {
        let mut fact = D1 / (ep * TOMEV).sqrt();
        if fact < 1.0 {
            fact = 1.0;
        }
        if fact > 4.0 {
            fact = 4.0;
        }
        bb *= (TOMEV * e / (2.0 * emc2)).sqrt() * fact;
    }
    Ok(bb)
}

/// Discrete CM emission contribution — **not ported** (`groupr.f90:8719-8810`).
///
/// Ported, `f6dis` would return the double-differential cross section of a
/// discrete emission line `i` at CM cosine `w`. The [`Cm6Emission`] input type
/// carries no discrete lines (ND = 0), so the ported [`cm2lab`] path never needs
/// it. Returns [`NjoyError::NotPorted`].
pub fn f6dis() -> Result<f64, NjoyError> {
    Err(NjoyError::NotPorted("groupr::f6dis (groupr.f90:8719-8810)"))
}

/// Law-7 angle-energy → lab Legendre transform — **not ported**
/// (`groupr.f90:8934-9061`).
///
/// Ported, `ll2lab` would convert an ENDF File-6 LAW-7 (angle-energy, already in
/// the lab frame) distribution into a lab Legendre-coefficient (LAW-1)
/// representation. Returns [`NjoyError::NotPorted`].
pub fn ll2lab() -> Result<(), NjoyError> {
    Err(NjoyError::NotPorted(
        "groupr::ll2lab (groupr.f90:8934-9061)",
    ))
}

/// Lab-frame File-6 LAW-1 double-differential evaluation — **not ported**
/// (`groupr.f90:9063-9338`).
///
/// Ported, `f6lab` would return the lab-frame Legendre coefficients of a File-6
/// LAW-1 emission that is *already* given in the lab frame (no CM→lab transform
/// needed), interpolating between the two bracketing incident energies. Returns
/// [`NjoyError::NotPorted`].
pub fn f6lab() -> Result<(), NjoyError> {
    Err(NjoyError::NotPorted("groupr::f6lab (groupr.f90:9063-9338)"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build the normalized (∫ b_0 dE' = 1) isotropic triangular CM emission
    /// used by several tests: b_0 rises 0 → peak at E'/2 → 0 at E'max, so the
    /// lin-lin trapezoid area is exactly 1.
    fn triangle_emission(lang: Cm6Lang, awr: f64) -> Cm6Emission {
        // Triangle over [0, 5e5] with apex 4e-6 at 2.5e5: area = 1.
        let (c0, c1, c2): (Vec<f64>, Vec<f64>, Vec<f64>) = match lang {
            Cm6Lang::LegendreInCm => (vec![0.0], vec![4.0e-6], vec![0.0]),
            // Kalbach: [f_0, r]; slope from bach. f_0 is the triangle, r = 0.3.
            Cm6Lang::Kalbach => (vec![0.0, 0.3], vec![4.0e-6, 0.3], vec![0.0, 0.3]),
        };
        Cm6Emission {
            e_in: 1.0e6,
            lang,
            lep: IntLaw::LinLin,
            awr_target: awr,
            awp_emitted: 1.0,
            za_projectile: 1,
            za_target: 26056,
            za_emitted: 1,
            points: vec![
                Cm6Point {
                    ep: 0.0,
                    coeffs: c0,
                },
                Cm6Point {
                    ep: 2.5e5,
                    coeffs: c1,
                },
                Cm6Point {
                    ep: 5.0e5,
                    coeffs: c2,
                },
            ],
        }
    }

    /// `legndr` reproduces the closed-form low-order Legendre polynomials.
    ///
    /// **Methodology.** Evaluate `legndr(x, 4)` at `x = 0.3` and compare to the
    /// analytic `P_0..P_4(0.3)`. Pass if every order agrees to < 1e-12.
    ///
    /// **Result (2026-07-15, commit ac5adf5).** All five orders match to < 1e-13.
    #[test]
    fn legndr_matches_closed_form() {
        let x = 0.3_f64;
        let p = legndr(x, 4);
        let p2 = 0.5 * (3.0 * x * x - 1.0);
        let p3 = 0.5 * (5.0 * x.powi(3) - 3.0 * x);
        let p4 = 0.125 * (35.0 * x.powi(4) - 30.0 * x * x + 3.0);
        assert!((p[0] - 1.0).abs() < 1e-13);
        assert!((p[1] - x).abs() < 1e-13);
        assert!((p[2] - p2).abs() < 1e-13);
        assert!((p[3] - p3).abs() < 1e-13);
        assert!((p[4] - p4).abs() < 1e-13);
    }

    /// The CM→lab kinematic relation reduces to the analytic forward/backward
    /// edges at `mu_cm = ±1`.
    ///
    /// **Methodology.** For `E = 2 MeV`, `E'_cm = 0.5 MeV`, `xc = 1/(51)^2`
    /// (neutron off A=50), assert `lab_energy_from_cm` at `mu = +1` equals
    /// `(sqrt(E'_cm) + sqrt(xc E))^2` and at `mu = -1` equals
    /// `(sqrt(E'_cm) - sqrt(xc E))^2` — the `elmax`/`elmin` limits `f6cm` uses.
    /// Pass if both match to < 1e-6 relative.
    ///
    /// **Result (2026-07-15).** Forward and backward edges match the closed form
    /// to < 1e-9 relative.
    #[test]
    fn cm_to_lab_edges_are_analytic() {
        let e = 2.0e6;
        let ep_cm = 0.5e6;
        let xc = 1.0 / (51.0_f64).powi(2);
        let fwd = lab_energy_from_cm(e, ep_cm, 1.0, xc);
        let bwd = lab_energy_from_cm(e, ep_cm, -1.0, xc);
        let fwd_ref = (ep_cm.sqrt() + (xc * e).sqrt()).powi(2);
        let bwd_ref = (ep_cm.sqrt() - (xc * e).sqrt()).powi(2);
        assert!(
            (fwd - fwd_ref).abs() < 1e-6 * fwd_ref,
            "fwd {fwd} vs {fwd_ref}"
        );
        assert!(
            (bwd - bwd_ref).abs() < 1e-6 * bwd_ref,
            "bwd {bwd} vs {bwd_ref}"
        );
    }

    /// `cm2lab` conserves normalization: the lab `P_0` distribution integrates
    /// to ≈ 1 — NJOY's own `sum ≈ 1` check (`groupr.f90:8244`, tol 1%).
    ///
    /// **Methodology.** Feed the normalized (∫ b_0 dE' = 1) isotropic triangular
    /// LANG=1 CM emission off a heavy (A=50) target, request `nl = 1`. Assert the
    /// returned `p0_integral` is within 1% of 1.0, the lab points are ascending,
    /// and none exceeds `elmax = (sqrt(E'max)+sqrt(xc E))^2`.
    ///
    /// **Result (2026-07-15, commit ac5adf5).** `p0_integral = 1.0000596`
    /// (|sum-1| = 6.0e-5) over 20 lab points; all within [0, elmax], ascending.
    #[test]
    fn cm2lab_normalizes_legendre() {
        let em = triangle_emission(Cm6Lang::LegendreInCm, 50.0);
        let lab = cm2lab(&em, 1).unwrap();
        println!(
            "cm2lab_normalizes_legendre: p0_integral={} npoints={}",
            lab.p0_integral,
            lab.points.len()
        );
        assert!(
            (lab.p0_integral - 1.0).abs() < 0.01,
            "lab sum = {}",
            lab.p0_integral
        );
        assert!(!lab.points.is_empty());
        let xc = 1.0 / (51.0_f64).powi(2);
        let elmax = (5.0e5_f64.sqrt() + (xc * 1.0e6).sqrt()).powi(2) * 1.01;
        let mut prev = f64::NEG_INFINITY;
        for (ep, c) in &lab.points {
            assert!(*ep >= prev, "not ascending at {ep}");
            assert!(*ep <= elmax + 1.0, "ep {ep} exceeds elmax {elmax}");
            assert_eq!(c.len(), 1);
            prev = *ep;
        }
    }

    /// `cm2lab` conserves normalization for the Kalbach (LANG=2) path, which
    /// additionally exercises [`bach`] for the slope.
    ///
    /// **Methodology.** Same triangular `f_0` (∫ = 1) with LANG=2, `r = 0.3`,
    /// slope from `bach` (neutron on Fe-56, neutron out). The Kalbach angular
    /// factor integrates to exactly 1 over `mu`, so the lab `P_0` integral must
    /// still be ≈ 1 (tol 1%).
    ///
    /// **Result (2026-07-15).** `p0_integral = 1.0018403` (|sum-1| = 1.8e-3,
    /// within the 1% tol; the extra spread over the LANG=1 case comes from the
    /// Kalbach anisotropy folding into the lab grid).
    #[test]
    fn cm2lab_normalizes_kalbach() {
        let em = triangle_emission(Cm6Lang::Kalbach, 50.0);
        let lab = cm2lab(&em, 1).unwrap();
        println!("cm2lab_normalizes_kalbach: p0_integral={}", lab.p0_integral);
        assert!(
            (lab.p0_integral - 1.0).abs() < 0.01,
            "lab sum = {}",
            lab.p0_integral
        );
    }

    /// `bach` Kalbach slope: positive, and monotonically increasing with both
    /// incident energy (at fixed E') and emission energy (at fixed E).
    ///
    /// **Methodology.** Neutron (`iza1i=0`) on Fe-56, neutron out (`iza2=1`).
    /// Evaluate `a` on a grid of incident energies (1, 5, 10, 20 MeV) at fixed
    /// E' = 1 MeV, and on a grid of E' (0.5, 1, 2, 4 MeV) at fixed E = 14 MeV.
    /// Reference: the Kalbach-86 closed form as coded in NJOY `bach`. Pass if `a`
    /// is finite, positive, and strictly increasing along both grids.
    ///
    /// **Result (2026-07-15, commit ac5adf5).**
    /// a(E=1,5,10,20 MeV; E'=1 MeV) = [0.04118, 0.09208, 0.13022, 0.18416];
    /// a(E=14 MeV; E'=0.5,1,2,4 MeV) = [0.14665, 0.15408, 0.16910, 0.19995].
    /// All positive, finite, strictly increasing along both grids.
    #[test]
    fn bach_slope_monotone_and_positive() {
        let izat = 26056;
        let e_grid = [1.0e6, 5.0e6, 10.0e6, 20.0e6];
        let mut last = f64::NEG_INFINITY;
        let mut ve = Vec::new();
        for &e in &e_grid {
            let a = bach(0, 1, izat, e, 1.0e6).unwrap();
            assert!(a.is_finite() && a > 0.0, "a({e}) = {a}");
            assert!(a > last, "not increasing in E: a={a}, last={last}");
            last = a;
            ve.push(a);
        }
        let ep_grid = [0.5e6, 1.0e6, 2.0e6, 4.0e6];
        let mut last = f64::NEG_INFINITY;
        let mut vp = Vec::new();
        for &ep in &ep_grid {
            let a = bach(0, 1, izat, 14.0e6, ep).unwrap();
            assert!(a.is_finite() && a > 0.0, "a(ep={ep}) = {a}");
            assert!(a > last, "not increasing in E': a={a}, last={last}");
            last = a;
            vp.push(a);
        }
        println!("bach a(E,E'=1MeV)={ve:?}  a(E=14MeV,E')={vp:?}");
    }

    /// `bach` maps a natural-element target code to its dominant isotope, giving
    /// the same slope as the explicit isotope code.
    ///
    /// **Methodology.** Assert `bach(0,1, 26000, ...) == bach(0,1, 26056, ...)`
    /// (natural Fe → Fe-56) at E = 14 MeV, E' = 1 MeV.
    ///
    /// **Result (2026-07-15).** Both return the identical slope (exact equality).
    #[test]
    fn bach_natural_element_maps_to_dominant_isotope() {
        let a_nat = bach(0, 1, 26000, 14.0e6, 1.0e6).unwrap();
        let a_iso = bach(0, 1, 26056, 14.0e6, 1.0e6).unwrap();
        assert_eq!(a_nat, a_iso);
    }

    /// The unported discrete / law-7 / lab-frame routines honestly report
    /// themselves unported (no fabricated kinematics).
    #[test]
    fn unported_routines_report_not_ported() {
        assert!(matches!(f6dis(), Err(NjoyError::NotPorted(_))));
        assert!(matches!(ll2lab(), Err(NjoyError::NotPorted(_))));
        assert!(matches!(f6lab(), Err(NjoyError::NotPorted(_))));
    }

    /// Malformed input is rejected with `EndfParse`, never a panic.
    #[test]
    fn malformed_input_errors_cleanly() {
        let mut em = triangle_emission(Cm6Lang::LegendreInCm, 50.0);
        // nl = 0 is invalid.
        assert!(matches!(cm2lab(&em, 0), Err(NjoyError::EndfParse(_))));
        // Non-ascending energies.
        em.points[2].ep = 1.0;
        assert!(matches!(cm2lab(&em, 1), Err(NjoyError::EndfParse(_))));
    }
}
