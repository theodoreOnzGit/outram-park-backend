// Part of the kovan Zotero port (GitHub #747, #749).
//
// Upstream: Zotero translators, https://github.com/zotero/translators
//   (commit 3d1c78530f42): "MODS.js" (lastUpdated 2022-10-31 14:01:52):
//   `processTitleInfo` :699-711, `processTitle` :713-727, `processGenre`
//   :727-771, `processItemType` :773-844, `processCreator` :842-918,
//   `processCreators` :920-926, `processExtent` :928-1053,
//   `processIdentifiers` :1055-1073, `getFirstResult` :1075-1081,
//   `doImport` :1083-1440; Zotero utilities (commit 4051881d59c6)
//   utilities.js `capitalizeTitle` :1067-1135.
// Copyright (c) 2019-2021 Simon Kornblith, Richard Karnesky, and Abe Jellinek.
// Licence: AGPL-3.0 (upstream: AGPL-3.0-or-later).

//! MODS import.
//!
//! Properties are assigned in upstream's order, `null` included (JavaScript
//! creates the property even for a null value; `_itemDone` drops it later),
//! because the order decides which of two fields with the same base field
//! `itemToAPIJSON` keeps.

use super::{
    lookup, DCT_GENRES, FROM_MARC_GENRE, FROM_TYPE_OF_RESOURCE, MARC_RELATORS,
    MODS_INTERNET_MEDIA_TYPES, MODS_TYPE_REGEX, XNS,
};
use crate::zotero::framework::js;
// ZU.capitalizeTitle: the framework's port (develop's title_case.rs, #749).
use crate::zotero::framework::title_case::capitalize_title;
use crate::zotero::framework::utilities::{
    clean_author, field_is_valid_for_type, get_creators_for_type, item_type_exists, trim_internal,
};
use crate::zotero::framework::xml::{node_type, NodeId, XNode, XmlDocument};
use crate::zotero::framework::xpath::{xpath, xpath_all, xpath_text};
use crate::zotero::framework::{
    ImportContext, JsObject, TranslateError, TranslatorCreator, TranslatorItem, TranslatorNote,
    TranslatorTag,
};
use regex::Regex;
use serde_json::Value;
use std::sync::OnceLock;

type R<T> = Result<T, TranslateError>;

