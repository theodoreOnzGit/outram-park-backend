// Differential reference for the citeproc port's engine (GitHub #795, #796):
// the registry, sorting, disambiguation modes that need no name rendering,
// the cite-position machinery of processCitationCluster, makeCitationCluster
// and makeBibliography. Writes crates/kovan-literature/tests/data/csl/units/engine.json.
//
// Each scenario is a style (layouts that render nothing, so the reference needs
// no rendering stage), items and a list of operations; after each operation the
// snapshot records what citeproc-js holds: the registry tokens (seq, ambig key,
// sort keys, disambig), the reflist, every citation's sortedItems with the
// position data processCitationCluster set, citationsByItemId, and what the
// call returned.
//
// Run by hand: CITEPROC_MODULE=<citeproc_commonjs.js> node scripts/csl-units/engine.cjs
'use strict';
const fs = require('fs');
const path = require('path');
const { freshCSL, write, root } = require('./common.cjs');

const LOCALE = fs
  .readFileSync(path.join(root, 'crates/kovan-literature/data/csl/locales-en-US.xml'), 'utf8')
  .split('\n')
  .filter((l) => !l.trimStart().startsWith('<?'))
  .join('\n');

let seed = 0x1234abcd;
function rnd() { seed |= 0; seed = (seed + 0x6d2b79f5) | 0; let t = Math.imul(seed ^ (seed >>> 15), 1 | seed); t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t; return ((t ^ (t >>> 14)) >>> 0) / 4294967296; }
const pick = (a) => a[Math.floor(rnd() * a.length)];

const PUBLISHERS = ['Zed Press', 'alpha books', 'Alpha Books', 'The Beta House', 'beta house', 'Gamma', '', undefined];
const EDITIONS = ['1', '2', '10', '3', undefined, '02'];
const GENRES = ['report', 'Thesis', 'thesis', undefined];
const YEARS = [1999, 2000, 2000, 2001, 1850, undefined];
function makeItems(n) {
  const items = [];
  for (let i = 0; i < n; i++) {
    const it = { id: 'ITEM-' + (i + 1), type: 'book' };
    const p = pick(PUBLISHERS); if (p !== undefined) it.publisher = p;
    const e = pick(EDITIONS); if (e !== undefined) it.edition = e;
    const g = pick(GENRES); if (g !== undefined) it.genre = g;
    const y = pick(YEARS); if (y !== undefined) it.issued = { 'date-parts': [[y]] };
    items.push(it);
  }
  return items;
}

function style({ cls = 'in-text', citationAttrs = '', citationSort = '', bibSort = '', layout = '<layout/>', bibAttrs = '', bibLayout = '<layout/>' }) {
  return `<style xmlns="http://purl.org/net/xbiblio/csl" class="${cls}" version="1.0">
  <citation ${citationAttrs}>${citationSort ? '<sort>' + citationSort + '</sort>' : ''}${layout}</citation>
  <bibliography ${bibAttrs}>${bibSort ? '<sort>' + bibSort + '</sort>' : ''}${bibLayout}</bibliography>
</style>`;
}
const POSITION_LAYOUT = '<layout><choose><if position="ibid-with-locator"/><else-if position="ibid"/><else-if position="subsequent"/><else/></choose></layout>';

const STYLES = {
  plain: style({ bibSort: '<key variable="publisher"/><key variable="edition" sort="descending"/>' }),
  datesort: style({ bibSort: '<key variable="issued"/><key variable="publisher"/>', citationSort: '<key variable="issued" sort="descending"/>' }),
  citnum: style({ bibSort: '<key variable="citation-number" sort="descending"/>' }),
  suffix: style({ citationAttrs: 'disambiguate-add-year-suffix="true"', bibSort: '<key variable="publisher"/>' }),
  note: style({ cls: 'note', layout: POSITION_LAYOUT, bibSort: '<key variable="genre"/>' }),
  notenear: style({ cls: 'note', citationAttrs: 'near-note-distance="2"', layout: POSITION_LAYOUT }),
  intextpos: style({ cls: 'in-text', layout: POSITION_LAYOUT, citationSort: '<key variable="publisher"/>' }),
  collapse: style({ citationAttrs: 'collapse="year"', citationSort: '<key variable="publisher"/>', bibSort: '<key variable="publisher"/>' }),
  nobib: '<style xmlns="http://purl.org/net/xbiblio/csl" class="in-text" version="1.0"><citation><layout/></citation></style>',
};

