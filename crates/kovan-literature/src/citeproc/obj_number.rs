// Part of the kovan port of citeproc-js (GitHub #790).
//
// Upstream:    citeproc-js, https://github.com/juris-m/citeproc-js
// Source:      src/obj_number.js (CSL.NumericBlob, CSL.Output.DefaultFormatter),
//              src/util_number.js (CSL.Util.Suffixator and CSL.Util.Romanizer
//              only: the two formatters small enough to be needed by
//              NumericBlob; the rest of util_number.js is another file's)
// Version:     2.4.63, commit 73bc1b44bc7d54d0bfec4e070fd27f5efe024ff9
// Copyright:   (c) 2009-2019 Frank Bennett
// Licence:     AGPL-3.0, taken from upstream's "CPAL-1.0 or AGPL-3.0-or-later"
//              (LICENSE at the commit above; see this crate's NOTICE).
// Modified:    2026-10-08, by the OUTRAM PARK contributors. This file is a
//              Rust translation (port) of the files named above, modified
//              from the original.
// No warranty: this program is distributed in the hope that it will be
//              useful, but WITHOUT ANY WARRANTY; without even the implied
//              warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR
//              PURPOSE. See the GNU Affero General Public License.

//! `CSL.NumericBlob`: a number (or a range of numbers) in the output queue,
//! and the number formatters (`format(num, gender)` objects) it carries.
//!
//! The *fields* of a `NumericBlob` live on [`Blob`](super::obj_blob::Blob)
//! (`kind == BlobKind::Numeric`); this file has its constructor
//! ([`new_numeric_blob`]), `setFormatter`, `checkNext` and `checkLast`.
//!
//! # Number formatters
//!
//! citeproc-js passes formatter *objects* around (`new CSL.Util.Suffixator(
//! CSL.SUFFIX_CHARS)`, `state.fun.romanizer`, ...). Here they are the enum
//! [`NumFormatter`]. A node builder that stores a formatter on a token puts it
//! in `token.extra["formatter"]` as JSON via [`NumFormatter::to_value`]
//! (`"default"`, `"romanizer"`, `"ordinalizer"`, `"long_ordinalizer"`, or
//! `{"type":"suffixator","slist":"a,b,..."}`); [`new_numeric_blob`] reads it
//! back with [`NumFormatter::from_value`].
//!
//! The formatters themselves are util_number.js's ([`Suffixator`],
//! [`Romanizer`], `Ordinalizer`, [`LongOrdinalizer`] in `util_number.rs`);
//! [`NumFormatter`] only names which one a blob carries. Its JSON names match
//! the ones the intermediate dump gives a token's `formatter`.

use serde_json::Value;

use super::formatters;
use super::js;
use super::obj_blob::{Blob, BlobContent, BlobId, BlobKind, Blobs};
use super::obj_token::Token;
use super::state::State;
use super::{CslResult, EngineError};

use super::load::{END, SEEN, START, SUCCESSOR, SUCCESSOR_OF_SUCCESSOR, SUPPRESS};
use super::load::SUFFIX_CHARS;
use super::util_number::{LongOrdinalizer, Romanizer, Suffixator};

/// A number formatter object: `CSL.Output.DefaultFormatter`,
/// `CSL.Util.Suffixator`, `CSL.Util.Romanizer`, `CSL.Util.Ordinalizer`,
/// `CSL.Util.LongOrdinalizer`.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum NumFormatter {
    /// `CSL.Output.DefaultFormatter`: `num.toString()`.
    #[default]
    Default,
    /// `CSL.Util.Suffixator`: `a, b, ... z, aa, ab, ...`; `slist` is the
    /// split `CSL.SUFFIX_CHARS` (or whatever list was passed).
    Suffixator { slist: Vec<String> },
    /// `CSL.Util.Romanizer`.
    Romanizer,
    /// `CSL.Util.Ordinalizer` (`state.fun.ordinalizer`).
    Ordinalizer,
    /// `CSL.Util.LongOrdinalizer` (`state.fun.long_ordinalizer`).
    LongOrdinalizer,
}

