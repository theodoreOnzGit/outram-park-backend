// Part of the kovan Zotero port (GitHub #747, #751).
//
// Upstream: Zotero, https://github.com/zotero/zotero (commit 9cbba8c4d281):
// chrome/content/zotero/xpcom/data/search.js (addCondition, removeCondition,
// updateCondition, setScope, search, fromJSON, toJSON,
// _fullTextContentMatches, _buildQuery, hasPostSearchFilter).
// Copyright (c) Corporation for Digital Scholarship, Vienna, Virginia, USA
// (search.js: (c) 2009 Center for History and New Media, George Mason
// University). Licence: AGPL-3.0. See this crate's NOTICE, "Upstream: Zotero".

//! [`Search`]: a Zotero search, evaluated in memory.
//!
//! The port follows `_buildQuery` step by step, replacing each piece of SQL
//! with the set of rows it selects. Line references in comments point at
//! search.js unless another file is named.

use std::sync::Arc;

use kovan_common::zotero::date::{is_sql_date_time, str_to_date};
use kovan_common::zotero::schema::type_fields_from_base;
use kovan_common::zotero::search::{SearchCondition, ZoteroSearch};
use kovan_common::zotero::{Field, LinkMode};
use serde_json::{json, Value};
use unicode_normalization::UnicodeNormalization;

use super::conditions::{
    is_valid_object_key, parse_condition, parse_search_string, resolve, Kind, Level, Operator,
};
use super::dates::{date_now_minus, date_of_sql_local, date_of_unix_local, local_today};
use super::fulltext::{
    can_search_content, can_search_notes, find_items_with_content, find_text_in_items, index_only,
};
use super::levels::{combine_conditions, Built, Predicate};
use super::library::{
    annotation_type_number, date_options, link_mode_number, RowSet, SearchLibrary, FILE_TYPES,
};
use super::normalize::{
    is_canonical_int, normalize_for_search, nocase_eq, sql_like, sqlite_cast_int, substr,
};

/// One search condition (`{condition, mode, operator, value}`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Condition {
    /// The condition name, e.g. `title`, `tag`, `joinMode`.
    pub condition: String,
    /// The mode after a `/` (e.g. `regexp` in `fulltextContent/regexp`).
    pub mode: Option<String>,
    /// The operator.
    pub operator: Operator,
    /// The value (`''` for markers and `isEmpty`/`isNotEmpty`).
    pub value: String,
}

/// Why a search could not be built or run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SearchError {
    /// An unknown condition name (searchConditions.js:909).
    InvalidCondition(String),
    /// An operator the condition does not take (search.js:309).
    InvalidOperator {
        /// The condition.
        condition: String,
        /// The operator as given.
        operator: String,
    },
    /// A value upstream rejects (an unknown `attachmentStorageType` or
    /// `fileTypeID`, search.js:1608, 1636), or one the generated SQL cannot
    /// evaluate (`isInTheLast` with an SQL date-time).
    InvalidValue {
        /// The condition.
        condition: String,
        /// The value.
        value: String,
    },
    /// A condition that needs database state a `ZoteroLibrary` does not
    /// carry (`libraryID`, `itemID`, `itemTypeID`, `tagID`, `tempTable`,
    /// `annotationAuthor`), or a combination upstream turns into invalid SQL
    /// (`savedSearch` with `includeParents`/`includeChildren`/
    /// `includeParentsAndChildren`, search.js:2031).
    Unsupported(String),
    /// A saved search that refers to itself through other saved searches
    /// (upstream recurses without end).
    RecursiveSavedSearch(String),
    /// An empty saved-search name (search.js:80).
    EmptyName,
    /// No condition with this id (search.js:470, 502).
    InvalidConditionId(usize),
    /// Malformed search JSON, or an unknown property in strict mode
    /// (search.js:860).
    Json(String),
}

impl std::fmt::Display for SearchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SearchError::InvalidCondition(c) => write!(f, "Invalid condition '{c}'"),
            SearchError::InvalidOperator {
                condition,
                operator,
            } => write!(f, "Invalid operator '{operator}' for condition {condition}"),
            SearchError::InvalidValue { condition, value } => {
                write!(f, "Invalid value '{value}' for condition {condition}")
            }
            SearchError::Unsupported(s) => write!(f, "Unsupported in memory: {s}"),
            SearchError::RecursiveSavedSearch(k) => {
                write!(f, "Saved search {k} refers to itself")
            }
            SearchError::EmptyName => write!(f, "Saved search name cannot be empty"),
            SearchError::InvalidConditionId(i) => write!(f, "Invalid searchConditionID {i}"),
            SearchError::Json(s) => write!(f, "{s}"),
        }
    }
}

impl std::error::Error for SearchError {}

/// The scope a search runs within (`setScope`, search.js:449).
#[derive(Debug, Clone)]
pub struct SearchScope {
    /// The search whose results bound this one.
    pub search: Search,
    /// Also allow the scope's child attachments, notes and annotations.
    pub include_children: bool,
}

/// A Zotero search: conditions, optionally a scope, and (for a saved search)
/// a key and name.
#[derive(Debug, Clone, Default)]
pub struct Search {
    /// The saved-search key (how `savedSearch` conditions refer to it).
    pub key: Option<String>,
    /// The saved-search version (sync).
    pub version: Option<u64>,
    /// The saved-search name.
    name: Option<String>,
    /// In the trash.
    pub deleted: bool,
    conditions: Vec<Condition>,
    scope: Option<Arc<SearchScope>>,
}

/// Special flags read in `_buildQuery`'s first loop (search.js:1110-1154).
#[derive(Debug, Clone, Copy, Default)]
struct Flags {
    deleted: bool,
    include_deleted: bool,
    no_children: bool,
    include_parents_and_children: bool,
    include_parents: bool,
    include_children: bool,
    unfiled: bool,
    retracted: bool,
    publications: bool,
    feed: bool,
    recursive: bool,
}

/// A table/savedSearch condition after inline-filter merging
/// (search.js:1090).
#[derive(Debug, Clone)]
struct Leaf {
    kind: Kind,
    name: String,
    operator: Operator,
    values: Vec<String>,
}

#[derive(Debug, Clone)]
enum Pending {
    Marker(Built),
    Leaf(Leaf),
    FullText {
        operator: Operator,
        value: String,
        mode: Option<String>,
    },
}

impl Search {
    /// An empty search.
    pub fn new() -> Self {
        Search::default()
    }

    /// The saved-search name.
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// Set the name; an empty name is an error (search.js:78-83).
    pub fn set_name(&mut self, name: &str) -> Result<(), SearchError> {
        if name.is_empty() {
            return Err(SearchError::EmptyName);
        }
        self.name = Some(name.to_owned());
        Ok(())
    }