// Generated citationIDs ("a" + base 32) are random in citeproc-js; compare them as one id.
const gen = (id) => (typeof id === 'string' && /^a[0-9a-v]{6,}$/.test(id) ? 'GEN' : id);

function snapshot(engine) {
  const reg = engine.registry;
  const norm = (v) => (v === false || v === undefined ? undefined : v);
  const tokens = {};
  for (const id in reg.registry) {
    const t = reg.registry[id];
    tokens[id] = {
      seq: t.seq, ambig: norm(t.ambig) === undefined ? undefined : String(t.ambig), sortkeys: norm(t.sortkeys), newItem: norm(t.newItem), offset: t.offset,
      disambig: t.disambig ? { names: t.disambig.names, givens: t.disambig.givens, year_suffix: norm(t.disambig.year_suffix), disambiguate: t.disambig.disambiguate } : undefined,
      frnn: t['first-reference-note-number'], fcrnn: t['first-container-reference-note-number'], count: t['citation-count'],
    };
  }
  const citations = reg.citationreg.citationByIndex.map((c) => ({
    id: gen(c.citationID), noteIndex: c.properties.noteIndex, index: c.properties.index,
    sorted: (c.sortedItems || []).map((pair) => ({
      id: pair[0].id, cid: pair[1].id, position: pair[1].position, frnn: pair[1]['first-reference-note-number'],
      fcrnn: pair[1]['first-container-reference-note-number'], near: pair[1]['near-note'], sortkeys: pair[1].sortkeys,
      locator: pair[1].locator, label: pair[1].label, xloc: pair[1]['locator-extra'],
    })),
  }));
  const byItem = {};
  if (reg.citationreg.citationsByItemId) for (const k in reg.citationreg.citationsByItemId) byItem[k] = reg.citationreg.citationsByItemId[k].map((c) => gen(c.citationID));
  return JSON.parse(JSON.stringify({ reflist: reg.reflist.map((t) => t.id), tokens, citations, byItem: reg.citationreg.citationsByItemId ? byItem : undefined, citationById: Object.keys(reg.citationreg.citationById).map(gen).sort() }));
}

function run(sc) {
  const CSL = freshCSL();
  const items = {};
  for (const it of sc.items) items[it.id] = JSON.parse(JSON.stringify(it));
  const sys = {
    retrieveItem: (id) => items[id],
    retrieveLocale: (lang) => (lang === 'en-US' ? LOCALE : false),
    variableWrapper: undefined,
  };
  const out = [];
  let engine;
  try { engine = new CSL.Engine(sys, sc.style, 'en-US'); } catch (e) { return { error: String(e) }; }
  for (const op of sc.ops) {
    const rec = { op: op.op };
    try {
      if (op.op === 'update') { engine.updateItems(op.ids.slice(), op.nosort); }
      else if (op.op === 'process') {
        const c = JSON.parse(JSON.stringify(op.citation));
        const r = engine.processCitationCluster(c, op.pre, op.post);
        rec.ret = r[1].map((x) => [x[0], x[1], gen(x[2])]); rec.bibchange = r[0].bibchange;
      } else if (op.op === 'append') {
        rec.ret = engine.appendCitationCluster(JSON.parse(JSON.stringify(op.citation))).map((x) => [x[0], x[1], gen(x[2])]);
      } else if (op.op === 'make') {
        rec.text = engine.makeCitationCluster(JSON.parse(JSON.stringify(op.items)));
      } else if (op.op === 'bib') {
        const b = op.section ? engine.makeBibliography(JSON.parse(JSON.stringify(op.section))) : engine.makeBibliography();
        rec.bib = b === false ? false : { params: JSON.parse(JSON.stringify(b[0])), entries: b[1] };
      } else if (op.op === 'preview') {
        rec.text = engine.previewCitationCluster(JSON.parse(JSON.stringify(op.citation)), op.pre, op.post, 'html');
      } else if (op.op === 'replace') {
        for (const k in items) delete items[k];
        for (const it of op.items) items[it.id] = JSON.parse(JSON.stringify(it));
      }
    } catch (e) { rec.error = String(e && e.message ? e.message : e).split('\n')[0]; }
    rec.snap = snapshot(engine);
    out.push(rec);
  }
  return { steps: out };
}

