# DBRC on the ACE route: attached when the data permits, never silently absent

GitHub **#307 item 3**. Implemented and measured 2026-09-25.

## The ambiguity being removed

`Nuclide::from_ace` set `dbrc: None` unconditionally, which made two very
different situations indistinguishable:

- the table is broadened, so 0 K elastic **is not in the file** and DBRC
  genuinely cannot be built; and
- the reader simply did not look.

That ambiguity is what #307 was filed about. It is resolved by making the
reason **queryable** rather than by guessing.

## Why a broadened table cannot supply it

DBRC samples the target velocity from the **unbroadened** elastic cross
section. An ACE table's ESZ elastic column is at the table's *own* temperature,
so a 293.6 K table's elastic has already been broadened once — using it would
broaden twice. The 0 K data is not hard to find in such a file; it is absent.

A **0 K table**, by contrast, carries exactly what is needed in its ESZ column.
`reference-data/ace` ships U-235 at both 0 K and 293.6 K, so both branches are
testable on real data.

## What was implemented

1. `elastic_0k_from_ace` takes the ESZ elastic pairs **only** when `kT <= 1e-9`
   eV. The threshold is on `kT` rather than a temperature: 1e-9 eV is far below
   any physical temperature (293.6 K is 2.53e-2 eV) and far above a 0 K header's
   rounding, so it separates the cases without a magic number.
2. `Nuclide::from_ace` now ends with `.with_dbrc(DBRC_DEFAULT_E_MAX_EV)` —
   **default ON**, matching the ENDF route. It is a no-op when `elastic_0k` is
   empty, so no branch is needed: the physics applies whenever the data permits,
   and a caller ablates explicitly with `without_dbrc`.
3. `Nuclide::with_elastic_0k_from_ace` pairs a broadened table with its 0 K
   companion and attaches DBRC.
4. `Nuclide::dbrc_unavailable_reason` returns `Some(reason)` whenever DBRC is
   off, naming both the missing data and the way out. No new struct field: the
   reason is derived from `dbrc`/`elastic_0k`, which keeps a hot type from
   growing.

## Results, 2026-09-25

`tests/dbrc_from_ace.rs` — **4 passed, 0 failed, no skips.**

| case | `has_dbrc()` | reason |
|---|---|---|
| U-235, 0 K table | **true** | `None` |
| U-235, 293.6 K table | false | names the missing 0 K elastic **and** `with_elastic_0k_from_ace` |
| 293.6 K paired with the 0 K companion | **true** | `None` |
| 293.6 K offered as its own companion | — | **refused** |

The refusal message, verbatim:

```text
the companion table is at kT = 2.530000e-2 eV, not 0 K. DBRC needs the
UNBROADENED elastic cross section; taking it from a broadened table samples a
distribution that has already been broadened once, which shifts k without
announcing itself.
```

### The guard that matters most

Pairing must supply the **velocity sample only**. If the 0 K companion
overwrote the broadened transport data, the whole problem would silently run at
0 K — plausible-looking and wrong. So the paired nuclide's total cross section
is asserted **unchanged** against the unpaired one at 1e-2, 1, 6.674, 1e3 and
1e5 eV, to 1e-12 relative. It is, at every energy checked.

A companion of the wrong **nuclide** is refused too, by comparing the ZAID's `A`
against the one this nuclide's AWR implies: 0 K elastic from the wrong nuclide
samples the wrong target mass.

## What this does not claim

- **No reactivity worth is measured here.** This establishes that DBRC is
  attached when it can be and explained when it cannot; it does not say what
  DBRC is worth on any case. The joint URR+DBRC worth on the homogenised LCT-008
  is bounded at **< 154 pcm at 2 sigma** in
  [`urr_dbrc_worth_2026_09_25.md`](urr_dbrc_worth_2026_09_25.md), which is a
  bound and not a measurement.
- **Most production ACE libraries are broadened**, so in practice DBRC on the
  ACE route requires the caller to hold a 0 K companion. That is a real
  operational cost of the ACE route relative to the ENDF one, where `from_tape`
  has the unbroadened RECONR output in hand already.
