// Part of the kovan port of citeproc-js (GitHub #790).
//
// Upstream:    citeproc-js, https://github.com/juris-m/citeproc-js
// Source:      src/obj_blob.js, src/obj_number.js (CSL.NumericBlob: its
//              fields here; its methods in obj_number.rs)
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

//! `CSL.Blob` and `CSL.NumericBlob`: the nodes of the output queue's tree
//! (PORTING.md §3).
//!
//! JS blobs are shared, mutated objects: the queue's `current` stack, a
//! group's `output_tip`, and a parent's `blobs` array all point at the same
//! one. Here every blob lives in one arena, [`Blobs`] (`state.blobs`), and is
//! referred to by its [`BlobId`]. A JS array that mixes strings and blobs is a
//! `Vec<BlobChild>`.
//!
//! The arena only grows during one top-level engine call; the engine clears it
//! between calls (nothing outlives a call except rendered strings).

use super::js::Obj;
use super::obj_token::{Decoration, Token};
use serde_json::Value;

/// An index into the blob arena.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BlobId(pub usize);

/// An element of a JS blob array: a blob, or (rarely: `append(str,
/// "literal")`, the joined output of `string()`) a bare string.
#[derive(Debug, Clone, PartialEq)]
pub enum BlobChild {
    Blob(BlobId),
    Str(String),
}

/// `blob.blobs`: either a string (a leaf) or an array of children.
#[derive(Debug, Clone, PartialEq)]
pub enum BlobContent {
    Text(String),
    List(Vec<BlobChild>),
}

impl Default for BlobContent {
    fn default() -> Self {
        BlobContent::List(Vec::new())
    }
}

/// Which JS constructor made the blob.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BlobKind {
    /// `CSL.Blob`.
    #[default]
    Plain,
    /// `CSL.NumericBlob` (JS code tests it as `"number" === typeof blob.num`).
    Numeric,
    /// The output queue's root array (`queue.queue`, a plain JS array, not a
    /// blob). Kept as a blob so `current` can hold it; code that tests
    /// `drip.length` (array) vs `drip.blobs` (blob) checks this kind.
    RootArray,
}

/// `CSL.Blob` / `CSL.NumericBlob`. Fields a plain blob never sets stay at
/// their defaults (`None`, `""`).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Blob {
    pub kind: BlobKind,
    /// `levelname`.
    pub levelname: Option<String>,
    /// `strings`: prefix, suffix, delimiter, text-case, first_blob, ...
    pub strings: Obj,
    /// `decorations`.
    pub decorations: Vec<Decoration>,
    /// `alldecor`: this blob's decorations followed by every ancestor's, as
    /// lists (JS `blob.alldecor = blob.alldecor.concat(this.alldecor)`).
    pub alldecor: Vec<Vec<Decoration>>,
    /// `blobs`.
    pub blobs: BlobContent,
    /// `punctuation_in_quote`.
    pub punctuation_in_quote: Option<bool>,
    /// `new_locale` / `old_locale` (cite-language switching).
    pub new_locale: Option<String>,
    pub old_locale: Option<String>,
    /// `particle` (NumericBlob; also set on name blobs).
    pub particle: Option<String>,
    // ---- CSL.NumericBlob ----
    /// `num`.
    pub num: Option<i64>,
    /// `id`: the item id, for cross-item joining.
    pub id: Option<String>,
    /// `status`: CSL.START / CSL.END / CSL.SUCCESSOR / CSL.SEEN / ...
    pub status: Option<i64>,
    /// `gender`.
    pub gender: Option<String>,
    /// `successor_prefix`, `range_prefix`, `splice_prefix`.
    pub successor_prefix: Option<String>,
    pub range_prefix: Option<String>,
    pub splice_prefix: Option<String>,
    /// `suppress_splice_prefix`.
    pub suppress_splice_prefix: Option<bool>,
    /// `formatter`: the number formatter's name (`"default"`, `"roman"`, ...);
    /// see obj_number.rs.
    pub formatter: Option<String>,
    /// `type` (`this.formatter.format(1)`).
    pub numeric_type: Option<String>,
    /// `UGLY_DELIMITER_SUPPRESS_HACK`.
    pub ugly_delimiter_suppress_hack: bool,
    /// Any other property a caller hangs on the blob (`isInverted`,
    /// `isInstitution`, ...), by JS name.
    pub extra: Obj,
}

