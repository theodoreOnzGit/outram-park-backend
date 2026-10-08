// Differential reference for the registered deviations D6-D12 (GitHub #808): for each
// probe, what citeproc-js 2.4.63 gives (`citeproc_js`, a value `v` or an error `e`) next to
// the output the port is MEANT to give (`intended`, written here by hand from the issue
// comments; it is NOT produced by running anything).
// Output: tests/data/csl/units/deviations_d6_d12.json
// Run: CITEPROC_MODULE=<repo>/target/csl-reference/node_modules/citeproc/citeproc_commonjs.js node scripts/csl-units/deviations_d6_d12.cjs
'use strict';
const fs = require('fs');
const path = require('path');
const { freshCSL, root, write } = require('./common.cjs');
const CSL = freshCSL();

const hdr = (cls, extra) => `<style xmlns="http://purl.org/net/xbiblio/csl" class="${cls}" version="1.0"${extra || ''}><info><id>http://example.org/s</id><title>S</title><updated>2020-01-01T00:00:00+00:00</updated></info>`;
const CIT = (inner, attrs, layoutAttrs) => hdr('in-text') + `<citation${attrs || ''}><layout${layoutAttrs || ''}>${inner}</layout></citation></style>`;
// Secondary/tertiary slots (translation) are rendered only in the bibliography.
const BIB = (inner) => hdr('in-text') + '<citation><layout><text variable="title"/></layout></citation><bibliography><layout>' + inner + '</layout></bibliography></style>';
const ENTRY = (x) => '<div class="csl-bib-body">\n  <div class="csl-entry">' + x + '</div>\n</div>';

