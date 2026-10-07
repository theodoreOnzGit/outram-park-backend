// Part of the kovan Zotero port (GitHub #747, #749).
//
// Upstream: Zotero translators, https://github.com/zotero/translators
//   (commit 3d1c78530f42): "Bibliontology RDF.js" `Type` :363-372,
//   `getMatchScore` :379-405, `_scoreNodeRelationship` :411-437,
//   `getItemSeriesNodes` :442-472, `LiteralProperty.prototype.mapToItem`
//   :601-620, `CreatorProperty.prototype.mapToCreator` :653-687,
//   `mapToCreators` :692-707, `detectImport` :786-803, `doImport` :805-1060.
// Copyright (c) Simon Kornblith, Corporation for Digital Scholarship.
// Licence: AGPL-3.0. Upstream carries no licence text; treated as AGPLv3 as
//   part of Zotero, maintainer decision 2026-10-07, #747.

//! Bibliontology RDF import: every node with an `rdf:type` is scored
//! against the Zotero-to-BIBO type table; nodes that fit become items, read
//! through their user-item, container, subcontainer and series nodes.

use super::*;
use crate::zotero::framework::{ImportContext, TranslatorCreator, TranslatorItem, TranslatorTag};
use crate::zotero::translators::rdf_creator_types::server_creators_for_type;
use crate::zotero::translators::rdf_support::js_key_order;

/// Node slots; `None` is `null` / absent.
type Nodes = [Option<RdfValue>; 8];

/// A property mapping in `collapsedProperties`.
#[derive(Debug, Clone)]
enum Prop {
    /// `LiteralProperty(field)`.
    Literal(&'static str),
    /// `CreatorProperty(creatorType)`.
    Creator(&'static str),
}

fn res_of(n: &Option<RdfValue>) -> Res {
    match n {
        Some(v) => Res::from(v),
        None => Res::Null,
    }
}

/// `toString()` of a value from the store.
fn to_string(z: &RdfSandbox, v: &RdfValue) -> String {
    match v {
        RdfValue::Str(s) => s.clone(),
        RdfValue::Node(n) => z.store.term_to_string(&n.term),
    }
}

struct Import<'a> {
    z: RdfSandbox,
    ctx: &'a mut ImportContext,
}

impl Import<'_> {
    fn matches(&mut self, node: &RdfValue, p: &str, o: &str) -> bool {
        self.z
            .get_statements_matching(&Res::from(node), &Res::from(p), &Res::from(o), false)
            .is_some()
    }

    /// `_scoreNodeRelationship(node, definition, score)` (:411-437).
    fn score_relationship(
        &mut self,
        node: &Option<RdfValue>,
        def: Option<&ContainerDef>,
        mut score: i64,
    ) -> (i64, Option<RdfValue>) {
        let mut sub = None;
        if let Some(def) = def {
            let sts = self.z.get_statements_matching(
                &res_of(node),
                &Res::from(def.predicate.as_str()),
                &Res::Null,
                false,
            );
            if let Some(sts) = sts {
                let mut best = -9999i64;
                for st in sts {
                    let mut test = -(def.pairs.len() as i64);
                    for (p, o) in &def.pairs {
                        if self.matches(&st.2, p, o) {
                            test += 3;
                        }
                    }
                    if test > best {
                        sub = Some(st.2.clone());
                        best = test;
                    }
                }
                score += best;
            } else if def.always_add {
                score -= def.pairs.len() as i64;
            }
        }
        (score, sub)
    }

    /// `Type.prototype.getMatchScore(node)` (:379-405).
    fn match_score(&mut self, ty: &TypeDef, node: &RdfValue) -> (i64, Nodes) {
        let mut nodes: Nodes = Default::default();
        nodes[ITEM] = Some(node.clone());
        let mut score = -(ty.item.len() as i64);
        for (p, o) in &ty.item {
            if self.matches(node, p, o) {
                score += 3;
            }
        }
        let (s, sub) = self.score_relationship(&nodes[ITEM].clone(), ty.sub.as_ref(), score);
        score = s;
        nodes[SUBCONTAINER] = sub;
        let base = if nodes[SUBCONTAINER].is_some() {
            nodes[SUBCONTAINER].clone()
        } else {
            nodes[ITEM].clone()
        };
        let (s, c) = self.score_relationship(&base, ty.container.as_ref(), score);
        score = s;
        nodes[CONTAINER] = c;
        if !truthy(&nodes[CONTAINER]) {
            nodes[CONTAINER] = nodes[ITEM].clone();
        }
        if !truthy(&nodes[SUBCONTAINER]) {
            nodes[SUBCONTAINER] = nodes[CONTAINER].clone();
        }
        (score, nodes)
    }

