# CLAUDE.md — buangkok

**BUANGKOK — Bioeffects, Uncertainty and ALARA for Nuclear Guidance, Keeping
Operational Knowledge.** Radiation dose and its biological effects, for
research-grade safety analysis.

The workspace root `CLAUDE.md` binds here in full. This file adds only what is
specific to this crate.

## Status: placeholder (2026-09-28)

Nothing is implemented. The crate exists to reserve the name and state the
scope. Do not describe it as providing anything, and do not add code here
without the maintainer asking for it.

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
- **The Table 7 dose table stays in `changi`** until the maintainer decides to
  move it.
- **Units:** `uom` 0.38 has no sievert quantity (checked 2026-09-28). Decide
  how dose is typed before the first API, and do not use `AvailableEnergy`
  (J/kg) as a stand-in.
