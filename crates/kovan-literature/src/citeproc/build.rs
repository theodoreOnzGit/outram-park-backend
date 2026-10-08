// Part of the kovan port of citeproc-js (GitHub #790, #792).
//
// Upstream:    citeproc-js, https://github.com/juris-m/citeproc-js
// Source:      src/build.js (all but CSL.Engine.prototype.retrieveItem, which is build_retrieve_item.rs),
//              src/util_datenode.js (CSL.Util.fixDateNode, as a private STUB)
// Version:     2.4.63, commit 73bc1b44bc7d54d0bfec4e070fd27f5efe024ff9
// Copyright:   (c) 2009-2019 Frank Bennett
// Licence:     AGPL-3.0, taken from upstream's "CPAL-1.0 or AGPL-3.0-or-later"
//              (LICENSE at the commit above; see this crate's NOTICE).
// Modified:    2026-10-08, by the OUTRAM PARK contributors. This file is a
//              Rust translation (port) of the file named above, modified
//              from the original.
// No warranty: this program is distributed in the hope that it will be
//              useful, but WITHOUT ANY WARRANTY; without even the implied
//              warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR
//              PURPOSE. See the GNU Affero General Public License.

//! The `CSL.Engine` constructor and the style-building core (src/build.js):
//! [`State::new`] (`new CSL.Engine(sys, style, lang, forceLang)`),
//! `setCloseQuotesArray`, `makeBuilder`, `buildTokenLists`,
//! `setStyleAttributes`, `getTerm`, `getDate`, `getOpt`, `getVariable`,
//! `getDateNum`, `CSL.Engine.getField`, `configureTokenLists`,
//! `configureTokenList`, `refetchItems`, `refetchItem`, `setOpt`, `inheritOpt`.
//!
//! `CSL.Engine.prototype.retrieveItem` (build.js 480-735) is
//! `build_retrieve_item.rs`.
//!
//! # Construction, in upstream's order
//!
//! 1. state objects (`opt`, `tmp`, `build`, `fun`, `configure`, the five
//!    areas, the two queues) with their defaults ([`super::state`]);
//! 2. `cslXml = setupXml(style)`; the boolean `sys` options (`SYS_OPTIONS`)
//!    override `opt.development_extensions`; `csl_reverse_lookup_support`
//!    numbers every node (`cslid`);
//! 3. XML preprocessing: `addMissingNameNodes`, `addInstitutionNodes`,
//!    `insertPublisherAndPlace`, `flagDateMacros`, a default
//!    `sort-separator`;
//! 4. `setStyleAttributes` (the `<style>` element's attributes through
//!    `CSL.Attributes`), `opt.class`/`styleID`/`styleName`, language
//!    selection (`localeResolve`, `localeConfigure`), the skip-words regexp;
//! 5. `buildTokenLists` for `citation`, `bibliography`, `intext`, then
//!    `configureTokenLists`;
//! 6. the helper objects (`dateparser`, `flipflopper`, the ordinalizers, the
//!    page and year manglers) and `setOutputFormat("html")`.
//!
//! # PORT-LATER seams (other agents' files)
//!
//! * `new CSL.Output.Queue.adjust(getOpt('punctuation-in-quote'))` (queue.js);
//! * `fun.ordinalizer.init(state)`, `long_ordinalizer.init(state)`
//!   (util_number.js) and `PageRangeMangler.getFunction` (util_page.js);
//! * `new CSL.Registry`, `new CSL.Disambiguation`, `new CSL.Parallel`,
//!   `new CSL.Util.FlipFlopper` (their defaults are used);
//! * `CSL.Util.fixDateNode` (util_datenode.js, wave1-nodes) is a faithful
//!   private STUB here, [`State::fix_date_node`], so style building can be
//!   checked; the integrator drops it for wave1-nodes's.

use std::sync::LazyLock;

use regex::Regex;
use serde_json::Value;

use super::js::{self, Obj};
use super::load;
use super::obj_token::{Token, TokenType};
use super::state::{new_opt, Area, Build, Configure, Fun, State, Tmp};
use super::util_locale::{locale_resolve, regexp_value};
use super::util_nodes;
use super::xmljson::{NodeId, XmlChild, XmlTree};
use super::{system, CslResult, EngineError, Sys};

fn type_error(what: &str) -> EngineError {
    EngineError::Csl(format!("TypeError: {what}"))
}

/// The value of `sort_sep`: `'dale|'.localeCompare('daleb', locale) > -1`
/// selects `"@"`, otherwise `"|"`. ICU's root collation puts the symbol `|`
/// before letters, so `'dale|'` sorts before `'daleb'`, the comparison is
/// `-1` and the answer is `"|"` for every locale citeproc-js was run with.
///
/// PORT-LATER(collation, #795): call `js::locale_compare` once the
/// provisional collator orders punctuation before letters.
fn sort_sep(_default_locale_sort: &str) -> &'static str {
    "|"
}

