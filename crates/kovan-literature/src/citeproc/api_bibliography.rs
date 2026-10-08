// Part of the kovan port of citeproc-js (GitHub #790).
//
// Upstream:    citeproc-js, https://github.com/juris-m/citeproc-js
// Source:      src/api_bibliography.js
// Version:     2.4.63, commit 73bc1b44bc7d54d0bfec4e070fd27f5efe024ff9
// Copyright:   (c) 2009-2019 Frank Bennett
// Licence:     AGPL-3.0, taken from upstream's "CPAL-1.0 or AGPL-3.0-or-later"
//              (LICENSE at the commit above; see this crate's NOTICE).
// Modified:    2026-10-08, by the OUTRAM PARK contributors. This file is a
//              Rust translation (port) of the files named above, modified
//              from the original.
// No warranty: this program is distributed in the hope that it will be
//              useful, but WITHOUT ANY WARRANTY; without even the implied
//              warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR
//              PURPOSE. See the GNU Affero General Public License.

//! The bibliography API: `makeBibliography` and `CSL.getBibliographyEntries`.

use std::collections::BTreeSet;

use serde_json::Value;

use super::formats;
use super::js::{self, Obj};
use super::obj_blob::{BlobChild, BlobContent};
use super::obj_token::{Decoration, Token, TokenType};
use super::queue::{self, QueueId, Rendered, StringParent};
use super::state::State;
use super::util_parallel;
use super::{CslResult, EngineError};

/// What `makeBibliography` returns: `[params, entry_strings]`.
#[derive(Debug, Clone, PartialEq)]
pub struct BibliographyResult {
    /// The parameter object: `maxoffset`, `entryspacing`, `linespacing`,
    /// `second-field-align`, `entry_ids`, `bibliography_errors`, `done`,
    /// `hangingindent` (when the style has it), `bibstart`, `bibend`.
    pub params: Obj,
    /// The formatted entries.
    pub entry_strings: Vec<String>,
}

fn type_error(what: &str) -> EngineError {
    EngineError::Csl(format!("TypeError: {what}"))
}

/// `eval_spec(a, b)` of `getBibliographyEntries`: does the item field value
/// `b` satisfy the filter value `a`?
fn eval_spec(a: &Value, b: Option<&Value>) -> bool {
    let b_truthy = js::truthy_opt(b);
    if matches!(a, Value::Bool(_)) || !js::truthy(a) {
        if js::truthy(a) {
            b_truthy
        } else {
            !b_truthy
        }
    } else {
        match b {
            // eval_string: a === b
            Some(Value::String(_)) => b == Some(a),
            _ if !b_truthy => false,
            // eval_list: a === lst[i] for i < lst.length
            Some(Value::Array(list)) => list.iter().any(|x| x == a),
            Some(_) => false,
            None => false,
        }
    }
}

/// The `{field, value}` filter specs under `key` of a bibsection.
fn specs<'a>(bibsection: &'a Value, key: &str) -> Option<&'a Vec<Value>> {
    bibsection.get(key).and_then(Value::as_array)
}

fn spec_matches(item: &Value, spec: &Value) -> bool {
    let field = spec.get("field").map(js::to_js_string).unwrap_or_default();
    eval_spec(
        spec.get("value").unwrap_or(&Value::Null),
        item.get(field.as_str()),
    )
}

