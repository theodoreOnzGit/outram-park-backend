// Differential reference for src/citeproc/util_dateparser.rs (+ dateParseArray of util_date.js).
// Output: tests/data/csl/units/dateparser.json
'use strict';
const { freshCSL, fixtureItems, write, plain } = require('./common.cjs');

const DATE_VARS = ['locator-date', 'issued', 'event-date', 'accessed', 'original-date', 'publication-date',
  'available-date', 'submitted', 'alt-issued', 'alt-event'];

// Every raw date string that occurs in the fixtures' INPUT.
const strings = new Set();
const dateObjs = [];
for (const { item } of fixtureItems()) {
  for (const k of Object.keys(item)) {
    if (DATE_VARS.indexOf(k.replace(/^alt-/, '')) > -1 && item[k]) {
      if (typeof item[k].raw === 'string') strings.add(item[k].raw);
      dateObjs.push(item[k]);
    }
  }
}

// Generated variants.
const months = ['January', 'Feb', 'march', 'Apr.', 'May', 'June', 'july', 'Aug', 'September', 'Oct', 'nov', 'December',
  'Sept', 'Jun', 'Dec.', 'janv.', 'mai', 'Mar', 'ocak', 'Şubat', 'mart', 'Nisan', 'bahar', 'Spring', 'Summer', 'Fall', 'Winter', 'Autumn'];
const gen = [
  '', ' ', '2012', '12', '1', '123', '0', '0000', '-2012', '-12', '2012-05', '2012-05-03', '2012/05/03', '05/03/2012',
  '5/3/2012', '03.05.2012', '2012-5-3', '2012-13-45', '2012-00-00', '12/25/2012', '25/12/2012', '2012-12', '12-2012',
  '12/2012', '2012/12', '5/2012', '2012-05-03T10:20:30Z', '2012-05-03T10:20:30', '2012-05-03 10:20:30', '2012-05-03 10:20',
  '"Summer 2012"', '"', '""', '"x"', '2012?', '?2012', '~2012', 'c2012', 'c. 2012', 'circa 2012', 'ca. 2012', 'cir 2012',
  '2012~', '1999-2001', '1999/2001', '1999-2001-05', '2012-05-03/2012-06-04', '2012-05-03-2012-06-04', '2012/05/03-2012/06/04',
  '2012-05-03_2012-06-04', '2012_2013', '2012-05/2012-06', '2012-05-03 - 2012-06-04', '2012 - 2013', '2012 – 2013',
  '5 BC', '50 BC', '2012 BC', '44 B.C.', '44 b.c.', '100 AD', '100 A.D.', '100 ad', 'AD 100', 'BC 44', '2012 AD', '33BC',
  '3 May 2012', 'May 3, 2012', 'May 2012', 'May', 'May 3', '3 May', 'Spring 2012', 'Summer, 2012', '2012 Winter', 'Fall 2012',
  'Autumn 2012', 'Michaelmas 2012', 'Trinity 2012', 'Hilary 2012', 'Easter 2012', 'Easter 3 May 2012', 'May 3-5, 2012',
  'May 3 - June 4, 2012', '3-5 May 2012', 'March-April 2012', 'Mar-Apr 2012', 'June 2012 - July 2013', '2012-06 - 2013-07',
  '1 January 2012 - 2 February 2013', 'xyz', 'xyz 2012', '2012 xyz', 'foo bar baz', '12345', '123456', '2012-05-03-04',
  '2012--05', '2012-', '-', '/', '--', '2012/', '/2012', '99999', '0001', '0012-05-03', '2012-05-00', '00-05-2012',
  '5-3', '5/3', '13/5', '5/13', '25-12', '12-25', '2012-0-3', '2012-3-0',
  '平成24年5月3日', '平成24年5月', '平成24年', '昭和30年1月1日', '大正3年12月25日', '明治45年7月30日', '平成元年1月8日',
  '2012年5月3日', '2012年5月', '2012年', '2012年5月3日〜2012年6月4日', '平成24年5月3日〜平成24年6月4日', '平成24年5月3日-6月4日',
  '2012年5月3日 〜 6月4日', '5月3日', '平成24.5.3', '2012. 5. 3', '2012.5.3.', 'a.d. 100', 'Jan. 3, 2012', 'Jan.3,2012',
  '3.5.2012', '2012.05.03', '1st May 2012', 'May 1st, 2012', 'Mayor 2012', 'maybe 2012', 'mar 2012', 'marzo 2012', 'MARCH 2012',
  '2012-05-03T', 'T2012', '2012-05-03Tx', '1 Jan', 'Jan 1', 'Jan 1 - 5', '1-5 Jan', 'Winter 2012-13', 'Winter 2012-2013',
  '2012 spring', 'spring', '2012-21', '2012-22', '2012-23', '2012-24', ' 2012', '2012 ', ' 2012 ', '2012\n', '\n2012',
  '2012-05-03\n2012-06-04', '10:20:30', 'x 10:20:30 2012', '2012 10:20', '2012 10:20:30',
];
for (const m of months) {
  gen.push(m, m + ' 2012', '2012 ' + m, m + ' 3, 2012', '3 ' + m + ' 2012', m + ' 2012 - ' + m + ' 2013', m + '-' + m + ' 2012', m + ' 2012 BC');
}
for (let y = 1; y < 40; y += 3) gen.push(String(y), String(y * 111), y + ' BC', y + '-' + (y + 1), y + '/' + (y + 1));
for (const s of gen) strings.add(s);

