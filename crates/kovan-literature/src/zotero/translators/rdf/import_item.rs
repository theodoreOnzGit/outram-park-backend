// Part of the kovan Zotero port (GitHub #747, #749).
//
// Upstream: Zotero translators, https://github.com/zotero/translators
//   (commit 3d1c78530f42): "RDF.js" `importItem` :863-1447.
// Copyright (c) 2011 Center for History and New Media, George Mason
//   University, Fairfax, Virginia, USA (Simon Kornblith).
// Licence: AGPL-3.0 (upstream: AGPL-3.0-or-later).

//! `importItem(newItem, node)`: every field, creator, note, tag and
//! attachment of one item. Properties are assigned in upstream's order,
//! `undefined` included, because the order of an item's properties decides
//! which of two fields mapping to the same base field wins in
//! `itemToAPIJSON`.

use super::detect_type::parse_int_is_nan;
use super::ns::*;
use super::{err, Rdf, V};
use crate::zotero::framework::rdf::RDF_NS;
use crate::zotero::framework::utilities::{clean_doi, field_is_valid_for_type, item_type_exists};
use crate::zotero::framework::{
    JsObject, TranslateError, TranslatorItem, TranslatorNote, TranslatorTag,
};
use crate::zotero::translators::rdf_creator_types::server_creators_for_type;
use crate::zotero::translators::rdf_support::{clean_isbn, clean_issn};
use serde_json::Value;

/// Property URIs from `(namespace, local name)` pairs.
fn p(list: &[(&str, &str)]) -> Vec<String> {
    list.iter().map(|(n, l)| format!("{n}{l}")).collect()
}

