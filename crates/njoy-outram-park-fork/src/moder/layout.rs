//! The **record layout** of an ENDF section, as NJOY's MODER walks it: which
//! of a section's lines are CONT heads, data, interpolation tables, text or
//! directory entries. [`crate::endf::tape::Tape::write`] needs it to format
//! each line as NJOY does, because a [`crate::endf::tape::Section`] keeps
//! only six numbers per line and not the record each line belongs to.
//!
//! Ported from `moder.f90`'s `file1` … `file40` (`:415-1714`): the same
//! sequence of `contio` / `listio` / `tab1io` / `tab2io` / `hdatio` /
//! `dictio` calls, driven by the same counts read from the same heads. Only
//! the bookkeeping is kept; each call records the kind of every line it
//! would write instead of writing it. The line formats themselves are
//! `endf.f90`'s:
//!
//! | record | line format |
//! |---|---|
//! | CONT (and every record's head) | `2a11, 4i11` (`contio`, `:105`) |
//! | LIST / TAB1 data | `6a11`, fields past the count blank (`lineio`, `:850-860`) |
//! | TAB1 / TAB2 interpolation table | `2i11`, `4i11` or `6i11`, rest blank (`tablio`) |
//! | MF=1/MT=451 text | 66 Hollerith columns (`hdatio` → `hollio`) |
//! | MF=1/MT=451 directory | `22x, 4i11` (`dictio`, `:701`) |
//!
//! **Not ported:** MF=32 (`file32`, resonance-parameter covariances, whose
//! compact `LCOMP = 2` rows are integer-packed text), and GENDF and ERRORR
//! materials ([`is_group_material`]), which MODER copies through a separate
//! path.
//! For those, and for any section whose structure does not walk to its last
//! line, [`section_layout`] returns `None` and the writer falls back to
//! writing every field as `a11`.

/// How one line of a section is formatted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LineKind {
    /// A head or CONT record: `C1, C2` as `a11`, `L1, L2, N1, N2` as `i11`.
    Cont,
    /// `n` (1..=6) data fields as `a11`, the rest blank.
    Data(u8),
    /// `n` (2, 4 or 6) interpolation-table integers as `i11`, the rest blank.
    Ints(u8),
    /// 66 columns of Hollerith text (MF=1/MT=451 description).
    Text,
    /// An MF=1/MT=451 directory entry: 22 blanks, then fields 3-6 as `i11`.
    Dir,
}

/// Walks a section's rows the way one `moder.f90` `fileN` routine reads
/// them, recording each line's kind.
struct Walker<'a> {
    rows: &'a [[f64; 6]],
    pos: usize,
    kinds: Vec<LineKind>,
    iverf: i32,
}

/// Fortran `nint` of a field.
fn ni(x: f64) -> i64 {
    x.round() as i64
}

impl<'a> Walker<'a> {
    fn new(rows: &'a [[f64; 6]], iverf: i32) -> Self {
        Walker { rows, pos: 0, kinds: Vec::with_capacity(rows.len()), iverf }
    }

    /// The row about to be read, or `None` past the end.
    fn peek(&self) -> Option<[f64; 6]> {
        self.rows.get(self.pos).copied()
    }

    /// `contio`: one CONT line; returns `(c1, c2, l1, l2, n1, n2)` as read.
    fn cont(&mut self) -> Option<[f64; 6]> {
        let r = self.peek()?;
        self.kinds.push(LineKind::Cont);
        self.pos += 1;
        Some(r)
    }

    /// `n` numbers laid out six to a line.
    fn data(&mut self, n: i64) -> Option<()> {
        let mut left = n.max(0);
        while left > 0 {
            self.peek()?;
            let k = left.min(6) as u8;
            self.kinds.push(LineKind::Data(k));
            self.pos += 1;
            left -= i64::from(k);
        }
        Some(())
    }

    /// `n` interpolation pairs, three to a line.
    fn ints(&mut self, pairs: i64) -> Option<()> {
        let mut left = pairs.max(0);
        while left > 0 {
            self.peek()?;
            let k = left.min(3) as u8;
            self.kinds.push(LineKind::Ints(2 * k));
            self.pos += 1;
            left -= i64::from(k);
        }
        Some(())
    }

