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

- **C1 (#800) — `deleteNodeByNameAttribute` skips the node after each deletion**
  (xmljson.js; it removes from an array while iterating it). Whether this ever
  changes a rendered output is not yet measured.
- **C2 (#801) — The flip-flopper turns `oblique` into `"undefined"`** for `<i>` inside
  an oblique context (util_flipflop.js: the flip table has no `oblique`
  entry), so the decoration lookup then fails. Probably a bug; needs the
  spec's rich-text markup section and a fixture to confirm the intended flip
  (to `normal`).
- **C3 (#802) — `CSL.getLocaleNames` throws a TypeError** through an unbound `this`
  (util_locale_sniff.js). Only reached through an API path the suite does not
  use.
- **C4 (#803) — Stale bundled locales.** citeproc-js 2.4.63's pinned `locale/`
  predates the test suite at `6eefc5b0`, so at least 9 of citeproc-js's 16
  fixture failures are locale data (`AD` vs ` AD`, `tran.` vs `trans.`). This
  is data, not engine code: the decision is which locale files the port ships
  and is verified with. The fixture `RESULT` shows the intended (newer)
  terms.
- **C5 (#804) — Greek and Lithuanian `toLocaleUpperCase`** are locale-specific in V8
  and only the Turkic rules are implemented here. A gap, not a quirk; listed
  so it is not forgotten.
- **C6 (#806) — `NAME_REX` typo**: `spans+class` where `\s+class` was meant
  (formatters.js; port `formatters.rs`), so name splitting never sees a
  `<span class="nocase">`.
- **C7–C15 (#807) — minor candidates found in the wave-1 integration**: number
  parsing branches that never fire (C7), unanchored default month matchers
  (C8), `year_numeric`'s `slice(0, -0)` (C9), the `"true"` sort-locale key
  (C10), `translit` listed twice in cite-affix forms (C11), `"undefined"`
  printed for a missing ordinal suffix or term (C12), falsy `Stack.push`
  storing `""` (C13), the misspelt `build_layout_locale_flag` (C14), cyclic
  gender terms in the locale merge (C15). Details and port locations in #807.

Update to C3 (#802), 2026-10-08: the unbound-`this` TypeError is reached by
**any `locale` attribute on `cs:layout`, `cs:if` or a condition**, which CSL
1.0.2 allows; the spec text is the evidence, so C3 is ready to register.

### Candidates from the wave-2 integration (2026-10-08)

Behaviour is unchanged: the port reproduces citeproc-js for each of these.
Locations are the port's files; the JS locations are in the names.

- **C16 — `CSL.Util.Dates.year["short"]` returns `undefined`** for a year that
  is not four digits, so `undefined` is printed (e.g. `March undefinedAD`); the
  same leak happens for the long/short month names when the locale lacks the
  term. Port: `util_dates.rs` (`year_short`, the month functions);
  `node_datepart.rs`.
- **C17 — `ad_end` / `bc_end` are computed in the date-part closure and never
  used** (node_datepart.js), so the end of a collapsed date range takes the
  start year's AD/BC label. Port: `node_datepart.rs` (`render`).
- **C18 — Empty cites are compared with `===` as arrays**, which is never
  true (api_cite.js; mirrored). Port: `api_cite.rs`.
- **C19 — `_locationOf` uses `end || length`** (sort.js / registry.js), so an
  `end` of 0 is read as "the whole list". Port: `registry.rs` / `sort.rs`.
- **C20 — `initVars` compares arrays as strings** (sort.js; numeric keys sort
  as text). Port: `sort.rs`.
- **C21 — An item without an `id` is keyed `"undefined"`** in the registry and
  caches (api_cite.js, registry.js). Port: `api_cite.rs`, `registry.rs`.
- **C22 — `makeBibliography` returns `false` for a style without a
  `cs:bibliography`** (api_bibliography.js); the port returns an `Err`.
  Port: `api_bibliography.rs`, `mod.rs`.
- **C23 — `state.tmp.multi_layout` is read but never assigned**
  (util_transform.js:477 and :502; `state.opt.multi_layout` is the assigned
  one), so the branches are dead. Port: `util_transform.rs`
  (`run_output_function`).
- **C24 — `localesets[0] === "locale-orig"` dead branch** in the same
  closure. Port: `util_transform.rs`.
- **C25 — The authority/committee split indexes a string** and takes one
  character (util_transform.js). Port: `util_transform.rs`.
- **C26 — The group END `done_vars` removal loop skips an element after each
  removal** (node_group.js, 2019-04-15 block: splices while iterating).
  Port: `node_group.rs` (`group_end`); same family as C1.
- **C27 — `outputNumericField` appends `"undefined"` when `labelSuffix` is
  undefined and does not recompute `labelPlaceholderPos`** (util_number.js).
  Port: `util_number.rs`.
- **C28 — The seventh argument of `getTextSubField` is ignored**
  (util_transform.js). Port: `util_transform.rs` (`get_text_sub_field`).
- **C29 — `init`/`reinit` use `for (var i in ...)` with `done_vars.slice(i+1)`**
  where `i` is a string key (util_names_output.js:53-58, 88-94); the slice
  start is a string-to-number coercion. Port: `util_names_output.rs`.
- **C30 — `PublisherOutput.render` always throws** `this._purgeEmptyBlobs is
  not a function` (util_publishers.js), after `clearVars`, `composeAndBlob` and
  `composeElements` ran; `state.publisherOutput` stays set. Port:
  `util_publishers.rs`, `node_group.rs` (`PublisherSpecialEnd`).
- **C31 — `_composeOneInstitutionPart`: `citeAffixes[slot.primary]` is never
  defined**, so the italic block is dead (util_names_render.js). Port:
  `util_names_render.rs`.
- **C32 — `_droppingParticle` overwrites `etal_spec[pos]`** (an array) with
  `1` or `2` (util_names_*.js). Port: `util_names_*.rs`.
- **C33 — `truncatePersonalNameLists` keeps a stale `v` and discards the
  result of `_truncateNameList(institutions)`** (util_names_truncate.js).
  Port: `util_names_truncate.rs`.
- **C34 — citeproc-js crashes reproduced as errors:** a classic abbreviation
  under `collapse` (`first_blob` TypeError), `cs:name delimiter=""` without
  `and` (`JSON.parse(undefined)`), an empty institution `and`
  (`undefined.blobs`). Port: `util_names_output.rs`, `util_names_render.rs`.
- **C35 — `getName` deletes `family`/`given` from the caller's name object
  for literal names** (util_names_render.js). The port works on a copy at the
  registry call sites (`disambig_names.rs::registry_name`,
  `util_citationlabel.rs`), so the caller's object is not mutated there.
