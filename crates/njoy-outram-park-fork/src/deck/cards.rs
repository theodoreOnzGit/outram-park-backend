// Translated from the semantics of Fortran list-directed input (`read(nsysi,*)`)
// as NJOY2016 `src/main.f90` and every module's `ruin`/input block use it.
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under AGPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! The free-format **card reader**: Fortran list-directed `read(nsysi,*)`.
//!
//! NJOY reads every input card with `read(nsysi,*) item, item, ...` on
//! standard input. This module reproduces what such a statement does, as
//! gfortran 13 does it (checked case by case against a compiled Fortran
//! oracle: `verification_and_validation/deck_driver/`):
//!
//! - values are separated by blanks, tabs or commas;
//! - a comma with no value before it, or a leading comma, is a **null**: the
//!   item keeps the value it had (a *default preserved across the card*);
//! - `/` ends the read: every item not yet reached keeps its value, and the
//!   rest of the record is ignored (this is how decks carry trailing comments);
//! - `r*v` repeats `v` for `r` items, and `r*` alone is `r` nulls;
//! - strings may be quoted with `'` or `"` (a doubled quote is the quote
//!   itself), or unquoted up to the next blank, comma or slash;
//! - a read continues over as many records as it needs to fill its items, a
//!   blank record carries no value, and **whatever is left on the record where
//!   the read finishes is discarded** (the next `read` starts on a new record).
//!   That is why a module name card `-- reconr: comment` reads as `--`;
//! - an integer item refuses `1.5`, a real item accepts integers and the
//!   Fortran exponent forms `d`, `q` and the letterless `1.5+3`.
//!
//! Items are described by [`Slot`], preset with the default the caller wants
//! to survive a null, and updated in place by [`CardReader::read`].

use crate::NjoyError;

/// One item of a `read(nsysi,*)` statement, holding its current value.
#[derive(Debug, Clone, PartialEq)]
pub enum Slot {
    /// An `integer` item (32-bit, as gfortran's default).
    Int(i32),
    /// A `real(kr)` item.
    Real(f64),
    /// A `character(len)` item: the value is kept blank-padded to `len`
    /// (Fortran assignment), see [`Slot::string`] for the trimmed text.
    Str {
        /// Current value, exactly `len` characters.
        value: String,
        /// Declared length of the Fortran variable.
        len: usize,
    },
}

impl Slot {
    /// A `character(len)` item preset to `init` (padded or truncated to `len`).
    pub fn string(len: usize, init: &str) -> Self {
        Slot::Str { value: pad(init, len), len }
    }

    /// The integer value; panics if this is not an `Int` slot (a caller bug).
    #[allow(clippy::panic)] // a slot of the wrong kind is a programming error in the card reader's caller
    pub fn int(&self) -> i32 {
        match self {
            Slot::Int(v) => *v,
            other => panic!("slot is not an integer: {other:?}"),
        }
    }

    /// The real value; panics if this is not a `Real` slot.
    #[allow(clippy::panic)] // as [`Slot::int`]
    pub fn real(&self) -> f64 {
        match self {
            Slot::Real(v) => *v,
            other => panic!("slot is not a real: {other:?}"),
        }
    }

    /// The string value with trailing blanks removed; panics if not `Str`.
    #[allow(clippy::panic)] // as [`Slot::int`]
    pub fn text(&self) -> &str {
        match self {
            Slot::Str { value, .. } => value.trim_end(),
            other => panic!("slot is not a string: {other:?}"),
        }
    }
}

fn pad(s: &str, len: usize) -> String {
    let mut v: String = s.chars().take(len).collect();
    while v.chars().count() < len {
        v.push(' ');
    }
    v
}

/// A deck: the records of an NJOY input stream and a read position.
#[derive(Debug, Clone)]
pub struct CardReader {
    lines: Vec<String>,
    pos: usize,
}

fn err(msg: impl Into<String>) -> NjoyError {
    NjoyError::InvalidInput(format!("deck card: {}", msg.into()))
}

fn is_blank(c: char) -> bool {
    c == ' ' || c == '\t'
}

impl CardReader {
    /// A reader over `text`, one record per line (`\r\n` accepted).
    pub fn new(text: &str) -> Self {
        Self { lines: text.lines().map(|l| l.trim_end_matches('\r').to_string()).collect(), pos: 0 }
    }

    /// Records not yet consumed.
    pub fn records_left(&self) -> usize {
        self.lines.len().saturating_sub(self.pos)
    }

    /// The records not yet consumed, one per line, for a module whose own
    /// reader takes text (LEAPR's [`crate::leapr::deck::LeaprDeck`]).
    pub fn rest(&self) -> String {
        let mut s = String::new();
        for l in self.lines.iter().skip(self.pos) {
            s.push_str(l);
            s.push('\n');
        }
        s
    }