    /// `listio`: head, then `NPL = N1` numbers.
    fn list(&mut self) -> Option<[f64; 6]> {
        let h = self.cont()?;
        self.data(ni(h[4]))?;
        Some(h)
    }

    /// `tab1io`: head, `NR` interpolation pairs, `NP` (x, y) pairs.
    fn tab1(&mut self) -> Option<[f64; 6]> {
        let h = self.cont()?;
        self.ints(ni(h[4]))?;
        self.data(2 * ni(h[5]))?;
        Some(h)
    }

    /// `tab2io`: head and `NR` interpolation pairs.
    fn tab2(&mut self) -> Option<[f64; 6]> {
        let h = self.cont()?;
        self.ints(ni(h[4]))?;
        Some(h)
    }

    /// `hdatio`: a CONT whose `N1` is the number of text lines, then those
    /// lines.
    fn hdat(&mut self) -> Option<[f64; 6]> {
        let h = self.cont()?;
        for _ in 0..ni(h[4]).max(0) {
            self.peek()?;
            self.kinds.push(LineKind::Text);
            self.pos += 1;
        }
        Some(h)
    }

    /// `dictio`: `n` directory lines.
    fn dict(&mut self, n: i64) -> Option<()> {
        for _ in 0..n.max(0) {
            self.peek()?;
            self.kinds.push(LineKind::Dir);
            self.pos += 1;
        }
        Some(())
    }

    /// `listio` repeated while a counter from the previous head is positive
    /// (`lt = l1h; do while (lt > 0) listio; lt = lt - 1`).
    fn lists_by_l1(&mut self, l1: i64) -> Option<()> {
        for _ in 0..l1.max(0) {
            self.list()?;
        }
        Some(())
    }

    // ---- moder.f90's file routines; `h` is the section HEAD -------------

    fn file1(&mut self, mt: i32, h: [f64; 6]) -> Option<()> {
        match mt {
            451 => {
                if self.iverf >= 5 {
                    self.cont()?;
                }
                if self.iverf >= 6 {
                    self.cont()?;
                }
                let hd = self.hdat()?;
                let nx = if self.iverf >= 5 { ni(hd[5]) } else { ni(h[5]) };
                self.dict(nx)?;
            }
            452 | 456 => {
                if ni(h[3]) != 2 {
                    self.list()?;
                } else {
                    self.tab1()?;
                }
            }
            453 => {
                for _ in 0..ni(h[4]) {
                    let l = self.list()?;
                    for _ in 0..ni(l[5]) {
                        self.list()?;
                    }
                }
            }
            454 => {
                for _ in 0..ni(h[2]) {
                    self.list()?;
                }
            }
            455 => {
                let lnd = ni(h[3]);
                self.list()?;
                if lnd != 2 {
                    self.list()?;
                } else {
                    self.tab1()?;
                }
            }
            457 => {
                let ns457 = ni(h[5]);
                self.list()?;
                self.list()?;
                for _ in 0..ns457 {
                    self.list()?;
                }
            }
            458 => {
                let (lfc, nfc) = (ni(h[3]), ni(h[5]));
                self.list()?;
                if lfc == 1 {
                    for _ in 0..nfc {
                        self.tab1()?;
                    }
                }
            }
            460 => match ni(h[2]) {
                1 => {
                    for _ in 0..ni(h[4]) {
                        self.tab1()?;
                    }
                }
                2 => {
                    self.list()?;
                }
                _ => return None,
            },
            _ => return None,
        }
        Some(())
    }

