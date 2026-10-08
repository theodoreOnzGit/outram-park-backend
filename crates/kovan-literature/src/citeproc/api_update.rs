// Part of the kovan port of citeproc-js (GitHub #790).
//
// Upstream:    citeproc-js, https://github.com/juris-m/citeproc-js
// Source:      src/api_update.js
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

//! The update API: `updateItems`, `updateUncitedItems`, `rebuildProcessorState`
//! and `restoreProcessorState` (deprecated upstream, ported for completeness).

use std::collections::BTreeSet;

use serde_json::Value;

use super::api_cite::{CitationInput, CitationPos, ClusterFlag};
use super::disambig_citations::{CitId, CitationRec, SortedItem};
use super::js;
use super::registry;
use super::state::{State, Tmp};
use super::build_retrieve_item::retrieve_item;
use super::{CslResult, EngineError};

/// The `[citationID, noteIndex, string]` triples `rebuildProcessorState`
/// returns, in document order.
pub type RebuiltCitation = (String, Value, String);

impl State {
    /// `updateItems(idList, nosort, rerun_ambigs, implicitUpdate)`: register
    /// exactly the items `id_list` (adding the missing, deleting the others),
    /// sort them and settle their disambiguation. Returns `getSortedIds()`.
    pub fn update_items(
        &mut self,
        id_list: &[String],
        nosort: bool,
        rerun_ambigs: bool,
        implicit_update: bool,
    ) -> CslResult<Vec<String>> {
        let old_area = self.tmp.area.clone();
        let old_root = self.tmp.root.clone();
        let old_extension = self.tmp.extension.clone();
        let nosort = nosort || self.bibliography_sort.tokens.is_empty();
        self.tmp.area = "citation".to_string();
        self.tmp.root = "citation".to_string();
        self.tmp.extension = String::new();
        if !implicit_update {
            self.tmp.loaded_item_ids = Default::default();
        }
        let r = self.update_items_steps(id_list, nosort, rerun_ambigs, false);
        self.tmp.extension = old_extension;
        self.tmp.area = old_area;
        self.tmp.root = old_root;
        r?;
        Ok(self.registry.get_sorted_ids())
    }

    /// `updateUncitedItems(idList, nosort)`: register `id_list` as uncited
    /// items (kept in the bibliography without being cited).
    pub fn update_uncited_items(&mut self, id_list: &[String], nosort: bool) -> CslResult<Vec<String>> {
        let old_area = self.tmp.area.clone();
        let old_root = self.tmp.root.clone();
        let old_extension = self.tmp.extension.clone();
        let nosort = nosort || self.bibliography_sort.tokens.is_empty();
        self.tmp.area = "citation".to_string();
        self.tmp.root = "citation".to_string();
        self.tmp.extension = String::new();
        self.tmp.loaded_item_ids = Default::default();
        let r = self.update_items_steps(id_list, nosort, false, true);
        self.tmp.extension = old_extension;
        self.tmp.area = old_area;
        self.tmp.root = old_root;
        r?;
        Ok(self.registry.get_sorted_ids())
    }

    /// The common body of the two update functions.
    fn update_items_steps(
        &mut self,
        id_list: &[String],
        nosort: bool,
        rerun_ambigs: bool,
        uncited: bool,
    ) -> CslResult<()> {
        registry::init(self, id_list.to_vec(), uncited);

        if rerun_ambigs {
            let keys: Vec<String> = self.registry.ambigcites.keys().cloned().collect();
            for ambig in keys {
                self.registry.ambigs_touched.insert(ambig);
            }
        }

        if uncited {
            // Use purge instead of delete.
            // this.registry.dodeletes(this.registry.myhash);
            let idhash: BTreeSet<String> = id_list.iter().cloned().collect();
            registry::dopurge(self, &idhash)?;
        } else {
            let keep = self.registry.myhash.clone();
            registry::dodeletes(self, &keep)?;
        }

        let mylist = self.registry.mylist.clone();
        registry::doinserts(self, &mylist)?;

        registry::dorefreshes(self)?;

        // *** affects reflist
        registry::rebuildlist(self, nosort)?;

        registry::setsortkeys(self)?;

        // taints always
        registry::setdisambigs(self)?;

        // *** affects reflist
        registry::sorttokens(self, nosort)?;

        // *** affects reflist
        // taints if numbered style
        registry::renumber(self);
        Ok(())
    }

