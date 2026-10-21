// Part of the kovan Zotero port (GitHub #747, #749).
//
// Upstream: Zotero translators, https://github.com/zotero/translators
//   (commit 3d1c78530f42): RIS.js — exportTypeMap :83-112 merged with
//   degenerateExportTypeMap :118-126 (:178-180); importTypeMap :131-161
//   supplemented from exportTypeMap (:165-167; the dataset branch :170-175
//   does not apply, since `title` is valid for `dataset`); fieldMap
//   :207-435; degenerateImportFieldMap :440-504; exportOrder :1920-2010;
//   ProCiteCleaner.proCiteMap :872-918.
// Copyright (C) 2006-2023 Simon Kornblith, Aurimas Vinckevicus, Abe Jellinek.
// Licence: AGPL-3.0 (upstream: AGPL-3.0-or-later).
//
// GENERATED mechanically, not hand-copied: a node one-off evaluated RIS.js's
// top-level definitions (with stub Zotero/ZU objects) and printed each table
// in its JavaScript key order (the order getField and reverseLookup iterate).

//! The RIS translator's tables.

/// The value under a field in a type-dependent tag map.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sel {
    /// A list of item types (an explicit mapping, or `__exclude`).
    Types(&'static [&'static str]),
    /// A field name (`__default`).
    Field(&'static str),
}

/// A tag's mapping: one field for every item type, or item-type dependent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TagMap {
    /// The same Zotero field for every item type.
    Field(&'static str),
    /// `{ field: [itemTypes], __default: field, __exclude: [itemTypes] }`.
    ByType(&'static [(&'static str, Sel)]),
}

/// `exportTypeMap` after the degenerate types are merged in (:83-126, :178-180).
pub static EXPORT_TYPE_MAP: &[(&str, &str)] = &[
    ("artwork", "ART"),
    ("audioRecording", "SOUND"),
    ("bill", "BILL"),
    ("blogPost", "BLOG"),
    ("book", "BOOK"),
    ("bookSection", "CHAP"),
    ("case", "CASE"),
    ("computerProgram", "COMP"),
    ("conferencePaper", "CONF"),
    ("dictionaryEntry", "DICT"),
    ("encyclopediaArticle", "ENCYC"),
    ("email", "ICOMM"),
    ("dataset", "DATA"),
    ("film", "MPCT"),
    ("hearing", "HEAR"),
    ("journalArticle", "JOUR"),
    ("letter", "PCOMM"),
    ("magazineArticle", "MGZN"),
    ("manuscript", "MANSCPT"),
    ("map", "MAP"),
    ("newspaperArticle", "NEWS"),
    ("patent", "PAT"),
    ("presentation", "SLIDE"),
    ("report", "RPRT"),
    ("statute", "STAT"),
    ("thesis", "THES"),
    ("videoRecording", "VIDEO"),
    ("webpage", "ELEC"),
    ("interview", "PCOMM"),
    ("instantMessage", "ICOMM"),
    ("forumPost", "ICOMM"),
    ("tvBroadcast", "MPCT"),
    ("radioBroadcast", "SOUND"),
    ("podcast", "SOUND"),
    ("document", "GEN"),
];

