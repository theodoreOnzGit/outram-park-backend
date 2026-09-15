// Ported from NJOY2016 `src/groupr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! CM-frame File-6 LAW-1 evaluation and its CM→lab transform: `f6ddx`, `f6cm`,
//! `f6dis`, and the public entry point `cm2lab`.
//!
//! # Fortran routine → Rust item map
//!
//! | Fortran routine | lines | Rust item | Status |
//! |---|---|---|---|
//! | `f6ddx`   | 8520–8717 | [`Cm6Emission::f6ddx`] | **DONE** for `lang=1,2` continuum; `lang=0` (phase space) and `lang>=11` (tabulated) → `NotPorted` |
//! | `f6dis`   | 8719–8810 | [`Cm6Emission::f6dis`] | **DONE** for `lang=1,2`; see the doc for a literal upstream indexing quirk this reproduces |
//! | `f6cm`    | 8260–8518 | [`Cm6Emission::f6cm`] | **DONE**, including the `ND` discrete-line branches (init bounds `8324-8335`, delta-function branch `8467-8501`) |
//! | `cm2lab`  | 8135–8258 | [`cm2lab`] | **DONE** |

use super::bach::bach;
use super::shared::{legndr, remove_small_moments, LabDistribution, EMAX, SMALL, TINY};
use crate::endf::interp::{terp1, IntLaw};
use crate::NjoyError;

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

/// One secondary-energy record of a File-6 LAW-1 CM continuum distribution,
/// **or** one discrete emission line of the same subsection.
///
/// Mirrors one `(E', b_0, b_1, ...)` entry of NJOY's `cnow` list
/// (`groupr.f90:8609-8658`) — the continuum and discrete-line tables share
/// this exact per-record layout in the raw ENDF data. `ep` is the CM
/// secondary energy \[eV\]; `coeffs` is the `LANG`-dependent coefficient list
/// (see [`Cm6Lang`]). All `coeffs` entries across the points *and* discrete
/// lines of one [`Cm6Emission`] must have the same length.
#[derive(Debug, Clone)]
pub struct Cm6Point {
    /// CM secondary (outgoing-particle) energy `E'` \[eV\].
    pub ep: f64,
    /// `LANG`-dependent coefficient list (Legendre `b_L`, or Kalbach `f_0, r[, a]`).
    pub coeffs: Vec<f64>,
}

/// An owned File-6 LAW-1 CM-frame emission block (one incident energy).
///
/// This is the Rust analogue of NJOY's `cnow` array for a single incident-energy
/// subsection (`groupr.f90` `f6cm`/`f6ddx`/`f6dis` consume `cnow(1..)`),
/// restricted to the ported path: **LAW 1, CM frame, LANG 1 or 2**. `points`
/// holds the smooth continuum part (possibly empty, for a purely-discrete
/// emission — ENDF `ND = NEP`); `discrete` holds the `ND` delta-function lines
/// (possibly empty, `ND = 0`, the ordinary continuum-only case).
/// [`Cm6Emission::to_lab`] / [`cm2lab`] transform it to the lab frame.
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
/// - `points` — the continuum records, **ascending** in `ep`. Empty for a
///   purely-discrete emission.
/// - `discrete` — the `ND` discrete emission lines (ENDF order, same record
///   layout as `points`). Empty for `ND = 0`.
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
    /// Continuum secondary-energy records, ascending in `ep`. Empty for a
    /// purely-discrete emission (`ND = NEP`).
    pub points: Vec<Cm6Point>,
    /// Discrete emission lines (ENDF `ND`), same record layout as `points`.
    /// Empty for `ND = 0` (the ordinary continuum-only case).
    pub discrete: Vec<Cm6Point>,
}