    /// The conditions, in order (their index is the `searchConditionID`).
    pub fn conditions(&self) -> &[Condition] {
        &self.conditions
    }

    /// The scope, if set.
    pub fn scope(&self) -> Option<&SearchScope> {
        self.scope.as_deref()
    }

    /// `setScope` (search.js:449): bound this search's results by `scope`'s.
    pub fn set_scope(&mut self, scope: Search, include_children: bool) {
        self.scope = Some(Arc::new(SearchScope {
            search: scope,
            include_children,
        }));
    }

    /// `addCondition` (search.js:301). `condition` may carry a `/mode`.
    ///
    /// Quick-search conditions (`quicksearch-*`) expand here into one
    /// OR-group per word or phrase, exactly as upstream (search.js:315-377).
    /// `collectionID`/`savedSearchID` take a **key** here (no database ids)
    /// and become `collection`/`savedSearch`; upstream checks the object
    /// exists at this point, this port when the search runs.
    pub fn add_condition(
        &mut self,
        condition: &str,
        operator: &str,
        value: &str,
    ) -> Result<(), SearchError> {
        let (name, mode) = parse_condition(condition);
        let kind = resolve(name).ok_or_else(|| SearchError::InvalidCondition(name.to_owned()))?;
        let op = Operator::from_name(operator)
            .filter(|o| kind.has_operator(*o))
            .ok_or_else(|| SearchError::InvalidOperator {
                condition: condition.to_owned(),
                operator: operator.to_owned(),
            })?;

        match kind {
            Kind::QuickTitleCreatorYear
            | Kind::QuickTitleCreatorYearNote
            | Kind::QuickFields
            | Kind::QuickEverything
            | Kind::QuickDeprecated => {
                for part in parse_search_string(value) {
                    let t = part.text.as_str();
                    if kind == Kind::QuickTitleCreatorYearNote {
                        self.add_condition("note", operator, t)?;
                        continue;
                    }
                    self.add_condition("groupStart", "true", "")?;
                    self.add_condition("joinMode", "any", "")?;
                    if op == Operator::Contains && is_valid_object_key(t) {
                        self.add_condition("key", "is", t)?;
                    }
                    if kind == Kind::QuickTitleCreatorYear {
                        for f in [
                            "title",
                            "publicationTitle",
                            "shortTitle",
                            "court",
                            "year",
                            "citationKey",
                        ] {
                            self.add_condition(f, operator, t)?;
                        }
                    } else {
                        self.add_condition("field", operator, t)?;
                        self.add_condition("tag", operator, t)?;
                        if part.in_quotes || can_search_notes(t) {
                            self.add_condition("note", operator, t)?;
                        }
                        self.add_condition("annotationText", operator, t)?;
                        self.add_condition("annotationComment", operator, t)?;
                    }
                    self.add_condition("creator", operator, t)?;
                    if kind == Kind::QuickEverything && (part.in_quotes || can_search_content(t)) {
                        self.add_condition("fulltextContent", operator, t)?;
                    }
                    self.add_condition("groupEnd", "true", "")?;
                }
                if kind == Kind::QuickTitleCreatorYear {
                    self.add_condition("noChildren", "true", "")?;
                } else if kind == Kind::QuickTitleCreatorYearNote {
                    self.add_condition("itemType", "is", "note")?;
                }
                return Ok(());
            }
            Kind::CollectionId => return self.add_condition("collection", operator, value),
            Kind::SavedSearchId => return self.add_condition("savedSearch", operator, value),
            _ => {}
        }

        let mut value = value.to_owned();
        // Old-style '0_ABCD2345' values (search.js:409-415).
        if matches!(kind, Kind::Collection | Kind::SavedSearch) && value.contains('_') {
            value = value.split('_').nth(1).unwrap_or("").to_owned();
        }
        if matches!(op, Operator::IsEmpty | Operator::IsNotEmpty) {
            value.clear();
        }
        self.conditions.push(Condition {
            condition: name.to_owned(),
            mode: mode.map(str::to_owned),
            operator: op,
            value: value.nfc().collect(),
        });
        Ok(())
    }

    /// `updateCondition` (search.js:462).
    pub fn update_condition(
        &mut self,
        id: usize,
        condition: &str,
        operator: &str,
        value: &str,
    ) -> Result<(), SearchError> {
        if id >= self.conditions.len() {
            return Err(SearchError::InvalidConditionId(id));
        }
        let (name, mode) = parse_condition(condition);
        let kind = resolve(name).ok_or_else(|| SearchError::InvalidCondition(name.to_owned()))?;
        let op = Operator::from_name(operator)
            .filter(|o| kind.has_operator(*o))
            .ok_or_else(|| SearchError::InvalidOperator {
                condition: condition.to_owned(),
                operator: operator.to_owned(),
            })?;
        self.conditions[id] = Condition {
            condition: name.to_owned(),
            mode: mode.map(str::to_owned),
            operator: op,
            value: value.nfc().collect(),
        };
        Ok(())
    }

    /// `removeCondition` (search.js:498): later conditions shift down.
    pub fn remove_condition(&mut self, id: usize) -> Result<(), SearchError> {
        if id >= self.conditions.len() {
            return Err(SearchError::InvalidConditionId(id));
        }
        self.conditions.remove(id);
        Ok(())
    }

    /// `hasPostSearchFilter` (search.js:556).
    pub fn has_post_search_filter(&self) -> bool {
        self.conditions
            .iter()
            .any(|c| c.condition == "fulltextContent")
    }

    /// `toJSON` (search.js:893).
    pub fn to_json(&self) -> Value {
        let mut o = serde_json::Map::new();
        if let Some(k) = &self.key {
            o.insert("key".into(), json!(k));
        }
        if let Some(v) = self.version {
            o.insert("version".into(), json!(v));
        }
        if let Some(n) = &self.name {
            o.insert("name".into(), json!(n));
        }
        let conds: Vec<Value> = self
            .conditions
            .iter()
            .map(|c| {
                json!({
                    "condition": match &c.mode {
                        Some(m) => format!("{}/{}", c.condition, m),
                        None => c.condition.clone(),
                    },
                    "operator": c.operator.as_str(),
                    "value": c.value,
                })
            })
            .collect();
        o.insert("conditions".into(), Value::Array(conds));
        if self.deleted {
            o.insert("deleted".into(), json!(true));
        }
        Value::Object(o)
    }

    /// The search engine's view of a stored saved search: kovan-common's
    /// [`ZoteroSearch`] (the `toJSON` shape, which is how a library stores
    /// it) read through [`Search::from_json`] (non-strict, so the
    /// `childNote` migration applies). A stored condition with a `null`
    /// operator is read as an empty operator, which `addCondition` rejects
    /// for every condition that takes operators (search.js:309).
    pub fn from_zotero_search(stored: &ZoteroSearch) -> Result<Search, SearchError> {
        let mut s = Search::new();
        s.from_json(&stored.to_json_value(), false)?;
        Ok(s)
    }

