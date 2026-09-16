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

> ## Outcome
>
> **That target was localised, fixed, and re-measured.** The `~181 pcm` leakage
> share was the discrete inelastic angular distributions, never sampled at all;
> wiring in the evaluation's own ENDF MF=4 for MT=51…90 (`op-tm9f`) moved Godiva
> **from `+214` to `+16 ± 11 pcm` over 256 seeds** — a `−198 pcm` move against
> `−168 / −181 / −219 pcm` predicted three ways *before* the work. The
> remaining offset is inside the ICSBEP experiment's own ±100 pcm band.
>
> **The study is not finished.** The `~69 pcm` spectral share lives in `k_inf`,
> is untouched by that fix, and our spectrum is still 0.45 % harder than
> OpenMC's (`op-os8x`). Agreement on `k` is not agreement on the physics — two
> offsetting errors land on the right `k` too. What makes this one credible is
> that the predicted mechanism moved `k` by the predicted amount in the
> predicted direction, not that the final number is small.
>
> **The diagnosis was checked, not just the answer.** `k_inf` has no leakage, so
> a leakage fix must leave it alone — it moved `+13 ± 71 pcm` (0.2 sigma) while
> `k_eff` moved `−198`. And a direct ablation prices the anisotropy at
> `−224 ± 44 pcm` (5.1 sigma), with its control arm independently reproducing
> the old `+214`.
>
> Details in *Discrete inelastic angular anisotropy* below.

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

That also prices the gap. **Measured over 16 seeds per arm: +43 ± 38 pcm —
1.1 sigma, consistent with ZERO.**

**A superseded number, kept on the record.** An earlier 8/12-seed read gave
`+79 ± 40 pcm (2.0 sigma)` and was written up here as confirming a predicted
positive sign. With 16 seeds per arm it is `+43 ± 38` and the sign is **not
established**. Do not quote the +79. Pairing the arms by seed does not rescue
it either: OpenMC's RNG stream diverges as soon as the physics differs, so
same-seed runs are uncorrelated — the paired difference has sd 170 pcm, *larger*
than either arm's. Resolving a ~43 pcm effect at 3 sigma from here would need
roughly 50 seeds per arm, or ~290x the histories per run; it was not judged
worth the hours.

The honest statement is that **URR self-shielding is a small term on Godiva,
bounded below about ±80 pcm**, which is physically unsurprising: a bare fast
metal sphere has little flux in the unresolved range. It remains worth
implementing for correctness, but it is not the explanation for anything here.

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

The spectrum comparison (section 6) — four seeds per side, then difference:

```sh
for s in 1 2 3 4; do
  OURS_SEED=$s OURS_SPECTRUM=/tmp/ours_spec_$s.csv \
    cargo run --release -p outram-mc-libs --features endf-pebble-cases \
      --example godiva_spectrum_vs_openmc
  OPENMC_SPECTRUM=/tmp/omc_spec_$s.csv python3 godiva.py --spectrum --seed=$s
done

python3 compare_spectrum.py --ours /tmp/ours_spec_*.csv \
                            --openmc /tmp/omc_spec_*.csv
```

The ACE files are ~270 MB in total and are **not** committed; regenerate them.

## Localising the +250 pcm: it is NOT the data, and it IS mostly leakage

Three measurements, in order, each narrowing the last.

### 1. The nuclear data is exonerated

`compare_xs.py` differences this crate's reconstructed cross sections against the
NJOY-produced ACE OpenMC read, flux-weighted over a Watt spectrum:

| nuclide | elastic | fission | inelastic | (n,2n) | ν̄σ_f | absorption |
|---|---|---|---|---|---|---|
| U-234 | −0.007% | +0.025% | +0.000% | +0.000% | — | −0.043% |
| **U-235** | **+0.0002%** | **+0.0002%** | −0.000% | +0.003% | **+0.0002%** | +0.0002% |
| U-238 | +0.001% | +0.057% | +0.000% | +0.000% | +0.057% | +0.002% |

Nothing above 0.06%, and U-235 — 94% of the material — agrees to 0.0002%. A
0.1% error in a dominant reaction is worth roughly 100 pcm, so the data is an
order of magnitude below what could explain the offset.

