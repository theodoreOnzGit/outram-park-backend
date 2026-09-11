# BROADR — Doppler broadening (SIGMA1 kernel)

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** All code in this workspace is **unverified and untrusted** unless a specific verification & validation (V&V) case demonstrates otherwise. V&V cases are human-reviewed and are intended for journal / arXiv publication — that is the trust workflow. See the workspace `VERIFICATION_AND_VALIDATION.md` and `RESPONSIBLE_USE.md`. Not for nuclear facility operation, reactor control, safety-critical, or licensing decisions.


> NJOY2016 module port. Theory summarised from the NJOY2016 manual
> (LA-UR-17-20093, §BROADR); upstream Fortran: `broadr.f90` (~2.0k lines).

## Theory

BROADR generates Doppler-broadened cross sections σ(E, T) from the 0 K
piecewise-linear σ(E) written by RECONR. It uses the **SIGMA1** kernel-broadening
method (D. E. Cullen): the effective cross section seen by a neutron of speed *v*
against a Maxwellian target gas at temperature *T* is

```
σ̄(v) = (1 / v²) · ∫₀^∞ σ(u) · u² · [exp(−(u−v)²·β²) − exp(−(u+v)²·β²)] · (β/√π) du
```

with β² = A·m_n / (2·k·T). "Kernel broadening" is fully accurate — it broadens
resonance and smooth cross sections together with no narrow-resonance
approximation, and reproduces the free-gas scattering kernel exactly.

## How the port implements it

The **SIGMA1 kernel is ported** in [`crate::broadr`]: it consumes a reconstructed
0 K grid and returns σ(E, T) on a (possibly re-thinned) grid, reached through
[`crate::interface`]. Broadening is done in the analytic exponential-integral
form so that adjacent linear panels of σ(E) integrate in closed form against the
Gaussian kernel.

This `modules::broadr` entry is the card-input **driver** (temperature list,
thinning tolerance, tape I/O) and is deferred with the NJOY `main` driver.

The output grid is **BROADR's own adaptive grid**, not the input grid —
`broadn` (`broadr.f90:1256-1508`, ported 2026-09-10 in `broadn.rs`,
`broadn_section`): the walk over the input grid picks *nodes* (slope-sign
change, "round" 3-figure energy, 0.0253 eV, every 10th point, energy
doubling), bisects each node interval on a stack until the kernel at the
midpoint is within `errthn` of the chord (`errmax`/`errint` relaxation and
the `rmax = 3` ratio guard as upstream), drops the input points it skipped
(that is BROADR's thinning: U-238 961k → 133k points below `thnmax`, NJOY
448k → 155k) and inserts midpoints where the broadened function needs them.
For a light nuclide that is the whole thermal `1/v` rise: before this port
the grid-preserving kernel gave H-2 elastic 3.0× (1e-3 eV) and 5.2×
(1e-2 eV) NJOY's PENDF, agreeing only *at* the surviving grid points (bead
`op-tubm`, found by the ERRORR tier-2 golden test).