/// A `num` argument: JS passes a number, or (util_number.js) a string.
#[derive(Debug, Clone, PartialEq)]
pub enum NumArg {
    /// A JS number (always an integer here).
    Number(i64),
    /// A JS string (`num.value` that is not a plain integer, e.g. `"2a"`).
    Text(String),
}

impl NumFormatter {
    /// `new CSL.Util.Suffixator(slist)`; `None` is the `!slist` default.
    pub fn suffixator(slist: Option<&str>) -> NumFormatter {
        let s = match slist {
            Some(s) if !s.is_empty() => s,
            _ => SUFFIX_CHARS,
        };
        NumFormatter::Suffixator {
            slist: s.split(',').map(str::to_string).collect(),
        }
    }

    /// Read a formatter back from the JSON form a token's `extra` holds
    /// (see the module docs). `None` for anything else (JS: falsy).
    pub fn from_value(v: &Value) -> Option<NumFormatter> {
        match v {
            Value::String(s) => match s.as_str() {
                "default" => Some(NumFormatter::Default),
                "romanizer" => Some(NumFormatter::Romanizer),
                "ordinalizer" => Some(NumFormatter::Ordinalizer),
                "long_ordinalizer" => Some(NumFormatter::LongOrdinalizer),
                "suffixator" => Some(NumFormatter::suffixator(None)),
                _ => None,
            },
            Value::Object(o) => match o.get("type").and_then(Value::as_str) {
                Some("suffixator") => Some(NumFormatter::suffixator(
                    o.get("slist").and_then(Value::as_str),
                )),
                Some(other) => NumFormatter::from_value(&Value::String(other.to_string())),
                None => None,
            },
            _ => None,
        }
    }

    /// The JSON form [`NumFormatter::from_value`] reads.
    pub fn to_value(&self) -> Value {
        match self {
            NumFormatter::Default => Value::String("default".into()),
            NumFormatter::Romanizer => Value::String("romanizer".into()),
            NumFormatter::Ordinalizer => Value::String("ordinalizer".into()),
            NumFormatter::LongOrdinalizer => Value::String("long_ordinalizer".into()),
            NumFormatter::Suffixator { slist } => {
                let mut o = serde_json::Map::new();
                o.insert("type".into(), Value::String("suffixator".into()));
                o.insert("slist".into(), Value::String(slist.join(",")));
                Value::Object(o)
            }
        }
    }

    /// `formatter.format(num, gender)`: `DefaultFormatter.prototype.format`,
    /// or the util_number.js formatter objects ([`Suffixator`],
    /// [`Romanizer`], `state.fun.ordinalizer`, [`LongOrdinalizer`]).
    ///
    /// A `Text` number with the suffixator or the romanizer does arithmetic
    /// on a string in JS (and loops forever in the suffixator); that is an
    /// `Err` here. The ordinalizers `parseInt` their argument, as upstream.
    pub fn format(&self, state: &mut State, num: &NumArg, gender: Option<&str>) -> CslResult<String> {
        let as_value = |num: &NumArg| match num {
            NumArg::Number(n) => Value::from(*n),
            NumArg::Text(t) => Value::String(t.clone()),
        };
        match (self, num) {
            (NumFormatter::Default, NumArg::Number(n)) => Ok(n.to_string()),
            (NumFormatter::Default, NumArg::Text(t)) => Ok(t.clone()),
            (NumFormatter::Suffixator { slist }, NumArg::Number(n)) => Ok(Suffixator {
                slist: slist.clone(),
            }
            .format(*n)),
            (NumFormatter::Romanizer, NumArg::Number(n)) => {
                Romanizer::default().format(&Value::from(*n))
            }
            (NumFormatter::Ordinalizer, num) => {
                let ordinalizer = state.fun.ordinalizer.clone();
                ordinalizer.format(state, &as_value(num), gender)
            }
            (NumFormatter::LongOrdinalizer, num) => {
                LongOrdinalizer::format(state, &as_value(num), gender)
            }
            (_, NumArg::Text(_)) => Err(EngineError::Csl(
                "number formatter applied to a non-numeric string".into(),
            )),
        }
    }
}

