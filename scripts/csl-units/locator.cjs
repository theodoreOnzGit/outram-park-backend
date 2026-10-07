// Differential reference for src/citeproc/util_static_locator.rs (remapSectionVariable,
// setNumberLabels, parseLocator, the citation-item input steps) and util_label.rs
// (evaluateLabel, castLabel). Output: tests/data/csl/units/locator.json
'use strict';
const fs = require('fs');
const path = require('path');
const { root, freshCSL, write, plain } = require('./common.cjs');
const CSL = freshCSL();
const localeDir = path.join(root, 'vendor/citeproc-js/locale');
const STYLE = `<style xmlns="http://purl.org/net/xbiblio/csl" class="in-text" version="1.0">
  <info><id/><title/><updated>2009-08-10T04:49:00+09:00</updated></info>
  <citation><layout><text variable="title"/></layout></citation>
</style>`;

function mkEngine(lang, ext) {
  const sys = {
    retrieveLocale: (l) => {
      try { return fs.readFileSync(path.join(localeDir, 'locales-' + l + '.xml'), 'utf8').replace(/\s*<\?[^>]*\?>\s*\n/g, ''); } catch (e) { return false; }
    },
    retrieveItem: () => null,
  };
  const e = new CSL.Engine(sys, STYLE, lang);
  e.setLangPrefsForCites({ persons: ['translit'], institutions: ['translit'], titles: ['translit', 'translat'], journals: ['translit'], publishers: ['translat'], places: ['translat'] });
  for (const k of Object.keys(ext || {})) e.opt.development_extensions[k] = ext[k];
  const log = {};
  const orig = e.getTerm;
  e.getTerm = function (term, form, plural, gender, mode, force) {
    const key = [term === undefined ? '' : term, typeof form === 'string' ? form : '~', typeof plural === 'number' ? plural : 0,
      (typeof gender === 'string' && gender) ? gender : '~', typeof mode === 'number' ? mode : '~', force ? 1 : 0].join('|');
    const r = orig.apply(this, arguments);
    log[key] = r === undefined ? null : r;
    return r;
  };
  return { e, log };
}
function optOf(e) { return JSON.parse(JSON.stringify(e.opt, (k, v) => (typeof v === 'function' ? undefined : v))); }

const out = {};

// ---- remapSectionVariable ----
const remap = [];
const types = ['bill', 'gazette', 'legislation', 'regulation', 'treaty', 'book', 'legal_case', undefined];
const sections = [undefined, '', '5', 'sec. 5', ' 5 ', 'art. 7', 'p. 3', 'p.3', '§ 4', '. 5', ', 6', 'vol. 2', 'xii', 'ch. 3 sec. 4', 'pp. 3-5', 'foo'];
const locators = [undefined, '', '12', ' 12 ', 'p. 12', 'pp. 12-15', 'ch. 3', 'sec. 5', '[12]', '(12)', '.5', ',5', ';5', ':5', '?5', 'art. 3', 'foo bar', 'p.', 'p. ', ' p. 12', 'at 12', 12, '12\n3'];
const labels = [undefined, '', 'page', 'chapter', 'section', 'sub verbo', 'foo', 'article'];
for (const type of types) for (const section of sections) for (const locator of locators) for (const label of labels) {
  if (type && ['bill', 'treaty', 'book'].indexOf(type) === -1 && (label || (typeof locator === 'string' && locator.length > 4))) continue;
  const hh = JSON.stringify([type, section, locator, label]).split('').reduce((a, ch) => (a * 31 + ch.charCodeAt(0)) >>> 0, 11);
  if (hh % 6 !== 0 && !(type === 'bill' && label === undefined)) continue;
  const Item = {}; if (type) Item.type = type; if (section !== undefined) Item.section = section;
  const item = {}; if (locator !== undefined) item.locator = locator; if (label !== undefined) item.label = label;
  const I = JSON.parse(JSON.stringify(Item)), i = JSON.parse(JSON.stringify(item));
  const c = { Item, item };
  try { CSL.Engine.prototype.remapSectionVariable.call({}, [[I, i]]); c.Item_out = plain(I); c.item_out = plain(i); } catch (err) { c.error = String(err.message); }
  remap.push(c);
}
out.remap = remap;

