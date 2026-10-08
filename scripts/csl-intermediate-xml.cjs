// csl-intermediate-xml.cjs: digests of the style XML tree as citeproc-js holds
// it after `new CSL.Engine` has built a style (`JSON.stringify(engine.cslXml.dataObj)`):
// parseXml, the preprocessing passes (addMissingNameNodes, addInstitutionNodes,
// insertPublisherAndPlace, flagDateMacros), the default sort-separator, and the
// `<date>` nodes replaced by fixDateNode during the build. GitHub #792.
// Companion of csl-intermediate-reference.cjs (it reuses that script's helpers,
// see csl-intermediate-reduced.cjs). Writes
// crates/kovan-literature/tests/data/csl/intermediate_reference_xml.json
'use strict';
const fs = require('fs');
const path = require('path');
const Module = require('module');
const file = path.join(__dirname, 'csl-intermediate-reference.cjs');
const src = fs.readFileSync(file, 'utf8');
const cut = src.indexOf('const args = process.argv.slice(2);');
if (cut < 0) throw new Error('csl-intermediate-reference.cjs changed shape');
const extra = `
function xmlEngineForFixture(name) {
  const test = parseFixture(name, path.join(fixtureDir, name + '.txt'));
  const sys = new RunnerSys(test);
  CSL.debug = function () {};
  const engine = new CSL.Engine(sys, test.CSL);
  return engine;
}
function xmlEngineForSite(st) {
  const siteItems = JSON.parse(fs.readFileSync(path.join(lit, 'tests/data/csl/items.json'), 'utf8'));
  const byId = {};
  for (const it of siteItems) byId[it.id] = it;
  const retrieveLocale = (lang) => {
    const p = path.join(lit, 'data/csl', 'locales-' + lang + '.xml');
    return fs.existsSync(p) ? fs.readFileSync(p, 'utf8') : false;
  };
  const style = fs.readFileSync(path.join(lit, 'data/csl', st.file), 'utf8');
  return new CSL.Engine({ retrieveLocale, retrieveItem: (id) => byId[id] }, style, st.locale || 'en-US');
}
module.exports = { CSL, sha, xmlEngineForFixture, xmlEngineForSite, SITE_STYLES, fixtureDir, outDir };
`;
const m = new Module(file, module);
m.filename = file;
m.paths = Module._nodeModulePaths(path.dirname(file));
m._compile(src.slice(0, cut) + extra, file);
const R = m.exports;
const crypto = require('crypto');
const dig = (e) => crypto.createHash('sha256').update(JSON.stringify(e.cslXml.dataObj)).digest('hex');
const names = fs.readdirSync(R.fixtureDir).filter((n) => /^[a-z]+_.*\.txt$/.test(n)).sort().map((n) => n.replace(/\.txt$/, ''));
const site = {};
for (const st of R.SITE_STYLES) site[st.name] = dig(R.xmlEngineForSite(st));
const fixtures = {};
for (const n of names) fixtures[n] = dig(R.xmlEngineForFixture(n));
fs.writeFileSync(path.join(R.outDir, 'intermediate_reference_xml.json'), JSON.stringify({ meta: { generator: 'scripts/csl-intermediate-xml.cjs', digest: 'SHA-256 of JSON.stringify(engine.cslXml.dataObj) after new CSL.Engine' }, site, fixtures }, null, 1) + '\n');
console.log(`csl-intermediate-xml: ${names.length} fixtures, ${R.SITE_STYLES.length} site styles`);
