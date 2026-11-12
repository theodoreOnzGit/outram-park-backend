// Ported from NJOY2016 `src/reconr.f90` (https://github.com/njoy/NJOY2016, version 2016.80,
// commit ab27c6414a0f25b173f2b637e6dc7e428fdaff00):
//   - `subroutine csunr1`, l.3826-4077 (unresolved range, format 1: SLBW, `LRF=1`)
//   - `subroutine csunr2`, l.4079-4317 (unresolved range, format 2: `LRF=2`)
//   - `subroutine unfac`,  l.4473-4496 (penetrability and phase shift)
//   - `subroutine gnrl`,   l.4498-4644 (fluctuation integrals)
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under AGPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! RECONR's own unresolved-range cross sections, `csunr1` and `csunr2`,
//! translated line for line (gh:#1448).
//!
//! Until 2026-11-11 this port evaluated the unresolved range with UNRESR's
//! kernel ([`crate::unresr::unresolved_cross_sections`]) handed one dilution
//! of `1e10`. That agrees to the printed 7 figures where `csunr` evaluates the
//! parameters at each energy, but not where `csunr1`/`csunr2` **interpolate the
//! cross sections between stored panel end points** — the state machine below
//! (`ilast`, `enxt`, `elst`, `slst`, `snxt`; `wide = 1.26`). That is the
//! "one unit in the 7th figure" of ENDF/B-V U-235 (`LFW=1`), the case this
//! module closes.
//!
//! # State
//!
//! Upstream keeps the panel state in `reconr`'s module variables, indexed by
//! the section number `isect`, and resets all of it with a call at `e = 0`.
//! [`PanelState`] is that state for one section; [`UnrState`] holds one per
//! unresolved range, in tape order, and is created fresh per RECONR run (the
//! `e = 0` call).
//!
//! # Upstream quirks kept, by decision (parity with the NJOY2016 tape)
//!
//! 1. `unfac` has no `l >= 3` branch, so `vl` and `ps` keep the values of the
//!    previous J-state (or L-state) and `csunr1` multiplies `vl` by `sqrt(E)`
//!    again per J. [`unfac`] reproduces that: the caller owns `vl` and `ps`.
//!    (Fe-58 ENDF/B-VIII.0 Beta4, `NLS = 4`: see
//!    `tests/reconr_mt152_all_unresolved_cases_vs_njoy2016.rs`.)
//! 2. In `csunr1`, the local `ne` is `0` at the panel test `if (ie.eq.ne)`
//!    (it is assigned only in the evaluation block below it), so that branch
//!    never fires. Kept; the consequence is that an energy above the last
//!    tabulated fission-width energy walks off the end of the list
//!    (upstream reads whatever follows it in `res`); here that is an error.
//! 3. `csunr2`'s final panel interpolation uses the `INT` of the **last**
//!    J-state when the energy was evaluated this call, and that of the first
//!    otherwise (`int` is overwritten in the J loop).

use petir::mathf::RealMath;

use crate::common::phys::PI;
use crate::endf::interp::{terp1, IntLaw};
use crate::reconr::slbw::WAVE_K;
use crate::unresr::channel_radius_urr;
use crate::unresr::mf2::{UnresolvedCase, UnresolvedRange};
use crate::NjoyError;

/// `wide = 1.26`, the panel-width ratio above which parameters rather than
/// cross sections are interpolated (`csunr1`, `csunr2`).
const WIDE: f64 = 1.26;
/// `small = 1.e-8` (`csunr2`).
const SMALL: f64 = 1.0e-8;

/// Panel state of one section: upstream's `ilast`, `enxt`, `elst`, `snxt`,
/// `slst` at index `isect` (`reconr.f90:26-27`).
#[derive(Debug, Clone, Copy, Default)]
pub struct PanelState {
    pub ilast: i32,
    enxt: f64,
    elst: f64,
    snxt: [f64; 4],
    slst: [f64; 4],
}

/// One [`PanelState`] per unresolved range: the result of the `e = 0` call.
#[derive(Debug, Clone)]
pub struct UnrState(Vec<PanelState>);

