# HTR-10 vs the RMC benchmark — V&V record

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** All code in this workspace is **unverified and untrusted** unless a specific verification & validation (V&V) case demonstrates otherwise. V&V cases are human-reviewed and are intended for journal / arXiv publication — that is the trust workflow. See the workspace `VERIFICATION_AND_VALIDATION.md` and `RESPONSIBLE_USE.md`. Not for nuclear facility operation, reactor control, safety-critical, or licensing decisions.

**The full twelve-height verification suite lives in
[`docs/htr10-rmc-verification-suite.md`](../../../../docs/htr10-rmc-verification-suite.md)**
(2026-09-24): every fuel loading the reference tabulates, at 10 000 histories
x [5 inactive + 135 active], with the eigenvalue curve, the fitted
height dependence, the separated data/transport timings, the P-core vs E-core
sweep and the constant-cavity ablation. **This page remains the authority for
how the model was built and corrected**; that one is the authority for how it
performs across the loading range.

**Images of the built geometry (2026-09-25):**
[`crates/nee_soon/verification_and_validation/htr10_geometry_images/`](../../../nee_soon/verification_and_validation/htr10_geometry_images/README.md)
— slices of the assembled `assemble_explicit_triso(14, 25, 0)` core rendered
with the OpenMC-parity plotter, from the whole R-Z model down to one TRISO
particle. gh:#309 and gh:#310 are directly visible there.

**Status as of 2026-09-18 (later): the conus was filled with the WRONG
CONTENTS, and correcting it removes the +3670 pcm overshoot.** `op-5n34`.

~~The core sits at `+3670 +/- 115 pcm`, OUTSIDE the gate.~~
**CORRECTED** — that number came from a conus full of **fuel** pebbles.

Terry et al. (2005) section 2, quoted in this repository's own derived
geometry (`kovan-literature/derived/terry2005-htr10-rz-zone-geometry.md:256`):

> *"the conus and discharge tube contained only **dummy** pebbles"*

The conus was built as part of the bed hex lattice, and `bed_tile_levels`
applies the core's 57:43 fuel:dummy split to every level it makes. So
extending the lattice down to the conus floor filled the conus with fuel that
is not there. **The geometry was right and the contents were wrong.**

### Measured, all single-seed, 3000 histories x [20 inactive + 60 active]

| arm | `k_eff` | vs RMC |
|---|---|---|
| conus with FUEL (the +3670 state) | 1.040179 +/- 0.003215 | +3589 +/- 322 |
| conus with DUMMY pebbles (Terry) | 0.988408 +/- 0.002693 | -1588 +/- 269 |
| **conus contents, fuel -> dummy** | | **-5177 +/- 420 pcm (12 sigma)** |
| + discharge tube as dummy pebbles | 0.991372 +/- 0.003002 | -1292 +/- 300 |

**POOLED, 8 seeds** — the physical model's residual is
~~**`-1592 pcm, sem +/-63`**~~ **`-1231 pcm, sem +/-63`**, seed-to-seed
`sd 179 pcm`.

> **RE-MEASURED 2026-09-23 after the correct-physics default change — the
> residual did NOT move.** Every number above was taken with **URR
> probability tables and DBRC OFF**, which was the default until
> `f8dbb49512` ("correct physics is the DEFAULT, not an opt-in") turned both
> ON. The workspace rule is that a changed default obliges a re-measurement
> of every V&V number that depended on it, so the case was re-run at
> `870e65f1b7ae`:
>
> ```text
> k_eff = 0.988174 +/- 0.002636    height-matched dk = -1250 +/- 264 pcm
> ```
>
> Against the pooled `-1231 +/- 63 pcm` above that is a shift of **19 pcm**,
> i.e. **nothing** — far inside the 179 pcm seed-to-seed spread. **Turning on
> URR and DBRC did not measurably move this benchmark.**
>
> **Read that as the weak statement it is.** The new figure is a SINGLE SEED
> against an 8-seed pool, and the cycle settings differ (2000 x [30 + 70]
> here against 3000 x [20 + 60] above), so this bounds the movement at a few
> hundred pcm rather than resolving it. A clean measurement needs both arms
> pooled at identical settings. What it does rule out is a *large* shift.
>
> Note also that the two pre-existing single draws of this same configuration
> — `0.991372` in the table above and `0.988088` in the library-swap section
> below — differ from each other by **~330 pcm**. Any comparison against a
> single one of them is therefore worth less than it looks; compare against
> the pool.
>
> **Determinism, measured in passing and worth keeping.** The re-measurement
> was run five times: 1 thread and 16 threads, pinned to P-cores and to
> E-cores, plus once more after `htr10_rmc::materials` was factored out of
> the example. **All five returned `0.988174 +/- 0.002636`, identical to six
> decimal places.** This page states elsewhere that thread-count independence
> is *tested* for `run_keff` and merely *assumed* for `run_keff_csg_hybrid`,
> which is the driver this case uses — that assumption now has direct
> evidence behind it.

> **CORRECTED 2026-09-18 — +361 pcm of that residual was the COMPARISON POINT,
> not the model.** Every number on this page was compared against RMC's
> **123.576 cm** value, 1.004288, while the model builds a bed
> `lat_height * n_axial` tall. At the 25 layers these runs use that is
> `4.8990 * 25 = 122.474 cm`, and RMC's own twelve-point curve interpolates
> there to **1.000676**. The curve runs **~270 pcm/cm** through this region, so
> a 1.1 cm mismatch is worth more than several of the physics terms ablated
> below.
>
> `examples/htr10_rmc_keff.rs` now interpolates `RMC_KEFF_VS_HEIGHT` to the
> height actually modelled (`rmc_at_height`) and prints both, refusing to
> extrapolate outside `[94.182, 201.960]` cm rather than inventing reactivity
> past the ends. Every `vs RMC` figure in the table above is therefore **361 pcm
> too negative** as written; the k_eff column is unaffected.
>
> **Open question, flagged not resolved:** whether the paper's "fuel loading
> height" is the bed proper or includes the conus. The conus holds *dummy*
> pebbles, so the fuel column is plausibly the bed alone — but the model's bed
> spans `+/-61.24 cm` with a conus reaching `-98.2 cm`, and if RMC measures from
> the conus floor the mapping differs. The +361 pcm correction stands either way,
> because it only requires that 122.474 != 123.576.

