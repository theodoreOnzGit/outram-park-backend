//! **THERMR's MF=3 for the inelastic MT, as ACER reads it back** (GitHub #865).
//!
//! ACER's thermal `ITIX` block is not `calcem`'s `xsi` at the incident
//! energies. It is what `gety1` interpolates, linearly, from the MF=3 section
//! THERMR writes for `mtref` (`aceth.f90:355`), and that section lives on the
//! PENDF's grid:
//!
//! 1. the **incident nuclide's elastic (MT=2) grid** up to `emax`, then `emax`
//!    and `1.00001·emax` (`thermr.f90:380-398`);
//! 2. at each of those points `calcem` stores `terp(esi, xsi, nne, E, 5)`, the
//!    last point zero (`thermr.f90:2440-2466`);
//! 3. for a coherent elastic scatterer `coh` then merges its own points: one
//!    7th-figure unit either side of every Bragg edge, bisection points where
//!    the coherent cross section is not yet linear to `tol`, and every point
//!    already there; at its new points the inelastic column is
//!    `terp(x, z, 5, E, 4)` over a 5-point window of the old grid
//!    (`thermr.f90:748-922`, `sigcoh` `:1146-1195`);
//! 4. `tpend` writes every point as `sigfig(E,7)`, `sigfig(σ,7)` (6 below
//!    1e-9), `thermr.f90:3217-3221`, through the ENDF text when the deck's
//!    units are (OpenMC's are).
//!
//! So the table depends on the incident evaluation's PENDF grid at the
//! thermal temperature, which the caller supplies (RECONR + BROADR, as NJOY's
//! deck runs them). Without it, `ITIX` differs from NJOY2016's by up to about
//! 1e-4 between grid points (Al-27 at 20 K).

use crate::acer::acesix::pendf_text;
use crate::acer::thermal::thermr_terp;
use crate::common::phys::{AMASSN_AMU, AMU_G, EV_ERG, HBAR_ERG_S};
use crate::endf::interp::{terp1, IntLaw};
use crate::mixr::mix::sigfig;
use crate::NjoyError;

/// `thermr.f90:160`, `:767`.
const SMALL: f64 = 1.0e-10;
/// The point THERMR adds just past `emax` (`thermr.f90:161`).
const UP: f64 = 1.00001;

/// `sigcoh` with the Bragg data read from an ENDF-6 tape (`lat = 10`,
/// `thermr.f90:1146-1195`): the edges as `tsq = E·econ` and their increments
/// `f`, and `emax`.
struct Sigcoh {
    tsq: Vec<f64>,
    f: Vec<f64>,
    recon: f64,
    emax: f64,
}

impl Sigcoh {
    fn new(s_of_e: &[(f64, f64)], emax: f64) -> Self {
        let amne = AMASSN_AMU * AMU_G;
        let econ = EV_ERG * 8.0 * (amne / HBAR_ERG_S) / HBAR_ERG_S;
        let mut blast = 0.0;
        let (mut tsq, mut f) = (Vec::new(), Vec::new());
        for &(e, s) in s_of_e {
            tsq.push(e * econ);
            f.push(s - blast);
            blast = s;
        }
        Self { tsq, f, recon: 1.0 / econ, emax }
    }

    /// The initial call (`e = 0`, label 200): the first `enext`.
    fn first_enext(&self, s_of_e: &[(f64, f64)]) -> f64 {
        s_of_e.first().map_or(self.emax, |p| sigfig(p.0, 7, -1))
    }

    /// Label 210: `(σ_coh(e), enext)`.
    fn at(&self, e: f64) -> (f64, f64) {
        let re = 1.0 / e;
        let mut s = 0.0;
        let mut last = false;
        let mut elim = 0.0;
        let k = self.tsq.len();
        for i in 0..k {
            elim = self.tsq[i] * self.recon;
            if elim >= e {
                break;
            }
            let f = if e > self.emax { 0.0 } else { self.f[i] };
            s += f; // `p(1) = 1` for nl = 1
            if i + 1 == k {
                last = true;
            }
        }
        let s = s * 1.0 * re; // `s*scon*re`, `scon = 1` for ENDF data
        if last {
            elim = self.emax;
        }
        if elim > self.emax {
            elim = self.emax;
        }
        let mut enext = sigfig(elim, 7, -1);
        if e > sigfig(enext, 7, -1) {
            enext = sigfig(elim, 7, 1);
        }
        (s, enext)
    }
}

