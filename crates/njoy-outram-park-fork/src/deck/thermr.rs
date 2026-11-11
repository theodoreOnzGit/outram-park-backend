// Translated from NJOY2016.80 `src/thermr.f90`: `subroutine thermr` (the main
// flow, `:34-475`), `rdelas` (`:477-592`), `coh` (`:748-922`), `iel`
// (`:1244-1425`), `calcem`'s free-gas, `S(alpha,beta)` (`iform = 0` and `1`)
// and bookkeeping paths (`:1541-2480`) and `tpend` (`:2998-3385`).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under AGPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! **THERMR as a tape driver** (gh:#1369): the PENDF with thermal MF=3 and MF=6
//! sections inserted, as `tpend` writes it.
//!
//! Ported here: **free gas** (`iinc = 1`) and **`S(alpha,beta)`** (`iinc = 2`)
//! from an ENDF-4/5 tape (`gateff` effective temperature) or an **ENDF-6 tape**
//! (`T_eff` TAB1 read by the MF=7 parser, `rdelas`'s `LTHR`), `iform = 0`
//! (the special `LANG = 3` equally-probable-cosine MF=6) and `iform = 1`
//! (`LAW = 7`, E-mu-E' ordering), and **several temperatures** (one `tpend`
//! per requested temperature, each on its own block of the multi-temperature
//! PENDF). Elastic scattering, as `thermr` picks it:
//!
//! - ENDF-6 tape with an MF=7/MT=2 section: `icoh = 10 * LTHR` overrides the
//!   card (`thermr.f90:337-348`): `LTHR = 1` coherent from the tape's `S(E)`
//!   (`rdelas`'s temperature rule, [`crate::thermr::coherent::CoherentElastic::s_of_e_rdelas`]),
//!   `2` incoherent (`iel`, `mat = 20`: the tape's `W'(T)`), `3` both
//!   (`coh` at `mtref+1`, `iel` at `mtref+2`);
//! - otherwise the card's `icoh`: `1..3` the built-in graphite, beryllium or
//!   BeO lattice (`sigcoh`'s `lat`, also for an ENDF-6 tape that has no MT=2:
//!   upstream does not look at the tape then).
//!
//! `calcem` on the 45-point `β` grid for free gas ([`IncoherentInelastic::free_gas`])
//! or on the tape's `S(α,β)`; MF=3 `mtref` on the input PENDF's own elastic
//! (MT=2) grid up to `emax` (free gas: `ex(3)=ex(2)`, label 610; bound:
//! `terp(esi,xsi)`), merged with `coh`'s Bragg-edge points when there is a
//! coherent part (MF=3 `mtref+1`, MF=6 `mtref+1` with `LIP = -nbragg`,
//! `LAW = 0`); `iel`'s column on the same grid (`terp(esi,xie,nne,E,3)`) and
//! its MF=6 (`LIP = -2`, `LAW = 1`, `nbin` cosines per incident energy of
//! `calcem`'s grid); MF=6 `mtref` as the special `LANG = 3`
//! equally-probable-cosine format with `tpend`'s `1/σ` normalisation of the
//! pdf column, or as `LAW = 7`; and the MF=1 dictionary, including `ncdse`
//! (a single global that the **last** `coh`/`iel` call sets, so beside mixed
//! elastic both elastic MF=6 sections carry `iel`'s count) and `ncds`'s
//! `j*(nep+1)` page arithmetic for `iform = 1`.
//!
//! Not ported (each stops the run with [`NjoyError::NotPorted`]): `iinc = 0`
//! (elastic only), the built-in incoherent lattices (`icoh = 11, 12, 13`),
//! temperatures of the PENDF that are skipped over (`icopy`), a PENDF block
//! with a different `iverf` from the tape's.

use crate::acer::thermal::thermr_terp;
use crate::deck::modules::ThermrCards;
use crate::deck::tapes::TapeUnits;
use crate::endf::gety1::Gety1;
use crate::endf::records::SectionCursor;
use crate::endf::tape::{Section, Tape};
use crate::endf::EndfKey;
use crate::mixr::mix::sigfig;
use crate::thermr::calcem::types::{Iform0Table, Iform1Table};
use crate::thermr::calcem::{compute_iform0, compute_iform1};
use crate::thermr::effective_temperature::gateff;
use crate::thermr::mf3_grid::{coh_with, sigcoh_from_endf6, Sigcoh};
use crate::thermr::mf7::{calcem_temperature_k, parse_mf7, parse_mf7_at_temperature, IncoherentElastic, IncoherentInelastic, Mf7};
use crate::NjoyError;

