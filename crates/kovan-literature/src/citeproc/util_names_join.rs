// Part of the kovan port of citeproc-js (GitHub #790).
//
// Upstream:    citeproc-js, https://github.com/juris-m/citeproc-js
// Source:      src/util_names_join.js
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

//! Port of `src/util_names_join.js`: joining rendered names with
//! delimiters, `and`, et-al and ellipsis
//! (`CSL.NameOutput.prototype._join` and its callers).

use serde_json::Value;

use super::obj_blob::BlobId;
use super::queue::{FormatRef, QueueId};
use super::state::State;
use super::util_names_output::{
    blob_is_empty, clone_blob_deep, q_append_blob, q_close_level, q_open_level, q_pop_blob,
    BlobPair, NameOutput,
};
use super::{CslResult, EngineError};

/// `CSL.NameOutput.prototype._purgeEmptyBlobs(blobs)`: drop `false`/empty
/// entries (`!blobs[i] || blobs[i].length === 0 || !blobs[i].blobs.length`).
pub fn purge_empty_blobs(st: &State, blobs: Vec<Option<BlobId>>) -> Vec<Option<BlobId>> {
    blobs
        .into_iter()
        .filter(|b| match b {
            Some(id) => !blob_is_empty(st, *id),
            None => false,
        })
        .collect()
}

/// `CSL.NameOutput.prototype._join` (also `CSL.PublisherOutput.prototype._join`,
/// which citeproc-js cannot run; DEVIATION(D9): we do, see util_publishers.rs).
pub fn join_blobs(
    st: &mut State,
    blobs: Vec<Option<BlobId>>,
    delimiter: &str,
    final_join: Option<BlobId>,
) -> CslResult<Option<BlobId>> {
    let mut blobs = purge_empty_blobs(st, blobs);
    if blobs.is_empty() {
        return Ok(None);
    }
    if blobs.len() > 1 {
        if blobs.len() == 2 {
            match final_join {
                None => {
                    if let Some(b0) = blobs[0] {
                        let s = st.blobs.get(b0).string("suffix");
                        st.blobs
                            .get_mut(b0)
                            .set_string("suffix", &format!("{s}{delimiter}"));
                    }
                }
                Some(fj) => {
                    blobs = vec![blobs[0], Some(fj), blobs[1]];
                }
            }
        } else {
            let offset = if final_join.is_some() { 1 } else { 0 };
            let blob = blobs.pop().flatten();
            let n = blobs.len() - offset;
            for b in blobs.iter().take(n).flatten() {
                let s = st.blobs.get(*b).string("suffix");
                st.blobs
                    .get_mut(*b)
                    .set_string("suffix", &format!("{s}{delimiter}"));
            }
            blobs.push(final_join);
            blobs.push(blob);
        }
    }

    q_open_level(st, FormatRef::None)?;
    // Delimiter is applied from separately saved source in this case,
    // for discriminate application of single and multiple joins.
    for b in blobs {
        q_append_blob(st, b, FormatRef::None, true)?;
    }
    q_close_level(st)?;
    q_pop_blob(st)
}

impl NameOutput {
    /// `CSL.NameOutput.prototype._purgeEmptyBlobs(blobs)`.
    pub fn purge_empty_blobs(&self, st: &State, blobs: Vec<Option<BlobId>>) -> Vec<Option<BlobId>> {
        purge_empty_blobs(st, blobs)
    }

    /// `CSL.NameOutput.prototype.joinPersons(blobs, pos, j, tokenname)`:
    /// join the names of one list (`j` is `None` for freeters, the index of
    /// the institution for affiliated persons).
    pub fn join_persons(
        &mut self,
        st: &mut State,
        blobs: Vec<Option<BlobId>>,
        pos: i64,
        j: Option<usize>,
    ) -> CslResult<Option<BlobId>> {
        let blobs = purge_empty_blobs(st, blobs);
        let spec = self.etal_spec(&pos.to_string()).cloned().ok_or_else(|| {
            super::load::type_error("Cannot read properties of undefined (reading 'freeters')")
        })?;
        let which = match j {
            None => spec.freeters,
            Some(j) => spec.persons.get(j).copied().unwrap_or(-1),
        };
        if which == 1 {
            self.join_et_al(st, blobs)
        } else if which == 2 {
            self.join_ellipsis(st, blobs)
        } else if !st.tmp.sort_key_flag {
            self.join_and(st, blobs)
        } else {
            let delim = self.name_delimiter(st)?;
            self.join(st, blobs, &delim, None)
        }
    }

