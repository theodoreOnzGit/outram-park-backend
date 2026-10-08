// Differential reference for the `authority` / `committee` string split in
// attributes.js (`@variable`, "check for output"), GitHub #808: a string value
// of either variable under `cs:names` is split on `;` into institutional names.
// Items x styles x {citation, bibliography}; each output is rendered by a FRESH
// engine (so the bibliography is the first thing to retrieve the item, which is
// the case the port got wrong), plus one engine rendering the whole pool.
// Output: tests/data/csl/units/authority_split.json
// Run: CITEPROC_MODULE=<repo>/target/csl-reference/node_modules/citeproc/citeproc_commonjs.js node scripts/csl-units/authority_split.cjs
'use strict';
const fs = require('fs');
const path = require('path');
const { freshCSL, root, write } = require('./common.cjs');
const CSL = freshCSL();

const M = (v, s) => ({ _keys: { [v]: s } });
const POOL = [
  { id: 'A01', type: 'report', title: 'T', authority: 'Supreme Court' },
  { id: 'A02', type: 'report', title: 'T', authority: 'Supreme Court; Appeals Court' },
  { id: 'A03', type: 'report', title: 'T', authority: 'Supreme Court;Appeals Court' },
  { id: 'A04', type: 'report', title: 'T', authority: 'Supreme Court ;  Appeals Court ; High Court' },
  { id: 'A05', type: 'report', title: 'T', authority: 'Supreme Court;' },
  { id: 'A06', type: 'report', title: 'T', authority: 'Supreme Court; Appeals Court;' },
  { id: 'A07', type: 'report', title: 'T', authority: ';Supreme Court' },
  { id: 'A08', type: 'report', title: 'T', authority: ';' },
  { id: 'A09', type: 'report', title: 'T', authority: '' },
  { id: 'A10', type: 'report', title: 'T', authority: 'A; B; C; D; E; F' },
  { id: 'A11', type: 'report', title: 'T', authority: 'Dept. of Health | Research Unit; Bureau | Science' },
  { id: 'A12', type: 'report', title: 'T', authority: 'Supreme Court; Supreme Court' },
  { id: 'C01', type: 'report', title: 'T', committee: 'Senate Committee' },
  { id: 'C02', type: 'report', title: 'T', committee: 'Senate Committee; House Committee' },
  { id: 'C03', type: 'report', title: 'T', committee: 'Finance ; Rules ; Ethics' },
  { id: 'C04', type: 'report', title: 'T', committee: 'Finance;' },
  { id: 'B01', type: 'report', title: 'T', authority: 'Supreme Court; Appeals Court', committee: 'Finance; Rules' },
  { id: 'B02', type: 'report', title: 'T', author: [{ family: 'Doe', given: 'John' }], authority: 'Supreme Court; Appeals Court' },
  { id: 'B03', type: 'book', title: 'T', authority: 'Supreme Court; Appeals Court', editor: [{ family: 'Roe', given: 'Jane' }] },
  { id: 'N01', type: 'report', title: 'T', authority: [{ literal: 'Supreme Court; Appeals Court' }] },
  { id: 'N02', type: 'report', title: 'T', authority: [{ family: 'Court', given: 'Supreme' }, { literal: 'Appeals Court' }] },
  { id: 'N03', type: 'report', title: 'T', authority: 5 },
  { id: 'M01', type: 'report', title: 'T', authority: 'Supreme Court; Appeals Court', language: 'ja', multi: M('authority', { 'ja-Latn': 'Saikou Saibansho; Koutou Saibansho' }) },
  { id: 'M02', type: 'report', title: 'T', authority: 'Supreme Court; Appeals Court', language: 'ja', multi: M('authority', { 'ja-Latn': 'Saikou Saibansho; Koutou Saibansho', en: 'Top; Appeal' }) },
  { id: 'M03', type: 'report', title: 'T', authority: 'Supreme Court; Appeals Court', language: 'ja', multi: M('authority', { 'ja-Latn': 'Saikou Saibansho' }) },
  { id: 'M04', type: 'report', title: 'T', authority: 'Supreme Court; Appeals Court; High Court', language: 'ja', multi: M('authority', { 'ja-Latn': 'One; Two', en: 'X; Y; Z' }) },
  { id: 'M05', type: 'report', title: 'T', authority: 'Supreme Court', language: 'ja', multi: M('authority', { 'ja-Latn': 'Saikou Saibansho' }) },
  { id: 'M06', type: 'report', title: 'T', committee: 'Finance; Rules', multi: M('committee', { 'ja-Latn': 'Zaisei; Kisoku' }) },
  { id: 'M07', type: 'report', title: 'T', committee: 'Finance; Rules', multi: M('committee', { 'ja-Latn': 'Zaisei' }) },
  { id: 'M08', type: 'report', title: 'T', authority: 'Supreme Court; Appeals Court', multi: M('title', { 'ja-Latn': 'x' }) },
  { id: 'M09', type: 'report', title: 'T', authority: 'Supreme Court; Appeals Court', multi: { main: {} } },
  { id: 'M10', type: 'report', title: 'T', authority: 'Supreme Court; Appeals Court', multi: { _keys: {} } },
  { id: 'L01', type: 'legal_case', title: 'T', authority: 'Supreme Court; Appeals Court', 'container-title': 'Rep.' },
  { id: 'L02', type: 'legislation', title: 'T', authority: 'Parliament; Senate', jurisdiction: 'us' },
  { id: 'L03', type: 'legal_case', title: 'T', authority: 'Supreme Court', jurisdiction: 'us' },
  { id: 'L04', type: 'legal_case', title: 'T', authority: 'Supreme Court; Appeals Court', jurisdiction: 'us:ca' },
  { id: 'D01', type: 'report', title: 'Dup', authority: 'Supreme Court; Appeals Court', issued: { 'date-parts': [[2001]] } },
  { id: 'D02', type: 'report', title: 'Dup', authority: 'Supreme Court; Appeals Court', issued: { 'date-parts': [[2001]] } },
  { id: 'D03', type: 'report', title: 'Dup', authority: 'Supreme Court', issued: { 'date-parts': [[2001]] } },
  { id: 'U01', type: 'report', title: 'T', authority: '  Supreme Court  ;  Appeals Court  ' },
  { id: 'U02', type: 'report', title: 'T', authority: 'Cour suprême; Tribunal d’appel' },
  { id: 'U03', type: 'report', title: 'T', authority: '最高裁判所; 高等裁判所' },
  { id: 'X01', type: 'report', title: 'T', author: [{ literal: 'Supreme Court' }] },
  { id: 'X02', type: 'report', title: 'T', author: [{ literal: 'Supreme Court' }, { literal: 'Appeals Court' }] },
];

