// csl-intermediate-reference.cjs: dump citeproc-js 2.4.63's INTERMEDIATE state
// (what the Engine holds after it has built a style, merged its locales and
// normalised the input items) for the CSL test suite's 845 fixtures and for the
// site set (five styles over tests/data/csl/items.json). GitHub #792, epic #790.
// Run by scripts/csl-reference.sh. The Rust port (kovan-literature
// src/citeproc/) produces the same JSON; tests/citeproc_intermediate.rs
// compares.
//
// WHY INTERMEDIATE STATE. #792 ports style loading and build, locale merging
// and input normalisation, but nothing that renders: no fixture can pass until
// #793. So this stage is verified code to code on what exists, which is the
// state citeproc-js itself holds at that point.
//
// HOW EACH CASE IS BUILT (exactly as the existing generators do)
//   fixture: csl-testsuite-reference.cjs. The same fixture parser, the same Sys
//            (retrieveItem, retrieveLocale from vendor/citeproc-js/locale,
//            getAbbreviation, normalizeAbbrevsKey, OPTIONS copied onto sys),
//            `new CSL.Engine(sys, CSL)` (lang unset), addDateParserMonths with
//            the runner's Turkish months, then the runner's configuration that
//            runs before updateItems: submodes, OPTIONS as
//            development_extensions, LANGPARAMS, MULTIAFFIX, ABBREVIATIONS.
//   site:    csl-reference.cjs. `new CSL.Engine(sys, style, lang)` with the
//            committed style and locale files; sys has no getAbbreviation.
//   The site cases run first, in the same process: the fixtures then add the
//   Turkish months to the shared CSL.DateParser singleton, which the port
//   models as one DateParser per Engine (equivalent: the add is idempotent,
//   checked below).
//
// WHAT IS DUMPED (one JSON document per case, six sections; every object's
// keys sorted; undefined and functions omitted; a RegExp as {"$regexp":
// source}; a Token as its own properties with execs/tests as counts)
//   style           engine.opt, the five areas (citation, bibliography, intext,
//                   citation_sort, bibliography_sort: opt, root and the token
//                   list citeproc-js built and configured), engine.macros
//                   (each macro's token list), engine.tmp.cite_affixes, and
//                   engine.build.names_level. A token carries name, tokentype
//                   (0 START, 1 END, 2 SINGLETON), strings, decorations,
//                   variables, and every other own property that is data
//                   (postponed_macro, dateparts, locale_*, gender, match,
//                   next/succeed/fail jump indices, ...). Its closures are
//                   not data: `execs` becomes execs_n, `tests` tests_n and
//                   `test` has_test, so the port must register as many
//                   closures per token, in total, as citeproc-js does.
//   locale          engine.locale: for every language loaded, terms (forms,
//                   plurals, genders), opts, dates (the date format nodes),
//                   ord and noun-genders; plus engine.opt.gender.
//   items           engine.retrieveItem(id) for every INPUT item (fixtures) or
//                   every item of items.json (site), in input order, with the
//                   runner's sys state: dates as parsed date objects, page-first,
//                   language-name, the title/short-title and abbreviation
//                   fixes, authority for force_jurisdiction, and so on.
//   names           for every name in every name variable of each normalised
//                   item: _normalizeNameInput (the particle parser) and the
//                   static-ordering test, i.e. the input side of the name
//                   renderer (NameOutput._normalizeNameInput, _parseName,
//                   getStaticOrder), which the output queue then consumes.
//   numbers         engine.processNumber(false, Item, variable) for every
//                   numeric variable and page-first that an item has: the
//                   parsed values, labels, plural/numeric/collapsible flags.
//                   processNumber(node, ...) (styling, range mangling) renders
//                   and is #793.
//   citation_items  the fixture's citation items (CITATION-ITEMS, or each
//                   CITATIONS entry) after the input steps makeCitationCluster
//                   and processCitationCluster apply: a copy, parseLocator, and
//                   the locator-label parse (remapSectionVariable when
//                   consolidate_legal_items is set).
//
// A case whose Engine constructor throws is recorded as {"error": message}
// (a throw in a later section is recorded in that section only).
//
// WHAT IT WRITES (tests/data/csl/)
//   intermediate_reference.json       meta + per case the SHA-256 of each
//                                     section's compact JSON. The dump of all
//                                     845 fixtures is far over 10 MB (every
//                                     fixture repeats its locale), so the
//                                     committed form is digests.
//   intermediate_reference_full.json  the sections of the site set and of
//                                     SAMPLE_FIXTURES, for reading a difference.
//                                     Two reductions keep it small: a site
//                                     section identical to the first style's is
//                                     {"same_as": "apa"}, and a token list over
//                                     SUMMARY_OVER tokens (the *_sort areas of
//                                     APA and Chicago hold 68,000: macros are
//                                     inlined into sort keys) is replaced by
//                                     {count, sha256, first: the first 30}.
//                                     The digests above are always of the
//                                     UNREDUCED sections.
// `node scripts/csl-intermediate-reference.cjs --case NAME [--section S]`
// prints one case's JSON (a fixture name or site:<style>) so a failing digest
// can be diffed against the port's `INTERMEDIATE_DUMP` output.
'use strict';
const fs = require('fs');
const path = require('path');
const crypto = require('crypto');
const CSL = require(process.env.CITEPROC_MODULE);

