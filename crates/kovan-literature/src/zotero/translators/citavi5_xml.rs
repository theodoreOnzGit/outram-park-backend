// Part of the kovan Zotero port (GitHub #747, #749).
//
// Upstream: Zotero translators, https://github.com/zotero/translators
//   (commit 3d1c78530f42): "Citavi 5 XML.js" (translatorID
//   e7243cef-a709-4a46-ba46-1b1318051bec, lastUpdated 2025-01-04 01:03:00):
//   `detectImport` :51-54, `typeMapping` :58-93, `importItems` :95-302,
//   `importUnfinished` :306-340, `importTasks` :343-364,
//   `addHierarchyNumberRecursive` :366-375, `importCategories` :377-433,
//   `doImport` :435-472, `attachName` :474-488, `attachPersons` :493-515,
//   `addExtraLine` :517-524, `extractPages` :526-532.
// Copyright (c) 2016 Philipp Zumstein.
// Licence: AGPL-3.0 (upstream: AGPL-3.0-or-later).

//! The Citavi 5 XML translator (import): a Citavi 5 or 6 project exported
//! as XML (`CitaviExchangeData`): references, persons, periodicals, series,
//! publishers, keywords, groups (tags), knowledge items (notes), locations
//! (attachments, call numbers), tasks (standalone notes) and categories
//! (collections).
//!
//! **Shared objects.** Upstream saves items when the translation ends, and
//! a saved item shares its `creators` array and creator objects with the
//! translator's object. `importUnfinished` turns a container's `author`s
//! into `bookAuthor`s while copying them to a contribution, after the
//! container was completed; the saved container changes too. The port keeps
//! its own copy of every item (`itemIdList`) and applies the same change to
//! the completed item ([`ImportContext::items_mut`]).
//!
//! **Maturity: AI draft (1).**

use crate::zotero::framework::options::{translator_type, HeaderValue, TranslatorMetadata};
use crate::zotero::framework::xml::{get_xml, NodeId, XNode, XmlDocument};
use crate::zotero::framework::xpath::{xpath, xpath_text};
use crate::zotero::framework::{
    CollectionChild, ImportContext, JsObject, TranslateError, TranslatorCollection,
    TranslatorCreator, TranslatorItem, TranslatorNote, TranslatorTag,
};
use regex::Regex;
use serde_json::Value;
use std::sync::OnceLock;

/// The translator header.
pub static METADATA: TranslatorMetadata = TranslatorMetadata {
    id: "e7243cef-a709-4a46-ba46-1b1318051bec",
    label: "Citavi 5 XML",
    creator: "Philipp Zumstein, Tomasz Najdek",
    target: "xml",
    min_version: "3.0",
    priority: 100,
    translator_type: translator_type::IMPORT,
    config_options: &[
        ("dataMode", HeaderValue::Str("xml/dom")),
        ("async", HeaderValue::Bool(true)),
    ],
    display_options: &[],
    hidden_prefs: &[],
    last_updated: "2025-01-04 01:03:00",
};

/// `detectImport` (:51-54).
pub fn detect_import(ctx: &mut ImportContext) -> bool {
    ctx.read_chars(1000)
        .is_some_and(|t| t.contains("<CitaviExchangeData"))
}

/// `typeMapping` (:58-93).
fn type_mapping(t: &str) -> Option<&'static str> {
    Some(match t {
        "ArchiveMaterial" => "manuscript",
        "AudioBook" => "book",
        "AudioOrVideoDocument" => "document",
        "Book" => "book",
        "BookEdited" => "book",
        "Broadcast" => "tvBroadcast",
        "CollectedWorks" => "book",
        "ComputerProgram" => "computerProgram",
        "ConferenceProceedings" => "book",
        "Contribution" => "bookSection",
        "ContributionInLegalCommentary" => "bookSection",
        "CourtDecision" => "case",
        "File" => "manuscript",
        "InternetDocument" => "webpage",
        "InterviewMaterial" => "interview",
        "JournalArticle" => "journalArticle",
        "Lecture" => "presentation",
        "LegalCommentary" => "book",
        "Manuscript" => "manuscript",
        "Map" => "map",
        "Movie" => "videoRecording",
        "MusicTrack" => "audioRecording",
        "MusicAlbum" => "audioRecording",
        "NewsAgencyReport" => "report",
        "NewspaperArticle" => "newspaperArticle",
        "Patent" => "patent",
        "PersonalCommunication" => "email",
        "PressRelease" => "report",
        "RadioPlay" => "podcast",
        "SpecialIssue" => "book",
        "Standard" => "report",
        "StatuteOrRegulation" => "statute",
        "Thesis" => "thesis",
        "Unknown" => "document",
        "UnpublishedWork" => "report",
        _ => return None,
    })
}

