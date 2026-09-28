# BUANGKOK

**B**ioeffects, **U**ncertainty and **A**LARA for **N**uclear **G**uidance,
**K**eeping **O**perational **K**nowledge.

> ⚠️ **Research, education and V&V only.** Not for medical, occupational
> exposure, public-health, emergency-response, licensing, regulatory or any
> other safety-critical decision. See the workspace `RESPONSIBLE_USE.md`.
> Dose here means research-grade safety analysis, nothing more.

The reserved home for **radiation dose and its biological effects**: dose from
air concentration and deposition (cloudshine, groundshine, inhalation,
ingestion), dose coefficients, dose uncertainty and ALARA. It answers **"what
dose follows from what was released and where it went?"**, at the end of the
offsite chain:

```text
  source term  ──►  CHANGI  ──►  BUANGKOK
  (SEMBAWANG,       air conc.,     dose, pathways,
   BISHAN)          deposition     uncertainty
```

Dose has its own crate so the dispersion and source-term crates keep their
"no dose" boundary, and so none of them reads as a health-assessment tool.

## Status: placeholder, nothing is implemented

Created 2026-09-28 to reserve the name and state the scope. The crate has no
dependencies and no behaviour. Its only public item is the `SCOPE` string
constant. Do not cite it as the location of any calculation.

Two published HTR-10 dose-versus-distance tables currently live in `changi`,
parked there by the maintainer: normal operation (Liu and Cao 2002, Table 7)
and two design-basis accidents (Table 9, added 2026-09-28). Whether they move
here is undecided.

## Bookkeeping status

> Maintainer sign-off tracker (see the workspace `CLAUDE.md` "Bookkeeping
> pass" command). A crate is **complete** only once the maintainer has
> personally signed off on BOTH axes below.

| Axis | Status |
|---|---|
| Verification & Validation (V&V) — human-reviewed | ❌ Not yet manually checked |
| Human / user interface — human-reviewed | ❌ Not yet manually checked |

**Status: INCOMPLETE** until both axes are manually checked and cleared by the maintainer.

## License

GPL-3.0. Part of the [OUTRAM PARK](../../README.md) workspace.
