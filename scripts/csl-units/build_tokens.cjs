// build_tokens.cjs: citeproc-js 2.4.63's compiled token lists for small hand-made
// styles, the reference for the Rust port of src/attributes.js and src/node_*.js
// (kovan-literature src/citeproc/, GitHub #792, epic #790).
//
// Run by hand:
//   CITEPROC_MODULE=target/csl-reference/node_modules/citeproc/citeproc_commonjs.js \
//     node scripts/csl-units/build_tokens.cjs
// writes crates/kovan-literature/tests/data/csl/units/build_tokens.json.
//
// Each case is a tree of CSL elements (the children of <style>; only the
// citation, bibliography and intext subtrees are compiled). The tree is
// serialised to a CSL style, a real Engine builds it, and the five token lists
// (citation, bibliography, intext, citation_sort, bibliography_sort) are dumped
// in the form scripts/csl-intermediate-reference.cjs uses: a token is its own
// data properties with closures counted (`execs` -> execs_n, `tests` -> tests_n
// only when the array exists, `test` -> has_test only when set), the jump
// indices next/succeed/fail are kept at the top level, and `decorations` are
// dropped (the Rust test driver does not run CSL.setDecorations). The selected
// engine/area options the attribute handlers write are dumped too.
//
// The Rust test (src/citeproc/attributes.rs, module `tests`) replays each tree
// the way CSL.makeBuilder/CSL.XmlToToken would (attributes applied in document
// order, then the node builder), runs the jump-index pass, and compares.
'use strict';
const fs = require('fs');
const path = require('path');
const CSL = require(process.env.CITEPROC_MODULE);

// The XML normalisations of build.js:116-118 insert name/institution nodes and
// publisher-place groups into the style. They belong to the XML layer, not to the
// node builders under test: switch them off, so the token lists are what the
// node builders make of exactly the tree written in the case.
CSL.XmlJSON.prototype.addMissingNameNodes = function () {};
CSL.XmlJSON.prototype.addInstitutionNodes = function () {};
CSL.XmlJSON.prototype.insertPublisherAndPlace = function () {};

const root = path.join(__dirname, '..', '..');
const lit = path.join(root, 'crates', 'kovan-literature');

// ---------------------------------------------------------------------------
// Tree helpers. An element is {n: name, a: [[attr, value], ...], c: [children]}.
function n(name, attrs, ...children) {
  const a = Array.isArray(attrs) ? attrs : Object.entries(attrs || {});
  return { n: name, a, c: children };
}
const t = (attrs, ...c) => n('text', attrs, ...c);
const layout = (attrs, ...c) => n('layout', attrs, ...c);
const cit = (attrs, ...c) => n('citation', attrs, ...c);
const bib = (attrs, ...c) => n('bibliography', attrs, ...c);
// A citation whose layout holds `children`, and a bibliography the same.
function std(children, citAttrs, bibAttrs, layoutAttrs) {
  return [
    cit(citAttrs || {}, layout(layoutAttrs || {}, ...children)),
    bib(bibAttrs || {}, layout(layoutAttrs || {}, ...children)),
  ];
}
function toXml(e, ind) {
  const attrs = e.a.map(([k, v]) => ` ${k}="${String(v).replace(/&/g, '&amp;').replace(/"/g, '&quot;')}"`).join('');
  if (!e.c.length) return `<${e.n}${attrs}/>`;
  return `<${e.n}${attrs}>${e.c.map((x) => toXml(x)).join('')}</${e.n}>`;
}

// ---------------------------------------------------------------------------
// Cases.
const cases = [];
function add(id, nodes, extra) {
  cases.push(Object.assign({ id, class: 'in-text', nodes }, extra || {}));
}