type R<T> = Result<T, TranslateError>;

fn type_error(m: &str) -> TranslateError {
    TranslateError::Translator(format!("TypeError: {m}"))
}

/// `ZU.xpathText(node, xpath)` with no namespaces.
fn xt(doc: &XmlDocument, node: impl Into<XNode>, expr: &str) -> R<Option<String>> {
    xpath_text(doc, node, expr, &[], None)
}

/// `doc.getElementById(id)` passed to `ZU.xpath*`: null makes `ZU.xpath`
/// throw (`"length" in null`).
fn by_id(doc: &XmlDocument, id: &str) -> R<NodeId> {
    doc.get_element_by_id(id)
        .ok_or_else(|| type_error("Cannot use 'in' operator to search for 'length' in null"))
}

/// JavaScript string of an `xpathText` result (`"" + null` is "null").
fn js_str(v: &Option<String>) -> String {
    v.clone().unwrap_or_else(|| "null".into())
}

fn truthy(v: &Option<String>) -> bool {
    v.as_deref().is_some_and(|s| !s.is_empty())
}

fn opt_value(v: Option<String>) -> Value {
    v.map_or(Value::Null, Value::String)
}

/// `addExtraLine(item, prefix, text)` (:517-524).
fn add_extra_line(item: &mut TranslatorItem, prefix: &str, text: &Option<String>) {
    if let Some(t) = text.as_deref().filter(|t| !t.is_empty()) {
        let extra = if item.truthy("extra") {
            item.get_string("extra").unwrap_or_default()
        } else {
            String::new()
        };
        item.set("extra", format!("{extra}{prefix}: {t}\n"));
    }
}

/// `extractPages(multilineText)` (:526-532).
fn extract_pages(text: &Option<String>) -> String {
    static R: OnceLock<Regex> = OnceLock::new();
    match text.as_deref().filter(|t| !t.is_empty()) {
        Some(t) => {
            let last = t.split('\n').next_back().unwrap_or("");
            R.get_or_init(|| Regex::new("[^0-9\\-–]").expect("regex"))
                .replace_all(last, "")
                .into_owned()
        }
        None => String::new(),
    }
}

/// `attachName(doc, ids)` (:474-488): the names of the ids after the
/// first (`None` for a null name).
fn attach_name(doc: &XmlDocument, ids: &Option<String>) -> R<Vec<Option<String>>> {
    let mut out = Vec::new();
    let Some(ids) = ids.as_deref().filter(|s| !s.is_empty()) else {
        return Ok(out);
    };
    for id in ids.split(';').skip(1) {
        let n = by_id(doc, id)?;
        out.push(xt(doc, n, "Name")?);
    }
    Ok(out)
}

/// `attachPersons(doc, item, ids, type)` (:493-515).
fn attach_persons(
    doc: &XmlDocument,
    item: &mut TranslatorItem,
    ids: &Option<String>,
    ty: &str,
) -> R<()> {
    let Some(ids) = ids.as_deref().filter(|s| !s.is_empty()) else {
        return Ok(());
    };
    for id in ids.split(';').skip(1) {
        let a = by_id(doc, id)?;
        let last = xt(doc, a, "LastName")?;
        let mut first = xt(doc, a, "FirstName")?;
        let middle = xt(doc, a, "MiddleName")?;
        if truthy(&first) && truthy(&last) {
            if truthy(&middle) {
                first = Some(format!("{} {}", js_str(&first), js_str(&middle)));
            }
            item.creators.push(TranslatorCreator {
                last_name: last.clone(),
                first_name: first.clone(),
                creator_type: Some(ty.to_owned()),
                ..Default::default()
            });
        }
        if !truthy(&first) && truthy(&last) {
            // `fieldMode: true` (== 1).
            item.creators.push(TranslatorCreator {
                last_name: last,
                creator_type: Some(ty.to_owned()),
                field_mode: Some(1),
                ..Default::default()
            });
        }
    }
    Ok(())
}

