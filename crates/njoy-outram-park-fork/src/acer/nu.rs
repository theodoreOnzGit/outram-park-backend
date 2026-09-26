//! The ACE **NU** block: fission ν̄(E), prompt and total.
//!
//! Ports the ν̄ portion of `acelod` in NJOY2016 `acefc.f90` (the `nut`/`nup`
//! assembly at ~5141-5277 and the `xss` layout at ~5755-5770).
//!
//! # Why this exists separately from [`crate::nuclear_data::NuBar`]
//!
//! [`NuBar`](crate::nuclear_data::NuBar) parses the same ENDF sections but
//! **flattens them to a lin-lin table**: an `LNU=1` polynomial is resampled
//! onto a 60-point log grid, and the interpolation ranges are discarded. That
//! is right for its consumer (`outram-mc-libs` interpolates it directly) and
//! wrong here, because an ACE table must carry the evaluation's *own*
//! representation — an ACE reader applies the `NR`/`NBT`/`INT` ranges itself.
//! Writing a resampled table would change the numbers a downstream code sees.
//! The same split already exists for MF=5 (`acer/energy.rs` has its own
//! narrower parser); this follows it rather than widening `NuBar`.
//!
//! # Block layout (`acefc.f90` ~5755)
//!
//! ```text
//! JXS(2) ─→ [ -NNUP ] [ prompt block, NNUP values ] [ total block ]
//! ```
//!
//! The leading negative count is present **only when a prompt section
//! (MF=1/MT=456) exists**. With no prompt section the total block starts
//! immediately and there is no prefix — a reader distinguishes the two by the
//! sign of the first value, so emitting a spurious `-0` would corrupt it.
//!
//! Each block is one of:
//!
//! ```text
//! LNU=1:  [1] [NC] [C_1 … C_NC]
//! LNU=2:  [2] [NR] [NBT_1 … NBT_NR] [INT_1 … INT_NR] [NE] [E_1 … E_NE] [nu_1 … nu_NE]
//! ```
//!
//! with `NR = 0` when the whole table is a single lin-lin range, which is the
//! common case and what NJOY writes for U-234/U-235/U-238 ENDF/B-VIII.0.
//!
//! # Units
//!
//! Energies are written in **MeV**, rounded to 7 significant figures exactly as
//! upstream does (`sigfig(scr(...)/emev,7,0)`). The ν̄ values themselves are
//! **not** rounded — upstream stores `scr(...)` unmodified, and rounding them
//! would be a gratuitous difference.

use crate::endf::records::SectionCursor;
use crate::endf::tape::Tape;
use crate::NjoyError;

use super::build::sigfig;

/// eV → MeV (NJOY `emev`).
const EMEV: f64 = 1.0e6;

/// One ν̄ representation, in the evaluation's own form.
#[derive(Debug, Clone, PartialEq)]
pub enum NuRepr {
    /// `LNU=1` — ν̄(E) = Σ Cₖ E^(k-1), coefficients already rescaled for E in MeV.
    Polynomial(Vec<f64>),
    /// `LNU=2` — tabulated ν̄(E), energies in MeV.
    Tabulated {
        /// Interpolation range boundaries; empty means a single lin-lin range.
        nbt: Vec<f64>,
        /// Interpolation laws, aligned with `nbt`.
        int: Vec<f64>,
        /// Incident energies \[MeV\], ascending.
        e_mev: Vec<f64>,
        /// ν̄ aligned with `e_mev`.
        nu: Vec<f64>,
    },
}

