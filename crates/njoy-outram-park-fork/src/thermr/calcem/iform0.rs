//! `calcem`'s `iform=0` path: the equally-probable-cosine (`LANG=3`) energy-
//! angle distribution over the fixed incident-energy grid.
//!
//! Faithful port of NJOY2016 `thermr.f90::calcem`, the `iform=0` branch from
//! label 300 to the `iform=1` dispatch at label 510 (lines 1918-2266) — the
//! incident-energy loop over [`super::egrid::EGRID`], the signed-beta panel
//! walk, the adaptive outgoing-energy (`E'`) linearisation comparing [`sigl`]
//! output component-by-component against the linear chord, and the
//! `ubar`/`p2`/`p3` Legendre-moment accumulation. `sigl` itself — the
//! per-`(E,E')` cross section and equally-probable-cosine computation — lives
//! in [`super::sigl`] and is reused, not reimplemented, here.
//!
//! Not ported (see the module docs in [`super`] for the full list): the
//! MF=6 tape record packing (headers, `TAB1`/`TAB2`/`LIST` words, `npage`
//! paging) and the final elastic+inelastic cross-section merge onto the old
//! PENDF tape (`thermr.f90` label 610 onward, `finda`/`loada`). What this
//! function returns is the physical content those steps would otherwise
//! write, as typed Rust data (see [`super::types`]).

use super::super::mf7::IncoherentInelastic;
use super::egrid;
use super::sigl::sigl;
use super::types::{EqualProbableRow, IncidentEnergyRecord, Iform0Table};
use crate::error::NjoyError;
use crate::groupr::kinematics::legndr;
use crate::mixr::mix::sigfig;

/// `calcem`'s `imax`: bisection-stack depth cap for the outgoing-energy
/// panel walk (shared name with `sigl`'s own, same value, different array).
const IMAX: usize = 20;
/// `calcem`'s `nlmax`: the largest supported `nl = nbin + 1`
/// (`thermr.f90:1576`).
const NLMAX: usize = 65;
/// `calcem`'s `tolmin`: a panel narrower than this (in accumulated
/// trapezoid area) is accepted without further subdivision.
const TOLMIN: f64 = 5.0e-7;
/// `calcem`'s `em9`: the `sigfig` significant-figure cutover for the stored
/// pdf value (9 sig figs at/above this, 8 below).
const EM9: f64 = 1.0e-9;
/// `calcem`'s `uumin`: absolute floor added to the aggregate cosine-sum
/// convergence test.
const UUMIN: f64 = 0.00001;
/// `calcem`'s `unity`: cosine clamp bound.
const UNITY: f64 = 1.0;

/// Round and clamp one output row's ordinates exactly as `calcem` does
/// before writing them (`thermr.f90:2168-2199`, `2245-2266`) — `sigfig`
/// rounding of the pdf and every cosine, then clamping any cosine outside
/// `[-1, 1]` back into range. The accompanying `mess()` warning for a
/// "big miss" overflow/underflow is not reproduced (no logging channel is
/// wired here); the numeric clamp itself, which is unconditional, is.
fn build_row(ep_ev: f64, y: &[Vec<f64>], i: usize, nl: usize) -> EqualProbableRow {
    let raw = y[1][i];
    let pdf = if raw >= EM9 { sigfig(raw, 9, 0) } else { sigfig(raw, 8, 0) };
    let mut cosines = Vec::with_capacity(nl.saturating_sub(1));
    for il in 2..=nl {
        let mut c = sigfig(y[il][i], 9, 0);
        if c > UNITY {
            c = UNITY;
        }
        if c < -UNITY {
            c = -UNITY;
        }
        cosines.push(c);
    }
    EqualProbableRow { ep_ev, pdf, cosines }
}

/// The first three Legendre moments `P₁, P₂, P₃` of a single equally-probable
/// cosine `mu`, each weighted by the panel's `y(1,i)` cross-section-like
/// ordinate and averaged over the `nl - 1` cosines — the `uu`/`u2`/`u3`
/// computation repeated at `thermr.f90:2087-2096` and `2144-2153`.
fn moment_sum(y: &[Vec<f64>], i: usize, nl: usize) -> (f64, f64, f64) {
    let (mut uu, mut u2, mut u3) = (0.0, 0.0, 0.0);
    for il in 2..=nl {
        let p = legndr(y[il][i], 3);
        uu += p[1];
        u2 += p[2];
        u3 += p[3];
    }
    let denom = (nl - 1) as f64;
    let scale = y[1][i] / denom;
    (uu * scale, u2 * scale, u3 * scale)
}

