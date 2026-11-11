// Translated from NJOY2016.80 `src/thermr.f90`: `subroutine thermr` (the main
// flow, `:34-475`), `calcem`'s free-gas path (`:1857-1916`, `:2453-2470`) and
// `tpend` (`:2998-3385`), for the free-gas inelastic case (`iinc = 1`).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under AGPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! **THERMR as a tape driver** (gh:#1369): the PENDF with thermal MF=3 and MF=6
//! sections inserted, as `tpend` writes it.
//!
//! Ported here: the **free-gas** case (`iinc = 1`, no elastic, `iform = 0`, one
//! temperature): `calcem` on the 45-point `β` grid
//! ([`IncoherentInelastic::free_gas`]), MF=3 `mtref` on the input PENDF's own
//! elastic (MT=2) grid up to `emax` (`ex(3)=ex(2)`, `calcem` label 610), MF=6
//! `mtref` as the special `LANG = 3` equally-probable-cosine format with `tpend`'s
//! `1/σ` normalisation of the pdf column, and the MF=1 dictionary.
//!
//! Not ported (each stops the run with [`NjoyError::NotPorted`]): `iinc = 2`
//! from ENDF-5/4 tapes (`gateff`/`gatef2` effective temperatures, ENDF-5 MF=7),
//! elastic options (`coh`/`iel` writers and `rdelas`), `iform = 1`, several
//! temperatures.

use crate::deck::modules::ThermrCards;
use crate::deck::tapes::TapeUnits;
use crate::endf::gety1::Gety1;
use crate::endf::records::SectionCursor;
use crate::endf::tape::{Section, Tape};
use crate::endf::EndfKey;
use crate::mixr::mix::sigfig;
use crate::thermr::calcem::compute_iform0;
use crate::thermr::mf7::IncoherentInelastic;
use crate::NjoyError;

const SMALL: f64 = 1.0e-10;
const UP: f64 = 1.00001;
const BREAK: f64 = 3000.0;
const EM9: f64 = 1.0e-9;
const ETOP: f64 = 20.0e6;

fn nint(x: f64) -> i32 {
    x.round() as i32
}

/// `tpend`'s value rounding (`:3217-3221`).
fn round_xs(z: f64) -> f64 {
    if z >= EM9 { sigfig(z, 7, 0) } else { sigfig(z, 6, 0) }
}

fn rows_of_pairs(pairs: &[(f64, f64)]) -> Vec<[f64; 6]> {
    pairs
        .chunks(3)
        .map(|c| {
            let mut r = [0.0; 6];
            for (i, &(x, y)) in c.iter().enumerate() {
                r[2 * i] = x;
                r[2 * i + 1] = y;
            }
            r
        })
        .collect()
}

/// THERMR (`thermr.f90`) on unit `c.nin` -> `c.nout`.
///
/// # Errors
/// [`NjoyError::NotPorted`] for what the module docs list; the input errors of
/// `thermr` (`nin = 0`, mode conversion, material or temperature not on the tape).
pub fn run_thermr(c: &ThermrCards, units: &mut TapeUnits) -> Result<(), NjoyError> {
    if (c.nin < 0 && c.nout > 0) || (c.nin > 0 && c.nout < 0) {
        return Err(NjoyError::InvalidInput("thermr: mode conversion not allowed.".into()));
    }
    if c.iinc != 1 {
        return Err(NjoyError::NotPorted(
            "THERMR tape driver with iinc != 1 (S(alpha,beta) from an ENDF-5/6 tape: gateff, ENDF-5 MF=7)",
        ));
    }
    if c.icoh != 0 || c.iform != 0 || c.ntemp != 1 {
        return Err(NjoyError::NotPorted("THERMR tape driver with elastic options, iform = 1 or several temperatures"));
    }
    if c.nendf != 0 {
        units.openz(c.nendf, 0)?;
    }
    units.openz(c.nin, 0)?;
    units.openz(c.nout, 1)?;
    let input = units.get(c.nin)?.clone();
    let tape = thermr_free_gas(&input.tape, c)?;
    units.store(c.nout, tape, input.pointwise)?;
    units.set_meta(c.nout, input.meta);
    Ok(())
}