impl State {
    /// `makeBibliography(bibsection)`: the bibliography's parameters and
    /// entries; `None` is upstream's `false` (a style without a
    /// `cs:bibliography`). `bibsection` is the filter object (`include`,
    /// `exclude`, `select`, `quash`, `page_start`, `page_length`) or a string
    /// (the `citation_number_slug`).
    pub fn make_bibliography(&mut self, bibsection: Option<&Value>) -> CslResult<Option<BibliographyResult>> {
        let mut bibsection: Option<Value> = bibsection.filter(|b| js::truthy(b)).cloned();
        let exclude_types = self.bibliography.opt.get("exclude_types").cloned();
        let exclude_fields = self.bibliography.opt.get("exclude_with_fields").cloned();
        if bibsection.is_none()
            && (js::truthy_opt(exclude_types.as_ref()) || js::truthy_opt(exclude_fields.as_ref()))
        {
            let mut exclude: Vec<Value> = Vec::new();
            if js::truthy_opt(exclude_types.as_ref()) {
                for val in exclude_types.iter().filter_map(Value::as_array).flatten() {
                    exclude.push(serde_json::json!({"field": "type", "value": val}));
                }
            }
            if js::truthy_opt(exclude_fields.as_ref()) {
                for field in exclude_fields.iter().filter_map(Value::as_array).flatten() {
                    exclude.push(serde_json::json!({"field": field, "value": true}));
                }
            }
            bibsection = Some(serde_json::json!({ "exclude": exclude }));
        }
        // API change: added in version 1.0.51
        if self.bibliography.tokens.is_empty() {
            return Ok(None);
        }
        if let Some(Value::String(slug)) = &bibsection {
            self.opt
                .insert("citation_number_slug".into(), Value::String(slug.clone()));
            bibsection = None;
        }

        // For paged returns
        let (processed_item_ids, entry_strings, done) =
            self.get_bibliography_entries(bibsection.as_ref())?;
        let entry_ids: Vec<Value> = processed_item_ids
            .iter()
            .map(|ids| Value::Array(ids.iter().cloned().map(Value::String).collect()))
            .collect();

        let mut params = Obj::new();
        params.insert("maxoffset".into(), Value::from(0));
        params.insert(
            "entryspacing".into(),
            self.bibliography.opt.get("entry-spacing").cloned().unwrap_or(Value::Null),
        );
        params.insert(
            "linespacing".into(),
            self.bibliography.opt.get("line-spacing").cloned().unwrap_or(Value::Null),
        );
        params.insert("second-field-align".into(), Value::Bool(false));
        params.insert("entry_ids".into(), Value::Array(entry_ids));
        params.insert(
            "bibliography_errors".into(),
            Value::Array(self.tmp.bibliography_errors.clone()),
        );
        params.insert("done".into(), Value::Bool(done));
        if js::truthy_opt(self.bibliography.opt.get("second-field-align")) {
            if let Some(v) = self.bibliography.opt.get("second-field-align") {
                params.insert("second-field-align".into(), v.clone());
            }
        }
        let mut maxoffset: i64 = 0;
        for id in &self.registry.reflist {
            if let Some(t) = self.registry.registry.get(id) {
                if t.offset > maxoffset {
                    maxoffset = t.offset;
                }
            }
        }
        params.insert("maxoffset".into(), Value::from(maxoffset));
        if js::truthy_opt(self.bibliography.opt.get("hangingindent")) {
            if let Some(v) = self.bibliography.opt.get("hangingindent") {
                params.insert("hangingindent".into(), v.clone());
            }
        }
        params.insert(
            "bibstart".into(),
            Value::String(self.fun.decorate.bibstart().to_string()),
        );
        params.insert("bibend".into(), Value::String(self.fun.decorate.bibend().to_string()));

        self.opt.insert("citation_number_slug".into(), Value::Bool(false));
        Ok(Some(BibliographyResult {
            params,
            entry_strings,
        }))
    }

