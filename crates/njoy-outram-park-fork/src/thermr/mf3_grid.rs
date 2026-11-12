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
pub(crate) struct Sigcoh {
    tsq: Vec<f64>,
    f: Vec<f64>,
    recon: f64,
    scon: f64,
    emax: f64,
    enext0: f64,
    /// `nl` returned by the initialisation call: the number of stored shells
    /// (`k`, including the closing `ulim` entry); `coh` writes it as `-LIP`.
    pub(crate) nbragg: usize,
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
        let enext0 = s_of_e.first().map_or(emax, |p| sigfig(p.0, 7, -1));
        Self { tsq, f, recon: 1.0 / econ, scon: 1.0, emax, enext0, nbragg: s_of_e.len() }
    }

    /// `sigcoh`'s internal lattices (`lat = 1` graphite, `2` beryllium, `3`
    /// beryllium oxide; `thermr.f90:1017-1114`, `form` `:1215-1243`): the
    /// initialisation call (`e = 0`) at `temp` [K], `emax` [eV] and `natom`
    /// principal atoms. Used for ENDF-4/5 tapes, whose MF=7 carries no Bragg data.
    ///
    /// # Errors
    /// [`NjoyError::InvalidInput`] for any other `lat` ("illegal lat").
    pub(crate) fn builtin(lat: i32, temp: f64, emax: f64, natom: f64) -> Result<Self, NjoyError> {
        use crate::common::phys::PI;
        use petir::mathf::RealMath;
        const DWF1: [f64; 10] = [2.1997, 2.7448, 3.2912, 3.8510, 4.4210, 4.9969, 6.1624, 7.3387, 9.6287, 11.992];
        const DWF2: [f64; 10] =
            [3.16663, 3.88842, 4.62944, 5.40517, 6.19880, 7.0042, 8.63665, 10.2865, 0.0, 0.0];
        const DWF3: [f64; 10] = [2.153, 2.6374, 3.1348, 3.6513, 4.1798, 4.7164, 5.8052, 6.9068, 0.0, 0.0];
        const TMP: [f64; 10] = [296.0, 400.0, 500.0, 600.0, 700.0, 800.0, 1000.0, 1200.0, 1600.0, 2000.0];
        const SQRT3: f64 = 1.732050808;
        const CW: f64 = 0.658173e-15;
        const EPS: f64 = 0.05;
        let twopis = (2.0 * PI) * (2.0 * PI);
        let amne = AMASSN_AMU * AMU_G;
        let econ = EV_ERG * 8.0 * (amne / HBAR_ERG_S) / HBAR_ERG_S;
        let recon = 1.0 / econ;
        let tsqx = econ / 20.0;
        let (a, c, amsc, scoh, wal2) = match lat {
            1 => (2.4573e-8, 6.700e-8, 12.011, 5.50 / natom, thermr_terp(&TMP, &DWF1, temp, 2)),
            2 => (2.2856e-8, 3.5832e-8, 9.01, 7.53 / natom, thermr_terp(&TMP, &DWF2, temp, 2)),
            3 => (2.695e-8, 4.39e-8, 12.5, 1.0 / natom, thermr_terp(&TMP, &DWF3, temp, 2)),
            _ => return Err(NjoyError::InvalidInput("sigcoh: illegal lat.".into())),
        };
        let c1 = 4.0 / (3.0 * a * a);
        let c2 = 1.0 / (c * c);
        let four_pi = 4.0 * PI;
        let scon = scoh * (four_pi * four_pi) / (2.0 * a * a * c * SQRT3 * econ);
        let wint = CW * amsc * wal2;
        let t2 = HBAR_ERG_S / (2.0 * AMU_G * amsc);
        let ulim = econ * emax;
        let tausq = |m1: i32, m2: i32, m3: i32| -> f64 {
            (c1 * f64::from(m1 * m1 + m2 * m2 + m1 * m2) + f64::from(m3 * m3) * c2) * twopis
        };
        let form = |l1: i32, l2: i32, l3: i32| -> f64 {
            match lat {
                1 => {
                    if (l3 / 2) * 2 != l3 {
                        let s = (PI * f64::from(l1 - l2) / 3.0).r_sin();
                        s * s
                    } else {
                        (6.0 + 10.0 * (2.0 * PI * f64::from(l1 - l2) / 3.0).r_cos()) / 4.0
                    }
                }
                2 => 1.0 + (2.0 * PI * f64::from(2 * l1 + 4 * l2 + 3 * l3) / 6.0).r_cos(),
                _ => {
                    (1.0 + (2.0 * PI * f64::from(2 * l1 + 4 * l2 + 3 * l3) / 6.0).r_cos())
                        * (7.54 + 4.24 + 11.31 * (3.0 * PI * f64::from(l3) / 4.0).r_cos())
                }
            }
        };
        let mut wrk: Vec<(f64, f64)> = Vec::new();
        let add = |tsq: f64, f: f64, wrk: &mut Vec<(f64, f64)>| {
            if !wrk.is_empty() && tsq > tsqx {
                for w in wrk.iter_mut() {
                    if !(tsq < w.0 || tsq >= (1.0 + EPS) * w.0) {
                        w.1 += f;
                        return;
                    }
                }
            }
            wrk.push((tsq, f));
        };
        let phi = ulim / twopis;
        let i1m = (a * phi.sqrt()) as i32 + 1;
        for i1 in 1..=i1m {
            let l1 = i1 - 1;
            let i2m = (0.5 * (f64::from(l1) + (3.0 * (a * a * phi - f64::from(l1 * l1))).sqrt())) as i32 + 1;
            for i2 in i1..=i2m {
                let l2 = i2 - 1;
                let x = phi - c1 * f64::from(l1 * l1 + l2 * l2 - l1 * l2);
                let i3m = if x > 0.0 { (c * x.sqrt()) as i32 } else { 0 } + 1;
                for i3 in 1..=i3m {
                    let l3 = i3 - 1;
                    let w1 = if l1 == l2 { 1.0 } else { 2.0 };
                    let w2 = if l1 == 0 && l2 == 0 {
                        0.5
                    } else if l1 == 0 || l2 == 0 {
                        1.0
                    } else {
                        2.0
                    };
                    let w3 = if l3 == 0 { 1.0 } else { 2.0 };
                    for sign in [1, -1] {
                        let l2s = sign * l2;
                        let tsq = tausq(l1, l2s, l3);
                        if tsq <= 0.0 || tsq > ulim {
                            continue;
                        }
                        let tau = tsq.sqrt();
                        let w = (-tsq * t2 * wint).r_exp() * w1 * w2 * w3 / tau;
                        let f = w * form(l1, l2s, l3);
                        add(tsq, f, &mut wrk);
                    }
                }
            }
        }
        // Selection sort, as `thermr.f90:1090-1104` (ties keep their order).
        let k = wrk.len();
        for i in 0..k.saturating_sub(1) {
            for j in (i + 1)..k {
                if wrk[j].0 < wrk[i].0 {
                    wrk.swap(i, j);
                }
            }
        }
        let last_f = wrk.last().map_or(0.0, |w| w.1);
        wrk.push((ulim, last_f));
        let enext0 = sigfig(recon * wrk[0].0, 7, -1);
        Ok(Self {
            tsq: wrk.iter().map(|w| w.0).collect(),
            f: wrk.iter().map(|w| w.1).collect(),
            recon,
            scon,
            emax,
            enext0,
            nbragg: wrk.len(),
        })
    }

    /// The initial call (`e = 0`, label 200): the first `enext`.
    fn first_enext(&self) -> f64 {
        self.enext0
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
        let s = s * self.scon * re; // `s*scon*re`, `scon = 1` for ENDF data
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

/// `sigcoh`'s initialisation for an ENDF-6 tape (`lat = 10`) from `rdelas`'s
/// `S(E)` table: the deck driver's entry to [`Sigcoh::new`].
pub(crate) fn sigcoh_from_endf6(s_of_e: &[(f64, f64)], emax: f64) -> Sigcoh {
    Sigcoh::new(s_of_e, emax)
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
    let sc = Sigcoh::new(s_of_e, emax);
    coh_with(old, &sc, emax, tol).into_iter().map(|(e, z, _)| (e, z)).collect()
}

/// [`coh`] on any [`Sigcoh`] (ENDF-6 `S(E)` table or the built-in `lat = 1..3`
/// lattices), returning `(E, previous column, coherent σ)` per point of the
/// merged grid: `ej(nex)` and `ej(nex+1)` of `thermr.f90:190-203`, both zero
/// at the last point (beyond `emax`).
pub(crate) fn coh_with(old: &[(f64, f64)], sc: &Sigcoh, emax: f64, tol: f64) -> Vec<(f64, f64, f64)> {
    const TOLMIN: f64 = 1.0e-6;
    const EPS: f64 = 3.0e-5;
    const IMAX: usize = 20;
    let ne = old.len();
    let mut nlt = 5.min(ne);
    let mut nlt1 = nlt - 1;
    let mut out: Vec<(f64, f64, f64)> = Vec::new();
    // The 5-point window `x`, `z` of old points (1-based in the Fortran).
    let mut x = [0.0f64; 6];
    let mut z = [0.0f64; 6];
    let mut enext = sc.first_enext();
    let mut iex = 0usize; // old points read so far
    let mut ix = 1usize;
    // Label 100: copy the old points up to the first edge.
    loop {
        if iex >= ne {
            return old.iter().map(|&(e, z)| (e, z, 0.0)).collect();
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
        out.push((e, v, 0.0));
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
            out.push((ej1, zz, if beyond { 0.0 } else { stk[i].1 }));
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
        // `gety1` shades the first abscissa down to `0.999999*x0` when its
        // value is not zero (`endf.f90:1967-1968`, `down`), so a point just
        // below it (an ulp, on a binary-unit deck whose MF=6 energies are not
        // rounded the way the MF=3 ones are) still reads `y0`.
        return match mf3.first() {
            Some(&(x0, y0)) if y0 != 0.0 && e >= 0.999999 * x0 => y0,
            _ => 0.0,
        };
    }
    let (x1, y1) = mf3[k - 1];
    if x1 == e || k == mf3.len() {
        return y1;
    }
    let (x2, y2) = mf3[k];
    terp1(x1, y1, x2, y2, e, IntLaw::LinLin).unwrap_or(y1)
}

#[cfg(test)]
mod itix_shading_tests {
    use super::itix_at;

    /// `gety1` shades a non-zero first abscissa down to `0.999999*x0`.
    #[test]
    fn first_point_is_shaded_down() {
        let mf3 = [(1.0e-5, 2.5), (2.0e-5, 3.0)];
        assert_eq!(itix_at(&mf3, 1.0e-5 * (1.0 - 1.0e-9)), 2.5);
        assert_eq!(itix_at(&mf3, 0.99e-5), 0.0);
        assert_eq!(itix_at(&mf3, 1.5e-5), 2.75);
    }
}