    /// This search in kovan-common's stored form ([`ZoteroSearch`]), as
    /// `toJSON` writes it (search.js:893). A search without a name gets an
    /// empty one.
    pub fn to_zotero_search(&self) -> ZoteroSearch {
        let mut z = ZoteroSearch::new(self.name.clone().unwrap_or_default());
        z.key = self.key.clone();
        z.version = self.version;
        z.deleted = self.deleted.then_some(true);
        z.conditions = self
            .conditions
            .iter()
            .map(|c| SearchCondition {
                condition: match &c.mode {
                    Some(m) => format!("{}/{}", c.condition, m),
                    None => c.condition.clone(),
                },
                operator: Some(c.operator.as_str().to_owned()),
                value: c.value.clone(),
            })
            .collect();
        z
    }

    /// `fromJSON` (search.js:848), including the migration of the obsolete
    /// `childNote` condition to `note` + `resultLevel item` (lines 877-885).
    pub fn from_json(&mut self, json: &Value, strict: bool) -> Result<(), SearchError> {
        let o = json
            .as_object()
            .ok_or_else(|| SearchError::Json("search JSON is not an object".into()))?;
        if strict {
            for k in o.keys() {
                if !matches!(
                    k.as_str(),
                    "key" | "version" | "name" | "conditions" | "deleted"
                ) {
                    return Err(SearchError::Json(format!("Unknown search property '{k}'")));
                }
            }
        }
        if let Some(n) = o
            .get("name")
            .and_then(Value::as_str)
            .filter(|n| !n.is_empty())
        {
            self.name = Some(n.to_owned());
        }
        if let Some(k) = o.get("key").and_then(Value::as_str) {
            self.key = Some(k.to_owned());
        }
        if let Some(v) = o.get("version").and_then(Value::as_u64) {
            self.version = Some(v);
        }
        self.conditions.clear();
        let conds = o
            .get("conditions")
            .and_then(Value::as_array)
            .ok_or_else(|| SearchError::Json("search JSON has no conditions array".into()))?;
        let mut child_note = false;
        for c in conds {
            let s = |k: &str| c.get(k).and_then(Value::as_str).unwrap_or("").to_owned();
            let mut name = s("condition");
            if name == "childNote" {
                name = "note".into();
                child_note = true;
            }
            self.add_condition(&name, &s("operator"), &s("value"))?;
        }
        if child_note {
            self.add_condition("resultLevel", "item", "")?;
        }
        if let Some(d) = o.get("deleted") {
            self.deleted = d.as_bool().unwrap_or(false);
        }
        Ok(())
    }

    /// Run the search (`search()`, search.js:573): the keys of the matching
    /// items, sorted.
    pub fn run(&self, lib: &SearchLibrary) -> Result<Vec<String>, SearchError> {
        let rows = self.run_rows(lib, &mut Vec::new())?;
        let mut keys: Vec<String> = rows.into_iter().map(|i| lib.key_of(i).to_owned()).collect();
        keys.sort();
        Ok(keys)
    }

    fn run_rows(
        &self,
        lib: &SearchLibrary,
        stack: &mut Vec<String>,
    ) -> Result<RowSet, SearchError> {
        let built = self.build(lib, stack)?;
        let mut ids = built.rows;

        // Flags as search() reads them (search.js:589-613).
        let mut ipc = false;
        let mut ip = false;
        let mut ic = false;
        for c in &self.conditions {
            match c.condition.as_str() {
                "includeParentsAndChildren" if c.operator == Operator::True => ipc = true,
                "includeParents" if c.operator == Operator::True => ip = true,
                "includeChildren" if c.operator == Operator::True => ic = true,
                _ => {}
            }
        }

        // Scope (search.js:616-667).
        if let Some(scope) = &self.scope {
            let scope_ids = scope.search.run_rows(lib, stack)?;
            let mut allowed = scope_ids.clone();
            if scope.include_children {
                for (i, r) in lib.rows.iter().enumerate() {
                    let child_of_scope = (r.is_attachment()
                        && r.parent.is_some_and(|p| scope_ids.contains(&p)))
                        || (r.in_item_notes
                            && r.note_parent.is_some_and(|p| scope_ids.contains(&p)));
                    let annot = r.is_annotation()
                        && r.parent.is_some_and(|a| {
                            scope_ids.contains(&a)
                                || (lib.rows[a].is_attachment()
                                    && lib.rows[a].parent.is_some_and(|g| scope_ids.contains(&g)))
                        });
                    if child_of_scope || annot {
                        allowed.insert(i);
                    }
                }
            }
            ids = ids.intersection(&allowed).copied().collect();
        }

        // Top-level fullTextContent post-filter (search.js:689-776).
        let any = built.join_mode_any;
        let mut full: Option<RowSet> = None;
        for (ci, c) in self.conditions.iter().enumerate() {
            if c.condition != "fulltextContent" || built.eager.contains(&ci) {
                continue;
            }
            let acc = full.get_or_insert_with(|| {
                if any && built.has_primary {
                    ids.clone()
                } else {
                    RowSet::new()
                }
            });
            let scope_ids: RowSet = if any {
                lib.all().difference(acc).copied().collect()
            } else {
                ids.clone()
            };
            let regexp = c.mode.as_deref().is_some_and(|m| m.starts_with("regexp"));
            let index = if regexp {
                None
            } else {
                find_items_with_content(lib, &c.value, Some(&scope_ids))
            };
            let content = match index {
                Some(m) => m,
                None => find_text_in_items(lib, &scope_ids, &c.value, c.mode.as_deref()),
            };
            let filtered: RowSet = scope_ids
                .iter()
                .copied()
                .filter(|i| {
                    if content.contains(i) {
                        c.operator == Operator::Contains
                    } else {
                        c.operator == Operator::DoesNotContain
                    }
                })
                .collect();
            if any {
                acc.extend(filtered.iter().copied());
                ids = ids.difference(&filtered).copied().collect();
            } else {
                ids = filtered.clone();
                *acc = filtered;
            }
        }
        if let Some(f) = full {
            ids = f;
        }

        // Parents/children of post-filtered results (search.js:778-817).
        if self.has_post_search_filter() && (ipc || ip || ic) {
            let mut add = RowSet::new();
            if ipc || ip {
                for &i in &ids {
                    let r = &lib.rows[i];
                    if r.is_attachment() {
                        if let Some(p) = r.parent {
                            add.insert(p);
                        }
                    }
                    if r.in_item_notes {
                        if let Some(p) = r.note_parent {
                            add.insert(p);
                        }
                    }
                }
            }
            if ipc || ic {
                for (j, r) in lib.rows.iter().enumerate() {
                    if (r.is_attachment() && r.parent.is_some_and(|p| ids.contains(&p)))
                        || (r.in_item_notes && r.note_parent.is_some_and(|p| ids.contains(&p)))
                    {
                        add.insert(j);
                    }
                }
            }
            ids.extend(add);
        }
        Ok(ids)
    }

