// End-to-end differential reference for the names subsystem (GitHub #794):
// generated names-only styles x a pool of items, rendered by citeproc-js 2.4.63
// in citation mode (one cluster per item, and one cluster with every item) and
// in bibliography mode. The Rust replay is the `names_e2e_matches_citeproc_js`
// test in src/citeproc/util_names_output_tests.rs.
// Output: tests/data/csl/units/names_e2e.json
// Run: CITEPROC_MODULE=<repo>/target/csl-reference/node_modules/citeproc/citeproc_commonjs.js node scripts/csl-units/names_e2e.cjs
'use strict';
const fs = require('fs');
const path = require('path');
const { freshCSL, root, write } = require('./common.cjs');
const { prng } = require('./common_output.cjs');
const CSL = freshCSL();
const rnd = prng(794);
const pick = (a) => a[Math.floor(rnd() * a.length)];
const chance = (p) => rnd() < p;

// ------------------------------------------------------------------ items
const P = (family, given, extra) => Object.assign({ family, given }, extra || {});
const POOL = [
  { id: 'I01', type: 'book', title: 'One', author: [P('Doe', 'John')] },
  { id: 'I02', type: 'book', title: 'Two', author: [P('Doe', 'John'), P('Roe', 'Jane')] },
  { id: 'I03', type: 'book', title: 'Three', author: [P('Doe', 'John Q.'), P('Roe', 'Jane-Marie'), P('Smith', 'A. B.')] },
  { id: 'I04', type: 'book', title: 'Many', author: [P('Doe', 'John'), P('Roe', 'Jane'), P('Smith', 'Adam'), P('Jones', 'Betty'), P('Brown', 'Carl'), P('Green', 'Dora'), P('White', 'Eric')] },
  { id: 'I05', type: 'book', title: 'Particles', author: [P('Gogh', 'Vincent', { 'non-dropping-particle': 'van' }), P('Beethoven', 'Ludwig', { 'dropping-particle': 'van' }), P('Fontaine', 'Jean', { 'non-dropping-particle': "de la" })] },
  { id: 'I06', type: 'book', title: 'Suffix', author: [P('King', 'Martin Luther', { suffix: 'Jr.', 'comma-suffix': true }), P('Ford', 'Henry', { suffix: 'III' })] },
  { id: 'I07', type: 'report', title: 'Inst', author: [{ literal: 'Acme Corporation | Research Division' }] },
  { id: 'I08', type: 'report', title: 'Inst2', author: [{ literal: 'Department of Education' }, P('Doe', 'John'), { literal: 'Bureau | Science' }] },
  { id: 'I09', type: 'book', title: 'Editors', editor: [P('Roe', 'Jane'), P('Poe', 'Edgar')], translator: [P('Roe', 'Jane'), P('Poe', 'Edgar')] },
  { id: 'I10', type: 'book', title: 'Editors2', editor: [P('Roe', 'Jane')], translator: [P('Poe', 'Edgar')] },
  { id: 'I11', type: 'book', title: 'JA', language: 'ja', author: [P('山田', '太郎'), P('鈴木', '花子')] },
  { id: 'I12', type: 'book', title: 'Partial', author: [{ family: 'Smith' }, { given: 'Madonna' }, P('', '')] },
  { id: 'I13', type: 'book', title: 'EtAlHack', author: [P('Doe', 'John'), P('Roe', 'Jane', { 'dropping-particle': 'et al.' })] },
  { id: 'I14', type: 'book', title: 'Multi', author: [{ family: '山田', given: '太郎', multi: { _key: { 'ja-Latn': { family: 'Yamada', given: 'Taro' }, en: { family: 'Yamada', given: 'T.' } } } }, P('Doe', 'John')] },
  { id: 'I15', type: 'book', title: 'None' },
  { id: 'I16', type: 'book', title: 'Hyph', author: [P('Chen', 'H.-L.'), P('Chen', 'Hsiao-Lan'), P('Doe', 'jean-pierre'), P('Li', 'X.Y.')] },
  { id: 'I17', type: 'book', title: 'Lord', author: [P('Byron', 'Lord'), P('Byron', 'Lady')] },
  { id: 'I18', type: 'book', title: 'Static', author: [P('Doe', 'John', { 'static-ordering': true }), P('Roe', 'Jane', { 'reverse-ordering': true }), P('Smith', 'Sam', { 'full-form-always': true })] },
  { id: 'I19', type: 'interview', title: 'Interview', author: [P('Doe', 'John')] },
  { id: 'I21', type: 'book', title: 'Dup', author: [P('Doe', 'John')] },
  { id: 'I22', type: 'classic', title: 'Poetics', author: [P('Aristotle', '')] },
  { id: 'I23', type: 'book', title: 'Dup2', author: [P('Doe', 'John'), P('Roe', 'Jane')] },
  { id: 'I24', type: 'book', title: 'StrAuthor', author: 'String Author' },
  { id: 'I25', type: 'book', title: 'ObjAuthor', author: P('Single', 'Object') },
  { id: 'I26', type: 'book', title: 'Quoted', author: [P('"Doe Jones"', 'John'), P('Smith', 'Ann', { 'parse-names': false }), P('von Neumann', 'John'), P('Foo', 'Bar', { 'static-particles': true })] },
  { id: 'I27', type: 'report', title: 'InstFlag', author: [{ family: 'Acme', isInstitution: true }, { family: 'Foo', given: '', isInstitution: true }] },
  { id: 'I28', type: 'book', title: 'EmptyAuth', author: [], editor: [P('Roe', 'Jane')] },
  { id: 'I29', type: 'book', title: 'NumAuth', author: 5 },
  { id: 'I30', type: 'report', title: 'Corp3', author: [{ literal: 'A | B | C | Department of Education' }, { literal: 'Acme Corporation' }] },
  { id: 'I31', type: 'book', title: 'Sixty', author: Array.from({ length: 60 }, (_, i) => P('Author' + (i + 1), 'N' + (i + 1))) },
  { id: 'I20', type: 'book', title: 'LitMix', author: [{ literal: 'The Foo | Bar Group' }, { literal: 'Solo' }] },
];