> **A correction kept on the record.** The first version of that script reported
> +16% (U-234) and +28% (U-238) on *total*. That was the script, not the data:
> these ACE files carry no MT=1 at all and MT=4 only for U-235, so a total must be
> hand-assembled from partials, and the fallback silently omitted every inelastic
> level. `total` is now excluded — it only ever measured the assembly. Only the
> channels the kernel partitions on are compared.

### 2. `k_inf` splits leakage from spectrum

Remove the boundary and leakage goes with it. Relative offsets add, since
`ln k_eff = ln k_inf + ln P_NL`:

| | relative offset |
|---|---|
| k_eff | **+250 ± 51 pcm** |
| k_inf | **+69 ± 23 pcm** (2.9 sigma) |
| ⇒ non-leakage probability | **≈ +181 pcm** |

So about **28% is spectral and 72% is leakage** — our neutrons escape less than
they should.

**The k_inf values themselves, recorded — they are the key debug number.**

| code | k_inf | sd | sem | seeds |
|---|---|---|---|---|
| `outram-mc-libs` (delta, reflective sphere) | **2.26557** | 140 pcm | 49 pcm | 8 |
| OpenMC 0.15.3 (reflective sphere, ptables off) | **2.26401** | 47 pcm | 19 pcm | 6 |

Absolute `Δk = +157 ± 53 pcm`; relative `Δk/k = +69 ± 23 pcm`. **Quote the
relative one** next to a `k_eff` offset — relative offsets add, and `k_eff ≈ 1`
makes its absolute and relative figures coincide, so mixing them silently
compares different quantities.

`k_inf` is worth keeping because it is the same physics with leakage deleted:
any future change classifies itself immediately by whether it moves `k_inf`,
`k_eff`, or both. The inelastic-anisotropy fix should move `k_eff` down ~180 pcm
and leave `k_inf` **essentially unchanged**; if it moves `k_inf`, it went in the
wrong place.

### 3. Elastic scattering is cleared against OpenMC

Before convicting the inelastic distributions, the elastic ones — 85 % of all
scattering — had to be excluded. `tests/elastic_mubar_vs_openmc.rs` compares
this crate's `⟨μ⟩` against OpenMC's own read of the same evaluation:

| E (eV) | this crate | OpenMC | difference |
|---|---|---|---|
| 1.0e3 | +0.00123 | +0.00179 | −5.6e-4 |
| 1.0e5 | +0.13007 | +0.13007 | 0.0 |
| 1.0e6 | +0.49728 | +0.49722 | +6e-5 |
| 2.0e6 | +0.62270 | +0.62245 | +2.5e-4 |
| 5.0e6 | +0.85160 | +0.85135 | +2.5e-4 |

Worst 5.6e-4, at 1 keV where `⟨μ⟩ ≈ 0` and Godiva has negligible flux; ≤ 2.5e-4
across the MeV range that carries it. 200 000 sampled draws at 2 MeV average
+0.62268 against the analytic +0.62270, so the sampler reproduces its table too.
**Reading and sampling are both correct; elastic is not the bug.**

The existing `tests/elastic_anisotropy_vs_endf_mf4.rs` could not have shown
this — both of its sides come from our own parse, so a misreading of MF=4 would
be invisible to it. That is why the oracle here is external.

> **A latent gap found while doing this, filed rather than fixed.** This crate's
> cosine tables carry **no interpolation flag**: `EnergyAngular` stores
> cosines/pdf/cdf and the sampler always applies the linear-linear inverse CDF,
> where OpenMC's `Tabular::sample` branches on `histogram` vs `lin_lin`. It is
> harmless here — all 126 incident energies of U-235's elastic distribution are
> linear-linear, which is *why* the table above agrees — but an evaluation using
> histogram angular data would be mis-sampled silently. Not fixed blind: no
> evaluation held here exercises it, so a fix would be untested code.

### 4. The leakage share is the missing inelastic angular distributions

`src/physics/scatter.rs` states it outright: *"Anisotropic **inelastic** angular
laws (coupled to the MF=5/MF=6 energy distributions) remain future work."*
Elastic uses the full ENDF MF=4 tabulated cosine; **inelastic is sampled
isotropic in CM** (`mu_cm = 2·ξ − 1`).