/// THERMR's MF=3 for the inelastic MT as `(E [eV], σ [b])` after `tpend`'s
/// rounding and the ENDF text, up to just past `emax`.
///
/// - `elastic`: the incident nuclide's MT=2 on its PENDF at the thermal
///   temperature, ascending (only the energies are used);
/// - `esi`, `xsi`: `calcem`'s incident energies and cross sections, every
///   record including the first above `emax`;
/// - `coherent`: the coherent-elastic cumulative `S(E)` when the material has
///   one, at the same temperature;
/// - `tol`: THERMR's card-4 tolerance, which `coh` linearises to.
///
/// # Errors
/// As [`pendf_text`].
pub fn inelastic_mf3(
    elastic: &[(f64, f64)],
    emax: f64,
    esi: &[f64],
    xsi: &[f64],
    coherent: Option<&[(f64, f64)]>,
    tol: f64,
) -> Result<Vec<(f64, f64)>, NjoyError> {
    tpend_mf3(&thermr_grid(elastic, emax, esi, xsi, coherent, tol)?)
}

/// Steps 1-3 of [`inelastic_mf3`]: THERMR's grid and its inelastic column
/// before `tpend` rounds them, the grid `iel` then adds its own column to.
///
/// # Errors
/// As [`pendf_text`].
pub fn thermr_grid(
    elastic: &[(f64, f64)],
    emax: f64,
    esi: &[f64],
    xsi: &[f64],
    coherent: Option<&[(f64, f64)]>,
    tol: f64,
) -> Result<Vec<(f64, f64)>, NjoyError> {
    // 1. The elastic grid (`thermr.f90:380-398`), as `gety1` walks it.
    let mut grid: Vec<f64> = Vec::new();
    let mut idx = 0usize;
    let mut enext = elastic.first().map_or(emax, |p| pendf_text(p.0).unwrap_or(p.0));
    loop {
        let e = enext;
        grid.push(e);
        if e > emax * (1.0 + SMALL) {
            break;
        }
        while idx < elastic.len() && pendf_text(elastic[idx].0)? <= e {
            idx += 1;
        }
        enext = match elastic.get(idx) {
            Some(p) => pendf_text(p.0)?,
            None => UP * emax,
        };
        if enext > emax * (1.0 + SMALL) {
            enext = emax;
        }
        if (e - emax).abs() < SMALL * emax {
            enext = UP * emax;
        }
    }
    // 2. `calcem`'s column (`thermr.f90:2446-2462`).
    let nlt = if esi.len() <= 5 { esi.len().saturating_sub(1).max(1) } else { 5 };
    let ne = grid.len();
    let mut old: Vec<(f64, f64)> = grid
        .iter()
        .enumerate()
        .map(|(i, &e)| (e, if i + 1 == ne { 0.0 } else { thermr_terp(esi, xsi, e, nlt) }))
        .collect();
    // 3. `coh`.
    if let Some(s_of_e) = coherent {
        old = coh(&old, s_of_e, emax, tol);
    }
    Ok(old)
}

/// Step 4, `tpend` (`thermr.f90:3217-3221`): one MF=3 column as written,
/// `sigfig(E,7)` and `sigfig(σ,7)` (6 below 1e-9), through the ENDF text.
///
/// # Errors
/// As [`pendf_text`].
pub fn tpend_mf3(column: &[(f64, f64)]) -> Result<Vec<(f64, f64)>, NjoyError> {
    column
        .iter()
        .map(|&(e, z)| {
            let z = if z >= 1.0e-9 { sigfig(z, 7, 0) } else { sigfig(z, 6, 0) };
            Ok((pendf_text(sigfig(e, 7, 0))?, pendf_text(z)?))
        })
        .collect()
}

/// THERMR's MF=3 for the incoherent-elastic MT (`mtref+1`, or `mtref+2`
/// beside coherent elastic) as ACER reads it back: `iel` puts
/// `terp(esi, xie, nne, E, 3)` at every point of THERMR's grid (`grid`, from
/// [`thermr_grid`], after `coh` when there is one), the last point zero
/// (`thermr.f90:1407-1418`), and `tpend` rounds it ([`tpend_mf3`]).
///
/// `esi`, `xie`: `calcem`'s incident energies (every record, including the
/// first above `emax`) and `iel`'s cross section at each.
///
/// # Errors
/// As [`pendf_text`].
pub fn incoherent_elastic_mf3(grid: &[(f64, f64)], esi: &[f64], xie: &[f64]) -> Result<Vec<(f64, f64)>, NjoyError> {
    let ne = grid.len();
    let column: Vec<(f64, f64)> = grid
        .iter()
        .enumerate()
        .map(|(i, &(e, _))| (e, if i + 1 == ne { 0.0 } else { thermr_terp(esi, xie, e, 3) }))
        .collect();
    tpend_mf3(&column)
}