    /// `this.state.inheritOpt(this.name, "delimiter", "name-delimiter", ", ")`.
    pub(super) fn name_delimiter(&self, st: &State) -> CslResult<String> {
        Ok(self
            .inherit_name_opt(
                st,
                "delimiter",
                Some("name-delimiter"),
                Some(Value::String(", ".into())),
            )?
            .map(|v| crate::citeproc::js::to_js_string(&v))
            .unwrap_or_default())
    }

    /// `CSL.NameOutput.prototype.joinInstitutionSets(blobs, pos)`.
    pub fn join_institution_sets(
        &mut self,
        st: &mut State,
        blobs: Vec<Option<BlobId>>,
        pos: i64,
    ) -> CslResult<Option<BlobId>> {
        let blobs = purge_empty_blobs(st, blobs);
        let spec = self.etal_spec(&pos.to_string()).cloned().ok_or_else(|| {
            super::load::type_error("Cannot read properties of undefined (reading 'institutions')")
        })?;
        if spec.institutions == 1 {
            self.join_et_al(st, blobs)
        } else if spec.institutions == 2 {
            self.join_ellipsis(st, blobs)
        } else {
            self.join_and(st, blobs)
        }
    }

    /// `CSL.NameOutput.prototype.joinPersonsAndInstitutions(blobs)`.
    pub fn join_persons_and_institutions(
        &mut self,
        st: &mut State,
        blobs: Vec<Option<BlobId>>,
    ) -> CslResult<Option<BlobId>> {
        let blobs = purge_empty_blobs(st, blobs);
        let delim = st
            .tmp
            .name_delimiter
            .clone()
            .unwrap_or_else(|| "undefined".into());
        let ret = self.join(st, blobs, &delim, None)?;
        match ret {
            Some(r) => {
                st.blobs
                    .get_mut(r)
                    .extra
                    .insert("isInstitution".into(), Value::Bool(true));
                Ok(Some(r))
            }
            None => Err(EngineError::BadInput(
                "Cannot create property 'isInstitution' on boolean 'false'".into(),
            )),
        }
    }

    /// `CSL.NameOutput.prototype.joinFreetersAndInstitutionSets(blobs)`:
    /// nothing, one or two blobs, never more.
    pub fn join_freeters_and_institution_sets(
        &mut self,
        st: &mut State,
        blobs: Vec<Option<BlobId>>,
    ) -> CslResult<Option<BlobId>> {
        let blobs = purge_empty_blobs(st, blobs);
        // `this._join(blobs, "[never here]", this["with"].single, this["with"].multiple)`:
        // _join takes three arguments, so the last is ignored.
        let with = self.with.ok_or_else(|| {
            super::load::type_error("Cannot read properties of undefined (reading 'single')")
        })?;
        self.join(st, blobs, "[never here]", with.single)
    }

    /// `CSL.NameOutput.prototype._getAfterInvertedName(blobs, delimiter,
    /// finalJoin)`.
    pub fn get_after_inverted_name(
        &self,
        st: &mut State,
        blobs: &[Option<BlobId>],
        delimiter: &str,
        final_join: Option<BlobId>,
    ) -> CslResult<Option<BlobId>> {
        if let Some(fj) = final_join {
            if blobs.len() > 1 {
                let dpl = self.inherit_name_opt(st, "delimiter-precedes-last", None, None)?;
                if dpl.as_ref().and_then(Value::as_str) == Some("after-inverted-name") {
                    let prev = blobs[blobs.len() - 2].ok_or_else(|| {
                        super::load::type_error(
                            "Cannot read properties of undefined (reading 'blobs')",
                        )
                    })?;
                    let first_inverted = match &st.blobs.get(prev).blobs {
                        super::obj_blob::BlobContent::List(l) if !l.is_empty() => match l[0] {
                            super::obj_blob::BlobChild::Blob(b) => crate::citeproc::js::truthy_opt(
                                st.blobs.get(b).extra.get("isInverted"),
                            ),
                            _ => false,
                        },
                        _ => false,
                    };
                    if first_inverted {
                        st.blobs.get_mut(fj).set_string("prefix", delimiter);
                    }
                }
            }
        }
        Ok(final_join)
    }