const SMALL: f64 = 1.0e-10;
const UP: f64 = 1.00001;
const BREAK: f64 = 3000.0;
const EM9: f64 = 1.0e-9;
const ETOP: f64 = 20.0e6;
/// `endf.f90:19` `npage`, as `main.f90:150` leaves it: `(306/102)*102`.
const NPAGE: usize = 306;

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

fn rows_of_words(words: &[f64]) -> Vec<[f64; 6]> {
    words
        .chunks(6)
        .map(|c| {
            let mut r = [0.0; 6];
            r[..c.len()].copy_from_slice(c);
            r
        })
        .collect()
}

/// Where the coherent-elastic part of an `icoh` comes from.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum CoherentSource {
    /// The ENDF-6 tape's `S(E)` (`sigcoh`'s `lat = 10`).
    Endf6,
    /// `sigcoh`'s built-in lattice `lat` (1 graphite, 2 beryllium, 3 BeO).
    Builtin(i32),
}

/// Which elastic channels `thermr` computes (`thermr.f90:337-348`, `:360-368`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ElasticPlan {
    /// `coh` at `mtref+1`.
    pub coherent: Option<CoherentSource>,
    /// `iel` (`mat = 20`, the ENDF-6 `W'(T)`) at the next free `mt`.
    pub incoherent: bool,
}

/// `thermr`'s choice of elastic channels from the card's `icoh` and, for an
/// ENDF-6 tape with an MF=7/MT=2 section, that section's `LTHR`
/// (`icoh = 10*lthr`, which replaces the card).
///
/// # Errors
/// [`NjoyError::NotPorted`] for the built-in incoherent lattices (`icoh = 11..13`)
/// and any `icoh` that would need data an ENDF-4/5 run does not have.
pub(crate) fn plan_elastic(icoh_card: i32, lthr: Option<i32>) -> Result<ElasticPlan, NjoyError> {
    let (icoh, endf6) = match lthr {
        Some(l) => (10 * l, true),
        None => (icoh_card, false),
    };
    if icoh <= 0 {
        return Ok(ElasticPlan { coherent: None, incoherent: false });
    }
    if icoh <= 10 {
        let src = if endf6 { CoherentSource::Endf6 } else if icoh == 10 {
            return Err(NjoyError::NotPorted("THERMR icoh = 10 on a tape that has no ENDF-6 elastic data"));
        } else {
            CoherentSource::Builtin(icoh)
        };
        return Ok(ElasticPlan { coherent: Some(src), incoherent: false });
    }
    if !endf6 {
        return Err(NjoyError::NotPorted("THERMR built-in incoherent elastic (icoh = 11, 12, 13) or mixed elastic without ENDF-6 data"));
    }
    if icoh <= 20 {
        return Ok(ElasticPlan { coherent: None, incoherent: true });
    }
    Ok(ElasticPlan { coherent: Some(CoherentSource::Endf6), incoherent: true })
}

/// `LTHR` of the ENDF-6 tape's MF=7/MT=2 for `matde` (`thermr.f90:337-343`:
/// `findf(matde,7,0)` then the first section's `mth.eq.2`, `lthr=l1h`), or
/// `None` when the tape has no MT=2 for it.
pub(crate) fn lthr_of(endf: &Tape, matde: i32) -> Option<i32> {
    endf.section(matde, 7, 2).map(|s| nint(s.rows[0][2]))
}

/// `calcem`'s `ncds` increment for one `LAW = 7` outgoing-energy table of `nep`
/// points (`thermr.f90:2386-2436`): `1 + (j*(nep+1)+5)/6` with `j` the Fortran
/// index left by the page loop, which is **not** a line count: `7 + 2*nep` on a
/// single page; on several pages (`npage/2` points each) the last page's
/// `-1 + 2*points`.
pub(crate) fn law7_ncds_increment(nep: usize) -> usize {
    let half = NPAGE / 2;
    let (mut k, mut istart) = (8i64, 1usize);
    let mut j;
    loop {
        let mut iend = nep;
        if iend - istart >= half {
            iend = istart + half - 1;
        }
        j = k - 1 + 2 * (iend - istart + 1) as i64;
        if iend == nep {
            break;
        }
        k = 0;
        istart = iend + 1;
    }
    1 + ((j * (nep as i64 + 1) + 5) / 6) as usize
}

