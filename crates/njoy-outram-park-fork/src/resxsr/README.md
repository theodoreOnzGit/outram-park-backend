# RESXSR — pointwise resonance cross-section files

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** All code in this workspace is **unverified and untrusted** unless a specific verification & validation (V&V) case demonstrates otherwise. V&V cases are human-reviewed and are intended for journal / arXiv publication — that is the trust workflow. See the workspace `VERIFICATION_AND_VALIDATION.md` and `RESPONSIBLE_USE.md`. Not for nuclear facility operation, reactor control, safety-critical, or licensing decisions.


> NJOY2016 module port. Theory summarised from the NJOY2016 manual
> (LA-UR-17-20093, §RESXSR); upstream Fortran: `resxsr.f90`.

## Theory

In thermal-reactor lattices, resonance self-shielding in the **near-epithermal**
range (≈4–200 eV) is poorly captured by the simple Bondarenko model, which assumes
every absorber resonance is *narrow* with respect to the energy a neutron loses
per scatter. Many resonances in this range violate that assumption. RESXSR
prepares **pointwise** resonance cross-section files (the RESXS format) so that
lattice codes can apply better treatments (e.g. intermediate-resonance theory, as
used in the WIMSR path) instead of relying on narrow-resonance f-factors alone.

RESXSR reads one or more NJOY **PENDF** tapes (pointwise, per-temperature σ from
RECONR/BROADR), for each material builds a single **union energy grid** across the
elastic (MT 2), fission (MT 18) and capture (MT 102) reactions over the requested
`[efirst, elast]` window, thins that grid with a relative tolerance `eps`, and
writes the result in the RESXS record layout (elastic/fission/capture vs
temperature, linear interpolation assumed).

## Ported vs. NotPorted

**Ported (self-contained, unit-tested):**

- **Input deck** (`input.rs`) — the free-format card sequence
  (`resxsr.f90:16-40, 236-248`): `ResxsrInput` + `MaterialSpec`.
- **RESXS record layout** (`format.rs`) — the header/field values and the
  word-count index math for every record in the RESXS spec
  (`resxsr.f90:43-189, 399-501`): file identification, file control,
  set-Hollerith, file data, material control, and cross-section-block sizing
  (`nn = 1 + nreac*ntemp`, `(nblok/nn)*nn`, block-count ceiling).
- **Union-grid assembly + adaptive linear thinning** (`assemble.rs`) — the two
  numerical steps (`resxsr.f90:306-397`), operating on already-parsed reactions.
  The thinning is a faithful translation of the Fortran run/restart loop,
  including its run-start node selection and the linear-collapse edge case.

**NotPorted (documented gaps):**

- **PENDF tape reader** — `tpidio`/`contio`/`hdatio`/`findf`/`gety1` walk of a
  real PENDF tape and the `loada`/`finda` scratch buffering (`resxsr.f90:250-352`).
- **Binary RESXS writer** — emitting the output records to a tape
  (`resxsr.f90:435-501`).
- `driver::run` therefore documents the full pipeline and returns
  `NjoyError::NotPorted("resxsr::run")`; the registry entry `resxsr::run()`
  returns `NjoyError::NotPorted("resxsr")`. No output is fabricated.

## Provenance

- Upstream: NJOY2016 `src/resxsr.f90`, git commit
  `ac5adf5f33d893e42f2eed7fb286b0d51c7580da`.
- NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
  this derivative is GPL-3.0-only, a modified non-LANL version not endorsed by
  LANL/DOE. See crate-root `LICENSE.njoy` + `NOTICE`.

## Testing (V&V status)

**Methodology.** Card round-trips and default values vs the Fortran `read`
statements; RESXS record word counts vs the spec formulas (`w = 1+3*mult`, `5`,
`nholl*mult`, `(mult+2)*nmat`, `mult+3+ntemp`); block sizing vs
`resxsr.f90:488-492`; union-grid interpolation and reaction-zero-outside-range vs
`gety1`; and the thinning loop hand-traced against `resxsr.f90:363-396`.

**Results (2026-07-15, commit ac5adf5).** All 10 RESXSR unit tests pass (run in
an isolated harness with identical sources because the in-crate `cargo test`
build was blocked by *other* modules under concurrent edit — see the handoff
notes):

- `input`: 2/2 — defaults zeroed (`maxt=10`), counters track lists.
- `format`: 3/3 — with `mult=2`: file-id `w=7`, file-control `w=5`,
  set-Hollerith(4) `=8`, file-data(3) `=12`, material-control(2 temps) `=7`;
  block `nn=7`, capacity `4998`, `nb=2` for 1000 points; `nreac_for` 3/2.
- `assemble`: 4/4 — union `{4,52,100,200}` eV with hand-checked interpolants;
  zero outside range; thinning `{4,8,12,16,20}→{4,8,20}` (faithful node
  selection, peak dropped — matches the Fortran trace); linear-collapse quirk.
- `driver`: 1/1 — `run` → `NotPorted("resxsr::run")`.

## Caveats / what a human must verify

- **Untrusted AI draft.** The union-grid/thinning kernels reproduce the Fortran
  *node selection* (retains run-start points, can drop a sharp peak); confirm
  this is acceptable for the intended intermediate-resonance use, or pair with a
  golden-file comparison once the PENDF reader lands.
- **Fortran exponent/byte formatting** is not part of RESXSR output (binary
  records), so there is no formatting-fidelity concern here.
- Golden-file V&V against an upstream RESXS file for a resonance absorber
  (e.g. U-238) is still required and depends on the NotPorted tape I/O.

## References

- NJOY2016 manual §RESXSR (LA-UR-17-20093)
- `resxsr.f90` (NJOY2016, commit ac5adf5); TRANSX; intermediate-resonance theory