    /// `_fullTextContentMatches` (search.js:972).
    fn full_text_content_matches(
        &self,
        lib: &SearchLibrary,
        value: &str,
        mode: Option<&str>,
        stack: &mut Vec<String>,
    ) -> Result<RowSet, SearchError> {
        let mut scope_ids: Option<RowSet> = None;
        if let Some(scope) = &self.scope {
            let mut s = Search::new();
            s.scope = Some(Arc::new(SearchScope {
                search: scope.search.clone(),
                include_children: true,
            }));
            let ids = s.run_rows(lib, stack)?;
            if ids.is_empty() {
                return Ok(RowSet::new());
            }
            scope_ids = Some(ids);
        }
        if !mode.is_some_and(|m| m.starts_with("regexp")) {
            if let Some(m) = find_items_with_content(lib, value, scope_ids.as_ref()) {
                return Ok(m);
            }
        }
        let scope = match scope_ids {
            Some(s) => s,
            None => {
                let ids = Search::new().run_rows(lib, stack)?;
                if ids.is_empty() {
                    return Ok(RowSet::new());
                }
                ids
            }
        };
        Ok(find_text_in_items(lib, &scope, value, mode))
    }

    /// `_buildQuery` (search.js:1017).
    fn build(&self, lib: &SearchLibrary, stack: &mut Vec<String>) -> Result<BuildOut, SearchError> {
        let mut to_process: Vec<(Option<usize>, Condition)> = self
            .conditions
            .iter()
            .cloned()
            .enumerate()
            .map(|(i, c)| (Some(i), c))
            .collect();

        // Top-level joinMode and resultLevel (search.js:1044-1065).
        let mut join_mode_any = false;
        let mut result_level = Level::Any;
        {
            let mut depth = 0i32;
            let mut found = false;
            for (_, c) in &to_process {
                match c.condition.as_str() {
                    "groupStart" => depth += 1,
                    "groupEnd" => depth -= 1,
                    "joinMode" if depth == 0 && !found => {
                        join_mode_any = c.operator == Operator::Any;
                        found = true;
                    }
                    "resultLevel" if depth == 0 => {
                        result_level = Level::from_operator(c.operator).unwrap_or(Level::Any);
                    }
                    _ => {}
                }
            }
        }

        let mut flags = Flags::default();
        let mut pending: Vec<Pending> = Vec::new();
        let mut eager: Vec<usize> = Vec::new();
        let mut has_primary = false;
        let mut loop_depth = 0i32;
        let mut last_leaf: Option<usize> = None;
        let mut idx = 0;
        while idx < to_process.len() {
            let (orig, cond) = to_process[idx].clone();
            idx += 1;
            let name = cond.condition.as_str();
            let kind =
                resolve(name).ok_or_else(|| SearchError::InvalidCondition(name.to_owned()))?;
            if kind.has_table() || matches!(kind, Kind::SavedSearch | Kind::TempTable) {
                // Inline-filter merging (search.js:1077-1088). Upstream merges
                // any two 'is*' operators; only 'is'/'isNot' are merged here,
                // because a merged isLessThan/isGreaterThan value becomes an
                // array upstream binds as one int (a defect).
                if let Some(li) = last_leaf {
                    if let Pending::Leaf(l) = &mut pending[li] {
                        if l.name == name
                            && l.operator == cond.operator
                            && matches!(cond.operator, Operator::Is | Operator::IsNot)
                            && kind.inline_filter()
                        {
                            l.values.push(cond.value.clone());
                            continue;
                        }
                    }
                }
                pending.push(Pending::Leaf(Leaf {
                    kind,
                    name: name.to_owned(),
                    operator: cond.operator,
                    values: vec![cond.value.clone()],
                }));
                last_leaf = Some(pending.len() - 1);
                has_primary = true;
                continue;
            }
            let t = cond.operator == Operator::True;
            match kind {
                Kind::Deleted => flags.deleted = t,
                Kind::IncludeDeleted => flags.include_deleted = t,
                Kind::NoChildren => flags.no_children = t,
                Kind::IncludeParentsAndChildren => flags.include_parents_and_children = t,
                Kind::IncludeParents => flags.include_parents = t,
                Kind::IncludeChildren => flags.include_children = t,
                Kind::Unfiled => flags.unfiled = t,
                Kind::Retracted => flags.retracted = t,
                Kind::Publications => flags.publications = t,
                Kind::Feed => flags.feed = t,
                Kind::Recursive => flags.recursive = t,
                Kind::FulltextContent => {
                    if loop_depth > 0 || result_level != Level::Any {
                        if let Some(o) = orig {
                            eager.push(o);
                        }
                        pending.push(Pending::FullText {
                            operator: cond.operator,
                            value: cond.value.clone(),
                            mode: cond.mode.clone(),
                        });
                        has_primary = true;
                    }
                }
                Kind::JoinMode => {
                    last_leaf = None;
                    pending.push(Pending::Marker(Built::JoinMode(cond.operator)));
                }
                Kind::ResultLevel => {
                    last_leaf = None;
                    let l = Level::from_operator(cond.operator).unwrap_or(Level::Any);
                    pending.push(Pending::Marker(Built::ResultLevel(l)));
                }
                Kind::GroupStart => {
                    last_leaf = None;
                    loop_depth += 1;
                    pending.push(Pending::Marker(Built::GroupStart));
                }
                Kind::GroupEnd => {
                    last_leaf = None;
                    loop_depth -= 1;
                    pending.push(Pending::Marker(Built::GroupEnd));
                }
                Kind::AnyField | Kind::TitleCreatorYear => {
                    // search.js:1214-1265: spliced in after this condition.
                    let op = cond.operator;
                    let join = if matches!(op, Operator::IsNot | Operator::DoesNotContain) {
                        Operator::All
                    } else {
                        Operator::Any
                    };
                    let fields: &[&str] = if kind == Kind::AnyField {
                        &[
                            "field",
                            "tag",
                            "note",
                            "annotationText",
                            "annotationComment",
                            "creator",
                        ]
                    } else {
                        &[
                            "title",
                            "publicationTitle",
                            "shortTitle",
                            "court",
                            "year",
                            "citationKey",
                            "creator",
                        ]
                    };
                    let mk = |c: &str, o: Operator, v: &str| {
                        (
                            None,
                            Condition {
                                condition: c.to_owned(),
                                mode: None,
                                operator: o,
                                value: v.to_owned(),
                            },
                        )
                    };
                    let mut ins = vec![
                        mk("groupStart", Operator::True, ""),
                        mk("joinMode", join, ""),
                    ];
                    for f in fields {
                        ins.push(mk(f, op, &cond.value));
                    }
                    ins.push(mk("groupEnd", Operator::True, ""));
                    for (k, c) in ins.into_iter().enumerate() {
                        to_process.insert(idx + k, c);
                    }
                }
                _ => {
                    return Err(SearchError::Unsupported(format!(
                        "special condition '{name}' outside addCondition"
                    )))
                }
            }
        }

        // Base filters (search.js:1272-1342).
        let deleted_items = lib.deleted_items();
        let mut base: RowSet = if flags.include_deleted {
            lib.all()
        } else if flags.deleted {
            deleted_items.clone()
        } else {
            lib.all().difference(&deleted_items).copied().collect()
        };
        if flags.no_children {
            base.retain(|&i| {
                let r = &lib.rows[i];
                !((r.in_item_notes && r.note_parent.is_some())
                    || (r.is_attachment() && r.parent.is_some())
                    || (r.is_annotation() && r.parent.is_some()))
            });
        }
        if flags.unfiled {
            base.retain(|&i| {
                let r = &lib.rows[i];
                let in_live_collection = r
                    .item
                    .collections
                    .iter()
                    .any(|k| lib.collection(k).is_some_and(|c| c.deleted != Some(true)));
                !(in_live_collection
                    || (r.is_attachment() && r.parent.is_some())
                    || (r.in_item_notes && r.note_parent.is_some())
                    || r.is_annotation()
                    || r.item.in_publications == Some(true))
            });
        }
        if flags.retracted || flags.feed {
            // No retractedItems / feedItems rows in a ZoteroLibrary.
            base.clear();
        }
        if flags.publications {
            base.retain(|&i| lib.rows[i].item.in_publications == Some(true));
        }
        match result_level {
            Level::Item => base.retain(|&i| {
                let r = &lib.rows[i];
                !((r.is_attachment() && r.parent.is_some())
                    || (r.in_item_notes && r.note_parent.is_some())
                    || r.is_annotation())
            }),
            Level::Attachment => base.retain(|&i| lib.rows[i].is_attachment()),
            Level::Note => base.retain(|&i| lib.rows[i].in_item_notes),
            Level::Annotation => base.retain(|&i| lib.rows[i].is_annotation()),
            Level::Any => {}
        }

        if has_primary {
            let mut built: Vec<Built> = Vec::new();
            for p in pending {
                match p {
                    Pending::Marker(m) => built.push(m),
                    Pending::FullText {
                        operator,
                        value,
                        mode,
                    } => {
                        // search.js:1162-1189, 1371-1386: a direct index
                        // subquery (unrestricted by scope) when the term is
                        // index-only, else the materialised matches.
                        let regexp = mode.as_deref().is_some_and(|m| m.starts_with("regexp"));
                        let matches = if !regexp && index_only(&value) {
                            find_items_with_content(lib, &value, None).unwrap_or_default()
                        } else {
                            self.full_text_content_matches(lib, &value, mode.as_deref(), stack)?
                        };
                        let rows = if operator == Operator::DoesNotContain {
                            lib.all().difference(&matches).copied().collect()
                        } else {
                            matches
                        };
                        built.push(Built::Pred(Predicate {
                            rows,
                            levels: vec![Level::Attachment],
                            negate: false,
                        }));
                    }
                    Pending::Leaf(leaf) => {
                        if let Some(p) =
                            self.eval_leaf(lib, &leaf, flags, result_level, &deleted_items, stack)?
                        {
                            built.push(Built::Pred(p));
                        }
                    }
                }
            }
            let exclude_deleted = !flags.include_deleted && !flags.deleted;
            if let Some(c) = combine_conditions(lib, &built, result_level, exclude_deleted) {
                base = base.intersection(&c).copied().collect();
            }
        }

        Ok(BuildOut {
            rows: base,
            join_mode_any,
            has_primary,
            eager,
        })
    }