// text
add('text_variable', std([t({ variable: 'title' })]));
add('text_variable_affixes', std([t({ variable: 'title', form: 'short', prefix: '(', suffix: ')' })]));
add('text_value', std([t({ value: 'hello' })]));
add('text_term', std([t({ term: 'and', form: 'short', plural: 'true' })]));
add('text_term_sub_verbo', std([t({ term: 'sub verbo', plural: 'never' })]));
add('text_term_contextual', std([t({ term: 'page', plural: 'contextual', 'text-case': 'capitalize-first' })]));
add('text_citation_number_citation', std([t({ variable: 'citation-number' })], {}, {}));
add('text_citation_number_bib', [cit({}, layout({}, t({ value: 'x' }))), bib({}, layout({}, t({ variable: 'citation-number' })))]);
add('text_year_suffix', std([t({ variable: 'year-suffix' })], { 'year-suffix-delimiter': ', ', 'cite-group-delimiter': '; ' }));
add('text_citation_label', [cit({}, layout({}, t({ value: 'x' }))), bib({}, layout({}, t({ variable: 'citation-label' })))]);
add('text_locator', std([t({ variable: 'locator' })]));
add('text_locator_extra', std([t({ variable: 'locator-extra' })]));
add('text_page', std([t({ variable: 'page' }), t({ variable: 'volume' }), t({ variable: 'edition' })]));
add('text_url_doi', std([t({ variable: 'URL', form: 'short' }), t({ variable: 'DOI', prefix: 'https://doi.org/' })]));
add('text_section', std([t({ variable: 'section' })]));
add('text_hereinafter', std([t({ variable: 'hereinafter' })]));
add('text_plain', std([t({ variable: 'abstract' }), t({ variable: 'note', quotes: 'true' })]));
add('text_multi_fields', std([
  t({ variable: 'container-title', form: 'short' }),
  t({ variable: 'title' }),
  t({ variable: 'publisher-place', form: 'short' }),
  t({ variable: 'title-short' }),
  t({ variable: 'language-name' }),
]));
add('text_text_case_title', std([t({ variable: 'title', 'text-case': 'title' }), t({ variable: 'abstract', 'text-case': 'normal' })]));
add('text_gender_cslid', std([t({ term: 'edition', gender: 'masculine', cslid: '7' })]));
add('text_strip_periods', std([t({ variable: 'container-title', 'strip-periods': 'true' })]));
add('text_default_locale', std([t({ term: 'and', 'default-locale': 'true' })]));
add('text_display', [cit({}, layout({}, t({ value: 'x' }))), bib({}, layout({}, t({ variable: 'title', display: 'block' }), t({ variable: 'abstract', display: 'left-margin' })))]);
add('text_second_field_align', [cit({}, layout({}, t({ value: 'x' }))), bib({ 'second-field-align': 'flush' }, layout({}, t({ variable: 'citation-number' }), t({ variable: 'title' })))]);

// group
add('group_basic', std([n('group', { delimiter: ', ', prefix: '[', suffix: ']' }, t({ variable: 'title' }), t({ variable: 'abstract' }))]));
add('group_nested', std([n('group', {}, n('group', { delimiter: '; ' }, t({ variable: 'title' })), t({ variable: 'abstract' }))]));
add('group_require_reject', std([n('group', { require: 'label-empty-or-alpha', 'label-form': 'short', 'label-capitalize-if-first': 'true' }, t({ variable: 'title' })), n('group', { reject: 'label-empty-or-alpha' }, t({ variable: 'title' }))]));
add('group_parallel', std([n('group', { 'parallel-first': 'title author', 'parallel-last': 'abstract', 'parallel-last-to-first': 'note', 'parallel-delimiter-override': ', ', 'parallel-delimiter-override-on-suppress': '; ', 'no-repeat': 'issued' }, t({ variable: 'title' }))]));
add('group_publisher_special', std([n('group', { 'has-publisher-and-publisher-place': 'true', 'subgroup-delimiter': ', ', 'subgroup-delimiter-precedes-last': 'always', 'publisher-delimiter': '; ', 'publisher-and': 'text', 'publisher-delimiter-precedes-last': 'never' }, t({ variable: 'publisher' }), t({ variable: 'publisher-place' }))]));
add('group_no_subgroup_delimiter', std([n('group', { 'has-publisher-and-publisher-place': 'true' }, t({ variable: 'publisher' }), t({ variable: 'publisher-place' }))]));
add('group_text_case_sort_direction', std([n('group', { sort: 'descending' }, t({ variable: 'title' }))]));