Isotropic scattering makes `⟨μ⟩` too small, so `Sigma_tr = Sigma_t(1 − ⟨μ⟩)` is
too **large**, the diffusion coefficient too small, leakage too low and k too
high. Right sign. `transport_decomposition.py` measures the size, flux-weighted
above 10 keV over the three nuclides at their densities:

| | ⟨μ_lab⟩ | share of scattering |
|---|---|---|
| elastic | 0.2758 | 85.2% |
| **inelastic** | **0.0254** | **14.8%** — sampled as 0 here |

`Sigma_tr` should be `0.37141 cm⁻¹`; this crate effectively uses `0.37287 cm⁻¹`,
**+0.39%**. Propagated through one-group diffusion from the *measured* reference
`P_NL = k_eff/k_inf = 0.4417` (55.8% leakage):

**predicted +219 pcm, against the measured +181 pcm leakage share.**

One-group diffusion in a bare fast metal sphere is a crude model, so that is an
order-of-magnitude check on whether the mechanism is big enough — not a precise
prediction. It is big enough, and it is the only candidate found with the right
sign and magnitude.

### 5. The fission spectrum is cleared too

The largest spectral candidate, checked the same way
(`tests/fission_spectrum_vs_openmc.rs`) — mean outgoing energy of U-235's
fission neutrons against OpenMC's, on the same evaluation:

| incident E (eV) | this crate | OpenMC | relative |
|---|---|---|---|
| 1.0e3 | 1.99946e6 | 1.99982e6 | −0.018 % |
| 1.0e6 | 2.02443e6 | 2.02459e6 | −0.008 % |
| 2.0e6 | 2.05370e6 | 2.05387e6 | −0.008 % |

> **The same nearest-point trap, a third time.** Read against OpenMC's *nearest*
> tabulated incident energy, χ looks 0.5 % soft at 1 keV. That grid is coarse —
> 22 points over `1e-5 … 3e7 eV`, the first step jumping straight from `1e-5` to
> `5e5` — so the nearest point is far away. Interpolated, the difference is
> 0.018 %. The artefact was in the oracle both times, and in the `⟨μ⟩` table
> above as well.

### 6. The spectrum itself — measured, and it is too hard

The five comparisons above are all *integrated*: they compare a cross section,
a mean, an eigenvalue. None of them can say **where in energy** the remaining
`~69 ± 23 pcm` of `k_inf` comes from. A flux-vs-energy tally can, so that is the
measurement that closes this section.

**Method.** Both codes tally track-length flux on the **same** 50 log-spaced
bins over `1e-3 … 2e7 eV` — ours via `examples/godiva_spectrum_vs_openmc.rs`
(the `EnergyFilter`/`ScoreType::Flux` construction already used by
`tests/openmc_notebooks/flux_spectrum.rs` and the TUI overlay, not written for
this study), OpenMC via `godiva.py --spectrum`. Both spectra are normalised to
unit integral, because neither side carries a volume or power normalisation and
only the shape is meaningful. Same geometry, same three ICSBEP nuclides, same
5000 histories × [40 inactive + 120 active], probability tables off on both
sides.

**Four seeds per side**, differenced by `compare_spectrum.py` using the
**seed-to-seed** spread as the uncertainty rather than the per-run tally sigma.
The question is whether the two *codes* differ, and at these statistics
re-randomisation dominates; a single pair of runs cannot answer it. (Three
small-sample results reversed earlier in this investigation. The seed count is
quoted with every number here for that reason.)

**Result, measured 2026-09-15 — our spectrum is harder:**

| quantity | ours | OpenMC | difference | sigma |
|---|---|---|---|---|
| mean `E` [eV] | 1.47466e6 | 1.46804e6 | **+0.45 %** | 3.5 |
| mean `ln E` | 13.6841 | 13.6774 | **+0.05 %** | 4.9 |
| flux fraction below 300 keV | 0.154809 | 0.156861 | **−1.31 %** | 5.2 |
| flux fraction above 4.8 MeV | 0.040170 | 0.039690 | **+1.21 %** | 2.8 |

