// Differential reference for the rendering nodes of the citeproc-js port
// (GitHub #793): cs:text, cs:group, cs:label, cs:number, cs:choose with every
// condition attribute, util_transform.js (language selection, abbreviations),
// util_substitute.js and the output queue behind them.
//
// Generates a few hundred random but DETERMINISTIC (seeded) styles without
// names or dates, runs each over a few random items through citeproc-js 2.4.63
// the way the CSL test runner does (new Engine, setLangPrefsForCites,
// updateItems, makeCitationCluster, makeBibliography) and records citeproc-js's
// output. The Rust test `render_cases_match_citeproc_js`
// (src/citeproc/util_transform.rs) replays every case through the port.
//
//   export CITEPROC_MODULE=<repo>/target/csl-reference/node_modules/citeproc/citeproc_commonjs.js
//   node scripts/csl-units/render.cjs [count]
//
// Output: crates/kovan-literature/tests/data/csl/units/render.json
'use strict';
const fs = require('fs');
const path = require('path');
const { root, freshCSL, write } = require('./common.cjs');
const CSL = freshCSL();

const localeDir = path.join(root, 'vendor/citeproc-js/locale');

// ---- seeded random ----
let seed = 0x5eed2026;
function rnd() {
  seed |= 0; seed = (seed + 0x6d2b79f5) | 0;
  let t = Math.imul(seed ^ (seed >>> 15), 1 | seed);
  t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
  return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
}
const pick = (a) => a[Math.floor(rnd() * a.length)];
const chance = (p) => rnd() < p;
const range = (n) => Array.from({ length: n }, (_, i) => i);

// ---- the runner's sys (scripts/csl-testsuite-reference.cjs) ----
function variableWrapper(params, prePunct, str, postPunct) {
  if (params.variableNames[0] === 'title' && params.itemData.URL && params.context === 'citation' && params.position === 'first') {
    return prePunct + '<a href="' + params.itemData.URL + '">' + str + '</a>' + postPunct;
  } else if (params.variableNames[0] === 'first-reference-note-number' && params.context === 'citation' && params.position !== 'first') {
    return prePunct + '<b>' + str + '</b>' + postPunct;
  }
  return prePunct + str + postPunct;
}