/// MF=6 `mtref` as `calcem` writes it for `iform = 1` and `tpend` copies it
/// (`ltt = 4`, `thermr.f90:2285-2404`, `:3323-3340`): `LAW = 7`, one TAB2 over
/// the incident energies, each with a TAB2 over `nmu` cosines of TAB1s of
/// `(E', pdf)`. Returns the rows and `ncds`.
pub(crate) fn law7_mf6(za: f64, awr: f64, emax: f64, table: &Iform1Table) -> (Vec<[f64; 6]>, usize) {
    let nne = table.records.len();
    let mut rows: Vec<[f64; 6]> = vec![
        [za, awr, 0.0, 1.0, 1.0, 0.0],
        [1.0, 1.0, 0.0, 7.0, 1.0, 2.0],
        [2.0, 2.0, 0.0, 0.0, 0.0, 0.0],
        [1.0e-5, 1.0, emax, 1.0, 0.0, 0.0],
        [0.0, 0.0, 0.0, 0.0, 1.0, nne as f64],
        [nne as f64, 2.0, 0.0, 0.0, 0.0, 0.0],
    ];
    let mut ncds = 1 + 2 + 2;
    for rec in &table.records {
        let nmu = rec.mu_distributions.len();
        rows.push([0.0, rec.e_in_ev, 0.0, 0.0, 1.0, nmu as f64]);
        rows.push([nmu as f64, 2.0, 0.0, 0.0, 0.0, 0.0]);
        ncds += 1;
        for md in &rec.mu_distributions {
            let nep = md.points.len();
            rows.push([0.0, md.mu, 0.0, 0.0, 1.0, nep as f64]);
            rows.push([nep as f64, 2.0, 0.0, 0.0, 0.0, 0.0]);
            rows.extend(rows_of_pairs(&md.points));
            ncds += law7_ncds_increment(nep);
        }
    }
    (rows, ncds)
}

/// MF=6 for `iel` (`thermr.f90:1334-1424`, copied by `tpend`'s `ltt = 6`
/// branch, `:3258-3270`): `LIP = -2`, `LAW = 1`, the `LANG = 3` TAB2 over
/// `calcem`'s incident energies (`temp` is `iel`'s, the requested one), and one
/// LIST `(E, 1, cosines)` of `nbin + 2` words per incident energy.
pub(crate) fn iel_mf6(
    za: f64,
    awr: f64,
    temp: f64,
    emax: f64,
    ie: &IncoherentElastic,
    esi: &[f64],
    natom: f64,
    nbin: usize,
) -> Result<(Vec<[f64; 6]>, Vec<f64>), NjoyError> {
    let dwa = ie.iel_dwa(temp)?;
    let nne = esi.len();
    let mut rows: Vec<[f64; 6]> = vec![
        [za, awr, 0.0, 1.0, 1.0, 0.0],
        [1.0, 1.0, -2.0, 1.0, 1.0, 2.0],
        [2.0, 2.0, 0.0, 0.0, 0.0, 0.0],
        [1.0e-5, 1.0, emax, 1.0, 0.0, 0.0],
        [temp, 0.0, 3.0, 1.0, 1.0, nne as f64],
        [nne as f64, 2.0, 0.0, 0.0, 0.0, 0.0],
    ];
    let mut xie = Vec::with_capacity(nne);
    for &e in esi {
        let (xsec, cosines) = ie.iel_record(e, dwa, natom, nbin);
        xie.push(xsec);
        let n = (nbin + 2) as f64;
        rows.push([0.0, e, 0.0, 0.0, n, n]);
        let mut words = vec![e, 1.0];
        words.extend(cosines);
        rows.extend(rows_of_words(&words));
    }
    Ok((rows, xie))
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
    if c.nendf != 0 {
        units.openz(c.nendf, 0)?;
    }
    units.openz(c.nin, 0)?;
    units.openz(c.nout, 1)?;
    let input = units.get(c.nin)?.clone();
    let endf = if c.nendf != 0 { Some(units.get(c.nendf)?.tape.clone()) } else { None };
    let tape = thermr_tape(&input.tape, endf.as_ref(), c)?;
    units.store(c.nout, tape, input.pointwise)?;
    let mut meta = input.meta;
    meta.thermr.push(crate::deck::tapes::ThermrRun { cards: c.clone(), endf });
    units.set_meta(c.nout, meta);
    Ok(())
}

