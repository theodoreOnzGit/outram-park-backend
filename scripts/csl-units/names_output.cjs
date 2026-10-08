// Differential reference for the names subsystem (GitHub #794):
//   - CSL.Util.Names.* (util_names.rs): initializeWith, unInitialize, mergetag,
//     tagonly, notag, getRawName
//   - CSL.NameOutput.prototype.getName (util_names_render.rs)
//   - fixupInstitution / _trimInstitution / _quashChecks (util_names_render.rs)
//   - isPerson, _compareNamesets (util_names_tests.rs, util_names_common.rs)
// Output: tests/data/csl/units/names_output.json
// Run: CITEPROC_MODULE=<repo>/target/csl-reference/node_modules/citeproc/citeproc_commonjs.js node scripts/csl-units/names_output.cjs
'use strict';
const { freshCSL, fixtureItems, write, plain } = require('./common.cjs');
const CSL = freshCSL();

const err = (e) => String(e && e.message ? e.message : e);

// ---------------------------------------------------------------- Util.Names
const givens = new Set();
const rawNames = [];
for (const { item } of fixtureItems()) {
  for (const v of CSL.NAME_VARIABLES) {
    if (Array.isArray(item[v])) {
      for (const n of item[v]) {
        if (typeof n === 'object' && n) {
          rawNames.push(n);
          if (typeof n.given === 'string') givens.add(n.given);
          if (typeof n.suffix === 'string') givens.add(n.suffix);
        }
      }
    }
  }
}
const extraGivens = ['', 'J', 'J.', 'J. Q.', 'J.Q.', 'JQ', 'John', 'John Q.', 'John Quincy', 'John-Paul', 'Jean-Pierre', 'J.-P.', 'J.-p.', 'Jean-pierre', 'jean-Pierre', 'J - P', 'Anne Marie de', 'de la', 'Ph. D.', 'Ph.D.', 'Jr.', 'Jr', 'III', 'Lord', 'Lady', 'Lord Byron', 'E. T. A.', 'ET', 'ETA.', 'E.T.A', 'Åsa', 'Éric', 'élan', 'Émile-Zola', 'Ünal Çelik', 'Oʻzbek', 'Ольга', 'Ольга Ивановна', 'Иван-Петр', 'שלום', '太郎', '山田 太郎', 'Hans Jürgen', 'Mary-Jane O\'Neil', 'O\'Neil', 'D\'Artagnan', 'John  Q.', ' John', 'John ', 'John Q.', 'John Q.', 'John﻿Q.', 'A.B.C.', 'A. B. C.', 'AB. C.', 'a.b.', 'a b', 'ab cd', 'Ab Cd', 'AB CD', 'ABc', 'AbC', 'McDonald', 'MacDonald', 'DeWitt', 'Je', 'Ja. Ba.', 'Th.', 'Chr.', 'Ch.-H.', 'Ch. - H.', 'H.-L.', 'H.L.', 'H L', '<i>J.</i> Smith', '<i>John</i> Q.', 'John <i>Q.</i>', '<span class="nocase">McD</span>onald', 'J.<sup>a</sup>', '<b>B</b>ob', 'Mr. Ed', 'St. John', 'Al-Hajj', 'al-Hajj', 'x.', 'X.', '.', '..', 'A..', 'A.-', '-A', 'A-', 'a-b', 'A-b', 'a-B'];
for (const g of extraGivens) givens.add(g);
// Fixture-derived strings first (a deterministic sample), then the edge cases.
const fixtureGivens = [...givens].filter((g) => !extraGivens.includes(g));
const sampled = fixtureGivens.filter((_, i) => i % 5 === 0);
const givenList = [...new Set(sampled.concat(extraGivens))];
const terminators = ['.', '. ', '', '%s', '%s.', '%s. ', ' ', ' ', '. ﻿', '.﻿', '﻿', '-', '.-', '%s ', '<b>%s</b>', '$&', '%s$&', ',', '. -', 'x%sy%s'];
const U = { names: [], tags: [], mergetag: [], tagonly: [], notag: [], getRaw: [] };
for (const hyph of [undefined, false, true]) {
  const state = { opt: {}, tmp: { lang_array: ['en'] } };
  if (hyph !== undefined) state.opt['initialize-with-hyphen'] = hyph;
  const rows = [];
  for (const g of (hyph === undefined ? givenList : extraGivens)) {
    for (const t of terminators) {
      for (const normalizeOnly of [false, true]) {
        // [given, terminator, normalizeOnly, result | null, error | null]
        let v = null, e = null;
        try { v = CSL.Util.Names.initializeWith(state, g, t, normalizeOnly); } catch (x) { e = err(x); }
        rows.push([g, t, normalizeOnly, v, e]);
      }
    }
  }
  U.names.push({ hyphen: hyph === undefined ? null : hyph, rows });
}
{
  const state = { opt: {}, tmp: { lang_array: ['en'] } };
  U.unInit = givenList.map((g) => ({ g, v: CSL.Util.Names.unInitialize(state, g) }));
  const tagStrs = ['', 'a', '<i>', '<i></i>', '-<i>-', '<i>x</i><b>', 'x<i>y</i>z', '<span class="nocase">', '--<sup>--', '<b>', '<>', '<'];
  const newStrs = ['', ' ', 'a', 'a ', 'a  ', ' a ', 'a\n', 'a ', 'a b ', 'ab\t', '. ', '.', '. ﻿', 'x.﻿ '];
  for (const a of tagStrs) for (const b of newStrs) {
    const r = { a, b };
    try { r.v = CSL.Util.Names.mergetag(state, a, b); } catch (e) { r.e = err(e); }
    U.mergetag.push(r);
  }
  for (const s of tagStrs.concat(newStrs, ['<i>a</i><b>b</b>', 'x<i>y', '</i>'])) {
    U.tagonly.push({ s, v: CSL.Util.Names.tagonly(state, s) });
    U.notag.push({ s, v: CSL.Util.Names.notag(s) });
  }
  for (const n of rawNames.slice(0, 600).concat([{}, { literal: 'X' }, { literal: '', family: 'F', given: 'G' }, { given: 'G' }, { family: 'F' }, { family: 'F', given: 'G' }, { family: 0, given: 5 }])) {
    U.getRaw.push({ n: plain(n), v: CSL.Util.Names.getRawName(n) });
  }
}