    /// `CSL.getBibliographyEntries.call(state, bibsection)`: render every
    /// registered item that the filter lets through. Returns the item ids
    /// of each entry, the entry strings, and (paged returns) whether the
    /// last item has been reached.
    pub fn get_bibliography_entries(
        &mut self,
        bibsection: Option<&Value>,
    ) -> CslResult<(Vec<Vec<String>>, Vec<String>, bool)> {
        let mut ret: Vec<String> = Vec::new();
        self.tmp.area = "bibliography".to_string();
        self.tmp.root = "bibliography".to_string();
        self.tmp.last_rendered_name = Value::Bool(false);
        self.tmp.bibliography_errors = Vec::new();
        self.tmp.bibliography_pos = 0;

        // For paged returns: disable generated entries and
        // do not fetch full items as a batch (input variable
        // consists of ids only in this case)
        let paged = bibsection
            .map(|b| js::truthy_opt(b.get("page_start")) && js::truthy_opt(b.get("page_length")))
            .unwrap_or(false);
        let sorted_ids = self.registry.get_sorted_ids();
        // `refetchItems(ids)`: the items themselves (here: looked up per id).
        let mut input: Vec<String> = sorted_ids;

        self.tmp.disambig_override = true;

        let mut skips: BTreeSet<String> = BTreeSet::new();

        // For paged returns
        let mut page_item_count: i64 = 0;
        if paged {
            if let Some(b) = bibsection {
                if b.get("page_start") != Some(&Value::Bool(true)) {
                    let page_start = b.get("page_start").map(js::to_js_string).unwrap_or_default();
                    for id in &input {
                        skips.insert(id.clone());
                        if page_start == *id {
                            break;
                        }
                    }
                }
            }
        }

        let mut processed_item_ids: Vec<Vec<String>> = Vec::new();

        let mut consolidated_ids: BTreeSet<String> = BTreeSet::new();
        self.tmp.container_item_count = Obj::new();
        if !paged {
            let mut kept: Vec<String> = Vec::new();
            for id in &input {
                let o = self.item_data(id)?;
                let mut keep = true;
                if let Some(l) = o.get("legislation_id").filter(|v| js::truthy(v)) {
                    let l = js::to_js_string(l);
                    if consolidated_ids.contains(&l) {
                        keep = false;
                    } else {
                        consolidated_ids.insert(l);
                    }
                } else if let Some(c) = o.get("container_id").filter(|v| js::truthy(v)) {
                    let c = js::to_js_string(c);
                    let n = self
                        .tmp
                        .container_item_count
                        .get(&c)
                        .and_then(Value::as_i64)
                        .unwrap_or(0);
                    self.tmp
                        .container_item_count
                        .insert(c.clone(), Value::from(n + 1));
                    let consolidate = self
                        .bibliography
                        .opt
                        .get("consolidate_containers")
                        .and_then(Value::as_array)
                        .ok_or_else(|| type_error("Cannot read properties of undefined (reading 'indexOf')"))?;
                    let ty = o.get("type").and_then(Value::as_str).unwrap_or("");
                    if consolidate.iter().any(|t| t.as_str() == Some(ty)) {
                        if consolidated_ids.contains(&c) {
                            keep = false;
                        } else {
                            consolidated_ids.insert(c);
                        }
                    }
                }
                if keep {
                    kept.push(id.clone());
                }
            }
            input = kept;
        }

        self.tmp.container_item_pos = Obj::new();

        for i in 0..input.len() {
            // For paged returns
            let item: Value;
            if paged {
                if skips.contains(&input[i]) {
                    continue;
                }
                item = self.item_data(&input[i])?;
                let page_length = bibsection
                    .and_then(|b| b.get("page_length"))
                    .and_then(Value::as_i64);
                if Some(page_item_count) == page_length {
                    break;
                }
            } else {
                item = self.item_data(&input[i])?;
                if skips.contains(&super::registry::id_key(item.get("id"))) {
                    continue;
                }
            }
            let item_id = super::registry::id_key(item.get("id"));
            if let Some(bs) = bibsection {
                let mut include = true;
                if js::truthy_opt(bs.get("include")) {
                    //
                    // Opt-in: these are OR-ed.
                    //
                    include = false;
                    for spec in specs(bs, "include").into_iter().flatten() {
                        if spec_matches(&item, spec) {
                            include = true;
                            break;
                        }
                    }
                } else if js::truthy_opt(bs.get("exclude")) {
                    //
                    // Opt-out: these are also OR-ed.
                    //
                    let mut anymatch = false;
                    for spec in specs(bs, "exclude").into_iter().flatten() {
                        if spec_matches(&item, spec) {
                            anymatch = true;
                            break;
                        }
                    }
                    if anymatch {
                        include = false;
                    }
                } else if js::truthy_opt(bs.get("select")) {
                    //
                    // Multiple condition opt-in: these are AND-ed.
                    //
                    include = false;
                    let mut allmatch = true;
                    for spec in specs(bs, "select").into_iter().flatten() {
                        if !spec_matches(&item, spec) {
                            allmatch = false;
                        }
                    }
                    if allmatch {
                        include = true;
                    }
                }
                if js::truthy_opt(bs.get("quash")) {
                    //
                    // Stop criteria: These are AND-ed.
                    //
                    let mut allmatch = true;
                    for spec in specs(bs, "quash").into_iter().flatten() {
                        if !spec_matches(&item, spec) {
                            allmatch = false;
                        }
                    }
                    if allmatch {
                        include = false;
                    }
                }
                if !include {
                    continue;
                }
            }

            if let Some(c) = item.get("container_id").filter(|v| js::truthy(v)) {
                let c = js::to_js_string(c);
                let n = self
                    .tmp
                    .container_item_pos
                    .get(&c)
                    .and_then(Value::as_i64)
                    .unwrap_or(0);
                self.tmp.container_item_pos.insert(c, Value::from(n + 1));
            }

            let mut bib_entry = Token::new("group", TokenType::Start);
            bib_entry.decorations = vec![Decoration::new("@bibliography", "entry")];
            bib_entry.decorations.extend(
                self.bibliography
                    .opt
                    .get("layout_decorations")
                    .and_then(queue::decorations_from_value)
                    .unwrap_or_default(),
            );
            queue::start_tag(self, QueueId::Output, "bib_entry", Some(&bib_entry))?;
            if let Some(cur) = queue::current(self, QueueId::Output) {
                let embed = self.fun.host_hooks.embed_bibliography_entry;
                match item.get("system_id").filter(|v| js::truthy(v)) {
                    Some(sid) if embed => {
                        self.blobs.get_mut(cur).extra.insert("item_id".into(), sid.clone());
                    }
                    _ => {
                        self.blobs
                            .get_mut(cur)
                            .extra
                            .insert("system_id".into(), item.get("id").cloned().unwrap_or(Value::Null));
                    }
                }
            }

            // 2019-06-25 Hacked to conform to new parallels evaluation method
            // 2020-04-25 Revised to work with latest, and final, parallel-first/parallel-last attributes
            let mut entry_item_ids: Vec<String> = Vec::new();
            let token = self.registry.registry.get(&item_id).cloned().ok_or_else(|| {
                type_error("Cannot read properties of undefined (reading 'master')")
            })?;
            if token.master && !paged {
                // Fetch item content
                let mut sorted_items: Vec<(Value, Obj)> = Vec::new();
                let mut first = Obj::new();
                first.insert("id".into(), item.get("id").cloned().unwrap_or(Value::Null));
                sorted_items.push((item.clone(), first));
                for sibling in token.siblings.clone().unwrap_or_default() {
                    let mut o = Obj::new();
                    o.insert("id".into(), Value::String(sibling.clone()));
                    sorted_items.push((self.item_data(&sibling)?, o));
                }
                // Adjust parameters
                util_parallel::start_citation(self, &mut sorted_items)?;
                let delimiter = token
                    .parallel_delimiter_override
                    .as_ref()
                    .filter(|v| js::truthy(v))
                    .map(js::to_js_string)
                    .unwrap_or_else(|| ", ".to_string());
                if let Some(BlobChild::Blob(first_blob)) =
                    queue::queue_children(self, QueueId::Output).first().cloned()
                {
                    self.blobs.get_mut(first_blob).set_string("delimiter", &delimiter);
                }
                self.tmp.term_predecessor = false;
                self.tmp.cite_index = 0;
                // Run cites
                for j in 0..sorted_items.len() {
                    self.tmp.parallel_and_not_last = j < sorted_items.len() - 1;
                    let cite = Value::Object(sorted_items[j].1.clone());
                    entry_item_ids.push(self.get_cite(&sorted_items[j].0, &cite, None, false)?);
                    self.tmp.cite_index += 1;
                    skips.insert(super::registry::id_key(sorted_items[j].0.get("id")));
                }
                self.tmp.parallel_and_not_last = false;
            } else if token.siblings.is_none() {
                self.tmp.term_predecessor = false;
                self.tmp.cite_index = 0;
                entry_item_ids.push(self.get_cite(&item, &Value::Null, None, false)?);
                if paged {
                    page_item_count += 1;
                }
                //skips[item.id] = true;
            }

            self.tmp.bibliography_pos += 1;

            processed_item_ids.push(entry_item_ids);
            //
            // XXX: loop to render parallels goes here
            // XXX: just have to mark them somehow ...
            //
            queue::end_tag(self, QueueId::Output, Some("bib_entry"))?;
            //
            // place layout prefix on first blob of each cite, and suffix
            // on the last non-empty blob of each cite.  there be dragons
            // here.
            //
            self.prefix_first_blob()?;
            for id in self.queue_blobs() {
                queue::purge_empty_blobs(&mut self.blobs, id);
            }
            let adjust = self.output.adjust.clone().ok_or_else(|| {
                type_error("Cannot read properties of undefined (reading 'upward')")
            })?;
            for id in self.queue_blobs() {
                adjust.upward(&mut self.blobs, id);
                adjust.leftward(&mut self.blobs, id);
                adjust.downward(&mut self.blobs, id);
                adjust.fix(&mut self.blobs, id);
            }

            // XXX Need to account for numeric blobs in input.
            // XXX No idea how this could have worked previously.
            let rendered = queue::string(
                self,
                QueueId::Output,
                &queue::queue_children(self, QueueId::Output),
                StringParent::None,
            )?;
            let mut res: Option<Rendered> = rendered.into_list().into_iter().next();
            let res_empty = res.as_ref().map(|r| !r.is_truthy()).unwrap_or(true);
            if res_empty && self.opt.get("update_mode").and_then(Value::as_i64) == Some(super::load::NUMERIC) {
                let err = format!("{}. [CSL STYLE ERROR: reference with no printed form.]", ret.len() + 1);
                res = Some(Rendered::Str(formats::decorate(
                    self,
                    None,
                    "@bibliography",
                    "entry",
                    Some(&err),
                    None,
                )?));
            }
            if let Some(r) = res {
                if r.is_truthy() {
                    ret.push(r.to_js_string());
                }
            }
        }

        let mut done = false;
        if paged {
            let last_expected = input.last().cloned();
            let last_seen = processed_item_ids.last().map(|v| v.join(","));
            if last_expected.is_none()
                || last_seen.is_none()
                || last_expected.as_ref() == last_seen.as_ref()
            {
                done = true;
            }
        }
        self.tmp.disambig_override = false;

        // XXX done
        Ok((processed_item_ids, ret, done))
    }

