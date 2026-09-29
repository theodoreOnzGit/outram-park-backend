# Correlated angle laws and histogram cosine tables as OpenMC (GitHub #365 audit)

*2026-09-29. Research, education and V&V only; not for any operational,
licensing or safety use (see `RESPONSIBLE_USE.md`).*

## What changed

| piece | before | OpenMC, and now | upstream |
|---|---|---|---|
| law-61 cosine row | the lower edge `k` of the `E′` bin | `k` or `k+1`, whichever cdf edge is nearer the draw, on a lin-lin table; `k` on a histogram table | `secondary_correlated.cpp`, `CorrelatedAngleEnergy::sample_dist` |
| cosine inverse within a row | linear in the cdf | quadratic for lin-lin, linear for histogram | `distribution.cpp`, `Tabular::sample_unbiased` |
| law-44 `r`, `a` | row `k`'s values | interpolated to the raw `E′` (before the envelope scaling) on lin-lin tables | `secondary_kalbach.cpp`, `KalbachMann::sample_params` |
| AND and law-61 rows with `intt = 1` | refused | histogram | `angle_distribution.py`, `correlated.py` |
| AND 32 equiprobable bins | read lin-lin, the end densities borrowed | histogram | `angle_distribution.py:176-185` |
| `EnergyAngular::mean_cosine` | lin-lin formula | exact for histogram rows too | — |

**Where the changes reach:**
- The row rule and the inverse reach the ENDF route too, through the
  linearised MF=6 Legendre rows. Those are the rows NJOY's ACER turns into
  law 61, so the two routes stay the same law.
- LAW=7 lab-tabulated rows (Be-9) take the same rule. The oracle of
  `tests/law7_lab_angle_energy_transport.rs` was updated to match, and is now at
  0.78σ. The old oracle, run against the new sampler, reads 6.36σ.

**No variate is added or removed** by any of these changes, so the RNG stream
stays aligned between the old and new code.

Code:
- `ContinuumAngularRow::{pdf, histogram}`, `AnglePick`, and
  `ContinuumAngular::sample_mu_at` in `njoy_outram_park_fork::nuclear_data::secondary`;
- `sample_ct_table_indexed` and `sample_mf4_mu_cm` in
  `outram_mc_libs::material::nuclide`;
- `physics::scatter::sample_continuum_branch`, which both the kernel and the
  test call.

## Methodology

`openmc_inputs/correlated_angle_reference.py` reads each table with OpenMC's
own ACE reader. It computes the **exact** `⟨μ⟩` and `⟨μE′⟩` of OpenMC's
sampling scheme:
1. take incident table `i` or `i+1` with probability `1−r` or `r`;
2. within each `E′` bin, the energy variate is uniform, so split the bin at
   its cdf midpoint;
3. integrate each piece with 48-point Gauss-Legendre in the variate. Each row's
   own mean cosine is in closed form: the Tabular mean, or
   `r (coth a − 1/a)` for Kalbach.

The script writes the same moments under the **old** scheme too, so the test
can show it has the power to tell the two apart.

**Tables:**
- **NJOY2016 U-235**, 293.6 K, ENDF/B-VIII.0, from `reference-data/ace`. Its
  MT=91 and MT=16 are law 61, lin-lin. This is real data.
- **NJOY2016 O-16**, from the five-route build. Its MT=91 is law 44, histogram
  in `E′`, so nothing should move. It is the **control**.
- **Constructed tables.** No table in reach has these forms: every held AND
  and law-61 cosine row is `intt = 2`, and every held law-44 table is a
  histogram in `E′`. So two were constructed:
  - `U235_hist.ace`:
    - every MT=91 cosine row is made a histogram;
    - 72 MT=2 AND rows are replaced by 32 equiprobable bins cut from their
      own cdf;
    - the MT=51 AND rows are made histograms.
  - `O16_kmlin.ace`: MT=91 law 44 is made lin-lin in `E′`, with the pdf
    renormalised and the cdf rebuilt.

**Pass criterion:** over `N = 4e6` draws per row, taken through the transport
path, every mean lies within 5 sample-sem of OpenMC's.

**Power:** every case except the O-16 control must have at least one row more
than 5 sem from the old scheme.

