// Probes for registered deviations D3 (oblique/light flip), D4 (short year, month leak)
// and D5 (BC-to-AD range era labels), GitHub #801 and #808. Records what citeproc-js
// 2.4.63 gives; the Rust tests (util_flipflop.rs, node_datepart.rs) pin it next to the
// port's intended output.
// Usage: CITEPROC_MODULE=<citeproc_commonjs.js> node scripts/csl-units/deviations_dqa.cjs
'use strict';
const fs = require('fs');
const path = require('path');
const { root, freshCSL, write } = require('./common.cjs');
const CSL = freshCSL();

const localeXml = fs.readFileSync(path.join(root, 'crates/kovan-literature/data/csl/locales-en-US.xml'), 'utf8')
  .split('\n').filter((l) => !l.trim().startsWith('<?')).join('\n');
const hdr = '<?xml version="1.0" encoding="utf-8"?><style xmlns="http://purl.org/net/xbiblio/csl" class="in-text" version="1.0"><info><id>x</id><title>x</title><updated>2026-10-08T00:00:00+00:00</updated></info>';
const style = (layoutAttrs, body) => `${hdr}<citation><layout ${layoutAttrs}>${body}</layout></citation></style>`;
const T = '<text variable="title"/>';
const issued = (inner) => `<date variable="issued">${inner}</date>`;
const MY = '<date-part name="month" suffix=" "/><date-part name="year" form="short"/>';
const items = [];
const cases = [];
function add(name, sty, item) {
  const id = 'ITEM-' + cases.length;
  items.push(Object.assign({ id, type: 'book' }, item));
  cases.push({ name, id, style: sty, noMonth03: !!item.noMonth03 });
}
// D3
add('oblique-i', style('font-style="oblique"', T), { title: 'a <i>b</i> c' });
add('light-b', style('font-weight="light"', T), { title: 'a <b>b</b> c' });
add('bold-b', style('font-weight="bold"', T), { title: 'a <b>b</b> c' });
add('italic-i', style('font-style="italic"', T), { title: 'a <i>b</i> c' });
// D4
for (const y of [99, 12345, 450, -5, 500, 2004]) {
  add('short-year-' + y, style('', issued(MY)), { issued: { 'date-parts': [[y, 3]] } });
}
add('short-year-only-99', style('', issued('<date-part name="year" form="short"/>')), { issued: { 'date-parts': [[99]] } });
add('missing-month-term', style('', issued('<date-part name="month" suffix=" "/><date-part name="year"/>')), { issued: { 'date-parts': [[2004, 3]] }, noMonth03: true });
add('missing-month-term-short', style('', issued('<date-part name="month" form="short" suffix=" "/><date-part name="year"/>')), { issued: { 'date-parts': [[2004, 3]] }, noMonth03: true });
// D5
const YM = '<date-part name="year"/><date-part name="month"/>';
const MDY = '<date-part name="month" suffix=" "/><date-part name="day" suffix=", "/><date-part name="year"/>';
add('range-year-first', style('', issued(YM).replace('<date ', '<date delimiter="-" ')), { issued: { 'date-parts': [[-200, 3], [200, 5]] } });
add('range-ready-mdy', style('', issued(MDY)), { issued: { 'date-parts': [[-200, 3, 1], [200, 5, 2]] } });
add('range-ready-mdy-both-ad', style('', issued(MDY)), { issued: { 'date-parts': [[100, 3, 1], [200, 5, 2]] } });
add('range-ready-mdy-both-bc', style('', issued(MDY)), { issued: { 'date-parts': [[-300, 3, 1], [-200, 5, 2]] } });
add('range-ready-mdy-ad-to-ad-big', style('', issued(MDY)), { issued: { 'date-parts': [[100, 3, 1], [600, 5, 2]] } });
add('range-ready-mdy-bc-to-bc-end-ad', style('', issued(MDY)), { issued: { 'date-parts': [[-5, 3, 1], [5, 5, 2]] } });

const sys = {
  retrieveItem: (id) => items.find((i) => i.id === id),
  retrieveLocale: () => (current.noMonth03 ? localeXml.split('\n').filter((l) => !l.includes('name="month-03"')).join('\n') : localeXml),
};
let current = {};
for (const c of cases) {
  current = items.find((i) => i.id === c.id);
  try {
    const e = new CSL.Engine(sys, c.style, 'en-US');
    c.js = { v: e.makeCitationCluster([{ id: c.id }]) };
  } catch (err) {
    c.js = { e: String((err && err.message) || err) };
  }
  console.log(c.name, JSON.stringify(c.js));
}
write('deviations_dqa', { items, cases });
