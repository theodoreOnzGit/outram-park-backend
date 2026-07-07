# RECONR — resonance reconstruction

> NJOY2016 module port. Theory summarised from the NJOY2016 manual
> (LA-UR-17-20093, §RECONR); upstream Fortran: `reconr.f90` (~5.7k lines).

## Theory

RECONR reconstructs pointwise cross sections σ(E) from the resonance parameters
in ENDF File 2 and from cross sections given with nonlinear ENDF interpolation
laws. The output is a **pointwise-ENDF (PENDF)** file on a *unionised* energy
grid, dense enough that linear interpolation reproduces the true σ(E) to within
a user tolerance (typically 0.1–1 %). Resonance parameters are removed from
File 2 and the material directory is corrected.

The resonance contribution is summed from every ENDF resolved-resonance
formalism (LRF=1/2/3/4/7), all now parsed and reconstructed:

- **SLBW / MLBW (LRF=1/2)** — single- and multi-level Breit–Wigner.
- **Reich–Moore (LRF=3)** — the modern resolved-range form for most current
  ENDF/B evaluations.
- **R-matrix limited (RML, LRF=7, via `samm`)** — the full multichannel
  R-matrix, needed for light nuclides / strongly overlapping resonances the
  pole approximations above can't represent correctly.
- **Adler–Adler (LRF=4)** — an older, empirical multi-reaction fit, mostly
  superseded by Reich-Moore/RML in modern evaluations but still present in
  some legacy ENDF/B-V-era files.

Reconstruction is adaptive: the grid is bisected around each resonance until the
linear-interpolation error between successive points falls under tolerance.
Redundant sums (total inelastic, charged-particle lumps) are re-derived so they
are exactly additive on the final grid.

## How the port implements it

The reconstruction **engine is ported** and lives in [`crate::reconr`]:

- `mf2` — File-2 resonance-parameter parsing (SLBW/MLBW/Reich-Moore/Adler-
  Adler fully; LRF=7/R-Matrix-Limited via
  `crate::samm::mf2::parse_rml_section`, wrapped as `EnergyRange::rml`),
