// Ported from NJOY2016 `src/heatr.f90` (subroutine `heatr`, lines 47-415).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! **HEATR as NJOY runs it**: an ENDF tape and a PENDF tape in, the PENDF
//! with the heating MTs added out.
//!
//! This is a translation of the whole of `heatr.f90`, one function per
//! upstream routine (GitHub #535; audit in
//! `verification_and_validation/heatr_upstream_audit.md`):
//!
//! | upstream | here |
//! |---|---|
//! | `heatr`, `horder` | [`heatr`], [`HeatrInput::from_cards`], `state::horder` |
//! | `hinit` | `hinit.rs` |
//! | `nheat`, `indx` | `nheat.rs`, `state::indx` |
//! | `disbar`, `capdam`, `df` | `disbar.rs`, `state.rs` |
//! | `conbar`, `hgtyld`, `anabar`, `anadam`, `sed`, `tabbar`, `tabdam` | `conbar.rs` |
//! | `sixbar` | `sixbar.rs` |
//! | `getsix`, `tabsq6`, `hgam102` | `getsix.rs` |
//! | `h6cm`, `h6ddx`, `h6dis`, `bacha`, `h6psp` | `h6.rs` |
//! | `hgtfle`, `hgetco` | `hgtfle.rs` |
//! | `hconvr` | `hconvr.rs` |
//! | `gheat`, `gambar`, `tabsqr`, `disgam` | `gheat.rs` |
//! | `hout` | `hout.rs` |
//!
//! Upstream's scratch tapes are in memory: `iold`/`inew` (the kerma table)
//! is one table updated in place, `nscr`/`nend4` is the material after
//! `hconvr`, and `nend6` is the MF=6 sections read once. Each routine's
//! Fortran `save` state is an explicit struct (`state.rs`).
//!
//! **Status.** Translated in full on 2026-10-05; ~~it has **not yet been
//! compared with NJOY2016's output**, which is the next step. Until then this
//! is untrusted draft, and `crate::heatr::Kerma` remains what the ACE route
//! uses.~~ **Compared the same day:** the output tape is byte-identical to
//! NJOY2016's HEATR on all 62 neutron evaluations in `reference-data/endf/`
//! at `local = 0` and at `local = 1, iprint = 2`, and on the 8 decks of
//! `tests/heatr_driver_vs_njoy2016.rs` (plot file and listing included);
//! `verification_and_validation/heatr_vs_njoy2016.md` §6. The ACE route
//! (`acer`, `interface`) now uses [`heatr_kerma`]. Still AI-assisted draft
//! pending human review, per `RESPONSIBLE_USE.md`.

mod conbar;
mod disbar;
mod flat;
mod getsix;
mod gheat;
mod h6;
mod hconvr;
mod hgtfle;
mod hinit;
mod hout;
mod nheat;
mod sixbar;
mod state;

use crate::common::phys::{AMASSN_AMU, AMU_G, CLIGHT_CM_S, EV_ERG};
use crate::endf::tape::{Section, Tape};
use crate::leapr::CardCursor;
use crate::NjoyError;
use state::{horder, Heatr, NQAMAX};

/// HEATR's input cards 2-5 (card 1's units are the caller's tapes).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct HeatrInput {
    /// `matd`: the material to process.
    pub matd: i32,
    /// `mtk`: the partial kermas wanted besides MT=301 (card 3): `MT + 300`
    /// for a reaction, or 303 (non-elastic), 304 (inelastic), 318
    /// (fission), 401 (disappearance), 442 (photon energy), 443 (kinematic
    /// limit), 444-447 (damage energy: total, elastic, inelastic,
    /// disappearance).
    pub partial_kermas: Vec<i32>,
    /// `(mta, qa)`: user Q values (cards 4-5) \[eV\]. A `qa >= 99e6` takes
    /// its energy-dependent Q from the matching entry of `qbar`.
    pub user_q: Vec<(i32, f64)>,
    /// Card 5a: for each `qa >= 99e6`, in order, a TAB1 of Q against
    /// incident energy, flat as NJOY reads it (`C1 C2 L1 L2 NR NP NBT INT x y …`).
    pub qbar: Vec<Vec<f64>>,
    /// `ntemp`: temperatures to process, 0 for all on the PENDF.
    pub ntemp: usize,
    /// `local`: `false` transports the photons (energy balance), `true`
    /// deposits their energy locally.
    pub local: bool,
    /// `iprint`: 0 minimal, 1 more, 2 also the kinematic-limit check.
    pub iprint: i32,
    /// `ed`: the displacement energy for damage \[eV\], 0 for the built-in
    /// table.
    pub ed: f64,
    /// `nplot != 0`: also produce the `viewr` energy-balance plot.
    pub plot: bool,
    /// Card 1's units, as the listing echoes them (the tapes themselves are
    /// [`heatr`]'s arguments).
    pub units: HeatrUnits,
}

