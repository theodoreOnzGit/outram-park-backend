// Differential reference for src/citeproc/util_number.rs: processNumber (input
// and node paths), Ordinalizer, LongOrdinalizer, Romanizer, Suffixator, padding,
// plus Util.Dates month/year/day formatters (util_dates.rs) which read the same
// locale terms.
// Output: tests/data/csl/units/numbers.json
'use strict';
const fs = require('fs');
const path = require('path');
const { root, freshCSL, fixtureItems, write, plain } = require('./common.cjs');
const CSL = freshCSL();

const localeDir = path.join(root, 'vendor/citeproc-js/locale');
const STYLE = `<style xmlns="http://purl.org/net/xbiblio/csl" class="in-text" version="1.0">
  <info><id/><title/><updated>2009-08-10T04:49:00+09:00</updated></info>
  <citation><layout><text variable="title"/></layout></citation>
</style>`;
const LANGS = ['en-US', 'fr-FR', 'de-DE', 'es-ES', 'ja-JP', 'nl-NL', 'pt-BR'];

function mkEngine(lang, translat) {
  const sys = {
    retrieveLocale: (l) => {
      try {
        return fs.readFileSync(path.join(localeDir, 'locales-' + l + '.xml'), 'utf8').replace(/\s*<\?[^>]*\?>\s*\n/g, '');
      } catch (e) {
        return false;
      }
    },
    retrieveItem: () => null,
  };
  const e = new CSL.Engine(sys, STYLE, lang);
  e.setLangPrefsForCites({ persons: ['translit'], institutions: ['translit'], titles: ['translit', 'translat'], journals: ['translit'], publishers: ['translat'], places: ['translat'] });
  if (translat) e.setLangTagsForCslTranslation(translat);
  const log = {};
  const orig = e.getTerm;
  e.getTerm = function (term, form, plural, gender, mode, force) {
    const key = [term === undefined ? '' : term, typeof form === 'string' ? form : '~', typeof plural === 'number' ? plural : 0,
      (typeof gender === 'string' && gender) ? gender : '~', typeof mode === 'number' ? mode : '~', force ? 1 : 0].join('|');
    const r = orig.apply(this, arguments);
    log[key] = r === undefined ? null : r;
    return r;
  };
  return { e, log };
}

function optOf(e) {
  return JSON.parse(JSON.stringify(e.opt, (k, v) => (typeof v === 'function' ? undefined : v)));
}

