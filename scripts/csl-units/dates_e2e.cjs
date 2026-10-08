// Differential reference for the date rendering of the citeproc-js port
// (src/citeproc/node_date.rs, node_datepart.rs, util_dates.rs): GitHub #793.
// Output: tests/data/csl/units/dates_e2e.json
//
// METHOD. A style whose citation layout holds only cs:date nodes is built by
// citeproc-js for each locale; each item's date is rendered by running the
// token list the way CSL.getCite does, except that every token but the date nodes
// (and the helper tokens of their `variable` attribute) is skipped: the layout
// closures belong to node_layout.js, not to the date code,
// then the output queue is finished as makeCitationCluster does (purgeEmptyBlobs,
// adjust.*, output.string) and the resulting array is serialised to JSON.
// The Rust test (src/citeproc/date_e2e_test.rs) does the same steps on the port.
// An engine that throws on an item is rebuilt before the next item (both sides).
//
// Usage: CITEPROC_MODULE=<citeproc_commonjs.js> node scripts/csl-units/dates_e2e.cjs
'use strict';
const fs = require('fs');
const path = require('path');
const { freshCSL, write, root } = require('./common.cjs');
const CSL = freshCSL();

const LANGS = ['en-US', 'de-DE', 'fr-FR', 'ja-JP', 'zh-CN'];
const localeDir = path.join(root, 'vendor/citeproc-js/locale');

// ---------------------------------------------------------------- items
const A = { 'date-parts': [[2010, 5, 6]] };
const O = { 'date-parts': [[1999, 1, 2], [1999, 3, 4]] };
const dates = {
  full: { 'date-parts': [[2003, 8, 10]] },
  ym: { 'date-parts': [[2003, 8]] },
  y: { 'date-parts': [[2003]] },
  r_day: { 'date-parts': [[2003, 8, 10], [2003, 8, 23]] },
  r_month: { 'date-parts': [[2003, 8, 10], [2003, 9, 23]] },
  r_year: { 'date-parts': [[2003, 8, 10], [2005, 9, 23]] },
  r_ym_same: { 'date-parts': [[2003, 8], [2003, 11]] },
  r_ym_diff: { 'date-parts': [[2003, 8], [2005, 2]] },
  r_y: { 'date-parts': [[2003], [2005]] },
  r_y_same: { 'date-parts': [[2003], [2003]] },
  r_y_open: { 'date-parts': [[2003], [0]] },
  r_y_century: { 'date-parts': [[1998], [2003]] },
  r_y_decade: { 'date-parts': [[1991], [1999]] },
  r_full_same: { 'date-parts': [[2003, 8, 10], [2003, 8, 10]] },
  r_md_same_day: { 'date-parts': [[2003, 8, 10], [2004, 8, 10]] },
  season1: { 'date-parts': [[2005]], season: '1' },
  season2: { 'date-parts': [[2005]], season: 2 },
  season4: { 'date-parts': [[2005]], season: '4' },
  season_text: { 'date-parts': [[2005]], season: 'Spring' },
  season_range: { 'date-parts': [[2005, 13], [2005, 14]] },
  season_range2: { 'date-parts': [[2005, 13], [2006, 15]] },
  month13: { 'date-parts': [[2005, 13]] },
  month16: { 'date-parts': [[2005, 16]] },
  month17: { 'date-parts': [[2005, 17]] },
  month24: { 'date-parts': [[2005, 24]] },
  month25: { 'date-parts': [[2005, 25]] },
  month0: { 'date-parts': [[2005, 0, 4]] },
  circa: { 'date-parts': [[1900]], circa: true },
  circa_full: { 'date-parts': [[1900, 4, 5]], circa: true },
  bc: { 'date-parts': [[-44, 3, 15]] },
  bc_y: { 'date-parts': [[-500]] },
  bc_range: { 'date-parts': [[-44], [-40]] },
  bc_ad_range: { 'date-parts': [[-44], [10]] },
  bc_range_full: { 'date-parts': [[-44, 3, 15], [-43, 4, 16]] },
  ad100: { 'date-parts': [[100, 1, 1]] },
  ad499: { 'date-parts': [[499]] },
  ad500: { 'date-parts': [[500]] },
  ad5: { 'date-parts': [[5]] },
  ad_range: { 'date-parts': [[100], [200]] },
  ad_range2: { 'date-parts': [[100], [600]] },
  year0: { 'date-parts': [[0]] },
  year12: { 'date-parts': [[12, 3]] },
  literal: { literal: 'Early 1900s' },
  literal_parts: { literal: 'Easter 2003', 'date-parts': [[2003, 4, 20]] },
  raw_iso: { raw: '2003-08-10' },
  raw_iso_range: { raw: '2003-08-10/2003-08-23' },
  raw_ym_y: { raw: '2001-02/2003' },
  raw_text: { raw: 'May 3, 2012' },
  raw_season: { raw: 'Spring 2005' },
  raw_circa: { raw: 'c. 1900' },
  raw_circa2: { raw: '1900?' },
  raw_junk: { raw: 'nonsense' },
  str_parts: { 'date-parts': [['2003', '08', '10']] },
  empty_parts: { 'date-parts': [[]] },
  empty_obj: {},
  day_only_range: { 'date-parts': [[2003, 8, 1], [2003, 8, 31]] },
  year_end_only: { 'date-parts': [[2003, 8, 10], [2003]] },
};
const items = [];
for (const [k, d] of Object.entries(dates)) {
  items.push({ id: 'd_' + k, type: 'book', issued: d, accessed: A, 'original-date': O });
}
items.push({ id: 'no_date', type: 'book', accessed: A });
items.push({ id: 'no_date_at_all', type: 'book' });
items.push({ id: 'legal1', type: 'legal_case', 'collection-number': '2003', issued: dates.y, country: 'us' });
items.push({ id: 'legal2', type: 'legal_case', 'collection-number': '2003', issued: dates.full });
items.push({ id: 'legis1', type: 'legislation', issued: dates.y });