    /// `rebuildProcessorState(citations, mode, uncitedItemIDs)`: rebuild the
    /// processor from scratch from a list of citation objects in document
    /// order. Returns `[citationID, noteIndex, string]` triples in document
    /// order. `mode` is `"html"` when empty.
    pub fn rebuild_processor_state(
        &mut self,
        citations: &[CitationInput],
        mode: &str,
        uncited_item_ids: &[String],
    ) -> CslResult<Vec<Option<RebuiltCitation>>> {
        let mode = if mode.is_empty() { "html" } else { mode };
        let mut done_ids: BTreeSet<String> = BTreeSet::new();
        let mut item_ids: Vec<String> = Vec::new();
        for c in citations {
            for item in &c.citation_items {
                let id = js::to_js_string(item.get("id").unwrap_or(&Value::Null));
                if !done_ids.contains(&id) {
                    item_ids.push(id.clone());
                }
                done_ids.insert(id);
            }
        }
        self.update_items(&item_ids, false, false, false)?;
        let mut pre: Vec<CitationPos> = Vec::new();
        let mut ret: Vec<Option<RebuiltCitation>> = Vec::new();
        let old_mode = js::get_string(&self.opt, "mode").unwrap_or_else(|| "html".into());
        self.set_output_format(mode)?;
        for citation in citations {
            // res contains a result report and a list of [index,string] pairs
            // index begins at 0
            let res = self.process_citation_cluster(
                citation.clone(),
                &pre,
                &[],
                ClusterFlag::AssumeAllItemsRegistered,
            )?;
            let note_index = citation
                .properties
                .as_ref()
                .and_then(|p| p.get("noteIndex").cloned())
                .unwrap_or(Value::Null);
            pre.push(CitationPos {
                citation_id: citation.citation_id.clone().unwrap_or_default(),
                note_index,
            });
            for (index, text, _) in res.updates {
                let index = index.max(0) as usize;
                while ret.len() <= index {
                    ret.push(None);
                }
                let p = pre.get(index).ok_or_else(|| {
                    EngineError::Csl("TypeError: Cannot read properties of undefined (reading '0')".into())
                })?;
                ret[index] = Some((p.citation_id.clone(), p.note_index.clone(), text));
            }
        }
        self.update_uncited_items(uncited_item_ids, false)?;
        self.set_output_format(&old_mode)?;
        Ok(ret)
    }

    /// `restoreProcessorState(citations)` (deprecated upstream: use
    /// [`State::rebuild_processor_state`]): quickly restore state from the
    /// details a calling application kept, taking `properties.index` and each
    /// item's `sortkeys` as correct.
    pub fn restore_processor_state(
        &mut self,
        citations: &[CitationInput],
    ) -> CslResult<Vec<(i64, String, String)>> {
        let mut item_list: Vec<String> = Vec::new();
        // Adjust citationIDs to avoid duplicates, save off index numbers
        let mut recs: Vec<CitId> = Vec::new();
        let mut citation_ids: BTreeSet<String> = BTreeSet::new();
        for c in citations {
            let mut properties = c.properties.clone().unwrap_or_default();
            if !properties.contains_key("noteIndex") && c.properties.is_none() {
                properties.insert("noteIndex".into(), Value::from(0));
            }
            let cid = self.registry.citationreg.alloc(CitationRec {
                citation_id: c.citation_id.clone().unwrap_or_default(),
                citation_items: c.citation_items.clone(),
                properties,
                sorted_items: Vec::new(),
            });
            let id = self.registry.citationreg.get(cid).citation_id.clone();
            if citation_ids.contains(&id) {
                self.set_citation_id(cid, true);
            }
            let id = self.registry.citationreg.get(cid).citation_id.clone();
            citation_ids.insert(id);
            recs.push(cid);
        }
        // Slice citations and sort by their declared index positions, if any,
        // then reassign index and noteIndex numbers.
        let mut old_citations = recs.clone();
        {
            let reg = &self.registry.citationreg;
            old_citations.sort_by(|a, b| {
                let ia = reg.get(*a).properties.get("index").and_then(Value::as_f64);
                let ib = reg.get(*b).properties.get("index").and_then(Value::as_f64);
                match (ia, ib) {
                    (Some(x), Some(y)) => x.partial_cmp(&y).unwrap_or(std::cmp::Ordering::Equal),
                    _ => std::cmp::Ordering::Equal,
                }
            });
        }
        for (i, c) in old_citations.iter().enumerate() {
            self.registry
                .citationreg
                .get_mut(*c)
                .properties
                .insert("index".into(), Value::from(i as i64));
        }
        for c in &old_citations {
            let mut sorted: Vec<SortedItem> = Vec::new();
            let items = self.registry.citationreg.get(*c).citation_items.clone();
            for mut item in items {
                if !item.contains_key("sortkeys") {
                    item.insert("sortkeys".into(), Value::Array(Vec::new()));
                }
                let id = js::to_js_string(item.get("id").unwrap_or(&Value::Null));
                retrieve_item(self, &id)?;
                item_list.push(id.clone());
                sorted.push(SortedItem { item_id: id, item });
            }
            self.registry.citationreg.get_mut(*c).sorted_items = sorted;
            if !js::truthy_opt(self.registry.citationreg.get(*c).properties.get("unsorted")) {
                self.sort_cite_items(*c)?;
            }
            // Save citation data in registry
            let id = self.registry.citationreg.get(*c).citation_id.clone();
            self.registry.citationreg.citation_by_id.insert(id, *c);
        }
        // Register Items
        self.update_items(&item_list, false, false, false)?;

        // Construct citationList from original copy
        let citation_list: Vec<CitationPos> = recs
            .iter()
            .map(|c| {
                let r = self.registry.citationreg.get(*c);
                CitationPos {
                    citation_id: r.citation_id.clone(),
                    note_index: r.properties.get("noteIndex").cloned().unwrap_or(Value::Null),
                }
            })
            .collect();

        if let Some(first) = recs.first() {
            // Rendering one citation restores remainder of processor state.
            // If citations is empty, rest to empty state.
            let rec = self.registry.citationreg.get(*first);
            let input = CitationInput {
                citation_id: Some(rec.citation_id.clone()),
                citation_items: rec.citation_items.clone(),
                properties: Some(rec.properties.clone()),
            };
            let res = self.process_citation_cluster(input, &[], &citation_list[1..], ClusterFlag::None)?;
            Ok(res.updates)
        } else {
            self.registry = registry::Registry::new(registry::Comparifier::new(self, "bibliography_sort"));
            self.tmp = Tmp::new();
            self.disambiguate = super::disambig_cites::Disambiguation::new(self);
            Ok(Vec::new())
        }
    }
}

