# Speed tiers: Standard, Fast, VeryFast (2026-09-27)

`outram_mc_libs::material::speed::SpeedTier` (GitHub #349) selects how much a
nuclide may trade for speed, in both nuclear-data processing and the
transport cross-section lookup. This record gives what each tier changes, the
evidence that `Fast` is exact, and the measured cost and effect of each tier on
the four ICSBEP benchmarks.

| tier | results | data (RECONR/BROADR) | transport lookup |
|---|---|---|---|
| `Standard` | the reference | tolerance 0.001 (NJOY's) | section found by search on every call; nuclide selection evaluates every channel |
| `Fast` (**default**) | **identical** to `Standard` | tolerance 0.001 | section positions resolved once at construction; nuclide selection evaluates the total only |
| `VeryFast` | **approximate** | tolerance **0.01** | as `Fast` |

`Fast` is the default because it is exact: `outram-mc-libs`' rule is that the
cheapest correct path is the default (crate `CLAUDE.md`, 2026-09-25).
`VeryFast` is never a default.

Select it with `Nuclide::from_endf_file_with_speed(path, name, temp_k, tier)`,
or `Nuclide::with_speed(tier)` (lookup only), or `OUTRAM_SPEED=standard|fast|very-fast`
in the ICSBEP examples.

## Why these changes

From `icsbep_2026_09_27.md`: callgrind put **72-92 % of transport
instructions** in `ReconrResult::eval_mt`, and most of that in its linear
search for the reaction's section. `Material::sample_nuclide`, which picks the
collision nuclide, was 65-82 %: it called `xs_at_energy` for every nuclide
**twice** and read only `.total`. The exact total-only path
(`Nuclide::total_at_energy`) already existed and was not used there.

## `Fast` is exact

### Methodology

`tests/speed_tier_fast_is_exact.rs`. Each nuclide is built once at `Fast` and
cloned to `Standard` with `.with_speed`, so the two share their data and differ
only in the lookup. Compared **bit for bit**, with no tolerance:

- every `MicroXS` field and `total_at_energy`, at 508 energies from 1e-5 eV to
  20 MeV, including the thermal range densely and U-238's and F-19's first
  inelastic thresholds;
- `sample_inelastic`'s choice and the seed it leaves, 64 draws per energy;
- `Material::sample_nuclide` over a three-nuclide mixture, 2000 draws.

Nuclides: U-238 (40 levels and the absorption partial sum), U-234 (fissile,
unresolved range), F-19 (threshold levels), H-1 free-gas and with
`c_H_in_H2O` (the S(a,b) branch, where the total is rebuilt rather than read).

### Results

| nuclide | values compared | differing |
|---|---|---|
| U-238 | 37 592 | 0 |
| U-234 | 37 592 | 0 |
| F-19 | 37 592 | 0 |
| H-1 free gas | 37 592 | 0 |
| H-1 with `c_H_in_H2O` | 37 592 | 0 |
| nuclide selection, mixture | 2 000 | 0 |

End to end, the same seed gives the same `k` to every printed digit on all four
benchmarks (table below).

## The four benchmarks at each tier

### Methodology

Release build, one seed, each example's default run size (5000 histories x
[40 + 120]; LCT-008 4000 x [120 + 250], `--cheap-nuclides` because its
default tape set runs out of memory at Fe-57 on this machine, GitHub #339).
Same machine for all runs (4 cores, 16 GB), nothing else running.
"Data" is the example's "Nuclear data ready" time; "transport" its own timer.

### Results (single seed)

| case | tier | data | transport | total wall | k |
|---|---|---|---|---|---|
| Godiva | Standard | 79.1 s | 38.2 s | 118 s | 1.00074 +- 0.00163 |
| Godiva | **Fast** | 77.9 s | **4.6 s** | 82 s | **1.00074 +- 0.00163** |
| Godiva | VeryFast | **28.4 s** | 4.4 s | **33 s** | 0.99928 +- 0.00166 |
| Jemima | Standard | 76.4 s | 60.7 s | 137 s | 0.99953 +- 0.00178 |
| Jemima | **Fast** | 76.0 s | **6.8 s** | 83 s | **0.99953 +- 0.00178** |
| Jemima | VeryFast | **28.0 s** | 6.5 s | **35 s** | 0.99751 +- 0.00170 |
| HST-009 | Standard | 95.6 s | 67.2 s | 162 s | 0.99779 +- 0.00172 |
| HST-009 | **Fast** | 94.5 s | **16.9 s** | 112 s | **0.99779 +- 0.00172** |
| HST-009 | VeryFast | **46.1 s** | 16.1 s | **62 s** | 1.00166 +- 0.00161 |
| LCT-008 cheap | Standard | 96.0 s | 102.0 s | 198 s | 0.99982 +- 0.00123 |
| LCT-008 cheap | **Fast** | 95.8 s | **33.2 s** | 129 s | **0.99982 +- 0.00123** |
| LCT-008 cheap | VeryFast | **46.3 s** | 31.2 s | **78 s** | 1.00195 +- 0.00123 |

**Fast:** transport **3.1x (LCT-008) to 8.9x (Jemima) faster**, `k` identical.
The largest gains are in the fast systems, whose collisions are dominated by
U-235/U-238 with ~40 inelastic levels each; the lattices spend relatively
more time in geometry and S(a,b). Godiva's transport is single-threaded (its
example uses the default `ComputeType`), the other three use every core.

**VeryFast:** nuclear data **2.1-2.8x faster** on top of that; end to end
2.5-3.6x faster than `Standard`.

**The single-seed `k` shifts of VeryFast (-146, -202, +387, +213 pcm) are
not measurements of the approximation.** Changing the data changes every
history's path, so the two runs decorrelate and their difference carries about
sqrt(2) x sigma of noise: 170-250 pcm here. The shifts above are 0.6 to 1.6 of
that, with mixed signs. What the approximation is worth is measured by the ensembles
below.

## What VeryFast costs in `k`

### Methodology

32 seeds per tier and benchmark, same run size as above, release build,
`OUTRAM_SPEED=fast` then `OUTRAM_SPEED=very-fast`. For Jemima, HST-009 and
LCT-008 the seeds are the same in both tiers, so the difference is taken
**per seed** (paired) and its standard error is sd(differences)/sqrt(32).
Godiva's ensemble example (`godiva_keff_ensemble`) reports only its pooled
mean and standard error against the benchmark, so its difference is
**unpaired**: the two standard errors are added in quadrature.

The tolerance 0.01 was chosen before measuring (ten times NJOY's default,
the usual "fast scoping" setting); it was not adjusted afterwards.

### Results (2026-09-27)

| case | seeds | Fast mean `k` (sd) | VeryFast mean `k` (sd) | dk = VeryFast - Fast |
|---|---|---|---|---|
| Godiva | 32 / 32 | +12 +- 32 pcm vs benchmark (sd 180) | +19 +- 26 pcm (sd 145) | **+7 +- 41 pcm** (+0.2 sigma), unpaired |
| Jemima | 32 | 0.99699 (sd 163 pcm) | 0.99682 (sd 196 pcm) | **-18 +- 42 pcm** (-0.4 sigma), paired |
| HST-009 | 32 | 0.99962 (sd 152 pcm) | 0.99905 (sd 171 pcm) | **-57 +- 35 pcm** (-1.6 sigma), paired |
| LCT-008 cheap | 32 | 1.00182 (sd 136 pcm) | 1.00118 (sd 169 pcm) | **-65 +- 40 pcm** (-1.6 sigma), paired |

**Interpretation.** No single case resolves the approximation: every shift
is within 1.6 sigma, at a resolution of 35-42 pcm. But the pattern is not
uniform. The two **fast** systems (Godiva, Jemima) show nothing (+7 and -18
pcm). The two **thermal** systems (HST-009, LCT-008) both shift the same way,
-57 and -65 pcm. Their inverse-variance weighted mean is **-61 +- 26 pcm
(-2.3 sigma)**. That combination was made after seeing the per-case numbers,
so it is a pointer rather than a measurement, but the likely reading is that
the coarser data grid biases thermal systems low by some tens of pcm. A
larger dedicated ensemble on one thermal case is the test that can confirm or
refute it.

**Which numbers to quote.** V&V results are quoted at `Fast` (identical to
`Standard`). VeryFast is for scoping runs only; on thermal systems allow for
a bias of order -60 pcm, and on all four cases here it stays below about
100 pcm.
