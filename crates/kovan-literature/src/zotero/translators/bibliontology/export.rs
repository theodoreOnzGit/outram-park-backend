// Part of the kovan Zotero port (GitHub #747, #749).
//
// Upstream: Zotero translators, https://github.com/zotero/translators
//   (commit 3d1c78530f42): "Bibliontology RDF.js" `Type` :363-372,
//   `Type.prototype.addNodeRelations` :478-526, `createNodes` :532-584,
//   `LiteralProperty.prototype.mapFromItem` :625-640,
//   `CreatorProperty.prototype.mapFromCreator` :712-766, `doExport`
//   :1064-1130.
// Copyright (c) Simon Kornblith, Corporation for Digital Scholarship.
// Licence: AGPL-3.0. Upstream carries no licence text; treated as AGPLv3 as
//   part of Zotero, maintainer decision 2026-10-07, #747.

//! Bibliontology RDF export: each item as a `z:UserItem` pointing at the
//! work (`res:resource`), with its container, subcontainer and series nodes,
//! creators in `rdf:Seq` lists and tags as Common Tag nodes.

use super::*;
use crate::zotero::framework::js;
use crate::zotero::framework::rdf::Node;
use crate::zotero::framework::{ExportContext, JsObject, TranslatorCreator, TranslatorItem};
use crate::zotero::translators::rdf_creator_types::server_creators_for_type;
use crate::zotero::translators::rdf_support::{encode_uri, item_unique_fields_order, js_key};
use serde_json::Value;
use std::collections::HashSet;

/// The eight node slots (`nodes[1..=7]`; index 0 unused).
type Nodes = [Option<RdfValue>; 8];

fn node_res(n: &Option<RdfValue>) -> Res {
    match n {
        Some(v) => Res::from(v),
        None => Res::Null,
    }
}

/// `/, ?| /g` split.
fn split_list(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            ',' => {
                if chars.peek() == Some(&' ') {
                    chars.next();
                }
                out.push(std::mem::take(&mut cur));
            }
            ' ' => out.push(std::mem::take(&mut cur)),
            c => cur.push(c),
        }
    }
    out.push(cur);
    out
}

struct Export {
    z: RdfSandbox,
    used_uris: HashSet<String>,
}

impl Export {
    fn uri_of(&mut self, n: &RdfValue) -> String {
        self.z.get_resource_uri_string(n)
    }

    /// `Type.prototype.createNodes(item)` (:532-584).
    fn create_nodes(
        &mut self,
        ty: &TypeDef,
        item: &TranslatorItem,
    ) -> Result<Nodes, TranslateError> {
        let mut nodes: Nodes = Default::default();
        let user_item = if item.truthy("uri") {
            js_key(item.get("uri"))
        } else {
            format!("#item_{}", js_key(item.get("itemID")))
        };
        nodes[USERITEM] = Some(RdfValue::Str(user_item));
        let mut item_node: Option<String> = None;
        if item.truthy("url") {
            let u = encode_uri(&js_key(item.get("url")));
            item_node = (!self.used_uris.contains(&u)).then_some(u);
        }
        if item_node.is_none() && item.truthy("DOI") {
            let doi = js_key(item.get("DOI"));
            let doi = ["doi:", "urn:doi:", "info:doi/", "http://dx.doi.org/"]
                .iter()
                .find_map(|p| doi.strip_prefix(p))
                .unwrap_or(&doi)
                .to_owned();
            let u = format!("info:doi/{}", encode_uri(&doi));
            item_node = (!self.used_uris.contains(&u)).then_some(u);
        }
        if item_node.is_none() && item.truthy("ISBN") {
            let isbn = split_list(&js_key(item.get("ISBN")))[0].clone();
            let u = format!("urn:isbn:{}", encode_uri(&isbn));
            item_node = (!self.used_uris.contains(&u)).then_some(u);
        }
        let item_v = match item_node {
            Some(u) => RdfValue::Str(u),
            None => RdfValue::Node(self.z.new_resource()),
        };
        let u = self.uri_of(&item_v);
        self.used_uris.insert(u);
        nodes[ITEM] = Some(item_v.clone());
        let ui = nodes[USERITEM].clone().expect("set");
        self.z
            .add_statement(Res::from(&ui), rdf_type().as_str(), format!("{Z}UserItem"))
            .map_err(err)?;
        self.z
            .add_statement(Res::from(&ui), format!("{RES}resource"), Res::from(&item_v))
            .map_err(err)?;
        nodes[CONTAINER] = Some(if ty.container.is_some() {
            RdfValue::Node(self.z.new_resource())
        } else {
            item_v.clone()
        });
        nodes[SUBCONTAINER] = if ty.sub.is_some() {
            Some(RdfValue::Node(self.z.new_resource()))
        } else {
            nodes[CONTAINER].clone()
        };
        nodes[ITEM_SERIES] = Some(RdfValue::Node(self.z.new_resource()));
        nodes[CONTAINER_SERIES] = if ty.container.is_some() {
            Some(RdfValue::Node(self.z.new_resource()))
        } else {
            nodes[ITEM_SERIES].clone()
        };
        nodes[SUBCONTAINER_SERIES] = if ty.sub.is_some() {
            Some(RdfValue::Node(self.z.new_resource()))
        } else {
            nodes[CONTAINER_SERIES].clone()
        };
        Ok(nodes)
    }

