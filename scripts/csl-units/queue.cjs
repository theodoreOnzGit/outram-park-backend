// Reference data for the output queue (queue.js, obj_number.js, GitHub #793).
// Output: crates/kovan-literature/tests/data/csl/units/queue.json
//   { seqs: [{ cfg, ops, res }] }
// Each sequence is replayed on a fresh en-US engine: `ops` are calls on
// engine.output (or engine.dateput) -- append, openLevel, closeLevel,
// startTag, endTag, addToken, pushFormats, popFormats, clearlevel, pop,
// purgeEmptyBlobs, the adjust passes, string() -- and `res` records what each
// returned plus serialised queue dumps. The Rust test (src/citeproc/queue.rs)
// replays the same ops with the same names.
//
// Token spec  { n: name, s: {prefix, suffix, delimiter, text-case, ...},
//               d: [[attr, value]...], x: {successor_prefix, range_prefix,
//               splice_prefix, gender, formatter: "default"|"suffixator"|"romanizer"} }
// Token ref   null | "name" | tokenSpec
'use strict';
const { CSL, makeEngine, ser, serRes, writeUnits, prng } = require('./common_output.cjs');

function mkTok(sp) {
  const t = new CSL.Token(sp.n || 'x', CSL.SINGLETON);
  for (const k of Object.keys(sp.s || {})) t.strings[k] = sp.s[k];
  t.decorations = (sp.d || []).map((x) => x.slice());
  const x = sp.x || {};
  for (const k of Object.keys(x)) {
    if (k === 'formatter') {
      t.formatter = x.formatter === 'suffixator' ? new CSL.Util.Suffixator(CSL.SUFFIX_CHARS)
        : x.formatter === 'romanizer' ? new CSL.Util.Romanizer() : new CSL.Output.DefaultFormatter();
    } else {
      t[k] = x[k];
    }
  }
  return t;
}
function ref(e, r) {
  if (r === null || r === undefined) return undefined;
  if (typeof r === 'string') return r;
  return mkTok(r);
}
function serTok(t) {
  if (!t) return null;
  const s = {};
  for (const k of Object.keys(t.strings || {})) if (t.strings[k] !== undefined) s[k] = t.strings[k];
  return { s, d: (t.decorations || []).map((x) => x.slice()) };
}

function exec(seq) {
  const cfg = seq.cfg;
  const e = makeEngine({ format: cfg.mode || 'html', area: cfg.area || 'citation', punctInQuote: cfg.piq });
  e.tmp.strip_periods = cfg.strip ? 1 : 0;
  e.tmp.just_looking = !!cfg.just_looking;
  e.tmp.suppress_decorations = !!cfg.suppress;
  if (cfg.note) { e.opt.xclass = 'note'; e.output.checkNestedBrace = new CSL.checkNestedBrace(e); }
  const q = cfg.q === 'dateput' ? e.dateput : e.output;
  const res = [];
  for (const op of seq.ops) {
    let r = null;
    try {
      switch (op[0]) {
        case 'append': r = q.append(op[1] === null ? undefined : op[1], ref(e, op[2]), op[3], op[4], op[5]); break;
        case 'appendBlobLiteral': {
          const inner = new CSL.Blob(op[1], q.formats.value().empty);
          r = q.append(inner, 'literal');
          break;
        }
        case 'appendNum': {
          const tok = mkTok(op[2]);
          const b = new CSL.NumericBlob(e, op[3] || false, op[1], tok, op[4] || 'ID');
          if (op[5]) { b.setFormatter(op[5] === 'suffixator' ? new CSL.Util.Suffixator(CSL.SUFFIX_CHARS) : new CSL.Util.Romanizer()); }
          r = q.append(b, 'literal');
          break;
        }
        case 'open': q.openLevel(ref(e, op[1])); break;
        case 'close': q.closeLevel(op[1] === null ? undefined : op[1]); break;
        case 'startTag': q.startTag(op[1], op[2] ? mkTok(op[2]) : undefined); break;
        case 'endTag': q.endTag(op[1] === null ? undefined : op[1]); break;
        case 'addToken': q.addToken(op[1], op[2] === null ? undefined : op[2], ref(e, op[3])); break;
        case 'pushFormats': {
          let ts = undefined;
          if (op[1]) { ts = {}; for (const k of Object.keys(op[1])) ts[k] = mkTok(op[1][k]); }
          q.pushFormats(ts); break;
        }
        case 'popFormats': q.popFormats(); break;
        case 'clearlevel': q.clearlevel(); break;
        case 'pop': { const p = q.pop(); r = p === undefined ? null : serRes(p); break; }
        case 'getToken': r = serTok(q.getToken(op[1])); break;
        case 'merge': r = serTok(q.mergeTokenStrings(op[1], op[2])); break;
        case 'purge': for (let i = 0; i < q.queue.length; i++) CSL.Output.Queue.purgeEmptyBlobs(q.queue[i]); break;
        case 'adjust':
          for (let j = 0; j < q.queue.length; j++) {
            for (const f of op[1]) e.output.adjust[f](q.queue[j]);
          }
          break;
        case 'dump': r = q.queue.map((b) => serRes(b)); break;
        case 'string': r = serRes(q.string(e, q.queue)); break;
        case 'tmp': for (const k of Object.keys(op[1])) e.tmp[k] = op[1][k]; break;
        default: throw new Error('unknown op ' + op[0]);
      }
    } catch (err) {
      r = { error: String(err) };
    }
    res.push(r === undefined ? null : r);
  }
  return res;
}

