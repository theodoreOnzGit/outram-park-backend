// Reference data for the Rust port of src/load.js (and the setters of
// src/api_control.js, src/util_processor.js): runs the bundled citeproc-js on
// batteries of inputs and writes crates/kovan-literature/tests/data/csl/units/load.json,
// which the #[cfg(test)] module `diff_tests` of load.rs replays. GitHub #792.
//   CITEPROC_MODULE=.../citeproc_commonjs.js node scripts/csl-units/load.cjs
'use strict';
const fs = require('fs');
const path = require('path');
const CSL = require(process.env.CITEPROC_MODULE);

const STRINGS = [
  '', 'a', 'vol. 12', 'p. 5', 'pp. 12-15', ' sec. 4', 'art. 1.2', 'ch. 3 foo', 'Smith, J.', 'abc.', 'abc. ', 'Foo: bar',
  'e.g.', 'été', 'Ünal', '日本語', '(a)', ' lead', 'trail ', 'x\ny', '{:original-date: 1999}',
  'type: book', 'title: A: B', 'von Neumann', "d'Artagnan", "O'Brien", 'al-Rashid', 'ʻOkina', 'Mc Donald', '12', '12a',
  'a12', '1,5', ' x', 'Ab. Cd', '. x', 'A.B.', 'Ј. Ј.', 'δx', 'x y', 'Phạm Văn',
  'Nguyễn Văn A', 'Trần', 'a-b', 'a – b', 'A. B. C.', 'Foo — bar', 'Foo - bar', 'Foo -- bar',
  'Foo.  Bar', 'What? Yes', 'Hi! There', 'The U.S. Constitution', 'A study: of things', 'x™', 'a² b³', 'abª',
  '﻿ x', 'foo ﻿', '"quoted"', 'it’s', 'a,b;c:d', 'x)', 'x]', 'x9', 'x,', '[x', 'مرحبا',
  'Foo: Bar: Baz', 'Hello, World!', 'foo.bar', 'vrs. x', 'n. 5', 'no. 7', 'pp. 3', 'p.5', 'x vol. 3', ';. x', ', p. 4',
  'note: x\ntitle: Y\nz', '{:author: A || B}', 'a  b', '   ', '.', '. ', 'Dr. Foo', 'Foo. Bar', 'U.S. Foo', 'FOO. Bar',
];

function exec(re, s) {
  const flags = re.flags.replace('g', '');
  const r = new RegExp(re.source, flags);
  if (re.global) {
    const all = s.match(re);
    return all === null ? null : all;
  }
  const m = r.exec(s);
  if (m === null) return null;
  return [m.index].concat(Array.prototype.slice.call(m).map((x) => (x === undefined ? null : x)));
}

const REGEXES = [
  'LOCATOR_LABELS_REGEXP', 'STATUTE_SUBDIV_PLAIN_REGEX', 'STATUTE_SUBDIV_PLAIN_REGEX_FRONT', 'PREFIX_PUNCTUATION',
  'SUFFIX_PUNCTUATION', 'NUMBER_REGEXP', 'NAME_INITIAL_REGEXP', 'ROMANESQUE_REGEXP', 'ROMANESQUE_NOT_REGEXP',
  'STARTSWITH_ROMANESQUE_REGEXP', 'ENDSWITH_ROMANESQUE_REGEXP', 'ALL_ROMANESQUE_REGEXP', 'VIETNAMESE_SPECIALS',
  'VIETNAMESE_NAMES', 'NOTE_FIELDS_REGEXP', 'NOTE_FIELD_REGEXP', 'PARTICLE_GIVEN_REGEXP', 'PARTICLE_FAMILY_REGEXP',
  'SUPERSCRIPTS_REGEXP',
];
const out = { strings: STRINGS, regexes: {}, title_split_regexp: {} };
for (const name of REGEXES) out.regexes[name] = STRINGS.map((s) => exec(CSL[name], s));
for (const k of ['match', 'matchfirst', 'split']) out.title_split_regexp[k] = STRINGS.map((s) => exec(CSL.TITLE_SPLIT_REGEXP[k], s));

