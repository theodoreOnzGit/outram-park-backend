# F-19 MT=16: an ACE `LNW` chain of correlated laws, sampled as a mixture (GitHub #365)

*2026-09-29. Research, education and V&V only; not for any operational,
licensing or safety use (see `RESPONSIBLE_USE.md`).*

## What was wrong

`Nuclide::from_ace` refused any DLW `LNW` chain that was not made only of
MF=5-style laws (4/7/9/11), with the error
`ACE DLW LNW chain mixing a correlated law (44/61) or a phase space with another law`.
F-19 (ENDF/B-VIII.0, MAT 925) writes MT=16 (n,2n) as a chain of **two law-61**
distributions, both applicable over 10.987–20 MeV with `p = 0.5` each (OpenMC's
reader reports the same thing). So the whole nuclide was refused on the NJOY2016
table and on the Rust-NJOY table, and routes 3 and 5 of the five-route ICSBEP
study could not run HEU-SOL-THERM-009.

## Upstream, read first

- **NJOY2016 ACER** (`acefc.f90`, `acelf6`, lines 7139–7420): each ZAP=1
  subsection of an MF=6 section becomes one link of the chain. The link's law is
  44 for MF=6 LAW=1 (later written as 61 for tabulated-angle data), 67 for LAW=7
  (converted to 61), 66 for LAW=6. Its applicability is **`y_k / sum_j y_j`** —
  on the ENDF yield's own interpolation regions when the yields are constant,
  and on the combined lin-lin grid when they are energy dependent. The reaction's
  multiplicity is written once, in `TY`. A chain from MF=5 (`acelf5`) is laws
  4/7/9/11; MF=5 and MF=6 links are never chained together.
- **OpenMC** reads the chain in `openmc/data/reaction.py:1082-1092` into
  `Product.distribution` + `Product.applicability`, and samples it in
  `ReactionProduct::sample_dist` (`src/reaction_product.cpp:110-129`): accumulate
  `applicability_[k](E)` and take the first law with `xi <= sum`, the last law as
  a fallback. `Tabulated1D::operator()` (`src/endf.cpp:243-268`) honours the
  interpolation regions and **clamps** outside the tabulated range.

## The representation chosen, and why

Each link becomes one `ContinuumBranch` — the type the ENDF route already builds
for F-19's two MF=6 subsections — with a new field
`applicability: Option<Tab1>`. `ContinuumEmission::branch_for` selects on
`p_k(E)` when the field is set and on the ENDF yield otherwise. One type and one
sampler serve both routes, so they cannot drift.

Yield and applicability are **separate fields on purpose.** ACER derives one from
the other, so a sampler reading either picks the same law; but a yield is a
multiplicity and an applicability is a probability, and
`ContinuumEmission::total_yield_at` must not return a probability to a caller
asking how many neutrons come out. For an applicability mixture it returns
`sum_k p_k y_k`, which is 1 for an ACE branch (ACE branches carry no yield — the
multiplicity is `TY`), the same answer a single-law ACE emission gives. MT=16/17
multiplicities are taken from the MT on both routes and never read it.

Selection normalises by the total weight before drawing (identical to OpenMC's
unnormalised accumulation whenever `sum p_k = 1`, which ACER guarantees and F-19
satisfies exactly). The applicability is evaluated OpenMC's way — regions
honoured, **clamped** outside the range, not zeroed as the workspace's
`eval_tab1` does for cross sections.

The frame is one per reaction (the sign of `TY`), applied to every link, as ACE
and OpenMC's `scatter_in_cm` both do.

### Chains this handles, and the narrower refusals that remain

| chain | handled as | status |
|---|---|---|
| all links 4/7/9/11 | `UncorrelatedEmission` with `FissionSpectrum::Mixture` + AND cosine | unchanged (Na-23 MT=91) |
| links from {44, 61, 66} | `ContinuumEmission`, one applicability-selected branch per link | **new** (F-19 MT=16) |
| a LAW=4 link with an isotropic / absent AND cosine alongside a correlated link | a branch with `EvaluatedIsotropic` | **new**, exact |
| a LAW=4 link with an **anisotropic** AND cosine alongside a correlated link | refused | the AND cosine is on its own incident grid, uncorrelated with `E'`; `ContinuumAngular` is indexed by the energy law's rows. ACE sets `LAND = -1` whenever a correlated law carries the angle, and ACER never writes such a chain |
| an analytic link (7/9/11) alongside a correlated link | refused | no exact tabulated form; ACER never chains MF=5 with MF=6 laws |
| a fission-MT chain with a tabulated or analytic link | `FissionSpectrum::Mixture` (angle dropped, as the single-law fission path does; ACER forces fission to the lab frame) | **new** |
| two-body / discrete-photon / nested links | refused | not continuum neutron laws |

Every refusal names its link in the error, so a table that meets one fails
loudly instead of losing a law.

## Methodology

`crates/outram-mc-libs/tests/ace_lnw_mixture_f19.rs`:

1. **Selection statistics (synthetic).** A two-branch emission with known
   `p_1(E)`; `branch_for` is drawn `N = 200 000` times and the fraction picking
   branch 1 must lie within **5 binomial sigma** of `p_1(E)`,
   `sigma = sqrt(p(1-p)/N)`. Cases: lin-lin interpolation (`p_1(1.5 MeV) = 0.35`),
   a histogram region (`INT = 1`, `p_1 = 0.10`, not the lin-lin 0.5), and clamping
   below and above the range (`p_1 = 0.20`, `0.80`). A fourth case checks an ENDF
   (yield-selected) emission is unchanged.
