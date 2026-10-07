// Part of the kovan Zotero port (GitHub #747, #749).
//
// Upstream: Zotero translators, https://github.com/zotero/translators
//   (commit 3d1c78530f42): "Endnote XML.js" (translatorID
//   eb7059a4-35ec-4961-a915-3cf58eb9784b, lastUpdated 2021-07-14
//   20:41:42): the header and `detectImport` :1-31, the field lists :33-58,
//   `processItemType` :60-109, `processNumberType` :111-159,
//   `exportItemType` :161-196, `exportRefNumber` :198-234, `fieldMap`
//   :236-466, `getField` :468-508.
// Copyright (c) Sebastian Karcher (creator named in the header; the file
//   carries no copyright line).
// Licence: AGPL-3.0 (no licence text upstream; treated as AGPLv3 as part of
//   Zotero, maintainer decision 2026-10-07, #747).

//! The Endnote XML translator: import ([`import`]) and export ([`export`]).
//!
//! Collections: the header asks for `getCollections`, but neither direction
//! uses them (upstream calls neither `Zotero.nextCollection` nor
//! `Zotero.Collection`), so nothing is lost.

mod export;
mod import;

pub use export::do_export;
pub use import::do_import;

use crate::zotero::framework::options::{translator_type, HeaderValue, TranslatorMetadata};
use crate::zotero::framework::xml::get_xml;
use crate::zotero::framework::xpath::xpath_text;
use crate::zotero::framework::ImportContext;

/// The translator header.
pub static METADATA: TranslatorMetadata = TranslatorMetadata {
    id: "eb7059a4-35ec-4961-a915-3cf58eb9784b",
    label: "Endnote XML",
    creator: "Sebastian Karcher",
    target: "xml",
    min_version: "4.0",
    priority: 100,
    translator_type: translator_type::IMPORT | translator_type::EXPORT,
    config_options: &[
        ("async", HeaderValue::Bool(true)),
        ("getCollections", HeaderValue::Bool(true)),
    ],
    display_options: &[
        ("exportNotes", HeaderValue::Bool(true)),
        ("exportFileData", HeaderValue::Bool(false)),
    ],
    hidden_prefs: &[],
    last_updated: "2021-07-14 20:41:42",
};

/// `detectImport` (:22-31): the document has a `record/ref-type` with text.
/// A parse error thrown by `getXML` makes detection fail (the framework
/// catches it).
pub fn detect_import(ctx: &mut ImportContext) -> bool {
    let Ok(doc) = get_xml(ctx) else {
        return false;
    };
    let Some(root) = doc.document_element() else {
        return false;
    };
    matches!(
        xpath_text(&doc, root, "//record/ref-type", &[], None),
        Ok(Some(s)) if !s.is_empty()
    )
}

/// `fields` (:34-45): the Endnote XML fields, in export order.
pub(super) const FIELDS: &[&str] = &[
    "database",
    "source-app",
    "rec-number",
    "ref-type",
    "contributors",
    "auth-address",
    "auth-affiliaton",
    "titles",
    "periodical",
    "pages",
    "volume",
    "number",
    "issue",
    "secondary-volume",
    "secondary-issue",
    "num-vols",
    "edition",
    "section",
    "reprint-edition",
    "reprint-status",
    "keywords",
    "dates",
    "pub-location",
    "publisher",
    "orig-pub",
    "isbn",
    "accession-num",
    "call-num",
    "report-id",
    "coden",
    "electronic-resource-num",
    "abstract",
    "label",
    "image",
    "caption",
    "notes",
    "research-notes",
    "work-type",
    "reviewed-item",
    "availability",
    "remote-source",
    "meeting-place",
    "work-location",
    "work-extent",
    "pack-method",
    "size",
    "repro-ratio",
    "remote-database-name",
    "remote-database-provider",
    "language",
    "urls",
    "access-date",
    "modified-date",
    "custom1",
    "custom2",
    "custom3",
    "custom4",
    "custom5",
    "custom6",
    "custom7",
    "misc1",
    "misc2",
    "misc3",
];