impl State {
    /// `new CSL.Engine(sys, style, lang, forceLang)`: build the engine's
    /// state and compile `style` (CSL XML or serialized JSON) for `lang`
    /// (`""` for none). See the module docs for the sequence.
    pub fn new(sys: Sys, style: &str, lang: &str, force_lang: bool) -> CslResult<State> {
        let mut s = State {
            processor_version: load::PROCESSOR_VERSION.to_string(),
            csl_version: "1.0".to_string(),
            ..State::default()
        };
        s.variable_wrapper_prepunct = js::truthy_opt(sys.options.get("variableWrapper"));
        // `this.sys.AbbreviationSegments = CSL.AbbreviationSegments`,
        // `CSL.stringCompare = this.sys.stringCompare`: no counterpart.
        s.sys = sys;

        s.transform = Default::default();
        s.opt = new_opt();
        s.tmp = Tmp::new();
        s.build = Build::new();
        s.fun = Fun::new();

        s.configure = Configure::new();
        // Build citation before citation_sort in order to pick up
        // state.opt.update_mode, needed it determine whether
        // a grouped sort should be performed.
        s.citation_sort = Area::new_citation_sort();
        s.bibliography_sort = Area::new_bibliography_sort();
        s.citation = Area::new_citation();
        s.bibliography = Area::new_bibliography();
        s.intext = Area::new_intext();

        s.output = Default::default();
        //
        // This latter queue is used for formatting date chunks
        // before they are folded back into the main queue.
        //
        s.dateput = Default::default();

        s.csl_xml = system::setup_xml(style)?;

        for option in load::SYS_OPTIONS {
            if let Some(Value::Bool(b)) = s.sys.options.get(*option) {
                let b = *b;
                s.dev_ext_set(option, Value::Bool(b));
            }
        }
        if s.dev_ext("uppercase_subtitles") || s.dev_ext("implicit_short_title") {
            s.dev_ext_set("main_title_from_short_title", Value::Bool(true));
        }
        let root = s
            .csl_xml
            .data_obj
            .ok_or_else(|| type_error("Cannot read properties of undefined (reading 'name')"))?;
        if s.dev_ext("csl_reverse_lookup_support") {
            s.build.csl_node_id = 0;
            s.set_csl_node_ids(root, "style");
        }
        // Preprocessing ops for the XML input
        s.csl_xml.add_missing_name_nodes(root, &mut Vec::new());
        s.csl_xml.add_institution_nodes(root)?;
        s.csl_xml.insert_publisher_and_place(root)?;
        s.csl_xml.flag_date_macros(root);
        let attrs = s.csl_xml.attributes(root);
        if !attrs.iter().any(|(k, _)| k == "@sort-separator") {
            s.csl_xml
                .set_attribute(root, "sort-separator", Value::String(", ".to_string()));
        }
        // This setting does the right thing and seems not to be side-effects
        s.opt
            .insert("initialize-with-hyphen".into(), Value::Bool(true));

        // Locale resolution
        //
        // (1) Get three locale strings
        //     -- default-locale (stripped)
        //     -- processor-locale
        //     -- en_US

        s.set_style_attributes()?;

        let xclass = s.csl_xml.get_attribute_value(root, "class");
        s.opt.insert("xclass".into(), xclass.clone());
        s.opt.insert("class".into(), xclass);
        match s.csl_xml.get_style_id(root, false) {
            Some(v) => {
                s.opt.insert("styleID".into(), v);
            }
            None => {
                s.opt.remove("styleID");
            }
        }
        match s.csl_xml.get_style_id(root, true) {
            Some(v) => {
                s.opt.insert("styleName".into(), v);
            }
            None => {
                s.opt.remove("styleName");
            }
        }

        // `this.opt.version.slice(0,4)`: the `@version` handler sets
        // `opt.version`. DEVIATION: upstream throws a TypeError for a style
        // with no version attribute; this port reads it as empty (the
        // foundation's `Engine::new(sys, "<style/>", ..)` tests rely on it).
        let version = s
            .opt
            .get("version")
            .map(js::to_js_string)
            .unwrap_or_default();
        if js::slice(&version, 0, Some(4)) == "1.1m" {
            for k in [
                "consolidate_legal_items",
                "consolidate_container_items",
                "main_title_from_short_title",
                "expect_and_symbol_form",
                "require_explicit_legal_case_title_short",
                "force_jurisdiction",
                "force_title_abbrev_fallback",
            ] {
                s.dev_ext_set(k, Value::Bool(true));
            }
        }
        // We seem to have two language specs flying around:
        //   this.opt["default-locale"], and this.opt.lang
        // Keeping them aligned for safety's sake, pending
        // eventual cleanup.
        let mut lang: Option<String> = if lang.is_empty() {
            None
        } else {
            let l = lang.replacen('_', "-", 1);
            load::normalize_locale_str(&l)
        };
        let mut default_locale: Vec<String> = match s.opt.get("default-locale") {
            Some(Value::Array(a)) => a.iter().map(js::to_js_string).collect(),
            _ => Vec::new(),
        };
        if default_locale
            .first()
            .map(|d| !d.is_empty())
            .unwrap_or(false)
        {
            let d = default_locale[0].replacen('_', "-", 1);
            default_locale[0] = load::normalize_locale_str(&d).unwrap_or_default();
        }
        if let (Some(l), true) = (&lang, force_lang) {
            default_locale = vec![l.clone()];
        }
        if lang.is_some()
            && !force_lang
            && default_locale
                .first()
                .map(|d| !d.is_empty())
                .unwrap_or(false)
        {
            lang = Some(default_locale[0].clone());
        }
        if default_locale.is_empty() {
            if lang.is_none() {
                lang = Some("en-US".to_string());
            }
            default_locale.push("en-US".to_string());
        }
        let lang = match lang {
            Some(l) if !l.is_empty() => l,
            _ => default_locale[0].clone(),
        };
        let langspec = locale_resolve(&lang, None);
        s.opt
            .insert("lang".into(), Value::String(langspec.best.clone()));
        default_locale[0] = langspec.best.clone();
        s.opt.insert(
            "default-locale".into(),
            Value::Array(default_locale.iter().cloned().map(Value::String).collect()),
        );
        s.locale = Default::default();
        if !js::truthy_opt(s.opt.get("default-locale-sort")) {
            s.opt.insert(
                "default-locale-sort".into(),
                Value::String(default_locale[0].clone()),
            );
        }
        // Test processor against JS engine locale mess to find a field separator that works
        let dls = js::get_string(&s.opt, "default-locale-sort").unwrap_or_default();
        s.opt
            .insert("sort_sep".into(), Value::String(sort_sep(&dls).to_string()));
        s.locale_configure(&langspec, false)?;

        // Build skip-word regexp
        let lst: Vec<String> = match s
            .locale
            .get(&langspec.best)
            .and_then(|l| l.opts.get("skip-words"))
        {
            Some(Value::Array(a)) => a.iter().map(js::to_js_string).collect(),
            _ => Vec::new(),
        };
        let source = format!(
            "(?:(?:[?!:]*\\s+|-|^)(?:{})(?=[!?:]*\\s+|-|$))",
            lst.join("|")
        );
        if let Some(l) = s.locale.get_mut(&langspec.best) {
            l.opts
                .insert("skip-words-regexp".into(), regexp_value(&source));
        }

        // PORT-LATER(queue): `this.output.adjust = new
        // CSL.Output.Queue.adjust(this.getOpt('punctuation-in-quote'))`
        // (queue.js, wave1-output).
        let _punctuation_in_quote = s.get_opt("punctuation-in-quote")?;

        // PORT-LATER(registry): `this.registry = new CSL.Registry(this)`
        // (registry.js, wave4); the default stands in.
        s.registry = Default::default();
        s.has_registry = true;

        // XXX For modular jurisdiction support, parameterize buildTokenLists().
        // XXX Feed as arguments:
        // XXX * actual node to be walked (cslXml)
        // XXX * actual target array

        s.macros = Default::default();

        for area in ["citation", "bibliography", "intext"] {
            s.build.area = area.to_string();
            let area_nodes = s.csl_xml.get_nodes_by_name(s.csl_xml.data_obj, area, "");
            let mut tokens = std::mem::take(&mut s.area_mut(area).tokens);
            let r = s.build_token_lists(&area_nodes, &mut tokens);
            s.area_mut(area).tokens = tokens;
            r?;
        }

        if s.opt
            .get("parallel")
            .and_then(|p| p.get("enable"))
            .map(js::truthy)
            .unwrap_or(false)
        {
            // PORT-LATER(util_parallel): `new CSL.Parallel(this)`.
            s.parallel = Some(Default::default());
        }

        s.juris = Default::default();

        s.configure_token_lists()?;

        // PORT-LATER(disambig_cites): `new CSL.Disambiguation(this)`.
        s.disambiguate = Default::default();

        s.splice_delimiter = None;

        //
        // date parser
        //
        // PORT-LATER(util_dateparser): `this.fun.dateparser = CSL.DateParser`.
        //
        // flip-flopper for inline markup
        //
        // PORT-LATER(util_flipflop): `new CSL.Util.FlipFlopper(this)`.
        //
        // utility functions for quotes
        //
        s.set_close_quotes_array()?;
        //
        // configure ordinal numbers generator
        //
        // PORT-LATER(util_number): `this.fun.ordinalizer.init(this)`.
        //
        // configure long ordinal numbers generator
        //
        // PORT-LATER(util_number): `this.fun.long_ordinalizer.init(this)`.
        //
        // set up page mangler
        //
        // PORT-LATER(util_page): `this.fun.page_mangler =
        // CSL.Util.PageRangeMangler.getFunction(this, "page")`, and the same
        // for `"year"` (`year_mangler`).

        s.set_output_format("html")?;
        Ok(s)
    }

