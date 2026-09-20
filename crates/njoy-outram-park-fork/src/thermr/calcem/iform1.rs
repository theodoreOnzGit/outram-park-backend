//! `calcem`'s `iform=1` path: the continuous angle-energy (`LAW=7`)
//! distribution — a fixed, adaptively-determined `μ` grid, each with its own
//! adaptively-linearised continuous outgoing-energy law.
//!
//! Faithful port of NJOY2016 `thermr.f90::calcem`, the `iform=1` branch from
//! label 510 through the per-`μ` write-out loop (lines 2276-2404). This is
//! the named cure for GitHub #188: unlike `iform=0`'s equally-probable-cosine
//! compaction, this path keeps a genuinely continuous secondary-energy law at
//! each of `nmu` cosines, built from [`super::sigu::sigu`] (reused, not
//! reimplemented).
//!
//! Two nested adaptive reconstructions happen here, matching the Fortran
//! exactly:
//!
//! 1. **The `μ` grid itself** (labels 530-580): `sigu(enow, μ)`'s own
//!    returned total (`SiguResult::sum2`, "twice" `∫σ dE'` at fixed `μ` — see
//!    that module's docs) is treated as `dσ/dμ`-like ordinate and adaptively
//!    linearised over `μ ∈ [-1, 1]`, with a hardcoded "panel too wide"
//!    override (`x(i-1)-x(i) > 0.25`) on top of the usual tolerance test.
//!    This produces the cross section `σ_inel(E)`, `⟨μ⟩(E)`, and the `nmu`
//!    cosines themselves.
//! 2. **Each cosine's `E'` law** (the `do il=1,nmu` loop, lines 2374-2404):
//!    `sigu` is called again at each of those `nmu` cosines (a *third* time
//!    for cosines already evaluated during step 1's linearisation — NJOY
//!    recomputes rather than caching, and this port matches that), its
//!    trailing low-probability tail trimmed against `yumin`, and the
//!    survivors normalized to a probability density by the **same** overall
//!    `sum` from step 1 (`2·σ_inel(E)`) — not each call's own local
//!    integral. Getting this shared-normalizer detail right matters: it is
//!    what makes the per-`μ` densities consistent with the reported cross
//!    section rather than each independently normalized to 1.
//!
//! Not ported here either (see [`super`]'s module docs): MF=6/LAW=7 tape
//! record packing and the final cross-section merge onto the old PENDF tape.

use super::super::mf7::IncoherentInelastic;
use super::egrid;
use super::sigu::sigu;
use super::types::{IncidentEnergyAngleRecord, Iform1Table, MuDistribution};
use crate::error::NjoyError;
use crate::mixr::mix::sigfig;

/// Bisection-stack depth cap, shared name/value with `sigl`/`sigu`/`iform0`.
const IMAX: usize = 20;
/// `calcem`'s `tolmin`, reused unchanged from the `iform=0` path
/// (`thermr.f90:1611`).
const TOLMIN: f64 = 5.0e-7;
/// `calcem`'s `mumax`: the largest supported number of adaptively-determined
/// cosines (`thermr.f90:1578`).
const MUMAX: usize = 300;
/// `calcem`'s `yumin`: the per-`(μ,E')` probability floor below which a
/// trailing point is trimmed from the written distribution
/// (`thermr.f90:1621`).
const YUMIN: f64 = 2.0e-7;