impl UnrState {
    /// `call csunr1(0,sigu); call csunr2(0,sigu)` (`genunr`, `:1667-1668`).
    pub fn new(nsect: usize) -> Self {
        UnrState(vec![PanelState::default(); nsect])
    }

    /// The cross sections `[total, elastic, fission, capture]` of range
    /// `isect` at `e`, by `csunr1` for `LRF=1` ranges and `csunr2` for `LRF=2`
    /// (`mode = 11` and `12` in `genunr`, `:1682-1683`).
    pub fn evaluate(
        &mut self,
        isect: usize,
        range: &UnresolvedRange,
        e: f64,
    ) -> Result<[f64; 4], NjoyError> {
        match range.case_ {
            UnresolvedCase::CaseC { .. } => csunr2(range, &mut self.0[isect], e),
            _ => csunr1(range, &mut self.0[isect], e),
        }
    }
}

/// `unfac` (`reconr.f90:4473-4496`): penetrability factor `vl` and phase
/// shift `ps`. For `l >= 3` neither is assigned (no `else` upstream), so the
/// caller's values stand.
pub fn unfac(l: i32, rho: f64, rhoc: f64, amun: f64, vl: &mut f64, ps: &mut f64) {
    let r2 = rho * rho;
    if l == 0 {
        *vl = amun;
        *ps = rhoc;
    } else if l == 1 {
        *vl = amun * r2 / (1.0 + r2);
        *ps = rhoc - rhoc.r_atan();
    } else if l == 2 {
        let r4 = r2 * r2;
        *vl = amun * r4 / (9.0 + 3.0 * r2 + r4);
        *ps = rhoc - (3.0 * rhoc / (3.0 - rhoc * rhoc)).r_atan();
    }
}

/// `gnrl` quadrature weights, `qw(j,mu)` stored as `QW[mu-1][j-1]`
/// (`reconr.f90:4504-4519`).
const QW: [[f64; 10]; 4] = [
    [1.1120413e-1, 2.3546798e-1, 2.8440987e-1, 2.2419127e-1, 0.10967668, 0.030493789, 0.0042930874, 2.5827047e-4, 4.9031965e-6, 1.4079206e-8],
    [0.033773418, 0.079932171, 0.12835937, 0.17652616, 0.21347043, 0.21154965, 0.13365186, 0.022630659, 1.6313638e-5, 2.745383e-31],
    [3.3376214e-4, 0.018506108, 0.12309946, 0.29918923, 0.33431475, 0.17766657, 0.042695894, 4.0760575e-3, 1.1766115e-4, 5.0989546e-7],
    [1.7623788e-3, 0.021517749, 0.080979849, 0.18797998, 0.30156335, 0.29616091, 0.10775649, 2.5171914e-3, 8.9630388e-10, 0.0],
];

/// `gnrl` quadrature nodes, `qp(j,mu)` stored as `QP[mu-1][j-1]`
/// (`reconr.f90:4520-4535`).
const QP: [[f64; 10]; 4] = [
    [3.0013465e-3, 7.8592886e-2, 4.3282415e-1, 1.3345267, 3.0481846, 5.8263198, 9.9452656, 1.5782128e1, 23.996824, 36.216208],
    [1.3219203e-2, 7.2349624e-2, 0.19089473, 0.39528842, 0.74083443, 1.3498293, 2.5297983, 5.2384894, 13.821772, 75.647525],
    [1.0004488e-3, 0.026197629, 0.14427472, 0.44484223, 1.0160615, 1.9421066, 3.3150885, 5.2607092, 7.9989414, 12.072069],
    [0.013219203, 0.072349624, 0.19089473, 0.39528842, 0.74083443, 1.3498293, 2.5297983, 5.2384894, 13.821772, 75.647525],
];