/// `importTypeMap` after `exportTypeMap` is added (:131-167).
pub static IMPORT_TYPE_MAP: &[(&str, &str)] = &[
    ("ABST", "journalArticle"),
    ("ADVS", "film"),
    ("AGGR", "document"),
    ("ANCIENT", "document"),
    ("CHART", "artwork"),
    ("CLSWK", "book"),
    ("CPAPER", "conferencePaper"),
    ("CTLG", "magazineArticle"),
    ("DBASE", "dataset"),
    ("EBOOK", "book"),
    ("ECHAP", "bookSection"),
    ("EDBOOK", "book"),
    ("EJOUR", "journalArticle"),
    ("EQUA", "document"),
    ("FIGURE", "artwork"),
    ("GEN", "journalArticle"),
    ("GOVDOC", "report"),
    ("GRNT", "document"),
    ("INPR", "manuscript"),
    ("JFULL", "journalArticle"),
    ("LEGAL", "case"),
    ("MULTI", "videoRecording"),
    ("MUSIC", "audioRecording"),
    ("PAMP", "manuscript"),
    ("SER", "book"),
    ("STAND", "report"),
    ("UNBILL", "manuscript"),
    ("UNPD", "manuscript"),
    ("WEB", "webpage"),
    ("ART", "artwork"),
    ("SOUND", "audioRecording"),
    ("BILL", "bill"),
    ("BLOG", "blogPost"),
    ("BOOK", "book"),
    ("CHAP", "bookSection"),
    ("CASE", "case"),
    ("COMP", "computerProgram"),
    ("CONF", "conferencePaper"),
    ("DICT", "dictionaryEntry"),
    ("ENCYC", "encyclopediaArticle"),
    ("ICOMM", "email"),
    ("DATA", "dataset"),
    ("MPCT", "film"),
    ("HEAR", "hearing"),
    ("JOUR", "journalArticle"),
    ("PCOMM", "letter"),
    ("MGZN", "magazineArticle"),
    ("MANSCPT", "manuscript"),
    ("MAP", "map"),
    ("NEWS", "newspaperArticle"),
    ("PAT", "patent"),
    ("SLIDE", "presentation"),
    ("RPRT", "report"),
    ("STAT", "statute"),
    ("THES", "thesis"),
    ("VIDEO", "videoRecording"),
    ("ELEC", "webpage"),
];

