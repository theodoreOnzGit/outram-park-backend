// Reference data for the flip-flopper (util_flipflop.js, GitHub #793).
// Output: crates/kovan-literature/tests/data/csl/units/flipflop.json
//   [{ s: input text, d: blob decorations [[a,b]...] | null,
//      l: layout_decorations | null, out: serialised blob tree }]
// All cases run in order on ONE engine, because the flip-flopper keeps
// state between calls (the outer quote form is mutated and stays mutated).
'use strict';
const { CSL, makeEngine, fixtureStrings, ser, writeUnits, prng } = require('./common.cjs');

const edge = [
  '', 'plain', ' leading', 'trailing ', '<i>it</i>', 'a <i>it</i> b', '<i>a <i>b</i> c</i>', '<b>b</b>',
  '<i><b>x</b></i>', '<sc>sc</sc>', '<sup>1</sup>', '<sub>2</sub>', 'H<sub>2</sub>O', 'x<sup>2</sup>y',
  '<sup>a<sub>b</sub></sup>', '<sub>a<sup>b</sup></sub>', '<span class="nocase">NC</span>',
  '<span class="nodecor">ND</span>', '<span style="font-variant:small-caps;">S</span>',
  '<span style="font-variant: small-caps;">S</span>', '<span  class="nocase" >x</span>',
  '<span class="nocase">a <i>b</i> c</span> d', 'a <span class="nocase">b</span> c',
  '<i>unclosed', 'unopened</i>', '</i>', '<i>', '<b><i>mis</b></i>', '<i><b>x</i></b>',
  '"quoted"', "'quoted'", 'a "quoted" b', "a 'quoted' b", 'a "b \'c\' d" e', "a 'b \"c\" d' e",
  '"a" and "b"', "'a' and 'b'", 'a "b', 'a b"', "a 'b", "a b'", "it's", "'tis", "rock 'n' roll",
  "don't stop", "the '90s", "O'Neill's", "l'amour", "'a", "a'", "'a'", "''", '""', '"', "'", ' "', " '",
  '("a")', "('a')", '("a \'b\' c")', ' "a"', ' \'a\'', '"a"b', 'a"b"c', "a'b'c", "a'b' c",
  '“double”', '‘single’', 'a “b ‘c’ d” e', 'a ‘b “c” d’ e',
  'it’s', 'rock ’n’ roll', '’tis', 'a’b’c', 'a ’ b', '’',
  '<i>"quoted italic"</i>', '"<i>italic quoted</i>"', '"a <i>b" c</i>', '<i>a "b</i> c"',
  '<i>it\'s</i>', "<i>'x'</i>", '"x" <b>y</b> \'z\'', ' "<i>a</i>"', "x \"y\" 'z' \"w\"",
  '"a" "b" "c"', "'a' 'b' 'c'", '" "', "' '", 'a  "b"', ' "  "', '"",', '"a",', "'a'.", 'x "a." y',
  '&amp; <unknown>', '< i >x</ i >', 'a<br>b', '<i >x</i>', '<I>x</I>',
  'multi\nline "q"', ' "nbsp"', '\t"tab"', "café's \"menu\"", '日本 "x" 語',
  "'x' <span class=\"nocase\">'y'</span>", '<span class="nocase">"unclosed</span> "', '<span class="nocase">x',
  'x</span>', '<sc><i>x</i></sc>', '<span class="nodecor"><i>x</i></span>', '<i><span class="nodecor">x</span></i>',
  '<b><span class="nodecor">x</span></b> <i>y</i>', '<span class="nodecor">a</span> <span class="nodecor">b</span>',
];

const rand = prng(7931);
const pick = (a) => a[Math.floor(rand() * a.length)];
const atoms = [
  'word', 'Word', 'x', 'it', "it's", "don't", 'café', 'the', 'of', '1', '2', 'A.B.', 'etc.', 'a', 'b',
];
const tags = ['<i>', '</i>', '<b>', '</b>', '<sc>', '</sc>', '<sup>', '</sup>', '<sub>', '</sub>',
  '<span class="nocase">', '</span>', '<span class="nodecor">', '"', "'", ' "', " '", '“', '”',
  '‘', '’', '("', "('", '<span style="font-variant:small-caps;">'];
function gen(n) {
  const out = [];
  for (let i = 0; i < n; i++) {
    let s = '';
    const k = 1 + Math.floor(rand() * 8);
    for (let j = 0; j < k; j++) {
      const r = rand();
      if (r < 0.45) s += pick(atoms);
      else if (r < 0.55) s += ' ';
      else s += pick(tags);
      if (rand() < 0.3) s += ' ';
    }
    out.push(s);
  }
  return out;
}

const contexts = [
  { d: null, l: null },
  { d: [['@font-style', 'italic']], l: null },
  { d: [['@font-weight', 'bold']], l: null },
  { d: [['@font-variant', 'small-caps']], l: null },
  { d: [['@vertical-align', 'sup']], l: null },
  { d: [['@quotes', 'true']], l: null },
  { d: [['@font-style', 'italic'], ['@quotes', 'true']], l: null },
  { d: null, l: [['@font-style', 'italic']] },
  { d: null, l: [['@font-weight', 'bold'], ['@font-variant', 'small-caps']] },
  { d: [['@font-style', 'normal']], l: [['@font-style', 'italic']] },
  { d: [['@font-style', 'oblique']], l: null },
];

const seen = new Set();
const strings = [];
for (const s of [].concat(edge, fixtureStrings(), gen(1500))) {
  if (!seen.has(s)) { seen.add(s); strings.push(s); }
}

const e = makeEngine();
const empty = e.output.formats.value().empty;
const cases = [];
let ci = 0;
for (const s of strings) {
  // every string with the plain context; a rotating other context for the rest
  const ctxs = [contexts[0], contexts[1 + (ci % (contexts.length - 1))]];
  ci++;
  for (const c of ctxs) {
    const blob = new CSL.Blob(s, empty);
    if (c.d) {
      blob.decorations = c.d.map((x) => x.slice());
      blob.alldecor = [blob.decorations];
    }
    e.citation.opt.layout_decorations = c.l ? c.l.map((x) => x.slice()) : undefined;
    let out;
    try {
      e.fun.flipflopper.processTags(blob);
      out = ser(blob, { alldecor: true });
    } catch (err) {
      out = { error: String(err) };
    }
    cases.push({ s, d: c.d, l: c.l, out });
  }
}
writeUnits('flipflop', { cases });
console.error('flipflop cases:', cases.length);