// ---------------------------------------------------------------------------
// Hand-written sequences
// ---------------------------------------------------------------------------
const T = (n, s, d, x) => ({ n, s: s || {}, d: d || [], x });
const AFTER = [['upward', 'leftward', 'downward', 'fix']];
const ADJ = ['upward', 'leftward', 'downward', 'fix'];

function simple(ops, cfg) { return { cfg: Object.assign({ piq: true }, cfg), ops }; }

const hand = [];
// quotes, punctuation in/out
for (const piq of [true, false]) {
  for (const suffix of ['.', ',', ';', ':', '!', '?', '. ', '', ').']) {
    for (const text of ['title', 'title.', 'title,', 'title!', 'title?']) {
      hand.push(simple([
        ['open', T('g', { delimiter: '', suffix })],
        ['open', T('q', { prefix: '', suffix: '' }, [['@quotes', 'true']])],
        ['append', text, null],
        ['close', null],
        ['close', null],
        ['dump'], ['adjust', ADJ], ['dump'], ['string'],
      ], { piq }));
      hand.push(simple([
        ['append', text, T('t', { suffix, prefix: '' }, [['@quotes', 'true']])],
        ['dump'], ['adjust', ADJ], ['dump'], ['string'],
      ], { piq }));
      hand.push(simple([
        ['open', T('g', { delimiter: suffix || ', ' })],
        ['append', text, T('t', { suffix: '' }, [['@quotes', 'true']])],
        ['append', 'next', T('t', { prefix: '' })],
        ['close', null],
        ['dump'], ['adjust', ADJ], ['dump'], ['string'],
      ], { piq }));
    }
  }
}
// groups with delimiters and affixes
const affixes = ['', '(', ')', ' ', ', ', '. ', '.', ',', ' (', ') ', ': ', '; ', '[', ']', ' ', '“', '”'];
const delims = ['', ', ', '; ', '. ', ': ', '.', ',', ' ', ' - ', ' – ', ' '];
let k = 0;
for (const pre of affixes) {
  for (const suf of affixes) {
    const dl = delims[k++ % delims.length];
    hand.push(simple([
      ['open', T('g', { prefix: pre, suffix: suf, delimiter: dl })],
      ['append', 'a', T('t', { prefix: affixes[(k * 3) % affixes.length], suffix: affixes[(k * 7) % affixes.length] })],
      ['append', 'b.', T('t', { prefix: affixes[(k * 5) % affixes.length], suffix: affixes[(k * 11) % affixes.length] })],
      ['close', null],
      ['dump'], ['adjust', ADJ], ['dump'], ['string'],
    ]));
  }
}
// append edge behaviour
for (const text of ['', ' ', 'x', ' x', '. x', ': y', '; z', '!? q', '. ', '..', ':', "'x'", " 'x", "a 'b'", 'a :b', 'a ;b', 'a ?b', 'a !b', 'a »', '« a', 'a  \'b', 'x.y. z.', 'A.B.C.', 'a.1', '<i>i</i>', '"q"', 'café é', '日本', 'M. de la Cruz.', 'x   y']) {
  for (const strip of (text.includes('.') ? [false, true] : [false])) {
    for (const tok of [null, T('t', { 'text-case': 'uppercase' }), T('t', {}, [['@font-style', 'italic']]), T('t', {}, [['@quotes', 'true']])]) {
      for (const flags of [[false, false, false], [true, false, false], [false, true, true]]) {
        hand.push(simple([
          ['append', text, tok, flags[0], flags[1], flags[2]],
          ['dump'],
        ], { strip }));
      }
    }
  }
}
// level/format-store management
hand.push(simple([
  ['startTag', 'aa', T('aa', { prefix: '<', suffix: '>' })],
  ['append', 'x', 'aa'],
  ['addToken', 'bb', ', ', 'aa'],
  ['append', 'y', 'bb'],
  ['getToken', 'bb'], ['merge', 'aa', 'bb'], ['merge', 'aa', 'nope'], ['merge', 'nope', 'bb'], ['getToken', 'empty'],
  ['pushFormats', { cc: T('cc', { prefix: '[' }) }],
  ['getToken', 'cc'], ['getToken', 'aa'], ['open', 'cc'], ['append', 'z', 'cc'], ['close', 'cc'],
  ['popFormats'], ['getToken', 'cc'],
  ['endTag', 'aa'], ['dump'], ['string'],
]));
hand.push(simple([
  ['open', null], ['append', 'x', null], ['close', 'empty'], ['open', 'nope'], ['close', 'wrong'],
  ['startTag', 'aa', T('aa')], ['close', 'bb'], ['endTag', 'aa'], ['dump'],
]));
hand.push(simple([
  ['append', 'x', 'nope'], ['append', 'y', 'literal'], ['append', null, null], ['pop'], ['pop'], ['pop'],
  ['open', null], ['append', 'k', null], ['append', 'l', null], ['clearlevel'], ['pop'], ['close', null], ['dump'], ['string'],
]));
hand.push(simple([
  ['tmp', { 'doing-macro-with-date': true }],
  ['append', 'a', 'empty'], ['append', 'b', 'macro-with-date'], ['append', 'c', null, true], ['dump'],
  ['startTag', 'z', T('z', { prefix: '(' })], ['append', 'd', 'macro-with-date'], ['endTag', 'z'], ['dump'],
  ['tmp', { extension: 'yes' }],
  ['startTag', 'z', T('z', { prefix: '(' })], ['append', 'd', 'macro-with-date', true], ['endTag', 'z'], ['dump'], ['string'],
]));
hand.push(simple([['appendBlobLiteral', 'abc'], ['append', 'plain', null], ['dump'], ['adjust', ADJ], ['string']]));
// note-style nested parentheses
hand.push(simple([
  ['open', T('g', { prefix: '(', suffix: ')' })],
  ['open', T('h', { prefix: '(', suffix: ')' })],
  ['append', 'x', null],
  ['close', null], ['close', null], ['dump'], ['string'],
], { note: true }));
hand.push(simple([
  ['open', T('g', { prefix: '(', suffix: ')' })],
  ['open', T('h', { prefix: '(', suffix: ')' })],
  ['append', 'x', null],
  ['close', null], ['close', null], ['dump'], ['string'],
], { note: true, just_looking: true }));
// first_blob (offset counting) and nested levels with parents carrying affixes
hand.push(simple([
  ['open', T('g', { delimiter: ', ', prefix: '(', suffix: ')' })],
  ['append', 'x', T('t', { first_blob: 'ID1' })],
  ['append', 'y', T('t', {})],
  ['close', null], ['dump'], ['adjust', ADJ], ['string'],
]));
// numeric blobs
const numTok = (extra, s) => T('n', Object.assign({ prefix: '', suffix: '' }, s || {}), [], extra);
for (const fmt of ['default', 'suffixator', 'romanizer']) {
  for (const nums of [[1], [1, 2], [1, 2, 3], [1, 2, 3, 5], [1, 3, 5], [2, 3, 5, 6, 7, 9], [5, 4, 3], [1, 1, 2], [10, 11, 12, 13, 15, 16], [1, 2, 4, 5, 6, 8]]) {
    for (const delim of [', ', '; ', '']) {
      for (const rp of ['', '–', '-']) {
        const x = { successor_prefix: delim || ',', range_prefix: rp, splice_prefix: delim, formatter: fmt };
        hand.push(simple([
          ['open', T('g', { delimiter: delim, prefix: '', suffix: '' })],
          ...nums.map((n) => ['appendNum', n, numTok(x, {}), false, 'ID', null]),
          ['close', null], ['dump'], ['string'],
        ]));
        hand.push(simple([
          ['open', T('g', { delimiter: delim, prefix: '[', suffix: ']' })],
          ...nums.map((n) => ['appendNum', n, numTok(x, { prefix: '', suffix: '' }), false, 'ID', null]),
          ['close', null], ['dump'], ['adjust', ADJ], ['dump'], ['string'],
        ]));
      }
    }
  }
}
// numeric with affixes, decorations, text-case, particles, text around
hand.push(simple([
  ['open', T('g', { delimiter: ', ', prefix: '(', suffix: ')' })],
  ['appendNum', 3, T('n', { prefix: 'a', suffix: 'b', 'text-case': 'uppercase' }, [['@font-style', 'italic']], { successor_prefix: ',', range_prefix: '-', splice_prefix: ';', formatter: 'default' }), 'p', 'ID'],
  ['appendNum', 4, T('n', { prefix: 'a', suffix: 'b' }, [], { successor_prefix: ',', range_prefix: '-', splice_prefix: ';', formatter: 'default' }), false, 'ID'],
  ['append', 'text', null],
  ['appendNum', 7, T('n', {}, [], { successor_prefix: ',', range_prefix: '-', splice_prefix: ';', formatter: 'default' }), false, 'ID'],
  ['close', null], ['dump'], ['string'],
]));
hand.push(simple([
  ['open', T('g', { delimiter: '', prefix: '', suffix: '' })],
  ['appendNum', 1, numTok({ successor_prefix: ',', range_prefix: '-', splice_prefix: ';', formatter: 'suffixator' }), false, 'ID'],
  ['appendNum', 2, numTok({ successor_prefix: ',', range_prefix: '-', splice_prefix: ';', formatter: 'suffixator' }), false, 'ID'],
  ['close', null], ['dump'], ['string'],
], { suppress: true }));
hand.push(simple([
  ['open', T('g', { delimiter: '', prefix: '', suffix: '' })],
  ['appendNum', 1, numTok({ successor_prefix: ',', range_prefix: '-', splice_prefix: ';', formatter: 'suffixator' }), false, 'ID'],
  ['close', null], ['dump'], ['string'],
], { area: 'bibliography' }));
hand.push(simple([
  ['open', T('g', { delimiter: ', ', prefix: '<', suffix: '>' }, [['@font-weight', 'bold']])],
  ['appendNum', 1, numTok({ successor_prefix: ',', range_prefix: '-', splice_prefix: ';', formatter: 'default' }), false, 'ID'],
  ['close', null], ['dump'], ['string'],
]));