/// One reference as the translator keeps it (`itemIdList`).
struct Entry {
    id: Option<String>,
    item: TranslatorItem,
    /// Its index among the completed items, once completed.
    saved: Option<usize>,
}

/// `_itemDone`'s creator filter applied to the shared `creators` array
/// (translate.js:144-152: `splice`, so the translator's object sees it).
fn drop_empty_creators(item: &mut TranslatorItem) {
    item.creators.retain(|c| {
        c.first_name.as_deref().is_some_and(|s| !s.is_empty())
            || c.last_name.as_deref().is_some_and(|s| !s.is_empty())
    });
}

fn complete(ctx: &mut ImportContext, e: &mut Entry) {
    e.saved = Some(ctx.items().len());
    ctx.item_done(e.item.clone());
    drop_empty_creators(&mut e.item);
}

/// The groups as tags: reference or knowledge item id -> group names.
type RememberTags = Vec<(String, Vec<Option<String>>)>;

fn remembered<'a>(r: &'a RememberTags, id: &str) -> Option<&'a Vec<Option<String>>> {
    r.iter().find(|(k, _)| k == id).map(|(_, v)| v)
}

/// `importItems` (:95-302).
fn import_items(
    ctx: &mut ImportContext,
    doc: &XmlDocument,
    references: &[XNode],
    version: &Option<String>,
    remember: &RememberTags,
    entries: &mut Vec<Entry>,
    unfinished: &mut Vec<usize>,
) -> R<()> {
    for &r in references {
        let ty = xt(doc, r, "ReferenceType")?;
        let mapped = ty.as_deref().and_then(type_mapping);
        let mut item = TranslatorItem::new(mapped.unwrap_or("journalArticle"));
        let item_id = xt(doc, r, "./@id")?;
        item.set("itemID", opt_value(item_id.clone()));
        let id_s = js_str(&item_id);

        let title = xt(doc, r, "./Title")?;
        item.set("title", opt_value(title.clone()));
        let subtitle = xt(doc, r, "./Subtitle")?;
        if truthy(&subtitle) {
            item.set(
                "title",
                format!("{}: {}", js_str(&title), js_str(&subtitle)),
            );
        }
        for (field, path) in [
            ("abstractNote", "./Abstract"),
            ("url", "./OnlineAddress"),
            ("volume", "./Volume"),
            ("issue", "./Number"),
            ("DOI", "./DOI"),
            ("ISBN", "./ISBN"),
            ("edition", "./Edition"),
            ("place", "./PlaceOfPublication"),
            ("numberOfVolumes", "./NumberOfVolumes"),
        ] {
            let v = xt(doc, r, path)?;
            item.set(field, opt_value(v));
        }
        add_extra_line(&mut item, "PMID", &xt(doc, r, "./PubMedID")?);
        add_extra_line(&mut item, "Citation Key", &xt(doc, r, "./BibTeXKey")?);
        item.set("pages", extract_pages(&xt(doc, r, "./PageRange")?));
        item.set("numPages", extract_pages(&xt(doc, r, "./PageCount")?));
        // `a || b || c`: the first truthy, else the last.
        let mut date = xt(doc, r, "./DateForSorting")?;
        if !truthy(&date) {
            date = xt(doc, r, "./Date")?;
        }
        if !truthy(&date) {
            date = xt(doc, r, "./Year")?;
        }
        item.set("date", opt_value(date));
        item.set("accessDate", opt_value(xt(doc, r, "./AccessDate")?));

        for field in ["Notes", "TableOfContents", "Evaluation"] {
            if let Some(note) = xt(doc, r, &format!("./{field}"))?.filter(|s| !s.is_empty()) {
                let mut props = JsObject::new();
                props.set("tags", Value::Array(vec![format!("#{field}").into()]));
                item.notes.push(TranslatorNote { note, props });
            }
        }

        if let Some(sid) = xt(doc, r, "./SeriesTitleID")?.filter(|s| !s.is_empty()) {
            let s = by_id(doc, &sid)?;
            item.set("series", opt_value(xt(doc, s, "./Name")?));
        }
        if let Some(pid) = xt(doc, r, "./PeriodicalID")?.filter(|s| !s.is_empty()) {
            let p = by_id(doc, &pid)?;
            item.set("publicationTitle", opt_value(xt(doc, p, "./Name")?));
            item.set("ISSN", opt_value(xt(doc, p, "./ISSN")?));
            let mut abbr = xt(doc, p, "./StandardAbbreviation")?;
            if !truthy(&abbr) {
                abbr = xt(doc, p, "./UserAbbreviation1")?;
            }
            if !truthy(&abbr) {
                abbr = xt(doc, p, "./UserAbbreviation2")?;
            }
            item.set("journalAbbreviation", opt_value(abbr));
        }

        let one_to_n = |list: &str| -> R<Option<String>> {
            xt(
                doc,
                doc.document(),
                &format!("//{list}/OnetoN[starts-with(text(), \"{id_s}\")]"),
            )
        };
        attach_persons(doc, &mut item, &one_to_n("ReferenceAuthors")?, "author")?;
        attach_persons(doc, &mut item, &one_to_n("ReferenceEditors")?, "editor")?;
        attach_persons(
            doc,
            &mut item,
            &one_to_n("ReferenceCollaborators")?,
            "contributor",
        )?;
        attach_persons(
            doc,
            &mut item,
            &one_to_n("ReferenceOrganizations")?,
            "contributor",
        )?;

        let publishers = one_to_n("ReferencePublishers")?;
        if truthy(&publishers) {
            let names: Vec<String> = attach_name(doc, &publishers)?
                .into_iter()
                .map(Option::unwrap_or_default)
                .collect();
            item.set("publisher", names.join("; "));
        }
        let keywords = one_to_n("ReferenceKeywords")?;
        if truthy(&keywords) {
            item.tags = attach_name(doc, &keywords)?
                .into_iter()
                .flatten()
                .map(TranslatorTag::new)
                .collect();
        }
        if let Some(tags) = remembered(remember, &id_s) {
            item.tags
                .extend(tags.iter().flatten().cloned().map(TranslatorTag::new));
        }

        // Knowledge items as notes (:186-212).
        for c in xpath(
            doc,
            doc.document(),
            &format!("//KnowledgeItem[ReferenceID=\"{id_s}\"]"),
            &[],
        )? {
            let note_id = xt(doc, c, "@id")?;
            let title = xt(doc, c, "CoreStatement")?;
            let text = xt(doc, c, "Text")?;
            let pages = extract_pages(&xt(doc, c, "PageRange")?);
            let mut note = String::new();
            if truthy(&title) {
                note.push_str(&format!("<h1>{}</h1>\n", js_str(&title)));
            }
            if truthy(&text) {
                note.push_str(&format!("<p>{}</p>\n", js_str(&xt(doc, c, "Text")?)));
            }
            if !pages.is_empty() {
                note.push_str(&format!("<i>{pages}</i>"));
            }
            let mut props = JsObject::new();
            props.set("id", opt_value(note_id.clone()));
            if let Some(t) = remembered(remember, &js_str(&note_id)) {
                props.set(
                    "tags",
                    Value::Array(t.iter().map(|x| opt_value(x.clone())).collect()),
                );
            }
            if !note.is_empty() {
                item.notes.push(TranslatorNote { note, props });
            }
        }

        // Locations (:214-266).
        let mut only_library = Vec::new();
        let mut only_call = Vec::new();
        for l in xpath(
            doc,
            doc.document(),
            &format!("//Locations/Location[ReferenceID=\"{id_s}\"]"),
            &[],
        )? {
            let mut address = xt(doc, l, "Address")?;
            if truthy(&address) {
                let v = version
                    .as_deref()
                    .ok_or_else(|| type_error("Cannot read properties of null (reading '0')"))?;
                // `citaviVersion[0] !== "5"`
                if !v.starts_with('5') {
                    let json: Value = serde_json::from_str(&js_str(&address))
                        .map_err(|e| TranslateError::Translator(format!("SyntaxError: {e}")))?;
                    address = match json.get("UriString") {
                        None | Some(Value::Null) => None,
                        Some(Value::String(s)) => Some(s.clone()),
                        Some(other) => Some(crate::zotero::framework::js::to_js_string(other)),
                    };
                }
            }
            let address_type = xt(doc, l, "MirrorsReferencePropertyId")?;
            if let Some(a) = address.filter(|a| !a.is_empty()) {
                let extra = item.get_string("extra");
                if address_type.as_deref() == Some("Doi") && !item.truthy("DOI") {
                    item.set("DOI", a);
                } else if address_type.as_deref() == Some("PubMedId")
                    && ((item.truthy("extra") && !extra.as_deref().unwrap_or("").contains("PMID"))
                        || !item.truthy("extra"))
                {
                    add_extra_line(&mut item, "PMID", &Some(a));
                } else {
                    let mut att = JsObject::new();
                    if a.starts_with("http://") || a.starts_with("https://") {
                        att.set("url", a);
                        att.set("title", "Online");
                    } else {
                        att.set("path", a);
                        att.set("title", "Full Text");
                    }
                    item.attachments.push(att);
                }
            }
            let call = xt(doc, l, "CallNumber")?;
            let lib = xt(doc, l, "LibraryID")?;
            if truthy(&call) && truthy(&lib) {
                item.set("callNumber", opt_value(call));
                let n = by_id(doc, &js_str(&lib))?;
                item.set("libraryCatalog", opt_value(xt(doc, n, "Name")?));
            } else if truthy(&call) {
                only_call.push(call);
            } else if truthy(&lib) {
                let n = by_id(doc, &js_str(&lib))?;
                only_library.push(xt(doc, n, "Name")?);
            }
        }
        if !item.truthy("callNumber") {
            if let Some(c) = only_call.into_iter().next() {
                item.set("callNumber", opt_value(c));
            } else if let Some(l) = only_library.into_iter().next() {
                item.set("libraryCatalog", opt_value(l));
            }
        }

        if item.truthy("DOI")
            && item.item_type != "journalArticle"
            && item.item_type != "conferencePaper"
        {
            let doi = item.get_string("DOI");
            add_extra_line(&mut item, "DOI", &doi);
        }

        let mut e = Entry {
            id: item_id,
            item,
            saved: None,
        };
        if ty.as_deref() == Some("Contribution") {
            entries.push(e);
            unfinished.push(entries.len() - 1);
        } else {
            complete(ctx, &mut e);
            entries.push(e);
        }
    }
    Ok(())
}

