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

What this crate makes **studyable and challengeable** is a published dose
code, read line by line: a Rust translation of pyDOSEIA (Sadhu et al. 2026)
that a student can open beside the Python, together with the 26 defects
(D1–D26) found by reading upstream, each reproduced and, where a correction
exists, offered as a labelled divergence rather than a silent fix. The
code-to-code fixture shows the port computes what pyDOSEIA computes; whether
pyDOSEIA's model represents reality is the open question this crate leaves
for the reader to challenge.

## Knowledge in this crate

See the workspace [vision](../../VISION.md).

| Knowledge artifact | In this crate |
|---|---|
| Scientific literature | pyDOSEIA: Sadhu et al., *Health Physics* **130**(1) (2026) 94–110, doi:10.1097/HP.0000000000002014; code at <https://github.com/BiswajitSadhu/pyDOSEIA>, commit `dca4cdc3`, MIT (notice in [`NOTICE`](NOTICE)). Dose coefficients: US EPA FGR-15 (2025, EPA 402-R-25-001) and FGR-11. HTR-10 dose tables: Liu and Cao (2002), Tables 7 and 9. Provenance in [`docs/References.md`](docs/References.md). |
| Mathematical equations | Gaussian-plume master equation (Hukkoo and Bapat eq. 2.5) with BARC/AERB sigma fits, height correction, triple joint frequency distribution and calm correction, the inhalation, ground-shine, submersion, ingestion (IAEA SRS 19 food chain; H-3 and C-14 specific activity) and finite-cloud plume-shine (gamma build-up) pathways, in the module docs of `src/pydoseia/`. The plume equations are walked through in `changi`'s lesson book, [`crates/changi/docs/lessons/src/plume.md`](../changi/docs/lessons/src/plume.md). |
| Source code | `src/pydoseia/` (`met`, `dispersion`, `dcf`, `dose`, `ingestion/`, `plume_shine/`, `quadpack`, `dcf_screening`, `plume_rise`, `config`, `assessment`), `src/coefficients.rs` (FGR tables in `reference/`), `src/published/` (Liu and Cao tables). |
| Interactive simulations | No web demo or GUI in this crate. CLI examples: `examples/plume_chi_over_q.rs`, `examples/plume_rise_and_wake.rs`, `examples/pydoseia_assessment.rs`. The egui app `crates/outram-park-digital-twin-engine/examples/htgr_sim_v1` (its Map tab) calls the dose-rate functions. |
| Visualisations | None yet. |
| Verification and validation evidence | Code-to-code against upstream: [`docs/pydoseia-code-to-code.md`](docs/pydoseia-code-to-code.md), `tests/pydoseia_code_to_code.rs`, generator in `verification_and_validation/pydoseia_code_to_code/`. Recorded 2026-11-13 (gh:#1429): 1 917 cases, 18 217 values, 44 groups, all passing; every group bit-exact except `boundary_totals_text` (limited by the four significant figures of upstream's text report) and one `boundary_totals` value at 1.1e-16. Ten mutation tests show the fixture can fail. Cross-check against Liu and Cao's published doses (external pathways only): `tests/liu_cao_external_dose_cross_check.rs`, 2026-09-29, recorded as "consistent, not verified". No validation against measurement. |
| Human reviews and corrections | 0 functions carry a kovan human-review stamp. No entry in `docs/human-corrections-to-ai-work.md`. Struck-through corrections in this README, `src/lib.rs` and the code-to-code doc (ingestion and plume shine "not ported", the QUADPACK-vs-petir integrator choice, the 1 899-case fixture count). |

**Status:** the port has code-to-code verification evidence against
pyDOSEIA; nothing here is validated, and no part has been human-reviewed.

## Status: pyDOSEIA ported, code-to-code verified (2026-09-28)

~~Status: placeholder, nothing is implemented.~~ **CHANGED 2026-09-28.** The
crate was created that day to reserve the name. It now holds:

- **`buangkok::pydoseia`**, a faithful Rust port of **all of the computation**
  in the MIT-licensed **pyDOSEIA** (commit `dca4cdc3`, see below), in two
  tranches the same day:
  - met processing: the triple joint frequency distribution, and the missing
    and calm corrections;
  - Gaussian-plume dilution factors for three release modes: a single plume,
    and a long-term release with or without met data (the fourth mode cannot
    run upstream and is offered as a labelled correction);
  - the **inhalation**, **ground-shine** and **submersion** dose pathways, with
    their age brackets, absorption-type selection, progeny correction,
    deposition velocities and weathering;
  - **ingestion** (IAEA SRS 19 food chain: crops, pasture, stored feed, milk,
    meat; H-3 and C-14 specific-activity models);
  - **plume shine** (finite-cloud gamma dose with build-up), integrated by
    default with `petir`'s port of GSL QAGS; a port of the SciPy QUADPACK
    integrator upstream relies on is kept as the bit-exact regression
    reference (the two agree to the last bit on the fixture's 120 integrals);
  - multi-source DCF screening, plume rise, the run configuration and its
    defaults, and the driver with its summary tables (a Rust API replaces the
    CLI; see `examples/pydoseia_assessment.rs`).
- **Verification:** code-to-code against pyDOSEIA itself, on synthetic inputs.
  ~~The fixture has 1 899 cases and 18 127 values in 43 function groups; 41 are
  bit-exact, one agrees to 4.8e-16 (a pandas mean) and one is limited by the
  four significant figures of upstream's text report.~~ **CORRECTED
  2026-11-27** — the fixture now has 1 917 cases and 18 217 values in 44
  groups; `speed_means` became bit-exact and a new in-memory
  `boundary_totals` group was added (gh:#1429, 2026-11-13; checked against
  the Results section of `docs/pydoseia-code-to-code.md`). Ten mutation tests show
  the suite can fail. Methodology and results are in
  [`docs/pydoseia-code-to-code.md`](docs/pydoseia-code-to-code.md).
- **What this does not show:** agreement with pyDOSEIA is not agreement with
  reality. There is **no validation** of any kind, and 26 upstream defects are
  recorded (D1–D26), some large (e.g. D10: long-term C-14 ingestion 3.15e7
  times too high; D20: the summary CSV counts ingestion twice). The port
  reproduces them, and corrected variants are labelled as divergences.
- **Not ported:** I/O and UI only (Excel reading, plots, text formatting, the
  interactive input generator, joblib). Every upstream function's status is
  in [`docs/pydoseia-port-scoping.md`](docs/pydoseia-port-scoping.md).
- ~~**No dose-coefficient or nuclear data ships with the crate.**~~
  **CHANGED 2026-09-29:** the pyDOSEIA port itself still ships none (upstream's
  tables are ICRP-, IAEA- or JAEA-derived, or of unestablished terms, so a
  caller supplies tables in upstream's CSV layout), but `buangkok::coefficients`
  now ships one such caller-supplied set: **US EPA FGR-15 (2025 revision, EPA
  402-R-25-001)** air-submersion (Table 4-6) and ground-surface (Table 4-1)
  dose-rate coefficients and **FGR-11** (Table 2.1) inhalation coefficients,
  for Kr-85, Xe-133, I-131, Cs-137 (+ Ba-137m) and Ag-110m (all ages) ~~only~~,
  plus, since 2026-09-29 (gh:#379), FGR-15 Adult-only rows for the 18 other
  nuclides Liu & Cao release, with
  provenance in [`docs/References.md`](docs/References.md). Half-lives come
  from `boon-lay`, not from here.
- ~~Nothing in the workspace calls it yet. It is not wired into
  `htgr_sim_v1`.~~ **Wired into `htgr_sim_v1` (2026-09-29):** the Map tab's
  "Dose rate" basis and dose-rate table compute an **indicative** effective
  dose rate (µSv/h) through `pydoseia::dose::submersion_dose_rate_msv_per_s`,
  `inhalation_committed_dose_rate_msv_per_s` and
  `ground_shine_dose_rate_msv_per_s` (the coefficient products the ported
  pathways are built on; the code-to-code fixture still passes bit for bit)
  and the `coefficients` tables. Indicative, research/education only, not a
  dose to any real person, not for emergency or regulatory use.

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
  ~~GPL-3.0~~ **AGPL-3.0-only** crate with attribution (MIT notice kept, verbatim in
  [`NOTICE`](NOTICE)). ~~**Partly ported as `buangkok::pydoseia` (2026-09-28).**~~
  **Ported as `buangkok::pydoseia` (2026-09-28; I/O and UI excepted).**
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

## `no_std` + `alloc` (gh:#1593)

The default `std` feature can be turned off: the library is then `no_std` +
`alloc` and builds for bare metal
(`cargo check --release -p buangkok --no-default-features --target x86_64-unknown-none`,
gated by `scripts/check-nostd.sh`). `std` turns on `changi`'s `std` (its thread pool). With `std` on (the default)
nothing changes.

## License

~~GPL-3.0~~ **AGPL-3.0-only** (relicensed 2026-10-20, gh:#973). Part of the [OUTRAM PARK](../../README.md) workspace.
