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

### D3 — `<i>` inside `oblique` and `<b>` inside `light` flip instead of crashing (C2, #801)
- **Where:** util_flipflop.js, the `_nestingData` flip tables of `<i>`
  (`@font-style`) and `<b>` (`@font-weight`); port `util_flipflop.rs`
  (`FlipFlopper::build`).
- **citeproc-js does:** the `<i>` table has only `italic` and `normal`, the
  `<b>` table only `bold` and `normal`. Inside a layout or element whose
  `font-style` is `oblique`, or whose `font-weight` is `light`, the lookup
  gives `undefined` and the html formatter throws `TypeError: Cannot read
  properties of undefined (reading 'call')`.
- **We do:** `<i>` inside `oblique` flips to `normal` (as inside `italic`);
  `<b>` inside `light` flips to `bold` (light is not bold, so the toggle adds
  bold; inside `bold` it already flips to `normal`). Maintainer decision
  2026-10-08.
- **Evidence:** CSL 1.0.2 lists `normal`/`italic`/`oblique` as the values of
  `font-style` and `normal`/`bold`/`light` as those of `font-weight`, so a
  crash on valid input is never the intended behaviour (no spec passage on how
  `<i>`/`<b>` flip; none of the 845 fixtures uses either value).
- **Affects:** only styles with `font-style="oblique"` or
  `font-weight="light"` on an ancestor of text containing `<i>` / `<b>`; none
  in the 845 fixtures or the 223-item site set. Probe output
  (`font-style="oblique"`, title `a <i>b</i> c`): `<em>a <span
  style="font-style:normal;">b</span> c</em>`; (`font-weight="light"`, title
  `a <b>b</b> c`): `a <b>b</b> c` (the html format has no wrapper for
  `light`, so the outer text is bare). Test: `util_flipflop.rs`
  `deviation_tests`.

### D4 — A non-four-digit year with `form="short"`, and a missing month term, no longer print "undefined" (C16, #808)
- **Where:** util_dates.js `CSL.Util.Dates.year["short"]` and
  `CSL.Util.Dates.month["long"|"short"]`, as called by `formatAndStrip` in
  node_datepart.js; port `node_datepart.rs` (`call_dates_formatter`);
  `util_dates.rs` itself still returns `None` (JS `undefined`) so its
  differential tests against citeproc-js stand.
- **citeproc-js does:** `year["short"]` returns `undefined` unless the year is
  four digits, and `""+undefined` is printed (`March undefined AD` for 99,
  `undefined` for 500); a month name whose term is missing from every locale
  also prints `undefined`.
- **We do:** a year that is not four digits uses the long form (99 gives
  `March 99 AD`, 12345 `March 12345`, -5 `March 5 BC`, 500 `March 500`); a
  missing month term gives the empty string.
- **Evidence:** CSL 1.0.2 gives `"short"` only as "e.g. 05" for the year; the
  literal word "undefined" is a JS artefact, never a spec output. The long
  form is the minimal fallback (maintainer decision 2026-10-08).
- **Affects:** none in the 845 fixtures or the site set. (The en-US locale
  bundled here has `<term name="ad"> AD</term>` with a leading space, so the
  probe strings carry a space before `AD`/`BC`; the issue text, measured
  with the older bundled locale, had none.) Test: `node_datepart.rs`
  `deviation_tests`.

### D5 — Each end of a BC-to-AD date range carries its own era label (C17, #808)
- **Where:** node_datepart.js, the date-part closure: `ad_end`/`bc_end` are
  computed and never read; port `node_datepart.rs` (`render`), two sites.
- **citeproc-js does:** (a) with the year before the collapsed parts, the end
  year takes the START year's label: -200 March to 200 May gives
  `200 BC-March–200 BC-May`; (b) when the year is the last part (the "ready"
  branch), the start year has no label and the start's label is appended once
  after the end year: `[[-200,3,1],[200,5,2]]` gives `March 1, 200–May 2,
  200 BC`, where AD 200 is labelled BC.