// --------------------------------------------------------------- styles
const NS = 'xmlns="http://purl.org/net/xbiblio/csl"';
function style(layout, root) {
  return `<style ${NS} class="note" version="1.0"${root || ''}>
  <info><id/><title/><updated>2009-08-10T04:49:00+09:00</updated></info>
  <citation><layout>${layout}</layout></citation>
</style>`;
}
const styles = {};
const add = (id, layout, root) => { styles[id] = style(layout, root); };

// Explicit date-part children (no form).
const monthForms = ['long', 'short', 'numeric', 'numeric-leading-zeros'];
const dayForms = ['numeric', 'numeric-leading-zeros', 'ordinal'];
const yearForms = ['long', 'short'];
for (const mf of monthForms) {
  for (const df of dayForms) {
    const yf = yearForms[(monthForms.indexOf(mf) + dayForms.indexOf(df)) % 2];
    add(`ex_${mf}_${df}_${yf}`,
      `<date variable="issued"><date-part name="day" form="${df}" suffix=" "/><date-part name="month" form="${mf}" suffix=" "/><date-part name="year" form="${yf}"/></date>`);
  }
}
add('ex_ymd_delim', '<date variable="issued" delimiter="-"><date-part name="year"/><date-part name="month" form="numeric-leading-zeros"/><date-part name="day" form="numeric-leading-zeros"/></date>');
add('ex_affix', '<date variable="issued" prefix="(" suffix=")"><date-part name="month" prefix="[" suffix="] "/><date-part name="day" suffix=", "/><date-part name="year" prefix="&lt;" suffix="&gt;"/></date>');
add('ex_textcase', '<date variable="issued"><date-part name="month" text-case="uppercase" suffix=" "/><date-part name="day" suffix=" "/><date-part name="year"/></date>');
add('ex_textcase2', '<date variable="issued"><date-part name="month" form="short" text-case="capitalize-first" suffix=" "/><date-part name="year" text-case="uppercase"/></date>');
add('ex_strip', '<date variable="issued"><date-part name="month" form="short" strip-periods="true" suffix=" "/><date-part name="year"/></date>');
add('ex_rangedelim', '<date variable="issued"><date-part name="month" suffix=" " range-delimiter="~"/><date-part name="day" suffix=", " range-delimiter="~"/><date-part name="year" range-delimiter="~"/></date>');
add('ex_rangedelim2', '<date variable="issued"><date-part name="year" range-delimiter=" to "/></date>');
add('ex_weight', '<date variable="issued"><date-part name="month" font-weight="bold" suffix=" "/><date-part name="year" font-style="italic"/></date>');
add('ex_quotes', '<date variable="issued"><date-part name="year" quotes="true"/></date>');
add('ex_year_only', '<date variable="issued"><date-part name="year"/></date>');
add('ex_month_only', '<date variable="issued"><date-part name="month"/></date>');
add('ex_day_only', '<date variable="issued"><date-part name="day"/></date>');
add('ex_year_month', '<date variable="issued"><date-part name="year" suffix="-"/><date-part name="month" form="numeric-leading-zeros"/></date>');
add('ex_month_year', '<date variable="issued"><date-part name="month" suffix=" "/><date-part name="year"/></date>');
add('ex_year_imperial', '<date variable="issued"><date-part name="year" form="imperial"/></date>');
add('ex_two_dates', '<date variable="issued" suffix=" / "><date-part name="year"/></date><date variable="original-date"><date-part name="month" suffix=" "/><date-part name="day" suffix=" "/><date-part name="year"/></date>');
add('ex_accessed', '<date variable="accessed"><date-part name="month" suffix=" "/><date-part name="day" suffix=", "/><date-part name="year"/></date>');
add('ex_accessed_issued', '<date variable="accessed" suffix="; "><date-part name="year"/></date><date variable="issued"><date-part name="year"/></date>');
add('ex_nonexistent', '<date variable="issued"><date-part name="year" suffix="!"/></date><date variable="nosuchdate"><date-part name="year"/></date>');
add('ex_empty_date', '<date variable="issued"/>');

