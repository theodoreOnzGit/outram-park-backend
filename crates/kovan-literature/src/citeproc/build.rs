// Part of the kovan port of citeproc-js (GitHub #790, #792).
//
// Upstream:    citeproc-js, https://github.com/juris-m/citeproc-js
// Source:      src/build.js (all but CSL.Engine.prototype.retrieveItem, which is build_retrieve_item.rs),
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
//! # Still defaulted
//!
//! `new CSL.Registry(this)`, `new CSL.Disambiguation(this)` and
//! `new CSL.Parallel(this)` (their files are later waves') keep their
//! defaults; `refetchItem(s)` returns nothing until the registry exists.

use std::sync::LazyLock;

use regex::Regex;
use serde_json::Value;

use super::js::{self, Obj};
use super::load;
use super::obj_token::{Token, TokenType};
use super::queue::Adjust;
use super::state::{new_opt, Area, Build, Configure, Fun, State, Tmp};
use super::util_dateparser::DateParser;
use super::util_flipflop::FlipFlopper;
use super::util_locale::{locale_resolve, regexp_value};
use super::util_nodes;
use super::util_page::PageRangeMangler;
use super::xmljson::{NodeId, XmlChild, XmlTree};
use super::{system, CslResult, EngineError, Sys};

fn type_error(what: &str) -> EngineError {
    EngineError::Csl(format!("TypeError: {what}"))
}