/// `new CSL.NumericBlob(state, particle, num, mother_token, id)`: build the
/// blob and add it to the arena. `particle` is the JS `particle` argument
/// (`false`/`undefined` is `None`); `mother_token` carries the styling.
///
/// Reads these token properties from `mother_token.extra`: `gender`,
/// `successor_prefix`, `range_prefix`, `splice_prefix`, `formatter` (see the
/// module docs for the formatter form).
pub fn new_numeric_blob(
    state: &mut State,
    particle: Option<&str>,
    num: NumArg,
    mother_token: Option<&Token>,
    id: Option<&str>,
) -> CslResult<BlobId> {
    let mut b = Blob {
        kind: BlobKind::Numeric,
        id: id.map(str::to_string),
        particle: particle.map(str::to_string),
        status: Some(START),
        ..Blob::default()
    };
    let blobs_text = match &num {
        NumArg::Number(n) => {
            b.num = Some(*n);
            n.to_string()
        }
        NumArg::Text(t) => {
            b.num_text = Some(t.clone());
            t.clone()
        }
    };
    b.blobs = BlobContent::Text(blobs_text);
    match mother_token {
        Some(mt) => {
            if js::truthy_opt(mt.strings.get("text-case")) {
                let text_case = mt.string("text-case");
                // this.particle = Formatters[textCase](state, this.particle)
                let p = b.particle.clone().unwrap_or_default();
                let p2 = formatters::apply(state, &text_case, &p)?;
                b.particle = if p2.is_empty() { None } else { Some(p2) };
                if let BlobContent::Text(t) = &b.blobs {
                    let t2 = formatters::apply(state, &text_case, t)?;
                    b.blobs = BlobContent::Text(t2);
                }
            }
            b.gender = mt
                .extra
                .get("gender")
                .and_then(|v| v.as_str())
                .map(str::to_string);
            b.decorations = mt.decorations.clone();
            b.strings
                .insert("prefix".into(), Value::String(mt.string("prefix")));
            b.strings
                .insert("suffix".into(), Value::String(mt.string("suffix")));
            if let Some(tc) = mt.strings.get("text-case") {
                b.strings.insert("text-case".into(), tc.clone());
            }
            b.successor_prefix = mt.extra.get("successor_prefix").map(js::to_js_string);
            b.range_prefix = mt.extra.get("range_prefix").map(js::to_js_string);
            b.splice_prefix = mt.extra.get("splice_prefix").map(js::to_js_string);
            let f = mt.extra.get("formatter").and_then(NumFormatter::from_value);
            let f = f.unwrap_or_default();
            b.formatter = Some(f.clone());
            b.numeric_type = Some(f.format(state, &NumArg::Number(1), None)?);
        }
        None => {
            b.decorations = Vec::new();
            b.strings
                .insert("prefix".into(), Value::String(String::new()));
            b.strings
                .insert("suffix".into(), Value::String(String::new()));
            b.successor_prefix = Some(String::new());
            b.range_prefix = Some(String::new());
            b.splice_prefix = Some(String::new());
            b.formatter = Some(NumFormatter::Default);
        }
    }
    // this.alldecor = [] (NOT [this.decorations] as on CSL.Blob).
    b.alldecor = Vec::new();
    Ok(state.blobs.add(b))
}

