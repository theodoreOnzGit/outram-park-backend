// Reference data for the text-case formatters (formatters.js, GitHub #793).
// Output: crates/kovan-literature/tests/data/csl/units/formatters.json
//   [{ s: input, la: tmp.lang_array or null, r: [lowercase, uppercase,
//      capitalize-first, capitalize-all, sentence, title] }]
'use strict';
const { CSL, makeEngine, fixtureStrings, writeUnits, prng } = require('./common_output.cjs');

const NAMES = ['lowercase', 'uppercase', 'capitalize-first', 'capitalize-all', 'sentence', 'title'];

const edge = [
  '', ' ', 'a', 'A', 'I', 'the', 'The', 'of the', 'hello world', 'HELLO WORLD', 'Hello World',
  'iPhone', 'eBay', "McDonald's", "O'Neill", "don't", "rock 'n' roll", '‘single’ quotes',
  '“double” quotes', 'He said "hi" to her', "He said 'hi' to her", 'a: the b', 'A.B.C.',
  'vs. v. versus', 'to be or not to be', 'state-of-the-art', 'well-known', 'x-ray', '-hyphen',
  'trailing-', 'em—dash', 'en–dash', 'slash/word', 'the (parenthetical) word',
  '[bracket] word', '<i>italic</i> words', '<b>bold</b> and plain', '<sc>small caps</sc> text',
  'H<sub>2</sub>O', 'E=mc<sup>2</sup>', '<span class="nocase">NoCase</span> word',
  '<span class="nodecor">x</span> y', '<span style="font-variant: small-caps;">sc</span> word',
  '<span style="font-variant:small-caps;">sc</span> word',
  'nested <i>ital <b>bold</b></i> end', 'a <span class="nocase">b</span> and the end',
  'Ünïcödé wörds', 'ÉCOLE', 'straße', 'ǆ', 'ΑΒΓ',
  'α', 'Σίσυφος', 'σ', 'İstanbul', 'ıstanbul',
  'istanbul ISTANBUL Iigi', '日本語 text', 'Привет мир',
  'العربية', '𐐨𐐩 deseret', ' leading space',
  'trailing space ', 'double  space', 'tab\there', 'newline\nhere', 'NBSP word',
  'word! Another? Yes: sure', 'Title: subtitle', 'A title? and more', 'up-to-date', "d'Artagnan",
  "l'amour de la vie", 'von Neumann', 'the van Gogh', 'Mr. Smith goes to Washington', 'a b',
  'The a', 'in', 'At the end of', 'a "quoted phrase" in the middle', 'a "quoted phrase',
  'a quoted phrase" tail', "a 'single quoted phrase' in the middle", '"Start quote" first',
  '“nested ‘inner’ quote” here', '“orphan open here', 'orphan close” here',
  '‘x', 'y’', 'on the road: a tale', 'what? the end', 'wow! is it', 'to', 'the end of the',
  'Nation of "Positive Obligations " of State under the European Convention on Human Rights',
  'ALL CAPS TITLE WITH THE STOP WORDS OF A KIND', 'mixed CAPS and lower', 'camelCaseWord and PascalCase',
  '3D printing of the 21st century', '1984', 'a-b c-d', 'the-end', 'of-the-', 'self-', '-the',
  'an apple a day', 'AN APPLE', 'a.b', 'q & a', 'Q&A', 'smith et al.', 'et al', 'e.g. this', 'ca. 1900',
  'x – y', 'x - y', 'x — y', 'in vitro', 'IN VITRO', 'via the web', 'vis-à-vis',
  'the “” empty', "'tis the season", "the 'tis", 'rock ’n’ roll', "o'clock",
  'under the &amp; entity', 'less < than', 'tags <unknown>x</unknown> here',
];

