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

## Status: partial pyDOSEIA port, code-to-code verified (2026-09-28)

~~Status: placeholder, nothing is implemented.~~ **CHANGED 2026-09-28.** The
crate was created that day to reserve the name. It now holds:

- **`buangkok::pydoseia`**, a partial, faithful Rust port of the MIT-licensed
  **pyDOSEIA** (commit `dca4cdc3`, see below). It covers:
  - met processing: the triple joint frequency distribution, and the missing
    and calm corrections;
  - Gaussian-plume dilution factors for three release modes: a single plume,
    and a long-term release with or without met data;
  - the **inhalation**, **ground-shine** and **submersion** dose pathways, with
    their age brackets, absorption-type selection, progeny correction,
    deposition velocities and weathering.
- **Verification:** code-to-code against pyDOSEIA itself, on synthetic inputs.
  The fixture has 1 015 cases and 13 422 values; 19 of the 20 function groups
  are bit-exact and the last agrees to 4.8e-16. Five mutation tests show the
  suite can fail. Methodology and results are in
  [`docs/pydoseia-code-to-code.md`](docs/pydoseia-code-to-code.md).
- **What this does not show:** agreement with pyDOSEIA is not agreement with
  reality. There is **no validation** of any kind, and seven upstream defects
  are recorded (D1–D7). The port reproduces them, and corrected variants are
  labelled as divergences.
- **Not ported yet:** ingestion, plume shine, upstream's multi-source DCF
  screening, and its I/O and input generator. The scope and the reasons are
  in [`docs/pydoseia-port-scoping.md`](docs/pydoseia-port-scoping.md).
- **No dose-coefficient data ships with the crate.** Upstream's tables are
  ICRP-derived (inhalation) or FGR-15 (external), so the caller supplies
  tables in upstream's CSV layout. Half-lives come from `boon-lay`, not from
  here.
- Nothing in the workspace calls it yet. It is not wired into `htgr_sim_v1`.

**Published dose tables (moved here 2026-09-28):** `buangkok::published` holds
the HTR-10 dose-versus-distance tables from Liu and Cao (2002) — normal
operation (Table 7, mSv/a) and two design-basis accidents (Table 9, thyroid
and whole-body mSv) — as cited reference data, with provenance in
`docs/References.md`. Nothing computes a dose from them.

## Reference implementations and literature to build from

- **pyDOSEIA** — B. Sadhu, T. Sarkar, S. Anand, K. D. Singh and D. K. Aswal,
  "pyDOSEIA: A Python Package for Radiological Impact Assessment during
  Long-term or Accidental Atmospheric Releases", *Health Physics* **130**(1)
  (2026) 94–110, doi:[10.1097/HP.0000000000002014](https://doi.org/10.1097/HP.0000000000002014),
  PMID [40622262](https://pubmed.ncbi.nlm.nih.gov/40622262/) (Bhabha Atomic
  Research Centre). A Gaussian-plume dose code following IAEA and AERB
  guidance: age-, distance- and radionuclide-specific doses from inhalation,
  ingestion, groundshine, submersion and plumeshine, for long-term and
  accidental releases. The **article** is © 2025 Health Physics Society
  (cite, don't reproduce). The **code** is open source under the **MIT
  licence** at <https://github.com/BiswajitSadhu/pyDOSEIA> (checked
  2026-09-28, commit `dca4cdc3`), so its logic may be ported into this
  GPL-3.0 crate with attribution (MIT notice kept, verbatim in
  [`NOTICE`](NOTICE)). **Partly ported as `buangkok::pydoseia` (2026-09-28).**
  Each bundled data table's licence is assessed in the scoping note; none is
  copied.

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