/// `titleFields` (:47-49).
pub(super) const TITLE_FIELDS: [&str; 6] = [
    "title",
    "secondary-title",
    "tertiary-title",
    "alt-title",
    "short-title",
    "translated-title",
];

/// `authorFields` (:53-55).
pub(super) const AUTHOR_FIELDS: [&str; 5] = [
    "authors",
    "secondary-authors",
    "tertiary-authors",
    "subsidiary-authors",
    "translated-authors",
];

/// `processItemType` (:60-109).
pub(super) fn process_item_type(name: &str) -> Option<&'static str> {
    Some(match name {
        "Artwork" => "artwork",
        "Audiovisual Material" => "videoRecording",
        "Bill" => "bill",
        "Book Section" => "bookSection",
        "Book" => "book",
        "Case" => "case",
        "Catalog" => "book",
        "Computer Program" => "computerProgram",
        "Conference Proceedings" => "conferencePaper",
        "Web Page" => "webpage",
        "Generic" => "document",
        "Hearing" => "hearing",
        "Journal Article" => "journalArticle",
        "Magazine Article" => "magazineArticle",
        "Map" => "map",
        "Film or Broadcast" => "film",
        "Newspaper Article" => "newspaperArticle",
        "Pamphlet" => "manuscript",
        "Patent" => "patent",
        "Personal Communication" => "letter",
        "Report" => "report",
        "Edited Book" => "book",
        "Statute" => "statute",
        "Thesis" => "thesis",
        "Unpublished Work" => "manuscript",
        "Manuscript" => "manuscript",
        "Figure" => "artwork",
        "Chart or Table" => "artwork",
        "Equation" => "artwork",
        "Electronic Article" => "journalArticle",
        "Electronic Book" => "book",
        "Online Database" => "webpage",
        "Government Document" => "bill",
        "Conference Paper" => "presentation",
        "Online Multimedia" => "webpage",
        "Classical Work" => "book",
        "Legal Rule or Regulation" => "report",
        "Ancient Text" => "book",
        "Dictionary" => "dictionaryEntry",
        "Encyclopedia" => "encyclopediaArticle",
        "Grant" => "report",
        "Aggregated Database" => "webpage",
        "Blog" => "blogPost",
        "Serial" => "book",
        "Standard" => "report",
        "Dataset" => "report",
        "Electronic Book Section" => "bookSection",
        "Music" => "audioRecording",
        _ => return None,
    })
}

/// `processNumberType` (:111-159), keyed by the text of `ref-type` (object
/// keys are strings: "17", not " 17"). Upstream's 45 maps to "wepage" (sic).
pub(super) fn process_number_type(n: &str) -> Option<&'static str> {
    Some(match n {
        "2" => "artwork",
        "3" => "videoRecording",
        "4" => "bill",
        "5" => "bookSection",
        "6" => "book",
        "7" => "case",
        "8" => "book",
        "9" => "computerProgram",
        "10" => "conferencePaper",
        "12" => "webpage",
        "13" => "document",
        "14" => "hearing",
        "17" => "journalArticle",
        "19" => "magazineArticle",
        "20" => "map",
        "21" => "film",
        "23" => "newspaperArticle",
        "24" => "manuscript",
        "25" => "patent",
        "26" => "letter",
        "27" => "report",
        "28" => "book",
        "31" => "statute",
        "32" => "thesis",
        "34" => "manuscript",
        "36" => "manuscript",
        "37" => "artwork",
        "38" => "artwork",
        "39" => "artwork",
        "43" => "journalArticle",
        "44" => "book",
        "45" => "wepage",
        "46" => "bill",
        "47" => "conferencePaper",
        "48" => "webpage",
        "49" => "book",
        "50" => "report",
        "51" => "book",
        "52" => "dictionaryEntry",
        "53" => "encyclopediaArticle",
        "54" => "report",
        "55" => "webpage",
        "56" => "blogPost",
        "57" => "book",
        "58" => "report",
        "59" => "report",
        "60" => "bookSection",
        "61" => "audioRecording",
        _ => return None,
    })
}