    /// `opt.development_extensions[name]` as a JS truthiness.
    pub fn dev_ext(&self, name: &str) -> bool {
        self.opt
            .get("development_extensions")
            .and_then(|d| d.get(name))
            .map(js::truthy)
            .unwrap_or(false)
    }

    /// `opt.development_extensions[name] = value`.
    pub fn dev_ext_set(&mut self, name: &str, value: Value) {
        if let Some(Value::Object(d)) = self.opt.get_mut("development_extensions") {
            d.insert(name.to_string(), value);
        }
    }

    /// The area called `name` (`this[name]`): one of `citation`,
    /// `citation_sort`, `bibliography`, `bibliography_sort`, `intext`.
    /// Any other name is a port bug; it gets the citation area.
    pub fn area_mut(&mut self, name: &str) -> &mut Area {
        match name {
            "citation_sort" => &mut self.citation_sort,
            "bibliography" => &mut self.bibliography,
            "bibliography_sort" => &mut self.bibliography_sort,
            "intext" => &mut self.intext,
            _ => &mut self.citation,
        }
    }

    /// `this[name]` (shared).
    pub fn area_ref(&self, name: &str) -> &Area {
        match name {
            "citation_sort" => &self.citation_sort,
            "bibliography" => &self.bibliography,
            "bibliography_sort" => &self.bibliography_sort,
            "intext" => &self.intext,
            _ => &self.citation,
        }
    }

