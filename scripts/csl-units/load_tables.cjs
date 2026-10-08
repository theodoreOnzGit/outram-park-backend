// Prints the Rust constant tables of crates/kovan-literature/src/citeproc/load.rs
// from the bundled citeproc-js 2.4.63 (CITEPROC_MODULE), so none is typed by hand.
// Usage: CITEPROC_MODULE=.../citeproc_commonjs.js node scripts/csl-units/load_tables.cjs > /tmp/tables.rs
const CSL = require(process.env.CITEPROC_MODULE);
const q = (s) => JSON.stringify(s).replace(/\\u([0-9a-f]{4})/gi, (m, h) => '\\u{' + h + '}');
function strs(name, doc, arr) {
  console.log(`/// ${doc}`);
  console.log(`pub const ${name}: &[&str] = &[`);
  for (const s of arr) console.log(`    ${q(s)},`);
  console.log('];\n');
}
function pairs(name, doc, obj) {
  console.log(`/// ${doc}`);
  console.log(`pub const ${name}: &[(&str, &str)] = &[`);
  for (const k of Object.keys(obj)) console.log(`    (${q(k)}, ${q(obj[k])}),`);
  console.log('];\n');
}
pairs('STATUTE_SUBDIV_STRINGS', '`CSL.STATUTE_SUBDIV_STRINGS`.', CSL.STATUTE_SUBDIV_STRINGS);
pairs('STATUTE_SUBDIV_STRINGS_REVERSE', '`CSL.STATUTE_SUBDIV_STRINGS_REVERSE`.', CSL.STATUTE_SUBDIV_STRINGS_REVERSE);
pairs('LOCATOR_LABELS_MAP', '`CSL.LOCATOR_LABELS_MAP`.', CSL.LOCATOR_LABELS_MAP);
strs('MODULE_MACROS', '`CSL.MODULE_MACROS` (the keys; every value is `true`).', Object.keys(CSL.MODULE_MACROS));
strs('MODULE_TYPES', '`CSL.MODULE_TYPES` (the keys; every value is `true`).', Object.keys(CSL.MODULE_TYPES));
strs('MULTI_FIELDS', '`CSL.MULTI_FIELDS`.', CSL.MULTI_FIELDS);
pairs('LANG_PREFS_MAP', '`CSL.LangPrefsMap`.', CSL.LangPrefsMap);
pairs('FIELD_CATEGORY_REMAP', '`CSL.FIELD_CATEGORY_REMAP`.', CSL.FIELD_CATEGORY_REMAP);
strs('GENDERS', '`CSL.GENDERS`.', CSL.GENDERS);
strs('POSITION_TEST_VARS', '`CSL.POSITION_TEST_VARS`.', CSL.POSITION_TEST_VARS);
strs('AREAS', '`CSL.AREAS`.', CSL.AREAS);
strs('CITE_FIELDS', '`CSL.CITE_FIELDS`.', CSL.CITE_FIELDS);
strs('SWAPPING_PUNCTUATION', '`CSL.SWAPPING_PUNCTUATION`.', CSL.SWAPPING_PUNCTUATION);
strs('TERMINAL_PUNCTUATION', '`CSL.TERMINAL_PUNCTUATION`.', CSL.TERMINAL_PUNCTUATION);
strs('DATE_PARTS', '`CSL.DATE_PARTS`.', CSL.DATE_PARTS);
strs('DATE_PARTS_ALL', '`CSL.DATE_PARTS_ALL`.', CSL.DATE_PARTS_ALL);
strs('DATE_PARTS_INTERNAL', '`CSL.DATE_PARTS_INTERNAL`.', CSL.DATE_PARTS_INTERNAL);
strs('NAME_PARTS', '`CSL.NAME_PARTS`.', CSL.NAME_PARTS);
strs('DISAMBIGUATE_OPTIONS', '`CSL.DISAMBIGUATE_OPTIONS`.', CSL.DISAMBIGUATE_OPTIONS);
strs('GIVENNAME_DISAMBIGUATION_RULES', '`CSL.GIVENNAME_DISAMBIGUATION_RULES`.', CSL.GIVENNAME_DISAMBIGUATION_RULES);
strs('NAME_ATTRIBUTES', '`CSL.NAME_ATTRIBUTES`.', CSL.NAME_ATTRIBUTES);
strs('DISPLAY_CLASSES', '`CSL.DISPLAY_CLASSES`.', CSL.DISPLAY_CLASSES);
strs('NAME_VARIABLES', '`CSL.NAME_VARIABLES`.', CSL.NAME_VARIABLES);
strs('CREATORS', '`CSL.CREATORS`.', CSL.CREATORS);
strs('NUMERIC_VARIABLES', '`CSL.NUMERIC_VARIABLES`.', CSL.NUMERIC_VARIABLES);
strs('DATE_VARIABLES', '`CSL.DATE_VARIABLES`.', CSL.DATE_VARIABLES);
strs('VARIABLES_WITH_SHORT_FORM', '`CSL.VARIABLES_WITH_SHORT_FORM`.', CSL.VARIABLES_WITH_SHORT_FORM);
strs('SKIP_WORDS', '`CSL.SKIP_WORDS`: the default `skip-words` of a locale\'s `opts`.', CSL.SKIP_WORDS);
strs('FORMAT_KEY_SEQUENCE', '`CSL.FORMAT_KEY_SEQUENCE`: the order decorations are applied in.', CSL.FORMAT_KEY_SEQUENCE);
strs('INSTITUTION_KEYS', '`CSL.INSTITUTION_KEYS`.', CSL.INSTITUTION_KEYS);
strs('SYS_OPTIONS', '`CSL.SYS_OPTIONS`.', CSL.SYS_OPTIONS);
console.log('/// `CSL.ROMAN_NUMERALS`.');
console.log('pub const ROMAN_NUMERALS: [&[&str]; 4] = [');
for (const r of CSL.ROMAN_NUMERALS) console.log(`    &[${r.map(q).join(', ')}],`);
console.log('];\n');
pairs('LANGS', '`CSL.LANGS`.', CSL.LANGS);
pairs('LANG_BASES', '`CSL.LANG_BASES` (underscores as upstream has them).', CSL.LANG_BASES);
console.log('/// `CSL.SUPERSCRIPTS`: superscript character to its plain form.');
console.log('pub const SUPERSCRIPTS: &[(char, &str)] = &[');
for (const k of Object.keys(CSL.SUPERSCRIPTS)) {
  const cp = k.codePointAt(0).toString(16).toUpperCase().padStart(4, '0');
  console.log(`    ('\\u{${cp}}', ${q(CSL.SUPERSCRIPTS[k])}),`);
}
console.log('];');
