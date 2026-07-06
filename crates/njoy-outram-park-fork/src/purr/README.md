# PURR — unresolved-resonance probability tables

> NJOY2016 module port. Theory summarised from the NJOY2016 manual
> (LA-UR-17-20093, §PURR); upstream Fortran: `purr.f90` (2919 lines).

## Theory

The Bondarenko self-shielding data from UNRESR is fine for multigroup methods but,
as Levitt observed, not directly usable by continuous-energy Monte Carlo. PURR
implements the **probability-table** method for the unresolved resonance range.

At each URR energy grid point PURR samples many explicit **resonance ladders**
from the ENDF average widths and spacings (with the correct χ²/Wigner statistics),
computes the total and partial cross sections for each ladder, and **bins** them
into a small number (typically 20) of equiprobable probability bins. The table
stores, per energy and temperature, the bin cross sections and their probabilities
— a discrete sampling of the cross-section probability distribution P(σ | E). A
Monte-Carlo code then samples a σ from this table on entering the URR, capturing
resonance self-shielding statistically without resolved resonances. An analytic
infinite-dilution reference (the same width-fluctuation theory UNRESR uses, in
its `σ₀→∞` limit) is computed independently as a convergence check on the tables.

## How the port implements it

**Ported** (the scaffolding around the Monte Carlo core — see the Caveats for
what is deferred):

- **ENDF parsing** — `rdf2un`/`rdf3un`/`unfac2`/`intrf2`/`intr2` are
  structurally identical to UNRESR's `rdunf2`/`rdunf3`/`uunfac`/`intrf`/`intr`
  (verified by direct comparison, same Case A/B/C field layout). This module
  reuses [`crate::unresr::mf2`] and [`crate::unresr::penetrability_factor`]
  directly rather than re-deriving a duplicate parser.
- **`wfun::uw2`** — PURR's own complex probability integral evaluator.
  Algorithmically identical to [`crate::unresr::wfun::uw`] (same break-point
  regions, same asymptotic/Taylor series), reusing that module's
  `WRecurrence` continued-fraction step rather than re-deriving it, with one
  genuine addition ported faithfully: an exactness shortcut for `Re(z)=0`
  (`purr.f90:2693`, `2736`), absent in `uw`.
- **`Rng`** — `rann`, NJOY's shuffled-LCG pseudo-random generator (a
  Numerical-Recipes-`ran1`-style algorithm), ported deterministically.
- **`generate_ladder`** — `ladr2`, sampling one resonance ladder (Wigner
  spacing, χ²/Porter-Thomas widths via a 20-quantile-bin table) for one
  sequence.
- **`infinite_dilution_reference`** (+ private `gnrx`) — `unresx`, the
  analytic infinite-dilution cross-section reference. Reuses
  `crate::unresr`'s `range_sequences`/`channel_radius_urr`/
  `penetrability_factor` (made `pub(crate)` for this purpose) so the L/J-state
  flattening logic is verified once, not twice.
- **`read_heating_cross_sections`** — `rdheat`, reading HEATR's partial
  heating cross sections (MT=301/302/318/402) from the PENDF tape, reusing
  `endf::interp::eval_tab1`.

## Testing

**TODO** (Opus verification pass — no tests were written as part of this
translation, per the crate's model-division-of-labour rule in `CLAUDE.md`).
Gate: reproduce upstream PURR probability tables for a URR nuclide (e.g.
U-238) within statistical tolerance (bin cross sections + probabilities depend
on ladder count and RNG seed — see Caveats), and confirm the table-averaged σ
recovers the `infinite_dilution_reference` limit. The ported pieces are
independently checkable before `unrest` lands: `Rng` against a known `rann`
output sequence, `generate_ladder`'s Wigner spacing against the analytic mean
spacing, `wfun::uw2` against `crate::unresr::wfun::uw` (should agree everywhere
except exactly on the imaginary axis), and `infinite_dilution_reference`
against UNRESR's `unresolved_cross_sections` at large σ₀ (both should converge
to the same infinite-dilution limit, since both use the identical ETOX
width-fluctuation theory in that limit).

## Caveats

- **`unrest` (`purr.f90:1789-2543`, ~750 lines) is not ported.** This is the
  actual Monte Carlo probability-table binning core: Monte Carlo energy
  sampling, per-resonance Doppler line-shape evaluation across **six**
  precision regimes (asymptotic → two rational-approximation tiers → two
  table-lookup variants depending on Doppler width `y` → back down — needing
  its own two-table `w(z)` lookup, `uwtab2`, also not ported), a dynamic
  non-uniform histogram bin-edge scheme, and simultaneous probability-table +
  Bondarenko-moment accumulation. Deferred as an explicit TODO — deliberately,
  after reading the full routine — rather than rushing the single most
  numerically delicate routine encountered in this crate's NJOY port so far
  (comparable in *kind* to BROADR's still-open wing-fidelity issue: both are
  regime-boundary Doppler line-shape bugs waiting to happen, but `unrest` is
  substantially larger). Matches the precedent set by deferring HEATR's H6.
- **PENDF MT=152/MT=153 output-tape bookkeeping is not ported** — this crate
  has no established PENDF output-section-writer concept yet (unlike ACE),
  and it is pure tape plumbing, not physics. `run()` remains `NotPorted`.
- Inherently **stochastic** (once `unrest` lands) — results depend on ladder
  count and RNG seed; reproducibility against the Fortran oracle is
  statistical, not exact, *unless* `Rng` is seeded identically, in which case
  the *same* pseudo-random sequence is reproduced deterministically (the RNG
  itself is a faithful, deterministic port).
- Requires an evaluation with a URR; feeds the ACE UNR block (when `unrest`
  and the ACE writer integration eventually land), so it is coupled to ACER
  4-series progress.

## References

- NJOY2016 manual §PURR (LA-UR-17-20093)
- `purr.f90` (NJOY2016 2016.79)
- L. B. Levitt, "The probability table method…", Nucl. Sci. Eng. (1972)