    /// One condition's predicate (search.js:1389-2054), or `None` when
    /// upstream skips it (an inline filter with no valid value, a saved
    /// search that refers to this one).
    fn eval_leaf(
        &self,
        lib: &SearchLibrary,
        leaf: &Leaf,
        flags: Flags,
        result_level: Level,
        deleted_items: &RowSet,
        stack: &mut Vec<String>,
    ) -> Result<Option<Predicate>, SearchError> {
        let op = leaf.operator;
        let value = leaf.values[0].as_str();
        let unsupported = |what: &str| Err(SearchError::Unsupported(what.to_owned()));
        let mut force_no_results = false;

        // The inner SELECT (search.js:1424 onwards).
        let inner: RowSet = match leaf.kind {
            Kind::LibraryId | Kind::ItemId | Kind::ItemTypeId | Kind::TagId => {
                return unsupported(&format!(
                    "'{}' needs Zotero database ids, which a ZoteroLibrary does not have",
                    leaf.name
                ))
            }
            Kind::TempTable => return unsupported("'tempTable' names a database temporary table"),
            Kind::AnnotationAuthor => {
                return unsupported(
                    "'annotationAuthor' reads groupItems.createdByUserID, which item JSON does not carry",
                )
            }
            Kind::SavedSearch => {
                if self.key.as_deref() == Some(value) {
                    // "references itself -- skipping condition" (search.js:1523)
                    return Ok(None);
                }
                let expands = flags.include_parents_and_children
                    || flags.include_parents
                    || flags.include_children;
                if expands && lib.saved_search(value).is_some() {
                    return unsupported(
                        "'savedSearch' with includeParents/includeChildren: upstream appends UNION to a non-SELECT predicate (search.js:2031)",
                    );
                }
                let rows = match lib.saved_search(value) {
                    None => RowSet::new(), // forceNoResults: itemID IN (0)
                    Some(s) => {
                        if stack.iter().any(|k| k == value) {
                            return Err(SearchError::RecursiveSavedSearch(value.to_owned()));
                        }
                        stack.push(value.to_owned());
                        let r = s.run_rows(lib, stack);
                        stack.pop();
                        let r = r?;
                        if op == Operator::IsNot {
                            lib.all().difference(&r).copied().collect()
                        } else {
                            r
                        }
                    }
                };
                return Ok(Some(Predicate {
                    rows,
                    levels: vec![Level::Item],
                    negate: op.is_negation(),
                }));
            }
            Kind::Collection => match lib.collection(value) {
                None => {
                    force_no_results = true;
                    RowSet::new()
                }
                Some(_) => {
                    let mut keys = vec![value.to_owned()];
                    if flags.recursive {
                        keys.extend(lib.descendant_collections(value));
                    }
                    (0..lib.rows.len())
                        .filter(|&i| lib.rows[i].item.collections.iter().any(|k| keys.contains(k)))
                        .collect()
                }
            },
            Kind::AttachmentStorageType => {
                let modes: &[LinkMode] = match value {
                    "storedFile" => &[LinkMode::ImportedFile, LinkMode::ImportedUrl],
                    "linkedFile" => &[LinkMode::LinkedFile],
                    "webLink" => &[LinkMode::LinkedUrl],
                    _ => {
                        return Err(SearchError::InvalidValue {
                            condition: leaf.name.clone(),
                            value: value.to_owned(),
                        })
                    }
                };
                lib.attachments()
                    .filter(|&i| {
                        lib.rows[i]
                            .item
                            .attachment
                            .as_ref()
                            .and_then(|a| a.link_mode)
                            .is_some_and(|m| modes.iter().any(|x| link_mode_number(*x) == link_mode_number(m)))
                    })
                    .collect()
            }
            Kind::FileTypeId => {
                let ft = FILE_TYPES
                    .iter()
                    .find(|(id, name, _)| id.to_string() == value || *name == value)
                    .ok_or_else(|| SearchError::InvalidValue {
                        condition: leaf.name.clone(),
                        value: value.to_owned(),
                    })?;
                lib.attachments()
                    .filter(|&i| {
                        let ct = lib.rows[i]
                            .item
                            .attachment
                            .as_ref()
                            .and_then(|a| a.content_type.as_deref());
                        ct.is_some_and(|ct| ft.2.iter().any(|p| sql_like(ct, &format!("{p}%"))))
                    })
                    .collect()
            }
            Kind::Note => {
                // search.js:1694: the note index when it can answer the term
                // (always, here: notes are indexed as they are built), which
                // matches the normalised plain text (fulltext.js:2545-2605).
                let term = normalize_for_search(value);
                lib.item_notes()
                    .filter(|&i| {
                        !term.is_empty()
                            && lib.rows[i]
                                .note_text
                                .as_deref()
                                .is_some_and(|t| t.contains(&term))
                    })
                    .collect()
            }
            _ => {
                let test = match ValueTest::build(leaf, lib)? {
                    Some(t) => t,
                    None => return Ok(None),
                };
                self.select_rows(lib, leaf, &test)
            }
        };

        let mut inner = inner;
        if !force_no_results {
            // search.js:2007-2039
            let ipc = flags.include_parents_and_children;
            if ipc || flags.include_parents {
                let child_not_deleted = !(flags.include_deleted || flags.deleted);
                let mut parents = RowSet::new();
                for &i in &inner {
                    if child_not_deleted && deleted_items.contains(&i) {
                        continue;
                    }
                    let r = &lib.rows[i];
                    if r.is_attachment() {
                        if let Some(p) = r.parent {
                            parents.insert(p);
                        }
                    }
                    if r.in_item_notes {
                        if let Some(p) = r.note_parent {
                            parents.insert(p);
                        }
                    }
                }
                let children: RowSet = if ipc || flags.include_children {
                    children_of(lib, &inner)
                } else {
                    RowSet::new()
                };
                inner.extend(parents);
                inner.extend(children);
            } else if flags.include_children {
                let children = children_of(lib, &inner);
                inner.extend(children);
            }
        }

        let negate = op.is_negation();
        let rows = if negate {
            if result_level == Level::Any {
                // search.js:1418-1422: exclude the implausible level.
                let annotation_level = leaf.kind.levels() == [Level::Annotation];
                for (i, r) in lib.rows.iter().enumerate() {
                    if r.is_annotation() != annotation_level {
                        inner.insert(i);
                    }
                }
            }
            lib.all().difference(&inner).copied().collect()
        } else {
            inner
        };
        Ok(Some(Predicate {
            rows,
            levels: leaf.kind.levels(),
            negate,
        }))
    }