const root = path.join(__dirname, '..');
const lit = path.join(root, 'crates', 'kovan-literature');
const fixtureDir = path.join(root, 'vendor/csl-test-suite/processor-tests/humans');
const localeDir = path.join(root, 'vendor/citeproc-js/locale');
const outDir = path.join(lit, 'tests/data/csl');

const SUMMARY_OVER = 500;
const SECTIONS = ['style', 'locale', 'items', 'names', 'numbers', 'citation_items'];
// Fixtures whose full dump is committed. One per area the port will meet, chosen
// by name before looking at any result.
const SAMPLE_FIXTURES = [
  'affix_CommaAfterQuote', 'date_Accessed', 'date_LocalizedTextDefault', 'locale_NonExistentLocaleDef',
  'locator_WorkaroundTestForSubVerbo', 'magic_NameParticle', 'magic_NumberRangeFrench',
  'name_CelticClanName', 'name_EtAlKanji', 'bugreports_ThesisUniversityAppearsTwice',
  'flipflop_Apostrophes', 'substitute_SuppressOrdinaryVariable',
];

// ---------------------------------------------------------------------------
// The test runner, copied from csl-testsuite-reference.cjs (itself a line by
// line copy of juris-m/citeproc-test-runner b1e72d5c). Only what runs before
// the first updateItems is needed.
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
    if (FIXTURE_SECTIONS[key] === 'json') {
      try {
        obj[key] = JSON.parse(obj[key]);
      } catch (err) {
        throw new Error('JSON parse fail for tag "' + key + '" in ' + fpth);
      }
    }
  }
  for (const key of REQUIRED) {
    if (typeof obj[key] === 'undefined') throw new Error('Missing required tag "' + key + '" in ' + fpth);
  }
  return obj;
}

