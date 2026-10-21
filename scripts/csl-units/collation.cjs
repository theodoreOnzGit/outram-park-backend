// Differential reference for the citeproc port's collator (GitHub #795).
//
// Writes crates/kovan-literature/tests/data/csl/units/collation.json: for a
// fixed list of strings (every string value of the CSL test-suite fixtures'
// INPUT items and of the site's items.json, plus generated case, diacritic,
// punctuation, number and script variants) and a fixed list of pairs, what
// node's ICU says for `a.localeCompare(b, locale)` ("default") and for
// `a.localeCompare(b, locale, {sensitivity:"base", ignorePunctuation:true,
// numeric:true})` ("sort", the options of citeproc-js's sort.js
// getSortCompare), under every default-locale the suite uses. Results are
// one character per pair: '<', '=', '>'.
//
// Run by hand: node scripts/csl-units/collation.cjs
'use strict';
const fs = require('fs');
const path = require('path');

const root = path.join(__dirname, '..', '..');
const fixtureDir = path.join(root, 'vendor/csl-test-suite/processor-tests/humans');
const itemsFile = path.join(root, 'crates/kovan-literature/tests/data/csl/items.json');
const outFile = path.join(root, 'crates/kovan-literature/tests/data/csl/units/collation.json');

const strings = new Set();
function walk(v) {
  if (typeof v === 'string') { if (v.length < 200) strings.add(v); }
  else if (Array.isArray(v)) v.forEach(walk);
  else if (v && typeof v === 'object') Object.values(v).forEach(walk);
}
for (const f of fs.readdirSync(fixtureDir).filter((n) => /^[a-z]+_.*\.txt$/.test(n)).sort()) {
  const text = fs.readFileSync(path.join(fixtureDir, f), 'utf8');
  const m = text.match(/>>=+ INPUT =+>>\n([\s\S]*?)<<=+ INPUT =+<</);
  if (!m) continue;
  try { walk(JSON.parse(m[1])); } catch (e) { /* a few fixtures carry comments */ }
}
try { walk(JSON.parse(fs.readFileSync(itemsFile, 'utf8'))); } catch (e) { /* optional */ }
const base = [...strings].sort();

// Sort keys as citeproc-js builds them: words joined with "|", ending in "|".
const keyLike = [];
for (const s of base.slice(0, 600)) keyLike.push(s.split(' ').join('|') + '|', s.toLowerCase().split(' ').join('|') + '|');

const gen = new Set();
const sample = base.filter((s) => s.length > 1 && s.length < 40);
for (let i = 0; i < sample.length; i += Math.max(1, Math.floor(sample.length / 400))) {
  const s = sample[i];
  gen.add(s.toUpperCase()); gen.add(s.toLowerCase());
  gen.add(s.normalize('NFD')); gen.add(s.normalize('NFC'));
  gen.add(s[0].toLowerCase() + s.slice(1)); gen.add(s[0].toUpperCase() + s.slice(1));
  gen.add('[' + s); gen.add('"' + s); gen.add("'" + s); gen.add('(' + s + ')');
  gen.add(s + '.'); gen.add(s + '|'); gen.add(s.replace(/ /g, '-')); gen.add(s.replace(/ /g, '  '));
  gen.add(s + ' 2'); gen.add(s + ' 10'); gen.add(s + ' 02');
  gen.add(s.replace(/[aeiou]/, (c) => c + '́'));
}
const hand = [
  '', ' ', '|', '@', 'dale|', 'daleb', 'dale', '[x', 'x', '"x', "'x", '-x', '.x', '_x', '~x', 'x~',
  'a', 'A', 'á', 'Á', 'á', 'ä', 'å', 'æ', 'ae', 'Æ', 'ø', 'o', 'ö', 'oe', 'œ', 'ł', 'l', 'ñ', 'n', 'ç', 'c', 'č', 'ß', 'ss', 'ſ', 'ij', 'ĳ', 'ǆ', 'dž', 'ﬁ', 'fi',
  'th', 'þ', 'ð', 'd', 'ə', 'ı', 'i', 'İ', 'I', 'ğ', 'g', 'ş', 's', 'ü', 'u', 'Ü',
  '1', '2', '10', '02', '1.5', '1,5', 'a1', 'a2', 'a10', 'a02', 'a 2', 'a  2', '0', '00', '007',
  'α', 'Α', 'ά', 'β', 'ω', 'Ω', 'а', 'я', 'Я', 'ё', 'е', 'ж', 'ا', 'ب', 'ع', 'ก', 'ข', 'ក', 'ខ', 'ひ', 'ヒ', 'ひらがな', 'カタカナ', '漢字', '東京', '北京', '中国', '日本', '한국', '한', '가',
  'van der Berg', 'Van der Berg', 'vanderberg', 'de la Cruz', 'De La Cruz', "O'Brien", 'OBrien', "O’Brien", 'McDonald', 'Mc Donald', 'MacDonald',
  'Smith, John', 'Smith John', 'Smith-Jones', 'Smith Jones', 'Smith', 'smith', 'SMITH', 'Smithe', 'Smyth',
  'The Title', 'Title', 'A Title', 'An Title', 'title', 'Title:', 'Title,', 'Title.', 'Title ', ' Title', ' Title', 'Title​', 'Ti­tle',
  '\u{1F600}', 'a\u{1F600}', 'b', 'B', 'z', 'Z', 'zz', 'Zz', 'aa', 'Aa', 'AA', 'aA',
];
const all = [...new Set([...base, ...keyLike, ...gen, ...hand])];