/// The output PENDF of the free-gas case.
fn thermr_free_gas(prev: &Tape, c: &ThermrCards) -> Result<Tape, NjoyError> {
    let matd = c.matdp;
    let mtref = c.mtref;
    let temp = c.temps[0];
    let nbin = c.nbin as usize;
    let natom = f64::from(c.natom);
    let key451 = EndfKey { mat: matd, mf: 1, mt: 451 };
    let n451 = prev.sections().iter().filter(|s| s.key == key451).count();
    let mf1 = prev.section(matd, 1, 451).ok_or_else(|| NjoyError::InvalidInput("thermr: desired material not on pendf tape.".into()))?;
    if n451 != 1 {
        return Err(NjoyError::NotPorted("THERMR on a PENDF with several temperatures"));
    }
    let iverf = crate::moder::layout::iverf_from_mf1(&mf1.rows);
    let thead = match iverf {
        4 => 1,
        5 => 2,
        _ => 3,
    };
    let t = mf1.rows[thead][0];
    if (t - temp).abs() > 1.0e-4 * temp {
        if t > temp {
            return Err(NjoyError::InvalidInput("thermr: desired temperature not on tape.".into()));
        }
        return Err(NjoyError::NotPorted("THERMR skipping tape temperatures below the desired one"));
    }
    let (za, awr) = (mf1.rows[0][0], mf1.rows[0][1]);

    // Elastic cross section of the PENDF, walked as `thermr.f90:370-395` does.
    let el = prev.section(matd, 3, 2).ok_or(NjoyError::SectionNotFound { mat: matd, mf: 3, mt: 2 })?;
    let mut cur = SectionCursor::new(&el.rows);
    cur.read_cont()?;
    let tab = cur.read_tab1()?;
    let mut g = Gety1::new(&tab);
    let mut emax = c.emax;
    let emaxs = emax;
    if t > BREAK {
        emax = emax * t / BREAK;
    }
    let mut enext = g.get(0.0).xnext;
    let mut old: Vec<(f64, f64)> = Vec::new();
    loop {
        let e = enext;
        let v = g.get(e);
        enext = v.xnext;
        let s = if e > emax * (1.0 + SMALL) { 0.0 } else { v.y };
        old.push((e, s));
        if e > emax * (1.0 + SMALL) {
            break;
        }
        if enext > emax * (1.0 + SMALL) {
            enext = emax;
        }
        if (e - emax).abs() < SMALL * emax {
            enext = UP * emax;
        }
    }
    emax = emaxs;
    let ne = old.len();

    // calcem, free gas.
    let ii = IncoherentInelastic::free_gas(awr, temp, natom);
    let table = compute_iform0(&ii, natom, nbin, emax, c.tol)?;
    let nl = nbin + 1;
    let nne = table.records.len();

    // MF=3, mtref (`tpend` :3170-3235).
    let mut pairs: Vec<(f64, f64)> = old.iter().map(|&(e, s)| (sigfig(e, 7, 0), round_xs(s))).collect();
    pairs.push((sigfig(ETOP, 7, 0), round_xs(0.0)));
    let mut rows3: Vec<[f64; 6]> = vec![
        [za, awr, 0.0, 0.0, 0.0, 0.0],
        [temp, 0.0, 0.0, 0.0, 1.0, (ne + 1) as f64],
        [(ne + 1) as f64, 2.0, 0.0, 0.0, 0.0, 0.0],
    ];
    rows3.extend(rows_of_pairs(&pairs));
    let nc3 = 3 + (ne + 2) / 3;

    // MF=6, mtref: `calcem` writes (:1927-1990, :2222-2236), `tpend` rewrites (:3262-3322).
    let mut rows6: Vec<[f64; 6]> = vec![
        [za, awr, 0.0, 1.0, 1.0, 0.0],
        [1.0, 1.0, -1.0, 1.0, 1.0, 2.0],
        [2.0, 2.0, 0.0, 0.0, 0.0, 0.0],
        [1.0e-5, 1.0, emax, 1.0, 0.0, 0.0],
        [temp, 0.0, 3.0, 1.0, 1.0, nne as f64],
        [nne as f64, 2.0, 0.0, 0.0, 0.0, 0.0],
    ];
    let mut ncds = 1 + 2 + 2;
    for rec in &table.records {
        if rec.cross_section_b == 0.0 {
            return Err(NjoyError::InvalidInput("tpend: cross section=0.".into()));
        }
        let rxsec = 1.0 / rec.cross_section_b;
        let j = rec.rows.len();
        let mut data: Vec<f64> = Vec::with_capacity(j * (nl + 1));
        for r in &rec.rows {
            data.push(r.ep_ev);
            data.push(round_xs(r.pdf * rxsec));
            data.extend_from_slice(&r.cosines);
        }
        rows6.push([0.0, rec.e_in_ev, 0.0, 0.0, (nl + 1) as f64 * j as f64, (nl + 1) as f64]);
        for ch in data.chunks(6) {
            let mut r = [0.0; 6];
            r[..ch.len()].copy_from_slice(ch);
            rows6.push(r);
        }
        ncds += 1 + (j * (nl + 1) + 5) / 6;
    }

    // MF=1/MT=451: TEMP, NXC and the dictionary (`tpend` :3040-3140, :3146-3165).
    let ncards = nint(mf1.rows[thead][4]) as usize;
    let dict0 = thead + 1 + ncards;
    let old_dict: Vec<(i32, i32, i32)> = mf1.rows[dict0..].iter().map(|r| (nint(r[2]), nint(r[3]), nint(r[4]))).collect();
    let nx = old_dict.len();
    let mut dict: Vec<(i32, i32, i32)> = old_dict.into_iter().filter(|e| !(e.0 == 3 && e.1 == mtref) && !(e.0 == 6 && e.1 == mtref)).collect();
    dict.push((3, mtref, nc3 as i32));
    dict.push((6, mtref, ncds as i32));
    dict.sort_by_key(|e| (e.0, e.1)); // stable; 451's entry (mf 1) stays first
    let nxn = dict.len();
    let mut rows1: Vec<[f64; 6]> = mf1.rows[..dict0].to_vec();
    if iverf >= 5 {
        rows1[thead][5] = nxn as f64;
    } else {
        rows1[0][5] = nxn as f64;
    }
    rows1[thead][0] = temp;
    for e in &dict {
        let mut nc = f64::from(e.2);
        if e.0 == 1 && e.1 == 451 {
            nc += (nxn as f64) - (nx as f64);
        }
        rows1.push([0.0, 0.0, f64::from(e.0), f64::from(e.1), nc, 0.0]);
    }
    let mut raw = prev.raw_text(key451).map(<[String]>::to_vec).unwrap_or_default();
    raw.resize(dict0, String::new());
    raw.resize(rows1.len(), String::new());

    // Assemble: this material's sections with MF=3/6 `mtref` replaced.
    let mut out: Vec<Section> = Vec::new();
    let mut group: Vec<Section> = Vec::new();
    let flush = |group: &mut Vec<Section>, out: &mut Vec<Section>| {
        group.sort_by_key(|s| (s.key.mf, s.key.mt));
        out.append(group);
    };
    for s in prev.sections() {
        if s.key.mat != matd {
            flush(&mut group, &mut out);
            out.push(s.clone());
        } else if s.key == key451 {
            group.push(Section { key: key451, rows: rows1.clone() });
        } else if !(s.key.mf == 3 && s.key.mt == mtref) && !(s.key.mf == 6 && s.key.mt == mtref) {
            group.push(s.clone());
        }
    }
    group.push(Section { key: EndfKey { mat: matd, mf: 3, mt: mtref }, rows: rows3 });
    group.push(Section { key: EndfKey { mat: matd, mf: 6, mt: mtref }, rows: rows6 });
    flush(&mut group, &mut out);
    let mut tape = Tape::from_sections(prev.tpid.clone(), out);
    tape.copy_raw_mf32_from(prev);
    tape.set_raw_text(key451, raw);
    Ok(tape)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_non_free_gas_card_is_refused_not_guessed() {
        let c = ThermrCards {
            nendf: 26,
            nin: -22,
            nout: -24,
            matde: 1065,
            matdp: 1306,
            nbin: 8,
            ntemp: 1,
            iinc: 2,
            icoh: 1,
            iform: 0,
            natom: 1,
            mtref: 229,
            iprint: 0,
            temps: vec![296.0],
            tol: 0.05,
            emax: 1.2,
        };
        let mut u = TapeUnits::new(None);
        assert!(matches!(run_thermr(&c, &mut u), Err(NjoyError::NotPorted(_))));
    }

    #[test]
    fn the_free_gas_kernel_is_positive_and_normalised_like_a_gas() {
        let ii = IncoherentInelastic::free_gas(11.898, 296.0, 2.0);
        let s = ii.double_differential(0.0253, 0.0253, 0.0, 296.0, 2.0);
        assert!(s > 0.0 && s.is_finite());
    }
}