function normalizeAbbrevsKey(variable, key) {
  key = key ? ('' + key).trim() : '';
  if (['jurisdiction', 'country'].indexOf(variable) > -1) return key.toUpperCase();
  key = key.toString().replace(/(?:\b|^)(?:and|et|y|und|l[ae]|the|[ld]')(?:\b|$)|[\x21-\x2C.\/\x3A-\x40\x5B-\x60\\\x7B\x7D-\x7E]/gi, '')
    .replace(/\s*\x7C\s*/g, '\x7C').replace(/\./g, ' ').replace(/\s+/g, ' ').trim();
  return key.toLowerCase();
}
function makeSys(items, abbrevs) {
  const cache = {};
  for (const i of items) cache[i.id] = JSON.parse(JSON.stringify(i));
  return {
    _acache: abbrevs ? JSON.parse(JSON.stringify(abbrevs)) : {},
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
const bibText = (e) => { const b = e.makeBibliography(); return b[0].bibstart + b[1].join('') + b[0].bibend; };

const FULL = { persons: ['translit'], institutions: ['translit'], titles: ['translit', 'translat'], journals: ['translit'], publishers: ['translat'], places: ['translat'] };
const NAMES = (v, inner, attrs) => `<names variable="${v}"${attrs || ''}>${inner}</names>`;
const A = (a) => ({ family: a[0], given: a[1] });

const PROBES = [
  // D6 (C25)
  {
    dev: 'D6', name: 'authority split with a mismatched translation count',
    note: 'two authorities, one translation string: citeproc-js takes its first character',
    style: BIB(NAMES('authority', '<name/><institution institution-parts="long"/>')),
    prefs: { institutions: ['orig', 'translat'] },
    translation: ['ja'],
    items: [{ id: 'X', type: 'report', title: 'T', language: 'en', authority: 'Supreme Court; Appeals Court', multi: { _keys: { authority: { ja: '最高裁判所' } } } }],
    ops: [['bib', ['X']]],
    intended: ENTRY('Supreme Court; Appeals Court 最高裁判所'),
  },
  // D7 (C28)
  {
    dev: 'D7', name: 'short form in the secondary slot (title)',
    note: 'form="short": the secondary (translation) slot must also take title-short',
    style: BIB('<text variable="title" form="short"/>'),
    prefs: { titles: ['orig', 'translat'] },
    items: [{ id: 'X', type: 'book', title: '長 Long', 'title-short': '短', language: 'ja', multi: { _keys: { title: { en: 'Gendai Long' }, 'title-short': { en: 'Tan' } } } }],
    ops: [['bib', ['X']]],
    intended: ENTRY('短 Tan'),
  },
  {
    dev: 'D7', name: 'short form in the secondary slot (container-title)',
    note: 'the same for container-title',
    style: BIB('<text variable="container-title" form="short"/>'),
    prefs: { journals: ['orig', 'translat'] },
    items: [{ id: 'X', type: 'article-journal', 'container-title': '長 Long', 'container-title-short': '短', language: 'ja', multi: { _keys: { 'container-title': { en: 'Gendai Long' }, 'container-title-short': { en: 'Tan' } } } }],
    ops: [['bib', ['X']]],
    intended: ENTRY('短 Tan'),
  },
  // D8 (C29)
  {
    dev: 'D8', name: 'failed require-match unwinds exactly its variables',
    note: 'a names node whose require-match fails must release its variables for a later names node',
    style: CIT(NAMES('author', '<name/><substitute>' + NAMES('container-author', '<name/>') + NAMES('editor translator', '<name/>', ' require-match="true"') + NAMES('editor', '<name/>') + '<text variable="title"/></substitute>')),
    prefs: {},
    items: [{ id: 'X', type: 'book', title: 'The Title', editor: [A(['Ed', 'E'])], translator: [A(['Tr', 'T'])] }],
    ops: [['cite', ['X']]],
    intended: 'E Ed',
  },
  // D9 (C30)
  {
    dev: 'D9', name: 'publisher/place pairs, two',
    note: 'subgroup-delimiter on a group holding only publisher and publisher-place',
    style: CIT('<group delimiter=", " subgroup-delimiter="; " and="text">' + '<text variable="publisher-place"/><text variable="publisher"/></group>'),
    prefs: {},
    items: [{ id: 'X', type: 'book', title: 'T', publisher: 'Pub A; Pub B', 'publisher-place': 'Place X; Place Y' }],
    ops: [['cite', ['X']]],
    intended: 'Place X, Pub A and Place Y, Pub B',
  },
  {
    dev: 'D9', name: 'publisher/place pairs, three',
    note: 'three pairs: the delimiter between the first two, the and before the last',
    style: CIT('<group delimiter=", " subgroup-delimiter="; " and="text">' + '<text variable="publisher-place"/><text variable="publisher"/></group>'),
    prefs: {},
    items: [{ id: 'X', type: 'book', title: 'T', publisher: 'Pub A; Pub B; Pub C', 'publisher-place': 'Place X; Place Y; Place Z' }],
    ops: [['cite', ['X']]],
    intended: 'Place X, Pub A; Place Y, Pub B and Place Z, Pub C',
  },
  // D10 (C32)
  {
    dev: 'D10', name: 'et al. dropping-particle on an affiliated person',
    note: 'a person with dropping-particle "et al." under an institution keeps the et al.',
    style: CIT(NAMES('editor', '<name and="text"/><institution institution-parts="long"/>')),
    prefs: {},
    spoof: true,
    items: [{ id: 'X', type: 'book', title: 'T', editor: [A(['Smith', 'Al']), A(['Zed', 'Ed']), { family: '', given: '', 'dropping-particle': 'et al.' }, { literal: 'Org One' }] }],
    ops: [['cite', ['X']]],
    intended: 'Al Smith, Ed Zed, et al., Org One',
  },
  // D11 (C34a)
  {
    dev: 'D11', name: 'cs:name delimiter="" without and, two names',
    note: 'delimiter="" is valid CSL 1.0.2',
    style: CIT(NAMES('author', '<name delimiter=""/>')),
    prefs: {},
    items: [{ id: 'X', type: 'book', title: 'T', author: [A(['Smith', 'Al']), A(['Jones', 'Bo'])] }],
    ops: [['cite', ['X']]],
    intended: 'Al SmithBo Jones',
  },
  {
    dev: 'D11', name: 'cs:name delimiter="" without and, three names',
    note: '',
    style: CIT(NAMES('author', '<name delimiter=""/>')),
    prefs: {},
    items: [{ id: 'X', type: 'book', title: 'T', author: [A(['Smith', 'Al']), A(['Jones', 'Bo']), A(['Poe', 'Cy'])] }],
    ops: [['cite', ['X']]],
    intended: 'Al SmithBo JonesCy Poe',
  },
  {
    dev: 'D11', name: 'cs:name delimiter="" without and, institutions',
    note: 'two institutions',
    style: CIT(NAMES('author', '<name delimiter=""/><institution institution-parts="long"/>')),
    prefs: {},
    items: [{ id: 'X', type: 'book', title: 'T', author: [{ literal: 'Org One' }, { literal: 'Org Two' }] }],
    ops: [['cite', ['X']]],
    intended: 'Org OneOrg Two',
  },
  // D12 (C34b)
  {
    dev: 'D12', name: 'classic abbreviation under collapse',
    note: 'intended output is the measurement agent\'s best judgement (no spec, no fixture)',
    style: hdr('in-text') + `<citation collapse="year"><layout delimiter="; ">${NAMES('author', '<name form="short"/>')}<date variable="issued"><date-part name="year" prefix=" "/></date></layout></citation></style>`,
    prefs: {},
    abbrevs: { default: { classic: { 'cicero de officiis': 'Cic. Off.' } } },
    items: [
      { id: 'A', type: 'classic', title: 'De Officiis', author: [{ family: 'Cicero' }], issued: { 'date-parts': [[1700]] } },
      { id: 'B', type: 'book', title: 'T2', author: [{ family: 'Cicero' }], issued: { 'date-parts': [[1990]] } },
    ],
    ops: [['cite', ['A', 'B']]],
    intended: 'Cic. Off. 1700; Cicero 1990',
  },
];

function engine(p) {
  const e = new CSL.Engine(makeSys(p.items, p.abbrevs), p.style, 'en-US');
  e.setLangPrefsForCites(Object.assign({}, FULL, p.prefs));
  e.setLangTagsForCslTranslation(p.translation || ['en']);
  if (p.spoof) e.opt.development_extensions.spoof_institutional_affiliations = true;
  return e;
}
const out = PROBES.map((p) => {
  const results = p.ops.map(([op, ids]) => {
    const r = wrap(() => {
      const e = engine(p);
      if (op === 'cite') return ids.length === 1 ? e.makeCitationCluster([{ id: ids[0] }]) : e.makeCitationCluster(ids.map((id) => ({ id })));
      if (op === 'bib') { e.updateItems(ids); return bibText(e); }
      throw new Error('op');
    });
    return { op, ids, citeproc_js: r };
  });
  return Object.assign({}, p, { results });
});
for (const p of out) console.error(p.dev, p.name, '=>', JSON.stringify(p.results.map((r) => r.citeproc_js)), ' intended:', JSON.stringify(p.intended));
if (process.argv[2] !== '--dry') write('deviations_d6_d12', { probes: out });