    /// `this.setCslNodeIds(myxml, nodename)` (defined inside the
    /// constructor for `csl_reverse_lookup_support`): number every node of
    /// the style with a `cslid` attribute and record its name in
    /// `opt.nodenames`.
    fn set_csl_node_ids(&mut self, myxml: NodeId, nodename: &str) {
        let children = self.csl_xml.children(myxml);
        let id = self.build.csl_node_id;
        self.csl_xml.set_attribute(myxml, "cslid", Value::from(id));
        if let Some(Value::Array(a)) = self.opt.get_mut("nodenames") {
            a.push(Value::String(nodename.to_string()));
        }
        self.build.csl_node_id += 1;
        for c in children {
            if let XmlChild::Node(n) = c {
                let name = self.csl_xml.nodename(n);
                if !name.is_empty() {
                    self.set_csl_node_ids(n, &name);
                }
            }
        }
    }

    /// `CSL.Engine.prototype.setCloseQuotesArray`: `opt.close_quotes_array =
    /// [close-quote, close-inner-quote, '"', "'"]`.
    pub fn set_close_quotes_array(&mut self) -> CslResult<()> {
        let mut ret: Vec<Value> = Vec::new();
        for t in ["close-quote", "close-inner-quote"] {
            ret.push(
                self.get_term(t, None, None, None, None, false)?
                    .map(Value::String)
                    .unwrap_or(Value::Null),
            );
        }
        ret.push(Value::String("\"".to_string()));
        ret.push(Value::String("'".to_string()));
        self.opt
            .insert("close_quotes_array".into(), Value::Array(ret));
        Ok(())
    }

    /// `CSL.makeBuilder(me, target)(nodes)`: walk `nodes` (a list of
    /// children, or a one-element list holding a root) depth first and build
    /// their tokens into `target`.
    ///
    /// Each node with children becomes a START token, the tokens of its
    /// children, and an END token; a node without children is one SINGLETON.
    /// A `<date>` child is first replaced by the locale's date template
    /// ([`State::fix_date_node`]).
    pub fn run_builder(&mut self, nodes: &[XmlChild], target: &mut Vec<Token>) -> CslResult<()> {
        let mut var_stack: Vec<Vec<String>> = Vec::new();
        let mut node_stack: Vec<NodeId> = Vec::new();
        self.build_style(nodes, None, target, &mut var_stack, &mut node_stack)
    }

    /// `buildStyle(nodes, parent, node_stack)` inside `CSL.makeBuilder`.
    fn build_style(
        &mut self,
        nodes: &[XmlChild],
        parent: Option<NodeId>,
        target: &mut Vec<Token>,
        var_stack: &mut Vec<Vec<String>>,
        node_stack: &mut Vec<NodeId>,
    ) -> CslResult<()> {
        for (i, child) in nodes.iter().enumerate() {
            let mut node = child.clone();
            if let XmlChild::Text(s) = &node {
                if s.is_empty() {
                    // me.cslXml.nodename(node) === null
                    continue;
                }
            }
            if let (Some(p), XmlChild::Node(n)) = (parent, &node) {
                if self.csl_xml.nodename(*n) == "date" {
                    self.fix_date_node(p, i, *n)?;
                    if let Some(c) = self.csl_xml.children(p).get(i) {
                        node = c.clone();
                    }
                }
            }
            let XmlChild::Node(n) = node else {
                // children() of a string: "abc".children is undefined.
                return Err(type_error(
                    "Cannot read properties of undefined (reading 'length')",
                ));
            };
            let kids = self.csl_xml.children(n);
            if !kids.is_empty() {
                node_stack.push(n);
                self.xml_to_token(n, TokenType::Start, target, var_stack)?;
                self.build_style(&kids, Some(n), target, var_stack, node_stack)?;
                if let Some(end) = node_stack.pop() {
                    self.xml_to_token(end, TokenType::End, target, var_stack)?;
                }
            } else {
                self.xml_to_token(n, TokenType::Singleton, target, var_stack)?;
            }
        }
        Ok(())
    }

    /// `CSL.Engine.prototype.buildTokenLists(area_nodes, target)`: build the
    /// first of `area_nodes` into `target` (nothing if there is none).
    pub fn build_token_lists(
        &mut self,
        area_nodes: &[NodeId],
        target: &mut Vec<Token>,
    ) -> CslResult<()> {
        // `if (!this.cslXml.getNodeValue(area_nodes)) return;`: the value of
        // a list is the list, always truthy.
        let mynode: Vec<XmlChild> = area_nodes
            .first()
            .map(|n| vec![XmlChild::Node(*n)])
            .unwrap_or_default();
        self.run_builder(&mynode, target)
    }

    /// `CSL.Engine.prototype.setStyleAttributes`: run each attribute of the
    /// root element through `CSL.Attributes`, with a stand-in token named
    /// after the element (`style`). An attribute with no handler throws in
    /// upstream (`CSL.Attributes[attrname]` is undefined).
    pub fn set_style_attributes(&mut self) -> CslResult<()> {
        let Some(root) = self.csl_xml.data_obj else {
            return Err(type_error(
                "Cannot read properties of undefined (reading 'name')",
            ));
        };
        let mut dummy = Token::new(&self.csl_xml.nodename(root), TokenType::Start);
        for (attrname, value) in self.csl_xml.attributes(root) {
            let arg = js::to_js_string(&value);
            // PORT-LATER(attributes): with the stub `apply` every attribute
            // reads as "defined" here; upstream throws a TypeError for one
            // `CSL.Attributes` does not define.
            if !super::attributes::apply(self, &mut dummy, &attrname, &arg)? {
                // STUB(attributes): the style-level handlers, until
                // attributes.rs defines them (it then returns true and this
                // is never reached).
                self.style_attribute_fallback(&attrname, &arg);
            }
        }
        Ok(())
    }

