# A nuclide built from NJOY2016's ACE, against one built from the ENDF tape

GitHub **#270**. Measured 2026-09-23 by
`crates/outram-mc-libs/tests/nuclide_from_ace_vs_endf.rs`.

## Why this is a cross-code result and not a round trip

The two construction paths share **no physics code**:

| | ACE route | ENDF route |
|---|---|---|
| processing | **NJOY2016 2016.79 `ac5adf5`** — RECONR+BROADR+PURR+ACER | **this workspace's** RECONR + BROADR |
| reader | `acer::ce_decode` + `acer::ce_laws` | `endf` + `reconr` |
| energy grid | NJOY's adaptive union grid | ours, tolerance 1e-3 |

So an agreement measures **this port's resonance reconstruction and Doppler
broadening against the Fortran's**, on the same evaluation (ENDF/B-VIII.0, the
same tapes NJOY was given per the submodule's `MANIFEST.tsv`). A round trip
through one codec could not say this.

It also closes the gap the HDF5 V&V record named: *"running the same problem
through `outram-mc-libs`' transport for a two-code comparison on one library …
needs a public constructor for a `Nuclide` built from supplied pointwise
data"*. `Nuclide::from_ace` is that constructor.

## Method

Build U-235 and U-238 both ways at 293.6 K and compare microscopic cross
sections at six energies chosen to exercise different physics rather than to be
easy: thermal (1/v capture, fission), epithermal, the **6.674 eV U-238
resonance** (the hardest point for reconstruction *and* for broadening), the
resolved range, the unresolved range, and fast.

## Results

**U-235**

| E [eV] | ACE total | ENDF total | rel | ACE fission | ENDF fission | rel |
|---|---|---|---|---|---|---|
| 2.530e-2 | 7.00116e2 | 7.00114e2 | **+0.00 %** | 5.86635e2 | 5.86633e2 | +0.00 % |
| 1.000e0 | 9.25585e1 | 9.25679e1 | −0.01 % | 6.74243e1 | 6.74310e1 | −0.01 % |
| 6.674e0 | 2.69871e1 | 2.69947e1 | **−0.03 %** | 5.91707e0 | 5.91890e0 | −0.03 % |
| 1.000e3 | 1.90379e1 | 1.90380e1 | −0.00 % | 6.89579e0 | 6.89590e0 | −0.00 % |
| 1.000e5 | 1.21475e1 | 1.21475e1 | −0.00 % | 1.58790e0 | 1.58790e0 | +0.00 % |
| 2.000e6 | 7.24685e0 | 7.24685e0 | −0.00 % | 1.28850e0 | 1.28850e0 | +0.00 % |

**U-238**

| E [eV] | ACE total | ENDF total | rel | ACE fission | ENDF fission | rel |
|---|---|---|---|---|---|---|
| 2.530e-2 | 1.19224e1 | 1.19224e1 | −0.00 % | 1.85097e-5 | 1.85280e-5 | −0.10 % |
| 1.000e0 | 9.57110e0 | 9.57115e0 | −0.00 % | 3.21474e-6 | 3.21907e-6 | −0.13 % |
| **6.674e0** | **7.67552e3** | **7.67578e3** | **−0.00 %** | 3.13106e-3 | 3.13027e-3 | +0.02 % |
| 1.000e3 | 2.14434e1 | 2.14434e1 | −0.00 % | 1.31459e-7 | 1.32328e-7 | −0.66 % |
| 1.000e5 | 1.18051e1 | 1.18051e1 | +0.00 % | 5.39380e-5 | 5.39380e-5 | −0.00 % |
| 2.000e6 | 7.28152e0 | 7.28152e0 | +0.00 % | 5.37860e-1 | 5.37860e-1 | +0.00 % |

**Worst total-cross-section disagreement anywhere: −0.03 %**, at U-235's
6.674 eV point. U-238's 6.674 eV resonance — 7675 barns, the single most
demanding point in either table for both reconstruction and broadening —
agrees to better than 0.005 %.

The largest relative number in the table, U-238 fission **−0.66 % at 1 keV**, is
on a cross section of `1.3e-7 b`. It is numerically irrelevant to any transport
result and is reported rather than hidden, since suppressing it would mean
choosing which disagreements to show.

## Absolute check, independent of both codes

Neither route can be validated by the other alone — two implementations can
share a mistake. Against accepted values:

| quantity | measured | accepted |
|---|---|---|
| U-235 thermal `sigma_f` | 586.635 b | ~585 b |
| U-235 thermal `sigma_t` | 700.116 b | ~698 b |
| U-238 thermal capture | 2.683 b | ~2.68 b |
| U-238 thermal `sigma_t` | 11.9224 b | ~11.9 b |

and `total` closes against `elastic + absorption` exactly at thermal
(14.109 + 686.007 = 700.116).

## A defect the partials could not reveal

The first working `from_ace` reported **`total = 0` at every energy while every
partial was correct**. `xs_at_energy` reads the total as `eval_mt(Mt1Total)`;
RECONR emits an MT=1 section, and **ACE does not list MT=1 in MTR** — the total
lives in the ESZ block. A zero macroscopic total makes distance-to-collision
infinite, so every particle streams straight out and `k` is zero, from data that
is perfect. Now pinned by an explicit assertion.

This is the third ACE convention in this work that looks like a decoder bug and
is not: MT=18 absent on a fissionable nuclide, ESZ absorption excluding fission,
lumps stored beside their own levels, and now the total not being in MTR.

## What this does and does not establish

**Established.** This port's RECONR and BROADR reproduce NJOY2016's on
ENDF/B-VIII.0 uranium to better than 0.03 % in total cross section across six
decades of energy, and `Nuclide::from_ace` builds a transport-ready nuclide from
a foreign library.

**Not established.** No `k` has been computed on the ACE route yet. The
comparison is of cross sections, not of a transport result. Also not decoded on
this path, and recorded as omissions rather than zeros: delayed neutrons, the
unresolved-resonance probability tables (`UNR`), DBRC, and S(α,β).
