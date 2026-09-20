// Ported from NJOY2016 `src/groupr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! ENDF File-6 **LAW = 7** (angle-energy, already lab-frame) → lab Legendre
//! transform: `ll2lab` (`groupr.f90:8934-9061`).
//!
//! LAW = 7 tabulates the double-differential emission directly in the lab
//! frame as `f(E'|mu)`: a grid of direction cosines `mu`, each carrying its
//! own `E' -> f` TAB1 curve. No CM->lab transform is needed (unlike the LAW=1
//! CM path in [`super::cm`]); `ll2lab` instead walks the union of every
//! curve's own energy grid and, at each union-grid point, Legendre-projects
//! the `nmu` values `f(ep|mu_i)` onto `P_0(mu) .. P_{nl-1}(mu)` by 8-point
//! Gauss quadrature over `mu`.
//!
//! Header check (per the crate's "read upstream first" rule): `ll2lab`'s own
//! comment reads only *"Change the angle-energy lab distribution in law 7
//! format into a laboratory Legendre coefficient representation in law 1
//! format"* — this is the only `ll2lab` in `groupr.f90`, so there is no
//! similarly-named variant to mis-pick.

use super::shared::{legndr, LabDistribution, EMAX, GAUSS8_NODES, GAUSS8_WEIGHTS, SMALL};
use crate::endf::interp::{terp1, IntLaw};
use crate::endf::records::Tab1;
use crate::NjoyError;

/// One direction-cosine curve of a File-6 LAW-7 subsection at fixed incident
/// energy: the lab double-differential emission `f(E'|mu)` \[1/eV\] as an ENDF
/// TAB1, tabulated at direction cosine `mu`.
#[derive(Debug, Clone)]
pub struct Law7Curve {
    /// Lab-frame direction cosine `mu` (dimensionless, `[-1, 1]`).
    pub mu: f64,
    /// `f(E'|mu)` \[1/eV\] as a function of lab secondary energy `E'` \[eV\].
    pub f_of_ep: Tab1,
}

/// A File-6 LAW-7 (angle-energy, already lab-frame) emission block at one
/// incident energy — the Rust analogue of the `c(inow..)` nested TAB2/TAB1
/// data `ll2lab` reads (`groupr.f90:8934-9061`; see `getmf6`'s `law.eq.7`
/// reader at `groupr.f90:7877-7907` for how it is assembled from the tape).
///
/// # Fields and units
/// - `e_in` — incident energy `E` \[eV\] this subsection is for.
/// - `curves` — the `NMU` direction-cosine curves, **ascending in `mu`**, at
///   least 2 entries (ENDF requires the grid to span `mu in [-1, 1]`).
#[derive(Debug, Clone)]
pub struct Law7Emission {
    /// Incident energy `E` \[eV\].
    pub e_in: f64,
    /// Direction-cosine curves, ascending in `mu`.
    pub curves: Vec<Law7Curve>,
}