    /// Mark the next `n` records consumed (after [`Self::rest`] was read by
    /// another reader).
    pub fn skip(&mut self, n: usize) {
        self.pos = (self.pos + n).min(self.lines.len());
    }

    /// The 1-based number of the next record, for messages.
    pub fn next_record_number(&self) -> usize {
        self.pos + 1
    }

    /// Execute one `read(nsysi,*) slots...`: fill the slots in place.
    ///
    /// # Errors
    /// [`NjoyError::InvalidInput`] on end of file before the items are filled
    /// (Fortran's `end of file` runtime error), or a value that is not valid
    /// for its item (`Bad integer`, `Bad real number`, a quoted value for a
    /// number, an unterminated quote).
    pub fn read(&mut self, slots: &mut [Slot]) -> Result<(), NjoyError> {
        let n = slots.len();
        let mut idx = 0usize;
        // true once a value has been taken and its separator is still owed
        let mut after_value = false;
        // list-directed read always consumes at least one record, even for no items
        if n == 0 {
            if self.pos < self.lines.len() {
                self.pos += 1;
            }
            return Ok(());
        }
        loop {
            if self.pos >= self.lines.len() {
                return Err(err(format!("end of file while reading item {} of {n}", idx + 1)));
            }
            let chars: Vec<char> = self.lines[self.pos].chars().collect();
            let rec = self.pos + 1;
            self.pos += 1;
            let mut p = 0usize;
            loop {
                if idx >= n {
                    return Ok(());
                }
                while p < chars.len() && is_blank(chars[p]) {
                    p += 1;
                }
                if p >= chars.len() {
                    break;
                }
                match chars[p] {
                    ',' => {
                        p += 1;
                        if after_value {
                            after_value = false;
                        } else {
                            idx += 1; // null value
                        }
                        continue;
                    }
                    '/' => return Ok(()),
                    _ => {}
                }
                // an optional repeat count `r*`
                let mut repeat = 1usize;
                let mut q = p;
                while q < chars.len() && chars[q].is_ascii_digit() {
                    q += 1;
                }
                if q > p && q < chars.len() && chars[q] == '*' {
                    let digits: String = chars[p..q].iter().collect();
                    repeat = digits.parse::<usize>().map_err(|_| err(format!("record {rec}: bad repeat count")))?;
                    if repeat == 0 {
                        return Err(err(format!("record {rec}: zero repeat count")));
                    }
                    p = q + 1;
                    let at_sep = p >= chars.len() || is_blank(chars[p]) || chars[p] == ',' || chars[p] == '/';
                    if at_sep {
                        // `r*`: r nulls
                        idx += repeat;
                        after_value = true;
                        if p < chars.len() && chars[p] == ',' {
                            p += 1;
                            after_value = false;
                        }
                        continue;
                    }
                }
                // the value itself
                let (text, quoted) = if chars[p] == '\'' || chars[p] == '"' {
                    let delim = chars[p];
                    p += 1;
                    let mut s = String::new();
                    loop {
                        if p >= chars.len() {
                            return Err(err(format!("record {rec}: unterminated quoted string")));
                        }
                        if chars[p] == delim {
                            if p + 1 < chars.len() && chars[p + 1] == delim {
                                s.push(delim);
                                p += 2;
                                continue;
                            }
                            p += 1;
                            break;
                        }
                        s.push(chars[p]);
                        p += 1;
                    }
                    (s, true)
                } else {
                    let st = p;
                    while p < chars.len() && !is_blank(chars[p]) && chars[p] != ',' && chars[p] != '/' {
                        p += 1;
                    }
                    (chars[st..p].iter().collect::<String>(), false)
                };
                for _ in 0..repeat {
                    if idx >= n {
                        break;
                    }
                    assign(&mut slots[idx], &text, quoted, idx, rec)?;
                    idx += 1;
                }
                after_value = true;
            }
            if idx >= n {
                return Ok(());
            }
            // end of record is a separator
            after_value = false;
        }
    }

    /// `read(nsysi,*) n ints`, each preset to `defaults`; returns the values.
    pub fn read_ints(&mut self, defaults: &[i32]) -> Result<Vec<i32>, NjoyError> {
        let mut s: Vec<Slot> = defaults.iter().map(|&d| Slot::Int(d)).collect();
        self.read(&mut s)?;
        Ok(s.iter().map(Slot::int).collect())
    }

    /// `read(nsysi,*) n reals`, each preset to `defaults`; returns the values.
    pub fn read_reals(&mut self, defaults: &[f64]) -> Result<Vec<f64>, NjoyError> {
        let mut s: Vec<Slot> = defaults.iter().map(|&d| Slot::Real(d)).collect();
        self.read(&mut s)?;
        Ok(s.iter().map(Slot::real).collect())
    }

