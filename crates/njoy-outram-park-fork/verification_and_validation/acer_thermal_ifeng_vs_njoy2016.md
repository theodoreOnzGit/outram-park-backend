# Thermal `IFENG = 1` and `IFENG = 2` vs NJOY2016

**Date:** 2026-09-22 · **Port:** `src/acer/acesix.rs` (`acesix_tabulated`) and
`src/acer/thermal.rs` (`InelasticForm`) · **Oracle:** NJOY2016 2016.79, built
`-O3`.

## What changed

`acer_thermal_vs_njoy2016.md` covers `IFENG = 0` and says of the other two
forms that the comparator *refuses* to run, because this port wrote the
equiprobable form alone. It now writes all three, chosen by
[`InelasticForm`]. `aceth.f90:674-676` maps ACER card 9's `iwt` onto them:

| `iwt` | `IFENG` | what it is | `NIL` | layout |
|---|---|---|---|---|
| 1 | 0 | `NIEB` bins of equal probability | `nang - 1` | `NEI × NIEB × (nang+1)` |
| 0 | 1 | the same bins weighted `1 4 10 … 10 4 1` | `nang - 1` | identical |
| 2 | 2 | continuous: each incident energy keeps its own points | **`nang + 1`** | `2·NEI` (offset, count) table, then `(E', pdf, cdf, μ₁…μ_nang)` per point |

The first two are one code path with two weight vectors and were already
implemented — only the `NXS(7)` flag was missing, and writing `0` while using
the variable weights would have been a table that lies about its own contents.
The third is a different routine: `acesix_tabulated`.

## Methodology

Per form: NJOY `reconr → broadr → thermr → acer`, changing only `iwt` on
card 9. This port builds the matching table **on NJOY's own incident-energy
grid** — its thermal writer takes the grid from the caller, so choosing ours
would measure the grid choice rather than the physics — and
`examples/thermal_ace_vs_njoy2016` differences every stored word.

```text
thermr / 26 22 23 / 53 1325 16 1 2 1 0 1 229 0 / 20. / .05 4.0
acer   / 20 23 0 30 40 / 2 1 1 .00 / 'al thermal' /
         1325 20. 'al27' 1 / 13027 / 229 16 230 0 1 4.0 <iwt>
```

```bash
cargo run --release -p njoy-outram-park-fork \
  --example thermal_ace_vs_njoy2016 -- <njoy.ace> --tsl <tsl.endf> --mat <MAT> --temp-k <K>
```

The program reads the form off the reference's own `NXS(7)`; it no longer
assumes one.

## Results

| form | tape, T | stored values | agreement |
|---|---|---|---|
| `IFENG = 1` | Al-27, 20 K | 106 × 16 × 17 = 28 832 | `E'` worst **2.358e-3** rel · cosines worst **2.643e-4** abs |
| `IFENG = 2` | Al-27, 20 K | 10 518 points | counts **identical** · CDF worst **6.517e-6** rel |
| `IFENG = 2` | crystalline graphite, 296 K | 36 092 points | counts **identical** · CDF worst **4.365e-6** rel |

`NXS` matches on every run; the table-length difference is the same
coherent-elastic edge thinning already attributed in the `IFENG = 0` record
(568 edges kept against NJOY's 309, so exactly `2 × 259` extra words).

### The continuous form agrees point for point except where the law ends

The point count is **not an input**. It falls out of `acesix`'s panel-merging
threshold (`aceth.f90:392-395`: a panel contributing less than `eps/10 = 1e-6`
of the law is absorbed into the next), so reproducing it is a statement about
the algorithm. Both tapes reproduce it exactly, incident energy by incident
energy — Al-27's 48…212 points and graphite's 239…549 — which is what makes a
**positional** comparison legitimate here, the same argument the `IFENG = 0`
record makes for `ITXE`.

Taken that way:

| | Al-27 | graphite |
|---|---|---|
| `E'`, worst relative | 1.997e0 | 1.853e0 |
| `cdf`, worst relative | **6.517e-6** | **4.365e-6** |
| cosine, worst absolute | 1.554e0 | 1.496e0 |
| points differing in `E'` by > 1e-6 | **106** of 10 518 | **106** of 36 092 |
| `E'` excluding each law's final point | **4.216e-7** | **4.577e-7** |
| cosine excluding each law's final point | **5.0e-5** | **3.9e-5** |

**106 is the number of incident energies on both tapes.** Every disagreement
above round-off is the single final point of a law, where the two codes'
outgoing-energy ranges end — Al-27 incident 5 stores `1.2151e-7 MeV` against
NJOY's `4.055e-8 MeV`, at a point whose density is zero and whose CDF is
already 1 on both sides. The 10 412 interior points agree to **4.2e-7**.

**The headline is the worst over the whole block, endpoints included.** The
breakdown is reported beside it and not instead of it: choosing the instrument
after seeing the result is the failure the root `CLAUDE.md` names by name, and
an "interior-only" figure would read a thousand times better while hiding
where the two codes actually differ.

No cause is established for the endpoint difference. It sits at the top of the
`calcem` outgoing-energy range, which is suggestive and is not evidence, and it
is the same boundary the `IFENG = 0` record already flags for the inelastic
cross section.

### A density may be negative, and upstream's is

`ifeng2_stored_law_is_normalised` bounds the stored density's magnitude rather
than its sign. That is not a loosened gate: **NJOY's own Al-27 `IFENG = 2`
table carries 7 negative densities** out of 10 518, the first `-8.47e-16` at
the final point of incident energy 14, where the closing interpolation is a
difference of nearly equal numbers and the CDF has already reached 1. Ours
dips no further than `-2.24e-38`. Asserting `pdf >= 0` would have failed
against a table NJOY wrote.

## Gates

`tests/acer_thermal_ifeng_vs_njoy2016.rs`:

- `ifeng2_itxe_layout_matches_njoy2016` — all 106 offsets and counts against
  `reference-data/acer/al27_20k_thermal_ifeng2_njoy2016.csv`, which is NJOY's
  own ITXE locator/count table (4 kB, rather than the 4 MB the whole file would
  cost).
- `ifeng2_stored_law_is_normalised` — `E'` ascends, cosines stay in `[-1, 1]`,
  the cumulative is monotone and ends at 1 to 1e-12, and the density's
  negative excursion is bounded.
- `ifeng1_is_skewed_and_declares_itself` — `NXS(7) = 1`, the layout and length
  are unchanged from `IFENG = 0`, and 28 832 of 29 969 `ITXE` words actually
  move. A flag whose value nothing changes is worse than no flag.

## Not covered

- **The full-value `IFENG = 2` comparison is not gated**, only documented and
  reproducible in one command: the reference files are 4.0 MB (Al-27) and
  13.9 MB (graphite). What *is* gated is the layout, which is what a port
  gets wrong.
- **`IFENG = 1` has no committed value oracle** either — its 2.358e-3 / 2.643e-4
  come from the run above. The gate covers the flag, the layout and the fact
  that the weighting bites.
- Only Al-27 and crystalline graphite were run at `IFENG = 2`; the other seven
  `tsl-*` tapes were not.
- **Nothing here is validation.** It says this port writes what NJOY2016
  writes.
