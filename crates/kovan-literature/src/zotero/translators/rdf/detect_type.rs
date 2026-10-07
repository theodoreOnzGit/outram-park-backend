// Part of the kovan Zotero port (GitHub #747, #749).
//
// Upstream: Zotero translators, https://github.com/zotero/translators
//   (commit 3d1c78530f42): "RDF.js" `detectType` :277-860.
// Copyright (c) 2011 Center for History and New Media, George Mason
//   University, Fairfax, Virginia, USA (Simon Kornblith).
// Licence: AGPL-3.0 (upstream: AGPL-3.0-or-later).

//! `detectType(newItem, node, ret)`: the item type from `rdf:type`,
//! `z:itemType`, `dc:type`, `eprints:type` and `og:type`, and the container
//! nodes (journal, book, periodical, ...) the rest of the import reads.
//!
//! Upstream also reads `prism:aggregationtype`, `prism:genre` and
//! `prism:platform`, but without `onlyOneString`, so the value it switches on
//! is an array and no case ever matches; those reads have no effect and are
//! not ported.

use super::ns::*;
use super::{Rdf, V};
use crate::zotero::framework::rdf::RDF_NS;
use crate::zotero::framework::utilities::item_type_exists;
use crate::zotero::framework::TranslatorItem;

/// What `detectType` hands back in `ret`.
#[derive(Debug, Clone)]
pub(crate) struct Containers {
    pub container: V,
    pub container_periodical: V,
    pub container_publication_volume: V,
    pub is_part_of: V,
}

/// `isNaN(parseInt(s))`: no digit after optional leading whitespace and sign.
pub(crate) fn parse_int_is_nan(s: &str) -> bool {
    let t = crate::zotero::framework::js::trim_start(s);
    let t = t.strip_prefix(['+', '-']).unwrap_or(t);
    !t.chars().next().is_some_and(|c| c.is_ascii_digit())
}

/// The item type field values `t.*` collects.
#[derive(Debug, Default)]
struct T {
    zotero: Option<String>,
    bib: Option<String>,
    so: Option<String>,
    z: Option<String>,
    eprints: Option<String>,
    og: Option<String>,
    dc: Option<String>,
    so_guess: Option<String>,
    og_guess: Option<String>,
    dc_guess: Option<String>,
}

fn s(x: &str) -> Option<String> {
    Some(x.to_owned())
}

fn l(xs: &[&str]) -> Vec<String> {
    xs.iter().map(|x| x.to_string()).collect()
}