    /// `CSL.NameOutput.prototype._getAndJoin(blobs, delimiter)`: a copy of
    /// the `and` blob (single or multiple) for this list, `None` when there
    /// is a single name.
    pub fn get_and_join(
        &self,
        st: &mut State,
        blobs: &[Option<BlobId>],
        delimiter: &str,
    ) -> CslResult<Option<BlobId>> {
        let mut final_join: Option<BlobId> = None;
        if blobs.len() > 1 {
            let multiple = blobs.len() > 2;
            let last_is_institution = match blobs[blobs.len() - 1] {
                Some(b) => {
                    crate::citeproc::js::truthy_opt(st.blobs.get(b).extra.get("isInstitution"))
                }
                None => false,
            };
            let pair: Option<BlobPair> = if last_is_institution {
                self.institution_and
            } else {
                self.name_and
            };
            let src = pair
                .and_then(|p| if multiple { p.multiple } else { p.single })
                .ok_or_else(|| {
                    EngineError::Csl("SyntaxError: \"undefined\" is not valid JSON".to_string())
                })?;
            // finalJoin = JSON.parse(JSON.stringify(finalJoin));
            let copy = clone_blob_deep(st, src);
            final_join = self.get_after_inverted_name(st, blobs, delimiter, Some(copy))?;
        }
        Ok(final_join)
    }

    /// `CSL.NameOutput.prototype._joinEtAl(blobs)`.
    pub fn join_et_al(
        &mut self,
        st: &mut State,
        blobs: Vec<Option<BlobId>>,
    ) -> CslResult<Option<BlobId>> {
        let delimiter = self.name_delimiter(st)?;
        let blob = self.join(st, blobs.clone(), &delimiter, None)?;

        // notSerious
        let name_tok = self.name_token()?.clone();
        q_open_level(st, FormatRef::Token(name_tok))?;
        // Delimiter is applied from separately saved source in this case,
        // for discriminate application of single and multiple joins.
        if let Some(cur) = super::queue::current(st, QueueId::Output) {
            st.blobs.get_mut(cur).set_string("delimiter", "");
        }
        q_append_blob(st, blob, FormatRef::Name("literal".into()), true)?;
        let et_al = self.et_al.unwrap_or_default();
        if blobs.len() > 1 {
            q_append_blob(st, et_al.multiple, FormatRef::Name("literal".into()), true)?;
        } else if blobs.len() == 1 {
            q_append_blob(st, et_al.single, FormatRef::Name("literal".into()), true)?;
        }
        q_close_level(st)?;
        q_pop_blob(st)
    }

    /// `CSL.NameOutput.prototype._joinEllipsis(blobs)`.
    pub fn join_ellipsis(
        &mut self,
        st: &mut State,
        blobs: Vec<Option<BlobId>>,
    ) -> CslResult<Option<BlobId>> {
        let delimiter = self.name_delimiter(st)?;
        let mut final_join: Option<BlobId> = None;
        if blobs.len() > 1 {
            let multiple = blobs.len() > 2;
            let src = self
                .name_ellipsis
                .and_then(|p| if multiple { p.multiple } else { p.single })
                .ok_or_else(|| {
                    EngineError::Csl("SyntaxError: \"undefined\" is not valid JSON".to_string())
                })?;
            let copy = clone_blob_deep(st, src);
            final_join = self.get_after_inverted_name(st, &blobs, &delimiter, Some(copy))?;
        }
        self.join(st, blobs, &delimiter, final_join)
    }

    /// `CSL.NameOutput.prototype._joinAnd(blobs)`.
    pub fn join_and(
        &mut self,
        st: &mut State,
        blobs: Vec<Option<BlobId>>,
    ) -> CslResult<Option<BlobId>> {
        let delimiter = self.name_delimiter(st)?;
        let final_join = self.get_and_join(st, &blobs, &delimiter)?;
        self.join(st, blobs, &delimiter, final_join)
    }

    /// `CSL.NameOutput.prototype._join(blobs, delimiter, finalJoin)`: join
    /// `blobs` into one new blob, `delimiter` appended to the suffix of each
    /// but the last (and `final_join` before the last); `None` (JS `false`)
    /// when there is nothing to join.
    pub fn join(
        &mut self,
        st: &mut State,
        blobs: Vec<Option<BlobId>>,
        delimiter: &str,
        final_join: Option<BlobId>,
    ) -> CslResult<Option<BlobId>> {
        join_blobs(st, blobs, delimiter, final_join)
    }

    /// `CSL.NameOutput.prototype._getToken(tokenname)`: the `cs:name` token,
    /// or for `"institution"` a new bare token (the same as `"empty"`).
    pub fn get_token(&self, tokenname: &str) -> CslResult<crate::citeproc::obj_token::Token> {
        use crate::citeproc::obj_token::{Token, TokenType};
        if tokenname == "institution" {
            return Ok(Token::new("", TokenType::Start));
        }
        match tokenname {
            "name" => Ok(self.name_token()?.clone()),
            "names" => Ok(self.names.clone()),
            _ => Err(super::load::type_error(
                "Cannot read properties of undefined (reading 'strings')",
            )),
        }
    }
}