/// `gnrl` (`reconr.f90:4498-4644`): the fluctuation integral `id` (1 elastic,
/// 2 capture, 3 fission) for degrees of freedom `mu` (neutron), `nu`
/// (fission) and `lamda` (competitive), widths `galpha`, `gbeta`, `gamma` and
/// competitive width `df`.
///
/// `id = 3` with `gbeta = 0` is not handled upstream and returns zero, as here.
/// A degrees-of-freedom value outside `1..=4` that the branch indexes is an
/// error (upstream indexes out of bounds).
#[allow(clippy::too_many_arguments)]
pub fn gnrl(
    galpha: f64,
    gbeta: f64,
    gamma: f64,
    mu: i32,
    nu: i32,
    lamda: i32,
    df: f64,
    id: i32,
) -> Result<f64, NjoyError> {
    let mut s = 0.0_f64;
    if galpha <= 0.0 || gamma <= 0.0 || gbeta < 0.0 || (gbeta > 0.0 && df < 0.0) {
        return Ok(s);
    }
    let idx = |d: i32, what: &str| -> Result<usize, NjoyError> {
        if (1..=4).contains(&d) {
            Ok((d - 1) as usize)
        } else {
            Err(NjoyError::EndfParse(format!("csunr: degrees of freedom {what} = {d} outside 1..4")))
        }
    };
    let (qw, qp) = (&QW, &QP);
    if gbeta == 0.0 && df == 0.0 {
        let m = idx(mu, "mu")?;
        if id == 1 {
            for j in 0..10 {
                s += qw[m][j] * qp[m][j] * qp[m][j] / (galpha * qp[m][j] + gamma);
            }
        } else if id == 2 {
            for j in 0..10 {
                s += qw[m][j] * qp[m][j] / (galpha * qp[m][j] + gamma);
            }
        }
        return Ok(s);
    }
    if gbeta == 0.0 && df > 0.0 {
        let (m, la) = (idx(mu, "mu")?, idx(lamda, "lamda")?);
        if id == 1 {
            for j in 0..10 {
                let xj = qp[m][j];
                for k in 0..10 {
                    s += qw[m][j] * qw[la][k] * xj * xj / (galpha * xj + gamma + df * qp[la][k]);
                }
            }
        } else if id == 2 {
            for j in 0..10 {
                let xj = qp[m][j];
                for k in 0..10 {
                    s += qw[m][j] * qw[la][k] * xj / (galpha * xj + gamma + df * qp[la][k]);
                }
            }
        }
        return Ok(s);
    }
    if gbeta > 0.0 && df == 0.0 {
        let (m, n) = (idx(mu, "mu")?, idx(nu, "nu")?);
        if id == 1 {
            for j in 0..10 {
                let xj = qp[m][j];
                for k in 0..10 {
                    s += qw[m][j] * qw[n][k] * xj * xj / (galpha * xj + gbeta * qp[n][k] + gamma);
                }
            }
        } else if id == 2 {
            for j in 0..10 {
                let xj = qp[m][j];
                for k in 0..10 {
                    s += qw[m][j] * qw[n][k] * xj / (galpha * xj + gbeta * qp[n][k] + gamma);
                }
            }
        } else if id == 3 {
            for j in 0..10 {
                for k in 0..10 {
                    s += qw[m][j] * qw[n][k] * qp[m][j] * qp[n][k]
                        / (galpha * qp[m][j] + gbeta * qp[n][k] + gamma);
                }
            }
        }
        return Ok(s);
    }
    if gbeta > 0.0 && df > 0.0 {
        let (m, n, la) = (idx(mu, "mu")?, idx(nu, "nu")?, idx(lamda, "lamda")?);
        if id == 1 {
            for j in 0..10 {
                let xj = qp[m][j];
                for k in 0..10 {
                    for l in 0..10 {
                        s += qw[m][j] * qw[n][k] * qw[la][l] * xj * xj
                            / (galpha * xj + gbeta * qp[n][k] + gamma + df * qp[la][l]);
                    }
                }
            }
        } else if id == 2 {
            for j in 0..10 {
                for k in 0..10 {
                    for l in 0..10 {
                        s += qw[m][j] * qw[n][k] * qw[la][l] * qp[m][j]
                            / (galpha * qp[m][j] + gbeta * qp[n][k] + gamma + df * qp[la][l]);
                    }
                }
            }
        } else if id == 3 {
            for j in 0..10 {
                for k in 0..10 {
                    for l in 0..10 {
                        s += qw[m][j] * qw[n][k] * qw[la][l] * qp[m][j] * qp[n][k]
                            / (galpha * qp[m][j] + gbeta * qp[n][k] + gamma + df * qp[la][l]);
                    }
                }
            }
        }
        return Ok(s);
    }
    Ok(s)
}