function citation(id, note, ids, extra) {
  const c = { citationID: id, citationItems: ids.map((i) => (typeof i === 'string' ? { id: i } : i)), properties: { noteIndex: note } };
  if (extra) Object.assign(c.properties, extra);
  return c;
}

const scenarios = [];
function add(name, styleName, items, ops) { scenarios.push({ name, styleName, style: STYLES[styleName], items, ops }); }

// ---- registry and bibliography ordering ----
for (const sname of ['plain', 'datesort', 'citnum', 'suffix', 'note', 'collapse']) {
  for (let k = 0; k < 6; k++) {
    const items = makeItems(4 + k);
    const ids = items.map((i) => i.id);
    const shuffled = ids.slice().sort(() => rnd() - 0.5);
    add(`order-${sname}-${k}`, sname, items, [
      { op: 'update', ids: shuffled, nosort: false },
      { op: 'bib' },
      { op: 'update', ids: shuffled.slice(1), nosort: false },
      { op: 'update', ids: ids.concat(['ITEM-1']), nosort: false },
      { op: 'update', ids: shuffled.slice(0, 3), nosort: true },
      { op: 'bib' },
      { op: 'replace', items: makeItems(items.length) },
      { op: 'update', ids: ids, nosort: false },
      { op: 'bib', section: { include: [{ field: 'genre', value: 'report' }] } },
      { op: 'bib', section: { exclude: [{ field: 'publisher', value: true }] } },
      { op: 'bib', section: { select: [{ field: 'genre', value: true }, { field: 'edition', value: '2' }] } },
      { op: 'bib', section: { quash: [{ field: 'genre', value: 'thesis' }] } },
    ]);
  }
}
// ---- makeCitationCluster ----
for (const sname of ['plain', 'intextpos', 'datesort', 'collapse']) {
  for (let k = 0; k < 4; k++) {
    const items = makeItems(5);
    const ids = items.map((i) => i.id);
    const cites = (n) => Array.from({ length: n }, () => ({ id: pick(ids), locator: pick([undefined, '12', 'p. 3', '4-5', ' 7 ', 'ch. 2']), label: pick([undefined, 'page', 'chapter']), prefix: pick([undefined, 'see ', 'cf. ']), suffix: pick([undefined, ', passim', '.']) }));
    add(`make-${sname}-${k}`, sname, items, [
      { op: 'update', ids: ids, nosort: false },
      { op: 'make', items: cites(1) },
      { op: 'make', items: cites(3) },
      { op: 'make', items: cites(4) },
    ]);
  }
}
// ---- processCitationCluster: positions, in note and in-text styles ----
function document(items, nCites, notes) {
  const ids = items.map((i) => i.id);
  const doc = [];
  let note = 0;
  for (let i = 0; i < nCites; i++) {
    if (notes) { if (rnd() < 0.75) note += 1 + (rnd() < 0.3 ? 1 : 0) + (rnd() < 0.15 ? 3 : 0); }
    const n = 1 + Math.floor(rnd() * 3);
    const cites = Array.from({ length: n }, () => {
      const c = { id: pick(ids) };
      if (rnd() < 0.5) c.locator = pick(['12', '13', 'p. 3', '4-5']);
      if (rnd() < 0.3) c.label = pick(['page', 'chapter']);
      if (rnd() < 0.1) c['author-only'] = true;
      return c;
    });
    doc.push(citation('C' + (i + 1), notes ? (rnd() < 0.85 ? note : 0) : 0, cites, rnd() < 0.1 ? { unsorted: true } : undefined));
  }
  return doc;
}
for (const sname of ['note', 'notenear', 'intextpos', 'plain', 'suffix']) {
  for (let k = 0; k < 8; k++) {
    const items = makeItems(4);
    const doc = document(items, 3 + Math.floor(rnd() * 5), sname.startsWith('note'));
    const ops = [];
    for (let i = 0; i < doc.length; i++) {
      const pre = doc.slice(0, i).map((c) => [c.citationID, c.properties.noteIndex]);
      ops.push({ op: 'process', citation: doc[i], pre, post: [] });
    }
    // re-edit a middle citation with a changed item list, with citations after it
    if (doc.length > 2) {
      const mid = 1;
      const edited = JSON.parse(JSON.stringify(doc[mid]));
      edited.citationItems = [{ id: pick(items).id }];
      ops.push({ op: 'process', citation: edited, pre: doc.slice(0, mid).map((c) => [c.citationID, c.properties.noteIndex]), post: doc.slice(mid + 1).map((c) => [c.citationID, c.properties.noteIndex]) });
      // delete the last citation from the document by processing the first with pre/post lacking it
      ops.push({ op: 'process', citation: doc[0], pre: [], post: doc.slice(1, doc.length - 1).map((c) => [c.citationID, c.properties.noteIndex]) });
    }
    ops.push({ op: 'bib' });
    ops.push({ op: 'preview', citation: citation('PREV', 0, [{ id: items[0].id }]), pre: doc.slice(0, 2).map((c) => [c.citationID, c.properties.noteIndex]), post: [] });
    ops.push({ op: 'append', citation: citation('APP', doc.length ? doc[doc.length - 1].properties.noteIndex + 1 : 1, [{ id: items[1].id }]) });
    add(`doc-${sname}-${k}`, sname, items, ops);
  }
}
// ---- hand-written edge cases ----
const edgeItems = makeItems(3);
add('edge-no-bibliography', 'nobib', edgeItems, [{ op: 'update', ids: edgeItems.map((i) => i.id), nosort: false }, { op: 'bib' }]);
add('edge-unknown-item', 'plain', edgeItems, [{ op: 'update', ids: ['ITEM-1', 'NOPE'], nosort: false }, { op: 'make', items: [{ id: 'NOPE' }] }, { op: 'process', citation: citation('X1', 0, ['NOPE']), pre: [], post: [] }]);
add('edge-duplicate-citation-id', 'note', edgeItems, [
  { op: 'process', citation: citation('D1', 1, ['ITEM-1']), pre: [], post: [] },
  { op: 'process', citation: citation('D2', 2, ['ITEM-1']), pre: [['D1', 1]], post: [] },
  { op: 'process', citation: citation('D2', 2, ['ITEM-2']), pre: [['D1', 1], ['D2', 2]], post: [] },
  { op: 'process', citation: citation('D3', 3, ['ITEM-1']), pre: [['D1', 1]], post: [['D9', 4]] },
]);
add('edge-no-ids-no-properties', 'plain', edgeItems, [
  { op: 'process', citation: { citationItems: [{ id: 'ITEM-1' }] }, pre: [], post: [] },
  { op: 'process', citation: { citationID: 'N1', citationItems: [{ id: 'ITEM-2' }] }, pre: [], post: [] },
]);
add('edge-composite', 'plain', edgeItems, [
  { op: 'process', citation: citation('K1', 0, ['ITEM-1'], { mode: 'composite', infix: ', ' }), pre: [], post: [] },
  { op: 'process', citation: citation('K2', 0, [{ id: 'ITEM-2', 'author-only': true }], { mode: 'author-only' }), pre: [['K1', 0]], post: [] },
  { op: 'process', citation: citation('K3', 0, ['ITEM-3'], { mode: 'suppress-author', prefix: 'see ', suffix: ' ok' }), pre: [['K1', 0], ['K2', 0]], post: [] },
]);

const results = [];
for (const sc of scenarios) {
  const r = run(sc);
  results.push({ name: sc.name, style: sc.style, items: sc.items, ops: sc.ops, result: r });
}
write('engine', { meta: { engine: 'citeproc-js 2.4.63', scenarios: results.length, node: process.version }, scenarios: results });
