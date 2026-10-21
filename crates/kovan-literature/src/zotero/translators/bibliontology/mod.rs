// Part of the kovan Zotero port (GitHub #747, #749).
//
// Upstream: Zotero translators, https://github.com/zotero/translators
//   (commit 3d1c78530f42): "Bibliontology RDF.js" (translatorID
//   14763d25-8ba0-45df-8f52-b8d1108e7ac9, lastUpdated 2022-09-30 10:56:50):
//   `n` :22-37, `TYPES` :64-117, `BIBO_TYPES` :122-142, node constants
//   :144-150, `FIELDS` :181-282, `CREATOR_LISTS` :284-291, `CREATORS`
//   :293-316, `SAME_ITEM_RELATIONS` :318-320, `getBlankNode` :327-357,
//   `getStatementsByDefinition` :773-784; import and export in the
//   submodules.
// Copyright (c) Simon Kornblith, Corporation for Digital Scholarship.
// Licence: AGPL-3.0. Upstream carries no licence text; treated as AGPLv3 as
//   part of Zotero, maintainer decision 2026-10-07, #747.

//! The Bibliontology RDF translator (BIBO, FOAF, Dublin Core terms):
//! import and export.

mod export;
mod import;

pub use export::do_export;
pub use import::{detect_import, do_import};

use crate::zotero::framework::options::{translator_type, HeaderValue, TranslatorMetadata};
use crate::zotero::framework::rdf::{RdfSandbox, RdfValue, Res, Triple};
use crate::zotero::framework::TranslateError;

/// The translator header.
pub static METADATA: TranslatorMetadata = TranslatorMetadata {
    id: "14763d25-8ba0-45df-8f52-b8d1108e7ac9",
    label: "Bibliontology RDF",
    creator: "Simon Kornblith",
    target: "rdf",
    min_version: "2.0",
    priority: 50,
    translator_type: translator_type::IMPORT | translator_type::EXPORT,
    config_options: &[
        ("getCollections", HeaderValue::Str("true")),
        ("dataMode", HeaderValue::Str("rdf/xml")),
    ],
    display_options: &[("exportNotes", HeaderValue::Bool(true))],
    hidden_prefs: &[],
    last_updated: "2022-09-30 10:56:50",
};

/// `n` (:22-37), in declaration order.
pub(crate) const NAMESPACES: [(&str, &str); 14] = [
    ("address", ADDRESS),
    ("bibo", BIBO),
    ("ctag", CTAG),
    ("dcterms", DCTERMS),
    ("doap", DOAP),
    ("foaf", FOAF),
    ("link", LINK),
    ("po", PO),
    ("rdf", RDF),
    ("rel", REL),
    ("res", RES),
    ("sc", SC),
    ("sioct", SIOCT),
    ("z", Z),
];

pub(crate) const ADDRESS: &str = "http://schemas.talis.com/2005/address/schema#";
pub(crate) const BIBO: &str = "http://purl.org/ontology/bibo/";
pub(crate) const CTAG: &str = "http://commontag.org/ns#";
pub(crate) const DCTERMS: &str = "http://purl.org/dc/terms/";
pub(crate) const DOAP: &str = "http://usefulinc.com/ns/doap#";
pub(crate) const FOAF: &str = "http://xmlns.com/foaf/0.1/";
pub(crate) const LINK: &str = "http://purl.org/rss/1.0/modules/link/";
pub(crate) const PO: &str = "http://purl.org/ontology/po/";
pub(crate) const RDF: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#";
pub(crate) const REL: &str = "http://www.loc.gov/loc.terms/relators/";
pub(crate) const RES: &str = "http://purl.org/vocab/resourcelist/schema#";
pub(crate) const SC: &str = "http://umbel.org/umbel/sc/";
pub(crate) const SIOCT: &str = "http://rdfs.org/sioc/types#";
pub(crate) const Z: &str = "http://www.zotero.org/namespaces/export#";

