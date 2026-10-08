// Reference data for the page/year range mangler (util_page.js, GitHub #793).
// Output: crates/kovan-literature/tests/data/csl/units/page.json
//   { formats, strings, runs: [{ t: "page"|"year", f: format|null, y: isyear,
//                                out: [output | {error}] aligned with strings }] }
'use strict';
const { CSL, makeEngine, fixtureStrings, writeUnits, prng } = require('./common.cjs');

const FORMATS = [null, 'expanded', 'minimal', 'minimal-two', 'chicago', 'chicago-15', 'chicago-16'];
const rand = prng(793793);
const pick = (a) => a[Math.floor(rand() * a.length)];

const ins = [];
// numeric pairs, systematically
const bases = [1, 2, 9, 10, 11, 42, 99, 100, 101, 108, 123, 199, 200, 321, 999, 1000, 1001, 1234, 1999, 2000, 2001, 2019, 9999, 10000, 10001, 12345, 99999, 100000];
const ends = [1, 2, 3, 5, 8, 9, 10, 11, 15, 20, 23, 25, 28, 33, 45, 55, 99, 100, 101, 110, 124, 128, 150, 200, 328, 400, 999, 1000, 1005, 2002, 2005, 2010, 20000, 102345];
for (const b of bases) {
  for (const e of ends) {
    if ((b * 31 + e * 17) % 3 === 0) ins.push(`${b}-${e}`);
  }
}
const dashes = ['-', '–', '—', ' - ', ' – ', '--', '\\-'];
for (const d of dashes) {
  for (const [b, e] of [[42, 45], [321, 328], [1999, 2005], [100, 104], [95, 101], [5, 3], [7, 7], [12, 120]]) {
    ins.push(`${b}${d}${e}`);
  }
}
const prefixes = ['A', 'S', 'E', 'iv', 'ii', 'B0', 'AB', 'p', 'vol', 'S0'];
for (const p of prefixes) {
  for (const [b, e] of [[1, 5], [10, 12], [321, 328], [99, 101], [3, 15]]) {
    ins.push(`${p}${b}-${p}${e}`);
    ins.push(`${p}${b}-${e}`);
    ins.push(`${b}-${p}${e}`);
    ins.push(`${p}${b}–${p}${e}`);
  }
}
const sufs = ['a', 'b', 'ab'];
for (const s of sufs) {
  ins.push(`12${s}-14${s}`, `12${s}-14`, `12-14${s}`, `100${s}-104`, `9${s}-10${s}`);
}
const texts = [
  '', 'xyz', '12', '12, 14', '1-5, 10-12', '1-5, 10-12, 20-30', 'see 12-14 and 20-22', 'pp. 12-14',
  '12-14, 16', '12-14; 16-18', '-5', '5-', '5--6', '5 - ', ' - 5', '12 -14', '12- 14', '12 - 14', '12–14',
  '12 – 14', 'a-b', 'a-b-c', '1-2-3', '1-2-3-4', '1–2–3', '007-009', '0007-0009', '10-09', '100-099',
  'x.1-x.5', '3.14-3.15', '1,000-1,005', '12-14-16', 'S1-S5, 10-12', 'A1-A5, B1-B5', 'a1-a5',
  '2005-2010', '1999-2000', '1999-00', '1999-02', '1899-1901', '2099-2101', '999-1001', 'c.1900-1910',
  '1\\-2', 'a\\-b 12-14', '12-14\\-', '12\\-14', 'foo - bar', 'Roman iv-vi', 'xiv-xvi', 'I-V',
  '00-05', '01-02', '1-02', '1-002', '0-0', '0-1', '1-1', '1-0', '199-200', '1-10', '9-10', '19-20', '99-100',
  '1000-1', '1000-10', '10000-1', '12345-6', '12345-67', '12345-678', '12345-6789', '12345-67890',
  '12 - 14 - 16', '12–14', '12—14', '12‑14', '12−14', '1 –2', '1– 2',
];
ins.push(...texts);
for (const s of fixtureStrings(/^(page|number|volume|issue|edition|section|locator)$/)) ins.push(s);
// random mixes
for (let i = 0; i < 600; i++) {
  let s = '';
  const k = 1 + Math.floor(rand() * 3);
  for (let j = 0; j < k; j++) {
    if (j) s += pick([', ', '; ', ' and ', ' ']);
    if (rand() < 0.15) s += pick(['A', 'S', 'ii', 'e']);
    s += String(Math.floor(rand() * pick([20, 200, 2000, 20000])));
    if (rand() < 0.1) s += pick(['a', 'b']);
    if (rand() < 0.8) {
      s += pick(dashes);
      if (rand() < 0.15) s += pick(['A', 'S', 'ii', 'e']);
      s += String(Math.floor(rand() * pick([20, 200, 2000, 20000])));
    }
  }
  ins.push(s);
}
const strings = [...new Set(ins)];

const runs = [];
let total = 0;
let errors = 0;
for (const t of ['page', 'year']) {
  for (const f of FORMATS) {
    const e = makeEngine();
    if (f) e.opt[`${t}-range-format`] = f; else delete e.opt[`${t}-range-format`];
    const fn = CSL.Util.PageRangeMangler.getFunction(e, t);
    for (const y of [false, true]) {
      const out = strings.map((s) => {
        try { return fn(s, y); } catch (err) { errors++; return { error: String(err) }; }
      });
      total += out.length;
      runs.push({ t, f, y, out });
    }
  }
}
writeUnits('page', { formats: FORMATS, strings, runs });
console.error('page strings:', strings.length, 'cases:', total, 'errors:', errors);
