// SPDX-License-Identifier: AGPL-3.0-only
//
// Attribution (RESEARCH_INTEGRITY_AND_PROVENANCE.md): port of EGSnrc.
//   Upstream project: EGSnrc, https://github.com/nrc-cnrc/EGSnrc
//   Upstream file:    HEN_HOUSE/src/get_inputs.mortran (and, for
//                     GET_INPUT_PLUS, HEN_HOUSE/src/get_media_inputs.mortran)
//   Upstream commit:  bd49a47aae3938f49f4100b8e018091861408b19
//   Copyright:        National Research Council of Canada (authors: Aaron
//                     Merovitz, Dave Rogers, Iwan Kawrakow 1998; contributors
//                     Ernesto Mainegra-Hing, Blake Walters, Frederic Tessier,
//                     Reid Townson)
//   Upstream licence: AGPL-3.0-or-later; this port is AGPL-3.0-only.
//! `HEN_HOUSE/src/get_inputs.mortran` (EGSnrc): the delimited `key = value`
//! input reader, [`get_input`] and its twin [`get_input_plus`].
//!
//! Upstream: <https://github.com/nrc-cnrc/EGSnrc>, commit
//! `bd49a47aae3938f49f4100b8e018091861408b19`
//! (read 2026-11-21, a shallow clone, never committed here). Licence: AGPL-3.0.
//!
//! **Status: first port of `GET_INPUT` only, not differentially tested
//! (gh:#1680).** Only the part `get_media_inputs` needs is here. Not ported:
//! `get_transport_parameter` (`get_inputs.mortran:828-1880`, the user of
//! `GET_INPUT` for the transport parameters) and `set_elastic_parameter`
//! (`:1900`), both deferred in the phase 1 brief.
//!
//! | upstream | here |
//! |---|---|
//! | `SUBROUTINE GET_INPUT` (`get_inputs.mortran:69-821`) | [`get_input`] |
//! | `entry get_input_set_error_level` (`:818`) | [`GetInputCommon::set_error_level`] |
//! | `SUBROUTINE GET_INPUT_PLUS` (`get_media_inputs.mortran:32-632`) | [`get_input_plus`] |
//! | `entry get_input_plus_set_error_level` (`get_media_inputs.mortran:627`) | [`GetInputCommon::set_error_level_plus`] |
//! | `COMIN/GetInput/` (`egsnrc.macros:3380-3406`) | [`GetInputCommon`] |
//! | the Fortran input unit (`REWIND`, `READ(UNITNUM,'(A256)')`) | [`InputUnit`] |
//!
//! # How the Fortran is mapped
//!
//! * **The COMMON block is an owned struct passed explicitly.** Upstream
//!   leaves every slot of `VALUES_SOUGHT`, `VALUE_MIN`, `ALLOWED_INPUTS`,
//!   `ERROR_FLAGS`... as the last caller left them, and callers rely on that
//!   (for example `get_media_inputs` never sets `value_min` for the
//!   `mass fractions` slot when there is more than one element; the limits
//!   are whatever `get_transport_parameter` stored in that slot). Because
//!   [`GetInputCommon`] persists between calls exactly as the COMMON block
//!   does, those dependencies are reproduced instead of papered over. See
//!   [`GetInputCommon::new`] for the slots it pre-seeds.
//! * **Fixed-length strings are kept as fixed-width byte vectors** (256 for a
//!   record, 80 for a delimiter, 64 for a name: `$STRING32` is 64 at
//!   `egsnrc.macros:3375`), because upstream's behaviour depends on the
//!   padding. `INDEX(TEXT, DELIM_START)` searches for the delimiter **with
//!   its trailing blanks to 80 characters**, so a delimiter only matches when
//!   enough blanks follow it in the (comment-stripped) line. The same is true
//!   of the "does this line contain another sought name" test, where the
//!   `$SKIP LEADING BLANKS` macro leaves `length` at 64, so the needle is the
//!   name padded to 64 characters (see the note in [`get_input`]).
//! * **Error text** that upstream writes to `i_errors` is appended to
//!   [`GetInputCommon::err_log`] (gated by the same `error_level` tests).
//!   The wording follows upstream; list-directed spacing is not reproduced.
//! * `READ ... END=` on an exhausted unit becomes the same jump upstream takes.
//!   Reads that have no `END=` (the string branch's continuation line) are a
//!   Fortran runtime error and return `Err` here. `STOP` returns `Err`.
//! * Debug output (`IDEBUG`, always `.false.` upstream) is not ported.

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;

use super::egs_utilities_mortran::lnblnk1;

/// `$NMAX` (`egsnrc.macros:3372`): the number of slots in the COMMON block.
pub const MAX_INPUTS: usize = 100;
/// `$NVALUE` (`egsnrc.macros:3373`): values per slot.
pub const MAX_VALUES: usize = 100;
/// `$STRING80` (`egsnrc.macros:3374`).
pub const STRING_80: usize = 80;
/// `$STRING32` (`egsnrc.macros:3375`): despite its name this is **64**.
pub const STRING_32: usize = 64;
/// `$STRING256` (`egsnrc.macros:3377`).
pub const STRING_256: usize = 256;
/// `$MXALINP` (`egsnrc.macros:3378`): `ALLOWED_INPUTS(I,0:$MXALINP)` has six entries.
pub const MAX_ALLOWED: usize = 5;

/// A Fortran input unit held in memory: the records of a text file, with the
/// read position that `REWIND` resets.
#[derive(Debug, Clone, Default)]
pub struct InputUnit {
    records: Vec<Vec<u8>>,
    pos: usize,
}

impl InputUnit {
    /// The unit for a text file's contents. Records are separated by `\n`
    /// only (a `\r` stays in the record, and `lnblnk1` treats it as blank, as
    /// on a Linux build of EGSnrc); a final newline does not start a record.
    pub fn from_text(text: &str) -> Self {
        let mut records: Vec<Vec<u8>> = text.split('\n').map(|l| l.as_bytes().to_vec()).collect();
        if text.ends_with('\n') || text.is_empty() {
            records.pop();
        }
        Self { records, pos: 0 }
    }

    /// `REWIND(UNITNUM)`.
    pub fn rewind(&mut self) {
        self.pos = 0;
    }

    /// One `READ(UNITNUM,FMT='(A..)')`: the next record, or `None` at end of file.
    pub fn read(&mut self) -> Option<Vec<u8>> {
        let r = self.records.get(self.pos).cloned();
        if r.is_some() {
            self.pos += 1;
        }
        r
    }
}

/// `TYPE(I)` of a slot (`egsnrc.macros:3392-3396`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InputType {
    /// `0`: integer (stored, like every value, as a real).
    #[default]
    Integer,
    /// `1`: real.
    Real,
    /// `2`: character.
    Text,
    /// `3`: character with `ALLOWED_INPUTS`; the value is the index matched.
    Allowed,
}

/// `COMIN/GetInput/` (`egsnrc.macros:3380-3406`), with the two `error_level`
/// variables that `GET_INPUT` and `GET_INPUT_PLUS` each `SAVE` and the error
/// output unit. Slots are numbered `1..=100` as upstream, so index `ival`
/// holds slot `ival`; index 0 is unused.
#[derive(Debug, Clone)]
pub struct GetInputCommon {
    /// `ALLOWED_INPUTS(I,0:5)`, each 64 wide.
    pub allowed_inputs: Vec<Vec<Vec<u8>>>,
    /// `VALUES_SOUGHT(I)`, 64 wide.
    pub values_sought: Vec<Vec<u8>>,
    /// `CHAR_VALUE(I,IVAL)`, 256 wide, grown on demand; blank until written.
    char_value: Vec<Vec<Vec<u8>>>,
    /// `VALUE(I,IVAL)`, grown on demand; `0.0` until written.
    value: Vec<Vec<f64>>,
    /// `DEFAULT(I)`.
    pub default: Vec<f64>,
    /// `VALUE_MIN(I)`.
    pub value_min: Vec<f64>,
    /// `VALUE_MAX(I)`.
    pub value_max: Vec<f64>,
    /// `NVALUE(I)`.
    pub nvalue: Vec<usize>,
    /// `TYPE(I)`.
    pub kind: Vec<InputType>,
    /// `ERROR_FLAGS(I)`: 0 is no error.
    pub error_flags: Vec<i32>,
    /// `NMIN`.
    pub nmin: usize,
    /// `NMAX`.
    pub nmax: usize,
    /// `ERROR_FLAG`.
    pub error_flag: i32,
    /// `DELIMETER`, 64 wide.
    pub delimeter: Vec<u8>,
    /// `error_level` of `GET_INPUT` (`SAVE`d, `DATA error_level/1/`,
    /// `get_inputs.mortran`).
    pub error_level: i32,
    /// `error_level` of `GET_INPUT_PLUS` (its own `SAVE`d variable).
    pub error_level_plus: i32,
    /// What upstream writes to unit `i_errors`.
    pub err_log: Vec<String>,
}