pub(crate) const USERITEM: usize = 1;
pub(crate) const ITEM: usize = 2;
pub(crate) const SUBCONTAINER: usize = 3;
pub(crate) const CONTAINER: usize = 4;
pub(crate) const ITEM_SERIES: usize = 5;
pub(crate) const SUBCONTAINER_SERIES: usize = 6;
pub(crate) const CONTAINER_SERIES: usize = 7;

pub(crate) const AUTHOR_LIST: usize = 1;
pub(crate) const EDITOR_LIST: usize = 2;
pub(crate) const CONTRIBUTOR_LIST: usize = 3;

/// `CREATOR_LISTS[list]`.
pub(crate) fn creator_list(list: usize) -> String {
    match list {
        AUTHOR_LIST => format!("{BIBO}authorList"),
        EDITOR_LIST => format!("{BIBO}editorList"),
        _ => format!("{BIBO}contributorList"),
    }
}

pub(crate) fn rdf_type() -> String {
    format!("{RDF}type")
}

/// A predicate/object pair.
pub(crate) type Pair = (String, String);

fn pr(p: &str, o: &str) -> Pair {
    (p.to_owned(), o.to_owned())
}

/// A subcontainer or container definition: `[ALWAYS_INCLUDE,
/// ITEM_PREDICATE, [[PREDICATE, OBJECT]*]]`.
#[derive(Debug, Clone)]
pub(crate) struct ContainerDef {
    pub always_add: bool,
    pub predicate: String,
    pub pairs: Vec<Pair>,
}

/// One `TYPES` entry (or a collapsed `BIBO_TYPES` one).
#[derive(Debug, Clone)]
pub(crate) struct TypeDef {
    pub zotero_type: String,
    pub item: Vec<Pair>,
    pub sub: Option<ContainerDef>,
    pub container: Option<ContainerDef>,
}

fn cdef(always: bool, pred: String, pairs: Vec<Pair>) -> Option<ContainerDef> {
    Some(ContainerDef {
        always_add: always,
        predicate: pred,
        pairs,
    })
}