Per bin, the difference is monotone on both sides of a crossing near
300–450 keV — a deficit below it that grows as energy falls, an excess above it
that grows as energy rises:

```text
       E_lo        E_hi       ours     OpenMC    rel diff   sigma   share
  1.011e+04   1.625e+04    0.00064    0.00067      -4.50%     1.2   0.07%
  1.625e+04   2.611e+04    0.00166    0.00172      -3.48%     2.5   0.17%
  2.611e+04   4.195e+04    0.00412    0.00425      -2.93%     1.2   0.42%
  4.195e+04   6.742e+04    0.00958    0.00977      -1.90%     1.9   0.98%
  6.742e+04   1.083e+05    0.02142    0.02176      -1.54%     1.7   2.18%
  1.083e+05   1.741e+05    0.04275    0.04324      -1.12%     2.8   4.32%
  1.741e+05   2.798e+05    0.07419    0.07499      -1.06%     2.1   7.50%
  2.798e+05   4.496e+05    0.11189    0.11185      +0.03%     0.1  11.18%   <- crossing
  4.496e+05   7.226e+05    0.14091    0.14140      -0.35%     1.4  14.14%
  7.226e+05   1.161e+06    0.15943    0.15872      +0.45%     2.5  15.87%
  1.161e+06   1.866e+06    0.16098    0.16048      +0.31%     0.7  16.05%
  1.866e+06   2.999e+06    0.13894    0.13851      +0.31%     1.3  13.85%
  2.999e+06   4.819e+06    0.09288    0.09249      +0.42%     1.0   9.25%
  4.819e+06   7.744e+06    0.03509    0.03473      +1.03%     2.2   3.47%
  7.744e+06   1.245e+07    0.00493    0.00480      +2.84%     6.1   0.48%

total absolute shape difference = 0.0051
  i.e. ~0.26% of the spectrum sits in different bins between the two codes
```

**No single bin proves this, and the write-up should not pretend otherwise.**
The one bin past 3 sigma — `7.744e6 … 1.245e7 eV` at `+2.84 %`, 6.1 sigma —
carries **0.48 %** of the flux, and the bins carrying 7–16 % agree to ±0.45 %.
The per-bin sigmas are individually weak and cannot be pooled: the spectra are
normalised, so a deficit anywhere forces an excess elsewhere and the bins are
not independent. It is the four **aggregates** above that resolve the
difference, which is why `compare_spectrum.py` prints them.

**Interpretation.** Too little down-scatter. For A ≈ 235, elastic scattering
removes almost no energy — `⟨E'/E⟩ ≈ 1 − 2A/(A+1)² ≈ 0.992` per collision — so
**inelastic scattering is the dominant energy-loss mechanism in this system**,
and a spectrum that is too hard is what an inelastic secondary-energy treatment
returning neutrons too high in energy would produce. A harder spectrum raises
both `ν̄(E)` and U-238 threshold fission, which is the right sign for a positive
`k_inf` residual.

**What it does not establish.** It shows that the spectrum differs and where,
not which reaction causes it. MT=91's continuum law shape, the discrete
MT=51–90 level selection, and `(n,2n)` would all produce a hardness difference
of this sign; separating them needs ablation of each in turn, the same way
elastic anisotropy was priced below. This measurement **corroborates** the
inelastic secondary-energy hypothesis and rules out "the spectra agree and the
residual is something else" — no more than that.

### What this leaves

- ~~**Implement anisotropic inelastic angular distributions.**~~ **Done**
  (`op-tm9f`, 2026-09-15). It removed the leakage share as predicted: Godiva
  moved `−198 pcm`, from `+214` to `+16 ± 11 pcm` at 256 seeds. See *Discrete
  inelastic angular anisotropy* below for the prediction-versus-outcome and the
  regression gates.