impl Cm6Emission {
    /// Number of CM angular parameters `NA` per record (`ncnow - 2` in NJOY).
    ///
    /// For `LANG = 1` this is the top Legendre order; for `LANG = 2` it is 1
    /// (slope from [`bach`]) or 2 (slope tabulated). Falls back to the
    /// discrete-line width when `points` is empty (a purely-discrete
    /// emission), matching NJOY's single shared `na`/`ncnow` computed once
    /// from the whole `cnow` table regardless of which entries are discrete.
    fn na(&self) -> usize {
        let width = self
            .points
            .first()
            .or(self.discrete.first())
            .map(|p| p.coeffs.len())
            .unwrap_or(0);
        width.saturating_sub(1)
    }

    /// f6ddx **initialization** (`groupr.f90:8552-8571`, `ep == 0` branch).
    ///
    /// Returns `(epnext0, epmax)`: the first CM secondary-energy break point
    /// (clamped to `0.05 * epmax`, NJOY's `step`) and the maximum CM secondary
    /// energy \[eV\]. Only called when `points` is non-empty (continuum present).
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
    /// is returned; discrete deltas are handled separately by [`Self::f6dis`].
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

    /// Discrete-line double-differential value at CM cosine `w` — faithful port
    /// of `f6dis` (`groupr.f90:8719-8810`), given the already-selected line
    /// index `i` (0-based into `self.discrete`).
    ///
    /// # A literal upstream indexing quirk, reproduced on purpose
    ///
    /// `f6dis`'s `LANG = 1` (Legendre) branch reads `cnow(inow+1+l)` for
    /// `l = 1..nl` (`groupr.f90:8767-8769`) — **one slot further** than the
    /// analogous continuum read in `f6ddx` (`cnow(lnow+l)`,
    /// `groupr.f90:8608`). Comparing against `f6dis`'s own `LANG = 2` branch
    /// (`cnow(inow+1)`, `cnow(inow+2)`, `cnow(inow+3)` — exactly matching
    /// `f6ddx`'s Kalbach offsets) shows this is specific to the Legendre
    /// branch, not a discrete-vs-continuum record-layout difference. The
    /// practical effect: for `l = 1 .. nl-1` the term uses `coeffs[l]` of
    /// *this* line (so `coeffs[0]` is never summed — it is instead pulled out
    /// as the separate final multiplier NJOY calls `cnow(inow+1)`), and the
    /// **top-order term (`l = nl`) reads one slot past this line's own
    /// `coeffs`** — which, in NJOY's flat `cnow` array, is the CM **energy**
    /// of the *next* discrete line (`discrete[i+1].ep` here), not a Legendre
    /// coefficient at all. This is ported faithfully (bugs and all — see the
    /// crate's translation policy); for the *last* line, where there is no
    /// next record to (mis)read, NJOY reads past the whole subsection into
    /// undefined memory, which we cannot reproduce and instead report as
    /// [`NjoyError::EndfParse`] rather than fabricate a value. See the module
    /// hand-off notes for the V&V pass.
    fn f6dis(&self, i: usize, w: f64) -> Result<f64, NjoyError> {
        let lines = &self.discrete;
        let na = self.na();
        let nl = na + 1;
        match self.lang {
            Cm6Lang::LegendreInCm => {
                let p = legndr(w, na);
                let mut t = 0.0;
                for l in 1..=nl {
                    let x = if l < nl {
                        lines[i].coeffs[l]
                    } else {
                        match lines.get(i + 1) {
                            Some(next) => next.ep,
                            None => {
                                return Err(NjoyError::EndfParse(
                                    "f6dis: LANG=1 top-order term of the last discrete line \
                                     reads past the table (upstream groupr.f90:8767-8769 \
                                     off-by-one; no data available to reproduce it faithfully)"
                                        .into(),
                                ))
                            }
                        }
                    };
                    t += (2.0 * l as f64 - 1.0) * x * p[l - 1] / 2.0;
                }
                t *= lines[i].coeffs[0];
                Ok(t.max(0.0))
            }
            Cm6Lang::Kalbach => {
                let s = lines[i].coeffs[0];
                let r = lines[i].coeffs[1];
                let aa = if na == 2 {
                    lines[i].coeffs[2]
                } else {
                    bach(
                        self.za_projectile,
                        self.za_emitted,
                        self.za_target,
                        self.e_in,
                        lines[i].ep,
                    )?
                };
                let mut t = aa * ((aa * w).cosh() + r * (aa * w).sinh()) / (2.0 * aa.sinh());
                t *= s;
                Ok(t.max(0.0))
            }
        }
    }