// Deterministic PRNG (mulberry32), so the pair list is reproducible.
let seed = 0x5eed1234;
function rnd() { seed |= 0; seed = (seed + 0x6d2b79f5) | 0; let t = Math.imul(seed ^ (seed >>> 15), 1 | seed); t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t; return ((t ^ (t >>> 14)) >>> 0) / 4294967296; }

const sortedRoot = all.map((s, i) => [s, i]).sort((a, b) => a[0].localeCompare(b[0], 'en-US'));
const pairs = [];
const seen = new Set();
function addPair(i, j) { const k = i + ',' + j; if (!seen.has(k)) { seen.add(k); pairs.push([i, j]); } }
for (let k = 0; k + 1 < sortedRoot.length; k++) addPair(sortedRoot[k][1], sortedRoot[k + 1][1]);
const handIdx = hand.map((h) => all.indexOf(h));
for (const i of handIdx.slice(0, 90)) for (const j of handIdx.slice(0, 90)) if (i !== j) addPair(i, j);
while (pairs.length < 15000) addPair(Math.floor(rnd() * all.length), Math.floor(rnd() * all.length));

const locales = ['en-US', 'en-GB', 'en', 'fr-FR', 'fr', 'fr-CA', 'de-DE', 'da-DK', 'ro-RO', 'pt-BR', 'el', 'ar', 'zh-TW', 'km-KH', 'gx', 'en-US-x-sort-ja-alalc97'];
const sortOpts = { sensitivity: 'base', ignorePunctuation: true, numeric: true };
const sym = (n) => (n < 0 ? '<' : n > 0 ? '>' : '=');
const out = { meta: { node: process.version, icu: process.versions.icu, unicode: process.versions.unicode, cldr: process.versions.cldr, strings: all.length, pairs: pairs.length, locales }, strings: all, pairs: pairs.flat(), default: {}, sort: {} };
for (const loc of locales) {
  out.default[loc] = pairs.map(([i, j]) => sym(all[i].localeCompare(all[j], loc))).join('');
  out.sort[loc] = pairs.map(([i, j]) => sym(all[i].localeCompare(all[j], loc, sortOpts))).join('');
}
fs.mkdirSync(path.dirname(outFile), { recursive: true });
fs.writeFileSync(outFile, JSON.stringify(out) + '\n');
console.log('collation.json:', all.length, 'strings,', pairs.length, 'pairs,', locales.length, 'locales; ICU', process.versions.icu);
