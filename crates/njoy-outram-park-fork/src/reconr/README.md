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

The resonance contribution is summed from the ENDF formalisms:

- **SLBW / MLBW** — single- and multi-level Breit–Wigner.
- **Reich–Moore (LRF=7 and the older RM)** — the modern resolved-range form.
- **Adler–Adler, R-matrix limited (RML, via `samm`)** — less common formalisms.

Reconstruction is adaptive: the grid is bisected around each resonance until the
linear-interpolation error between successive points falls under tolerance.
Redundant sums (total inelastic, charged-particle lumps) are re-derived so they
are exactly additive on the final grid.

## How the port implements it

The reconstruction **engine is ported** and lives in [`crate::reconr`]:

- `mf2` — File-2 resonance-parameter parsing (SLBW/MLBW/Reich-Moore fully;
  LRF=7/R-Matrix-Limited via `crate::samm::mf2::parse_rml_section`, wrapped
  as `EnergyRange::rml`),
- `slbw` / `rm` — SLBW/MLBW and Reich–Moore kernels,
- `add_rml_range` (in `mod.rs`) — R-Matrix-Limited kernel, dispatching to
  `crate::samm::setup::setup` (once per range) +
  `crate::samm::xsformula::cssammy` (once per energy grid point) rather
  than a pole approximation — needed for light nuclides / strongly
  overlapping resonances SLBW/MLBW/Reich-Moore can't represent correctly,
- `linearize` — adaptive linearisation to tolerance.

It is reached through [`crate::interface`] (`ReconrResult`), not through this
module's `run()`. This `modules::reconr` entry is the *card-input driver* only
(sequencing + tape I/O), which is deferred until the NJOY `main` driver is
ported.

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

## Caveats

- Near the critical point of the resolved/unresolved boundary, grid density is
  driven by tolerance; extremely narrow resonances need a tight tolerance.
- **LRF=7 (RML) wiring is new and unverified** (2026-07-07) — see the Testing
  section above. Derivatives and angular distributions are out of scope for
  `samm` (and hence for this reconstruction path) until `ERRORR` is built.
- The `run()` driver returns `NotPorted`; use `crate::interface` instead.

## References

- NJOY2016 manual §RECONR (LA-UR-17-20093)
- `reconr.f90` (NJOY2016 2016.79, commit `ac5adf5`)
- ENDF-102 format manual, File 2 resonance formats
