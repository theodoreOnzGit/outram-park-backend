// Part of the kovan port of citeproc-js (GitHub #790).
//
// Upstream:    citeproc-js, https://github.com/juris-m/citeproc-js
// Source:      src/util_publishers.js
// Version:     2.4.63, commit 73bc1b44bc7d54d0bfec4e070fd27f5efe024ff9
// Copyright:   (c) 2009-2019 Frank Bennett
// Licence:     AGPL-3.0, taken from upstream's "CPAL-1.0 or AGPL-3.0-or-later"
//              (LICENSE at the commit above; see this crate's NOTICE).
// Modified:    2026-10-08, by the OUTRAM PARK contributors. This file is a
//              Rust translation (port) of the file named above, modified
//              from the original.
// No warranty: this program is distributed in the hope that it will be
//              useful, but WITHOUT ANY WARRANTY; without even the implied
//              warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR
//              PURPOSE. See the GNU Affero General Public License.

//! Port of `src/util_publishers.js`: `CSL.PublisherOutput`, which joins
//! parallel `publisher` and `publisher-place` lists (`"A; B"`, `"X; Y"`)
//! under `cs:group` elements that carry `subgroup-delimiter`.
//!
//! **DEVIATION(D9) — upstream cannot run this.** In citeproc-js
//! `PublisherOutput.prototype._join` is assigned
//! `CSL.NameOutput.prototype._join`, which begins with
//! `this._purgeEmptyBlobs(blobs)`; `PublisherOutput` has no such method, so
//! `render()` always throws `this._purgeEmptyBlobs is not a function` at
//! `composePublishers` (after `clearVars`, `composeAndBlob` and
//! `composeElements` ran, leaving `state.publisherOutput` set). No fixture of
//! the CSL test suite reaches it. We do what the code is written to do: join
//! each publisher/place pair with the group delimiter and the pairs with
//! `subgroup-delimiter` and the `and` blob, using the existing purge-empty
//! join ([`super::util_names_join::join_blobs`]). One consequence kept from
//! upstream's `_join(blobs, delimiter, finalJoin)`: it has three parameters, so
//! `joinPublishers` passes the `and` blob `single` and the fourth argument
//! (`multiple`) is never read. See `DEVIATIONS.md` D9.

use super::obj_blob::BlobId;
use super::obj_token::{Token, TokenType};
use super::queue::FormatRef;
use super::state::State;
use super::util_names_join::join_blobs;
use super::util_names_output::{q_append_blob, q_append_str, q_pop_blob, BlobPair};
use super::{CslResult, EngineError};

/// `CSL.PublisherOutput`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PublisherOutput {
    /// `this.group_tok`.
    pub group_tok: Token,
    /// `this.varlist`.
    pub varlist: Vec<String>,
    /// `this["publisher-list"]` (strings, until `composeElements`).
    pub publisher_list: Vec<String>,
    /// `this["publisher-place-list"]`.
    pub publisher_place_list: Vec<String>,
    /// `this["publisher-token"]` and `this["publisher-place-token"]`.
    pub publisher_token: Option<Token>,
    pub publisher_place_token: Option<Token>,
    /// `this.and_blob`.
    pub and_blob: BlobPair,
    /// The two lists after `composeElements` replaced the strings by blobs.
    publisher_blobs: Vec<Option<BlobId>>,
    publisher_place_blobs: Vec<Option<BlobId>>,
}

impl PublisherOutput {
    /// `new CSL.PublisherOutput(state, group_tok)` followed by the two list
    /// assignments of node_group.js:174-176.
    pub fn new(
        group_tok: &Token,
        publisher_list: Vec<String>,
        publisher_place_list: Vec<String>,
    ) -> PublisherOutput {
        PublisherOutput {
            group_tok: group_tok.clone(),
            varlist: Vec::new(),
            publisher_list,
            publisher_place_list,
            ..PublisherOutput::default()
        }
    }

    /// `CSL.PublisherOutput.prototype.render()`.
    pub fn render(&mut self, st: &mut State) -> CslResult<()> {
        self.clear_vars(st);
        self.compose_and_blob(st)?;
        self.compose_elements(st)?;
        self.compose_publishers(st)?;
        self.join_publishers(st)
    }

    /// `CSL.PublisherOutput.prototype.composeAndBlob()`.
    pub fn compose_and_blob(&mut self, st: &mut State) -> CslResult<()> {
        let mut and_term: Option<String> = None;
        match self.group_tok.string_opt("and").as_deref() {
            Some("text") => and_term = st.get_term("and", None, None, None, None, false)?,
            Some("symbol") => and_term = Some("&".to_string()),
            _ => {}
        }
        let mut tok = Token::new("", TokenType::Start);
        tok.set_string("suffix", " ");
        tok.set_string("prefix", " ");
        self.append_term(st, and_term.as_deref(), &tok)?;
        let no_delim = q_pop_blob(st)?;

        tok.strings.insert(
            "prefix".into(),
            self.group_tok
                .strings
                .get("subgroup-delimiter")
                .cloned()
                .unwrap_or(serde_json::Value::Null),
        );
        self.append_term(st, and_term.as_deref(), &tok)?;
        let with_delim = q_pop_blob(st)?;

        self.and_blob = BlobPair::default();
        if and_term.is_some() {
            match self
                .group_tok
                .string_opt("subgroup-delimiter-precedes-last")
                .as_deref()
            {
                Some("always") => self.and_blob.single = with_delim,
                Some("never") => {
                    self.and_blob.single = no_delim;
                    self.and_blob.multiple = no_delim;
                }
                _ => {
                    self.and_blob.single = no_delim;
                    self.and_blob.multiple = with_delim;
                }
            }
        }
        Ok(())
    }