impl GetInputCommon {
    /// A COMMON block as it stands when `get_media_inputs` is entered after
    /// `get_transport_parameter` has run: everything zero or blank except
    /// slots 1 to 3, which `get_transport_parameter` fills with its `ECUT`,
    /// `PCUT` and `SMAX` entries (`get_inputs.mortran:1233-1258`):
    /// `value_min = 0`, `value_max = 1e15`, `default = 0`, `0`, `1e10`
    /// (`$GLOBAL-ECUT`, `$GLOBAL-PCUT`, `$MAX-SMAX` at `egsnrc.macros:2269-2273`).
    /// `get_media_inputs` relies on these limits for `number of atoms` and
    /// `mass fractions` (which only set their own limits when there is one
    /// element); a COMMON block that is all zero would make every such value
    /// out of range and replace it by `DEFAULT`.
    ///
    /// `error_level` is 1 for both routines (`data error_level/1/`).
    pub fn new() -> Self {
        let n = MAX_INPUTS + 1;
        let mut c = Self {
            allowed_inputs: (0..n).map(|_| (0..=MAX_ALLOWED).map(|_| fixed(b"", STRING_32)).collect()).collect(),
            values_sought: (0..n).map(|_| fixed(b"", STRING_32)).collect(),
            char_value: (0..n).map(|_| Vec::new()).collect(),
            value: (0..n).map(|_| Vec::new()).collect(),
            default: vec![0.0; n],
            value_min: vec![0.0; n],
            value_max: vec![0.0; n],
            nvalue: vec![0; n],
            kind: vec![InputType::Integer; n],
            error_flags: vec![0; n],
            nmin: 0,
            nmax: 0,
            error_flag: 0,
            delimeter: fixed(b"", STRING_32),
            error_level: 1,
            error_level_plus: 1,
            err_log: Vec::new(),
        };
        for (slot, dflt) in [(1usize, 0.0), (2, 0.0), (3, 1e10)] {
            c.value_min[slot] = 0.0;
            c.value_max[slot] = 1e15;
            c.default[slot] = dflt;
            c.kind[slot] = InputType::Real;
            c.nvalue[slot] = 1;
        }
        c
    }

    /// `call get_input_set_error_level(level)`.
    pub fn set_error_level(&mut self, level: i32) {
        self.error_level = level;
    }

    /// `call get_input_plus_set_error_level(level)`.
    pub fn set_error_level_plus(&mut self, level: i32) {
        self.error_level_plus = level;
    }

    /// Declare a slot: `values_sought(ival)`, `type(ival)`, `nvalue(ival)`.
    /// The name is cut at 64 characters like the Fortran assignment.
    pub fn declare(&mut self, slot: usize, name: &str, kind: InputType, nvalue: usize) {
        self.values_sought[slot] = fixed(name.as_bytes(), STRING_32);
        self.kind[slot] = kind;
        self.nvalue[slot] = nvalue;
    }

    /// `value_min(ival)`, `value_max(ival)`, `default(ival)`.
    pub fn set_limits(&mut self, slot: usize, min: f64, max: f64, default: f64) {
        self.value_min[slot] = min;
        self.value_max[slot] = max;
        self.default[slot] = default;
    }

    /// `allowed_inputs(ival,k)=text`.
    pub fn set_allowed(&mut self, slot: usize, k: usize, text: &str) {
        self.allowed_inputs[slot][k] = fixed(text.as_bytes(), STRING_32);
    }

    /// `NMIN = NMAX = slot` (a single-slot call).
    pub fn seek(&mut self, slot: usize) {
        self.nmin = slot;
        self.nmax = slot;
    }

    /// `VALUE(slot, ival)`; `0.0` for a value never written.
    pub fn value(&self, slot: usize, ival: usize) -> f64 {
        self.value[slot].get(ival - 1).copied().unwrap_or(0.0)
    }

    /// Store `VALUE(slot, ival)`.
    pub fn set_value(&mut self, slot: usize, ival: usize, v: f64) {
        let row = &mut self.value[slot];
        if row.len() < ival {
            row.resize(ival, 0.0);
        }
        row[ival - 1] = v;
    }

    /// `CHAR_VALUE(slot, ival)` without trailing blanks; empty if never written.
    pub fn char_value(&self, slot: usize, ival: usize) -> String {
        match self.char_value[slot].get(ival - 1) {
            Some(b) => lossy(trim_blanks(b)),
            None => String::new(),
        }
    }

    fn set_char_value_bytes(&mut self, slot: usize, ival: usize, v: Vec<u8>) {
        let row = &mut self.char_value[slot];
        if row.len() < ival {
            row.resize(ival, fixed(b"", STRING_256));
        }
        row[ival - 1] = v;
    }

    /// `$CONVERT CHAR_VALUE(slot, ival) TO UPPER CASE`, in place.
    pub fn upper_char_value(&mut self, slot: usize, ival: usize) {
        if let Some(b) = self.char_value[slot].get_mut(ival - 1) {
            upper(b);
        }
    }

    fn name(&self, slot: usize) -> String {
        lossy(trim_blanks(&self.values_sought[slot]))
    }
}