- **We do:** (a) the end year takes `ad_end`/`bc_end`: `200 BC-March–200
  AD-May`. (b) when the two ends have DIFFERENT labels (BC to AD, or AD to a
  year from 500 on, which has none), each end carries its own: `March 1, 200
  BC–May 2, 200 AD`; `100`..`600` gives `March 1, 100 AD–May 2, 600`. When
  both ends have the SAME label (both BC, both AD, both none) citeproc-js's
  single trailing label is kept: `March 1, 300–May 2, 200 BC`. **Choice made
  here:** the issue says the intended string for (b) is `March 1, 200 BC–May 2,
  200 AD` but does not say what a same-era range should do; labelling both
  ends there would also turn the conventional `300–200 BC` into `300 BC–200
  BC`, a change nothing motivates, so only a range whose ends disagree is
  changed. (Same-era output is identical to citeproc-js at both sites.)
  One further consequence: under `collapse="year-suffix"` the start year of
  `[[-44,3,15],[-43,4,16]]` is suppressed (it repeats the previous cite), so
  its label is empty while the end year's is `BC`; the ends then "differ" and
  the end year keeps its `BC` (`March –April 43 BC`; citeproc-js dropped the
  era altogether, `March –April 43`).
- **Evidence:** a range labelled BC at both ends when its end is AD is
  plainly wrong; CSL 1.0.2 has no text on era labels in ranges (maintainer
  decision 2026-10-08).
- **Affects:** only ranges whose ends have different era labels; none in the
  845 fixtures or the site set. In the date end-to-end replay
  (`date_e2e_test.rs`), see its `KNOWN_DIFFERENCES`. Test: `node_datepart.rs`
  `deviation_tests`.
### D13 — The port is verified and shipped with current CSL locale data
- **Where:** locale data, not code: `tests/citeproc_test_suite.rs` reads
  `vendor/csl-locales` (`citation-style-language/locales` at `a89adec`,
  2026-09-10; fetched by `scripts/csl-reference.sh`) instead of
  `vendor/citeproc-js/locale`.
- **citeproc-js does:** citeproc-js 2.4.63 pins the 2019 locales
  (`6b0cb46`, 2019-02-25), so e.g. `AD` is `AD` (not ` AD`), `tran.` for
  translator, `Jun` for June.
- **We do:** the port is verified and shipped with the current locales, the
  same commit as the site's `data/csl/locales-en-*.xml` (byte-identical).
  `tests/citeproc_intermediate.rs` stays on the pinned locales, because it
  compares engine state with citeproc-js's committed digests, made with them.