    /// `LiteralProperty.prototype.mapFromItem(item, nodes)` (:625-640).
    fn map_from_item(
        &mut self,
        field: &str,
        item: &TranslatorItem,
        unique: &serde_json::Map<String, Value>,
        nodes: &Nodes,
    ) -> Result<(), TranslateError> {
        match field_mapping(field) {
            FieldMap::Isbn | FieldMap::PriorityNumbers => {
                let (key, domain, f): (&str, usize, fn(&str) -> String) =
                    if matches!(field_mapping(field), FieldMap::Isbn) {
                        ("ISBN", CONTAINER, |isbn: &str| {
                            if isbn.encode_utf16().count() == 10 {
                                format!("{BIBO}isbn10")
                            } else {
                                format!("{BIBO}isbn13")
                            }
                        })
                    } else {
                        ("priorityNumbers", ITEM, |_: &str| {
                            format!("{Z}priorityNumber")
                        })
                    };
                let Some(v) = item.get(key) else {
                    return Err(err(format!(
                        "TypeError: Cannot read properties of undefined (reading 'split') ({key})"
                    )));
                };
                for part in split_list(&js::to_js_string(v)) {
                    let p = f(&part);
                    self.z
                        .add_literal(node_res(&nodes[domain]), p, &part)
                        .map_err(err)?;
                }
            }
            FieldMap::Plain(domain, Pred::Simple(p)) => {
                let v = literal_of(unique.get(field))?;
                self.z
                    .add_literal(node_res(&nodes[domain]), p, &v)
                    .map_err(err)?;
            }
            FieldMap::Plain(domain, Pred::Complex(p0, pairs, p2)) => {
                let attach = nodes[domain]
                    .clone()
                    .ok_or_else(|| err("about must be defined in Zotero.RDF.addStatement"))?;
                let b = get_blank_node(&mut self.z, &attach, &p0, &pairs, true)?.expect("created");
                let v = literal_of(unique.get(field))?;
                self.z.add_literal(Res::from(&b), p2, &v).map_err(err)?;
            }
        }
        Ok(())
    }