/// The labels of the `go to` network shared by `csunr1`/`csunr2`.
#[derive(Clone, Copy)]
enum Lab {
    L110,
    L120,
    L130,
    /// `csunr2` 185 / `csunr1` 200: `e = ee`, recompute at the original energy.
    Recompute,
    /// `csunr1` 205: `ifirst = 1`, then interpolate.
    L205,
    /// The panel interpolation (`csunr1` 210, `csunr2` 200).
    Interpolate,
    Done,
}

/// The channel radius `aa` and scattering radius `ay` at `e`: the shared
/// head of `csunr1` (`:3926-3951`) and `csunr2` (`:4189-4203`).
fn radii(range: &UnresolvedRange, awri: f64, e: f64) -> Result<(f64, f64), NjoyError> {
    let ay = range.scattering_radius(e)?;
    let aa = channel_radius_urr(awri, range.naps, range.nro, ay, range.case_.ap()).map_err(|_| {
        NjoyError::EndfParse(format!("csunr: illegal naps={} (nro={})", range.naps, range.nro))
    })?;
    Ok((aa, ay))
}

/// `csunr1` (`reconr.f90:3826-4077`) for a `LFW = 0` or `LFW = 1`, `LRF = 1`
/// range at `e > 0`. `lfw = 1` interpolates the cross sections linearly
/// between stored panel end points on the fission-width energy grid unless the
/// panel is wider than `wide`, where the parameters are interpolated.
pub fn csunr1(range: &UnresolvedRange, st: &mut PanelState, e0: f64) -> Result<[f64; 4], NjoyError> {
    let (awri, spi, lfw, fe, lstates_a, lstates_b) = match &range.case_ {
        UnresolvedCase::CaseA { awri, spi, l_states, .. } => (*awri, *spi, 0, &[][..], Some(l_states), None),
        UnresolvedCase::CaseB { awri, spi, fission_energies, l_states, .. } => {
            (*awri, *spi, 1, &fission_energies[..], None, Some(l_states))
        }
        UnresolvedCase::CaseC { .. } => {
            return Err(NjoyError::EndfParse("csunr1 called on an LRF=2 range".into()));
        }
    };
    let cwaven = WAVE_K;
    let mut e = e0;
    let ee = e;
    let mut ifirst = st.ilast;
    let mut enext;
    let mut elast;
    let mut ie: i32;
    let mut sigp = [0.0_f64; 4];
    let mut lab;
    // `vl` and `ps` persist across the loops of one call (see `unfac`).
    let (mut vl, mut ps) = (0.0_f64, 0.0_f64);
    let mut spot = 0.0_f64;

    if ifirst > 0 {
        // 100
        enext = st.enxt;
        elast = st.elst;
        ie = st.ilast;
        lab = if lfw == 0 || (e < enext && enext >= WIDE * elast) {
            Lab::L130
        } else if e <= enext {
            Lab::Interpolate
        } else {
            // `if (ie.eq.ne) go to 120` with `ne = 0` here (module doc, 2).
            Lab::L110
        };
    } else {
        enext = e;
        elast = 0.0;
        st.enxt = e;
        st.elst = 0.0;
        ie = 1;
        lab = Lab::L120;
    }

    let mut guard = 0;
    loop {
        guard += 1;
        if guard > 64 {
            return Err(NjoyError::EndfParse("csunr1: panel logic did not terminate".into()));
        }
        match lab {
            Lab::L110 => {
                elast = enext;
                st.elst = elast;
                st.slst = st.snxt;
                ie += 1;
                enext = *fe.get((ie - 1) as usize).ok_or_else(|| {
                    NjoyError::EndfParse(
                        "csunr1: energy above the last fission-width energy (upstream reads past the list)".into(),
                    )
                })?;
                st.enxt = enext;
                st.ilast = ie;
                lab = Lab::L120;
            }
            Lab::L120 => {
                e = enext;
                lab = Lab::L130;
            }
            Lab::L130 => {
                // compute the unresolved cross sections at `e`
                sigp = [0.0; 4];
                let (aa, ay) = radii(range, awri, e)?;
                let rat = awri / (awri + 1.0);
                let const_ = (2.0 * PI * PI) / ((cwaven * rat) * (cwaven * rat));
                let nls = lstates_a.map_or_else(|| lstates_b.map_or(0, |b| b.len()), |a| a.len());
                for li in 0..nls {
                    let (ll, njs) = match (lstates_a, lstates_b) {
                        (Some(a), _) => (a[li].l, a[li].j_states.len()),
                        (_, Some(b)) => (b[li].l, b[li].j_states.len()),
                        _ => unreachable!("one of the two is set"),
                    };
                    for j in 0..njs {
                        // (muf, dx, aj, amun, gnox, ggx, fission widths)
                        let (muf, dx, aj, amun, gnox, ggx, gfs): (i32, f64, f64, f64, f64, f64, &[f64]) =
                            match (lstates_a, lstates_b) {
                                (Some(a), _) => {
                                    let s = &a[li].j_states[j];
                                    (0, s.d, s.aj, s.amun, s.gno, s.gg, &[][..])
                                }
                                (_, Some(b)) => {
                                    let s = &b[li].j_states[j];
                                    (s.amuf.round() as i32, s.d, s.aj, s.amun, s.gno, s.gg, &s.gf[..])
                                }
                                _ => unreachable!("one of the two is set"),
                            };
                        let gxx = 0.0_f64;
                        let mut gfx = 0.0_f64;
                        let gj = (2.0 * aj + 1.0) / (4.0 * spi + 2.0);
                        let mu = amun.trunc() as i32;
                        let nu = muf;
                        let lamda = 0;
                        if lfw != 0 {
                            // interpolate the fission width at `e` on panel `ie`
                            let i1 = if ie > 1 { (ie - 2) as usize } else { 0 };
                            if i1 + 1 >= fe.len() || i1 + 1 >= gfs.len() {
                                return Err(NjoyError::EndfParse(
                                    "csunr1: fission-width panel outside the tabulated energies".into(),
                                ));
                            }
                            gfx = terp1(fe[i1], gfs[i1], fe[i1 + 1], gfs[i1 + 1], e, IntLaw::LinLin)?;
                        }
                        let e2 = e.sqrt();
                        let k = rat * e2 * cwaven;
                        let rho = k * aa;
                        let rhoc = k * ay;
                        unfac(ll, rho, rhoc, amun, &mut vl, &mut ps);
                        vl *= e2;
                        if j == 0 {
                            spot = 4.0 * PI * (2.0 * ll as f64 + 1.0) * (ps.r_sin() / k).r_powi(2);
                        }
                        let gnx = gnox * vl;
                        let diff = gxx;
                        let den = e * dx;
                        let temp = const_ * gj * gnx / den;
                        let terg = temp * ggx;
                        let ters = temp * gnx;
                        let terf = temp * gfx;
                        let mut gs = gnrl(gnx, gfx, ggx, mu, nu, lamda, diff, 1)?;
                        let mut gc = gnrl(gnx, gfx, ggx, mu, nu, lamda, diff, 2)?;
                        let mut gff = gnrl(gnx, gfx, ggx, mu, nu, lamda, diff, 3)?;
                        gc *= terg;
                        gff *= terf;
                        gs *= ters;
                        let mut add = const_ * gj * 2.0 * gnx * ps.r_sin().r_powi(2);
                        add /= e * dx;
                        gs -= add;
                        sigp[1] += gs;
                        sigp[2] += gff;
                        sigp[3] += gc;
                    }
                    sigp[1] += spot;
                }
                sigp[0] = sigp[1] + sigp[2] + sigp[3];
                if ifirst == 0 && lfw == 0 {
                    ifirst = 1;
                }
                if lfw == 0 || (ifirst > 0 && e < enext && enext > WIDE * elast) {
                    lab = Lab::Done;
                } else {
                    st.snxt = sigp;
                    if ifirst > 0 && e == enext && enext > WIDE * elast {
                        lab = Lab::Recompute;
                    } else if ifirst > 0 {
                        lab = Lab::Interpolate;
                    } else if ifirst < 0 {
                        lab = Lab::L205;
                    } else {
                        ifirst = -1;
                        lab = Lab::L110;
                    }
                }
            }
            Lab::Recompute => {
                e = ee;
                lab = Lab::L130;
            }
            Lab::L205 => {
                ifirst = 1;
                lab = Lab::Interpolate;
            }
            Lab::Interpolate => {
                e = ee;
                for i in 1..4 {
                    sigp[i] = terp1(elast, st.slst[i], enext, st.snxt[i], e, IntLaw::LinLin)?;
                }
                sigp[0] = sigp[1] + sigp[2] + sigp[3];
                lab = Lab::Done;
            }
            Lab::Done => return Ok(sigp),
        }
    }
}