    /// Rows of the condition's table whose value passes `test`.
    fn select_rows(&self, lib: &SearchLibrary, leaf: &Leaf, test: &ValueTest) -> RowSet {
        let n = lib.rows.len();
        let direct = lib.deleted_direct();
        let normalized = test.normalized;
        let norm = |s: &str| -> String {
            if normalized {
                normalize_for_search(s)
            } else {
                s.to_owned()
            }
        };
        let mut out = RowSet::new();
        for i in 0..n {
            let r = &lib.rows[i];
            let hit = match leaf.kind {
                Kind::DateAdded => test.matches_text(r.date_added.as_deref(), lib),
                Kind::DateModified => test.matches_text(r.date_modified.as_deref(), lib),
                Kind::LastRead => {
                    r.is_attachment()
                        && test
                            .matches_unix(r.item.attachment.as_ref().and_then(|a| a.last_read), lib)
                }
                Kind::ItemType => test.matches_text(Some(r.item.item_type.as_str()), lib),
                Kind::Key => test.matches_text(Some(&r.key), lib),
                Kind::Tag => r
                    .item
                    .tags
                    .iter()
                    .any(|t| test.matches_text(Some(&norm(&t.tag)), lib)),
                Kind::Creator | Kind::Author | Kind::Editor | Kind::BookAuthor => {
                    let ty = match leaf.kind {
                        Kind::Author => Some("author"),
                        Kind::Editor => Some("editor"),
                        Kind::BookAuthor => Some("bookAuthor"),
                        _ => None,
                    };
                    r.creators.iter().any(|(t, first, last)| {
                        ty.is_none_or(|ty| t == ty) && {
                            let v = format!("{} {}", norm(first), norm(last));
                            test.matches_text(Some(v.trim_matches(' ')), lib)
                        }
                    })
                }
                Kind::LastName => r
                    .creators
                    .iter()
                    .any(|(_, _, last)| test.matches_text(Some(&norm(last)), lib)),
                Kind::NumTags => test.matches_int(r.item.tags.len() as i64),
                Kind::NumNotes | Kind::NumAttachments => {
                    use kovan_common::zotero::ItemType as T;
                    if matches!(r.item.item_type, T::Note | T::Attachment | T::Annotation) {
                        false
                    } else {
                        let count = lib
                            .rows
                            .iter()
                            .enumerate()
                            .filter(|(j, c)| {
                                !direct.contains(j)
                                    && if leaf.kind == Kind::NumNotes {
                                        c.in_item_notes && c.note_parent == Some(i)
                                    } else {
                                        c.is_attachment() && c.parent == Some(i)
                                    }
                            })
                            .count();
                        test.matches_int(count as i64)
                    }
                }
                Kind::NumAnnotations => {
                    use kovan_common::zotero::ItemType as T;
                    if matches!(r.item.item_type, T::Note | T::Annotation) {
                        false
                    } else {
                        let atts: Vec<usize> = lib
                            .attachments()
                            .filter(|&a| {
                                (a == i || lib.rows[a].parent == Some(i)) && !direct.contains(&a)
                            })
                            .collect();
                        let count = lib
                            .annotations()
                            .filter(|&x| {
                                !direct.contains(&x)
                                    && lib.rows[x].parent.is_some_and(|p| atts.contains(&p))
                            })
                            .count();
                        test.matches_int(count as i64)
                    }
                }
                Kind::AnnotationText
                | Kind::AnnotationComment
                | Kind::AnnotationType
                | Kind::AnnotationColor => {
                    r.is_annotation() && {
                        let a = r.item.annotation.as_ref();
                        let v: Option<String> = match leaf.kind {
                            Kind::AnnotationText => a.and_then(|a| a.text.as_deref()).map(norm),
                            Kind::AnnotationComment => {
                                a.and_then(|a| a.comment.as_deref()).map(norm)
                            }
                            Kind::AnnotationType => a
                                .and_then(|a| a.annotation_type)
                                .map(|t| annotation_type_number(t).to_string()),
                            _ => a.and_then(|a| a.color.clone()),
                        };
                        test.matches_text(v.as_deref(), lib)
                    }
                }
                Kind::Field(f) | Kind::DateField(f) | Kind::NumberField(f) => {
                    let allowed: Option<Vec<Field>> = f.map(|f| {
                        let mut v = vec![f];
                        v.extend(type_fields_from_base(f));
                        v
                    });
                    r.item_data.iter().any(|(field, val)| {
                        allowed.as_ref().is_none_or(|a| a.contains(field))
                            && test.matches_field(&norm(val), *field, leaf.kind, lib)
                    })
                }
                Kind::Year => {
                    let mut allowed = vec![Field::Date];
                    allowed.extend(type_fields_from_base(Field::Date));
                    r.item_data.iter().any(|(field, val)| {
                        allowed.contains(field) && test.matches_text(Some(&substr(val, 1, 4)), lib)
                    })
                }
                _ => false,
            };
            if hit {
                out.insert(i);
            }
        }
        out
    }
}