// ---------------------------------------------------------------------------
// Random sequences
// ---------------------------------------------------------------------------
const rand = prng(793793793);
const pick = (a) => a[Math.floor(rand() * a.length)];
const texts = ['Title', 'title', 'the title.', 'Title!', 'A question?', 'Colon:', 'semi;', 'comma,', 'x', 'y.', 'Smith', 'J.', 'J. R.', 'et al.', 'In', 'p. 5', '12', '12-14', ' lead', 'trail ',
  '"quoted"', "it's", "'single'", 'a <i>b</i> c', 'x<sup>2</sup>', 'café', '日本', '. dot first', ': colon first', 'Ph.D.', 'a.b.', '', ' ', 'v. 3', 'No.', 'e.g.', '…', '(par)', '[br]'];
const affs = ['', '', '', '', ' ', ', ', '. ', '.', ',', ':', ';', '!', '?', '(', ')', ' (', ') ', '[', ']', '“', '”', '. ', ' - ', ' '];
const dls = ['', '', '', ', ', '; ', '. ', ': ', ' ', '.', ',', ' & ', '–', ' — '];
const decs = [[], [], [], [], [['@font-style', 'italic']], [['@font-weight', 'bold']], [['@quotes', 'true']], [['@quotes', 'true']], [['@quotes', 'inner']],
  [['@font-variant', 'small-caps']], [['@vertical-align', 'sup']], [['@text-decoration', 'underline']], [['@font-style', 'normal']], [['@display', 'block']], [['@bibliography', 'entry']],
  [['@font-style', 'italic'], ['@quotes', 'true']], [['@display', 'left-margin']], [['@display', 'right-inline']], [['@display', 'indent']]];