`N` was `1e6` in the first run. Every row agreed with OpenMC then (worst |z|
1.74), but the power gate failed: the two constructed cases sat only 5.0 and
4.8 sem from the old scheme. The gate was kept and `N` was raised.

## Results (2026-09-29)

Test: `crates/outram-mc-libs/tests/correlated_angle_vs_openmc.rs`. 21 rows
and 36 moments. **Worst |z| against OpenMC: 1.96.**

| case | rows | worst \|z\| vs OpenMC | largest separation from the old scheme |
|---|---|---|---|
| U-235 MT=91, 16 (real, law 61) | 6 | 1.95 | 18.3 sem |
| U-235 MT=91, histogram cosine rows (constructed) | 3 | 1.96 | 10.0 sem |
| O-16 MT=91, histogram law 44 (real, control) | 3 | 1.78 | old = new (1.8) |
| O-16 MT=91, lin-lin law 44 (constructed) | 3 | 1.11 | 9.8 sem |
| U-235 MT=2, 32 equiprobable bins (constructed) | 3 | 1.01 | — |
| U-235 MT=51, histogram AND (constructed) | 3 | 1.51 | — |

**Size of the change on real data.** U-235 MT=91 `⟨μ_cm⟩`:

| incident energy | OpenMC (and now) | old scheme |
|---|---|---|
| 5 MeV | 0.02017 | 0.01561 |
| 14 MeV | 0.2799 | 0.2763 |
| 1–2 MeV | ≈ 0 | ≈ 0 |

**Mutations:** each change was reverted in turn, and the test failed each time.

| mutation | fails at |
|---|---|
| lower row always | U-235 MT=91 5 MeV, z = −10.8 |
| no Kalbach interpolation | O-16 lin-lin, z = +10.2 |
| linear-cdf cosine inverse | U-235 MT=16 14 MeV, z = −9.4 |
| cosine histogram read lin-lin | U-235 hist MT=91, z = −10.3 |
| AND histogram read lin-lin | U-235 MT=2 1 MeV, z = −7.6 |

**Analog pin.** `tests/variance_reduction_is_bit_identical_when_analog.rs`
was re-recorded. With the three transport changes reverted in place, it
reproduces its previous four values bit for bit, so the move is these changes
and nothing else. Its first generation moves only in the ninth digit, because
no variate is added.

## k shift (paired A/B, route 3)

**Recorded before the first run** (#365): |Δk| < 10 pcm on Godiva and
< 5 pcm on the LCT-008 lattice, negative if anything. The reasoning: the whole
continuum anisotropy on Godiva is worth +2 ± 13 pcm (400 seeds per arm), and
this is a 20-30 % correction to it, confined to the part of the spectrum above
3 MeV.

**Protocol.**
- Route 3: NJOY2016 ACE, transported by outram-mc.
- The five-route driver `examples/icsbep_five_route_keff.rs`, at its default
  settings.
- Two clean binaries:
  - baseline: `d32bf81af8`;
  - new: the same tree plus this change.
- Paired seeds, 4 threads.
- On Godiva seeds 1-3, both binaries were checked to reproduce the arm
  binaries of the first look bit for bit.
- The baseline is also the new build with only the three transport changes
  reverted, which reproduces it bit for bit. So the rest of this change (the
  histogram readers, the exact `mubar`, the interpolation regions) is
  bit-neutral on the ACE route.

**Godiva.**

| seeds | paired Δk (new − old) | note |
|---|---|---|
| 1-32 | −75 ± 35 pcm | first look |
| 1-128 | −51 ± 20 pcm (2.6σ) | declared extension |
| 129-512 | +3.1 ± 10.6 pcm | declared extension |
| **1-512** | **−10.5 ± 9.4 pcm (1.1σ)** | **the number to quote** |

The 512-seed result is consistent with the prediction. The 128-seed value was
a fluctuation, and all three extensions were declared on #365 before they were
run. The distributional test agrees: the old and new schemes sample the same
continuum cosine at 0.6-2 MeV (KS `D <= 1.42e-3` against a critical `1.95e-3`).

**LCT-008 lattice.** First look, seeds 1-32: −40 ± 18 pcm (2.2σ). An
extension to 96 seeds per arm was declared and is running. LATTICE_PENDING