/// `TYPES` (:64-117), in declaration order.
pub(crate) fn types() -> Vec<TypeDef> {
    let t = rdf_type();
    let ty = |s: &str| pr(&t, s);
    let part = format!("{DCTERMS}isPartOf");
    let issue = || cdef(true, part.clone(), vec![ty(&format!("{BIBO}Issue"))]);
    let mk =
        |z: &str, item: Vec<Pair>, sub: Option<ContainerDef>, c: Option<ContainerDef>| TypeDef {
            zotero_type: z.to_owned(),
            item,
            sub,
            container: c,
        };
    vec![
        mk("artwork", vec![ty(&format!("{BIBO}Image"))], None, None),
        mk(
            "attachment",
            vec![ty(&format!("{Z}Attachment"))],
            None,
            None,
        ),
        mk(
            "audioRecording",
            vec![ty(&format!("{BIBO}AudioDocument"))],
            None,
            None,
        ),
        mk(
            "bill",
            vec![ty(&format!("{BIBO}Bill"))],
            None,
            cdef(false, part.clone(), vec![ty(&format!("{BIBO}Code"))]),
        ),
        mk(
            "blogPost",
            vec![
                ty(&format!("{SIOCT}BlogPost")),
                ty(&format!("{BIBO}Article")),
            ],
            None,
            cdef(
                false,
                part.clone(),
                vec![ty(&format!("{SIOCT}Weblog")), ty(&format!("{BIBO}Website"))],
            ),
        ),
        mk("book", vec![ty(&format!("{BIBO}Book"))], None, None),
        mk(
            "bookSection",
            vec![ty(&format!("{BIBO}BookSection"))],
            None,
            cdef(false, part.clone(), vec![ty(&format!("{BIBO}EditedBook"))]),
        ),
        mk(
            "case",
            vec![ty(&format!("{BIBO}LegalCaseDocument"))],
            None,
            cdef(
                false,
                part.clone(),
                vec![ty(&format!("{BIBO}CourtReporter"))],
            ),
        ),
        mk(
            "computerProgram",
            vec![
                ty(&format!("{SC}ComputerProgram_CW")),
                ty(&format!("{BIBO}Document")),
            ],
            None,
            None,
        ),
        mk(
            "conferencePaper",
            vec![ty(&format!("{BIBO}Article"))],
            None,
            cdef(true, part.clone(), vec![ty(&format!("{BIBO}Proceedings"))]),
        ),
        mk(
            "dictionaryEntry",
            vec![ty(&format!("{BIBO}Article"))],
            None,
            cdef(
                true,
                part.clone(),
                vec![
                    ty(&format!("{SC}Dictionary")),
                    ty(&format!("{BIBO}ReferenceSource")),
                ],
            ),
        ),
        mk("document", vec![ty(&format!("{BIBO}Document"))], None, None),
        mk("email", vec![ty(&format!("{BIBO}Email"))], None, None),
        mk(
            "encyclopediaArticle",
            vec![ty(&format!("{BIBO}Article"))],
            None,
            cdef(
                true,
                part.clone(),
                vec![
                    ty(&format!("{SC}Encyclopedia")),
                    ty(&format!("{BIBO}ReferenceSource")),
                ],
            ),
        ),
        mk(
            "forumPost",
            vec![
                ty(&format!("{SIOCT}BoardPost")),
                ty(&format!("{BIBO}Article")),
            ],
            None,
            cdef(
                false,
                part.clone(),
                vec![
                    ty(&format!("{SIOCT}MessageBoard")),
                    ty(&format!("{BIBO}Website")),
                ],
            ),
        ),
        mk("film", vec![ty(&format!("{BIBO}Film"))], None, None),
        mk("hearing", vec![ty(&format!("{BIBO}Hearing"))], None, None),
        mk(
            "instantMessage",
            vec![
                ty(&format!("{SIOCT}InstantMessage")),
                ty(&format!("{BIBO}PersonalCommunication")),
            ],
            None,
            None,
        ),
        mk(
            "interview",
            vec![ty(&format!("{BIBO}Interview"))],
            None,
            None,
        ),
        mk(
            "journalArticle",
            vec![ty(&format!("{BIBO}AcademicArticle"))],
            issue(),
            cdef(true, part.clone(), vec![ty(&format!("{BIBO}Journal"))]),
        ),
        mk("letter", vec![ty(&format!("{BIBO}Letter"))], None, None),
        mk(
            "magazineArticle",
            vec![ty(&format!("{BIBO}Article"))],
            issue(),
            cdef(true, part.clone(), vec![ty(&format!("{BIBO}Magazine"))]),
        ),
        mk(
            "manuscript",
            vec![ty(&format!("{BIBO}Manuscript"))],
            None,
            None,
        ),
        mk("map", vec![ty(&format!("{BIBO}Map"))], None, None),
        mk(
            "newspaperArticle",
            vec![ty(&format!("{BIBO}Article"))],
            issue(),
            cdef(true, part.clone(), vec![ty(&format!("{BIBO}Newspaper"))]),
        ),
        mk("note", vec![ty(&format!("{BIBO}Note"))], None, None),
        mk("patent", vec![ty(&format!("{BIBO}Patent"))], None, None),
        mk(
            "podcast",
            vec![
                ty(&format!("{Z}Podcast")),
                ty(&format!("{BIBO}AudioDocument")),
            ],
            None,
            None,
        ),
        mk(
            "presentation",
            vec![ty(&format!("{BIBO}Slideshow"))],
            None,
            None,
        ),
        mk(
            "radioBroadcast",
            vec![
                ty(&format!("{PO}AudioDocument")),
                ty(&format!("{PO}Episode")),
                pr(&format!("{PO}broadcast_on"), &format!("{PO}Radio")),
            ],
            None,
            cdef(false, part.clone(), vec![ty(&format!("{PO}Programme"))]),
        ),
        mk("report", vec![ty(&format!("{BIBO}Report"))], None, None),
        mk(
            "statute",
            vec![ty(&format!("{BIBO}Statute"))],
            None,
            cdef(false, part.clone(), vec![ty(&format!("{BIBO}Code"))]),
        ),
        mk("thesis", vec![ty(&format!("{BIBO}Thesis"))], None, None),
        mk(
            "tvBroadcast",
            vec![
                ty(&format!("{BIBO}AudioVisualDocument")),
                ty(&format!("{PO}Episode")),
                pr(&format!("{PO}broadcast_on"), &format!("{PO}TV")),
            ],
            None,
            cdef(false, part.clone(), vec![ty(&format!("{PO}Programme"))]),
        ),
        mk(
            "videoRecording",
            vec![ty(&format!("{BIBO}AudioVisualDocument"))],
            None,
            None,
        ),
        mk(
            "webpage",
            vec![ty(&format!("{BIBO}Webpage"))],
            None,
            cdef(false, part, vec![ty(&format!("{BIBO}Website"))]),
        ),
    ]
}