That is the number to quote. The single draw above sits **1.7 sd** off the
pooled mean, which is the whole argument for `gh:#196` / `bn:op-awwi` in one
line: at `sd = 179 pcm`, one run of this case is worth +/-179 pcm of
re-randomisation, so a single-seed residual cannot be quoted to better than
a few hundred pcm. Every OTHER number on this page is still a single draw.

So the conus **geometry** is worth far less than the +4578 pcm previously
recorded; almost all of that was fuel that should not have been there.

### This also revises the "two offsetting errors" reading

The earlier `-909 pcm` was explained as a model missing 13.6 % of its fuel and
therefore agreeing by coincidence. That explanation is **wrong**: the
flat-bottomed model was not missing fuel, it was missing the conus's *dummy*
pebbles and had reflector graphite in their place. Replacing that graphite
with dummy pebbles is worth only about **-680 pcm** (-909 -> -1588), which is
a believable perturbation rather than a masked error. The "two offsetting
errors" account should not be cited.

### The discharge tube: neither of the two bounds

The same Terry sentence covers it. The model had **solid reflector graphite**
there (over-reflecting the conus tip where the fuel converges); an ablation to
**pure helium** was worth `-2100 +/- 456 pcm`, which is the opposite bound.
Neither is the reactor. It is now **pebble graphite at the bed's 0.61 filling
fraction**, which sits between them.

### The control-rod boring band — measured, but NOT enabled

TECDOC zone 22 is used for every reflector region in this model, and it is
**rank 1 of 40 distinct carbon densities in Table 4-3** — the densest graphite
available, everywhere. Giving the band at r 95.6-108.6 cm the reduced density
of zones 31-40 (-28.1 % carbon) is worth **-1572 +/- 425 pcm (3.7 sigma)**.

**It is left OFF by default and must not be read as a correction.** The
derived zone map records only the bottom two axial layers; the core-height
assignment for that radial band is explicitly *"not yet placed"*. Zone 47 is
the documented zone for `[95.6, 108.6]` at the bottom, and zones 31-40 were
inferred only from ten consecutive zones sharing one density — the doc warns
in terms, *"use this as a check, not a generator"*. Enabling it would swap one
unjustified composition for another in the reflector band nearest the core.
`OUTRAM_HTR10_BORINGS=1` measures it; nothing depends on it.

### Also fixed: the source box and entropy mesh were blind to the conus

Both were fixed numbers predating the conus — `SourceBox` `[-50,50]^3` and the
entropy mesh `[-60,60]^3`, against a bed now reaching `conus_floor = -98.2 cm`.
A starting source that misses fuel is recoverable given inactive generations;
**an entropy mesh blind to part of the core is not** — it reports convergence
of the region it can see. Both now span the bed's actual extent.

## How it got there — the ablation chain

Every step below was measured, and each one is a PHYSICAL correction, not a
tuned parameter. Nothing in this model is fitted to the reference.

| # | change | `k_eff` | worth |
|---|---|---|---|
| 0 | `HexLattice` axial-frame port defect | 0.000000 | — |
| 1 | lattice fix (see below) | 0.707506 | first nonzero |
| 2 | critical loading height | 0.803706 | +9,620 pcm |
| 3 | ~~ball-packing target~~ (my error) | ~~0.857140~~ | *spurious* |
| 4 | bed cylinder inscribed in the tiled hexagon | 0.897875 | +4,074 pcm |
| 5 | target fuel-zone fraction, not ball packing | 0.877606 | -2,027 pcm, removing #3 |
| 6 | lattice axial CENTRE corrected | — | see below |
| 7 | ring count from `(sqrt(3)/2)*pitch` | 1.089253 | **6+7 together +21,165 pcm** |
| 8 | boronated carbon bricks, 167.793->190 cm | 1.076647 | -1,260 pcm |
| 9 | cold coolant annulus, 140.6->148.6 cm | 1.075431 | -122 pcm (0.3 sigma) |
| 10 | **empty core cavity above the bed** | 0.934346 | **-14,108 pcm** |
| 11 | axial reflector above the cavity | **0.995200** | +6,121 pcm |

### The three geometry defects, and how each was caught

**Steps 6 and 7 were one symptom with two causes**, both found by MEASURING
coverage rather than deriving it — formulas for how much of a cylinder a hex
lattice tiles were wrong twice here, so `examples/htr10_fuel_fraction.rs`
samples the real geometry through `locate` instead.

- **Axial offset (step 6).** The lattice was given
  `center.z = -bed_half_height + h/2`, but `HexLattice::center_offset` already
  centres the stack about that point. The whole lattice sat 58.79 cm low: it
  spanned z = [-120.03, +2.45] against a bed cell of [-61.24, +61.24]. The
  tell was that the untiled fraction was a flat **0.48 at every radius,
  including r = 0** — an axial offset, not a radial shortfall.
- **Ring count (step 7).** A Y-oriented hex lattice steps `(sqrt(3)/2)*pitch`
  in x, so 15 rings reached ~80 cm where `(n-0.5)*pitch` claimed 95.8 cm.
- **Both were SILENT.** The untiled region took the lattice's `outer`
  universe -- dummy graphite pebbles -- so no history was lost, no distance was
  negative, and the geometry simply contained less fuel than the model said.
  Measured kernel volume fraction went **0.000690 -> 0.001605** across the two
  fixes, against a paper-implied 0.0016769 (now 4.3 % low, from 59 % low).

**Step 10 is the largest single term and was a missing VOID, not missing
material.** Terry (2005) Fig. 2 and TECDOC-1382 put the core cavity at 221.818
cm with the bed occupying 123.06 cm of it, leaving **98.758 cm of helium above
the bed**. Modelling that as reflector graphite returned neutrons the real
reactor leaks. Carving it out moved `k` by -14,108 pcm and took leakage from
3.1 % to 15.7 %; step 11 then put the real ~130 cm of graphite ABOVE the cavity
(an unextended `bed_half_height + 100` had left 1.2 cm), bringing leakage to
0.479 %.