/// Card 1's units.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct HeatrUnits {
    /// `nendf`.
    pub nendf: i32,
    /// `nin`.
    pub nin: i32,
    /// `nout`.
    pub nout: i32,
    /// `nplot`.
    pub nplot: i32,
}

impl HeatrInput {
    /// Read HEATR's cards from the text that follows the `heatr` module card
    /// of an NJOY deck, with Fortran list-directed semantics (each card on a
    /// fresh record, `/` leaving the defaults).
    ///
    /// # Errors
    /// [`NjoyError::EndfParse`] on a malformed card, too many partial kermas
    /// (`npk + 3 > 28`, or `3·npk + 7` with `iprint = 2`) or Q values (> 30).
    pub fn from_cards(text: &str) -> Result<(HeatrUnits, HeatrInput), NjoyError> {
        let mut cur = CardCursor::new(text);
        let c1 = cur.read_reals(4)?;
        let iv = |v: &[Option<f64>], i: usize| v.get(i).copied().flatten().unwrap_or(0.0);
        let units = HeatrUnits {
            nendf: iv(&c1, 0) as i32,
            nin: iv(&c1, 1) as i32,
            nout: iv(&c1, 2) as i32,
            nplot: iv(&c1, 3) as i32,
        };
        let c2 = cur.read_reals(7)?;
        let npk = iv(&c2, 1) as usize;
        let nqa = iv(&c2, 2) as usize;
        let mut input = HeatrInput {
            matd: iv(&c2, 0) as i32,
            ntemp: iv(&c2, 3) as usize,
            local: iv(&c2, 4) != 0.0,
            iprint: iv(&c2, 5) as i32,
            ed: iv(&c2, 6),
            plot: units.nplot != 0,
            units,
            ..HeatrInput::default()
        };
        if npk > 0 {
            input.partial_kermas = cur
                .read_reals_required(npk, "heatr card 3")?
                .into_iter()
                .map(|v| v as i32)
                .collect();
        }
        if nqa > 0 {
            let mta = cur.read_reals_required(nqa, "heatr card 4")?;
            let qa = cur.read_reals_required(nqa, "heatr card 5")?;
            for (m, q) in mta.into_iter().zip(qa) {
                input.user_q.push((m as i32, q));
                if q >= 99.0e6 {
                    let mut vals: Vec<f64> = cur.read_reals(1000)?.into_iter().map_while(|v| v).collect();
                    // `nz0` (`heatr.f90:178-181`): everything up to the last
                    // value above the flag `-1e-9`, so a table whose last
                    // value is negative loses it, as upstream's does.
                    let nz0 = vals.iter().rposition(|&v| v > -1.0e-9).map_or(0, |p| p + 1);
                    vals.truncate(nz0);
                    input.qbar.push(vals);
                }
            }
        }
        Ok((units, input))
    }
}

/// What a HEATR run produces.
#[derive(Debug)]
pub struct HeatrOutput {
    /// The output PENDF: `matd` at each temperature processed, with
    /// MF=3/MT=301 and the partial MTs added and the directory revised.
    pub tape: Tape,
    /// Upstream's listing (`nsyso`): messages, and with `iprint >= 1` the
    /// per-reaction tables.
    pub listing: String,
    /// The `viewr` plot file, when [`HeatrInput::plot`] is set.
    pub plot: Option<String>,
}

