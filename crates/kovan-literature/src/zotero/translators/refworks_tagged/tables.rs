// Part of the kovan Zotero port (GitHub #747, #749).
//
// Upstream: Zotero translators, https://github.com/zotero/translators
//   (commit 3d1c78530f42): RefWorks Tagged.js — exportTypeMap :60-86 merged
//   with degenerateExportTypeMap :92-102 (:130-132); importTypeMap :107-120
//   supplemented from exportTypeMap (:125-127); fieldMap :141-330;
//   degenerateImportFieldMap :333-345; exportOrder :815-826.
// Copyright: no notice upstream; translator by Simon Kornblith, Aurimas
//   Vinckevicius and Sebastian Karcher (header `creator`).
// Licence: AGPL-3.0 (no licence text upstream; treated as AGPLv3 as part of
//   Zotero, maintainer decision 2026-10-07, #747).
//
// GENERATED mechanically, not hand-copied: a node one-off evaluated the
// translator's top-level definitions and printed each table in its
// JavaScript key order (the order getFields iterates).

//! The RefWorks Tagged translator's tables.

use crate::zotero::translators::ris::tables::{Sel, TagMap};

/// `exportTypeMap` with `degenerateExportTypeMap` merged in (:60-102, :130-132).
pub static EXPORT_TYPE_MAP: &[(&str, &str)] = &[
    ("artwork", "Artwork"),
    ("audioRecording", "Sound Recording"),
    ("bill", "Bills"),
    ("blogPost", "Web Page"),
    ("book", "Book, Whole"),
    ("bookSection", "Book, Section"),
    ("case", "Case"),
    ("computerProgram", "Computer Program"),
    ("conferencePaper", "Conference Proceedings"),
    ("email", "Personal Communication"),
    ("film", "Motion Picture"),
    ("forumPost", "Online Discussion Forum"),
    ("hearing", "Hearing"),
    ("journalArticle", "Journal Article"),
    ("letter", "Personal Communication"),
    ("magazineArticle", "Magazine Article"),
    ("manuscript", "Unpublished Material"),
    ("map", "Map"),
    ("newspaperArticle", "Newspaper Article"),
    ("patent", "Patent"),
    ("report", "Report"),
    ("statute", "Statutes"),
    ("thesis", "Dissertation"),
    ("videoRecording", "Video"),
    ("webpage", "Web Page"),
    ("interview", "Personal Communication"),
    ("instantMessage", "Personal Communication"),
    ("tvBroadcast", "Motion Picture"),
    ("radioBroadcast", "Sound Recording"),
    ("presentation", "Report"),
    ("podcast", "Sound Recording"),
    ("dictionaryEntry", "Book, Section"),
    ("encyclopediaArticle", "Book, Section"),
    ("document", "Generic"),
];

/// `importTypeMap` supplemented from `exportTypeMap` (:107-127).
pub static IMPORT_TYPE_MAP: &[(&str, &str)] = &[
    ("Abstract", "journalArticle"),
    ("Book, Edited", "book"),
    ("Court Decisions", "case"),
    ("DVD", "videoRecording"),
    ("Grant", "report"),
    ("Journal, Electronic", "journalArticle"),
    ("Laws", "statute"),
    ("Monograph", "book"),
    ("Music Score", "audioRecording"),
    ("Resolutions", "bill"),
    ("Thesis, Unpublished", "thesis"),
    ("Thesis", "thesis"),
    ("Artwork", "artwork"),
    ("Sound Recording", "audioRecording"),
    ("Bills", "bill"),
    ("Web Page", "webpage"),
    ("Book, Whole", "book"),
    ("Book, Section", "bookSection"),
    ("Case", "case"),
    ("Computer Program", "computerProgram"),
    ("Conference Proceedings", "conferencePaper"),
    ("Personal Communication", "letter"),
    ("Motion Picture", "film"),
    ("Online Discussion Forum", "forumPost"),
    ("Hearing", "hearing"),
    ("Journal Article", "journalArticle"),
    ("Magazine Article", "magazineArticle"),
    ("Unpublished Material", "manuscript"),
    ("Map", "map"),
    ("Newspaper Article", "newspaperArticle"),
    ("Patent", "patent"),
    ("Report", "report"),
    ("Statutes", "statute"),
    ("Dissertation", "thesis"),
    ("Video", "videoRecording"),
];

