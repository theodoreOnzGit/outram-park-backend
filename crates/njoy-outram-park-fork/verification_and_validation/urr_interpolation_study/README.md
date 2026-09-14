# Where a pointwise cross section in the unresolved range comes from, and how accurate it is

**A controlled refinement study of NJOY2016 RECONR's unresolved-range output,
using an independent Rust reimplementation as the comparison arm.**

Date: 2026-09-14 · Tracked: `bn:op-12lu` · Status: **verification, not validation**

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** All code in this workspace is **unverified and untrusted** unless a specific verification & validation (V&V) case demonstrates otherwise. V&V cases are human-reviewed and are intended for journal / arXiv publication — that is the trust workflow. See the workspace `VERIFICATION_AND_VALIDATION.md` and `RESPONSIBLE_USE.md`. Not for nuclear facility operation, reactor control, safety-critical, or licensing decisions.

---

## Abstract

In the unresolved resonance range (URR) an evaluation supplies only *average*
resonance parameters, and a processing code must turn them into pointwise cross
sections. NJOY2016's RECONR does this in two stages: it evaluates the
infinitely-dilute cross sections on an internal grid (`eunr`), stores them as
MF=2/MT=152, and then **interpolates that table** to fill MF=3 at every other
energy its output grid carries.

We show that this second stage is the dominant source of error in RECONR's URR
output for ENDF/B-VIII.0 U-234, and we quantify it by a refinement study in
which **the underlying physics is held provably unchanged**. Refining the
parameter grid by lin-lin resampling — an exact operation, because NJOY forces
the parameter interpolation law to lin-lin — leaves every evaluated point
bit-identical while making `eunr` finer. Under that refinement NJOY's own
output converges onto the values produced by a direct evaluation at each energy,
the relative difference falling from `6.3e-3` to `4.7e-5` at worst, and to
between `4e-10` and `4e-7` at four of the seven energies — at or below the
floor of what NJOY's 7-significant-figure output can express.

The practical consequence is a statement about accuracy that is testable rather
than asserted: at the energies RECONR interpolates, a directly-evaluated value
is closer to the converged answer than RECONR's tabulated one, by up to four
orders of magnitude.

---

## 1. Why this was looked at

The comparison arm is `njoy-outram-park-fork`, a Rust port of NJOY2016. During
its verification, its unresolved-range reconstruction agreed with NJOY2016
**exactly at 26 of the 34 grid points** NJOY writes inside U-234's URR — at the
7-significant-figure floor of NJOY's printed output — and disagreed at the other
8 by up to `6.3e-3` relative.

A port that agrees at 26 of 34 points is not obviously right or obviously wrong.
The disagreements were not randomly placed: `7.0e3 eV` disagreed while
`7.2e3 eV` was exact; `7.0e4 eV` disagreed while `7.2e4 eV` was exact. That
pattern is the finding, and it is what the rest of this note explains.

**The inference that had to be earned.** Agreement at nodes and disagreement
between them shows only that the two codes differ between nodes. It does *not*
establish which is closer to the truth: an implementation that is correct at the
nodes and wrong in between would produce identical evidence. That gap is the
reason for the refinement study in §4 rather than a bare comparison.

---

## 2. What RECONR actually does in the URR

From `reconr.f90`'s own header (l.81-87):

> If unresolved parameters are present, the infinitely dilute cross sections are
> computed on a special energy grid chosen to preserve the required
> interpolation properties. This table is added to the pendf tape using a
> special format in MF2/MT152, and the table is also used to compute the
> unresolved contributions in MF3.

Concretely:

| routine | lines | role |
|---|---|---|
| `genunr` | 1628-1735 | evaluates the dilute cross sections on `eunr`, writes MF=2/MT=152; returns early when `LSSF ≠ 0` (l.1694) |
| `csunr1` / `csunr2` | 3826-4077 / 4079-4317 | the dilute cross-section kernels (Cases A/B and C) |
| `sigunr` | 1737-1769 | **interpolates** the MT=152 table when filling MF=3; called at l.2662 |