/// `BIBO_TYPES` (:122-142), in declaration order.
pub(crate) const BIBO_TYPES: [(&str, &str); 19] = [
    ("Article", "magazineArticle"),
    ("Brief", "case"),
    ("Chapter", "bookSection"),
    ("CollectedDocument", "document"),
    ("DocumentPart", "document"),
    ("EditedBook", "book"),
    ("Excerpt", "note"),
    ("Quote", "note"),
    ("Film", "videoRecording"),
    ("LegalDecision", "case"),
    ("LegalDocument", "case"),
    ("Legislation", "bill"),
    ("Manual", "book"),
    ("Performance", "presentation"),
    ("PersonalCommunication", "letter"),
    ("PersonalCommunicationDocument", "letter"),
    ("Slide", "presentation"),
    ("Standard", "report"),
    ("Website", "webpage"),
];

/// A predicate as `FIELDS` and `CREATORS` give it: a string, or `[ITEM_PREDICATE,
/// [[BLANK_NODE_PREDICATE, BLANK_NODE_OBJECT]*], PREDICATE]`.
#[derive(Debug, Clone)]
pub(crate) enum Pred {
    Simple(String),
    Complex(String, Vec<Pair>, String),
}

impl Pred {
    /// `predicate` or `predicate[0]`: the key `collapsedProperties` uses.
    pub(crate) fn key(&self) -> &str {
        match self {
            Pred::Simple(p) | Pred::Complex(p, _, _) => p,
        }
    }
}

/// A `FIELDS` entry.
#[derive(Debug, Clone)]
pub(crate) enum FieldMap {
    /// `[DOMAIN, PREDICATE]`.
    Plain(usize, Pred),
    /// The `ISBN` function pair.
    Isbn,
    /// The `priorityNumbers` function pair.
    PriorityNumbers,
}

