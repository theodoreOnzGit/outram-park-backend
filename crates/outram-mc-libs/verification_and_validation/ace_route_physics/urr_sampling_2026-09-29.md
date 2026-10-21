# URR probability tables sampled as OpenMC samples them (GitHub #365 audit)

*2026-09-29. Research, education and V&V only; not for any operational,
licensing or safety use (see `RESPONSIBLE_USE.md`).*

Both data routes share this code. The ENDF route's PURR tables and the ACE
route's UNR block both become `UrrProbabilityTables`, and both are applied by
`Nuclide::xs_at_energy_urr`.

## What differed from upstream

OpenMC's `Nuclide::calculate_urr_xs` (`src/nuclide.cpp`) and
`UrrData::energy_in_bounds` (`include/openmc/urr.h`) do three things this port
did differently:

| # | OpenMC | this port before | now |
|---|---|---|---|
| 1 | interpolates between the two bracketing tables, drawing the band in each with the same ξ (lin-lin for `INT = 2`, log-log for 5) | band from the **nearest** table energy | OpenMC's rule |
| 2 | `total = elastic + inelastic(ILF) + capture + fission` ("sum of partials instead of the table-provided value") | total from the band's total column, so the elastic remainder in the collision kernel absorbed any mismatch | OpenMC's rule |
| 3 | tables apply strictly inside `(E_first, E_last)` | inclusive at both ends | strict |

The band pick is also aligned. OpenMC takes the first band whose cdf
**exceeds** ξ (`upper_bound_index + 1`); this port previously took the first
band whose cdf reached ξ. They differ only when ξ lands exactly on a band edge.

## Prediction, recorded before measuring

The prediction was posted on #365 before any k_eff run.
- **Item 2.** On U-235 and U-238 the band total and the sum of partials agree on
  the band-probability-weighted mean to <1e-4 b.
  - The largest single-band mismatch is 3.5 b, in U-235's lowest bin; elsewhere
    it is ≤0.07 b.
  - U-234 (LSSF = 0) agrees on the mean exactly.
- **Item 1.** The band-weighted mean of every factor is 1 at every table
  energy, so interpolation cannot move the mean cross section to first order.
  It moves only the self-shielding correlation between table points.
- **Predicted |Δk|:** ≤ 20–30 pcm, no sign predicted, on Godiva and Jemima and
  on both routes. The fix was **not** expected to explain Jemima's +124 pcm.

## Verification

`crates/outram-mc-libs/tests/urr_sampling_vs_openmc.rs`.

**Reference.** `calculate_urr_xs` transcribed in
[`openmc_inputs/urr_sampling_reference.py`](openmc_inputs/urr_sampling_reference.py),
on OpenMC's reader (0.16.1.dev25, `d7d3284a1`) of the NJOY2016
U-234/235/238 tables, written to
[`data/urr_sampling_openmc.csv`](data/urr_sampling_openmc.csv). It covers:
- interior table points and bin interiors, crossed with six ξ values including 0;
- LSSF = 0 (U-234) and LSSF = 1 (U-235, U-238);
- ILF = 51 and ILF = 4.

**Metric.** `xs_at_energy_urr(E, 293.6, ξ)` must reproduce OpenMC's elastic,
fission, capture, inelastic and total to 1e-9 relative. The total must also
equal the sum of the partials.

**Result.** 186 rows agree; the worst relative difference is **0.0**.

**Mutations, run and reverted:**
- The old sampler fails at U-234, 1600.0005 eV: elastic off by 35 %.
- The old band-total rule fails at U-234, 1700 eV: total off by 9.4e-4.

**Knock-on test change.** In njoy-outram-park-fork,
`unr_block_write_vs_njoy2016::build_full_with_purr...` now samples:
- interior grid energies only, because of the strict bounds;
- off-edge ξ (0.037, 0.51, 0.943), because at ξ = 0.95, exactly a PURR band
  edge, the 7-figure rounding of the cdf decided the band.

Its 1e-6 tolerance is unchanged.

## k_eff effect (32 seeds, campaign settings, 4 threads, `target/ace_parity_366/`)

Before: route 3 at `a15958912c`, and route 4 re-run at `a15958912c` (which
reproduced the campaign's route-4 values exactly). After: the same with this
change. The URR draw desynchronises the streams after the first unresolved
collision, so the per-seed differences are not strongly correlated. The σ
below is the paired sem.

| case | route | before | after | after − before [pcm] |
|---|---|---|---|---|
| Godiva | 3 (ACE) | 0.99962 ± 0.00035 | 0.99985 ± 0.00031 | +23 ± 37 |
| Godiva | 4 (ENDF) | 0.99933 ± 0.00035 | 0.99963 ± 0.00029 | +29 ± 33 |
| Jemima | 3 (ACE) | 0.99718 ± 0.00034 | 0.99698 ± 0.00032 | −20 ± 43 |
| Jemima | 4 (ENDF) | 0.99699 ± 0.00029 | 0.99689 ± 0.00037 | −11 ± 33 |

**Against OpenMC (route 1), after:**
- Godiva: route 3 −31 ± 37 pcm, route 4 −53 ± 36 pcm.
- Jemima: route 3 **+104 ± 38** pcm, route 4 **+95 ± 43** pcm.

## Interpretation

- **The prediction held.** Every shift is consistent with zero and within the
  predicted 20–30 pcm.
- **The change is kept because it is OpenMC's algorithm**, verified bit-equal,
  not because it moves k.
- **URR sampling is excluded as the cause of Jemima's residual against OpenMC.**
  That residual is shared by the ACE and ENDF routes, so it lies in transport
  or physics common to both.
- **A remaining ACE-reader gap that is also common to both routes:** delayed
  neutrons are born with the prompt χ, because DNED is not read and the ENDF
  route carries no delayed spectrum either. It is recorded on #365 as the next
  port that can move both routes.
