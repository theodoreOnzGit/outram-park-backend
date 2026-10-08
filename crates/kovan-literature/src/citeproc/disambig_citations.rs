// Part of the kovan port of citeproc-js (GitHub #790).
//
// Upstream:    citeproc-js, https://github.com/juris-m/citeproc-js
// Source:      src/disambig_citations.js (CSL.Registry.CitationReg), and the citation objects it holds (api_cite.js, api_integration)
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

//! `CSL.Registry.CitationReg`: the registry of citations in the document.
//!
//! In citeproc-js a citation is a plain object that the caller passes to
//! `processCitationCluster`, and the same object is then held by
//! `citationById`, `citationByIndex` and `citationsByItemId`, and mutated
//! (`properties.index`, `sortedItems`, ...). Here the citations live in an
//! arena ([`CitationReg::arena`]) and the three indexes hold [`CitId`]s, which
//! keeps that sharing without reference counting.

use std::collections::BTreeMap;

use super::js::{self, Obj};

/// An index into [`CitationReg::arena`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CitId(pub usize);

/// One `[Item, item]` pair of `citation.sortedItems`: the cite item (a copy of
/// the caller's `citationItems[i]`, plus `sortkeys`, `position`,
/// `near-note`, ...) and the id of the registered `Item` it cites. The `Item`
/// itself is `registry.refhash[item_id]`, read when needed, so a re-fetched
/// item (`INPUT2`) is seen by earlier citations as it is in JS (where the
/// object is updated in place).
#[derive(Debug, Clone, PartialEq)]
pub struct SortedItem {
    /// `Item.id` as a string (the key of `registry.refhash`).
    pub item_id: String,
    /// `item`.
    pub item: Obj,
}

/// A citation object: `{citationID, citationItems, properties, sortedItems}`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CitationRec {
    /// `citationID`.
    pub citation_id: String,
    /// `citationItems` (the caller's items).
    pub citation_items: Vec<Obj>,
    /// `properties`: `noteIndex`, `index`, `mode`, `unsorted`, `prefix`,
    /// `suffix`, `infix`, `suppress-trailing-punctuation`, ...
    pub properties: Obj,
    /// `sortedItems`.
    pub sorted_items: Vec<SortedItem>,
}

impl CitationRec {
    /// `properties.noteIndex`, read as JS numbers read it (`parseInt`-free:
    /// a number, a numeric string, else `0`).
    pub fn note_index(&self) -> i64 {
        match self.properties.get("noteIndex") {
            Some(v) => js::parse_int_value(v).unwrap_or(0),
            None => 0,
        }
    }
}

/// `new CSL.Registry.CitationReg()`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CitationReg {
    /// Every citation object ever registered (never shrunk during a session).
    pub arena: Vec<CitationRec>,
    /// `citationById`.
    pub citation_by_id: BTreeMap<String, CitId>,
    /// `citationByIndex`.
    pub citation_by_index: Vec<CitId>,
    /// `citationsByItemId`: `undefined` until `processCitationCluster` sets it
    /// (`None`), then citations by cited item id, in document order.
    pub citations_by_item_id: Option<BTreeMap<String, Vec<CitId>>>,
}

impl CitationReg {
    /// Add a citation object to the arena.
    pub fn alloc(&mut self, rec: CitationRec) -> CitId {
        self.arena.push(rec);
        CitId(self.arena.len() - 1)
    }

    /// The citation behind `id`.
    pub fn get(&self, id: CitId) -> &CitationRec {
        &self.arena[id.0]
    }

    /// The citation behind `id`, mutably.
    pub fn get_mut(&mut self, id: CitId) -> &mut CitationRec {
        &mut self.arena[id.0]
    }

    /// `citationById[citation_id]`.
    pub fn by_id(&self, citation_id: &str) -> Option<CitId> {
        self.citation_by_id.get(citation_id).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn the_arena_shares_citations_between_the_indexes() {
        let mut reg = CitationReg::default();
        let mut rec = CitationRec {
            citation_id: "c1".into(),
            ..CitationRec::default()
        };
        rec.properties.insert("noteIndex".into(), json!("3"));
        let id = reg.alloc(rec);
        reg.citation_by_id.insert("c1".into(), id);
        reg.citation_by_index.push(id);
        reg.get_mut(id).properties.insert("index".into(), json!(0));
        assert_eq!(
            reg.get(reg.by_id("c1").unwrap()).properties["index"],
            json!(0)
        );
        assert_eq!(reg.get(id).note_index(), 3);
        assert!(reg.by_id("zz").is_none());
        assert!(reg.citations_by_item_id.is_none());
    }
}