impl Rdf<'_> {
    /// `ZU.fieldIsValidForType('title', 'dataset') ? 'dataset' : 'document'`.
    fn dataset_type() -> &'static str {
        if crate::zotero::framework::utilities::field_is_valid_for_type("title", "dataset") {
            "dataset"
        } else {
            "document"
        }
    }

    /// Whether `a` and `b` are the same value under `includes`
    /// (SameValueZero: strings by value, objects by identity).
    fn same_value(a: &V, b: &V) -> bool {
        match (a, b) {
            (V::Str(x), V::Str(y)) => x == y,
            (V::Node(x), V::Node(y)) => x.same_object(y),
            (V::Undef, V::Undef) | (V::False, V::False) => true,
            _ => false,
        }
    }

    /// `detectType(newItem, node, ret)` (:277-860).
    pub(crate) fn detect_type(
        &mut self,
        item: &mut TranslatorItem,
        node: &V,
    ) -> (Option<String>, Containers) {
        let mut ret = Containers {
            container: V::Undef,
            container_periodical: V::Undef,
            container_publication_volume: V::Undef,
            is_part_of: V::Undef,
        };
        if !node.truthy() {
            return (None, ret);
        }
        let part_props = l(&[&format!("{DCTERMS}isPartOf"), &format!("{SO}isPartOf")]);
        let mut is_part_of = self.first_results(node, &part_props, false);
        if let V::List(parts) = &mut is_part_of {
            let mut processed: Vec<V> = Vec::new();
            let mut i = 0;
            while i < parts.len() {
                let p = parts[i].clone();
                if processed.iter().any(|q| Self::same_value(q, &p)) {
                    i += 1;
                    continue;
                }
                if let V::List(sub) = self.first_results(&p, &part_props, false) {
                    parts.extend(sub);
                }
                processed.push(p);
                i += 1;
            }
            let own = self.resource_uri_string(node);
            let mut i = 0;
            while i < parts.len() {
                let p = parts[i].clone();
                if self.resource_uri_string(&p) == own {
                    parts.remove(i);
                    continue;
                }
                i += 1;
            }
        }

        let mut container = V::Undef;
        let mut container_periodical = V::Undef;
        let mut container_publication_volume = V::Undef;
        let mut t = T::default();

        // rdf:type
        let ty = self.first_results(node, &[format!("{RDF_NS}type")], true);
        if ty.truthy() {
            let ty = self.to_string(&ty);
            let pref = if ty.starts_with(BIB) {
                BIB
            } else if ty.starts_with(BIBO) {
                BIBO
            } else if ty.starts_with(SO) {
                SO
            } else if ty == format!("{Z}Attachment") {
                Z
            } else {
                ""
            };
            let lower = ty[pref.len()..].to_lowercase();
            let iso = &is_part_of;
            match lower.as_str() {
                "book" | "thesis" | "letter" | "manuscript" | "interview" | "report" | "patent"
                | "map" => {
                    if pref == BIB || pref == BIBO {
                        t.bib = Some(lower.clone());
                    }
                    if pref == Z {
                        t.z = Some(lower.clone());
                    }
                    if pref == SO {
                        t.so = Some(lower.clone());
                    }
                }
                "booksection" => {
                    t.bib = s("bookSection");
                    container = self.node_by_type(iso, &l(&[&format!("{BIB}Book")]));
                }
                "motionpicture" => t.bib = s("film"),
                "image" | "illustration" => t.bib = s("artwork"),
                "legislation" => t.bib = s("statute"),
                "recording" => t.bib = s("audioRecording"),
                "memo" => t.bib = s("note"),
                "document" => {
                    container = self.node_by_type(
                        iso,
                        &l(&[
                            &format!("{BIB}CourtReporter"),
                            &format!("{BIBO}CourtReporter"),
                        ]),
                    );
                    if container.truthy() {
                        t.bib = s("case");
                    } else if self
                        .first_results(
                            node,
                            &l(&[&format!("{BIBO}isbn10"), &format!("{BIBO}isbn13")]),
                            true,
                        )
                        .truthy()
                    {
                        t.bib = s("book");
                    } else {
                        t.bib = s("webpage");
                    }
                }
                "newsarticle"
                | "analysisnewsarticle"
                | "backgroundnewsarticle"
                | "opinionNewsarticle"
                | "reportagenewsarticle"
                | "reviewnewsarticle" => t.so = s("newspaperArticle"),
                "scholarlyarticle" | "medicalscholarlyarticle" => {
                    t.so = s("journalArticle");
                    // containerPublicationIssue is read but never used.
                    let _issue = self.node_by_type(iso, &l(&[&format!("{SO}PublicationIssue")]));
                    container_publication_volume =
                        self.node_by_type(iso, &l(&[&format!("{SO}PublicationVolume")]));
                    container_periodical =
                        self.node_by_type(iso, &l(&[&format!("{SO}Periodical")]));
                    container = self.node_by_type(
                        iso,
                        &l(&[
                            &format!("{SO}PublicationIssue"),
                            &format!("{SO}PublicationVolume"),
                            &format!("{SO}Periodical"),
                        ]),
                    );
                }
                "chapter" => {
                    t.so = s("bookSection");
                    container = self.node_by_type(iso, &l(&[&format!("{SO}Book")]));
                }
                "socialmediaposting" | "blogposting" | "liveblogposting" => t.so = s("blogPost"),
                "discussionforumposting" => t.so = s("forumPost"),
                "techarticle" | "apireference" => t.so_guess = s("report"),
                "clip" | "movieclip" | "videogameclip" => t.so_guess = s("videoRecording"),
                "tvclip" | "tvepisode" => t.so = s("tvBroadcast"),
                "tvseries" | "episode" => t.so_guess = s("tvBroadcast"),
                "radioclip" | "radioepisode" => t.so = s("radioBroadcast"),
                "radioseries" => t.so_guess = s("radioBroadcast"),
                "presentationdigitaldocument" => t.so_guess = s("presentation"),
                "message" | "emailmessage" => t.so = s("email"),
                "movie" => t.so = s("film"),
                "musicrecording" | "musicalbum" | "audiobook" | "audioobject" => {
                    t.so = s("audioRecording")
                }
                "softwareapplication"
                | "mobileapplication"
                | "videogame"
                | "webapplication"
                | "softwaresourcecode" => t.so = s("computerProgram"),
                "painting" | "photograph" | "visualartwork" | "sculpture" => t.so = s("artwork"),
                "datacatalog" | "dataset" => t.so = s(Self::dataset_type()),
                "article" => 'article: {
                    container = self.node_by_type(
                        iso,
                        &l(&[&format!("{BIB}Journal"), &format!("{BIBO}Journal")]),
                    );
                    if container.truthy() {
                        t.bib = s("journalArticle");
                        break 'article;
                    }
                    container = self.node_by_type(
                        iso,
                        &l(&[&format!("{BIB}Periodical"), &format!("{BIBO}Periodical")]),
                    );
                    if container.truthy() {
                        t.bib = s("magazineArticle");
                        break 'article;
                    }
                    container = self.node_by_type(
                        iso,
                        &l(&[&format!("{BIB}Newspaper"), &format!("{BIBO}Newspaper")]),
                    );
                    if container.truthy() {
                        t.bib = s("newspaperArticle");
                        break 'article;
                    }
                    if pref == SO {
                        container = self.node_by_type(
                            iso,
                            &l(&[
                                &format!("{SO}PublicationIssue"),
                                &format!("{SO}PublicationVolume"),
                            ]),
                        );
                        if container.truthy() {
                            t.so = s("journalArticle");
                        } else {
                            t.so_guess = s("magazineArticle");
                        }
                    }
                }
                "attachment" => {
                    t.zotero = s("attachment");
                    let path = self.first_results(
                        node,
                        &l(&[&format!("{Z}path"), &format!("{RDF_NS}resource")]),
                        false,
                    );
                    if path.truthy() {
                        let p0 = path.list()[0].clone();
                        let u = self.resource_uri(&p0);
                        self.set(item, "path", &u);
                    }
                    let charset = self.first_results(node, &[format!("{LINK}charset")], true);
                    self.set(item, "charset", &charset);
                    let mime = self.first_results(node, &[format!("{LINK}type")], true);
                    self.set(item, "mimeType", &mime);
                }
                _ => {}
            }
        }

        // z:itemType, z:type
        let ty = self.first_results(
            node,
            &l(&[&format!("{Z}itemType"), &format!("{Z}type")]),
            true,
        );
        if ty.truthy() {
            let tys = self.to_string(&ty);
            if parse_int_is_nan(&tys) && item_type_exists(&tys) {
                if tys == "encyclopediaArticle" || tys == "dictionaryEntry" {
                    container = self.node_by_type(&is_part_of, &l(&[&format!("{BIB}Book")]));
                } else if tys == "conferencePaper" {
                    container = self.node_by_type(&is_part_of, &l(&[&format!("{BIB}Journal")]));
                }
                t.zotero = Some(tys);
            }
        }

        // dc:type, dcterms:type
        let ty = self.first_results(
            node,
            &l(&[
                &format!("{DC}type"),
                &format!("{DC1_0}type"),
                &format!("{DCTERMS}type"),
            ]),
            true,
        );
        if ty.truthy() {
            let tys = self.to_string(&ty);
            if parse_int_is_nan(&tys) && item_type_exists(&tys) {
                t.dc = Some(tys);
            } else {
                let lower: String = tys
                    .to_lowercase()
                    .chars()
                    .filter(|c| !crate::zotero::framework::js::is_space(*c))
                    .collect();
                match lower.as_str() {
                    "book" | "patent" | "report" | "thesis" => t.dc = Some(lower.clone()),
                    "bookitem" => t.dc = s("bookSection"),
                    "conferenceitem" | "conferencepaper" | "conferenceposter" => {
                        t.dc = s("conferencePaper")
                    }
                    "dataset" => t.dc = s(Self::dataset_type()),
                    "article"
                    | "journalitem"
                    | "journalarticle"
                    | "submittedjournalarticle"
                    | "text.serial.journal" => t.dc = s("journalArticle"),
                    "newsitem" => t.dc = s("newspaperArticle"),
                    "scholarlytext" => t.dc = s("journalArticle"),
                    "workingpaper" => t.dc = s("report"),
                    "musicitem" => t.dc_guess = s("audioRecording"),
                    "artdesignitem" => t.dc_guess = s("artwork`"),
                    "authoredbook" => t.dc = s("book"),
                    "bookchapter" => t.dc = s("bookSection"),
                    "bachelorthesis" | "masterthesis" | "doctoralthesis" => t.dc = s("thesis"),
                    "electronicbook" => t.dc = s("book"),
                    "homepage" | "webpage" => t.dc = s("webpage"),
                    "illustration" => t.dc = s("artwork"),
                    "map" => t.dc = s("map"),
                    "event" => t.dc_guess = s("presentation"),
                    "image" => t.dc_guess = s("artwork"),
                    "movingimage" => t.dc_guess = s("videoRecording"),
                    "software" => t.dc_guess = s("computerProgram"),
                    "sound" => t.dc_guess = s("audioRecording"),
                    "stillimage" => t.dc_guess = s("artwork"),
                    "text" => t.dc_guess = s("journalArticle"),
                    _ => {}
                }
            }
        }

        // eprints:type
        let ty = self.first_results(node, &[format!("{EPRINTS}type")], true);
        if ty.truthy() {
            let tys = self.to_string(&ty);
            match tys.as_str() {
                "book" | "patent" | "report" | "thesis" => t.eprints = Some(tys.clone()),
                "bookitem" => t.eprints = s("bookSection"),
                "conferenceitem" | "conferencepaper" | "conferenceposter" => {
                    t.eprints = s("conferencePaper")
                }
                "dataset" => t.eprints = s(Self::dataset_type()),
                "journalitem" | "journalarticle" | "submittedjournalarticle" | "article" => {
                    t.eprints = s("journalArticle")
                }
                "newsitem" => t.eprints = s("newspaperArticle"),
                "scholarlytext" => t.eprints = s("journalArticle"),
                "workingpaper" => t.eprints = s("report"),
                "techreport" => t.eprints = s("report"),
                "bookedit" | "proceedings" => t.eprints = s("book"),
                "book_section" => t.eprints = s("bookSection"),
                "ad_item" => t.eprints = s("artwork"),
                "mu_item" => t.eprints = s("audioRecording"),
                "confpaper" | "conference_item" => {
                    let p = self.first_results(node, &[format!("{EPRINTS}ispublished")], true);
                    if p.as_str() == Some("unpub") {
                        t.eprints = s("presentation");
                    } else {
                        t.eprints = s("conferencePaper");
                    }
                }
                _ => {}
            }
        }

        // og:type
        let ty = self.first_results(node, &[format!("{OG}type")], true);
        match ty.as_str() {
            Some("video.movie" | "video.episode" | "video.tv_show" | "video.other") => {
                t.og = s("videoRecording")
            }
            Some("article") => t.og_guess = s("journalArticle"),
            Some("book") => t.og = s("book"),
            Some("music.song" | "music.album") => t.og = s("audioRecording"),
            Some("website") => t.og = s("webpage"),
            _ => {}
        }

        // t.zotero || t.bib || t.prism || t.eprints || t.og || t.dc || t.so
        //   || exports.defaultUnknownType || t.zoteroGuess || t.bibGuess
        //   || t.prismGuess || t.ogGuess || t.dcGuess || t.soGuess
        // (t.z is set but never read; t.prism* are never set, see the module
        // docs; t.zoteroGuess and t.bibGuess are never set.)
        let _ = &t.z;
        let item_type = [
            &t.zotero,
            &t.bib,
            &t.eprints,
            &t.og,
            &t.dc,
            &t.so,
            &t.og_guess,
            &t.dc_guess,
            &t.so_guess,
        ]
        .into_iter()
        .flatten()
        .find(|x| !x.is_empty())
        .cloned();

        if !container.truthy() {
            let types: Option<Vec<String>> = match item_type.as_deref() {
                Some("blogPost") => Some(l(&[&format!("{Z}Blog")])),
                Some("forumPost") => Some(l(&[&format!("{Z}Forum")])),
                Some("webpage") => Some(l(&[&format!("{Z}Website")])),
                Some("bookSection") => Some(l(&[&format!("{BIB}Book")])),
                Some("case") => Some(l(&[
                    &format!("{BIB}CourtReporter"),
                    &format!("{BIBO}CourtReporter"),
                ])),
                Some("journalArticle") => {
                    Some(l(&[&format!("{BIB}Journal"), &format!("{BIBO}Journal")]))
                }
                Some("magazineArticle") => Some(l(&[
                    &format!("{BIB}Periodical"),
                    &format!("{BIBO}Periodical"),
                    &format!("{SO}Periodical"),
                ])),
                Some("newspaperArticle") => Some(l(&[
                    &format!("{BIB}Newspaper"),
                    &format!("{BIBO}Newspaper"),
                ])),
                _ => None,
            };
            if let Some(types) = types {
                container = self.node_by_type(&is_part_of, &types);
            }
        }

        ret.container = container;
        ret.container_periodical = container_periodical;
        ret.container_publication_volume = container_publication_volume;
        ret.is_part_of = is_part_of;
        (item_type, ret)
    }
}
