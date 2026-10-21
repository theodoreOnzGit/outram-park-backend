// csl-intermediate-reduced.cjs: the digests of the REDUCED style section of
// citeproc-js 2.4.63's intermediate dump, for the 845 fixtures and the 5 site
// styles. GitHub #792, epic #790. Companion of csl-intermediate-reference.cjs,
// whose dump it reuses: the reduced form is that script's `style` section with
// the closure counts (`execs_n`, `tests_n`, `has_test`) removed from every
// token, so that a port whose node builders are not wired yet can still be
// compared on everything else (token names, types, strings, decorations,
// variables, jump indices, macros, opt).
//
// It loads csl-intermediate-reference.cjs's helpers without running its CLI
// (everything before `const args = process.argv`), so the two cannot drift.
//
//   CITEPROC_MODULE=.../citeproc_commonjs.js node scripts/csl-intermediate-reduced.cjs
//
// writes crates/kovan-literature/tests/data/csl/intermediate_reference_reduced.json
'use strict';
const fs = require('fs');
const path = require('path');
const Module = require('module');

const file = path.join(__dirname, 'csl-intermediate-reference.cjs');
const src = fs.readFileSync(file, 'utf8');
const cut = src.indexOf('const args = process.argv.slice(2);');
if (cut < 0) throw new Error('csl-intermediate-reference.cjs changed shape');
const patched =
  src.slice(0, cut) +
  '\nmodule.exports = { CSL, canon, sha, jsonOf, fixtureCase, siteCase, SITE_STYLES, fixtureDir, outDir, dateParserIsIdempotent };\n';
const m = new Module(file, module);
m.filename = file;
m.paths = Module._nodeModulePaths(path.dirname(file));
m._compile(patched, file);
const R = m.exports;

function stripClosureCounts(v) {
  if (Array.isArray(v)) return v.map(stripClosureCounts);
  if (v && typeof v === 'object') {
    const out = {};
    const isToken = Object.prototype.hasOwnProperty.call(v, 'tokentype');
    for (const k of Object.keys(v)) {
      if (isToken && (k === 'execs_n' || k === 'tests_n' || k === 'has_test')) continue;
      out[k] = stripClosureCounts(v[k]);
    }
    return out;
  }
  return v;
}

const names = fs.readdirSync(R.fixtureDir).filter((n) => /^[a-z]+_.*\.txt$/.test(n)).sort().map((n) => n.replace(/\.txt$/, ''));
const site = {};
for (const st of R.SITE_STYLES) {
  const d = R.siteCase(st);
  site[st.name] = d.error ? { error: d.error } : { style_reduced: R.sha(stripClosureCounts(d.style)) };
}
const fixtures = {};
for (const n of names) {
  const d = R.fixtureCase(n);
  fixtures[n] = d.error ? { error: d.error } : { style_reduced: R.sha(stripClosureCounts(d.style)) };
}
const meta = {
  engine: 'citeproc-js ' + JSON.parse(fs.readFileSync(path.join(path.dirname(process.env.CITEPROC_MODULE), 'package.json'), 'utf8')).version,
  node: process.version,
  generator: 'scripts/csl-intermediate-reduced.cjs',
  digest: 'SHA-256 of JSON.stringify of the style section with execs_n, tests_n and has_test removed from every token',
  fixtures: names.length,
};
fs.writeFileSync(path.join(R.outDir, 'intermediate_reference_reduced.json'), JSON.stringify({ meta, site, fixtures }, null, 1) + '\n');
console.log(`csl-intermediate-reduced: ${names.length} fixtures, ${R.SITE_STYLES.length} site styles`);