/// `FIELDS` (:181-282), in declaration order.
pub(crate) fn fields() -> Vec<(&'static str, FieldMap)> {
    let s = |d: usize, p: String| FieldMap::Plain(d, Pred::Simple(p));
    let t = rdf_type();
    let org = |n: &str| {
        FieldMap::Plain(
            CONTAINER,
            Pred::Complex(
                format!("{DCTERMS}publisher"),
                vec![pr(&t, &format!("{FOAF}Organization"))],
                n.to_owned(),
            ),
        )
    };
    let conference = || {
        FieldMap::Plain(
            ITEM,
            Pred::Complex(
                format!("{BIBO}presentedAt"),
                vec![pr(&t, &format!("{BIBO}Conference"))],
                format!("{DCTERMS}title"),
            ),
        )
    };
    vec![
        ("url", s(ITEM, format!("{BIBO}uri"))),
        ("rights", s(USERITEM, format!("{DCTERMS}rights"))),
        ("series", s(CONTAINER_SERIES, format!("{DCTERMS}title"))),
        ("volume", s(SUBCONTAINER, format!("{BIBO}volume"))),
        ("issue", s(SUBCONTAINER, format!("{BIBO}issue"))),
        ("edition", s(SUBCONTAINER, format!("{BIBO}edition"))),
        ("place", org(&format!("{ADDRESS}localityName"))),
        ("country", org(&format!("{ADDRESS}countryName"))),
        ("publisher", org(&format!("{FOAF}name"))),
        ("pages", s(ITEM, format!("{BIBO}pages"))),
        ("firstPage", s(ITEM, format!("{BIBO}pageStart"))),
        ("ISBN", FieldMap::Isbn),
        ("publicationTitle", s(CONTAINER, format!("{DCTERMS}title"))),
        ("ISSN", s(CONTAINER, format!("{BIBO}issn"))),
        ("date", s(SUBCONTAINER, format!("{DCTERMS}date"))),
        ("section", s(ITEM, format!("{BIBO}section"))),
        ("callNumber", s(SUBCONTAINER, format!("{BIBO}lccn"))),
        ("archiveLocation", s(ITEM, format!("{DCTERMS}source"))),
        ("distributor", s(SUBCONTAINER, format!("{BIBO}distributor"))),
        ("extra", s(ITEM, format!("{Z}extra"))),
        (
            "journalAbbreviation",
            s(CONTAINER, format!("{BIBO}shortTitle")),
        ),
        ("DOI", s(ITEM, format!("{BIBO}doi"))),
        ("accessDate", s(USERITEM, format!("{Z}accessDate"))),
        ("seriesTitle", s(ITEM_SERIES, format!("{DCTERMS}title"))),
        (
            "seriesText",
            s(ITEM_SERIES, format!("{DCTERMS}description")),
        ),
        ("seriesNumber", s(CONTAINER_SERIES, format!("{BIBO}number"))),
        ("code", s(CONTAINER, format!("{DCTERMS}title"))),
        ("session", conference()),
        (
            "legislativeBody",
            FieldMap::Plain(
                ITEM,
                Pred::Complex(
                    format!("{BIBO}organizer"),
                    vec![
                        pr(&t, &format!("{SC}LegalGovernmentOrganization")),
                        pr(&t, &format!("{FOAF}Organization")),
                    ],
                    format!("{FOAF}name"),
                ),
            ),
        ),
        ("history", s(ITEM, format!("{Z}history"))),
        ("reporter", s(CONTAINER, format!("{DCTERMS}title"))),
        ("court", s(CONTAINER, format!("{BIBO}court"))),
        (
            "numberOfVolumes",
            s(CONTAINER_SERIES, format!("{BIBO}numberOfVolumes")),
        ),
        (
            "committee",
            FieldMap::Plain(
                ITEM,
                Pred::Complex(
                    format!("{BIBO}organizer"),
                    vec![
                        pr(&t, &format!("{SC}Committee_Organization")),
                        pr(&t, &format!("{FOAF}Organization")),
                    ],
                    format!("{FOAF}name"),
                ),
            ),
        ),
        ("assignee", s(ITEM, format!("{Z}assignee"))),
        ("priorityNumbers", FieldMap::PriorityNumbers),
        ("references", s(ITEM, format!("{Z}references"))),
        ("legalStatus", s(ITEM, format!("{BIBO}status"))),
        ("codeNumber", s(CONTAINER, format!("{BIBO}number"))),
        ("number", s(ITEM, format!("{BIBO}number"))),
        ("artworkSize", s(ITEM, format!("{DCTERMS}extent"))),
        ("libraryCatalog", s(USERITEM, format!("{Z}repository"))),
        ("archive", s(ITEM, format!("{Z}repository"))),
        ("scale", s(ITEM, format!("{Z}scale"))),
        ("meetingName", conference()),
        ("runningTime", s(ITEM, format!("{PO}duration"))),
        ("version", s(ITEM, format!("{DOAP}revision"))),
        ("system", s(ITEM, format!("{DOAP}os"))),
        ("conferenceName", conference()),
        ("language", s(ITEM, format!("{DCTERMS}language"))),
        (
            "programmingLanguage",
            s(ITEM, format!("{DOAP}programming-language")),
        ),
        ("abstractNote", s(ITEM, format!("{DCTERMS}abstract"))),
        ("type", s(ITEM, format!("{DCTERMS}type"))),
        ("medium", s(ITEM, format!("{DCTERMS}medium"))),
        ("title", s(ITEM, format!("{DCTERMS}title"))),
        ("shortTitle", s(ITEM, format!("{BIBO}shortTitle"))),
        ("numPages", s(ITEM, format!("{BIBO}numPages"))),
        (
            "applicationNumber",
            s(ITEM, format!("{Z}applicationNumber")),
        ),
        (
            "issuingAuthority",
            FieldMap::Plain(
                ITEM,
                Pred::Complex(
                    format!("{BIBO}issuer"),
                    vec![pr(&t, &format!("{FOAF}Organization"))],
                    format!("{FOAF}name"),
                ),
            ),
        ),
        ("filingDate", s(ITEM, format!("{DCTERMS}dateSubmitted"))),
    ]
}