    /// STUB(attributes): the handlers of `CSL.Attributes` for the attributes
    /// of the `<style>` element (src/attributes.js 1081-1090, 1164, 1509-1565):
    /// `@default-locale`, `@default-locale-sort`, `@demote-non-dropping-particle`,
    /// `@class`, `@version`, `@page-range-format`, `@year-range-format`,
    /// `@initialize-with-hyphen`, `@sort-separator`, `@xmlns`.
    fn style_attribute_fallback(&mut self, key: &str, arg: &str) {
        static X: LazyLock<Regex> =
            LazyLock::new(|| Regex::new("-x-(sort|translit|translat)-").expect("static"));
        match key {
            "@default-locale" => {
                let m: Vec<String> = X.captures_iter(arg).map(|c| c[1].to_string()).collect();
                let lst = js::split(&X, arg);
                let mut ret = vec![lst[0].clone()];
                for pos in 1..lst.len() {
                    ret.push(m.get(pos - 1).cloned().unwrap_or_default());
                    ret.push(lst[pos].clone());
                }
                let mut pos = 1;
                while pos < ret.len() {
                    let k = format!("locale-{}", ret[pos]);
                    let v =
                        js::trim(ret.get(pos + 1).map(String::as_str).unwrap_or("")).to_string();
                    if let Some(Value::Array(a)) = self.opt.get_mut(&k) {
                        a.push(Value::String(v));
                    }
                    pos += 2;
                }
                self.opt.insert(
                    "default-locale".into(),
                    Value::Array(vec![Value::String(ret[0].clone())]),
                );
            }
            "@default-locale-sort" => {
                self.opt
                    .insert("default-locale-sort".into(), Value::String(arg.to_string()));
            }
            "@demote-non-dropping-particle" => {
                self.opt.insert(
                    "demote-non-dropping-particle".into(),
                    Value::String(arg.to_string()),
                );
            }
            "@class" => {
                self.opt
                    .insert("class".into(), Value::String(arg.to_string()));
            }
            "@version" => {
                self.opt
                    .insert("version".into(), Value::String(arg.to_string()));
            }
            "@page-range-format" | "@year-range-format" => {
                self.opt
                    .insert(key[1..].to_string(), Value::String(arg.to_string()));
            }
            "@initialize-with-hyphen" => {
                if arg == "false" {
                    self.opt
                        .insert("initialize-with-hyphen".into(), Value::Bool(false));
                }
            }
            "@sort-separator" => {
                let mut t = Token::new("style", TokenType::Start);
                self.set_opt(&mut t, "sort-separator", Value::String(arg.to_string()));
            }
            _ => {}
        }
    }

    /// `CSL.Engine.prototype.getTerm(term, form, plural, gender, mode,
    /// forceDefaultLocale)`: the locale term `term` in `form`
    /// (`long`, `short`, `verb`, `symbol`, ...) for `plural` (0 or 1), or for
    /// `gender`.
    ///
    /// `None` is JS `undefined` (no such term; with `mode` `TOLERANT` the
    /// result is `Some("")`, with `STRICT` an error). A term written in all
    /// upper case is lower-cased first. A non-empty result sets
    /// `tmp.cite_renders_content`.
    pub fn get_term(
        &mut self,
        term: &str,
        form: Option<&str>,
        plural: Option<i64>,
        gender: Option<&str>,
        mode: Option<i64>,
        force_default_locale: bool,
    ) -> CslResult<Option<String>> {
        let mut term = term.to_string();
        static UPPER: LazyLock<Regex> = LazyLock::new(|| Regex::new("[A-Z]").expect("static"));
        if !term.is_empty() && UPPER.is_match(&term) && term == term.to_uppercase() {
            // CSL.debug("Warning: term key is in uppercase form: "+term);
            term = term.to_lowercase();
        }
        let lang = if force_default_locale {
            self.opt
                .get("default-locale")
                .and_then(|d| d.get(0))
                .map(js::to_js_string)
                .unwrap_or_default()
        } else {
            js::get_string(&self.opt, "lang").unwrap_or_default()
        };
        let locale = self
            .locale
            .get(&lang)
            .ok_or_else(|| type_error("Cannot read properties of undefined (reading 'terms')"))?;
        let mut ret = Self::get_field(load::LOOSE, &locale.terms, &term, form, plural, gender)?
            .map(|v| js::to_js_string(&v));
        // XXXXX Temporary, until locale term is deployed in CSL.
        if ret.as_deref().map(str::is_empty).unwrap_or(true) && term == "range-delimiter" {
            ret = Some("\u{2013}".to_string());
        }
        // XXXXX Not so good if mode is neither strict nor tolerant ...
        if ret.is_none() {
            if mode == Some(load::STRICT) {
                return Err(EngineError::Csl(format!(
                    "Error in getTerm: term \"{term}\" does not exist."
                )));
            } else if mode == Some(load::TOLERANT) {
                ret = Some(String::new());
            }
        }
        if ret.as_deref().map(|r| !r.is_empty()).unwrap_or(false) {
            self.tmp.cite_renders_content = true;
        }
        Ok(ret)
    }

