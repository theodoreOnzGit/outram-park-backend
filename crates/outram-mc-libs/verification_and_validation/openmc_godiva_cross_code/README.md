# Godiva cross-code verification against OpenMC

**Question this answers.** `outram-mc-libs` reproduces ICSBEP HEU-MET-FAST-001
(Godiva) to `+247 ± 43 pcm`. Is that gap *our transport*, *our data processing*,
or *the evaluation itself*? Measuring against an experiment cannot tell them
apart. Running OpenMC on the **same evaluation, processed by the same NJOY
build**, can.

**Answer (2026-09-15): it is almost entirely ours.**

| arm | seeds | mean Δk vs ICSBEP | sem |
|---|---|---|---|
| **OpenMC 0.15.3, probability tables OFF** | 12 | **−3 pcm** | ±27 |
| OpenMC 0.15.3, probability tables ON | 8 | +76 pcm | ±29 |
| **`outram-mc-libs`, surface tracking** | 16 | **+247 pcm** | ±43 |
| `outram-mc-libs`, surface tracking | 8 | +273 pcm | ±50 |
| `outram-mc-libs`, **delta (Woodcock)** tracking | 8 | +283 pcm | ±52 |

- **`outram-mc-libs` − OpenMC (ptables off) = +250 ± 51 pcm — 4.9 sigma.**
- ENDF/B-VIII.0, processed here, puts Godiva at **k = 0.99997 ± 0.00027** in
  OpenMC: essentially exactly critical. The evaluation contributes ~0 pcm of
  the bias.

So the `+247 pcm` measured against the experiment is **not** the evaluation
being off and **not** RECONR/BROADR: it is the difference between this crate's
transport and OpenMC's. That is a real, localisable target of about 250 pcm.

## What "the same data" means here, and why it matters

Both codes read the **same three ENDF/B-VIII.0 tapes** from
`reference-data/endf/` (U-234, U-235 released VIII.0, U-238), processed by the
**same NJOY2016 build** vendored at
`crates/njoy-outram-park-fork/upstream_source/NJOY2016` (commit `ac5adf5f`,
`vers 2016.79`):

- `outram-mc-libs` runs its own RECONR + BROADR through
  `Nuclide::from_endf_file`.
- OpenMC reads ACE produced by NJOY's RECONR → BROADR → PURR → ACER
  (`make_ace.sh`), converted to HDF5 by `build_data.py`.

Geometry, the three ICSBEP atom densities, radius `8.7407 cm`, temperature
`293.6 K`, 5000 histories per generation and the 40-inactive/120-active split
are identical on both sides. **The transport is the only thing that differs.**

This is what makes the `+250 pcm` attributable. Had OpenMC been fed a
pre-built library (a different evaluation, a different processing chain, a
different temperature grid), the comparison would have folded data differences
back in and answered nothing.

## Probability tables: a prediction, tested

`outram-mc-libs` has **no unresolved-resonance probability tables at all** — its
chain is RECONR + BROADR only, so the unresolved range is treated at infinite
dilution. OpenMC has them, so leaving them on would not be a like-for-like
comparison. `godiva.py` therefore takes `--ptables`, and the run above is done
both ways from one dataset.

That also prices the gap. **Measured: +79 ± 40 pcm (2.0 sigma)** — turning URR
self-shielding on **raises** k.

The sign was predicted in advance, and it is the unwelcome direction: adding
probability tables to this crate would move it from `+250` to roughly `+330`
pcm against OpenMC, i.e. **further from the benchmark, not closer**. It remains
worth implementing for correctness; it is not a route to better Godiva
agreement, and anyone reaching for it as one should know that first.