/// `importUnfinished` (:306-340).
fn import_unfinished(
    ctx: &mut ImportContext,
    doc: &XmlDocument,
    entries: &mut [Entry],
    unfinished: &[usize],
) -> R<()> {
    for &u in unfinished {
        let id_s = js_str(&entries[u].id);
        let container = xt(
            doc,
            doc.document(),
            &format!("//ReferenceReferences/OnetoN[contains(text(), \"{id_s}\")]"),
        )?;
        if let Some(cs) = container.filter(|s| !s.is_empty()) {
            let cid = cs.split(';').next().unwrap_or("").to_owned();
            // `itemIdList[containerId]`: the last item with that id.
            let ci = entries
                .iter()
                .rposition(|e| js_str(&e.id) == cid)
                .ok_or_else(|| {
                    type_error("Cannot read properties of undefined (reading 'type')")
                })?;
            if entries[ci].item.get_str("type") == Some("ConferenceProceedings") {
                entries[u].item.item_type = "conferencePaper".into();
            }
            let fields = [
                "title",
                "place",
                "publisher",
                "ISBN",
                "volume",
                "edition",
                "series",
            ];
            let targets = [
                "publicationTitle",
                "place",
                "publisher",
                "ISBN",
                "volume",
                "edition",
                "series",
            ];
            for (f, t) in fields.iter().zip(targets) {
                let v = entries[ci].item.get(f).cloned().unwrap_or(Value::Null);
                entries[u].item.set(t, v);
            }
            for j in 0..entries[ci].item.creators.len() {
                if entries[ci].item.creators[j].creator_type.as_deref() == Some("author") {
                    entries[ci].item.creators[j].creator_type = Some("bookAuthor".into());
                    if let Some(s) = entries[ci].saved {
                        if let Some(c) = ctx.items_mut()[s].creators.get_mut(j) {
                            c.creator_type = Some("bookAuthor".into());
                        }
                    }
                }
                let c = entries[ci].item.creators[j].clone();
                entries[u].item.creators.push(c);
            }
            let see = opt_value(entries[ci].id.clone());
            entries[u].item.see_also.push(see);
        }
        let mut e = Entry {
            id: entries[u].id.clone(),
            item: entries[u].item.clone(),
            saved: None,
        };
        complete(ctx, &mut e);
        entries[u] = e;
    }
    Ok(())
}