    /// `CSL.Engine.prototype.getDate(form, forceDefaultLocale)`: the locale's
    /// `<date form="...">` template, `None` for upstream's `false`. (With
    /// `forceDefaultLocale` upstream indexes `locale` with the whole
    /// `default-locale` *array*, which JS turns into its comma-joined string.)
    pub fn get_date(&self, form: &str, force_default_locale: bool) -> CslResult<Option<XmlTree>> {
        let lang = if force_default_locale {
            self.opt
                .get("default-locale")
                .map(js::to_js_string)
                .unwrap_or_default()
        } else {
            js::get_string(&self.opt, "lang").unwrap_or_default()
        };
        let locale = self
            .locale
            .get(&lang)
            .ok_or_else(|| type_error("Cannot read properties of undefined (reading 'dates')"))?;
        Ok(locale.dates.get(form).cloned())
    }

    /// `CSL.Engine.prototype.getOpt(arg)`: `locale[opt.lang].opts[arg]`, or
    /// `false` when undefined.
    pub fn get_opt(&self, arg: &str) -> CslResult<Value> {
        let lang = js::get_string(&self.opt, "lang").unwrap_or_default();
        let locale = self
            .locale
            .get(&lang)
            .ok_or_else(|| type_error("Cannot read properties of undefined (reading 'opts')"))?;
        Ok(locale.opts.get(arg).cloned().unwrap_or(Value::Bool(false)))
    }

    /// `CSL.Engine.prototype.getVariable(Item, varname, form, plural)`:
    /// [`State::get_field`] over the item.
    pub fn get_variable(
        &self,
        item: &Obj,
        varname: &str,
        form: Option<&str>,
        plural: Option<i64>,
    ) -> CslResult<Option<Value>> {
        Self::get_field(load::LOOSE, item, varname, form, plural, None)
    }

    /// `CSL.Engine.prototype.getDateNum(ItemField, partname)`: `0` for an
    /// absent date field, else its `partname` (`year`, `month`, ...).
    pub fn get_date_num(&self, item_field: Option<&Value>, partname: &str) -> Option<Value> {
        match item_field {
            None => Some(Value::from(0)),
            Some(f) => f.get(partname).cloned(),
        }
    }

    /// `CSL.Engine.getField(mode, hash, term, form, plural, gender)`: look
    /// `term` up in `hash` (the locale's terms, or an item).
    ///
    /// A term is a string, a number, or an object of forms; with `gender` the
    /// gender-specific object is used if present. The forms tried are `form`
    /// then `long` (`symbol` falls back to `short`, `verb-short` to `verb`),
    /// and a `[single, multiple]` form yields element `plural` (0 if not a
    /// number). `None` is `undefined`; in `STRICT` mode a missing term is an
    /// error.
    pub fn get_field(
        mode: i64,
        hash: &Obj,
        term: &str,
        form: Option<&str>,
        plural: Option<i64>,
        gender: Option<&str>,
    ) -> CslResult<Option<Value>> {
        let mut ret: Option<Value> = Some(Value::String(String::new()));
        let Some(entry) = hash.get(term) else {
            if mode == load::STRICT {
                return Err(EngineError::Csl(format!(
                    "Error in getField: term \"{term}\" does not exist."
                )));
            }
            return Ok(None);
        };
        let hashterm: &Value = match gender {
            Some(g) if !g.is_empty() && js::truthy_opt(entry.get(g)) => &entry[g],
            _ => entry,
        };
        let mut forms: Vec<Option<&str>> = Vec::new();
        match form {
            Some("symbol") => forms = vec![Some("symbol"), Some("short")],
            Some("verb-short") => forms = vec![Some("verb-short"), Some("verb")],
            Some("long") => {}
            other => forms = vec![other],
        }
        forms.push(Some("long"));
        for f in forms {
            if hashterm.is_string() || hashterm.is_number() {
                ret = Some(hashterm.clone());
            } else if let Some(sub) = f.and_then(|f| hashterm.get(f)) {
                if sub.is_string() || sub.is_number() {
                    ret = Some(sub.clone());
                } else if let Some(p) = plural {
                    ret = sub.get(p as usize).cloned();
                } else {
                    ret = sub.get(0).cloned();
                }
                break;
            }
        }
        Ok(ret)
    }

    /// `CSL.Engine.prototype.configureTokenLists`: run
    /// [`State::configure_token_list`] over each area's tokens
    /// (`citation`, `citation_sort`, `bibliography`, `bibliography_sort`,
    /// `intext`).
    pub fn configure_token_lists(&mut self) -> CslResult<()> {
        for area in load::AREAS {
            let mut tokens = std::mem::take(&mut self.area_mut(area).tokens);
            let r = self.configure_token_list(&mut tokens);
            self.area_mut(area).tokens = tokens;
            r?;
        }
        Ok(())
    }

