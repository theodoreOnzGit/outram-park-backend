// Differential reference for the processNumber-based condition closures of
// src/citeproc/attributes.rs: @is-numeric, @locator, @page, @number
// (attributes.js:36-62, 91-104, 427-473). Each case builds the closure with
// CSL.Attributes[attr].call(token, engine, arg) on a fresh token and calls
// token.tests[0](Item, item) with tmp.shadow_numbers reset, as a cs:if would.
// Items and cite items are those of the CSL test suite's fixtures.
// Output: crates/kovan-literature/tests/data/csl/units/attribute_tests.json
'use strict';
const fs = require('fs');
const path = require('path');
const { root, freshCSL, fixtures, section, write, plain } = require('./common.cjs');
const CSL = freshCSL();
const localeDir = path.join(root, 'vendor/citeproc-js/locale');
const STYLE = `<style xmlns="http://purl.org/net/xbiblio/csl" class="in-text" version="1.0">
  <info><id/><title/><updated>2009-08-10T04:49:00+09:00</updated></info>
  <citation><layout><text variable="title"/></layout></citation>
</style>`;

function mkEngine(lang) {
  const sys = {
    retrieveLocale: (l) => {
      try { return fs.readFileSync(path.join(localeDir, 'locales-' + l + '.xml'), 'utf8').replace(/\s*<\?[^>]*\?>\s*\n/g, ''); } catch (e) { return false; }
    },
    retrieveItem: () => null,
  };
  return new CSL.Engine(sys, STYLE, lang);
}

const items = [];
const cites = [];
const seenI = new Set();
const seenC = new Set();
for (const f of fixtures()) {
  const input = section(f.text, 'INPUT');
  if (Array.isArray(input)) {
    for (const it of input) {
      const k = JSON.stringify(it);
      if (!seenI.has(k)) { seenI.add(k); items.push(it); }
    }
  }
  const lists = [];
  const ci = section(f.text, 'CITATION-ITEMS');
  if (Array.isArray(ci)) for (const l of ci) lists.push(l);
  const cs = section(f.text, 'CITATIONS');
  if (Array.isArray(cs)) for (const c of cs) if (c && c[0] && c[0].citationItems) lists.push(c[0].citationItems);
  for (const l of lists) for (const c of l) {
    if (!c || c.locator === undefined) continue;
    const k = JSON.stringify(c);
    if (!seenC.has(k)) { seenC.add(k); cites.push(c); }
  }
}

const NUMERIC = ['volume', 'issue', 'number', 'edition', 'page', 'locator', 'title', 'version', 'number-of-pages', 'chapter-number', 'collection-number'];
const LABELS = ['page', 'chapter', 'section', 'volume', 'number', 'paragraph', 'sub-verbo', 'line', 'issue', 'figure', 'book', 'folio'];

function run(e, attr, arg, Item, item) {
  const tok = new CSL.Token('if', CSL.START);
  CSL.Attributes[attr].call(tok, e, arg);
  e.tmp.shadow_numbers = {};
  try {
    return { r: tok.tests[0](Item, item) };
  } catch (err) {
    return { error: String(err && err.message ? err.message : err) };
  }
}

const cases = [];
for (const lang of ['en-US', 'de-DE']) {
  const e = mkEngine(lang);
  for (const it of items) {
    for (const v of NUMERIC) {
      if (it[v] === undefined) continue;
      cases.push(Object.assign({ lang, attr: '@is-numeric', arg: v, Item: plain(it), item: null }, run(e, '@is-numeric', v, it, undefined)));
    }
    if (it.page !== undefined) for (const l of LABELS) cases.push(Object.assign({ lang, attr: '@page', arg: l, Item: plain(it), item: null }, run(e, '@page', l, it, undefined)));
    if (it.number !== undefined) for (const l of ['number', 'issue', 'volume', 'page']) cases.push(Object.assign({ lang, attr: '@number', arg: l, Item: plain(it), item: null }, run(e, '@number', l, it, undefined)));
  }
  for (const c of cites) {
    const Item = { id: 'x', type: 'book', title: 'T' };
    for (const l of LABELS) cases.push(Object.assign({ lang, attr: '@locator', arg: l, Item, item: plain(c) }, run(e, '@locator', l, Item, c)));
    for (const v of ['locator', 'locator-extra']) cases.push(Object.assign({ lang, attr: '@is-numeric', arg: v, Item, item: plain(c) }, run(e, '@is-numeric', v, Item, c)));
  }
}
write('attribute_tests', { generator: 'scripts/csl-units/attribute_tests.cjs', engine: 'citeproc-js 2.4.63', cases });
console.log(cases.length, 'cases');