// choose
const ifc = (attrs, ...c) => n('if', attrs, ...c);
const elseif = (attrs, ...c) => n('else-if', attrs, ...c);
const els = (...c) => n('else', {}, ...c);
const choose = (...c) => n('choose', {}, ...c);
add('choose_if_else', std([choose(ifc({ variable: 'title' }, t({ variable: 'title' })), els(t({ value: 'none' })))]));
add('choose_if_elseif_else', std([choose(ifc({ type: 'book chapter' }, t({ value: 'a' })), elseif({ variable: 'volume issue', match: 'any' }, t({ value: 'b' })), elseif({ 'is-numeric': 'edition', match: 'none' }, t({ value: 'c' })), els(t({ value: 'd' })))]));
add('choose_match_modes', std([choose(
  ifc({ variable: 'title abstract', match: 'all' }, t({ value: 'a' })),
  elseif({ variable: 'title abstract', match: 'nand' }, t({ value: 'b' })),
  elseif({ type: 'book' }, t({ value: 'c' })),
  els(t({ value: 'd' })))]));
add('choose_position', std([choose(
  ifc({ position: 'first' }, t({ value: 'a' })),
  elseif({ position: 'ibid-with-locator ibid' }, t({ value: 'b' })),
  elseif({ position: 'subsequent near-note far-note container-subsequent' }, t({ value: 'c' })),
  els(t({ value: 'd' })))]));
add('choose_locator_page_number', std([choose(
  ifc({ locator: 'page chapter' }, t({ value: 'a' })),
  elseif({ page: 'range' }, t({ value: 'b' })),
  elseif({ number: 'number' }, t({ value: 'c' })),
  els(t({ value: 'd' })))]));
add('choose_jurisdiction_country_context', std([choose(
  ifc({ jurisdiction: 'us:c us:ny' }, t({ value: 'a' })),
  elseif({ country: 'us' }, t({ value: 'b' })),
  elseif({ context: 'citation' }, t({ value: 'c' })),
  elseif({ context: 'bibliography' }, t({ value: 'c' })),
  elseif({ context: 'alternative' }, t({ value: 'c' })),
  els(t({ value: 'd' })))]));
add('choose_date_tests', std([choose(
  ifc({ 'has-year-only': 'issued accessed' }, t({ value: 'a' })),
  elseif({ 'has-to-month-or-season': 'issued' }, t({ value: 'b' })),
  elseif({ 'has-day': 'issued' }, t({ value: 'c' })),
  elseif({ 'is-uncertain-date': 'issued' }, t({ value: 'd' })),
  els(t({ value: 'e' })))]));
add('choose_plural_multiple', std([choose(
  ifc({ 'is-plural': 'author' }, t({ value: 'a' })),
  elseif({ 'is-multiple': 'page' }, t({ value: 'b' })),
  elseif({ 'has-subunit': 'author' }, t({ value: 'c' })),
  elseif({ 'cite-form': 'short' }, t({ value: 'd' })),
  els(t({ value: 'e' })))]));
add('choose_disambiguate', std([choose(
  ifc({ disambiguate: 'true' }, t({ value: 'a' })),
  elseif({ disambiguate: 'check-ambiguity-and-backreference' }, t({ value: 'b' })),
  elseif({ disambiguate: 'false' }, t({ value: 'c' })),
  els(t({ value: 'd' })))]));
add('choose_containers_court', std([choose(
  ifc({ 'container-multiple': 'true' }, t({ value: 'a' })),
  elseif({ 'container-subsequent': 'false' }, t({ value: 'b' })),
  elseif({ 'court-class': 'civil' }, t({ value: 'c' })),
  els(t({ value: 'd' })))]));
add('choose_conditions', std([choose(
  ifc({}, n('conditions', { match: 'any' }, n('condition', { variable: 'title' }), n('condition', { type: 'book' })), t({ value: 'a' })),
  elseif({}, n('conditions', { match: 'all' }, n('condition', { variable: 'abstract' }), n('condition', { 'is-numeric': 'volume' })), t({ value: 'b' })),
  els(t({ value: 'c' })))]));
add('choose_nested', std([choose(ifc({ type: 'book' }, choose(ifc({ variable: 'title' }, t({ value: 'x' })), els(t({ value: 'y' })))), els(t({ value: 'z' })))]));