    /// `Type.prototype.getItemSeriesNodes(nodes)` (:442-472).
    fn item_series_nodes(&mut self, nodes: &mut Nodes) {
        let series = ContainerDef {
            always_add: true,
            predicate: format!("{DCTERMS}isPartOf"),
            pairs: vec![(rdf_type(), format!("{BIBO}Series"))],
        };
        let st = self.z.get_statements_matching(
            &Res::Null,
            &Res::from(format!("{RES}resource").as_str()),
            &res_of(&nodes[ITEM]),
            false,
        );
        nodes[USERITEM] = match st {
            Some(s) => Some(RdfValue::Node(s[0].0.clone())),
            None => nodes[ITEM].clone(),
        };
        let (score, sub) = self.score_relationship(&nodes[ITEM].clone(), Some(&series), 0);
        if score >= 1 {
            nodes[ITEM_SERIES] = sub;
        }
        let (score, sub) = self.score_relationship(&nodes[SUBCONTAINER].clone(), Some(&series), 0);
        if score >= 1 {
            nodes[CONTAINER_SERIES] = sub;
        }
        let (score, sub) = self.score_relationship(&nodes[CONTAINER].clone(), Some(&series), 0);
        if score >= 1 {
            nodes[CONTAINER_SERIES] = sub;
        }
    }

    /// `LiteralProperty.prototype.mapToItem(newItem, nodes)` (:601-620).
    fn map_to_item(
        &mut self,
        field: &str,
        item: &mut TranslatorItem,
        nodes: &Nodes,
    ) -> Result<(), TranslateError> {
        match field_mapping(field) {
            FieldMap::Isbn => {
                let mut isbns = Vec::new();
                for p in [format!("{BIBO}isbn13"), format!("{BIBO}isbn10")] {
                    if let Some(sts) = self.z.get_statements_matching(
                        &res_of(&nodes[CONTAINER]),
                        &Res::from(p.as_str()),
                        &Res::Null,
                        false,
                    ) {
                        for s in sts {
                            isbns.push(to_string(&self.z, &s.2));
                        }
                    }
                }
                // `if (!isbns.length) return false; return isbns.join(", ")`,
                // then `if (!content) return false`.
                let joined = isbns.join(", ");
                if !joined.is_empty() {
                    item.set(field, joined);
                }
            }
            FieldMap::PriorityNumbers => {
                if let Some(sts) = self.z.get_statements_matching(
                    &res_of(&nodes[ITEM]),
                    &Res::from(format!("{Z}priorityNumber").as_str()),
                    &Res::Null,
                    false,
                ) {
                    let v: Vec<String> = sts.iter().map(|s| to_string(&self.z, &s.2)).collect();
                    let joined = v.join(", ");
                    if !joined.is_empty() {
                        item.set(field, joined);
                    }
                }
            }
            FieldMap::Plain(domain, pred) => {
                let Some(node) = nodes[domain].clone().filter(|n| truthy(&Some(n.clone()))) else {
                    return Ok(());
                };
                let Some(sts) = statements_by_definition(&mut self.z, &pred, &node)? else {
                    return Ok(());
                };
                let content: Vec<String> = sts.iter().map(|s| to_string(&self.z, &s.2)).collect();
                item.set(field, content.join(","));
            }
        }
        Ok(())
    }