### What is still NOT modelled

- ~~**The conus.** ... Adding it needs conditional tile omission, which the
  lattice does not have.~~ **MODELLED 2026-09-18 (`op-5n34`) -- and the stated
  blocker did not exist.** See "The conus" below. It is worth **+4578 +/- 158 pcm** and
  **OVERSHOOTS**: the model goes from -909 pcm to roughly +3.5k pcm.
- ~~**The bottom is modelled symmetrically** with the top rather than as conus +
  discharge tube.~~ **FIXED on develop (`d619b2e77e`, 2026-09-25); this line was
  stale and is corrected here.** Both assembly functions now place `refl_bottom`
  from the reactor — `HTR10_BOTTOM_REFLECTOR_CM = 610.0 - 388.764` below the
  conus floor — rather than mirroring the top. The conus and discharge tube were
  already modelled (see "The conus" above); what was left was the *extent*, which
  the mirror made loading-dependent: 192.4 cm of bottom graphite at the benchmark
  loading but only 114.0 cm at the tallest, in a model 581 cm tall rather than
  610. **Every result recorded on this page before that commit used the mirrored
  bottom.**
- ~~**Control-rod borings** (r 95.6-108.6 cm) are solid graphite here, not
  homogenised with their borings.~~
- ~~**Control rods themselves** are absent; the benchmark arm is rods-out.~~
- **CHANGED 2026-09-25 (WIP, branch `claude/htr10-reflector`, not yet priced
  or drawn):** every reflector channel is explicit geometry — 20 coolant,
  10 control-rod, 3 irradiation and 7 absorber-ball channels plus the hot gas
  duct, in solid graphite, inside the full IAEA-TECDOC-1382 Fig. 4.10 zone map
  with the p. 242 corrections — and the ten rods sit in their channels at the
  withdrawn position (B1 is rods-out) with explicit B4C, steel and iron. The
  discharge tube holds explicit whole graphite balls with Li (2014)'s
  rejection at the cone and tube. See `nee_soon::htr10_rmc::reflector_geometry`
  and `crates/kovan-literature/derived/tecdoc1382-htr10-mc-borings-and-zone-map.md`.
  The residuals on this page predate it.
- **4.76 % of ALL core graphite** is clipped away by the one-ball-per-tile
  construction and replaced by helium. Every pebble loses 4.74 % of its volume
  — 11.1 % of the *fuel* pebble's fuel-free shell, and the whole cap for the
  43 % of tiles that are solid graphite dummies, which have no shell at all —
  while the fuel zone loses only 0.061 %. So core graphite goes
  **0.59988 → 0.57131**, helium **39.0 % → 41.9 %**, and the heavy metal stays
  exact to **+0.060 %**: **C/U is 4.8 % low**. The `core_model.rs` comment said
  4.7 % of the *shell*, which is the BALL deficit mislabelled, and omitted the
  dummy pebbles entirely; both corrected 2026-09-25. Sign on `k` not predicted,
  not measured — **gh:#309**.
- **`mat::HOMOG_DUMMY` smears the discharge tube at `PAPER_FILLING_FRACTION =
  0.61` while the bed it homogenises realises 0.5814**, so the tube is 4.9 %
  denser in graphite than the bed above it — the same error with the opposite
  sign, in the one place the model homogenises rather than resolves. It should
  read the realised packing from the assembled lattice — **gh:#309**.
- **The bed lattice drops the paper's A-B layer offset, so the pebbles
  INTERPENETRATE.** The paper's cell is a two-ball prism whose layers sit in each
  other's hollows (interlayer centre distance 6.2102 cm, clear of the 6.0 cm
  diameter); one ball per tile puts every ball in a column at the same `(x, y)`,
  so axial neighbours sit at **4.8990 cm centres — 1.101 cm less than a
  diameter**. The tile cut lands exactly on the plane where the two spheres
  cross (intersection circle 1.7321 cm radius), so it is the *correct* union and
  each 2.6816 cm³ "cap" **is the interpenetration lens**; the fuel zones
  interpenetrate too (intersection circle 0.5 cm, 0.0199 cm³ each). This is the
  cause of the graphite deficit above: pebbles at 4.899 cm centres cannot occupy
  0.61 of the volume. **`bed.rs`'s own `is_non_overlapping()` guard cannot see
  it** — it runs on `HexBedCell::from_paper()`, which passes, and `HexBedCell`
  has no method that returns the columnar spacing — **gh:#310**.
- **B-11 is never placed** anywhere in this model (only B-10), which leaves the
  boronated brick ~3.5 % short on atom density. `nee_soon::rod_insertion`
  already splits it correctly — **gh:#311**.
- **Every cell hardcodes 293.6 K** while the material temperature is what
  transport reads, so `Cell::temperature` is inert here — **gh:#313**.
- **`core_model::assemble` (the homogenised-fuel path) has none of the above
  fixed.** No cavity (solid graphite there), no boronated brick, no coolant
  annulus, no conus, no discharge tube: roughly **+15 500 pcm** in terms this
  page already prices individually. It carries `assemble_explicit_triso`'s
  comments without its geometry, and it feeds `examples/htr10_mgxs_genfoam.rs`.
  **Do not read `OUTRAM_HTR10_HOMOG=1` as isolating the fuel zone** —
  **gh:#308**.

## THE RESIDUAL WAS THE DATA LIBRARY — measured 2026-09-18

The model was run on **ENDF/B-VII.0**, the library RMC, MCNP, Serpent and HCP
all used, against the ENDF/B-VIII.0 it had always used. Same geometry, same
seed, same settings (3000 histories x [20 + 60], 14 rings x 25 layers, bed
122.474 cm), compared against RMC interpolated to the height actually modelled.

| library | `k_eff` | dk vs RMC (height-matched) |
|---|---|---|
| ENDF/B-VIII.0 | 0.988088 +/- 0.003095 | **-1259 pcm** |
| **ENDF/B-VII.0** | **1.004525 +/- 0.003096** | **+385 pcm** |
| **library term** | | **+1644 +/- 438 pcm (3.75 sigma, RESOLVED)** |

