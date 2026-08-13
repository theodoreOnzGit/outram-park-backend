# KOVAN literature archive — catalogue

The human-readable index of every document in this archive: what it is, what
tier it sits in, what it is good for, and what work depends on it. Maintained
by hand at every librarian pass; last full pass **2026-08-11**.

**Why this file lives here:** it sits at the crate root, directly beside the
`open/` and `proprietary/` trees it indexes, where anyone browsing the archive
on GitHub will meet it. It is deliberately *not* the crate `README.md` — a
crate README carries code-facing duties (crate description, the maintainer's
Bookkeeping-status sign-off block) that do not belong in a library catalogue.
(The crate currently has no `README.md`; writing one is tracked in `op-1lti`.)

## How the archive works

- **Access tier is decided from the document's own copyright page, before
  cataloguing** — never from where it was downloaded. Public hosting (IAEA
  INIS, gen-4.org, nucleus.iaea.org) does not grant redistribution rights.
  When unsure: proprietary. See `kovan_import/README.md` (workspace root) for
  the full intake workflow and `DATA_POLICY.md` for the provenance rules.
- **`open/`** is committed; `.gitignore` deliberately un-ignores
  `open/**/*.pdf` so collaborators get the open literature with the repo.
- **`proprietary/`** and `generated/markdown/proprietary/` are gitignored and
  never leave the machine. The JSON records under `proprietary/` hold
  bibliographic facts only in their metadata; they and their PDFs stay local.
- The canonical record for each document is its **`KovanDocument` JSON**;
  `kovan lit bibtex <json>` regenerates a citation from it on demand. Every
  record below round-trips through `lit bibtex` cleanly (checked 2026-08-11).

## Open tier

### Papers (`open/papers/`)