    /// `state.output.append(term, tok, true)` where `term` may be `false`.
    fn append_term(&self, st: &mut State, term: Option<&str>, tok: &Token) -> CslResult<()> {
        match term {
            Some(t) => {
                q_append_str(st, Some(t), FormatRef::Token(tok.clone()), true)?;
            }
            None => {
                q_append_blob(st, None, FormatRef::Token(tok.clone()), true)?;
            }
        }
        Ok(())
    }

    /// `CSL.PublisherOutput.prototype.composeElements()`: render each
    /// publisher and each place with its token.
    pub fn compose_elements(&mut self, st: &mut State) -> CslResult<()> {
        for varname in ["publisher", "publisher-place"] {
            let n = self.publisher_list.len();
            let mut blobs: Vec<Option<BlobId>> = Vec::new();
            for j in 0..n {
                let list = if varname == "publisher" {
                    &self.publisher_list
                } else {
                    &self.publisher_place_list
                };
                let s = list.get(j).cloned();
                let tok = if varname == "publisher" {
                    &self.publisher_token
                } else {
                    &self.publisher_place_token
                };
                let fr = match tok {
                    Some(t) => FormatRef::Token(t.clone()),
                    None => FormatRef::None,
                };
                // notSerious
                q_append_str(st, s.as_deref(), fr, true)?;
                blobs.push(q_pop_blob(st)?);
            }
            if varname == "publisher" {
                self.publisher_blobs = blobs;
            } else {
                self.publisher_place_blobs = blobs;
            }
        }
        Ok(())
    }

    /// `CSL.PublisherOutput.prototype.composePublishers()`: pair each
    /// publisher with its place, in the order the style rendered them
    /// (`varlist`), joined by the group delimiter.
    ///
    /// DEVIATION(D9): citeproc-js throws in `_join` (module docs).
    pub fn compose_publishers(&mut self, st: &mut State) -> CslResult<()> {
        let delimiter = self
            .group_tok
            .string_opt("delimiter")
            .unwrap_or_default();
        let (Some(first), Some(second)) = (self.varlist.first(), self.varlist.get(1)) else {
            if self.publisher_list.is_empty() {
                return Ok(());
            }
            return Err(EngineError::BadInput(
                "Cannot read properties of undefined (reading '0')".to_string(),
            ));
        };
        let (first, second) = (first.clone(), second.clone());
        let list_of = |po: &PublisherOutput, name: &str| -> Vec<Option<BlobId>> {
            if name == "publisher" {
                po.publisher_blobs.clone()
            } else {
                po.publisher_place_blobs.clone()
            }
        };
        let (a, b) = (list_of(self, &first), list_of(self, &second));
        let mut joined: Vec<Option<BlobId>> = Vec::new();
        for i in 0..self.publisher_list.len() {
            let pair = vec![
                a.get(i).copied().flatten(),
                b.get(i).copied().flatten(),
            ];
            joined.push(join_blobs(st, pair, &delimiter, None)?);
        }
        // `this["publisher-list"][i] = ...`
        self.publisher_blobs = joined;
        Ok(())
    }

    /// `CSL.PublisherOutput.prototype.joinPublishers()`: join the pairs with
    /// `subgroup-delimiter` (and the `and` blob) and append the result.
    ///
    /// DEVIATION(D9): citeproc-js throws in `_join` (module docs).
    pub fn join_publishers(&mut self, st: &mut State) -> CslResult<()> {
        let delimiter = self
            .group_tok
            .string_opt("subgroup-delimiter")
            .unwrap_or_default();
        // `_join` takes three parameters: `and_blob.multiple` is never read.
        let publishers = join_blobs(
            st,
            self.publisher_blobs.clone(),
            &delimiter,
            self.and_blob.single,
        )?;
        q_append_blob(st, publishers, FormatRef::Name("literal".into()), false)?;
        Ok(())
    }

    /// `CSL.PublisherOutput.prototype.clearVars()`.
    pub fn clear_vars(&mut self, st: &mut State) {
        st.tmp.publisher_list = false;
        st.tmp.publisher_token = None;
        st.tmp.publisher_place_token = None;
    }
}

#[cfg(test)]
mod tests {
    //! `CSL.PublisherOutput.prototype.render()`. citeproc-js 2.4.63 throws
    //! `TypeError: this._purgeEmptyBlobs is not a function` (confirmed
    //! 2026-10-08 with `scripts/csl-units/names_e2e.cjs`'s engine setup and a
    //! `cs:group` with `subgroup-delimiter` over an item with
    //! `publisher: "A; B"` and `publisher-place: "X; Y"`); DEVIATION(D9): the
    //! port joins the pairs instead. The rendered text is pinned end to end in
    //! `deviation_tests_d6_d12.rs` (D9).
    use super::*;
    use crate::citeproc::obj_token::TokenType;

    #[test]
    fn render_joins_the_pairs_instead_of_throwing_as_upstream_does_d9() {
        let mut st = State::default();
        let mut group = Token::new("group", TokenType::Start);
        group.set_string("subgroup-delimiter", "; ");
        group.set_string("delimiter", ", ");
        let mut po = PublisherOutput::new(
            &group,
            vec!["A".to_string(), "B".to_string()],
            vec!["X".to_string(), "Y".to_string()],
        );
        po.varlist = vec!["publisher".to_string(), "publisher-place".to_string()];
        assert_eq!(po.render(&mut st), Ok(()));
        // The two list elements were rendered into blobs, then paired.
        assert_eq!(po.publisher_blobs.len(), 2);
        assert_eq!(po.publisher_place_blobs.len(), 2);
        assert!(po.publisher_blobs.iter().all(Option::is_some));
    }
}