fn re(cell: &'static OnceLock<Regex>, pattern: impl FnOnce() -> String) -> &'static Regex {
    cell.get_or_init(|| Regex::new(&pattern()).expect("static regex compiles"))
}

/// `ZU.xpath(node, expr, xns)` returning element ids.
fn xp(doc: &XmlDocument, node: impl Into<XNode>, expr: &str) -> R<Vec<XNode>> {
    xpath(doc, node, expr, XNS)
}

fn xpt(doc: &XmlDocument, node: impl Into<XNode>, expr: &str) -> R<Option<String>> {
    xpath_text(doc, node, expr, XNS, None)
}

fn xpt_d(doc: &XmlDocument, node: impl Into<XNode>, expr: &str, d: &str) -> R<Option<String>> {
    xpath_text(doc, node, expr, XNS, Some(d))
}

fn opt(v: Option<String>) -> Value {
    v.map_or(Value::Null, Value::String)
}

/// `getAttribute` as JS: `null` when absent.
fn attr(doc: &XmlDocument, x: XNode, name: &str) -> Option<String> {
    doc.get_attribute(x.node(), name).map(str::to_owned)
}

/// `processTitleInfo` (:699-711). Upstream calls `.trim()` on the title's
/// text, which throws when the titleInfo has no `m:title`.
fn process_title_info(doc: &XmlDocument, title_info: XNode) -> R<String> {
    let Some(t) = xpt(doc, title_info, "m:title[1]")? else {
        return Err(TranslateError::Translator(
            "TypeError: Cannot read properties of null (reading 'trim')".into(),
        ));
    };
    let mut title = js::trim(&t).to_owned();
    if let Some(sub) = xpt(doc, title_info, "m:subTitle[1]")?.filter(|s| !s.is_empty()) {
        title = format!(
            "{}: {}",
            title.strip_suffix(':').unwrap_or(&title),
            js::trim(&sub)
        );
    }
    if let Some(ns) = xpt(doc, title_info, "m:nonSort[1]")?.filter(|s| !s.is_empty()) {
        title = format!("{} {title}", js::trim(&ns));
    }
    let part_number = xpt(doc, title_info, "m:partNumber[1]")?.filter(|s| !s.is_empty());
    let part_name = xpt(doc, title_info, "m:partName[1]")?.filter(|s| !s.is_empty());
    let strip_dot = |t: &str| t.strip_suffix('.').unwrap_or(t).to_owned();
    match (part_number, part_name) {
        (Some(n), Some(m)) => {
            title = format!("{}. {}: {}", strip_dot(&title), js::trim(&n), js::trim(&m))
        }
        (Some(n), None) => title = format!("{}. {}", strip_dot(&title), js::trim(&n)),
        (None, Some(m)) => title = format!("{}. {}", strip_dot(&title), js::trim(&m)),
        (None, None) => {}
    }
    Ok(title)
}

/// `processTitle` (:713-727).
fn process_title(doc: &XmlDocument, ctx: XNode) -> R<Option<String>> {
    let els = xp(doc, ctx, "m:titleInfo[not(@type)][m:title][1]")?;
    if let Some(&e) = els.first() {
        return Ok(Some(process_title_info(doc, e)?));
    }
    if let Some(t) = xpt(doc, ctx, "m:titleInfo[not(@type)][1]")?.filter(|s| !s.is_empty()) {
        return Ok(Some(t));
    }
    xpt(doc, ctx, "m:titleInfo[1]")
}

fn remove_ws(s: &str) -> String {
    s.chars().filter(|&c| !js::is_space(c)).collect()
}

fn type_regex_match(s: &str) -> Option<&'static str> {
    static RES: OnceLock<Vec<(&'static str, Regex)>> = OnceLock::new();
    let res = RES.get_or_init(|| {
        MODS_TYPE_REGEX
            .iter()
            .map(|(t, p)| (*t, Regex::new(&format!("(?i){p}")).expect("regex")))
            .collect()
    });
    res.iter().find(|(_, r)| r.is_match(s)).map(|(t, _)| *t)
}

/// `processGenre` (:727-771).
fn process_genre(doc: &XmlDocument, ctx: XNode) -> R<Option<String>> {
    for g in xp(doc, ctx, "m:genre[@authority=\"local\"]")? {
        let s = doc.text(g);
        if item_type_exists(&s) {
            return Ok(Some(s));
        }
    }
    for g in xp(
        doc,
        ctx,
        "m:genre[@authority=\"marcgt\"] | m:genre[@authority=\"marc\"]",
    )? {
        if let Some(t) = lookup(FROM_MARC_GENRE, &doc.text(g)) {
            return Ok(Some(t.to_owned()));
        }
    }
    for g in xp(doc, ctx, "m:genre[@authority=\"dct\"]")? {
        if let Some(t) = lookup(DCT_GENRES, &remove_ws(&doc.text(g))) {
            return Ok(Some(t.to_owned()));
        }
    }
    for g in xp(doc, ctx, "m:genre")? {
        let s = doc.text(g);
        if item_type_exists(&s) {
            return Ok(Some(s));
        }
        if let Some(t) = lookup(FROM_MARC_GENRE, &s) {
            return Ok(Some(t.to_owned()));
        }
        if let Some(t) = lookup(DCT_GENRES, &remove_ws(&s)) {
            return Ok(Some(t.to_owned()));
        }
        if let Some(t) = type_regex_match(&s) {
            return Ok(Some(t.to_owned()));
        }
    }
    Ok(None)
}

/// `processItemType` (:773-844).
fn process_item_type(doc: &XmlDocument, ctx: XNode) -> R<String> {
    if let Some(t) = process_genre(doc, ctx)? {
        return Ok(t);
    }
    for t in xp(doc, ctx, "m:typeOfResource")? {
        let s = js::trim(&doc.text(t)).to_owned();
        if let Some(x) = lookup(FROM_TYPE_OF_RESOURCE, &s) {
            return Ok(x.to_owned());
        }
        if let Some(x) = type_regex_match(&s) {
            return Ok(x.to_owned());
        }
    }
    let hosts = xp(doc, ctx, "m:relatedItem[@type=\"host\"]")?;
    for &h in &hosts {
        if let Some(t) = process_genre(doc, h)? {
            return Ok(t);
        }
    }
    let periodical = !xp(
        doc,
        ctx,
        "m:relatedItem[@type=\"host\"]/m:originInfo/m:issuance[text()=\"continuing\" or text()=\"serial\"]",
    )?
    .is_empty();
    for t in xp(doc, ctx, "m:physicalDescription/m:internetMediaType")? {
        if let Some(x) = lookup(MODS_INTERNET_MEDIA_TYPES, js::trim(&doc.text(t))) {
            return Ok(x.to_owned());
        }
    }
    let is_letter = !xp(
        doc,
        ctx,
        "m:name/m:role/m:roleTerm[@type=\"code\"][contains(@authority, \"marc\") or contains(@authority, \"MARC\")][text()=\"rcp\"]",
    )?
    .is_empty();
    if is_letter {
        return Ok("letter".into());
    }
    if !xp(doc, ctx, "//m:billNumber")?.is_empty() {
        return Ok("bill".into());
    }
    if !xp(doc, ctx, "//m:congCommittee")?.is_empty() {
        return Ok("hearing".into());
    }
    if !hosts.is_empty() {
        return Ok(if periodical {
            "journalArticle"
        } else {
            "bookSection"
        }
        .into());
    }
    Ok("document".into())
}

/// `processCreator` (:842-918).
fn process_creator(
    doc: &XmlDocument,
    name: XNode,
    item_type: &str,
    default_type: &str,
) -> R<Option<TranslatorCreator>> {
    let mut c = TranslatorCreator {
        first_name: xpt_d(doc, name, "m:namePart[@type=\"given\"]", " ")?.filter(|s| !s.is_empty()),
        last_name: xpt_d(doc, name, "m:namePart[@type=\"family\"]", " ")?,
        ..Default::default()
    };
    if c.last_name.as_deref().is_none_or(str::is_empty) {
        let personal = attr(doc, name, "type").as_deref() == Some("personal");
        let backup = xpt_d(
            doc,
            name,
            "m:namePart[not(@type=\"date\")][not(@type=\"termsOfAddress\")]",
            if personal { " " } else { ": " },
        )?;
        let Some(backup) = backup.filter(|s| !s.is_empty()) else {
            return Ok(None);
        };
        if personal {
            static BR: OnceLock<Regex> = OnceLock::new();
            let cleaned = re(&BR, || r"[\[(][^A-Za-z]*[\])]".into()).replace_all(&backup, "");
            let a = clean_author(&cleaned, "author", cleaned.contains(','));
            c = TranslatorCreator {
                first_name: a.first_name,
                last_name: Some(a.last_name),
                ..Default::default()
            };
        } else {
            c.last_name = Some(trim_internal(&backup));
            c.field_mode = Some(1);
        }
    }
    if c.last_name.as_deref().is_none_or(str::is_empty) {
        return Ok(None);
    }
    let valid = get_creators_for_type(item_type);
    for role in xp(doc, name, "m:role/m:roleTerm[@type=\"text\" or not(@type)]")? {
        let s = doc.text(role).to_lowercase();
        if valid.contains(&s.as_str()) {
            c.creator_type = Some(s);
        }
    }
    if item_type == "bill" && attr(doc, name, "type").as_deref() == Some("corporate") {
        c.creator_type = Some("contributor".into());
    }
    let mut only_publisher: Option<bool> = None;
    if c.creator_type.is_none() {
        for role in xp(
            doc,
            name,
            "m:role/m:roleTerm[@type=\"code\"][contains(@authority, \"marc\") or contains(@authority, \"MARC\")]",
        )? {
            let s = doc.text(role).to_lowercase();
            if s == "pbl" || s == "dst" {
                if only_publisher.is_none() {
                    only_publisher = Some(true);
                }
            } else {
                only_publisher = Some(false);
            }
            if let Some(m) = lookup(MARC_RELATORS, &s) {
                if valid.contains(&m) {
                    c.creator_type = Some(m.to_owned());
                }
            }
        }
        if c.creator_type.is_none() {
            c.creator_type = Some(default_type.to_owned());
        }
    }
    if only_publisher == Some(true) {
        return Ok(None);
    }
    Ok(Some(c))
}

/// `processCreators` (:920-926).
fn process_creators(
    doc: &XmlDocument,
    ctx: XNode,
    item: &mut TranslatorItem,
    default_type: &str,
) -> R<()> {
    for n in xp(doc, ctx, "m:name")? {
        if let Some(c) = process_creator(doc, n, &item.item_type, default_type)? {
            item.creators.push(c);
        }
    }
    Ok(())
}

/// A JavaScript value for the running-time arithmetic (:995-1030).
#[derive(Clone)]
enum Jv {
    N(f64),
    S(String),
}

impl Jv {
    fn num(&self) -> f64 {
        match self {
            Jv::N(n) => *n,
            Jv::S(s) => crate::zotero::framework::xpath::js_to_number(s),
        }
    }
    fn string(&self) -> String {
        match self {
            Jv::N(n) => crate::zotero::framework::xpath::js_number_to_string(*n),
            Jv::S(s) => s.clone(),
        }
    }
    /// `a += b`.
    fn add(&self, b: &Jv) -> Jv {
        match (self, b) {
            (Jv::N(x), Jv::N(y)) => Jv::N(x + y),
            _ => Jv::S(format!("{}{}", self.string(), b.string())),
        }
    }
}

/// `processExtent` (:928-1053).
fn process_extent(extent: &str, item: &mut TranslatorItem) {
    let ws = js::WS;
    // extentRe: `.` matches no line terminator, so a terminator anywhere
    // makes the whole match fail.
    let ma: Option<(String, Option<String>)> = if extent
        .chars()
        .any(|c| matches!(c, '\n' | '\r' | '\u{2028}' | '\u{2029}'))
    {
        None
    } else {
        let p = extent.find([':', ';']).unwrap_or(extent.len());
        let m1 = extent[..p].to_owned();
        let rest = &extent[p..];
        let m2 = if let Some(r) = rest.strip_prefix(';') {
            Some(r.to_owned())
        } else if rest.starts_with(':') {
            rest.find(';').map(|q| rest[q + 1..].to_owned())
        } else {
            None
        };
        Some((m1, m2))
    };
    let ty = item.item_type.clone();
    if let Some((m1, _)) = ma.as_ref().filter(|(m1, _)| !m1.is_empty()) {
        let m1 = match m1.find('+') {
            Some(i) => m1[..i].to_owned(),
            None => m1.clone(),
        };
        static PAGES: OnceLock<Regex> = OnceLock::new();
        static VOL: OnceLock<Regex> = OnceLock::new();
        static ISSUE: OnceLock<Regex> = OnceLock::new();
        static NUMPAGES: OnceLock<Regex> = OnceLock::new();
        static NVOL: OnceLock<Regex> = OnceLock::new();
        static WS1: OnceLock<Regex> = OnceLock::new();
        if !item.truthy("pages") && field_is_valid_for_type("pages", &ty) {
            let r = re(&PAGES, || {
                format!(r"(?i)(?-u:\b)p(?:ages?)?\.?{ws}+([a-z]?[0-9]+(?:{ws}*-{ws}*[a-z]?[0-9]+))")
            });
            if let Some(c) = r.captures(&m1) {
                let w = re(&WS1, || format!("{ws}+"));
                item.set("pages", w.replace(&c[1], "").into_owned());
            }
        }
        if !item.truthy("volume") && field_is_valid_for_type("volume", &ty) {
            let r = re(&VOL, || {
                format!(r"(?i)(?-u:\b)v(?:ol(?:ume)?)?\.?{ws}+([0-9]+)")
            });
            if let Some(c) = r.captures(&m1) {
                item.set("volume", c[1].to_owned());
            }
        }
        if !item.truthy("issue") && field_is_valid_for_type("issue", &ty) {
            let r = re(&ISSUE, || {
                format!(r"(?i)(?-u:\b)(?:no?|iss(?:ue)?)\.?{ws}+([0-9]+)")
            });
            if let Some(c) = r.captures(&m1) {
                item.set("issue", c[1].to_owned());
            }
        }
        if !item.truthy("numPages") && field_is_valid_for_type("numPages", &ty) {
            let r = re(&NUMPAGES, || {
                format!(r"(?i)([0-9]+){ws}*p(?:ages?)?(?-u:\b)")
            });
            if let Some(c) = r.captures(&m1) {
                item.set("numPages", c[1].to_owned());
            }
        }
        if !item.truthy("numberOfVolumes") && field_is_valid_for_type("numberOfVolumes", &ty) {
            let r = re(&NVOL, || {
                format!(r"(?i)([0-9]+){ws}+(?:v(?:olumes?)?|scores?|sound|video)(?-u:\b)")
            });
            if let Some(c) = r.captures(&m1) {
                item.set("numberOfVolumes", c[1].to_owned());
            }
        }
        if !item.truthy("runningTime") && field_is_valid_for_type("runningTime", &ty) {
            running_time(&m1, item);
        }
    }
    if let Some((_, Some(m2))) = ma
        .as_ref()
        .filter(|(_, m2)| m2.as_deref().is_some_and(|x| !x.is_empty()))
    {
        if !item.truthy("artworkSize") && field_is_valid_for_type("artworkSize", &ty) {
            let m2 = match m2.find('+') {
                Some(i) => &m2[..i],
                None => m2.as_str(),
            };
            static DIM: OnceLock<Regex> = OnceLock::new();
            let r = re(&DIM, || {
                format!(
                    r"(?i)(?:(?:(?:[0-9]+{ws}+)?[0-9]+/)?[0-9]+{ws}*x{ws}*)?(?:(?:[0-9]+{ws}+)?[0-9]+/)?[0-9]+{ws}*(?:cm|mm|m|in|ft)\."
                )
            });
            if let Some(m) = r.find(m2) {
                item.set("artworkSize", m.as_str().to_owned());
            }
        }
    }
}

/// The `runningTime` branch of `processExtent` (:986-1040), with
/// JavaScript's mixed string/number arithmetic.
fn running_time(m1: &str, item: &mut TranslatorItem) {
    let ws = js::WS;
    static A: OnceLock<Regex> = OnceLock::new();
    static B: OnceLock<Regex> = OnceLock::new();
    static C: OnceLock<Regex> = OnceLock::new();
    let a = re(&A, || {
        r"(?-u:\b)([0-9]{2,3})([0-9]{2})([0-9]{2})(?-u:\b)".into()
    });
    if let Some(c) = a.captures(m1) {
        item.set("runningTime", format!("{}:{}:{}", &c[1], &c[2], &c[3]));
        return;
    }
    let unit = r"((?:hours?|hrs?)|(?:minutes?|mins?)|(?:seconds?|secs?))";
    let b = re(&B, || {
        format!(
            r"(?i)(([0-9]+){ws}*{unit}\.?{ws}+)?(([0-9]+){ws}*{unit}\.?{ws}+)?(([0-9]+){ws}*{unit}\.?)"
        )
    });
    if let Some(rt) = b.captures(m1) {
        let (mut hrs, mut mins, mut secs) = (Jv::N(0.0), Jv::N(0.0), Jv::N(0.0));
        // Groups: 1 (2 3) 4 (5 6) 7 (8 9); upstream's `rt[i]` for i = 2, 4, 6
        // reads the groups numbered 2, 4, 6 (a number, a whole part, a
        // number) and `rt[i-1]` the groups before them.
        for i in [2usize, 4, 6] {
            let Some(g) = rt.get(i).map(|m| m.as_str()).filter(|s| !s.is_empty()) else {
                continue;
            };
            let prev = rt
                .get(i - 1)
                .map(|m| m.as_str().to_owned())
                .unwrap_or_default();
            match g.chars().next().map(|c| c.to_ascii_lowercase()) {
                Some('h') => hrs = Jv::S(prev),
                Some('m') => mins = Jv::S(prev),
                Some('s') => secs = Jv::S(prev),
                _ => {}
            }
        }
        if secs.num() > 59.0 {
            mins = mins.add(&Jv::N(secs.num() / 60.0));
            secs = Jv::N(secs.num() % 60.0);
        }
        if secs.num() < 10.0 {
            secs = Jv::S(format!("0{}", secs.string()));
        }
        if mins.num() > 59.0 {
            hrs = hrs.add(&Jv::N(hrs.num() / 60.0));
            mins = Jv::N(mins.num() % 60.0);
        }
        if mins.num() < 10.0 {
            mins = Jv::S(format!("0{}", mins.string()));
        }
        let h = hrs.num() * 1.0;
        let lead = if h != 0.0 && !h.is_nan() {
            format!("{}:", hrs.string())
        } else {
            String::new()
        };
        item.set(
            "runningTime",
            format!("{lead}{}:{}", mins.string(), secs.string()),
        );
        return;
    }
    let c = re(&C, || {
        r"(?-u:\b)([0-9]{0,3}:[0-9]{1,2}:[0-9]{2})(?-u:\b)".into()
    });
    if let Some(m) = c.captures(m1) {
        item.set("runningTime", m[1].to_owned());
    }
}

/// `processIdentifiers` (:1055-1073).
fn process_identifiers(doc: &XmlDocument, ctx: XNode, item: &mut TranslatorItem) -> R<()> {
    static ISBN_DASH: OnceLock<Regex> = OnceLock::new();
    static ISBN: OnceLock<Regex> = OnceLock::new();
    static ISSN: OnceLock<Regex> = OnceLock::new();
    let ws = js::WS;
    let mut isbns = Vec::new();
    for n in xp(doc, ctx, ".//m:identifier[@type=\"isbn\"]")? {
        let t = re(&ISBN_DASH, || format!("{ws}*-{ws}*"))
            .replace_all(&doc.text(n), "")
            .into_owned();
        if let Some(m) = re(&ISBN, || r"(?i)(?:[0-9X]{10}|[0-9]{13})".into()).find(&t) {
            isbns.push(m.as_str().to_owned());
        }
    }
    if !isbns.is_empty() {
        item.set("ISBN", isbns.join(", "));
    }
    let mut issns = Vec::new();
    for n in xp(doc, ctx, ".//m:identifier[@type=\"issn\"]")? {
        let t = doc.text(n);
        let r = re(&ISSN, || {
            format!(r"(?i)(?-u:\b)[0-9]{{4}}{ws}*-?{ws}*[0-9]{{4}}(?-u:\b)")
        });
        if let Some(m) = r.find(&t) {
            issns.push(m.as_str().to_owned());
        }
    }
    if !issns.is_empty() {
        item.set("ISSN", issns.join(", "));
    }
    item.set("DOI", opt(xpt(doc, ctx, "m:identifier[@type=\"doi\"]")?));
    Ok(())
}

/// `getFirstResult` (:1075-1081) over several context nodes.
fn first_result(doc: &XmlDocument, nodes: &[XNode], xpaths: &[&str]) -> R<Option<String>> {
    for x in xpaths {
        let r = xpath_all(doc, nodes, x, XNS)?;
        if let Some(&f) = r.first() {
            return Ok(Some(doc.text(f)));
        }
    }
    Ok(None)
}


/// `doImport` (:1083-1440).
pub fn do_import(ctx: &mut ImportContext) -> R<()> {
    let doc = crate::zotero::framework::xml::get_xml(ctx)?;
    let root = XNode::Node(doc.document());
    let mods_elements = xp(&doc, root, "/m:mods | /m:modsCollection/m:mods")?;
    for &mods in &mods_elements {
        let item = import_one(&doc, mods)?;
        ctx.item_done(item);
    }
    Ok(())
}

fn import_one(doc: &XmlDocument, mods: XNode) -> R<TranslatorItem> {
    let d = doc;
    let mut item = TranslatorItem::new(String::new());
    item.set("title", opt(process_title(d, mods)?));
    let abbr = xp(d, mods, "m:titleInfo[@type=\"abbreviated\"]")?;
    if let Some(&a) = abbr.first() {
        item.set("shortTitle", process_title_info(d, a)?);
    }
    item.item_type = process_item_type(d, mods)?;
    let default_type = get_creators_for_type(&item.item_type)
        .first()
        .map(|s| (*s).to_owned())
        .unwrap_or_default();
    process_creators(d, mods, &mut item, &default_type)?;
    item.set(
        "source",
        opt(xpt(d, mods, "m:recordInfo/m:recordContentSource")?),
    );
    item.set(
        "accessionNumber",
        opt(xpt(d, mods, "m:recordInfo/m:recordIdentifier")?),
    );
    item.set("rights", opt(xpt(d, mods, "m:accessCondition")?));

    if item.item_type == "hearing" || item.item_type == "bill" {
        item.set(
            "committee",
            opt(xpt(
                d,
                mods,
                "m:extension/m:congCommittee/m:name[@type=\"authority-standard\"]",
            )?),
        );
        if let Some(ch) = xpt(d, mods, "m:extension/m:chamber|m:extension/m:originChamber")?
            .filter(|s| !s.is_empty())
        {
            item.set("legislativeBody", capitalize_title(&ch, true));
        }
        item.set("session", opt(xpt(d, mods, "m:extension/m:congress")?));
        item.set("documentNumber", opt(xpt(d, mods, "m:extension/m:number")?));
        if item.item_type == "bill" {
            item.creators.clear();
            let st = xpt(d, mods, "m:extension/m:chamber|m:extension/m:shortTitle[1]")?;
            item.set("shortTitle", opt(st.clone()));
            if let Some(st) = st.filter(|s| !s.is_empty()) {
                let title = item.get("title").cloned().unwrap_or(Value::Null);
                item.set("abstractNote", title);
                item.set("title", st);
            }
            let bill_number = xpt(d, mods, "m:extension/m:chamber|m:extension/m:billNumber")?;
            let bill_type = xpt(d, mods, "m:extension/m:chamber|m:extension/m:docClass")?;
            let map = [
                ("hr", "H.R."),
                ("s", "S."),
                ("hjres", "H.R.J. Res."),
                ("sjres", "S.J. Res."),
                ("hconres", "H.R. Con. R."),
                ("sconres", "S. Con. R."),
                ("hres", "H.R. Res."),
                ("sres", "S. Res."),
            ];
            let bn = bill_number.clone().unwrap_or_else(|| "null".into());
            match bill_type
                .as_deref()
                .and_then(|t| map.iter().find(|(k, _)| *k == t))
            {
                Some((_, v)) => item.set("billNumber", format!("{v} {bn}")),
                None => item.set("billNumber", opt(bill_number)),
            }
            let usc = xp(d, mods, "m:extension/m:USCode")?;
            if let Some(&u) = usc.first() {
                item.set("code", "U.S.C.");
                item.set("codeVolume", opt(attr(d, u, "title")));
                let sections: Vec<String> = d
                    .query_selector_all(u.node(), "section")
                    .into_iter()
                    .map(|s| d.get_attribute(s, "number").unwrap_or("").to_owned())
                    .collect();
                item.set("codePages", sections.join(", "));
            }
            item.remove("accessionNumber");
        }
    }

    let mut part: Vec<XNode> = Vec::new();
    let mut origin_info: Vec<XNode> = Vec::new();
    for host in xp(d, mods, "m:relatedItem[@type=\"host\"]")? {
        if !item.truthy("publicationTitle") {
            item.set("publicationTitle", opt(process_title(d, host)?));
        }
        if !item.truthy("journalAbbreviation") {
            let ti = xp(d, host, "m:titleInfo[@type=\"abbreviated\"]")?;
            if let Some(&t) = ti.first() {
                item.set("journalAbbreviation", process_title_info(d, t)?);
            }
        }
        process_creators(d, host, &mut item, "contributor")?;
        process_identifiers(d, host, &mut item)?;
        part.extend(xp(d, host, "m:part")?);
        origin_info.extend(xp(d, host, "m:originInfo")?);
    }
    if !item.truthy("publicationTitle") {
        let ja = item
            .get("journalAbbreviation")
            .cloned()
            .unwrap_or(Value::Null);
        item.set("publicationTitle", ja);
    }

    for sn in xp(d, mods, ".//m:relatedItem[@type=\"series\"]")? {
        let series = opt(xpt(d, sn, "m:titleInfo/m:title")?);
        if field_is_valid_for_type("series", &item.item_type) {
            item.set("series", series);
        } else if field_is_valid_for_type("seriesTitle", &item.item_type) {
            item.set("seriesTitle", series);
        }
        if !item.truthy("seriesText") {
            item.set("seriesText", opt(xpt(d, sn, "m:titleInfo/m:subTitle")?));
        }
        if !item.truthy("seriesNumber") {
            item.set(
                "seriesNumber",
                opt(first_result(
                    d,
                    &[sn],
                    &[
                        "m:part/m:detail[@type=\"volume\"]/m:number",
                        "m:titleInfo/m:partNumber",
                    ],
                )?),
            );
        }
        process_creators(d, sn, &mut item, "seriesEditor")?;
    }

    part.extend(xp(d, mods, "m:part")?);
    origin_info.extend(xp(d, mods, "m:originInfo")?);

    if !part.is_empty() {
        for detail in ["volume", "issue", "section"] {
            let a = format!("m:detail[@type=\"{detail}\"]/m:number");
            let b = format!("m:detail[@type=\"{detail}\"]");
            item.set(detail, opt(first_result(d, &part, &[&a, &b])?));
        }
        for extent in xpath_all(d, &part, "m:extent", XNS)? {
            let unit = attr(d, extent, "unit");
            if matches!(unit.as_deref(), Some("pages") | Some("page")) {
                if item.truthy("pages") {
                    continue;
                }
                let start = xpt(d, extent, "m:start[1]")?;
                let end = xpt(d, extent, "m:end[1]")?;
                let st = start.as_deref().filter(|s| !s.is_empty());
                let en = end.as_deref().filter(|s| !s.is_empty());
                if st.is_some() || en.is_some() {
                    if start == end {
                        item.set("pages", opt(start));
                    } else if let (Some(s), Some(e)) = (st, en) {
                        item.set("pages", format!("{s}-{e}"));
                    } else {
                        // `pagesStart + pagesEnd`: null concatenates as "null".
                        let js = |v: &Option<String>| v.clone().unwrap_or_else(|| "null".into());
                        item.set("pages", format!("{}{}", js(&start), js(&end)));
                    }
                }
            } else {
                process_extent(&d.text(extent), &mut item);
            }
        }
        item.set(
            "date",
            opt(first_result(
                d,
                &part,
                &[
                    "m:date[not(@point=\"end\")][@encoding]",
                    "m:date[not(@point=\"end\")]",
                    "m:date",
                ],
            )?),
        );
    }

    for e in xp(d, mods, "m:physicalDescription/m:extent")? {
        process_extent(&d.text(e), &mut item);
    }

    process_identifiers(d, mods, &mut item)?;

    if !origin_info.is_empty() {
        let ed = xpath_all(d, &origin_info, "m:edition", XNS)?;
        if let Some(&e) = ed.first() {
            item.set("edition", d.text(e));
        }
        let pl = xpath_all(d, &origin_info, "m:place/m:placeTerm[@type=\"text\"]", XNS)?;
        if let Some(&p) = pl.first() {
            item.set("place", d.text(p));
        }
        let pb = xpath_all(d, &origin_info, "m:publisher", XNS)?;
        if let Some(&p) = pb.first() {
            item.set("publisher", d.text(p));
            if item.item_type == "webpage" && !item.truthy("publicationTitle") {
                let v = item.get("publisher").cloned().unwrap_or(Value::Null);
                item.set("publicationTitle", v);
            }
        }
        let date = first_result(
            d,
            &origin_info,
            &[
                "m:copyrightDate[@encoding]",
                "m:copyrightDate",
                "m:dateIssued[not(@point=\"end\")][@encoding]",
                "m:dateIssued[not(@point=\"end\")]",
                "m:dateIssued",
                "m:dateCreated[@encoding]",
                "m:dateCreated",
            ],
        )?
        .filter(|s| !s.is_empty());
        match date {
            Some(dt) => item.set("date", dt),
            None => {
                let v = item.get("date").cloned().unwrap_or(Value::Null);
                item.set("date", v);
            }
        }
        item.set(
            "lastModified",
            opt(first_result(
                d,
                &origin_info,
                &["m:dateModified[@encoding]", "m:dateModified"],
            )?),
        );
        item.set(
            "accessDate",
            opt(first_result(
                d,
                &origin_info,
                &[
                    "m:dateCaptured[@encoding]",
                    "m:dateCaptured[not(@encoding)]",
                ],
            )?),
        );
    }

    item.set("callNumber", opt(xpt(d, mods, "m:classification")?));
    item.set(
        "archiveLocation",
        opt(xpt_d(d, mods, ".//m:location/m:physicalLocation", "; ")?),
    );

    for u in xp(d, mods, "m:location/m:url")? {
        let access = attr(d, u, "access");
        let usage = attr(d, u, "usage");
        let text = d.text(u);
        if access.as_deref() == Some("raw object") {
            let mut a = JsObject::new();
            a.set(
                "title",
                attr(d, u, "displayLabel")
                    .filter(|s| !s.is_empty())
                    .unwrap_or_else(|| "Attachment".into()),
            );
            a.set("url", text.clone());
            let mime = if text.ends_with(".pdf") {
                Some("application/pdf")
            } else if text.ends_with(".html") || text.ends_with(".htm") {
                Some("text/html")
            } else if text.ends_with(".jpeg") || text.ends_with(".jpg") {
                Some("image/jpeg")
            } else if text.ends_with(".png") {
                Some("image/png")
            } else {
                None
            };
            if let Some(m) = mime {
                a.set("mimeType", m);
            }
            item.attachments.push(a);
        }
        if (!item.truthy("url")
            || usage.as_deref() == Some("primary")
            || usage.as_deref() == Some("primary display"))
            && access.as_deref() != Some("preview")
        {
            item.set("url", text);
        }
        if !item.truthy("accessDate") {
            item.set("accessDate", opt(attr(d, u, "dateLastAccessed")));
        }
    }

    item.set("abstractNote", opt(xpt_d(d, mods, "m:abstract", "\n\n")?));

    for n in xp(d, mods, "m:note")? {
        let prefix = match attr(d, n, "type") {
            Some(t) => format!("{t}: "),
            None => String::new(),
        };
        item.notes
            .push(TranslatorNote::new(format!("{prefix}{}", d.text(n))));
    }
    for n in xp(d, mods, "m:tableOfContents")? {
        item.notes.push(TranslatorNote::new(format!(
            "Table of Contents: {}",
            d.text(n)
        )));
    }
    for t in xp(d, mods, "m:subject/m:topic")? {
        item.tags
            .push(TranslatorTag::new(trim_internal(&d.text(t))));
    }

    if field_is_valid_for_type("scale", &item.item_type) {
        if let Some(scale) =
            xpt(d, mods, "m:subject/m:cartographics/m:scale")?.filter(|s| !s.is_empty())
        {
            static SC: OnceLock<Regex> = OnceLock::new();
            let ws = js::WS;
            let r = re(&SC, || format!("1{ws}*:{ws}*[0-9]+(?:,[0-9]+)"));
            if let Some(m) = r.find(&scale) {
                item.set("scale", m.as_str().to_owned());
            }
        }
    }

    let mut names: Vec<String> = Vec::new();
    let mut codes: Vec<String> = Vec::new();
    for ln in xp(d, mods, "m:language")? {
        let terms = xp(d, ln, "m:languageTerm")?;
        let n: NodeId = ln.node();
        if terms.is_empty() && d.children(n).len() == 1 {
            let f = d.children(n)[0];
            if d.node_type(XNode::Node(f)) == node_type::TEXT {
                codes.push(d.node_value(XNode::Node(f)).unwrap_or_default());
                continue;
            }
        }
        for t in terms {
            let tt = attr(d, t, "type");
            if tt.as_deref() == Some("text") {
                names.push(d.text(t));
            } else if tt.as_deref() == Some("code") || d.has_attribute(t.node(), "authority") {
                codes.push(d.text(t));
            }
        }
    }
    let langs = if codes.is_empty() { names } else { codes };
    item.set("language", langs.join("; "));
    Ok(item)
}