    /// `CreatorProperty.prototype.mapFromCreator(item, creator, nodes)` (:712-766).
    fn map_from_creator(
        &mut self,
        item_type: &str,
        creator: &TranslatorCreator,
        nodes: &Nodes,
    ) -> Result<(), TranslateError> {
        let field = creator
            .creator_type
            .clone()
            .unwrap_or_else(|| "undefined".to_owned());
        let for_type = server_creators_for_type(item_type);
        let is_primary = for_type.first().copied() == Some(field.as_str());
        let mapping = match creator_mapping(&field) {
            Some(m) => m,
            None => {
                if is_primary && !for_type.contains(&"author") {
                    creator_mapping("author").expect("author")
                } else {
                    CreatorMap {
                        domain: ITEM,
                        list: AUTHOR_LIST,
                        pred: Pred::Simple(format!("{Z}{field}")),
                    }
                }
            }
        };
        let cn: Node = self.z.new_resource();
        let truthy = |s: &Option<String>| s.as_deref().is_some_and(|x| !x.is_empty());
        if creator.field_mode == Some(1) {
            self.z
                .add_statement(
                    cn.clone(),
                    rdf_type().as_str(),
                    format!("{FOAF}Organization"),
                )
                .map_err(err)?;
            if truthy(&creator.last_name) {
                self.z
                    .add_literal(
                        cn.clone(),
                        format!("{FOAF}name"),
                        creator.last_name.as_deref().unwrap_or(""),
                    )
                    .map_err(err)?;
            }
        } else {
            self.z
                .add_statement(cn.clone(), rdf_type().as_str(), format!("{FOAF}Person"))
                .map_err(err)?;
            if truthy(&creator.first_name) {
                self.z
                    .add_literal(
                        cn.clone(),
                        format!("{FOAF}givenName"),
                        creator.first_name.as_deref().unwrap_or(""),
                    )
                    .map_err(err)?;
            }
            if truthy(&creator.last_name) {
                self.z
                    .add_literal(
                        cn.clone(),
                        format!("{FOAF}surname"),
                        creator.last_name.as_deref().unwrap_or(""),
                    )
                    .map_err(err)?;
            }
        }
        for (k, p) in [("birthYear", "birthday"), ("shortName", "nick")] {
            if let Some(v) = creator.other.get(k).filter(|v| js::truthy(Some(v))) {
                self.z
                    .add_literal(cn.clone(), format!("{FOAF}{p}"), &js::to_js_string(v))
                    .map_err(err)?;
            }
        }
        let base = nodes[mapping.domain].clone();
        let (attach_to, relation) = match &mapping.pred {
            Pred::Simple(p) => (base.clone(), p.clone()),
            Pred::Complex(p0, pairs, p2) => {
                let a = base
                    .clone()
                    .ok_or_else(|| err("about must be defined in Zotero.RDF.addStatement"))?;
                (
                    get_blank_node(&mut self.z, &a, p0, pairs, true)?,
                    p2.clone(),
                )
            }
        };
        self.z
            .add_statement(node_res(&attach_to), relation.as_str(), cn.clone())
            .map_err(err)?;
        let mut list = mapping.list;
        if list == CONTRIBUTOR_LIST && is_primary {
            list = AUTHOR_LIST;
        }
        let existing = self.z.get_statements_matching(
            &node_res(&base),
            &Res::from(creator_list(list).as_str()),
            &Res::Null,
            false,
        );
        let seq: RdfValue = match existing {
            Some(s) => s[0].2.clone(),
            None => {
                let r = self.z.new_resource();
                self.z.new_container("seq", r.clone()).map_err(err)?;
                self.z
                    .add_statement(node_res(&base), creator_list(list).as_str(), r.clone())
                    .map_err(err)?;
                RdfValue::Node(r)
            }
        };
        self.z
            .add_container_element(Res::from(&seq), cn)
            .map_err(err)?;
        Ok(())
    }

    /// `Type.prototype.addNodeRelations(nodes)` (:478-526).
    fn add_node_relations(
        &mut self,
        ty: &TypeDef,
        nodes: &mut Nodes,
    ) -> Result<(), TranslateError> {
        let has = |i: usize| match i {
            ITEM => true,
            SUBCONTAINER => ty.sub.is_some(),
            CONTAINER => ty.container.is_some(),
            _ => false,
        };
        for i in [ITEM_SERIES, SUBCONTAINER_SERIES, CONTAINER_SERIES] {
            if !has(i - 3) {
                continue;
            }
            if self.z.get_arcs_out(node_res(&nodes[i])).is_none() {
                continue;
            }
            self.z
                .add_statement(
                    node_res(&nodes[i]),
                    rdf_type().as_str(),
                    format!("{BIBO}Series"),
                )
                .map_err(err)?;
            self.z
                .add_statement(
                    node_res(&nodes[i - 3]),
                    format!("{DCTERMS}isPartOf"),
                    node_res(&nodes[i]),
                )
                .map_err(err)?;
        }
        for i in [ITEM, SUBCONTAINER, CONTAINER] {
            if nodes[i].is_none() {
                continue;
            }
            let (pairs, predicate): (Vec<Pair>, String) = match i {
                ITEM => (ty.item.clone(), format!("{RES}resource")),
                _ => {
                    let def = if i == SUBCONTAINER {
                        &ty.sub
                    } else {
                        &ty.container
                    };
                    let Some(def) = def else { continue };
                    // `!this[i][0]`: the definition object has no "0", so
                    // only the arcs decide (`alwaysAdd` is never read here).
                    if self.z.get_arcs_out(node_res(&nodes[i])).is_none() {
                        nodes[i] = nodes[i - 1].clone();
                        continue;
                    }
                    (def.pairs.clone(), def.predicate.clone())
                }
            };
            for (p, o) in &pairs {
                self.z
                    .add_statement(node_res(&nodes[i]), p.as_str(), o.as_str())
                    .map_err(err)?;
            }
            let mut j = i - 1;
            while j > 1 {
                let a = nodes[j].clone().map(|n| self.uri_of(&n));
                let b = nodes[i].clone().map(|n| self.uri_of(&n));
                if a != b {
                    self.z
                        .add_statement(node_res(&nodes[j]), predicate.as_str(), node_res(&nodes[i]))
                        .map_err(err)?;
                    break;
                }
                j -= 1;
            }
        }
        Ok(())
    }
}

