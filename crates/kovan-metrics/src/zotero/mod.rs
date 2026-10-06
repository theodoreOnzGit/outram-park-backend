// Part of the kovan Zotero port (GitHub #747, #751).
//
// No upstream logic is ported in this module: it is kovan's own accounting,
// built on kovan-common's port of the Zotero data model (Zotero, AGPL-3.0,
// Corporation for Digital Scholarship) and on its `ZoteroItem` <->
// `KovanDocument` conversion. See this crate's NOTICE, "Upstream: Zotero".

//! Library and import accounting for Zotero libraries (GitHub #751, epic
//! #747).
//!
//! | Item | What it answers |
//! |---|---|
//! | [`LibraryCounts`] | How many items of each item type, per collection, per tag, per attachment link mode and per annotation type a [`ZoteroLibrary`] holds |
//! | [`import_library`] -> [`ImportReport`] | Converting the library to kovan documents: what was imported, what was skipped and why, and what was lossy, per item and per field |
//!
//! Both render as Markdown ([`LibraryCounts::to_markdown`],
//! [`ImportReport::to_markdown`]), kovan's format for reports, and both are
//! deterministic: every list is sorted, every map is a `BTreeMap`, and
//! nothing depends on time, locale or the file system.
//!
//! **This is not a port.** Zotero has no import report; the accounting is
//! kovan's own. The conversion it measures is
//! [`kovan_common::zotero::kovan`] (`ZoteroItem::to_kovan_document`).
//!
//! ## How "lossy" is measured
//!
//! Empirically, from the conversion itself rather than from a hand-kept list:
//! each imported item is converted to a [`kovan_common::KovanDocument`], the
//! verbatim copy in `zotero_item` is removed, and the document is converted
//! back with `ZoteroItem::from_kovan_document`. Every property of the
//! original that the round trip does not reproduce is a [`FieldLoss`]:
//!
//! * [`LossKind::Dropped`]: absent after the round trip;
//! * [`LossKind::Altered`]: present with a different value (e.g. `date`
//!   `1999-12-31` comes back as `1999`, kovan keeping only the year;
//!   `itemType` `book` comes back as `document`);
//! * [`LossKind::MovedToExtra`]: the value survives only as a
//!   `Name: value` line of Extra (a field the mapped item type lacks).
//!
//! Fields are compared through their **base** field, so `websiteTitle` on a
//! `webpage` is compared with whatever field carries `publicationTitle`
//! after the round trip. Creators are compared one by one; tags as a set.
//! Values added by the round trip (e.g. a `citationKey` written from the
//! slug) are not losses and are not reported.
//!
//! "Lossy" therefore means **not carried by kovan's own document fields**.
//! Every imported item is also stored verbatim in `KovanDocument::zotero_item`,
//! so Zotero -> kovan -> Zotero is lossless (kovan-common tests this); the
//! report says what a reader of kovan's fields, or of anything exported from
//! them without `zotero_item`, does not see.
//!
//! ## What is imported and what is skipped
//!
//! Kovan's policy, applied in library order:
//!
//! | Item | Outcome | [`SkipReason`] |
//! |---|---|---|
//! | top-level regular item, with a key, not in the trash | imported | |
//! | `parentItem` set (child note, attachment, annotation) | skipped | `ChildItem` |
//! | standalone note, attachment or annotation | skipped | `NotRegular` |
//! | `deleted: true` (in the trash) | skipped | `Trashed` |
//! | no `key` (no stable kovan id) | skipped | `MissingKey` |
//! | key already imported | skipped | `DuplicateKey` |
//!
//! A child item that is skipped is **not preserved anywhere** in the kovan
//! documents: its parent's `zotero_item` holds only the parent. (Children
//! nested in the translator export format, in a parent's `attachments` and
//! `notes`, do travel inside the parent's `zotero_item`, and show up as an
//! `attachments`/`notes` loss of kovan's own fields.)
//!
//! **Maturity: AI draft (1).** Not yet human-reviewed.

mod counts;
mod import_report;

pub use counts::{CollectionCount, LibraryCounts, TagCount};
pub use import_report::{
    import_library, FieldLoss, ImportReport, ImportedItem, LossKind, SkipReason, SkippedItem,
};

// Re-exported for callers that only need the accounting.
pub use kovan_common::zotero::ZoteroLibrary;

/// Escape a value for a Markdown table cell: pipes escaped, line breaks
/// flattened, and long values cut at `max` characters with an ellipsis.
pub(crate) fn md_cell(s: &str, max: usize) -> String {
    let flat: String = s
        .chars()
        .map(|c| if c == '\n' || c == '\r' { ' ' } else { c })
        .collect();
    let cut: String = if flat.chars().count() > max {
        let mut t: String = flat.chars().take(max).collect();
        t.push('…');
        t
    } else {
        flat
    };
    cut.replace('|', "\\|")
}
