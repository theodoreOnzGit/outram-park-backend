# UNRESR — unresolved-range self-shielded cross sections

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** All code in this workspace is **unverified and untrusted** unless a specific verification & validation (V&V) case demonstrates otherwise. V&V cases are human-reviewed and are intended for journal / arXiv publication — that is the trust workflow. See the workspace `VERIFICATION_AND_VALIDATION.md` and `RESPONSIBLE_USE.md`. Not for nuclear facility operation, reactor control, safety-critical, or licensing decisions.


> NJOY2016 module port. Theory summarised from the NJOY2016 manual
> (LA-UR-17-20093, §UNRESR); upstream Fortran: `unresr.f90` (1665 lines).

## Theory

In the **unresolved resonance range (URR)** individual resonances are too dense
to resolve, so ENDF File 2 (LRU=2) gives only *average* resonance widths and
level spacings plus χ²/Porter-Thomas distribution functions. UNRESR converts
this statistical description into **effective self-shielded cross sections**
using the ETOX/MC2-2 method (`unresx.tex`'s theory section): the flux depresses
inside a resonance as `φ(E) ∝ 1/(σ₀ + σ_t(E))ˡ`, and the effective cross
section reduces to two "fluctuation integrals" `I_0x`/`I_1t` evaluated by
quadrature over the Porter-Thomas width distributions (Gauss nodes weighted by
degrees of freedom μ = 1..4), summed over resonance sequences with in-sequence
and sequence-sequence overlap corrections.

## How the port implements it

**Ported** — the physics kernel is complete, split across three files:

- **`mf2.rs`** (`rdunf2`/`rdunf3`) — the ENDF File-2 LRU=2 parameter reader.
  Where upstream packs every l/j-state into one flat scratch array walked by
  hand-tracked offsets, this port follows `docs/porting-plan.md` §5's
  "scratch arrays → owned structs" convention: [`UnresolvedCase`] (Case
  A/B/C, per ENDF-102's three LRU=2 representations — LFW=0 fully
  energy-independent, LFW=1 only the fission width tabulated, LRF=2 every
  parameter tabulated) with named fields, cross-checked both against how
  `rdunf2` *writes* each slot and how `unresl` *reads* it back (one field,
  Case B's per-J `L2` header word, is genuinely `AMUF` — not the more
  obvious guess `AJ` — confirmed by that cross-check). `rdunf3`'s File-3
  background read reuses the existing `endf::interp::eval_tab1`.
- **`wfun.rs`** (`uw`/`uwtab`/`quikw`/`ajku`) — the complex probability
  integral (Faddeeva function) `w(z)`, a dual-series (asymptotic
  continued-fraction / Taylor) evaluator, pre-tabulated on a 62×62 grid with
  local biquadratic lookup, feeding the J/K width-fluctuation-integral
  quadrature. This crate's `crate::wmp::faddeeva` evaluates the same
  mathematical function elsewhere (WMP Doppler broadening) but is
  **deliberately not reused here** — see `wfun.rs`'s module docs for why a
  faithful port keeps NJOY's own numerical path distinct.
- **`mod.rs`** (`unresl` + `uunfac`/`intrf`/`intr`) — the per-energy
  self-shielded cross-section calculator: penetrability/phase-shift
  (`penetrability_factor`, a genuinely different formula from RECONR's
  resolved-region penetrability — see its doc comment), Case-B/C parameter
  interpolation (reusing `endf::interp::terp1`, both forced to linear-linear
  per NJOY's own hard-coded override), the two-pass potential-scattering +
  fluctuation-quadrature calculation, and the final cross-sequence overlap
  correction.

Fortran's `arry(*)` scratch buffer becomes named structs throughout; Fortran's
computed-`GOTO` "shared subroutine" trick in `uw` (three re-entry labels
dispatched by `kw`) becomes an explicit shared step method
(`WRecurrence::step`) called from two separate driving loops — a mechanical
translation Rust's lack of `GOTO` requires, not an algorithmic change.

## Testing

**TODO** (Opus verification pass — no tests were written as part of this
translation, per the crate's model-division-of-labour rule in `CLAUDE.md`).
Gate: reproduce upstream UNRESR effective cross sections for a nuclide with a
URR (e.g. U-238, unresolved 20–149 keV) versus dilution σ₀ and temperature,
within tolerance against the Fortran oracle. The 62×62 `w(z)` table and the
`ajk` quadrature are cross-checkable independently of ENDF parsing (pure
numerics — build a `WTable` and compare `ajk` against known asymptotic limits
before validating the full per-energy cross section).

## Caveats

- **`NRO=1`** (energy-dependent scattering radius, `NAPS=2`) is explicitly
  **rejected** (`NjoyError::EndfParse`), not silently mis-parsed — no ranges
  in common evaluations were available to verify this path against.
- **PENDF MT=152 output-tape bookkeeping is not ported.** This crate has no
  established "write an unresolved-region PENDF section" concept yet (unlike
  ACE); `unresolved_cross_sections` returns the computed cross sections
  directly rather than writing them back to a tape. `run()` remains
  `NotPorted`, matching every other module's driver-vs-typed-API split.
- **Sequence numbering / temperature batching:** this port evaluates one
  temperature per call rather than Fortran's `ntempu`-batched loop — a
  calling-convention simplification (call once per temperature), not a
  physics difference.
- Bondarenko/narrow-resonance self-shielding is **not** well suited to
  continuous-energy Monte Carlo on its own — for CE self-shielding use
  **PURR** probability tables (`../purr/README.md`), UNRESR's usual next step
  and this crate's next porting target.
- Requires an evaluation with an actual unresolved range; smooth-only
  nuclides produce an empty sequence list (all cross sections come from
  `sigbkg` alone).

## References

- NJOY2016 manual §UNRESR (LA-UR-17-20093)
- `unresr.f90` (NJOY2016 2016.79)
- I. I. Bondarenko et al., group constants for reactor calculations (1964)
- MC2-2 / ETOX codes — the quadrature scheme UNRESR's theory chapter credits