const MONTH_LISTS = {
  turkish: ['ocak', 'Şubat', 'mart', 'nisan', 'mayıs', 'haziran', 'temmuz', 'ağustos', 'eylül', 'ekim', 'kasım', 'aralık', 'bahar', 'yaz', 'sonbahar', 'kış'],
  french: ['janvier', 'février', 'mars', 'avril', 'mai', 'juin', 'juillet', 'août', 'septembre', 'octobre', 'novembre', 'décembre'],
  french_short: ['janv.', 'févr.', 'mars', 'avr.', 'mai', 'juin', 'juil.', 'août', 'sept.', 'oct.', 'nov.', 'déc.'],
  german: ['Januar', 'Februar', 'März', 'April', 'Mai', 'Juni', 'Juli', 'August', 'September', 'Oktober', 'November', 'Dezember', 'Frühling', 'Sommer', 'Herbst', 'Winter'],
  spanish: ['enero', 'febrero', 'marzo', 'abril', 'mayo', 'junio', 'julio', 'agosto', 'septiembre', 'octubre', 'noviembre', 'diciembre'],
  english_again: ['january', 'february', 'march', 'april', 'may', 'june', 'july', 'august', 'september', 'october', 'november', 'december'],
  short12: ['jan', 'feb', 'mar', 'apr', 'may', 'jun', 'jul', 'aug', 'sep', 'oct', 'nov', 'dec'],
  wrong_len: ['a', 'b', 'c'],
  weird: ['ma', 'mar', 'mara', 'abr', 'ju', 'jul', 'juli', 'au', 'sep', 'ok', 'no', 'de'],
};

const all = Array.from(strings);
const out = { stages: [], date_parse_array: [], numeric_order: [] };

function stage(parser, label, added) {
  const rec = { label, added, month_rexes: parser.monthRexes.map((r) => r.source), month_abbrevs: plain(parser.monthAbbrevs), cases: [] };
  for (const s of all) {
    const c = { in: s };
    for (const [k, f] of [['obj', 'parseDateToObject'], ['arr', 'parseDateToArray'], ['str', 'parseDateToString']]) {
      try {
        c[k] = plain(parser[f](s));
      } catch (e) {
        c[k + '_error'] = String(e.message);
      }
    }
    rec.cases.push(c);
  }
  out.stages.push(rec);
}

// Stage 0: a fresh parser; later stages add month lists cumulatively (the
// Rust test replays the same sequence on one DateParser).
{
  const CSL = freshCSL();
  const p = CSL.DateParser;
  stage(p, 'default', null);
  for (const name of ['turkish', 'turkish', 'french', 'french_short', 'german', 'spanish', 'english_again', 'short12', 'wrong_len', 'weird']) {
    p.addDateParserMonths(MONTH_LISTS[name]);
    stage(p, 'after ' + name, name);
  }
  p.addDateParserMonths('janu febr marc apri mayy june july augu sept octo nove dece');
  stage(p, 'after string list', 'string');
  // day-month order
  p.setOrderDayMonth();
  out.numeric_order.push({ order: 'dm', cases: all.map((s) => ({ in: s, obj: plain(p.parseDateToObject(s)) })) });
  p.setOrderMonthDay();
  p.resetDateParserMonths();
  stage(p, 'after reset', 'reset');
}
out.month_lists = MONTH_LISTS;

// dateParseArray over every date object in the fixtures plus odd ones.
{
  const CSL = freshCSL();
  const objs = dateObjs.slice();
  objs.push({ 'date-parts': [[2012, 5, 3]] }, { 'date-parts': [[2012, 5, 3], [2013, 6, 4]] }, { 'date-parts': [[2012], [2013]] },
    { 'date-parts': [['2012', '05', '03']] }, { 'date-parts': [[2012, 5]] }, { 'date-parts': [[]] }, { 'date-parts': [] },
    { literal: 'x' }, { literal: { part: 'weird' } }, { raw: '2012', 'date-parts': [[2012]] }, { season: 'Spring', 'date-parts': [[2012]] },
    { 'date-parts': [[2012, 'x', 3]] }, { 'date-parts': [[-5]] }, { 'date-parts': [[2012.7, 5.2]] }, { circa: true, 'date-parts': [[2012]] },
    { 'date-parts': [[2012, 5, 3], [2013]] });
  for (const o of objs) {
    const c = { in: plain(o) };
    try {
      c.out = plain(CSL.Engine.prototype.dateParseArray.call({}, JSON.parse(JSON.stringify(o))));
    } catch (e) {
      c.error = String(e.message);
    }
    out.date_parse_array.push(c);
  }
}
write('dateparser', out);