// ---- inputs ----
const NUMERIC = CSL.NUMERIC_VARIABLES;
const strings = new Set();
const fixtureVals = [];
for (const { item } of fixtureItems()) {
  for (const v of NUMERIC.concat(['page'])) {
    if (item[v] !== undefined && (typeof item[v] === 'string' || typeof item[v] === 'number')) strings.add(JSON.stringify([v, item[v], item.type, item.label]));
  }
}
const gen = [
  '', ' ', '0', '1', '12', '123', '12-15', '12–15', '12 - 15', '12 – 15', '12--15', '12---15', '12-5', '125-12', '1-', '-1', 'xii', 'XII', 'xii-xv', 'iv-vi', 'i', 'ii, iii',
  '2nd ed.', '2nd', '3rd edition', 'vol. 3 & 4', 'vol. 3 and 4', 'vols. 3 and 4', '3 & 4', '3 and 4', '3, 4', '3, 4 & 5', '3, 4, and 5', '3; 4', '3;4',
  'p. 12', 'pp. 12-15', 'p. 12-15', 'pp. 12–15', 'p.12', 'p 12', 'pp.12-15', 'ch. 3', 'chap. 3', 'chapter 3', 'sec. 5', 'sec. 5-7', 'secs. 5-7', '§ 5', 'art. 3', 'para. 4', 'paras. 4-6',
  'no. 5', 'No. 5', 'n. 5', 'fig. 3', 'figs. 3 & 4', 'l. 5', 'll. 5-7', 'col. 3', 'cols. 3-4', 'pt. 2', 'vol. 2 pt. 3', 'vol. 2, pt. 3', 'bk. 2 ch. 3', 'tit. 2 sec. 3',
  '12a', '12a-c', '12a–c', '12a-12c', 'A12', 'A12-A15', 'A-12', 'a12', 'S1', 'S1-S5', 'S12-15', '12S', 'E1234', 'e1234-e1240', 'B3-4', 'ABC12', 'AB12-AB15',
  '12.5', '12/3', '1/2', '1/2/3', '12/13-15', '1995/6', '1995-96', '1995–1996', '2012/2013', '12e', '12e-15e', 'xii a', '3rd & 4th', '1st-3rd', 'one', 'one-two', 'twelve', 'foo', 'foo bar', 'foo, bar', 'foo-bar', 'foo & bar',
  '91 Civ. 5442 (RPP)|91 Civ. 5471', 'a|b', '12|13', 'S. 3', 'H.R. 5', 'Pub. L. 100-100', 'Cir. 9', '12\\-15', '12\\–15', '12 \\- 15', 'a\\-b', '1\\-2\\-3', '\\-', '12-\\-15',
  '"12"', '"12-15"', '"quoted text"', '"a" "b"', '0x1F', '0x1F-0x20', '010', '1e3', ' 12 ', '\n12', '12\n', '12 - 15', '12 – 15 (2)', '12(3)', '(12)', '[12]', '12, 15, 17-19', '12, 15 & 17-19',
  'sub verbo foo', 'sv. foo', 'sv. 3', 'op. 3', 'subpara. 3', 'amend. 12', 'bibliog. 3', 'annot. 3', 'illus. 2', 'princ. 3', 'intro. 3', 'sched. 3', 'subdiv. 3', 'subsec. 3',
  'vrs. 3', 'add. 3', 'app. 3', 'cl. 3', 'cmt. 3', 'dec. 3', 'dept. 3', 'div. 3', 'ex. 3', 'fld. 3', 'fol. 3', 'hypo. 3', 'pmbl. 3', 'pub. 3', 'r. 3', 'rn. 3', 'ser. 3', 'supp. 3', 'tbl. 3',
  'pp. 12-15, 18, 20 & 22', 'vol. 1, p. 12', 'vol. 1 p. 12', 'v. 3', 'vv. 3-5', 'c. 3', 'ca. 3', 'foo vol. 3', 'foo. 3', 'a. 3', 'ab. 3', 'abc. 3', 'abcd. 3', 'abcde. 3', 'a 3', 'ab 3', 'abc 3', 'abcd 3', 'abcde 3',
];
const ALLVARS = ['page', 'volume', 'issue', 'number', 'edition', 'locator', 'chapter-number', 'number-of-pages', 'number-of-volumes', 'section', 'collection-number', 'page-first', 'locator-extra', 'citation-number', 'division', 'part-number', 'version'];
const SOMEVARS = ['page', 'page-first', 'volume', 'number', 'locator', 'number-of-pages'];
gen.forEach((g, i) => {
  for (const v of (i < 90 ? ALLVARS : SOMEVARS)) strings.add(JSON.stringify([v, g, undefined, undefined]));
});
for (const n of [0, 1, 5, 12, 100, 2012, -3, 1.5]) for (const v of ['page', 'volume', 'number-of-pages', 'number-of-volumes', 'edition', 'issue']) strings.add(JSON.stringify([v, n, undefined, undefined]));
for (const g of ['12', '12-15', 'p. 12', '3 & 4', 'ch. 3', 'sec. 5-7', 'no. 5', 'xii']) for (const lab of ['page', 'chapter', 'section', 'sub verbo', 'foo', 'figure']) {
  for (const v of ['locator', 'page']) strings.add(JSON.stringify([v, g, undefined, lab]));
}
for (const g of ['12', '3-4', 'no. 5', 'sec. 5', 'ch. 2']) for (const ty of ['legal_case', 'bill', 'legislation', 'book']) strings.add(JSON.stringify(['number', g, ty, undefined]));

const cases = [];
const engines = {};
const engineList = [];
for (const lang of LANGS) engineList.push({ name: lang, lang, translat: null });
engineList.push({ name: 'en-US+de', lang: 'en-US', translat: ['de'] });
for (const spec of engineList) {
  const { e, log } = mkEngine(spec.lang, spec.translat);
  engines[spec.name] = { lang: spec.lang, opt: optOf(e), log };
  spec.e = e;
}
function mkNode() {
  const t = new CSL.Token('number', CSL.SINGLETON);
  t.strings.prefix = '(';
  t.strings.suffix = ')';
  t.decorations = [['@font-style', 'italic']];
  return t;
}
function nodeOut(e) {
  const out = {};
  const sn = e.tmp.shadow_numbers;
  for (const k of Object.keys(sn)) {
    const o = JSON.parse(JSON.stringify(sn[k], (key, v) => (key === 'styling' || key === 'masterStyling' ? undefined : v)));
    out[k] = o;
    if (sn[k].masterStyling) o._master = { strings: plain(sn[k].masterStyling.strings), decorations: plain(sn[k].masterStyling.decorations) };
    o._styling = (sn[k].values || []).map((v) => (v && v.styling ? { strings: plain(v.styling.strings), decorations: plain(v.styling.decorations) } : null));
  }
  return out;
}

