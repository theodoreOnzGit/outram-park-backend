// The engine state src/citeproc/build_retrieve_item.rs's whole-suite test needs for each
// of the 845 CSL test-suite fixtures, so that the port can reproduce the `items`, `names`,
// `numbers` and `citation_items` digests of tests/data/csl/intermediate_reference.json
// without the style/locale builder: engine.opt (deduplicated), the abbreviation cache,
// bibliography.opt.track_container_items, and the locale terms citeproc-js read
// (getTerm calls logged while the sections are produced).
// Setup copied from scripts/csl-intermediate-reference.cjs (the runner's configuration).
// Output: tests/data/csl/units/fixture_contexts.json
'use strict';
const fs = require('fs');
const path = require('path');
const crypto = require('crypto');
const { root, write, plain } = require('./common.cjs');
const CSL = require(process.env.CITEPROC_MODULE);
CSL.debug = function () {};

const fixtureDir = path.join(root, 'vendor/csl-test-suite/processor-tests/humans');
const localeDir = path.join(root, 'vendor/citeproc-js/locale');

const FIXTURE_SECTIONS = {
  CSL: 'xml', KEYS: 'json', DESCRIPTION: 'string', INPUT: 'json', MODE: 'string',
  RESULT: 'string', NAME: 'string', PATH: 'string', ABBREVIATIONS: 'json',
  BIBENTRIES: 'json', BIBSECTION: 'json', 'CITATION-ITEMS': 'json',
  CITATIONS: 'json', INPUT2: 'json', LANGPARAMS: 'json', MULTIAFFIX: 'json',
  OPTIONS: 'json', OPTIONZ: 'json',
};
const REQUIRED = ['CSL', 'INPUT', 'MODE', 'RESULT', 'NAME', 'PATH'];
function parseFixture(tn, fpth) {
  const obj = { NAME: [tn], PATH: [fpth] };
  const names = Object.keys(FIXTURE_SECTIONS).join('|');
  const openRex = new RegExp('^.*>>===*\\s(' + names + ')\\s.*=>>.*');
  const closeRex = new RegExp('^.*<<===*\\s(' + names + ')\\s.*=<<.*');
  let section = false;
  let state = null;
  const raw = fs.readFileSync(fpth).toString();
  for (const line of raw.split(/(?:\r\n|\n)/)) {
    let m = null;
    if (openRex.test(line)) {
      m = openRex.exec(line);
      if (state) throw new Error('Attempted to open tag "' + m[1] + '" before tag "' + section + '" was closed.');
      section = m[1];
      state = 'opening';
    } else if (closeRex.test(line)) {
      m = closeRex.exec(line);
      if (section !== m[1]) throw new Error('Expected closing tag "' + section + '" but found "' + m[1] + '"');
      state = 'closing';
      if (section === 'RESULT' && !obj[section]) obj[section] = [''];
    } else if (state === 'opening') {
      obj[section] = [];
      state = 'reading';
    } else if (state === 'closing') {
      state = null;
    }
    if (state === 'reading') obj[section].push(line);
  }
  for (const key of Object.keys(obj)) {
    obj[key] = obj[key].join('\n');
    if (FIXTURE_SECTIONS[key] === 'json') obj[key] = JSON.parse(obj[key]);
  }
  return obj;
}
function normalizeAbbrevsKey(variable, key) {
  key = key ? ('' + key).trim() : '';
  if (['jurisdiction', 'country'].indexOf(variable) > -1) return key.toUpperCase();
  key = key.toString()
    .replace(/(?:\b|^)(?:and|et|y|und|l[ae]|the|[ld]')(?:\b|$)|[\x21-\x2C.\/\x3A-\x40\x5B-\x60\\\x7B\x7D-\x7E]/gi, '')
    .replace(/\s*\x7C\s*/g, '\x7C').replace(/\./g, ' ').replace(/\s+/g, ' ').trim();
  return key.toLowerCase();
}
function RunnerSys(test) {
  this.test = test;
  this._acache = { default: new CSL.AbbreviationSegments() };
  this._cache = {};
  for (const item of this.test.INPUT) this._cache[item.id] = item;
  if (this.test.OPTIONS) for (const option in this.test.OPTIONS) this[option] = this.test.OPTIONS[option];
}
RunnerSys.prototype.retrieveItem = function (id) { return this._cache[id]; };
RunnerSys.prototype.retrieveLocale = function (lang) {
  try { return fs.readFileSync(path.join(localeDir, 'locales-' + lang + '.xml')).toString().replace(/\s*<\?[^>]*\?>\s*\n/g, ''); } catch (e) { return false; }
};
RunnerSys.prototype.retrieveStyleModule = function () { return null; };
RunnerSys.prototype.getAbbreviation = function (dummyListNameVar, obj, jurisdiction, category, key) {
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
RunnerSys.prototype.normalizeAbbrevsKey = normalizeAbbrevsKey;
const TURKISH_MONTHS = ['ocak', 'Şubat', 'mart', 'nisan', 'mayıs', 'haziran', 'temmuz', 'ağustos', 'eylül', 'ekim', 'kasım', 'aralık', 'bahar', 'yaz', 'sonbahar', 'kış'];
function configureLikeRunner(sys, test, style) {
  style.fun.dateparser.addDateParserMonths(TURKISH_MONTHS);
  const mode = (test.MODE || 'all').split('-');
  const submode = {};
  for (let i = 1; i < mode.length; i++) submode[mode[i]] = true;
  for (const fmt of ['rtf', 'plain', 'asciidoc', 'xslfo']) if (submode[fmt]) style.setOutputFormat(fmt);
  if (submode.suppress_trailing_punctuation) style.citation.opt.suppressTrailingPunctuation = true;
  for (const opt in test.OPTIONS) {
    if (opt === 'variableWrapper') continue;
    style.opt.development_extensions[opt] = test.OPTIONS[opt];
  }
  const langParams = { persons: ['translit'], institutions: ['translit'], titles: ['translit', 'translat'], journals: ['translit'], publishers: ['translat'], places: ['translat'] };
  if (test.LANGPARAMS) {
    for (const key in test.LANGPARAMS) {
      if (key === 'langs') {
        const langsToUse = test.LANGPARAMS[key];
        if (langsToUse.translat) style.setLangTagsForCslTranslation(langsToUse.translat);
        if (langsToUse.translit) style.setLangTagsForCslTransliteration(langsToUse.translat);
        continue;
      }
      langParams[key] = test.LANGPARAMS[key];
    }
  }
  style.setLangPrefsForCites(langParams);
  if (test.MULTIAFFIX) style.setLangPrefsForCiteAffixes(test.MULTIAFFIX);
  if (test.ABBREVIATIONS) {
    const abbrevs = {};
    for (const jurisd in test.ABBREVIATIONS) {
      abbrevs[jurisd] = {};
      for (const segment in test.ABBREVIATIONS[jurisd]) {
        abbrevs[jurisd][segment] = {};
        for (const key in test.ABBREVIATIONS[jurisd][segment]) {
          const isJurisdiction = jurisd === 'default' && segment === 'place' && key.toUpperCase() === key;
          const isCourt = ['institution-entire', 'institution-part'].indexOf(segment) > -1 && segment.toLowerCase() === segment;
          const normkey = !isJurisdiction && !isCourt ? sys.normalizeAbbrevsKey('title', key) : key;
          abbrevs[jurisd][segment][normkey] = test.ABBREVIATIONS[jurisd][segment][key];
        }
      }
    }
    sys._acache = Object.assign(sys._acache, abbrevs);
  }
  return test.ABBREVIATIONS ? plain(sys._acache) : {};
}

// The same sections the dump produces; used here only to log getTerm calls.
const DUMP = fs.readFileSync(path.join(root, 'scripts/csl-intermediate-reference.cjs'), 'utf8');
function grab(name) {
  // extract a top-level function's source from the dump script
  const i = DUMP.indexOf('function ' + name + '(');
  let depth = 0, j = DUMP.indexOf('{', i);
  for (let k = j; k < DUMP.length; k++) {
    if (DUMP[k] === '{') depth++;
    if (DUMP[k] === '}') { depth--; if (depth === 0) return DUMP.slice(i, k + 1); }
  }
  throw new Error('no ' + name);
}
const helpers = ['isToken', 'formatterName', 'canon', 'plain', 'namesSection', 'numbersSection', 'citationItemInput', 'citationItemsSection']
  .map(grab).join('\n') + '\nconst TOKEN_JUMPS = ["next","succeed","fail"]; const NUMBER_VARS = CSL.NUMERIC_VARIABLES.concat(["page-first"]);';
const sections = new Function('CSL', helpers + '\nreturn { namesSection, numbersSection, citationItemsSection, canon };')(CSL);

const names = fs.readdirSync(fixtureDir).filter((n) => /^[a-z]+_.*\.txt$/.test(n)).sort().map((n) => n.replace(/\.txt$/, ''));
const contexts = {};
const fixtures = {};
for (const n of names) {
  const test = parseFixture(n, path.join(fixtureDir, n + '.txt'));
  const sys = new RunnerSys(test);
  const engine = new CSL.Engine(sys, test.CSL);
  const abbrevs = configureLikeRunner(sys, test, engine);
  const log = {};
  const orig = engine.getTerm;
  engine.getTerm = function (term, form, plural, gender, mode, force) {
    const key = [term === undefined ? '' : term, typeof form === 'string' ? form : '~', typeof plural === 'number' ? plural : 0,
      (typeof gender === 'string' && gender) ? gender : '~', typeof mode === 'number' ? mode : '~', force ? 1 : 0].join('|');
    const r = orig.apply(this, arguments);
    log[key] = r === undefined ? null : r;
    return r;
  };
  const ctx = {
    opt: JSON.parse(JSON.stringify(engine.opt, (k, v) => (typeof v === 'function' ? undefined : v))),
    track: plain(engine.bibliography.opt.track_container_items === undefined ? null : engine.bibliography.opt.track_container_items),
  };
  const h = crypto.createHash('sha256').update(JSON.stringify(ctx)).digest('hex').slice(0, 12);
  contexts[h] = ctx;
  // run the sections the way dumpEngine does (items first, then names, numbers, citation items)
  const norm = [];
  for (const it of test.INPUT) norm.push(engine.retrieveItem('' + it.id));
  for (const I of norm) sections.namesSection(engine, I);
  for (const I of norm) sections.numbersSection(engine, I);
  const lists = [];
  if (test['CITATION-ITEMS']) for (const c of test['CITATION-ITEMS']) lists.push(c);
  if (test.CITATIONS) for (const c of test.CITATIONS) lists.push(c[0].citationItems);
  for (const list of lists) for (const ci of list) { try { const item = {}; for (const key in ci) item[key] = ci[key]; engine.retrieveItem('' + item.id); } catch (e) {} }
  // the dump runs citationItemsSection (which reads getTerm for locator_label_parse)
  sections.citationItemsSection(engine, test);
  const f = { ctx: h };
  if (Object.keys(log).length) f.terms = log;
  if (Object.keys(abbrevs).length > 1 || (abbrevs.default && Object.keys(abbrevs.default).length)) f.abbrevs = abbrevs;
  fixtures[n] = f;
}
// The five site styles over tests/data/csl/items.json (siteCase of the dump script).
const lit = path.join(root, 'crates', 'kovan-literature');
const SITE_STYLES = [
  { name: 'apa', file: 'apa.csl' }, { name: 'chicago-author-date', file: 'chicago-author-date.csl' },
  { name: 'ieee', file: 'ieee.csl' }, { name: 'nature', file: 'nature.csl', locale: 'en-GB' },
  { name: 'vancouver', file: 'nlm-citation-sequence.csl' },
];
const site = {};
const siteItems = JSON.parse(fs.readFileSync(path.join(lit, 'tests/data/csl/items.json'), 'utf8'));
for (const st of SITE_STYLES) {
  const byId = {};
  for (const it of siteItems) byId[it.id] = it;
  const retrieveLocale = (lang) => { const p = path.join(lit, 'data/csl', `locales-${lang}.xml`); return fs.existsSync(p) ? fs.readFileSync(p, 'utf8') : false; };
  const style = fs.readFileSync(path.join(lit, 'data/csl', st.file), 'utf8');
  const sys = { retrieveLocale, retrieveItem: (id) => byId[id] };
  const engine = new CSL.Engine(sys, style, st.locale || 'en-US');
  engine.setOutputFormat('html');
  const log = {};
  const orig = engine.getTerm;
  engine.getTerm = function (term, form, plural, gender, mode, force) {
    const key = [term === undefined ? '' : term, typeof form === 'string' ? form : '~', typeof plural === 'number' ? plural : 0,
      (typeof gender === 'string' && gender) ? gender : '~', typeof mode === 'number' ? mode : '~', force ? 1 : 0].join('|');
    const r = orig.apply(this, arguments);
    log[key] = r === undefined ? null : r;
    return r;
  };
  const ctx = {
    opt: JSON.parse(JSON.stringify(engine.opt, (k, v) => (typeof v === 'function' ? undefined : v))),
    track: plain(engine.bibliography.opt.track_container_items === undefined ? null : engine.bibliography.opt.track_container_items),
  };
  const h = crypto.createHash('sha256').update(JSON.stringify(ctx)).digest('hex').slice(0, 12);
  contexts[h] = ctx;
  const norm = [];
  for (const it of siteItems) norm.push(engine.retrieveItem('' + it.id));
  for (const I of norm) sections.namesSection(engine, I);
  for (const I of norm) sections.numbersSection(engine, I);
  site[st.name] = { ctx: h, terms: log };
}
write('fixture_contexts', { order: names, contexts, fixtures, site });