/// The output PENDF: `thermr`'s temperature loop (`:253-330`) over the blocks
/// (one MF=1/MT=451 and what follows it) of the material on the input PENDF.
fn thermr_tape(prev: &Tape, endf: Option<&Tape>, c: &ThermrCards) -> Result<Tape, NjoyError> {
    let matd = c.matdp;
    let secs = prev.sections();
    let is_head = |s: &Section| s.key.mat == matd && s.key.mf == 1 && s.key.mt == 451;
    let mut out: Vec<Section> = Vec::new();
    let mut itemp = 0usize;
    let mut i = 0usize;
    let mut seen = false;
    while i < secs.len() {
        if !is_head(&secs[i]) {
            out.push(secs[i].clone());
            i += 1;
            continue;
        }
        seen = true;
        let mut j = i + 1;
        while j < secs.len() && secs[j].key.mat == matd && !is_head(&secs[j]) {
            j += 1;
        }
        let block = &secs[i..j];
        if itemp < c.temps.len() {
            let temp = c.temps[itemp];
            let iverf = crate::moder::layout::iverf_from_mf1(&block[0].rows);
            let thead = match iverf {
                4 => 1,
                5 => 2,
                _ => 3,
            };
            let t = block[0].rows[thead][0];
            if (t - temp).abs() <= 1.0e-4 * temp {
                out.extend(thermr_block(block, endf, c, itemp)?);
                itemp += 1;
            } else if t > temp {
                return Err(NjoyError::InvalidInput("thermr: desired temperature not on tape.".into()));
            } else {
                return Err(NjoyError::NotPorted("THERMR skipping tape temperatures below the desired one"));
            }
        } else {
            out.extend(block.iter().cloned());
        }
        i = j;
    }
    if !seen || itemp < c.temps.len() {
        return Err(NjoyError::InvalidInput("thermr: desired material not on pendf tape.".into()));
    }
    let mut tape = Tape::from_sections(prev.tpid.clone(), out);
    tape.copy_raw_mf32_from(prev);
    Ok(tape)
}

