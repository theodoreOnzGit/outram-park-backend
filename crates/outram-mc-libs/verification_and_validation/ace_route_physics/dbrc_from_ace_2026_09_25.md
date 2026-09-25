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

---

# Item 4 — the pairing happens by itself now

Implemented and measured **2026-09-25**, same day, after the above.

## What was still wrong

Everything above made DBRC *possible* on the ACE route. It left the **pairing** to
the caller: build the broadened nuclide, notice DBRC is off, find the 0 K
companion, read it, call `with_elastic_0k_from_ace`. Four steps, the first of
which is "notice".

A physics default that has to be noticed is not a default. It is the same defect
the workspace's *"correct physics is the DEFAULT SETTING, not an opt-in"* rule was
written about, one level up: the term was no longer behind a flag, it was behind
knowing that a second file existed. The ENDF route asks nothing of a caller —
`from_tape` broadens from unbroadened RECONR output and has the 0 K elastic in
hand — so the ACE route asking for four steps is an asymmetry, not a convenience.

## What was implemented

`Nuclide::from_ace_file(path, name)` — the ordinary entry point for a file, which
reads the table (Type 1, Type 2 or gzipped; `acer::read::read` sniffs), builds the
nuclide, and **if DBRC came out off for want of 0 K elastic** searches for the
companion in the three layouts ACE libraries use:

1. a **sibling temperature directory** — `…/293.6K/U235.ace.gz` →
   `…/0K/U235.ace.gz`, which is how `reference-data/ace` is laid out. The parent
   is treated as a temperature only if it ends in `K` and the rest parses as a
   number, so a directory called `LANL` is left alone;
2. a **`0K` subdirectory beside the file**, for a library that nests;
3. the **temperature in the file name** — `U235.0K.ace.gz` and `U235_0K.ace.gz`,
   with the suffix before the whole extension chain rather than inside it.

`zero_kelvin_companion_candidates` is public so a library with a fourth layout can
be searched by the caller rather than by patching that list, and
`Nuclide::zero_kelvin_companion_report` prints the candidates with `FOUND`/`absent`
— because when DBRC is off the question is always *where should it have been*, and
a list of paths answers it where a sentence cannot.

**A candidate that exists and is not usable is an error, not a silent skip.** A
file sitting in a library's `0K/` directory under this nuclide's name that turns
out to be a different nuclide, or not at 0 K, is a setup mistake worth hearing
about; `with_elastic_0k_from_ace`'s two refusals (wrong ZA, non-zero `kT`) are
wrapped with the path that failed. A companion that is simply **absent** is not an
error: the nuclide comes back with DBRC off and the reason says so.

## Results (2026-09-25)

Run on the real submodule, not a temporary directory —
`reference-data/ace` ships U-235 at both temperatures, which is layout 1:

```
0 K companion search for …/endf-b-viii.0/293.6K/U235.ace.gz:
  [FOUND ] …/endf-b-viii.0/0K/U235.ace.gz
  [absent] …/endf-b-viii.0/293.6K/0K/U235.ace.gz
  [absent] …/endf-b-viii.0/293.6K/U235.0K.ace.gz
  [absent] …/endf-b-viii.0/293.6K/U235_0K.ace.gz

from_ace(&hot)        -> has_dbrc = false  (reason: no 0 K elastic …)
from_ace_file(hot)    -> has_dbrc = true,  123 887 DBRC table points
from_ace(&cold)       -> has_dbrc = true,  123 887 DBRC table points
```

The paired table is asserted to be the **same length** as the one the 0 K table
gives directly, so the pairing is checked to have used the companion's grid rather
than to have merely turned a flag on.

Gates: `outram-mc-libs`'s `tests/dbrc_from_ace.rs` — 6 passed, of which
`from_ace_file_finds_the_0k_companion_in_the_reference_library` and
`the_companion_search_covers_the_three_documented_layouts` are new.

## What this does not claim

- **No `k` is re-measured here.** The worth of DBRC on LCT-008 is the separate
  measurement in [`urr_dbrc_worth_2026_09_25.md`](urr_dbrc_worth_2026_09_25.md),
  which reported a bound rather than a difference; this change makes the term
  reachable without asking, it does not re-price it.
- **The search is a list of conventions, not a discovery mechanism.** A library
  that stores its 0 K table under a name none of the three layouts predicts is not
  found, and the report is what says so.
