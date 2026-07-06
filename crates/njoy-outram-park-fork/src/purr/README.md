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

**Ported** — every piece of the module, including the Monte Carlo core:

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
- **`wfun::DopplerTable`** — `uwtab2`, the two 41×27 `w(z)` lookup grids
  (coarse for `y≥0.5`, fine for `y<0.5`) `unrest` uses instead of calling
  `uw2` at every Monte Carlo sample point. Built from `uw2` the same way
  [`crate::unresr::wfun::WTable`] builds from `uw`; the biquadratic
  interpolation formula is the identical 6-point stencil, only the grid-index
  mapping differs per table (ported precisely, both offset conventions
  cross-checked against `uwtab2`'s own grid construction).
- **`Rng`** — `rann`, NJOY's shuffled-LCG pseudo-random generator (a
  Numerical-Recipes-`ran1`-style algorithm), ported deterministically.
- **`generate_ladder`** — `ladr2`, sampling one resonance ladder (Wigner
  spacing, χ²/Porter-Thomas widths via a 20-quantile-bin table) for one
  sequence.
- **`infinite_dilution_reference`** (+ private `gnrx`) — `unresx`, the
  analytic infinite-dilution cross-section reference. Reuses
  `crate::unresr`'s `range_sequences`/`channel_radius_urr`/
  `penetrability_factor` (made `pub(crate)` for this purpose) so the L/J-state
  flattening logic is verified once, not twice. Also stages the additional
  per-sequence quantities (`csz`, `cth_ref`, `cc2p`, `cs2p`) that only
  `probability_table` needs.
- **`read_heating_cross_sections`** — `rdheat`, reading HEATR's partial
  heating cross sections (MT=301/302/318/402) from the PENDF tape, reusing
  `endf::interp::eval_tab1`.
- **`line_shape`** + **`probability_table`** — `unrest`, the Monte Carlo
  probability-table binning core (see the module docs for the tier-equivalence
  argument, and the caveat below).

### The one deliberate structural deviation: per-point tier classification

Upstream finds, for each resonance, which of its sample points need which of
four Doppler line-shape precision tiers via a **chain of binary searches**
(`fsrch`) on shrinking sub-ranges of the sorted energy array — a 1970s
performance technique for narrowing a large sorted array without re-scanning
it. This port (`line_shape`) instead classifies each sample point **directly**
against the four tier thresholds. These give **identical** results: the
Doppler-scaled offset `xs(ie) = ctx·(es(ie)−E_r)` is a monotonic function of
the (already sorted) `es(ie)`, so a binary-search boundary and a direct
per-point threshold check place the same point in the same tier every time —
verified by tracing every threshold, fall-through, and resonance-level
shortcut in `unrest`'s source (`purr.f90:1950-2204`) down to a single
combined rule: **`|x| > 100` or `y > 100`** → asymptotic; **`|x| > 6` or
`y > 6`** → 2-term rational; **`|x| > 3.9` or `y > 3.0`** → 3-term rational
(note the genuinely asymmetric `3.9`/`3.0` thresholds, ported as-is, not
harmonized); otherwise → table lookup (coarse/fine chosen by `y`). This is a
mechanical simplification of *how the same formula gets selected*, not a
change to any formula.

## Testing

**TODO** (Opus verification pass — no tests were written or run as part of
this translation, per the crate's model-division-of-labour rule in
`CLAUDE.md`; the ported code has been reviewed line-by-line against the
Fortran source but has **not been executed even once**, so treat it as
unverified in the strongest sense until a real run happens).

Gate: reproduce upstream PURR probability tables for a URR nuclide (e.g.
U-238) within statistical tolerance (bin cross sections + probabilities depend
on ladder count and RNG seed), and confirm the table-averaged σ recovers the
`infinite_dilution_reference` limit. Suggested verification order, easiest
first:
1. `Rng` against a known `rann` output sequence (fully deterministic, exact
   match expected).
2. `wfun::uw2` against `crate::unresr::wfun::uw` (should agree everywhere
   except exactly on the imaginary axis) and against `wfun::DopplerTable`
   lookups (should agree to the table's interpolation error).
3. `generate_ladder`'s Wigner spacing against the analytic mean spacing.
4. `infinite_dilution_reference` against UNRESR's `unresolved_cross_sections`
   at large σ₀ (both should converge to the same infinite-dilution limit,
   since both use the identical ETOX width-fluctuation theory there).
5. `line_shape`'s four tiers against direct `uw2` evaluation at representative
   `(x,y)` points spanning every tier boundary (100, 6, 3.9/3.0, 0.5) — this
   is the highest-value check, since it directly probes the tier-equivalence
   argument above.
6. Only then, the full `probability_table` Monte Carlo pipeline end to end.

## Caveats

- **Not runtime-tested at all** — see Testing above. The bin-edge construction
  in particular (`purr.f90:2283-2319`'s dynamic non-uniform schedule) has
  many small index-arithmetic steps (Fortran 1-indexed → Rust 0-indexed) that
  were hand-verified branch-by-branch during translation but never exercised.
- **PENDF MT=152/MT=153 output-tape bookkeeping is not ported** — this crate
  has no established PENDF output-section-writer concept yet (unlike ACE),
  and it is pure tape plumbing, not physics. [`probability_table`] returns the
  computed tables directly; `run()` remains `NotPorted`.
- **`nmode==1`'s renormalization branch is not ported** (`purr.f90:2256-2268`)
  — `nmode` is initialised to `0` in the driver and never set to `1` anywhere
  else in `purr.f90`; this is an always-inactive code path upstream (no card
  or feature reaches it), so there is nothing to reproduce.
- Inherently **stochastic** — results depend on ladder count and RNG seed;
  reproducibility against the Fortran oracle is statistical, not exact,
  *unless* `Rng` is seeded identically and called in the identical order, in
  which case the *same* pseudo-random sequence is reproduced deterministically
  (the RNG itself is a faithful, deterministic port; call-order fidelity to
  upstream has not been specifically verified).
- Requires an evaluation with a URR; feeds the ACE UNR block (once the ACE
  writer integration lands), so it is coupled to ACER 4-series progress.

## References

- NJOY2016 manual §PURR (LA-UR-17-20093)
- `purr.f90` (NJOY2016 2016.79)
- L. B. Levitt, "The probability table method…", Nucl. Sci. Eng. (1972)
