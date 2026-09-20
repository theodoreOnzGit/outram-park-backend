# Thermal S(α,β) ACE tables vs NJOY2016 — all nine `tsl-*` tapes

**Generated:** 2026-09-20, UTC.
**Crate / commit:** `njoy-outram-park-fork` 0.0.3, on
`claude/neutronics-runs-handoff-8xk972`.
**Reference:** NJOY2016 **2016.79** (`ac5adf5`), built from
`upstream_source/NJOY2016/` and run.
**Class:** verification — code-to-code against upstream. **Not validation**:
no experiment is compared against, and no human V&V has been done.
AI-assisted draft.

## Why this record exists

`acer_ce_vs_njoy2016_multi_nuclide.md` covers the incident-neutron
sublibrary. The **thermal** sublibrary is a different ACE class (`…t`) with its
own NXS/JXS and its own blocks, so the continuous-energy comparator cannot read
it. The nine `tsl-*.endf` tapes in `reference-data/endf/` had **no comparison
against NJOY at all** — `tests/thermal_ace.rs` and `tests/thermal_ace_zrh.rs`
assert structure and sign only (`σ_inel ≥ 0`, "not all zero", ascending grid,
block lengths). **The thermal inelastic cross section had never been checked
against any reference value.** This is its first check.

## Methodology

Per tape: NJOY2016 `reconr` → `broadr` → `thermr` → `acer` with `iopt = 2`,
paired with the scatterer's own incident-neutron evaluation. Then
`examples/thermal_ace_vs_njoy2016` builds our table and compares.

Three choices in that deck are load-bearing, and each was a mistake first:

1. **`iwt = 1` on ACER card 9 is required.** `aceth.f90:674-676` reads
   `ifeng = 0; if (iwt.eq.0) ifeng=1; if (iwt.eq.2) ifeng=2`. This port writes
   the equiprobable `IFENG = 0` form only, so the default `iwt = 0` produces a
   **skewed `IFENG = 1`** reference and any number from it would be comparing
   two different representations. The comparator now **refuses** and says so
   rather than printing a figure.
2. **Each tape is run at its OWN base temperature**, read from its MF=7.
   `IncoherentInelastic` holds `S(α,β)` at the base temperature, and the base
   temperatures are not what one would guess: Al-27's is **20 K**, H-in-H₂O's
   is 283.6 K. A first run asked both sides for 293.6 K and produced a
   **~3000× "disagreement"** on Al-27 that was entirely the temperature
   mismatch. That number is discarded, not reported as a defect. The
   comparator now prints the evaluation's `T₀`, `LAT`, `LASYM` and `LLN`
   *before* any comparison, so the inputs are visible.
3. **Ours is built on NJOY's own incident-energy grid**, read from the
   reference's ITIE block, with NJOY's own `NIEB`/`NIL`. This port's thermal
   writer takes its grid from the caller, so choosing our own would measure the
   grid choice rather than the physics.

**Bragg edges are matched by ENERGY, never by index.** The two codes keep
different numbers of edges (Al-27: ours 568, NJOY 309), so differencing by
position would repeat the 1753× artefact that shaped the CE comparator.

## Results

| tape | MAT | T₀ (K) | NXS | inelastic σ, worst rel | coherent-elastic cumulative `S`, worst rel |
|---|---|---|---|---|---|
| Al-27 ENDF/B-VIII.0 | 53 | 20 | **ok** | 2.238e-2 | **3.253e-6** |
| C in SiC | 44 | 300 | **ok** | 7.790e-3 | **4.697e-7** |
| Si in SiC | 43 | 300 | **ok** | 5.985e-3 | **4.697e-7** |
| crystalline graphite | 30 | 296 | **ok** | 7.166e-3 | **4.809e-7** |
| graphite ENDF/B-VII.0 | 31 | 296 | **ok** | 7.170e-3 | **4.906e-7** |
| reactor graphite 10 % porosity | 31 | 296 | **ok** | 8.803e-3 | **4.562e-7** |
| reactor graphite 30 % porosity | 32 | 296 | **ok** | 8.567e-3 | **4.166e-7** |
| H in H₂O | 1 | 283.6 | **ok** | 7.633e-2 | no elastic on this tape |
| H in ZrH ENDF/B-VIII.0 | 7 | 296 | **ok** | 4.737e-1 | incoherent — **not compared** |