// names, label, institution, et-al, substitute
add('names_basic', std([n('names', { variable: 'author' }, n('name', {}))]));
add('names_full', std([n('names', { variable: 'author editor', delimiter: '; ' },
  n('name', { and: 'text', 'delimiter-precedes-last': 'always', 'et-al-min': '4', 'et-al-use-first': '2', 'et-al-subsequent-min': '3', 'et-al-subsequent-use-first': '1', 'et-al-use-last': 'true', initialize: 'false', 'initialize-with': '. ', 'name-as-sort-order': 'all', 'sort-separator': ', ', 'name-form': 'long', 'delimiter-precedes-et-al': 'never' },
    n('name-part', { name: 'family', 'text-case': 'uppercase' }), n('name-part', { name: 'given' })),
  n('et-al', { term: 'and others' }),
  n('label', { form: 'short', prefix: ' (', suffix: ')', plural: 'contextual' }),
  n('substitute', {}, n('names', { variable: 'editor' }), t({ variable: 'title' }))
)]));
add('names_label_before_after', std([n('names', { variable: 'author translator' },
  n('label', { form: 'verb' }), n('name', {}), n('label', { form: 'short' }))]));
add('names_institution', std([n('names', { variable: 'author' },
  n('name', {}), n('institution', { 'institution-parts': 'long', delimiter: '; ', 'use-first': '1', 'substitute-use-first': '1', 'use-last': '1', 'reverse-order': 'true' },
    n('institution-part', { name: 'long', 'if-short': 'true' }), n('institution-part', { name: 'short' })))]));
add('names_institution_part_other', std([n('names', { variable: 'author' }, n('institution', {}, n('institution-part', { name: 'long' }), n('institution-part', { name: 'bogus' })))]));
add('names_nested_substitute', std([n('names', { variable: 'author' }, n('name', {}),
  n('substitute', {}, n('names', { variable: 'editor' }, n('name', {}), n('label', {})), t({ variable: 'title' }), n('names', { variable: 'translator' })))]));
add('names_name_attrs_on_citation', std([n('names', { variable: 'author' }, n('name', {}))],
  { 'names-delimiter': '; ', 'name-delimiter': ', ', 'name-form': 'short', and: 'symbol', 'delimiter-precedes-last': 'never', 'delimiter-precedes-et-al': 'always', 'initialize-with': '.', initialize: 'false', 'name-as-sort-order': 'first', 'sort-separator': ' ', 'et-al-min': '5', 'et-al-use-first': '3', 'et-al-use-last': 'false', 'et-al-subsequent-min': '2', 'et-al-subsequent-use-first': '1' },
  { 'names-delimiter': '. ', 'et-al-min': '9' }));
add('names_names_attrs', std([n('names', { variable: 'author', 'names-min': '3', 'names-use-first': '1', 'names-use-last': 'true', 'suppress-min': '2', 'suppress-max': '4', 'stop-first': '1', 'stop-last': '2', require_match: 'x', 'require-match': 'true', 'name-never-short': 'true', 'name-as-reverse-order': 'true', 'leading-noise-words': 'a' }, n('name', {}))]));
add('names_two_names_nodes', std([n('names', { variable: 'author' }, n('name', {})), n('names', { variable: 'editor' }, n('name', {}), n('label', {}))]));

// number, label
add('number_forms', std([
  n('number', { variable: 'edition', form: 'ordinal' }),
  n('number', { variable: 'volume', form: 'roman' }),
  n('number', { variable: 'issue', form: 'long-ordinal' }),
  n('number', { variable: 'page', 'label-form': 'short' }),
  n('number', { variable: 'locator', substring: '2' }),
]));
add('label_term', std([n('label', { variable: 'locator', form: 'short', plural: 'always', 'strip-periods': 'true' }), n('label', { variable: 'page' })]));
add('label_no_variable', std([n('label', { term: 'page' })]));