/// `CSL.NumericBlob.prototype.setFormatter`.
pub fn set_formatter(state: &mut State, blob: BlobId, formatter: NumFormatter) -> CslResult<()> {
    let ty = formatter.format(state, &NumArg::Number(1), None)?;
    let b = state.blobs.get_mut(blob);
    b.formatter = Some(formatter);
    b.numeric_type = Some(ty);
    Ok(())
}

/// `CSL.NumericBlob.prototype.checkNext(next, start)`. `next` is `None` for
/// JS `undefined` and for a bare string (neither has `.num`, and neither is
/// `"object" === typeof`).
pub fn check_next(blobs: &mut Blobs, this: BlobId, next: Option<BlobId>, start: bool) {
    let this_num = blobs.get(this).num;
    let this_next_num = this_num.map(|n| n + 1);
    if start {
        blobs.get_mut(this).status = Some(START);
        if let Some(nx) = next {
            if blobs.get(nx).num.is_some() && blobs.get(nx).num == this_next_num {
                blobs.get_mut(nx).status = Some(SUCCESSOR);
            } else {
                blobs.get_mut(nx).status = Some(SEEN);
            }
        }
    } else {
        // ! next || !next.num || this.type !== next.type || next.num !== (this.num + 1)
        let cond = match next {
            None => true,
            Some(nx) => {
                let n = blobs.get(nx);
                !n.num.map(|v| v != 0).unwrap_or(false)
                    || blobs.get(this).numeric_type != n.numeric_type
                    || n.num != this_next_num
            }
        };
        let status = blobs.get(this).status;
        if cond {
            if status == Some(SUCCESSOR_OF_SUCCESSOR) {
                blobs.get_mut(this).status = Some(END);
            }
            if let Some(nx) = next {
                blobs.get_mut(nx).status = Some(SEEN);
            }
        } else if let Some(nx) = next {
            // next number is in the sequence
            if status == Some(START) || status == Some(SEEN) {
                blobs.get_mut(nx).status = Some(SUCCESSOR);
            } else if status == Some(SUCCESSOR) || status == Some(SUCCESSOR_OF_SUCCESSOR) {
                let has_range_prefix = blobs
                    .get(this)
                    .range_prefix
                    .as_deref()
                    .map(|s| !s.is_empty())
                    .unwrap_or(false);
                if has_range_prefix {
                    blobs.get_mut(nx).status = Some(SUCCESSOR_OF_SUCCESSOR);
                    blobs.get_mut(this).status = Some(SUPPRESS);
                } else {
                    blobs.get_mut(nx).status = Some(SUCCESSOR);
                }
            }
        }
    }
}

/// `CSL.NumericBlob.prototype.checkLast(last)`: adjusts the final non-range
/// join; `last` is `None` when it has no `.num` (a bare string). Returns
/// whether it changed the status.
pub fn check_last(blobs: &mut Blobs, this: BlobId, last: Option<BlobId>) -> bool {
    let b = blobs.get(this);
    let last_num = last.and_then(|l| blobs.get(l).num);
    let this_prev = b.num.map(|n| n - 1);
    let cond = b.status == Some(SEEN)
        || ((last_num.is_none() || last_num != this_prev) && b.status == Some(SUCCESSOR));
    if cond {
        blobs.get_mut(this).status = Some(SUCCESSOR);
        true
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suffixator_and_romanizer_follow_upstream() {
        let mut st = State::default();
        let s = NumFormatter::suffixator(None);
        let mut f = |n| s.format(&mut st, &NumArg::Number(n), None).unwrap();
        assert_eq!(f(0), "a");
        assert_eq!(f(1), "b");
        assert_eq!(f(25), "z");
        assert_eq!(f(26), "aa");
        let r = NumFormatter::Romanizer;
        assert_eq!(
            r.format(&mut st, &NumArg::Number(1994), None).unwrap(),
            "mcmxciv"
        );
        assert_eq!(r.format(&mut st, &NumArg::Number(6000), None).unwrap(), "");
    }
}