// ---------------------------------------------------------------------- getName
// A fake `this` with just what getName / getNameParams read.
function fakeName(opts) {
  const f = Object.create(CSL.NameOutput.prototype);
  f.state = {
    opt: {
      'default-locale': ['en-US'],
      'locale-translit': opts.translit || [],
      'locale-translat': opts.translat || [],
      'locale-sort': opts.sort || [],
      'auto-vietnamese-names': !!opts.vn,
      development_extensions: { parse_names: opts.parse !== false },
    },
    locale: {
      'en-US': { opts: { 'name-as-sort-order': { ja: true, zh: true, ko: true }, 'name-as-reverse-order': { hu: true }, 'name-never-short': { ja: true } } },
      'ja-JP': { opts: { 'name-as-sort-order': { ja: true }, 'name-as-reverse-order': {}, 'name-never-short': { zh: true } } },
      'fr-FR': { opts: {} },
    },
  };
  f.Item = opts.lang ? { language: opts.lang } : {};
  return f;
}
const GN_VARIANTS = [
  { name: 'plain', translit: ['ja-Latn', 'en'], translat: ['en'] },
  { name: 'ja', lang: 'ja-JP', translit: ['ja-Latn'], translat: ['en-US'] },
  { name: 'vn', vn: true, lang: 'vi', translit: ['vi'], translat: ['en'] },
  { name: 'hu', parse: false, lang: 'hu-HU', translit: ['hu'], translat: ['hu', 'en'] },
];
const SLOTS = ['locale-orig', 'locale-translit', 'locale-translat', 'locale-sort'];
const gnNames = [];
const seen = new Set();
function addN(n) { const k = JSON.stringify(n); if (!seen.has(k)) { seen.add(k); gnNames.push(n); } }
for (const n of rawNames.filter((_, i) => i % 12 === 0)) addN(n);
for (const f of ['Doe', 'von Doe', '山田', 'Nguyễn', '"Doe Jones"', '']) {
  for (const g of ['John', '太郎', '', 'Văn An']) {
    addN({ family: f, given: g });
    addN({ family: f, given: g, multi: { main: 'ja', _key: { 'ja-Latn': { family: 'Yamada', given: 'Taro' }, en: { family: 'Yamada', given: 'T.' } } } });
    addN({ family: f, given: g, multi: { _key: { 'ja-Latn': { family: 'Yamada', given: 'Taro', isInstitution: true }, en: { literal: 'Yamada Taro' } } } });
    addN({ family: f, given: g, isInstitution: true, multi: { _key: { en: { family: 'Foo' } } } });
    addN({ family: f, given: g, multi: { main: 'hu', _key: {} } });
    addN({ family: f, given: g, multi: { main: 'zh-CN', _key: { en: { family: 'W', given: 'X' } } } });
    addN({ family: f, given: g, 'static-ordering': true, 'non-dropping-particle': 'van', 'dropping-particle': 'de', suffix: 'Jr.', 'comma-suffix': true });
  }
}
for (const lit of ['Acme Corp', 'The Foo | Bar', '']) { addN({ literal: lit }); addN({ literal: lit, multi: { _key: { en: { literal: 'Eng' } } } }); }
addN({}); addN({ isInstitution: true, family: 'Inst' }); addN({ isInstitution: true, family: 'Inst', given: 'x' });
const GN = { variants: GN_VARIANTS, slots: SLOTS, cases: [] };
for (const n of gnNames) {
  const c = { name: plain(n), v: [] };
  for (const v of GN_VARIANTS) {
    const f = fakeName(v);
    const row = [];
    for (const slot of SLOTS) {
      for (const fallback of [true, false]) {
        for (const stop of [undefined, true]) {
          const copy = plain(n);
          const r = {};
          try {
            const res = f.getName(copy, slot, fallback, stop);
            r.name = plain(res.name); r.usedOrig = res.usedOrig === undefined ? null : res.usedOrig;
          } catch (e) { r.e = err(e); }
          r.after = plain(copy);
          row.push(r);
        }
      }
    }
    c.v.push(row);
  }
  GN.cases.push(c);
}