/// One evaluation of a TAB1 at `x`: `(y, x_next, is_discontinuity)`.
///
/// Faithful, stateless port of NJOY2016 `terpa` (`endf.f90:1729-1818`; that
/// crate module does not carry this — see the groupr-only scope note in the
/// module hand-off). NJOY's version threads a monotone-search state (`ip`,
/// `ir`) across calls purely for speed; a fresh search each call (as here)
/// gives the identical result, just without that optimisation.
///
/// Returns `y = 0` with `x_next` = the first grid point when `x` is below the
/// table's range (`endf.f90:1798-1803`, label `170`); returns the boundary
/// value with `x_next` pushed to [`EMAX_TAB`] (or just short of the last
/// point, if that value is positive — `endf.f90:1793-1797`, label `160`) at or
/// above the table's last point; otherwise linearly locates the bracket and
/// interpolates with that segment's own ENDF law, flagging a discontinuity
/// when the segment is histogram or the *next* point repeats this bracket's
/// right edge (`endf.f90:1780-1787`, label `130`/`140`).
fn terpa_lookup(tab: &Tab1, x: f64) -> Result<(f64, f64, bool), NjoyError> {
    /// NJOY's `shade = 1.00001` (`endf.f90:1737`).
    const SHADE: f64 = 1.000_01;
    /// NJOY's `xbig = 1.e12` (`endf.f90:1738`) — "no further point" sentinel.
    const EMAX_TAB: f64 = 1.0e12;

    let pairs = &tab.pairs;
    let n = pairs.len();
    if n == 0 {
        return Err(NjoyError::EndfParse(
            "terpa: TAB1 has no data points".into(),
        ));
    }
    let (x_first, _) = pairs[0];
    if x < x_first {
        // Fortran label 170: below the table.
        return Ok((0.0, x_first, true));
    }
    let (x_last, y_last) = pairs[n - 1];
    if x >= x_last {
        // Fortran label 150/160: at or beyond the last point.
        if x < SHADE * x_last {
            let xnext = if y_last > 0.0 {
                SHADE * SHADE * x_last
            } else {
                EMAX_TAB
            };
            return Ok((y_last, xnext, false));
        }
        return Ok((0.0, EMAX_TAB, false));
    }

    // Interior: locate the bracket [i, i+1] with pairs[i].0 <= x < pairs[i+1].0
    // (x_first <= x < x_last guarantees 0 <= i <= n-2).
    let mut i = 0usize;
    while i + 1 < n && pairs[i + 1].0 <= x {
        i += 1;
    }
    let (x1, y1) = pairs[i];
    let (x2, y2) = pairs[i + 1];

    let right_point_1based = (i + 2) as u32;
    let law_code = tab
        .interp
        .iter()
        .find(|&&(nbt, _)| right_point_1based <= nbt)
        .map(|&(_, int)| int)
        .unwrap_or(2);
    let law = IntLaw::from_code(law_code);

    // Discontinuity: a histogram segment, or the point after this bracket
    // repeats its right edge (endf.f90:1786 `a(jp+2).eq.xnext`).
    let idis = law == IntLaw::Histogram || (i + 2 < n && pairs[i + 2].0 == x2);

    let y = if x == x1 {
        y1 // exact left-edge match (label 140): no interpolation needed
    } else {
        terp1(x1, y1, x2, y2, x, law)?
    };
    Ok((y, x2, idis))
}

