// Translated from NJOY2016.80 (commit ac5adf5) `src/matxsr.f90`: the cards of
// `matxsr` (`:127-189`) and `ruinm` (`:795-906`).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under AGPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! MATXSR's cards.

use crate::deck::cards::{CardReader, Slot};
use crate::NjoyError;

/// MATXSR's cards.
#[derive(Debug, Clone, PartialEq)]
pub struct MatxsrCards {
    /// `ngen1 ngen2 nmatx ngen3 ... ngen8`, in card order.
    pub units: [i32; 9],
    /// `ivers`.
    pub ivers: i32,
    /// `tuse(1:2)`, `word` read `(a8,a8)`.
    pub tuse: [String; 2],
    /// `npart ntype nholl nmat`.
    pub control: [i32; 4],
    /// The `nholl` identification cards, 72 columns each.
    pub holl: Vec<String>,
    /// `hprt` (`character(8)`).
    pub hprt: Vec<String>,
    /// `ngrp`.
    pub ngrp: Vec<i32>,
    /// `htype` (`character(8)`).
    pub htype: Vec<String>,
    /// `jinp`.
    pub jinp: Vec<i32>,
    /// `joutp`.
    pub joutp: Vec<i32>,
    /// `(hmatn, matno, matgg)`, `matgg` defaulted to `100*(matno/100)`.
    pub mats: Vec<(String, i32, i32)>,
}

fn strings(r: &mut CardReader, n: usize, len: usize) -> Result<Vec<String>, NjoyError> {
    let mut s: Vec<Slot> = (0..n).map(|_| Slot::string(len, " ")).collect();
    r.read(&mut s)?;
    Ok(s.iter().map(|x| format!("{:<w$.w$}", x.text(), w = len)).collect())
}

fn count(n: i32) -> Result<usize, NjoyError> {
    usize::try_from(n).map_err(|_| NjoyError::InvalidInput(format!("matxsr: negative count {n}")))
}

/// Read MATXSR's cards.
///
/// # Errors
/// A malformed card.
pub fn read_matxsr(r: &mut CardReader) -> Result<MatxsrCards, NjoyError> {
    let u = r.read_ints(&[0; 9])?;
    let mut c2 = [Slot::Int(0), Slot::string(16, " ")];
    r.read(&mut c2)?;
    let word = format!("{:<16.16}", c2[1].text());
    let tuse = [word[..8].to_string(), word[8..].to_string()];
    let c = r.read_ints(&[0, 0, 0, 0])?;
    let (npart, ntype, nholl, nmat) = (count(c[0])?, count(c[1])?, count(c[2])?, count(c[3])?);
    let mut holl = Vec::with_capacity(nholl);
    for _ in 0..nholl {
        holl.push(strings(r, 1, 72)?.remove(0));
    }
    let hprt = strings(r, npart, 8)?;
    let ngrp = r.read_ints(&vec![0; npart])?;
    let htype = strings(r, ntype, 8)?;
    let jinp = r.read_ints(&vec![0; ntype])?;
    let joutp = r.read_ints(&vec![0; ntype])?;
    let mut mats = Vec::with_capacity(nmat);
    for _ in 0..nmat {
        let mut s = [Slot::string(8, " "), Slot::Int(0), Slot::Int(0)];
        r.read(&mut s)?;
        let matno = s[1].int();
        let mut matgg = s[2].int();
        if matgg == 0 {
            matgg = 100 * (matno / 100);
        }
        mats.push((format!("{:<8.8}", s[0].text()), matno, matgg));
    }
    let mut units = [0i32; 9];
    units.copy_from_slice(&u);
    Ok(MatxsrCards { units, ivers: c2[0].int(), tuse, control: [c[0], c[1], c[2], c[3]], holl, hprt, ngrp, htype, jinp, joutp, mats })
}
