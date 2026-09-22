// Ported from NJOY2016 `src/acepa.f90` (subroutine `iheat`, lines 381-502).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! `iheat` — the incoherent (Compton) cross section and mean scattered-photon
//! energy at one incident energy, integrated over the Klein–Nishina kernel
//! weighted by the MF=27/MT=504 scattering function `S(x, Z)`.
//!
//! ## Why this is not `gtff_incoherent`
//!
//! [`crate::gaminr::gtff::gtff_incoherent`] integrates the *same* kernel and
//! accumulates the *same* `siginc` and `ebar`, so reusing it was the first
//! thing tried. It does not fit, for two reasons that change the number:
//!
//! 1. **Panel bounds.** GAMINR splits each panel at the sink-group boundary
//!    (`pnext = c3 egg(igp-1)`, `gaminr.f90:1367`); `iheat` has no group
//!    structure and splits at `pnow/2` instead (`acepa.f90:463`). Passing a
//!    single all-encompassing group does not recover the halving rule.
//! 2. **Quadrature order.** `iheat` is fixed at the 10-point Lobatto rule
//!    (`nq = 10`, `acepa.f90:468`); GAMINR picks 6 unless `nl > 6`.
//!
//! Two different quadratures over two different panel sets give two different
//! answers, so this is a port of `iheat`, not a call into GAMINR. What *is*
//! shared is the physics constants and the `terpa` conventions, which are
//! taken from their existing homes rather than redeclared.

use crate::endf::interp::terpa;
use crate::endf::records::Tab1;

/// `c2` (`acepa.f90:407`) — `pi r_e^2 / 2` in barns.
const C2: f64 = 0.249_467;
/// `c3` (`:408`) — `1 / (m_e c^2)` per eV.
const C3: f64 = 1.956_93e-6;
/// `c4` (`:409`).
const C4: f64 = 0.048_526_2;
/// `c5` (`:410`).
const C5: f64 = 20.607_44;
/// `rndoff` (`:411`).
const RNDOFF: f64 = 1.000_000_1;
/// `big` (`:412`).
const BIG: f64 = 1.0e10;

/// 10-point Lobatto abscissae, `qp10` (`:401-404`).
const QP10: [f64; 10] = [
    -1.0,
    -0.919_533_908_2,
    -0.738_773_865_1,
    -0.477_924_949_8,
    -0.165_278_957_7,
    0.165_278_957_7,
    0.477_924_949_8,
    0.738_773_865_1,
    0.919_533_908_2,
    1.0,
];
/// 10-point Lobatto weights, `qw10` (`:405-407`).
const QW10: [f64; 10] = [
    0.022_222_222_2,
    0.133_305_990_8,
    0.224_889_342_0,
    0.292_042_683_6,
    0.327_539_761_2,
    0.327_539_761_2,
    0.292_042_683_6,
    0.224_889_342_0,
    0.133_305_990_8,
    0.022_222_222_2,
];

/// `iheat`'s two answers at one incident energy.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct IncoherentHeating {
    /// `heat = e - ebar` \[eV\] — the energy deposited per incoherent scatter.
    pub heat: f64,
    /// `siginc` \[barn\] — the incoherent cross section the same integral
    /// produces. ACER does not use it (it takes MT=504 off the file instead),
    /// but it is the integral's own normalisation and is returned so a caller
    /// can check it against MF=23/MT=504.
    pub siginc: f64,
    /// How many Lobatto panels the integral used. Upstream does not report
    /// this; it is here because the panel count is what a threshold flip
    /// changes, and a heating disagreement that tracks a panel count is a
    /// different finding from one that does not.
    pub panels: usize,
}

/// Incoherent heating at incident energy `e` \[eV\] against the MF=27/MT=504
/// scattering function `sf` (whose `C2` field is `Z`).
///
/// Follows `acepa.f90:413-500` term for term, including two behaviours that
/// look like bugs and are not:
///
/// - **`arg` persists across panels.** The `iq = 1` endpoint of each Lobatto
///   panel reuses the value left by the previous panel's `iq = 10` endpoint,
///   because they are the same abscissa. Re-zeroing it per panel drops half of
///   every interior endpoint's weight.
/// - **`S` is not looked up above `x_zz = c5 sqrt(enow/500)`.** Past that the
///   scattering function has saturated at `Z` and upstream substitutes the
///   constant rather than extrapolating the table.
pub fn iheat(e: f64, sf: &Tab1) -> IncoherentHeating {
    iheat_inner(e, sf, false)
}

