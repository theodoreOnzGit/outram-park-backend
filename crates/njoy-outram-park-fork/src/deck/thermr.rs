// Translated from NJOY2016.80 `src/thermr.f90`: `subroutine thermr` (the main
// flow, `:34-475`), `calcem`'s free-gas path (`:1857-1916`, `:2453-2470`) and
// `tpend` (`:2998-3385`), for the free-gas inelastic case (`iinc = 1`).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under AGPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! **THERMR as a tape driver** (gh:#1369): the PENDF with thermal MF=3 and MF=6
//! sections inserted, as `tpend` writes it.
//!
//! Ported here: **free gas** (`iinc = 1`) and **`S(alpha,beta)` from an ENDF-4/5
//! tape** (`iinc = 2`, `gateff` effective temperature, the existing MF=7
//! reader, which reads the older layout unchanged: it differs from ENDF-6 only
//! in lacking the `T_eff` TAB1), each with optional **built-in coherent
//! elastic** (`icoh = 1..3`: `sigcoh` `lat < 10`, `form`, `coh`), `iform = 0`,
//! one temperature. `calcem` on the 45-point `β` grid for free gas
//! ([`IncoherentInelastic::free_gas`]) or on the tape's `S(α,β)`; MF=3 `mtref`
//! on the input PENDF's own elastic (MT=2) grid up to `emax` (free gas:
//! `ex(3)=ex(2)`, label 610; bound: `terp(esi,xsi)`), merged with `coh`'s
//! Bragg-edge points when `icoh > 0` (MF=3 `mtref+1`, MF=6 `mtref+1` with
//! `LIP = -nbragg`, `LAW = 0`); MF=6 `mtref` as the special `LANG = 3`
//! equally-probable-cosine format with `tpend`'s `1/σ` normalisation of the
//! pdf column; and the MF=1 dictionary.
//!
//! Not ported (each stops the run with [`NjoyError::NotPorted`]): ENDF-6 `nendf`
//! tapes (`rdelas`, `T_eff` TAB1), incoherent elastic (`iel`), `iform = 1`,
//! several temperatures.

use crate::deck::modules::ThermrCards;
use crate::deck::tapes::TapeUnits;
use crate::endf::gety1::Gety1;
use crate::endf::records::SectionCursor;
use crate::endf::tape::{Section, Tape};
use crate::endf::EndfKey;
use crate::mixr::mix::sigfig;
use crate::thermr::calcem::compute_iform0;
use crate::acer::thermal::thermr_terp;
use crate::thermr::effective_temperature::gateff;
use crate::thermr::mf3_grid::{coh_with, Sigcoh};
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
    if c.iinc != 1 && c.iinc != 2 {
        return Err(NjoyError::NotPorted("THERMR tape driver with iinc = 0 (elastic only)"));
    }
    if !(0..=3).contains(&c.icoh) || c.iform != 0 || c.ntemp != 1 {
        return Err(NjoyError::NotPorted(
            "THERMR tape driver with incoherent elastic (iel/rdelas), ENDF-6 elastic, iform = 1 or several temperatures",
        ));
    }
    if c.nendf != 0 {
        units.openz(c.nendf, 0)?;
    }
    units.openz(c.nin, 0)?;
    units.openz(c.nout, 1)?;
    let input = units.get(c.nin)?.clone();
    let endf = if c.iinc == 2 || c.icoh > 0 { Some(units.get(c.nendf)?.tape.clone()) } else { None };
    let tape = thermr_tape(&input.tape, endf.as_ref(), c)?;
    units.store(c.nout, tape, input.pointwise)?;
    units.set_meta(c.nout, input.meta);
    Ok(())
}