- **The ~69 pcm spectral share is still unattributed, but now by elimination
  rather than by ignorance.** It is in `k_inf`, so it is secondary-energy or
  reaction sampling, not geometry — and cross sections (≤0.06 %), ν̄ (0.0002 %)
  and now χ (≤0.018 %) are all excluded. What remains untested is the
  **inelastic** secondary energy: level selection, and the continuum law that
  `scatter.rs` documents as a Weisskopf evaporation stand-in. Note that this
  lands on the *same* reaction class as the leakage share, which is suggestive
  but not evidence.

  **That measurement has now been made** (section 6 above): the spectrum is
  harder than OpenMC's at 5 sigma on the flux fraction below 300 keV, with a
  monotone high-energy excess — consistent with too little inelastic
  down-scatter, and the right sign for a positive `k_inf` residual. It narrows
  the residual to the inelastic secondary-energy treatment without yet naming
  which part of it.
- URR probability tables are a **separate** gap, and a small one: measured at
  `+43 ± 38 pcm`, consistent with zero (see the ablation section below). Fixing
  the anisotropy moves k down; probability tables, if they move it at all, move
  it up. Independent defects — price them independently, never net them.

## Ablation: pricing the physics directly

Three knobs, ablated and measured, with the tests kept for regression.

### Elastic angular anisotropy — +10511 pcm

[`Nuclide::with_isotropic_elastic_scattering`] empties the MF=4 table, so
elastic scatters isotropically in CM. **The transport kernel is not modified**:
`sample_elastic_mu_cm` already returns `None` for a nuclide with no anisotropic
data and every driver already falls back to isotropic, so the ablation runs the
production code path and the two arms differ in exactly one input.

| arm | evaluated | ablated | worth |
|---|---|---|---|
| surface | 1.00273 ± 50 pcm | 1.10785 ± 44 pcm | **+10511 ± 67 pcm** |
| delta | 1.00283 ± 52 pcm | 1.10812 ± 72 pcm | **+10528 ± 89 pcm** |

Removing anisotropy raises k by 10 %, in the predicted direction, and the two
independent collision loops price it to within 17 ± 111 pcm of each other.

**This calibrates the diagnosis.** Elastic carries `0.852 × 0.2758 = 0.23498` of
the `⟨μ⟩` budget; inelastic carries `0.148 × 0.0254 = 0.00376`, a ratio of
0.0160. Scaling gives the missing inelastic anisotropy as **+168 pcm** — against
**+181 pcm** measured from the `k_eff`/`k_inf` split and **+219 pcm** from
one-group diffusion. Three independent routes, agreeing to ~25 %. Linear scaling
across a 10 000 pcm ablation is crude, so this is corroboration rather than
precision, but it is a third line of evidence for the same mechanism.

`examples/godiva_anisotropy_ablation.rs` carries three gates: the worth must be
a significant *increase*, both tracking methods must price it the same, and the
value must reproduce the recorded +10511 within 4 sigma.

### Discrete inelastic angular anisotropy — the leakage share, closed

**This is the fix the whole study was pointing at**, and it is the one place
where a prediction made *before* the work can be checked against what happened.

Until bead `op-tm9f`, every inelastic collision drew `mu_cm = 2*prn − 1`,
isotropic in the centre of mass, while elastic used the evaluation's full MF=4
cosine. The discrete levels are not isotropic: ENDF/B-VIII.0 gives U-238's MT=51
a CM `⟨μ⟩` of `+0.033` at 1 MeV rising to `+0.510` at 14 MeV, and **39 of the 40
levels MT=51…90 carry anisotropic data on each of U-235 and U-238**, all with
`LCT = 2` so no frame conversion is needed. (U-235's MT=51/52/54 read as
isotropic; that is a real property of that evaluation, not a parse defect — they
carry 9-point distributions with `a₁ ≈ 0` at all 124 tabulated energies.)

**The prediction, from this study and recorded before the code was written:**
Godiva moves **down by ~180 pcm**, priced three independent ways at −168
(scaled from the elastic ablation above), −181 (the leakage share of the
`k_eff`/`k_inf` split) and −219 pcm (one-group diffusion from the measured
`P_NL`). Stated with it: *if it moves up, or moves far more than ~220, the
diagnosis is wrong and should be revisited rather than tuned around.*

**What happened**, `examples/godiva_keff_ensemble.rs`, **256 seeds**:

| | mean vs ICSBEP | sd | sem |
|---|---|---|---|
| before (`RECORDED_PCM`, 64 seeds) | +214 pcm | 160 | ±20 |
| **after, 256 seeds** | **+16 pcm** | 173 | **±11** |
| **move** | **−198 pcm** | | |