2. **F-19 loads** from both ACE libraries; MT=16 has two law-61 branches with
   `p = 0.5`, laboratory frame.
3. **Code-to-code against OpenMC** on the same `.ace` files. The reference is
   the exact CDF of `E'` that OpenMC's C++ sampler produces, computed from
   **OpenMC's own Python ACE reader** (OpenMC 0.16.1.dev25, commit `d7d3284a1`) by
   [`openmc_inputs/f19_mt16_reference_cdf.py`](openmc_inputs/f19_mt16_reference_cdf.py)
   into [`data/f19_mt16_openmc_cdf_njoy.csv`](data/f19_mt16_openmc_cdf_njoy.csv)
   and [`data/f19_mt16_openmc_cdf_rust.csv`](data/f19_mt16_openmc_cdf_rust.csv).
   For histogram tables with no discrete lines (asserted) that CDF is piecewise
   linear between the union of the scaled knots, all of which are listed, so it
   is exact, not binned. outram-mc samples `N = 200 000` energies through the
   transport kernel's entry point `continuum_inelastic_scatter_evaluated_with`.
   Metric: one-sample **Kolmogorov–Smirnov** distance; pass if below the
   `alpha = 0.001` critical value `1.95/sqrt(N) = 4.36e-3`. 14 and 18 MeV lie on
   the incident grid (`r = 0`); 16.25 MeV lies mid-bin (`r = 0.5`), exercising the
   statistical interpolation and envelope scaling.
4. **The check can fail.** The same statistic for draws from branch 0 alone (a
   mixture that loses its second law) must exceed **5x** the critical value.
   Separately, two source mutations were run and reverted: making
   `selection_weight_at` ignore the applicability failed all three synthetic
   selection tests; zeroing the applicability below its range (as `eval_tab1`
   would) failed the clamping test.
5. **ACE route vs ENDF route.** The ENDF route's MT=16
   (`ContinuumEmission::from_endf_mf6` on `reference-data/endf/n-009_F_019-ENDF8.0.endf`,
   two yield-1 subsections) is sampled the same way; two-sample KS,
   `alpha = 0.001`, critical `1.95 sqrt(2/N) = 6.17e-3`.

The driver script is embedded here as the CLAUDE.md rule asks:

```python
# openmc_inputs/f19_mt16_reference_cdf.py (abridged; the committed file is canonical)
t = od.IncidentNeutron.from_ace(ace_path)
prod = t.reactions[16].products[0]
for e in [14.0e6, 16.25e6, 18.0e6]:
    p = [float(a(e)) for a in prod.applicability]          # Tabulated1D, clamped
    x = union of scaled knots of tables i, i+1 of every distribution
    cdf = sum_k p[k] * ((1-r) F_{k,i}(y_i(x)) + r F_{k,i+1}(y_{i+1}(x)))
```

## Results (2026-09-29)

Both libraries give identical MT=16 data (the two reference CSVs agree line for
line below their headers) and identical outram-mc samples.

| incident E | OpenMC reference `<E'>` | outram-mc `<E'>` (N = 2e5) | KS D vs OpenMC | branch-0-only D |
|---|---|---|---|---|
| 14.00 MeV | 1.141511 MeV | 1.14162 MeV | 1.82e-3 | 6.92e-2 |
| 16.25 MeV | 1.943615 MeV | 1.94127 MeV | 2.14e-3 | 1.33e-1 |
| 18.00 MeV | 2.595567 MeV | 2.59565 MeV | 1.32e-3 | 1.88e-1 |

Critical value 4.36e-3: all three pass, and the one-law mutation is 16–43x over
it, so the comparison has the power to see a lost link. The two laws are very
different (at 18 MeV their means are 3.24 and 1.96 MeV), which is why the
selection matters.

| incident E | ACE `<E'>` | ENDF `<E'>` | two-sample KS D |
|---|---|---|---|
| 14.00 MeV | 1.13898 MeV | 1.14017 MeV | 2.22e-3 |
| 16.25 MeV | 1.94132 MeV | 1.94054 MeV | 2.76e-3 |
| 18.00 MeV | 2.59842 MeV | 2.59357 MeV | 2.96e-3 |

Critical value 6.17e-3: the ACE and ENDF routes agree within statistics.

Synthetic selection (z-scores against the tabulated `p`): lin-lin `-0.34`,
histogram `+1.48`, clamped below `-0.60`, clamped above `+0.39`.

## Interpretation

F-19 now builds from both ACE libraries, and its (n,2n) secondary-energy
distribution matches OpenMC's sampling of the same file and the ENDF route's
sampling of the evaluation, within statistics at `alpha = 0.001`. The cosine half
of law 61 is sampled by the pre-existing `LabTabulated` path and was not the
subject of this check. The reactivity effect on HEU-SOL-THERM-009 is expected to
be small — MT=16 opens at 10.99 MeV, above almost all of a thermal system's
flux — but the refusal was all-or-nothing, so the case could not run at all.
The HST-009 rows of routes 3 and 5 are to be rerun by the five-route study;
nothing here measures `k`.