/// `importTasks` (:343-364): standalone notes.
fn import_tasks(ctx: &mut ImportContext, doc: &XmlDocument, tasks: &[XNode]) -> R<()> {
    for &t in tasks {
        let mut item = TranslatorItem::new("note");
        let name = js_str(&xt(doc, t, "./Name")?);
        let due = xt(doc, t, "./DueDate")?;
        let mut note = if truthy(&due) {
            format!("<h1>{name} until {}</h1>", js_str(&due))
        } else {
            format!("<h1>{name}</h1>")
        };
        let text = xt(doc, t, "./Notes")?;
        if truthy(&text) {
            note.push_str(&format!("\n{}", js_str(&text)));
        }
        item.set("note", note);
        item.see_also.push(opt_value(xt(doc, t, "./ReferenceID")?));
        item.tags.push(TranslatorTag::new("#todo"));
        ctx.item_done(item);
    }
    Ok(())
}

/// A collection while categories are imported (`collectionsMap`): the
/// collections are shared objects upstream (a child is the same object as
/// the map's entry), so they live in an arena here.
struct Coll {
    name: Option<String>,
    children: Vec<Child>,
}

enum Child {
    Item(String),
    Coll(usize),
}

/// `addHierarchyNumberRecursive(collections, level)` (:366-375).
fn number(arena: &mut [Coll], colls: &[usize], level: Option<&str>) {
    for (i, &c) in colls.iter().enumerate() {
        let n = match level {
            None => format!("{}", i + 1),
            Some(l) => format!("{l}.{}", i + 1),
        };
        let name = arena[c].name.clone().unwrap_or_else(|| "null".into());
        arena[c].name = Some(format!("{n} {name}"));
        let kids: Vec<usize> = arena[c]
            .children
            .iter()
            .filter_map(|k| match k {
                Child::Coll(x) => Some(*x),
                Child::Item(_) => None,
            })
            .collect();
        number(arena, &kids, Some(&n));
    }
}