/// `exportItemType` (:161-196).
pub(super) fn export_item_type(t: &str) -> Option<&'static str> {
    Some(match t {
        "artwork" => "Artwork",
        "audioRecording" => "Music",
        "bill" => "Bill",
        "blogPost" => "Blog",
        "book" => "Book",
        "bookSection" => "Book Section",
        "case" => "Case",
        "computerProgram" => "Computer Program",
        "conferencePaper" => "Conference Proceedings",
        "dictionaryEntry" => "Dictionary",
        "document" => "Generic",
        "mail" => "Personal Communication",
        "encyclopediaArticle" => "Encyclopedia",
        "film" => "Film or Broadcast",
        "forumPost" => "Web Page",
        "hearing" => "Hearing",
        "instantMessage" => "Personal Communication",
        "interview" => "Personal Communication",
        "journalArticle" => "Journal Article",
        "letter" => "Personal Communication",
        "magazineArticle" => "Magazine Article",
        "manuscript" => "Manuscript",
        "map" => "Map",
        "newspaperArticle" => "Newspaper Article",
        "patent" => "Patent",
        "podcast" => "Film or Broadcast",
        "presentation" => "Conference Paper",
        "radioBroadcast" => "Film or Broadcast",
        "report" => "Report",
        "statute" => "Statute",
        "thesis" => "Thesis",
        "tvBroadcast" => "Film or Broadcast",
        "videoRecording" => "Audiovisual Material",
        "webpage" => "Web Page",
        _ => return None,
    })
}

/// `exportRefNumber` (:198-234).
pub(super) fn export_ref_number(t: &str) -> Option<&'static str> {
    Some(match t {
        "artwork" => "2",
        "videoRecording" => "3",
        "bill" => "4",
        "blogPost" => "56",
        "book" => "6",
        "bookSection" => "5",
        "case" => "6",
        "computerProgram" => "9",
        "presentation" => "47",
        "conferencePaper" => "10",
        "dictionaryEntry" => "52",
        "encyclopediaArticle" => "53",
        "film" => "21",
        "podcast" => "21",
        "radioBroadcast" => "21",
        "tvBroadcast" => "21",
        "document" => "13",
        "hearing" => "14",
        "journalArticle" => "17",
        "magazineArticle" => "19",
        "manuscript" => "36",
        "map" => "20",
        "audioRecording" => "61",
        "newspaperArticle" => "23",
        "patent" => "25",
        "email" => "26",
        "instantMessage" => "26",
        "interview" => "26",
        "letter" => "26",
        "report" => "27",
        "statute" => "31",
        "thesis" => "32",
        "forumPost" => "12",
        "webpage" => "12",
        _ => return None,
    })
}

/// One `fieldMap` entry: a Zotero field for every type, or per-type
/// mappings (`__default`, `__exclude` and explicit lists, in upstream's key
/// order).
enum Mapping {
    All(&'static str),
    PerType(&'static [(&'static str, &'static [&'static str])]),
}