**On the reference's own library the model agrees with RMC to `+385 +/- 310
pcm` — 1.24 sigma, statistically indistinguishable from the reference, and
inside the 500-1000 pcm gate.**

The page had named this as a known uncorrected systematic "worth hundreds of
pcm" and never priced it. It is worth **1644 pcm**, and on this problem it was
essentially the entire residual. **Nothing was tuned**: the number moved because
the model was given the same evaluated data the reference used.

### Where the prediction was wrong

The library term was expected to sit mainly in **U-238 capture**, and U-238 was
ablated first (VIII.0 -> JENDL-3.3): **-79 +/- 441 pcm, unresolved at 0.18
sigma**. The full-library swap is **+1644 pcm**, so the effect is **NOT
dominated by U-238**. It lives in U-235, the carbon evaluation, O-16, or the
thermal law. The single-nuclide ablation was wrong twice over — underpowered,
and aimed at the wrong nuclide.

### Provenance

ENDF/B-VII.0 downloaded 2026-09-18 from the IAEA NDS `download-endf` tree
(`https://www-nds.iaea.org/public/download-endf/ENDF-B-VII.0/`), the same pinned
host `njoy-outram-park-fork::acquire` uses. Open, publicly released evaluated
nuclear data. Tapes: `n_9228_92-U-235`, `n_9237_92-U-238`, `n_0825_8-O-16`,
`n_0600_6-C-0`, `n_1425_14-Si-28`, `n_0525_5-B-10`, `tsl_0031_graphite`.
Reachable as `OUTRAM_HTR10_ENDF7=1`.

**Two evaluation differences that are part of the term, not bugs in it:**

- **VII.0 carbon is ELEMENTAL natural carbon** (`6-C-0`, MAT 600); VIII.0 ships
  C-12 separately. The VII.0 arm therefore carries 1.1 % C-13 and the VIII.0 arm
  does not. Not separable without a third arm.
- **The graphite thermal tape's MAT changed**: VIII.0 crystalline graphite is
  MAT 30 (ZA 130), VII.0 is MAT 31 (ZA 131). Passing the wrong one makes
  `ThermalScattering::from_endf_file` return `Err`, which turned the whole
  nuclide set into `None` and surfaced as the misleading *"reference-data/endf/
  not in this checkout"*. Both the MAT selection and the error message are fixed.

### Read this before quoting `+385 pcm`

- **SINGLE SEED.** `sigma = 310 pcm`, seed-to-seed `sd = 179 pcm`. One draw, not
  a mean. Pool it (`OUTRAM_BENCH_SEEDS`) before it goes anywhere citable.
- It is against the **height-matched** RMC value (1.000676). Against the old
  hardcoded 1.004288 the same run reads `+24 pcm` — a *better-looking* number
  that is wrong, and a good illustration of why the comparison-point fix
  mattered even though it made the headline residual larger at the time.
- The cavity defect (below) is ~zero at this loading, so it does not contaminate
  this point. It is still a real defect at every other loading.
- One loading height, one temperature, rods out, no control rods or absorber
  balls, R-Z homogenised reflector. Unchanged.

## THE CHAIN CLOSES — two routes to the answer agree (2026-09-18)

Fixing the cavity was predicted, **with opposite signs at the two ends**, before
either run. A common-mode error cannot satisfy both directions.

| loading | void error | predicted | fixed-void dk | fixed-cavity dk | move |
|---|---|---|---|---|---|
| 102.879 cm | 20.2 cm too LITTLE | k must FALL | -54 | **-891 +/- 288** | **-837 +/- 374 (2.2 sigma)** |
| 171.464 cm | 48.4 cm too MUCH | k must RISE | -1640 | **-447 +/- 276** | **+1193 +/- 402 (3.0 sigma)** |

Both held. And the structure they were creating collapses:

```text
BEFORE (fixed void):    dk = -54, -1259, -2194, -1640    chi2 = 38   on 3 dof  NOT constant
AFTER  (fixed cavity):  dk = -891, -1259, -447           chi2 = 3.9  on 2 dof  CONSISTENT
                        weighted mean = -835 +/- 168 pcm
```

**That is the decisive check.** A real geometry defect should destroy the height
dependence it was generating, and it does.

### The two routes

| term | value |
|---|---|
| baseline residual, VIII.0 + fixed cavity | **-835 +/- 168 pcm** (3 loadings, constant) |
| data library, VIII.0 -> VII.0 | **+1644 +/- 438 pcm** (3.75 sigma) |
| **predicted: VII.0 + fixed cavity** | **+809 +/- 469 pcm** |
| **measured: VII.0 at 122.5 cm** | **+385 +/- 310 pcm** |
| | **agree to 0.8 sigma** |

Measuring the answer directly and summing the ablated terms agree to **0.8
sigma**. That is the closure condition for an ablation chain: it is what
separates "the number came out right" from "the terms are understood".

### What this does and does not say

**Does:** on the reference's own library and with the cavity modelled as fixed
geometry, this model reproduces RMC within the 500-1000 pcm band the module docs
call "a real success here" — and those same docs warn that agreement to 50 pcm
would be *suspicious*, not good. Nothing was tuned. The reflector boring band,
the one available knob that could have been turned toward the answer, was left
OFF because its sign is wrong (-1572 pcm, which would have widened the gap).

**Does not:** every number on this page is a **single seed** against a
seed-to-seed `sd = 179 pcm`. Pool before citing. The **-835 pcm baseline is the
real open residual**, and its candidates are unchanged: R-Z homogenised
reflector, absent control-rod borings, one temperature, rods out, one spec.
And **RMC quotes no uncertainty on any of its twelve values**, so the reference
side of every sigma here is unmeasured.

## The core cavity is modelled as a FIXED VOID, and it should be a fixed CAVITY

`HTR10_CAVITY_ABOVE_BED_CM = 98.758` is applied as a constant void above the bed
at **every** loading. Its own docstring derives it as the void at **one**
loading: the cavity spans `z = 130.0` to `351.818` (**221.818 cm, fixed
geometry**) and the benchmark bed occupies 123.06 cm of it.

