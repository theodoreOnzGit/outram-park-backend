// csl-testsuite-reference.cjs: run the CSL test suite's processor fixtures
// through citeproc-js 2.4.63 exactly as citeproc-js's own runner does, and
// record what citeproc-js produced (GitHub #791, epic #790). Run by
// scripts/csl-reference.sh.
//
// WHAT IT READS (all git-ignored, under vendor/; fetched by csl-reference.sh)
//   vendor/csl-test-suite/processor-tests/humans/*.txt
//       citation-style-language/test-suite at 6eefc5b07c6969ab8999e48542acbcc131cba864
//       (845 fixtures; the "863" in the first brief was a miscount)
//   vendor/citeproc-js/locale/
//       the locales submodule citeproc-js 2.4.63 pins (6b0cb4689127), the
//       directory the runner's Sys.retrieveLocale reads
//   CITEPROC_MODULE: npm citeproc 2.4.63's citeproc_commonjs.js, which
//       scripts/csl-reference.sh checks is byte-identical to the build of
//       vendor/citeproc-js (juris-m/citeproc-js 73bc1b44) src/
//
// THE RUNNER BEING REPLICATED
//   juris-m/citeproc-test-runner (package 1.1.116, commit b1e72d5c, MIT):
//     lib/fixture-parser.js  -> parseFixture() below, line by line
//     lib/sections.js        -> SECTIONS below
//     lib/sys.js (Sys.run)   -> run() below, the non-"all" branch (no fixture
//                               in this suite has MODE "all")
//     lib/preload.js         -> preloadAbbreviations() below (see the note
//                               there on what is and is not ported)
//     lib/templateJS.js      -> passes := assert.equal(ret, RESULT)
//   Parsing: a section opens on a line matching ^.*>>===*\s(NAME)\s.*=>>.*
//   and closes on ^.*<<===*\s(NAME)\s.*=<<.*; the lines between are joined
//   with "\n" (not trimmed); json sections are JSON.parse'd. VERSION and TAGS
//   are not sections to the runner and are ignored. A section absent from the
//   fixture is undefined, except an empty RESULT, which is "".
//   Driving (per fixture, a fresh Sys and a fresh CSL.Engine, one Node process,
//   fixtures in sorted order as mocha ran them): MODE "citation" or
//   "bibliography" with submodes after "-" (rtf, plain, asciidoc, xslfo,
//   nosort, header, suppress_trailing_punctuation, nojuris); OPTIONS become
//   development_extensions and properties of sys; LANGPARAMS, MULTIAFFIX,
//   ABBREVIATIONS (keys normalised by normalizeAbbrevsKey); BIBENTRIES ->
//   updateItems per set (else updateItems(all ids) unless CITATIONS);
//   CITATION-ITEMS -> makeCitationCluster each; CITATIONS ->
//   processCitationCluster sequence with the runner's updateDoc bookkeeping
//   and INPUT2; bibliography -> makeBibliography(BIBSECTION?) joined as
//   bibstart + entries + bibend, or the sorted "key: value" header dump for
//   mode bibliography-header. Output normalised with normalize-newline
//   (\r\n and \r to \n). HTML output unless a submode says otherwise.
//
// WHAT IT WRITES
//   crates/kovan-literature/tests/data/csl/test_suite_reference.json
//     meta, and per fixture name: mode, output (citeproc-js's actual output,
//     null if it threw), error (the exception text, only if it threw),
//     expected (the fixture's RESULT), passes (output === expected).
//   THE PORT IS VERIFIED AGAINST `output`, NOT AGAINST `expected`. `passes`
//   only records where citeproc-js itself disagrees with the suite; there the
//   port must reproduce citeproc-js, not the suite.
//   The fixtures themselves (inputs, styles) are NOT committed: the suite has
//   no licence file and the #790 licence decision keeps them in vendor/.
//   `expected` equals `output` wherever `passes` is true.
//
// Not ported from the runner, and why: the "all" mode and style-capabilities
// (no fixture uses MODE all); jurisdiction abbreviation files and style
// modules (no fixture in this suite has a jurisdiction; the script throws if
// one appears, rather than run without them).
'use strict';
const fs = require('fs');
const path = require('path');
const CSL = require(process.env.CITEPROC_MODULE);