    /// Lab-frame Legendre coefficients at lab secondary energy `ep` \[eV\] —
    /// faithful port of `f6cm` (`groupr.f90:8260-8518`), covering both the
    /// continuum branch (`215`, when `points` is non-empty) and the
    /// purely-discrete delta-function branch (`330`, when `points` is empty —
    /// NJOY's `ndnow.eq.npnow`).
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
    /// The discrete branch instead inverts the two-body kinematics directly for
    /// each line's fixed CM energy to find the CM cosine giving this lab `ep`.
    ///
    /// **A limitation this reproduces, not fixes:** for a *mixed* emission
    /// (`points` non-empty **and** `discrete` non-empty, ENDF `0 < ND < NEP`),
    /// upstream `f6cm` takes the continuum branch unconditionally
    /// (`groupr.f90:8347`, `if (ndnow.eq.npnow) go to 330` — the discrete branch
    /// is reached *only* when there is no continuum at all) and never adds the
    /// discrete lines' own contribution inside the per-point loop; they only
    /// widen the `elmax`/`epnext` search bounds computed once at init
    /// (`groupr.f90:8324-8335`, ported in [`Cm6Emission::to_lab`]). This port
    /// reproduces that: a mixed emission's discrete lines never contribute to
    /// `term` here, exactly as upstream never does.
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
        if self.points.is_empty() {
            return self.f6cm_discrete_only(ep, xc, nl_lab);
        }

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