let nodeMangler = 0;
for (const spec of engineList) {
  const e = spec.e;
  let mangled = false;
  e.fun.page_mangler = new Proxy(e.fun.page_mangler, { apply(t, th, args) { mangled = true; return Reflect.apply(t, th, args); } });
  const multi = spec.name === 'en-US+de';
  const isMain = spec.name === 'en-US';
  const othersVars = ['page', 'volume', 'number', 'locator'];
  for (const s of strings) {
    const [v, val, type, label] = JSON.parse(s, (k, x) => x);
    if (!isMain && !(othersVars.indexOf(v) > -1 && typeof val === 'string' && gen.indexOf(val) > -1 && !type && !label && (spec.name === 'fr-FR' || spec.name === 'en-US+de' || gen.indexOf(val) < 60))) continue;
    const item = {};
    if (val !== null) item[v === 'page-first' ? 'page' : v] = val;
    if (v === 'page-first') {
      item.page = val;
      item['page-first'] = val;
      const m = ('' + val).split(/\s*(?:&|, |-|–)\s*/);
      if (m[0].slice(-1) !== '\\') item['page-first'] = m[0];
    }
    if (type) item.type = type;
    if (label) item.label = label;
    if (multi && ['volume', 'issue', 'number', 'edition'].indexOf(v) > -1) item.multi = { _keys: { [v]: { de: 'vol. 7-9' } } };
    const c = { engine: spec.name, variable: v, item: plain(item) };
    for (const withNode of (isMain ? [false, true] : [false])) {
      mangled = false;
      e.tmp.shadow_numbers = {};
      const it = JSON.parse(JSON.stringify(item));
      try {
        e.processNumber(withNode ? mkNode() : false, it, v);
        if (withNode) { c.node_out = nodeOut(e); c.node_mangled = mangled; }
        else c.out = plain(e.tmp.shadow_numbers);
      } catch (err) {
        c[withNode ? 'node_error' : 'error'] = String(err.message);
      }
    }
    cases.push(c);
  }
}