/// `fieldMap` (:141-330).
pub static FIELD_MAP: &[(&str, TagMap)] = &[
    ("AB", TagMap::Field("abstractNote")),
    ("CN", TagMap::Field("callNumber")),
    ("DO", TagMap::Field("DOI")),
    ("SL", TagMap::Field("archive")),
    ("LL", TagMap::Field("archiveLocation")),
    ("IS", TagMap::Field("issue")),
    ("JO", TagMap::Field("journalAbbreviation")),
    ("K1", TagMap::Field("tags")),
    ("LK", TagMap::Field("attachments/other")),
    ("NO", TagMap::Field("notes")),
    ("ST", TagMap::Field("shortTitle")),
    ("RD", TagMap::Field("accessDate")),
    ("UL", TagMap::Field("url")),
    ("T1", TagMap::ByType(&[
        ("__default", Sel::Field("title")),
        ("subject", Sel::Types(&["email"])),
        ("caseName", Sel::Types(&["case"])),
        ("nameOfAct", Sel::Types(&["statute"])),
    ])),
    ("T2", TagMap::ByType(&[
        ("code", Sel::Types(&["bill", "statute"])),
        ("bookTitle", Sel::Types(&["bookSection"])),
        ("blogTitle", Sel::Types(&["blogPost"])),
        ("conferenceName", Sel::Types(&["conferencePaper"])),
        ("dictionaryTitle", Sel::Types(&["dictionaryEntry"])),
        ("encyclopediaTitle", Sel::Types(&["encyclopediaArticle"])),
        ("committee", Sel::Types(&["hearing"])),
        ("forumTitle", Sel::Types(&["forumPost"])),
        ("websiteTitle", Sel::Types(&["webpage"])),
        ("programTitle", Sel::Types(&["radioBroadcast", "tvBroadcast"])),
        ("meetingName", Sel::Types(&["presentation"])),
        ("seriesTitle", Sel::Types(&["computerProgram", "map", "report"])),
        ("series", Sel::Types(&["book"])),
        ("publicationTitle", Sel::Types(&["journalArticle", "magazineArticle", "newspaperArticle"])),
    ])),
    ("T3", TagMap::ByType(&[
        ("legislativeBody", Sel::Types(&["hearing", "bill"])),
        ("series", Sel::Types(&["bookSection", "conferencePaper"])),
        ("seriesTitle", Sel::Types(&["audioRecording"])),
    ])),
    ("A1", TagMap::ByType(&[
        ("__default", Sel::Field("creators/author")),
        ("creators/artist", Sel::Types(&["artwork"])),
        ("creators/cartographer", Sel::Types(&["map"])),
        ("creators/composer", Sel::Types(&["audioRecording"])),
        ("creators/director", Sel::Types(&["film", "radioBroadcast", "tvBroadcast", "videoRecording"])),
        ("creators/interviewee", Sel::Types(&["interview"])),
        ("creators/inventor", Sel::Types(&["patent"])),
        ("creators/podcaster", Sel::Types(&["podcast"])),
        ("creators/programmer", Sel::Types(&["computerProgram"])),
    ])),
    ("A2", TagMap::ByType(&[
        ("creators/sponsor", Sel::Types(&["bill"])),
        ("creators/performer", Sel::Types(&["audioRecording"])),
        ("creators/presenter", Sel::Types(&["presentation"])),
        ("creators/interviewer", Sel::Types(&["interview"])),
        ("creators/editor", Sel::Types(&["journalArticle", "bookSection", "conferencePaper", "dictionaryEntry", "document", "encyclopediaArticle"])),
        ("creators/seriesEditor", Sel::Types(&["book"])),
        ("creators/recipient", Sel::Types(&["email", "instantMessage", "letter"])),
        ("reporter", Sel::Types(&["case"])),
        ("issuingAuthority", Sel::Types(&["patent"])),
    ])),
    ("A3", TagMap::ByType(&[
        ("creators/cosponsor", Sel::Types(&["bill"])),
        ("creators/producer", Sel::Types(&["film", "tvBroadcast", "videoRecording", "radioBroadcast"])),
        ("creators/editor", Sel::Types(&["book"])),
        ("creators/seriesEditor", Sel::Types(&["bookSection", "conferencePaper", "dictionaryEntry", "encyclopediaArticle", "map", "report"])),
    ])),
    ("A4", TagMap::ByType(&[
        ("__default", Sel::Field("creators/translator")),
        ("creators/counsel", Sel::Types(&["case"])),
        ("creators/contributor", Sel::Types(&["conferencePaper", "film"])),
    ])),
    ("U1", TagMap::ByType(&[
        ("filingDate", Sel::Types(&["patent"])),
        ("creators/castMember", Sel::Types(&["radioBroadcast", "tvBroadcast", "videoRecording"])),
        ("scale", Sel::Types(&["map"])),
        ("place", Sel::Types(&["conferencePaper"])),
    ])),
    ("U2", TagMap::ByType(&[
        ("issueDate", Sel::Types(&["patent"])),
        ("creators/bookAuthor", Sel::Types(&["bookSection"])),
        ("creators/commenter", Sel::Types(&["blogPost"])),
    ])),
    ("U3", TagMap::ByType(&[
        ("artworkSize", Sel::Types(&["artwork"])),
        ("proceedingsTitle", Sel::Types(&["conferencePaper"])),
        ("country", Sel::Types(&["patent"])),
    ])),
    ("U4", TagMap::ByType(&[
        ("creators/wordsBy", Sel::Types(&["audioRecording"])),
        ("creators/attorneyAgent", Sel::Types(&["patent"])),
        ("genre", Sel::Types(&["film"])),
    ])),
    ("U5", TagMap::ByType(&[
        ("references", Sel::Types(&["patent"])),
        ("audioRecordingFormat", Sel::Types(&["audioRecording", "radioBroadcast"])),
        ("videoRecordingFormat", Sel::Types(&["film", "tvBroadcast", "videoRecording"])),
    ])),
    ("U6", TagMap::ByType(&[
        ("legalStatus", Sel::Types(&["patent"])),
    ])),
    ("PP", TagMap::ByType(&[
        ("__default", Sel::Field("place")),
        ("__exclude", Sel::Types(&["conferencePaper"])),
    ])),
    ("FD", TagMap::ByType(&[
        ("__default", Sel::Field("date")),
        ("dateEnacted", Sel::Types(&["statute"])),
        ("dateDecided", Sel::Types(&["case"])),
        ("issueDate", Sel::Types(&["patent"])),
    ])),
    ("ED", TagMap::ByType(&[
        ("__default", Sel::Field("edition")),
        ("session", Sel::Types(&["bill", "hearing", "statute"])),
        ("version", Sel::Types(&["computerProgram"])),
    ])),
    ("LA", TagMap::ByType(&[
        ("__default", Sel::Field("language")),
        ("programmingLanguage", Sel::Types(&["computerProgram"])),
    ])),
    ("CL", TagMap::ByType(&[
        ("billNumber", Sel::Types(&["bill"])),
        ("system", Sel::Types(&["computerProgram"])),
        ("documentNumber", Sel::Types(&["hearing"])),
        ("applicationNumber", Sel::Types(&["patent"])),
        ("publicLawNumber", Sel::Types(&["statute"])),
        ("episodeNumber", Sel::Types(&["podcast", "radioBroadcast", "tvBroadcast"])),
        ("manuscriptType", Sel::Types(&["manuscript"])),
        ("mapType", Sel::Types(&["map"])),
        ("reportType", Sel::Types(&["report"])),
        ("thesisType", Sel::Types(&["thesis"])),
        ("websiteType", Sel::Types(&["blogPost", "webpage"])),
        ("postType", Sel::Types(&["forumPost"])),
        ("letterType", Sel::Types(&["letter"])),
        ("interviewMedium", Sel::Types(&["interview"])),
        ("presentationType", Sel::Types(&["presentation"])),
        ("artworkMedium", Sel::Types(&["artwork"])),
        ("audioFileType", Sel::Types(&["podcast"])),
    ])),
    ("PB", TagMap::ByType(&[
        ("__default", Sel::Field("publisher")),
        ("label", Sel::Types(&["audioRecording"])),
        ("court", Sel::Types(&["case"])),
        ("distributor", Sel::Types(&["film"])),
        ("assignee", Sel::Types(&["patent"])),
        ("institution", Sel::Types(&["report"])),
        ("university", Sel::Types(&["thesis"])),
        ("company", Sel::Types(&["computerProgram"])),
        ("studio", Sel::Types(&["videoRecording"])),
        ("network", Sel::Types(&["radioBroadcast", "tvBroadcast"])),
    ])),
    ("YR", TagMap::ByType(&[
        ("__default", Sel::Field("date")),
        ("dateEnacted", Sel::Types(&["statute"])),
        ("dateDecided", Sel::Types(&["case"])),
        ("issueDate", Sel::Types(&["patent"])),
    ])),
    ("SN", TagMap::ByType(&[
        ("__default", Sel::Field("ISBN")),
        ("ISSN", Sel::Types(&["journalArticle", "magazineArticle", "newspaperArticle"])),
        ("patentNumber", Sel::Types(&["patent"])),
        ("reportNumber", Sel::Types(&["report"])),
    ])),
    ("SP", TagMap::ByType(&[
        ("__default", Sel::Field("pages")),
        ("codePages", Sel::Types(&["bill"])),
        ("numPages", Sel::Types(&["book", "thesis", "manuscript"])),
        ("firstPage", Sel::Types(&["case"])),
        ("runningTime", Sel::Types(&["film"])),
    ])),
    ("VO", TagMap::ByType(&[
        ("__default", Sel::Field("volume")),
        ("codeNumber", Sel::Types(&["statute"])),
        ("codeVolume", Sel::Types(&["bill"])),
        ("reporterVolume", Sel::Types(&["case"])),
        ("__exclude", Sel::Types(&["patent"])),
    ])),
];