/// Run HEATR (`heatr.f90`'s `heatr`) on material `input.matd` of `endf`, using
/// the pointwise cross sections of `pendf` (a RECONR/BROADR tape).
///
/// # Errors
/// Upstream's fatal conditions as [`NjoyError`]: too many partial kermas or Q
/// values, a missing section a routine needs, an MF=4 or MF=15 request above
/// the last tabulated energy, malformed records, and the paths upstream does
/// not code either (`hgetco`'s lab-to-CM conversion, LO=2 in `gheat`).
pub fn heatr(endf: &Tape, pendf: &Tape, input: &HeatrInput) -> Result<HeatrOutput, NjoyError> {
    const NPKMAX: usize = 28;
    const QTEST: f64 = 99.0e6;
    let t0 = std::time::Instant::now();
    let secs = |t0: std::time::Instant| t0.elapsed().as_secs_f64();
    let mut h = Heatr::new(Tape::from_sections(String::new(), Vec::new()));
    h.listing.push_str(&format!("\n heatr...prompt kerma{:48}{:8.1}s\n", "", secs(t0)));
    h.matd = input.matd;
    let mut npk = input.partial_kermas.len();
    h.nqa = input.user_q.len();
    h.local = i32::from(input.local);
    h.iprint = input.iprint;
    h.brk = input.ed;
    h.kchk = 0;
    if h.iprint == 2 {
        h.kchk = 1;
        h.iprint = 1;
    }
    let npkk = if h.kchk == 1 { 3 * npk + 7 } else { npk + 3 };
    if npkk > NPKMAX {
        return Err(NjoyError::EndfParse(
            "heatr: requested too many kerma mt-s (6+mt301 allowed).".into(),
        ));
    }
    if h.nqa > NQAMAX {
        return Err(NjoyError::EndfParse(
            "heatr: requested too many q values.".into(),
        ));
    }
    // Card 5a: energy-dependent Q, concatenated 1-based.
    let mut qbar = vec![0.0];
    let mut qtabs = input.qbar.iter();
    for (i, &(m, q)) in input.user_q.iter().enumerate() {
        h.mta[i + 1] = m;
        h.qa[i + 1] = q;
        h.lqs[i + 1] = 0;
        if q >= QTEST {
            h.lqs[i + 1] = qbar.len();
            if let Some(t) = qtabs.next() {
                qbar.extend_from_slice(t);
            }
        }
    }
    h.qbar = qbar;

    // The partial kermas.
    let mut mtk: Vec<i32> = input.partial_kermas.clone();
    if mtk.iter().any(|&m| m == 443) && h.kchk == 0 {
        h.kchk = 2;
    }
    if let Some(pos) = mtk.iter().position(|&m| m == 301) {
        h.mess(
            "heatr",
            "mt301 always calculated",
            "--you do not need to ask for it.",
        );
        mtk.remove(pos);
        npk -= 1;
    }
    let mut mtp = vec![0i32; NPKMAX + 3];
    for (i, &m) in mtk.iter().enumerate() {
        mtp[npk + 3 - (i + 1)] = m;
    }
    mtp[1] = 0;
    mtp[2] = 301;
    npk += 2;
    if npk > 3 {
        horder(&mut mtp, npk);
    }
    h.mtp = mtp;
    h.npk = npk;

    // The input echo (`heatr.f90:251-300`).
    let u = input.units;
    let mut l = String::new();
    l.push_str(&format!("\n input endf unit ...................... {:10}\n", u.nendf));
    l.push_str(&format!(" input pendf unit ..................... {:10}\n", u.nin));
    l.push_str(&format!(" output pendf unit .................... {:10}\n", u.nout));
    l.push_str(&format!(" mat to be processed .................. {:10}\n", input.matd));
    l.push_str(&format!(" no. temperatures (0=all) ............. {:10}\n", input.ntemp));
    l.push_str(&format!(" gamma heat (0 nonlocal, 1 local) ..... {:10}\n", h.local));
    l.push_str(&format!(" print option (0 min, 1 more, 2 chk) .. {:10}\n", h.iprint));
    if h.brk == 0.0 {
        l.push_str(" damage displacement energy ...........    default\n");
    } else {
        l.push_str(&format!(" damage displacement energy ........... {:10.1} ev\n", h.brk));
    }
    if npk >= 3 {
        l.push_str(&format!(" partial kerma mt-s desired ........... {:10}\n", h.mtp[3]));
        for i in 4..=npk {
            l.push_str(&format!("{:40}{:10}\n", "", h.mtp[i]));
        }
    }
    if h.nqa != 0 {
        l.push_str(&format!(" auxiliary reactions .................. {:10}\n", h.mta[1]));
        for i in 2..=h.nqa {
            l.push_str(&format!("{:40}{:10}\n", "", h.mta[i]));
        }
        l.push_str(&format!(" auxiliary q values ................... {}\n", crate::acer::fortran_fmt::fortran_e(h.qa[1], 4, 12)));
        for i in 2..=h.nqa {
            l.push_str(&format!("{:40}{}\n", "", crate::acer::fortran_fmt::fortran_e(h.qa[i], 4, 12)));
        }
        // `:286-297`: upstream writes a heading per energy-dependent Q and
        // hands the table to `tab1io` on the listing unit, which writes no
        // lines there.
        if h.qbar.len() > 1 {
            for i in 1..=h.nqa {
                if h.qa[i] >= QTEST {
                    l.push_str(" input q ----\n");
                }
            }
        }
    }
    h.listing.push_str(&l);
    let ntemp = if input.ntemp == 0 { 100 } else { input.ntemp };

    // The temperatures of this material on the PENDF.
    let mut blocks: Vec<Vec<Section>> = Vec::new();
    for s in pendf.sections().iter().filter(|s| s.key.mat == input.matd) {
        if s.key.mf == 1 && s.key.mt == 451 {
            blocks.push(Vec::new());
        }
        if let Some(b) = blocks.last_mut() {
            b.push(s.clone());
        }
    }
    if blocks.is_empty() {
        return Err(NjoyError::SectionNotFound {
            mat: input.matd,
            mf: 1,
            mt: 451,
        });
    }
    let mut out: Vec<Section> = Vec::new();
    let mut plot = None;
    for block in blocks.iter().take(ntemp) {
        let head = block[0].rows[0];
        h.za = head[0];
        h.awr = head[1];
        h.emc2 = AMASSN_AMU * AMU_G * CLIGHT_CM_S * CLIGHT_CM_S / EV_ERG;
        h.tm = h.emc2 * (h.awr + 1.0);
        h.rtm = 1.0 / h.tm;
        if h.brk == 0.0 {
            let iz = (h.za / 1000.0).round() as u32;
            h.brk = crate::heatr::default_displacement_energy(iz);
            h.listing
                .push_str(&format!("\n default damage energy ={:5.1} ev\n", h.brk));
        }
        h.hinit(endf, block)?;
        let itemp = out.iter().filter(|s| s.key.mf == 1 && s.key.mt == 451).count() + 1;
        h.listing.push_str(&format!("{:61}temp{itemp:2}  {:8.1}s\n", "", secs(t0)));
        h.nheat(endf, block)?;
        if h.mgam > 0 && h.mgam != 10 && h.local == 0 {
            h.listing.push_str(&format!("{:61}temp{itemp:2}  {:8.1}s\n", "", secs(t0)));
            h.gheat(endf, block)?;
        }
        let (secs, p) = h.hout(block, input.plot)?;
        out.extend(secs);
        if p.is_some() {
            plot = p;
        }
    }
    h.listing.push_str(&format!("\n{:69}{:8.1}s\n {}*******\n", "", secs(t0), "**********".repeat(7)));
    let mut tape = Tape::from_sections(pendf.tpid.clone(), out);
    tape.copy_raw_mf32_from(pendf);
    Ok(HeatrOutput {
        tape,
        listing: h.listing,
        plot,
    })
}