−198 pcm against −168/−181/−219 predicted. The remaining offset is **+16 ± 11
pcm**, 1.5 sem from the benchmark and well inside its own ±100 pcm band.

#### The price, ablated directly

`examples/godiva_inelastic_anisotropy_ablation.rs` runs the case twice against
[`Nuclide::with_isotropic_inelastic_scattering`], which restores exactly the
pre-`op-tm9f` behaviour. **The transport kernel is not modified**:
`sample_inelastic_mu_cm` already returns `None` for a level with no data and
every driver already falls back to isotropic-CM, so both arms run the production
path and differ in one input.

| arm | n | mean vs ICSBEP | sd | sem |
|---|---|---|---|---|
| **ANISO** (evaluated MF=4) | 32 | **+45 pcm** | 182 | ±32 |
| **ISO** (pre-`op-tm9f`) | 32 | **+269 pcm** | 172 | ±30 |
| **difference** | | **−224 pcm** | | **±44 (5.1 sigma)** |

The ISO arm's `+269 ± 30` independently reproduces the `+214 ± 20` recorded
before the fix, to 1.5 sigma — a check on the harness, not a restatement of it.

**The seeds do not pair here, and the run says so rather than assuming either
way.** Paired `sd` is 258 against either arm's 182, because the two arms' RNG
streams diverge at the first inelastic collision — the ablation changes how many
draws a history consumes. So the unpaired figure is the one quoted. (The same
thing defeated pairing in the URR study; it is a property of ablating *physics*
rather than of this particular ablation.)

#### The independent check: `k_inf` must NOT move

This was specified on `op-tm9f` in advance, and it is the part that separates
"the fix is right" from "the fix happens to land on the right number".
`k_inf` uses a reflective boundary, so it has **no leakage**. If the missing
anisotropy really was a leakage error, `k_inf` must be largely unchanged by
removing it.

| | `k_inf` | sd | sem |
|---|---|---|---|
| before | 2.26558 | 140 | ±49 |
| after | 2.26587 | 148 | ±52 |
| **change** | **+13 pcm** | | **±71 — 0.2 sigma** |

`k_eff` moved **−198 pcm**. `k_inf` moved **+13 ± 71 pcm**, consistent with
zero. The effect is entirely in the leakage term, exactly as diagnosed.

Had `k_inf` moved materially, the `k_eff` agreement would be two errors
cancelling rather than one error removed — and the right response would have
been to distrust the result, not to bank it.

Correspondingly, the **spectral residual is untouched**: `k_inf` against OpenMC
is now `+82 ± 55 pcm` relative, against `+69 ± 23 pcm` measured before this
change — the same number within statistics. Bead `op-os8x` stays open.

#### What this does and does not establish

It is worth being precise here, because a number this close invites over-reading
and this investigation has already had three small-sample results reverse.

- **The ±11 pcm is our sampling uncertainty, not the comparison's.** ICSBEP
  quotes `1.0000 ± 0.0010`. Nothing on our side can see past a ±100 pcm band on
  the reference, so `+16` is agreement but is **not meaningfully better than
  `+80` would be**. Quoting it as "16 pcm accuracy" would claim a resolution the
  experiment does not have.
- **It does not clear the physics underneath.** The `~69 pcm` spectral residual
  measured in section 6 lives in `k_inf`, has no leakage component, and is
  untouched by this — our spectrum is still 0.45 % harder than OpenMC's
  (`op-os8x`). Two offsetting errors can land on the right `k`. That the
  *predicted* mechanism moved `k` by the *predicted* amount in the *predicted*
  direction is the reason to believe this one is real, not the closeness of the
  final number.
- **One benchmark, one geometry, one temperature.** A bare fast HEU metal
  sphere. It says nothing about thermal systems or about this crate's other
  cases.

#### Regression

`examples/godiva_keff_ensemble.rs` carries three gates, each sized off **what
the run in hand can resolve** rather than off the accuracy achieved:

