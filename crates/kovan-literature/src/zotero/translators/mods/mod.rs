// Part of the kovan Zotero port (GitHub #747, #749).
//
// Upstream: Zotero translators, https://github.com/zotero/translators
//   (commit 3d1c78530f42): "MODS.js" (translatorID
//   0e2235e7-babf-413c-9acf-f27cce5f059c, lastUpdated 2022-10-31 14:01:52):
//   header :1-18, the type tables :43-320 (`fromMarcGenre`, `toMarcGenre`,
//   `dctGenres`, `fromTypeOfResource`, `toTypeOfResource`, `modsTypeRegex`,
//   `modsInternetMediaTypes`, `marcRelators`, `partialItemTypes`),
//   `detectImport` :325-338; import and export in this module's files.
// Copyright (c) 2019-2021 Simon Kornblith, Richard Karnesky, and Abe Jellinek.
// Licence: AGPL-3.0 (upstream: AGPL-3.0-or-later).

//! The MODS translator: import and export.
//!
//! | File | Upstream |
//! |---|---|
//! | this one | header, type tables, `detectImport` |
//! | [`import`] | `processTitleInfo` ... `processIdentifiers`, `getFirstResult`, `doImport` (:699-1440) |
//! | [`export`] | `mapProperty`, `doExport` (:351-697) |
//!
//! **Maturity: AI draft (1).** Verified code-to-code against the Zotero
//! translation-server (`tests/zotero_xml_translators.rs`, `mods_*`).

pub mod export;
pub mod import;

use crate::zotero::framework::options::{translator_type, HeaderValue, TranslatorMetadata};
use crate::zotero::framework::xml::XmlDocument;
use crate::zotero::framework::{ExportContext, ImportContext, TranslateError};

/// The translator header.
pub static METADATA: TranslatorMetadata = TranslatorMetadata {
    id: "0e2235e7-babf-413c-9acf-f27cce5f059c",
    label: "MODS",
    creator: "Simon Kornblith, Richard Karnesky, and Abe Jellinek",
    target: "xml",
    min_version: "2.1.9",
    priority: 50,
    translator_type: translator_type::IMPORT | translator_type::EXPORT,
    config_options: &[("dataMode", HeaderValue::Str("xml/dom"))],
    display_options: &[("exportNotes", HeaderValue::Bool(true))],
    hidden_prefs: &[],
    last_updated: "2022-10-31 14:01:52",
};

/// The MODS namespace (`ns`, :322).
pub const NS: &str = "http://www.loc.gov/mods/v3";

/// `xns` (:323).
pub const XNS: &[(&str, &str)] = &[("m", NS)];

/// `fromMarcGenre` (:43-148), in upstream's key order.
pub const FROM_MARC_GENRE: &[(&str, &str)] = &[
    ("art reproduction", "artwork"),
    ("article", "journalArticle"),
    ("autobiography", "book"),
    ("bibliography", "book"),
    ("biography", "book"),
    ("book", "book"),
    ("chart", "artwork"),
    ("comic or graphic novel", "book"),
    ("comic", "book"),
    ("graphic novel", "book"),
    ("comic strip", "artwork"),
    ("conference publication", "conferencePaper"),
    ("dictionary", "dictionaryEntry"),
    ("diorama", "artwork"),
    ("drama", "book"),
    ("encyclopedia", "encyclopediaArticle"),
    ("festschrift", "book"),
    ("fiction", "book"),
    ("filmstrip", "videoRecording"),
    ("folktale", "book"),
    ("graphic", "artwork"),
    ("globe", "map"),
    ("handbook", "book"),
    ("history", "book"),
    ("hymnal", "book"),
    ("humor,satire", "book"),
    ("humor", "book"),
    ("satire", "book"),
    ("journal", "journalArticle"),
    ("kit", "artwork"),
    ("law report or digest", "journalArticle"),
    ("law report", "journalArticle"),
    ("digest", "journalArticle"),
    ("law digest", "journalArticle"),
    ("legal article", "journalArticle"),
    ("legal case and case notes", "case"),
    ("legal case", "case"),
    ("case notes", "case"),
    ("legislation", "statute"),
    ("loose-leaf", "manuscript"),
    ("map", "map"),
    ("memoir", "book"),
    ("microscope slide", "artwork"),
    ("model", "artwork"),
    ("novel", "book"),
    ("online system or service", "webpage"),
    ("online system", "webpage"),
    ("service", "webpage"),
    ("online service", "webpage"),
    ("patent", "patent"),
    ("periodical", "journalArticle"),
    ("picture", "artwork"),
    ("realia", "artwork"),
    ("script", "book"),
    ("slide", "artwork"),
    ("sound", "audioRecording"),
    ("speech", "audioRecording"),
    ("standard or specification", "report"),
    ("standard", "report"),
    ("technical report", "report"),
    ("newspaper", "newspaperArticle"),
    ("theses", "thesis"),
    ("thesis", "thesis"),
    ("transparency", "artwork"),
    ("videorecording", "videoRecording"),
    ("letter", "letter"),
    ("motion picture", "film"),
    ("art original", "artwork"),
    ("web site", "webpage"),
    ("yearbook", "book"),
];

