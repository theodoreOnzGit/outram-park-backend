// Shared helpers for the citeproc-js differential unit-test generators in this
// directory (GitHub #793). Each generator writes
// crates/kovan-literature/tests/data/csl/units/<area>.json, which a Rust test
// in src/citeproc/ replays. Run by hand:
//   export CITEPROC_MODULE=<repo>/target/csl-reference/node_modules/citeproc/citeproc_commonjs.js
//   node scripts/csl-units/<area>.cjs
'use strict';
const fs = require('fs');
const path = require('path');
const CSL = require(process.env.CITEPROC_MODULE);

const root = path.join(__dirname, '..', '..');
const FIXTURES = path.join(root, 'vendor', 'csl-test-suite', 'processor-tests', 'humans');
const OUT = path.join(root, 'crates', 'kovan-literature', 'tests', 'data', 'csl', 'units');

function readLocale(lang) {
  const p = path.join(root, 'vendor', 'citeproc-js', 'locale', `locales-${lang}.xml`);
  return fs.existsSync(p) ? fs.readFileSync(p, 'utf8') : false;
}

const STYLE = `<style xmlns="http://purl.org/net/xbiblio/csl" class="note" version="1.0"><info><id/><title/><updated>2009-08-10T04:49:00+09:00</updated></info><citation><layout><text variable="title"/></layout></citation></style>`;

// An en-US engine, the state the Rust replay starts from.
function makeEngine(opts = {}) {
  const sys = { retrieveLocale: readLocale, retrieveItem: (id) => ({ id, type: 'book', title: 'x' }) };
  // Must exist before construction: build.js defines CSL.VARIABLE_WRAPPER_PREPUNCT_REX only then.
  if (opts.variableWrapper) sys.variableWrapper = opts.variableWrapper;
  const e = new CSL.Engine(sys, STYLE, 'en-US');
  e.setOutputFormat(opts.format || 'html');
  e.tmp.area = opts.area || 'citation';
  if (opts.punctInQuote !== undefined) {
    e.locale[e.opt.lang].opts['punctuation-in-quote'] = !!opts.punctInQuote;
    e.output.adjust = new CSL.Output.Queue.adjust(!!opts.punctInQuote);
  }
  return e;
}

// Every string-valued field of every INPUT item in the fixtures that could
// reach text-case or the flip-flopper, de-duplicated, in file order.
function fixtureStrings(keys) {
  const keep = keys || /^(title|container-title|title-short|publisher|genre|medium|event|collection-title|note|authority|archive|abstract|number|page|volume|issue|edition|section|source|dimensions|status|version|call-number|publisher-place|event-place|jurisdiction)$/;
  const seen = new Set();
  const out = [];
  for (const f of fs.readdirSync(FIXTURES).sort()) {
    if (!f.endsWith('.txt')) continue;
    const txt = fs.readFileSync(path.join(FIXTURES, f), 'utf8');
    const m = txt.match(/>>=+ INPUT =+>>\n([\s\S]*?)<<=+ INPUT =+<</);
    if (!m) continue;
    let data;
    try { data = JSON.parse(m[1]); } catch (e) { continue; }
    (function walk(v, k) {
      if (typeof v === 'string') {
        if (k && keep.test(k) && !seen.has(v)) { seen.add(v); out.push(v); }
      } else if (Array.isArray(v)) {
        for (const x of v) walk(x, k);
      } else if (v && typeof v === 'object') {
        for (const kk of Object.keys(v)) walk(v[kk], kk);
      }
    })(data, null);
  }
  return out;
}

// Serialise a blob (CSL.Blob / NumericBlob / plain array) to plain JSON.
function ser(b, opts = {}) {
  if (typeof b === 'string') return { str: b };
  if (Array.isArray(b)) return { arr: b.map((x) => ser(x, opts)) };
  const o = { s: {} };
  for (const k of Object.keys(b.strings || {})) {
    if (b.strings[k] !== undefined) o.s[k] = b.strings[k];
  }
  o.d = (b.decorations || []).map((d) => d.slice());
  if (opts.alldecor) o.a = (b.alldecor || []).map((set) => (set || []).map((d) => d.slice()));
  if (typeof b.blobs === 'string') o.t = b.blobs;
  else o.t = (b.blobs || []).map((x) => ser(x, opts));
  if (typeof b.num !== 'undefined') o.num = b.num;
  if (b.status !== undefined && typeof b.num !== 'undefined') o.status = b.status;
  if (b.punctuation_in_quote !== undefined) o.piq = b.punctuation_in_quote;
  if (b.particle) o.particle = b.particle;
  return o;
}

// Serialise the result of Queue.string / renderBlobs.
function serRes(r, opts) {
  if (typeof r === 'string') return { str: r };
  if (Array.isArray(r)) return { arr: r.map((x) => serRes(x, opts)) };
  return { blob: ser(r, opts) };
}

function writeUnits(name, data) {
  fs.mkdirSync(OUT, { recursive: true });
  fs.writeFileSync(path.join(OUT, name + '.json'), JSON.stringify(data) + '\n');
  console.error(`${name}: wrote ${OUT}/${name}.json`);
}

// Deterministic PRNG (mulberry32) so regenerated data is reproducible.
function prng(seed) {
  let a = seed >>> 0;
  return function () {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

module.exports = { CSL, makeEngine, fixtureStrings, ser, serRes, writeUnits, prng, FIXTURES };