Two deliberate deviations, both documented in `broadn.rs`: the stack ceiling
is 40 instead of upstream's 12 (upstream never fills it because it walks
RECONR's *union* grid; this crate keeps each reaction on its own grid, and
eleven halvings from H-2's next point at 100 eV only reach 0.12 eV, so the
thermal rise was accepted as one chord), and the last input point below
`thnmax` is always a node (upstream judges the seam interval against the
`thnmax` node's broadened value and then writes that node's unbroadened
copy, so the resolved side of the seam is wrong unless that point is a node
— NJOY's U-238 PENDF keeps `(19999.99 eV, 0.2809805 b)`). Reactions are
broadened one at a time (`nreac = 1`), across sections in parallel
(`rayon`); the joint multi-reaction walk of upstream is not reproduced, so
grids differ from NJOY's union grid while the values agree to `errthn`.

**Validated (`tests/broadr_light_nuclide_pendf_golden.rs`, 2026-09-10)**
against NJOY2016 `tape22` PENDFs for H-2, Be-9, Li-6, C-12, F-19, Si-30
(`reconr 0.001 / broadr 293.6 K, errthn 0.001`, 400 log-spaced samples per
reaction): elastic within 6e-4 everywhere (H-2 5.8e-4, Be-9 2.3e-4, Li-6
3.6e-4, C-12 1.8e-4, F-19 7.0e-4, Si-30 1.1e-3); capture 8–9e-3 at tens of
keV where NJOY converges ~1e-5 b cross sections only to `errmax = 1 %`
(`op-428f`). Residuals recorded on their own beads, printed by the test
but not asserted: the SIGMA1 kernel's low-`y` treatment of `1/v` (H-2
capture 2.3 % low at 1e-5 eV, 0.17 % at 1e-4, `op-0xv5`) and RECONR
under-resolving Si-30's inter-resonance capture (13 % at 3.3 keV,
`op-yr43`). The C-12 comparison only became possible once
`parse_endf_float` accepted Fortran `E`-exponent fields (`op-sti5`).

`doppler_broaden` (the unbounded kernel on the input grid, used by the
kernel-level tests) remains **data-parallel** (`rayon`) per point: each
`bsigma_scalar` call is an independent pure function of the shared,
immutable 0 K grid.

## The wing-pedestal investigation (2026-07-07) — it wasn't SIGMA1

A U-238 code-to-code verification against OpenMC
(`tests/u238_doppler_verification/`) surfaced a striking artefact: after
broadening, U-238 capture carried a spurious **~200 b flat pedestal** several eV
past every strong resonance, where OpenMC (reference NJOY) had decayed to ~1 b
(e.g. 105 eV, 900 K: OpenMC 1.8 b vs the port 211 b). It looked like a SIGMA1
kernel bug — a wing that decays far too slowly.

It was **not** the kernel. Direct kernel checks and grid dumps traced it
entirely to **RECONR's input grid being too coarse in the resonance wings**: the
0 K reconstruction left multi-eV gaps between resonances (for the 102.56 eV line,
nothing between 103.03 eV and 111.10 eV), so the true Lorentzian wing was
represented by a single straight line ~35× too high, and SIGMA1 was both *fed*
that over-stated wing and *sampled* on the same coarse output grid. Fixing the
reconstruction grid density (`reconr::refine_resonance_grid`, adaptive bisection
to tolerance — see `../reconr/README.md`) dropped the U-238 capture RRR L1 vs
OpenMC from **≈0.30 → ≈0.0007** with **no change to the SIGMA1 code at all**.

The lesson worth recording: a "broadening" artefact was really an upstream
reconstruction-grid deficiency. When a broadened wing looks wrong, check the
input grid the kernel is integrating (and the output grid it is sampled on)
before suspecting the kernel.

## Testing

**Ported and verified** — `crate::broadr` unit tests plus the U-238 Doppler
study (`docs/keff-doppler-roadmap.md`): BROADR-broadened capture is the
independent kernel oracle cross-checked against the WMP analytic-broadening path
(`crate::wmp`) at the 6.67 eV resonance. With the reconstruction-grid fix above,
the full RECONR→BROADR pipeline now reproduces OpenMC's Doppler-broadened U-238
capture across the whole resolved region to **<0.1% L1** at 900 K and 1200 K.

## The upper limit `thnmax` and the resolved/unresolved seam (fixed 2026-09-10)

Upstream BROADR never broadens the whole grid. It stops at `thnmax`
(`broadr.f90:107-124`, `:354-441`): by default the top of the resolved
resonance region as RECONR recorded it on the PENDF MF=2 range record (or the
start of the unresolved region if there is no resolved one, or the lesser of
6.5 MeV and the first threshold for a non-resonance nuclide), never above the
evaluation's `EMAX`. Points above `thnmax` are copied through verbatim
(label 190). The reason is physical: above the resolved region the
evaluation holds energy-*averaged* values, not the pointwise σ(E) the SIGMA1
kernel assumes.