The cavity is fixed; the **void** shrinks as fuel is added. Modelling the void
as constant instead grows the whole cavity with the bed.

Measured across four loadings, dk vs RMC height-matched, all ENDF/B-VIII.0:

| layers | bed height | required void | modelled void | error | dk |
|---|---|---|---|---|---|
| 21 | 102.879 cm | 118.94 cm | 98.758 | 20.2 too LITTLE | **-54** |
| 25 | 122.474 cm | 99.34 cm | 98.758 | 0.6 — correct | -1259 |
| 30 | 146.970 cm | 74.85 cm | 98.758 | 23.9 too MUCH | -2194 |
| 35 | 171.464 cm | 50.35 cm | 98.758 | 48.4 too MUCH | -1640 |

Against a constant offset these four give **chi-square 38 on 3 dof** — the
residual is not constant, and the structure has the sign the defect predicts:
too little void below the benchmark loading (graphite where the reactor has
helium, over-reflecting, `k` HIGH) and too much above it (over-leaking, `k`
LOW). The 35-layer point should be the most negative and is not, but it sits
1.4 sigma from the 30-layer point, so it does not refute this.

**The `-54 pcm` at 102.9 cm is NOT evidence of correctness.** It is an
over-reflecting cavity cancelling most of the baseline residual — two errors
cancelling, the same failure mode this page has already recorded twice (the
`-909 pcm` "two offsetting errors" reading, and the conus-with-fuel overshoot).
Had the sweep started there and stopped, it would have been reported as
agreement with RMC to 54 pcm.

`OUTRAM_HTR10_FIXED_CAVITY=1` computes `void = 221.818 - bed_full_height`
(`core_model::cavity_above_bed`). Default keeps the historical constant so no
committed result moves silently.

### Ablations measured 2026-09-18 — data-side terms, and a determinism check

All at 3000 histories x [20 inactive + 60 active], 14 rings x 25 layers,
seed 20260917, single draw per arm unless stated.

| ablation | `k_eff` | difference | resolved? |
|---|---|---|---|
| control | 0.988088 +/- 0.003095 | — | reference |
| graphite S(a,b) -> free gas | 1.008729 +/- 0.002622 | **+2064 +/- 406 pcm** | **yes, 5.1 sigma** |
| U-238 VIII.0 -> JENDL-3.3 | 0.987298 +/- 0.003142 | -79 +/- 441 pcm | **NO, 0.18 sigma** |

**S(alpha,beta) is a first-order term — larger than the whole residual.**
Including it is worth **-2064 pcm**. Predicted before the run as "large and
resolved, order 1000 pcm or more"; the **sign was deliberately NOT predicted**
(coherent elastic against suppressed sub-Debye transfer, not resolvable from
first principles here), so the measured direction is a result and not a
confirmation. Its other job was a harness check: near-zero would have meant the
thermal scattering law was not engaged at all, and every thermal number here
rested on nothing. It is engaged.

**The U-238 library ablation is UNDERPOWERED and must not be read as a null
result.** -79 +/- 441 pcm bounds library sensitivity below ~880 pcm at 2 sigma,
which does **not** exclude a few-hundred-pcm term — precisely the size that
matters against the residual. The central value is negative as predicted, but at
0.18 sigma that is noise, not a hit. Resolving it to +/-150 pcm needs ~9 seeds
per arm.

**This is NOT the ENDF/B-VIII.0-vs-VII.0 offset** the references carry; no VII.0
tape exists in `reference-data/endf`. It bounds library sensitivity on the
dominant absorber and nothing more, and `OUTRAM_HTR10_U238_JENDL`'s own doc
comment says so.

### Determinism of `run_keff_csg_hybrid` — measured, not assumed

`physics::keff::tests::cpu_multi_is_reproducible` asserts bit-identity between
1 and 4 threads, but through **`run_keff`** — the simple sphere path. This case
runs **`run_keff_csg_hybrid`**, which nothing covered, and the example hardcoded
`ThreadCount::Auto`, so the count follows machine load.

Measured (`OUTRAM_HTR10_THREADS`, added for this):

```text
threads=1   k_eff = 0.585633 +/- 0.015214     (reduced 6x8 core, a code-property test)
threads=4   k_eff = 0.585633 +/- 0.015214
threads=1   k_eff = 0.585633 +/- 0.015214
```

and at production scale, two separate processes under different system load:

```text
control          k_eff = 0.988088 +/- 0.003095
control, repeat  k_eff = 0.988088 +/- 0.003095
```

**Identical in every printed digit.** The driver is thread-count independent and
run-to-run reproducible. The test-coverage gap was real; the behaviour is sound.

### A step in the chain above that is NOT resolved

| step | `k_eff` | vs RMC |
|---|---|---|
| conus with dummy pebbles | 0.988408 +/- 0.002693 | -1588 |
| + discharge tube as dummy | 0.991372 +/- 0.003002 | -1292 |
| **the discharge-tube step** | | **+296 +/- 403 pcm = 0.73 sigma** |

That step is **unresolved**, yet it appears in the chain as a result, and it is
what produces the -1292 single-seed headline. The pooled 8-seed number is
**-1592** (pre-correction), essentially identical to the **-1588** *before* the
step — so the pooled data says the discharge-tube change is worth approximately
nothing and the +296 pcm was noise in one draw. Re-measure it paired and
multi-seed before quoting it.

### Ablations that bound the terms

| ablation | `k` | reads as |
|---|---|---|
| all fuel, no dummy balls | 0.915527 +/- 0.003962 | 57:43 dilution ~11,000 pcm |
| all boron removed | 0.943105 +/- 0.003170 | boron 8,597 pcm -- ~6x its bare-pebble worth |
| reflector all boronated (zone 17) | 0.453025 +/- 0.002857 | the reflector composition brackets [-55,126, +8,496] pcm |
| 150 inactive generations | 0.852762 +/- 0.002915 | **source convergence ruled out** (1.0 sigma) |

Entropy is flat at ~5.13 bits from generation 0, which is the independent check
on that last row.

