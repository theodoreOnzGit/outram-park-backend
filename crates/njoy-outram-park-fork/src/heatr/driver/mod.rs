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
//! **Status.** Translated in full on 2026-10-05; it has **not yet been
//! compared with NJOY2016's output**, which is the next step. Until then this
//! is untrusted draft, and `crate::heatr::Kerma` remains what the ACE route
//! uses.

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
                    let vals: Vec<f64> = cur
                        .read_reals(1000)?
                        .into_iter()
                        .map_while(|v| v)
                        .filter(|&v| v > -1.0e-9)
                        .collect();
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
    let mut h = Heatr::new(Tape::from_sections(String::new(), Vec::new()));
    h.listing.push_str("\n heatr...prompt kerma\n");
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
        h.nheat(endf, block)?;
        if h.mgam > 0 && h.mgam != 10 && h.local == 0 {
            h.gheat(endf, block)?;
        }
        let (secs, p) = h.hout(block, input.plot)?;
        out.extend(secs);
        if p.is_some() {
            plot = p;
        }
    }
    let mut tape = Tape::from_sections(pendf.tpid.clone(), out);
    tape.copy_raw_mf32_from(pendf);
    Ok(HeatrOutput {
        tape,
        listing: h.listing,
        plot,
    })
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
