// Prints Rust source for the load.js constant tables the input-side modules
// use (pasted, marked "DUP-CHECK", into the Rust files that need them).
// Usage: CITEPROC_MODULE=... node scripts/csl-units/gen_tables.cjs <outdir>
'use strict';
const fs = require('fs');
const path = require('path');
const CSL = require(process.env.CITEPROC_MODULE);
const out = process.argv[2];
const q = (s) => JSON.stringify(s).replace(/\\u([0-9a-f]{4})/gi, (m, h) => '\\u{' + h + '}');
function pairs(name, doc, obj) {
  const keys = Object.keys(obj);
  let s = `/// ${doc}\nconst ${name}: [(&str, &str); ${keys.length}] = [\n`;
  for (const k of keys) s += `    (${q(k)}, ${q(obj[k])}),\n`;
  return s + '];\n';
}
function list(name, doc, arr) {
  let s = `/// ${doc}\nconst ${name}: [&str; ${arr.length}] = [\n`;
  for (const k of arr) s += `    ${q(k)},\n`;
  return s + '];\n';
}
fs.mkdirSync(out, { recursive: true });
fs.writeFileSync(path.join(out, 'statute.rs'),
  '// DUP-CHECK: load.js CSL.STATUTE_SUBDIV_STRINGS\n' + pairs('STATUTE_SUBDIV_STRINGS', '`CSL.STATUTE_SUBDIV_STRINGS` (abbreviated label to term).', CSL.STATUTE_SUBDIV_STRINGS) +
  '// DUP-CHECK: load.js CSL.STATUTE_SUBDIV_STRINGS_REVERSE\n' + pairs('STATUTE_SUBDIV_STRINGS_REVERSE', '`CSL.STATUTE_SUBDIV_STRINGS_REVERSE` (term to abbreviated label).', CSL.STATUTE_SUBDIV_STRINGS_REVERSE) +
  '// DUP-CHECK: load.js CSL.LOCATOR_LABELS_MAP\n' + pairs('LOCATOR_LABELS_MAP', '`CSL.LOCATOR_LABELS_MAP`.', CSL.LOCATOR_LABELS_MAP));
fs.writeFileSync(path.join(out, 'lang_prefs.rs'),
  '// DUP-CHECK: load.js CSL.LangPrefsMap\n' + pairs('LANG_PREFS_MAP', '`CSL.LangPrefsMap` (variable to language-role).', CSL.LangPrefsMap));
fs.writeFileSync(path.join(out, 'vars.rs'),
  '// DUP-CHECK: load.js CSL.NAME_VARIABLES\n' + list('NAME_VARIABLES', '`CSL.NAME_VARIABLES`.', CSL.NAME_VARIABLES) +
  '// DUP-CHECK: load.js CSL.NUMERIC_VARIABLES\n' + list('NUMERIC_VARIABLES', '`CSL.NUMERIC_VARIABLES`.', CSL.NUMERIC_VARIABLES) +
  '// DUP-CHECK: load.js CSL.DATE_VARIABLES\n' + list('DATE_VARIABLES', '`CSL.DATE_VARIABLES`.', CSL.DATE_VARIABLES));
// particle list
let s = '// DUP-CHECK: none (CSL.ParticleList lives only in util_name_particles.js)\n';
s += '/// `CSL.ParticleList`: `[particle, [[dropping_from,dropping_to] | None, [non_dropping_from, non_dropping_to] | None] ...]`, flattened as\n/// `(particle, &[(drop, nondrop)])` where each range is `Some((from, to))`.\n';
s += 'type Range = Option<(u8, u8)>;\n';
s += `const PARTICLE_LIST: [(&str, &[(Range, Range)]); ${CSL.ParticleList.length}] = [\n`;
const r = (x) => (x === null ? 'None' : `Some((${x[0]}, ${x[1]}))`);
for (const [p, alts] of CSL.ParticleList) s += `    (${q(p)}, &[${alts.map((a) => `(${r(a[0])}, ${r(a[1])})`).join(', ')}]),\n`;
s += '];\n';
fs.writeFileSync(path.join(out, 'particles.rs'), s);
fs.writeFileSync(path.join(out, 'roman.rs'),
  '// DUP-CHECK: load.js CSL.ROMAN_NUMERALS\n/// `CSL.ROMAN_NUMERALS`: per decimal position, the numeral for digit 0..=9 (position 3 has 0..=5).\nconst ROMAN_NUMERALS: [&[&str]; 4] = [\n' +
  CSL.ROMAN_NUMERALS.map((row) => `    &[${row.map(q).join(', ')}],\n`).join('') + '];\n');
console.log('ok', Object.keys(CSL.STATUTE_SUBDIV_STRINGS).length);