// date (no form: CSL.Util.fixDateNode leaves the node alone but raises date_key)
add('date_plain', std([n('date', { variable: 'issued' }, n('date-part', { name: 'year' }), n('date-part', { name: 'month', form: 'short', 'strip-periods': 'true' }), n('date-part', { name: 'day', 'range-delimiter': '--' }))]));
add('date_singleton', std([n('date', { variable: 'accessed', 'date-parts': 'year' })]));
add('date_two_vars', std([n('date', { variable: 'issued event-date' }, n('date-part', { name: 'year' }))]));
add('date_in_substitute', std([n('names', { variable: 'author' }, n('substitute', {}, n('date', { variable: 'issued' }, n('date-part', { name: 'year' }))))]));

// sort
const sortOf = (...keys) => n('sort', {}, ...keys);
const key = (attrs) => n('key', attrs);
add('sort_variables', [
  cit({}, sortOf(key({ variable: 'author' }), key({ variable: 'title', sort: 'descending' }), key({ variable: 'edition' }), key({ variable: 'citation-number' }), key({ variable: 'citation-label' }), key({ variable: 'abstract' })), layout({}, t({ variable: 'title' }))),
  bib({}, sortOf(key({ variable: 'issued' }), key({ variable: 'court-class' }), key({ variable: 'citation-number', sort: 'descending' })), layout({}, t({ variable: 'title' }))),
]);
add('sort_names_et_al', [
  cit({ 'et-al-min': '3', 'et-al-use-first': '1' }, sortOf(key({ variable: 'author', 'names-min': '2', 'names-use-first': '1', 'names-use-last': 'true' })), layout({}, t({ variable: 'title' }))),
  bib({}, sortOf(key({ variable: 'editor' })), layout({}, t({ variable: 'title' }))),
]);
add('sort_citation_date_key', [cit({}, sortOf(key({ variable: 'issued' })), layout({}, t({ variable: 'title' })))]);
add('sort_citation_grouped', [cit({ collapse: 'year', 'cite-group-delimiter': ', ' }, sortOf(key({ variable: 'author' }), key({ variable: 'title' })), layout({}, t({ variable: 'title' })))]);
add('sort_citation_grouped_in_text_collapse', [cit({ collapse: 'year' }, sortOf(key({ variable: 'author' })), layout({}, t({ variable: 'title' })))]);
add('sort_citation_note_class', [cit({ collapse: 'year' }, sortOf(key({ variable: 'author' })), layout({}, t({ variable: 'title' })))], { class: 'note' });
add('sort_et_al_on_citation_position', [cit({ 'et-al-subsequent-min': '2', 'et-al-min': '4' }, sortOf(key({ variable: 'author' })), layout({}, n('names', { variable: 'author' }, n('name', {}))))]);

// layout / citation / bibliography / intext attributes
add('layout_attrs', [cit({ 'after-collapse-delimiter': '; ', 'near-note-distance': '7', 'givenname-disambiguation-rule': 'by-cite', 'disambiguate-add-names': 'true', 'disambiguate-add-givenname': 'true', 'disambiguate-add-year-suffix': 'true', collapse: 'year-suffix-ranged' }, layout({ prefix: '(', suffix: ')', delimiter: '; ' }, t({ variable: 'title' }))),
  bib({ 'hanging-indent': 'true', 'line-spacing': '2', 'entry-spacing': '0', 'second-field-align': 'margin', 'subsequent-author-substitute': '---', 'subsequent-author-substitute-rule': 'partial-each', 'exclude-types': 'book bill', 'exclude-with-fields': 'DOI URL', 'year-suffix-delimiter': ', ', 'after-collapse-delimiter': 'x', 'near-note-distance': '3', 'et-al-min': '8' }, layout({ suffix: '.' }, t({ variable: 'title' })))]);
