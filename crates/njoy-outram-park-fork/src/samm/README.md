# SAMM — R-matrix-limited (RML) resonance kernel

> NJOY2016 port. `samm.f90` has **no standalone manual chapter** — theory is in
> the NJOY2016 manual §RECONR and the ENDF-102 LRF=7 specification. Upstream
> Fortran: `samm.f90` (7169 lines — the SAMMY method, ported into NJOY from
> coding provided by Nancy Larson, ORNL).

## Theory

`samm` is not a driver module — it is the **R-matrix engine** shared by RECONR and
UNRESR. Where SLBW/MLBW and Reich–Moore (as implemented in `crate::reconr::slbw`)
approximate the resonance cross section with isolated poles, the **R-matrix-limited
(RML, ENDF LRF=7)** formalism computes it from the full multichannel R-matrix:

```
R_{cc'} = Σ_λ  γ_{λc} γ_{λc'} / (E_λ − E)
```

The scattering matrix U (and hence the cross sections) follows from the channel
matrix `(I − R L)⁻¹`, where L carries the penetrabilities/shift factors of each
channel. This handles **overlapping resonances**, multiple particle channels, and
light-nuclide evaluations (¹⁶O, ¹⁹F, …) correctly, where pole approximations fail.

### Scope, matching what `samm.f90` itself supports

Upstream's own `rdsammy` reader hard-errors on:
- `IFG≠0` — "reduced resonance widths are not supported"
- `KRM≠3` — "LRF=7 currently only supports Reich-Moore"

So NJOY **itself** never exercises the fully general R-matrix (KRM=1/2/4,
requiring an explicit gamma channel in the inversion) or reduced-width
(IFG=1) cases — only **Reich-Moore-limited** (KRM=3, IFG=0), where the
radiative-capture channel is eliminated analytically (folded into the level
matrix as an additive term, never needing its own row/column in the
inversion). This port matches that restriction rather than attempting scope
NJOY's own driver never reaches.

**Scope history (superseded, kept for context):** on 2026-07-07 it was
temporarily decided to defer the derivative routines (`babb`, `abpart`,
`derres`, `derext`) and angular-distribution routines
(`angle`/`lmaxxx`/`kclbsch`/`clbsch`/`setleg`) until `ERRORR` existed, since
`RECONR` — the only caller in this workspace at the time — hardcodes
`Want_Partial_Derivs=.false.`/`Want_Angular_Dist=.false.`
(`reconr.f90:149-150`; only `ERRORR` sets them `.true.`,
`errorr.f90:392-393`). **This was superseded the same day**: the user asked
to finish every phase of `samm` and then port `errorr.f90` itself, so the
derivative/angular routines are back in scope and will be built together
with `ERRORR`, which is now their real, in-workspace caller — not ported
speculatively ahead of a consumer. See `docs/porting-plan.md` for the
up-to-date phase status.

## How the port implements it

This is a **large, multi-phase port** (7169 lines — roughly 2× UNRESR+PURR
combined, before the derivatives/angular scope reduction above). Phased plan,
each phase independently portable/verifiable:

1. **Data model + ENDF LRF=7 reader** — ✅ done (`mf2.rs`, ported from
   `rdsammy`'s `mode==7` branch + `s2sammy`'s size-scanning pass). Owned
   structs (`ParticlePair`, `RmlChannel`, `RmlResonance`, `SpinGroup`,
   `RmlSection`) replace `samm.f90`'s module-global arrays, reusing the
   crate's general [`crate::endf::records::SectionCursor`] (CONT/LIST/TAB1)
   rather than `unresr::mf2`'s more limited cursor.