    /// The purely-discrete `f6cm` branch (`330`, `groupr.f90:8467-8501`):
    /// invert the two-body kinematics for each discrete line's fixed CM energy
    /// to find the CM cosine `w` giving lab energy `ep`, then add
    /// `f6dis(i, w) * P_l(u)` (`u` the corresponding **lab** cosine) into
    /// `term`. Reached only when `self.points.is_empty()` (NJOY's
    /// `ndnow.eq.npnow`).
    fn f6cm_discrete_only(
        &self,
        ep: f64,
        xc: f64,
        nl_lab: usize,
    ) -> Result<(Vec<f64>, f64), NjoyError> {
        const CHECK: f64 = 0.999_99;
        const RDN: f64 = 0.999_995;
        const RUP: f64 = 1.000_005;
        let e = self.e_in;
        let na_lab = nl_lab - 1;
        let mut term = vec![0.0_f64; nl_lab];
        let mut epnxt = EMAX;

        for (i, line) in self.discrete.iter().enumerate() {
            let epp = line.ep;
            let denom = 2.0 * (xc * e * epp).sqrt();
            let w = (ep - epp - xc * e) / denom;
            let u = (xc * e + ep - epp) / (2.0 * (xc * e * ep).sqrt());
            if w < -1.0 {
                let mut epn = e * ((epp / e).sqrt() - xc.sqrt()).powi(2);
                epn *= if ep < CHECK * epn { RDN } else { RUP };
                if epn < epnxt * (1.0 - SMALL) {
                    epnxt = epn;
                }
            } else if w <= 1.0 {
                let p = legndr(u, na_lab);
                let f = self.f6dis(i, w)? / denom;
                let mut epn = e * ((epp / e).sqrt() + xc.sqrt()).powi(2);
                epn *= if ep < CHECK * epn { RDN } else { RUP };
                if epn < epnxt * (1.0 - SMALL) {
                    epnxt = epn;
                }
                for l in 1..=nl_lab {
                    term[l - 1] += f * p[l - 1];
                }
            }
            // w > 1: neither branch — no contribution, no epnxt update
            // (groupr.f90:8479 `else if (w.le.wmax)`, falls through otherwise).
        }

        remove_small_moments(&mut term, nl_lab, 0.005);
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
    /// - [`NjoyError::EndfParse`] if both `points` and `discrete` are empty,
    ///   not ascending, `nl == 0`, or the coefficient lists are inconsistent /
    ///   too short for `lang`.
    pub fn to_lab(&self, nl: usize) -> Result<LabDistribution, NjoyError> {
        // Local constants (groupr.f90:8152-8156, 8286-8296, 8324-8335).
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

        // --f6cm initialization (groupr.f90:8299-8340).
        let has_continuum = !self.points.is_empty();
        let mut epnn = EMAX;
        let mut epmax_c = 0.0_f64;
        let mut elmax = 0.0_f64;
        let mut epnext = EMAX;
        let mut eps = TINY; // meaningless (never used) on the pure-discrete path

        if has_continuum {
            let (epn0, epmax) = self.f6ddx_init();
            epnn = EMAX.min(epn0);
            epmax_c = epmax.max(0.0);
            elmax = e * ((epmax_c / e).sqrt() + xc.sqrt()).powi(2);
            elmax *= UP;
            let mut epn = STEP * elmax;
            if epmax_c * UP < xc * e {
                epn = e * ((epmax_c / e).sqrt() - xc.sqrt()).powi(2);
            }
            epnext = EMAX.min(epn);
            eps = TINY / elmax;
            epnext *= DN;
        }
        if !self.discrete.is_empty() {
            for line in &self.discrete {
                let epn = line.ep;
                if epn < epnn * (1.0 - SMALL) {
                    epnn = epn;
                }
                if epn > epmax_c * (1.0 + SMALL) {
                    epmax_c = epn;
                }
                let epx_lo = DN * e * ((epn / e).sqrt() - xc.sqrt()).powi(2);
                if epx_lo < epnext * (1.0 - SMALL) {
                    epnext = epx_lo;
                }
                let epx_hi = UP * e * ((epn / e).sqrt() + xc.sqrt()).powi(2);
                if epx_hi > elmax * (1.0 + SMALL) {
                    elmax = epx_hi;
                }
            }
        }
        let _ = epnn; // epnn only feeds the (already-applied) bounds above.

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

    /// Validate the input record shape for the ported `LANG 1 / 2` path.
    fn validate(&self, nl: usize) -> Result<(), NjoyError> {
        if nl == 0 {
            return Err(NjoyError::EndfParse("cm2lab: nl must be >= 1".into()));
        }
        if self.points.is_empty() && self.discrete.is_empty() {
            return Err(NjoyError::EndfParse(
                "cm2lab: no CM emission points (points and discrete both empty)".into(),
            ));
        }
        let width = self
            .points
            .first()
            .or(self.discrete.first())
            .unwrap()
            .coeffs
            .len();
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
        for pt in self.points.iter().chain(self.discrete.iter()) {
            if pt.coeffs.len() != width {
                return Err(NjoyError::EndfParse(
                    "cm2lab: inconsistent coeff-list widths across points/discrete".into(),
                ));
            }
            if pt.ep < 0.0 {
                return Err(NjoyError::EndfParse(
                    "cm2lab: CM secondary energies must be >= 0".into(),
                ));
            }
        }
        for pt in &self.points {
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

/// Transform a CM-frame File-6 LAW-1 emission into the lab frame.
///
/// Faithful port of NJOY2016 `cm2lab` (`groupr.f90:8135-8258`) for the ported
/// scope (LAW 1, CM frame, LANG 1 or 2). This is the public entry point of the
/// CM-frame half of this module; it forwards to [`Cm6Emission::to_lab`].
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
///     discrete: vec![],
/// };
/// let lab = cm2lab(&emission, 1).unwrap();
/// assert!((lab.p0_integral - 1.0).abs() < 0.01);
/// ```
pub fn cm2lab(emission: &Cm6Emission, nl: usize) -> Result<LabDistribution, NjoyError> {
    emission.to_lab(nl)
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
            discrete: vec![],
        }
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
