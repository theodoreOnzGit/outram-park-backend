# ACER continuous-energy tables vs NJOY2016 — 57-tape sweep

**Generated:** 2026-09-20, UTC.
**Crate / commit:** `njoy-outram-park-fork` 0.0.3, on
`claude/neutronics-runs-handoff-8xk972`.
**Reference:** NJOY2016 **2016.79** (`ac5adf5`), built from
`upstream_source/NJOY2016/` and run. Tables published as the
`reference-data/ace` submodule.
**Class:** verification — code-to-code against upstream. **Not validation**:
no experiment is compared against, and no human V&V has been done.
AI-assisted draft.

## What changed on this date

The comparison previously covered **U-235 at 0 K only**, and recorded three
gaps. Three are closed and the coverage is extended to three nuclides at
293.6 K. Closing them also exposed **three gaps nobody had measured**, because
they only become visible once a second nuclide and a second temperature are in
play.

| gap | before | after |
|---|---|---|
| reaction inventory | ours 47, NJOY 84 (U-235 0 K) | **matches exactly on all three nuclides** |
| fission ν̄ (NU block) | `JXS(2) = 0` — no fission source at all | **347/347 values identical** at Type-1 print precision (~~bit-identical~~, corrected 2026-09-26, see the NU section) |
| fission representation | MT=18 always kept, partials always dropped | **follows upstream's `mt19` rule** |
| nuclide coverage | U-235 | **U-234, U-235, U-238**, then **all 57 tapes** — see the sweep below |
| temperature coverage | 0 K | **0 K and 293.6 K** |
| MT=460 delayed photons | written into the photon block | **excluded, per `acefc.f90:400/3642/4030`** |
| MT=4 keep/drop rule | keyed on discrete levels (this port's own invention) | **keyed on MF=12/MT=4, per `acefc.f90:1713-1731`** |

## Methodology

`examples/ace_vs_njoy2016` builds a table through this crate's own pipeline —
RECONR (tolerance 0.001) → BROADR at the requested temperature → ACER — and
compares it against the NJOY2016 table built by
`make_ace.sh` (`reconr / broadr / purr / acer`, `errthn = .001`, 293.6 K) from
the **same ENDF/B-VIII.0 tape**.

Three instruments, and **which one to quote matters more than any number here**:

1. **Interpolated onto NJOY's grid.** What a consumer of our table would see.
   Contaminated by each code's own lin-lin tolerance across resonance peaks.
2. **At shared grid energies, no interpolation.** Exact same energy in both
   tables, so it measures the port and not the grid.
3. **Split at the broadening limit `thnmax`** — added 2026-09-20, and the
   reason this record exists in its present form. See below.

## The finding that changes how the earlier numbers read

> **SUPERSEDED THE SAME DAY — read to the end of this section.** The claim
> below ("not verified at all") was true of the instruments that existed when
> it was written. A grid-independent one was then built, and Doppler broadening
> **is now verified on all three nuclides**. The original text stands because
> the reasoning is what produced the fix.

~~**Doppler broadening is NOT verified against NJOY2016. Not to 1e-6, not at
all.**~~

Above `thnmax` neither code runs SIGMA1 (`broadr.f90:441`, ported as
`broadr::broadening_limit`), so both tables there are the *same* unbroadened
RECONR output. Only shared points **below** `thnmax` test the broadening. On
all three nuclides:

| nuclide | thnmax | shared grid points | of those, below thnmax |
|---|---|---|---|
| U-234 | 1.5 keV | 411 of 25 393 (1.6 %) | **0** |
| U-235 | 2.25 keV | 729 of 76 027 (1.0 %) | **0** |
| U-238 | 20 keV | 334 of 155 207 (0.2 %) | **0** |

Every shared point on every nuclide lies in the region neither code broadens.
The 293.6 K "agreement at ~1e-6" is therefore a **re-measurement of the 0 K
result**, and reporting it as agreement on broadening would have been simply
false. Before the split was added, that is exactly what this comparison
appeared to show.

Below `thnmax` the two codes' adaptive grids essentially never coincide, so a
shared-point comparison cannot reach the broadened region at all. **Verifying
broadening needs a different instrument** — a grid-independent one.

### That instrument was built, and broadening now IS verified

Both tables are lin-lin by construction, so the integral of each over a fixed
energy band is exact and depends only on the function the table *defines* —
not on where either code put its grid points. Equal-lethargy bands, 20 per
decade, each integrated on its own grid.

**The instrument validates itself before it is trusted:** in the unbroadened
region it reproduces the shared-point answer it must (U-235 3.5e-7 against the
shared-point 4.7e-7; U-238 2.0e-7 against 4.5e-7). A new instrument that
disagreed with the old one where the old one works would be measuring
something else.

Worst relative difference in band integrals, **below `thnmax`** — the bands
that actually test broadening:

| nuclide | bands | total | absorption | elastic |
|---|---|---|---|---|
| U-234 | 163 | 6.680e-4 | 5.132e-4 | 8.305e-4 |
| U-235 | 167 | 4.393e-4 | 4.088e-4 | 3.174e-4 |
| U-238 | 186 | 3.547e-4 | 4.840e-4 | 3.599e-4 |

**All nine are inside `errthn = 1e-3`**, the thinning tolerance each table is
written to in the first place — so the two codes' broadened cross sections
agree to better than either table individually promises. That is the strongest
statement the data supports, and it is a real verification of this port's
SIGMA1 against NJOY2016's, which nothing in this file supported an hour
earlier.

## Results — reaction inventory

| nuclide, T | NTR ours/NJOY | NR ours/NJOY | verdict |
|---|---|---|---|
| U-235, 0 K | **84 / 84** | **44 / 44** | identical sets, zero difference either direction |
| U-234, 293.6 K | ~~49 / 49~~ **46 / 49** | ~~48 / 48~~ **45 / 48** | **deliberately not matched — see below** |
| U-238, 293.6 K | **49 / 49** | **44 / 44** | every NJOY MT present, producer sets match |

`NR` = `NXS(5)`, the count of reactions carrying a secondary-neutron
distribution. It was off by one on U-235/U-238 and by four on U-234 until
fission secondaries were implemented — see below.

Two distinct defects were behind the earlier mismatches.

**MT=649 and MT=800–835 (37 reactions).** `role_of`'s `m >= 251` catch-all
swept the discrete charged-particle levels up with the genuinely derived
quantities. Upstream stores them in a second pass over MF=3 (`acefc.f90` ~5536)
and — the half that is easy to miss — keeps them **out of the ESZ total and
disappearance** when the lumped MT=103/107 is present (`mt103.eq.0 .and. …`,
~5652/~5670), because the lumped section already sums them. Storing without
that guard double-counts the whole (n,p)/(n,α) channel into the total,
silently. Evidence the guard is right: the ESZ agreement is **bit-identical
before and after** adding 37 reactions.

**The fission representation.** Upstream does not choose between MT=18 and
MT=19/20/21/38 on whether MT=18 exists — it keys on `mt19` (`acefc.f90:388`),
whether the evaluation supplies **MF=4/5/6 for MT=19**:

```text
skip if (mt19==1 and mth==18) or (mt19==0 and mth in {19,20,21,38})
```

Checked on the tapes: U-234 has MF=4/MT=19 (so NJOY stores the partials and no
MT=18); U-235 and U-238 have none (so NJOY stores MT=18). This port had the
rule hardcoded to one side, which is why U-234 carried MT=18 where NJOY
carried the four partials. Both tables were internally consistent — nothing
failed, they simply disagreed about which reactions exist, which is why only a
cross-nuclide comparison could surface it.

### …and why U-234 deliberately still does NOT match

Applying that rule to U-234 **broke the table**, and the band-integral
instrument caught it: the total went from `2.5e-3` to `1.8e-1` — ours
`56.5 b` against NJOY's `69.2 b` at 516 eV, **18 % low**.

Cause, measured rather than guessed: **RECONR adds the resonance
reconstruction to MT=18 only.** This port's MT=19 comes back as the smooth
355-point background, against MT=18's several thousand points. Swapping MT=18
for the partials therefore dropped resonance fission out of the ESZ total —
silently, because every individual section stayed self-consistent and the
*inventory* looked more correct than before.

**This is a difference between the two codes' architectures, not a free
choice.** `acelod` copies the total from MF=3 MT=1, which carries full fission
whatever is stored in MTR, so NJOY can store an incomplete MT=19 harmlessly.
This port rebuilds the total as `elastic + Σ partials`, which makes "what is
stored" and "what the total contains" the same question.

So the `mt19` rule is now applied **only when the partials actually sum to
MT=18** (`FISSION_SUM_TOL = 1e-3`, checked over MT=18's own grid, where a
higher-chance channel below its threshold correctly contributes zero). U-234
fails that check, keeps MT=18, and its inventory reads 46 against NJOY's 49.

**The cost is stated rather than hidden:** three MTs of inventory parity, in
exchange for a total that is right. The check is measured and self-correcting
— if RECONR is later taught to reconstruct the partials, it passes and the
inventory matches with nobody revisiting this. **The real defect is in RECONR,
not in ACER**, and that is where the fix belongs.

Restored by the guard: U-234's broadened-band total went `2.129e-2` →
**`6.680e-4`**, a 32× improvement, back inside tolerance.

## Results — ESZ cross sections at shared grid energies

All in the unbroadened region (see above).

| nuclide, T | total | absorption | elastic |
|---|---|---|---|
| U-235, 0 K | 4.697e-7 | 1.527e-6 | 8.058e-7 |
| U-235, 293.6 K | 4.657e-7 | 1.527e-6 | 8.058e-7 |
| U-238, 293.6 K | 4.484e-7 | 4.659e-7 | 0.0 |
| **U-234, 293.6 K** | **1.912e-3** | **4.212e-3** | 3.178e-4 |

**U-234 is an outlier by three to four orders of magnitude, and it is not
explained by broadening or by interpolation** — these are exact shared
energies in the region neither code broadens. Worst cases sit at 43.7 keV and
90 keV, above U-234's 1.5 keV resolved limit, i.e. in the unresolved/smooth
region. This is an open defect candidate; the leading hypotheses are the
unresolved-range treatment (the `LSSF` flag, as in `op-mzvp.2.12`) and PURR's
effect on the reference's MF=3. **Neither has been checked** — recorded as a
hypothesis, not a cause, per this workspace's own rule about that exact
mistake.

## Results — fission secondaries (χ), and what the NR gap actually was

`NXS(5)` was 43 against NJOY's 44 on U-235. The missing producer was **MT=18**
— found by diffing the two TYR blocks rather than inferred, which matters
because the first guess was MT=5 and it was wrong.

The consequence was worse than a count: **the table had ν̄ but no fission
energy distribution.** A fission source needs both, so the NU block alone did
not make these tables usable — that only became visible once NR was chased.

Implemented as Law 4 from **MF=5**, laboratory frame, with `TYR = 19` — the
ACE flag meaning *the yield comes from the NU block*, verified against
NJOY's own tables (MT=18 for U-235/U-238, each of MT=19/20/21/38 for U-234).

**MF=5, not MF=6, and that choice is load-bearing.** U-235 and U-238 carry
both an MF=5 and an MF=6 for MT=18. Upstream skips the MF=6 explicitly —
`acefc.f90:4398`, `call tosend(...) !skip past mf6/mt18 (for now)` — so taking
MF=6 because it happens to parse would silently disagree with NJOY on the
fission spectrum of every major actinide.

Predicted before measuring: U-235 and U-238 gain one producer (MT=18) to reach
44, U-234 gains four (MT=19/20/21/38, each with its own MF=5) to reach 48.
Measured: exactly that, on all three, with the producer sets identical to
NJOY's.

## Results — ν̄ (the NU block)

~~**347 of 347 values bit-identical** to NJOY2016's~~ **CORRECTED 2026-09-26:
347 of 347 values identical as NJOY's Type-1 file prints them (`1pE20.11`)**,
header included (`[-173, 2, 0, 85, …]` both sides), for U-235 at 0 K.

Why the wording moved: the oracle is read from a Type-1 file, which cannot
carry upstream `sigfig`'s `bias = 1.0000000000001` (`util.f90:392`). Bit
equality held only because this port's `sigfig` omitted the bias too; when it
was made faithful (delegating to `mixr::mix::sigfig`, needed to reproduce
NJOY's DNED block), 170 energies moved in the 14th figure -- below anything the
file can show. The gate still demands exact equality, of the printed value.

Exact equality is the right bar, not a tolerance: the block is the
evaluation's own table carried through with one unit change (eV → MeV), so
anything short of an identical printed value would mean a real difference in how it is read
or written. Pinned by `tests/acer_ce_esz_vs_njoy2016.rs` against
`reference-data/acer/u235_0k_nu_njoy2016.csv`.

**One deliberate divergence from upstream, unverified.** For `LNU=1` NJOY
writes `nut(i+2)*emev*(i-1)` — emev *times* `(i-1)`, not *raised to* it. The
unit change requires the power; the two agree for the first two coefficients
and diverge from the third on. This port uses the power. **No tape in
`reference-data/` uses LNU=1** — U-234/235/238 are all LNU=2 for MT=452/455/456
— so the branch is unexercised and must not be described as matching NJOY.


## Sweep — every incident-neutron tape in the repository (2026-09-20)

The three-nuclide result above answers *"does this port reproduce NJOY on the
uranium isotopes we actually run"*. It cannot answer *"where does it stop
reproducing NJOY"*, because three actinides from one library share a resonance
formalism, a mass range and an evaluation style. So every incident-neutron
tape in `reference-data/endf/` — **57 of them** — was put through the same
comparison.

### Protocol

Per tape: NJOY2016 `reconr` + `acer` at **0 K**, then
`examples/ace_vs_njoy2016` against this crate's own RECONR → ACER output, then
the reference is deleted before the next tape starts (Fe-56's alone is 114 MB
and the container has 16 GB free). `errthn = .001` on both sides.

**0 K is deliberate.** No BROADR and no PURR, so any difference is
attributable to ACER and the reconstruction rather than confounded with
Doppler broadening or with probability tables.

**The comparison is at shared grid energies, never by array position.** Two
tables built by different codes do not share a grid; an early version of this
comparator differenced `xss[i]` against `xss[i]` and reported a 1753x artefact
for it. `SUMMARY` gives the worst *relative* difference on the total, the
absorption and the elastic columns over the energies present in both tables.

### Read this by MECHANISM, not by mass

The first pass over these results was written up as *"the gaps are in the
light nuclides"*. That was wrong, and it was wrong three times over:
**Sr-88** (A=88) has one of the largest `NXS(5)` undercounts, **Zn-67** has one
of the largest ESZ residuals, and **U-238 JENDL-3.3** (A=238) has both. Sorting
by mass hides the structure. Sorting by mechanism gives four causes with four
different owners, and two of them were fixed on the spot.

### Headline

| | |
|---|---|
| tapes compared | **54** (of 57; 3 failed to produce a comparison — see below) |
| MT set (`MTR`) identical to NJOY | **53 / 54** — the one exception is U-234's deliberate `mt19` difference, recorded above |
| ESZ worst relative difference `< 1e-4` | **27 / 54** |
| `NU` block presence agrees | **54 / 54** |
| photon block (`MTRP`) agrees wherever we emit one | **48 / 54** |

The inventory result is the strongest single statement here: across 54
evaluations from five libraries (ENDF/B-VII.0, VII.1, VIII.0, VIII.1,
JENDL-3.3, TENDL-2023) and masses 1 to 239, this port and NJOY2016 disagree
about *which reactions exist* on exactly one tape, and that one disagreement is
deliberate and documented.

### Two defects the sweep found, both fixed and verified

**1. MT=460 delayed photon data was written into the photon block.** U-235
ENDF/B-VII.0 came back with `NXS(6) = 3295` against NJOY's 33. The arithmetic
localises it with no guesswork: that tape's MF=12 sections are MT=4 (`NK=30`),
MT=18 (1), MT=102 (1) and **MT=460 (`NK=3262`)**, plus one MF=13 section, and
`30+1+1+3262+1 = 3295` while `30+1+1+1 = 33`. MT=460 is *delayed* photon data
and upstream excludes it in three separate places — `convr`'s `gmt` list
(`acefc.f90:400`), `gamout`'s counting pass (`:3642`) and `gamout`'s writing
pass, which skips MF=12 **and** MF=14 for it (`:4030`). All three are now
ported. Because `LSIGP`/`LDLWP` are positional, this was a malformed table
rather than merely an over-full one.

**Scope, measured rather than assumed:** U-235 ENDF/B-VII.0 is the *only* tape
in `reference-data/endf/` carrying MF=12/MT=460, so no other recorded result
moved. Re-measured after the fix: **`NTRP` 33 / 33, `mtrp = ok`.**

**2. MT=4 was dropped on the wrong criterion.** The same tape reported 46
reactions against NJOY's 47, missing MT=4. This port kept MT=4 only when no
discrete inelastic levels (MT=51–91) were present. **That is not upstream's
rule and does not even correlate with it.** `convr` (`acefc.f90:1713-1731`)
eliminates MT=3 and MT=4 unless the MT is in `mf12s`, in `mf16s`, or is the
unresolved-resonance competition reaction; `mf12s` is built from the tape's own
dictionary for `mfd.eq.12.and.(mtd.lt.5.or.mtd.gt.600)` (`:390`) and `mf16s`
only ever receives MTs `.ge.600` (`:4418`), so for MT=4 the test reduces to
**"does MF=12/MT=4 exist"**.

The discriminating evidence is that U-235 VII.0 and U-235 VIII.0 *both* carry
~40 discrete levels, and NJOY keeps MT=4 on the first and drops it on the
second — because only VII.0 carries MF=12/MT=4. Checked across the set:

| tape | MF=12/MT=4 | NJOY stores MT=4 |
|---|---|---|
| U-235 ENDF/B-VII.0 | **yes** | **yes** |
| U-235 ENDF/B-VIII.0 | no | no |
| U-238 ENDF/B-VIII.0 | no | no |
| U-234 ENDF/B-VIII.0 | no | no |

Storing MT=4 beside its own levels would double-count inelastic scattering into
the rebuilt ESZ total, so `covered_by_levels` keeps it out of the sums —
upstream never needs this because `acelod` takes the total straight off MF=3
MT=1 rather than rebuilding it.

**The verification is that the ESZ did not move.** After the fix U-235 VII.0
reads `ntr = 47/47`, `mtr = ok` — and `nes = 245762`, `esz_tot = 5.603e-5`,
`esz_abs = 1.763e-7`, `esz_ela = 7.857e-7`, every one **bit-identical to the
pre-fix run**. A reaction was added to the inventory and the cross sections did
not change, which is the same evidence pattern used for the 37
charged-particle levels above. U-235 ENDF/B-VIII.0 was re-run as a regression
check on the *drop* path and is byte-identical to its pre-fix `SUMMARY`.

**Not ported:** the `mtcomp.eq.4` clause (`:1731`), which keeps MT=4 when the
URR data names it as the competition reaction. No tape here exercises it
against this port's PURR-free path, so it is left unimplemented rather than
written untested.

### Mechanism 1 — `NXS(5)` undercount: reactions represented as MF=4 + MF=5

`NXS(5)` counts reactions carrying a secondary-neutron distribution. **Sixteen
tapes are short.** One of those sixteen is U-234, whose shortfall is the
deliberate `mt19` fission-representation difference documented earlier in this
file; it is excluded below. **For the other fifteen the cause is identified,
and it is a single cause:** this port's
ACER writes a DLW entry from MF=6, and writes none for a reaction the
evaluation represents as **MF=4 (angular) + MF=5 (energy)** instead.

The test is a count match against tape structure alone — for each tape, how
many MTs carry MF=5 with no MF=6 (excluding MT=18/455, which go through the
fission and delayed-ν̄ paths rather than DLW):

| tape | `NXS(5)` short by | MF=5-without-MF=6 MTs | match |
|---|---|---|---|
| Li-6 ENDF/B-VIII.0 | 1 | 1 — MT=24 | ✅ |
| Li-7 ENDF/B-VIII.0 | 3 | 3 — 16, 24, 25 | ✅ |
| C-nat ENDF/B-VII.0 | 2 | 2 — 28, 91 | ✅ |
| C-12 ENDF/B-VIII.0 | 2 | 2 — 28, 91 | ✅ |
| Na-23 ENDF/B-VIII.0 | 2 | 2 — 16, 91 | ✅ |
| Mg-24 ENDF/B-VIII.0 | 4 | 4 — 16, 22, 28, 91 | ✅ |
| Mg-25 ENDF/B-VIII.0 | 4 | 4 — 16, 22, 28, 91 | ✅ |
| Mg-26 ENDF/B-VIII.0 | 4 | 4 — 16, 22, 28, 91 | ✅ |
| Sr-88 ENDF/B-VIII.1 | 5 | 5 — 16, 17, 22, 28, 91 | ✅ |
| U-238 JENDL-3.3 | 4 | 4 — 16, 17, 37, 91 | ✅ |
| Pu-239 JENDL-3.3 | 4 | 4 — 16, 17, 37, 91 | ✅ |
| H-2 ENDF/B-VIII.0 | 1 | **0** | ❌ |
| Be-9 ENDF/B-VIII.0 | 1 | **0** | ❌ |
| Ar-37 TENDL-2023 (both tapes) | 1 | **0** | ❌ |

**Eleven of the fifteen tapes match exactly** (the table has fourteen rows
because the two Ar-37 tapes behave identically), across shortfalls of 1, 2, 3,
4 and 5 — not a coincidence at that spread. The mechanism is **mass-independent** (Li-6
through Pu-239) and **library-independent** (VII.0, VIII.0, VIII.1, JENDL-3.3
all appear).

**The three exceptions are a different, undiagnosed mechanism.** H-2, Be-9 and
Ar-37 are each short by exactly one and have **no** MF=5-without-MF=6 gap at
all, so the MF=4/5 explanation cannot account for them. No cause is proposed
here.

**This mechanism is now gated.** `tests/continuum_law_coverage_survey.rs`
already asserted that every MF=4/5 section builds an `UncorrelatedEmission`,
but its inventory pin (`MF45_SECTIONS`) ran *before* that assertion, so adding
tapes made the pin fire and the real gate never ran — the only way to clear it
was to bump a number on faith. The gate now runs first and the pin second, so
updating the count records a confirmed inventory instead of asserting one.
Re-measured over all 57 tapes: **98 sections from MF=6, 20 from MF=4/5, 0
MF=6-but-no-law, 0 with no law anywhere**, and all 20 MF=4/5 sections build a
law.

### Mechanism 2 — photon block: an over-broad refusal

| tape | `NTRP` ours / NJOY |
|---|---|
| H-1 ENDF/B-VIII.0-Beta6 | 0 / 1 |
| C-nat ENDF/B-VII.0 | 0 / 5 |
| C-12 ENDF/B-VIII.0 | 0 / 5 |
| O-16 ENDF/B-VII.0 | 0 / 131 |
| O-16 ENDF/B-VIII.0 | 0 / 131 |
| Fe-58 ENDF/B-VIII.0 | 350 / 368 |

The first five are **this port's own refusal, not a parsing failure**.
`photon_blocks::build` returns `None` when it meets MF=14 with `LI = 0`
(anisotropic photon emission), because writing those entries isotropically
would be a silently wrong table. The refusal is correct in kind and **far too
broad in scope**: it discards the *whole material's* photon block over
anisotropy that may affect only a few entries. O-16 loses 131 entries this way.

Fe-58 ENDF/B-VIII.0 is different — 350 of 368, so a genuine partial shortfall,
undiagnosed. Note its sibling `Fe-58-ENDF8.0-Beta4` matches at **370 / 370**,
which makes this a difference between two revisions of one evaluation and a
good place to start.

Everything else agrees: mid-weight metals match exactly including large blocks
(Mn-55 571/571, Al-27 413/413, Ar-37 479/479, Cu-65 481/481, Fe-56 532/532,
Cl-35 554/554).

### Mechanism 3 — ESZ residuals, and one genuinely large one

27 of 54 tapes agree to better than `1e-4` on every ESZ column. Of the rest,
one is in a different class from all the others.

**Cl-35 ENDF/B-VII.1 — total `3.036e-1`, absorption `6.822e-1`.** Two orders of
magnitude worse than anything else in the sweep, and it is localised:

| channel | bands above `thnmax` | bands inside the resolved range |
|---|---|---|
| elastic | 6.4e-8 | **1.0e-4** |
| total | 1.4e-6 | **1.9e-1** |
| absorption | 3.0e-5 | **5.3e-1** |

(Grid-independent band integrals, 20 per decade. `thnmax = 1.2 MeV` is exactly
this evaluation's resolved-resonance upper bound. This is a 0 K run, so
"below `thnmax`" means *inside the resolved resonance range*, not "broadened".)

So the port reproduces NJOY **exactly outside the resonance range**, and
reproduces the **elastic** channel to `1e-4` inside it, while the
**absorption** channel is wrong by up to 53 %. Worst point: ours 0.772 b
against NJOY's 2.430 b at 51.6 keV. The reaction inventory is clean
(159/159) and so is the photon block (554/554), so this is neither ACER nor the
grid — it is the **absorption channel of the resonance reconstruction**.

Cl-35 is `LRU=1 / LRF=7` (R-matrix limited). **A hypothesis, explicitly
untested:** RML carries the charged-particle reactions as explicit R-matrix
channels, and Cl-35 has a large (n,p). Whether this port adds RML resonance
structure to those channels has not been checked, and no fix is proposed on the
strength of it.

The remaining residuals are `1e-4`–`1e-2`, absorption-dominated, and are not
grouped by mass: O-16 VII.0 `1.0e-2`, U-238 JENDL-3.3 `9.0e-3`, U-234 `4.2e-3`,
Zn-64 `4.0e-3`, Zn-67 `2.1e-3`, Pu-239 JENDL-3.3 `1.1e-3`, then a long tail of
titanium, silicon, magnesium and zinc isotopes in the `5e-4`–`1e-3` band. **No
cause is established for any of them.**

### Mechanism 4 — three tapes produced no comparison

| tape | outcome |
|---|---|
| B-10 ENDF/B-VIII.0 | ~~**`NJOYFAIL`** — NJOY2016's own run fails. Not this port.~~ **CORRECTED 2026-09-20 — INCOMPLETE, and now COMPARED.** NJOY fails only with `newfor = 1`; with `newfor = 0` it returns `rc = 0`. See below. |
| Fe-57 ENDF/B-VIII.0 | ~~comparator exceeded its 2400 s budget~~ **CORRECTED — OOM-killed**, see below |
| Mo-95 ENDF/B-VIII.0-beta | ~~comparator exceeded its 2400 s budget~~ **CORRECTED — same OOM class**, see below |

**The three "failures" were re-examined on 2026-09-20 and two of the three
descriptions above were wrong.**

**B-10 ENDF/B-VIII.0 is not an NJOY failure — it is a format failure, and the
tape is now compared.** NJOY `STOP 77`s with `***error in change***Undefined
law for dlwh block: 0`, from `change` (`acefc.f90:13942`), the routine that
writes Type-1 ASCII. Tested across all four `newfor`/`iopp` combinations:

| `newfor iopp` | rc | outcome |
|---|---|---|
| 1 1 | **77** | `Undefined law for dlwh block: 0` |
| 1 0 | **77** | same |
| 0 1 | **0** | 902 327-byte table |
| 0 0 | **0** | 782 183-byte table |

So `newfor = 1` (the new ACE format, carrying the charged-particle DLWH blocks)
is what NJOY cannot write for this tape; VIII.0 adds an MT=700 triton-production
section that VII.0 lacks. Against a `newfor = 0` reference the tape compares:
**`ntr` 50/50, `nr` 35/35, `ntrp` 38/38, `mtr` and `mtrp` both ok**, ESZ
`6.207e-4` / `8.117e-4` / `2.038e-4`. That comparison is against an **old-format**
reference and is caveated accordingly — the DLW law numbering differs between
the two formats — but the inventory and ESZ results above are directly
comparable.

**A method note, because the first answer here was wrong.** The four-variant
test initially reported all four as succeeding, because it checked whether
`tape24` was non-empty. `change` writes progressively, so a failed run leaves a
**truncated but non-empty** file. Checking the exit status instead reversed two
of the four rows. A file that exists is not a file that is finished.

**Fe-57 and Mo-95 are OOM kills, not timeouts.** Re-run with a 4-hour budget,
Fe-57 died with the kernel recording:

```
Memory cgroup out of memory: Killed process 1591 (ace_vs_njoy2016)
  total-vm:20129492kB, anon-rss:13723352kB
```

**13.7 GB resident, 20.1 GB virtual**, on a 15 GB container — and it took a
concurrently running `rustc` down with it. It never reached the first report
section, so the memory is consumed in **this port's own reconstruction**, not
in the ACE comparison or the photon blocks (an earlier guess that
`append_photon_blocks` was responsible is therefore wrong). Mo-95 was last seen
at 51 % of memory and climbing, which is the same class. These two are
**unmeasured, and now for a specific and actionable reason**: unbounded memory
growth in the `LRF=7` reconstruction path.

**The tolerance does not bound it, which is the discriminating result.** The
comparator gained a `--tol` flag so a tape can be compared with *both* codes at
a looser reconstruction tolerance — relaxing the input symmetrically, never the
comparison criterion. Fe-57 was then run down a tolerance ladder:

| tape | RECONR tolerance (both codes) | outcome |
|---|---|---|
| Fe-57 | 0.001 | **OOM-killed**, 13.72 GB anon-rss |
| Fe-57 | 0.01 | **OOM-killed** |
| Fe-57 | 0.1 | **OOM-killed** |
| Mo-95 | 0.001 | killed at the 2400 s budget, 51 % of memory and climbing |
| Mo-95 | 0.1 | **OOM-killed**, 13.96 GB anon-rss, 21.36 GB total-vm, 72 min CPU |

Kernel records for both, which is what makes this a measurement rather than an
inference:

```
Killed process 5794 (ace_vs_njoy2016) total-vm:20443892kB, anon-rss:13857792kB
Killed process 6339 (ace_vs_njoy2016) total-vm:21358092kB, anon-rss:13962948kB
```

A hundredfold relaxation does not save Fe-57, and Mo-95 dies at the loosest
tolerance tried, while **NJOY2016's own `reconr` on the Fe-57 tape reports
`0.0s`**. Both tapes converge on the same ~13.7-14.0 GB ceiling, which is the
container's, not the algorithm's — the algorithm has no ceiling. So this is not "a big problem that needs a
bigger machine" — it is a defect in this port's resolved-resonance
reconstruction on `LRF=7`, and no amount of container memory would make the
comparison meaningful.

**A candidate mechanism, stated as a candidate.** `src/reconr/mod.rs` caps
resolved-resonance bisection at `RES_MAX_BISECTION_DEPTH = 64`, which is no
practical bound at all — the cap exists only to guard "a pathological
`delta_at`", with real termination expected from the significant-figure test
(`reconr.f90:2374-2379`) that the port reproduces. Two sibling caps in the same
crate are 50 (`linearize.rs`) and 10 (`urr.rs`). If the significant-figure test
does not fire on this evaluation, depth 64 permits astronomically many points.
**This has NOT been confirmed** — no instrumented run has shown the achieved
depth — and no constant was changed on the strength of it.

**Both timeouts are `LRF=7` tapes, and so is the worst ESZ outlier.** Seven
tapes in the set use `LRU=1 / LRF=7`: Cl-35, Fe-54, Fe-57, Cu-63, Cu-65, Sr-88,
Mo-95. Zero of the 50 non-`LRF=7` tapes timed out.

**That correlation is recorded with its counter-examples, not as a cause.**
Four of the seven (Fe-54, Cu-63, Cu-65, Sr-88) complete and agree to better
than `1e-3`. It is **not a size effect**: Fe-57 has the *fewest* MF=2/MT=151
cards of the seven (162) and timed out, while Cu-63 has the most (1133) and
finished clean. And no header flag separates them — all seven are `KRM=3`
(Reich-Moore), and `NJS` does not split them either (Fe-54 `NJS=5` clean
against Fe-57 `NJS=5` timeout). Localising this needs a dynamic profile, which
has not been done.

### What this sweep does NOT establish

- **It is 0 K only.** Doppler broadening is verified on the three uranium
  isotopes (above), not across the sweep.
- **It compares three ESZ columns, `MTR`, `NXS(5)`, the photon block and the
  presence of `NU`.** It does not compare AND/DLW contents, `LQR`, `TYR`, or
  the heating column.
- **Agreement with NJOY2016 is not validation.** It says this port reproduces
  NJOY2016, not that either describes a reactor.
- **`CMPFAIL` is a budget, not a verdict.** Fe-57 and Mo-95 are unmeasured, not
  known-bad.

### Reproducing

The driver is `scripts/` — not committed, it is nine lines of shell — but the
per-tape command is the whole of it:

```bash
# NJOY2016 side: reconr + acer at 0 K on tape20, ACE to tape24
# then, for each tape:
cargo build --release -p njoy-outram-park-fork --example ace_vs_njoy2016
./target/release/examples/ace_vs_njoy2016 <njoy.ace> --mat <MAT> --temp-k 0 \
    --tape <name>.endf
```

Each run ends in one machine-readable `SUMMARY` line, which is what the tables
above are built from.

## All 70 tapes accounted for (2026-09-20)

The sweep above covers the 57 `n-*` incident-neutron tapes.
`reference-data/endf/` holds **70 `.endf` tapes** in total (75 files, less
`README.md`, two `.generator.py`, `screen_endf_flags.py` and a `.readme`). The
other 13 are a different problem in each case, and every one is now resolved
with evidence rather than left unstated.

| group | n | `NSUB` | outcome |
|---|---|---|---|
| incident neutron, `n-*` | 57 | 10 | **55 compared** (54 in the sweep + B-10 VIII.0, above); **2 not comparable** — Fe-57 and Mo-95, OOM at every tolerance tried |
| `synthetic-caseb-lfw1` | 1 | **10** | a neutron tape the `n-*` glob missed — **compared at RECONR level**; NJOY itself cannot ACE it |
| thermal `tsl-*` | 9 | 12 | **all 9 compared** — see `acer_thermal_vs_njoy2016.md` |
| photoatomic | 2 | 3 | **both compared** against NJOY GAMINR goldens |
| incident alpha, He-4 | 1 | 20040 | **not comparable** — different ACE class |

**Final tally: 67 of 70 tapes carry a comparison against NJOY2016**, and all
70 are resolved.

| | count |
|---|---|
| incident-neutron ACE compared | **55** |
| thermal ACE compared | **9** |
| photoatomic compared (GAMINR) | **2** |
| RECONR-level compared (`synthetic-caseb-lfw1`) | **1** |
| **carrying a comparison** | **67** |
| not comparable — different ACE class (He-4) | 1 |
| not comparable — this port's reconstruction OOMs (Fe-57, Mo-95) | 2 |
| **total** | **70** |

The three without a number are not gaps in the sweep: He-4's is *settled*
(NJOY writes a `2004.00a` table, this port has no incident-charged-particle
class, so the correct output is "no number"), and Fe-57/Mo-95's is a measured
defect with a tolerance ladder behind it.


### `synthetic-caseb-lfw1.endf` — NJOY cannot ACE it either

It is `NSUB = 10` (incident neutron) but carries **MF=1, 2, 3 only**; it is a
generated RECONR test material for the unresolved `LRU=2/LRF=1/LFW=1` Case-B
branch. ACER stops on it with NJOY's own words:

```
***error in findf*** mat9998 mf 4 mt  0 not on tape 20
```

so the limit is upstream's, not this port's. The tape **is** compared against
NJOY, at the level it supports: `tests/reconr_mt152_all_unresolved_cases_vs_njoy2016.rs`
checks our MF=2/MT=152 against NJOY's committed 16-energy PENDF, and
`case_b_stored_values_match_njoys_own` passes.

### The two photoatomic tapes are compared through GAMINR, not ACER

~~`src/acer/README.md` already records the dosimetry/photoatomic/photonuclear
ACE classes (`acedo`/`acepa`/`acepn`) as unported, so there is no ACE table to
difference.~~ **CORRECTED 2026-09-21** — `acepa` is now ported, and both
photoatomic tapes **are** differenced as ACE tables against NJOY's own
`acer iopt=4` output: byte-identical on the synthetic Z=6 tape, and 71 781 of
71 807 words at print precision on U. See
`acer_photoatomic_vs_njoy2016.md`. Dosimetry (`acedo`) and photonuclear
(`acepn`) remain unported. The GAMINR comparison below stands as written and is
now one of two independent routes through these tapes:
`njoy_gaminr_u_photoatomic_matches_gamout` and
`njoy_gaminr_synthetic_z6_matches_gamout` both pass.

### He-4 is a different ACE class, and the ZAID says so

`a-002_He_004-ENDF8.0.endf` is `NSUB = 20040`, incident alpha: MF=1/451,
MF=3/MT=2 and MF=6/MT=2, nothing else. NJOY processes it happily and writes a
**`2004.00a`** table. This port's ACER writes the incident-neutron class, so the
same tape through our pipeline gives **`2004.00c`** — and the tables disagree
structurally, as they must:

| | ours | NJOY |
|---|---|---|
| ZAID | `2004.00c` | **`2004.00a`** |
| `NXS(1)` LEN_XSS | 145 | 674 |
| `NXS(3)` NES | 29 | 51 |
| MTR/LQR/TYR/LSIG/SIG | absent | present |

There is no common table to difference, so **no number is reported for this
tape**. The ZAID class difference is the evidence; incident-charged-particle
ACER is simply not ported.

## What is NOT covered

- ~~**Doppler broadening** — see above. Zero verified points. The single largest
  gap in this file.~~ **CORRECTED 2026-09-20, same day** — superseded by the
  section above before this file was first committed. Doppler broadening **is**
  verified, on all three nuclides, at 3-8e-4 on the shared points *below*
  `thnmax`, and gated by `tests/acer_broadening_vs_njoy2016.rs`. The struck
  sentence is kept because it is what motivated building the band-integral
  instrument that produced the result.
- **Photon production.** `NXS(6)` ours 0 against NJOY's 583 (U-235), 358
  (U-238), 6 (U-234). Neutron transport does not need it.

  **CLOSED 2026-09-20.** Verified on all three nuclides:

  | nuclide | `NTRP` ours / NJOY | MTRP set | MFTYPE + LAW |
  |---|---|---|---|
  | U-234 | **6 / 6** | identical | 6 of 6 agree |
  | U-235 | **583 / 583** | identical | 583 of 583 agree |
  | U-238 | **358 / 358** | identical | 358 of 358 agree |

  Three separate sources feed the block, and all three had to be right:

  1. **MF=12 `LO=1`** yields and **MF=13** cross sections — the direct forms.
  2. **MF=12 `LO=2`** transition-probability cascades. A reaction MT=51+n
     leaves the nucleus in level *n*, and every photon emitted on the way down
     belongs to that reaction — which is why MT=52 yields two lines and MT=54
     four, and how U-235 reaches 583 entries from ~40 levels. **`TP` and `GP`
     do different jobs:** `TP` carries the population downward, `GP` is the
     fraction of those decays emitting a photon rather than an
     internal-conversion electron, so it scales the yield only. U-238 proves
     the distinction — its first level is almost entirely converted
     (`GP = 1.639e-3`), which is exactly why NJOY's `52002` repeats `51001`'s
     tiny yield while the population through that level is 1.0.
  3. **MF=6 with `ZAP = 0`** — several evaluations give the continuum
     reactions' photons as a secondary particle instead (U-238's MT=5, 16, 17,
     91, 102, 649), written as `MFTYPE = 16`. Upstream calls this "move any
     MF=6 photon production".

  **Entry ORDER is part of the answer, not a detail.** NJOY emits every MF=12
  entry, then every MF=13, then every MF=6 — verified against U-238, where the
  six MF=6 entries occupy positions 352-357 of 358. `LSIGP` and `LDLWP` are
  positional, so a table with the right MTs in the wrong order pairs each entry
  with the wrong cross section and the wrong distribution. The gate therefore
  asserts the MTRP block **in order**, against a committed oracle, not as a set.

  Still not covered: anisotropic photons (MF=14 with `LI = 0`). Every uranium
  evaluation here is `LI = 1`, and the builder refuses `LI = 0` outright rather
  than writing an isotropic approximation.

  ~~**Scoped and deliberately not started**, because a half-written photon
  block is worse than none.~~ The reasoning stands and is why the refusal path
  exists at all: `NXS(6) = 0` is a *legal* ACE table meaning
  "carries no photon production"; a partial one is a malformed table that a
  reader cannot recover from. What it would actually take:

  | piece | state today |
  |---|---|
  | MF=13 photon production cross sections | **parsed** (`photon.rs`) |
  | MF=12 LO=1 yields | **parsed** (`photon.rs`) |
  | MF=15 continuum spectra | only the **first moment** (mean E_γ) is taken; the full `g(E→E_γ)` is not retained |
  | MF=14 photon angular distributions | **not parsed at all** |
  | MF=12 LO=2 transition-probability cascades | **not parsed**, and explicitly skipped today |
  | `jxs` constants for GPD/MTRP/LSIGP/SIGP/LANDP/ANDP/LDLWP/DLWP | **absent** — the locator names do not exist in `acer::jxs` |
  | the eight blocks + their locator arithmetic | **absent** |
  | MTRP's `MT·1000 + line` numbering (how U-235 reaches 583 entries) | **absent** |

  **The table above is the COST ESTIMATE as it stood before the work, kept as
  the record of what was actually built. Every "absent"/"not parsed" row in it
  is now done** — `acer/photon_blocks.rs` parses MF=12 LO=1 and LO=2, MF=13 and
  MF=6 ZAP=0; `acer::jxs` carries MTRP/LSIGP/SIGP/LANDP/ANDP/LDLWP/DLWP;
  `append_photon_blocks` writes all seven with their locator arithmetic; and the
  `MT*1000 + k` numbering is gated in order against a committed oracle. Read it
  as history, not as current state.

  So this was a larger increment than everything else closed on this date put
  together, and it is the one gap where no partial result can be verified: an
  ACE reader either gets a self-consistent photon block or it gets a broken
  table. It is listed here with its parts so the next session can cost it,
  rather than being described as nearly done.
- **Grid construction differs by rule.** `acelod` takes the ACE grid straight
  off MF=3 MT=1 of the PENDF (`acefc.f90:5343`); this port builds the union of
  elastic and every stored partial. At 0 K these nearly coincide (+1.5 %)
  because RECONR already unionised every MT. After BROADR, which thins each MT
  separately, ours is **67–89 % denser**:

  | nuclide | our MF=3 MT=1 | our ACE grid | NJOY's ACE grid |
  |---|---|---|---|
  | U-234 | 16 560 | 36 897 | 25 393 |
  | U-235 | 50 100 | 143 789 | 76 027 |
  | U-238 | 103 244 | 284 415 | 155 207 |

  Note NJOY's grid is **not** simply its MT=1 either — its ACER reads PURR's
  output, not BROADR's. So "switch to MT=1" is *not* a demonstrated fix and was
  not applied.
- ~~**No temperature other than 293.6 K**, and no nuclide outside these
  three.~~ **CORRECTED 2026-09-20** — the temperature half stands (293.6 K and
  0 K only), but the nuclide half does not: the sweep section above compares
  **54 of the 57** incident-neutron tapes in `reference-data/endf/`, spanning
  masses 1-239 and five libraries, at 0 K. What that sweep does *not* cover is
  listed in its own "What this sweep does NOT establish" subsection.
- **Nothing here is validation.** Agreement with NJOY2016 says this port
  reproduces NJOY2016, not that either describes a reactor.

## Reproducing

```bash
cargo build --release -p njoy-outram-park-fork --example ace_vs_njoy2016
zcat reference-data/ace/reference-njoy/endf-b-viii.0/293.6K/U235.ace.gz > /tmp/U235_293.ace
./target/release/examples/ace_vs_njoy2016 /tmp/U235_293.ace --mat 9228 --temp-k 293.6
```

`--mat` 9225 (U-234), 9228 (U-235), 9237 (U-238); `--temp-k 0` for the
RECONR-only 0 K tables under `reference-njoy/endf-b-viii.0/0K/`. The gate that
runs in the ordinary test suite is
`cargo test --release -p njoy-outram-park-fork --test acer_ce_esz_vs_njoy2016`
(U-235 at 0 K, ~136 s).