const root = path.join(__dirname, '..');
const fixtureDir = path.join(root, 'vendor/csl-test-suite/processor-tests/humans');
const localeDir = path.join(root, 'vendor/citeproc-js/locale');
const outFile = path.join(root, 'crates/kovan-literature/tests/data/csl/test_suite_reference.json');

// lib/sections.js: name -> type (only the processor-test sections matter).
const SECTIONS = {
  CSL: 'xml', KEYS: 'json', DESCRIPTION: 'string', INPUT: 'json', MODE: 'string',
  RESULT: 'string', NAME: 'string', PATH: 'string', ABBREVIATIONS: 'json',
  BIBENTRIES: 'json', BIBSECTION: 'json', 'CITATION-ITEMS': 'json',
  CITATIONS: 'json', INPUT2: 'json', LANGPARAMS: 'json', MULTIAFFIX: 'json',
  OPTIONS: 'json', OPTIONZ: 'json',
};
const REQUIRED = ['CSL', 'INPUT', 'MODE', 'RESULT', 'NAME', 'PATH'];

// lib/fixture-parser.js
function parseFixture(tn, fpth) {
  const obj = { NAME: [tn], PATH: [fpth] };
  const names = Object.keys(SECTIONS).join('|');
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
    if (SECTIONS[key] === 'json') {
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

function normalizeNewline(s) {
  return s.replace(/\r\n|\r/g, '\n');
}

// lib/preload.js, preloadAbbreviations, ported for items WITHOUT a
// jurisdiction (the runner then reads no abbreviation file; it only creates
// empty AbbreviationSegments entries in style.transform.abbrevs and sets
// item["language-name"], which this does the same). An item with a
// jurisdiction would read citeproc-abbrevs files: not available, so throw.
function preloadAbbreviations(styleEngine, citation, acache) {
  const styleID = styleEngine.opt.styleID;
  const obj = styleEngine.transform.abbrevs;
  const rawFieldFunction = {
    'container-title': (item, v) => (item[v] ? [item[v]] : []),
    'collection-title': (item, v) => (item[v] ? [item[v]] : []),
    'institution-entire': (item, v) => {
      const ret = [];
      const names = item[v];
      for (let i = 0; i < names.length; i++) if (names[i].literal) ret.push(names[i].literal);
      return ret.length ? ret : [];
    },
    'institution-part': (item, v) => {
      const ret = [];
      const names = item[v];
      for (let i = 0; i < names.length; i++) {
        if (names[i].literal) for (const p of names[i].literal.split(/\s*\|\s*/)) ret.push(p);
      }
      return ret.length ? ret : [];
    },
    number: (item, v) => (v === 'number' ? [item[v]] : []),
    title: (item, v) => (['title', 'title-short', 'genre', 'event', 'medium'].indexOf(v) > -1 ? [item[v]] : []),
    place: (item, v) =>
      ['archive-place', 'publisher-place', 'event-place', 'country', 'jurisdiction', 'language-name', 'language-name-original'].indexOf(v) > -1
        ? [item[v]]
        : [],
  };
  const rawItemFunction = {
    nickname: (item) => {
      const ret = [];
      for (const varname in CSL.CREATORS) {
        if (item[varname]) {
          for (let i = 0; i < item[varname].length; i++) {
            const name = item[varname][i];
            if (!name.literal) ret.push(CSL.Util.Names.getRawName(item[varname][i]));
          }
        }
      }
      return ret.length ? ret : false;
    },
    hereinafter: (item) => [item.id],
    classic: (item) => [item.id],
  };
  const setCacheEntry = (jurisdiction, category, rawval, domain) => {
    if (!rawval) return;
    rawval = '' + rawval;
    const itemJurisd = domain ? jurisdiction + '@' + domain : jurisdiction;
    if (!obj[itemJurisd]) obj[itemJurisd] = new CSL.AbbreviationSegments();
    if (!obj[itemJurisd][category]) obj[itemJurisd][category] = {};
    let abbrev = false;
    if (acache[itemJurisd] && acache[itemJurisd][category] && acache[itemJurisd][category][rawval]) {
      abbrev = acache[itemJurisd][category][rawval];
    }
    if (abbrev) obj[itemJurisd][category][rawval] = abbrev;
  };
  const registerEntries = (val, jurisdictions, category, passedField, domain) => {
    if (passedField) val = styleEngine.sys.normalizeAbbrevsKey(passedField, val);
    for (let i = jurisdictions.length; i > 0; i--) {
      setCacheEntry(jurisdictions.slice(0, i).join(':'), category, val, domain);
    }
    setCacheEntry('default', category, val, domain);
  };

  for (let i = 0; i < citation.citationItems.length; i++) {
    const id = citation.citationItems[i].id;
    const item = styleEngine.sys.retrieveItem(id);
    if (item.jurisdiction) throw new Error('fixture item with a jurisdiction: abbreviation files not available');
    const jurisdictions = [];
    if (item.language) {
      const lst = item.language.toLowerCase().split('<');
      if (lst.length > 0) item['language-name'] = lst[0];
      if (lst.length === 2) item['language-name-original'] = lst[1];
    }
    const domain = CSL.getAbbrevsDomain(styleEngine, jurisdictions[0], item.language);
    for (const field of Object.keys(item)) {
      let category = CSL.FIELD_CATEGORY_REMAP[field];
      let rawvals = false;
      if (category) {
        rawvals = rawFieldFunction[category](item, field).map((val) => [val, category, field]);
        if (field === 'jurisdiction') {
          rawvals = rawvals.concat(rawFieldFunction[category](item, field).map((val) => [val.split(':')[0], category, 'country']));
        }
      } else if (CSL.CREATORS.indexOf(field) > -1) {
        rawvals = rawFieldFunction['institution-entire'](item, field).map((val) => [val, 'institution-entire', field]);
        rawvals = rawvals.concat(rawFieldFunction['institution-part'](item, field).map((val) => [val, 'institution-part', field]));
      } else if (field === 'authority') {
        const spoof = 'string' === typeof item[field] ? { authority: [{ literal: item[field] }] } : item;
        rawvals = rawFieldFunction['institution-entire'](spoof, field).map((val) => [val, 'institution-entire', field]);
        rawvals = rawvals.concat(rawFieldFunction['institution-part'](spoof, field).map((val) => [val, 'institution-part', field]));
      }
      if (!rawvals) continue;
      for (let j = 0; j < rawvals.length; j++) {
        category = rawvals[j][1];
        registerEntries(rawvals[j][0], jurisdictions, category, rawvals[j][2], domain);
      }
    }
    for (const functionType in rawItemFunction) {
      const rawvals = rawItemFunction[functionType](item);
      for (let k = 0; k < rawvals.length; k++) registerEntries(rawvals[k], [], functionType);
    }
  }
}

// lib/sys.js
function Sys(test) {
  this.test = test;
  this._acache = { default: new CSL.AbbreviationSegments() };
  this._cache = {};
  this._ids = [];
  this._setCache();
  if (this.test.OPTIONS) for (const option in this.test.OPTIONS) this[option] = this.test.OPTIONS[option];
}
Sys.prototype._setCache = function () {
  this._cache = {};
  this._ids = [];
  for (const item of this.test.INPUT) {
    this._cache[item.id] = item;
    this._ids.push(item.id);
  }
};
Sys.prototype.retrieveItem = function (id) {
  return this._cache[id];
};
Sys.prototype.retrieveLocale = function (lang) {
  try {
    return fs.readFileSync(path.join(localeDir, 'locales-' + lang + '.xml')).toString().replace(/\s*<\?[^>]*\?>\s*\n/g, '');
  } catch (e) {
    return false;
  }
};
Sys.prototype.retrieveStyleModule = function () {
  return null;
};
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
  key = key
    .toString()
    .replace(/(?:\b|^)(?:and|et|y|und|l[ae]|the|[ld]')(?:\b|$)|[\x21-\x2C.\/\x3A-\x40\x5B-\x60\\\x7B\x7D-\x7E]/gi, '')
    .replace(/\s*\x7C\s*/g, '\x7C')
    .replace(/\./g, ' ')
    .replace(/\s+/g, ' ')
    .trim();
  return key.toLowerCase();
};
Sys.prototype.updateDoc = function () {
  let data, result;
  for (let i = 0; i < this.test.CITATIONS.length; i++) {
    const citation = this.test.CITATIONS[i];
    [data, result] = this.style.processCitationCluster(citation[0], citation[1], citation[2]);
    for (let j = this.doc.length - 1; j > -1; j--) {
      const citationID = this.doc[j].citationID;
      if (!this.style.registry.citationreg.citationById[citationID]) {
        this.doc = this.doc.slice(0, j).concat(this.doc.slice(j + 1));
      }
    }
    const prePost = citation[1].concat(citation[2]);
    const posMap = {};
    for (let j = 0; j < prePost.length; j++) posMap[prePost[j][0]] = j;
    this.doc.sort(function (a, b) {
      if (posMap[a.citationID] > posMap[b.citationID]) return 1;
      if (posMap[a.citationID] < posMap[b.citationID]) return -1;
      return 0;
    });
    for (const j in this.doc) this.doc[j].prefix = '..';
    for (const j in result) {
      const insert = result[j];
      for (const k in this.doc) {
        const cite = this.doc[k];
        if (cite.citationID === insert[2]) {
          this.doc[k] = { prefix: '>>', citationID: cite.citationID, String: insert[1] };
          result[j] = null;
          break;
        }
      }
    }
    for (const j in result) {
      const insert = result[j];
      if (!insert) continue;
      this.doc = this.doc
        .slice(0, insert[0])
        .concat([{ prefix: '>>', citationID: insert[2], String: insert[1] }])
        .concat(this.doc.slice(insert[0]));
    }
  }
};

Sys.prototype.run = function () {
  const self = this;
  let ret = [];
  function variableWrapper(params, prePunct, str, postPunct) {
    if (params.variableNames[0] === 'title' && params.itemData.URL && params.context === 'citation' && params.position === 'first') {
      return prePunct + '<a href="' + params.itemData.URL + '">' + str + '</a>' + postPunct;
    } else if (params.variableNames[0] === 'first-reference-note-number' && params.context === 'citation' && params.position !== 'first') {
      return prePunct + '<b>' + str + '</b>' + postPunct;
    }
    return prePunct + str + postPunct;
  }
  if (this.test.OPTIONS && this.test.OPTIONS.variableWrapper) this.variableWrapper = variableWrapper;
  const langBasesNeeded = {};
  for (const lang in CSL.LANGS) langBasesNeeded[lang.split('-')[0]] = true;
  for (const base in langBasesNeeded) {
    if (!CSL.LANG_BASES[base]) throw 'ERROR: missing in CSL.LANG_BASES: ' + base;
  }
  CSL.debug = function () {};
  this.style = new CSL.Engine(this, this.test.CSL);
  this.style.fun.dateparser.addDateParserMonths(['ocak', 'Şubat', 'mart', 'nisan', 'mayıs', 'haziran', 'temmuz', 'ağustos', 'eylül', 'ekim', 'kasım', 'aralık', 'bahar', 'yaz', 'sonbahar', 'kış']);

  if (!this.test.MODE) this.test.MODE = 'all';
  const mode = this.test.MODE.split('-');
  this.test.submode = {};
  for (let i = 1; i < mode.length; i++) this.test.submode[mode[i]] = true;
  this.test.MODE = mode[0];
  if (this.test.MODE === 'all') throw new Error('MODE all is not supported by this harness');

  for (const fmt of ['rtf', 'plain', 'asciidoc', 'xslfo']) {
    if (this.test.submode[fmt]) this.style.setOutputFormat(fmt);
  }
  if (this.test.submode.suppress_trailing_punctuation) this.style.citation.opt.suppressTrailingPunctuation = true;
  for (const opt in this.test.OPTIONS) {
    if (opt === 'variableWrapper') continue;
    this.style.opt.development_extensions[opt] = this.test.OPTIONS[opt];
  }
  const langParams = {
    persons: ['translit'], institutions: ['translit'], titles: ['translit', 'translat'],
    journals: ['translit'], publishers: ['translat'], places: ['translat'],
  };
  if (this.test.LANGPARAMS) {
    for (const key in this.test.LANGPARAMS) {
      if (key === 'langs') {
        const langsToUse = this.test.LANGPARAMS[key];
        if (langsToUse.translat) this.style.setLangTagsForCslTranslation(langsToUse.translat);
        // As in the runner: it passes .translat here too (sys.js).
        if (langsToUse.translit) this.style.setLangTagsForCslTransliteration(langsToUse.translat);
        continue;
      }
      langParams[key] = this.test.LANGPARAMS[key];
    }
  }
  this.style.setLangPrefsForCites(langParams);
  if (this.test.MULTIAFFIX) this.style.setLangPrefsForCiteAffixes(this.test.MULTIAFFIX);
  if (this.test.ABBREVIATIONS) {
    const abbrevs = {};
    for (const jurisd in this.test.ABBREVIATIONS) {
      abbrevs[jurisd] = {};
      for (const segment in this.test.ABBREVIATIONS[jurisd]) {
        abbrevs[jurisd][segment] = {};
        for (const key in this.test.ABBREVIATIONS[jurisd][segment]) {
          const isJurisdiction = jurisd === 'default' && segment === 'place' && key.toUpperCase() === key;
          const isCourt = ['institution-entire', 'institution-part'].indexOf(segment) > -1 && segment.toLowerCase() === segment;
          const normkey = !isJurisdiction && !isCourt ? this.normalizeAbbrevsKey('title', key) : key;
          abbrevs[jurisd][segment][normkey] = this.test.ABBREVIATIONS[jurisd][segment][key];
        }
      }
    }
    this._acache = Object.assign(this._acache, abbrevs);
  }
  let citation;
  if (this.test.BIBENTRIES) {
    for (let i = 0; i < this.test.BIBENTRIES.length; i++) {
      this.style.updateItems(this.test.BIBENTRIES[i], this.test.submode.nosort);
    }
  } else if (!this.test.CITATIONS) {
    this.style.updateItems(this._ids, this.test.submode.nosort);
  }
  if (!this.test['CITATION-ITEMS'] && !this.test.CITATIONS) {
    citation = [];
    for (let i = 0; i < this.style.registry.reflist.length; i++) citation.push({ id: this.style.registry.reflist[i].id });
    this.test['CITATION-ITEMS'] = [citation];
  }
  if (!this.test.ABBREVIATIONS) {
    if (this.test['CITATION-ITEMS']) {
      for (const citationItems of this.test['CITATION-ITEMS']) preloadAbbreviations(this.style, { citationItems }, this._acache);
    } else if (this.test.CITATIONS) {
      for (const mycitation of this.test.CITATIONS) preloadAbbreviations(this.style, mycitation[0], this._acache);
    }
  }

  const citations = [];
  if (this.test['CITATION-ITEMS']) {
    for (let i = 0; i < this.test['CITATION-ITEMS'].length; i++) {
      citations.push(this.style.makeCitationCluster(this.test['CITATION-ITEMS'][i]));
    }
  } else if (this.test.CITATIONS) {
    this.doc = [];
    this.updateDoc();
    if (this.test.INPUT2) {
      this.test.INPUT = this.test.INPUT2;
      this._setCache();
      this.updateDoc();
    }
    for (let idx = 0; idx < this.doc.length; idx++) citations.push(this.doc[idx].prefix + '[' + idx + '] ' + this.doc[idx].String);
  }
  ret = citations.join('\n');
  if (this.test.MODE === 'bibliography' && !this.test.submode.header) {
    const b = this.test.BIBSECTION ? this.style.makeBibliography(this.test.BIBSECTION) : this.style.makeBibliography();
    ret = b[0].bibstart + b[1].join('') + b[0].bibend;
  } else if (this.test.MODE === 'bibliography' && this.test.submode.header) {
    const o = this.style.makeBibliography()[0];
    const lst = [];
    for (const key in o) lst.push([key, o[key]]);
    lst.sort(function (a, b) {
      if (a > b) return 1;
      if (a < b) return -1;
      return 0;
    });
    ret = '';
    for (let pos = 0; pos < lst.length; pos++) ret += lst[pos][0] + ': ' + lst[pos][1] + '\n';
    ret = ret.replace(/^\s+/, '').replace(/\s+$/, '');
  }
  if (['citation', 'bibliography'].indexOf(this.test.MODE) === -1) throw 'Invalid mode in test file ' + this.test.NAME + ': ' + this.test.MODE;
  void self;
  return normalizeNewline(ret);
};

const names = fs.readdirSync(fixtureDir).filter((n) => /^[a-z]+_.*\.txt$/.test(n)).sort();
const fixtures = {};
let failed = 0;
let threw = 0;
const byArea = {};
for (const fn of names) {
  const tn = fn.replace(/\.txt$/, '');
  const area = tn.split('_')[0];
  const test = parseFixture(tn, path.join(fixtureDir, fn));
  const rec = { mode: test.MODE };
  let output = null;
  try {
    output = new Sys(test).run();
  } catch (err) {
    rec.error = String(err && err.stack ? err.stack.split('\n')[0] : err);
    threw++;
  }
  rec.output = output;
  rec.expected = test.RESULT;
  rec.passes = output === test.RESULT;
  fixtures[tn] = rec;
  if (!byArea[area]) byArea[area] = { total: 0, failed: 0 };
  byArea[area].total++;
  if (!rec.passes) {
    failed++;
    byArea[area].failed++;
  }
}

const pkg = JSON.parse(fs.readFileSync(path.join(path.dirname(process.env.CITEPROC_MODULE), 'package.json'), 'utf8'));
const result = {
  meta: {
    engine: 'citeproc-js ' + pkg.version,
    engine_source: 'juris-m/citeproc-js 73bc1b44bc7d54d0bfec4e070fd27f5efe024ff9 (src/ builds a citeproc_commonjs.js byte-identical to npm citeproc ' + pkg.version + ')',
    test_suite: 'citation-style-language/test-suite 6eefc5b07c6969ab8999e48542acbcc131cba864, processor-tests/humans',
    runner: 'juris-m/citeproc-test-runner 1.1.116 (b1e72d5cb1363b7f4abbe1f6546c9e2c443db726), replicated in scripts/csl-testsuite-reference.cjs',
    locales: 'citation-style-language/locales 6b0cb4689127a69852f48608b6d1a879900f418b (the submodule citeproc-js pins)',
    node: process.version,
    fixtures: names.length,
    citeproc_js_fails: failed,
    citeproc_js_threw: threw,
    citeproc_js_fails_by_area: Object.keys(byArea).sort().filter((a) => byArea[a].failed).reduce((o, a) => { o[a] = byArea[a].failed + ' of ' + byArea[a].total; return o; }, {}),
  },
  fixtures,
};
fs.writeFileSync(outFile, JSON.stringify(result, null, 1) + '\n');
console.log('csl-testsuite-reference: ' + names.length + ' fixtures, citeproc-js fails ' + failed + ' (' + threw + ' threw)');
for (const a of Object.keys(byArea).sort()) if (byArea[a].failed) console.log('  ' + a + ': ' + byArea[a].failed + ' of ' + byArea[a].total);
