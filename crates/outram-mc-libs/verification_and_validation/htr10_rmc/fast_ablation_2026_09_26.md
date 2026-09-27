# HTR-10 vs RMC: fast ENDF/B-VII.0 / VIII.0 ablation on the explicit-reflector geometry (2026-09-26)

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** Fast-testing statistics, single seed per
> point. Tentative in the sense of gh:#336. Not for any operational use.

## What was run

- **Geometry:** `nee_soon::htr10_rmc::core_model::assemble_explicit_triso`, 14
  rings. It includes the two-ball bed of draft PR #328 and the explicit
  reflector of draft PR #327 (every boring explicit, the TECDOC zone map, a
  dummy-ball discharge tube with Li's rejection). Branch
  `claude/htr10-geometry-verification-gsg8jx`. **This is the first eigenvalue
  ever computed on the #327 geometry.** Drawn and inspected before the runs:
  `crates/nee_soon/verification_and_validation/htr10_python_plots/`.
- **Heights:** `n_axial` = 20, 25 (the critical loading) and 41, i.e. beds of
  97.980, 122.474 and 200.858 cm (volume-equivalent), i.e. 99.081, 123.576 and
  201.960 cm in the paper's whole-ball-extent convention (gh:#333, see below).
  The reference is Li, Yu & Wei (2014), RMC, linearly interpolated where the
  height is not tabulated.
- **Libraries:** ENDF/B-VIII.0 (default), and ENDF/B-VII.0 (`OUTRAM_HTR10_ENDF7=1`).
  - The VII.0 arm uses VII.0 elemental C.
  - Its UO2 thermal laws are generated from the VIII.0 LEAPR decks.
  - It has no SiC law (VII.0 ships none).
  - Its rod metals come from VIII.0.
  - All of this is as documented in `htr10_rmc_keff.rs`.
- **Statistics (fast testing, maintainer spec):** 2000 particles per generation
  × [30 inactive + 70 active]. Seed 20260917. Hybrid delta/surface tracking.
  ~~293.6 K~~ **CORRECTED 2026-09-27:** cross sections at 300.15 K (27 °C, as
  Li states; `TEMP_K` in `htr10_rmc_keff.rs`).

### Modelling assumptions (maintainer direction, stated in every log)

1. **Rod-steel Ni → Fe**, atom for atom: Ni-58/60→Fe-56, Ni-61→Fe-57, Ni-62→Fe-54, Ni-64→Fe-58
   (`OUTRAM_HTR10_NI_AS_FE=1`). No Ni evaluation is in `reference-data/endf/`,
   and the IAEA/NNDC hosts are unreachable from the remote session.