// ---- ordinalizers, romanizer, suffixator, padding ----
const ordinal = [];
const ord_engines = {};
const fieldLogs = {};
for (const lang of LANGS) {
  const { e, log } = mkEngine(lang, null);
  ord_engines[lang] = { opt: optOf(e), ord_101: plain(e.locale[lang].ord['1.0.1']), log, fields: {} };
  const origField = CSL.Engine.getField;
  CSL.Engine.getField = function (mode, hash, term, form, plural, gender) {
    const r = origField.apply(this, arguments);
    if (hash === e.locale[lang].terms) {
      ord_engines[lang].fields[[mode, term, form, typeof plural === 'number' ? plural : 0, (typeof gender === 'string' && gender) ? gender : '~'].join('|')] = r === undefined ? null : r;
    }
    return r;
  };
  for (let n = -3; n <= 130; n++) {
    for (const g of [undefined, 'masculine', 'feminine', 'neuter']) {
      const c = { lang, kind: 'ordinal', num: n, gender: g || null };
      try { c.out = e.fun.ordinalizer.format(n, g); } catch (err) { c.error = String(err.message); }
      ordinal.push(c);
      const c2 = { lang, kind: 'long', num: n, gender: g || null };
      try { e.tmp.cite_renders_content = false; c2.out = e.fun.long_ordinalizer.format(n, g); c2.crc = e.tmp.cite_renders_content; } catch (err) { c2.error = String(err.message); }
      ordinal.push(c2);
    }
  }
  for (const n of [200, 201, 202, 203, 204, 211, 212, 213, 1000, 1001, 1011, 1012, 1013, 2012, 9999, 10, 110, 210, '5', '05', '12', 'x', '1x']) {
    const c = { lang, kind: 'ordinal', num: n, gender: null };
    try { c.out = e.fun.ordinalizer.format(n); } catch (err) { c.error = String(err.message); }
    ordinal.push(c);
    const c2 = { lang, kind: 'long', num: n, gender: null };
    try { c2.out = e.fun.long_ordinalizer.format(n); } catch (err) { c2.error = String(err.message); }
    ordinal.push(c2);
  }
  CSL.Engine.getField = origField;
  // month / year / day formatters
  ord_engines[lang].dates = [];
  for (const num of [0, 1, 2, 5, 9, 10, 12, 13, 14, 15, 16, 17, 20, 24, 25, '3', '03', 'x', null, undefined, '', false]) {
    for (const [fn, key] of [['long', 'long'], ['short', 'short']]) {
      for (const force of [false, true]) {
        const c = { num: num === undefined ? { undef: true } : num, fn: key, force };
        try { c.out = CSL.Util.Dates.month[fn](e, num, undefined, force); if (c.out === undefined) c.undef = true; } catch (err) { c.error = String(err.message); }
        ord_engines[lang].dates.push(c);
      }
    }
  }
}
const roman = [];
for (const n of [0, 1, 4, 9, 14, 40, 90, 400, 900, 1994, 3999, 4000, 5000, 5999, 6000, 6001, -1, -12, -99999, 1.5, '12', 'x', '']) {
  const c = { num: n };
  try { c.out = new CSL.Util.Romanizer().format(n); } catch (err) { c.error = String(err.message); }
  roman.push(c);
}
const suffix = [];
for (const slist of [null, 'a,b,c', 'x,y', '1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23,24,25,26']) {
  const s = slist ? new CSL.Util.Suffixator(slist) : new CSL.Util.Suffixator();
  for (const n of [0, 1, 2, 3, 24, 25, 26, 27, 51, 52, 701, 702, 703, 18277, 18278, 100000]) suffix.push({ slist, n, out: s.format(n) });
}
const padding = [];
for (const p of ['1', '12', 'abc', 'x12y', ' 5', '-5', '-0', '123456789012345678901', '12.5', '', '00012', 'a-3', '99999999999999999999', '+5']) {
  padding.push({ in: p, out: CSL.Util.padding(p) });
}
const year = [];
for (const num of [2012, '2012', 12, 0, '', null, undefined, false, true, -5, '-5', 'abc', 1999.5, 99999, 123, 12345]) {
  const c = { num: num === undefined ? { undef: true } : num };
  for (const fn of ['long', 'short', 'numeric']) {
    try { const r = CSL.Util.Dates.year[fn]({}, num); c[fn] = r === undefined ? { undef: true } : r; } catch (err) { c[fn + '_error'] = String(err.message); }
  }
  year.push(c);
}
const month = [];
for (const num of [0, 1, 5, 12, 13, 16, 17, 24, 25, '3', 'x', null, undefined, false, '']) {
  const c = { num: num === undefined ? { undef: true } : num };
  for (const fn of ['numeric', 'numeric-leading-zeros']) {
    try { const r = CSL.Util.Dates.month[fn]({}, num); c[fn] = r === undefined ? { undef: true } : r; } catch (err) { c[fn + '_error'] = String(err.message); }
  }
  c.norm = CSL.Util.Dates.normalizeMonth(num);
  c.norm_season = CSL.Util.Dates.normalizeMonth(num, true);
  month.push(c);
}
const day = [];
for (const num of [1, 5, 12, 0, '', null, '3', '03', 'x']) {
  const c = { num };
  for (const fn of ['numeric', 'long', 'numeric-leading-zeros']) {
    try { const r = CSL.Util.Dates.day[fn]({}, num); c[fn] = r; } catch (err) { c[fn + '_error'] = String(err.message); }
  }
  day.push(c);
}
// imperial
const imperial = [];
for (const [yv, mo, d] of [[1900, 1, 1], [1868, 9, 8], [1868, 9, 7], [1912, 7, 30], [1926, 12, 25], [1989, 1, 8], [2012, 5, 3], [1800, 1, 1], ['2012', '5', '3'], [0, 0, 0]]) {
  for (const end of [false, true]) {
    for (const abbr of [null, 'X']) {
      const state = { tmp: { date_object: { month: mo, day: d, month_end: mo ? mo + 1 : 0, day_end: d ? d + 1 : 0 } }, sys: { normalizeAbbrevsKey: (a, b) => b }, transform: { abbrevs: { default: { number: {} } }, loadAbbreviation: function () { if (abbr) state.transform.abbrevs.default.number['平成'] = abbr; } } };
      imperial.push({ year: yv, month: mo, day: d, end, abbr, out: CSL.Util.Dates.year.imperial(state, yv, end) });
    }
  }
}
write('numbers', { langs: LANGS, engines, cases, ordinal, ord_engines, roman, suffix, padding, year, month, day, imperial });
