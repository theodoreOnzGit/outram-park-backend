// Differential reference for src/citeproc/build_retrieve_item.rs under the
// development extensions the CSL test suite never turns on (and the ones it
// does, on items it does not have). Hand-written items covering every branch of
// retrieveItem, load.js's parseNoteFieldHacks / extractTitleAndSubtitle, and the
// abbreviation steps; plus every fixture item under every option set (digests).
// Output: tests/data/csl/units/retrieve_options.json
'use strict';
const fs = require('fs');
const path = require('path');
const crypto = require('crypto');
const { root, freshCSL, fixtureItems, write, plain } = require('./common.cjs');
const CSL = freshCSL();
const localeDir = path.join(root, 'vendor/citeproc-js/locale');
const STYLE = `<style xmlns="http://purl.org/net/xbiblio/csl" class="in-text" version="1.0">
  <info><id/><title/><updated>2009-08-10T04:49:00+09:00</updated></info>
  <citation><layout><text variable="title"/></layout></citation>
</style>`;

function normalizeAbbrevsKey(variable, key) {
  key = key ? ('' + key).trim() : '';
  if (['jurisdiction', 'country'].indexOf(variable) > -1) return key.toUpperCase();
  key = key.toString()
    .replace(/(?:\b|^)(?:and|et|y|und|l[ae]|the|[ld]')(?:\b|$)|[\x21-\x2C.\/\x3A-\x40\x5B-\x60\\\x7B\x7D-\x7E]/gi, '')
    .replace(/\s*\x7C\s*/g, '\x7C').replace(/\./g, ' ').replace(/\s+/g, ' ').trim();
  return key.toLowerCase();
}

// ---- hand-written items ----
const items = [];
function add(it) { items.push(it); }
const P = 'ITEM';
let n = 0;
const id = () => P + ++n;
// pages
for (const page of ['12', '12-15', '12–15', '12 - 15', '12, 15', '12 & 15', '12\\-15', 'xii', '', 12, 12.5, 'A12-A15', '12\\', 'S12', null, undefined]) add({ id: id(), type: 'book', page });
// language
for (const language of ['en', 'fr<en', 'fr>en', 'en>', '<en', 'a<b<c', 'ja<', 'x>y', '', 'de-DE', 'fr<en\nz']) add({ id: id(), type: 'book', language, title: 'T' });
// dates
for (const issued of [{ 'date-parts': [[2012, 5, 3]] }, { raw: '2012-05-03' }, { raw: 'May 3, 2012' }, { raw: '2012', 'date-parts': [] }, { raw: '2012', 'date-parts': [[1999]] }, { raw: '' }, { literal: 'Spring 2012' }, { 'date-parts': [[2012, 5, 3], [2013, 6, 4]] }, { 'date-parts': [[2012], [2013, 5]] }, '2012-05-03', 2012, ['x', 'y'], null, { raw: 'c. 1999', season: 'x' }, { raw: '-0044-03-15' }, { raw: '2012-05-03/2012-06-04' }, { raw: 12 }])
  add({ id: id(), type: 'book', issued, 'alt-issued': issued, accessed: issued, 'original-date': issued, 'locator-date': issued, submitted: issued });
// note field hacks
const notes = ['original-date: 1999', 'type: legal_case', '{:original-date: 1999}', '{:author: Doe || John}{:title: X}', 'author: Doe || John\nauthor: Roe || Jane\nfoo: bar', 'author: Smith', 'author: A || B || C', 'Some text\noriginal-date: 1999', 'original-date: 1999\nSome text', '\noriginal-date: 1999\n', 'title: Over\n\nrest', 'x{:foo: bar}y', '{:foo: bar} text {:baz: qux}', 'text {:foo: bar}', '{:foo: bar}\n{:baz: qux}', 'editor: von Doe || John', 'editor: van der Berg || Jan, Jr.', 'alt-issued: 2001', 'issued: 1980-04', 'volume: 3', 'DOI: 10.1/x', 'accessed: May 3, 2012', 'type: book\nissued: 2000', '  author: Doe || J  ', 'foo: bar baz\n', 'foo:\nbar', 5, 'a: b: c', 'alt-author: X || Y', 'translator: Smith', '{:translator: Y. Z}'];
for (const note of notes) {
  add({ id: id(), type: 'book', note });
  add({ id: id(), type: 'article', note, issued: { 'date-parts': [[2000]] }, author: [{ family: 'Pre', given: 'Existing' }], volume: '9' });
}
// titles
const titles = ['Main: Subtitle', 'Main. Subtitle', 'Main? Sub', 'Main! Sub', 'Main - Sub', 'Main — Sub', 'Main --- Sub', 'Main :: Sub', 'Just title', 'A. B. C. Title', 'Main: Sub: More', 'U.S. Foo: Bar', 'Main: Sub', '', 'Q? Why', 'Why: Because. Yes'];
const shorts = [undefined, 'Main', 'main', 'Main: Subtitle', 'Main?', 'Other', '', 'Just title', 'Why'];
for (const title of titles) for (const shortTitle of shorts) {
  add({ id: id(), type: 'book', title, shortTitle });
  if (title && shortTitle !== undefined && n % 3 === 0) add({ id: id(), type: 'legal_case', title, 'title-short': shortTitle });
}
add({ id: id(), type: 'book', title: 'Main: Sub', 'container-title': 'Cont: Sub', 'container-title-short': 'C', journalAbbreviation: 'J. Abbr.' });
add({ id: id(), type: 'book', title: 'Main: Sub', multi: { _keys: { title: { de: 'Haupt: Unter', FR: 'Principal: Sous' }, 'title-short': { de: 'Haupt' }, 'container-title': {} }, main: { title: 'DE' } } });
add({ id: id(), type: 'book', title: 'Main: Sub', multi: { _keys: { 'title-short': { de: 'Haupt' } } } });
add({ id: id(), type: 'book', title: 'Main: Sub', multi: {} });
add({ id: id(), type: 'book', multi: { _keys: { title: { FR: 'x' } }, main: { title: 'FR' } } });
// legal / jurisdiction / authority
for (const type of ['bill', 'gazette', 'legislation', 'regulation', 'treaty', 'legal_case', 'book', 'article-journal', 'chapter', undefined]) {
  for (const extra of [{}, { jurisdiction: 'us' }, { jurisdiction: 'us:c9:x' }, { authority: 'Supreme Court' }, { authority: [{ literal: 'X' }] }, { authority: 'Court', multi: { _keys: { authority: { de: 'Gericht' } } } }, { title: 'Law', volume: '5', genre: 'act', 'container-title': 'Stat.', issued: { 'date-parts': [[1999, 1, 1]] } }, { 'original-date': { 'date-parts': [[1990]] }, issued: { 'date-parts': [[1999]] } }, { 'original-date': { 'date-parts': [[]] }, issued: { 'date-parts': [[2001]] } }, { publisher: 'Pub', edition: 3, 'container-title': 'Container' }]) {
    const it = { id: id() }; if (type) it.type = type; Object.assign(it, extra); add(it);
  }
}
// abbreviations
for (const [title, j] of [['Journal of Things', undefined], ['Journal of Things', 'us'], ['Journal of Things', 'us:c9'], ['The Law and the Land', 'gb'], ['Other', undefined], ['abbr: test.', 'us:c9']]) {
  add({ id: id(), type: 'article-journal', title, jurisdiction: j, 'container-title': title, language: 'en' });
  add({ id: id(), type: 'bill', title, jurisdiction: j });
}
// misc
add({ id: id(), type: 'book', 'title-short': 'Existing', shortTitle: 'Other' });
add({ id: id(), type: 'book', 'title-short': '', shortTitle: 'Other', 'container-title-short': '', journalAbbreviation: 'JA' });
add({ id: id(), type: 'book', jurisdiction: 'us:ny:albany' });
add({ id: id(), type: 'book', jurisdiction: 'GB' });

const ABBREVS = {
  default: { title: { 'journal of things': 'J. Things', 'law land': 'Law Land', 'abbr test': 'AT' }, 'container-title': { 'journal of things': 'J. Th.' }, number: {} },
  us: { title: { 'journal of things': 'US J. Things' } },
  'us:c9': { title: { 'journal of things': 'C9 J. Things' } },
  gb: { title: { 'law land': 'GB Law' } },
};
const VARIANTS = [
  { name: 'default', ext: {}, opt: {}, lang: 'en-US' },
  { name: 'titles', ext: { main_title_from_short_title: true, split_container_title: true, implicit_short_title: true }, opt: {}, lang: 'en-US' },
  { name: 'titles-fr', ext: { main_title_from_short_title: true, force_short_title_casing_alignment: false }, opt: {}, lang: 'fr-FR' },
  { name: 'legal', ext: { consolidate_legal_items: true, force_jurisdiction: true, normalize_lang_keys_to_lowercase: true }, opt: {}, lang: 'en-US' },
  { name: 'no-hacks', ext: { raw_date_parsing: false, field_hack: false }, opt: {}, lang: 'en-US' },
  { name: 'no-override', ext: { allow_field_hack_date_override: false }, opt: {}, lang: 'en-US' },
  { name: 'multi-track', ext: {}, opt: { multi_layout: true }, track: ['book', 'chapter', 'article-journal'], lang: 'en-US' },
  { name: 'everything', ext: { main_title_from_short_title: true, split_container_title: true, implicit_short_title: true, consolidate_legal_items: true, force_jurisdiction: true, normalize_lang_keys_to_lowercase: true, require_explicit_legal_case_title_short: true }, opt: { multi_layout: true }, track: ['book', 'bill'], lang: 'fr-FR' },
];
const fixtureAll = fixtureItems().map((x, i) => ({ key: 'FX' + i, item: x.item }));

function mkEngine(v, itemMap) {
  const sys = {
    retrieveLocale: (l) => { try { return fs.readFileSync(path.join(localeDir, 'locales-' + l + '.xml'), 'utf8').replace(/\s*<\?[^>]*\?>\s*\n/g, ''); } catch (e) { return false; } },
    retrieveItem: (id) => itemMap[id],
    normalizeAbbrevsKey,
    _acache: JSON.parse(JSON.stringify(ABBREVS)),
    getAbbreviation(dummy, obj, jurisdiction, category, key) {
      if (!this._acache[jurisdiction]) this._acache[jurisdiction] = new CSL.AbbreviationSegments();
      const jurisdictions = ['default'];
      if (jurisdiction !== 'default') { const lst = jurisdiction.split(':'); for (let i = 1; i < lst.length + 1; i++) jurisdictions.push(lst.slice(0, i).join(':')); }
      jurisdictions.reverse();
      let myjurisdiction;
      for (let i = 0; i < jurisdictions.length; i++) {
        myjurisdiction = jurisdictions[i];
        if (!obj[myjurisdiction]) obj[myjurisdiction] = new CSL.AbbreviationSegments();
        if (this._acache[myjurisdiction] && this._acache[myjurisdiction][category] && this._acache[myjurisdiction][category][key]) { obj[myjurisdiction][category][key] = this._acache[myjurisdiction][category][key]; break; }
      }
      return myjurisdiction;
    },
  };
  const e = new CSL.Engine(sys, STYLE, v.lang);
  e.setLangPrefsForCites({ persons: ['translit'], institutions: ['translit'], titles: ['translit', 'translat'], journals: ['translit'], publishers: ['translat'], places: ['translat'] });
  for (const k of Object.keys(v.ext)) e.opt.development_extensions[k] = v.ext[k];
  for (const k of Object.keys(v.opt)) e.opt[k] = v.opt[k];
  if (v.track) e.bibliography.opt.track_container_items = v.track;
  return e;
}
function canonJSON(x) {
  const sort = (v) => (Array.isArray(v) ? v.map(sort) : v && typeof v === 'object' ? Object.keys(v).sort().reduce((o, k) => { if (v[k] !== undefined) o[k] = sort(v[k]); return o; }, {}) : v);
  return JSON.stringify(sort(x));
}

const out = { abbrevs: ABBREVS, items: JSON.parse(JSON.stringify(items)), variants: [] };
for (const v of VARIANTS) {
  const itemMap = {};
  for (const it of items) itemMap[it.id] = JSON.parse(JSON.stringify(it));
  const e = mkEngine(v, itemMap);
  const optBefore = JSON.parse(JSON.stringify(e.opt, (k, x) => (typeof x === 'function' ? undefined : x)));
  const rec = { name: v.name, opt: optBefore, track: v.track || null, cases: [] };
  for (const it of items) {
    const c = { id: it.id };
    try {
      const r = e.retrieveItem('' + it.id);
      c.out = plain(r);
      const r2 = e.retrieveItem('' + it.id);
      c.cached_same = r2 === r;
    } catch (err) { c.error = String(err && err.message ? err.message : err); }
    rec.cases.push(c);
  }
  rec.opt_after = {};
  for (const k of ['default-locale', 'locale-translit', 'locale-translat']) rec.opt_after[k] = plain(e.opt[k]);
  rec.dev_after = e.opt.development_extensions.normalize_lang_keys_to_lowercase;
  rec.tainted = Object.keys(e.tmp.taintedItemIDs || {});
  // whole-suite digest: every fixture item under this option set
  const fxMap = {};
  for (const f of fixtureAll) fxMap[f.key] = JSON.parse(JSON.stringify(f.item));
  const e2 = mkEngine(v, fxMap);
  const h = crypto.createHash('sha256');
  let errors = 0;
  for (const f of fixtureAll) {
    try { h.update(canonJSON(e2.retrieveItem(f.key)) + '\n'); } catch (err) { errors++; h.update('ERR ' + err.message + '\n'); }
  }
  rec.fixtures_digest = h.digest('hex');
  rec.fixtures_errors = errors;
  out.variants.push(rec);
}
write('retrieve_options', out);