out.normalize_locale_str = ['', 'en', 'en-us', 'EN-us', 'de-DE', 'zh-tw-x', 'fr-', '-US', 'a-b-c', 'pt-br'].map((s) => [s, CSL.normalizeLocaleStr(s) === undefined ? null : CSL.normalizeLocaleStr(s)]);
out.title_split = STRINGS.filter((s) => s).map((s) => [s, CSL.TITLE_SPLIT(s)]);
out.prefix_space_append = STRINGS.map((s) => [s, CSL.checkPrefixSpaceAppend({}, s)]);
out.suffix_space_prepend = STRINGS.map((s) => [s, CSL.checkSuffixSpacePrepend({}, s)]);
out.ignore_predecessor = STRINGS.map((s) => {
  const st = { tmp: { term_predecessor: true } };
  const r = CSL.checkIgnorePredecessor(st, s);
  return [s, r, st.tmp.term_predecessor];
});
out.locale_resolve = ['', 'en', 'en-US', 'en_GB', 'fr', 'fr-CA', 'de_x_sort', 'xx', 'xx-YY', 'zh-TW', 'pt', 'ja-JP-x', 'EN'].map((s) => [s, CSL.localeResolve(s)]).concat(
  [['', 'pt-BR'], ['de', 'fr-FR'], ['xx', 'fr-FR']].map(([s, d]) => [s + '|' + d, CSL.localeResolve(s, d)]));

out.nested_brace = ['(a (b))', 'a)b(', '((x))', ')(', 'plain', '(', ')', '((', '))'].map((s) => {
  const st = { opt: { xclass: 'note' } };
  const c = new CSL.checkNestedBrace(st);
  return [s, c.update(s), c.depth, c.update(s), c.depth];
});

// extractTitleAndSubtitle on a fake state
function extract(item, opts, narrow) {
  const st = { opt: { development_extensions: Object.assign({}, opts) } };
  const it = JSON.parse(JSON.stringify(item));
  CSL.extractTitleAndSubtitle.call(st, it, narrow);
  return it;
}
const TITLE_ITEMS = [
  { title: 'Foo: bar baz' }, { title: 'Foo: bar baz', 'title-short': 'Foo' }, { title: 'Foo: bar baz', 'title-short': 'foo: bar baz' },
  { title: 'Why? Because it is', 'title-short': 'Why?' }, { title: 'Who are you? Me!' }, { title: 'Just a title' },
  { title: 'A - B - C' }, { title: 'A — B' }, { title: 'Foo. Bar' }, { title: 'The U.S. Constitution' }, { title: '' }, {},
  { title: 'Foo: bar', 'title-short': 'Quux' }, { title: 'Foo!  Bar' }, { title: 'x: y', type: 'legal_case' },
  { title: 'Titre : sous-titre' }, { title: 'A: b', 'container-title': 'C: d', 'container-title-short': 'C' },
  { title: 'A: b', multi: { _keys: { 'title-short': { de: 'A' }, title: { de: 'A: b' } } } },
  { title: 'A: b', multi: { _keys: {} } },
];
out.extract_title = [];
for (const it of TITLE_ITEMS) {
  for (const opts of [{ force_short_title_casing_alignment: true }, { implicit_short_title: true }, { split_container_title: true }]) {
    for (const narrow of [false, true]) out.extract_title.push([it, opts, narrow, extract(it, opts, narrow)]);
  }
}

// parseNoteFieldHacks (no date fields: the date parser is another file)
const NOTES = [
  'plain note', 'type: book\ntitle: X', '{:type: book}', '{:author: Doe || John}', 'first line\n{:foo: bar}', 'junk {:foo: bar}', '{:foo: bar} junk',
  'foo: bar\nbaz: qux', 'foo: bar\n\nmore text', '', '{:author: Solo}', 'author: A || B\nauthor2: C', '{:x: 1}{:y: 2}', 'a\nfoo: bar',
];
out.note_hacks = [];
for (const n of NOTES) {
  const item = { note: n };
  CSL.parseNoteFieldHacks(item, false, false);
  out.note_hacks.push([n, item]);
}

// demoteNoiseWords
out.demote = [];
for (const f of ['The Foo Bar', 'a b', 'Of Mice', 'plain', '', ' The x', 'the', 'the of']) {
  for (const d of ['drop', 'demote']) {
    const st = { opt: { lang: 'en-US' }, locale: { 'en-US': { opts: { 'leading-noise-words': ['the', 'of', 'a'] } } } };
    out.demote.push([f, d, CSL.demoteNoiseWords(st, f, d)]);
  }
}