- **Evidence:** the fixtures' own `RESULT`: 842 of 845 equal it with the
  current locales (829 with the pinned ones). Exactly 13 fixtures now differ
  from citeproc-js's committed output and each equals its own `RESULT`:
  AD/BC spacing (6: `collapse_AuthorCollapseNoDateSorted`, `date_DateAD`,
  `date_DateBC`, `date_NegativeDateSort*` x3), `tran.` -> `trans.` (4:
  `name_EditorTranslatorSameWithTerm`,
  `magic_SubsequentAuthorSubstituteNotFooled`,
  `name_SubsequentAuthorSubstituteMultipleNames`, `label_EditorTranslator1`),
  `Jun` -> `June` (2: `punctuation_DateStripPeriods`,
  `bugreports_SortedIeeeItalicsFail`), and `name_EtAlWithCombined`. No
  fixture regresses. The 3 remaining `RESULT` misses (`decorations_Baseline`,
  `page_Chicago`, `textcase_TitleCaseWithVolumeTitle`) are unchanged
  citeproc-js behaviour, not locale data. Measured 2026-10-08 (#803).
- **Affects:** those 13 fixtures (listed in
  `tests/data/csl/test_suite_known_differences.json`, each citing D13). **The
  site set is unchanged**: both sides already used `a89adec`; with the pinned
  locales instead, 4 Vancouver bibliography entries would lose the "Report"
  label.
- **Licence:** the locales are CC BY-SA 3.0 (locales repo `README.md`
  "Licensing"; `<rights>` in each file). Attribution: mention the CSL project
  and link CitationStyles.org; keep the translator listings as is.

## Candidates (maintainer to decide)

Behaviour that looks accidental but for which no spec or fixture evidence has
been found yet. Until decided, **the port reproduces citeproc-js**.

- **C1 (#800) — `deleteNodeByNameAttribute` skips the node after each deletion**
  (xmljson.js; it removes from an array while iterating it). Whether this ever
  changes a rendered output is not yet measured.
- ~~**C2 (#801) — The flip-flopper turns `oblique` into `"undefined"`** for `<i>` inside an oblique context (util_flipflop.js: the flip table has no `oblique` entry), so the decoration lookup then fails.~~ → registered as D3 on 2026-10-08.
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
- ~~**C4 (#803) — Stale bundled locales. MEASURED 2026-10-08.** citeproc-js
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
  harness (the 13 comparisons would cite it).~~ → registered as D13 on 2026-10-08.
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

- ~~**C16 (#808) — `CSL.Util.Dates.year["short"]` returns `undefined`** for a year that is not four digits, so `undefined` is printed; the same leak happens for the month names when the locale lacks the term.~~ → registered as D4 on 2026-10-08.
- ~~**C17 (#808) — `ad_end` / `bc_end` are computed in the date-part closure and never used** (node_datepart.js), so the end of a collapsed date range takes the start year's AD/BC label.~~ → registered as D5 on 2026-10-08.
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
- **C25 (#808) — The authority/committee split indexes a string** and takes one
  character (util_transform.js). Port: `util_transform.rs`.
- **C26 (#808) — The group END `done_vars` removal loop skips an element after each
  removal** (node_group.js, 2019-04-15 block: splices while iterating).
  Port: `node_group.rs` (`group_end`); same family as C1.
- **C27 (#808) — `outputNumericField` appends `"undefined"` when `labelSuffix` is
  undefined and does not recompute `labelPlaceholderPos`** (util_number.js).
  Port: `util_number.rs`.
- **C28 (#808) — The seventh argument of `getTextSubField` is ignored**
  (util_transform.js). Port: `util_transform.rs` (`get_text_sub_field`).
- **C29 (#808) — `init`/`reinit` use `for (var i in ...)` with `done_vars.slice(i+1)`**
  where `i` is a string key (util_names_output.js:53-58, 88-94); the slice
  start is a string-to-number coercion. Port: `util_names_output.rs`.
- **C30 (#808) — `PublisherOutput.render` always throws** `this._purgeEmptyBlobs is
  not a function` (util_publishers.js), after `clearVars`, `composeAndBlob` and
  `composeElements` ran; `state.publisherOutput` stays set. Port:
  `util_publishers.rs`, `node_group.rs` (`PublisherSpecialEnd`).
- **C31 (#808) — `_composeOneInstitutionPart`: `citeAffixes[slot.primary]` is never
  defined**, so the italic block is dead (util_names_render.js). Port:
  `util_names_render.rs`.
- **C32 (#808) — `_droppingParticle` overwrites `etal_spec[pos]`** (an array) with
  `1` or `2` (util_names_*.js). Port: `util_names_*.rs`.
- **C33 (#808) — `truncatePersonalNameLists` keeps a stale `v` and discards the
  result of `_truncateNameList(institutions)`** (util_names_truncate.js).
  Port: `util_names_truncate.rs`.
- **C34 (#808) — citeproc-js crashes reproduced as errors:** a classic abbreviation
  under `collapse` (`first_blob` TypeError), `cs:name delimiter=""` without
  `and` (`JSON.parse(undefined)`), an empty institution `and`
  (`undefined.blobs`). Port: `util_names_output.rs`, `util_names_render.rs`.
- **C35 (#808) — `getName` deletes `family`/`given` from the caller's name object
  for literal names** (util_names_render.js). The port works on a copy at the
  registry call sites (`disambig_names.rs::registry_name`,
  `util_citationlabel.rs`), so the caller's object is not mutated there.