const words = [
  'the', 'The', 'a', 'an', 'of', 'in', 'on', 'and', 'or', 'but', 'for', 'to', 'by', 'at', 'from', 'with',
  'war', 'Peace', 'WAR', 'PEACE', 'iPad', 'McKay', "O'Brien", "l'été", 'café', 'Über', 'naïve',
  'state-of-the-art', 'e-mail', 'x', 'Y', '1', '42nd', 'v.', 'vs.', 'etc.', 'Dr.', 'U.S.A.', 'α-helix',
  'Δ', 'Zusammenhang', 'der', 'von', 'van', 'de', "d'", 'al', 'et', 'ca', 'c', 'down', 'up', 'out', 'over',
];
const seps = [' ', ' ', ' ', ' ', ': ', '; ', ', ', '? ', '! ', ' - ', ' – ', ' — ', ' / ', '. '];
const opens = ['', '', '', '', '"', "'", '“', '‘', '(', '[', '<i>', '<b>', '<sc>', '<sub>', '<sup>',
  '<span class="nocase">', '<span class="nodecor">', '<span style="font-variant: small-caps;">'];
const closeOf = { '': '', '"': '"', "'": "'", '“': '”', '‘': '’', '(': ')', '[': ']',
  '<i>': '</i>', '<b>': '</b>', '<sc>': '</sc>', '<sub>': '</sub>', '<sup>': '</sup>',
  '<span class="nocase">': '</span>', '<span class="nodecor">': '</span>',
  '<span style="font-variant: small-caps;">': '</span>' };

function gen(rand, n) {
  const pick = (a) => a[Math.floor(rand() * a.length)];
  const out = [];
  for (let i = 0; i < n; i++) {
    const nw = 1 + Math.floor(rand() * 9);
    let s = '';
    let stack = [];
    for (let j = 0; j < nw; j++) {
      if (j > 0) s += pick(seps);
      if (rand() < 0.25) { const o = pick(opens); s += o; stack.push(closeOf[o]); }
      s += pick(words);
      if (stack.length && rand() < 0.35) s += stack.pop();
    }
    // sometimes leave tags unbalanced on purpose
    if (rand() < 0.9) while (stack.length) s += stack.pop();
    out.push(s);
  }
  return out;
}

const rand = prng(793);
const strings = [];
const seen = new Set();
for (const s of [].concat(edge, fixtureStrings(), gen(rand, 1800))) {
  if (!seen.has(s)) { seen.add(s); strings.push(s); }
}

const cases = [];
const engines = {
  null: makeEngine(),
  tr: makeEngine(),
  bad: makeEngine(),
  en: makeEngine(),
};
engines.tr.tmp.lang_array = ['tr-TR'];
engines.bad.tmp.lang_array = ['en_US'];
engines.en.tmp.lang_array = ['en-US', 'fr-FR'];
function run(e, s) {
  return NAMES.map((n) => {
    try { return CSL.Output.Formatters[n](e, s); } catch (err) { return { error: String(err) }; }
  });
}
for (const s of strings) cases.push({ s, la: null, r: run(engines.null, s) });
// language-sensitive casing on a subset
const sub = strings.filter((s) => /[iIİı]/.test(s)).slice(0, 150);
for (const s of sub) {
  cases.push({ s, la: ['tr-TR'], r: run(engines.tr, s) });
  cases.push({ s, la: ['en_US'], r: run(engines.bad, s) });
  cases.push({ s, la: ['en-US', 'fr-FR'], r: run(engines.en, s) });
}
// nameDoppel.split (used by names output): tags/strings/origStrings
const nameStrings = strings.filter((x, i) => i % 3 === 0 || /[-<]/.test(x)).slice(0, 700).concat(['Jean-Paul', 'van der Berg-Smith', 'A <i>B</i> C', 'X - Y', 'a--b', '-', '- -', ' - ', '<span class="nocase">x</span> y', 'spans class="nocase">']);
const doppel = nameStrings.map((x) => {
  const d = CSL.Output.Formatters.nameDoppel.split(x);
  return { s: x, tags: d.tags, strings: d.strings, orig: d.origStrings === undefined ? null : d.origStrings, joined: CSL.Output.Formatters.nameDoppel.join(d) };
});
writeUnits('formatters', { names: NAMES, cases, doppel });
console.error('formatters cases:', cases.length);