    /// `file2a` (MT=151) and `file2b` (MT=152/153).
    fn file2(&mut self, mt: i32, h: [f64; 6]) -> Option<()> {
        if mt == 152 || mt == 153 {
            self.list()?;
            return Some(());
        }
        if mt != 151 {
            return None;
        }
        for _ in 0..ni(h[4]) {
            let iso = self.cont()?;
            let (ner, lfw) = (ni(iso[4]), ni(iso[3]));
            for _ in 0..ner {
                let rng = self.cont()?;
                let (lru, lrf, nro) = (ni(rng[2]), ni(rng[3]), ni(rng[4]));
                if nro != 0 {
                    self.tab1()?;
                }
                if lru == 2 && lrf == 1 && lfw == 1 {
                    let l = self.list()?;
                    for _ in 0..ni(l[5]) {
                        let c = self.cont()?;
                        for _ in 0..ni(c[4]) {
                            self.list()?;
                        }
                    }
                } else {
                    let c = self.cont()?;
                    if lrf != 7 {
                        for _ in 0..ni(c[4]) {
                            if lru == 2 && lrf == 2 {
                                let c2 = self.cont()?;
                                for _ in 0..ni(c2[4]) {
                                    self.list()?;
                                }
                            } else {
                                self.list()?;
                                if (lru == 1 && lrf == 4) || (lru == 2 && lfw != 0) {
                                    let c2 = self.cont()?;
                                    for _ in 0..ni(c2[4]) {
                                        self.list()?;
                                    }
                                }
                            }
                        }
                    } else {
                        let njs = ni(c[4]);
                        self.list()?;
                        for _ in 0..njs {
                            let l1 = self.list()?;
                            let (kbk, kps) = (ni(l1[2]), ni(l1[3]));
                            self.list()?;
                            for _ in 0..kbk.max(0) {
                                let b = self.cont()?;
                                match ni(b[3]) {
                                    1 => {
                                        self.tab1()?;
                                        self.tab1()?;
                                    }
                                    2 | 3 => {
                                        self.list()?;
                                    }
                                    _ => {}
                                }
                            }
                            if kps == 1 {
                                let l = self.list()?;
                                if ni(l[4]) == 1 {
                                    self.tab1()?;
                                }
                            }
                        }
                    }
                }
            }
        }
        Some(())
    }

    fn file3(&mut self) -> Option<()> {
        let t = self.tab1()?;
        self.lists_by_l1(ni(t[2]))
    }

    fn file4(&mut self, h: [f64; 6]) -> Option<()> {
        let (lvt, ltt) = (ni(h[2]), ni(h[3]));
        let nk = ni(h[4]).max(1);
        for _ in 0..nk {
            let mut l1 = ni(h[2]);
            if lvt == 0 {
                l1 = ni(self.cont()?[2]);
            }
            if lvt != 0 || l1 != 1 {
                if lvt == 1 {
                    self.list()?;
                }
                let t2 = self.tab2()?;
                for _ in 0..ni(t2[5]) {
                    let r = if ltt != 2 { self.list()? } else { self.tab1()? };
                    self.lists_by_l1(ni(r[2]))?;
                }
            }
            if ltt >= 3 {
                let t2 = self.tab2()?;
                for _ in 0..ni(t2[5]) {
                    self.tab1()?;
                }
            }
        }
        Some(())
    }

    fn file5(&mut self, h: [f64; 6]) -> Option<()> {
        for _ in 0..ni(h[4]) {
            let t = self.tab1()?;
            match ni(t[3]) {
                1 => {
                    let t2 = self.tab2()?;
                    for _ in 0..ni(t2[5]) {
                        self.tab1()?;
                    }
                }
                5 | 11 => {
                    self.tab1()?;
                    self.tab1()?;
                }
                7 | 9 | 12 => {
                    self.tab1()?;
                }
                10 => {
                    self.list()?;
                }
                3 => {}
                _ => return None,
            }
        }
        Some(())
    }

    fn file6(&mut self, h: [f64; 6]) -> Option<()> {
        for _ in 0..ni(h[4]) {
            let t = self.tab1()?;
            match ni(t[3]) {
                6 => {
                    self.cont()?;
                }
                1 | 2 | 5 => {
                    let t2 = self.tab2()?;
                    for _ in 0..ni(t2[5]) {
                        self.list()?;
                    }
                }
                7 => {
                    let t2 = self.tab2()?;
                    for _ in 0..ni(t2[5]) {
                        let t3 = self.tab2()?;
                        for _ in 0..ni(t3[5]) {
                            self.tab1()?;
                        }
                    }
                }
                0 | 3 | 4 => {}
                law if law < 0 => {}
                _ => return None,
            }
        }
        Some(())
    }