`eunr` is built from the energies at which the unresolved parameters are
tabulated, plus the range boundaries shaded by `sigfig(E,7,∓1)`
(`rdf2u0`/`rdf2u1` l.1303, 1380-1400, 1510-1531; boundary shading l.757-775).

For U-234 the evaluation tabulates its unresolved parameters at **10 energies**,
while NJOY's PENDF carries **34 points** inside the URR — the extra ones
contributed by other MF=3 sections' grids. Those extra points are filled by
`sigunr`, i.e. by interpolation.

---

## 3. Baseline

Material: ENDF/B-VIII.0 **U-234**, MAT 9225. Its unresolved range is
`1.5e3 .. 1.0e5 eV`, Case C (energy-dependent parameters), **`LSSF = 0`** — so
MF=3 carries no unresolved background of its own and the PENDF value in this
window is the unresolved cross section itself.

MT=18 (fission) is used throughout as the probe, because U-234's MF=3 MT=18 is
empty below the top of the window, making NJOY's PENDF value there the pure
unresolved cross section with nothing added. (It does carry a point at exactly
`1.0e5 eV`, the upper bound, where the fast-range background begins; every
energy sampled here lies strictly below it.)

Reference: NJOY2016 upstream `ac5adf5f33d893e42f2eed7fb286b0d51c7580da`
(version 2016.79), built with gfortran 13.3.0, CMake 3.28.3, `Release`. Deck:

```
reconr
20 22
'u234 pendf'/
9225 0/
.001/
0/
stop
```

**Baseline result.** Of the 34 NJOY grid points in the URR, 26 agree with the
direct evaluation to `~1e-7`. The other 8 disagree, and each is reproduced by
lin-lin interpolation of NJOY's *own* neighbouring agreeing points:

| E (eV) | NJOY | lin-lin of NJOY's neighbours | rel |
|---|---|---|---|
| 7.00000e3 | 1.228344e-2 | from 6.0e3, 7.2e3 | 1.4e-7 |
| 4.368748e4 | 1.485438e-2 | from 4.0e4, 5.0e4 | 2.9e-7 |
| 4.368749e4 | 1.485437e-2 | from 4.0e4, 5.0e4 | 2.1e-7 |
| 4.51800e4 | 1.447637e-2 | from 4.0e4, 5.0e4 | 2.4e-7 |
| 5.25000e4 | 1.277159e-2 | from 5.0e4, 6.0e4 | 1.4e-16 |
| 7.00000e4 | 1.429696e-2 | from 6.0e4, 7.2e4 | 2.3e-7 |
| 8.00000e4 | 1.669535e-2 | from 7.2e4, 8.5e4 | 2.8e-7 |
| 9.00000e4 | 1.866262e-2 | from 8.5e4, 1.0e5 | 1.2e-7 |

So those 8 are interpolated, not evaluated — consistent with `sigunr`. This
identifies the mechanism but, as noted in §1, does not yet establish accuracy.

### 3.1 Three explanations tested and rejected first

- **A different ENDF interpolation law on the table.** Rejected: no single law
  fits. At `4.368748e4` log-lin came closest for MT=18 (3.7e-4) but lin-log for
  MT=102 (1.3e-3), and neither reproduced NJOY's value.
