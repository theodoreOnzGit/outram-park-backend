# Predictions, written before any run (gh:#1393, 2026-11-13)

Committed BEFORE the C++ dump driver was compiled or run and before the Rust
comparison test existed.  Upstream: OFFBEAT `80e84450a115b0c411e1bfa5d166379f6bf6c084`
(GPL-3.0), archive fetched from gitlab.com/foam-for-nuclear/offbeat.

## Route (decided from feasibility, not from a result)

* `GeN-Foam-src/src/classes/offbeat` in this environment is a 4-file stub
  (`fuelBehaviour` stubbed); the OFFBEAT libraries are not built, and a build
  needs the full `libfiniteVolume`-linked tree of 829 directories.
* OFFBEAT `80e84450` targets OpenFOAM-ESI (its `Make/options` and
  `offbeatLib` use `volScalarField`/`dictionary`/`mesh_.lookupObject`), the
  same line as the rebuilt v2506, so a library build is possible in principle,
  but each correlation `.C` is a pure per-cell scalar kernel inside
  `correct(scalarField&, const scalarField& T, const labelList&)`; the only
  OpenFOAM content in the arithmetic is `pow`, `exp`, `tanh`, `sqrt`, `min`,
  `max` on `double` (`scalar` = double, DP build).
* Chosen: standalone C++ dump driver that copies each kernel's arithmetic
  lines **verbatim** with a minimal shim (no mesh, no registry), 4 cores
  shared, 8 GB disk free.  This is recorded as a transcription route, not a
  run of the compiled OFFBEAT library.  Nothing here ran OFFBEAT's
  constructors, dictionary look-ups or registry plumbing.
* code_aster: skipped, see the record.

## Predictions

1. Every ported correlation agrees with the transcribed upstream kernel to
   **< 1e-14 relative** on the (T, burnup, porosity, ...) grid (issue
   prediction).  A larger miss is a port transcription defect.
2. Bit-identity is NOT expected everywhere: the port uses `r_exp`/`r_powf`
   from PETIR where upstream uses glibc `exp`/`pow`, and writes
   `t*t*t` where upstream writes `pow(Ti, 3.0)`.  Predicted: at least 70 % of
   all grid points bit-identical; the remainder within 4 ulp (about 9e-16).
3. Models with no transcendental calls and the same operation order
   (Constant, Relap/Matpro-CTHCON Zircaloy cubic, Molybdenum, Swindeman,
   Tobbe, ParfumeSiC, IFA601 apart from its `exp`) are predicted
   **100 % bit-identical** except where an operand is formed differently
   (burnup unit conversion).
4. Operand-formation differences predicted to show up as 1-2 ulp:
   * MAMOX `exp(-Bui/phi)` with `Bu` in MWd/tHM and `phi = 128.75e3`
     upstream, vs `bu = Bu*1e-3` and `128.75` in the port;
   * Philipponneau `tau = Bu*1e-3/9.5/100` upstream vs `bu/950` in the port.
5. Documented port deviations (clamping in `value`, no `max(...,0.05)` in
   upstream's volume correction, `3*s/3` for swelling) predicted NOT to
   appear on the grid because the grid stays inside the validity windows and
   uses `swelling = 0`; one dedicated swelling row is added to test the
   `3*s/3` claim, predicted to differ by at most 1 ulp.
6. The Kato model's `PiLim`/`fp` are dead code upstream (computed, never
   used); predicted that the port, which uses only the unclamped porosity in
   the Barani expression, agrees.
7. No model fails the 1e-14 gate; if one does the first divergent operation
   is localised before any fix.