const hdr = (cls, extra) => `<style xmlns="http://purl.org/net/xbiblio/csl" class="${cls}" version="1.0"${extra || ''}><info><id>http://example.org/s</id><title>S</title><updated>2020-01-01T00:00:00+00:00</updated></info>`;
const NAMES = (v, inner) => `<names variable="${v}">${inner}</names>`;
const STYLES = {
  // The defect's case: authority only in the bibliography.
  bib_only: hdr('in-text') + `<citation><layout><text variable="title"/></layout></citation><bibliography><layout>${NAMES('authority', '<name/>')}</layout></bibliography></style>`,
  // names-only in both areas, institution settings.
  names_only: hdr('in-text') + `<citation><layout delimiter="; ">${NAMES('authority committee', '<name/><institution institution-parts="long" delimiter=" + " and="text"/>')}</layout></citation><bibliography><layout suffix=".">${NAMES('authority committee', '<name/><institution institution-parts="long" delimiter=" + " and="text"/>')}</layout></bibliography></style>`,
  // note style: authority in the footnote, and committee + authority in the bibliography with a label.
  note: hdr('note') + `<citation><layout delimiter="; "><group delimiter=", ">${NAMES('authority', '<name and="text"/>')}<text variable="title" font-style="italic"/></group></layout></citation><bibliography><layout><group delimiter=". ">${NAMES('committee', '<name and="text"/><label form="short" prefix=" (" suffix=")"/>')}${NAMES('authority', '<name and="text"/><institution institution-parts="short-long" use-first="1"/>')}<text variable="title"/></group></layout></bibliography></style>`,
  // the note bibliography on author (a control: no authority involved).
  note_author: hdr('note') + `<citation><layout><text variable="title"/></layout></citation><bibliography><layout><group delimiter=". ">${NAMES('author', '<name and="text"/><institution institution-parts="short-long" use-first="1"/>')}<text variable="title"/></group></layout></bibliography></style>`,
  // author-date style: authority substitutes for author, with a year and disambiguation.
  author_date: hdr('in-text') + `<citation disambiguate-add-year-suffix="true" collapse="year"><layout prefix="(" suffix=")" delimiter="; "><group delimiter=" ">${NAMES('author', '<name form="short"/><substitute>' + NAMES('authority', '<name form="short"/>') + '</substitute>')}<date variable="issued"><date-part name="year"/></date></group></layout></citation><bibliography subsequent-author-substitute="---"><layout><group delimiter=" ">${NAMES('author', '<name/><substitute>' + NAMES('authority', '<name/>') + '</substitute>')}<date variable="issued" prefix="(" suffix=")"><date-part name="year"/></date><text variable="title"/></group></layout></bibliography></style>`,
};