add('layout_attrs_odd_values', [cit({ 'line-spacing': '1.5.2', 'entry-spacing': '.', 'hanging-indent': 'false', 'second-field-align': 'bogus', 'givenname-disambiguation-rule': 'bogus', 'disambiguate-add-names': 'false', 'near-note-distance': 'abc' }, layout({}, t({ variable: 'title' }))), bib({ 'line-spacing': '1.5', 'entry-spacing': '2.0' }, layout({}, t({ variable: 'title' })))]);
add('layout_options_on_style_level_attrs', std([t({ variable: 'title' })], { 'page-range-format': 'expanded', 'year-range-format': 'minimal', 'default-locale-sort': 'en', 'demote-non-dropping-particle': 'never', 'initialize-with-hyphen': 'false', 'cite-group-delimiter': ', ', 'track-containers': 'a b', 'consolidate-containers': 'c', 'disable-duplicate-year-suppression': 'US GB', 'require-comma-on-symbol': 'always', class: 'note', version: '1.0.1', lang: 'fr', 'range-delimiter': '-', 'part-separator': ',', 'macro-has-date': 'true', xmlns: 'x', lingo: 'x', bogus: 'x' }));
add('layout_decorated_layout', [cit({}, layout({ prefix: '[', suffix: ']' }, t({ variable: 'title' })))]);
add('intext_basic', [cit({}, layout({}, t({ variable: 'title' }))), n('intext', {}, layout({ prefix: '<', suffix: '>' }, t({ variable: 'abstract' })))]);
add('intext_with_sort', [cit({}, sortOf(key({ variable: 'author' })), layout({}, t({ variable: 'title' }))), n('intext', {}, layout({}, t({ variable: 'abstract' })))]);
add('bibliography_only', [bib({}, sortOf(key({ variable: 'title' })), layout({}, t({ variable: 'title' })))]);
add('display_first_element', [cit({}, layout({}, t({ value: 'x' }))), bib({}, layout({}, t({ variable: 'title', display: 'block' })))]);

// alternative, alternative-text
add('alternative_text', std([n('alternative-text', {}), t({ variable: 'title' })]));
add('alternative_node', std([n('alternative', {}, t({ variable: 'title' }))]));
add('alternative_nested_names', std([n('alternative', {}, n('names', { variable: 'author' }, n('name', {})))]));

// Cases that need machinery outside this wave; the Rust test skips them when it
// reports NotYetPorted for them (so it documents what they would exercise):
add('macro_in_text', [cit({}, layout({}, t({ macro: 'm' })))].concat([]), { macros: [n('macro', { name: 'm' }, t({ variable: 'title' }))] });
add('layout_locale', [cit({}, layout({ locale: 'en' }, t({ variable: 'title' })), layout({}, t({ variable: 'abstract' })))]);
add('if_locale', std([choose(ifc({ locale: 'en fr' }, t({ value: 'a' })), els(t({ value: 'b' })))]));
add('sort_key_macro', [cit({}, sortOf(key({ macro: 'm' })), layout({}, t({ variable: 'title' })))], { macros: [n('macro', { name: 'm' }, t({ variable: 'title' }))] });
add('date_with_form', std([n('date', { variable: 'issued', form: 'numeric' })]));
add('text_collapse_citation_number', [cit({ collapse: 'citation-number' }, layout({}, t({ variable: 'citation-number' })))]);
add('text_collapse_year_suffix_ranged', [cit({ collapse: 'year-suffix-ranged' }, layout({}, t({ variable: 'year-suffix' })))]);