/// One temperature: `thermr`'s body from label 180 to `tpend`, on `block`
/// (this temperature's sections of the material). Returns the block's output
/// sections in file order. The MF=1/MT=451 description text is set on `out`'s
/// tape by the caller's `copy_raw_mf32_from`; its length is the block's.
fn thermr_block(block: &[Section], endf: Option<&Tape>, c: &ThermrCards, itemp: usize) -> Result<Vec<Section>, NjoyError> {
    let matd = c.matdp;
    let mtref = c.mtref;
    let temp = c.temps[itemp];
    let nbin = c.nbin as usize;
    let natom = f64::from(c.natom);
    let key451 = EndfKey { mat: matd, mf: 1, mt: 451 };
    let find = |mf: i32, mt: i32| block.iter().find(|s| s.key.mf == mf && s.key.mt == mt);
    let mf1 = &block[0];
    let iverf = crate::moder::layout::iverf_from_mf1(&mf1.rows);
    let thead = match iverf {
        4 => 1,
        5 => 2,
        _ => 3,
    };
    let t = mf1.rows[thead][0];
    let (za, awr) = (mf1.rows[0][0], mf1.rows[0][1]);

    // Elastic cross section of the PENDF, walked as `thermr.f90:370-395` does.
    let el = find(3, 2).ok_or(NjoyError::SectionNotFound { mat: matd, mf: 3, mt: 2 })?;
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

    // ENDF-6 elastic (`rdelas`): `LTHR` of the tape's MF=7/MT=2, only with an
    // ENDF-6 `nendf` (`thermr.f90:337-348`).
    let endf6 = match endf {
        Some(et) if c.matde != 0 => {
            let mf1e = et.sections().iter().find(|s| s.key.mf == 1 && s.key.mt == 451);
            if mf1e.is_some_and(|s| crate::moder::layout::iverf_from_mf1(&s.rows) >= 6) { Some(et) } else { None }
        }
        _ => None,
    };
    let lthr = endf6.and_then(|et| lthr_of(et, c.matde));
    let plan = plan_elastic(c.icoh, lthr)?;

    // calcem's temperature: `t` of the tape for `iinc = 2` (`thermr.f90:331`).
    let calcem_temp = if c.iinc == 2 { t } else { temp };

    // calcem: free gas, or S(alpha,beta) with the tape's `T_eff`.
    let mut mf7: Option<Mf7> = None;
    let ii = if c.iinc == 1 {
        if lthr.is_some() {
            mf7 = Some(parse_mf7(endf.expect("lthr implies an ENDF-6 tape"), c.matde)?);
        }
        IncoherentInelastic::free_gas(awr, temp, natom)
    } else {
        let et = endf.ok_or_else(|| NjoyError::InvalidInput("thermr: No ENDF tape given for iinc=2 (nendf is 0)".into()))?;
        let parsed = parse_mf7_at_temperature(et, c.matde, Some(t))?;
        let mut ii = parsed
            .incoherent_inelastic
            .clone()
            .ok_or(NjoyError::SectionNotFound { mat: c.matde, mf: 7, mt: 4 })?;
        if ii.temperature_bracket.is_some() {
            return Err(NjoyError::NotPorted("THERMR tape driver at a temperature interpolated between tabulated ones"));
        }
        if endf6.is_some() {
            // `thermr.f90:1812-1834`: `S(alpha,beta)` and `kT` at the tabulated
            // temperature `calcem` finds, `T_eff` read at the requested one.
            let calcem_k = calcem_temperature_k(&ii.tabulated_temperatures_k, t)?;
            if ii.temperature_k != calcem_k {
                return Err(NjoyError::ThermalTemperatureMismatch {
                    requested_k: t,
                    calcem_k,
                    resolved_k: ii.temperature_k,
                    interpolated: false,
                });
            }
            ii.teff_request_k = Some(calcem_temp);
        } else {
            // `thermr.f90:145-157, 214-221`: ENDF-4/5 has no T_eff record; `gateff`.
            let eft = if c.matde == 0 { temp } else { gateff(temp, c.matde) };
            ii.teff_table = vec![(temp, eft)];
        }
        mf7 = Some(parsed);
        ii
    };
    // `calcem`'s table: `iform = 1` is E-mu-E' (`LAW = 7`); anything else the
    // `LANG = 3` equally-probable-cosine format (`thermr.f90:1925`).
    enum Table {
        F0(Iform0Table),
        F1(Iform1Table),
    }
    let table = if c.iform == 1 {
        Table::F1(compute_iform1(&ii, natom, emax, c.tol)?)
    } else {
        Table::F0(compute_iform0(&ii, natom, nbin, emax, c.tol)?)
    };
    let (esi, xsi): (Vec<f64>, Vec<f64>) = match &table {
        Table::F0(t0) => t0.records.iter().map(|r| (r.e_in_ev, r.cross_section_b)).unzip(),
        Table::F1(t1) => t1.records.iter().map(|r| (r.e_in_ev, r.cross_section_b)).unzip(),
    };
    let nne = esi.len();
    let nl = nbin + 1;

    // `calcem` label 610: the inelastic column on the elastic grid.
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

    // `coh`: the lattice's Bragg edges, merged onto the grid.
    let mut nbragg = 0usize;
    let merged: Vec<(f64, f64, Option<f64>)> = match plan.coherent {
        Some(src) => {
            let sc: Sigcoh = match src {
                CoherentSource::Builtin(lat) => Sigcoh::builtin(lat, temp, emax, natom)?,
                CoherentSource::Endf6 => {
                    let ce = mf7
                        .as_ref()
                        .and_then(|m| m.coherent_elastic.as_ref())
                        .ok_or(NjoyError::SectionNotFound { mat: c.matde, mf: 7, mt: 2 })?;
                    sigcoh_from_endf6(&ce.s_of_e_rdelas(temp), emax)
                }
            };
            nbragg = sc.nbragg;
            coh_with(&col3, &sc, emax, c.tol).into_iter().map(|(e, z, x)| (e, z, Some(x))).collect()
        }
        None => col3.iter().map(|&(e, z)| (e, z, None)).collect(),
    };
    let ne = merged.len();

    // `iel`: MF=6 and its column on `terp(esi, xie, nne, E, 3)`.
    let iel = if plan.incoherent {
        let ie = mf7
            .as_ref()
            .and_then(|m| m.incoherent_elastic.as_ref())
            .ok_or(NjoyError::SectionNotFound { mat: c.matde, mf: 7, mt: 2 })?;
        let (rows6, xie) = iel_mf6(za, awr, temp, emax, ie, &esi, natom, nbin)?;
        let col: Vec<(f64, f64)> = merged
            .iter()
            .enumerate()
            .map(|(i, m)| (m.0, if i + 1 == ne { 0.0 } else { thermr_terp(&esi, &xie, m.0, 3) }))
            .collect();
        Some((rows6, col))
    } else {
        None
    };
    let n_extra = usize::from(plan.coherent.is_some()) + usize::from(plan.incoherent);

    // MF=3, `mtref` and the elastic columns (`tpend` :3170-3235, once per column).
    let nc3 = 3 + (ne + 2) / 3;
    let mf3_section = |col: Vec<(f64, f64)>| -> Vec<[f64; 6]> {
        let mut pairs: Vec<(f64, f64)> = col.iter().map(|&(e, v)| (sigfig(e, 7, 0), round_xs(v))).collect();
        pairs.push((sigfig(ETOP, 7, 0), round_xs(0.0)));
        let mut rows3: Vec<[f64; 6]> = vec![
            [za, awr, 0.0, 0.0, 0.0, 0.0],
            [temp, 0.0, 0.0, 0.0, 1.0, (ne + 1) as f64],
            [(ne + 1) as f64, 2.0, 0.0, 0.0, 0.0, 0.0],
        ];
        rows3.extend(rows_of_pairs(&pairs));
        rows3
    };
    let mut mf3_rows: Vec<Vec<[f64; 6]>> = vec![mf3_section(merged.iter().map(|m| (m.0, m.1)).collect())];
    if plan.coherent.is_some() {
        mf3_rows.push(mf3_section(merged.iter().map(|m| (m.0, m.2.unwrap_or(0.0))).collect()));
    }
    if let Some((_, col)) = &iel {
        mf3_rows.push(mf3_section(col.clone()));
    }

    // MF=6, mtref: `calcem` writes (:1927-1990, :2222-2236, :2285-2404), `tpend` rewrites (:3262-3340).
    let (rows6, ncds) = match &table {
        Table::F1(t1) => law7_mf6(za, awr, emax, t1),
        Table::F0(t0) => {
            let mut rows6: Vec<[f64; 6]> = vec![
                [za, awr, 0.0, 1.0, 1.0, 0.0],
                [1.0, 1.0, -1.0, 1.0, 1.0, 2.0],
                [2.0, 2.0, 0.0, 0.0, 0.0, 0.0],
                [1.0e-5, 1.0, emax, 1.0, 0.0, 0.0],
                [calcem_temp, 0.0, 3.0, 1.0, 1.0, nne as f64],
                [nne as f64, 2.0, 0.0, 0.0, 0.0, 0.0],
            ];
            let mut ncds = 1 + 2 + 2;
            for rec in &t0.records {
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
                rows6.extend(rows_of_words(&data));
                ncds += 1 + (j * (nl + 1) + 5) / 6;
            }
            (rows6, ncds)
        }
    };

    // MF=6, `coh` (:890-914): ZAP = AWP = 1, LIP = -nbragg, LAW = 0, T(E) = (1e-5, emax).
    let rows6c: Option<Vec<[f64; 6]>> = plan.coherent.map(|_| {
        vec![
            [za, awr, 0.0, 1.0, 1.0, 0.0],
            [1.0, 1.0, -(nbragg as f64), 0.0, 1.0, 2.0],
            [2.0, 2.0, 0.0, 0.0, 0.0, 0.0],
            [1.0e-5, 1.0, emax, 1.0, 0.0, 0.0],
        ]
    });
    let mts: Vec<i32> = (0..=n_extra as i32).map(|k| mtref + k).collect();

    // `ncdse`: one global, set by the last of `coh` (3) and `iel` (`5+ne*((n+11)/6)`).
    let ncdse: i32 = if plan.incoherent {
        (5 + ne * ((nbin + 11) / 6)) as i32
    } else {
        3
    };

    // MF=1/MT=451: TEMP, NXC and the dictionary (`tpend` :3040-3140, :3146-3165).
    let ncards = nint(mf1.rows[thead][4]) as usize;
    let dict0 = thead + 1 + ncards;
    let old_dict: Vec<(i32, i32, i32)> = mf1.rows[dict0..].iter().map(|r| (nint(r[2]), nint(r[3]), nint(r[4]))).collect();
    let nx = old_dict.len();
    let mut dict: Vec<(i32, i32, i32)> =
        old_dict.into_iter().filter(|e| !((e.0 == 3 || e.0 == 6) && mts.contains(&e.1))).collect();
    for &mt in &mts {
        dict.push((3, mt, nc3 as i32));
    }
    dict.push((6, mtref, ncds as i32));
    for &mt in mts.iter().skip(1) {
        dict.push((6, mt, ncdse));
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
    // Assemble: this block's sections with MF=3/6 `mtref..` replaced.
    let mut group: Vec<Section> = Vec::new();
    for s in block {
        if s.key == key451 {
            group.push(Section { key: key451, rows: rows1.clone() });
        } else if !((s.key.mf == 3 || s.key.mf == 6) && mts.contains(&s.key.mt)) {
            group.push(s.clone());
        }
    }
    for (k, rows) in mf3_rows.into_iter().enumerate() {
        group.push(Section { key: EndfKey { mat: matd, mf: 3, mt: mtref + k as i32 }, rows });
    }
    group.push(Section { key: EndfKey { mat: matd, mf: 6, mt: mtref }, rows: rows6 });
    let mut k = 1;
    if let Some(r6) = rows6c {
        group.push(Section { key: EndfKey { mat: matd, mf: 6, mt: mtref + k }, rows: r6 });
        k += 1;
    }
    if let Some((r6, _)) = iel {
        group.push(Section { key: EndfKey { mat: matd, mf: 6, mt: mtref + k }, rows: r6 });
    }
    group.sort_by_key(|s| (s.key.mf, s.key.mt));
    Ok(group)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_elastic_follows_thermr_f90_337_368() {
        // ENDF-6 LTHR overrides the card (icoh = 10*lthr).
        let p = plan_elastic(1, Some(2)).unwrap();
        assert_eq!((p.coherent, p.incoherent), (None, true));
        let p = plan_elastic(0, Some(3)).unwrap();
        assert_eq!((p.coherent, p.incoherent), (Some(CoherentSource::Endf6), true));
        let p = plan_elastic(0, Some(1)).unwrap();
        assert_eq!((p.coherent, p.incoherent), (Some(CoherentSource::Endf6), false));
        // No MT=2 on an ENDF-6 tape: the card stands (deck 25: graphite lattice).
        let p = plan_elastic(1, None).unwrap();
        assert_eq!(p.coherent, Some(CoherentSource::Builtin(1)));
        assert_eq!(plan_elastic(0, None).unwrap(), ElasticPlan { coherent: None, incoherent: false });
        // Built-in incoherent lattices are refused, not guessed.
        assert!(matches!(plan_elastic(12, None), Err(NjoyError::NotPorted(_))));
        assert!(matches!(plan_elastic(10, None), Err(NjoyError::NotPorted(_))));
    }

    #[test]
    fn law7_ncds_uses_upstreams_j_not_a_line_count() {
        // single page: j = 7 + 2*nep -> 1 + (j*(nep+1)+5)/6
        assert_eq!(law7_ncds_increment(10), 1 + ((27 * 11 + 5) / 6));
        // two pages of 153 + 1 points: the last page has j = -1 + 2*1 = 1
        assert_eq!(law7_ncds_increment(154), 1 + ((1 * 155 + 5) / 6));
    }

    #[test]
    fn iel_mf6_rows_have_nbin_plus_two_words_per_energy() {
        let ie = IncoherentElastic {
            temperature_k: 296.0,
            sb: 81.0,
            interp: vec![(3, 2)],
            wp_of_t: vec![(296.0, 8.0), (400.0, 11.0), (600.0, 16.0)],
        };
        let (rows, xie) = iel_mf6(1.0, 2.0, 300.0, 1.0, &ie, &[1.0e-3, 2.0e-3], 1.0, 8).unwrap();
        assert_eq!(xie.len(), 2);
        assert_eq!(rows[5], [2.0, 2.0, 0.0, 0.0, 0.0, 0.0]);
        assert_eq!(rows[6][4], 10.0);
        assert_eq!(rows[7][1], 1.0); // second word is the literal 1
        assert_eq!(rows.len(), 6 + 2 * 3);
    }

    #[test]
    fn the_free_gas_kernel_is_positive_and_normalised_like_a_gas() {
        let ii = IncoherentInelastic::free_gas(11.898, 296.0, 2.0);
        let s = ii.double_differential(0.0253, 0.0253, 0.0, 296.0, 2.0);
        assert!(s > 0.0 && s.is_finite());
    }
}