impl NuRepr {
    /// Parse one ν̄ section: `mt` is 452 (total), 455 (delayed) or 456 (prompt).
    ///
    /// Returns `Ok(None)` when the material has no such section — a non-fissile
    /// nuclide, or a fissile one that gives only the total.
    pub fn from_endf(tape: &Tape, mat: i32, mt: i32) -> Result<Option<Self>, NjoyError> {
        let Some(sec) = tape.section(mat, 1, mt) else {
            return Ok(None);
        };
        let mut cur = SectionCursor::new(&sec.rows);
        let head = cur.read_cont()?; // HEAD: L2 = LNU
        match head.l2 {
            1 => {
                let list = cur.read_list()?;
                // ENDF gives Cₖ for E in eV; ACE wants E in MeV, so
                // Cₖ' = Cₖ · (1e6)^(k-1).
                //
                // DIVERGENCE FROM UPSTREAM, stated rather than hidden:
                // `acefc.f90` ~5158 writes `nut(i+2)*emev*(i-1)` — emev TIMES
                // (i-1), not emev RAISED TO (i-1). The two agree for the first
                // two coefficients (k=1 unscaled, k=2 ×1e6) and diverge from
                // k=3 on (1e12 here against 2e6 there). This port uses the
                // power, which is what the unit change requires.
                //
                // NOT VERIFIED EITHER WAY: no tape in `reference-data/` uses
                // LNU=1 — U-234, U-235 and U-238 are all LNU=2 for MT=452/455/
                // 456 (checked 2026-09-20) — so this branch is unexercised by
                // the parity gate and must not be described as matching NJOY.
                let coeffs = list
                    .data
                    .iter()
                    .enumerate()
                    .map(|(k, &c)| c * EMEV.powi(k as i32))
                    .collect();
                Ok(Some(NuRepr::Polynomial(coeffs)))
            }
            2 => {
                // MT=455 carries its precursor decay constants as a LIST BEFORE
                // the TAB1 (`acefc.f90:5194-5199`: `if (mta.eq.455) call listio`).
                // Reading the TAB1 straight after the HEAD, as this did for every
                // MT, parsed that LIST as the table -- found 2026-09-26 when the
                // delayed-neutron blocks were first compared against NJOY's
                // (U-234's DNU came out 15 words against NJOY's 11). The NU block
                // itself uses only 452/456, which is why nothing had noticed.
                if mt == 455 {
                    let _decay_constants = cur.read_list()?;
                }
                let tab1 = cur.read_tab1()?;
                // A single lin-lin range is written as NR=0, which is both what
                // upstream does (`if (m.ne.1.or.jnt.ne.2)`) and what every
                // reader expects as the default.
                let single_linlin =
                    tab1.interp.len() == 1 && tab1.interp.first().map(|&(_, i)| i) == Some(2);
                let (nbt, int) = if single_linlin {
                    (Vec::new(), Vec::new())
                } else {
                    (
                        tab1.interp.iter().map(|&(b, _)| b as f64).collect(),
                        tab1.interp.iter().map(|&(_, i)| i as f64).collect(),
                    )
                };
                let e_mev = tab1
                    .pairs
                    .iter()
                    .map(|&(e, _)| sigfig(e / EMEV, 7))
                    .collect();
                let nu = tab1.pairs.iter().map(|&(_, n)| n).collect();
                Ok(Some(NuRepr::Tabulated {
                    nbt,
                    int,
                    e_mev,
                    nu,
                }))
            }
            _ => Ok(None),
        }
    }

    /// This representation as the XSS values upstream calls `nut`/`nup`.
    pub fn xss(&self) -> Vec<f64> {
        match self {
            NuRepr::Polynomial(c) => {
                let mut v = Vec::with_capacity(2 + c.len());
                v.push(1.0);
                v.push(c.len() as f64);
                v.extend_from_slice(c);
                v
            }
            NuRepr::Tabulated {
                nbt,
                int,
                e_mev,
                nu,
            } => {
                let mut v = Vec::with_capacity(3 + 2 * nbt.len() + 2 * e_mev.len());
                v.push(2.0);
                v.push(nbt.len() as f64);
                v.extend_from_slice(nbt);
                v.extend_from_slice(int);
                v.push(e_mev.len() as f64);
                v.extend_from_slice(e_mev);
                v.extend_from_slice(nu);
                v
            }
        }
    }
}

/// The complete NU block for one material, ready to splice into XSS.
///
/// `None` from [`build`] means the material is not fissile (no MF=1/MT=452),
/// in which case `JXS(2)` stays 0 — which is correct, not a gap.
pub fn build(tape: &Tape, mat: i32) -> Result<Option<Vec<f64>>, NjoyError> {
    let Some(total) = NuRepr::from_endf(tape, mat, 452)? else {
        return Ok(None);
    };
    let prompt = NuRepr::from_endf(tape, mat, 456)?;

    let total_xss = total.xss();
    let mut out = Vec::new();
    if let Some(p) = prompt {
        let p_xss = p.xss();
        // The prefix is the NEGATED prompt length; a reader keys off the sign,
        // so this is written only when a prompt block actually follows.
        out.push(-(p_xss.len() as f64));
        out.extend(p_xss);
    }
    out.extend(total_xss);
    Ok(Some(out))
}