/// Compute the `iform=0` secondary distribution over the incident-energy
/// grid up to `emax` \[eV\], for temperature `ii.temperature_k` \[K\].
///
/// `natom` is the number of principal-scatterer atoms per molecule; `nbin`
/// is the requested number of equally-probable-cosine bins (`calcem`'s
/// `nbin` card input, so `nl = nbin + 1`); `tolin` is the reconstruction
/// tolerance (`calcem`'s `tol`, thermr.f90:1577's `tolin` card default is
/// typically `0.001`).
///
/// # Errors
/// [`NjoyError::NotConvergent`] if `nbin + 1` exceeds [`NLMAX`] (mirrors
/// `thermr.f90`'s `call error('calcem','nl too large for binning.',...)`),
/// or if the underlying [`sigl`] call fails to find a legal bin boundary.
pub fn compute_iform0(
    ii: &IncoherentInelastic,
    natom: f64,
    nbin: usize,
    emax: f64,
    tolin: f64,
) -> Result<Iform0Table, NjoyError> {
    let nl = nbin + 1;
    if nl > NLMAX {
        return Err(NjoyError::NotConvergent {
            routine: "calcem: nl too large for binning",
        });
    }
    let nnl = -(nl as i32);
    let nne = egrid::egrid_count(emax);
    let temp = ii.temperature_k;
    let tev = crate::common::phys::BK_EV_PER_K * temp;
    let lat = ii.lat;
    let lasym = ii.lasym;
    let nbeta = ii.beta.len() as i32;

    let mut records = Vec::with_capacity(nne);

    for ie1 in 1..=nne {
        let mut enow = egrid::EGRID[ie1 - 1];
        enow = egrid::rescale_iform0(enow, temp);
        enow = sigfig(enow, 8, 0);

        let mut xsi = 0.0f64;
        let mut ubar = 0.0f64;
        let mut p2 = 0.0f64;
        let mut p3 = 0.0f64;

        // Panel arrays, 1-indexed (index 0 unused) to mirror the Fortran
        // `x(imax)`, `y(nlmax,imax)` exactly.
        let mut x = [0.0f64; IMAX + 1];
        let mut y: Vec<Vec<f64>> = vec![[0.0f64; IMAX + 1].to_vec(); nl + 1];

        x[1] = 0.0;
        let seed = sigl(ii, enow, x[1], nnl, natom, tolin)?;
        for il in 1..=nl {
            y[il][1] = seed[il];
        }

        let mut jbeta: i32 = if lasym > 0 { 1 } else { -nbeta };
        let mut j: usize = 0;
        let mut iskip = false;
        let mut xlast = 0.0f64;
        let mut ylast = 0.0f64;
        let mut ulast = 0.0f64;
        let mut u2last = 0.0f64;
        let mut u3last = 0.0f64;
        let mut jnz: usize = 0;
        let mut rows: Vec<EqualProbableRow> = Vec::new();

        'panels: loop {
            // label 311: set up next panel.
            x[2] = x[1];
            for il in 1..=nl {
                y[il][2] = y[il][1];
            }
            // label 313: walk the beta grid outward for the next candidate ep.
            let mut ep;
            loop {
                if jbeta == 0 {
                    jbeta = 1;
                }
                if jbeta <= 0 {
                    let bj = ii.beta[(-jbeta) as usize - 1];
                    ep = if lat == 1 { enow - bj * super::sigl::TEVZ } else { enow - bj * tev };
                    ep = if ep == enow { sigfig(enow, 8, -1) } else { sigfig(ep, 8, 0) };
                } else {
                    let bj = ii.beta[jbeta as usize - 1];
                    ep = if lat == 1 { enow + bj * super::sigl::TEVZ } else { enow + bj * tev };
                    if ep == enow {
                        ep = sigfig(enow, 8, 1);
                        iskip = true;
                    } else {
                        ep = sigfig(ep, 8, 0);
                    }
                }
                if ep > x[2] {
                    break;
                }
                jbeta += 1;
            }
            ep = sigfig(ep, 8, 0);
            x[1] = ep;
            let yt0 = sigl(ii, enow, ep, nnl, natom, tolin)?;
            for il in 1..=nl {
                y[il][1] = yt0[il];
            }

            let mut i: usize = 2;

            'bisect: loop {
                // label 330: compare the linear approximation to the true function.
                let accept = if i == IMAX {
                    true
                } else if iskip {
                    iskip = false;
                    true
                } else if 0.5 * (y[1][i - 1] + y[1][i]) * (x[i - 1] - x[i]) < TOLMIN {
                    true
                } else {
                    let xm = sigfig(0.5 * (x[i - 1] + x[i]), 8, 0);
                    if xm <= x[i] || xm >= x[i - 1] {
                        true
                    } else {
                        let yt = sigl(ii, enow, xm, nnl, natom, tolin)?;
                        let mut uu = 0.0f64;
                        let mut uum = 0.0f64;
                        let mut failed = false;
                        for k in 1..=nl {
                            let ym = crate::endf::interp::terp1(
                                x[i],
                                y[k][i],
                                x[i - 1],
                                y[k][i - 1],
                                xm,
                                crate::endf::interp::IntLaw::LinLin,
                            )
                            .unwrap_or(y[k][i]);
                            if k > 1 {
                                uu += yt[k];
                                uum += ym;
                            }
                            let test = tolin * yt[k].abs();
                            let test2 = if k > 1 { tolin } else { test };
                            if (yt[k] - ym).abs() > test2 {
                                failed = true;
                                break;
                            }
                        }
                        if !failed {
                            let test = 2.0 * tolin * uu.abs() + UUMIN;
                            if (uu - uum).abs() > test {
                                failed = true;
                            }
                        }
                        if failed {
                            // label 410: add point to the stack and continue.
                            i += 1;
                            x[i] = x[i - 1];
                            for il in 1..=nl {
                                y[il][i] = y[il][i - 1];
                            }
                            x[i - 1] = xm;
                            for il in 1..=nl {
                                y[il][i - 1] = yt[il];
                            }
                            false
                        } else {
                            true
                        }
                    }
                };
                if !accept {
                    continue 'bisect;
                }

                // label 360: point passes.
                j += 1;
                if j > 1 {
                    xsi += (x[i] - xlast) * (y[1][i] + ylast) * 0.5;
                    let (uu, u2, u3) = moment_sum(&y, i, nl);
                    ubar += 0.5 * (x[i] - xlast) * (uu + ulast);
                    p2 += 0.5 * (x[i] - xlast) * (u2 + u2last);
                    p3 += 0.5 * (x[i] - xlast) * (u3 + u3last);
                }
                if j == 3 && xsi < TOLMIN {
                    j = 2;
                }
                let row = build_row(x[i], &y, i, nl);
                if j <= rows.len() {
                    rows[j - 1] = row;
                } else {
                    rows.push(row);
                }
                xlast = x[i];
                ylast = y[1][i];
                if ylast != 0.0 {
                    jnz = j;
                }
                let (uu, u2, u3) = moment_sum(&y, i, nl);
                ulast = uu;
                u2last = u2;
                u3last = u3;

                i -= 1;
                if i >= 2 {
                    continue 'bisect;
                }
                jbeta += 1;
                if jbeta <= nbeta {
                    continue 'panels;
                }
                for il in 1..=nl {
                    y[il][1] = 0.0;
                }
                break 'panels; // i == 1: go to 430
            }
        }

        // label 430: linearisation complete for this incident energy.
        let i = 1usize;
        j += 1;
        xsi += (x[i] - xlast) * (y[1][i] + ylast) * 0.5;
        // uu = u2 = u3 = 0 here (thermr.f90:2244-2246): the closing point's
        // own moment contribution is zero, only `ulast`/`u2last`/`u3last`
        // (from the last real acceptance) carry forward into the trapezoid.
        ubar += 0.5 * (x[i] - xlast) * ulast;
        p2 += 0.5 * (x[i] - xlast) * u2last;
        p3 += 0.5 * (x[i] - xlast) * u3last;
        xsi = sigfig(xsi, 9, 0);
        let row = build_row(x[i], &y, i, nl);
        if j <= rows.len() {
            rows[j - 1] = row;
        } else {
            rows.push(row);
        }
        // thermr.f90:2225: "if (y(1,1).ne.zero) jnz=j" — y(1,1) was just
        // zero-filled above, so this is always false here; `jnz` keeps the
        // value set at the last real (label 360) acceptance.
        if jnz < j {
            j = jnz + 1;
        }
        rows.truncate(j);

        let (mubar, p2n, p3n) = if xsi != 0.0 {
            (ubar / xsi, p2 / xsi, p3 / xsi)
        } else {
            (0.0, 0.0, 0.0)
        };

        records.push(IncidentEnergyRecord {
            e_in_ev: enow,
            cross_section_b: xsi,
            mubar,
            p2: p2n,
            p3: p3n,
            rows,
        });
    }

    Ok(Iform0Table { records })
}
