// Differential reference for src/citeproc/util_name_particles.rs (parseParticles) and
// the input side of util_names_render.rs (_normalizeNameInput, _parseName, getStaticOrder).
// Output: tests/data/csl/units/names.json
'use strict';
const { freshCSL, fixtureItems, write, plain } = require('./common.cjs');
const CSL = freshCSL();

const names = [];
const seen = new Set();
function add(n) {
  const k = JSON.stringify(n);
  if (!seen.has(k)) { seen.add(k); names.push(n); }
}
// every name in the fixtures
let nFixture = 0;
for (const { item } of fixtureItems()) {
  for (const v of CSL.NAME_VARIABLES) {
    if (Array.isArray(item[v])) for (const n of item[v]) { add(n); nFixture++; }
  }
}
const families = ['Doe', 'von Doe', 'van der Berg', 'de la Cruz', 'd\'Artagnan', 'D\'Artagnan', 'al-Rashid', 'Al-Rashid', 'O\'Neil', '"Doe Jones"', '"Doe"', '"', 'Smith-Jones', 'McDonald',
  '\'t Hooft', 'ten Brink', 'von und zu Liechtenstein', 'van\'t Hoff', 'of Wessex', 'ibn Saud', 'bin Laden', 'du Bois', 'DuBois', '', ' ', '山田', 'Нгуен', 'Nguyễn Văn', 'Đặng', 'Lê', 'Kovács', 'de', 'van', 'von de la',
  'd’Estaing', 'de l’Orme', 'dell\'Acqua', 'Mc Donald', 'van de Velde-Smith', 'op de Beeck', 'in \'t Veld', 'zu', 'La Fontaine', 'la Fontaine', 'ʻOkala', 'a b c d', 'x  y', 'Smith Jr.', 'Ünal', 'çelik', 'öztürk', 'Åberg', 'de Åberg'];
const givens = ['John', 'John von', 'John Q.', 'von', 'Jean-Pierre de', 'Charles, Jr.', 'Charles, Jr', 'Charles,Jr.', 'John, III', 'John, et al.', 'John, et al', 'A. B.', 'de la', 'd’', 'd\' ', 'van der', '', ' ',
  '太郎', 'Văn An', 'Jr.', 'John,!', 'John, !Jr.', 'Hans von der', 'Jean de la Fontaine', 'Ludwig van', 'ʻAbdu', 'ibn', 'Ahmad ibn', 'John de', 'x y z', 'John  von', 'Gerrit \'t', 'Jan "Bob"', 'Q', 'Ünal', 'Ünal de'];
for (const f of families) for (const g of givens) add({ family: f, given: g });
for (const f of families.slice(0, 25)) {
  add({ family: f });
  add({ family: f, isInstitution: true });
  add({ family: f, given: '', isInstitution: true });
  add({ family: f, given: 'John', suffix: 'Jr.' });
  add({ family: f, given: 'John', 'dropping-particle': 'von' });
  add({ family: f, given: 'John', 'non-dropping-particle': 'van' });
  add({ family: f, given: 'John', 'static-particles': true });
  add({ family: f, given: 'John', 'parse-names': false });
  add({ family: f, given: 'John', 'parse-names': 0 });
  add({ family: f, given: 'John', 'parse-names': 1 });
  add({ family: f, given: 'John', 'parse-names': null });
  add({ family: f, given: 'John', 'static-ordering': true });
  add({ family: f, given: 'John', 'reverse-ordering': true, 'full-form-always': true, block_initialize: true });
  add({ family: f, given: 'John', multi: { main: 'ja' } });
  add({ family: f, given: 'John', multi: { main: 'vi' } });
  add({ family: f, given: 'John', multi: { main: 'hu-HU' } });
  add({ family: f, given: 'John', multi: { main: 'zh', _key: {} } });
  add({ family: f, given: 'John', multi: {} });
  add({ family: f, given: 'John', 'comma-suffix': true, 'comma-dropping-particle': ',' });
}
for (const lit of ['Acme Corp', 'The Foo & Bar', '']) { add({ literal: lit }); add({ literal: lit, family: 'X' }); }
add({}); add({ given: 'John' }); add({ given: 'John', family: null }); add({ family: 5, given: 'x' }); add({ family: 'x', given: 5 });

const VARIANTS = [
  { name: 'default', parse_names: true, vn: false, lang: null, refresh: false },
  { name: 'no-parse', parse_names: false, vn: false, lang: null, refresh: false },
  { name: 'vietnamese', parse_names: true, vn: true, lang: 'vi', refresh: false },
  { name: 'ja', parse_names: true, vn: false, lang: 'ja', refresh: true },
  { name: 'hu', parse_names: true, vn: true, lang: 'hu', refresh: false },
  { name: 'zh-CN', parse_names: true, vn: true, lang: 'zh-CN', refresh: false },
];
function fake(v) {
  const f = Object.create(CSL.NameOutput.prototype);
  f.state = { opt: { development_extensions: { parse_names: v.parse_names }, 'auto-vietnamese-names': v.vn } };
  f.Item = v.lang ? { language: v.lang } : {};
  return f;
}
const out = { variants: VARIANTS, particles: [], cases: [], n_fixture: nFixture };
for (const n of names) {
  const c = { name: plain(n) };
  const copy = plain(n);
  try { CSL.parseParticles(copy); c.particles = plain(copy); } catch (e) { c.particles_error = String(e.message); }
  c.v = VARIANTS.map((v) => {
    const r = {};
    const f = fake(v);
    try {
      const forStatic = plain(n);
      if (!forStatic.family) forStatic.family = '';
      if (!forStatic.given) forStatic.given = '';
      r.static_ordering = f.getStaticOrder(forStatic, v.refresh);
    } catch (e) { r.static_error = String(e.message); }
    try {
      const nn = f._normalizeNameInput(plain(n));
      r.norm = plain(nn);
    } catch (e) { r.norm_error = String(e.message); }
    try {
      const raw = plain(n);
      r.static_raw = f.getStaticOrder(raw, v.refresh);
    } catch (e) { r.static_raw_error = String(e.message); }
    try { r.romanesque = f._isRomanesque(plain(n)); } catch (e) { r.romanesque_error = String(e.message); }
    return r;
  });
  out.cases.push(c);
}
out.particle_list = plain(CSL.ParticleList);
write('names', out);
