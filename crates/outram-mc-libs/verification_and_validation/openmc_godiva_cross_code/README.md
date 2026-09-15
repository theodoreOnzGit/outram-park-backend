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

### What this leaves

- **Implement anisotropic inelastic angular distributions.** Expected to remove
  most of the ~181 pcm leakage share, moving Godiva *down* toward OpenMC.
  Tracked in beads.
- **The ~69 pcm spectral share is still unattributed, but now by elimination
  rather than by ignorance.** It is in `k_inf`, so it is secondary-energy or
  reaction sampling, not geometry — and cross sections (≤0.06 %), ν̄ (0.0002 %)
  and now χ (≤0.018 %) are all excluded. What remains untested is the
  **inelastic** secondary energy: level selection, and the continuum law that
  `scatter.rs` documents as a Weisskopf evaporation stand-in. Note that this
  lands on the *same* reaction class as the leakage share, which is suggestive
  but not evidence.

  A flux-vs-energy tally comparison in both codes is the natural next
  measurement; an eigenvalue cannot localise a spectral shift, a spectrum can.
- URR probability tables are a **separate** gap, and a small one: measured at
  `+43 ± 38 pcm`, consistent with zero (see the ablation section below). Fixing
  the anisotropy moves k down; probability tables, if they move it at all, move
  it up. Independent defects — price them independently, never net them.

## Ablation: pricing the physics directly

Two knobs, ablated and measured, with the tests kept for regression.

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

### URR self-shielding — not resolved

This crate has no probability tables to ablate, so it was done on the OpenMC
side (`godiva.py --ptables`). **+43 ± 38 pcm over 16 seeds per arm, consistent
with zero** — see the correction recorded above.

### Regression coverage

| test | cost | guards |
|---|---|---|
| `tests/anisotropy_ablation_control.rs` | ~3 s | that the ablation *ablates*, that the evaluated data is genuinely anisotropic, and that cross sections are bit-identical across it |
| `examples/godiva_anisotropy_ablation.rs` | ~35 min | the three gates above |
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
| **inelastic angular** | **not sampled at all** | **convicted: ~181 pcm leakage** |
| inelastic secondary energy | not compared | open: ~69 pcm spectral |

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