const tcs = [undefined, undefined, undefined, undefined, undefined, 'lowercase', 'uppercase', 'title', 'capitalize-first', 'capitalize-all', 'sentence'];

function randTok(depthLeaf) {
  const s = { prefix: pick(affs), suffix: pick(affs) };
  if (!depthLeaf) s.delimiter = pick(dls);
  else if (rand() < 0.2) s.delimiter = pick(dls);
  const tc = pick(tcs);
  if (depthLeaf && tc) s['text-case'] = tc;
  return T('r', s, pick(decs).map((x) => x.slice()));
}

function genSeq(i) {
  const cfg = {
    piq: rand() < 0.5,
    area: pick(['citation', 'citation', 'bibliography']),
    strip: rand() < 0.15,
    mode: pick(['html', 'html', 'html', 'html', 'text', 'rtf']),
    q: rand() < 0.12 ? 'dateput' : 'output',
  };
  if (rand() < 0.1) cfg.suppress = true;
  if (rand() < 0.08) cfg.note = true;
  const ops = [];
  let open = 0;
  let numId = 0;
  function emit(depth) {
    ops.push(['open', randTok(false)]);
    open++;
    const n = 1 + Math.floor(rand() * 4);
    for (let j = 0; j < n; j++) {
      const r = rand();
      if (r < 0.18 && depth < 3) emit(depth + 1);
      else if (r < 0.26) {
        const x = { successor_prefix: pick([',', ';', '']), range_prefix: pick(['', '-', '–']), splice_prefix: pick(['', ',', ';']), formatter: pick(['default', 'default', 'suffixator', 'romanizer']) };
        const base = Math.floor(rand() * 8);
        const cnt = 1 + Math.floor(rand() * 4);
        for (let m = 0; m < cnt; m++) {
          ops.push(['appendNum', base + m + (rand() < 0.2 ? 1 : 0), T('n', { prefix: pick(['', '', '(']), suffix: pick(['', '', ')', '.']), 'text-case': pick([undefined, undefined, 'uppercase']) }, rand() < 0.2 ? [['@font-style', 'italic']] : [], x), false, 'ID' + (numId++ % 2), null]);
        }
      } else {
        ops.push(['append', pick(texts), rand() < 0.15 ? null : randTok(true), rand() < 0.05, rand() < 0.05, rand() < 0.05]);
      }
    }
    ops.push(['close', null]);
    open--;
  }
  const top = 1 + Math.floor(rand() * 3);
  for (let t = 0; t < top; t++) {
    if (rand() < 0.15) ops.push(['append', pick(texts), randTok(true)]);
    else emit(0);
  }
  if (rand() < 0.3) ops.push(['purge']);
  ops.push(['dump']);
  if (cfg.q === 'output') {
    ops.push(['adjust', rand() < 0.85 ? ADJ : pick([['upward'], ['leftward'], ['downward'], ['fix'], ['upward', 'downward']])]);
    ops.push(['dump']);
  }
  ops.push(['string']);
  return { cfg, ops };
}

const seqs = hand.slice();
const nRandom = 650;
for (let i = 0; i < nRandom; i++) seqs.push(genSeq(i));

let errors = 0;
for (const s of seqs) {
  s.res = exec(s);
  for (const r of s.res) if (r && r.error) errors++;
}
// Trim the file: store each sequence compactly.
writeUnits('queue', { seqs });
console.error('queue sequences:', seqs.length, 'hand-written:', hand.length, 'op errors:', errors);