**`choo2023criticality`** — Choo, A. J. Y. and Xiao, S. (2023). *Criticality
Analysis of HTR-10 Using the High-Temperature Gas-Cooled Reactor Code
Package.* Proceedings of the 15th Vietnam Conference on Nuclear Science and
Technology (VINANST-15), Nha Trang, Vietnam, 9–11 August 2023. IAEA INIS
record [9cr77-d5t43](https://inis.iaea.org/records/9cr77-d5t43),
proceedings report INIS-VN-006.
- Files: `choo-htr10-criticality.{json,pdf}`.
- *Good for:* HTR-10 initial-criticality modelling with Serpent and the HTR
  Code Package (HCP); the SNRSI-local companion to the IAEA benchmark. The
  authors are the maintainer's supervisor (Xiao) and a former colleague.
- *Provenance note:* venue and year are **not stated in the PDF**; they were
  resolved 2026-08-11 from the INIS record (which matches this exact file:
  11 pages, 584.3 kB). The extractor's earlier guess of 2024 was wrong.
  No licence statement in the document — cite and use; do not assume
  redistribution rights beyond the INIS public copy.
- *Used by:* `outram-park-digital-twin-engine/src/htr10/neutronics.rs`
  (which still cites "(2024)" — fix tracked in `op-1lti`); HTR-10 slate
  `op-jyyp`.

**`she2021pangu`** — She, D., Chen, F., Xia, B. and Shi, L. (2021).
*Simulation of the HTR-10 Operation History With the PANGU Code.* Frontiers
in Energy Research 9:704116.
[doi:10.3389/fenrg.2021.704116](https://doi.org/10.3389/fenrg.2021.704116).
- Files: `she2021pangu.{json,pdf}`.
- *Licence:* **CC-BY**, verified from the copyright statement in the document
  itself — redistribution with attribution permitted, PDF committed.
- *Good for:* the full HTR-10 operation history (Jan 2003 – May 2007) as a
  validation target: measured criticality and outlet temperatures vs. PANGU
  fuel-cycle simulation; k_eff agreement below 0.5 percent; shows graphite
  impurity burnup is worth over 1.5 percent in k_eff at end of history.
- *Used by:* HTR-10 slate `op-jyyp` (operation-history validation data);
  complements `xhonneux2014fissionproduct`, which simulates the same history
  for fission-product release.

### Reports (`open/reports/`)

**`iaea2003tecdoc1382part1`** and **`iaea2003tecdoc1382part2`** — IAEA
(2003). *Evaluation of high temperature gas cooled reactor performance:
Benchmark analysis related to initial testing of the HTTR and HTR-10.*
IAEA-TECDOC-1382, Vienna, November 2003. Source:
[www-pub.iaea.org](https://www-pub.iaea.org/MTCD/publications/PDF/te_1382_web/).
- Files: `iaea-tecdoc-1382-part1.{json,pdf}`, `iaea-tecdoc-1382-part2.{json,pdf}`.
- *Tier basis:* openly published IAEA TECDOC.
- *Good for:* **part 2, Chapter 4 is the HTR-10 core-physics benchmark** —
  problems B1 (critical loading height), B2 (k_eff vs. temperature), B3/B4
  (control-rod worths) with participant results. Part 1 is the HTTR half plus
  front matter. This is the anchor document of the HTR-10 slate.
- *Used by:* `docs/reactor-scoping/htr10-neutronics.md`;
  `outram-park-digital-twin-engine/src/htr10/{mod,neutronics}.rs`; epics
  `op-jyyp` (HTR-10 simulator) and `op-hc2o` (graphite thermal scattering).
- *Deduplication (resolved 2026-08-11):* an earlier ingest of the same
  Chapter 4 without TECDOC provenance (`htr-10-iaea`) was removed; part 2 is
  the sole record. Never count them as two sources.

**`kugeler2017vhtr`** — Kugeler, K., Nabielek, H. and Buckthorpe, D. (2017).
*The High Temperature Gas-cooled Reactor: Safety considerations of the
(V)HTR-Modul.* EUR 28712 EN, JRC107642, Publications Office of the European
Union. [doi:10.2760/270321](https://doi.org/10.2760/270321).
- Files: `vhtr-modul-safety-jrc.json`, PDF archived as `kjna28712enn.pdf`.
- *Licence:* reuse authorised provided the source is acknowledged (EC Decision
  2011/833/EU); photos not under EU copyright need separate permission.
- *Good for:* the reference safety treatment of pebble-bed HTGRs — decay-heat
  removal, air/water ingress phenomenology, TRISO fuel performance limits,
  graphite behaviour; 264 pages of curated German HTR programme experience.
- *Used by:* HTR-10 accident children `op-jyyp.13` (air ingress),
  `op-jyyp.14` (water ingress), `op-jyyp.7` (decay-heat path).

**`nrc1982marviken`** — Joint Reactor Safety Experiments in the Marviken
Power Station, Sweden (1982). *The Marviken Full Scale Critical Flow Tests:
Summary Report.* NUREG/CR-2671 (MXC-301), U.S. Nuclear Regulatory Commission,
Washington DC. Manuscript completed December 1979, published May 1982; 288 pp.
Multinational project (Denmark, Finland, France, Netherlands, Norway, Sweden,
USA, West Germany).
- Files: `nureg-cr-2671-marviken.{json,pdf}`; generated markdown at
  `generated/markdown/open/nureg-cr-2671-marviken.md`.
- *Tier basis:* US NRC / NTIS publication — US federal government work, openly
  published. Access tier OPEN, decided from the document's own front matter.
- *Good for:* THE reference for the Marviken full-scale critical-flow
  (choked-flow) blowdown tests. Fig. 8:24 provides the test 23 (3 K nominal
  subcooling) and test 24 (33 K nominal subcooling) critical-mass-flux
  envelopes for the 500 mm / L/D = 0.3 nozzle — this workspace's full-scale
  experimental validation data for subcooled/saturated choked flow.
- *Used by:* the `tampines-steam-tables` Marviken V&V gate
  (`src/steam_turbine_equations/converging_diverging_nozzles/tests/marviken_tests.rs`,
  bead `op-21g.16`) and the subcooled choke branch-selection fix (`op-dqng`).
  The `tampines` drift-flux / two-fluid Marviken comparison cases (`op-ja3t`)
  will cite it too, but have not landed yet.
- *Ingest note (from the JSON record):* the extractor produced a barcode-line
  title and a scan-artefact year (2009); both were corrected by hand at
  ingest. Accessed 2026-08-11.

**`robertson1965msre`** — Robertson, R. C. (1965). *MSRE Design and
Operations Report, Part I: Description of Reactor Design.* ORNL-TM-728, Oak
Ridge National Laboratory, January 1965.
- Files: `msre-design-and-operation.json` (metadata + full extracted text).
  **PDF not archived** — ingested from a since-removed scratch directory;
  re-acquisition from the public ORNL/OSTI mirrors is tracked in `op-1lti`.
- *Tier basis:* US AEC contractor report; the report itself states none of
  its content is classified. Public literature.
- *Good for:* the canonical MSRE plant description — geometry, salt loops,
  drain systems, instrumentation — for the MSRE digital-twin group
  (`outram-park-fork-moltres` / `-onix` / `-thermochimica`).
- *Used by:* MSRE digital-twin epic `op-6w0`; `docs/reactor-scoping/msre.md`.

**`nuclear-digital-twins-and-shadows-review.md`** — Ong Kay Chen, T. (2026).
*Nuclear Digital Twins and Digital Shadows: Literature Review — and Where
Outram Park Fits.* Condensed working review, 24 July 2026.
- A first-party survey (markdown only, no JSON record or PDF — it was born as
  markdown). Citation-verification status is annotated inline.
- *Good for:* orientation in the digital-twin/digital-shadow literature and
  the public positioning of Outram Park within it.
- *Used by:* `outram-park-digital-twin-engine` framing; refinery epic
  `op-1z3o` context.

**`kim1975thermophysical`** — Kim, C. S. (1975). *Thermophysical Properties of
Stainless Steels.* ANL-75-55, Argonne National Laboratory, Chemical Engineering
Division, September 1975. Distribution category LMFBR Fuels and Materials
(UC-79b). [OSTI 4152287](https://www.osti.gov/servlets/purl/4152287).
- Files: `kim1975-thermophysical-properties-stainless-steels.{json,md,pdf}`.
- *Licence:* **US Government work — public domain.** Prepared under contract
  W-31-109-Eng-38 for the U.S. Energy Research and Development Administration;
  the title page carries "DISTRIBUTION OF THIS DOCUMENT IS UNLIMITED". PDF
  committed.
- *Good for:* recommended thermodynamic and transport properties of **Type 304L
  and Type 316L** stainless steel over **300–3000 K**, solid *and* liquid, as
  fitted equations: enthalpy, entropy, specific heat, vapour pressure, density,
  thermal expansion coefficient, thermal conductivity, thermal diffusivity and
  viscosity. Melting range 1670–1730 K, `T_m` = 1700 K, heat of fusion
  64.0 cal/g.
- *Provenance / accuracy notes:* the underlying experimental data run to
  ~1620 K (enthalpy) and ~1600 K (conductivity, density); Kim smoothed by least
  squares and **extrapolated** to the melting range. So 300–1600 K is
  measured-data-backed and 1600–1700 K is the author's extrapolation — the
  distinction is carried through into the code. The scan is OCR-damaged in
  places: the Type 304L *liquid* conductivity slope reads `3.248e-3` but must be
  `3.248e-5` (Kim's own rule that liquid `k` is half solid `k` at `T_m` closes
  only with `-5`, and the parallel 316L equation is `3.279e-5`). Extractor
  metadata was wrong on ingest (title taken as "DISCLAIMER", year as 2013, no
  authors, tier as proprietary) and was corrected by hand; `lit bibtex`
  round-trips cleanly.
- *Used by:* `tuas_boussinesq_solver`'s
  `SolidMaterial::SteelSS304LHighTemp` (`solid_database/ss_304_l_high_temp.rs`),
  which implements the solid-region `c_p`, density and conductivity equations
  over 300–1700 K; consumed by `htgr_sim_v1`'s steam-generator tube metal.
  Beads `op-x0v1`, `op-v9u5`, `op-szmi.17`.

**`pichler2019measurements`** — Pichler, P., Simonds, B. J., Sowards, J. W. and
Pottlacher, G. *Measurements of thermophysical properties of solid and liquid
NIST SRM 316L stainless steel.* Journal of Materials Science, Springer.
[doi:10.1007/s10853-019-04261-6](https://doi.org/10.1007/s10853-019-04261-6).
- Files: `pichler2020-316l-thermophysical-properties.{json,md,pdf}`.
- *Licence:* **CC BY 4.0**, verified from the copyright statement in the
  document, *and* declared an "Official contribution of the National Institute
  of Standards and Technology; not subject to copyright in the United States".
  Redistribution with attribution permitted; PDF committed.
- *Good for:* high-accuracy ohmic pulse-heating measurements of **316L** —
  electrical resistivity, enthalpy, density and thermal expansion from room
  temperature to vaporisation, plus DSC specific heat to ~1400 K. Covers the
  **solid, the melt and the liquid**. All data carry GUM uncertainties, which
  makes it the better of the two steel sources for anything needing stated
  uncertainty rather than a recommended curve.
- *Provenance note:* the PDF is the online-first version and carries **no
  volume or page numbers**; the copyright line reads "© The Author(s) 2019"
  while the journal issue is 2020. The record errs toward what the document
  itself states: year recorded as printed (2019), volume and pages left null
  rather than invented. Resolve from the DOI if a full citation is needed.
- *Used by:* **nothing yet.** Ingested 2026-08-13 alongside `kim1975…` as a
  candidate source for 316L properties; no correlation in the workspace derives
  from it. Its melt and liquid-region data have no consumer because no
  liquid-metal material exists (see `op-k74g`).

### Theses (`open/theses/`)

Three open-access UC Berkeley dissertations from eScholarship, each with
committed PDF, generated markdown and BibTeX. Full provenance, access-terms
discussion and processing steps: **`open/theses/References.md`** (the
authoritative record for these three; they predate the JSON workflow — JSON
backfill is optional, tracked in `op-1lti`).

**`wang2018coupled`** — Wang, X. (2018). *Coupled neutronics and
thermal-hydraulics modeling for pebble-bed Fluoride-Salt-Cooled,
High-Temperature Reactor (FHR).* PhD dissertation, UC Berkeley.
[escholarship.org/uc/item/40q3985m](https://escholarship.org/uc/item/40q3985m).
- *Good for:* FHR pebble-bed coupled-physics methodology feeding the TUAS /
  `tampines` FHR examples.

**`poresky2019model`** — Poresky, C. M. (2019). *Model Network Methodology
for Experimental Development of Industrial Monitoring Systems.* PhD
dissertation, UC Berkeley.
[escholarship.org/uc/item/9bz6h8d2](https://escholarship.org/uc/item/9bz6h8d2).
- *Good for:* monitoring-system methodology relevant to the digital-twin
  engine's instrumentation/shadow side.

**`alivisatos2023evaluating`** — Alivisatos, C. (2023). *Evaluating Remote
Operations for Advanced Nuclear Reactor Control: Feasibility, Benefits, and
Implementation Criteria.* PhD dissertation, UC Berkeley.
[escholarship.org/uc/item/1wt929p1](https://escholarship.org/uc/item/1wt929p1).
- *Good for:* remote-operations framing for the (offline, demonstration-only)
  digital-twin work.

## Proprietary tier (`proprietary/papers/` — local only, never committed)

Bibliographic facts below are fine to publish; the PDFs and extracted bodies
stay on this machine. *Use* means cite and implement from with provenance;
re-hosting is not permitted for any of these.

**`wu2002htr10`** — Wu, Z., Lin, D. and Zhong, D. (2002). *The design
features of the HTR-10.* Nuclear Engineering and Design 218, 25–32.
[doi:10.1016/S0029-5493(02)00182-6](https://doi.org/10.1016/S0029-5493(02)00182-6).
© 2002 Elsevier, all rights reserved.
- *Good for:* the primary published description of the HTR-10 plant — core
  layout, fuel, passive safety systems. First stop for any HTR-10 geometry or
  design parameter.
- *Used by:* `outram-park-digital-twin-engine/src/htr10/design.rs`; `op-jyyp`.

**`gao2002htr10th`** — Gao, Z. and Shi, L. (2002). *Thermal hydraulic
calculation of the HTR-10 for the initial and equilibrium core.* Nuclear
Engineering and Design 218, 51–64.
[doi:10.1016/S0029-5493(02)00198-X](https://doi.org/10.1016/S0029-5493(02)00198-X).
© 2002 Elsevier, all rights reserved. Companion paper to `wu2002htr10`.
- *Good for:* HTR-10 power/temperature/flow distributions for initial and
  equilibrium cores — the TH validation targets for the pebble-bed loop.
- *Used by:* `outram-park-digital-twin-engine/src/htr10/design.rs`; the
  op-jyyp TH children (`op-jyyp.2`, `.3`, `.5`, `.9`).

**`wang2014htr10criticality`** — Wang, M.-J., Sheu, R.-J., Peir, J.-J. and
Liang, J.-H. (2014). *Criticality calculations of the HTR-10 pebble-bed
reactor with SCALE6/CSAS6 and MCNP5* (Technical Note). Annals of Nuclear
Energy 64, 1–7.
[doi:10.1016/j.anucene.2013.09.031](https://doi.org/10.1016/j.anucene.2013.09.031).
© 2013 Elsevier, all rights reserved.
- *Good for:* quantified k_eff biases of double-heterogeneity unit-cell
  treatments vs. continuous-energy reference (INFHOMMEDIUM ~+2800 pcm,
  DOUBLEHET ~+280 pcm) — directly relevant to `outram-mc-libs` pebble-bed
  transport choices.
- *Used by:* `outram-park-digital-twin-engine/src/htr10/neutronics.rs`.

**`tantillo2020hcpneutronics`** — Tantillo, F., Kasselmann, S., Xhonneux,
A., Lambertz, D., Trabadela, A. and Allelein, H.-J. (2020). *HTR code
package neutronics developments and benchmarks.* Nuclear Engineering and
Design 362, 110603.
[doi:10.1016/j.nucengdes.2020.110603](https://doi.org/10.1016/j.nucengdes.2020.110603).
© 2020 Elsevier, all rights reserved.
- *Good for:* HTR-10 first-criticality benchmark results B1/B2 with the HCP
  (TRISHA spectrum code, MGT-N/MGT-3D comparison); the code package that
  `choo2023criticality` validates against.
- *Used by:* `outram-park-digital-twin-engine/src/htr10/neutronics.rs`.

**`xhonneux2014fissionproduct`** — Xhonneux, A., Druska, C., Struth, S. and
Allelein, H.-J. (2014). *Calculation of the Fission Product Release for the
HTR-10 based on its Operation History.* Proceedings of the HTR 2014, Weihai,
China, 27–31 October 2014, Paper HTR2014-5-181.
- *Tier basis (investigated 2026-08-11):* no licence statement in the
  document; no JuSER open-access deposit found. The archived PDF is
  byte-identical to the copy publicly hosted on the
  [IAEA HTGR Knowledge Base](https://nucleus.iaea.org/sites/htgr-kb/HTR2014/Paper%20list/Track5/HTR2014-51181.pdf),
  but public hosting grants no redistribution rights — **stays proprietary**.
- *Good for:* fission-product release (Ag-110m, Cs-137, Sr-90, I-131) over
  the real HTR-10 operation history with VSOP/STACY; the release-side
  companion to `she2021pangu`.
- *Used by:* `boon-lay` TRISO-release context (`op-jyyp.10`).

**`huang2025waterIngress`** — Huang, H., Xie, R., Liu, S., Wu, X., Cheng, G.
and Zhang, Y. (2025). *The water ingress analysis on steam generator
heat-exchange tube rupture accident of high temperature gas-cooled reactor.*
Annals of Nuclear Energy 211, 110968.
[doi:10.1016/j.anucene.2024.110968](https://doi.org/10.1016/j.anucene.2024.110968).
© 2024 Elsevier — all rights reserved **including text and data mining, AI
training, and similar technologies**.
- *Special status:* the explicit TDM/AI reservation means **full text was
  deliberately not extracted**; the record is metadata plus factual findings
  only, pending a maintainer decision under AI_USAGE.md (bead `op-b7bx`).
  It is the only document in the archive carrying that clause (all
  proprietary PDFs audited 2026-08-11).
- *Good for:* SGTR water-ingress transient analysis for HTR-10
  (RELAP5/MOD3.2): reactivity insertion, graphite oxidation, combustible gas.
- *Used by:* HTR-10 accident child `op-jyyp.14` (water/steam ingress).

**`tobias1980decay`** — Tobias, A. (1980). *Decay Heat.* Progress in Nuclear
Energy 5, 1–93.
[doi:10.1016/0149-1970(80)90002-5](https://doi.org/10.1016/0149-1970(80)90002-5).
© 1980 Pergamon Press, all rights reserved (no TDM/AI clause — predates them).
- *Good for:* the canonical review of decay-heat evaluation — burst
  functions, summation methods, standards, uncertainties. The decay-heat
  figures are the planned **golden oracle for the graph digitiser**
  (`op-didp` / `op-amfh`); the digitised *points* are facts and can be
  committed with provenance even though the scans stay local.
- *Used by:* `op-jyyp.8` (fix the suspect decay-heat model), digitiser V&V.
- *Note:* 1980 scan OCR'd in 2003 — text quality imperfect in places.

## Proprietary tier — reports (`proprietary/reports/`)

**`terry2005evaluation`** — Terry, W. K., Kim, S. S., Montierth, L. M.,
Cogliati, J. J. and Ougouag, A. M. (2005). *Evaluation of the HTR-10 Reactor as
a Benchmark for Physics Code QA.* INL/CON-05-00852 **(PREPRINT)**, Idaho
National Laboratory; International Reactor Physics Experiment Program Working
Group Meeting, November 2005. Obtained from
<https://www.osti.gov/servlets/purl/911178> (accessed 2026-08-13).
- *Tier rationale:* the preprint's own first page states it "should not be
  cited or reproduced without permission of the author". OSTI hosting grants no
  redistribution rights — tier follows the copyright page, not the host.
- *Good for:* the IRPhEP benchmark-model dimensions of the HTR-10 initial
  criticality experiment. Its Table 2 supplies the axial build that
  IAEA-TECDOC-1382 part 2 does not carry as text — core cavity height
  221.818 cm, conus height 36.946 cm — plus packing fraction 0.61 and the
  19.5° upper-surface cone angle (the latter DEM-calculated, not measured).
- *Does NOT contain:* the 83-zone R-Z boundaries of TECDOC Table 4-3. It is a
  summary and directs the reader to the IRPhEP evaluation report itself.
- *Citation caution:* for publication cite the IRPhEP evaluation report or the
  IAEA TECDOCs, not this preprint, unless permission is obtained.
- *Used by:* `docs/reactor-scoping/htr10-neutronics.md` (final section, values
  transcribed with provenance); `op-tvmf`, `op-lhu6`, `op-5c5r`.

## Proprietary tier — theses (`proprietary/theses/`)

All three are UC Berkeley Electronic Theses and Dissertations, publicly readable
via eScholarship but carrying a bare `Copyright <year>` with no reuse licence —
author retains all rights, so: proprietary.

**`wang2018coupled`** — Wang, Xin (2018). *Coupled neutronics and
thermal-hydraulics modeling for pebble-bed Fluoride-Salt-Cooled,
High-Temperature Reactor (FHR).* PhD thesis, UC Berkeley.
<https://escholarship.org/uc/item/40q3985m>. © 2018 the author.
- *Good for:* coupled neutronics/TH methodology for a pebble-bed FHR;
  COMSOL-based neutron-diffusion + heat-transfer coupling. Relevant to the
  Mk1 FHR line and to pebble-bed coupling strategy generally.

**`poresky2019model`** — Poresky, Christopher Morris (2019). *Model Network
Methodology for Experimental Development of Industrial Monitoring Systems.*
PhD thesis, UC Berkeley. <https://escholarship.org/uc/item/9bz6h8d2>.
© 2019 Christopher Poresky.
- *Good for:* monitoring-system and digital-twin methodology; operator-facing
  fault interfaces. Relevant to `outram-park-digital-twin-engine`.

**`alivisatos2023evaluating`** — Alivisatos, Clara (2023). *Evaluating Remote
Operations for Advanced Nuclear Reactor Control: Feasibility, Benefits, and
Implementation Criteria.* PhD thesis, UC Berkeley.
<https://escholarship.org/uc/item/1wt929p1>. © 2023 the author.
- *Good for:* remote-operations feasibility and control-room criteria for
  advanced reactors. Context for the digital-twin engine's intended-use
  boundary — note RESPONSIBLE_USE.md forbids operational deployment.

## Librarian history

- **2026-08-11** — full pass: staging cleared (two new documents catalogued,
  eight SHA-verified duplicates deleted); metadata defects fixed across the
  archive (furniture titles, scan-date years, missing hashes, wrong slugs);
  `htr-10-iaea` duplicate removed in favour of `iaea2003tecdoc1382part2`;
  Choo venue/year resolved from INIS; Xhonneux provenance investigated and
  left proprietary; TDM/AI-clause audit of all proprietary PDFs (only
  `huang2025waterIngress` carries it). Beads: `op-nv6g`, `op-b7bx`,
  `op-1lti`.
- **2026-07-30** — theses re-verified against eScholarship deposits (see
  `open/theses/References.md`).
