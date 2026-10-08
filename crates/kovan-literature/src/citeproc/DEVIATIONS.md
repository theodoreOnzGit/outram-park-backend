# Deviations from citeproc-js 2.4.63 (epic #790)

The port follows citeproc-js except where citeproc-js's behaviour is a quirk
that contradicts CSL's intent (rule and arbiter order: `PORTING.md` §0). Each
entry below is a quirk we deliberately do **not** reproduce. Every
known-differences entry in the comparison tests that is caused by one of these
cites its `Dnn`; any difference without one is a defect in the port.

Entry format: **where** (JS file:function), **citeproc-js does**, **we do**,
**evidence** (CSL 1.0.2 spec section, or fixture whose `RESULT` shows the
intent), **affects** (which outputs change; "none observed" until measured).

## Registered deviations

### D1 — UTF-16 surrogate halves
- **Where:** every `str.slice(-1)`, `str.charAt(i)`, `str.slice(0, 1)` on text
  (queue.js `append`, util_flipflop.js, util_name_particles.js, ...).
- **citeproc-js does:** splits a character outside the Basic Multilingual
  Plane (emoji, some CJK extension B ideographs) into a lone surrogate, which
  can then be emitted or compared.
- **We do:** operate on whole characters (`js.rs` helpers map a split pair to
  the whole character or U+FFFD; a Rust `String` cannot hold a lone
  surrogate).
- **Evidence:** a lone surrogate is not a Unicode scalar value, so no CSL
  output containing one is well-formed text; CSL 1.0.2 operates on strings of
  characters throughout.
- **Affects:** none observed in the test suite (no astral characters at a
  split point). To be re-measured by the fixture sweep.

### D2 — A style without a `version` attribute is accepted
- **Where:** build.js (the Engine constructor reads `opt.version`).
- **citeproc-js does:** throws on `undefined.slice`.
- **We do:** treat the version as `"1.0"`.
- **Evidence:** CSL 1.0.2 "Style" says `version` is required, so such a style
  is invalid input; throwing a JS TypeError is an accident, not a validation
  decision. Valid styles are unaffected.
- **Affects:** no valid style; only the port's own minimal test styles.

## Candidates (maintainer to decide)

Behaviour that looks accidental but for which no spec or fixture evidence has
been found yet. Until decided, **the port reproduces citeproc-js**.

- **C1 — `deleteNodeByNameAttribute` skips the node after each deletion**
  (xmljson.js; it removes from an array while iterating it). Whether this ever
  changes a rendered output is not yet measured.
- **C2 — The flip-flopper turns `oblique` into `"undefined"`** for `<i>` inside
  an oblique context (util_flipflop.js: the flip table has no `oblique`
  entry), so the decoration lookup then fails. Probably a bug; needs the
  spec's rich-text markup section and a fixture to confirm the intended flip
  (to `normal`).
- **C3 — `CSL.getLocaleNames` throws a TypeError** through an unbound `this`
  (util_locale_sniff.js). Only reached through an API path the suite does not
  use.
- **C4 — Stale bundled locales.** citeproc-js 2.4.63's pinned `locale/`
  predates the test suite at `6eefc5b0`, so at least 9 of citeproc-js's 16
  fixture failures are locale data (`AD` vs ` AD`, `tran.` vs `trans.`). This
  is data, not engine code: the decision is which locale files the port ships
  and is verified with. The fixture `RESULT` shows the intended (newer)
  terms.
- **C5 — Greek and Lithuanian `toLocaleUpperCase`** are locale-specific in V8
  and only the Turkic rules are implemented here. A gap, not a quirk; listed
  so it is not forgotten.