/// A PENDF tape for [`heatr`] from a RECONR (or BROADR) result for material
/// `mat`: MF=1/MT=451
/// and every reconstructed MF=3 section, with the TAB1 heads HEATR reads
/// (`QM`, `QI`, `LR`).
///
/// RECONR copies each reaction's `QM` from the evaluation; `ReconrResult`
/// does not carry it, so it is taken from `endf`'s MF=3 head for the same MT
/// (0 for a section RECONR builds itself, as RECONR writes for its sums).
/// The values stay at full precision, as on RECONR's binary tape.
pub fn pendf_for_heatr(endf: &Tape, mat: i32, recon: &crate::reconr::ReconrResult) -> Tape {
    use crate::endf::tape::Section;
    use crate::endf::EndfKey;
    let (za, awr) = (recon.material.za, recon.material.awr);
    let iverf = endf
        .section(mat, 1, 451)
        .map_or(6, |s| crate::moder::layout::iverf_from_mf1(&s.rows));
    let header = Tape::pendf_from_pointwise(mat, iverf, za, awr, 0.0, std::iter::empty());
    let mut secs: Vec<Section> = header.sections().to_vec();
    for sec in &recon.sections {
        let mt = sec.mt.number();
        let qm = endf
            .section(mat, 3, mt)
            .and_then(|s| s.rows.get(1).map(|r| r[0]))
            .unwrap_or(0.0);
        let np = sec.pairs.len() as f64;
        let mut flat = vec![qm, sec.qi, 0.0, f64::from(sec.lr), 1.0, np, np, 2.0];
        for &(x, y) in &sec.pairs {
            flat.push(x);
            flat.push(y);
        }
        let mut rows = vec![[za, awr, 0.0, 0.0, 0.0, 0.0]];
        rows.extend(flat::rows_of_tab1(&flat));
        secs.push(Section { key: EndfKey { mat, mf: 3, mt }, rows });
    }
    Tape::from_sections(String::new(), secs)
}