**The boron reading was queried and is CORRECT as modelled.** `reflector.rs`
already records that TECDOC Table 4-3's column is *natural* boron, so the
x0.199 to B-10 is right. The 8,597 pcm is physics, not an input error, and
removing boron is not available as a route to agreement.

## The k = 0 failure and its root cause — RESOLVED 2026-09-17

~~The eigenvalue comparison has been ATTEMPTED AND FAILED; k = 0.000000.~~
**CORRECTED 2026-09-17** — the cause was a **port defect in
`HexLattice::distance`**, not the model, the tracker, or the fuel loading.

`HexLattice::distance` reconstructs a lattice-frame position from the caller's
tile-local one. It reconstructed **all three** components; the axial test at the
end of that function compares `z` against `+/- 0.5 * pitch[1]`, which is a
**tile-local** half-height. The comparison was therefore wrong by the tile's own
`z` offset and returned a **negative** distance-to-boundary.

OpenMC builds the hybrid -- x,y lattice-frame, z tile-local -- at the *call*
site (`src/geometry.cpp:459-467`) and guards the result with
`if (d_lat < 0) p.mark_as_lost(...)`. This port had neither.

**Why it survived.** The error cancels **exactly** when the tile z-offset is
zero, i.e. `n_axial == 1`, and `from_rings_3d` had unit tests only -- no
integration test and no example. Every existing test sat on the one
configuration that hides it.

**Why it was invisible in `k`.** A negative distance steps the neutron
backwards, so it re-crosses the same boundary until the per-history event budget
kills it -- and a budget-exhausted history is **scored as a leak**, so the
neutron balance closes and nothing in the output points at geometry.

**Measured, on this model:**

| `n_axial` | negative distances before | after |
|---|---|---|
| 1 | 0 | 0 |
| 2 | 8,199,697 | 0 |
| 20 | 17,498,719 (worst -7.7e3 cm) | 0 |

and the history-termination histogram, 12 rings x 20 layers:

| end | before | after |
|---|---|---|
| stuck on the event budget | **68.5 %** | 0 % |
| lost in `locate` | 14.5 % | 0 % |
| genuine vacuum leak | 5.4 % | — |
| collisions per history | 4.2 (denominator-corrected 105.6) | 498.9 |
| `k_eff` | 0.000000 | **0.707506 +/- 0.006010** |

Gated by `tests/hex_lattice_axial_frame.rs`. Filed as a P0 bug.

### How it was found — the instrumentation is the finding

`k` alone could not distinguish "absorbed" from "lost", because both leak arms
scored identically. Five counters were added to the CSG driver and are now part
of `KeffResult`: `histories`, `collisions`, `lost_locate`, `stuck_events`,
`leak_vacuum`, `leak_infinity`, plus `neg_dist`/`neg_from_lattice`/
`neg_from_surface`. Each step below eliminated a hypothesis:

1. **Reflective outer boundary changed `k` bit-for-bit not at all** -> no
   history was reaching the boundary; this is not leakage.
