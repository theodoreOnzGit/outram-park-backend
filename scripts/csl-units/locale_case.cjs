// Differential reference for the citeproc port's locale casing (GitHub #804).
//
// Writes crates/kovan-literature/tests/data/csl/units/locale_case.json: for a
// generated set of strings (Greek letters with every diacritic, precomposed
// and combining, polytonic forms, final sigma, Lithuanian i/j/i-ogonek with
// combining marks, other-language samples) and a set of lang arrays, what
// node returns for `s.toLocaleUpperCase(arr)` / `toLocaleLowerCase(arr)`
// with load.js's try/catch fallback to toUpperCase/toLowerCase.
// Layout: {strings, rows:[{langs, diffs:[[index, upper, lower], ...]}]}; a string
// not in a row's diffs maps to plain toUpperCase()/toLowerCase().
//
// Run by hand: node scripts/csl-units/locale_case.cjs
'use strict';
const fs = require('fs');
const path = require('path');
const outFile = path.join(__dirname, '..', '..', 'crates/kovan-literature/tests/data/csl/units/locale_case.json');

const strings = new Set();
let rnd0 = 0;
const comb = ['̀','́','̂','̃','̄','̆','̇','̈','̓','̔','͂','̓','̈́','ͅ','̣','̨'];
const greekLower = []; for (let c = 0x3b1; c <= 0x3c9; c++) greekLower.push(String.fromCharCode(c));
const greekUpper = []; for (let c = 0x391; c <= 0x3a9; c++) greekUpper.push(String.fromCharCode(c));
const greekAll = greekLower.concat(greekUpper);
// every code point in the Greek blocks
for (let c = 0x370; c <= 0x3ff; c++) strings.add(String.fromCodePoint(c));
for (let c = 0x1f00; c <= 0x1fff; c++) strings.add(String.fromCodePoint(c));
const bases = greekAll.concat(['i','j','I','J','į','Į','a','A','Ì','Í','Ĩ','ì','í','ĩ']);
for (const b of bases) {
  strings.add(b);
  for (const c of comb) {
    strings.add(b + c);
    strings.add(b + c + 'x');
    strings.add('x' + b + c);
    if (rnd0++ % 3 === 0) for (const d of comb) strings.add(b + c + d);
  }
}
// vowel + (accented) iota/upsilon sequences, both orders, with a following context
const vowels = ['α','ε','η','ι','ο','υ','ω','ά','έ','ή','ί','ό','ύ','ώ','Ά','Έ','Ή','Ί','Ό','Ύ','Ώ','Α','Ε','Ι','Υ'];
const ius = ['ι','υ','ί','ύ','ϊ','ϋ','ΐ','ΰ','Ι','Υ','Ί','Ύ'];
for (const v of vowels) for (const i of ius) { strings.add(v + i); strings.add(v + i + ' '); strings.add(' ' + v + i + 'x'); strings.add(v + '́' + i); strings.add(v + i + '́'); strings.add(v + '͂' + i + '̈'); }
const words = ['άλφα Μάιος','Οδυσσέας','Χριστός','ΑΘΗΝΑ','αιμα','γιος','πού','πού είναι','μάϊος','ΑΣΣ ΑΣ Σ ΣΣ','ΣΑΣ', 'ΑΣ', 'ΑΣ.', 'Σ', 'ΑΣΑ', 'ΟΔΥΣΣΕΑΣ ΑΣ ΒΑΣΙΛΕΥΣ', 'Ὀδυσσεύς', 'ᾍδης', 'ᾳ', 'ᾼ', 'ᾀ', 'ᾈ', 'ῼ', 'ῃ', 'ῌ', 'ᾲ', 'ᾴ', 'ῂ', 'ῄ', 'ῲ', 'ῴ', 'ᾷ', 'ῇ', 'ῷ', 'ΐ', 'ΰ', 'ῒ', 'ῢ', 'ῖ', 'ῦ', 'ῗ', 'ῧ',
  'Straße','STRASSE','ǆ','ǅ','ǈ','ﬁ','ﬃ','ŉ','ǰ','ΐ','ß','İ','ı','İstanbul','ISPARTA','istanbul','İ','i̇','Ijsselmeer','IJsselmeer','ijsselmeer','ĳ','Ĳ','hello World','HELLO WORLD','The Quick Brown Fox','éàü','ÉÀÜ','Ǆ','ǅ','ǉ','Я я Ж ж','Ա ա','日本語','العربية','xͅ','i̇́','Ìx','Í','Ĩ','i̇̀','j̇','Ì','Í','Ĩ','Į̀','Į́','Į̃','J́','ì','í','ĩ','į́','į̇','Į̇','Ị̇', 'iı', 'Iİ', 'ABCabc', 'abc123 def_ghi', 'ὈΔΥΣΣΕΎΣ', 'ΜΆΙΟΣ', 'μάιος', 'ΜΑΪΟΣ'];
for (const w of words) strings.add(w);
// random mixtures
let seed = 12345;
const rnd = (n) => { seed = (seed * 1103515245 + 12345) & 0x7fffffff; return seed % n; };
const pool = greekAll.concat(comb, ['i','j','I','J','į','Ì','Í','Ĩ',' ','.','a','B','ß','ς','Σ','σ','İ','ı']);
for (let k = 0; k < 1500; k++) { let s = ''; const n = 1 + rnd(7); for (let q = 0; q < n; q++) s += pool[rnd(pool.length)]; strings.add(s); }
const list = [...strings];

const langs = [['el'],['el-GR'],['el-polyton'],['lt'],['lt-LT'],['tr'],['tr-TR'],['az'],['en-US'],['el','en-US'],['en-US','el'],['lt','en-US'],['en-US','lt'],['xx-invalid'],['el','xx_invalid'],['xx_invalid','el'],['en-US','xx_invalid'],[],['de'],['nl'],['EL'],['Lt'],['el','tr'],['tr','el'],['lt','el'],['zz'],['x'],['ell'],['gre'],['tur'],['aze'],['lit-LT'],['el-u-ca'],['el-x-foo'],['el-GR-GR'],['el--GR'],['el_GR'],['en-a-b-a-c'],['lt-x'],[''],['und'],['root'],['el','xx_invalid','tr'],['tr','xx_invalid']];
function up(s, a) { try { return s.toLocaleUpperCase(a); } catch (e) { return s.toUpperCase(); } }
function lo(s, a) { try { return s.toLocaleLowerCase(a); } catch (e) { return s.toLowerCase(); } }
const rootUp = list.map((s) => s.toUpperCase());
const rootLo = list.map((s) => s.toLowerCase());
// Per lang array: only the strings whose result differs from the root (plain
// toUpperCase/toLowerCase) one, as [index, upper, lower].
const rows = langs.map((a) => {
  const diffs = [];
  list.forEach((s, i) => {
    const u = up(s, a), l = lo(s, a);
    if (u !== rootUp[i] || l !== rootLo[i]) diffs.push([i, u, l]);
  });
  return { langs: a, diffs };
});
fs.writeFileSync(outFile, JSON.stringify({ node: process.version, icu: process.versions.icu, unicode: process.versions.unicode, strings: list, rows }));
console.log(list.length, 'strings x', langs.length, 'lang arrays');