    /// `CSL.Engine.prototype.configureTokenList(tokens)`: the back-to-front
    /// pass that sets `token.next = index + 1`, collects each `<date>`'s
    /// `dateparts` from its `date-part` children, and lets
    /// `choose`/`if`/`else-if`/`else`/`institution` set their jump targets
    /// (`CSL.Node[name].configure`).
    pub fn configure_token_list(&mut self, tokens: &mut Vec<Token>) -> CslResult<()> {
        const DATEPARTS_MASTER: [&str; 3] = ["year", "month", "day"];
        let mut dateparts: Option<Vec<String>> = None;
        let mut ppos = tokens.len();
        while ppos > 0 {
            ppos -= 1;
            let (name, tokentype) = (tokens[ppos].name.clone(), tokens[ppos].tokentype);
            if name == "date" && tokentype == TokenType::End {
                dateparts = Some(Vec::new());
            }
            if name == "date-part" {
                if let Some(n) = tokens[ppos].string_opt("name").filter(|n| !n.is_empty()) {
                    let list = dateparts.as_mut().ok_or_else(|| {
                        type_error("Cannot read properties of undefined (reading 'push')")
                    })?;
                    for part in DATEPARTS_MASTER {
                        if part == n {
                            list.push(n.clone());
                        }
                    }
                }
            }
            if name == "date" && tokentype == TokenType::Start {
                let mut list = dateparts.clone().ok_or_else(|| {
                    type_error("Cannot read properties of undefined (reading 'reverse')")
                })?;
                list.reverse();
                tokens[ppos].extra.insert(
                    "dateparts".into(),
                    Value::Array(list.into_iter().map(Value::String).collect()),
                );
            }
            tokens[ppos].next = Some(ppos + 1);
            if !name.is_empty() {
                util_nodes::node_configure(self, &name, tokens, ppos)?;
            }
        }
        Ok(())
    }

    /// `CSL.Engine.prototype.refetchItems(ids)`.
    ///
    /// PORT-LATER(registry): `this.registry.refhash` is a wave4/wave1-input
    /// field; until it exists there is nothing to fetch, so every id gives
    /// `None`.
    pub fn refetch_items(&self, ids: &[Value]) -> Vec<Option<Value>> {
        ids.iter()
            .map(|id| self.refetch_item(&js::to_js_string(id)))
            .collect()
    }

    /// `CSL.Engine.prototype.refetchItem(id)`: `this.registry.refhash[id]`.
    /// PORT-LATER(registry): see [`State::refetch_items`].
    pub fn refetch_item(&self, _id: &str) -> Option<Value> {
        None
    }

    /// `CSL.Engine.prototype.setOpt(token, name, value)` (executed during
    /// style build): a `style` token sets an inherited attribute on the
    /// engine and on `citation` and `bibliography`; a `citation` or
    /// `bibliography` token on its own area; any other token keeps the value
    /// in its `strings`.
    pub fn set_opt(&mut self, token: &mut Token, name: &str, value: Value) {
        fn inherit(opt: &mut Obj, name: &str, value: &Value) {
            if let Some(Value::Object(i)) = opt.get_mut("inheritedAttributes") {
                i.insert(name.to_string(), value.clone());
            }
        }
        if token.name == "style" || token.name == "cslstyle" {
            inherit(&mut self.opt, name, &value);
            inherit(&mut self.citation.opt, name, &value);
            inherit(&mut self.bibliography.opt, name, &value);
        } else if token.name == "citation" {
            inherit(&mut self.citation.opt, name, &value);
        } else if token.name == "bibliography" {
            inherit(&mut self.bibliography.opt, name, &value);
        } else {
            token.strings.insert(name.to_string(), value);
        }
    }

    /// `CSL.Engine.prototype.inheritOpt(token, attrname, parentname,
    /// defaultValue)` (executed at runtime, since macros can occur in the
    /// context of citation or bibliography): the token's own value, else the
    /// current root's inherited attribute `parentname` (default `attrname`),
    /// else `default_value`.
    pub fn inherit_opt(
        &self,
        token: &Token,
        attrname: &str,
        parentname: Option<&str>,
        default_value: Option<Value>,
    ) -> Option<Value> {
        if let Some(v) = token.strings.get(attrname) {
            return Some(v.clone());
        }
        let root = self.area_ref(&self.tmp.root.clone());
        let parent = root
            .opt
            .get("inheritedAttributes")
            .and_then(|i| i.get(parentname.filter(|p| !p.is_empty()).unwrap_or(attrname)));
        match parent {
            Some(v) => Some(v.clone()),
            None => default_value,
        }
    }

