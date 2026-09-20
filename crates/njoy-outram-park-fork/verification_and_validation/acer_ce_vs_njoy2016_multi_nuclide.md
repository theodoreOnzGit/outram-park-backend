# ACER continuous-energy tables vs NJOY2016 — U-234, U-235, U-238

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
| fission ν̄ (NU block) | `JXS(2) = 0` — no fission source at all | **347/347 values bit-identical** |
| fission representation | MT=18 always kept, partials always dropped | **follows upstream's `mt19` rule** |
| nuclide coverage | U-235 | **U-234, U-235, U-238** |
| temperature coverage | 0 K | **0 K and 293.6 K** |

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

**Doppler broadening is NOT verified against NJOY2016. Not to 1e-6, not at
all.**

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
broadening needs a different instrument** — a grid-independent one (band
integrals), or a comparison against a reference evaluated at our own energies.
That is open work, and until it is done no claim about this port's Doppler
broadening against NJOY is supported by anything in this file.

## Results — reaction inventory

| nuclide, T | NTR ours/NJOY | NR ours/NJOY | verdict |
|---|---|---|---|
| U-235, 0 K | **84 / 84** | **44 / 44** | identical sets, zero difference either direction |
| U-234, 293.6 K | **49 / 49** | **48 / 48** | every NJOY MT present, producer sets match |
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

**347 of 347 values bit-identical** to NJOY2016's, header included
(`[-173, 2, 0, 85, …]` both sides), for U-235 at 0 K.

Exact equality is the right bar, not a tolerance: the block is the
evaluation's own table carried through with one unit change (eV → MeV), so
anything short of bit-identical would mean a real difference in how it is read
or written. Pinned by `tests/acer_ce_esz_vs_njoy2016.rs` against
`reference-data/acer/u235_0k_nu_njoy2016.csv`.

**One deliberate divergence from upstream, unverified.** For `LNU=1` NJOY
writes `nut(i+2)*emev*(i-1)` — emev *times* `(i-1)`, not *raised to* it. The
unit change requires the power; the two agree for the first two coefficients
and diverge from the third on. This port uses the power. **No tape in
`reference-data/` uses LNU=1** — U-234/235/238 are all LNU=2 for MT=452/455/456
— so the branch is unexercised and must not be described as matching NJOY.

## What is NOT covered

- **Doppler broadening** — see above. Zero verified points. The single largest
  gap in this file.
- **Photon production.** `NXS(6)` ours 0 against NJOY's 583 (U-235), 358
  (U-238), 6 (U-234). Neutron transport does not need it.

  **Scoped 2026-09-20 and deliberately not started**, because a half-written
  photon block is worse than none. `NXS(6) = 0` is a *legal* ACE table meaning
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

  So this is a larger increment than everything else closed on this date put
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
- **No temperature other than 293.6 K**, and no nuclide outside these three.
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