/// `csunr2` (`reconr.f90:4079-4317`): `LRF = 2` ranges, every parameter
/// tabulated per J-state; cross sections interpolated between the first
/// J-state's energy nodes unless the panel is at least `wide` wide.
pub fn csunr2(range: &UnresolvedRange, st: &mut PanelState, e0: f64) -> Result<[f64; 4], NjoyError> {
    let UnresolvedCase::CaseC { awri, spi, l_states, .. } = &range.case_ else {
        return Err(NjoyError::EndfParse("csunr2 called on a non-LRF=2 range".into()));
    };
    let (awri, spi) = (*awri, *spi);
    let cwaven = WAVE_K;
    let first = l_states
        .first()
        .and_then(|l| l.j_states.first())
        .ok_or_else(|| NjoyError::EndfParse("csunr2: no J-states".into()))?;
    let mut e = e0;
    let ee = e;
    let mut ifirst = st.ilast;
    let mut int = first.int_;
    let ne = first.points.len() as i32;
    let mut enext;
    let mut elast;
    let mut ie: i32;
    let mut sigp = [0.0_f64; 4];
    let mut lab;
    let (mut vl, mut ps) = (0.0_f64, 0.0_f64);
    let mut spot = 0.0_f64;

    if ifirst > 0 {
        enext = st.enxt;
        elast = st.elst;
        ie = st.ilast;
        lab = if e < enext && enext >= WIDE * elast {
            Lab::L130
        } else if e <= enext {
            Lab::Interpolate
        } else if ie == ne {
            Lab::L120
        } else {
            Lab::L110
        };
    } else {
        enext = e;
        elast = 0.0;
        st.enxt = e;
        st.elst = 0.0;
        ie = 1;
        lab = Lab::L120;
    }

    let mut guard = 0;
    loop {
        guard += 1;
        if guard > 64 {
            return Err(NjoyError::EndfParse("csunr2: panel logic did not terminate".into()));
        }
        match lab {
            Lab::L110 => {
                elast = enext;
                st.elst = elast;
                st.slst = st.snxt;
                ie += 1;
                enext = first
                    .points
                    .get((ie - 1) as usize)
                    .ok_or_else(|| NjoyError::EndfParse("csunr2: energy above the last tabulated energy".into()))?
                    .e;
                st.enxt = enext;
                st.ilast = ie;
                lab = Lab::L120;
            }
            Lab::L120 => {
                e = enext;
                lab = Lab::L130;
            }
            Lab::L130 => {
                sigp = [0.0; 4];
                let (aa, ay) = radii(range, awri, e)?;
                let rat = awri / (awri + 1.0);
                let const_ = (2.0 * PI * PI) / ((cwaven * rat) * (cwaven * rat));
                let e2 = e.sqrt();
                let k = cwaven * rat * e2;
                let rho = k * aa;
                let rhoc = k * ay;
                for lst in l_states.iter() {
                    let ll = lst.l;
                    for (j, js) in lst.j_states.iter().enumerate() {
                        int = js.int_;
                        let gj = (2.0 * js.aj + 1.0) / (4.0 * spi + 2.0);
                        let mu = js.amun.round() as i32;
                        let nu = js.amuf.round() as i32;
                        let lamda = js.amux.round() as i32;
                        // `do while (res(i1+6).le.e-small)`: the row below `e`
                        let pts = &js.points;
                        let mut i1 = 0usize;
                        while i1 + 1 < pts.len() && pts[i1 + 1].e <= e - SMALL {
                            i1 += 1;
                        }
                        let i2 = (i1 + 1).min(pts.len() - 1);
                        let (p1, p2) = (&pts[i1], &pts[i2]);
                        let dx = terp1(p1.e, p1.d, p2.e, p2.d, e, IntLaw::LinLin)?;
                        let mut gxx = terp1(p1.e, p1.gx, p2.e, p2.gx, e, IntLaw::LinLin)?;
                        let gnox = terp1(p1.e, p1.gno, p2.e, p2.gno, e, IntLaw::LinLin)?;
                        let ggx = terp1(p1.e, p1.gg, p2.e, p2.gg, e, IntLaw::LinLin)?;
                        let mut gfx = terp1(p1.e, p1.gf, p2.e, p2.gf, e, IntLaw::LinLin)?;
                        if gxx < SMALL {
                            gxx = 0.0;
                        }
                        if gfx < SMALL {
                            gfx = 0.0;
                        }
                        unfac(ll, rho, rhoc, js.amun, &mut vl, &mut ps);
                        vl *= e2;
                        if j == 0 {
                            spot = 4.0 * PI * (2.0 * ll as f64 + 1.0) * (ps.r_sin() / k).r_powi(2);
                        }
                        let gnx = gnox * vl;
                        let (gamma, galpha, gbeta, diff) = (ggx, gnx, gfx, gxx);
                        let den = e * dx;
                        let temp = const_ * gj * gnx / den;
                        let terg = temp * gamma;
                        let ters = temp * gnx;
                        let terf = temp * gbeta;
                        let mut gs = gnrl(galpha, gbeta, gamma, mu, nu, lamda, diff, 1)?;
                        let mut gc = gnrl(galpha, gbeta, gamma, mu, nu, lamda, diff, 2)?;
                        let mut gff = gnrl(galpha, gbeta, gamma, mu, nu, lamda, diff, 3)?;
                        gc *= terg;
                        gff *= terf;
                        gs *= ters;
                        let mut add = const_ * gj * 2.0 * gnx * ps.r_sin().r_powi(2);
                        add /= e * dx;
                        gs -= add;
                        sigp[1] += gs;
                        sigp[2] += gff;
                        sigp[3] += gc;
                    }
                    sigp[1] += spot;
                }
                sigp[0] = sigp[1] + sigp[2] + sigp[3];
                if ifirst > 0 && e < enext && enext >= WIDE * elast {
                    lab = Lab::Done;
                } else {
                    st.snxt = sigp;
                    if ifirst > 0 && e == enext && enext >= WIDE * elast {
                        lab = Lab::Recompute; // 185
                    } else if ifirst > 0 {
                        lab = Lab::Interpolate;
                    } else if ifirst < 0 {
                        ifirst = 1; // 190
                        lab = Lab::Interpolate;
                    } else {
                        ifirst = -1;
                        lab = Lab::L110;
                    }
                }
            }
            Lab::Recompute => {
                e = ee;
                lab = Lab::L130;
            }
            Lab::Interpolate => {
                e = ee;
                let law = IntLaw::from_code(int as u32);
                for i in 1..4 {
                    sigp[i] = terp1(elast, st.slst[i], enext, st.snxt[i], e, law)?;
                }
                sigp[0] = sigp[1] + sigp[2] + sigp[3];
                lab = Lab::Done;
            }
            Lab::L205 | Lab::Done => return Ok(sigp),
        }
    }
}