/// The value of `sort_sep` (build.js:185): `'dale|'.localeCompare('daleb', locale) > -1`
/// selects `"@"`, otherwise `"|"`. ICU puts the symbol `|` before letters, so the
/// comparison is `-1` and the answer is `"|"` for every locale tried (the 16 the
/// suite uses are checked in `js::collation_tests`); the probe is run for real so
/// a locale that collates otherwise would get `"@"` as in citeproc-js.
fn sort_sep(default_locale_sort: &str) -> &'static str {
    if js::locale_compare("dale|", "daleb", default_locale_sort) != std::cmp::Ordering::Less {
        "@"
    } else {
        "|"
    }
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
        // `this.sys.variableWrapper` exists (the runner copies OPTIONS onto sys).
        let variable_wrapper = js::truthy_opt(sys.options.get("variableWrapper"));
        // `this.sys.AbbreviationSegments = CSL.AbbreviationSegments`,
        // `CSL.stringCompare = this.sys.stringCompare`: no counterpart.
        s.sys = sys;

        s.transform = Default::default();
        s.opt = new_opt();
        s.tmp = Tmp::new();
        s.build = Build::new();
        s.fun = Fun::new();
        // Set AFTER `Fun::new()` (which would otherwise reset the hook).
        s.fun.host_hooks.variable_wrapper = variable_wrapper;

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
        // The compiled form the text-case formatters use (formatters.rs).
        // A list Rust's regex cannot compile (JS regexp syntax it lacks) leaves
        // it unset, and the formatters fall back to the default list.
        s.fun.skip_words_rex = super::formatters::make_skip_words_regex(&lst).ok();

        // this.output.adjust = new CSL.Output.Queue.adjust(this.getOpt('punctuation-in-quote'));
        let punctuation_in_quote = js::truthy(&s.get_opt("punctuation-in-quote")?);
        s.output.adjust = Some(Adjust::new(punctuation_in_quote));

        // `this.registry = new CSL.Registry(this)`
        s.registry = super::registry::Registry::new(super::registry::Comparifier::new(
            &s,
            "bibliography_sort",
        ));
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
            if area == "bibliography" {
                s.build.bibliography_tokens_len = tokens.len();
            }
            s.area_mut(area).tokens = tokens;
            r?;
        }

        if s.opt
            .get("parallel")
            .and_then(|p| p.get("enable"))
            .map(js::truthy)
            .unwrap_or(false)
        {
            // `this.parallel = new CSL.Parallel(this)`.
            s.parallel = Some(super::util_parallel::Parallel::new());
        }

        s.juris = Default::default();

        s.configure_token_lists()?;

        // `this.disambiguate = new CSL.Disambiguation(this)`.
        s.disambiguate = super::disambig_cites::Disambiguation::new(&s);

        s.splice_delimiter = None;

        //
        // date parser
        //
        // `this.fun.dateparser = CSL.DateParser` (upstream's one shared
        // instance; this port keeps one per engine, which is equivalent since
        // `addDateParserMonths` is idempotent: checked by the reference script).
        s.fun.dateparser = DateParser::new();
        //
        // flip-flopper for inline markup
        //
        s.fun.flipflopper = FlipFlopper::new(&s);
        //
        // utility functions for quotes
        //
        s.set_close_quotes_array()?;
        //
        // configure ordinal numbers generator, and long ordinal numbers
        // generator: `this.fun.ordinalizer.init(this)` and
        // `this.fun.long_ordinalizer.init(this)` fill per-locale suffix caches;
        // this port's `Ordinalizer` / `LongOrdinalizer` read the locale on each
        // call instead (same answers, the locale does not change), so there is
        // nothing to initialise.
        //
        // set up page mangler
        //
        s.fun.page_mangler = PageRangeMangler::get_function(&s, "page");
        s.fun.year_mangler = PageRangeMangler::get_function(&s, "year");

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
        self.build.builder_depth += 1;
        let r = self.build_style(nodes, None, target, &mut var_stack, &mut node_stack);
        self.build.builder_depth -= 1;
        r
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
            if !super::attributes::apply(self, &mut dummy, &attrname, &arg)? {
                // `CSL.Attributes[attrname]` is undefined: `.call` of it throws.
                return Err(type_error(
                    "Cannot read properties of undefined (reading 'call')",
                ));
            }
        }
        Ok(())
    }

    /// `CSL.Engine.prototype.getTerm(term, form, plural, gender, mode,
    /// forceDefaultLocale)`: the locale term `term` in `form`
    /// (`long`, `short`, `verb`, `symbol`, ...) for `plural` (0 or 1), or for
    /// `gender`.
    ///
    /// `None` is JS `undefined` (no such term; with `mode` `TOLERANT` the
    /// result is `Some("")`, with `STRICT` an error). A term written in all
    /// upper case is lower-cased first. A non-empty result sets
    /// `tmp.cite_renders_content` (build.js:382).
    pub fn get_term(
        &mut self,
        term: &str,
        form: Option<&str>,
        plural: Option<i64>,
        gender: Option<&str>,
        mode: Option<i64>,
        force_default_locale: bool,
    ) -> CslResult<Option<String>> {
        let ret = self.get_term_no_flag(term, form, plural, gender, mode, force_default_locale)?;
        if ret.as_deref().map(|r| !r.is_empty()).unwrap_or(false) {
            self.tmp.cite_renders_content = true;
        }
        Ok(ret)
    }

    /// [`State::get_term`] without its one side effect (setting
    /// `tmp.cite_renders_content`), for readers that only hold `&State`: the
    /// quote terms of the output decorators ([`super::formats`]), and the
    /// range-delimiter terms the page mangler and the flip-flopper read when
    /// they are built.
    pub fn get_term_no_flag(
        &self,
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

    /// `CSL.Engine.prototype.refetchItems(ids)`: the items of `registry.refhash`
    /// for `ids` (`None` for an id not there, JS `undefined`).
    pub fn refetch_items(&self, ids: &[Value]) -> Vec<Option<Value>> {
        ids.iter()
            .map(|id| self.refetch_item(&js::to_js_string(id)))
            .collect()
    }

    /// `CSL.Engine.prototype.refetchItem(id)`: `this.registry.refhash[id]`.
    pub fn refetch_item(&self, id: &str) -> Option<Value> {
        self.registry.refhash.get(id).cloned()
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