// ------------------------------------------------------ fixupInstitution & co
function fakeInst(opts) {
  const f = Object.create(CSL.NameOutput.prototype);
  const acache = opts.abbrevs || {};
  const sys = {
    AbbreviationSegments: CSL.AbbreviationSegments,
    _acache: acache,
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
  };
  const state = {
    sys,
    opt: { styleID: 'x', development_extensions: { legacy_institution_name_ordering: !!opts.legacy }, availableAbbrevDomains: false },
    tmp: { done_vars: [], just_looking: !!opts.justLooking },
    locale: {}, // unused
  };
  state.transform = new CSL.Transform(state);
  f.state = state;
  f.Item = opts.item || {};
  f.institution = { strings: Object.assign({}, opts.strings || {}) };
  return f;
}
const ABBREVS = {
  default: {
    'institution-entire': { 'Acme Corporation': 'ACME', 'The | Foo': '#2!title>>>FOO', 'Dated | Co': 'DC>>2001>>DC2>>1999>>old' },
    'institution-part': { 'Department of Education': 'DoE', Education: 'Ed', Bureau: 'B|X', Science: '#1!publisher,issued>>>Sci', 'Old Name': 'Newer>>2010>>Newest>>1990>>Mid' },
  },
  'us:ca': { 'institution-part': { Education: 'CA-Ed' } },
};
const INST_NAMES = ['Acme Corporation', 'Acme Corporation | Department of Education', 'Department of Education | Bureau | Science', 'Education', 'Bureau', 'A | B | C | D | E', ' A  |  B ', 'The | Foo', 'Dated | Co', 'Old Name', 'Science', '', 'x||y', '|', 'Solo'];
const STRINGS = [
  {},
  { form: 'short' },
  { form: 'long' },
  { 'institution-parts': 'short' },
  { 'institution-parts': 'short-long' },
  { 'institution-parts': 'long-short' },
  { 'institution-parts': 'long' },
  { form: 'short', 'institution-parts': 'short-long' },
  { form: 'short', 'institution-parts': 'long-short', 'reverse-order': true },
  { 'use-first': 1 },
  { 'use-first': 2, 'stop-last': 1 },
  { 'use-last': 1 },
  { 'use-last': 2, 'stop-first': 1 },
  { 'use-first': 1, 'use-last': 1 },
  { 'use-first': 1, 'use-last': 2, 'stop-last': 1, 'stop-first': 3 },
  { 'reverse-order': true },
  { 'use-first': 2, 'reverse-order': true, 'institution-parts': 'short' },
  { 'use-last': 1, form: 'short', 'institution-parts': 'short' },
];
const ITEMS = [{}, { jurisdiction: 'us:ca' }, { issued: { year: '2005' } }, { issued: { year: '1995' }, 'original-date': { year: '2015' } }, { language: 'en', issued: { year: 'x' } }];
const FX = { abbrevs: ABBREVS, names: INST_NAMES, strings: STRINGS, items: ITEMS, cases: [] };
for (const nm of INST_NAMES) for (let si = 0; si < STRINGS.length; si++) for (let ii = 0; ii < ITEMS.length; ii++) for (const legacy of [false, true]) for (const jl of [false, true]) {
  const f = fakeInst({ abbrevs: JSON.parse(JSON.stringify(ABBREVS)), strings: STRINGS[si], item: ITEMS[ii], legacy, justLooking: jl });
  const c = { n: nm, s: si, i: ii, l: legacy, j: jl };
  try {
    const r = f.fixupInstitution({ literal: nm }, 'author', 0);
    c.v = plain(r);
  } catch (e) { c.e = err(e); }
  c.done = plain(f.state.tmp.done_vars);
  FX.cases.push(c);
}
FX.trim = [];
for (const si of STRINGS.keys()) for (const lst of [[], ['a'], ['a', 'b'], ['a', 'b', 'c'], ['a', 'b', 'c', 'd', 'e']]) {
  const f = fakeInst({ strings: STRINGS[si] });
  FX.trim.push({ s: si, l: lst, v: f._trimInstitution(lst.slice()) });
}