/// `coh` (`thermr.f90:748-922`) on a grid of `(E, σ_inel)`: returns the merged
/// grid with the inelastic column carried to `coh`'s new points.
fn coh(old: &[(f64, f64)], s_of_e: &[(f64, f64)], emax: f64, tol: f64) -> Vec<(f64, f64)> {
    const TOLMIN: f64 = 1.0e-6;
    const EPS: f64 = 3.0e-5;
    const IMAX: usize = 20;
    let sc = Sigcoh::new(s_of_e, emax);
    let ne = old.len();
    let mut nlt = 5.min(ne);
    let mut nlt1 = nlt - 1;
    let mut out: Vec<(f64, f64)> = Vec::new();
    // The 5-point window `x`, `z` of old points (1-based in the Fortran).
    let mut x = [0.0f64; 6];
    let mut z = [0.0f64; 6];
    let mut enext = sc.first_enext(s_of_e);
    let mut iex = 0usize; // old points read so far
    let mut ix = 1usize;
    // Label 100: copy the old points up to the first edge.
    loop {
        if iex >= ne {
            return old.to_vec();
        }
        let (e, v) = old[iex];
        iex += 1;
        if e > enext * (1.0 + SMALL) {
            // Label 105.
            ix += 1;
            x[ix] = e;
            z[ix] = v;
            if ix < nlt {
                continue;
            }
            break;
        }
        x[1] = e;
        z[1] = v;
        out.push((e, v));
    }
    // The stack: index 1..=i, `stk[i]` the lowest energy.
    let mut stk = [(0.0f64, 0.0f64); IMAX + 2];
    let upstk = |e: f64, s: f64, i: &mut usize, stk: &mut [(f64, f64); IMAX + 2]| {
        *i += 1;
        stk[*i] = stk[*i - 1];
        stk[*i - 1] = (e, s);
    };
    let mut e = enext;
    let (s, en) = sc.at(e);
    enext = en;
    stk[1] = (e, s);
    let mut i = 1usize;
    'l120: loop {
        // Label 120: add the next Bragg point.
        e = enext;
        let (s, en) = sc.at(e);
        enext = en;
        upstk(e, s, &mut i, &mut stk);
        'l125: loop {
            // Label 125: make sure input grid points are included.
            let mut hit = None;
            for k in 1..=nlt {
                if x[k] > stk[i].0 * (1.0 + SMALL) {
                    hit = Some(k);
                    break;
                }
            }
            if let Some(k) = hit {
                if x[k] < stk[i - 1].0 * (1.0 - SMALL) {
                    let (s, _) = sc.at(x[k]);
                    upstk(x[k], s, &mut i, &mut stk);
                }
            }
            // Label 140: is the panel linear?
            let accept = if i == IMAX {
                true
            } else {
                let xm = sigfig(0.5 * (stk[i - 1].0 + stk[i].0), 7, 0);
                if stk[i - 1].0 - stk[i].0 < EPS * xm {
                    true
                } else {
                    let (s, _) = sc.at(xm);
                    let ym = terp1(stk[i].0, stk[i].1, stk[i - 1].0, stk[i - 1].1, xm, IntLaw::LinLin)
                        .unwrap_or(stk[i].1);
                    let test = (tol * s.abs()).max(TOLMIN);
                    if (s - ym).abs() > test {
                        // Label 210.
                        upstk(xm, s, &mut i, &mut stk);
                        continue 'l125;
                    }
                    true
                }
            };
            debug_assert!(accept);
            // Label 160: save the top of the stack.
            let ej1 = stk[i].0;
            // Label 170: slide the window so that ej1 <= x(3).
            while !(ej1 <= x[3] * (1.0 + SMALL) || iex == ne) {
                for k in 1..=nlt1 {
                    x[k] = x[k + 1];
                    z[k] = z[k + 1];
                }
                let (e2, v2) = old[iex];
                iex += 1;
                x[nlt] = e2;
                z[nlt] = v2;
                if iex == ne {
                    nlt -= 1;
                    nlt1 -= 1;
                }
            }
            // Label 190.
            let beyond = stk[i].0 > emax * (1.0 + SMALL);
            let zz = if beyond { 0.0 } else { thermr_terp(&x[1..=nlt], &z[1..=nlt], ej1, nlt1) };
            out.push((ej1, zz));
            if beyond {
                return out; // label 230
            }
            i -= 1;
            if i > 1 {
                continue 'l125;
            }
            continue 'l120;
        }
    }
}

/// `gety1` on THERMR's MF=3 (one interpolation region, lin-lin): the
/// inelastic cross section ACER stores in `ITIX` at `e`.
pub fn itix_at(mf3: &[(f64, f64)], e: f64) -> f64 {
    let k = mf3.partition_point(|p| p.0 <= e);
    if k == 0 {
        return 0.0;
    }
    let (x1, y1) = mf3[k - 1];
    if x1 == e || k == mf3.len() {
        return y1;
    }
    let (x2, y2) = mf3[k];
    terp1(x1, y1, x2, y2, e, IntLaw::LinLin).unwrap_or(y1)
}