2. **Rod-steel Fe-57 → Fe-56** (`OUTRAM_HTR10_FE57_AS_FE56=1`). Reconstructing Fe-57
   (LRF=7, 3 particle pairs) was OOM-killed at 13.4 GB (gh:#339).
3. Channel azimuths, channel contents, side-wall clipping and homogenised zones
   are as in PR #327. ~~Open questions in gh:#330, #331 and #332.~~ All
   three were decided by the maintainer on 2026-09-27: #330, keep the 18°
   convention with KLAK and irradiation channels empty; #332, zones without
   documented internals as TECDOC-1382 gives them.
   (#331 decided 2026-09-27: balls crossing the side wall are cut, not
   rejected, because rejection breaks Li's stated 61 % filling fraction.)

## Results

> **CORRECTED 2026-09-27 (gh:#333): the residuals in this first table used the
> wrong reference height.** They are kept, struck, because they were published
> on 2026-09-26. Li's tabulated heights are whole-ball extents
> (`9.798 N + 6.0` cm), and the model's `n_axial = 25` bed is exactly the
> paper's 123.576 cm loading (25 whole fuel ball layers, measured on the built
> bed). The corrected table follows. The k values are unchanged; only the
> reference moved.


| n | bed [cm] | RMC (interp.) | k, VIII.0 | Δk VIII.0 [pcm] | k, VII.0 | Δk VII.0 [pcm] | VII − VIII [pcm] |
|---|---|---|---|---|---|---|---|
| 20 | 97.980 | ~~0.911138~~ | 0.885174 ± 0.002916 | ~~−2596~~ | 0.893254 ± 0.003269 | ~~−1788~~ | +808 ± 438 |
| **25** | **122.474** | ~~1.000676~~ | **0.977026 ± 0.003604** | ~~−2365~~ | **0.991456 ± 0.003124** | ~~−922~~ | **+1443 ± 477** |
| 41 | 200.858 | ~~1.160581~~ | 1.145118 ± 0.003028 | ~~−1546~~ | 1.157031 ± 0.003427 | ~~−355~~ | +1191 ± 457 |

**Corrected residuals (2026-09-27)**, matched on the paper's whole-ball extent:

| n | paper height [cm] | RMC | k, VIII.0 | Δk VIII.0 [pcm] | k, VII.0 | Δk VII.0 [pcm] |
|---|---|---|---|---|---|---|
| 20 | 99.081 (interp.) | 0.915907 | 0.885174 ± 0.002916 | **−3073** | 0.893254 ± 0.003269 | **−2265** |
| **25** | **123.576 (tabulated)** | **1.004288** | 0.977026 ± 0.003604 | **−2726** | 0.991456 ± 0.003124 | **−1283** |
| 41 | 201.960 (tabulated) | 1.162230 | 1.145118 ± 0.003028 | **−1711** | 1.157031 ± 0.003427 | **−520** |

Rods removed at n = 25 (VIII.0): 0.980195 ± 0.003496, **−2409 pcm** (was −2048).

Every run: 0 lost locates, 0 stuck events, 0 negative distances. Shannon
entropy over the active generations moved by at most 0.05 bits. Transport took
930–1640 s per run. Nuclear-data processing took about 280–400 s per run
(gh:#341).

**Ablation at the critical loading (VIII.0): the rods themselves.** With the
withdrawn rods removed (channels empty; `OUTRAM_HTR10_NO_WITHDRAWN_RODS=1`):
k = 0.980195 ± 0.003496 (−2048 pcm). The rods (B4C, steel, iron) are worth
**−317 ± 502 pcm**. The sign is as expected; the magnitude is not resolved.

### Interpretation

- **Library term:** VII.0 − VIII.0 = +808, +1443 and +1191 pcm (±440–480). All
  three agree with the previously recorded constant **+965 ± 91 pcm** (gh:#218)
  within 0.4, 1.0 and 0.5 sigma respectively (combined with its ±91). The
  library offset is unchanged by the new geometry.
- **Height drift persists:** a weighted linear fit of Δk against bed height
  gives ~~+10.2 ± 4.0 pcm/cm (VIII.0) and +12.3 ± 4.4 pcm/cm (VII.0)~~
  **+13.2 ± 4.0 pcm/cm (VIII.0)** and **+15.2 ± 4.4 pcm/cm (VII.0)** on the
  corrected heights (2026-09-27, gh:#333: steeper, as that issue predicted).
  Both are consistent with each other. With three single-seed points they are
  only ~2.5 sigma from zero. gh:#218's drift is still present on this geometry.
- ~~**On VII.0, the reference's own library, the critical loading reads
  −922 ± 312 pcm.** That is inside the 500–1000 pcm gate. On VIII.0 it is
  −2365 ± 360 pcm.~~ **CORRECTED 2026-09-27:** on VII.0 the critical loading
  reads **−1283 ± 312 pcm**, i.e. **outside** the 500–1000 pcm gate. On VIII.0
  it is **−2726 ± 360 pcm**.
- **The explicit reflector moved k down by about 4000 pcm** relative to draft
  PR #328 (+1646 pcm at n = 25 on VIII.0 with the homogenised reflector). That
  comparison is across different statistics and seed counts (and #328's
  +1646 used the same volume-equivalent height, so its corrected value is
  about +1285). Quote it as a direction, not a worth. The
  channels (void near the core), the zone map (boronated zones placed) and the
  explicit dummy tube are all candidates; they were not ablated one at a time
  here.

## Graphite thermal scattering law (added 2026-09-27)

**Question.** The model uses the ENDF/B-VIII.0 **crystalline** graphite
S(α,β) (MAT 30) for every graphite region. VIII.0 also ships Hawari's
reactor-graphite laws at 10 % and 30 % porosity (MAT 31, 32), which carry the
disorder of a real porous graphite. How much k rides on that choice?

**Prediction, posted on gh:#218 before the runs:** k goes up for 10P and further
up for 30P, by a few hundred pcm (no more than about 1000 pcm).

**Method.** VIII.0, n = 25 (123.576 cm, the critical loading), with the same
geometry, modelling assumptions, seed (20260917) and fast statistics as the
table above. `OUTRAM_HTR10_GRAPHITE_TSL=10P|30P` swaps the law in every graphite
material and changes nothing else.

| Graphite S(α,β) | k | Δk vs RMC [pcm] | vs crystalline [pcm] |
|---|---|---|---|
| crystalline (default) | 0.977026 ± 0.003604 | −2726 | — |
| reactor graphite 10P | 0.980160 ± 0.003360 | −2413 | **+313 ± 493 (0.6σ)** |
| reactor graphite 30P | 0.986496 ± 0.003220 | −1779 | **+947 ± 483 (2.0σ)** |

All three runs had 0 lost locates, 0 stuck events and 0 negative distances.

**Reading.** The central values follow the predicted order, crystalline < 10P
< 30P, and fall within the predicted size. **Neither difference is resolved**:
these are single fast runs, and 2σ on one seed is not a measurement. The
predicted sign is not yet confirmed.

**Which law is right is a separate question from this sensitivity.** Li used
ENDF/B-VII.0, which has only a crystalline graphite law. So crystalline is the
like-for-like choice for comparing against RMC, whatever real HTR-10 graphite
is.

**Maintainer decision, 2026-09-27: 30P is the VIII.0 default from now on.**
The basis is density, not k. HTR-10 graphite is 1.76 g/cm³ (reflector,
TECDOC-1382), against about 2.25 g/cm³ for the crystal, so it is about 22 %
porous. Hawari's laws come at 10 % and 30 % only, and 30 % is the nearer. The
"porosity" in these laws is vacancy disorder in the phonon spectrum, not a
bulk-density scaling: each material's carbon atom density is set separately
and unchanged. The default is pinned by
`nee_soon::htr10_rmc::tests::the_default_graphite_law_is_30p_reactor_graphite`
and lives in `nee_soon::htr10_rmc::materials::GraphiteLaw`. `crystalline`
remains available as the explicit ablation.

**Every VIII.0 number earlier in this document, and every earlier HTR-10
VIII.0 number in the V&V record, was measured with crystalline graphite.**
They are kept as measured. On the fast single-seed runs above, the new default
would read about −1779 pcm against RMC at n = 25, not −2726. The pooled study
below gives the default's number properly. Against Li's VII.0 reference, the
residual now contains a TSL term as well as the library term.

**Follow-up (maintainer direction: "313 pcm is still significant … worth a
closer look"):** a pooled study is running. It uses 4 new seeds per arm
(20260927–30) at 10 000 × [40 + 100], for crystalline and 10P first and then
30P. Results will be added here.

## MCNP as a rough gauge (added 2026-09-27)

The paper also prints MCNP results from an independently built model, with no
uncertainty (`MCNP_TABLE3_KEFF_VS_HEIGHT`, `MCNP_TABLE4_KEFF_VS_HEIGHT`; both
tables are captioned "(vacuum)", so which is helium is unknown). **They are a
gauge, not a reference.** The same six runs against them:

| n | paper height [cm] | MCNP T3 | MCNP T4 | VIII.0 − T3 / T4 [pcm] | VII.0 − T3 / T4 [pcm] |
|---|---|---|---|---|---|
| 20 | 99.081 (interp.) | 0.909540 | 0.910940 | −2437 / −2577 | −1629 / −1769 |
| 25 | 123.576 | 1.003300 | 1.004790 | −2627 / −2776 | −1184 / −1333 |
| 41 | 201.960 | 1.171930 | 1.169730 | −2681 / −2461 | −1490 / −1270 |

**What the gauge shows.** Against either MCNP column the residual is roughly
flat with height. From 99 to 202 cm it changes by −244 / +116 pcm on VIII.0
(about −2.4 / +1.1 pcm/cm), against +13.2 pcm/cm relative to RMC. That is
because RMC's own difference from MCNP grows with height: 0.10 % / 0.05 % at the
critical point, and 0.83 % / 0.65 % at 201.96 cm (the paper's Re-diff column).

This is **a hint for gh:#218, not a result**. The runs are single-seed, MCNP
used a different model and gives no error, and a drift that tracks RMC-vs-MCNP
could equally be a trait of RMC's model or of ours. It does say that the
drift should not be assumed to be our model's defect until the reference's own
spread is accounted for.

## gh:#333, settled from the paper (2026-09-27)

Li, Yu & Wei (2014), text of the HTR 2014 paper:

- *"The height of a layer is 9.798 cm. Top and bottom planes of hexagonal
  prisms are flat and contain half spheres."*
- *"The top layer is formed by adding half spheres to each ball present in this
  layer."*
- *"the step size of fuel addition is selected as the height of a layer i.e.
  9.798 cm in order to avoid fractional fuel or moderator balls."*

So `N` prisms hold `2N + 1` ball layers (faces and mid-planes), and the top
face's half balls are completed. Every tabulated height is `9.798 N + 6.0` cm,
the extent from the bottom of the lowest ball to the top of the highest.

Checked on the built two-ball bed with `assemble_explicit_triso(14, n, 0)`:
the fuelled ball layers have centre spans of 97.980, 122.474 and 200.858 cm at
n = 20, 25, 41. Their whole-ball extents (span + 6.0 cm) are 103.98, 128.47 and
206.86 cm counting a 26th-layer ball that is centred 2.45 cm above the bed-top
plane. Only a 0.55 cm cap of that ball is inside the bed: shell graphite plus
~0.05 cm of fuel zone. Excluding it, n = 25 is 25 whole fuel layers with extent
**123.576 cm**, the paper's critical loading exactly. The mapping is
`4.899 (n − 1) + 6.0` cm (`paper_height` in `htr10_rmc_keff.rs`). The cap is
not in Li's model; it is noted here and not priced.

## What to quote, and what not

Quote the table **as fast-test single-seed numbers**. They are not a pooled
result. gh:#336's full re-run (10 000 × [5 + 135], several seeds, twelve
heights) is still the step that makes any of these citable.