/// `degenerateImportFieldMap` (:333-345).
pub static DEGENERATE_IMPORT_FIELD_MAP: &[(&str, TagMap)] = &[
    ("OP", TagMap::Field("pages")),
    ("JF", TagMap::Field("publicationTitle")),
    ("JO", TagMap::ByType(&[
        ("__default", Sel::Field("journalAbbreviation")),
        ("conferenceName", Sel::Types(&["conferencePaper"])),
    ])),
    ("T2", TagMap::Field("backupPublicationTitle")),
    ("T3", TagMap::ByType(&[
        ("series", Sel::Types(&["book"])),
    ])),
];

/// `exportOrder.__default` (:818-820; "OP," with its comma, as upstream).
pub static EXPORT_ORDER_DEFAULT: &[&str] = &["T1", "A1", "T2", "A2", "T3", "A3", "A4", "AB", "U1", "U2", "U3", "U4", "U5", "U6", "CN", "PP", "FD", "YR", "DO", "SL", "LL", "ED", "VO", "IS", "SP", "OP,", "JO", "LA", "CL", "PB", "SN", "ST", "UL", "RD", "LK", "NO", "K1"];

/// `exportOrder.bill` (:822-824).
pub static EXPORT_ORDER_BILL: &[&str] = &["T1", "A1", "T2", "A2", "A3", "T3", "A4", "AB", "U1", "U2", "U3", "U4", "U5", "U6", "CN", "PP", "FD", "YR", "DO", "SL", "LL", "ED", "VO", "IS", "SP", "OP", "JO", "LA", "CL", "PB", "SN", "ST", "UL", "RD", "LK", "NO", "K1"];