impl Default for GetInputCommon {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------
// Fortran character helpers
// ---------------------------------------------------------------------

/// `s` cut or blank-padded to `w` characters (a Fortran character assignment).
pub(super) fn fixed(s: &[u8], w: usize) -> Vec<u8> {
    let mut v: Vec<u8> = s.iter().take(w).copied().collect();
    v.resize(w, b' ');
    v
}

/// `s` without trailing blanks (only `' '`; Fortran's comparison pads with spaces).
pub(super) fn trim_blanks(s: &[u8]) -> &[u8] {
    let mut n = s.len();
    while n > 0 && s[n - 1] == b' ' {
        n -= 1;
    }
    &s[..n]
}

pub(super) fn lossy(s: &[u8]) -> String {
    String::from_utf8_lossy(s).into_owned()
}

/// Fortran `a == b` for character operands: equal after padding with blanks.
pub(super) fn eq_blank_padded(a: &[u8], b: &[u8]) -> bool {
    trim_blanks(a) == trim_blanks(b)
}

/// Fortran `INDEX(hay, needle)`: 1-based position of the first occurrence, 0 if none.
pub(super) fn index1(hay: &[u8], needle: &[u8]) -> usize {
    if needle.is_empty() {
        return 1;
    }
    if needle.len() > hay.len() {
        return 0;
    }
    for p in 0..=(hay.len() - needle.len()) {
        if &hay[p..p + needle.len()] == needle {
            return p + 1;
        }
    }
    0
}

/// `s(from:)` (1-based) blank-padded to `w`; blank when `from` is past the end.
pub(super) fn tail(s: &[u8], from: usize, w: usize) -> Vec<u8> {
    if from >= 1 && from <= s.len() {
        fixed(&s[from - 1..], w)
    } else {
        fixed(b"", w)
    }
}

/// `$CONVERT # TO UPPER CASE` (`get_inputs.mortran:255`): ASCII `a-z` to
/// upper case over `1..lnblnk1(s)`.
pub(super) fn upper(s: &mut [u8]) {
    let n = lnblnk1(s);
    for b in s.iter_mut().take(n) {
        if (97..=122).contains(b) {
            *b -= 32;
        }
    }
}

/// `$SKIP LEADING BLANKS IN #` (`get_inputs.mortran:268`): shift out leading
/// blanks (the string stays the same width). **Returns the value the macro
/// leaves in the variable `length`**, which is not the string's length: the
/// macro sets `length = len(s)` and decrements it once per shift, stopping at
/// 1, so it is `len(s) - blanks` (at least 1).
pub(super) fn skip_leading_blanks(s: &mut Vec<u8>) -> usize {
    let w = s.len();
    let lead = s.iter().take_while(|&&b| b == b' ').count();
    let shifts = lead.min(w.saturating_sub(1));
    if shifts > 0 {
        let t = tail(s, shifts + 1, w);
        *s = t;
    }
    w - shifts
}

/// `$REMOVE EVERYTHING AFTER # IN #` (`get_inputs.mortran:278`): cut the
/// string at the first `c` (and `c` itself); a `c` in column 1 blanks it.
fn remove_after(s: &mut Vec<u8>, c: u8) {
    let w = s.len();
    let p = index1(s, &[c]);
    if p > 1 {
        *s = fixed(&s[..p - 1], w);
    } else if p == 1 {
        *s = fixed(b"", w);
    }
}

/// The processing every record gets (`get_inputs.mortran:140-150`): skip
/// leading blanks, drop everything after `#` and after `;`, trim, and return
/// `(TEXT in upper case, ORIGTEXT in original case)`, both 256 wide.
fn clean_record(rec: &[u8]) -> (Vec<u8>, Vec<u8>) {
    let mut text = fixed(rec, STRING_256);
    skip_leading_blanks(&mut text);
    remove_after(&mut text, b'#');
    remove_after(&mut text, b';');
    let length = lnblnk1(&text);
    let text = fixed(&text[..length], STRING_256);
    let orig = text.clone();
    let mut up = text;
    upper(&mut up);
    (up, orig)
}

// ---------------------------------------------------------------------
// List-directed reading (READ(TEXT,FMT=*) ...)
// ---------------------------------------------------------------------

/// One Fortran list-directed real (`1.5`, `-2`, `.5`, `5.`, `1e3`, `1.5D-3`,
/// `1.0+5`, `inf`, `nan`), as gfortran reads it. `None` if the token is not one.
pub fn parse_fortran_real(tok: &[u8]) -> Option<f64> {
    let s = core::str::from_utf8(tok).ok()?;
    let b = s.as_bytes();
    let mut i = 0;
    let mut neg = false;
    if i < b.len() && (b[i] == b'+' || b[i] == b'-') {
        neg = b[i] == b'-';
        i += 1;
    }
    let rest = &s[i..];
    let lower: String = rest.chars().map(|c| c.to_ascii_lowercase()).collect();
    if lower == "inf" || lower == "infinity" {
        return Some(if neg { f64::NEG_INFINITY } else { f64::INFINITY });
    }
    if lower == "nan" {
        return Some(f64::NAN);
    }
    let int_start = i;
    while i < b.len() && b[i].is_ascii_digit() {
        i += 1;
    }
    let int_part = &s[int_start..i];
    let mut frac_part = "";
    if i < b.len() && b[i] == b'.' {
        i += 1;
        let fs = i;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
        frac_part = &s[fs..i];
    }
    if int_part.is_empty() && frac_part.is_empty() {
        return None;
    }
    let mut exp = String::new();
    if i < b.len() {
        let c = b[i].to_ascii_lowercase();
        if c == b'e' || c == b'd' || c == b'q' {
            i += 1;
        } else if b[i] != b'+' && b[i] != b'-' {
            return None;
        }
        if i < b.len() && (b[i] == b'+' || b[i] == b'-') {
            exp.push(b[i] as char);
            i += 1;
        }
        let es = i;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
        if es == i {
            return None;
        }
        exp.push_str(&s[es..i]);
    }
    if i != b.len() {
        return None;
    }
    let mut norm = String::new();
    norm.push_str(if int_part.is_empty() { "0" } else { int_part });
    norm.push('.');
    norm.push_str(if frac_part.is_empty() { "0" } else { frac_part });
    if !exp.is_empty() {
        norm.push('e');
        norm.push_str(&exp);
    }
    let v: f64 = norm.parse().ok()?;
    Some(if neg { -v } else { v })
}

fn parse_fortran_int(tok: &[u8]) -> Option<f64> {
    let s = core::str::from_utf8(tok).ok()?;
    let digits = s.strip_prefix(['+', '-']).unwrap_or(s);
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    s.parse::<i64>().ok().map(|v| v as f64)
}

/// Scan one record for list-directed items, appending to `out` until it holds
/// `want` items. Separators are blanks, tabs and commas; two commas (or a
/// leading one) are a null value (`None`, "variable unchanged"); `r*c` is a
/// repeat and `r*` repeated nulls; `/` ends the statement (the remaining
/// items are nulls). `is_int[k]` says item `k` is read as an integer (a real
/// token for an integer item is the runtime error `Err`). Returns `Ok(true)`
/// when the statement is complete.
fn scan_record(rec: &[u8], is_int: &[bool], want: usize, out: &mut Vec<Option<f64>>) -> Result<bool, ()> {
    let mut p = 0;
    let n = rec.len();
    while out.len() < want {
        while p < n && matches!(rec[p], b' ' | b'\t' | b'\r') {
            p += 1;
        }
        if p >= n {
            return Ok(false);
        }
        match rec[p] {
            b',' => {
                out.push(None);
                p += 1;
            }
            b'/' => {
                while out.len() < want {
                    out.push(None);
                }
                return Ok(true);
            }
            _ => {
                let s = p;
                while p < n && !matches!(rec[p], b' ' | b'\t' | b'\r' | b',' | b'/') {
                    p += 1;
                }
                let tok = &rec[s..p];
                let (repeat, val_tok) = match tok.iter().position(|&b| b == b'*') {
                    Some(star) => {
                        let r = parse_fortran_int(&tok[..star]).ok_or(())?;
                        if r < 1.0 {
                            return Err(());
                        }
                        (r as usize, &tok[star + 1..])
                    }
                    None => (1, tok),
                };
                let kind_int = is_int.get(out.len()).copied().unwrap_or(false);
                let v = if val_tok.is_empty() {
                    None
                } else if kind_int {
                    Some(parse_fortran_int(val_tok).ok_or(())?)
                } else {
                    Some(parse_fortran_real(val_tok).ok_or(())?)
                };
                for _ in 0..repeat {
                    if out.len() < want {
                        out.push(v);
                    }
                }
                while p < n && matches!(rec[p], b' ' | b'\t' | b'\r') {
                    p += 1;
                }
                if p < n && rec[p] == b',' {
                    p += 1;
                }
            }
        }
    }
    Ok(true)
}

/// `READ(unit,*) item1, item2, ...`: list-directed read of `is_int.len()`
/// items across as many records as it takes; the rest of the last record is
/// discarded. Nulls come back as `None`.
///
/// # Errors
///
/// `Err` for a bad token (Fortran's "Bad real number in item"), or when the
/// unit runs out of records (end-of-file without `END=`).
pub fn read_list(unit: &mut InputUnit, is_int: &[bool]) -> Result<Vec<Option<f64>>, String> {
    let want = is_int.len();
    let mut out: Vec<Option<f64>> = Vec::with_capacity(want);
    while out.len() < want {
        let rec = unit.read().ok_or_else(|| String::from("end of file during list-directed read"))?;
        let rec = &rec[..rec.len().min(STRING_256.max(rec.len()))];
        scan_record(rec, is_int, want, &mut out).map_err(|()| String::from("bad item in list-directed read"))?;
    }
    Ok(out)
}

// ---------------------------------------------------------------------
// GET_INPUT / GET_INPUT_PLUS
// ---------------------------------------------------------------------

/// `GET_INPUT` (`get_inputs.mortran:69-821`): for each slot `nmin..=nmax` of
/// `c`, find `name = value(s)` between `:START <DELIMETER>:` and
/// `:STOP <DELIMETER>:` (or anywhere if `DELIMETER = 'NONE'`) in `unit` and
/// store the result in `c`; the caller declares the slots first
/// ([`GetInputCommon::declare`]) and reads `value`, `char_value`, `nvalue` and
/// `error_flags` afterwards.
///
/// Behaviour worth knowing (all as upstream, see the line cites):
///
/// * The name is found with `INDEX` anywhere in the line, not only at its
///   start (`:149`). Case is ignored for names, kept for strings.
/// * After the name, text up to `=` (or else `:`) is dropped; the value is
///   what follows. **A value shorter than two characters counts as missing**
///   (`lnblnk1(TEXT)=1`, `:422`; `GET_INPUT_PLUS` tests only `=0`). TEXT still
///   holds the blank after `=`, so `rho = 5` works and `rho=5` fails with
///   "VALUE NOT THERE" and a `RETURN` that abandons every later slot.
/// * `default` anywhere in the value of a numeric or allowed slot selects
///   `DEFAULT(I)` (or index 0), flag 0.
/// * A numeric value outside `[VALUE_MIN, VALUE_MAX]` is replaced by
///   `DEFAULT(I)` with a warning in the error log and **flag 0**.
/// * A trailing comma means "more values on the next line"; the next line is
///   consumed whatever it holds. The check that it is not another sought name
///   tests for the name **padded to 64 characters** (the `length` left by the
///   `$SKIP LEADING BLANKS` macro, `:522`), so it only fires when the name
///   ends the line.
/// * Several RETURN paths leave later slots' `ERROR_FLAGS` untouched, i.e.
///   stale from an earlier call, which is why [`GetInputCommon`] persists.
///
/// # Errors
///
/// `Err` where upstream stops: `NMAX < NMIN` or `NMAX > 100` (the `STOP`),
/// more than 100 values in a slot, or end of file while a string value is
/// being continued onto the next line (the string branch's `READ` has no
/// `END=`).
pub fn get_input(unit: &mut InputUnit, c: &mut GetInputCommon) -> Result<(), String> {
    run(unit, c, None)
}

/// `GET_INPUT_PLUS` (`get_media_inputs.mortran:32-632`): [`get_input`] with the
/// start and end delimiters given literally (matched in upper case, leading
/// blanks skipped, cut at 80 characters and matched **with their trailing
/// blanks**) instead of built from `DELIMETER`. `delim_start = "NONE"` means
/// "from the top of the file", `delim_end = "NONE"` "to the end of the file".
/// It differs from `GET_INPUT` in the value-missing test (`=0`, not `<=1`) and
/// in having no `TITLE` special case. Uses [`GetInputCommon::error_level_plus`].
///
/// # Errors
///
/// As [`get_input`].
pub fn get_input_plus(unit: &mut InputUnit, c: &mut GetInputCommon, delim_start: &str, delim_end: &str) -> Result<(), String> {
    run(unit, c, Some((delim_start, delim_end)))
}

/// `WRITE (ERR,*)` into the error log.
fn note(c: &mut GetInputCommon, msg: String) {
    c.err_log.push(msg);
}

/// What `run` does after each `READ` of a continuation line (numeric branch
/// `get_inputs.mortran:497-540`, string branch `:663-700`).
fn continuation_scan(c: &mut GetInputCommon, i: usize, rec: &[u8], keeptext: &[u8], error_level: i32, gated: bool) -> (Vec<u8>, Vec<u8>) {
    let (text, orig) = clean_record(rec);
    let nmax = c.nmax;
    for k in 1..=nmax {
        let mut vname1 = c.values_sought[k].clone();
        if lnblnk1(&vname1) > 0 {
            let length = skip_leading_blanks(&mut vname1);
            upper(&mut vname1);
            if index1(&text, &vname1[..length]) != 0 {
                if !gated || error_level > 0 {
                    let m = format!(
                        "ERROR: VALUE SOUGHT: {}; {} <--COMMA INDICATES ANOTHER INPUT; SEARCHED NEXT LINE: {}; BUT NO OTHER INPUT WAS DETECTED",
                        c.name(i),
                        lossy(trim_blanks(keeptext)),
                        lossy(trim_blanks(&text))
                    );
                    note(c, m);
                }
                c.error_flag = 1;
                c.error_flags[i] = 1;
            }
        }
    }
    (text, orig)
}

fn run(unit: &mut InputUnit, c: &mut GetInputCommon, plus: Option<(&str, &str)>) -> Result<(), String> {
    let is_plus = plus.is_some();
    let el = if is_plus { c.error_level_plus } else { c.error_level };
    c.error_flag = 0;
    if c.nmax < c.nmin || c.nmax > MAX_INPUTS {
        return Err(format!(
            "Error entering get_inputs: Asked for values from {} to {} with a max of {}. This implies a bug in the calling routine.",
            c.nmin, c.nmax, MAX_INPUTS
        ));
    }
    // delimiters (get_inputs.mortran:92-96, get_media_inputs.mortran:99-105)
    let (delim_start, delim_end) = match plus {
        None => {
            let d = trim_blanks(&c.delimeter);
            let mut s = b":START ".to_vec();
            s.extend_from_slice(d);
            s.push(b':');
            let mut e = b":STOP ".to_vec();
            e.extend_from_slice(d);
            e.push(b':');
            let (mut s, mut e) = (fixed(&s, STRING_80), fixed(&e, STRING_80));
            upper(&mut s);
            upper(&mut e);
            (s, e)
        }
        Some((s, e)) => {
            let (mut s, mut e) = (fixed(s.as_bytes(), STRING_80), fixed(e.as_bytes(), STRING_80));
            skip_leading_blanks(&mut s);
            skip_leading_blanks(&mut e);
            upper(&mut s);
            upper(&mut e);
            (s, e)
        }
    };
    let (no_start, no_end) = if is_plus {
        (eq_blank_padded(&delim_start, b"NONE"), !eq_blank_padded(&delim_end, b"NONE"))
    } else {
        (eq_blank_padded(&c.delimeter, b"NONE"), !eq_blank_padded(&c.delimeter, b"NONE"))
    };
    let check_end = no_end;
    let mut keeptext = fixed(b"", STRING_256);

    'items: for i in c.nmin..=c.nmax {
        unit.rewind();
        c.error_flags[i] = 0;
        match c.kind[i] {
            InputType::Integer | InputType::Real => {
                let d = c.default[i];
                c.set_value(i, 1, d);
            }
            InputType::Allowed => c.set_value(i, 1, 0.0),
            InputType::Text => {}
        }
        let mut vname = c.values_sought[i].clone();
        let ivname = lnblnk1(&vname);
        if ivname < 1 {
            if el > 0 {
                note(c, String::from("Warning: Empty VALUES_SOUGHT passed to Get_Inputs()"));
            }
            c.error_flag = 1;
            c.error_flags[i] = 1;
            continue 'items;
        }
        upper(&mut vname);
        let vname_key = vname[..ivname].to_vec();
        let is_title = !is_plus && eq_blank_padded(&vname_key, b"TITLE");
        // `vname(:ivname)='TITLE'` is also tested in GET_INPUT_PLUS's string
        // branch (get_media_inputs.mortran:446): see `title_exit` below.
        let title_exit = eq_blank_padded(&vname_key, b"TITLE");

        let mut start_found = no_start;
        let mut iindex = 0usize;
        let mut text = fixed(b"", STRING_256);
        let mut origtext = fixed(b"", STRING_256);
        while iindex == 0 {
            let Some(rec) = unit.read() else {
                // END=GI1003 (start found) / GI1004 (not found)
                if el > 0 {
                    let what = if start_found { "END DELIMETER (probably missing or misspelled)" } else { "START DELIMETER (probably missing or misspelled)" };
                    note(c, format!("ERROR: END OF FILE REACHED BUT VALUE SOUGHT NOT FOUND; {}; VALUE SOUGHT: >>{}<<", what, c.name(i)));
                }
                c.error_flag = 1;
                c.error_flags[i] = 1;
                continue 'items;
            };
            let (t, o) = clean_record(&rec);
            text = t;
            origtext = o;
            if !start_found {
                if index1(&text, &delim_start) != 0 {
                    start_found = true;
                }
                continue;
            }
            iindex = index1(&text, &vname_key);
            if check_end && index1(&text, &delim_end) != 0 {
                if el > 0 {
                    note(c, format!("ERROR: >>{}<< NOT FOUND; END OF DELIMETER: {}", c.name(i), lossy(trim_blanks(&delim_end))));
                }
                c.error_flag = 1;
                c.error_flags[i] = 1;
                continue 'items;
            }
        }

        // position after the name, and after '=' (or ':')
        iindex += ivname;
        text = tail(&text, iindex, STRING_256);
        origtext = tail(&origtext, iindex, STRING_256);
        let mut k = index1(&text, b"=");
        if k == 0 {
            k = index1(&text, b":");
        }
        if k != 0 {
            text = tail(&text, k + 1, STRING_256);
            origtext = tail(&origtext, k + 1, STRING_256);
        }

        // value missing? (GET_INPUT: length 0 or 1; GET_INPUT_PLUS: 0)
        let tl = lnblnk1(&text);
        if tl == 0 || (!is_plus && tl == 1) {
            if is_title {
                let rec = unit.read().ok_or_else(|| String::from("end of file reading the line after TITLE"))?;
                let piece = fixed(&rec, STRING_256);
                let pl = lnblnk1(&piece);
                if pl != 0 {
                    text = fixed(&piece[..pl], STRING_256);
                    skip_leading_blanks(&mut text);
                    // upstream skips blanks in ORIGTEXT, which still holds the
                    // text after '=' (not this line): get_inputs.mortran:429
                    skip_leading_blanks(&mut origtext);
                } else {
                    if el > 0 {
                        note(c, format!("ERROR: VALUE SOUGHT: {}; VALUE NOT THERE!!", c.name(i)));
                    }
                    c.error_flag = 1;
                    c.error_flags[i] = 1;
                    return Ok(());
                }
            } else {
                if el > 0 {
                    note(c, format!("ERROR: VALUE SOUGHT: {}; VALUE NOT THERE!!", c.name(i)));
                }
                c.error_flag = 1;
                c.error_flags[i] = 1;
                return Ok(());
            }
        }

        // 'default'
        if index1(&text, b"DEFAULT") != 0 && c.kind[i] != InputType::Text {
            if c.kind[i] != InputType::Allowed {
                let d = c.default[i];
                c.set_value(i, 1, d);
            } else {
                c.set_value(i, 1, 0.0);
            }
            continue 'items;
        }

        // ---------------- numeric ----------------
        if matches!(c.kind[i], InputType::Integer | InputType::Real) {
            let mut ival = 1usize;
            loop {
                if ival > MAX_VALUES {
                    return Err(format!("more than {} values for {}", MAX_VALUES, c.name(i)));
                }
                if lnblnk1(&text) == 0 {
                    if el > 0 {
                        note(c, format!("ERROR: VALUE SOUGHT: {}; VALUE NOT THERE!!", c.name(i)));
                    }
                    c.error_flag = 1;
                    c.error_flags[i] = 1;
                    return Ok(());
                }
                let mut got: Vec<Option<f64>> = Vec::new();
                match scan_record(&text, &[false], 1, &mut got) {
                    Err(()) => {
                        // GI1007
                        if el > 0 {
                            note(c, format!("ERROR READING VALUE SOUGHT: {}; LINE # {}; COULD NOT READ THE VALUE!!; SHOULD BE AN INTEGER OR A REAL...; IS THERE AN EXTRA COMMA AT THE END OF YOUR INPUT?", c.name(i), 0));
                        }
                        c.error_flag = 1;
                        c.error_flags[i] = 1;
                        continue 'items;
                    }
                    Ok(_) => {
                        if let Some(Some(v)) = got.first() {
                            c.set_value(i, ival, *v);
                        }
                    }
                }
                let v = c.value(i, ival);
                if v > c.value_max[i] || v < c.value_min[i] {
                    if el > 0 {
                        note(c, format!("WARNING: Default= {} used for: {}; value {} should be between {} and {}", c.default[i], c.name(i), v, c.value_min[i], c.value_max[i]));
                    }
                    let d = c.default[i];
                    c.set_value(i, ival, d);
                }
                if ival == c.nvalue[i] {
                    break;
                }
                if index1(&text, b",") != 0 || lnblnk1(&text) == 0 {
                    let comma = index1(&text, b",");
                    text = tail(&text, comma + 1, STRING_256);
                    while lnblnk1(&text) == 0 {
                        let Some(rec) = unit.read() else {
                            // END=GI1010: out of the numeric block, no flag, NVALUE untouched
                            continue 'items;
                        };
                        let (t, o) = continuation_scan(c, i, &rec, &keeptext, el, true);
                        text = t;
                        origtext = o;
                    }
                } else {
                    break;
                }
                ival += 1;
            }
            if c.nvalue[i] != 0 && c.nvalue[i] != ival {
                if el > 0 {
                    note(c, format!("ERROR: VALUE SOUGHT: {}; ASKED FOR {} NUMERICAL INPUT(S); HOWEVER, {} WERE DETECTED", c.name(i), c.nvalue[i], ival));
                }
                c.error_flag = 1;
                c.error_flags[i] = 1;
            } else {
                c.nvalue[i] = ival;
            }
            continue 'items;
        }

        // ---------------- character / allowed ----------------
        let mut ival = 1usize;
        loop {
            if ival > MAX_VALUES {
                return Err(format!("more than {} values for {}", MAX_VALUES, c.name(i)));
            }
            if lnblnk1(&text) == 0 {
                if el > 0 {
                    note(c, format!("ERROR: VALUE SOUGHT: {}; VALUE NOT THERE!!", c.name(i)));
                }
                c.error_flag = 1;
                c.error_flags[i] = 1;
                return Ok(());
            }
            let piece: Vec<u8> = if is_title {
                origtext.clone()
            } else {
                let p = index1(&origtext, b",");
                if p != 0 {
                    fixed(&origtext[..p - 1], STRING_256)
                } else {
                    origtext.clone()
                }
            };
            let mut cv = fixed(&piece, STRING_256);
            skip_leading_blanks(&mut cv);
            c.set_char_value_bytes(i, ival, cv);
            if c.kind[i] == InputType::Allowed {
                c.upper_char_value(i, ival);
                let cvb = c.char_value[i][ival - 1].clone();
                let mut allowed = false;
                for kk in 0..=MAX_ALLOWED {
                    // upstream strips and upper-cases ALLOWED_INPUTS(I,K) in place
                    let mut a = c.allowed_inputs[i][kk].clone();
                    skip_leading_blanks(&mut a);
                    upper(&mut a);
                    if eq_blank_padded(&a, &cvb) {
                        allowed = true;
                        c.set_value(i, ival, kk as f64);
                    }
                    c.allowed_inputs[i][kk] = a;
                }
                if !allowed {
                    // not gated by error_level upstream
                    let m = if ival != 1 {
                        format!("ERROR: VALUE SOUGHT: {}; SHOULD HAVE ONE INPUT ONLY; APPARENT STATE: COMMA INDICATING SECOND VALUE", c.name(i))
                    } else {
                        let opts: Vec<String> = (0..=MAX_ALLOWED).map(|kk| lossy(trim_blanks(&c.allowed_inputs[i][kk]))).collect();
                        format!("ERROR: VALUE SOUGHT: {}; INPUT-->{}<--NOT ALLOWED; OPTIONS ARE: {}", c.name(i), c.char_value(i, ival), opts.join(" | "))
                    };
                    note(c, m);
                    c.error_flag = 1;
                    c.error_flags[i] = 1;
                }
            }
            if title_exit {
                break;
            }
            // KEEPTEXT: erase and keep this TEXT
            keeptext = fixed(trim_blanks(&text), STRING_256);
            let comma = index1(&text, b",");
            if comma != 0 || lnblnk1(&text) == 0 {
                text = tail(&text, comma + 1, STRING_256);
                origtext = tail(&origtext, comma + 1, STRING_256);
                while lnblnk1(&text) == 0 {
                    let Some(rec) = unit.read() else {
                        return Err(format!("end of file reading the continuation of {} (READ without END=)", c.name(i)));
                    };
                    let (t, o) = continuation_scan(c, i, &rec, &keeptext, el, false);
                    text = t;
                    origtext = o;
                }
            } else {
                break;
            }
            ival += 1;
        }
        if c.nvalue[i] != 0 && c.nvalue[i] != ival {
            if el > 0 {
                note(c, format!("ERROR: VALUE SOUGHT: {}; ASKED FOR {} INPUT(S); HOWEVER, {} WERE DETECTED", c.name(i), c.nvalue[i], ival));
            }
            c.error_flag = 1;
            c.error_flags[i] = 1;
        } else {
            c.nvalue[i] = ival;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn real_slot(c: &mut GetInputCommon, slot: usize, name: &str, nvalue: usize, min: f64, max: f64, dflt: f64) {
        c.declare(slot, name, InputType::Real, nvalue);
        c.set_limits(slot, min, max, dflt);
        c.seek(slot);
    }

    #[test]
    fn get_inputs_input_unit_records_and_rewind() {
        let mut u = InputUnit::from_text("a\nb\n");
        assert_eq!(u.read().unwrap(), b"a");
        assert_eq!(u.read().unwrap(), b"b");
        assert!(u.read().is_none());
        u.rewind();
        assert_eq!(u.read().unwrap(), b"a");
        // no trailing newline: the last record is still a record; empty text has none
        let mut u2 = InputUnit::from_text("x\ny");
        assert_eq!(u2.read().unwrap(), b"x");
        assert_eq!(u2.read().unwrap(), b"y");
        assert!(InputUnit::from_text("").read().is_none());
        assert_eq!(InputUnit::from_text("\n\n").read().unwrap(), b"");
    }

    #[test]
    fn get_inputs_fortran_real_tokens() {
        assert_eq!(parse_fortran_real(b"1.5"), Some(1.5));
        assert_eq!(parse_fortran_real(b"-2"), Some(-2.0));
        assert_eq!(parse_fortran_real(b".5"), Some(0.5));
        assert_eq!(parse_fortran_real(b"5."), Some(5.0));
        assert_eq!(parse_fortran_real(b"1E3"), Some(1000.0));
        assert_eq!(parse_fortran_real(b"1.5D-3"), Some(1.5e-3));
        assert_eq!(parse_fortran_real(b"1.0+2"), Some(100.0));
        assert_eq!(parse_fortran_real(b"+Q1"), None);
        assert_eq!(parse_fortran_real(b"INF"), Some(f64::INFINITY));
        assert_eq!(parse_fortran_real(b"-inf"), Some(f64::NEG_INFINITY));
        assert!(parse_fortran_real(b"nan").unwrap().is_nan());
        assert_eq!(parse_fortran_real(b"abc"), None);
        assert_eq!(parse_fortran_real(b"1.0abc"), None);
        assert_eq!(parse_fortran_real(b"1e"), None);
        assert_eq!(parse_fortran_real(b"."), None);
        assert_eq!(parse_fortran_real(b""), None);
        assert_eq!(parse_fortran_int(b"+7"), Some(7.0));
        assert_eq!(parse_fortran_int(b"7.0"), None);
        assert_eq!(parse_fortran_int(b"-"), None);
    }

    #[test]
    fn get_inputs_list_directed_read() {
        let mut u = InputUnit::from_text("3  1.5, 2.5\n7 8\n");
        let v = read_list(&mut u, &[true, false, false]).unwrap();
        assert_eq!(v, vec![Some(3.0), Some(1.5), Some(2.5)]);
        // the rest of the last record is discarded: next read starts at record 2
        assert_eq!(read_list(&mut u, &[false]).unwrap(), vec![Some(7.0)]);
        // nulls, repeats, slash
        let mut u = InputUnit::from_text(",2,3*4.0\n");
        let v = read_list(&mut u, &[false, false, false, false]).unwrap();
        assert_eq!(v, vec![None, Some(2.0), Some(4.0), Some(4.0)]);
        let mut u = InputUnit::from_text("1 / 5\n");
        assert_eq!(read_list(&mut u, &[false, false]).unwrap(), vec![Some(1.0), None]);
        let mut u = InputUnit::from_text("2*\n");
        assert_eq!(read_list(&mut u, &[false, false]).unwrap(), vec![None, None]);
        // multi-record, then end of file
        let mut u = InputUnit::from_text("1\n2\n");
        assert_eq!(read_list(&mut u, &[false, false]).unwrap(), vec![Some(1.0), Some(2.0)]);
        let mut u = InputUnit::from_text("1\n");
        assert!(read_list(&mut u, &[false, false]).is_err());
        // an integer item given a real token, and a bad token
        let mut u = InputUnit::from_text("1.5\n");
        assert!(read_list(&mut u, &[true]).is_err());
        let mut u = InputUnit::from_text("x\n");
        assert!(read_list(&mut u, &[false]).is_err());
        let mut u = InputUnit::from_text("0*3\n");
        assert!(read_list(&mut u, &[false]).is_err());
    }

    #[test]
    fn get_inputs_string_helpers_follow_the_macros() {
        // skip leading blanks leaves `length` = width - blanks (>= 1)
        let mut s = fixed(b"   ab", 10);
        assert_eq!(skip_leading_blanks(&mut s), 7);
        assert_eq!(trim_blanks(&s), b"ab");
        let mut blank = fixed(b"", 10);
        assert_eq!(skip_leading_blanks(&mut blank), 1);
        let mut none = fixed(b"ab", 10);
        assert_eq!(skip_leading_blanks(&mut none), 10);
        // remove everything after
        let mut t = fixed(b"ab # c", 20);
        remove_after(&mut t, b'#');
        assert_eq!(trim_blanks(&t), b"ab");
        let mut t = fixed(b"# c", 20);
        remove_after(&mut t, b'#');
        assert_eq!(lnblnk1(&t), 0);
        let mut t = fixed(b"ab", 20);
        remove_after(&mut t, b';');
        assert_eq!(trim_blanks(&t), b"ab");
        // INDEX and tail
        assert_eq!(index1(b"abcabc", b"ca"), 3);
        assert_eq!(index1(b"abc", b"x"), 0);
        assert_eq!(index1(b"abc", b""), 1);
        assert_eq!(index1(b"ab", b"abc"), 0);
        assert_eq!(trim_blanks(&tail(b"abcdef", 4, 8)), b"def");
        assert_eq!(lnblnk1(&tail(b"abc", 9, 8)), 0);
        let mut u = b"abC1 x".to_vec();
        upper(&mut u);
        assert_eq!(u, b"ABC1 X");
        let (t, o) = clean_record(b"  Rho = 2.0  # c; d");
        assert_eq!(trim_blanks(&t), b"RHO = 2.0");
        assert_eq!(trim_blanks(&o), b"Rho = 2.0");
        assert!(eq_blank_padded(b"AB  ", b"AB"));
        assert_eq!(lossy(b"ok"), "ok");
    }

    #[test]
    fn get_inputs_numeric_value_default_range_and_absent() {
        let deck = ":start tst:\n  alpha = 2.5   # comment\n  beta = 99\n  gamma = default\n:stop tst:\n";
        let mut u = InputUnit::from_text(deck);
        let mut c = GetInputCommon::new();
        c.delimeter = fixed(b"tst", STRING_32);
        real_slot(&mut c, 1, "alpha", 1, 0.0, 10.0, 7.0);
        real_slot(&mut c, 2, "beta", 1, 0.0, 10.0, 3.0);
        real_slot(&mut c, 3, "gamma", 1, 0.0, 10.0, 4.0);
        real_slot(&mut c, 4, "delta", 1, 0.0, 10.0, 5.0);
        c.nmin = 1;
        c.nmax = 4;
        get_input(&mut u, &mut c).unwrap();
        assert_eq!(c.error_flags[1], 0);
        assert_eq!(c.value(1, 1), 2.5);
        // out of range: replaced by DEFAULT, flag still 0
        assert_eq!(c.error_flags[2], 0);
        assert_eq!(c.value(2, 1), 3.0);
        assert!(c.err_log.iter().any(|m| m.contains("Default=")));
        // 'default' keyword
        assert_eq!(c.error_flags[3], 0);
        assert_eq!(c.value(3, 1), 4.0);
        // absent: the end delimiter stops the search, value is the default, flag 1
        assert_eq!(c.error_flags[4], 1);
        assert_eq!(c.value(4, 1), 5.0);
        assert_eq!(c.error_flag, 1);
        assert_eq!(c.nvalue[1], 1);
    }

    #[test]
    fn get_inputs_single_character_value_is_missing_in_get_input() {
        // upstream quirk: lnblnk1(TEXT)=1 counts as no value (get_inputs.mortran:422)
        let deck = ":start t:\nrho=5\nsig = 5\n:stop t:\n";
        let mut c = GetInputCommon::new();
        c.delimeter = fixed(b"t", STRING_32);
        let mut u = InputUnit::from_text(deck);
        real_slot(&mut c, 1, "rho", 1, 0.0, 10.0, 1.0);
        real_slot(&mut c, 2, "sig", 1, 0.0, 10.0, 1.0);
        c.nmin = 1;
        c.nmax = 2;
        get_input(&mut u, &mut c).unwrap();
        assert_eq!(c.error_flags[1], 1);
        // the RETURN abandoned slot 2: its flag is the stale one (0) and its value is untouched
        assert_eq!(c.error_flags[2], 0);
        // alone, with the blank after '=', it reads
        c.nmin = 2;
        c.nmax = 2;
        get_input(&mut u, &mut c).unwrap();
        assert_eq!(c.error_flags[2], 0);
        assert_eq!(c.value(2, 1), 5.0);
        // GET_INPUT_PLUS accepts the one-character value
        let mut c2 = GetInputCommon::new();
        real_slot(&mut c2, 1, "rho", 1, 0.0, 10.0, 1.0);
        let mut u2 = InputUnit::from_text(deck);
        get_input_plus(&mut u2, &mut c2, ":start t:", ":stop t:").unwrap();
        assert_eq!(c2.error_flags[1], 0);
        assert_eq!(c2.value(1, 1), 5.0);
    }

    #[test]
    fn get_inputs_delimiters_need_their_trailing_blanks() {
        // the delimiter matches with its padding: a line with text after it does not start the block
        let deck = ":start t: junk\nrho = 4\n:stop t:\n";
        let mut c = GetInputCommon::new();
        c.delimeter = fixed(b"t", STRING_32);
        real_slot(&mut c, 1, "rho", 1, 0.0, 10.0, 1.0);
        get_input(&mut InputUnit::from_text(deck), &mut c).unwrap();
        assert_eq!(c.error_flags[1], 1);
        assert!(c.err_log.iter().any(|m| m.contains("START DELIMETER")));
        // DELIMETER = 'NONE': the whole file
        let mut c = GetInputCommon::new();
        c.delimeter = fixed(b"NONE", STRING_32);
        real_slot(&mut c, 1, "rho", 1, 0.0, 10.0, 1.0);
        get_input(&mut InputUnit::from_text("x\nRHO = 4\n"), &mut c).unwrap();
        assert_eq!(c.error_flags[1], 0);
        assert_eq!(c.value(1, 1), 4.0);
        // a start delimiter with no end delimiter: end of file with the name not found
        let mut c = GetInputCommon::new();
        c.delimeter = fixed(b"t", STRING_32);
        real_slot(&mut c, 1, "rho", 1, 0.0, 10.0, 1.0);
        get_input(&mut InputUnit::from_text(":start t:\nsig = 4\n"), &mut c).unwrap();
        assert_eq!(c.error_flags[1], 1);
        assert!(c.err_log.iter().any(|m| m.contains("END DELIMETER")));
        // the name before the start delimiter is not taken
        let mut c = GetInputCommon::new();
        c.delimeter = fixed(b"t", STRING_32);
        real_slot(&mut c, 1, "rho", 1, 0.0, 10.0, 1.0);
        get_input(&mut InputUnit::from_text("rho = 9\n:start t:\n:stop t:\n"), &mut c).unwrap();
        assert_eq!(c.error_flags[1], 1);
    }

    #[test]
    fn get_inputs_multiple_values_commas_and_continuation_lines() {
        let deck = ":start t:\nlist = 1, 2,\n  3\nshort = 1, 2\nmany = 1 2\n:stop t:\n";
        let mut c = GetInputCommon::new();
        c.delimeter = fixed(b"t", STRING_32);
        real_slot(&mut c, 1, "list", 0, 0.0, 10.0, 0.0);
        real_slot(&mut c, 2, "short", 3, 0.0, 10.0, 0.0);
        real_slot(&mut c, 3, "many", 1, 0.0, 10.0, 0.0);
        c.nmin = 1;
        c.nmax = 3;
        get_input(&mut InputUnit::from_text(deck), &mut c).unwrap();
        // nvalue 0: the count is returned
        assert_eq!(c.error_flags[1], 0);
        assert_eq!(c.nvalue[1], 3);
        assert_eq!((c.value(1, 1), c.value(1, 2), c.value(1, 3)), (1.0, 2.0, 3.0));
        // asked for 3, found 2
        assert_eq!(c.error_flags[2], 1);
        assert_eq!(c.nvalue[2], 3);
        assert!(c.err_log.iter().any(|m| m.contains("ASKED FOR 3")));
        // "1 2" is one list-directed read of the first item
        assert_eq!(c.error_flags[3], 0);
        assert_eq!(c.value(3, 1), 1.0);
    }

    #[test]
    fn get_inputs_bad_number_and_empty_value() {
        let deck = ":start t:\nbad = abc\nnone =\nok = 2 \n:stop t:\n";
        let mut c = GetInputCommon::new();
        c.delimeter = fixed(b"t", STRING_32);
        real_slot(&mut c, 1, "bad", 1, 0.0, 10.0, 6.0);
        real_slot(&mut c, 2, "none", 1, 0.0, 10.0, 6.0);
        real_slot(&mut c, 3, "ok", 1, 0.0, 10.0, 6.0);
        c.nmin = 1;
        c.nmax = 1;
        get_input(&mut InputUnit::from_text(deck), &mut c).unwrap();
        assert_eq!(c.error_flags[1], 1);
        assert_eq!(c.value(1, 1), 6.0);
        assert!(c.err_log.iter().any(|m| m.contains("COULD NOT READ")));
        // "none =" has nothing after '=': VALUE NOT THERE, and RETURN
        c.nmin = 2;
        c.nmax = 3;
        c.err_log.clear();
        get_input(&mut InputUnit::from_text(deck), &mut c).unwrap();
        assert_eq!(c.error_flags[2], 1);
        assert!(c.err_log.iter().any(|m| m.contains("VALUE NOT THERE")));
        // error_level 0 silences the gated messages
        c.set_error_level(0);
        c.err_log.clear();
        get_input(&mut InputUnit::from_text(deck), &mut c).unwrap();
        assert!(c.err_log.is_empty());
        assert_eq!(c.error_flag, 1);
    }

    #[test]
    fn get_inputs_strings_keep_case_and_split_on_commas() {
        let deck = ":start t:\nnames = Water, Air ,Lead\ntitle = My Run, with comma\n:stop t:\n";
        let mut c = GetInputCommon::new();
        c.delimeter = fixed(b"t", STRING_32);
        c.declare(1, "names", InputType::Text, 0);
        c.declare(2, "title", InputType::Text, 1);
        c.nmin = 1;
        c.nmax = 2;
        get_input(&mut InputUnit::from_text(deck), &mut c).unwrap();
        assert_eq!(c.nvalue[1], 3);
        assert_eq!(c.char_value(1, 1), "Water");
        assert_eq!(c.char_value(1, 2), "Air");
        assert_eq!(c.char_value(1, 3), "Lead");
        // TITLE is taken whole, commas included, and ends the value loop
        assert_eq!(c.error_flags[2], 0);
        assert_eq!(c.char_value(2, 1), "My Run, with comma");
        assert_eq!(c.char_value(2, 2), "");
        // asking for two where one is given
        c.nvalue[1] = 2;
        c.seek(1);
        get_input(&mut InputUnit::from_text(deck), &mut c).unwrap();
        assert_eq!(c.error_flags[1], 1);
    }

    #[test]
    fn get_inputs_string_continuation_and_end_of_file() {
        let deck = ":start t:\nnames = a,\n  b\n:stop t:\n";
        let mut c = GetInputCommon::new();
        c.delimeter = fixed(b"t", STRING_32);
        c.declare(1, "names", InputType::Text, 0);
        c.seek(1);
        get_input(&mut InputUnit::from_text(deck), &mut c).unwrap();
        assert_eq!((c.nvalue[1], c.char_value(1, 2).as_str()), (2, "b"));
        // a trailing comma on the last line: the continuation READ has no END=
        let mut c = GetInputCommon::new();
        c.delimeter = fixed(b"NONE", STRING_32);
        c.declare(1, "names", InputType::Text, 0);
        c.seek(1);
        assert!(get_input(&mut InputUnit::from_text("names = a,\n"), &mut c).is_err());
        // the next line holding another sought name is reported (name must end the line)
        let mut c = GetInputCommon::new();
        c.delimeter = fixed(b"NONE", STRING_32);
        c.declare(1, "names", InputType::Text, 0);
        c.declare(2, "other", InputType::Text, 1);
        c.nmin = 1;
        c.nmax = 2;
        get_input(&mut InputUnit::from_text("names = a,\nother\n"), &mut c).unwrap();
        assert_eq!(c.error_flags[1], 1);
        assert!(c.err_log.iter().any(|m| m.contains("COMMA INDICATES ANOTHER INPUT")));
    }

    #[test]
    fn get_inputs_allowed_inputs_and_title_on_next_line() {
        let mut c = GetInputCommon::new();
        c.delimeter = fixed(b"NONE", STRING_32);
        c.declare(1, "mode", InputType::Allowed, 1);
        c.set_allowed(1, 0, "Off");
        c.set_allowed(1, 1, "  On");
        c.set_allowed(1, 2, "Simple");
        c.seek(1);
        get_input(&mut InputUnit::from_text("mode = simple\n"), &mut c).unwrap();
        assert_eq!(c.error_flags[1], 0);
        assert_eq!(c.value(1, 1), 2.0);
        assert_eq!(c.char_value(1, 1), "SIMPLE");
        get_input(&mut InputUnit::from_text("mode = on\n"), &mut c).unwrap();
        assert_eq!(c.value(1, 1), 1.0);
        // not allowed: flag 1 and a message listing the options (not gated)
        c.set_error_level(0);
        c.err_log.clear();
        get_input(&mut InputUnit::from_text("mode = maybe\n"), &mut c).unwrap();
        assert_eq!(c.error_flags[1], 1);
        assert_eq!(c.value(1, 1), 0.0);
        assert!(c.err_log.iter().any(|m| m.contains("NOT ALLOWED")));
        // a second value for an allowed slot
        c.err_log.clear();
        get_input(&mut InputUnit::from_text("mode = on, off\n"), &mut c).unwrap();
        assert_eq!(c.error_flags[1], 1);
        // 'default' selects index 0
        get_input(&mut InputUnit::from_text("mode = default\n"), &mut c).unwrap();
        assert_eq!((c.error_flags[1], c.value(1, 1)), (0, 0.0));
        // TITLE: a one-character value triggers the read of the next line
        let mut c = GetInputCommon::new();
        c.delimeter = fixed(b"NONE", STRING_32);
        c.declare(1, "title", InputType::Text, 1);
        c.seek(1);
        get_input(&mut InputUnit::from_text("title=\nSecond line\n"), &mut c).unwrap();
        // upstream does not copy the new line into ORIGTEXT (get_inputs.mortran:429): the value is blank
        assert_eq!(c.error_flags[1], 0);
        assert_eq!(c.char_value(1, 1), "");
        // TITLE with an empty next line is VALUE NOT THERE; with no next line it is a runtime error
        let mut c = GetInputCommon::new();
        c.delimeter = fixed(b"NONE", STRING_32);
        c.declare(1, "title", InputType::Text, 1);
        c.seek(1);
        get_input(&mut InputUnit::from_text("title=\n\n"), &mut c).unwrap();
        assert_eq!(c.error_flags[1], 1);
        assert!(get_input(&mut InputUnit::from_text("title=\n"), &mut c).is_err());
    }

    #[test]
    fn get_inputs_calling_errors_and_empty_name() {
        let mut c = GetInputCommon::new();
        c.nmin = 5;
        c.nmax = 4;
        assert!(get_input(&mut InputUnit::from_text(""), &mut c).is_err());
        c.nmin = 1;
        c.nmax = 101;
        assert!(get_input(&mut InputUnit::from_text(""), &mut c).is_err());
        // an empty VALUES_SOUGHT is flagged and skipped
        let mut c = GetInputCommon::new();
        c.delimeter = fixed(b"NONE", STRING_32);
        c.declare(10, "", InputType::Real, 1);
        c.seek(10);
        get_input(&mut InputUnit::from_text("x = 1\n"), &mut c).unwrap();
        assert_eq!(c.error_flags[10], 1);
        assert!(c.err_log.iter().any(|m| m.contains("Empty VALUES_SOUGHT")));
        c.set_error_level_plus(0);
        assert_eq!(c.error_level_plus, 0);
        // a numeric slot with too many values
        let mut c = GetInputCommon::new();
        c.delimeter = fixed(b"NONE", STRING_32);
        real_slot(&mut c, 1, "v", 0, 0.0, 1e9, 0.0);
        let many: Vec<String> = (0..101).map(|_| String::from("1")).collect();
        let deck = format!("v = {}\n", many.join(","));
        assert!(get_input(&mut InputUnit::from_text(&deck), &mut c).is_err());
    }

    #[test]
    fn get_inputs_plus_literal_delimiters_and_none() {
        let deck = "medium = AIR\n rho = 0.0012\nmedium = WATER\n rho = 1.0\n";
        let mut c = GetInputCommon::new();
        real_slot(&mut c, 1, "rho", 1, 0.0, 1e15, 9.0);
        // the first block: from its header to the next header
        let mut u = InputUnit::from_text(deck);
        get_input_plus(&mut u, &mut c, "medium = AIR", "medium = WATER").unwrap();
        assert_eq!(c.value(1, 1), 0.0012);
        // second block to end of file ('NONE')
        get_input_plus(&mut u, &mut c, " medium = water", "NONE").unwrap();
        assert_eq!(c.value(1, 1), 1.0);
        // start 'NONE' = from the top
        get_input_plus(&mut u, &mut c, "NONE", "NONE").unwrap();
        assert_eq!(c.value(1, 1), 0.0012);
        // header not in the file
        get_input_plus(&mut u, &mut c, "medium = LEAD", "NONE").unwrap();
        assert_eq!(c.error_flags[1], 1);
    }
}