2. **Macroscopic cross sections printed per material** -> every material is
   correct (graphite absorption/total 1.0e-3 thermal, 3.3e-6 at 1 MeV; the
   kernel's nu-fission 5.69 against absorption 2.79). Not the materials.
3. **The history denominator was wrong** -- rates were being divided by the
   *planned* history count while the run died after 4 generations. Correcting it
   turned "2.7 % stuck, 4.2 collisions/history" into "68.5 % stuck, 105.6
   collisions/history", which is what made the defect visible at all.
4. **Negative distances split by source** -> 100 % from the lattice, 0 % from
   any CSG surface.
5. **Swept `n_axial`** -> 1 layer gives exactly zero negatives, more gives
   millions. That named the axial branch.
6. **Read OpenMC's caller before patching** (workspace rule) -> found the
   hybrid position it builds, which is the fix.

**Two of my own hypotheses were wrong and were measured down rather than
assumed away**, and both are recorded because a discarded hypothesis is
evidence: a degenerate ball-tangent-to-prism geometry (it was real, and fixing
it changed nothing), and the TRISO lattice failing to cover its fuel zone (also
real, also not this). A third -- clipping the tile universes with explicit
planes -- was implemented, made things worse, and was reverted.

## The target

Li, Yu & Wei (2014), *Research on Benchmark Calculation and Analysis of HTR-10
with RMC Code*, HTR 2014 Weihai, paper HTR2014-51207. Catalogued **proprietary**
(no licence statement on its pages) as `li2014htr10rmc`.

Critical loading height **123.576 cm**: RMC **k = 1.004288**, MCNP **1.0033**.

**The reference quotes no uncertainty on any of its 12 values.** At its stated
1.35 M active histories the implied σ is ~60–100 pcm, but it is never printed,
so "agreement to 100 pcm" against it is not a well-posed claim. The gate is
**500–1000 pcm** (maintainer decision), which matches the existing bar recorded
in `nee_soon::htr10_rmc` — *"~500 pcm would be success, 50 pcm would be
suspicious."*

## What has been built and measured

| Component | Evidence | Measured |
|---|---|---|
| Hybrid delta/surface tracking | `tests/hybrid_tracking_equivalence.rs` | hybrid vs surface **−133 ± 215 pcm (0.62 σ)** at 12,000 histories |
| — absorber isolation | same | region-local vs global majorant **448×** in virtual collisions, `k` unchanged (1.32 σ) |
| Majorant price of a rod | `examples/majorant_absorber_price.rs` | **26.3×** at the thermal peak, **1.00×** above ~1 keV |
| Boundary handoff unbiased | `tests/bounded_delta_flight.rs` | one region vs two + handoff, **1.38 σ** on collided fraction |
| Depth-3 lattice descent | `tests/nested_lattice_depth3.rs` | 3 levels, streaming stops at the **inner** tile edge (0.050000 cm) |
| Shannon entropy in the driver | `tests/shannon_entropy_in_keff.rs` | plateaus at 5.32 bits below the log2(64) ceiling; `k` bit-identical with/without |
| TRISO radii adjudicated | `op-867c.12` | TECDOC-1382 states 90 µm twice, in two units; `TrisoRadii::HTR10` corrected from 95 |
| Cubic TRISO array | `tests/cubic_triso_array.rs` | **8340** particles, **+0.060 %** vs the stated 8335 |
| Bed 57:43 split | `nee_soon` `htr10_rmc::bed` | 0.569995 at 18,930 tiles, within one tile at **every prefix** |
| Reflector, 82 zones | `nee_soon` `htr10_rmc::reflector` | densest zone **100.0 %** of solid graphite at the separately-stated 1.76 g/cm³ |
| Geometry integrity | `nee_soon` `tests/htr10_geometry_integrity.rs` | 27,038 balls vs stated 27,000, **+0.142 %** |
| Core assembly cost | `nee_soon` `examples/htr10_core_scaling.rs` | **547× tiles → 5 % locate time** |

## Three findings that changed the plan

1. **8,335 TRISO is unattainable.** The count moves in symmetry shells (8336 →
   8240 in one step of pitch); nearest reachable are 8330 and 8340. The paper's
   arrangement is therefore *not* exactly the one specified — its zone radius,
   particle radius or rejection rule must differ in the last digit.
2. **One shared surface cannot clip the conus.** Region surfaces inside a
   lattice tile are evaluated in the **tile-local** frame, so each boundary tile
   needs its own translated copy (`surface_in_tile_frame`).
3. **Scale is not a runtime risk.** The plan treated the 650× gap to full core
   as its main threat. Locate cost is flat, because lattice indexing is O(1)
   arithmetic rather than a search.

## What is NOT done, and must not be implied

- **No eigenvalue has been computed for HTR-10.** The assembled core carries a
  **homogenised** fuel zone, not an explicit TRISO lattice, so the double
  heterogeneity is absent and its `k` is not comparable to the reference.
- **The data library differs from every reference.** RMC, MCNP, Serpent and HCP
  all used **ENDF/B-VII.0**; this workspace has **VIII.0**. On a
  graphite-moderated LEU system that is worth hundreds of pcm, so a
  disagreement could not be attributed to transport.
- **The reflector densities are homogenised in R-Z.** TECDOC says explicitly
  that a 3-D model must correct them for the boring geometries; using them
  unadjusted smears the control-rod and helium-flow channels uniformly.
- **This crate's thermal accuracy floor is ~200–400 pcm**, not 100 — LCT-008
  sits at +87 to +237 pcm against ICSBEP with a ~69 pcm spectral residual still
  open (`op-os8x`, gh #206).
- **No control rods or absorber balls** are modelled.

## The fuel deficit — real, but NOT the cause of `k = 0`

~~`k = 0` traces to the model carrying far too little fuel.~~
**CORRECTED 2026-09-17** — it does not. The cause was the `HexLattice::distance`
axial-frame defect recorded above; `k = 0` persisted through every fuel-loading
fix in this section and vanished the moment the lattice defect was fixed, with
the fuel loading unchanged. The packing finding below is nonetheless **real and
was fixed**, so it is kept — as a loading correction, not as a diagnosis.

The reasoning that went wrong is worth keeping too: a ~5x fuel deficit was
measured, `k = 0` was attributed to it, and the attribution was never tested
against the alternative that neutrons were being *destroyed*. They were. The
lesson is the one the instrumentation section states — `k` alone could not
distinguish a model that under-produces from one that loses its histories, and
no amount of reasoning about fuel fractions could substitute for counting how
each history actually ended.

The deficit as originally measured:

| quantity | value |
|---|---|
| expected kernel volume fraction of an HTR-10 bed | **1.676e-3** |
| measured source acceptance over the box | **3.375e-4** |
| ratio | **~5x too little fuel** |

**Half of that factor is found exactly.** `HexBedCell::from_paper()` carries
`balls: 2.0` — the paper's hexagonal prism holds **two** balls, one per
close-packed layer — but the assembled lattice places **one pebble universe per
tile**:

| | packing |
|---|---|
| one ball per tile (what is built) | **0.3050** |
| two balls per tile (the paper) | **0.6100** ← the stated 0.61 |

Exactly 2x, reproducing the published filling fraction to four digits. Not an
approximation — the defect.

The confusion underneath is worth stating because it is easy to repeat: a hex
**lattice** places one universe at each tile centre, while the paper's **cell**
is a two-layer prism carrying half-spheres on its faces and full balls between.
They are not the same object. `HexBedCell`'s arithmetic is correct and gated —
it predicts 27,038 balls against a stated 27,000 — but it was never reconciled
with the lattice that consumes it, and nothing checked that the geometry
realised the packing the arithmetic assumed.

~~**Residual: 2.31x**, most likely the per-tile TRISO count.~~
**CORRECTED 2026-09-17 — the TRISO count is right.** Measured directly
(`nee_soon/examples/htr10_triso_count.rs`): the assembly builds **8,385**
particles for a realised packing of **0.050550** against the `TrisoSpec`
intended **0.050248** — a ratio of **1.006**, i.e. 0.6 % high, not 2.31x low.
The tile-centre keep rule and `cubic_array_in_ball`'s whole-particle rule agree
to within one part in 170 at this pitch.

**The packing fix, and a false start inside it.** The two-balls-per-tile factor
was closed by keeping the paper's pitch (6.6106 cm) and halving its height
(9.79796 -> 4.899 cm), so one ball per tile reproduces 0.610 exactly with the
ball clipped axially. An earlier attempt set the pitch to the **ball diameter**
(6.0 cm) instead; that also gives ~0.61, but makes the ball exactly tangent to
all six prism faces. That degeneracy is a genuine defect and was measured --
but fixing it changed the negative-distance count not at all, which is how it
was ruled out as the cause.

**What is established now:** the loading is right to ~1 %, the transport is
clean (0 lost, 0 stuck, 0 negative), and the remaining -29,678 pcm is a
MODEL-completeness question -- starting with the fact that the run above is a
98.0 cm bed against the benchmark's 123.576 cm critical height.

## Reproducing what exists

```bash
cargo test -p outram-mc-libs --release --test hybrid_tracking_equivalence
cargo test -p outram-mc-libs --release --test bounded_delta_flight
cargo test -p outram-mc-libs --release --test nested_lattice_depth3
cargo test -p outram-mc-libs --release --test cubic_triso_array
cargo test -p nee_soon        --release --test htr10_geometry_integrity
cargo run  -p nee_soon        --release --example htr10_core_scaling
cargo run  -p outram-mc-libs  --release --example majorant_absorber_price
```


## The conus (2026-09-18, `op-5n34`) -- predicted sign CONFIRMED, magnitude OVERSHOOTS

### Methodology

The bed sits on a conus 36.946 cm tall tapering from the 90 cm core radius to
the 25 cm fuel-discharge tube, and it is **full of pebbles**. Geometry from
`kovan-literature/derived/terry2005-htr10-rz-zone-geometry.md` (Terry 2005
Fig. 2, hand-read by the maintainer, corroborated at ten independent points):
conus z in [351.818, 388.764] in Terry's downward-z frame, the 36.946 cm
agreeing with TECDOC-1382 Table 2 exactly.

Modelled as a `ZCone` with the apex 51.156 cm below the bed bottom (slope
(90-25)/36.946 = 1.759324, `r_sq` = 3.095222), truncated at the discharge-tube
radius by a floor plane. The bed cell's region becomes
**cylinder UNION conus**, and the hex lattice is extended axially to reach the
conus floor (25 -> 41 layers; the extra tiles above the bed lie outside the
cell region and are never reached).

### The stated blocker did not exist

`op-5n34` and the plan both recorded this as needing **conditional tile
omission**, which `HexLattice` does not have. **That premise is wrong.**
`Geometry::locate` calls `find_cell` -- a CSG region test -- *before*
descending into a lattice, so a point is only assigned a tile if it is inside
the enclosing cell's region. The cone therefore clips the pebble lattice for
free, exactly as `ins(7)` already clipped it to r < 90 cm, and
`Cell::distance_to_boundary` tests the cone because it is one of that cell's
own surfaces. No new lattice mechanism was needed, and none was written.

The three options recorded on the bead -- per-tile universes via
`surface_in_tile_frame`, real per-tile omission in `HexLattice`, and rejection
in the delta path's `material_at` -- were all **rejected as unnecessary**.

### Results -- the geometry, verified two ways before the eigenvalue

Measured through `Geometry::locate` by `examples/htr10_fuel_fraction.rs`, not
derived from a formula (coverage formulas have been wrong twice in this model):

| quantity | measured | independent prediction | agrees |
|---|---|---|:--:|
| kernel volume | **+14.8 % +/- 4.2 %** | +13.6 % from the cone frustum | 0.3 sigma |
| non-conus fraction of the sampling box | **0.127070** | 0.12707 from `pi r^2 h` minus the frustum | exact |
| untiled fraction, filler removed | **0** | lattice must reach the conus floor | yes |

The second row is the sharper check: it is the cone's own volume, to five
decimal places, measured by sampling rather than integrated.

### Results -- eigenvalue

Both arms at identical settings -- 14 rings x 25 layers, 10000 histories x
[40 inactive + 120 active], surface tracking, ENDF/B-VIII.0, same seed:

| | `k_eff` | vs RMC 1.004288 |
|---|---|---|
| no conus (committed baseline) | 0.995200 +/- 0.001082 | **-909 +/- 108 pcm** |
| with conus | **1.040984 +/- 0.001148** | **+3670 +/- 115 pcm** |
| **conus worth** | **+4578 +/- 158 pcm** | **29.0 sigma** |

The baseline arm **reproduced the committed number exactly** (0.995200 +/-
0.001082), which is the check that the two arms differ only by the conus.

Both arms stayed clean: 0 lost, 0 stuck, 0 negative distances, leakage 0.479 %
-> 0.451 %.

**The predicted sign is confirmed.** The conus adds fuel and `k` moved UP,
exactly as stated in advance on the bead. **The magnitude overshoots by 5x**:
it was supposed to close a 909 pcm gap and delivered 4578 pcm.

### What the overshoot means -- read this, it is the important part

A model that sat 909 pcm LOW while missing 13.6 % of its fuel volume was
**agreeing for the wrong reason**. Adding real physics has exposed a
compensating error of comparable size elsewhere: something in this model is
over-reactive by a few thousand pcm and was being masked by the absent conus.

This is the failure mode the workspace `CLAUDE.md` names explicitly -- *"two
offsetting errors land on the right `k` too"* -- and it means **the earlier
-909 pcm must not be cited as agreement**. It was a coincidence of two errors,
and this change is what revealed that, which is worth more than the number it
replaced.

**Nothing was tuned to recover the old agreement**, and nothing should be. The
candidates for the compensating error, none yet measured:

- **The discharge tube** (r < 25 cm below the conus floor) is reflector
  graphite here. In the real reactor it is a tube of pebbles and void; solid
  graphite there over-reflects the conus tip.
- **Control-rod borings** (r 95.6-108.6 cm) are solid graphite, not homogenised
  with their borings -- this over-moderates and over-reflects.
- **The bottom is symmetric with the top**, so there is 229 cm of graphite
  below the conus where the real model has the discharge tube.
- **Packing in the conus** is the bed's 0.61 everywhere; pebbles against a
  sloping wall pack worse than in bulk.
- Every reflector region is a **single homogenised TECDOC zone**.

### Statistical caveat, stated plainly

Both arms above are **SINGLE SEEDS** (`seed: 20260917`), as is every ablation
in this record. Pooled multi-seed re-measurement is gh:#196 / `bn:op-awwi` and
has **not** been done. The conus effect (+4578 +/- 158 pcm) is about ten times the
combined single-run sigma, so it is resolved by a single pair and is safe to
quote as an effect. The *residual* after it is not similarly safe, and no
sub-sigma difference anywhere in this record should be quoted as a result.

## 2026-09-26: fast VII.0/VIII.0 ablation on the explicit-reflector geometry

Six single-seed fast runs (2000 x [30 + 70]) at n = 20, 25, 41 on both libraries, on PR #327's explicit reflector with Ni and Fe-57 substituted in the rod steel. Critical loading: **VIII.0 -2365 pcm, VII.0 -922 pcm** height-matched. See [`fast_ablation_2026_09_26.md`](fast_ablation_2026_09_26.md).