/// `new LiteralProperty(field).mapping`: the `FIELDS` entry, or `[ITEM,
/// z:field]`.
pub(crate) fn field_mapping(field: &str) -> FieldMap {
    fields()
        .into_iter()
        .find(|(f, _)| *f == field)
        .map(|(_, m)| m)
        .unwrap_or_else(|| FieldMap::Plain(ITEM, Pred::Simple(format!("{Z}{field}"))))
}

/// A `CREATORS` entry: `[DOMAIN, LIST, PREDICATE]`.
#[derive(Debug, Clone)]
pub(crate) struct CreatorMap {
    pub domain: usize,
    pub list: usize,
    pub pred: Pred,
}

/// `CREATORS` (:293-316), in declaration order.
pub(crate) fn creators() -> Vec<(&'static str, CreatorMap)> {
    let c = |d: usize, l: usize, p: String| CreatorMap {
        domain: d,
        list: l,
        pred: Pred::Simple(p),
    };
    vec![
        ("author", c(ITEM, AUTHOR_LIST, format!("{DCTERMS}creator"))),
        (
            "attorneyAgent",
            c(ITEM, CONTRIBUTOR_LIST, format!("{Z}attorneyAgent")),
        ),
        (
            "bookAuthor",
            c(CONTAINER, AUTHOR_LIST, format!("{DCTERMS}creator")),
        ),
        ("castMember", c(ITEM, CONTRIBUTOR_LIST, format!("{REL}ACT"))),
        (
            "commenter",
            CreatorMap {
                domain: ITEM,
                list: CONTRIBUTOR_LIST,
                pred: Pred::Complex(
                    format!("{SIOCT}has_reply"),
                    vec![pr(&rdf_type(), &format!("{SIOCT}Comment"))],
                    format!("{DCTERMS}creator"),
                ),
            },
        ),
        ("composer", c(ITEM, CONTRIBUTOR_LIST, format!("{REL}CMP"))),
        (
            "contributor",
            c(ITEM, CONTRIBUTOR_LIST, format!("{DCTERMS}contributor")),
        ),
        ("cosponsor", c(ITEM, CONTRIBUTOR_LIST, format!("{REL}SPN"))),
        ("counsel", c(ITEM, CONTRIBUTOR_LIST, format!("{Z}counsel"))),
        (
            "director",
            c(ITEM, CONTRIBUTOR_LIST, format!("{BIBO}director")),
        ),
        (
            "editor",
            c(SUBCONTAINER, EDITOR_LIST, format!("{BIBO}editor")),
        ),
        (
            "guest",
            c(ITEM, CONTRIBUTOR_LIST, format!("{PO}participant")),
        ),
        (
            "interviewer",
            c(ITEM, CONTRIBUTOR_LIST, format!("{BIBO}interviewer")),
        ),
        (
            "interviewee",
            c(ITEM, CONTRIBUTOR_LIST, format!("{BIBO}interviewee")),
        ),
        (
            "performer",
            c(ITEM, CONTRIBUTOR_LIST, format!("{BIBO}performer")),
        ),
        (
            "producer",
            c(ITEM, CONTRIBUTOR_LIST, format!("{BIBO}producer")),
        ),
        (
            "recipient",
            c(ITEM, CONTRIBUTOR_LIST, format!("{BIBO}recipient")),
        ),
        (
            "reviewedAuthor",
            CreatorMap {
                domain: ITEM,
                list: CONTRIBUTOR_LIST,
                pred: Pred::Complex(
                    format!("{BIBO}reviewOf"),
                    vec![],
                    format!("{DCTERMS}creator"),
                ),
            },
        ),
        (
            "scriptwriter",
            c(ITEM, CONTRIBUTOR_LIST, format!("{REL}AUS")),
        ),
        (
            "seriesEditor",
            c(CONTAINER_SERIES, EDITOR_LIST, format!("{BIBO}editor")),
        ),
        (
            "translator",
            c(SUBCONTAINER, CONTRIBUTOR_LIST, format!("{BIBO}translator")),
        ),
        ("wordsBy", c(ITEM, CONTRIBUTOR_LIST, format!("{REL}LYR"))),
    ]
}

