# CLAUDE.md — buangkok

**BUANGKOK — Bioeffects, Uncertainty and ALARA for Nuclear Guidance, Keeping
Operational Knowledge.** Radiation dose and its biological effects, for
research-grade safety analysis.

The workspace root `CLAUDE.md` binds here in full. This file adds only what is
specific to this crate.

## Status: pyDOSEIA ported (2026-09-28)

~~Nothing is implemented. The crate exists to reserve the name and state the
scope.~~ **CHANGED 2026-09-28** (maintainer asked for the pyDOSEIA port, then
"port all of pyDOSEIA into buangkok"). Two things live here:

- `pydoseia`: all of pyDOSEIA's computation (met processing, dilution factors,
  inhalation, ground shine, submersion, ingestion, plume shine, DCF
  screening, plume rise, config, driver). It is code-to-code verified against
  upstream (`tests/pydoseia_code_to_code.rs`, `docs/pydoseia-code-to-code.md`).
- `published`: the Liu and Cao dose tables.

~~Ingestion and plume shine are not ported~~ (**ported 2026-09-28**); what is
not ported is I/O and UI, listed per function in
`docs/pydoseia-port-scoping.md`. The crate is **not mature**: no maturity bar
has been declared, and there is no validation.

## pyDOSEIA port rules

- **Faithful first.** The port reproduces upstream's control flow, constants
  and defects, bit-exact where the arithmetic allows. A correction is a
  separate, labelled divergence, never a silent edit, and the default stays
  upstream's. The defects found so far are D1–D26 in
  `docs/pydoseia-code-to-code.md`. Read upstream (`vendor/pyDOSEIA`, commit
  `dca4cdc3`) before changing any ported function.
- **No upstream data in the repo.** The code-to-code fixture is generated from
  synthetic tables. Regenerate it with
  `verification_and_validation/pydoseia_code_to_code/gen_pydoseia_reference.py`
  (it needs a Python venv with numpy, pandas, scipy, openpyxl, xlrd, joblib
  and matplotlib; it imports its second half from `tranche2.py` in the same
  folder), and commit the scripts and the fixture together.
- ~~**Plume shine rides on `pydoseia::quadpack`**, a port of SciPy's C
  QUADPACK (BSD-3, notice in `NOTICE`). Do not swap in `petir`'s GSL `qag`:
  a mutation test shows the fixture rejects it (no epsilon extrapolation).~~
  **CHANGED 2026-09-28** — maintainer: "can you replace QUADPACK integrator
  with stuff from petir? i don't want so many duplicate integrators here" …
  "or rather, include petir as a dependency to buangkok. quadpack should be
  used as regression test, but petir is the main one". **Plume shine runs on
  petir by default** (`PlumeShineIntegrator::Petir`: nested
  `petir::integration::qags_with_status`, petir's port of GSL
  `gsl_integration_qags`, which is QUADPACK `dqagse` with extrapolation).
  `pydoseia::quadpack` (SciPy's C QUADPACK, BSD-3, notice in `NOTICE`) stays
  as `PlumeShineIntegrator::ScipyQuadpackReference`, the **regression
  reference**: the code-to-code tests select it explicitly, and
  `tests/plume_shine_petir_vs_quadpack.rs` compares the two. Measured
  2026-09-28: the petir path is bit-identical to the reference on all 120
  fixture integrals and reproduces the fixture bit for bit. Petir's plain
  `qag` is still not a substitute for `dqagse` on singular integrands (the
  `quadpack`-group mutation test); do not add a third integrator here —
  extend petir.
- **Every ported file keeps the MIT provenance header** and points at
  `NOTICE`.
- **`chi/Q` is `changi`'s `DilutionFactor`.** Do not add a second s/m^3
  type.

## Rules for when work starts

- **Research-grade safety analysis only.** Never frame output as a dose to a
  real person or population, or as fit for medical, occupational, public-health,
  emergency, licensing or regulatory use (`RESPONSIBLE_USE.md`).
- **Dose coefficients must be citable and freely usable.** Prefer the US EPA
  Federal Guidance Reports (FGR-11, FGR-12, FGR-15), which are distributed
  freely; ICRP publications are copyrighted, so cite rather than reproduce
  them. Record provenance per `DATA_POLICY.md`.
- **Search before building.** CHANGI supplies air concentration and
  deposition; RAFFLES supplies uncertainty propagation. Reuse them.
- **Port, don't write from scratch.** The first candidate upstream is
  **pyDOSEIA** (Sadhu et al., *Health Physics* 130(1) (2026) 94–110,
  doi:10.1097/HP.0000000000002014; code MIT-licensed at
  github.com/BiswajitSadhu/pyDOSEIA, commit `dca4cdc3` checked 2026-09-28).
  Keep its MIT notice on ported files and verify code-to-code against it,
  but check the licence of each dose-coefficient table it bundles first.
- **The Table 7 and Table 9 dose tables live here** (`published`), moved from
  changi 2026-09-28. `AccidentCase` stays defined in changi (Table 8, the
  accident releases, uses it) and is re-exported here — keep it that way.
- **Units:** `uom` 0.38 has no sievert quantity (checked 2026-09-28). Decide
  how dose is typed before the first API, and do not use `AvailableEnergy`
  (J/kg) as a stand-in.