/// Compute the `iform=1` secondary distribution over the incident-energy
/// grid up to `emax` \[eV\], for temperature `ii.temperature_k` \[K\].
///
/// `natom` is the number of principal-scatterer atoms per molecule; `tolin`
/// is `calcem`'s reconstruction tolerance (the same card value `iform=0`
/// uses, unhalved here — matching `thermr.f90` passing its own global `tol`
/// straight through to `sigu`).
///
/// # Errors
/// [`NjoyError::NotConvergent`] if more than [`MUMAX`] cosines would be
/// needed at some incident energy (mirrors `thermr.f90`'s
/// `call error('calcem','too many angles','see mumax')`).
pub fn compute_iform1(
    ii: &IncoherentInelastic,
    natom: f64,
    emax: f64,
    tolin: f64,
) -> Result<Iform1Table, NjoyError> {
    let nne = egrid::egrid_count(emax);
    let temp = ii.temperature_k;

    let mut records = Vec::with_capacity(nne);

    for ie1 in 1..=nne {
        let mut enow = egrid::EGRID[ie1 - 1];
        enow = egrid::rescale_iform1(enow, temp, ie1);
        enow = sigfig(enow, 8, 0);

        // ── Step 1: adaptive reconstruction of the mu grid (labels 520-580). ──
        let mut x = [0.0f64; IMAX + 1];
        let mut yy = [0.0f64; IMAX + 1];
        x[2] = -1.0;
        yy[2] = sigu(ii, enow, x[2], natom, tolin).sum2;
        let mut xl = x[2];
        let mut yl = yy[2];
        x[1] = 1.0;
        yy[1] = sigu(ii, enow, x[1], natom, tolin).sum2;
        let mut i: usize = 2;
        let mut sum_outer = 0.0f64;
        let mut uj: Vec<f64> = Vec::new();
        let mut sj: Vec<f64> = Vec::new();

        loop {
            // label 530
            let accept = if i == IMAX {
                true
            } else {
                let xm = sigfig(0.5 * (x[i - 1] + x[i]), 7, 0);
                if xm <= x[i] || xm >= x[i - 1] {
                    true
                } else {
                    let yu1 = sigu(ii, enow, xm, natom, tolin).sum2;
                    let reject = if x[i - 1] - x[i] > 0.25 {
                        true
                    } else {
                        let ym = yy[i] + (xm - x[i]) * (yy[i - 1] - yy[i]) / (x[i - 1] - x[i]);
                        (yu1 - ym).abs() > 2.0 * tolin * ym + TOLMIN
                    };
                    if reject {
                        i += 1;
                        x[i] = x[i - 1];
                        yy[i] = yy[i - 1];
                        x[i - 1] = xm;
                        yy[i - 1] = yu1;
                        false
                    } else {
                        true
                    }
                }
            };
            if !accept {
                continue;
            }
            // label 560
            if uj.len() >= MUMAX - 1 {
                return Err(NjoyError::NotConvergent {
                    routine: "calcem: too many angles (see mumax)",
                });
            }
            uj.push(x[i]);
            sj.push(yy[i]);
            if uj.len() > 1 {
                sum_outer += 0.5 * (yy[i] + yl) * (x[i] - xl);
                xl = x[i];
                yl = yy[i];
            }
            i -= 1;
            if i >= 2 {
                continue;
            }
            break; // -> label 580
        }

        // label 580: linearisation complete.
        uj.push(x[1]);
        sj.push(yy[1]);
        let nmu = uj.len();
        sum_outer += 0.5 * (yy[1] + yl) * (x[1] - xl);
        let xsi = sum_outer / 2.0;
        let mut mubar = 0.0f64;
        for k in 1..nmu {
            mubar += 0.5 * (uj[k] - uj[k - 1]) * (sj[k] + sj[k - 1]) * (uj[k] + uj[k - 1]);
        }
        mubar = if sum_outer != 0.0 {
            0.5 * mubar / sum_outer
        } else {
            0.0
        };

        // ── Step 2: per-mu continuous E' law (thermr.f90:2374-2404). ──────────
        let mut mu_distributions = Vec::with_capacity(nmu);
        for &mu in &uj {
            let r = sigu(ii, enow, mu, natom, tolin);
            let nep_orig = r.points.len();
            let mut nep_trim = 0usize;
            for i in 1..=nep_orig {
                nep_trim = nep_orig - i;
                if sum_outer != 0.0 && r.points[nep_trim].1 / sum_outer > YUMIN {
                    break;
                }
            }
            let points: Vec<(f64, f64)> = r.points[..nep_trim]
                .iter()
                .map(|&(ep, val)| {
                    (
                        ep,
                        if sum_outer != 0.0 {
                            val * 2.0 / sum_outer
                        } else {
                            0.0
                        },
                    )
                })
                .collect();
            mu_distributions.push(MuDistribution { mu, points });
        }

        records.push(IncidentEnergyAngleRecord {
            e_in_ev: enow,
            cross_section_b: xsi,
            mubar,
            mu_distributions,
        });
    }

    Ok(Iform1Table { records })
}