/// MT=301 as an ACE build needs it: HEATR (this translation) run on
/// [`pendf_for_heatr`]`(endf, mat, recon)` with photons transported
/// (`local = 0`), and the MT=301 TAB1 of its output returned as a
/// [`crate::heatr::Kerma`] (energies and values exactly as HEATR wrote them).
///
/// # Errors
/// Whatever [`heatr`] returns, or [`NjoyError::SectionNotFound`] if its tape
/// has no MT=301 (it always writes one).
pub fn heatr_kerma(endf: &Tape, mat: i32, recon: &crate::reconr::ReconrResult) -> Result<crate::heatr::Kerma, NjoyError> {
    let pendf = pendf_for_heatr(endf, mat, recon);
    let input = HeatrInput { matd: mat, ..HeatrInput::default() };
    let out = heatr(endf, &pendf, &input)?;
    let s301 = out.tape.section(mat, 3, 301).ok_or(NjoyError::SectionNotFound { mat, mf: 3, mt: 301 })?;
    let mut cur = crate::endf::records::SectionCursor::new(&s301.rows);
    cur.read_cont()?;
    let t = cur.read_tab1()?;
    Ok(crate::heatr::Kerma { energy: t.pairs.iter().map(|p| p.0).collect(), h: t.pairs.iter().map(|p| p.1).collect() })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cards_parse_as_upstream_reads_them() {
        let (u, i) =
            HeatrInput::from_cards("-21 -22 -23 0 /\n2637 5 0 0 0 1 /\n442 443 444 445 446 /\n")
                .unwrap();
        assert_eq!(
            u,
            HeatrUnits {
                nendf: -21,
                nin: -22,
                nout: -23,
                nplot: 0
            }
        );
        assert_eq!(i.matd, 2637);
        assert_eq!(i.partial_kermas, vec![442, 443, 444, 445, 446]);
        assert!(!i.local);
        assert_eq!(i.iprint, 1);
        assert_eq!(i.ed, 0.0);
    }
}