// api_control setters
out.lang_tags = [];
for (const tags of [['de', 'en-US', 'fr'], ['en-US-x-sort', 'en', 'en-US'], [], ['a-bb', 'a-b', 'a-b-c']]) {
  const st = { opt: { 'locale-sort': [], 'locale-translit': [], 'locale-translat': [] }, getSortFunc: CSL.Engine.prototype.getSortFunc };
  CSL.Engine.prototype.setLangTagsForCslTransliteration.call(st, tags);
  CSL.Engine.prototype.setLangTagsForCslTranslation.call(st, tags);
  CSL.Engine.prototype.setLangTagsForCslSort.call(st, tags);
  out.lang_tags.push([tags, st.opt['locale-translit'], st.opt['locale-translat'], st.opt['locale-sort']]);
}
out.lang_prefs = [];
const OPT0 = () => JSON.parse(JSON.stringify(new CSL.Engine.Opt()['cite-lang-prefs']));
for (const obj of [
  { titles: ['orig', 'translat', 'translit'] }, { titles: ['orig', 'translit', 'translat'] }, { persons: ['translit'], places: ['translat', 'x'] },
  { journals: [] }, { titles: ['a', 'b', 'c', 'd'] }, { persons: ['orig', 'translit'], institutions: ['translit', 'orig', 'translat'] },
]) {
  const st = { opt: { 'cite-lang-prefs': OPT0() } };
  CSL.Engine.prototype.setLangPrefsForCites.call(st, JSON.parse(JSON.stringify(obj)));
  out.lang_prefs.push([obj, st.opt['cite-lang-prefs']]);
}
out.cite_affixes = [];
const aff = (n) => Array.from({ length: n }, (_, i) => (i % 3 === 0 ? '' : String.fromCharCode(97 + (i % 26))));
for (const list of [aff(48), aff(47), Array.from({ length: 48 }, (_, i) => 'p' + i), Array.from({ length: 48 }, (_, i) => (i % 5 ? 'q' + i : null))]) {
  const st = { opt: new CSL.Engine.Opt() };
  CSL.Engine.prototype.setLangPrefsForCiteAffixes.call(st, list);
  out.cite_affixes.push([list, st.opt.citeAffixes]);
}

// setDecorations + Doppeler + substitute
out.decorations = [
  { '@font-weight': 'bold', '@variable': 'x', '@font-style': 'italic' }, { '@quotes': '', '@strip-periods': 'true' }, {}, { '@vertical-align': 'sup', '@text-decoration': 'underline', '@font-variant': 'small-caps' },
].map((a) => { const attrs = Object.assign({}, a); const d = CSL.setDecorations.call({}, {}, attrs); return [a, d, attrs]; });
out.doppeler = [];
for (const [rex, strs] of [['<i>|</i>|<b>|</b>', ['a <i>b</i> c', 'plain', '<b>x', '', '<i></i>']], ["'", ["it's", "'x'", "a'", "''"]]]) {
  const d = new CSL.Doppeler(rex);
  for (const s of strs) { const sp = d.split(s); out.doppeler.push([rex, s, sp, d.join(sp)]); }
}
out.superscripts_sample = Object.keys(CSL.SUPERSCRIPTS).slice(0, 5).map((k) => [k, CSL.SUPERSCRIPTS[k]]);

// scalar tables compared whole
out.tables = {};
for (const k of ['NAME_VARIABLES', 'CREATORS', 'NUMERIC_VARIABLES', 'DATE_VARIABLES', 'SKIP_WORDS', 'FORMAT_KEY_SEQUENCE', 'AREAS', 'MULTI_FIELDS', 'SYS_OPTIONS', 'LANGS', 'LANG_BASES', 'STATUTE_SUBDIV_STRINGS', 'LOCATOR_LABELS_MAP', 'FIELD_CATEGORY_REMAP', 'LangPrefsMap', 'SUPERSCRIPTS', 'POSITION_MAP', 'ROMAN_NUMERALS']) out.tables[k] = CSL[k];

fs.writeFileSync(path.join(__dirname, '../../crates/kovan-literature/tests/data/csl/units/load.json'), JSON.stringify(out) + '\n');
console.log('load.json written');
