// csl-reference.cjs: the citeproc-js reference that kovan-literature's CSL
// citations are verified against (GitHub #789, #791). Run by
// scripts/csl-reference.sh.
//
// Input: tests/data/csl/items.json, the site .bib as CSL-JSON exactly as
// kovan-literature produces it (`kovan-cli references --csl-json-out`), plus
// the same style and locale files the port uses (kovan-literature/data/csl/).
// citeproc-js (2.4.63, from npm, byte-identical to the vendored 73bc1b44
// source build; see scripts/csl-testsuite-reference.cjs) is Zotero's CSL
// engine.
//
// One document holds every item (engine.updateItems with all ids), so
// disambiguation sees the whole set, as in the port's test. For each item:
//   parenthetical  makeCitationCluster([{id}])
//   narrative      author-only cluster + " " + suppress-author cluster, the
//                  composition Zotero's word-processor plugins use
//   bibliography   the makeBibliography() entry
// HTML output. No normalisation is applied here; the Rust test normalises
// both sides the same way (see tests/csl_vs_citeproc_js.rs).
//
// Five styles (the site set), one reference file each:
//   apa                  -> tests/data/csl/reference_apa.json
//   chicago-author-date  -> reference_chicago-author-date.json
//   ieee                 -> reference_ieee.json
//   nature               -> reference_nature.json
//   vancouver            -> reference_vancouver.json, from
//                           nlm-citation-sequence.csl: the CSL repository
//                           has no independent "vancouver" style; its
//                           "Vancouver - NLM (citation-sequence)" is a
//                           dependent style whose parent is that file.
// Locales: retrieveLocale(lang) returns data/csl/locales-<lang>.xml when the
// file exists (en-US, en-GB: Nature's default-locale) and false otherwise,
// as the test runner's Sys.retrieveLocale does for a missing file. The APA
// reference only ever asks for en-US, so it is byte-identical to the one
// made with the single en-US locale this script used before #791.
'use strict';
const fs = require('fs');
const path = require('path');
const CSL = require(process.env.CITEPROC_MODULE);

const lit = path.join(__dirname, '..', 'crates', 'kovan-literature');
const items = JSON.parse(fs.readFileSync(path.join(lit, 'tests/data/csl/items.json'), 'utf8'));

const STYLES = [
  { name: 'apa', file: 'apa.csl' },
  { name: 'chicago-author-date', file: 'chicago-author-date.csl' },
  { name: 'ieee', file: 'ieee.csl' },
  { name: 'nature', file: 'nature.csl', locale: 'en-GB' },
  { name: 'vancouver', file: 'nlm-citation-sequence.csl' },
];

const byId = {};
for (const it of items) byId[it.id] = it;
const ids = items.map((i) => i.id);
const pkg = JSON.parse(fs.readFileSync(path.join(path.dirname(process.env.CITEPROC_MODULE), 'package.json'), 'utf8'));

function retrieveLocale(lang) {
  const p = path.join(lit, 'data/csl', `locales-${lang}.xml`);
  return fs.existsSync(p) ? fs.readFileSync(p, 'utf8') : false;
}

for (const st of STYLES) {
  const style = fs.readFileSync(path.join(lit, 'data/csl', st.file), 'utf8');
  const sys = { retrieveLocale, retrieveItem: (id) => byId[id] };
  const engine = new CSL.Engine(sys, style, st.locale || 'en-US');
  engine.setOutputFormat('html');
  engine.updateItems(ids);

  const out = {};
  for (const id of ids) {
    const paren = engine.makeCitationCluster([{ id }]);
    const author = engine.makeCitationCluster([{ id, 'author-only': true }]);
    const year = engine.makeCitationCluster([{ id, 'suppress-author': true }]);
    out[id] = { parenthetical: paren, narrative: `${author} ${year}` };
  }
  const [params, entries] = engine.makeBibliography();
  params.entry_ids.forEach((eid, n) => {
    out[eid[0]].bibliography = entries[n].trim();
  });

  const result = {
    meta: {
      engine: `citeproc-js ${pkg.version}`,
      node: process.version,
      style: `kovan-literature/data/csl/${st.file}`,
      locale: `kovan-literature/data/csl/locales-${st.locale || 'en-US'}.xml`,
    },
    bibliography_order: params.entry_ids.map((e) => e[0]),
    items: out,
  };
  fs.writeFileSync(path.join(lit, `tests/data/csl/reference_${st.name}.json`), JSON.stringify(result, null, 1) + '\n');
  console.log(`csl-reference: ${st.name}: ${ids.length} items, ${entries.length} bibliography entries`);
}