**A caution on how this number was reached.** At 3 seeds per arm the `ON` mean
read *below* `OFF`, which would have reversed the conclusion. The 4th seed
flipped it and 8 settled it at 2.0 sigma. Godiva's seed-to-seed spread is
`sd ≈ 85–170 pcm` depending on the code, so **no single run, and no three-run
sample, can resolve an effect this size.** The same trap is recorded in
`crates/outram-mc-libs/CLAUDE.md` for a single-seed `+57 pcm` that turned out to
be a −0.97 sigma draw.

## Delta tracking and surface tracking

Verified separately, in `examples/godiva_delta_vs_surface_tracking.rs`: Woodcock
and surface tracking are two estimators of the *same* eigenvalue, so they must
agree within combined statistics, and **nothing in that comparison is tunable**.

**Measured (8 seeds per arm, same settings): surface `+273 ± 50 pcm`, delta
`+283 ± 52 pcm`, difference `−10 ± 73 pcm` — 0.1 sigma.** The two transport
methods in this crate agree with each other essentially exactly, and both agree
with the 16-seed surface ensemble above. Whatever the `+250 pcm` against OpenMC
is, it is therefore **common to both tracking methods** — which rules out a
whole class of explanation (a boundary-crossing bug, a flight-sampling bug, a
majorant defect) and points at the shared physics: cross-section lookup,
reaction partition, and secondary energy-angle sampling.

This required `DeltaDomain::SphereVacuum`, added the same day. Every delta
domain variant had been reflective — correct for the pebble-bed `k_inf` work the
module was written for, where the cell boundary *is* the model, but it meant
delta tracking could not express a bare critical assembly at all, and so had
never been checked against the surface path on this case.

## Reproducing it

Needs: the NJOY2016 build in `upstream_source/`, OpenMC (Python API + the C++
solver), HDF5.

```sh
export ENDF_DIR=../../../../reference-data/endf
export NJOY=../../../njoy-outram-park-fork/upstream_source/NJOY2016/build/njoy
export ACE_DIR=$PWD/ace WORK_DIR=$PWD/work

mkdir -p "$ACE_DIR" && cd "$ACE_DIR"
../make_ace.sh 9225 n-092_U_234-ENDF8.0.endf U234
../make_ace.sh 9228 n-092_U_235-ENDF8.0.endf U235
../make_ace.sh 9237 n-092_U_238.endf        U238
cd ..

python3 build_data.py                     # ACE -> HDF5 + cross_sections.xml
for s in $(seq 1 12); do python3 godiva.py --seed=$s; done            # ptables off
for s in $(seq 1 8);  do python3 godiva.py --ptables --seed=$s; done  # ptables on
```

and the two Rust arms:

```sh
cargo run --release -p outram-mc-libs --features endf-pebble-cases \
    --example godiva_delta_vs_surface_tracking          # delta vs surface
OUTRAM_GODIVA_SEEDS=16 cargo run --release -p outram-mc-libs \
    --features endf-pebble-cases --example godiva_mf6_continuum_ensemble
```

The ACE files are ~270 MB in total and are **not** committed; regenerate them.

## Scope, and what this is not

**Verification, not validation.** Everything here compares codes to each other
on identical data. The only comparison to an *experiment* is the ICSBEP value
the table is quoted against, and that comparison is the subject of
`examples/godiva_keff_endf_local.rs`, not of this study.

One geometry, one material, one temperature, fast spectrum, 0 K resonance
reconstruction broadened to 293.6 K. It says nothing about thermal systems,
about other benchmarks, or about where in the physics the `+250 pcm` lives —
only that it is in the transport rather than in the data. Localising it is the
follow-up, and the crate's own record already shows secondary energy-angle
distributions moving Godiva by `+85` and `−105 pcm` in earlier work, which is
the obvious place to look first.

OpenMC is MIT-licensed; this crate is an independent GPL-3.0 port and is not
affiliated with or endorsed by the OpenMC project. The OpenMC build used here
is release **v0.15.3** (commit `27e38e89`), chosen over `develop` because it is
a tagged release and because `develop` requires Python ≥ 3.12.
