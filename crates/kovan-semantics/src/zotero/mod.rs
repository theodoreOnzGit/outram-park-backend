// Part of the kovan Zotero port (GitHub #747, #751).
//
// Upstream: Zotero, https://github.com/zotero/zotero (commit 9cbba8c4d281);
// Zotero utilities, https://github.com/zotero/utilities (commit 4051881d59c6).
// Copyright (c) Corporation for Digital Scholarship, Vienna, Virginia, USA.
// Licence: AGPL-3.0. See this crate's NOTICE, "Upstream: Zotero".

//! Zotero's duplicate detection, relations model and item merge, ported to
//! run over `kovan_common::zotero`'s in-memory [`ZoteroLibrary`]
//! (GitHub #751, epic #747), plus the mapping of Zotero relations,
//! collections and tags onto kovan's relation and concept schema.
//!
//! | Module | What | Ported from |
//! |---|---|---|
//! | [`duplicates`] | [`find_duplicates`]: ISBN, DOI, normalised title + creators + year | `xpcom/duplicates.js` |
//! | [`relations`] | predicates, URIs, add/remove/has/set, subjects of an object, related items, `updateUser`, `purge`, linked items | `xpcom/data/relations.js`, `dataObject.js`, `item.js`, `uri.js` |
//! | [`merge`] | [`merge_items`] (pure), [`merge_pane_order`], [`field_alternatives`] | `mergeItems.mjs` (`Zotero.Items.merge`), `notes.js`, `item.js`, `dataObjectUtilities.js`, `duplicatesMergePane.js` |
//! | [`text`] | `removeDiacritics`, `cleanISBN`, `toISBN13`, JS string semantics | utilities `utilities.js` |
//! | [`kovan_map`] | [`map_library`]: relations/collections/tags -> kovan drafts | (kovan's own) |
//!
//! Everything is deterministic (sorted outputs, no clock, no randomness)
//! and does no file or process I/O, so the module would build for wasm32.
//!
//! **Verification.** Upstream Zotero desktop cannot run here, and the
//! Zotero translation server running locally (http://127.0.0.1:1969) does
//! not expose duplicate detection, relations or merging, so there is no
//! code-to-code run against upstream (that is GitHub #752). The reference
//! is upstream's own test suites, ported case by case to
//! `tests/zotero_upstream.rs` with the same inputs and expected results:
//! `test/tests/duplicatesTest.js`, `relationsTest.js`, `mergeItemsTest.js`,
//! the relation cases of `dataObjectTest.js` and `itemTest.js`, and
//! utilities' `utilitiesTest.js` `cleanISBN`/`toISBN13`. Where a test sets
//! up a database, files or the UI, the state it creates is written as a
//! [`ZoteroLibrary`] fixture (and file facts as `AttachmentEvidence`), and
//! the test says so. `docs/zotero-port.md` lists every case, its result,
//! and the cases that cannot be ported.
//!
//! **Maturity: AI draft (1).** Not yet human-reviewed.
//!
//! [`ZoteroLibrary`]: kovan_common::zotero::ZoteroLibrary

#[rustfmt::skip]
mod diacritics_generated;
pub mod duplicates;
pub mod kovan_map;
pub mod merge;
pub mod relations;
pub mod text;

pub use duplicates::{find_duplicates, DuplicateOptions, DuplicateSets};
pub use kovan_map::{
    map_library, KovanClassificationDraft, KovanConceptDraft, KovanMapping, KovanRelationDraft,
    LossKind, MappingLoss,
};
pub use merge::{
    field_alternatives, merge_items, merge_pane_order, AttachmentEvidence, MergeError,
    MergeOptions, MergeOutcome,
};
pub use relations::{
    LibraryUri, ObjectType, RelationError, ZoteroUri, LINKED_OBJECT_PREDICATE,
    RELATED_ITEM_PREDICATE, REPLACED_ITEM_PREDICATE,
};