/// `fieldMap` (:207-435).
pub static FIELD_MAP: &[(&str, TagMap)] = &[
    ("AB", TagMap::Field("abstractNote")),
    ("AN", TagMap::Field("archiveLocation")),
    ("CN", TagMap::Field("callNumber")),
    ("DB", TagMap::Field("archive")),
    ("DO", TagMap::Field("DOI")),
    ("DP", TagMap::Field("libraryCatalog")),
    ("J2", TagMap::Field("journalAbbreviation")),
    ("KW", TagMap::Field("tags")),
    ("L1", TagMap::Field("attachments/PDF")),
    ("L2", TagMap::Field("attachments/HTML")),
    ("L4", TagMap::Field("attachments/other")),
    ("N1", TagMap::Field("notes")),
    ("ST", TagMap::Field("shortTitle")),
    ("UR", TagMap::Field("url")),
    ("Y2", TagMap::Field("accessDate")),
    ("TI", TagMap::ByType(&[
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
        ("series", Sel::Types(&["bookSection", "conferencePaper", "journalArticle"])),
        ("seriesTitle", Sel::Types(&["audioRecording"])),
    ])),
    ("AU", TagMap::ByType(&[
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
        ("creators/seriesEditor", Sel::Types(&["book", "report"])),
        ("creators/recipient", Sel::Types(&["email", "instantMessage", "letter"])),
        ("reporter", Sel::Types(&["case"])),
        ("issuingAuthority", Sel::Types(&["patent"])),
    ])),
    ("A3", TagMap::ByType(&[
        ("creators/contributor", Sel::Types(&["thesis"])),
        ("creators/cosponsor", Sel::Types(&["bill"])),
        ("creators/producer", Sel::Types(&["film", "tvBroadcast", "videoRecording", "radioBroadcast"])),
        ("creators/editor", Sel::Types(&["book"])),
        ("creators/seriesEditor", Sel::Types(&["bookSection", "conferencePaper", "dictionaryEntry", "encyclopediaArticle", "map"])),
    ])),
    ("A4", TagMap::ByType(&[
        ("__default", Sel::Field("creators/translator")),
        ("creators/counsel", Sel::Types(&["case"])),
        ("creators/contributor", Sel::Types(&["conferencePaper", "film", "dataset"])),
    ])),
    ("C1", TagMap::ByType(&[
        ("filingDate", Sel::Types(&["patent"])),
        ("creators/castMember", Sel::Types(&["radioBroadcast", "tvBroadcast", "videoRecording"])),
        ("scale", Sel::Types(&["map"])),
        ("place", Sel::Types(&["conferencePaper"])),
    ])),
    ("C2", TagMap::ByType(&[
        ("issueDate", Sel::Types(&["patent"])),
        ("creators/bookAuthor", Sel::Types(&["bookSection"])),
        ("creators/commenter", Sel::Types(&["blogPost"])),
    ])),
    ("C3", TagMap::ByType(&[
        ("artworkSize", Sel::Types(&["artwork"])),
        ("proceedingsTitle", Sel::Types(&["conferencePaper"])),
        ("country", Sel::Types(&["patent"])),
    ])),
    ("C4", TagMap::ByType(&[
        ("creators/wordsBy", Sel::Types(&["audioRecording"])),
        ("creators/attorneyAgent", Sel::Types(&["patent"])),
        ("genre", Sel::Types(&["film"])),
    ])),
    ("C5", TagMap::ByType(&[
        ("references", Sel::Types(&["patent"])),
        ("audioRecordingFormat", Sel::Types(&["audioRecording", "radioBroadcast"])),
        ("videoRecordingFormat", Sel::Types(&["film", "tvBroadcast", "videoRecording"])),
    ])),
    ("C6", TagMap::ByType(&[
        ("legalStatus", Sel::Types(&["patent"])),
    ])),
    ("CY", TagMap::ByType(&[
        ("__default", Sel::Field("place")),
        ("repositoryLocation", Sel::Types(&["dataset"])),
        ("__exclude", Sel::Types(&["conferencePaper"])),
    ])),
    ("DA", TagMap::ByType(&[
        ("__default", Sel::Field("date")),
        ("dateEnacted", Sel::Types(&["statute"])),
        ("dateDecided", Sel::Types(&["case"])),
        ("issueDate", Sel::Types(&["patent"])),
    ])),
    ("ET", TagMap::ByType(&[
        ("__default", Sel::Field("edition")),
        ("session", Sel::Types(&["bill", "hearing", "statute"])),
        ("versionNumber", Sel::Types(&["computerProgram", "dataset"])),
    ])),
    ("IS", TagMap::ByType(&[
        ("__default", Sel::Field("issue")),
        ("numberOfVolumes", Sel::Types(&["bookSection"])),
        ("__exclude", Sel::Types(&["dataset"])),
    ])),
    ("LA", TagMap::ByType(&[
        ("__default", Sel::Field("language")),
        ("programmingLanguage", Sel::Types(&["computerProgram"])),
    ])),
    ("M1", TagMap::ByType(&[
        ("seriesNumber", Sel::Types(&["book"])),
        ("billNumber", Sel::Types(&["bill"])),
        ("system", Sel::Types(&["computerProgram"])),
        ("documentNumber", Sel::Types(&["hearing"])),
        ("applicationNumber", Sel::Types(&["patent"])),
        ("publicLawNumber", Sel::Types(&["statute"])),
        ("episodeNumber", Sel::Types(&["podcast", "radioBroadcast", "tvBroadcast"])),
    ])),
    ("M3", TagMap::ByType(&[
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
    ("NV", TagMap::ByType(&[
        ("__default", Sel::Field("numberOfVolumes")),
        ("identifier", Sel::Types(&["dataset"])),
        ("__exclude", Sel::Types(&["bookSection"])),
    ])),
    ("OP", TagMap::ByType(&[
        ("history", Sel::Types(&["hearing", "statute", "bill", "case"])),
        ("priorityNumbers", Sel::Types(&["patent"])),
    ])),
    ("PB", TagMap::ByType(&[
        ("__default", Sel::Field("publisher")),
        ("label", Sel::Types(&["audioRecording"])),
        ("court", Sel::Types(&["case"])),
        ("distributor", Sel::Types(&["film"])),
        ("assignee", Sel::Types(&["patent"])),
        ("institution", Sel::Types(&["report"])),
        ("repository", Sel::Types(&["dataset"])),
        ("university", Sel::Types(&["thesis"])),
        ("company", Sel::Types(&["computerProgram"])),
        ("studio", Sel::Types(&["videoRecording"])),
        ("network", Sel::Types(&["radioBroadcast", "tvBroadcast"])),
    ])),
    ("PY", TagMap::ByType(&[
        ("__default", Sel::Field("date")),
        ("dateEnacted", Sel::Types(&["statute"])),
        ("dateDecided", Sel::Types(&["case"])),
        ("issueDate", Sel::Types(&["patent"])),
    ])),
    ("SE", TagMap::ByType(&[
        ("__default", Sel::Field("section")),
        ("__exclude", Sel::Types(&["case"])),
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
    ("SV", TagMap::ByType(&[
        ("seriesNumber", Sel::Types(&["bookSection"])),
        ("docketNumber", Sel::Types(&["case"])),
    ])),
    ("VL", TagMap::ByType(&[
        ("__default", Sel::Field("volume")),
        ("codeNumber", Sel::Types(&["statute"])),
        ("codeVolume", Sel::Types(&["bill"])),
        ("reporterVolume", Sel::Types(&["case"])),
        ("__exclude", Sel::Types(&["patent", "webpage"])),
    ])),
];

/// `degenerateImportFieldMap` (:440-504).
pub static DEGENERATE_IMPORT_FIELD_MAP: &[(&str, TagMap)] = &[
    ("AD", TagMap::ByType(&[
        ("__default", Sel::Field("unsupported/Author Address")),
        ("unsupported/Inventor Address", Sel::Types(&["patent"])),
    ])),
    ("AV", TagMap::Field("archiveLocation")),
    ("BT", TagMap::ByType(&[
        ("title", Sel::Types(&["book", "manuscript"])),
        ("bookTitle", Sel::Types(&["bookSection"])),
        ("__default", Sel::Field("backupPublicationTitle")),
    ])),
    ("CA", TagMap::Field("unsupported/Caption")),
    ("CR", TagMap::Field("rights")),
    ("CT", TagMap::Field("title")),
    ("CY", TagMap::Field("place")),
    ("ED", TagMap::Field("creators/editor")),
    ("EP", TagMap::Field("pages")),
    ("H1", TagMap::Field("unsupported/Library Catalog")),
    ("H2", TagMap::Field("unsupported/Call Number")),
    ("ID", TagMap::Field("__ignore")),
    ("IS", TagMap::ByType(&[
        ("versionNumber", Sel::Types(&["dataset"])),
    ])),
    ("JA", TagMap::Field("journalAbbreviation")),
    ("JF", TagMap::Field("publicationTitle")),
    ("JO", TagMap::ByType(&[
        ("__default", Sel::Field("journalAbbreviation")),
        ("conferenceName", Sel::Types(&["conferencePaper"])),
    ])),
    ("LB", TagMap::Field("unsupported/Label")),
    ("M1", TagMap::ByType(&[
        ("__default", Sel::Field("extra")),
        ("issue", Sel::Types(&["journalArticle"])),
        ("numberOfVolumes", Sel::Types(&["bookSection"])),
        ("accessDate", Sel::Types(&["webpage"])),
    ])),
    ("M2", TagMap::Field("extra")),
    ("M3", TagMap::Field("DOI")),
    ("N2", TagMap::Field("abstractNote")),
    ("NV", TagMap::Field("numberOfVolumes")),
    ("OP", TagMap::ByType(&[
        ("__default", Sel::Field("unsupported/Original Publication")),
        ("unsupported/Content", Sel::Types(&["blogPost", "computerProgram", "film", "presentation", "report", "videoRecording", "webpage"])),
    ])),
    ("RI", TagMap::ByType(&[
        ("__default", Sel::Field("unsupported/Reviewed Item")),
        ("unsupported/Article Number", Sel::Types(&["statute"])),
    ])),
    ("RN", TagMap::Field("notes")),
    ("SE", TagMap::ByType(&[
        ("unsupported/File Date", Sel::Types(&["case"])),
    ])),
    ("T1", TagMap::ByType(&[
        ("__default", Sel::Field("title")),
        ("subject", Sel::Types(&["email"])),
        ("caseName", Sel::Types(&["case"])),
        ("nameOfAct", Sel::Types(&["statute"])),
    ])),
    ("T2", TagMap::Field("backupPublicationTitle")),
    ("T3", TagMap::ByType(&[
        ("series", Sel::Types(&["book"])),
    ])),
    ("TA", TagMap::Field("unsupported/Translated Author")),
    ("TT", TagMap::Field("unsupported/Translated Title")),
    ("VL", TagMap::ByType(&[
        ("unsupported/Patent Version Number", Sel::Types(&["patent"])),
        ("accessDate", Sel::Types(&["webpage"])),
    ])),
    ("Y1", TagMap::ByType(&[
        ("__default", Sel::Field("date")),
        ("dateEnacted", Sel::Types(&["statute"])),
        ("dateDecided", Sel::Types(&["case"])),
        ("issueDate", Sel::Types(&["patent"])),
    ])),
];

/// `exportOrder.__default` (:1921-1964).
pub static EXPORT_ORDER_DEFAULT: &[&str] = &["TI", "AU", "T2", "A2", "T3", "A3", "A4", "AB", "C1", "C2", "C3", "C4", "C5", "C6", "CN", "CY", "DA", "PY", "DO", "DP", "ET", "VL", "IS", "SP", "J2", "LA", "M1", "M3", "NV", "OP", "PB", "SE", "SN", "ST", "SV", "UR", "AN", "DB", "Y2", "L1", "L2", "L4", "N1", "KW"];

/// `exportOrder.bill` (:1966-2009).
pub static EXPORT_ORDER_BILL: &[&str] = &["TI", "AU", "T2", "A2", "A3", "T3", "A4", "AB", "C1", "C2", "C3", "C4", "C5", "C6", "CN", "CY", "DA", "PY", "DO", "DP", "ET", "VL", "IS", "SP", "J2", "LA", "M1", "M3", "NV", "OP", "PB", "SE", "SN", "ST", "SV", "UR", "AN", "DB", "Y2", "L1", "L2", "L4", "N1", "KW"];

/// `proCiteMap['Author Role']` (:873-901).
pub static PROCITE_AUTHOR_ROLE: &[(&str, &str)] = &[
    ("actor", "cast-member"),
    ("author", "author"),
    ("cartographer", "cartographer"),
    ("composer", "composer"),
    ("composed", "composer"),
    ("director", "director"),
    ("directed", "director"),
    ("performer", "performer"),
    ("performed", "performer"),
    ("producer", "producer"),
    ("produced", "producer"),
    ("editor", "editor"),
    ("ed", "editor"),
    ("edited", "editor"),
    ("editor-in-chief", "editor"),
    ("compiler", "editor"),
    ("compiled", "editor"),
    ("collected", "editor"),
    ("assembled", "editor"),
    ("presenter", "presenter"),
    ("presented", "presenter"),
    ("translator", "translator"),
    ("translated", "translator"),
    ("introduction", "contributor"),
];

/// `proCiteMap` without `Author Role` (:902-917).
pub static PROCITE_MAP: &[(&str, &str)] = &[
    ("Call Number", "callNumber"),
    ("Edition", "edition"),
    ("ISBN", "ISBN"),
    ("Language", "language"),
    ("Publisher Name", "publisher"),
    ("Series Title", "series"),
    ("Proceedings Title", "proceedingsTitle"),
    ("Page(s)", "pages"),
    ("Volume ID", "volume"),
    ("Issue ID", "issue"),
    ("Issue Identification", "issue"),
    ("Series Volume ID", "seriesNumber"),
    ("Scale", "scale"),
    ("Place of Publication", "place"),
    ("Histroy", "history"),
    ("Size", "artworkSize"),
];