// Localised forms.
for (const form of ['text', 'numeric']) {
  for (const dp of [null, 'year', 'year-month', 'year-month-day', 'month-day']) {
    const attr = dp ? ` date-parts="${dp}"` : '';
    add(`loc_${form}_${dp || 'default'}`, `<date variable="issued" form="${form}"${attr}/>`);
    add(`loc_${form}_${dp || 'default'}_affix`, `<date variable="issued" form="${form}"${attr} prefix="(" suffix=")"/>`);
  }
}
add('loc_text_monthform', '<date variable="issued" form="text"><date-part name="month" form="short"/></date>');
add('loc_text_monthform2', '<date variable="issued" form="text"><date-part name="month" form="numeric" suffix="/"/></date>');
add('loc_text_dayform', '<date variable="issued" form="text"><date-part name="day" form="ordinal"/></date>');
add('loc_text_textcase', '<date variable="issued" form="text"><date-part name="month" text-case="uppercase"/><date-part name="year" text-case="uppercase"/></date>');
add('loc_text_strip', '<date variable="issued" form="text"><date-part name="month" form="short" strip-periods="true"/></date>');
add('loc_text_rangedelim', '<date variable="issued" form="text"><date-part name="year" range-delimiter="~"/></date>');
add('loc_text_yearshort', '<date variable="issued" form="text"><date-part name="year" form="short"/></date>');
add('loc_numeric_two', '<date variable="issued" form="numeric" suffix=" | "/><date variable="original-date" form="text"/>');
add('loc_text_bold', '<date variable="issued" form="text" font-weight="bold"/>');
add('loc_text_textcase_date', '<date variable="issued" form="text" text-case="uppercase"/>');
add('loc_text_default_locale', '<date variable="issued" form="text" default-locale="true"/>');