fn children_of(lib: &SearchLibrary, set: &RowSet) -> RowSet {
    lib.rows
        .iter()
        .enumerate()
        .filter(|(_, r)| {
            (r.is_attachment() && r.parent.is_some_and(|p| set.contains(&p)))
                || (r.in_item_notes && r.note_parent.is_some_and(|p| set.contains(&p)))
        })
        .map(|(i, _)| i)
        .collect()
}

struct BuildOut {
    rows: RowSet,
    join_mode_any: bool,
    has_primary: bool,
    eager: Vec<usize>,
}

/// What a date condition compares (search.js:1779-1790).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DateExpr {
    /// `DATE(field, 'localtime')` (dateAdded, dateModified, accessDate).
    LocalDate,
    /// `DATE(field, 'unixepoch', 'localtime')` (lastRead).
    UnixLocal,
    /// `SUBSTR(field, 1, 10)` (the SQL part of a multipart date).
    Substr10,
}

#[derive(Debug, Clone)]
enum DateCmp {
    Like(String),
    Before(String),
    After(String),
    InTheLast(Option<String>),
}

#[derive(Debug, Clone)]
struct DateTest {
    expr: DateExpr,
    cmp: Option<DateCmp>,
    parts: Vec<String>,
    never: bool,
}

#[derive(Debug, Clone)]
enum Cmp {
    /// `isEmpty`/`isNotEmpty`: any row with a value (search.js:1717).
    Always,
    Like(String),
    EqNoCase(String),
    /// Inline-filter values compared with `=`/`IN` (search.js:1930).
    Inline(Vec<String>),
    NumLt(i64),
    NumGt(i64),
    StrLt(String),
    StrGt(String),
    Date(DateTest),
}

/// The value comparison of one condition, built once.
#[derive(Debug, Clone)]
struct ValueTest {
    cmp: Cmp,
    /// Compare the normalised column (the caller normalises the text).
    normalized: bool,
}

fn is_date_kind(k: Kind) -> bool {
    matches!(
        k,
        Kind::DateAdded | Kind::DateModified | Kind::LastRead | Kind::DateField(_)
    )
}