2. **Spin/parity/penetrability setup** — ✅ done, including `betset`'s core:
   - `penetrability.rs` — `pf`/`genpsf`/`pgh`/`sinsix` (hard-sphere
     penetrability, shift factor, phase shift for uncharged channels;
     `l=0..4` closed-form plus a recursion for `l>4`). See
     [`penetrability::genpsf`]'s doc comment for a flagged likely-latent
     upstream bug (a use-before-set local at the `l=4` recursion seed),
     ported literally with the variable seeded to `0.0`.
   - `context.rs` — `ppdefs` → [`context::apply_particle_pair_defaults`],
     `checkqn` → [`context::check_quantum_numbers`], `fxradi` →
     [`context::compute_channel_kinematics`].
   - `betset.rs` — `betset`'s non-derivative core →
     [`betset::compute_resonance_amplitudes`]: per-resonance reduced-width
     amplitudes `beta_c`, their triangular products, and the eliminated
     channel's own amplitude `gbetpr`. The first real consumer of both
     `penetrability::pgh` (uncharged channels) and `coulomb::pghcou`
     (charged channels). See its doc comment for a flagged stale-`drho`
     issue in the (not-yet-ported) derivative term, inherited from upstream.
   - **Not ported, and not needed for `mode==7`:** `findsp`/`rearrange` (only
     used by the non-RML resonance-to-spin-group lookup — `mode==7`'s
     resonances are already read per spin group, see `mf2.rs`, so this
     bookkeeping is structurally dead code for our scope) and `orders`
     (generic sort+dedup for the PENDF energy-grid node list — a Phase 6/
     top-level-orchestration concern, not spin/parity setup).
   - **Deferred until built alongside `ERRORR`:** `angle`/`lmaxxx`/`kclbsch`/
     `clbsch` (Legendre/Clebsch-Gordan angular-distribution coefficients) and
     `betset`'s `Want_Partial_Derivs`/`Want_Partial_U`-gated u-parameter
     conversion — see the scope-history note above.
