// Differential reference for src/citeproc/util_date.rs (dateAsSortKey / dateMacroAsSortKey).
// Output: tests/data/csl/units/datekey.json
'use strict';
const { freshCSL, fixtureItems, write, plain } = require('./common.cjs');
const CSL = freshCSL();

const dates = [
  undefined, { 'date-parts': [[2012, 5, 3]] }, { 'date-parts': [[2012, 5]] }, { 'date-parts': [[2012]] }, { 'date-parts': [[-44, 3, 15]] },
  { 'date-parts': [[-5]] }, { 'date-parts': [[2012, 5, 3], [2013, 6, 4]] }, { 'date-parts': [[2012], [2013]] }, { 'date-parts': [[0]] },
  { 'date-parts': [[2012, 13, 32]] }, { 'date-parts': [['2012', '05', '03']] }, { raw: '2012-05-03' }, { raw: 'May 3, 2012' }, { raw: '2012/2013' },
  { raw: 'nonsense' }, { raw: '44 BC' }, { literal: 'Summer 2012' }, { year: 2012, month: 5, day: 3 }, { year: 2012, month: 5, day: 3, year_end: 2013, month_end: 6, day_end: 4 },
  { year: -44, month: 3, day: 15 }, { 'date-parts': [[2012, 16, 3]] }, { 'date-parts': [[1, 1, 1]] }, { 'date-parts': [[999, 9, 9]] }, { 'date-parts': [[12345, 1, 1]] },
  { season: 'Spring', 'date-parts': [[2012]] }, { circa: true, 'date-parts': [[2012, 5]] },
];
const states = [];
const cases = [];
for (const raw of dates) {
  for (const dateparts of [undefined, ['year', 'month', 'day'], ['year'], ['year', 'month'], ['month', 'day'], ['day'], []]) {
    for (const isMacro of [false, true]) {
      for (const ext of [false, true]) {
        const item = {};
        let dp = raw === undefined ? undefined : JSON.parse(JSON.stringify(raw));
        // like retrieveItem: date-parts objects are flattened by dateParseArray before sorting
        for (const flat of [false, true]) {
          let it = {};
          if (dp !== undefined) it.issued = flat && dp['date-parts'] ? CSL.Engine.prototype.dateParseArray.call({}, JSON.parse(JSON.stringify(dp))) : JSON.parse(JSON.stringify(dp));
          const appended = [];
          const state = { tmp: { extension: ext }, output: { append: (s, f) => appended.push([s, f]) }, fun: { dateparser: CSL.DateParser }, dateParseArray: CSL.Engine.prototype.dateParseArray };
          const token = { variables: ['issued'] };
          if (dateparts) token.dateparts = dateparts.slice();
          const c = { item: plain(it), dateparts: dateparts || null, isMacro, ext };
          try {
            if (isMacro) CSL.dateMacroAsSortKey.call(token, state, it); else CSL.dateAsSortKey.call(token, state, it);
            c.out = appended;
            c.token_dateparts = token.dateparts;
          } catch (err) {
            c.error = String(err.message);
          }
          cases.push(c);
        }
      }
    }
  }
}
write('datekey', { cases });
