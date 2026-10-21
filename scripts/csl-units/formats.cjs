// Reference data for the output formats (formats.js, GitHub #793).
// Output: crates/kovan-literature/tests/data/csl/units/formats.json
//   { modes: { <mode>: { escape: [[in, out]], bibstart, bibend,
//       deco: [{ k: "@a/b", s: str|null, x: extra|null, th: this-object|null,
//                hook: bool, r: output | {error} }] } },
//     safe: [{ area, mode, thin, s, r }] }
'use strict';
const { CSL, makeEngine, writeUnits } = require('./common_output.cjs');

const MODES = ['html', 'text', 'rtf', 'asciidoc', 'fo', 'latex'];

const escStrings = [
  '', 'plain', 'a & b', '<tag> & "quote" \'apos\'', 'a  b', 'a   b', 'a   b', 'a\t\tb', 'a\n\nb', 'x\ty',
  'back\\slash {brace} }', 'star * under_score # hash ^ caret ~ tilde [[wiki]] [[[', '*a* *b* _c_ _d_ #e# #f#',
  '  two  pairs  here  ', 'm² and H₂O and x³', '™ ℠ ª º ʰ ᴬ ⁰⁴⁵', 'café üñî',
  '日本語', 'emoji 😀 ok', '\u007f\u0080ÿĀ￿', '$1 $& $$ $`', 'a&amp;b', '&lt;', ' thin space',
  'a » b : c ; d ? e !', '– —', 'x<sup>2</sup>', 'กข', 'tab\tand  nbsp',
];
const decoStrings = ['', 'abc', 'x & y', 'with $& and $$ and $\' and $` and $1 end', '"q"', 'http://example.org/a?b=1&c=2', '10.1000/xyz', 'https://doi.org/10.1/abc', 'http://doi.org/10.1/abc', ' .lead', 'trail. ', '!?x:y,.', 'line1\nline2', '    '];
const thisObjs = [null, { item_id: 'ITEM-1', locator_txt: 'p. 5', suffix_txt: ', ok', system_id: 'sys-9' }];
const params = { variableNames: ['title'], itemData: { URL: 'http://x.org/' }, context: 'citation', position: 'first' };
const params2 = { variableNames: ['first-reference-note-number'], itemData: {}, context: 'citation', position: 'subsequent' };

const out = { modes: {}, safe: [] };
for (const mode of MODES) {
  const fmt = CSL.Output.Formats[mode];
  const keys = Object.keys(fmt).filter((k) => k[0] === '@');
  const m = { escape: [], deco: [] };
  for (const s of escStrings) {
    let r;
    try { r = fmt.text_escape(s); } catch (e) { r = { error: String(e) }; }
    m.escape.push([s, r]);
  }
  m.bibstart = fmt.bibstart === undefined ? null : fmt.bibstart;
  m.bibend = fmt.bibend === undefined ? null : fmt.bibend;
  for (const hook of [false, true]) {
    const wrapper = hook ? function (p, pre, str, post) {
        if (p.variableNames[0] === 'title' && p.itemData.URL && p.context === 'citation' && p.position === 'first') {
          return pre + '<a href="' + p.itemData.URL + '">' + str + '</a>' + post;
        } else if (p.variableNames[0] === 'first-reference-note-number' && p.context === 'citation' && p.position !== 'first') {
          return pre + '<b>' + str + '</b>' + post;
        }
        return pre + str + post;
    } : undefined;
    const e = makeEngine({ format: mode, variableWrapper: wrapper });
    e.opt.nodenames = ['zero', 'one', 'two'];
    e.bibliography.opt.hangingindent = 2;
    for (const k of keys) {
      const [name, value] = k.split('/');
      for (const s of decoStrings.concat([null])) {
        for (const th of thisObjs) {
          const paramVariants = k === '@showid/true' && th ? [params, params2, null] : [null];
          for (const pv of paramVariants) {
            for (const x of (k === '@showid/true' ? [null, '1', '2'] : [null])) {
              const thisObj = th ? Object.assign({}, th, pv ? { params: pv } : {}) : (pv ? { params: pv } : null);
              if (!hook && k !== '@showid/true') continue; // hook only matters for showid
              let r;
              try {
                const f = e.fun.decorate[name][value];
                r = f.call(thisObj || {}, e, s === null ? undefined : s, x === null ? undefined : x);
              } catch (err) { r = { error: String(err) }; }
              m.deco.push({ k, s, x, th: thisObj, hook, r: r === undefined ? { undef: true } : r });
            }
          }
        }
      }
    }
  }
  out.modes[mode] = m;
}
for (const area of ['citation', 'bibliography', 'intext', 'citation_sort']) {
  for (const mode of ['html', 'text', 'rtf']) {
    for (const thin of [false, true]) {
      const e = makeEngine({ format: mode, area });
      e.opt.development_extensions.thin_non_breaking_space_html_hack = thin;
      const esc = CSL.getSafeEscape(e);
      for (const s of ['a b', 'x y', 'p & q', ' ', '']) {
        out.safe.push({ area, mode, thin, s, r: esc(s) });
      }
    }
  }
}
writeUnits('formats', out);
console.error('deco cases:', Object.values(out.modes).reduce((a, m) => a + m.deco.length, 0), 'escape:', MODES.length * escStrings.length);