// ---- setNumberLabels ----
const snl = [];
for (const ext of [true, false]) for (const type of ['bill', 'legislation', 'book', undefined]) for (const num of [undefined, '', '5', 'art. 5', 'sec. 5-7', 'p. 3', 'vol. 2 sec. 4', 'foo bar', 'art. 5 sec. 2', 'a\\b', ' sec. 5', 'sec.5', 'ch. 3, sec. 4', 5, 'sv. foo', 'sec. ']) for (const pre of [false, true]) {
  const Item = {}; if (type) Item.type = type; if (num !== undefined) Item.number = num;
  const fake = { opt: { development_extensions: { consolidate_legal_items: ext } }, tmp: { shadow_numbers: pre ? { number: { values: [] } } : {} } };
  const c = { Item, ext, pre };
  try { CSL.Engine.prototype.setNumberLabels.call(fake, JSON.parse(JSON.stringify(Item))); c.out = plain(fake.tmp.shadow_numbers); } catch (err) { c.error = String(err.message); }
  snl.push(c);
}
out.set_number_labels = snl;

// ---- parseLocator ----
const pl = [];
for (const ext of [true, false]) for (const loc of [undefined, '', '12', '12 ', ' 12', '12|2012-05-03', '12|2012-05-03 extra', '12|extra', '12|', '|', '|2012-05-03', '12|2012-05-03T', '12|2012-5-3', '12|1999-12-31 rev. 2', 12, 0, '12\t', '12\n', 'a|b|c', 'a|2012-05-03|c']) {
  const item = { id: 'x' }; if (loc !== undefined) item.locator = loc;
  const fake = { opt: { development_extensions: { locator_date_and_revision: ext } }, fun: { dateparser: CSL.DateParser } };
  const c = { item, ext };
  try { c.out = plain(CSL.parseLocator.call(fake, JSON.parse(JSON.stringify(item)))); } catch (err) { c.error = String(err.message); }
  pl.push(c);
}
out.parse_locator = pl;

// ---- citation item input (api_cite.js loop) ----
const cii = [];
const ciiEngines = {};
for (const lang of ['en-US', 'fr-FR']) {
  for (const ext of [{}, { consolidate_legal_items: true }, { locator_label_parse: true }, { consolidate_legal_items: true, locator_label_parse: true, locator_date_and_revision: true }, { locator_date_and_revision: true }]) {
    const name = lang + ':' + JSON.stringify(ext);
    const { e, log } = mkEngine(lang, ext);
    ciiEngines[name] = { opt: optOf(e), log };
    const items = [{ type: 'book' }, { type: 'bill', section: '5' }, { type: 'bill', section: 'art. 3' }, { type: 'legislation' }, { type: 'article-journal' }];
    const locs = [undefined, '12', 'ch. 3', 'chap. 3', 'sec. 4', 'p. 5', 'vol. 2', 'foo. 3', 'art. 3', 'no. 5', 'n. 3', 'sv. foo', 'l. 4', '12|2012-05-03 x', 'bk. 2', 'pp. 3-4', ' ch. 5 '];
    const labs = [undefined, 'page', 'chapter', 'section'];
    for (const Item of items) for (const locator of locs) for (const label of labs) {
      const hh = JSON.stringify([name, Item, locator, label]).split('').reduce((a, ch) => (a * 31 + ch.charCodeAt(0)) >>> 0, 13);
      if (hh % 3 !== 0) continue;
      const ci = { id: 'ITEM-1' }; if (locator !== undefined) ci.locator = locator; if (label !== undefined) ci.label = label;
      const I = JSON.parse(JSON.stringify(Item));
      const item = {}; for (const key in ci) item[key] = ci[key];
      const c = { engine: name, Item, ci };
      try {
        let it = CSL.parseLocator.call(e, item);
        if (e.opt.development_extensions.consolidate_legal_items) e.remapSectionVariable([[I, it]]);
        if (e.opt.development_extensions.locator_label_parse) {
          if (it.locator && ['bill', 'gazette', 'legislation', 'regulation', 'treaty'].indexOf(I.type) === -1 && (!it.label || it.label === 'page')) {
            const m = CSL.LOCATOR_LABELS_REGEXP.exec(it.locator);
            if (m) { const tryLabel = CSL.LOCATOR_LABELS_MAP[m[2]]; if (e.getTerm(tryLabel)) { it.label = tryLabel; it.locator = m[3]; } }
          }
        }
        c.item_out = plain(it); c.Item_out = plain(I);
      } catch (err) { c.error = String(err.message); }
      cii.push(c);
    }
  }
}
out.citation_item_input = cii;
out.cii_engines = ciiEngines;