- `slbw` / `rm` / `aa` — SLBW/MLBW, Reich–Moore, and Adler-Adler kernels
  (`aa`'s `eval_aa_range` takes a whole range rather than one l-state at a
  time — Adler-Adler's shared background/phase terms don't decompose
  per-l-state the way the other two formalisms' do),
- `add_rml_range` (in `mod.rs`) — R-Matrix-Limited kernel, dispatching to
  `crate::samm::setup::setup` (once per range) +
  `crate::samm::xsformula::cssammy` (once per energy grid point) rather
  than a pole approximation — needed for light nuclides / strongly
  overlapping resonances SLBW/MLBW/Reich-Moore can't represent correctly,
- `linearize` — adaptive linearisation to tolerance,
- `refine_resonance_grid` (in `mod.rs`) — **adaptive refinement of the
  resonance-reconstruction grid** to tolerance (see the finding below).

It is reached through [`crate::interface`] (`ReconrResult`), not through this
module's `run()`. This `modules::reconr` entry is the *card-input driver* only
(sequencing + tape I/O), which is deferred until the NJOY `main` driver is
ported.

## Finding — resonance-wing grid density (fixed 2026-07-07)

Originally the resonance contribution was evaluated only on a fixed grid: the
smooth-background energies plus a **fixed halo** of points within about ±10
half-widths (≈ ±0.25 eV) of each resonance peak. Between resonances that left
**multi-eV gaps with no grid points at all** — e.g. for U-238 capture the 0 K
grid jumped straight from 103.03 eV (46.96 b) to 111.10 eV (0.31 b), with the
entire Lorentzian wing across those 8 eV represented by a **single straight
line**. Linear interpolation of a convex 1/(E−E_r)² wing lies far above the
true curve, so σ was grossly over-stated a few eV past every strong resonance.

This surfaced downstream as the **"BROADR wing pedestal"**: after Doppler
broadening, U-238 capture showed a spurious ~200 b flat pedestal several eV
past each resonance where OpenMC (reference NJOY) had already decayed to ~1 b.
The SIGMA1 kernel was **not** at fault — it was being fed the over-linearised
grid *as input* and sampled on the same coarse grid *as output*. Code-to-code
verification traced it to the reconstruction grid (see
`tests/u238_doppler_verification/`).

The fix ([`refine_resonance_grid`]) adaptively bisects each grid interval until
linear interpolation of the resonance contribution reproduces the
directly-evaluated value to within the reconstruction tolerance `eps` — exactly
the criterion NJOY's own RECONR reconstructs to. Result: the U-238 900 K/1200 K
capture RRR magnitude-weighted L1 vs OpenMC dropped from **≈0.30 → ≈0.0007**
(a ~400× accuracy gain), matching reference NJOY to <0.1% across the resolved
region.

Note this is a **correctness fix bringing the port to parity with NJOY**, not a
fidelity improvement over it — NJOY already reconstructs to tolerance; the
port's original fixed-halo grid was a deficiency. The genuine improvement *over*
NJOY is that both the reconstruction (`refine_resonance_grid`) and the SIGMA1
broadening (`crate::broadr`) are **data-parallel** (`rayon`) — each energy
window / query point is an independent pure evaluation — where NJOY's Fortran
is serial. On the resulting ~20× denser grid (960k points for U-238's RRR at
0.1%), that parallelism keeps reconstruction ≈10 s and broadening ≈13 s on a
many-core host instead of minutes.

**Known follow-up:** the fissile 3×3-fission-matrix Reich-Moore path (U-235) is
much heavier per energy point than U-238's scalar path, so on the denser grid
its reconstruction is slow enough to strain the test timeout even parallelised
— a resonance-windowing optimisation (skip resonances whose tail is negligible
at the evaluation energy, as NJOY does) is the natural next step and is tracked
in `docs/porting-plan.md`.

## Testing

**Ported and verified for SLBW/MLBW/Reich-Moore** — see `crate::reconr` unit
tests and the workspace Godiva/Jezebel k-eff V&V (`docs/development-history.md`).
Reconstructed 0 K pointwise σ(E) feeds BROADR and ACER.

**LRF=7 (R-Matrix-Limited) wiring is untested** — `add_rml_range` compiles and
type-checks (workspace build + full test suite pass as a regression check,
zero regressions) but has never been run against a real LRF=7 evaluation.
`samm` itself (the R-matrix engine this wiring calls into) is also untested
end-to-end — see `../samm/README.md` for its own caveats, most importantly
the still-unresolved eliminated-channel-reorder question in `samm::mf2`.

**LRF=4 (Adler-Adler) is also untested** — `aa::eval_aa_range`/
`add_aa_range` compile and type-check (same regression-check basis) but
have never been run against a real Adler-Adler evaluation. Its ENDF-6
record layout was reconstructed entirely by tracing `reconr.f90`'s
`rdf2aa`/`csaa` flat-array offset arithmetic (no ENDF-102 manual excerpt
was available locally) — see `aa.rs`'s doc comment for two specific
as-coded oddities ported literally rather than "fixed" (a background term
scaled by the same energy factor twice, and an `LI==6` special case that
redefines total rather than using the just-computed sum).

## Caveats

- Near the critical point of the resolved/unresolved boundary, grid density is
  driven by tolerance; extremely narrow resonances need a tight tolerance.
- **LRF=7 (RML) wiring is new and unverified** (2026-07-07) — see the Testing
  section above. Derivatives and angular distributions are out of scope for
  `samm` (and hence for this reconstruction path) until `ERRORR` is built.
- **LRF=4 (Adler-Adler) is new and unverified** (2026-07-07) — see the
  Testing section above. Zero-temperature only, matching this crate's
  SLBW/Reich-Moore precedent (Doppler broadening is BROADR's job).
- The `run()` driver returns `NotPorted`; use `crate::interface` instead.

## References

- NJOY2016 manual §RECONR (LA-UR-17-20093)
- `reconr.f90` (NJOY2016 2016.79, commit `ac5adf5`)
- ENDF-102 format manual, File 2 resonance formats