    /// `topblobs[0].strings.prefix = layout_prefix + topblobs[0].strings.prefix`
    /// (api_bibliography.js:329-345): put the layout prefix on the first blob
    /// of the entry.
    fn prefix_first_blob(&mut self) -> CslResult<()> {
        let Some(BlobChild::Blob(entry)) = queue::queue_children(self, QueueId::Output).first().cloned()
        else {
            return Ok(());
        };
        let entry_kids: Vec<BlobChild> = match &self.blobs.get(entry).blobs {
            BlobContent::List(l) => l.clone(),
            BlobContent::Text(_) => Vec::new(),
        };
        let Some(first) = entry_kids.first().cloned() else {
            return Ok(());
        };
        // `queue[0].blobs[0].blobs.length`
        let first_len = match &first {
            BlobChild::Blob(b) => match &self.blobs.get(*b).blobs {
                BlobContent::List(l) => l.len(),
                BlobContent::Text(t) => js::len(t),
            },
            BlobChild::Str(s) => js::len(s),
        };
        if first_len == 0 {
            return Ok(());
        }
        // The output queue stuff needs cleaning up.  the result of
        // output.current.value() is sometimes a blob, sometimes its list
        // of blobs.  this inconsistency is a source of confusion, and
        // should be cleaned up across the code base in the first
        // instance, before making any other changes to output code.
        let grandchild_has_strings = match &first {
            BlobChild::Blob(b) => match &self.blobs.get(*b).blobs {
                BlobContent::List(l) => matches!(l.first(), Some(BlobChild::Blob(_))),
                BlobContent::Text(_) => false,
            },
            BlobChild::Str(_) => false,
        };
        let topblobs: Vec<BlobChild> = if !grandchild_has_strings {
            entry_kids
        } else {
            match &first {
                BlobChild::Blob(b) => match &self.blobs.get(*b).blobs {
                    BlobContent::List(l) => l.clone(),
                    BlobContent::Text(_) => Vec::new(),
                },
                BlobChild::Str(_) => Vec::new(),
            }
        };
        match topblobs.first() {
            Some(BlobChild::Blob(b)) => {
                let layout_prefix = js::get_string(&self.bibliography.opt, "layout_prefix")
                    .unwrap_or_else(|| "undefined".to_string());
                let current = self.blobs.get(*b).string("prefix");
                self.blobs
                    .get_mut(*b)
                    .set_string("prefix", &format!("{layout_prefix}{current}"));
                Ok(())
            }
            _ => Err(type_error("Cannot read properties of undefined (reading 'strings')")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn eval_spec_follows_the_nested_ifs_of_upstream() {
        // boolean / falsy filter values test presence
        assert!(eval_spec(&json!(true), Some(&json!("x"))));
        assert!(!eval_spec(&json!(true), None));
        assert!(eval_spec(&json!(false), None));
        assert!(!eval_spec(&json!(false), Some(&json!(1))));
        assert!(eval_spec(&json!(""), Some(&json!(""))));
        // string filter value: equal to a string field, or a member of a list field
        assert!(eval_spec(&json!("book"), Some(&json!("book"))));
        assert!(!eval_spec(&json!("book"), Some(&json!("article"))));
        assert!(eval_spec(&json!("a"), Some(&json!(["b", "a"]))));
        assert!(!eval_spec(&json!("a"), Some(&json!(["b"]))));
        assert!(!eval_spec(&json!("a"), None));
        assert!(!eval_spec(&json!("a"), Some(&json!({"year": 1}))));
        let item = json!({"type": "book", "keyword": ["x", "y"]});
        assert!(spec_matches(&item, &json!({"field": "keyword", "value": "y"})));
        assert!(!spec_matches(&item, &json!({"field": "type", "value": "article"})));
    }
}