/// `fieldMap` (:236-466).
fn field_map(field: &str) -> Option<Mapping> {
    use Mapping::{All, PerType};
    Some(match field {
        "abstract" => All("abstractNote"),
        "call-num" => All("callNumber"),
        "electronic-resource-num" => All("DOI"),
        "remote-database-name" => All("libraryCatalog"),
        "abbr-1" => All("journalAbbreviation"),
        "short-title" => All("shortTitle"),
        "full-title" => All("publicationTitle"),
        "language" => All("language"),
        "access-date" => All("accessDate"),
        "title" => PerType(&[
            ("__default", &["title"]),
            ("subject", &["email"]),
            ("caseName", &["case"]),
            ("nameOfAct", &["statute"]),
        ]),
        "secondary-title" => PerType(&[
            ("code", &["bill", "statute"]),
            ("bookTitle", &["bookSection"]),
            ("blogTitle", &["blogPost"]),
            ("conferenceName", &["conferencePaper"]),
            ("dictionaryTitle", &["dictionaryEntry"]),
            ("encyclopediaTitle", &["encyclopediaArticle"]),
            ("committee", &["hearing"]),
            ("forumTitle", &["forumPost"]),
            ("websiteTitle", &["webpage"]),
            ("programTitle", &["radioBroadcast", "tvBroadcast"]),
            ("meetingName", &["presentation"]),
            ("seriesTitle", &["computerProgram", "map", "report"]),
            ("series", &["book"]),
            ("reporter", &["case"]),
            (
                "publicationTitle",
                &["journalArticle", "magazineArticle", "newspaperArticle"],
            ),
        ]),
        "tertiary-title" => PerType(&[
            ("legislativeBody", &["hearing", "bill"]),
            ("series", &["bookSection", "conferencePaper"]),
            ("seriesTitle", &["audioRecording"]),
        ]),
        "authors" => PerType(&[
            ("__default", &["author"]),
            ("artist", &["artwork"]),
            ("cartographer", &["map"]),
            ("composer", &["audioRecording"]),
            (
                "director",
                &["film", "radioBroadcast", "tvBroadcast", "videoRecording"],
            ),
            ("interviewee", &["interview"]),
            ("inventor", &["patent"]),
            ("podcaster", &["podcast"]),
            ("programmer", &["computerProgram"]),
        ]),
        "secondary-authors" => PerType(&[
            ("sponsor", &["bill"]),
            ("performer", &["audioRecording"]),
            ("presenter", &["presentation"]),
            ("interviewer", &["interview"]),
            (
                "editor",
                &[
                    "journalArticle",
                    "bookSection",
                    "conferencePaper",
                    "dictionaryEntry",
                    "document",
                    "encyclopediaArticle",
                ],
            ),
            ("seriesEditor", &["book", "report"]),
            ("recipient", &["email", "instantMessage", "letter"]),
            ("issuingAuthority", &["patent"]),
        ]),
        "tertiary-authors" => PerType(&[
            ("cosponsor", &["bill"]),
            (
                "producer",
                &["film", "tvBroadcast", "videoRecording", "radioBroadcast"],
            ),
            ("editor", &["book"]),
            (
                "seriesEditor",
                &[
                    "bookSection",
                    "conferencePaper",
                    "dictionaryEntry",
                    "encyclopediaArticle",
                    "map",
                ],
            ),
        ]),
        "subsidiary-authors" => PerType(&[
            ("__default", &["translator"]),
            ("counsel", &["case"]),
            (
                "castMember",
                &["radioBroadcast", "tvBroadcast", "videoRecording"],
            ),
            ("contributor", &["conferencePaper", "film"]),
        ]),
        "work-type" => PerType(&[
            ("manuscriptType", &["manuscript"]),
            ("websiteType", &["webpage"]),
            ("genre", &["film"]),
            ("postType", &["forumPost"]),
            ("letterType", &["letter"]),
            ("mapType", &["map"]),
            ("presentationType", &["presentation"]),
            ("reportType", &["report"]),
            ("thesisType", &["thesis"]),
        ]),
        "custom1" => PerType(&[
            ("filingDate", &["patent"]),
            ("scale", &["map"]),
            ("place", &["conferencePaper"]),
        ]),
        "custom2" => PerType(&[("issueDate", &["patent"])]),
        "custom3" => PerType(&[
            ("artworkSize", &["artwork"]),
            ("proceedingsTitle", &["conferencePaper"]),
            ("runningTime", &["videoRecording"]),
            ("country", &["patent"]),
        ]),
        "custom4" => PerType(&[
            ("creators/attorneyAgent", &["patent"]),
            ("genre", &["film"]),
        ]),
        "custom5" => PerType(&[
            ("references", &["patent"]),
            (
                "audioRecordingFormat",
                &["audioRecording", "radioBroadcast"],
            ),
            (
                "videoRecordingFormat",
                &["film", "tvBroadcast", "videoRecording"],
            ),
        ]),
        "custom6" => PerType(&[("legalStatus", &["patent"])]),
        "pub-location" => PerType(&[
            ("__default", &["place"]),
            ("__exclude", &["conferencePaper"]),
        ]),
        "pub-dates" | "year" => PerType(&[
            ("__default", &["date"]),
            ("dateEnacted", &["statute"]),
            ("dateDecided", &["case"]),
            ("issueDate", &["patent"]),
        ]),
        "edition" => PerType(&[
            ("__default", &["edition"]),
            ("session", &["bill", "hearing", "statute"]),
            ("version", &["computerProgram"]),
        ]),
        "issue" => PerType(&[
            ("__default", &["issue"]),
            ("numberOfVolumes", &["bookSection"]),
        ]),
        "misc1" => PerType(&[
            ("seriesNumber", &["book"]),
            ("billNumber", &["bill"]),
            ("system", &["computerProgram"]),
            ("documentNumber", &["hearing"]),
            ("applicationNumber", &["patent"]),
            ("publicLawNumber", &["statute"]),
            (
                "episodeNumber",
                &["podcast", "radioBroadcast", "tvBroadcast"],
            ),
        ]),
        "misc2" => PerType(&[
            ("manuscriptType", &["manuscript"]),
            ("mapType", &["map"]),
            ("reportType", &["report"]),
            ("thesisType", &["thesis"]),
            ("websiteType", &["blogPost", "webpage"]),
            ("postType", &["forumPost"]),
            ("letterType", &["letter"]),
            ("interviewMedium", &["interview"]),
            ("presentationType", &["presentation"]),
            ("artworkMedium", &["artwork"]),
            ("audioFileType", &["podcast"]),
        ]),
        "num-vols" => PerType(&[
            ("__default", &["numberOfVolumes"]),
            ("__exclude", &["bookSection"]),
        ]),
        "orig-pub" => PerType(&[
            ("history", &["hearing", "statute", "bill", "case"]),
            ("priorityNumbers", &["patent"]),
        ]),
        "publisher" => PerType(&[
            ("__default", &["publisher"]),
            ("label", &["audioRecording"]),
            ("court", &["case"]),
            ("distributor", &["film"]),
            ("assignee", &["patent"]),
            ("institution", &["report"]),
            ("university", &["thesis"]),
            ("company", &["computerProgram"]),
            ("studio", &["videoRecording"]),
            ("network", &["radioBroadcast", "tvBroadcast"]),
        ]),
        "section" => PerType(&[("__default", &["section"]), ("__exclude", &["case"])]),
        "isbn" => PerType(&[
            ("__default", &["ISBN"]),
            (
                "ISSN",
                &["journalArticle", "magazineArticle", "newspaperArticle"],
            ),
            ("patentNumber", &["patent"]),
            ("reportNumber", &["report"]),
        ]),
        "pages" => PerType(&[
            ("__default", &["pages"]),
            ("codePages", &["bill"]),
            ("numPages", &["book", "thesis", "manuscript"]),
            ("firstPage", &["case"]),
            ("runningTime", &["film"]),
        ]),
        "number" => PerType(&[
            ("seriesNumber", &["bookSection", "book"]),
            ("issue", &["journalArticle", "magazineArticle"]),
            ("docketNumber", &["case"]),
            ("artworkSize", &["artwork"]),
        ]),
        "volume" => PerType(&[
            ("__default", &["volume"]),
            ("codeNumber", &["statute"]),
            ("codeVolume", &["bill"]),
            ("reporterVolume", &["case"]),
            ("__exclude", &["patent", "webpage"]),
        ]),
        _ => return None,
    })
}

/// `getField(field, type)` (:468-508): the Zotero field (or
/// `creators/<type>`, or a creator type for the author fields) an Endnote
/// field maps to for an item type; `None` is upstream's `false`. (The
/// `__default` entries hold a one-element list here; upstream holds the
/// string.)
pub(super) fn get_field(field: &str, item_type: &str) -> Option<&'static str> {
    match field_map(field)? {
        Mapping::All(f) => Some(f),
        Mapping::PerType(entries) => {
            let mut def = None;
            let mut exclude = false;
            let mut zfield = None;
            for (f, types) in entries {
                if *f == "__default" {
                    def = types.first().copied();
                    continue;
                }
                if *f == "__exclude" {
                    if types.contains(&item_type) {
                        exclude = true;
                    }
                    continue;
                }
                if types.contains(&item_type) {
                    zfield = Some(*f);
                    break;
                }
            }
            if zfield.is_none() && !exclude {
                zfield = def;
            }
            zfield
        }
    }
}