// ---------------------------------------------------------- isPerson, compare
const IP = { isPerson: [], compare: [] };
const pnames = [{}, { literal: 'x' }, { literal: '' }, { family: 'f' }, { family: 'f', isInstitution: true }, { family: 'f', given: 'g', isInstitution: true }, { given: 'g', isInstitution: true }, { family: '', isInstitution: true }, { family: 'f', literal: '' }];
for (const n of pnames.concat(rawNames.slice(0, 300))) {
  const f = Object.create(CSL.NameOutput.prototype);
  IP.isPerson.push({ n: plain(n), v: f.isPerson(n) });
}
const lists = [[], [{ family: 'A', given: 'B' }], [{ family: 'A', given: 'B' }, { family: 'C' }], [{ family: 'A', given: 'B' }, { family: 'C', given: '' }], [{ family: 'A', given: 'B', suffix: 'Jr.' }], [{ literal: 'X' }], [{ family: 'A', given: 'B', 'non-dropping-particle': 'van' }], [{ family: 'A', given: undefined }], [{ family: 'A' }], [{ family: 'A', given: '' }], [{ family: 'A', given: null }]];
for (let i = 0; i < lists.length; i++) for (let j = 0; j < lists.length; j++) {
  const f = Object.create(CSL.NameOutput.prototype);
  IP.compare.push({ a: plain(lists[i]), b: plain(lists[j]), v: f._compareNamesets(lists[i], lists[j]) });
}
IP.compare.push({ a: null, b: [], v: false, undefinedA: true });

write('names_output', { util: U, getName: GN, inst: FX, person: IP });
