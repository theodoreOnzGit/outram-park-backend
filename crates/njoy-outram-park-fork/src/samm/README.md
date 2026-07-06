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

## How the port implements it

This is a **large, multi-phase port** (7169 lines — roughly 2× UNRESR+PURR
combined). Phased plan, each phase independently portable/verifiable:

1. **Data model + ENDF LRF=7 reader** — ✅ done (`mf2.rs`, ported from
   `rdsammy`'s `mode==7` branch + `s2sammy`'s size-scanning pass). Owned
   structs (`ParticlePair`, `RmlChannel`, `RmlResonance`, `SpinGroup`,
   `RmlSection`) replace `samm.f90`'s module-global arrays, reusing the
   crate's general [`crate::endf::records::SectionCursor`] (CONT/LIST/TAB1)
   rather than `unresr::mf2`'s more limited cursor.
2. **Spin/parity/penetrability setup** — not started (`angle`, `findsp`,
   `checkqn`, `fxradi`, `betset`, `lmaxxx`, Clebsch-Gordan `kclbsch`/`clbsch`,
   ~1500 lines).
3. **Coulomb wave-function library** — not started (`coulfg`, `jwkb`,
   `coulx`, `asymp1`/`asymp2`, `taylor`, `getfg`, `bigeta`, `getps`,
   `pgh`/`pghcou`/`pspcou`, `sinsix`, ~1400 lines). Self-contained special
   functions; only exercised for charged-particle exit channels.
4. **R-matrix inversion** — not started (LINPACK-style symmetric-indefinite
   solver `xspfa`/`xspsl`/`xswap`/`xaxpy`/`ixamax`/`xdot`, plus
   `zeror`/`yinvrs`/`onech`/`twoch`/`threech`/`yfour`/`setxqx`/`sectio`/
   `setqri`/`settri`, ~1800 lines).
5. **Cross-section evaluation + derivatives + angular distributions** — not
   started (`babb`, `abpart`, `crosss`, `setr`, `derres`/`derext`, `setleg`,
   ~1300 lines) — the actual physics tying everything together.
6. **Top-level orchestration** — surveyed, not yet ported (`cssammy`,
   `desammy`, ~250 lines).

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
  numbering twice, by two different routes, both predicted a different index
  (`igamma-1`) than what `rdsammy` actually reads (`igamma+1`,
  `samm.f90:1186`). Ported literally rather than "corrected" based on an
  unconfirmed derivation — this is the single highest-priority thing to
  check against a real evaluation where the eliminated channel is not first
  in the raw channel list. The read is bounds-checked (returns
  [`crate::NjoyError::EndfParse`] rather than panicking) in case the
  discrepancy indicates a genuine out-of-range access for some inputs.
- **Background R-matrix elements (`KBK>0` per spin group) are not parsed
  into data** — the cursor is advanced correctly past them (so subsequent
  parsing isn't corrupted) but their content is discarded. A secondary,
  rarer LRF=7 feature; port on demand if a target evaluation uses it.
- **Phases 2–6 (the actual R-matrix physics) are not ported at all yet** —
  `mf2.rs` alone does not make RECONR able to reconstruct LRF=7 cross
  sections; it only makes the ENDF data readable into structured form.
- **RECONR currently lacks RML entirely** — evaluations using LRF=7 fail
  until this port reaches Phase 5. Per `docs/porting-plan.md`, check the
  evaluation's LRF before relying on RECONR.
- Numerically delicate — channel-matrix conditioning near thresholds needs
  care (Phase 4).

## References

- NJOY2016 manual §RECONR (LA-UR-17-20093), resonance formalisms
- `samm.f90` (NJOY2016 2016.79) — SAMMY method coding, N. Larson (ORNL)
- ENDF-102, File 2 LRF=7 (R-matrix limited); Lane & Thomas, R-matrix theory