function normalizeAbbrevsKey(variable, key) {
  key = key ? ('' + key).trim() : '';
  if (['jurisdiction', 'country'].indexOf(variable) > -1) return key.toUpperCase();
  key = key
    .toString()
    .replace(/(?:\b|^)(?:and|et|y|und|l[ae]|the|[ld]')(?:\b|$)|[\x21-\x2C.\/\x3A-\x40\x5B-\x60\\\x7B\x7D-\x7E]/gi, '')
    .replace(/\s*\x7C\s*/g, '\x7C')
    .replace(/\./g, ' ')
    .replace(/\s+/g, ' ')
    .trim();
  return key.toLowerCase();
}

function RunnerSys(test) {
  this.test = test;
  this._acache = { default: new CSL.AbbreviationSegments() };
  this._cache = {};
  this._ids = [];
  for (const item of this.test.INPUT) {
    this._cache[item.id] = item;
    this._ids.push(item.id);
  }
  if (this.test.OPTIONS) for (const option in this.test.OPTIONS) this[option] = this.test.OPTIONS[option];
}
RunnerSys.prototype.retrieveItem = function (id) { return this._cache[id]; };
RunnerSys.prototype.retrieveLocale = function (lang) {
  try {
    return fs.readFileSync(path.join(localeDir, 'locales-' + lang + '.xml')).toString().replace(/\s*<\?[^>]*\?>\s*\n/g, '');
  } catch (e) {
    return false;
  }
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

// The runner's configuration between `new CSL.Engine` and the first updateItems.
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
  const langParams = {
    persons: ['translit'], institutions: ['translit'], titles: ['translit', 'translat'],
    journals: ['translit'], publishers: ['translat'], places: ['translat'],
  };
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
}

// ---------------------------------------------------------------------------
// Canonical form.
function isToken(v) {
  return v instanceof CSL.Token;
}
const TOKEN_JUMPS = ['next', 'succeed', 'fail'];

function formatterName(state, f) {
  if (f === state.fun.romanizer) return 'romanizer';
  if (f === state.fun.ordinalizer) return 'ordinalizer';
  if (f === state.fun.long_ordinalizer) return 'long_ordinalizer';
  return 'other';
}

function canon(v, state, nested) {
  if (v === undefined || typeof v === 'function') return undefined;
  if (v === null) return null;
  const t = typeof v;
  if (t === 'number') return Number.isFinite(v) ? v : null;
  if (t === 'string' || t === 'boolean') return v;
  if (v instanceof RegExp) return { $regexp: v.source };
  if (Array.isArray(v)) {
    return v.map((x) => {
      const c = canon(x, state, nested);
      return c === undefined ? null : c;
    });
  }
  if (t === 'object') {
    const out = {};
    if (isToken(v)) {
      for (const k of Object.keys(v).sort()) {
        if (nested && TOKEN_JUMPS.indexOf(k) > -1) continue;
        if (k === 'execs') { out.execs_n = v.execs.length; continue; }
        if (k === 'tests') { out.tests_n = v.tests.length; continue; }
        if (k === 'test') { if (v.test) out.has_test = true; continue; }
        if (k === 'formatter') { out.formatter = formatterName(state, v.formatter); continue; }
        const c = canon(v[k], state, true);
        if (c !== undefined) out[k] = c;
      }
      return out;
    }
    for (const k of Object.keys(v).sort()) {
      const c = canon(v[k], state, nested);
      if (c !== undefined) out[k] = c;
    }
    return out;
  }
  return undefined;
}

function jsonOf(x) {
  return JSON.stringify(x === undefined ? null : x);
}
function sha(x) {
  return crypto.createHash('sha256').update(jsonOf(x)).digest('hex');
}

// ---------------------------------------------------------------------------
// Sections.
function styleSection(engine) {
  const areas = {};
  for (const a of CSL.AREAS) {
    const area = engine[a];
    const o = { root: area.root, tokens: area.tokens.map((t) => canon(t, engine, false)) };
    o.opt = canon(area.opt, engine, false);
    areas[a] = o;
  }
  const macros = {};
  for (const k of Object.keys(engine.macros).sort()) macros[k] = engine.macros[k].map((t) => canon(t, engine, false));
  return {
    csl_version: engine.csl_version,
    processor_version: engine.processor_version,
    opt: canon(engine.opt, engine, false),
    areas,
    macros,
    cite_affixes: canon(engine.tmp.cite_affixes, engine, false),
    names_level: engine.build.names_level,
  };
}

function localeSection(engine) {
  const out = {};
  for (const lang of Object.keys(engine.locale).sort()) out[lang] = canon(engine.locale[lang], engine, false);
  return { locale: out, gender: canon(engine.opt.gender, engine, false) };
}

function plain(x) {
  return JSON.parse(JSON.stringify(x));
}

function namesSection(engine, Item) {
  const fake = Object.create(CSL.NameOutput.prototype);
  fake.state = engine;
  fake.Item = Item;
  const out = {};
  for (const v of CSL.NAME_VARIABLES) {
    const list = Item[v];
    if (!Array.isArray(list)) continue;
    out[v] = list.map((name) => {
      const rec = {};
      try {
        const raw = plain(name);
        const forStatic = plain(name);
        if (!forStatic.family) forStatic.family = '';
        if (!forStatic.given) forStatic.given = '';
        rec.static_ordering = fake.getStaticOrder(forStatic);
        const clone = plain(name);
        rec.name = canon(fake._normalizeNameInput(clone), engine, false);
        void raw;
      } catch (e) {
        rec.error = String(e && e.message ? e.message : e);
      }
      return rec;
    });
  }
  return out;
}

const NUMBER_VARS = CSL.NUMERIC_VARIABLES.concat(['page-first']);
function numbersSection(engine, Item) {
  const out = {};
  for (const v of NUMBER_VARS) {
    if (typeof Item[v] === 'undefined') continue;
    engine.tmp.shadow_numbers = {};
    try {
      engine.processNumber(false, Item, v);
      out[v] = canon(engine.tmp.shadow_numbers, engine, false);
    } catch (e) {
      out[v] = { error: String(e && e.message ? e.message : e) };
    }
  }
  engine.tmp.shadow_numbers = {};
  return out;
}

// api_cite.js makeCitationCluster, the loop over citation.citationItems.
function citationItemInput(engine, ci) {
  const item = {};
  for (const key in ci) item[key] = ci[key];
  const Item = engine.retrieveItem('' + item.id);
  let it = CSL.parseLocator.call(engine, item);
  if (engine.opt.development_extensions.consolidate_legal_items) {
    engine.remapSectionVariable([[Item, it]]);
  }
  if (engine.opt.development_extensions.locator_label_parse) {
    if (it.locator && ['bill', 'gazette', 'legislation', 'regulation', 'treaty'].indexOf(Item.type) === -1 && (!it.label || it.label === 'page')) {
      const m = CSL.LOCATOR_LABELS_REGEXP.exec(it.locator);
      if (m) {
        const tryLabel = CSL.LOCATOR_LABELS_MAP[m[2]];
        if (engine.getTerm(tryLabel)) {
          it.label = tryLabel;
          it.locator = m[3];
        }
      }
    }
  }
  return it;
}

function citationItemsSection(engine, test) {
  const lists = [];
  if (test && test['CITATION-ITEMS']) for (const c of test['CITATION-ITEMS']) lists.push(c);
  if (test && test.CITATIONS) for (const c of test.CITATIONS) lists.push(c[0].citationItems);
  return lists.map((list) =>
    list.map((ci) => {
      try {
        return canon(plain(citationItemInput(engine, plain(ci))), engine, false);
      } catch (e) {
        return { error: String(e && e.message ? e.message : e) };
      }
    })
  );
}

function guarded(fn) {
  try {
    return fn();
  } catch (e) {
    return { error: String(e && e.message ? e.message : e) };
  }
}

function dumpEngine(engine, items, test) {
  const sec = {};
  sec.style = guarded(() => styleSection(engine));
  sec.locale = guarded(() => localeSection(engine));
  const norm = [];
  sec.items = guarded(() => {
    const o = {};
    for (const it of items) {
      const I = engine.retrieveItem('' + it.id);
      norm.push(I);
      o[it.id] = canon(plain(I), engine, false);
    }
    return o;
  });
  sec.names = guarded(() => {
    const o = {};
    for (const I of norm) o[I.id] = namesSection(engine, I);
    return o;
  });
  sec.numbers = guarded(() => {
    const o = {};
    for (const I of norm) o[I.id] = numbersSection(engine, I);
    return o;
  });
  sec.citation_items = guarded(() => citationItemsSection(engine, test));
  return sec;
}

// ---------------------------------------------------------------------------
// Cases.
const SITE_STYLES = [
  { name: 'apa', file: 'apa.csl' },
  { name: 'chicago-author-date', file: 'chicago-author-date.csl' },
  { name: 'ieee', file: 'ieee.csl' },
  { name: 'nature', file: 'nature.csl', locale: 'en-GB' },
  { name: 'vancouver', file: 'nlm-citation-sequence.csl' },
];

function siteCase(st) {
  const siteItems = JSON.parse(fs.readFileSync(path.join(lit, 'tests/data/csl/items.json'), 'utf8'));
  const byId = {};
  for (const it of siteItems) byId[it.id] = it;
  const retrieveLocale = (lang) => {
    const p = path.join(lit, 'data/csl', `locales-${lang}.xml`);
    return fs.existsSync(p) ? fs.readFileSync(p, 'utf8') : false;
  };
  const style = fs.readFileSync(path.join(lit, 'data/csl', st.file), 'utf8');
  const sys = { retrieveLocale, retrieveItem: (id) => byId[id] };
  let engine;
  try {
    engine = new CSL.Engine(sys, style, st.locale || 'en-US');
  } catch (e) {
    return { error: String(e && e.message ? e.message : e) };
  }
  engine.setOutputFormat('html');
  return dumpEngine(engine, siteItems, null);
}

function fixtureCase(name) {
  const test = parseFixture(name, path.join(fixtureDir, name + '.txt'));
  const sys = new RunnerSys(test);
  CSL.debug = function () {};
  let engine;
  try {
    engine = new CSL.Engine(sys, test.CSL);
  } catch (e) {
    return { error: String(e && e.message ? e.message : e) };
  }
  configureLikeRunner(sys, test, engine);
  sys.style = engine;
  return dumpEngine(engine, test.INPUT, test);
}

// ---------------------------------------------------------------------------
function dateParserIsIdempotent() {
  const p = CSL.DateParser;
  p.addDateParserMonths(TURKISH_MONTHS);
  const a = JSON.stringify([p.monthSets, p.monthAbbrevs, p.monthRexStrs]);
  p.addDateParserMonths(TURKISH_MONTHS);
  const b = JSON.stringify([p.monthSets, p.monthAbbrevs, p.monthRexStrs]);
  return a === b;
}

function reduce(dump) {
  const out = {};
  for (const s of Object.keys(dump)) out[s] = dump[s];
  if (dump.style && dump.style.areas) {
    const areas = {};
    for (const a of Object.keys(dump.style.areas)) {
      const area = dump.style.areas[a];
      if (area.tokens.length > SUMMARY_OVER) {
        areas[a] = Object.assign({}, area, {
          tokens: { count: area.tokens.length, sha256: sha(area.tokens), first: area.tokens.slice(0, 30) },
        });
      } else {
        areas[a] = area;
      }
    }
    out.style = Object.assign({}, dump.style, { areas });
  }
  return out;
}

const args = process.argv.slice(2);
const argv = (k) => (args.indexOf(k) > -1 ? args[args.indexOf(k) + 1] : null);

if (argv('--case')) {
  const c = argv('--case');
  const dump = c.startsWith('site:') ? siteCase(SITE_STYLES.find((s) => s.name === c.slice(5))) : fixtureCase(c);
  const sect = argv('--section');
  if (args.indexOf('--raw') > -1) process.stdout.write(jsonOf(dump[sect]));
  else process.stdout.write(JSON.stringify(sect ? dump[sect] : dump, null, 1) + '\n');
  process.exit(0);
}

const names = fs.readdirSync(fixtureDir).filter((n) => /^[a-z]+_.*\.txt$/.test(n)).sort().map((n) => n.replace(/\.txt$/, ''));
const digests = {};
const full = { site: {}, fixtures: {} };
const siteDigests = {};
for (const st of SITE_STYLES) {
  const dump = siteCase(st);
  siteDigests[st.name] = {};
  const reduced = reduce(dump);
  full.site[st.name] = {};
  for (const s of SECTIONS) {
    siteDigests[st.name][s] = sha(dump[s]);
    const first = SITE_STYLES[0].name;
    full.site[st.name][s] = st.name !== first && siteDigests[st.name][s] === siteDigests[first][s]
      ? { same_as: first }
      : reduced[s];
  }
}
// The idempotence the port's one-parser-per-engine model relies on is checked
// after the site cases (the first add happens in the first fixture).
let errors = 0;
for (const n of names) {
  const dump = fixtureCase(n);
  if (dump.error) {
    digests[n] = { error: dump.error };
    errors++;
    continue;
  }
  digests[n] = {};
  for (const s of SECTIONS) digests[n][s] = sha(dump[s]);
  if (SAMPLE_FIXTURES.indexOf(n) > -1) full.fixtures[n] = reduce(dump);
}
const idem = dateParserIsIdempotent();
if (!idem) throw new Error('addDateParserMonths is not idempotent: the one-parser-per-engine model is wrong');
const missing = SAMPLE_FIXTURES.filter((n) => names.indexOf(n) === -1);
if (missing.length) throw new Error('SAMPLE_FIXTURES not in the suite: ' + missing.join(', '));

const pkg = JSON.parse(fs.readFileSync(path.join(path.dirname(process.env.CITEPROC_MODULE), 'package.json'), 'utf8'));
const meta = {
  engine: 'citeproc-js ' + pkg.version,
  node: process.version,
  generator: 'scripts/csl-intermediate-reference.cjs',
  sections: SECTIONS,
  digest: 'SHA-256 of JSON.stringify(section) with every object\'s keys sorted, undefined and functions omitted',
  fixtures: names.length,
  fixtures_whose_engine_constructor_threw: errors,
  date_parser_add_is_idempotent: idem,
  sample_fixtures: SAMPLE_FIXTURES,
};
fs.writeFileSync(path.join(outDir, 'intermediate_reference.json'), JSON.stringify({ meta, site: siteDigests, fixtures: digests }, null, 1) + '\n');
fs.writeFileSync(path.join(outDir, 'intermediate_reference_full.json'), JSON.stringify({ meta, site: full.site, fixtures: full.fixtures }, null, 1) + '\n');
console.log(`csl-intermediate-reference: ${names.length} fixtures (${errors} engine constructors threw), ${SITE_STYLES.length} site styles`);