3. **Coulomb wave-function library** — ✅ done (`coulomb.rs`): `jwkb`,
   `coulfg` (Steed's method, the CPC "COULFG" algorithm), `xsigll`,
   `asymp1`/`asymp2`, `taylor`, `end1`, `getfg`, `bigeta`, `getps`, `coulx`,
   `pspcou`, `pghcou`. Self-contained special functions, exercised only for
   charged-particle exit channels (`zeta != 0` in [`context::ChannelKinematics`]).
   See `coulomb.rs`'s module doc for the 0-indexed-by-`L` array convention
   used throughout, checked position-by-position against the Fortran rather
   than re-derived.
4. **R-matrix inversion** — ✅ done:
   - `linpack.rs` — the general complex-symmetric packed solver (`xspfa`
     Bunch-Kaufman factorization, `xspsl` solve, `xaxpy`/`xdot`/`xswap`/
     `ixamax` BLAS-1 helpers, stride-1 only — every call site in `samm.f90`
     passes `incx=incy=1`, verified by grep, so the general-stride branches
     and 1970s manual loop-unrolling are not ported). Indices are kept
     numerically identical to the Fortran's 1-indexed flat packed offsets
     rather than translated to 0-indexed, checked line-by-line against the
     source — see the module doc for why.
   - `rmatrix_invert.rs` — `yinvrs`'s dispatcher plus the closed-form
     `onech`/`twoch`/`threech` inverters (1/2/3-channel cases; `threech`
     includes its `scale3`/`unscale3` numerical-conditioning helpers) and
     `yfour` (4+ channels, via `linpack.rs`). `zeror` (trivial all-zero
     init) is also here, ready for Phase 5's `crosss` to call.
   - **Not yet ported (belongs with Phase 5's cross-section assembly, not
     here):** `setxqx`, `sectio`, `gcphase`, `setqri`, `settri` — these
     build/rearrange the R-matrix input to `yinvrs` from `beta`/`echan`,
     which is `crosss`'s job.
5. **Cross-section evaluation** — not started (`babb`, `abpart`, `crosss`,
   `setr`, `setxqx`, `sectio`, `gcphase`, `setqri`, `settri`, `derres`/
   `derext`, ~1600 lines) — the actual physics tying everything together,
   using [`rmatrix_invert::invert`] as its core linear-algebra step.
6. **Top-level orchestration** — surveyed, not yet ported (`cssammy`,
   `ppsammy`, `allo`, `desammy`, `orders`, `angle`, `lmaxxx`, `kclbsch`,
   `clbsch`, `setleg`, ~850 lines).

## Testing

**TODO** (Opus verification pass — no tests were written as part of this
translation, per the crate's model-division-of-labour rule in `CLAUDE.md`).
Gate: reconstruct an LRF=7 (KRM=3) evaluation (e.g. ¹⁶O or ¹⁹F, whose ENDF/B
files use RML) and reproduce upstream RECONR's pointwise σ(E) within
tolerance — but that gate needs Phases 2–6 first. For Phase 1 alone: parse a
real LRF=7 section and manually cross-check the particle-pair/spin-group/
resonance counts and a few resonance energies/widths against the raw ENDF
file by eye.

## Caveats

- **The eliminated-channel reorder step in `mf2.rs` is unverified and
  flagged prominently in its own doc comment.** Hand-tracing the raw-channel
  numbering three separate times, by different routes, all three predicted a
  different index (`igamma-1`) than what `rdsammy` actually reads
  (`igamma+1`, `samm.f90:1186`). A concrete worked example (in the doc
  comment) shows `igamma+1` would read past this spin group's valid channel
  range when the eliminated channel isn't last — evidence this may be a
  genuine latent bug in `samm.f90` itself, not a translation error, but
  deciding that and fixing it is a verification-pass call, not a translation
  one. Ported literally rather than "corrected" based on an unconfirmed
  derivation — this is the single highest-priority thing to check against a
  real evaluation where the eliminated channel is not first in the raw
  channel list. The read is bounds-checked (returns
  [`crate::NjoyError::EndfParse`] rather than panicking) in case the
  discrepancy indicates a genuine out-of-range access for some inputs.
- **`penetrability::genpsf`'s `l=4` recursion seed reads a local (`dss`)
  before it is set** — see that function's doc comment. Ported literally
  with the value seeded to `0.0` (both to make it translatable into safe
  Rust at all, and because that's the most common real-world Fortran
  compiler default for an uninitialized local) rather than "fixed" using the
  pattern `pgh`'s own `l=4` branch suggests. Only affects `l>4` channels,
  essentially never seen in real resonance-region evaluations.
- **Background R-matrix elements (`KBK>0` per spin group) are not parsed
  into data** — the cursor is advanced correctly past them (so subsequent
  parsing isn't corrupted) but their content is discarded. A secondary,
  rarer LRF=7 feature; port on demand if a target evaluation uses it.
- **`context::check_quantum_numbers`'s diagnostics are `log::warn!`, not
  errors** — matching `checkqn`'s own behavior (`write` to the output
  listing, not `call error`), except for the two conditions upstream itself
  treats as fatal (invalid group spin, negative channel `l`).
- **`betset::compute_resonance_amplitudes` inherits a stale-`drho` issue
  from upstream** for resonances sitting exactly on a channel threshold —
  see that function's doc comment. Only matters for the not-yet-ported
  derivative term; the amplitude computed here is unaffected.
- **`coulomb.rs`'s `pspcou` dead local (`paccq`) is intentionally not
  ported** — write-only in the Fortran (computed, never read again within
  the subroutine); see [`coulomb::coulfg`]'s doc comment.
- **`linpack.rs`/`rmatrix_invert.rs` are untested against a real R-matrix
  problem** — the LINPACK Bunch-Kaufman factorization/solve is a
  well-established published algorithm, but this is dense pivoting logic
  (index arithmetic corrupts silently, not with a panic) that has not been
  exercised end-to-end yet; that needs Phase 5 wired up first.
- **Phases 5–6 (cross-section formula, top-level orchestration) are not
  ported yet** — `mf2.rs`/`context.rs`/`penetrability.rs`/`coulomb.rs`/
  `betset.rs`/`linpack.rs`/`rmatrix_invert.rs` alone do not make RECONR
  able to reconstruct LRF=7 cross sections; they provide the ENDF data,
  kinematic/quantum-number setup, per-resonance channel amplitudes, and a
  working `Y^-1` inverter the not-yet-built cross-section formula needs.
- **RECONR currently lacks RML entirely** — evaluations using LRF=7 fail
  until this port reaches Phase 5. Per `docs/porting-plan.md`, check the
  evaluation's LRF before relying on RECONR.
- Numerically delicate — channel-matrix conditioning near thresholds needs
  care (Phase 4).

## References

- NJOY2016 manual §RECONR (LA-UR-17-20093), resonance formalisms
- `samm.f90` (NJOY2016 2016.79) — SAMMY method coding, N. Larson (ORNL)
- ENDF-102, File 2 LRF=7 (R-matrix limited); Lane & Thomas, R-matrix theory