    fn file7(&mut self, mt: i32, h: [f64; 6]) -> Option<()> {
        match mt {
            4 => {
                let l = self.list()?;
                // `b = a(7)`: the first data word of that LIST.
                let first = self.first_data_of_last_list(ni(l[4]));
                let mut it2 = 0i64;
                let mut ia1 = [0i64; 3];
                if self.iverf >= 6 {
                    it2 = ni(l[5]);
                    if (1..=3).contains(&it2) {
                        for (i, slot) in ia1.iter_mut().enumerate().take(it2 as usize) {
                            // `ii = 6*i + 7` (1-based in `a`), i.e. data word 6*i+1.
                            *slot = ni(self.list_word(ni(l[4]), 6 * (i as i64 + 1)));
                        }
                    } else if it2 > 3 {
                        return None;
                    }
                }
                if first.unwrap_or(0.0) == 0.0 {
                    return Some(());
                }
                let t2 = self.tab2()?;
                for _ in 0..ni(t2[5]) {
                    let t = self.tab1()?;
                    self.lists_by_l1(ni(t[2]))?;
                }
                if self.iverf >= 6 {
                    self.tab1()?;
                    for &a in ia1.iter().take(it2.max(0) as usize) {
                        if a == 0 {
                            self.tab1()?;
                        }
                    }
                }
            }
            2 if self.iverf >= 6 => {
                let lthr = ni(h[2]);
                if lthr == 1 || lthr == 3 {
                    let t = self.tab1()?;
                    self.lists_by_l1(ni(t[2]))?;
                }
                if lthr == 2 || lthr == 3 {
                    self.tab1()?;
                }
                if !(1..=3).contains(&lthr) {
                    return None;
                }
            }
            451 => {
                for _ in 0..ni(h[2]) {
                    self.list()?;
                }
            }
            _ => return None,
        }
        Some(())
    }

    /// Data word `k` (0-based) of the LIST just read, whose `NPL` is `npl`.
    fn list_word(&self, npl: i64, k: i64) -> f64 {
        if k >= npl {
            return 0.0;
        }
        let nlines = ((npl + 5) / 6) as usize;
        let start = self.pos - nlines;
        self.rows[start + (k / 6) as usize][(k % 6) as usize]
    }

    fn first_data_of_last_list(&self, npl: i64) -> Option<f64> {
        (npl > 0).then(|| self.list_word(npl, 0))
    }

    fn file8(&mut self, mt: i32, h: [f64; 6]) -> Option<()> {
        match mt {
            454 | 459 => {
                for _ in 0..ni(h[2]) {
                    self.list()?;
                }
            }
            457 => {
                let ns = ni(h[5]);
                self.list()?;
                self.list()?;
                for _ in 0..ns.max(0) {
                    let l = self.list()?;
                    let lcon = ni(l[2]);
                    if lcon != 1 {
                        for _ in 0..ni(l[5]) {
                            self.list()?;
                        }
                    }
                    if lcon != 0 {
                        self.tab1()?;
                    }
                }
            }
            _ => {
                let (ns, no) = (ni(h[4]), ni(h[5]));
                for _ in 0..ns {
                    if no != 1 {
                        self.list()?;
                    } else {
                        self.cont()?;
                    }
                }
            }
        }
        Some(())
    }

    fn file9(&mut self, h: [f64; 6]) -> Option<()> {
        for _ in 0..ni(h[4]) {
            self.tab1()?;
        }
        Some(())
    }

    /// MF=12, 13 and 30.
    fn file1x(&mut self, mf: i32, mt: i32, h: [f64; 6]) -> Option<()> {
        let lo = ni(h[2]);
        if mf == 12 && mt == 460 {
            match lo {
                1 => {
                    for _ in 0..ni(h[4]) + 1 {
                        self.tab1()?;
                    }
                }
                2 => {
                    self.list()?;
                }
                _ => return None,
            }
        } else if lo != 2 {
            let nk = ni(h[4]);
            let nkp = if nk == 1 { nk } else { nk + 1 };
            for _ in 0..nkp {
                self.tab1()?;
            }
        } else {
            self.list()?;
        }
        Some(())
    }