/// Convert a File-6 LAW-7 angle-energy (lab-frame) emission into a lab
/// Legendre-coefficient (LAW-1-like) representation.
///
/// Faithful port of NJOY2016 `ll2lab` (`groupr.f90:8934-9061`). `nl` is the
/// number of lab Legendre coefficients to produce per point (`P_0 .. P_{nl-1}`).
///
/// # Method
/// Walks the union of every curve's own `E'` grid (via [`terpa_lookup`],
/// tracking the minimum next-break-point across all `nmu` curves,
/// `groupr.f90:8985-8990`). At each union-grid energy `ep`, evaluates all
/// `nmu` curves there, then Legendre-projects `f(ep|mu)` onto
/// `P_0(mu) .. P_{nl-1}(mu)` with 8-point Gauss quadrature
/// (`groupr.f90:8995-9010`), rescaling the result (`fact`) so its `P_0` term
/// exactly matches a direct trapezoid integral of `f(ep|mu)` over the mu grid
/// (`groupr.f90:9013-9021`). Adjacent points found to lie on the same local
/// linear trend as their neighbours are then thinned
/// (`groupr.f90:9026-9048`).
///
/// The returned [`LabDistribution::p0_integral`] is left at `0.0`: unlike
/// [`super::cm::cm2lab`], `ll2lab` performs no whole-spectrum energy-integral
/// normalization check (its own normalization is the per-point angular `fact`
/// rescale above).
///
/// # Errors
/// [`NjoyError::EndfParse`] for `nl == 0`, fewer than 2 curves, non-ascending
/// `mu`, or `e_in <= 0`.
pub fn ll2lab(emission: &Law7Emission, nl: usize) -> Result<LabDistribution, NjoyError> {
    /// NJOY's `tol = .005` (`groupr.f90:8958`).
    const TOL: f64 = 0.005;
    /// NJOY's `shade = .99999` (`groupr.f90:8959`).
    const SHADE: f64 = 0.999_99;

    if nl == 0 {
        return Err(NjoyError::EndfParse("ll2lab: nl must be >= 1".into()));
    }
    if emission.curves.len() < 2 {
        return Err(NjoyError::EndfParse(
            "ll2lab: need >= 2 direction-cosine curves".into(),
        ));
    }
    if emission.e_in <= 0.0 {
        return Err(NjoyError::EndfParse(
            "ll2lab: incident energy must be > 0".into(),
        ));
    }
    let mut prev_mu = f64::NEG_INFINITY;
    for c in &emission.curves {
        if c.mu < prev_mu {
            return Err(NjoyError::EndfParse(
                "ll2lab: direction cosines must be ascending".into(),
            ));
        }
        prev_mu = c.mu;
    }
    let nmu = emission.curves.len();
    let nll = nl - 1;

    let mut kept: Vec<(f64, Vec<f64>)> = Vec::new();
    let mut epnext = 0.0_f64;

    let mut guard: u64 = 0;
    while epnext < EMAX * (1.0 - SMALL) {
        let ep = epnext;
        epnext = EMAX;

        // --Retrieve the angular distribution at `ep` (groupr.f90:8977-8991).
        let mut amu = Vec::with_capacity(nmu);
        let mut fmu = Vec::with_capacity(nmu);
        for curve in &emission.curves {
            let (f, epn0, idis) = terpa_lookup(&curve.f_of_ep, ep)?;
            let mut epn = epn0;
            let test = SHADE * epn;
            if idis && ep < test {
                epn = test;
            }
            if epn < epnext * (1.0 - SMALL) {
                epnext = epn;
            }
            amu.push(curve.mu);
            fmu.push(f);
        }

        // --Legendre coefficients for this distribution (groupr.f90:8993-9010).
        let mut term = vec![0.0_f64; nl];
        for (&qp, &qw) in GAUSS8_NODES.iter().zip(GAUSS8_WEIGHTS.iter()) {
            let p = legndr(qp, nll);
            // Smallest 1-based j with amu(j) > qp (groupr.f90:8997-8999);
            // amu[j-2]/amu[j-1] (0-based) are Fortran's amu(j-1)/amu(j).
            let mut j = 1usize;
            while j <= nmu && qp >= amu[j - 1] {
                j += 1;
            }
            if j < 2 || j > nmu {
                return Err(NjoyError::EndfParse(
                    "ll2lab: quadrature abscissa outside the mu grid (grid must span [-1, 1])"
                        .into(),
                ));
            }
            let (mu_lo, mu_hi) = (amu[j - 2], amu[j - 1]);
            let (f_lo, f_hi) = (fmu[j - 2], fmu[j - 1]);
            let f = (qp - mu_lo) / (mu_hi - mu_lo);
            let ff = (1.0 - f) * f_lo + f * f_hi;
            for l in 0..nl {
                term[l] += ff * p[l] * qw;
            }
        }

        // --Normalization check against a direct trapezoid over mu
        //   (groupr.f90:9013-9021).
        let mut sum = 0.0;
        for i in 0..nmu - 1 {
            sum += (amu[i + 1] - amu[i]) * (fmu[i + 1] + fmu[i]) / 2.0;
        }
        let fact = if term[0] != 0.0 { sum / term[0] } else { 1.0 };
        let scaled: Vec<f64> = term.iter().map(|t| t * fact).collect();

        // --Is the previous kept point still needed? (groupr.f90:9024-9048).
        if kept.len() < 3 {
            kept.push((ep, scaled));
        } else {
            let n = kept.len();
            let (x0, y0) = kept[n - 2].clone();
            let (x1, y1) = kept[n - 1].clone();
            let f1 = (x1 - x0) / (ep - x0);
            let f2 = 1.0 - f1;
            let mut redundant = true;
            for l in 0..nl {
                let tmid = f1 * y0[l] + f2 * scaled[l];
                let test = TOL * tmid.abs() + SMALL / 100.0;
                if (tmid - y1[l]).abs() > test {
                    redundant = false;
                    break;
                }
            }
            if redundant {
                *kept.last_mut().unwrap() = (ep, scaled);
            } else {
                kept.push((ep, scaled));
            }
        }

        guard += 1;
        if guard > 10_000_000 {
            return Err(NjoyError::EndfParse(
                "ll2lab: union-grid march failed to advance".into(),
            ));
        }
    }

    Ok(LabDistribution {
        e_in: emission.e_in,
        nl,
        points: kept,
        p0_integral: 0.0,
    })
}
