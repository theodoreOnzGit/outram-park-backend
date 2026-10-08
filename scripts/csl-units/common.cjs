// Shared helpers for the differential unit generators (scripts/csl-units/*.cjs).
// Usage: CITEPROC_MODULE=<path to citeproc_commonjs.js> node scripts/csl-units/<area>.cjs
'use strict';
const fs = require('fs');
const path = require('path');

const root = path.join(__dirname, '..', '..');
const fixtureDir = path.join(root, 'vendor/csl-test-suite/processor-tests/humans');
const outDir = path.join(root, 'crates/kovan-literature/tests/data/csl/units');

function freshCSL() {
  const p = require.resolve(process.env.CITEPROC_MODULE);
  delete require.cache[p];
  const CSL = require(p);
  CSL.debug = function () {};
  return CSL;
}

// All fixtures: { name, text } in directory order.
function fixtures() {
  return fs.readdirSync(fixtureDir).filter((f) => f.endsWith('.txt')).sort().map((f) => ({
    name: f.replace(/\.txt$/, ''),
    text: fs.readFileSync(path.join(fixtureDir, f), 'utf8'),
  }));
}

// The JSON of one fixture section, or undefined.
function section(text, name) {
  const re = new RegExp('>>=+ ' + name + ' =+>>\\n([\\s\\S]*?)<<=+ ' + name + ' =+<<');
  const m = re.exec(text);
  if (!m) return undefined;
  try {
    return JSON.parse(m[1]);
  } catch (e) {
    return undefined;
  }
}

function fixtureItems() {
  const out = [];
  for (const f of fixtures()) {
    const items = section(f.text, 'INPUT');
    if (Array.isArray(items)) for (const it of items) out.push({ fixture: f.name, item: it });
  }
  return out;
}

function write(area, obj) {
  fs.mkdirSync(outDir, { recursive: true });
  const file = path.join(outDir, area + '.json');
  fs.writeFileSync(file, JSON.stringify(obj) + '\n');
  console.log('wrote', file, fs.statSync(file).size, 'bytes');
}

// JSON-safe copy with undefined omitted (what the Rust side models).
function plain(x) {
  return x === undefined ? null : JSON.parse(JSON.stringify(x));
}

module.exports = { root, freshCSL, fixtures, section, fixtureItems, write, plain };