// ---------------------------------------------------------------------------
// Dump.
let curEngine = null;
function formatterName(f) {
  if (f === curEngine.fun.romanizer) return 'romanizer';
  if (f === curEngine.fun.ordinalizer) return 'ordinalizer';
  if (f === curEngine.fun.long_ordinalizer) return 'long_ordinalizer';
  return 'other';
}
const TOKEN_JUMPS = ['next', 'succeed', 'fail'];
function canon(v, nested) {
  if (v === undefined || typeof v === 'function') return undefined;
  if (v === null) return null;
  const ty = typeof v;
  if (ty === 'number') return Number.isFinite(v) ? v : null;
  if (ty === 'string' || ty === 'boolean') return v;
  if (v instanceof RegExp) return { $regexp: v.source };
  if (Array.isArray(v)) return v.map((x) => { const c = canon(x, nested); return c === undefined ? null : c; });
  if (ty === 'object') {
    const out = {};
    if (v instanceof CSL.Token) {
      for (const k of Object.keys(v).sort()) {
        if (nested && TOKEN_JUMPS.indexOf(k) > -1) continue;
        if (k === 'decorations') continue;
        if (k === 'execs') { out.execs_n = v.execs.length; continue; }
        if (k === 'tests') { out.tests_n = v.tests.length; continue; }
        if (k === 'test') { if (v.test) out.has_test = true; continue; }
        if (k === 'formatter') { out.formatter = formatterName(v.formatter); continue; }
        const c = canon(v[k], true);
        if (c !== undefined) out[k] = c;
      }
      return out;
    }
    for (const k of Object.keys(v).sort()) {
      const c = canon(v[k], nested);
      if (c !== undefined) out[k] = c;
    }
    return out;
  }
  return undefined;
}
const OPT_KEYS = ['update_mode', 'bib_mode', 'has_year_suffix', 'sort_citations', 'grouped_sort', 'using_display', 'has_layout_locale', 'multi_layout', 'parallel', 'track_repeat', 'use_context_condition', 'has_disambiguate', 'page-range-format', 'year-range-format', 'disambiguate-add-names', 'disambiguate-add-givenname', 'disambiguate-add-year-suffix', 'initialize-with-hyphen', 'default-locale-sort', 'demote-non-dropping-particle', 'require_comma_on_symbol', 'disable_duplicate_year_suppression', 'class', 'version', 'inheritedAttributes', 'locale-sort', 'locale-translit', 'locale-translat'];
const AREA_OPT_KEYS = ['collapse', 'cite_group_delimiter', 'layout_prefix', 'layout_suffix', 'layout_delimiter', 'layout_decorations', 'topdecor', 'sort_directions', 'sort_locales', 'max_number_of_names', 'inheritedAttributes', 'year-suffix-delimiter', 'after-collapse-delimiter', 'subsequent-author-substitute', 'subsequent-author-substitute-rule', 'second-field-align', 'hangingindent', 'line-spacing', 'entry-spacing', 'near-note-distance', 'exclude_types', 'exclude_with_fields', 'track_container_items', 'consolidate_containers', 'givenname-disambiguation-rule'];
function pick(obj, keys) {
  const out = {};
  for (const k of keys) {
    const c = canon(obj[k], false);
    if (c !== undefined) out[k] = c;
  }
  return out;
}

function retrieveLocale(lang) {
  const p = path.join(lit, 'data', 'csl', `locales-${lang}.xml`);
  return fs.existsSync(p) ? fs.readFileSync(p, 'utf8') : false;
}

const out = { meta: { engine: 'citeproc-js 2.4.63', generator: 'scripts/csl-units/build_tokens.cjs', opt_keys: OPT_KEYS, area_opt_keys: AREA_OPT_KEYS }, cases: [] };
for (const c of cases) {
  const xml = `<?xml version="1.0" encoding="utf-8"?><style xmlns="http://purl.org/net/xbiblio/csl" class="${c.class}" version="1.0" default-locale="en-US"><info><title>t</title><id>http://example.org/t</id><updated>2000-01-01T00:00:00+00:00</updated></info>${(c.macros || []).map((m) => toXml(m)).join('')}${c.nodes.map((x) => toXml(x)).join('')}</style>`;
  const rec = { id: c.id, class: c.class, nodes: c.nodes };
  try {
    const sys = { retrieveLocale, retrieveItem: () => ({}) };
    const e = new CSL.Engine(sys, xml, 'en-US');
    curEngine = e;
    const areas = {};
    for (const a of CSL.AREAS) {
      areas[a] = { tokens: e[a].tokens.map((tk) => canon(tk, false)), opt: pick(e[a].opt, AREA_OPT_KEYS) };
    }
    rec.expected = { areas, opt: pick(e.opt, OPT_KEYS), cite_affixes: canon(e.tmp.cite_affixes, false), date_key: !!e.build.date_key };
  } catch (err) {
    rec.error = String(err && err.message || err);
  }
  out.cases.push(rec);
}
const dest = path.join(lit, 'tests', 'data', 'csl', 'units', 'build_tokens.json');
fs.mkdirSync(path.dirname(dest), { recursive: true });
fs.writeFileSync(dest, JSON.stringify(out, null, 1) + '\n');
const errs = out.cases.filter((c) => c.error);
console.log(`wrote ${dest}: ${out.cases.length} cases, ${errs.length} engine errors`);
for (const c of errs) console.log('  ' + c.id + ': ' + c.error);