function Sys(items, acache, wrap) {
  if (wrap) this.variableWrapper = variableWrapper;
  this._acache = Object.assign({ default: new CSL.AbbreviationSegments() }, acache || {});
  this._cache = {};
  for (const it of items) this._cache[it.id] = it;
}
Sys.prototype.retrieveItem = function (id) { return this._cache[id]; };
Sys.prototype.retrieveLocale = function (lang) {
  try {
    return fs.readFileSync(path.join(localeDir, 'locales-' + lang + '.xml')).toString().replace(/\s*<\?[^>]*\?>\s*\n/g, '');
  } catch (e) { return false; }
};
Sys.prototype.retrieveStyleModule = function () { return null; };
Sys.prototype.getAbbreviation = function (dummyListNameVar, obj, jurisdiction, category, key) {
  if (!this._acache[jurisdiction]) this._acache[jurisdiction] = new CSL.AbbreviationSegments();
  const jurisdictions = ['default'];
  if (jurisdiction !== 'default') {
    const lst = jurisdiction.split(':');
    for (let i = 1; i < lst.length + 1; i++) jurisdictions.push(lst.slice(0, i).join(':'));
  }
  jurisdictions.reverse();
  let myjurisdiction;
  for (let i = 0; i < jurisdictions.length; i++) {
    myjurisdiction = jurisdictions[i];
    if (!obj[myjurisdiction]) obj[myjurisdiction] = new CSL.AbbreviationSegments();
    if (this._acache[myjurisdiction] && this._acache[myjurisdiction][category] && this._acache[myjurisdiction][category][key]) {
      obj[myjurisdiction][category][key] = this._acache[myjurisdiction][category][key];
      break;
    }
  }
  return myjurisdiction;
};
Sys.prototype.normalizeAbbrevsKey = function (variable, key) {
  key = key ? ('' + key).trim() : '';
  if (['jurisdiction', 'country'].indexOf(variable) > -1) return key.toUpperCase();
  key = key.toString()
    .replace(/(?:\b|^)(?:and|et|y|und|l[ae]|the|[ld]')(?:\b|$)|[\x21-\x2C.\/\x3A-\x40\x5B-\x60\\\x7B\x7D-\x7E]/gi, '')
    .replace(/\s*\x7C\s*/g, '\x7C').replace(/\./g, ' ').replace(/\s+/g, ' ').trim();
  return key.toLowerCase();
};

// ---- the item pool ----
const TITLES = [
  'The Structure of Scientific Revolutions', 'a tale of two cities', 'WAR AND PEACE', 'Of Mice & Men', 'Dr. Strangelove: Or, How I Learned to Stop Worrying',
  'The "Quoted" Title', 'Notes on <i>Italic</i> Words', "Rock 'n' Roll: A History", 'La Belle Époque', 'Über den Wolken', 'on the origin of species by means of natural selection',
  'A Very Long Title That Goes On: And Has a Subtitle', 'iPhone and eBay', 'Title ending with period.', 'Title with question?', '10 Things', 'the', '',
];
const CONTAINERS = ['Journal of Applied Physics', 'The Journal of Irreproducible Results', 'Proc. Natl. Acad. Sci. U.S.A.', 'Annals of Mathematics', 'nature', 'JOURNAL OF X'];
const PUBLISHERS = ['Oxford University Press', 'Springer', 'the MIT Press', 'Elsevier B.V.', 'Penguin'];
const PLACES = ['New York', 'Oxford', 'Berlin', 'London, UK', 'Paris'];
const PAGES = ['1-10', '123', 'A1', 'xi-xv', '100,102', '5–7', '33-4', '1 - 2', '12, 15-17', 'S12-S15', '', 'ii', 'e123'];
const NUMS = ['1', '2', '3', '12', '42', '2nd', 'second', 'vol. 3', '1-3', 'xii', '', '7/8', '3 & 4'];
const TYPES = ['book', 'article-journal', 'chapter', 'webpage', 'legal_case', 'report', 'thesis', 'patent', 'bill', 'paper-conference'];
const LANGS = ['en', 'en-US', 'fr', 'de-DE', 'ja', 'zh-CN', 'xx', ''];

function genItem(n) {
  const it = { id: 'item-' + n, type: pick(TYPES) };
  const put = (k, v, p) => { if (chance(p) && v !== '' && v !== undefined) it[k] = v; };
  put('title', pick(TITLES), 0.9);
  put('title-short', pick(['Revolutions', 'Tale', 'Peace', 'Mice']), 0.3);
  put('container-title', pick(CONTAINERS), 0.6);
  put('container-title-short', pick(['JAP', 'JIR', 'PNAS']), 0.2);
  put('collection-title', pick(['Oxford Series', 'The Great Books']), 0.2);
  put('publisher', pick(PUBLISHERS), 0.5);
  put('publisher-place', pick(PLACES), 0.5);
  put('event', pick(['Annual Meeting', 'the conference']), 0.15);
  put('genre', pick(['PhD thesis', 'technical report', 'Dissertation']), 0.2);
  put('medium', pick(['CD-ROM', 'Print']), 0.1);
  put('note', pick(['A note.', 'see also', 'note: "quoted"']), 0.2);
  put('abstract', pick(['An abstract.', 'Another, longer abstract; with punctuation.']), 0.1);
  put('URL', pick(['http://www.example.com/a/b', 'https://example.org/path?x=1', 'http://example.com']), 0.3);
  put('DOI', pick(['10.1000/xyz123', 'https://doi.org/10.1/abc']), 0.3);
  put('volume', pick(NUMS), 0.5);
  put('issue', pick(NUMS), 0.4);
  put('edition', pick(NUMS), 0.4);
  put('number', pick(NUMS), 0.3);
  put('number-of-pages', pick(['300', '12', '1000']), 0.3);
  put('number-of-volumes', pick(['2', '10']), 0.2);
  put('chapter-number', pick(['3', 'IV']), 0.2);
  put('collection-number', pick(['5', '12']), 0.2);
  put('page', pick(PAGES), 0.5);
  put('section', pick(['12', 'xi', '3.2']), 0.2);
  put('version', pick(['1.0', '2']), 0.1);
  put('source', pick(['Scopus', 'JSTOR']), 0.1);
  put('status', pick(['published', 'forthcoming']), 0.1);
  put('archive', pick(['National Archives']), 0.1);
  put('archive_location', pick(['Box 3']), 0.1);
  put('language', pick(LANGS), 0.35);
  put('citation-label', pick(['Smi01', 'Doe99']), 1);
  if (chance(0.5)) it.issued = chance(0.7) ? { 'date-parts': [[1990 + Math.floor(rnd() * 30), 1 + Math.floor(rnd() * 12)]] } : { literal: 'forthcoming' };
  if (chance(0.15)) it.accessed = { 'date-parts': [[2020, 1, 2]] };
  if (chance(0.2)) it.jurisdiction = pick(['us', 'us:c:ma', 'uk']);
  if (chance(0.15)) {
    // a multilingual field
    it.multi = { _keys: { title: { 'ja-Latn': 'Nihongo no Daimei', ja: '日本語の題名', fr: 'Titre français' }, 'container-title': { 'ja-Latn': 'Zasshi', en: 'Journal' } }, main: { title: 'ja' } };
    if (!it.title) it.title = '日本語の題名';
    if (!it['container-title']) it['container-title'] = 'ジャーナル';
  }
  return it;
}

// ---- the style generator ----
const TEXT_VARS = ['title', 'title-short', 'container-title', 'collection-title', 'publisher', 'publisher-place', 'event', 'genre', 'medium', 'note', 'abstract',
  'URL', 'DOI', 'section', 'version', 'source', 'status', 'archive', 'archive_location', 'language', 'jurisdiction', 'type', 'volume', 'issue', 'edition', 'number',
  'page', 'number-of-pages', 'number-of-volumes', 'chapter-number', 'collection-number', 'locator'];
const TERMS = ['and', 'accessed', 'page', 'volume', 'edition', 'issue', 'no date', 'et-al', 'in', 'ibid', 'online', 'retrieved', 'forthcoming', 'at', 'from', 'editor', 'chapter',
  'section', 'paragraph', 'open-quote', 'close-quote', 'cited', 'anonymous', 'no-date', 'sub verbo', 'long-ordinal-01', 'ordinal-01', 'opus', 'nosuchterm'];
const LABEL_VARS = ['page', 'volume', 'locator', 'issue', 'edition', 'number-of-pages', 'number-of-volumes', 'chapter-number', 'section', 'number', 'collection-number'];
const NUM_VARS = ['volume', 'issue', 'edition', 'number', 'number-of-pages', 'number-of-volumes', 'chapter-number', 'collection-number', 'page', 'locator'];
const CASES = ['lowercase', 'uppercase', 'capitalize-first', 'capitalize-all', 'title', 'sentence'];
const VALUES = ['literal', 'See', 'p. 3', '&amp; more', 'à la', '', '123', 'e.g.', '(x)', '[y]'];
const AFFIXES = ['', '', '', '(', ')', ', ', '. ', ': ', ' ', '[', ']', '“', '; ', '.', ',', '–'];

function esc(s) { return s.replace(/&(?!amp;)/g, '&amp;').replace(/"/g, '&quot;').replace(/</g, '&lt;'); }

function fmtAttrs(allowDisplay) {
  const a = [];
  if (chance(0.25)) a.push(`prefix="${esc(pick(AFFIXES))}"`);
  if (chance(0.25)) a.push(`suffix="${esc(pick(AFFIXES))}"`);
  if (chance(0.12)) a.push(`font-style="${pick(['italic', 'oblique', 'normal'])}"`);
  if (chance(0.1)) a.push(`font-weight="${pick(['bold', 'light', 'normal'])}"`);
  if (chance(0.06)) a.push(`font-variant="small-caps"`);
  if (chance(0.06)) a.push(`text-decoration="underline"`);
  if (chance(0.06)) a.push(`vertical-align="${pick(['sup', 'sub', 'baseline'])}"`);
  if (allowDisplay && chance(0.1)) a.push(`display="${pick(['block', 'left-margin', 'right-inline', 'indent'])}"`);
  return a.join(' ');
}

function genCond() {
  const kinds = [
    () => `type="${range(1 + Math.floor(rnd() * 3)).map(() => pick(TYPES)).join(' ')}"`,
    () => `variable="${range(1 + Math.floor(rnd() * 2)).map(() => pick(TEXT_VARS.concat(['issued', 'accessed', 'author', 'locator', 'page']))).join(' ')}"`,
    () => `is-numeric="${pick(NUM_VARS.concat(['title', 'version']))}"`,
    () => `is-uncertain-date="${pick(['issued', 'accessed'])}"`,
    () => `locator="${pick(['page', 'chapter', 'section', 'paragraph', 'volume', 'page chapter'])}"`,
    () => `position="${pick(['first', 'subsequent', 'ibid', 'ibid-with-locator', 'first subsequent'])}"`,
    () => `page="${pick(['range', 'arabic', 'roman', 'page', 'minimal range', 'plural'])}"`,
    () => `number="${pick(['range', 'arabic', 'roman', 'number', 'plural'])}"`,
    () => `has-year-only="${pick(['issued', 'accessed'])}"`,
    () => `has-day="${pick(['issued', 'accessed'])}"`,
    () => `has-to-month-or-season="${pick(['issued', 'accessed'])}"`,
    () => `context="${pick(['citation', 'bibliography'])}"`,
    () => `jurisdiction="${pick(['us', 'us:c:ma', 'uk'])}"`,
    () => `is-plural="${pick(['author', 'editor'])}"`,
  ];
  return pick(kinds)();
}

let macroDefs = [];
let macroCeiling = null; // a macro body may only call macros defined before it

function genNode(depth, ctx) {
  const kinds = ['text-var', 'text-var', 'text-var', 'registry-var', 'text-term', 'text-value', 'label', 'number', 'group', 'group', 'choose', 'macro'];
  if (depth >= 3) kinds.splice(kinds.indexOf('group'), 2);
  if (depth >= 3) kinds.splice(kinds.indexOf('choose'), 1);
  const k = pick(kinds);
  const fa = fmtAttrs(ctx.bib && depth === 0);
  switch (k) {
    case 'text-var': {
      const v = pick(TEXT_VARS);
      const a = [`variable="${v}"`];
      if (chance(0.3)) a.push(`form="${pick(['short', 'long'])}"`);
      if (chance(0.15)) a.push(`quotes="true"`);
      if (chance(0.1)) a.push(`strip-periods="true"`);
      if (chance(0.25)) a.push(`text-case="${pick(CASES)}"`);
      if (chance(0.05)) a.push(`plural="${pick(['always', 'never', 'contextual'])}"`);
      return `<text ${a.join(' ')} ${fa}/>`;
    }
    case 'registry-var': {
      const a = [`variable="${pick(['citation-number', 'year-suffix', 'citation-label'])}"`];
      return `<text ${a.join(' ')} ${fa}/>`;
    }
    case 'text-term': {
      const a = [`term="${pick(TERMS)}"`];
      if (chance(0.4)) a.push(`form="${pick(['short', 'long', 'symbol', 'verb', 'verb-short'])}"`);
      if (chance(0.3)) a.push(`plural="${pick(['always', 'never', 'contextual'])}"`);
      if (chance(0.15)) a.push(`quotes="true"`);
      if (chance(0.1)) a.push(`strip-periods="true"`);
      if (chance(0.2)) a.push(`text-case="${pick(CASES)}"`);
      return `<text ${a.join(' ')} ${fa}/>`;
    }
    case 'text-value': {
      const a = [`value="${esc(pick(VALUES))}"`];
      if (chance(0.15)) a.push(`quotes="true"`);
      if (chance(0.2)) a.push(`text-case="${pick(CASES)}"`);
      return `<text ${a.join(' ')} ${fa}/>`;
    }
    case 'label': {
      const a = [`variable="${pick(LABEL_VARS)}"`];
      if (chance(0.5)) a.push(`form="${pick(['short', 'long', 'symbol', 'static'])}"`);
      if (chance(0.3)) a.push(`plural="${pick(['always', 'never', 'contextual'])}"`);
      if (chance(0.1)) a.push(`strip-periods="true"`);
      if (chance(0.2)) a.push(`text-case="${pick(CASES)}"`);
      if (chance(0.1)) a.push(`capitalize-if-first="true"`);
      return `<label ${a.join(' ')} ${fa}/>`;
    }
    case 'number': {
      const a = [`variable="${pick(NUM_VARS)}"`];
      if (chance(0.6)) a.push(`form="${pick(['numeric', 'ordinal', 'long-ordinal', 'roman'])}"`);
      if (chance(0.15)) a.push(`text-case="${pick(CASES)}"`);
      return `<number ${a.join(' ')} ${fa}/>`;
    }
    case 'group': {
      const a = [];
      if (chance(0.6)) a.push(`delimiter="${esc(pick(AFFIXES))}"`);
      if (chance(0.12)) a.push(`${pick(['require', 'reject'])}="${pick(['comma-safe', 'comma-safe-numbers-only', 'empty-label', 'empty-label-no-decor'])}"`);
      if (chance(0.1)) a.push(`label-form="${pick(['short', 'long', 'symbol', 'static'])}"`);
      if (chance(0.08)) a.push(`label-capitalize-if-first="true"`);
      const n = 1 + Math.floor(rnd() * 4);
      const kids = range(n).map(() => genNode(depth + 1, ctx)).join('');
      return `<group ${a.join(' ')} ${fa}>${kids}</group>`;
    }
    case 'choose': {
      const nb = 1 + Math.floor(rnd() * 3);
      let out = '<choose>';
      for (let i = 0; i < nb; i++) {
        const tag = i === 0 ? 'if' : 'else-if';
        const nconds = 1 + Math.floor(rnd() * 2);
        const match = chance(0.4) ? ` match="${pick(['any', 'all', 'none'])}"` : '';
        let head;
        if (chance(0.15)) {
          head = `<${tag}${match}><conditions match="${pick(['any', 'all'])}">${range(2).map(() => `<condition ${genCond()}/>`).join('')}</conditions>`;
        } else {
          head = `<${tag} ${range(nconds).map(() => genCond()).filter((x, j, arr) => arr.findIndex((y) => y.split('=')[0] === x.split('=')[0]) === j).join(' ')}${match}>`;
        }
        const kids = range(1 + Math.floor(rnd() * 2)).map(() => genNode(depth + 1, ctx)).join('');
        out += head + kids + `</${tag}>`;
      }
      if (chance(0.5)) out += `<else>${genNode(depth + 1, ctx)}</else>`;
      return out + '</choose>';
    }
    case 'macro': {
      if (macroDefs.length < 3 && chance(0.5)) {
        const name = 'm' + macroDefs.length;
        const saved = macroCeiling;
        macroCeiling = macroDefs.length;
        macroDefs.push({ name, body: null });
        const def = macroDefs[macroDefs.length - 1];
        def.body = range(1 + Math.floor(rnd() * 3)).map(() => genNode(depth + 1, ctx)).join('');
        macroCeiling = saved;
      }
      const avail = macroDefs.slice(0, macroCeiling === null ? macroDefs.length : macroCeiling);
      if (avail.length === 0) return `<text value="x" ${fa}/>`;
      return `<text macro="${pick(avail).name}" ${fa}/>`;
    }
  }
  return '';
}

function genStyle(ctx) {
  macroDefs = [];
  const cls = pick(['in-text', 'in-text', 'note']);
  const body = (bib) => range(1 + Math.floor(rnd() * 5)).map(() => genNode(0, { bib })).join('');
  const citBody = body(false);
  const bibBody = body(true);
  const macros = macroDefs.map((m) => `<macro name="${m.name}">${m.body}</macro>`).join('');
  const layoutAttrs = (bib) => {
    const a = [];
    if (chance(0.3)) a.push(`prefix="${esc(pick(['(', '[', '']))}"`);
    if (chance(0.3)) a.push(`suffix="${esc(pick([')', ']', '.', '']))}"`);
    if (!bib && chance(0.5)) a.push(`delimiter="${esc(pick(['; ', ', ', ' ']))}"`);
    if (chance(0.1)) a.push(`font-style="italic"`);
    return a.join(' ');
  };
  const bibAttrs = [];
  if (chance(0.2)) bibAttrs.push('hanging-indent="true"');
  if (chance(0.2)) bibAttrs.push('second-field-align="flush"');
  if (chance(0.2)) bibAttrs.push('entry-spacing="0"');
  return `<style xmlns="http://purl.org/net/xbiblio/csl" class="${cls}" version="1.0">
  <info><id>x</id><title>x</title><updated>2009-08-10T04:49:00+09:00</updated></info>
  ${macros}
  <citation><layout ${layoutAttrs(false)}>${citBody}</layout></citation>
  <bibliography ${bibAttrs.join(' ')}><layout ${layoutAttrs(true)}>${bibBody}</layout></bibliography>
</style>`;
}

// ---- abbreviations ----
function genAbbrevs(items) {
  if (!chance(0.5)) return undefined;
  const ab = { default: {} };
  const put = (cat, k, v) => { ab.default[cat] = ab.default[cat] || {}; ab.default[cat][k] = v; };
  for (const it of items) {
    if (it['container-title'] && chance(0.7)) put('container-title', it['container-title'], pick(['J. Abbr.', '!container-title>>>Short J.', '#1>>>Num J', 'J.A.P.']));
    if (it.title && chance(0.5)) put('title', it.title, pick(['T. Abbr', '!title,page>>>TT', 'Ti.']));
    if (it.publisher && chance(0.5)) put('institution-part', it.publisher, 'Pub');
    if (it['publisher-place'] && chance(0.5)) put('place', it['publisher-place'], pick(['NY', 'Ox.']));
    if (it.genre && chance(0.5)) put('title', it.genre, 'Gen.');
    if (it.volume && chance(0.3)) put('number', it.volume, 'V' + it.volume);
    if (it['collection-title'] && chance(0.5)) put('collection-title', it['collection-title'], 'Coll.');
    if (it.event && chance(0.5)) put('title', it.event, 'Ev.');
  }
  return ab;
}

function citeItems(items) {
  return items.map((it) => {
    const c = { id: it.id };
    if (chance(0.5)) c.locator = pick(['12', '12-15', 'ch. 3', 'sec. 4', '33-4', 'xi', '12, 15', 'para. 2', '']);
    if (c.locator && chance(0.5)) c.label = pick(['page', 'chapter', 'section', 'paragraph', 'volume', 'verse', 'sub verbo', 'line', 'column']);
    if (chance(0.2)) c.prefix = pick(['see ', 'See, ', '(', 'cf. ', 'As in: ']);
    if (chance(0.2)) c.suffix = pick([' passim', ', at 3', '.', ' (emphasis added)', ';']);
    if (chance(0.05)) c['suppress-author'] = true;
    return c;
  });
}

function runCase(c) {
  const out = { citation: null, bibliography: null, citation_error: null, bibliography_error: null };
  out.registry = {};
  const sys = new Sys(c.input, c.abbreviations ? normalizeAbbrevs(c.abbreviations) : {}, !!(c.options && c.options.variableWrapper));
  const mk = () => {
    const e = new CSL.Engine(sys, c.csl, c.lang || '');
    if (c.format && c.format !== 'html') e.setOutputFormat(c.format);
    e.setLangPrefsForCites(c.langparams);
    if (c.tags) { e.setLangTagsForCslTranslation(c.tags.translat); e.setLangTagsForCslTransliteration(c.tags.translit); }
    if (c.multiaffix) e.setLangPrefsForCiteAffixes(c.multiaffix);
    for (const k in c.options || {}) if (k !== 'variableWrapper') e.opt.development_extensions[k] = c.options[k];
    e.updateItems(c.input.map((i) => i.id));
    // updateItems renders every item once (disambiguation); the Rust driver has no registry,
    // so drop the one piece of state that rendering leaves behind and the styles can see.
    e.tmp.just_did_number = false;
    for (const id in (c.year_suffix || {})) e.registry.registry[id].disambig.year_suffix = c.year_suffix[id];
    return e;
  };
  try {
    const probe = mk();
    for (const i of c.input) out.registry[i.id] = { seq: probe.registry.registry[i.id].seq, year_suffix: probe.registry.registry[i.id].disambig.year_suffix };
  } catch (err) { /* reported below */ }
  try {
    out.citation = mk().makeCitationCluster(c.citation_items).replace(/\r\n?/g, '\n');
  } catch (err) { out.citation_error = String(err && err.message || err).split('\n')[0]; }
  try {
    const b = mk().makeBibliography();
    out.bibliography = b ? (b[0].bibstart + b[1].join('') + b[0].bibend).replace(/\r\n?/g, '\n') : false;
  } catch (err) { out.bibliography_error = String(err && err.message || err).split('\n')[0]; }
  return out;
}

function normalizeAbbrevs(raw) {
  const abbrevs = {};
  for (const jurisd in raw) {
    abbrevs[jurisd] = {};
    for (const segment in raw[jurisd]) {
      abbrevs[jurisd][segment] = {};
      for (const key in raw[jurisd][segment]) {
        const isJurisdiction = jurisd === 'default' && segment === 'place' && key.toUpperCase() === key;
        const isCourt = ['institution-entire', 'institution-part'].indexOf(segment) > -1 && segment.toLowerCase() === segment;
        const normkey = !isJurisdiction && !isCourt ? Sys.prototype.normalizeAbbrevsKey('title', key) : key;
        abbrevs[jurisd][segment][normkey] = raw[jurisd][segment][key];
      }
    }
  }
  return abbrevs;
}


// ---- focused cases: multilingual fields and abbreviations ----
const MULTI_VARS = ['title', 'container-title', 'collection-title', 'publisher', 'publisher-place', 'event', 'genre', 'medium', 'title-short', 'archive', 'archive-place', 'jurisdiction', 'authority'];
const MULTI_TITLES = ['Nihongo no Daimei', 'The Journal', 'Revue de physique', 'Berlin Verlag', 'Tōkyō'];
function genFocusedItem(n) {
  const it = { id: 'item-' + n, type: pick(['book', 'article-journal', 'chapter', 'legal_case']) };
  const keys = {};
  for (const v of MULTI_VARS) {
    if (!chance(0.55)) continue;
    it[v] = pick(['日本語の題名', 'A title: with subtitle', 'THE UPPER CASE', 'a lower case title', 'Journal of Things', 'Oxford University Press', 'New York', 'the committee', 'Revue de physique']);
    if (chance(0.7)) {
      keys[v] = {};
      for (const tag of ['ja-Latn', 'en', 'fr', 'ja', 'en-GB', 'de'].filter(() => chance(0.5))) keys[v][tag] = pick(MULTI_TITLES) + (chance(0.3) ? ': ' + pick(['Part one', 'part two.']) : '');
    }
  }
  if (Object.keys(keys).length) it.multi = { _keys: keys, main: chance(0.6) ? { title: pick(['ja', 'fr', 'en']) } : {} };
  if (chance(0.5)) it.language = pick(['ja', 'fr', 'en', 'de', 'en-GB', 'xx', 'zh<ja']);
  if (chance(0.3)) it['title-short'] = it['title-short'] || 'Short T.';
  if (chance(0.3)) it['container-title-short'] = 'Short C.';
  if (chance(0.2)) it['language-name'] = 'ja';
  if (chance(0.35)) { it['language-name'] = pick(['ja', 'fr', 'en', 'de']); it['language-name-original'] = 'English'; }
  if (chance(0.3)) it['alt-title'] = 'Alt Title';
  if (chance(0.3)) it['alt-container-title'] = 'Alt Journal';
  if (chance(0.2)) it['alt-publisher'] = 'Alt Press';
  if (chance(0.3)) it.jurisdiction = pick(['us', 'us:c:ma', 'uk']);
  if (chance(0.3)) it.country = pick(['us', 'jp', 'GB']);
  return it;
}
function genFocusedStyle() {
  const nodes = range(2 + Math.floor(rnd() * 4)).map(() => {
    const v = pick(MULTI_VARS.concat(['language-name', 'country']));
    const a = [`variable="${v}"`];
    if (chance(0.5)) a.push(`form="short"`);
    if (chance(0.3)) a.push(`text-case="${pick(CASES)}"`);
    if (chance(0.15)) a.push('quotes="true"');
    if (chance(0.15)) a.push('strip-periods="true"');
    const f = fmtAttrs(false);
    return `<text ${a.join(' ')} ${f}/>`;
  });
  let grouped = chance(0.5) ? `<group delimiter="${esc(pick([', ', ' | ', '. ']))}">${nodes.join('')}</group>` : nodes.join('');
  if (chance(0.3)) grouped = `<alternative>${grouped}</alternative>` + (chance(0.5) ? '<choose><if context="alternative"><text value="ALT"/></if><else><text value="MAIN"/></else></choose>' : '');
  const cls = pick(['in-text', 'note']);
  return `<style xmlns="http://purl.org/net/xbiblio/csl" class="${cls}" version="1.0">
  <info><id>x</id><title>x</title><updated>2009-08-10T04:49:00+09:00</updated></info>
  <citation><layout delimiter="; ">${grouped}</layout></citation>
  <bibliography><layout>${grouped}</layout></bibliography>
</style>`;
}
function genFocusedAbbrevs(items) {
  const ab = { default: {} };
  const put = (cat, k, v) => { ab.default[cat] = ab.default[cat] || {}; ab.default[cat][k] = v; };
  const catOf = { title: 'title', 'title-short': 'title', 'container-title': 'container-title', 'collection-title': 'collection-title', publisher: 'institution-part', authority: 'institution-part', 'publisher-place': 'place', 'archive-place': 'place', event: 'title', genre: 'title', medium: 'title', archive: 'container-title' };
  for (const it of items) {
    const vals = [];
    for (const v of MULTI_VARS) if (it[v]) vals.push([v, it[v]]);
    if (it.multi) for (const v in it.multi._keys) for (const t in it.multi._keys[v]) vals.push([v, it.multi._keys[v][t]]);
    for (const [v, val] of vals) {
      if (!catOf[v] || !chance(0.5)) continue;
      put(catOf[v], val, pick(['A.bbr.', 'Abbr', '!title>>>NoTitle', '#2>>>Cf', 'Abbr: 1', 'X.Y.', '!container-title,publisher>>>Q']));
    }
    if (it.country && chance(0.5)) put('place', it.country, pick(['USA', 'Nippon']));
    if (it.jurisdiction && chance(0.5)) put('place', it.jurisdiction, 'Jur.');
  }
  return ab;
}

const N = parseInt(process.argv[2] || '400', 10);
const cases = [];
for (let n = 0; n < N; n++) {
  const items = range(1 + Math.floor(rnd() * 3)).map((i) => genItem(i + 1));
  const c = {
    name: 'case-' + n,
    csl: genStyle({}),
    lang: pick(['', '', '', 'en-US', 'fr-FR', 'de-DE', 'ja-JP']),
    input: items,
    citation_items: citeItems(items),
    langparams: { persons: ['translit'], institutions: ['translit'], titles: ['translit', 'translat'], journals: ['translit'], publishers: ['translat'], places: ['translat'] },
    format: pick(['html', 'html', 'html', 'text', 'rtf', 'asciidoc']),
  };
  if (chance(0.25)) c.langparams = { persons: ['orig'], institutions: ['orig'], titles: ['orig'], journals: ['orig'], publishers: ['orig'], places: ['orig'] };
  if (chance(0.15)) {
    c.langparams.titles = ['translit', 'translat', 'orig'];
    c.langparams.journals = ['translit', 'translat'];
    c.tags = { translit: ['ja-Latn'], translat: ['en', 'fr'] };
    if (chance(0.7)) {
      c.multiaffix = range(24).reduce((a) => {
        const p = pick(['', '[', '(', '<i>', ' ', '\u201c']);
        const s = { '': '', '[': ']', '(': ')', '<i>': '</i>', ' ': '', '\u201c': '\u201d' }[p];
        a.push(p, s);
        return a;
      }, []);
    }
  }
  const ab = genAbbrevs(items);
  if (ab) c.abbreviations = ab;
  if (chance(0.5)) { c.year_suffix = {}; for (const it of items) c.year_suffix[it.id] = chance(0.7) ? Math.floor(rnd() * 30) : false; }
  if (chance(0.15)) c.options = { strict_text_case_locales: true };
  if (chance(0.15)) c.options = Object.assign(c.options || {}, { wrap_url_and_doi: true });
  if (chance(0.1)) c.options = Object.assign(c.options || {}, { consolidate_legal_items: true });
  if (chance(0.12)) c.options = Object.assign(c.options || {}, { variableWrapper: true });
  if (chance(0.1)) c.options = Object.assign(c.options || {}, { force_title_abbrev_fallback: true });
  Object.assign(c, runCase(c));
  cases.push(c);
}
const N2 = parseInt(process.argv[3] || '300', 10);
for (let n = 0; n < N2; n++) {
  const items = range(1 + Math.floor(rnd() * 3)).map((i) => genFocusedItem(i + 1));
  const c = {
    name: 'focused-' + n,
    csl: genFocusedStyle(),
    lang: pick(['', '', 'en-US', 'fr-FR', 'ja-JP']),
    input: items,
    citation_items: citeItems(items),
    langparams: pick([
      { persons: ['translit'], institutions: ['translit'], titles: ['translit', 'translat'], journals: ['translit'], publishers: ['translat'], places: ['translat'] },
      { persons: ['orig'], institutions: ['orig'], titles: ['orig'], journals: ['orig'], publishers: ['orig'], places: ['orig'] },
      { persons: ['translit'], institutions: ['translit'], titles: ['translat', 'translit', 'orig'], journals: ['translat'], publishers: ['translit', 'orig'], places: ['translit', 'translat', 'orig'] },
      { persons: ['orig'], institutions: ['orig'], titles: ['translit'], journals: ['translat'], publishers: ['translit'], places: ['translat'] },
      { persons: ['orig'], institutions: ['orig'], titles: ['orig', 'translit'], journals: ['orig', 'translat'], publishers: ['orig', 'translit', 'translat'], places: ['orig'] },
    ]),
    format: pick(['html', 'html', 'text', 'rtf']),
    tags: pick([{ translit: ['ja-Latn'], translat: ['en', 'fr'] }, { translit: ['ja-Latn'], translat: ['en-GB'] }, { translit: [], translat: ['de', 'en'] }, { translit: ['ja-Latn', 'ja'], translat: ['fr'] }]),
  };
  if (chance(0.5)) {
    const close = { '': '', '[': ']', '(': ')', '<i>': '</i>', ' ': '', '\u201c': '\u201d' };
    c.multiaffix = [];
    for (let i = 0; i < 24; i++) { const pp = pick(Object.keys(close)); c.multiaffix.push(pp, close[pp]); }
  }
  if (chance(0.8)) c.abbreviations = genFocusedAbbrevs(items);
  if (chance(0.2)) c.options = { strict_text_case_locales: true, normalize_lang_keys_to_lowercase: chance(0.5) };
  if (chance(0.2)) c.options = Object.assign(c.options || {}, { force_title_abbrev_fallback: true });
  Object.assign(c, runCase(c));
  cases.push(c);
}
write('render', { cases });
const errs = cases.filter((c) => c.citation_error || c.bibliography_error).length;
console.log('cases', cases.length, 'with a citeproc-js error', errs);
