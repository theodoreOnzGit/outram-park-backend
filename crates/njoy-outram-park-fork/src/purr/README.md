# PURR — unresolved-resonance probability tables

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** All code in this workspace is **unverified and untrusted** unless a specific verification & validation (V&V) case demonstrates otherwise. V&V cases are human-reviewed and are intended for journal / arXiv publication — that is the trust workflow. See the workspace `VERIFICATION_AND_VALIDATION.md` and `RESPONSIBLE_USE.md`. Not for nuclear facility operation, reactor control, safety-critical, or licensing decisions.


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

**Verification pass executed 2026-09-10** — every step of the ladder below
now has a test, and steps 4 and 6 run against NJOY2016 itself (built
in-session from `upstream_source/NJOY2016`, gfortran 13.3.0). The pass found
**four defects**, all fixed and pinned by the tests named:

| # | Defect | Symptom | Test |
|---|---|---|---|
| 1 | `wfun::uw2` (and UNRESR's `uw`) inverted the last break-line test — a false `aimz-brk9.ge.zero` falls through to the *Taylor* series upstream | `w(2.13+0.001i)` 37× low; UNRESR's `WTable` nodes near (1.8–2.3, 0–0.3) wrong | `uw2_matches_gfortran_oracle` |
| 2 | `wfun::DopplerTable` lookups used upstream's 1-based `ii+2`/`jj-3`/`jj+2` on 0-based rows | every table lookup one cell off in *x* and *y* | `doppler_table_is_exact_at_its_own_nodes` |
| 3 | `infinite_dilution_reference` used UNRESR's un-folded `V_l` with `unresx`'s `/nu` — upstream `unfac2` folds `AMUN` into `V_l` first | every `AMUN=2` sequence's neutron width halved: U-235 fission/capture 1.7 % low at 2.25 keV, 6 % at 10 keV | `tests/purr_u235_urr.rs` |
| 4 | `reconr::slbw::WAVE_K` was `2.1977e-3`, the `cwaven` formula rounded up in the 4th figure | all `1/k²` resonance terms 0.081 % low in RECONR/UNRESR/PURR | `wave_k_is_the_upstream_cwaven` |

Results on ENDF/B-VIII.0 U-235 (URR 2.25–25 keV, `LSSF=1`) against the NJOY
run `reconr 0.001 / broadr 300 K / unresr / purr` (`sigz 1e10 … 1`, `nbin 20`,
`nladr 32`):

- `infinite_dilution_reference` reproduces NJOY's `unresx` (`spot`, `dbar`,
  total/elastic/fission/capture) to the 7 figures NJOY writes at 2.25, 5.5
  and 10 keV, and agrees with this crate's own UNRESR at σ₀ = 10¹⁰ to all
  digits.
- `probability_table` at NJOY's first energy with the same seed: probabilities
  sum to 1, table means reproduce the renormalised reference, Bondarenko
  moments monotone in σ₀, and the σ₀ = 1 b self-shielding ratios match NJOY's
  direct sampling to 0.1 % (0.9562/0.9909/0.9037/0.8968 vs
  0.9557/0.9904/0.9035/0.8956). Absolute 32-ladder means agree within the
  Monte Carlo noise for total/elastic/fission; capture is 2.4 % low where
  NJOY's own mean sits 2σ above its analytic reference. The two random
  streams are **not** bit-identical despite the same seed — not chased.

**U-238 (2026-09-10, `tests/purr_u238_urr.rs`)** — the `NRO=1` gate is closed
(`op-as32`; see `../unresr/README.md`). Same deck on ENDF/B-VIII.0 U-238
(URR 20–149.0087 keV, `LSSF=1`, no fission, competition from 45.1 keV):
`infinite_dilution_reference` reproduces NJOY's `spot`/`dbar` and the
7-figure `unresx` cross sections at 20 and 40 keV (below the competition
threshold — PURR's `infd` *total* above it adds the PENDF `sigx`, which this
port does not read); `probability_table` at 20 keV with seed −101 gives
32-ladder means 14.3509/13.8394/0.51146 vs NJOY 14.352/13.841/0.51136
(pcsd 1.85/1.87/2.99 vs 1.81/1.83/2.99) and σ₀ = 1 b self-shielding ratios
0.8758/0.8759/0.8719 vs 0.8764/0.8766/0.8707 — a far stronger shielding case
than U-235 (D = 2.4 eV vs 0.16 eV). The verification ladder, easiest first:
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

Steps 1–3 and 5 are unit tests in `tests.rs`/`wfun.rs` (`Rng` against a
verbatim-`rann` gfortran oracle for `idum=-101`; `uw2` against a
verbatim-`uw2` oracle at 16 points; Wigner spacing mean/width; all four
`line_shape` tiers against `uw2` across every boundary, and tier *selection*
pinned bit-for-bit against the closed-form tier formulas).

## Caveats

- **Runtime-tested on U-235 and U-238 only** — see Testing above. The
  bin-edge construction (`purr.f90:2283-2319`'s dynamic non-uniform
  schedule) is exercised by both tests (edges ascending, last edge `1e6`,
  probabilities summing to one) but its values have not been compared
  bin-by-bin against NJOY's `MT=153` output.
- **Competition `sigx` is the caller's to supply.** Under `LSSF=1`,
  `rdf3un`'s "sanity check" (`purr.f90:1195-1215`) rewrites the File-3
  background as `bkg = [σ_total − σ_el − σ_f − σ_γ, 0, 0, 0]` — i.e. the
  competing-reaction cross section at the working energy — and `unrest`
  adds `bkg(1)` to its infinite-dilution and sampled *totals*
  (`purr.f90:2378`). This port's [`infinite_dilution_reference`] takes only
  the File-2 parameters, so its total is elastic + fission + capture; pass
  `bkg = [sigx, 0, 0, 0]` (derived from the PENDF as `rdf3un` does) to
  [`probability_table`] to reproduce NJOY's totals above a competition
  threshold. U-238 above 45.1 keV is the case in point (`sigx` = 0.17 b at
  60 keV, 0.52 b at 100 keV); the ENDF `GX` width enters the ladders
  regardless.
- At the closed edges of the table tier (`|x| = 3.9`, `y = 3.0` exactly)
  upstream reads one row/column past its `tr`/`ti` arrays (an unchecked
  out-of-bounds read); [`wfun::DopplerTable`] clamps the cell and
  extrapolates the quadratic instead.
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