fn to_collection(arena: &[Coll], c: usize, depth: usize) -> TranslatorCollection {
    TranslatorCollection {
        name: arena[c].name.clone().unwrap_or_default(),
        children: arena[c]
            .children
            .iter()
            .filter_map(|k| match k {
                Child::Item(id) => Some(CollectionChild::Item { id: id.clone() }),
                // A cycle in the category hierarchy would recurse forever.
                Child::Coll(x) if depth < 64 => Some(CollectionChild::Collection(to_collection(
                    arena,
                    *x,
                    depth + 1,
                ))),
                Child::Coll(_) => None,
            })
            .collect(),
    }
}

/// `importCategories` (:377-433).
fn import_categories(ctx: &mut ImportContext, doc: &XmlDocument, categories: &[XNode]) -> R<()> {
    let hierarchy = xpath(
        doc,
        doc.document(),
        "//CategoryCatgories/OnetoN|//CategoryCategories/OnetoN",
        &[],
    )?;
    let mut parent_map: Vec<(String, Vec<String>)> = Vec::new();
    for h in hierarchy {
        let text = doc.text(h);
        let mut parts = text.split(';').map(str::to_owned);
        let first = parts.next().unwrap_or_default();
        let rest: Vec<String> = parts.collect();
        match parent_map.iter_mut().find(|(k, _)| *k == first) {
            Some(x) => x.1 = rest,
            None => parent_map.push((first, rest)),
        }
    }
    let mut arena: Vec<Coll> = Vec::new();
    // collectionsMap: id -> arena index, insertion order (`Map.set` of an
    // existing key keeps its place and replaces the value).
    let mut map: Vec<(String, usize)> = Vec::new();
    for &c in categories {
        let id = xt(doc, c, "./@id")?;
        let name = xt(doc, c, "./Name")?;
        let id_s = js_str(&id);
        let mut children = Vec::new();
        for rc in xpath(
            doc,
            doc.document(),
            &format!("//ReferenceCategories/OnetoN[contains(text(), \"{id_s}\")]"),
            &[],
        )? {
            let refid = doc.text(rc).split(';').next().unwrap_or("").to_owned();
            children.push(Child::Item(refid));
        }
        arena.push(Coll { name, children });
        let idx = arena.len() - 1;
        match map.iter_mut().find(|(k, _)| *k == id_s) {
            Some(x) => x.1 = idx,
            None => map.push((id_s, idx)),
        }
    }
    let mut added: Vec<String> = Vec::new();
    for (parent, kids) in &parent_map {
        let Some(&(_, p)) = map.iter().find(|(k, _)| k == parent) else {
            continue;
        };
        for k in kids {
            if let Some(&(_, ci)) = map.iter().find(|(m, _)| m == k) {
                arena[p].children.push(Child::Coll(ci));
                added.push(k.clone());
            }
        }
    }
    for a in &added {
        map.retain(|(k, _)| k != a);
    }
    let roots: Vec<usize> = map.iter().map(|(_, i)| *i).collect();
    number(&mut arena, &roots, None);
    for r in roots {
        ctx.collection_done(to_collection(&arena, r, 0));
    }
    Ok(())
}