// The CSL test runner's sys (what the port models, see util_transform.rs): it has
// getAbbreviation and normalizeAbbrevsKey, here over an EMPTY abbreviation cache.
function normalizeAbbrevsKey(variable, key) {
  key = key ? ('' + key).trim() : '';
  if (['jurisdiction', 'country'].indexOf(variable) > -1) return key.toUpperCase();
  key = key.toString().replace(/(?:\b|^)(?:and|et|y|und|l[ae]|the|[ld]')(?:\b|$)|[\x21-\x2C.\/\x3A-\x40\x5B-\x60\\\x7B\x7D-\x7E]/gi, '')
    .replace(/\s*\x7C\s*/g, '\x7C').replace(/\./g, ' ').replace(/\s+/g, ' ').trim();
  return key.toLowerCase();
}
function makeSys(items) {
  const cache = {};
  for (const i of items) cache[i.id] = JSON.parse(JSON.stringify(i));
  return {
    _acache: {},
    normalizeAbbrevsKey,
    getAbbreviation(dummy, obj, jurisdiction, category, key) {
      if (!this._acache[jurisdiction]) this._acache[jurisdiction] = new CSL.AbbreviationSegments();
      const js = ['default'];
      if (jurisdiction !== 'default') { const lst = jurisdiction.split(':'); for (let i = 1; i < lst.length + 1; i++) js.push(lst.slice(0, i).join(':')); }
      js.reverse();
      let my;
      for (let i = 0; i < js.length; i++) {
        my = js[i];
        if (!obj[my]) obj[my] = new CSL.AbbreviationSegments();
        if (this._acache[my] && this._acache[my][category] && this._acache[my][category][key]) { obj[my][category][key] = this._acache[my][category][key]; break; }
      }
      return my;
    },
    retrieveItem: (id) => cache[id],
    retrieveLocale: (lang) => { try { return fs.readFileSync(path.join(root, 'vendor/citeproc-js/locale', 'locales-' + lang + '.xml')).toString().replace(/\s*<\?[^>]*\?>\s*\n/g, ''); } catch (e) { return false; } },
    retrieveStyleModule: () => null,
  };
}
const wrap = (f) => { try { return { v: f() }; } catch (e) { return { e: String((e && e.message) || e) }; } };
function engine(style, items) {
  const e = new CSL.Engine(makeSys(items), style, 'en-US');
  e.setLangPrefsForCites({ persons: ['translit'], institutions: ['translit'], titles: ['translit', 'translat'], journals: ['translit'], publishers: ['translat'], places: ['translat'] });
  return e;
}
const bibText = (e) => { const b = e.makeBibliography(); return b[0].bibstart + b[1].join('') + b[0].bibend; };

const cases = [];
let outputs = 0, errors = 0;
const count = (r) => { if (r.e) errors++; else outputs++; };
for (const name of Object.keys(STYLES)) {
  const style = STYLES[name];
  const ids = POOL.map((i) => i.id);
  // M09 throws in every call (multi without _keys); it is checked alone above, not in the pool runs.
  const poolIds = ids.filter((i) => i !== 'M09');
  for (const id of ids) {
    // citation, fresh engine
    const c = wrap(() => engine(style, POOL).makeCitationCluster([{ id }]));
    // bibliography, fresh engine, one item registered
    const b = wrap(() => { const e = engine(style, POOL); e.updateItems([id]); return bibText(e); });
    // bibliography after the same item was cited (the item is already retrieved)
    const bc = wrap(() => { const e = engine(style, POOL); e.makeCitationCluster([{ id }]); e.updateItems([id]); return bibText(e); });
    count(c); count(b); count(bc);
    cases.push({ style: name, id, cite: c, bib: b, bib_after_cite: bc });
  }
  // whole pool: one engine, every item registered, bibliography first
  const all = wrap(() => { const e = engine(style, POOL); e.updateItems(poolIds); return bibText(e); });
  count(all);
  cases.push({ style: name, id: '*', bib: all });
  // whole pool: all cited in one cluster, then the bibliography
  const allc = wrap(() => { const e = engine(style, POOL); const cl = e.makeCitationCluster(poolIds.map((id) => ({ id }))); e.updateItems(poolIds); return cl + '\n' + bibText(e); });
  count(allc);
  cases.push({ style: name, id: '**', cite_then_bib: allc });
}
console.error(JSON.stringify({ items: POOL.length, styles: Object.keys(STYLES).length, cases: cases.length, outputs, errors }));
write('authority_split', { pool: POOL, styles: STYLES, cases });