/// `toMarcGenre` (:150-185).
pub const TO_MARC_GENRE: &[(&str, &str)] = &[
    ("artwork", "art original"),
    ("audioRecording", "sound"),
    ("bill", "legislation"),
    ("blogPost", "web site"),
    ("book", "book"),
    ("bookSection", "book"),
    ("case", "legal case and case notes"),
    ("conferencePaper", "conference publication"),
    ("dictionaryEntry", "dictionary"),
    ("email", "letter"),
    ("encyclopediaArticle", "encyclopedia"),
    ("film", "motion picture"),
    ("forumPost", "web site"),
    ("hearing", "government publication"),
    ("instantMessage", "letter"),
    ("interview", "interview"),
    ("journalArticle", "journal"),
    ("letter", "letter"),
    ("magazineArticle", "periodical"),
    ("map", "map"),
    ("newspaperArticle", "newspaper"),
    ("patent", "patent"),
    ("podcast", "speech"),
    ("radioBroadcast", "sound"),
    ("report", "technical report"),
    ("statute", "legislation"),
    ("thesis", "thesis"),
    ("videoRecording", "videorecording"),
    ("webpage", "web site"),
];

/// `dctGenres` (:187-203).
pub const DCT_GENRES: &[(&str, &str)] = &[
    ("image", "artwork"),
    ("interactiveresource", "webpage"),
    ("movingimage", "videoRecording"),
    ("software", "computerProgram"),
    ("sound", "audioRecording"),
    ("stillimage", "artwork"),
];

/// `fromTypeOfResource` (:205-216).
pub const FROM_TYPE_OF_RESOURCE: &[(&str, &str)] = &[
    ("cartographic", "map"),
    ("sound recording-musical", "audioRecording"),
    ("sound recording-nonmusical", "audioRecording"),
    ("sound recording", "audioRecording"),
    ("still image", "artwork"),
    ("moving image", "videoRecording"),
    ("software, multimedia", "computerProgram"),
];

/// `toTypeOfResource` (:218-253).
pub const TO_TYPE_OF_RESOURCE: &[(&str, &str)] = &[
    ("artwork", "still image"),
    ("audioRecording", "sound recording"),
    ("bill", "text"),
    ("blogPost", "software, multimedia"),
    ("book", "text"),
    ("bookSection", "text"),
    ("case", "text"),
    ("computerProgram", "software, multimedia"),
    ("conferencePaper", "text"),
    ("dictionaryEntry", "text"),
    ("document", "text"),
    ("email", "text"),
    ("encyclopediaArticle", "text"),
    ("film", "moving image"),
    ("forumPost", "text"),
    ("hearing", "text"),
    ("instantMessage", "text"),
    ("interview", "text"),
    ("journalArticle", "text"),
    ("letter", "text"),
    ("magazineArticle", "text"),
    ("manuscript", "text"),
    ("map", "cartographic"),
    ("newspaperArticle", "text"),
    ("patent", "text"),
    ("podcast", "sound recording-nonmusical"),
    ("presentation", "mixed material"),
    ("radioBroadcast", "sound recording-nonmusical"),
    ("report", "text"),
    ("statute", "text"),
    ("thesis", "text"),
    ("tvBroadcast", "moving image"),
    ("videoRecording", "moving image"),
    ("webpage", "software, multimedia"),
];

/// `modsTypeRegex` (:255-289): (type, case-insensitive pattern), in key
/// order. `newspaper\*article` is upstream's (a literal `*`).
pub const MODS_TYPE_REGEX: &[(&str, &str)] = &[
    ("blogPost", r"(?-u:\b)blog"),
    (
        "journalArticle",
        r"journal[\t\n\x0B\x0C\r \x{A0}\x{1680}\x{2000}-\x{200A}\x{2028}\x{2029}\x{202F}\x{205F}\x{3000}\x{FEFF}]*article",
    ),
    (
        "magazineArticle",
        r"magazine[\t\n\x0B\x0C\r \x{A0}\x{1680}\x{2000}-\x{200A}\x{2028}\x{2029}\x{202F}\x{205F}\x{3000}\x{FEFF}]*article",
    ),
    ("newspaperArticle", r"newspaper\*article"),
];

/// `modsInternetMediaTypes` (:291-294).
pub const MODS_INTERNET_MEDIA_TYPES: &[(&str, &str)] = &[("text/html", "webpage")];

/// `marcRelators` (:296-307).
pub const MARC_RELATORS: &[(&str, &str)] = &[
    ("aut", "author"),
    ("edt", "editor"),
    ("ctb", "contributor"),
    ("pbd", "seriesEditor"),
    ("trl", "translator"),
    ("cmp", "composer"),
    ("lyr", "wordsBy"),
    ("prf", "performer"),
    ("cre", "author"),
    ("rcp", "recipient"),
];

/// `partialItemTypes` (:310-319).
pub const PARTIAL_ITEM_TYPES: &[&str] = &[
    "blogPost",
    "bookSection",
    "conferencePaper",
    "dictionaryEntry",
    "encyclopediaArticle",
    "forumPost",
    "journalArticle",
    "magazineArticle",
    "newspaperArticle",
    "webpage",
];

/// A lookup in one of the tables (`table[key]`).
pub fn lookup(table: &[(&str, &'static str)], key: &str) -> Option<&'static str> {
    table.iter().find(|(k, _)| *k == key).map(|(_, v)| *v)
}

/// `detectImport` (:325-338): the root element is in the MODS namespace and
/// its tag name ends with `modsCollection` or `mods`.
pub fn detect_import(ctx: &mut ImportContext) -> bool {
    let Ok(doc) = XmlDocument::parse(&ctx.input.text()) else {
        return false;
    };
    let Some(root) = doc.document_element() else {
        return false;
    };
    let tag = doc.tag_name(root);
    doc.namespace_uri(root.into()) == Some(NS)
        && (tag.ends_with("modsCollection") || tag.ends_with("mods"))
}

/// `doImport`.
pub fn do_import(ctx: &mut ImportContext) -> Result<(), TranslateError> {
    import::do_import(ctx)
}

/// `doExport`.
pub fn do_export(ctx: &mut ExportContext) -> Result<(), TranslateError> {
    export::do_export(ctx)
}