impl ValueTest {
    /// `None` when an inline filter leaves no value (search.js:1942).
    fn build(leaf: &Leaf, lib: &SearchLibrary) -> Result<Option<ValueTest>, SearchError> {
        let op = leaf.operator;
        let value = leaf.values[0].as_str();
        let invalid = || SearchError::InvalidValue {
            condition: leaf.name.clone(),
            value: value.to_owned(),
        };
        if matches!(op, Operator::IsEmpty | Operator::IsNotEmpty) {
            return Ok(Some(ValueTest {
                cmp: Cmp::Always,
                normalized: false,
            }));
        }
        if is_date_kind(leaf.kind)
            && matches!(
                op,
                Operator::Is
                    | Operator::IsNot
                    | Operator::IsBefore
                    | Operator::IsAfter
                    | Operator::IsInTheLast
            )
            && !is_sql_date_time(value)
        {
            let expr = match leaf.kind {
                Kind::LastRead => DateExpr::UnixLocal,
                Kind::DateAdded | Kind::DateModified | Kind::DateField(Some(Field::AccessDate)) => {
                    DateExpr::LocalDate
                }
                _ => DateExpr::Substr10,
            };
            let clock = lib.clock;
            let mut t = DateTest {
                expr,
                cmp: None,
                parts: Vec::new(),
                never: false,
            };
            if op == Operator::IsInTheLast {
                t.cmp = Some(DateCmp::InTheLast(date_now_minus(
                    &format!("-{value}"),
                    &clock,
                )));
            } else {
                let (alt, freeform) = match op {
                    Operator::Is | Operator::IsNot => ("__", true),
                    Operator::IsBefore => ("00", false),
                    _ => ("__", false),
                };
                let lc = value.to_lowercase();
                let v = match lc.as_str() {
                    "yesterday" => local_today(&clock, -1),
                    "today" => local_today(&clock, 0),
                    "tomorrow" => local_today(&clock, 1),
                    _ => value.to_owned(),
                };
                let opts = date_options(&clock);
                let dp = str_to_date(&v, &opts);
                let mut sqldate = match &dp.year {
                    Some(y) if !y.is_empty() => kovan_common::zotero::date::lpad(y, '0', 4),
                    _ => "____".to_owned(),
                };
                sqldate.push('-');
                match dp.month {
                    Some(m) => sqldate.push_str(&format!("{:02}", m + 1)),
                    None => sqldate.push_str(alt),
                }
                sqldate.push('-');
                match dp.day.filter(|d| *d != 0) {
                    Some(d) => sqldate.push_str(&format!("{d:02}")),
                    None => sqldate.push_str(alt),
                }
                let mut go = false;
                if sqldate != "____-__-__" {
                    go = true;
                    t.cmp = Some(match op {
                        Operator::Is | Operator::IsNot => DateCmp::Like(sqldate),
                        Operator::IsBefore => DateCmp::Before(sqldate),
                        _ => DateCmp::After(sqldate),
                    });
                }
                if freeform {
                    if let Some(p) = dp.part.as_deref().filter(|p| !p.is_empty()) {
                        go = true;
                        t.parts = p.split(' ').map(str::to_owned).collect();
                    }
                }
                if !go {
                    t.never = true;
                }
            }
            return Ok(Some(ValueTest {
                cmp: Cmp::Date(t),
                normalized: false,
            }));
        }

        let use_norm = leaf.kind.normalized()
            && matches!(
                op,
                Operator::Contains
                    | Operator::DoesNotContain
                    | Operator::BeginsWith
                    | Operator::Is
                    | Operator::IsNot
            );
        let sv = if use_norm {
            normalize_for_search(value)
        } else {
            value.to_owned()
        };
        let cmp = match op {
            Operator::Contains | Operator::DoesNotContain => Cmp::Like(format!("%{sv}%")),
            Operator::Is | Operator::IsNot => {
                if leaf.kind.inline_filter() {
                    let vals: Vec<String> = leaf
                        .values
                        .iter()
                        .filter(|v| {
                            if leaf.kind == Kind::Key {
                                is_valid_object_key(v)
                            } else {
                                !v.is_empty() && v.chars().all(|c| c.is_ascii_digit())
                            }
                        })
                        .cloned()
                        .collect();
                    if vals.is_empty() {
                        return Ok(None);
                    }
                    Cmp::Inline(vals)
                } else if !value.is_empty()
                    && value.starts_with(|c: char| ('1'..='9').contains(&c))
                    && value.chars().all(|c| c.is_ascii_digit())
                {
                    Cmp::Like(sv)
                } else {
                    Cmp::EqNoCase(sv)
                }
            }
            Operator::BeginsWith => Cmp::Like(format!("{sv}%")),
            Operator::IsLessThan => Cmp::NumLt(sqlite_cast_int(value)),
            Operator::IsGreaterThan => Cmp::NumGt(sqlite_cast_int(value)),
            Operator::IsBefore => Cmp::StrLt(value.to_owned()),
            Operator::IsAfter => Cmp::StrGt(value.to_owned()),
            _ => return Err(invalid()),
        };
        Ok(Some(ValueTest {
            cmp,
            normalized: use_norm,
        }))
    }

    /// A text column (`None` = SQL NULL, which no comparison matches).
    fn matches_text(&self, v: Option<&str>, lib: &SearchLibrary) -> bool {
        let Some(v) = v else { return false };
        match &self.cmp {
            Cmp::Always => true,
            Cmp::Like(p) => sql_like(v, p),
            Cmp::EqNoCase(s) => nocase_eq(v, s),
            Cmp::Inline(vals) => vals.iter().any(|x| x == v),
            Cmp::NumLt(n) => is_canonical_int(v) && sqlite_cast_int(v) < *n,
            Cmp::NumGt(n) => is_canonical_int(v) && sqlite_cast_int(v) > *n,
            Cmp::StrLt(s) => v < s.as_str(),
            Cmp::StrGt(s) => v > s.as_str(),
            Cmp::Date(d) => d.matches(v, v, lib),
        }
    }

    /// An integer column (the `COUNT(*)` subqueries).
    fn matches_int(&self, n: i64) -> bool {
        match &self.cmp {
            Cmp::Always => true,
            Cmp::Inline(vals) => vals.iter().any(|x| x.parse::<i64>() == Ok(n)),
            Cmp::NumLt(m) => n < *m,
            Cmp::NumGt(m) => n > *m,
            Cmp::Like(p) => sql_like(&n.to_string(), p),
            Cmp::EqNoCase(s) => n.to_string() == *s,
            _ => false,
        }
    }

    /// `lastRead` (Unix seconds).
    fn matches_unix(&self, v: Option<i64>, lib: &SearchLibrary) -> bool {
        let Some(v) = v else { return false };
        match &self.cmp {
            Cmp::Date(d) => {
                let date = date_of_unix_local(v, &lib.clock);
                d.matches_expr(Some(date), &v.to_string())
            }
            _ => self.matches_text(Some(&v.to_string()), lib),
        }
    }

    /// An `itemData` value.
    fn matches_field(&self, v: &str, field: Field, kind: Kind, lib: &SearchLibrary) -> bool {
        match &self.cmp {
            Cmp::Date(d) => {
                // The expression is fixed by the condition (search.js:1782):
                // DATE(...,'localtime') only for the accessDate alias.
                let _ = (field, kind);
                d.matches(v, v, lib)
            }
            _ => self.matches_text(Some(v), lib),
        }
    }
}

impl DateTest {
    fn matches(&self, raw: &str, field_text: &str, lib: &SearchLibrary) -> bool {
        let e = match self.expr {
            DateExpr::LocalDate => date_of_sql_local(raw, &lib.clock),
            DateExpr::Substr10 => Some(substr(raw, 1, 10)),
            DateExpr::UnixLocal => Some(date_of_unix_local(sqlite_cast_int(raw), &lib.clock)),
        };
        self.matches_expr(e, field_text)
    }

    /// Evaluate `EXPR <cmp> [AND guard] [AND SUBSTR(field,12,100) LIKE ...]`.
    fn matches_expr(&self, e: Option<String>, field_text: &str) -> bool {
        if self.never {
            return false; // EXPR=0
        }
        let Some(e) = e else { return false };
        let main = match &self.cmp {
            Some(DateCmp::Like(p)) => sql_like(&e, p),
            Some(DateCmp::Before(s)) => {
                e.as_str() < s.as_str() && substr(field_text, 1, 10).as_str() > "0000-00-00"
            }
            Some(DateCmp::After(s)) => e.as_str() > s.as_str(),
            Some(DateCmp::InTheLast(s)) => s.as_ref().is_some_and(|s| e.as_str() > s.as_str()),
            // Only freeform parts: the bare expression is the WHERE operand,
            // true when its numeric value is non-zero.
            None => sqlite_cast_int(&e) != 0,
        };
        main && self
            .parts
            .iter()
            .all(|p| sql_like(&substr(field_text, 12, 100), &format!("%{p}%")))
    }
}