/// A uniqueFields value as a literal (`undefined`/`null` are errors in
/// `addStatement`).
fn literal_of(v: Option<&Value>) -> Result<String, TranslateError> {
    match v {
        None | Some(Value::Null) => Err(err("value must be defined in Zotero.RDF.addStatement")),
        Some(v) => Ok(js::to_js_string(v)),
    }
}

/// `doExport()` (:1066-1130).
///
/// `originals` are the export input objects the context's items were made
/// from, in order (for the order of `uniqueFields`, as in Zotero RDF).
pub fn do_export(ctx: &mut ExportContext, originals: &[JsObject]) -> Result<(), TranslateError> {
    let mut ex = Export {
        z: RdfSandbox::new(ctx.options.env.first_blank_node_id),
        used_uris: HashSet::new(),
    };
    for (p, u) in NAMESPACES {
        ex.z.add_namespace(p, u);
    }
    // `items[item.itemID] = item`, then `for (var h in items)`: itemIDs are
    // 1, 2, ... so the order is the order of nextItem().
    let mut items: Vec<(String, (TranslatorItem, Option<JsObject>))> = Vec::new();
    let mut n = 0;
    while let Some(item) = ctx.next_item() {
        let original = originals.get(n).cloned();
        n += 1;
        if item.item_type == "note" {
            continue;
        }
        let id = js_key(item.get("itemID"));
        match items.iter_mut().find(|(k, _)| *k == id) {
            Some((_, it)) => *it = (item, original),
            None => items.push((id, (item, original))),
        }
    }
    let order = super::super::rdf_support::js_key_order(items.iter().map(|(k, _)| k.as_str()));
    let order: Vec<String> = order.into_iter().map(str::to_owned).collect();
    let types = types();
    let mut user_tags: Vec<(String, Node)> = Vec::new();
    let mut auto_tags: Vec<(String, Node)> = Vec::new();
    for key in order {
        let (item, original) = items
            .iter()
            .find(|(k, _)| *k == key)
            .map(|(_, i)| i.clone())
            .expect("key");
        let Some(ty) = types
            .iter()
            .find(|t| t.zotero_type == item.item_type)
            .cloned()
        else {
            // `new Type(item.itemType, TYPES[item.itemType])` with no entry.
            return Err(err(
                "TypeError: Cannot read properties of undefined (reading '0')",
            ));
        };
        let mut nodes = ex.create_nodes(&ty, &item)?;
        let unique = match item.get("uniqueFields") {
            Some(Value::Object(m)) => m.clone(),
            _ => serde_json::Map::new(),
        };
        for field in item_unique_fields_order(&item, original.as_ref()) {
            let v = unique.get(&field);
            if !js::truthy(v) && !matches!(v, Some(Value::Number(n)) if n.as_f64() == Some(0.0)) {
                continue;
            }
            ex.map_from_item(&field, &item, &unique, &nodes)?;
        }
        for c in &item.creators {
            ex.map_from_creator(&item.item_type, c, &nodes)?;
        }
        for tag in &item.tags {
            // `tag.type == 0`
            let user = tag.tag_type.unwrap_or(0) == 0 && tag.tag_type.is_some();
            let coll = if user { &mut user_tags } else { &mut auto_tags };
            let node = match coll.iter().find(|(t, _)| *t == tag.tag) {
                Some((_, n)) => n.clone(),
                None => {
                    let n = ex.z.new_resource();
                    let class = if user { "UserTag" } else { "AutoTag" };
                    ex.z.add_statement(n.clone(), format!("{RDF}type"), format!("{CTAG}{class}"))
                        .map_err(err)?;
                    ex.z.add_literal(n.clone(), format!("{CTAG}label"), &tag.tag)
                        .map_err(err)?;
                    let coll = if user { &mut user_tags } else { &mut auto_tags };
                    coll.push((tag.tag.clone(), n.clone()));
                    n
                }
            };
            ex.z.add_statement(node_res(&nodes[USERITEM]), format!("{CTAG}tagged"), node)
                .map_err(err)?;
        }
        ex.add_node_relations(&ty, &mut nodes)?;
    }
    let out = ex.z.serialize().map_err(err)?;
    ctx.write(&out);
    Ok(())
}