/// `CREATORS[field]`.
pub(crate) fn creator_mapping(field: &str) -> Option<CreatorMap> {
    creators()
        .into_iter()
        .find(|(f, _)| *f == field)
        .map(|(_, m)| m)
}

/// `SAME_ITEM_RELATIONS` (:318-320).
pub(crate) fn same_item_relations() -> Vec<String> {
    [
        format!("{DCTERMS}isPartOf"),
        format!("{DCTERMS}isVersionOf"),
        format!("{BIBO}affirmedBy"),
        format!("{BIBO}presentedAt"),
        format!("{BIBO}presents"),
        format!("{BIBO}reproducedIn"),
        format!("{BIBO}reviewOf"),
        format!("{BIBO}translationOf"),
        format!("{BIBO}transcriptOf"),
    ]
    .to_vec()
}

pub(crate) fn err(m: impl Into<String>) -> TranslateError {
    TranslateError::Translator(m.into())
}

/// A JavaScript node value as this translator holds one: a string (a URI,
/// or a literal standing where a node was expected) or a term.
pub(crate) fn res(v: &RdfValue) -> Res {
    Res::from(v)
}

/// `getBlankNode(attachToNode, itemPredicate, blankNodePairs, create)`
/// (:327-357).
pub(crate) fn get_blank_node(
    z: &mut RdfSandbox,
    attach_to: &RdfValue,
    item_predicate: &str,
    pairs: &[Pair],
    create: bool,
) -> Result<Option<RdfValue>, TranslateError> {
    let statements1: Vec<Triple> = z
        .get_statements_matching(
            &res(attach_to),
            &Res::from(item_predicate),
            &Res::Undefined,
            false,
        )
        .unwrap_or_default();
    for st in statements1 {
        let test_node = st.2.clone();
        let mut ok = true;
        for (p, o) in pairs {
            if z.get_statements_matching_one(
                &res(&test_node),
                &Res::from(p.as_str()),
                &Res::from(o.as_str()),
                false,
            )
            .is_none()
            {
                ok = false;
                break;
            }
        }
        if ok {
            return Ok(Some(test_node));
        }
    }
    if !create {
        return Ok(None);
    }
    let b = z.new_resource();
    z.add_statement(res(attach_to), item_predicate, b.clone())
        .map_err(err)?;
    for (p, o) in pairs {
        z.add_statement(b.clone(), p.as_str(), o.as_str())
            .map_err(err)?;
    }
    Ok(Some(RdfValue::Node(b)))
}

/// `getStatementsByDefinition(definition, node)` (:773-784).
pub(crate) fn statements_by_definition(
    z: &mut RdfSandbox,
    definition: &Pred,
    node: &RdfValue,
) -> Result<Option<Vec<Triple>>, TranslateError> {
    match definition {
        Pred::Simple(p) => {
            Ok(z.get_statements_matching(&res(node), &Res::from(p.as_str()), &Res::Null, false))
        }
        Pred::Complex(p0, pairs, p2) => match get_blank_node(z, node, p0, pairs, false)? {
            Some(b) => {
                Ok(z.get_statements_matching(&res(&b), &Res::from(p2.as_str()), &Res::Null, false))
            }
            None => Ok(None),
        },
    }
}