/// The same integral with **one** expression rearranged, as a control.
///
/// Upstream forms the scattering cosine as `unow = 1 + 1/enow - 1/pnow` and
/// then needs `1 - unow`, which at high incident energy is the difference of
/// two nearly equal reciprocals: at `E = 1.5 GeV` both are ~3.5e-4 and the
/// first panel's points cancel most of their digits away. Passing `true` here
/// forms `(1/pnow - 1/enow)` directly instead, which is algebraically the same
/// and loses nothing.
///
/// This exists to *measure* how much of a disagreement that cancellation can
/// carry, not to replace upstream's expression — [`iheat`] keeps the Fortran's
/// arithmetic, because a port that silently improves the numerics stops being
/// a port and the comparison against NJOY stops meaning anything.
pub fn iheat_ablate_cancellation(e: f64, sf: &Tab1) -> IncoherentHeating {
    iheat_inner(e, sf, true)
}

/// The same integral with the panel limit tightened from `pnow/2` to
/// `pnow * (1 - 1/split)`, as a **convergence control**.
///
/// `split = 2` is upstream. Larger values subdivide further, so the spread
/// between `split = 2` and, say, `split = 128` is the panel rule's own
/// truncation error — the number any comparison between two implementations
/// of this routine has to be read against.
pub fn iheat_refined(e: f64, sf: &Tab1, split: f64) -> IncoherentHeating {
    iheat_full(e, sf, false, split)
}

fn iheat_inner(e: f64, sf: &Tab1, stable_rm2: bool) -> IncoherentHeating {
    iheat_full(e, sf, stable_rm2, 2.0)
}

fn iheat_full(e: f64, sf: &Tab1, stable_rm2: bool, split: f64) -> IncoherentHeating {
    let zz = sf.head.c2;
    let enow = C3 * e;
    let enowi = 1.0 / enow;
    let enow2 = enow * enow;
    let mut pnow = enow;
    let mut xnow;
    let xzz = C5 * (enow / 500.0).sqrt();
    let q2m = (2.0 * enow * (1.0 + enow) / (1.0 + 2.0 * enow)).powi(2);
    let use_table = xzz <= zz;
    let (mut snow, mut xnext) = if use_table {
        let (s, xn, _) = terpa(&sf.interp, &sf.pairs, 0.0);
        (s, xn)
    } else {
        (zz, BIG)
    };
    let mut arg = 0.0f64;
    let mut siginc = 0.0f64;
    let mut ebar = 0.0f64;
    let mut unow = 1.0f64;
    let mut panels = 0usize;
    loop {
        panels += 1;
        // Panel upper bound in cosine, from the S(x) grid (`:440-459`).
        let mut unext;
        loop {
            let q2 = (C4 * xnext).powi(2);
            unext = -1.0f64;
            if q2 > q2m {
                break;
            }
            unext = 1.0 - ((1.0 - q2 * enowi) - (1.0 + q2).sqrt()) / (q2 - enow2 - 2.0 * enow);
            if unext < -1.0 {
                unext = -1.0;
            }
            if unext < 0.99999 {
                break;
            }
            xnow = xnext * RNDOFF;
            let (s, xn, _) = terpa(&sf.interp, &sf.pairs, xnow);
            snow = s;
            xnext = xn;
        }
        let mut pnext = enow / (1.0 + 2.0 * enow);
        let px = enow / (1.0 + enow * (1.0 - unext));
        if px > pnext {
            pnext = px;
        }
        let floor_p = pnow * (1.0 - 1.0 / split);
        if pnext < floor_p {
            pnext = floor_p;
        }
        if pnext > pnow / RNDOFF {
            pnext = pnow / RNDOFF;
        }
        let aq = (pnext + pnow) / 2.0;
        let bq = (pnext - pnow) / 2.0;
        for iq in 0..10 {
            let uq = aq + bq * QP10[iq];
            let wq = -C2 * bq * QW10[iq];
            pnow = uq;
            if iq > 0 {
                let pnowi = 1.0 / pnow;
                unow = 1.0 + enowi - pnowi;
                if use_table {
                    let rm2 = if stable_rm2 {
                        (pnowi - enowi) / 2.0
                    } else {
                        (1.0 - unow) / 2.0
                    };
                    let rm = rm2.sqrt();
                    let rt = 1.0 + 2.0 * enow * rm2;
                    xnow = C5 * 2.0 * enow * rm * (rt + enow2 * rm2).sqrt() / rt;
                    xnow *= RNDOFF;
                    let (s, xn, _) = terpa(&sf.interp, &sf.pairs, xnow);
                    snow = s;
                    xnext = xn;
                }
                let dk = unow - 1.0;
                arg = snow * (enow * pnowi + pnow * enowi + dk * (2.0 + dk)) / enow2;
            }
            siginc += wq * arg;
            ebar += wq * arg * pnow / C3;
        }
        if unow < -0.9999 {
            break;
        }
    }
    ebar /= siginc;
    IncoherentHeating {
        heat: e - ebar,
        siginc,
        panels,
    }
}