// -------------------------------------------------------------- abbreviations
const ACACHE = {
  default: {
    'institution-entire': { 'acme corporation': 'ACME', 'department of education': 'DoE' },
    'institution-part': { 'research division': 'RD', bureau: 'B', science: 'Sci', 'the foo': 'TF' },
    nickname: { 'john doe': 'Johnny' },
    classic: { 'aristotle poetics': 'Poet.' },
  },
};
function normalizeAbbrevsKey(variable, key) {
  key = key ? ('' + key).trim() : '';
  if (['jurisdiction', 'country'].indexOf(variable) > -1) return key.toUpperCase();
  key = key.toString().replace(/(?:\b|^)(?:and|et|y|und|l[ae]|the|[ld]')(?:\b|$)|[\x21-\x2C.\/\x3A-\x40\x5B-\x60\\\x7B\x7D-\x7E]/gi, '')
    .replace(/\s*\x7C\s*/g, '\x7C').replace(/\./g, ' ').replace(/\s+/g, ' ').trim();
  return key.toLowerCase();
}
function makeSys(items) {
  const cache = {};
  for (const i of items) cache[i.id] = JSON.parse(JSON.stringify(i));
  const acache = {};
  for (const j of Object.keys(ACACHE)) {
    acache[j] = new CSL.AbbreviationSegments();
    for (const seg of Object.keys(ACACHE[j])) {
      acache[j][seg] = {};
      for (const k of Object.keys(ACACHE[j][seg])) acache[j][seg][k] = ACACHE[j][seg][k];
    }
  }
  return {
    _acache: acache,
    retrieveItem: (id) => cache[id],
    retrieveLocale: (lang) => { try { return fs.readFileSync(path.join(root, 'vendor/citeproc-js/locale', 'locales-' + lang + '.xml')).toString().replace(/\s*<\?[^>]*\?>\s*\n/g, ''); } catch (e) { return false; } },
    retrieveStyleModule: () => null,
    normalizeAbbrevsKey,
    getAbbreviation(dummy, obj, jurisdiction, category, key) {
      if (!this._acache[jurisdiction]) this._acache[jurisdiction] = new CSL.AbbreviationSegments();
      const js = ['default'];
      if (jurisdiction !== 'default') { const lst = jurisdiction.split(':'); for (let i = 1; i < lst.length + 1; i++) js.push(lst.slice(0, i).join(':')); }
      js.reverse();
      let my;
      for (let i = 0; i < js.length; i++) {
        my = js[i];
        if (!obj[my]) obj[my] = new CSL.AbbreviationSegments();
        if (this._acache[my] && this._acache[my][category] && this._acache[my][category][key]) { obj[my][category][key] = this._acache[my][category][key]; break; }
      }
      return my;
    },
  };
}

// ----------------------------------------------------------------- styles
const q = (v) => '"' + String(v).replace(/&/g, '&amp;').replace(/"/g, '&quot;').replace(/</g, '&lt;') + '"';
function attrs(o) { return Object.keys(o).filter((k) => o[k] !== undefined).map((k) => ` ${k}=${q(o[k])}`).join(''); }
function optAttr(name, values, p) { return chance(p) ? { [name]: pick(values) } : {}; }
function genStyle() {
  const common = Object.assign({},
    optAttr('and', ['text', 'symbol'], 0.5),
    optAttr('delimiter-precedes-last', ['contextual', 'always', 'never', 'after-inverted-name'], 0.5),
    optAttr('delimiter-precedes-et-al', ['contextual', 'always', 'never', 'after-inverted-name'], 0.3),
    optAttr('et-al-min', ['1', '2', '3', '4', '8'], 0.5),
    optAttr('et-al-use-first', ['1', '1', '2', '3'], 0.5),
    optAttr('et-al-use-last', ['true', 'false'], 0.2),
    optAttr('initialize-with', ['.', '. ', '', ' ', ' '], 0.4),
    optAttr('name-as-sort-order', ['first', 'all'], 0.4),
    optAttr('sort-separator', [', ', ' ', '; '], 0.2),
    optAttr('name-delimiter', ['; ', ' & ', ' + '], 0.2),
    optAttr('names-delimiter', [' / ', '; '], 0.15));
  const nameOnly = Object.assign({},
    optAttr('form', ['long', 'short', 'count'], 0.35),
    optAttr('initialize', ['true', 'false'], 0.15),
    optAttr('delimiter', [', ', ' + ', ';', ' ', ''], 0.25),
    optAttr('suppress-min', ['2', '3'], 0.05));
  if (chance(0.2)) common['demote-non-dropping-particle'] = undefined;
  const styleAttrs = Object.assign({}, optAttr('initialize-with-hyphen', ['false'], 0.15), optAttr('demote-non-dropping-particle', ['never', 'sort-only', 'display-and-sort'], 0.3));
  const nameParts = [];
  if (chance(0.25)) nameParts.push(`<name-part name="family"${attrs(Object.assign({}, optAttr('text-case', ['uppercase', 'lowercase', 'capitalize-first'], 0.5), optAttr('font-variant', ['small-caps'], 0.2), optAttr('prefix', ['{'], 0.1), optAttr('suffix', ['}'], 0.1)))}/>`);
  if (chance(0.25)) nameParts.push(`<name-part name="given"${attrs(Object.assign({}, optAttr('text-case', ['uppercase', 'lowercase'], 0.3), optAttr('font-style', ['italic'], 0.3), optAttr('prefix', ['['], 0.1), optAttr('suffix', [']'], 0.1)))}/>`);
  const etal = chance(0.25) ? `<et-al${attrs({ term: pick(['et-al', 'and others']), 'font-style': chance(0.5) ? 'italic' : undefined })}/>` : '';
  const labelForm = pick(['long', 'short', 'verb', 'verb-short', 'symbol']);
  const labelPos = pick(['none', 'none', 'after', 'before']);
  const labelAttrs = attrs({ form: chance(0.7) ? labelForm : undefined, prefix: chance(0.3) ? ' (' : undefined, suffix: chance(0.3) ? ')' : undefined, plural: chance(0.1) ? pick(['always', 'never', 'contextual']) : undefined });
  const inst = chance(0.5) ? `<institution${attrs(Object.assign({}, optAttr('institution-parts', ['long', 'short', 'short-long', 'long-short'], 0.7), optAttr('use-first', ['1', '2'], 0.3), optAttr('use-last', ['1'], 0.15), optAttr('stop-last', ['1'], 0.1), optAttr('reverse-order', ['true'], 0.1), optAttr('delimiter', [', ', ' - '], 0.2), optAttr('and', ['text', 'symbol', 'none'], 0.3)))}>${chance(0.6) ? `<institution-part name="long"${attrs({ 'if-short': chance(0.3) ? 'true' : undefined, 'font-style': chance(0.3) ? 'italic' : undefined })}/>` : ''}${chance(0.6) ? `<institution-part name="short"${attrs({ 'font-weight': chance(0.3) ? 'bold' : undefined })}/>` : ''}</institution>` : '';
  const substVar = pick(['editor', 'editor translator', 'translator']);
  const subst = chance(0.5) ? (chance(0.6) ? `<substitute><names variable="${substVar}"/></substitute>` : `<substitute><names variable="${substVar}"><name${attrs(optAttr('and', ['text', 'symbol'], 0.5))}/>${chance(0.5) ? '<label form="short" prefix=", "/>' : ''}</names></substitute>`) : '';
  const variable = pick(['author', 'author', 'author', 'author editor', 'editor translator']);
  const namesAttrs = attrs(Object.assign({ variable }, optAttr('delimiter', [' / ', '; '], 0.15), optAttr('prefix', ['(', '['], 0.15), optAttr('suffix', [')', '.'], 0.15), optAttr('font-style', ['italic'], 0.1)));
  const nameEl = chance(0.9) ? `<name${attrs(Object.assign({}, chance(0.7) ? nameOnly : {}, chance(0.3) ? common : {}))}>${nameParts.join('')}</name>` : '';
  const lbl = labelPos === 'none' ? '' : `<label${labelAttrs}/>`;
  const body = labelPos === 'before' ? `${lbl}${nameEl}${etal}${inst}${subst}` : `${nameEl}${etal}${lbl}${inst}${subst}`;
  const layoutAttrs = attrs(Object.assign({}, optAttr('delimiter', ['; ', ' | '], 0.4), optAttr('prefix', ['(', '['], 0.2), optAttr('suffix', [')', '.'], 0.3)));
  const bibLayoutAttrs = attrs(Object.assign({}, optAttr('suffix', ['.'], 0.4), optAttr('prefix', ['> '], 0.1)));
  const bibAttrs = attrs(Object.assign({}, chance(0.3) ? common : {}, optAttr('hanging-indent', ['true'], 0.1), chance(0.35) ? { 'subsequent-author-substitute': pick(['---', '——', '']), 'subsequent-author-substitute-rule': pick(['complete-all', 'complete-each', 'partial-each', 'partial-first']) } : {}));
  const citAttrs = attrs(Object.assign({}, chance(0.7) ? common : {}, chance(0.3) ? { collapse: 'year' } : {}, chance(0.1) ? { 'cite-group-delimiter': ', ' } : {}));
  const style = `<style xmlns="http://purl.org/net/xbiblio/csl" class="in-text" version="1.0"${attrs(styleAttrs)}><info><id>http://example.org/s</id><title>S</title><updated>2020-01-01T00:00:00+00:00</updated></info><citation${citAttrs}><layout${layoutAttrs}><names${namesAttrs}>${body}</names></layout></citation><bibliography${bibAttrs}><layout${bibLayoutAttrs}><names${namesAttrs}>${body}</names></layout></bibliography></style>`;
  const options = {};
  if (chance(0.1)) options.spoof_institutional_affiliations = true;
  if (chance(0.08)) options.parse_names = false;
  if (chance(0.12)) options.etal_min_etal_usefirst_hack = true;
  const lang = chance(0.2) ? { translit: ['ja-Latn'], translat: ['en'], persons: pick([['translit'], ['translit', 'translat'], ['translat'], ['orig', 'translit']]), institutions: pick([['translit'], ['orig']]) } : null;
  return { style, options, lang };
}

// --------------------------------------------------------------- runner
function runStyle(spec) {
  const sys = makeSys(POOL);
  for (const o of Object.keys(spec.options)) sys[o] = spec.options[o];
  const out = { cites: [], sa: [], multi: null, bib: null };
  let style;
  try {
    style = new CSL.Engine(sys, spec.style, 'en-US');
  } catch (e) { return { build_error: String(e && e.message || e) }; }
  for (const o of Object.keys(spec.options)) style.opt.development_extensions[o] = spec.options[o];
  const lp = { persons: ['translit'], institutions: ['translit'], titles: ['translit', 'translat'], journals: ['translit'], publishers: ['translat'], places: ['translat'] };
  if (spec.lang) {
    style.setLangTagsForCslTranslation(spec.lang.translat);
    style.setLangTagsForCslTransliteration(spec.lang.translat);
    lp.persons = spec.lang.persons; lp.institutions = spec.lang.institutions;
  }
  style.setLangPrefsForCites(lp);
  const ids = POOL.map((i) => i.id);
  // Cites are rendered BEFORE updateItems: with the items in the registry,
  // citeStart takes the cite's name counts from the registry's disambiguation
  // data (the registry is the engine's, not the names code's).
  for (const id of ids) {
    try { out.cites.push({ v: style.makeCitationCluster([{ id }]) }); } catch (e) { out.cites.push({ e: String(e && e.message || e) }); }
  }
  for (const id of ids) {
    try { out.sa.push({ v: style.makeCitationCluster([{ id, 'suppress-author': true }]) }); } catch (e) { out.sa.push({ e: String(e && e.message || e) }); }
  }
  try { out.multi = { v: style.makeCitationCluster(ids.map((id) => ({ id }))) }; } catch (e) { out.multi = { e: String(e && e.message || e) }; }
  try {
    style.updateItems(ids);
    const b = style.makeBibliography();
    out.bib = { v: b[0].bibstart + b[1].join('') + b[0].bibend };
  } catch (e) { out.bib = { e: String(e && e.message || e) }; }
  return out;
}

const N = 700;
const specs = [];
for (let i = 0; i < N; i++) specs.push(genStyle());
const cases = specs.map((s) => Object.assign({ style: s.style, options: s.options, lang: s.lang }, { out: runStyle(s) }));
const stats = { cases: cases.length, errors: 0, outputs: 0 };
for (const c of cases) {
  if (c.out.build_error) { stats.errors++; continue; }
  for (const x of c.out.cites.concat(c.out.sa, [c.out.multi, c.out.bib])) { if (x.e) stats.errors++; else stats.outputs++; }
}
console.error(JSON.stringify(stats));
write('names_e2e', { pool: POOL, acache: ACACHE, cases });