/// `doImport` (:435-472).
pub fn do_import(ctx: &mut ImportContext) -> Result<(), TranslateError> {
    let doc = get_xml(ctx)?;
    let root = doc.document();
    let version = xt(&doc, root, "//CitaviExchangeData/@Version")?;

    let mut remember: RememberTags = Vec::new();
    for g in xpath(&doc, root, "//Groups/Group", &[])? {
        let id = js_str(&xt(&doc, g, "./@id")?);
        let name = xt(&doc, g, "./Name")?;
        let expr = format!(
            "//ReferenceGroups/OnetoN[contains(text(), \"{id}\")]|//KnowledgeItemGroups/OnetoN[contains(text(), \"{id}\")]"
        );
        for rg in xpath(&doc, root, &expr, &[])? {
            let refid = doc.text(rg).split(';').next().unwrap_or("").to_owned();
            match remember.iter_mut().find(|(k, _)| *k == refid) {
                Some(x) => x.1.push(name.clone()),
                None => remember.push((refid, vec![name.clone()])),
            }
        }
    }
    let tasks = xpath(&doc, root, "//TaskItems/TaskItem", &[])?;
    let categories = xpath(&doc, root, "//Categories/Category", &[])?;
    let references = xpath(&doc, root, "//References/Reference", &[])?;
    let mut entries: Vec<Entry> = Vec::new();
    let mut unfinished: Vec<usize> = Vec::new();
    import_items(
        ctx,
        &doc,
        &references,
        &version,
        &remember,
        &mut entries,
        &mut unfinished,
    )?;
    import_unfinished(ctx, &doc, &mut entries, &unfinished)?;
    import_tasks(ctx, &doc, &tasks)?;
    import_categories(ctx, &doc, &categories)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::zotero::framework::TranslateOptions;

    /// A container's author copied to a contribution becomes `bookAuthor`
    /// in the saved container as well (shared creator objects). Expected
    /// creator types recorded from the reference translation-server's
    /// `/import` on this input (2026-10-07): container `bookAuthor`;
    /// contribution `author`, `bookAuthor`.
    #[test]
    fn container_authors_become_book_authors_in_both_items() {
        let input = r#"<?xml version="1.0" encoding="utf-8"?><CitaviExchangeData Version="5.7.1.0"><Persons><Person id="p1"><FirstName>Ada</FirstName><LastName>Lovelace</LastName></Person><Person id="p2"><FirstName>Bob</FirstName><LastName>B</LastName></Person></Persons><References><Reference id="r4"><ReferenceType>BookEdited</ReferenceType><Title>Container</Title></Reference><Reference id="r3"><ReferenceType>Contribution</ReferenceType><Title>Chapter</Title></Reference></References><ReferenceAuthors><OnetoN>r4;p1</OnetoN><OnetoN>r3;p2</OnetoN></ReferenceAuthors><ReferenceReferences><OnetoN>r4;r3</OnetoN></ReferenceReferences></CitaviExchangeData>"#;
        let mut ctx = ImportContext::new(
            input,
            &METADATA,
            TranslateOptions::for_translator(&METADATA),
        );
        do_import(&mut ctx).unwrap();
        let items = ctx.finish().items;
        let types = |i: usize| -> Vec<String> {
            items[i]
                .creators
                .iter()
                .map(|c| c.creator_type.clone().unwrap_or_default())
                .collect()
        };
        assert_eq!(types(0), ["bookAuthor"]);
        assert_eq!(types(1), ["author", "bookAuthor"]);
        assert_eq!(items[1].get_str("publicationTitle"), Some("Container"));
    }
}