1. **No regression** from `RECORDED_PCM`, within `4σ` of
   `√(sem_run² + sem_recorded²)` — the difference of two *independent*
   ensembles, not `4 × sem_run`, which would treat the recorded value as exact
   and be too tight by `√2`. That exact sizing error is on this crate's record
   (gh:#196), so the reasoning is written into the code.
2. **Agreement with the experiment**: `|mean|` inside the ±100 pcm band, or
   `4σ` of the run, whichever is wider.
3. **The seeds are independent** (`sd` has not collapsed) — because `op-rbo` is
   a case where correlated streams left the central value nearly unchanged while
   making every quoted uncertainty a fiction.

A gate at the achieved accuracy — `|mean| ≤ 30 pcm` — was deliberately **not**
written: at the default 32 seeds a run's own `sem` is ±31 pcm, so a correct
build would fail it about a third of the time, and a gate that cries wolf gets
muted.

The fast guard that runs in the ordinary suite is
`tests/inelastic_anisotropy_ablation_control.rs` (2 tests, 285 s): a regression
that silently dropped the MF=4 inelastic tables would put Godiva back at
+214 pcm with **nothing else in the suite failing**, so it asserts a cosine
comes back at all, that it is forward-peaked, that `⟨μ⟩` rises monotonically
with energy (catching a mis-indexed incident-energy lookup), and that cross
sections are bit-identical across the ablation.

### Verifying the fix independently of `k` — and a fourth nearest-point trap

**An eigenvalue landing on a benchmark is weak evidence about physics.** Two
offsetting errors land there too, and this study's own record contains a
physically *correct* fix that moved `k` **away** from the experiment (the MT=91
Q-value cap, gh:#192, `+85 ± 26 pcm`, kept because it was right). So after
`op-tm9f` the fix was checked against quantities that do not involve a transport
run at all.

**Per-level `⟨μ_cm⟩` against OpenMC's ACE**
(`tests/inelastic_mubar_vs_openmc.rs`, 32 compared points across U-235 and
U-238 at 1/2/5/14 MeV): **worst difference 3.1e-3**, `≤1.3e-3` across the
1–5 MeV band that carries most inelastic collisions. Differences are random in
sign — the signature of two independent linearisations of the same Legendre
coefficients, not of a misreading.

Worth more than the numbers: **both codes independently agree that U-235's
MT=51/52/54/55 are near-isotropic while its MT=53/56 and all of U-238's low
levels are strongly forward-peaked.** That asymmetry is a real property of the
evaluation and is exactly what a naive parse would flatten.

**Aggregate transport quantities** (`tests/ablation_suite.rs`):

| quantity | ours | OpenMC | apart |
|---|---|---|---|
| `⟨μ_elastic⟩` | 0.2631 | 0.2645 | 0.5 % |
| `⟨μ_inelastic⟩` | 0.0236 | 0.0245 | 3.7 % |
| `⟨μ_inelastic⟩`, discrete only | 0.0344 | 0.0357 | 3.6 % |
| **MT=91 share of inelastic** | **0.3135** | **0.3140** | **0.2 %** |
| `Σ_tr` shift on ablating inelastic anisotropy | +0.36 % | +0.38 % | — |

The **MT=91 share** is the quiet one. The continuum scores `μ = 0` on both
sides, so it dilutes the inelastic mean cosine directly — two codes could agree
on every angular distribution and still disagree on `⟨μ_inel⟩` if they split
inelastic differently. They agree to 0.2 %.

#### The reference values in this study were biased, and by more than our disagreement with them

`transport_decomposition.py` read OpenMC's angular tables with
`np.searchsorted` — the table at the energy **above** `e`, not interpolated.
`⟨μ⟩` grows with energy, so the reference came out high:

| | as published here | corrected (interpolated) |
|---|---|---|
| `⟨μ_elastic⟩` | 0.2740 | **0.2645** |
| `⟨μ_inelastic⟩` | 0.0254 | **0.0245** |
| `Σ_tr` | 0.37141 | **0.37457** |
| one-group diffusion price | +219 pcm | **+209 pcm** |

**This is the fourth appearance of the nearest-point trap in this study** — the
first three are recorded in sections 3 and 5 above — and the first *inside our
own reference script* rather than in a comparison against someone else's. It was
found only because an independent Rust computation of the same aggregate
disagreed by 30 % and the disagreement was chased rather than absorbed into a
tolerance.

The conclusion survives it: the price moves +219 → +209 pcm, against −198 pcm
measured. That robustness is the reassuring part, but the lesson is that a
reference is not automatically right for being external.

#### The spectral residual is unchanged, as predicted

`op-tm9f` is a *leakage* fix, so the spectrum should be untouched. Re-measured
after it, 4 seeds a side on the same 50-bin grid:

| quantity | before `op-tm9f` | after | sigma |
|---|---|---|---|
| mean `E` | +0.45 % | **+0.45 %** | 4.0 |
| mean `ln E` | +0.05 % | **+0.05 %** | 4.9 |
| flux fraction below 300 keV | −1.31 % | −1.03 % | 4.3 |
| flux fraction above 4.8 MeV | +1.21 % | +0.43 % | 0.9 |

The two integral measures of hardness are **identical**. `op-os8x` is exactly
where it was, and the leakage/spectrum decomposition holds.

### URR self-shielding — not resolved

This crate has no probability tables to ablate, so it was done on the OpenMC
side (`godiva.py --ptables`). **+43 ± 38 pcm over 16 seeds per arm, consistent
with zero** — see the correction recorded above.

### Regression coverage

| test | cost | guards |
|---|---|---|
| `tests/anisotropy_ablation_control.rs` | ~3 s | that the **elastic** ablation *ablates*, that the evaluated data is genuinely anisotropic, and that cross sections are bit-identical across it |
| `tests/inelastic_anisotropy_ablation_control.rs` | ~285 s | that the **inelastic** MF=4 tables are read at all, are forward-peaked, that `⟨μ⟩` rises monotonically with energy, that the ablation ablates, and that the two ablations are independent |
| `tests/inelastic_mubar_vs_openmc.rs` | ~230 s | per-level `⟨μ_cm⟩` against OpenMC's ACE, and the U-235/U-238 anisotropy asymmetry |
| `tests/ablation_suite.rs` | ~530 s | each mechanism priced on `⟨μ⟩`/`Σ_tr` instead of `k`, the MT=91 partition, and grid convergence of the aggregates |
| `examples/godiva_keff_ensemble.rs` | ~80 min (256 seeds) | no regression from `RECORDED_PCM`, agreement with the benchmark, and that the seeds stayed independent |
| `examples/godiva_anisotropy_ablation.rs` | ~35 min | the three elastic gates above |
| `examples/godiva_inelastic_anisotropy_ablation.rs` | ~40 min | the inelastic price (−224 ± 44 pcm, 32 seeds/arm) |
| `examples/godiva_kinf_vs_openmc.rs` | ~20 min | that `k_inf` did NOT move (+13 ± 71 pcm), i.e. the fix is in the leakage term |
| `examples/godiva_spectrum_vs_openmc.rs` + `compare_spectrum.py` | ~20 min | the spectral shape against OpenMC |
| `godiva.py --ptables` | ~1 h | the URR arm, re-runnable |

The unit test exists because a silently no-op control is the worst failure mode
an ablation study has: it reports "no difference" and reads as "this physics
does not matter".

### Summary: what is cleared, and what is convicted

| quantity | agreement with OpenMC | verdict |
|---|---|---|
| cross sections, all channels | ≤ 0.06 % flux-weighted | cleared |
| ν̄σ_f | 0.0002 % | cleared |
| reaction-partition catch-all | 0.0000 % of Σ_t | cleared |
| elastic ⟨μ⟩ | ≤ 5.6e-4 absolute | cleared |
| fission spectrum ⟨E_out⟩ | ≤ 0.018 % | cleared |
| **inelastic angular** | **was not sampled at all; now read from MF=4/MT=51…90** | **convicted and FIXED: −198 pcm measured, −181 predicted** |
| inelastic secondary energy | spectrum **+0.45 % harder** in mean `E` (3.5 sigma); −1.31 % of the flux below 300 keV (5.2 sigma); `k_inf` vs OpenMC `+82 ± 55 pcm` after the angular fix, against `+69 ± 23` before | **still implicated and still open (`op-os8x`)** — unchanged by the angular fix, as expected |

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