- **A difference in how the unresolved *parameters* are interpolated.**
  Rejected by reading upstream: `intr` (`unresr.f90:1261-1288`) interpolates
  D/GX/GN0/GG/GF with `terp1(…, int)` where `int` has just been **forced to 2**
  (`l.1057-1058` reads the file's flag and overwrites it). The comparison arm
  does the same.
- **Parameter energies being privileged.** Rejected by the data: `1.7e3`,
  `1.0e4`, `3.0e4` and `5.0e4 eV` are *not* parameter energies and agree
  exactly, while `7.0e3 eV` is not one and is interpolated.

---

## 4. The refinement study

### 4.1 Design

The experiment refines NJOY's internal grid **without changing the physics**.

Because NJOY forces the unresolved-parameter interpolation law to lin-lin
(§3.1), inserting lin-lin-interpolated parameter points is an **exact**
operation: the parameter functions `D(E)`, `GX(E)`, `GN0(E)`, `GG(E)`, `GF(E)`
are pointwise unchanged. What does change is `eunr`, which is built from those
energies — so NJOY evaluates its kernel at more energies and interpolates at
fewer.

`densify.py` (in this directory) rewrites the MF=2/MT=151 unresolved section
with `n-1` inserted points per interval, updates each J-state's `NPL`/`NE`, and
updates the MF=1/MT=451 dictionary record count for MF=2/151.

> **Practical note for anyone repeating this.** Omitting the dictionary update
> does not produce a clean error: NJOY reads past the section and aborts with
> `malloc(): invalid size (unsorted)`. The record counts are load-bearing.

Runs: `x2`, `x4`, `x8`, `x16` (10 parameter energies → 19, 37, 73, 145).

### 4.2 Control — the physics is unchanged

At every original parameter energy, for every refinement level, NJOY's output is
**bit-identical to the baseline**:

| refinement | worst change at an original parameter energy |
|---|---|
| x2 | 0.00e+00 |
| x4 | 0.00e+00 |
| x8 | 0.00e+00 |
| x16 | 0.00e+00 |

Across all 45,954 grid points the two runs share, the median and 95th-percentile
relative difference are both exactly `0.00e+00`; only the interpolated tail
moves (max `6.0e-3`). The refinement therefore changes the grid and nothing
else, which is what makes §4.3 interpretable.

### 4.3 Result — NJOY converges onto the directly-evaluated values

Relative deviation of NJOY's MF=3 MT=18 from the direct evaluation:

| E (eV) | base | x2 | x4 | x8 | x16 |
|---|---|---|---|---|---|
| 7.00000e3 | -9.00e-4 | -2.07e-3 | -5.41e-4 | -1.32e-4 | **-3.33e-5** |
| 4.368748e4 | 5.80e-3 | 5.80e-3 | 1.29e-3 | 4.01e-4 | **1.90e-5** |
| 4.51800e4 | 6.25e-3 | 6.25e-3 | 2.02e-4 | 1.01e-4 | **4.70e-5** |
| 5.25000e4 | 3.70e-3 | 3.70e-3 | 1.31e-3 | ~0 | **4.29e-10** |
| 7.00000e4 | -4.62e-3 | -4.62e-3 | ~0 | ~0 | **8.59e-8** |
| 8.00000e4 | -5.69e-3 | ~0 | ~0 | ~0 | **-1.86e-7** |
| 9.00000e4 | -4.71e-3 | -9.80e-3 | -5.3e-7 | -5.3e-7 | **-3.53e-7** |

The baseline column and the `x2`-`x8` columns come from `converge.py`, which
compares against this crate's values **rounded to the 7 figures NJOY prints**;
entries shown there as `~0` were exactly `0.00e+00` at that precision. The
`x16` column is computed by the Rust gate
(`tests/reconr_urr_njoy_converges_to_ours.rs`) against the *unrounded* kernel
output, which is why it resolves values like `4.29e-10` and `8.59e-8` where the
script reported zero.

**That distinction matters and the weaker number is the one to quote.** The
script's zeros are an artefact of rounding the comparison arm, not evidence of
exact agreement. The honest statement is that after 16-fold refinement the
worst remaining deviation is `4.7e-5` and four of the seven energies sit
between `4e-10` and `4e-7` — at or below the floor of what NJOY's 7-figure
output can express. The rest fall by factors of 27 to 300.

**Convergence is not monotone at every point.** `7.0e3` and `9.0e4` get *worse*
at `x2` before improving, because inserting points changes which neighbours
bracket the target energy; a coarse grid can be accidentally favourable. This is
reported rather than smoothed — a monotone table would be more persuasive and
would also be less honest.

---

## 5. What this does and does not establish

**Establishes.** At the energies RECONR fills by interpolating its MT=152 table,
a direct evaluation of the unresolved cross section is closer to the converged
answer than RECONR's tabulated value, for this material and this reaction. The
evidence is that RECONR's *own* output moves onto the directly-evaluated values
under grid refinement, with the physics held provably fixed.

**Does not establish.**

- **Not a general claim about NJOY.** One material (U-234), one reaction
  (MT=18), one range, `LSSF = 0`, infinite dilution, 0 K. Nothing here bears on
  NJOY's resolved-range reconstruction, its self-shielded URR treatment
  (UNRESR/PURR), or any other module.
- **Not validation.** No measurement is involved. This is code-to-code
  verification plus an internal convergence study.
- **Not an error bound on RECONR's intended use.** RECONR's URR output is an
  infinitely-dilute table; the physically meaningful quantity in most
  applications is the self-shielded cross section from UNRESR or PURR, where
  this interpolation is not the dominant term.
- **Magnitude.** The effect is `≤ 6.3e-3` relative on one reaction of one
  nuclide. Whether it matters anywhere depends on the application; that has not
  been assessed.

---

## 6. Reproducing

```bash
# 1. Build NJOY2016 at the pinned commit
git clone https://github.com/njoy/NJOY2016.git && cd NJOY2016
git checkout ac5adf5f33d893e42f2eed7fb286b0d51c7580da
cmake -S . -B build -DCMAKE_BUILD_TYPE=Release && cmake --build build

# 2. Baseline
mkdir -p run_base && cp n-092_U_234-ENDF8.0.endf run_base/tape20
cd run_base && ../build/njoy < ../u234-ENDF8.0-0K-err0.001.njoy-input

# 3. Refined runs (densify.py is in this directory)
for n in 2 4 8 16; do
  python3 densify.py n-092_U_234-ENDF8.0.endf u234_x$n.endf $n
  mkdir -p run_x$n && cp u234_x$n.endf run_x$n/tape20
  (cd run_x$n && ../build/njoy < ../u234-ENDF8.0-0K-err0.001.njoy-input)
done

# 4. Control and convergence
python3 control.py     # must print 0.00e+00 at every parameter energy
python3 converge.py
```

The deck is committed at
`reference-data/reconr/u234-ENDF8.0-0K-err0.001.njoy-input`; the baseline PENDF
at `reference-data/reconr/u234-ENDF8.0-0K-err0.001.pendf`.

The comparison arm's values are produced by
`crates/njoy-outram-park-fork/tests/reconr_urr_kernel_vs_njoy2016.rs`, which
asserts the §3 result, and `reconr_u234_unresolved_njoy_golden.rs`.

> **On the scripts being Python.** This workspace's `CLAUDE.md` bars Python for
> documentation and repository accounting. This is neither — it is a one-off
> experiment that manipulates an ENDF tape — and keeping it in Python keeps the
> reproduction bar at "run a script" rather than "build a 40-crate Rust
> workspace". If this is submitted, that choice should be revisited with the
> maintainer.

---

## 7. Provenance

- **NJOY2016**, Los Alamos National Laboratory, upstream commit `ac5adf5f`
  (2016.79). Modified BSD 3-Clause (LANL/DOE).
- **ENDF/B-VIII.0** U-234 (MAT 9225) and the ENDF-6 format specification
  (ENDF-102). Openly published evaluated data.
- **`njoy-outram-park-fork`**, the Rust comparison arm, GPL-3.0-only.

All data used here is open, publicly released evaluated nuclear data. No
restricted, proprietary, or operational data is involved
(`DATA_POLICY.md`).

**AI-assisted work.** The comparison arm, this study and this document were
produced with AI assistance and are untrusted draft material pending human
review (`AI_USAGE.md`, `RESPONSIBLE_USE.md`). The numbers reported here were
produced by running the commands in §6; none is estimated or recalled.
