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

### D6 — The authority/committee split keeps a whole translation string (was C25)
- **Where:** attributes.js, the `@variable` closure of `cs:names` ("check for
  output"); port `attributes.rs::variable_check_output`.
- **citeproc-js does:** splits `authority` / `committee` on `;`. If a
  translation (`multi._keys[variable][lang]`) has a different number of parts,
  everything is recombined into one name, but `rawMultiNames[langTag][j]`
  then indexes the translation *string* and takes its first character.
- **We do:** keep the whole translation string.
- **Evidence:** the code comment ("we only recombine everything if the length
  of all the splits matches") implies the whole string; a single character of
  a translation is never meaningful. Measured in #808 (2026-10-08).
- **Affects:** items with a string `authority`/`committee` and a translation
  whose part count differs. Probe (bibliography, `institutions` =
  `orig`+`translat`, translation tag `ja`): citeproc-js
  `Supreme Court; Appeals Court 最`, port `Supreme Court; Appeals Court
  最高裁判所`. No fixture or site-set change. Test:
  `deviation_tests_d6_d12.rs`.

### D7 — The short-form flag reaches the secondary and tertiary slots (was C28)
- **Where:** util_transform.js, `getOutputFunction` (calls) and
  `getTextSubField`; port `util_transform.rs::run_output_function`.
- **citeproc-js does:** passes `family_var` as the *seventh* argument of
  `getTextSubField`, which only has six parameters, so its `family_var` is
  `null` for the secondary and tertiary slots: with `form="short"` the primary
  takes `title-short` and the translation takes the *long* title.
- **We do:** pass `family_var`, so every slot applies the same short-form
  rule (`title-short` first, then the long form).
- **Evidence:** the call passes the argument deliberately; the result of the
  quirk (short primary, long secondary) is internally inconsistent. Measured
  in #808.
- **Affects:** `title` / `container-title` with `form="short"` and a
  translation or transliteration in the bibliography. Probes: citeproc-js
  `短 Gendai Long`, port `短 Tan` (title and container-title). No fixture or
  site-set change.

### D8 — A failed `require-match` unwinds its own variables (was C29)
- **Where:** util_names_output.js `init` and `reinit` (the loop after
  `checkCommonAuthor`); port `util_names_output.rs::unwind_done_vars`.
- **citeproc-js does:** `done_vars.slice(0, idx).concat(done_vars.slice(i+1))`
  with `i` the string key of `for (var i in this.variables)`, so the tail
  starts at `"0"+1 = "01"`, `"11"`, ... instead of `idx + 1`. When the
  variable is not first in `done_vars` it stays blocked.
- **We do:** `slice(idx + 1)`: exactly the variable is removed.
- **Evidence:** the loop's purpose (the comment in node_group.js: removing
  from `done_vars` "allows it to re-render"); `i+1` is plainly `idx+1`
  mistyped. Measured in #808.
- **Affects:** a `cs:names` with `require-match="true"` that fails inside a
  `cs:substitute`, followed by another `cs:names` over one of its variables.
  Probe: citeproc-js `The Title`, port `E Ed`. No fixture or site-set change.

### D9 — `PublisherOutput.render` joins the pairs instead of throwing (was C30)
- **Where:** util_publishers.js (`render`, `composePublishers`,
  `joinPublishers`); port `util_publishers.rs`, `util_names_join.rs::join_blobs`,
  `node_group.rs` (`PublisherSpecialEnd`).
- **citeproc-js does:** always throws `this._purgeEmptyBlobs is not a
  function` (`PublisherOutput.prototype._join` is `NameOutput`'s, which needs
  that method), after `clearVars`, `composeAndBlob` and `composeElements` ran.
- **We do:** the existing purge-empty join: each publisher/place pair joined
  with the group `delimiter`, the pairs with `subgroup-delimiter` and the
  `and` blob (`and_blob.single`: `_join` has three parameters, so the
  `multiple` argument is never read, as upstream).
- **Evidence:** the code exists to do exactly this and cannot ever have
  worked; the maintainer's decision (2026-10-08). The output format is the
  measurement agent's (#808).
- **Affects:** a `cs:group` with `subgroup-delimiter` over `;`-separated
  publishers and places. Probes: citeproc-js throws; port `Place X, Pub A and
  Place Y, Pub B` and `Place X, Pub A; Place Y, Pub B and Place Z, Pub C`. No
  fixture or site-set change. Unit test `render_joins_the_pairs_instead_of_throwing_as_upstream_does_d9`
  replaces `render_fails_as_upstream_does`.

### D10 — An "et al." dropping particle on an affiliated person keeps its et al. (was C32)
- **Where:** util_names_render.js `_droppingParticle`; port
  `util_names_render.rs::dropping_particle`.
- **citeproc-js does:** `this.etal_spec[pos].persons = 1|2` for a name that
  belongs to an institution, overwriting the *array* with a number, so every
  later `persons[j]` is `undefined` and the "et al." is lost.
- **We do:** `persons[j] = 1|2`, as the freeters path (`freeters = ...`)
  already does.
- **Evidence:** `etal_spec[pos].persons` is read as an array everywhere else
  (`persons[j] === 1`, util_names_join.js). Measured in #808.
- **Affects:** `spoof_institutional_affiliations` items with an `et al.`
  pseudo-name before an institution. Probe: citeproc-js `Al Smith and Ed Zed,
  Org One`, port `Al Smith, Ed Zed, et al., Org One`. No fixture or site-set
  change.

### D11 — `cs:name delimiter=""` is a delimiter (was C34a)
- **Where:** node_name.js (`else if (state.tmp.name_delimiter)`); port
  `node_name.rs`.
- **citeproc-js does:** tests the delimiter for truthiness, so `""` leaves
  `this.and` empty and the first join fails (`"undefined" is not valid JSON`,
  or `Cannot read properties of undefined (reading 'blobs')` with
  institutions).
- **We do:** build the delimiter blobs for `""` too.
- **Evidence:** `delimiter=""` is valid CSL 1.0.2 (a `delimiter` attribute may
  be empty), and the surrounding code ("Workaround to allow explicit empty
  string on cs:name delimiter") shows the case is meant to work.
- **Affects:** styles with `delimiter=""` and no `and`. Probes: citeproc-js
  throws; port `Al SmithBo Jones`, `Al SmithBo JonesCy Poe`, `Org OneOrg Two`.
  No fixture or site-set change; the `names_e2e` generated styles that hit
  this (see the test's results note) now render.

### D12 — A classic-item abbreviation under `collapse` uses the leaf's text (was C34b)
- **Where:** util_names_output.js `_collapseAuthor`; port
  `util_names_output.rs::last_child_string`.
- **citeproc-js does:** passes `top.blobs.slice(-1)[0].blobs`, which for the
  abbreviation of a `classic` item is a *string*, to `output.string`, which
  indexes it by character and throws on `first_blob`.
- **We do:** use the leaf's text as the author string (minimal change: only
  the crash is removed).
- **Evidence:** **none (spec or fixture): the intended output is the
  measurement agent's best judgement** (#808), as the maintainer was told.
- **Affects:** a `classic` item whose author+title abbreviation applies, under
  `collapse` or `cite-group-delimiter`. Probe: citeproc-js throws; port
  `Cic. Off. 1700; Cicero 1990`. No fixture or site-set change.

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
- **C3 (#802) — RESOLVED 2026-10-08, not a quirk: it was a port defect.**
  ~~`CSL.getLocaleNames` throws a TypeError through an unbound `this`~~:
  measured in node 22 with citeproc-js 2.4.63, it does **not** throw.
  `sniffLocaleOnOneNodeName(nodeName)` is called with `stylexml` as its first
  argument, matches no node, and never reaches the `this.` line, so it returns
  `en-US`, the preferred locale and the `default-locale` only. The port threw;
  it now returns what citeproc-js returns (`util_locale_sniff.rs`, test
  `locale_attributes_on_nodes_add_nothing_as_upstream`). No caller in the
  engine; no fixture or site style has a `locale` attribute on a layout or
  condition.
- **C4 (#803) — Stale bundled locales. MEASURED 2026-10-08.** citeproc-js
  2.4.63's `locale/` is pinned at `6b0cb46` (2019-02-25). With the CSL locales
  repository at `ec17593` (2026-04-01, the last commit before the suite's
  `6eefc5b0`) or at HEAD `a89adec` (2026-09-10) — identical fixture outputs —
  the port equals the fixtures' own `RESULT` on **842 of 845** (829 with the
  pinned locales): **13 of citeproc-js's 16 misses are fixed and none
  regress**. The 13: AD/BC spacing (6), `tran.`→`trans.` (4), `Jun`→`June`
  (2), one term change (`name_EtAlWithCombined`). The 3 still missing
  (`decorations_Baseline`, `page_Chicago`, `textcase_TitleCaseWithVolumeTitle`)
  are not locale data. The **site** already uses `a89adec`
  (`data/csl/locales-en-US.xml`, `locales-en-GB.xml` byte-identical), on
  both sides of its comparison; with the pinned locales instead, 4 Vancouver
  bibliography entries lose the "Report" label. Locales are CC BY-SA 3.0
  (locales repo README; `<rights>` in each file). **Ready to register** as a
  data deviation if the maintainer chooses the newer locales for the fixture
  harness (the 13 comparisons would cite it).
- **C5 (#804) — Greek and Lithuanian `toLocaleUpperCase`. FIXED 2026-10-08.**
  ~~MEASURED 2026-10-08: a gap, not a quirk. The port's non-Turkic path
  (`s.to_uppercase()`) is **wrong against citeproc-js** for `el` (V8 strips
  tonos and adds dialytika: `άλφα ΜΆΙΟΣ` → node `ΑΛΦΑ ΜΑΪΟΣ`, port
  `ΆΛΦΑ ΜΆΙΟΣ`) and `lt` (dot above dropped after i/j). No effect on the 845
  fixtures (their Greek text only reaches lowercasing) or the site set (no
  `el`/`lt` items). Fix = implement the el/lt special casing with differential
  tests against node.~~ **FIXED**: `load::to_locale_upper_case` /
  `to_locale_lower_case` now take V8's choice of language (the **first** list
  element only, validated as a BCP 47 tag and canonicalised, so `ell`/`gre` =
  `el`; later elements are never looked at, e.g. `["en-US","xx_invalid"]` does
  not throw, `["el","tr"]` is Greek) and then use ICU4X `icu_casemap`
  (Unicode-3.0) for `el` lower, `lt` upper/lower and `tr`/`az`, plus a
  hand port of ICU's `GreekUpper::toUpper` (`citeproc/greek_upper.rs`) for `el`
  upper, because ICU4X's own Greek upper path disagrees with node on combining
  marks after consonants and on accented vowels followed by further marks.
  Also corrected: the old "any invalid tag in the list falls back" rule was
  wrong against node (only the first element matters). Differential
  (`scripts/csl-units/locale_case.cjs`, node 22.22.2 / ICU 78.2): 12,038
  strings x 44 lang arrays x upper/lower = **1,059,344 comparisons, 0
  mismatches** (`load::locale_case_tests`). First attempt with ICU4X alone:
  12,888 mismatches, all `el` upper.
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

~~Update to C3 (#802), 2026-10-08: the unbound-`this` TypeError is reached by
**any `locale` attribute on `cs:layout`, `cs:if` or a condition**, which CSL
1.0.2 allows; the spec text is the evidence, so C3 is ready to register.~~
**CORRECTED 2026-10-08** (measurement in #802): both halves were wrong.
citeproc-js never throws there (see C3 above), and the CSL 1.0.2
specification (`documentation/specification.rst`, master) has no `locale`
attribute on `cs:layout` or conditions — that is a CSL-M (Juris-M) extension
citeproc-js implements in `@locale`. There was no spec evidence to cite.

### Candidates from the wave-2 integration (2026-10-08)

Behaviour is unchanged: the port reproduces citeproc-js for each of these.
Locations are the port's files; the JS locations are in the names.

- **C16 (#808) — `CSL.Util.Dates.year["short"]` returns `undefined`** for a year that
  is not four digits, so `undefined` is printed (e.g. `March undefinedAD`); the
  same leak happens for the long/short month names when the locale lacks the
  term. Port: `util_dates.rs` (`year_short`, the month functions);
  `node_datepart.rs`.
- **C17 (#808) — `ad_end` / `bc_end` are computed in the date-part closure and never
  used** (node_datepart.js), so the end of a collapsed date range takes the
  start year's AD/BC label. Port: `node_datepart.rs` (`render`).
- **C18 (#808) — Empty cites are compared with `===` as arrays**, which is never
  true (api_cite.js; mirrored). Port: `api_cite.rs`.
- **C19 (#808) — `_locationOf` uses `end || length`** (sort.js / registry.js), so an
  `end` of 0 is read as "the whole list". Port: `registry.rs` / `sort.rs`.
- **C20 (#808) — `initVars` compares arrays as strings** (sort.js; numeric keys sort
  as text). Port: `sort.rs`.
- **C21 (#808) — An item without an `id` is keyed `"undefined"`** in the registry and
  caches (api_cite.js, registry.js). Port: `api_cite.rs`, `registry.rs`.
- **C22 (#808) — `makeBibliography` returns `false` for a style without a
  `cs:bibliography`** (api_bibliography.js); the port returns an `Err`.
  Port: `api_bibliography.rs`, `mod.rs`.
- **C23 (#808) — `state.tmp.multi_layout` is read but never assigned**
  (util_transform.js:477 and :502; `state.opt.multi_layout` is the assigned
  one), so the branches are dead. Port: `util_transform.rs`
  (`run_output_function`).
- **C24 (#808) — `localesets[0] === "locale-orig"` dead branch** in the same
  closure. Port: `util_transform.rs`.
- ~~**C25 (#808) — The authority/committee split indexes a string** and takes one
  character (util_transform.js). Port: `util_transform.rs`.~~ **-> registered as D6 on 2026-10-08.**
- **C26 (#808) — The group END `done_vars` removal loop skips an element after each
  removal** (node_group.js, 2019-04-15 block: splices while iterating).
  Port: `node_group.rs` (`group_end`); same family as C1.
- **C27 (#808) — `outputNumericField` appends `"undefined"` when `labelSuffix` is
  undefined and does not recompute `labelPlaceholderPos`** (util_number.js).
  Port: `util_number.rs`.
- ~~**C28 (#808) — The seventh argument of `getTextSubField` is ignored**
  (util_transform.js). Port: `util_transform.rs` (`get_text_sub_field`).~~ **-> registered as D7 on 2026-10-08.**
- ~~**C29 (#808) — `init`/`reinit` use `for (var i in ...)` with `done_vars.slice(i+1)`**
  where `i` is a string key (util_names_output.js:53-58, 88-94); the slice
  start is a string-to-number coercion. Port: `util_names_output.rs`.~~ **-> registered as D8 on 2026-10-08.**
- ~~**C30 (#808) — `PublisherOutput.render` always throws** `this._purgeEmptyBlobs is
  not a function` (util_publishers.js), after `clearVars`, `composeAndBlob` and
  `composeElements` ran; `state.publisherOutput` stays set. Port:
  `util_publishers.rs`, `node_group.rs` (`PublisherSpecialEnd`).~~ **-> registered as D9 on 2026-10-08.**
- **C31 (#808) — `_composeOneInstitutionPart`: `citeAffixes[slot.primary]` is never
  defined**, so the italic block is dead (util_names_render.js). Port:
  `util_names_render.rs`.
- ~~**C32 (#808) — `_droppingParticle` overwrites `etal_spec[pos]`** (an array) with
  `1` or `2` (util_names_*.js). Port: `util_names_*.rs`.~~ **-> registered as D10 on 2026-10-08.**
- **C33 (#808) — `truncatePersonalNameLists` keeps a stale `v` and discards the
  result of `_truncateNameList(institutions)`** (util_names_truncate.js).
  Port: `util_names_truncate.rs`.
- ~~**C34 (#808) — citeproc-js crashes reproduced as errors:** a classic abbreviation
  under `collapse` (`first_blob` TypeError), `cs:name delimiter=""` without
  `and` (`JSON.parse(undefined)`), an empty institution `and`
  (`undefined.blobs`). Port: `util_names_output.rs`, `util_names_render.rs`.~~ **-> registered as D11 and D12 (C34a, C34b) on 2026-10-08.**
- **C35 (#808) — `getName` deletes `family`/`given` from the caller's name object
  for literal names** (util_names_render.js). The port works on a copy at the
  registry call sites (`disambig_names.rs::registry_name`,
  `util_citationlabel.rs`), so the caller's object is not mutated there.