    /// `CreatorProperty.prototype.mapToCreator(creatorNode, zoteroType)` (:653-687).
    fn map_to_creator(
        &mut self,
        field: &str,
        creator_node: &RdfValue,
        zotero_type: &str,
    ) -> Result<Option<TranslatorCreator>, TranslateError> {
        let r = Res::from(creator_node);
        let mut c = TranslatorCreator::default();
        if let Some(l) = self.z.get_statements_matching(
            &r,
            &Res::from(format!("{FOAF}surname").as_str()),
            &Res::Null,
            false,
        ) {
            c.last_name = Some(to_string(&self.z, &l[0].2));
            let f = self
                .z
                .get_statements_matching(
                    &r,
                    &Res::from(format!("{FOAF}givenname").as_str()),
                    &Res::Null,
                    false,
                )
                .or_else(|| {
                    self.z.get_statements_matching(
                        &r,
                        &Res::from(format!("{FOAF}givenName").as_str()),
                        &Res::Null,
                        false,
                    )
                });
            if let Some(f) = f {
                c.first_name = Some(to_string(&self.z, &f[0].2));
            }
        } else if let Some(n) = self.z.get_statements_matching(
            &r,
            &Res::from(format!("{FOAF}name").as_str()),
            &Res::Null,
            false,
        ) {
            c.last_name = Some(to_string(&self.z, &n[0].2));
            c.field_mode = Some(1);
        } else {
            return Ok(None);
        }
        // `getStatementsMatching(node, foaf:birthday, null, true)`: an array
        // of statements, of which `[2]` is the third statement (upstream's
        // `toString()` of it, or a TypeError when there are fewer than three).
        for (p, k) in [("birthday", "birthYear"), ("nick", "shortName")] {
            if let Some(sts) = self.z.get_statements_matching(
                &r,
                &Res::from(format!("{FOAF}{p}").as_str()),
                &Res::Null,
                true,
            ) {
                let Some(st) = sts.get(2) else {
                    return Err(err(
                        "TypeError: Cannot read properties of undefined (reading 'toString')",
                    ));
                };
                let s = format!(
                    "{},{},{}",
                    self.z.store.term_to_string(&st.0.term),
                    self.z.store.term_to_string(&st.1.term),
                    to_string(&self.z, &st.2)
                );
                c.other.set(k, s);
            }
        }
        if field == "author" {
            let for_type = server_creators_for_type(zotero_type);
            if !for_type.contains(&"author") {
                c.creator_type = for_type.first().map(|s| s.to_string());
            }
        } else {
            c.creator_type = Some(field.to_owned());
        }
        Ok(Some(c))
    }

    /// `CreatorProperty.prototype.mapToCreators(node, zoteroType)` (:692-707).
    fn map_to_creators(
        &mut self,
        field: &str,
        node: &RdfValue,
        zotero_type: &str,
    ) -> Result<Vec<(TranslatorCreator, RdfValue)>, TranslateError> {
        let mapping = creator_mapping(field).expect("CREATORS entry");
        let mut out = Vec::new();
        if let Some(sts) = statements_by_definition(&mut self.z, &mapping.pred, node)? {
            for st in sts {
                if let Some(c) = self.map_to_creator(field, &st.2, zotero_type)? {
                    out.push((c, st.2.clone()));
                }
            }
        }
        Ok(out)
    }

    fn uri(&mut self, v: &RdfValue) -> String {
        self.z.get_resource_uri_string(v)
    }
}

fn truthy(n: &Option<RdfValue>) -> bool {
    match n {
        None => false,
        Some(RdfValue::Str(s)) => !s.is_empty(),
        Some(RdfValue::Node(_)) => true,
    }
}

/// `detectImport()` (:786-803): some `rdf:type` points at a BIBO class.
pub fn detect_import(ctx: &mut ImportContext) -> bool {
    let Ok(mut z) = RdfSandbox::from_xml(&ctx.input.text(), ctx.options.env.first_blank_node_id)
    else {
        return false;
    };
    let Some(types) = z.get_statements_matching(
        &Res::Null,
        &Res::from(rdf_type().as_str()),
        &Res::Null,
        false,
    ) else {
        return false;
    };
    for t in types {
        if let RdfValue::Node(_) = &t.2 {
            let u = z.get_resource_uri_string(&t.2);
            if u.starts_with(BIBO) {
                return true;
            }
        }
    }
    false
}