    /// `read(nsysi,*) text` into a `character(len)` variable, trimmed.
    pub fn read_string(&mut self, len: usize) -> Result<String, NjoyError> {
        let mut s = [Slot::string(len, "")];
        self.read(&mut s)?;
        Ok(s[0].text().to_string())
    }
}

fn assign(slot: &mut Slot, text: &str, quoted: bool, idx: usize, rec: usize) -> Result<(), NjoyError> {
    match slot {
        Slot::Str { value, len } => {
            *value = pad(text, *len);
            Ok(())
        }
        Slot::Int(v) => {
            if quoted {
                return Err(err(format!("record {rec}: item {}: quoted value for an integer", idx + 1)));
            }
            let body = text.strip_prefix(['+', '-']).unwrap_or(text);
            if body.is_empty() || !body.bytes().all(|b| b.is_ascii_digit()) {
                return Err(err(format!("record {rec}: item {}: bad integer `{text}`", idx + 1)));
            }
            *v = text
                .parse::<i32>()
                .map_err(|_| err(format!("record {rec}: item {}: integer overflow `{text}`", idx + 1)))?;
            Ok(())
        }
        Slot::Real(v) => {
            if quoted {
                return Err(err(format!("record {rec}: item {}: quoted value for a real", idx + 1)));
            }
            *v = parse_fortran_real(text)
                .ok_or_else(|| err(format!("record {rec}: item {}: bad real number `{text}`", idx + 1)))?;
            Ok(())
        }
    }
}

/// A Fortran real: `[sign]digits[.digits][(e|d|q)[sign]digits]`, or the
/// letterless exponent `1.5+3`; also `inf`/`nan`.
pub fn parse_fortran_real(text: &str) -> Option<f64> {
    let t = text.trim();
    if t.is_empty() {
        return None;
    }
    let lower = t.to_ascii_lowercase();
    let body = lower.trim_start_matches(['+', '-']);
    if matches!(body, "inf" | "infinity" | "nan") {
        return lower.parse::<f64>().ok();
    }
    let mut s = String::with_capacity(lower.len() + 1);
    let b = lower.as_bytes();
    let mut seen_exp = false;
    let mut seen_digit = false;
    for (i, &c) in b.iter().enumerate() {
        match c {
            b'0'..=b'9' => {
                seen_digit = true;
                s.push(c as char);
            }
            b'.' => {
                if seen_exp {
                    return None;
                }
                s.push('.');
            }
            b'e' | b'd' | b'q' => {
                if seen_exp || !seen_digit {
                    return None;
                }
                seen_exp = true;
                s.push('e');
            }
            b'+' | b'-' => {
                if i == 0 || b[i - 1] == b'e' || b[i - 1] == b'd' || b[i - 1] == b'q' {
                    s.push(c as char);
                } else if !seen_exp && seen_digit {
                    // letterless exponent
                    seen_exp = true;
                    s.push('e');
                    s.push(c as char);
                } else {
                    return None;
                }
            }
            _ => return None,
        }
    }
    if !seen_digit || s.ends_with('e') || s.ends_with('+') || s.ends_with('-') {
        return None;
    }
    s.parse::<f64>().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn null_slash_and_repeat() {
        let mut r = CardReader::new("1,,3\n1 2 / 3\n2*7 9\n");
        assert_eq!(r.read_ints(&[-1, -2, -3]).unwrap(), vec![1, -2, 3]);
        assert_eq!(r.read_ints(&[-1, -2, -3]).unwrap(), vec![1, 2, -3]);
        assert_eq!(r.read_ints(&[-1, -2, -3]).unwrap(), vec![7, 7, 9]);
    }

    #[test]
    fn module_name_with_comment() {
        let mut r = CardReader::new("-- moder check\nreconr\n'stop'\n");
        assert_eq!(r.read_string(6).unwrap(), "--");
        assert_eq!(r.read_string(6).unwrap(), "reconr");
        assert_eq!(r.read_string(6).unwrap(), "stop");
    }

    #[test]
    fn eof_and_bad_values_are_errors() {
        assert!(CardReader::new("1 2").read_ints(&[0, 0, 0]).is_err());
        assert!(CardReader::new("1.5 2").read_ints(&[0, 0]).is_err());
        assert!(CardReader::new("1.5e 2").read_reals(&[0.0, 0.0]).is_err());
        assert_eq!(parse_fortran_real("1.5+3"), Some(1500.0));
        assert_eq!(parse_fortran_real("2q1"), Some(20.0));
    }
}
