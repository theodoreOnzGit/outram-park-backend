// csl-reference.cjs: the citeproc-js reference that kovan-literature's CSL
// citations are verified against (GitHub #789). Run by scripts/csl-reference.sh.
//
// Input: tests/data/csl/items.json, the site .bib as CSL-JSON exactly as
// kovan-literature produces it (`kovan-cli references --csl-json-out`), plus
// the same style and locale files the port uses (kovan-literature/data/csl/).
// citeproc-js is Zotero's CSL engine.
//
// One document holds every item (engine.updateItems with all ids), so
// disambiguation sees the whole set, as in the port's test. For each item:
//   parenthetical  makeCitationCluster([{id}])
//   narrative      author-only cluster + " " + suppress-author cluster, the
//                  composition Zotero's word-processor plugins use
//   bibliography   the makeBibliography() entry
// HTML output. No normalisation is applied here; the Rust test normalises
// both sides the same way (see tests/csl_vs_citeproc_js.rs).
'use strict';
const fs = require('fs');
const path = require('path');
const CSL = require(process.env.CITEPROC_MODULE);

const lit = path.join(__dirname, '..', 'crates', 'kovan-literature');
const items = JSON.parse(fs.readFileSync(path.join(lit, 'tests/data/csl/items.json'), 'utf8'));
const style = fs.readFileSync(path.join(lit, 'data/csl/apa.csl'), 'utf8');
const locale = fs.readFileSync(path.join(lit, 'data/csl/locales-en-US.xml'), 'utf8');

const byId = {};
for (const it of items) byId[it.id] = it;
const sys = { retrieveLocale: () => locale, retrieveItem: (id) => byId[id] };
const engine = new CSL.Engine(sys, style, 'en-US');
engine.setOutputFormat('html');
const ids = items.map((i) => i.id);
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

const pkg = JSON.parse(fs.readFileSync(path.join(path.dirname(process.env.CITEPROC_MODULE), 'package.json'), 'utf8'));
const result = {
  meta: {
    engine: `citeproc-js ${pkg.version}`,
    node: process.version,
    style: 'kovan-literature/data/csl/apa.csl',
    locale: 'kovan-literature/data/csl/locales-en-US.xml',
  },
  bibliography_order: params.entry_ids.map((e) => e[0]),
  items: out,
};
fs.writeFileSync(path.join(lit, 'tests/data/csl/reference_apa.json'), JSON.stringify(result, null, 1) + '\n');
console.log(`csl-reference: ${ids.length} items, ${entries.length} bibliography entries`);