`NXS` covers `IDPNI`, `NIL`, `NIEB`, `IDPNC`, `NCL` and `IFENG`: **all six
match on all nine tapes.**

### Coherent elastic is essentially exact

On all six coherent tapes the cumulative `S` agrees to **4e-7 – 3e-6**, and
every NJOY Bragg edge is found in ours to 1e-6 relative (Al-27: 309 of 309;
the others likewise, with ours carrying more edges than NJOY keeps). The extra
edges are a thinning difference, not a disagreement about values.

### The inelastic cross section, and where its two large numbers come from

Seven of nine agree to **≤ 2.3e-2** across the whole grid, the crystalline
solids to **6e-3 – 9e-3**. The two large numbers are **single-point endpoint
effects**, which the sampled points show plainly:

| H in ZrH, E (MeV) | ours | NJOY | rel |
|---|---|---|---|
| 1.0000e-11 | 3.037136e1 | 3.034982e1 | 7.10e-4 |
| 3.5000e-9 | 2.087551e0 | 2.086748e0 | 3.85e-4 |
| 3.9500e-8 | 3.466972e0 | 3.430733e0 | 1.06e-2 |
| 3.2064e-7 | 1.623680e1 | 1.620241e1 | 2.12e-3 |
| **3.7500e-6** | **1.046651e1** | **1.988869e1** | **4.74e-1** |

H-in-ZrH tracks NJOY to 2e-4 – 1e-2 across five decades and then diverges by a
factor 1.9 at the **top of the thermal range** (3.75 eV, against `emax = 4.0`).
H-in-H₂O is the mirror image: its worst is the **lowest** grid point (7.63e-2
at 1e-5 eV) and the rest of the grid is ≤ 1.27e-2.

**The headline column above is still the worst over the whole grid.** An
"interior" statistic excluding the endpoints would read far better and is
deliberately not what is tabulated — choosing the instrument after seeing the
result is the failure this workspace's rules name explicitly. The endpoint
concentration is reported as an observation, supported by the table, not used
to replace the number.

**No cause is established for either endpoint effect.** Both sit at a boundary
of the `calcem` treatment, which is suggestive and is not evidence.

## What is NOT covered

- **ITXE, the equiprobable emission bins, is not differenced.** Its length
  agrees by construction once `NIEB` and `NIL` match, so a length check proves
  nothing about the values. The comparator prints this rather than implying
  coverage it does not have.
- **Incoherent-elastic is not compared** (H in ZrH). Only the coherent
  (`IDPNC = 4`) elastic blocks are differenced.
- **One temperature per tape** — each evaluation's own `T₀`. The
  `IncoherentInelastic` structure carries `S(α,β)` at the base temperature
  only, so no other temperature can be produced from it for the inelastic half.
- **`IFENG = 1` and `IFENG = 2`** (skewed, continuous-tabular) are unported, so
  no reference in those forms is comparable.
- **Nothing here is validation.** Agreement with NJOY2016 says this port
  reproduces NJOY2016, not that either describes a moderator.

## Reproducing

```bash
cargo build --release -p njoy-outram-park-fork --example thermal_ace_vs_njoy2016
# inventory first -- the NJOY deck cannot be written until T0 is known
./target/release/examples/thermal_ace_vs_njoy2016 - --tsl tsl-013_Al_027-ENDF8.0.endf --mat 53
# then, against an NJOY table built with iopt=2 and iwt=1 at that T0:
./target/release/examples/thermal_ace_vs_njoy2016 <njoy.ace> \
    --tsl tsl-013_Al_027-ENDF8.0.endf --mat 53 --temp-k 20 --natom 1
```