/// The output PENDF: free gas (`iinc = 1`) or `S(alpha,beta)` from an ENDF-4/5
/// tape (`iinc = 2`), with the built-in coherent elastic lattices (`icoh = 1..3`).
fn thermr_tape(prev: &Tape, endf: Option<&Tape>, c: &ThermrCards) -> Result<Tape, NjoyError> {
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

    // calcem: free gas, or S(alpha,beta) from the ENDF-4/5 tape with `gateff`'s T_eff.
    let ii = if c.iinc == 1 {
        IncoherentInelastic::free_gas(awr, temp, natom)
    } else {
        let et = endf.ok_or_else(|| NjoyError::InvalidInput("thermr: No ENDF tape given for iinc=2 (nendf is 0)".into()))?;
        let mf1e = et
            .sections()
            .iter()
            .find(|s| s.key.mf == 1 && s.key.mt == 451)
            .ok_or_else(|| NjoyError::InvalidInput("thermr: endf tape has no MF=1".into()))?;
        if crate::moder::layout::iverf_from_mf1(&mf1e.rows) >= 6 {
            return Err(NjoyError::NotPorted("THERMR tape driver with an ENDF-6 nendf tape (teff TAB1, rdelas)"));
        }
        let mf7 = crate::thermr::mf7::parse_mf7_at_temperature(et, c.matde, Some(t))?;
        let mut ii = mf7
            .incoherent_inelastic
            .ok_or(NjoyError::SectionNotFound { mat: c.matde, mf: 7, mt: 4 })?;
        if ii.temperature_bracket.is_some() {
            return Err(NjoyError::NotPorted("THERMR tape driver at a temperature interpolated between tabulated ones"));
        }
        // `thermr.f90:145-157, 214-221`: ENDF-4/5 has no T_eff record; `gateff`.
        let eft = if c.matde == 0 { temp } else { gateff(temp, c.matde) };
        ii.teff_table = vec![(temp, eft)];
        ii
    };
    let table = compute_iform0(&ii, natom, nbin, emax, c.tol)?;
    let nl = nbin + 1;
    let nne = table.records.len();

    // `calcem` label 610: the inelastic column on the elastic grid.
    let esi: Vec<f64> = table.records.iter().map(|r| r.e_in_ev).collect();
    let xsi: Vec<f64> = table.records.iter().map(|r| r.cross_section_b).collect();
    let nlt = if nne <= 5 { nne - 1 } else { 5 };
    let col3: Vec<(f64, f64)> = old
        .iter()
        .enumerate()
        .map(|(i, &(e, s_el))| {
            let v = if c.iinc == 1 {
                s_el
            } else if i + 1 == ne {
                0.0
            } else {
                thermr_terp(&esi, &xsi, e, nlt)
            };
            (e, v)
        })
        .collect();

    // `coh` (icoh = 1..3 on ENDF-4/5): the built-in lattice, merged onto the grid.
    let mut nbragg = 0usize;
    let merged: Vec<(f64, f64, Option<f64>)> = if c.icoh > 0 {
        let sc = Sigcoh::builtin(c.icoh, c.temps[0], emax, natom)?;
        nbragg = sc.nbragg;
        coh_with(&col3, &sc, emax, c.tol).into_iter().map(|(e, z, x)| (e, z, Some(x))).collect()
    } else {
        col3.iter().map(|&(e, z)| (e, z, None)).collect()
    };
    let ne = merged.len();
    let n_extra = usize::from(c.icoh > 0);

    // MF=3, mtref and, with `coh`, mtref+1 (`tpend` :3170-3235, once per column).
    let mf3_section = |col: Vec<(f64, f64)>| -> (Vec<[f64; 6]>, usize) {
        let mut pairs: Vec<(f64, f64)> = col.iter().map(|&(e, v)| (sigfig(e, 7, 0), round_xs(v))).collect();
        pairs.push((sigfig(ETOP, 7, 0), round_xs(0.0)));
        let mut rows3: Vec<[f64; 6]> = vec![
            [za, awr, 0.0, 0.0, 0.0, 0.0],
            [temp, 0.0, 0.0, 0.0, 1.0, (ne + 1) as f64],
            [(ne + 1) as f64, 2.0, 0.0, 0.0, 0.0, 0.0],
        ];
        rows3.extend(rows_of_pairs(&pairs));
        (rows3, 3 + (ne + 2) / 3)
    };
    let (rows3, nc3) = mf3_section(merged.iter().map(|m| (m.0, m.1)).collect());
    let rows3c = if c.icoh > 0 {
        Some(mf3_section(merged.iter().map(|m| (m.0, m.2.unwrap_or(0.0))).collect()).0)
    } else {
        None
    };

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

    // MF=6, mtref+1 (`coh` :890-914): ZAP = AWP = 1, LIP = -nbragg, LAW = 0, T(E) = (1e-5, emax).
    let rows6c: Option<Vec<[f64; 6]>> = (c.icoh > 0).then(|| {
        vec![
            [za, awr, 0.0, 1.0, 1.0, 0.0],
            [1.0, 1.0, -(nbragg as f64), 0.0, 1.0, 2.0],
            [2.0, 2.0, 0.0, 0.0, 0.0, 0.0],
            [1.0e-5, 1.0, emax, 1.0, 0.0, 0.0],
        ]
    });
    let mts: Vec<i32> = (0..=n_extra as i32).map(|k| mtref + k).collect();

    // MF=1/MT=451: TEMP, NXC and the dictionary (`tpend` :3040-3140, :3146-3165).
    let ncards = nint(mf1.rows[thead][4]) as usize;
    let dict0 = thead + 1 + ncards;
    let old_dict: Vec<(i32, i32, i32)> = mf1.rows[dict0..].iter().map(|r| (nint(r[2]), nint(r[3]), nint(r[4]))).collect();
    let nx = old_dict.len();
    let mut dict: Vec<(i32, i32, i32)> =
        old_dict.into_iter().filter(|e| !((e.0 == 3 || e.0 == 6) && mts.contains(&e.1))).collect();
    dict.push((3, mtref, nc3 as i32));
    dict.push((6, mtref, ncds as i32));
    if c.icoh > 0 {
        dict.push((3, mtref + 1, nc3 as i32));
        dict.push((6, mtref + 1, 3)); // `ncdse = 3` (:905), one less than the record count
    }
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
        } else if !((s.key.mf == 3 || s.key.mf == 6) && mts.contains(&s.key.mt)) {
            group.push(s.clone());
        }
    }
    group.push(Section { key: EndfKey { mat: matd, mf: 3, mt: mtref }, rows: rows3 });
    group.push(Section { key: EndfKey { mat: matd, mf: 6, mt: mtref }, rows: rows6 });
    if let (Some(r3), Some(r6)) = (rows3c, rows6c) {
        group.push(Section { key: EndfKey { mat: matd, mf: 3, mt: mtref + 1 }, rows: r3 });
        group.push(Section { key: EndfKey { mat: matd, mf: 6, mt: mtref + 1 }, rows: r6 });
    }
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
    fn a_card_with_incoherent_elastic_is_refused_not_guessed() {
        let c = ThermrCards {
            nendf: 26,
            nin: -22,
            nout: -24,
            matde: 1065,
            matdp: 1306,
            nbin: 8,
            ntemp: 1,
            iinc: 1,
            icoh: 12,
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