    /// `CSL.Util.fixDateNode.call(state, parent, pos, node)` (src/util_datenode.js).
    ///
    /// STUB(util_datenode): a faithful private copy so that style building
    /// can be verified without wave1-nodes's file; the integrator replaces it.
    /// Replaces the `<date>` node `node` (child `pos` of `parent`) by a copy
    /// of the locale's date template of the same `form`, carrying over the
    /// style's `variable`, affixes, `date-parts` (dropping month/day/year
    /// parts) and per-`date-part` attributes. Returns `parent`.
    pub fn fix_date_node(&mut self, parent: NodeId, pos: usize, node: NodeId) -> CslResult<NodeId> {
        let default_locale = self.csl_xml.get_attribute_value(node, "default-locale");

        // Raise date flag, used to control inclusion of year-suffix key in sorts
        // This may be a little reckless: not sure what happens on no-date conditions
        self.build.date_key = true;

        let form = self.csl_xml.get_attribute_string(node, "form");
        let default_locale_t = js::truthy(&default_locale);
        let lingo = if default_locale_t {
            self.opt
                .get("default-locale")
                .and_then(|d| d.get(0))
                .map(js::to_js_string)
                .unwrap_or_default()
        } else {
            self.csl_xml.get_attribute_string(node, "lingo")
        };

        let Some(template) = self.get_date(&form, default_locale_t)? else {
            return Ok(parent);
        };

        let dateparts = self.csl_xml.get_attribute_value(node, "date-parts");

        let variable = self.csl_xml.get_attribute_value(node, "variable");
        let prefix = self.csl_xml.get_attribute_value(node, "prefix");
        let suffix = self.csl_xml.get_attribute_value(node, "suffix");
        let display = self.csl_xml.get_attribute_value(node, "display");
        let cslid = self.csl_xml.get_attribute_value(node, "cslid");

        //
        // Xml: Copy a node
        //
        let datexml = self.csl_xml.node_copy_tree(&template);
        let lang = js::get_string(&self.opt, "lang").unwrap_or_default();
        self.csl_xml
            .set_attribute(datexml, "lingo", Value::String(lang.clone()));
        self.csl_xml
            .set_attribute(datexml, "form", Value::String(form));
        self.csl_xml.set_attribute(datexml, "date-parts", dateparts);
        self.csl_xml.set_attribute(datexml, "cslid", cslid);
        //
        // Xml: Set attribute
        //
        self.csl_xml.set_attribute(datexml, "variable", variable);
        self.csl_xml
            .set_attribute(datexml, "default-locale", default_locale.clone());
        if js::truthy(&prefix) {
            self.csl_xml.set_attribute(datexml, "prefix", prefix);
        }
        if js::truthy(&suffix) {
            self.csl_xml.set_attribute(datexml, "suffix", suffix);
        }
        if js::truthy(&display) {
            self.csl_xml.set_attribute(datexml, "display", display);
        }
        //
        // Step through any date-part children of the layout date node,
        // and lay their attributes onto the corresponding node in the
        // locale template node copy.
        //
        // tests: language_BaseLocale
        // tests: date_LocalizedTextInStyleLocaleWithTextCase
        //
        for subnode in self.csl_xml.children(datexml) {
            let XmlChild::Node(subnode) = subnode else {
                continue;
            };
            if self.csl_xml.nodename(subnode) == "date-part" {
                let partname = self.csl_xml.get_attribute_string(subnode, "name");
                if default_locale_t {
                    self.csl_xml
                        .set_attribute_on_node_identified_by_name_attribute(
                            datexml,
                            "date-part",
                            &partname,
                            "@default-locale",
                            Value::String("true".to_string()),
                        );
                }
            }
        }

        for subnode in self.csl_xml.children(node) {
            let XmlChild::Node(subnode) = subnode else {
                continue;
            };
            if self.csl_xml.nodename(subnode) == "date-part" {
                let partname = self.csl_xml.get_attribute_string(subnode, "name");
                for (attr, val) in self.csl_xml.attributes(subnode) {
                    if attr == "@name" {
                        continue;
                    }
                    if !lingo.is_empty()
                        && lingo != lang
                        && ["@suffix", "@prefix", "@form"].contains(&attr.as_str())
                    {
                        continue;
                    }
                    self.csl_xml
                        .set_attribute_on_node_identified_by_name_attribute(
                            datexml,
                            "date-part",
                            &partname,
                            &attr,
                            val,
                        );
                }
            }
        }

        let date_parts_attr = self.csl_xml.get_attribute_string(node, "date-parts");
        if date_parts_attr == "year" {
            //
            // Xml: Find one node by attribute and delete
            //
            self.csl_xml.delete_node_by_name_attribute(datexml, "month");
            self.csl_xml.delete_node_by_name_attribute(datexml, "day");
        } else if date_parts_attr == "year-month" {
            self.csl_xml.delete_node_by_name_attribute(datexml, "day");
        } else if date_parts_attr == "month-day" {
            let child_nodes = self.csl_xml.children(datexml);
            for i in 1..child_nodes.len() {
                if let XmlChild::Node(c) = &child_nodes[i] {
                    if self.csl_xml.get_attribute_value(*c, "name").as_str() == Some("year") {
                        if let XmlChild::Node(prev) = &child_nodes[i - 1] {
                            self.csl_xml.set_attribute(
                                *prev,
                                "suffix",
                                Value::String(String::new()),
                            );
                        }
                        break;
                    }
                }
            }
            self.csl_xml.delete_node_by_name_attribute(datexml, "year");
        }
        Ok(self
            .csl_xml
            .insert_child_node_after(parent, node, pos, datexml))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn get_field_follows_upstream() {
        let hash = match json!({
            "a": "plain",
            "page": {"long": ["page", "pages"], "short": ["p.", "pp."]},
            "ord": {"long": "x", "feminine": {"long": "f"}},
            "n": 7
        }) {
            Value::Object(o) => o,
            _ => unreachable!(),
        };
        let g = |t, f, p, g| State::get_field(load::LOOSE, &hash, t, f, p, g).unwrap();
        assert_eq!(g("a", Some("short"), None, None), Some(json!("plain")));
        assert_eq!(g("page", Some("short"), Some(1), None), Some(json!("pp.")));
        assert_eq!(g("page", Some("short"), None, None), Some(json!("p.")));
        assert_eq!(g("page", Some("verb"), Some(0), None), Some(json!("page")));
        assert_eq!(
            g("ord", Some("long"), None, Some("feminine")),
            Some(json!("f"))
        );
        assert_eq!(g("n", None, None, None), Some(json!(7)));
        assert_eq!(g("missing", None, None, None), None);
        assert!(State::get_field(load::STRICT, &hash, "missing", None, None, None).is_err());
    }
}