/// `doImport()` (:805-1060).
pub fn do_import(ctx: &mut ImportContext) -> Result<(), TranslateError> {
    let z = RdfSandbox::from_xml(&ctx.input.text(), ctx.options.env.first_blank_node_id)
        .map_err(err)?;
    let mut im = Import { z, ctx };

    // collapsedTypes: type URI -> types, BIBO_TYPES first (keyed by
    // bibo:X, though their pair is written with the namespace twice).
    let mut collapsed_types: Vec<(String, Vec<TypeDef>)> = Vec::new();
    let push_type = |k: String, t: TypeDef, ct: &mut Vec<(String, Vec<TypeDef>)>| match ct
        .iter_mut()
        .find(|(x, _)| *x == k)
    {
        Some((_, v)) => v.push(t),
        None => ct.push((k, vec![t])),
    };
    for (un, zt) in BIBO_TYPES {
        let bibo_type = format!("{BIBO}{un}");
        let t = TypeDef {
            zotero_type: zt.to_owned(),
            item: vec![(rdf_type(), format!("{BIBO}{bibo_type}"))],
            sub: None,
            container: None,
        };
        push_type(bibo_type, t, &mut collapsed_types);
    }
    for t in types() {
        for (_, o) in t.item.clone() {
            push_type(o, t.clone(), &mut collapsed_types);
        }
    }

    // collapsedProperties[domain][predicate]
    let mut collapsed: Vec<Vec<(String, Vec<Prop>)>> = vec![Vec::new(); 8];
    let mut function_props: Vec<&'static str> = Vec::new();
    for (f, m) in fields() {
        match m {
            FieldMap::Isbn | FieldMap::PriorityNumbers => function_props.push(f),
            FieldMap::Plain(d, p) => {
                let k = p.key().to_owned();
                match collapsed[d].iter_mut().find(|(x, _)| *x == k) {
                    Some((_, v)) => v.push(Prop::Literal(f)),
                    None => collapsed[d].push((k, vec![Prop::Literal(f)])),
                }
            }
        }
    }
    for (ct, m) in creators() {
        let k = m.pred.key().to_owned();
        match collapsed[m.domain].iter_mut().find(|(x, _)| *x == k) {
            Some((_, v)) => v.insert(0, Prop::Creator(ct)),
            None => collapsed[m.domain].push((k, vec![Prop::Creator(ct)])),
        }
    }

    // Every node with an object-valued rdf:type, keyed by URI.
    let rdf_types =
        im.z.get_statements_matching(
            &Res::Null,
            &Res::from(rdf_type().as_str()),
            &Res::Null,
            false,
        )
        .unwrap_or_default();
    let mut item_nodes: Vec<(String, RdfValue)> = Vec::new();
    for t in rdf_types {
        if !matches!(t.2, RdfValue::Node(_)) {
            continue;
        }
        let node = RdfValue::Node(t.0.clone());
        let uri = im.uri(&node);
        if uri.is_empty() {
            continue;
        }
        match item_nodes.iter_mut().find(|(u, _)| *u == uri) {
            Some((_, n)) => *n = node,
            None => item_nodes.push((uri, node)),
        }
    }
    let order: Vec<String> = js_key_order(item_nodes.iter().map(|(k, _)| k.as_str()))
        .into_iter()
        .map(str::to_owned)
        .collect();
    let same_rel = same_item_relations();

    for key in order {
        let item_node = item_nodes
            .iter()
            .find(|(k, _)| *k == key)
            .map(|(_, n)| n.clone())
            .expect("key");
        if let Some(arcs) = im.z.get_arcs_in(Res::from(&item_node)) {
            if arcs.iter().any(|a| same_rel.contains(a)) {
                continue;
            }
        }
        let item_types =
            im.z.get_statements_matching(
                &Res::from(&item_node),
                &Res::from(rdf_type().as_str()),
                &Res::Null,
                false,
            )
            .unwrap_or_default();
        let mut best_score = -9999i64;
        let mut best: Option<(TypeDef, Nodes)> = None;
        for t in item_types {
            if !matches!(t.2, RdfValue::Node(_)) {
                continue;
            }
            let u = im.uri(&t.2);
            let Some(cands) = collapsed_types
                .iter()
                .find(|(k, _)| *k == u)
                .map(|(_, v)| v.clone())
            else {
                continue;
            };
            for ty in cands {
                let (score, nodes) = im.match_score(&ty, &item_node);
                if score > best_score {
                    best_score = score;
                    best = Some((ty, nodes));
                }
            }
        }
        if best_score < 1 {
            continue;
        }
        let (best_type, mut nodes) = best.expect("scored");
        im.item_series_nodes(&mut nodes);
        let zotero_type = best_type.zotero_type.clone();
        let mut item = TranslatorItem::new(zotero_type.clone());

        // `for (var i in nodes)`: the slots that exist, in numeric order.
        let present: Vec<usize> = (1..8).filter(|&i| nodes[i].is_some()).collect();
        let mut all_creators: Vec<(String, TranslatorCreator)> = Vec::new();
        for &i in &present {
            let mut handled: Vec<String> = Vec::new();
            let props = im.z.get_arcs_out(res_of(&nodes[i])).unwrap_or_default();
            for property in props {
                if handled.contains(&property) {
                    continue;
                }
                handled.push(property.clone());
                let Some(mappings) = collapsed[i]
                    .iter()
                    .find(|(k, _)| *k == property)
                    .map(|(_, v)| v.clone())
                else {
                    continue;
                };
                for m in mappings {
                    match m {
                        Prop::Literal(f) => im.map_to_item(f, &mut item, &nodes)?,
                        Prop::Creator(f) => {
                            let node = nodes[i].clone().expect("present");
                            for (c, cn) in im.map_to_creators(f, &node, &zotero_type)? {
                                let u = im.uri(&cn);
                                if !all_creators.iter().any(|(k, _)| *k == u) {
                                    all_creators.push((u, c));
                                }
                            }
                        }
                    }
                }
            }
        }
        for f in &function_props {
            im.map_to_item(f, &mut item, &nodes)?;
        }

        let mut creator_lists: Vec<String> = Vec::new();
        let mut creators_added: Vec<String> = Vec::new();
        for &i in &present {
            let subj = res_of(&nodes[i]);
            for st in
                im.z.get_statements_matching(
                    &subj,
                    &Res::from(format!("{DCTERMS}subject").as_str()),
                    &Res::Null,
                    false,
                )
                .unwrap_or_default()
            {
                item.tags.push(TranslatorTag {
                    tag: to_string(&im.z, &st.2),
                    tag_type: Some(if i == USERITEM { 0 } else { 1 }),
                });
            }
            for st in
                im.z.get_statements_matching(
                    &subj,
                    &Res::from(format!("{CTAG}tagged").as_str()),
                    &Res::Null,
                    false,
                )
                .unwrap_or_default()
            {
                let mut ty = 0;
                if let Some(types) = im.z.get_statements_matching(
                    &Res::from(&st.2),
                    &Res::from(rdf_type().as_str()),
                    &Res::Null,
                    false,
                ) {
                    let u = im.uri(&types[0].2);
                    if u == format!("{CTAG}AutoTag") || u == format!("{CTAG}AuthorTag") {
                        ty = 1;
                    }
                }
                if let Some(labels) = im.z.get_statements_matching(
                    &Res::from(&st.2),
                    &Res::from(format!("{CTAG}label").as_str()),
                    &Res::Null,
                    false,
                ) {
                    item.tags.push(TranslatorTag {
                        tag: to_string(&im.z, &labels[0].2),
                        tag_type: Some(ty),
                    });
                }
            }
            for list in [AUTHOR_LIST, EDITOR_LIST, CONTRIBUTOR_LIST] {
                let sts =
                    im.z.get_statements_matching(
                        &subj,
                        &Res::from(creator_list(list).as_str()),
                        &Res::Null,
                        false,
                    )
                    .unwrap_or_default();
                for st in sts {
                    let list_uri = im.uri(&st.2);
                    if creator_lists.contains(&list_uri) {
                        continue;
                    }
                    creator_lists.push(list_uri);
                    let members = im.z.get_container_elements(Res::from(&st.2));
                    for m in members {
                        let Some(m) = m else {
                            return Err(err(
                                "TypeError: Cannot read properties of undefined (reading 'uri')",
                            ));
                        };
                        let u = im.uri(&m);
                        if creators_added.contains(&u) {
                            continue;
                        }
                        creators_added.push(u.clone());
                        if let Some((_, c)) = all_creators.iter().find(|(k, _)| *k == u) {
                            item.creators.push(c.clone());
                        } else {
                            let field = match list {
                                AUTHOR_LIST => "author",
                                EDITOR_LIST => "editor",
                                _ => "contributor",
                            };
                            if let Some(c) = im.map_to_creator(field, &m, &zotero_type)? {
                                item.creators.push(c);
                            }
                        }
                    }
                }
            }
        }
        // `for (var creatorNodeURI in allCreators)`: object key order.
        let keys: Vec<String> = js_key_order(all_creators.iter().map(|(k, _)| k.as_str()))
            .into_iter()
            .map(str::to_owned)
            .collect();
        for k in keys {
            if !creators_added.contains(&k) {
                let c = all_creators
                    .iter()
                    .find(|(x, _)| *x == k)
                    .map(|(_, c)| c.clone())
                    .expect("key");
                item.creators.push(c);
            }
        }
        im.ctx.item_done(item);
    }
    Ok(())
}
