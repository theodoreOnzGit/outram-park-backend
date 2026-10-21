//! The PDF follow-up of identifier lookup (GitHub #756): after a PDF is
//! ingested and an identifier is found in it, kovan can OFFER to fill the
//! document's metadata from the record a lookup fetched. Nothing here goes
//! online and nothing is applied automatically:
//!
//! ```text
//! PDF --extract_metadata--> KovanDocument (extracted)
//!      \--identifiers_in_document--> Identifier --(user asks; caller fetches)--> ZoteroItem
//! propose(extracted, fetched, user_edited) --> LookupProposal: one row per field,
//!     extracted value | fetched value | Same / Fill / Replace / UserEdited / NothingFetched
//! user confirms some rows (or all) --> apply(doc, proposal, accepted) --> KovanDocument
//!     (only accepted rows change; UserEdited rows never; zotero_item = fetched item)
//! ```
//!
//! **Fields the user edited are never overwritten.** The caller says which
//! fields the user changed (the TUI review form knows: `ReviewState::is_edited`);
//! those rows are [`Decision::UserEdited`] and [`apply`] skips them even when
//! a caller passes them as accepted. KovanDocument has no persisted record of
//! edits, and none is added here (schema unchanged): a later lookup on a saved
//! document gets the same protection only for the fields its caller names.
//!
//! **Schema:** the fetched item is stored in the existing optional
//! `KovanDocument.zotero_item`; no field is added, renamed or re-typed.

use kovan_common::zotero::ZoteroItem;
use kovan_common::{Author, KovanDocument};

use crate::zotero::search::{extract_identifiers, Identifier};

/// A metadata field the proposal covers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum DocField {
    /// `title`.
    Title,
    /// `authors`.
    Authors,
    /// `year`.
    Year,
    /// `doi`.
    Doi,
    /// `journal`.
    Journal,
    /// `institution`.
    Institution,
    /// `publisher`.
    Publisher,
    /// `volume`.
    Volume,
    /// `number` (issue).
    Number,
    /// `pages`.
    Pages,
    /// `abstract_text`.
    Abstract,
    /// `keywords`.
    Keywords,
    /// `document_type`.
    DocumentType,
}

impl DocField {
    /// Every field, in display order.
    pub const ALL: [DocField; 13] = [
        DocField::Title,
        DocField::Authors,
        DocField::Year,
        DocField::Doi,
        DocField::Journal,
        DocField::Institution,
        DocField::Publisher,
        DocField::Volume,
        DocField::Number,
        DocField::Pages,
        DocField::Abstract,
        DocField::Keywords,
        DocField::DocumentType,
    ];

    /// The `KovanDocument` field name.
    pub fn name(self) -> &'static str {
        match self {
            DocField::Title => "title",
            DocField::Authors => "authors",
            DocField::Year => "year",
            DocField::Doi => "doi",
            DocField::Journal => "journal",
            DocField::Institution => "institution",
            DocField::Publisher => "publisher",
            DocField::Volume => "volume",
            DocField::Number => "number",
            DocField::Pages => "pages",
            DocField::Abstract => "abstract_text",
            DocField::Keywords => "keywords",
            DocField::DocumentType => "document_type",
        }
    }

    /// The field with this name (as [`DocField::name`]).
    pub fn from_name(name: &str) -> Option<DocField> {
        DocField::ALL.into_iter().find(|f| f.name() == name)
    }
}

/// What the proposal suggests for one field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    /// Both say the same: nothing to do.
    Same,
    /// The document has nothing; the record has a value.
    Fill,
    /// Both have values and they differ.
    Replace,
    /// The record has no value: the document's is kept.
    NothingFetched,
    /// The user edited this field: never overwritten.
    UserEdited,
}

/// One row: a field, the document's value, the record's value, the
/// suggestion. Values are rendered as text for display.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldProposal {
    /// The field.
    pub field: DocField,
    /// The document's current value ("" when empty).
    pub current: String,
    /// The fetched record's value ("" when it has none).
    pub fetched: String,
    /// The suggestion.
    pub decision: Decision,
}