Until 2026-09-10 this port had no such bound (`op-sdbk`). On U-238 that
destroyed the 20 keV seam: MF=3/MT=102 steps from 0.0 b (resolved side; the
resonances carry it) to 0.52987 b (the evaluator's infinitely-dilute
unresolved value), and the unbounded kernel smeared the step into the
unresolved side — 46.7 % low at the seam and, because the next MF=3 point is
24 keV, 41 % low at 20.5 keV and 25 % low at 22 keV
(`examples/seam_stage_probe.rs`, 600 K).

Two things had to change, and both are what upstream does:

1. **RECONR shades discontinuities** (`reconr.f90` `rdfil2` "shade nodes to
   prevent discontinuities", `lunion` "check ahead for discontinuity"): a
   step at `E` is written as `sigfig(E,7,-1)` / `sigfig(E,7,+1)`
   (1.999999e4 / 2.000001e4 for U-238), never as two points at one energy.
   This is what makes a bound at `E` meaningful — an index-based grid walk
   cannot put a limit *between* two identical energies. See
   [`crate::reconr::shade_discontinuities`] and `rebuild_range`.
2. **BROADR bounds broadening at `thnmax`** — [`crate::broadr::broadening_limit`]
   and [`crate::broadr::doppler_broaden_below`] / [`crate::broadr::broaden_result`],
   which the pipeline ([`crate::interface`], `outram-mc-libs`) now uses. The
   unbounded [`crate::broadr::doppler_broaden`] remains as the bare kernel.

After the fix the unresolved side is bit-identical to the tape from
2.000001e4 eV up; the resolved-side point at 1.999999e4 is broadened and
sees the step above it, exactly as upstream's does (`bsigma` integrates over
all three loaded pages). Regression test:
`tests/broadr_u238_urr_seam.rs`.

**Checked against NJOY2016 itself (2026-09-10).** Upstream (commit
`ac5adf5`, gfortran 13.3.0) run on the same tape with `reconr 0.001` and
`broadr 600 K` writes `1.999999e4 → 0.2809805 b` and `2.000001e4 →
0.5298699 b` for MT=102, with lin-lin values 0.52262 / 0.51536 / 0.50086 /
0.48635 b at 20.5 / 21 / 22 / 23 keV and "final maximum energy for
broadening/thinning = 2.00000E+04 eV" in its listing. This port gives
0.2809594 and 0.529870 at the two shaded points and the same four
interpolants to every printed digit. (Before the fix: 0.306 at the seam,
0.30593 at 20.5 keV.)

**A negative result worth keeping (2026-09-10).** The same probe checked the
hypothesis that the unbounded kernel also smeared fast-neutron thresholds
(U-238 MT=18 rise near 1 MeV, MT=16 onset at 6.179 MeV) and so depressed
fast fission. It does not: even unbounded, MT=18 moves by ≤ 0.04 % over
0.8–2 MeV and MT=16 by ≤ 0.07 % at 6.3 MeV (1e-6 b at the onset), as the
~30 eV Doppler width at 1 MeV predicts. A fast-fission discrepancy is not a
BROADR effect.

## Caveats

- Only free-gas broadening — bound/crystalline effects at very low energy are
  THERMR's job (S(α,β)), not BROADR's.
- Thinning tolerance (`BroadnTolerances`, default `errthn = 0.001`) trades
  grid size against accuracy; keep it tighter than the downstream ACER
  tolerance.
- The `run()` driver returns `NotPorted`; use `crate::interface`.

## References

- NJOY2016 manual §BROADR (LA-UR-17-20093)
- `broadr.f90` (NJOY2016 2016.79)
- D. E. Cullen, "SIGMA1" / Cullen–Weisbin kernel broadening