    fn file14(&mut self, h: [f64; 6]) -> Option<()> {
        if ni(h[2]) == 1 {
            return Some(());
        }
        let (ltt, nk, nii) = (ni(h[3]), ni(h[4]), ni(h[5]));
        for i in 1..=nk {
            if i <= nii {
                self.cont()?;
            } else {
                let t2 = self.tab2()?;
                for _ in 0..ni(t2[5]) {
                    if ltt != 2 {
                        self.list()?;
                    } else {
                        self.tab1()?;
                    }
                }
            }
        }
        Some(())
    }

    fn file15(&mut self) -> Option<()> {
        let t = self.tab1()?;
        match ni(t[3]) {
            1 => {
                let t2 = self.tab2()?;
                for _ in 0..ni(t2[5]) {
                    self.tab1()?;
                }
            }
            2 => {
                let t2 = self.tab2()?;
                for _ in 0..ni(t2[3]) {
                    self.list()?;
                }
            }
            _ => return None,
        }
        Some(())
    }

    fn file28(&mut self, h: [f64; 6]) -> Option<()> {
        for _ in 0..ni(h[4]) {
            self.list()?;
        }
        Some(())
    }

    /// MF=31 and 33.
    fn file3x(&mut self, h: [f64; 6]) -> Option<()> {
        let nl = if self.iverf >= 5 { ni(h[5]) } else { ni(h[3]) };
        for _ in 0..nl.max(0) {
            let c = self.cont()?;
            let (nc, nii) = (ni(c[4]), ni(c[5]));
            for _ in 0..nc.max(0) {
                if self.iverf >= 5 {
                    self.cont()?;
                }
                self.list()?;
            }
            for _ in 0..nii.max(0) {
                self.list()?;
            }
        }
        Some(())
    }

    fn file34(&mut self, mt: i32, h: [f64; 6]) -> Option<()> {
        for _ in 0..ni(h[5]) {
            let c = self.cont()?;
            let (mt1, nl, nl1) = (ni(c[3]), ni(c[4]), ni(c[5]));
            for l in 1..=nl {
                for l1 in 1..=nl1 {
                    if mt1 != i64::from(mt) || l1 >= l {
                        let c2 = self.cont()?;
                        for _ in 0..ni(c2[5]) {
                            self.list()?;
                        }
                    }
                }
            }
        }
        Some(())
    }

    fn file35(&mut self, h: [f64; 6]) -> Option<()> {
        for _ in 0..ni(h[4]) {
            self.list()?;
        }
        Some(())
    }

    fn file40(&mut self, h: [f64; 6]) -> Option<()> {
        for _ in 0..ni(h[4]) {
            let c = self.cont()?;
            for _ in 0..ni(c[5]) {
                let c2 = self.cont()?;
                let (nc, nii) = (ni(c2[4]), ni(c2[5]));
                for _ in 0..nc.max(0) {
                    self.cont()?;
                    self.list()?;
                }
                for _ in 0..nii.max(0) {
                    self.list()?;
                }
            }
        }
        Some(())
    }
}

/// The kind of every row of a section with key `(mf, mt)`, for an ENDF
/// format version `iverf` (4, 5 or 6), or `None` when the section is not
/// covered (see the module docs) or its records do not account for exactly
/// all of its rows.
pub(crate) fn section_layout(mf: i32, mt: i32, rows: &[[f64; 6]], iverf: i32) -> Option<Vec<LineKind>> {
    let mut w = Walker::new(rows, iverf);
    let h = w.cont()?;
    match mf {
        1 => w.file1(mt, h)?,
        2 => w.file2(mt, h)?,
        3 | 23 | 27 => w.file3()?,
        4 | 24 => w.file4(h)?,
        5 | 25 => w.file5(h)?,
        6 | 26 => w.file6(h)?,
        7 => w.file7(mt, h)?,
        8 => w.file8(mt, h)?,
        9 | 10 => w.file9(h)?,
        12 | 13 | 30 => w.file1x(mf, mt, h)?,
        14 => w.file14(h)?,
        15 => w.file15()?,
        28 => w.file28(h)?,
        31 | 33 => w.file3x(h)?,
        34 => w.file34(mt, h)?,
        35 => w.file35(h)?,
        40 => w.file40(h)?,
        _ => return None,
    }
    (w.pos == rows.len()).then_some(w.kinds)
}