impl Rdf<'_> {
    fn get(&self, item: &TranslatorItem, key: &str) -> V {
        match item.get(key) {
            None | Some(Value::Null) => V::Undef,
            Some(Value::String(s)) => V::Str(s.clone()),
            Some(Value::Bool(false)) => V::False,
            Some(Value::Array(a)) => V::List(
                a.iter()
                    .map(|x| match x {
                        Value::String(s) => V::Str(s.clone()),
                        _ => V::Undef,
                    })
                    .collect(),
            ),
            Some(other) => V::Str(crate::zotero::framework::js::to_js_string(other)),
        }
    }

    fn truthy(item: &TranslatorItem, key: &str) -> bool {
        item.truthy(key)
    }

    /// `importItem(newItem, node)` (:863-1447). Returns false where upstream
    /// does (no type and no title).
    pub(crate) fn import_item(
        &mut self,
        item: &mut TranslatorItem,
        node: &V,
    ) -> Result<bool, TranslateError> {
        let (item_type, ret) = self.detect_type(item, node);
        let zotero_type = self.first_results(node, &p(&[(Z, "itemType"), (Z, "type")]), true);
        let is_zotero_rdf = zotero_type.truthy();
        let unknown_item_type: Option<String> = if zotero_type.truthy() {
            let s = self.to_string(&zotero_type);
            (parse_int_is_nan(&s) && !item_type_exists(&s)).then_some(s)
        } else {
            None
        };
        // `newItem.itemType = exports.itemType || itemType`
        item.item_type = item_type.unwrap_or_default();
        let container = ret.container.clone();
        let container_periodical = ret.container_periodical.clone();
        let container_publication_volume = ret.container_publication_volume.clone();
        let is_part_of = ret.is_part_of.clone();

        // title
        let title = self.first_results(
            node,
            &p(&[
                (DC, "title"),
                (DC1_0, "title"),
                (DCTERMS, "title"),
                (EPRINTS, "title"),
                (VCARD2, "fn"),
                (OG, "title"),
                (SO, "headline"),
            ]),
            true,
        );
        self.set(item, "title", &title);
        if item.item_type.is_empty() {
            if unknown_item_type.is_some() {
                item.item_type = "document".to_owned();
            } else if !Self::truthy(item, "title") {
                return Ok(false);
            } else {
                item.item_type = "journalArticle".to_owned();
            }
        } else if !Self::truthy(item, "title") {
            let t = self.first_results(node, &[format!("{SO}name")], true);
            self.set(item, "title", &t);
        }

        // creators
        let creator_types: Vec<&str> = server_creators_for_type(&item.item_type).to_vec();
        for ct in creator_types {
            let props: Vec<String> = match ct {
                "author" => p(&[
                    (BIB, "authors"),
                    (SO, "author"),
                    (SO, "creator"),
                    (DC, "creator"),
                    (DC, "creator.PersonalName"),
                    (DC1_0, "creator"),
                    (DCTERMS, "creator"),
                    (EPRINTS, "creators_name"),
                    (DC, "contributor"),
                    (DC1_0, "contributor"),
                    (DCTERMS, "contributor"),
                ]),
                "editor" | "contributor" => vec![
                    format!("{BIB}{ct}s"),
                    format!("{EPRINTS}{ct}s_name"),
                    format!("{SO}{ct}"),
                ],
                "presenter" => vec![format!("{Z}{ct}s"), format!("{EPRINTS}creators_name")],
                "castMember" => p(&[(VIDEO, "actor")]),
                "scriptwriter" => p(&[(VIDEO, "writer")]),
                "producer" => p(&[(SO, "producer")]),
                "programmer" => p(&[(Z, "programmers"), (SO, "author"), (CODEMETA, "maintainer")]),
                _ => vec![format!("{Z}{ct}s")],
            };
            let creators = self.first_results(node, &props, false);
            if creators.truthy() {
                self.handle_creators(item, &creators, ct)?;
            }
        }

        // publicationTitle
        let pt = self.first_results(
            node,
            &p(&[
                (PRISM, "publicationName"),
                (PRISM2_0, "publicationName"),
                (PRISM2_1, "publicationName"),
                (EPRINTS, "publication"),
                (EPRINTS, "book_title"),
                (DC, "source"),
                (DC1_0, "source"),
                (DCTERMS, "source"),
            ]),
            true,
        );
        self.set(item, "publicationTitle", &pt);
        if container.truthy() {
            let pt = self.first_results(
                &container,
                &p(&[
                    (DC, "title"),
                    (DC1_0, "title"),
                    (DCTERMS, "title"),
                    (SO, "name"),
                ]),
                true,
            );
            self.set(item, "publicationTitle", &pt);
            let v = self.get(item, "publicationTitle");
            self.set(item, "reporter", &v);
        }
        if container_periodical.truthy() {
            let nodes = V::List(vec![
                container_periodical.clone(),
                container_publication_volume.clone(),
            ]);
            let pt = self.first_results(&nodes, &[format!("{SO}name")], true);
            self.set(item, "publicationTitle", &pt);
        }

        let site_name = self.first_results(node, &[format!("{OG}site_name")], true);
        if site_name.truthy()
            && !Self::truthy(item, "publicationTitle")
            && matches!(
                item.item_type.as_str(),
                "blogPost" | "webpage" | "newspaperArticle"
            )
        {
            self.set(item, "publicationTitle", &site_name);
        }

        // rights
        let rights = self.first_results(
            node,
            &p(&[
                (PRISM, "copyright"),
                (PRISM2_0, "copyright"),
                (PRISM2_1, "copyright"),
                (DC, "rights"),
                (DC1_0, "rights"),
                (DCTERMS, "rights"),
                (SO, "license"),
            ]),
            true,
        );
        self.set(item, "rights", &rights);

        // section
        let section = self.node_by_type(&is_part_of, &[format!("{BIB}Part")]);
        if section.truthy() {
            let v = self.first_results(
                &section,
                &p(&[(DC, "title"), (DC1_0, "title"), (DCTERMS, "title")]),
                true,
            );
            self.set(item, "section", &v);
        }
        if !section.truthy() {
            let v = self.first_results(node, &p(&[(ARTICLE, "section"), (SO, "genre")]), true);
            self.set(item, "section", &v);
        }

        // series
        let series = self.node_by_type(&is_part_of, &[format!("{BIB}Series")]);
        if series.truthy() {
            let v = self.first_results(
                &series,
                &p(&[(DC, "title"), (DC1_0, "title"), (DCTERMS, "title")]),
                true,
            );
            self.set(item, "series", &v);
            let v = self.first_results(&series, &[format!("{DCTERMS}alternative")], true);
            self.set(item, "seriesTitle", &v);
            let v = self.first_results(
                &series,
                &p(&[
                    (DC, "description"),
                    (DC1_0, "description"),
                    (DCTERMS, "description"),
                ]),
                true,
            );
            self.set(item, "seriesText", &v);
            let v = self.first_results(
                &series,
                &p(&[
                    (DC, "identifier"),
                    (DC1_0, "identifier"),
                    (DCTERMS, "description"),
                ]),
                true,
            );
            self.set(item, "seriesNumber", &v);
        }

        // volume
        let nodes = V::List(vec![
            container.clone(),
            node.clone(),
            container_publication_volume.clone(),
            container_periodical.clone(),
        ]);
        let v = self.first_results(
            &nodes,
            &p(&[
                (PRISM, "volume"),
                (PRISM2_0, "volume"),
                (PRISM2_1, "volume"),
                (EPRINTS, "volume"),
                (BIBO, "volume"),
                (DC, "source.Volume"),
                (DCTERMS, "citation.volume"),
                (SO, "volumeNumber"),
            ]),
            true,
        );
        self.set(item, "volume", &v);

        // issue
        let mut issue_nodes = vec![node.clone()];
        if container.truthy() {
            issue_nodes.insert(0, container.clone());
        }
        let v = self.first_results(
            &V::List(issue_nodes),
            &p(&[
                (PRISM, "number"),
                (PRISM2_0, "number"),
                (PRISM2_1, "number"),
                (EPRINTS, "number"),
                (BIBO, "issue"),
                (DC, "source.Issue"),
                (DCTERMS, "citation.issue"),
                (SO, "issueNumber"),
            ]),
            true,
        );
        self.set(item, "issue", &v);
        if !field_is_valid_for_type("issue", &item.item_type) {
            let v = self.get(item, "issue");
            self.set(item, "number", &v);
            item.remove("issue");
        }

        // edition
        let v = self.first_results(
            node,
            &p(&[
                (PRISM, "edition"),
                (PRISM2_0, "edition"),
                (PRISM2_1, "edition"),
                (BIBO, "edition"),
                (SO, "bookEdition"),
                (SO, "version"),
            ]),
            true,
        );
        self.set(item, "edition", &v);
        let v = self.get(item, "edition");
        self.set(item, "versionNumber", &v);

        // pages
        let v = self.first_results(
            node,
            &p(&[
                (BIB, "pages"),
                (EPRINTS, "pagerange"),
                (PRISM2_0, "pageRange"),
                (PRISM2_1, "pageRange"),
                (BIBO, "pages"),
                (DC, "identifier.pageNumber"),
                (SO, "pagination"),
            ]),
            true,
        );
        self.set(item, "pages", &v);
        if !Self::truthy(item, "pages") {
            let spage = self.first_results(
                node,
                &p(&[
                    (PRISM, "startingPage"),
                    (PRISM2_0, "startingPage"),
                    (PRISM2_1, "startingPage"),
                    (BIBO, "pageStart"),
                    (DCTERMS, "relation.spage"),
                    (SO, "pageStart"),
                ]),
                true,
            );
            let epage = self.first_results(
                node,
                &p(&[
                    (PRISM, "endingPage"),
                    (PRISM2_0, "endingPage"),
                    (PRISM2_1, "endingPage"),
                    (BIBO, "pageEnd"),
                    (DCTERMS, "relation.epage"),
                    (SO, "pageEnd"),
                ]),
                true,
            );
            let mut pages = Vec::new();
            if spage.truthy() {
                pages.push(self.to_string(&spage));
            }
            if epage.truthy() {
                pages.push(self.to_string(&epage));
            }
            if !pages.is_empty() {
                item.set("pages", pages.join("-"));
            }
        }

        let v = self.first_results(
            node,
            &p(&[
                (BIBO, "numPages"),
                (EPRINTS, "pages"),
                (SO, "numberOfPages"),
            ]),
            true,
        );
        self.set(item, "numPages", &v);
        let v = self.first_results(node, &[format!("{BIBO}numVolumes")], true);
        self.set(item, "numberOfVolumes", &v);
        let v = self.first_results(node, &[format!("{BIBO}shortTitle")], true);
        self.set(item, "shortTitle", &v);
        // `newItem.artworkMedium = newItem.interviewMedium = ...`: the
        // right-hand assignment runs first.
        let v = self.first_results(node, &[format!("{DCTERMS}medium")], true);
        self.set(item, "interviewMedium", &v);
        self.set(item, "artworkMedium", &v);
        let v = self.first_results(node, &[format!("{SO}programmingLanguage")], true);
        self.set(item, "programmingLanguage", &v);
        let v = self.first_results(node, &[format!("{SO}operatingSystem")], true);
        self.set(item, "system", &v);

        // publisher
        let nodes = V::List(vec![
            node.clone(),
            container_periodical.clone(),
            container_publication_volume.clone(),
        ]);
        let publisher = self.first_results(
            &nodes,
            &p(&[
                (DC, "publisher"),
                (DC1_0, "publisher"),
                (DCTERMS, "publisher"),
                (VCARD2, "org"),
                (EPRINTS, "institution"),
                (SO, "publisher"),
                (SO, "publishedBy"),
            ]),
            false,
        );
        if publisher.truthy() {
            let p0 = publisher.list()[0].clone();
            if let V::Str(_) = p0 {
                self.set(item, "publisher", &p0);
            } else if let Some(ty) = self.first_type(&p0) {
                let ty = self.to_string(&ty);
                if ty == format!("{FOAF}Organization") || ty == format!("{FOAF}Agent") {
                    let v = self.first_results(&p0, &[format!("{FOAF}name")], true);
                    self.set(item, "publisher", &v);
                    let place = self.first_results(&p0, &[format!("{VCARD}adr")], false);
                    if place.truthy() {
                        let pl0 = place.list()[0].clone();
                        let v = self.first_results(&pl0, &[format!("{VCARD}locality")], false);
                        self.set(item, "place", &v);
                    }
                } else if ty == format!("{VCARD2}Organization") {
                    let v = self.first_results(&p0, &[format!("{VCARD2}organization-name")], true);
                    self.set(item, "publisher", &v);
                } else {
                    let v = self.first_results(&p0, &[format!("{SO}name")], true);
                    self.set(item, "publisher", &v);
                    let v = self.first_results(&p0, &[format!("{SO}location")], true);
                    self.set(item, "place", &v);
                }
            }
        }

        // place
        if !Self::truthy(item, "place") {
            let v = self.first_results(
                node,
                &p(&[(EPRINTS, "place_of_pub"), (EPRINTS, "event_location")]),
                true,
            );
            self.set(item, "place", &v);
        }

        // `newItem.distributor = newItem.label = newItem.company =
        // newItem.institution = newItem.publisher` (right to left).
        let v = self.get(item, "publisher");
        for k in ["institution", "company", "label", "distributor"] {
            self.set(item, k, &v);
        }

        // date
        let v = self.first_results(
            node,
            &p(&[
                (EPRINTS, "date"),
                (PRISM, "publicationDate"),
                (PRISM2_0, "publicationDate"),
                (PRISM2_1, "publicationDate"),
                (OG, "published_time"),
                (ARTICLE, "published_time"),
                (BOOK, "release_date"),
                (MUSIC, "release_date"),
                (VIDEO, "release_date"),
                (DC, "date.issued"),
                (DCTERMS, "date.issued"),
                (DCTERMS, "issued"),
                (DC, "date"),
                (DC1_0, "date"),
                (DCTERMS, "date"),
                (DCTERMS, "dateSubmitted"),
                (EPRINTS, "datestamp"),
                (SO, "datePublished"),
            ]),
            true,
        );
        self.set(item, "date", &v);
        let v = self.first_results(node, &[format!("{DCTERMS}dateSubmitted")], true);
        self.set(item, "accessDate", &v);
        let v = self.first_results(
            node,
            &p(&[(DCTERMS, "modified"), (SO, "dateModified")]),
            true,
        );
        self.set(item, "lastModified", &v);

        // identifiers
        let id_props = p(&[
            (DC, "identifier"),
            (DC1_0, "identifier"),
            (DCTERMS, "identifier"),
        ]);
        let mut identifiers = self.first_results(node, &id_props, false);
        if container.truthy() {
            let ci = self.first_results(&container, &id_props, false);
            if ci.truthy() {
                identifiers = if identifiers.truthy() {
                    let mut l = identifiers.list().to_vec();
                    l.extend(ci.list().iter().cloned());
                    V::List(l)
                } else {
                    ci
                };
            }
        }
        for ident in identifiers.list().to_vec() {
            match &ident {
                V::Str(s) => {
                    let before = match s.find(' ') {
                        Some(i) => s[..i].to_uppercase(),
                        None => String::new(),
                    };
                    let rest = |n: usize| -> String { s.chars().skip(n).collect() };
                    if before == "ISBN" {
                        item.set("ISBN", rest(5).to_uppercase());
                    } else if before == "ISSN" {
                        item.set("ISSN", rest(5).to_uppercase());
                    } else if before == "DOI" {
                        item.set("DOI", rest(4));
                    } else if let Some(isbn) = clean_isbn(s) {
                        item.set("ISBN", isbn);
                    } else if let Some(issn) = clean_issn(s) {
                        item.set("ISSN", issn);
                    } else if let Some(doi) = clean_doi(s) {
                        item.set("DOI", doi);
                    }
                }
                _ => {
                    if let Some(ty) = self.first_type(&ident) {
                        if self.to_string(&ty) == format!("{DCTERMS}URI") {
                            let v = self.first_results(&ident, &[format!("{RDF_NS}value")], true);
                            self.set(item, "url", &v);
                        }
                    }
                }
            }
        }

        // ISSN (PRISM etc.)
        let nodes = V::List(vec![
            container.clone(),
            node.clone(),
            container_periodical.clone(),
            container_publication_volume.clone(),
        ]);
        let v = self.first_results(
            &nodes,
            &p(&[
                (PRISM, "issn"),
                (PRISM2_0, "issn"),
                (PRISM2_1, "issn"),
                (EPRINTS, "issn"),
                (BIBO, "issn"),
                (DC, "source.ISSN"),
                (PRISM, "eIssn"),
                (PRISM2_0, "eIssn"),
                (PRISM2_1, "eIssn"),
                (BIBO, "eissn"),
                (SO, "issn"),
            ]),
            true,
        );
        let v = if v.truthy() {
            v
        } else {
            self.get(item, "ISSN")
        };
        self.set(item, "ISSN", &v);
        let isbn_node = if container.truthy() {
            container.clone()
        } else {
            node.clone()
        };
        let v = self.first_results(
            &isbn_node,
            &p(&[
                (PRISM2_1, "isbn"),
                (BIBO, "isbn"),
                (BIBO, "isbn13"),
                (BIBO, "isbn10"),
                (BOOK, "isbn"),
                (SO, "isbn"),
            ]),
            true,
        );
        let v = if v.truthy() {
            v
        } else {
            self.get(item, "ISBN")
        };
        self.set(item, "ISBN", &v);
        let v = self.first_results(node, &[format!("{EPRINTS}isbn")], true);
        let v = if v.truthy() {
            v
        } else {
            self.get(item, "ISBN")
        };
        self.set(item, "ISBN", &v);
        let v = self.first_results(
            node,
            &p(&[
                (DC, "identifier.DOI"),
                (PRISM2_0, "doi"),
                (PRISM2_1, "doi"),
                (BIBO, "doi"),
            ]),
            true,
        );
        let v = if v.truthy() { v } else { self.get(item, "DOI") };
        self.set(item, "DOI", &v);

        if !Self::truthy(item, "url") {
            let url = self.first_results(
                node,
                &p(&[
                    (EPRINTS, "official_url"),
                    (VCARD2, "url"),
                    (OG, "url"),
                    (PRISM2_0, "url"),
                    (PRISM2_1, "url"),
                    (BIBO, "uri"),
                    (SO, "url"),
                    (SO, "sameAs"),
                ]),
                false,
            );
            if url.truthy() {
                let u0 = url.list()[0].clone();
                let u = self.resource_uri(&u0);
                self.set(item, "url", &u);
            }
        }

        let v = self.first_results(
            node,
            &p(&[(DC, "coverage"), (DC1_0, "coverage"), (DCTERMS, "coverage")]),
            true,
        );
        self.set(item, "archiveLocation", &v);

        let v = self.first_results(
            node,
            &p(&[
                (EPRINTS, "abstract"),
                (PRISM, "teaser"),
                (PRISM2_0, "teaser"),
                (PRISM2_1, "teaser"),
                (OG, "description"),
                (BIBO, "abstract"),
                (DCTERMS, "abstract"),
                (DC, "description.abstract"),
                (DCTERMS, "description.abstract"),
                (DC1_0, "description"),
                (SO, "description"),
            ]),
            true,
        );
        self.set(item, "abstractNote", &v);

        // type (the "dataset" Extra fallback applies only where the schema
        // has no dataset type; it has one)
        let ty = self.first_results(
            node,
            &p(&[(DC, "type"), (DC1_0, "type"), (DCTERMS, "type")]),
            true,
        );
        for k in [
            "reportType",
            "letterType",
            "manuscriptType",
            "mapType",
            "thesisType",
            "websiteType",
            "presentationType",
            "postType",
            "audioFileType",
        ] {
            self.set(item, k, &ty);
        }
        if item.item_type == "thesis" {
            let v = self.first_results(node, &[format!("{EPRINTS}thesis_type")], true);
            let v = if v.truthy() {
                v
            } else {
                self.get(item, "thesisType")
            };
            self.set(item, "thesisType", &v);
        }
        if item.item_type == "presentation" {
            let v = self.first_results(node, &[format!("{EPRINTS}event_type")], true);
            let v = if v.truthy() {
                v
            } else {
                self.get(item, "presentationType")
            };
            self.set(item, "presentationType", &v);
        }

        // conferenceName
        let conference = self.first_results(node, &[format!("{BIB}presentedAt")], false);
        if conference.truthy() {
            let c0 = conference.list()[0].clone();
            if let V::Str(_) = c0 {
                self.set(item, "conferenceName", &c0);
            } else {
                let v = self.first_results(
                    &c0,
                    &p(&[(DC, "title"), (DC1_0, "title"), (DCTERMS, "title")]),
                    true,
                );
                self.set(item, "conferenceName", &v);
            }
        }
        if !Self::truthy(item, "conferenceName") {
            let v = self.first_results(node, &[format!("{EPRINTS}event_title")], false);
            self.set(item, "conferenceName", &v);
        }
        let v = self.get(item, "conferenceName");
        self.set(item, "meetingName", &v);

        let jnode = if container.truthy() {
            container.clone()
        } else {
            node.clone()
        };
        let v = self.first_results(&jnode, &[format!("{DCTERMS}alternative")], true);
        self.set(item, "journalAbbreviation", &v);

        let v = self.first_results(
            node,
            &[
                format!("{VIDEO}duration"),
                format!("{SONG_UNDEFINED}duration"),
                format!("{SO}duration"),
            ],
            true,
        );
        self.set(item, "runningTime", &v);

        let adr = self.first_results(node, &[format!("{VCARD2}adr")], false);
        if adr.truthy() {
            let a0 = adr.list()[0].clone();
            let v = self.first_results(&a0, &[format!("{VCARD2}label")], true);
            self.set(item, "address", &v);
        }
        let v = self.first_results(node, &[format!("{VCARD2}tel")], true);
        self.set(item, "telephone", &v);
        let v = self.first_results(node, &[format!("{VCARD2}email")], true);
        self.set(item, "email", &v);
        let v = self.first_results(node, &[format!("{DCTERMS}dateAccepted")], true);
        self.set(item, "accepted", &v);
        let v = self.first_results(
            node,
            &p(&[
                (DC, "language"),
                (DC1_0, "language"),
                (DCTERMS, "language"),
                (SO, "inLanguage"),
            ]),
            true,
        );
        self.set(item, "language", &v);

        // see also
        self.process_see_also(node, item);

        // description / attachment note / Extra
        if item.item_type == "attachment" {
            let v = self.first_results(
                node,
                &p(&[
                    (DC, "description"),
                    (DC1_0, "description"),
                    (DCTERMS, "description"),
                ]),
                true,
            );
            self.set(item, "note", &v);
        } else if is_zotero_rdf {
            let v = self.first_results(node, &[format!("{DC}description")], true);
            self.set(item, "extra", &v);
            if let Some(u) = &unknown_item_type {
                let extra = self.get(item, "extra");
                let e = if extra.truthy() {
                    format!("{}\nType: {u}", self.to_string(&extra))
                } else {
                    format!("Type: {u}")
                };
                item.set("extra", e);
            }
        } else if !Self::truthy(item, "abstractNote") {
            let v = self.first_results(
                node,
                &p(&[(DC, "description"), (DCTERMS, "description")]),
                true,
            );
            self.set(item, "abstractNote", &v);
        }

        // notes
        let referenced_by = self.targets(node, &format!("{DCTERMS}isReferencedBy"));
        for referent in referenced_by.list().to_vec() {
            let Some(ty) = self.first_type(&referent) else {
                continue;
            };
            if self.to_string(&ty) != format!("{BIB}Memo") {
                continue;
            }
            let text = self.first_results(
                &referent,
                &p(&[
                    (RDF_NS, "value"),
                    (DC, "description"),
                    (DC1_0, "description"),
                    (DCTERMS, "description"),
                ]),
                true,
            );
            if matches!(text, V::Undef) {
                continue;
            }
            let mut props = JsObject::new();
            self.note_see_also_and_tags(&referent, &mut props);
            item.notes.push(TranslatorNote {
                note: self.to_string(&text),
                props,
            });
        }

        if item.item_type == "note" {
            let v = self.first_results(
                node,
                &p(&[
                    (RDF_NS, "value"),
                    (DC, "description"),
                    (DC1_0, "description"),
                    (DCTERMS, "description"),
                ]),
                true,
            );
            let v = if v.truthy() {
                v
            } else {
                V::Str(" ".to_owned())
            };
            self.set(item, "note", &v);
        }

        // tags
        let subjects = self.first_results(
            node,
            &p(&[
                (DC, "subject"),
                (DC1_0, "subject"),
                (DCTERMS, "subject"),
                (ARTICLE, "tag"),
                (PRISM2_0, "keyword"),
                (PRISM2_1, "keyword"),
                (PRISM2_0, "object"),
                (PRISM2_1, "object"),
                (PRISM2_0, "organization"),
                (PRISM2_1, "organization"),
                (PRISM2_0, "person"),
                (PRISM2_1, "person"),
                (SO, "keywords"),
                (SO, "about"),
            ]),
            false,
        );
        let call_number_types = p(&[(DCTERMS, "LCC"), (DCTERMS, "DDC"), (DCTERMS, "UDC")]);
        for subject in subjects.list().to_vec() {
            match &subject {
                V::Str(s) => item.tags.push(TranslatorTag::new(s.clone())),
                _ => {
                    if let Some(ty) = self.first_type(&subject) {
                        let ty = self.to_string(&ty);
                        if call_number_types.contains(&ty) {
                            let v = self.first_results(&subject, &[format!("{RDF_NS}value")], true);
                            self.set(item, "callNumber", &v);
                        } else if ty == format!("{Z}AutomaticTag") {
                            let v = self.first_results(&subject, &[format!("{RDF_NS}value")], true);
                            let tag = match &v {
                                V::Undef => String::new(),
                                other => self.to_string(other),
                            };
                            item.tags.push(TranslatorTag {
                                tag,
                                tag_type: Some(1),
                            });
                        }
                    }
                }
            }
        }

        // attachments
        let relations = self.first_results(node, &[format!("{LINK}link")], false);
        for relation in relations.list().to_vec() {
            let ty = self.targets(&relation, &format!("{RDF_NS}type"));
            let Some(t0) = ty.list().first().cloned() else {
                // `getResourceURI(type[0])` of `false[0]`.
                return Err(err(
                    "TypeError: Cannot read properties of undefined (reading 'uri')",
                ));
            };
            if self.resource_uri_string(&t0) == format!("{Z}Attachment") {
                let mut attachment = TranslatorItem::new("");
                self.import_item(&mut attachment, &relation)?;
                item.attachments.push(self.attachment_json(&attachment));
            }
        }

        let pdf = self.first_results(node, &[format!("{EPRINTS}document_url")], false);
        if pdf.truthy() {
            let mut o = JsObject::new();
            o.set("title", "Full Text PDF");
            o.set("mimeType", "application/pdf");
            let p0 = pdf.list()[0].clone();
            o.set("path", self.json_of(&p0));
            item.attachments.push(o);
        }

        // other fields in the Zotero namespace
        let r = self.res(node);
        if let Some(arcs) = self.z.get_arcs_out(r) {
            for uri in arcs {
                let Some(property) = uri.strip_prefix(Z) else {
                    continue;
                };
                if matches!(property, "path" | "itemType" | "creators") {
                    continue;
                }
                let t = self.targets(node, &format!("{Z}{property}"));
                let v = t.list().first().cloned().unwrap_or(V::Undef);
                self.set_any(item, property, &v);
            }
        }
        Ok(true)
    }

    /// `newItem[property] = value` for any property name, including the
    /// class members (`notes`, `tags`, `seeAlso`, `attachments`), which a
    /// `z:` arc can overwrite; `_itemDone` then turns a non-object there into
    /// a one-element array.
    fn set_any(&self, item: &mut TranslatorItem, property: &str, v: &V) {
        match property {
            "notes" => {
                item.notes = match v {
                    V::Undef => Vec::new(),
                    other => vec![TranslatorNote::new(self.to_string(other))],
                }
            }
            "tags" => {
                item.tags = match v {
                    V::Undef => Vec::new(),
                    other => vec![TranslatorTag::new(self.to_string(other))],
                }
            }
            "seeAlso" => item.see_also = vec![self.json_of(v)],
            "attachments" => item.attachments = Vec::new(),
            _ => self.set(item, property, v),
        }
    }

    /// An imported attachment as the plain object upstream holds (a
    /// `Zotero.Item`: its class members, then its properties; `undefined`
    /// ones are not kept, as `JSON.stringify` drops them).
    pub(crate) fn attachment_json(&self, a: &TranslatorItem) -> JsObject {
        let mut o = JsObject::new();
        if !a.item_type.is_empty() {
            o.set("itemType", a.item_type.clone());
        }
        o.set(
            "creators",
            Value::Array(a.creators.iter().map(|c| c.to_value()).collect()),
        );
        o.set(
            "notes",
            Value::Array(
                a.notes
                    .iter()
                    .map(|n| {
                        let mut m = serde_json::Map::new();
                        m.insert("note".into(), n.note.clone().into());
                        for (k, v) in n.props.iter() {
                            m.insert(k.to_owned(), v.clone());
                        }
                        Value::Object(m)
                    })
                    .collect(),
            ),
        );
        o.set(
            "tags",
            Value::Array(
                a.tags
                    .iter()
                    .map(|t| match t.tag_type {
                        Some(ty) => serde_json::json!({"tag": t.tag, "type": ty}),
                        None => Value::String(t.tag.clone()),
                    })
                    .collect(),
            ),
        );
        o.set("seeAlso", Value::Array(a.see_also.clone()));
        o.set(
            "attachments",
            Value::Array(a.attachments.iter().map(JsObject::to_value).collect()),
        );
        for (k, v) in a.props.iter() {
            if !v.is_null() {
                o.set(k, v.clone());
            }
        }
        o
    }
}