// Year range formats (style-level option).
for (const f of ['expanded', 'minimal', 'minimal-two', 'chicago', 'chicago-16']) {
  add(`yrf_${f}`, '<date variable="issued"><date-part name="year"/></date>', ` year-range-format="${f}"`);
  add(`yrf_${f}_text`, '<date variable="issued" form="text" date-parts="year"/>', ` year-range-format="${f}"`);
  add(`yrf_${f}_ym`, '<date variable="issued"><date-part name="month" suffix=" "/><date-part name="year"/></date>', ` year-range-format="${f}"`);
}
// Year-suffix collapse option (read by date-part; the registry is not involved
// for the cite alone).
add('collapse_ys', '<date variable="issued"><date-part name="year"/></date>', ' ');

// ---------------------------------------------------------------- run
function readLocale(lang) {
  const f = path.join(localeDir, `locales-${lang}.xml`);
  const xml = fs.readFileSync(f, 'utf8');
  return xml.replace(/\s*<\?[^>]*\?>\s*\n/g, '');
}
const localeCache = {};
function makeSys() {
  const byId = {};
  for (const it of items) byId[it.id] = JSON.parse(JSON.stringify(it));
  return {
    retrieveLocale(lang) {
      if (!(lang in localeCache)) {
        try { localeCache[lang] = readLocale(lang); } catch (e) { localeCache[lang] = false; }
      }
      return localeCache[lang];
    },
    retrieveItem(id) { return byId[id]; },
    variableWrapper: undefined,
  };
}

function renderOne(state, id) {
  const Item = state.retrieveItem(id);
  state.tmp.area = 'citation';
  state.tmp.root = 'citation';
  state.tmp.years_used = [];
  state.tmp.done_vars = [];
  state.tmp.have_collapsed = false;
  state.tmp.has_done_year_suffix = false;
  state.tmp.cite_renders_content = false;
  state.tmp.probably_rendered_something = false;
  const toks = state.citation.tokens;
  // The @variable check-for-output closure (attributes.js) only maintains
  // group-context flags; the port's copy is another agent's, so both sides drop it.
  for (const t of toks) {
    t.execs = t.execs.filter((fn) => !/dateparts\.indexOf\(key\)/.test(String(fn)));
  }
  // The layout closure opens a level for the cite; its tokens are skipped here.
  state.output.openLevel("empty");
  let next = 0;
  while (next < toks.length) {
    const t = toks[next];
    if (!(t.name === 'date' || t.name === 'date-part' || (t.name === 'text' && t.variables_real !== undefined))) {
      next = t.next;
    } else {
      next = CSL.tokenExec.call(state, t, Item, undefined);
    }
  }
  state.output.closeLevel();
  for (let i = 0; i < state.output.queue.length; i++) CSL.Output.Queue.purgeEmptyBlobs(state.output.queue[i]);
  if (state.opt.development_extensions.clean_up_csl_flaws) {
    for (let j = 0; j < state.output.queue.length; j++) {
      state.output.adjust.upward(state.output.queue[j]);
      state.output.adjust.leftward(state.output.queue[j]);
      state.output.adjust.downward(state.output.queue[j]);
      state.output.adjust.fix(state.output.queue[j]);
    }
  }
  return state.output.string(state, state.output.queue);
}

const results = {};
const errors = [];
for (const [sid, xml] of Object.entries(styles)) {
  for (const lang of LANGS) {
    let state = null;
    const build = () => { state = new CSL.Engine(makeSys(), xml, lang); };
    try { build(); } catch (e) {
      results[`${sid}|${lang}`] = { build_error: String(e && e.message || e) };
      continue;
    }
    const outs = {};
    for (const it of items) {
      try {
        outs[it.id] = JSON.parse(JSON.stringify(renderOne(state, it.id)));
      } catch (e) {
        outs[it.id] = { error: true };
        errors.push(`${sid}|${lang}|${it.id}: ${e && e.message || e}`);
        build();
      }
    }
    results[`${sid}|${lang}`] = outs;
  }
}
console.log('styles', Object.keys(styles).length, 'items', items.length, 'langs', LANGS.length, 'throws', errors.length);
if (process.env.SHOW_ERRORS) console.log(errors.slice(0, 4000).join('\n'));
write('dates_e2e', { langs: LANGS, styles, items, results });