/// Whether a material is a GENDF or ERRORR-output material, which MODER
/// copies through a separate path (`moder.f90:161`, `n1h = -1`, and
/// `:271-273`, `n1h = -11, -12, -14` on MF=1/MT=451's HEAD) because its
/// sections are group-wise LISTs, not ENDF-structured. [`section_layout`] does
/// not apply to them.
pub(crate) fn is_group_material(mf1_451_rows: &[[f64; 6]]) -> bool {
    mf1_451_rows.first().is_some_and(|h| matches!(ni(h[4]), -1 | -11 | -12 | -14))
}

/// Whether `kinds` can format `rows` without losing a value: no non-zero
/// number in a field the layout leaves blank (past a data or integer count,
/// or in the first two fields of a directory line). A layout that fails this
/// has misread the structure, and the writer must not use it.
pub(crate) fn layout_keeps_every_value(rows: &[[f64; 6]], kinds: &[LineKind]) -> bool {
    rows.iter().zip(kinds).all(|(r, k)| match k {
        LineKind::Data(n) | LineKind::Ints(n) => r[*n as usize..].iter().all(|&x| x == 0.0),
        LineKind::Dir => r[0] == 0.0 && r[1] == 0.0,
        LineKind::Cont | LineKind::Text => true,
    })
}

/// MODER's ENDF format version from a material's MF=1/MT=451 rows
/// (`moder.f90:186-195`): the second record's `N1 ≠ 0` means ENDF-4,
/// otherwise its `N2 = 0` means ENDF-5, else ENDF-6.
pub(crate) fn iverf_from_mf1(rows: &[[f64; 6]]) -> i32 {
    match rows.get(1) {
        Some(r) if ni(r[4]) != 0 => 4,
        Some(r) if ni(r[5]) == 0 => 5,
        Some(_) => 6,
        None => 6,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tab1_section_is_head_ints_and_data() {
        // MF=3: HEAD, TAB1 head (NR=1, NP=4), one interpolation line, two data lines.
        let rows = [
            [1.0, 2.0, 0.0, 0.0, 0.0, 0.0],
            [0.0, 0.0, 0.0, 0.0, 1.0, 4.0],
            [4.0, 2.0, 0.0, 0.0, 0.0, 0.0],
            [1.0, 1.0, 2.0, 2.0, 3.0, 3.0],
            [4.0, 4.0, 0.0, 0.0, 0.0, 0.0],
        ];
        let k = section_layout(3, 1, &rows, 6).expect("walks");
        assert_eq!(k, vec![LineKind::Cont, LineKind::Cont, LineKind::Ints(2), LineKind::Data(6), LineKind::Data(2)]);
    }

    #[test]
    fn a_short_section_is_refused_rather_than_guessed() {
        let rows = [[1.0, 2.0, 0.0, 0.0, 0.0, 0.0], [0.0, 0.0, 0.0, 0.0, 1.0, 4.0]];
        assert!(section_layout(3, 1, &rows, 6).is_none());
    }

    #[test]
    fn iverf_follows_moder() {
        let six = [[0.0; 6], [0.0, 0.0, 0.0, 0.0, 0.0, 6.0]];
        let five = [[0.0; 6], [0.0; 6]];
        let four = [[0.0; 6], [0.0, 0.0, 0.0, 0.0, 3.0, 0.0]];
        assert_eq!(iverf_from_mf1(&six), 6);
        assert_eq!(iverf_from_mf1(&five), 5);
        assert_eq!(iverf_from_mf1(&four), 4);
    }
}
