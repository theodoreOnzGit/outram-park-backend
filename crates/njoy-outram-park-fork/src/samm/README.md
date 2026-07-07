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

**Second scope reduction (2026-07-07):** `RECONR` — the only module in this
workspace that currently calls into `samm` — hardcodes
`Want_Partial_Derivs=.false.` and `Want_Angular_Dist=.false.`
(`reconr.f90:149-150`). Those flags gate `betset`'s derivative branches and
the entire `angle`/`lmaxxx`/`kclbsch`/`clbsch`/`setleg` Legendre/
Clebsch-Gordan machinery. Only `ERRORR` sets them `.true.`
(`errorr.f90:392-393`), and `errorr.f90` is not in this workspace yet (an
unstarted item in `docs/porting-plan.md`). By explicit user decision, the
derivative routines (`babb`, `abpart`, `derres`, `derext`) and the
angular-distribution routines are **deferred until `ERRORR` is actually being
built**, rather than ported now against no reachable caller — mirroring the
`findsp`/`rearrange` dead-code-for-our-scope call below, except this one
saves several thousand lines rather than a few hundred.

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
2. **Spin/parity/penetrability setup** — ✅ done for the non-derivative,
   non-Coulomb core:
   - `penetrability.rs` — `pf`/`genpsf`/`pgh` (hard-sphere penetrability,
     shift factor, phase shift for uncharged channels; `l=0..4` closed-form
     plus a recursion for `l>4`). See [`penetrability::genpsf`]'s doc comment
     for a flagged likely-latent upstream bug (a use-before-set local at the
     `l=4` recursion seed), ported literally with the variable seeded to
     `0.0`.
   - `context.rs` — `ppdefs` → [`context::apply_particle_pair_defaults`],
     `checkqn` → [`context::check_quantum_numbers`], `fxradi` →
     [`context::compute_channel_kinematics`].
   - **Not ported, and not needed for `mode==7`:** `findsp`/`rearrange` (only
     used by the non-RML resonance-to-spin-group lookup — `mode==7`'s
     resonances are already read per spin group, see `mf2.rs`, so this
     bookkeeping is structurally dead code for our scope) and `orders`
     (generic sort+dedup for the PENDF energy-grid node list — a Phase 6/
     top-level-orchestration concern, not spin/parity setup).
   - **Deferred (see the scope note above):** `angle`/`lmaxxx`/`kclbsch`/
     `clbsch` (Legendre/Clebsch-Gordan angular-distribution coefficients) and
     `betset`'s derivative branches (`Want_Partial_Derivs`/
     `Want_Partial_U`-gated). `betset`'s non-derivative, non-Coulomb core
     (the energy-independent channel-amplitude `β` parameters) is not yet
     wired up — it needs `pghcou` (Phase 3) for charged-particle channels
     before it can be ported in full; `pgh` alone (done) covers the neutral
     (neutron) channel path.
3. **Coulomb wave-function library** — not started (`coulfg`, `jwkb`,
   `coulx`, `asymp1`/`asymp2`, `taylor`, `getfg`, `bigeta`, `getps`,
   `pghcou`/`pspcou`, `sinsix`, ~1400 lines). Self-contained special
   functions; only exercised for charged-particle exit channels.
4. **R-matrix inversion** — not started (LINPACK-style symmetric-indefinite
   solver `xspfa`/`xspsl`/`xswap`/`xaxpy`/`ixamax`/`xdot`, plus
   `zeror`/`yinvrs`/`onech`/`twoch`/`threech`/`yfour`/`setxqx`/`sectio`/
   `setqri`/`settri`, ~1800 lines).
5. **Cross-section evaluation** — not started (`babb` [deferred, see scope
   note], `abpart`, `crosss`, `setr`, ~1000 lines excluding derivatives) —
   the actual physics tying everything together.
6. **Top-level orchestration** — surveyed, not yet ported (`cssammy`,
   `ppsammy`, `allo`, `desammy`, `orders`, ~350 lines).

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
- **Phases 3–6 (the actual R-matrix physics) are not ported at all yet** —
  `mf2.rs`/`context.rs`/`penetrability.rs` alone do not make RECONR able to
  reconstruct LRF=7 cross sections; they only make the ENDF data readable and
  provide the non-Coulomb kinematic/quantum-number setup.
- **Derivatives and angular distributions are deferred, not scoped out
  permanently** — see the "second scope reduction" note above. Revisit when
  `ERRORR` is actually being built.
- **RECONR currently lacks RML entirely** — evaluations using LRF=7 fail
  until this port reaches Phase 5. Per `docs/porting-plan.md`, check the
  evaluation's LRF before relying on RECONR.
- Numerically delicate — channel-matrix conditioning near thresholds needs
  care (Phase 4).

## References

- NJOY2016 manual §RECONR (LA-UR-17-20093), resonance formalisms
- `samm.f90` (NJOY2016 2016.79) — SAMMY method coding, N. Larson (ORNL)
- ENDF-102, File 2 LRF=7 (R-matrix limited); Lane & Thomas, R-matrix theory