impl FieldProposal {
    /// Whether accepting this row would change the document.
    pub fn changes(&self) -> bool {
        matches!(self.decision, Decision::Fill | Decision::Replace)
    }
}

/// The field-by-field comparison of a document with a fetched record.
#[derive(Debug, Clone, PartialEq)]
pub struct LookupProposal {
    /// One row per [`DocField::ALL`].
    pub fields: Vec<FieldProposal>,
    /// The fetched item (stored in `zotero_item` on [`apply`]).
    pub fetched_item: ZoteroItem,
    /// The fetched item as a document (what the rows' `fetched` show).
    fetched_doc: KovanDocument,
}

impl LookupProposal {
    /// The rows that would change the document if accepted.
    pub fn changes(&self) -> impl Iterator<Item = &FieldProposal> {
        self.fields.iter().filter(|f| f.changes())
    }
}

/// The identifiers in an extracted document worth looking up: its DOI when
/// the extractor found one, then `extractIdentifiers` over the start of the
/// Markdown body (title page, abstract; the first 4000 characters, so a
/// reference list's identifiers are not mistaken for the document's own).
pub fn identifiers_in_document(doc: &KovanDocument) -> Vec<Identifier> {
    let mut out: Vec<Identifier> = Vec::new();
    if let Some(d) = doc.doi.as_deref().filter(|d| !d.is_empty()) {
        out.extend(extract_identifiers(d));
    }
    let head: String = doc.markdown_body.chars().take(4000).collect();
    for id in extract_identifiers(&head) {
        if !out.contains(&id) {
            out.push(id);
        }
    }
    out
}

fn authors_text(a: &[Author]) -> String {
    a.iter()
        .map(|a| {
            if a.given.is_empty() {
                a.family.clone()
            } else {
                format!("{}, {}", a.family, a.given)
            }
        })
        .collect::<Vec<_>>()
        .join("; ")
}

fn value(doc: &KovanDocument, f: DocField) -> String {
    let o = |v: &Option<String>| v.clone().unwrap_or_default();
    match f {
        DocField::Title => doc.title.clone(),
        DocField::Authors => authors_text(&doc.authors),
        DocField::Year => doc.year.map(|y| y.to_string()).unwrap_or_default(),
        DocField::Doi => o(&doc.doi),
        DocField::Journal => o(&doc.journal),
        DocField::Institution => o(&doc.institution),
        DocField::Publisher => o(&doc.publisher),
        DocField::Volume => o(&doc.volume),
        DocField::Number => o(&doc.number),
        DocField::Pages => o(&doc.pages),
        DocField::Abstract => doc.abstract_text.clone(),
        DocField::Keywords => doc.keywords.join("; "),
        DocField::DocumentType => format!("{:?}", doc.document_type),
    }
}

/// Compare `current` with the fetched record, field by field. `user_edited`
/// names the fields the user changed: those are [`Decision::UserEdited`].
pub fn propose(
    current: &KovanDocument,
    fetched: &ZoteroItem,
    user_edited: &[DocField],
) -> LookupProposal {
    let fetched_doc = fetched.to_kovan_document();
    let fields = DocField::ALL
        .into_iter()
        .map(|field| {
            let cur = value(current, field);
            let new = value(&fetched_doc, field);
            let decision = if user_edited.contains(&field) {
                Decision::UserEdited
            } else if new.trim().is_empty() {
                Decision::NothingFetched
            } else if new == cur {
                Decision::Same
            } else if cur.trim().is_empty() {
                Decision::Fill
            } else {
                Decision::Replace
            };
            FieldProposal {
                field,
                current: cur,
                fetched: new,
                decision,
            }
        })
        .collect();
    LookupProposal {
        fields,
        fetched_item: fetched.clone(),
        fetched_doc,
    }
}