impl Blob {
    /// `new CSL.Blob(str, token, levelname)`. `text` is the `str` argument:
    /// `Some` for a leaf (JS: a string `str`), `None` for an empty list.
    /// (JS `new CSL.Blob(blobObject)` — a list holding one blob — is
    /// [`Blobs::new_blob_wrapping`].)
    pub fn new(text: Option<&str>, token: Option<&Token>, levelname: Option<&str>) -> Blob {
        let mut b = Blob {
            levelname: levelname.map(str::to_string),
            ..Blob::default()
        };
        match token {
            Some(t) => {
                b.strings.insert("prefix".into(), Value::String(String::new()));
                b.strings.insert("suffix".into(), Value::String(String::new()));
                for (k, v) in &t.strings {
                    b.strings.insert(k.clone(), v.clone());
                }
                b.decorations = t.decorations.clone();
            }
            None => {
                b.strings.insert("prefix".into(), Value::String(String::new()));
                b.strings.insert("suffix".into(), Value::String(String::new()));
                b.strings.insert("delimiter".into(), Value::String(String::new()));
            }
        }
        b.blobs = match text {
            Some(s) => BlobContent::Text(s.to_string()),
            None => BlobContent::List(Vec::new()),
        };
        b.alldecor = vec![b.decorations.clone()];
        b
    }

    /// `blob.strings[key]` as a string, `""` when absent.
    pub fn string(&self, key: &str) -> String {
        self.strings
            .get(key)
            .map(super::js::to_js_string)
            .unwrap_or_default()
    }

    /// `"string" === typeof blob.blobs`.
    pub fn is_leaf(&self) -> bool {
        matches!(self.blobs, BlobContent::Text(_))
    }
}

/// The blob arena (`state.blobs`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Blobs {
    pub arena: Vec<Blob>,
}

impl Blobs {
    /// Store a blob, returning its id.
    pub fn add(&mut self, blob: Blob) -> BlobId {
        self.arena.push(blob);
        BlobId(self.arena.len() - 1)
    }

    /// `new CSL.Blob(childBlob)`: a plain blob whose list holds `child`.
    pub fn new_blob_wrapping(&mut self, child: BlobId, token: Option<&Token>, levelname: Option<&str>) -> BlobId {
        let mut b = Blob::new(None, token, levelname);
        b.blobs = BlobContent::List(vec![BlobChild::Blob(child)]);
        self.add(b)
    }

    /// The blob behind an id.
    pub fn get(&self, id: BlobId) -> &Blob {
        &self.arena[id.0]
    }

    /// The blob behind an id, mutably.
    pub fn get_mut(&mut self, id: BlobId) -> &mut Blob {
        &mut self.arena[id.0]
    }

    /// `CSL.Blob.prototype.push(blob)`: append `child` to `parent`'s list
    /// after appending the parent's `alldecor` to the child's. Upstream
    /// errors when `parent` is a leaf.
    pub fn push(&mut self, parent: BlobId, child: BlobId) -> Result<(), crate::citeproc::EngineError> {
        if self.get(parent).is_leaf() {
            return Err(crate::citeproc::EngineError::Csl(
                "Attempt to push blob onto string object".into(),
            ));
        }
        let parent_decor = self.get(parent).alldecor.clone();
        self.get_mut(child).alldecor.extend(parent_decor);
        if let BlobContent::List(l) = &mut self.get_mut(parent).blobs {
            l.push(BlobChild::Blob(child));
        }
        Ok(())
    }

    /// Forget every blob (between top-level engine calls).
    pub fn clear(&mut self) {
        self.arena.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::citeproc::obj_token::TokenType;

    #[test]
    fn blobs_copy_token_strings_and_chain_decorations() {
        let mut t = Token::new("text", TokenType::Singleton);
        t.set_string("prefix", "(");
        t.decorations.push(Decoration::new("@font-style", "italic"));
        let mut arena = Blobs::default();
        let parent = arena.add(Blob::new(None, Some(&t), None));
        let child = arena.add(Blob::new(Some("x"), None, None));
        arena.push(parent, child).unwrap();
        assert_eq!(arena.get(parent).string("prefix"), "(");
        assert_eq!(arena.get(child).string("delimiter"), "");
        assert_eq!(arena.get(child).alldecor.len(), 2);
        assert!(arena.push(child, parent).is_err());
    }
}