// ---- evaluateLabel / castLabel ----
const labelCases = [];
const labelEngines = {};
for (const lang of ['en-US', 'fr-FR', 'de-DE']) {
  const { e, log } = mkEngine(lang, { csl_reverse_lookup_support: false });
  const { e: e2, log: log2 } = mkEngine(lang, { csl_reverse_lookup_support: true });
  labelEngines[lang] = { opt: optOf(e), log };
  labelEngines[lang + '+rev'] = { opt: optOf(e2), log: log2 };
  for (const [ename, eng] of [[lang, e], [lang + '+rev', e2]]) {
    for (const term of ['locator', 'page', 'volume', 'issue', 'number', 'chapter-number', 'edition', 'section', 'collection-number', 'number-of-pages', 'number-of-volumes', 'book', 'foo']) {
      for (const form of [undefined, 'short', 'long', 'symbol', 'static', 'verb']) {
        for (const plural of [undefined, 0, 1, 'always']) {
          for (const cap of [undefined, true]) {
            for (const tipForm of [undefined, 'short']) {
              for (const stripPeriods of [false, true]) {
                for (const itemSpec of [{ v: '12' }, { v: '12-15' }, { v: 'xii' }, { v: undefined }, { v: '3 & 4', lab: 'chapter' }, { v: 'ch. 3', lab: 'page' }, { v: '5-7', lab: 'sub verbo' }]) {
                  // reduce: not every combination
                  const h = (term + form + plural + cap + tipForm + stripPeriods + JSON.stringify(itemSpec)).split('').reduce((a, ch) => (a * 31 + ch.charCodeAt(0)) >>> 0, 7);
                  if (h % 53 !== 0) continue;
                  const node = new CSL.Token('label', CSL.SINGLETON);
                  node.strings.term = term;
                  if (form !== undefined) node.strings.form = form;
                  if (plural !== undefined) node.strings.plural = plural;
                  if (cap) node.strings.capitalize_if_first = true;
                  node.cslid = 7;
                  node.decorations = (h % 3 === 0) ? [['@strip-periods', 'true']] : [];
                  if (h % 5 === 0) node.default_locale = true;
                  const Item = { id: 'i', type: 'book' };
                  if (itemSpec.v !== undefined) Item[term === 'locator' ? 'page' : term] = itemSpec.v;
                  const cite = (h % 2 === 0) ? { id: 'i' } : undefined;
                  if (cite) { cite.locator = itemSpec.v; if (itemSpec.lab) cite.label = itemSpec.lab; }
                  const tip = { label_form: tipForm, label_static: false };
                  eng.tmp.group_context = { tip };
                  eng.tmp.shadow_numbers = {};
                  eng.tmp.strip_periods = stripPeriods;
                  eng.tmp.cite_renders_content = false;
                  const c = { engine: ename, node: { strings: plain(node.strings), decorations: plain(node.decorations), cslid: 7, default_locale: !!node.default_locale }, Item, cite: cite || null, tip_form: tipForm || null, strip: stripPeriods };
                  try {
                    c.out = CSL.evaluateLabel(node, eng, Item, cite);
                    c.tip_static = tip.label_static;
                    c.node_decorations = plain(node.decorations);
                    c.shadow = plain(eng.tmp.shadow_numbers);
                    c.shadow_extra = {};
                    for (const k of Object.keys(eng.tmp.shadow_numbers)) {
                      const s = eng.tmp.shadow_numbers[k];
                      c.shadow_extra[k] = { labelForm: s.labelForm === undefined ? null : s.labelForm, labelDecorations: plain(s.labelDecorations), labelCapitalizeIfFirst: s.labelCapitalizeIfFirst === undefined ? null : s.labelCapitalizeIfFirst };
                    }
                  } catch (err) { c.error = String(err.message); }
                  labelCases.push(c);
                }
              }
            }
          }
        }
      }
    }
  }
}
out.label_cases = labelCases;
out.label_engines = labelEngines;
write('locator', out);