/// Apply the rows the user accepted. Rows that are not
/// [`Decision::Fill`]/[`Decision::Replace`] are skipped whatever `accepted`
/// says (a user-edited field is never overwritten). The fetched item goes
/// into `zotero_item` when at least one row was accepted, or when
/// `keep_record` is set. The id, slug, source, body and every field not
/// accepted are left as they are.
pub fn apply(
    current: &KovanDocument,
    proposal: &LookupProposal,
    accepted: &[DocField],
    keep_record: bool,
) -> KovanDocument {
    let mut doc = current.clone();
    let src = &proposal.fetched_doc;
    let mut any = false;
    for row in &proposal.fields {
        if !row.changes() || !accepted.contains(&row.field) {
            continue;
        }
        any = true;
        match row.field {
            DocField::Title => doc.title = src.title.clone(),
            DocField::Authors => doc.authors = src.authors.clone(),
            DocField::Year => doc.year = src.year,
            DocField::Doi => doc.doi = src.doi.clone(),
            DocField::Journal => doc.journal = src.journal.clone(),
            DocField::Institution => doc.institution = src.institution.clone(),
            DocField::Publisher => doc.publisher = src.publisher.clone(),
            DocField::Volume => doc.volume = src.volume.clone(),
            DocField::Number => doc.number = src.number.clone(),
            DocField::Pages => doc.pages = src.pages.clone(),
            DocField::Abstract => doc.abstract_text = src.abstract_text.clone(),
            DocField::Keywords => doc.keywords = src.keywords.clone(),
            DocField::DocumentType => doc.document_type = src.document_type,
        }
    }
    if any || keep_record {
        doc.zotero_item = Some(proposal.fetched_item.clone());
    }
    doc
}

/// Every row that would change the document ("accept all").
pub fn all_changes(proposal: &LookupProposal) -> Vec<DocField> {
    proposal.changes().map(|r| r.field).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use kovan_common::{DocumentType, Visibility};

    fn fetched() -> ZoteroItem {
        ZoteroItem::from_json_value(&serde_json::json!({
            "itemType": "journalArticle", "title": "Fetched Title",
            "creators": [{"creatorType": "author", "firstName": "Ada", "lastName": "Lovelace"}],
            "date": "1843", "DOI": "10.1000/xyz", "publicationTitle": "Journal",
            "volume": "7", "pages": "1-10"
        }))
        .unwrap()
    }

    fn extracted() -> KovanDocument {
        let mut d = KovanDocument::new(
            "id",
            "slug",
            Visibility::Open,
            DocumentType::Paper,
            "Extracted title",
        );
        d.year = Some(1842);
        d
    }

    #[test]
    fn proposal_rows_and_user_edits() {
        let p = propose(&extracted(), &fetched(), &[DocField::Year]);
        let row = |f| p.fields.iter().find(|r| r.field == f).unwrap().decision;
        assert_eq!(row(DocField::Title), Decision::Replace);
        assert_eq!(row(DocField::Year), Decision::UserEdited);
        assert_eq!(row(DocField::Doi), Decision::Fill);
        assert_eq!(row(DocField::Institution), Decision::NothingFetched);
        // Accepting everything, the edited year included, leaves the year.
        let all: Vec<DocField> = DocField::ALL.to_vec();
        let d = apply(&extracted(), &p, &all, false);
        assert_eq!(d.year, Some(1842));
        assert_eq!(d.title, "Fetched Title");
        assert_eq!(d.doi.as_deref(), Some("10.1000/xyz"));
        assert!(d.zotero_item.is_some());
        assert_eq!(d.id, "id");
    }

    #[test]
    fn nothing_accepted_changes_nothing() {
        let p = propose(&extracted(), &fetched(), &[]);
        assert_eq!(apply(&extracted(), &p, &[], false), extracted());
    }

    #[test]
    fn identifiers_from_document() {
        let mut d = extracted();
        d.markdown_body = "Title\narXiv:1706.03762v5 [cs.CL]\n".to_owned();
        assert_eq!(
            identifiers_in_document(&d),
            vec![Identifier::ArXiv("1706.03762".into())]
        );
        d.doi = Some("10.1000/xyz".into());
        assert_eq!(
            identifiers_in_document(&d)[0],
            Identifier::Doi("10.1000/xyz".into())
        );
    }
}
