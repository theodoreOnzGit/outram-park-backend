# BWR thermal-hydraulics and mechanistic boiling: literature review (gh:#895)

> ⚠️ **Research, education and V&V only.** Not for nuclear facility operation, reactor control, licensing, or safety-critical decisions. See `RESPONSIBLE_USE.md`.

**Date:** 2026-10-16.

**Method.** An agent fleet of three read-only slices. Each slice's full report is
kept here unedited, apart from a status note in 01:

| File | Slice |
|---|---|
| [`01-workspace-audit.md`](01-workspace-audit.md) | What the workspace already has, with file:line paths |
| [`02-mechanistic-boiling-literature.md`](02-mechanistic-boiling-literature.md) | Mechanistic models: ONB/OSV, subcooled void, RPI partitioning, flow boiling, CHF/DNB, annular dryout, post-dryout, pool boiling |
| [`03-open-vv-data-and-bwr-system-models.md`](03-open-vv-data-and-bwr-system-models.md) | Open V&V datasets with their access class; drift flux, two-phase friction, stability, critical flow |

**How far to trust the citations.** The sandbox could not fetch NEA, NRC, HAL or
Crossref pages directly. Every bibliographic detail and access term therefore
comes from search-result text. The reports mark anything unconfirmed as
**"not verified"**, and only a handful of DOIs were seen printed in full.
**Re-check a citation before it enters code or kovan.** The maintainer supplies
the kovan corpus; nothing here was ingested.

## The finding in one paragraph

The workspace has **many boiling closures and no heated boiling channel that
uses them**. There are four lineages that share no code:

- `outram-foam-multiphase`: RPI wall boiling; Biasi, W-3 and Bowring CHF; a
  Groeneveld LUT interpolator holding **made-up sample data**.
- The GeN-Foam closure port: Chen, Saha–Zuber, Basu ONB, eight two-phase
  multipliers, Bestion drag. Its **two-phase driver is not ported**.
- `bedok`: HEM channel with Zuber–Findlay void and W-3 DNBR.
- DWSIM pipe correlations.

`tampines`' drift-flux and two-fluid pipe solvers are adiabatic. **Nothing
boiling-related is validated against a boiling experiment.** The only
experimental two-phase validation is critical flow (Marviken, Moody/Zaloudek)
and blowdown (Edwards).

## What is missing, ranked

1. **Annular film-flow dryout**: the BWR CHF mechanism (Whalley/Hewitt
   structure, with Ishii–Mishima or Okawa entrainment). `dryout.rs` returns
   `NotImplemented` for it.
   - It is mechanistic, so it can be run **uncalibrated first** under the
     model-hierarchy rule.
   - It replaces any temptation to use GEXL, which is **proprietary and must
     not be sought**.
2. **Real CHF data.**
   - NUREG/KM-0011 (NRC, 2019) holds the 25 000+ points behind the 2006
     Groeneveld LUT. It is an open US-government report and the best open
     dryout/CHF validation set found.
   - Whether the LUT *values* may be committed is a licence decision for the
     maintainer.
3. **A heated channel.** `DriftFlux1d` needs wall heat, subcooled void
   (Saha–Zuber is present; Levy or Lahey profile) and a two-phase friction
   multiplier. All the closures exist in the GeN-Foam port but none is wired
   into `tampines`.
4. **Ishii (ANL-77-47) regime-dependent drift-flux closures.** Open on UNT. This
   is the cheapest win, because it extends code that already exists.
5. **Alternative flow-boiling correlations:** Gungor–Winterton, Liu–Winterton,
   Steiner–Taborek. Python `ht` (MIT licence) can serve as an independent
   cross-check.
6. **MIT (Kommajosyula) partitioning,** with TrioCFD (BSD-3) as the open
   reference. This is a CFD-scale model, so it comes later.

## First V&V cases, open data first

| # | Case | Access class | Validates |
|---|---|---|---|
| 1 | Groeneveld 2006 LUT / dryout model vs NUREG/KM-0011 | A (open NUREG) | CHF/dryout |
| 2 | Ishii drift-flux closures against Ishii's own regime equations | A (ANL-77-47) | void / drift flux |
| 3 | Bartolomei subcooled void, digitised, run uncalibrated | B (paper figures) | ONB/OSV, subcooled void |
| 4 | DEBORA area-averaged void, **if** the reported CC BY 4.0 release is confirmed | A? | subcooled void, fluid scaling |
| 5 | Super Moby Dick critical flow (Henry–Fauske, DEM) | access not verified | critical flow |
| 6 | Two-phase multipliers cross-checked | B | pressure drop |
| 7 | March-Leuba reduced-order stability model, analytical | A (ORNL/NUREG) | density-wave stability |
| 8 | Marchaterre ANL-5735 natural circulation | A (UNT scan) | natural-circulation void |

## Data the project must NOT build on without the maintainer

- **BFBT and PSBT numeric databases** (NEA-1731 and NEA-1849). These are
  participant-only under an NDA. Only the public specification and results
  reports may be used.
- **Ringhals-1** (NEA-restricted) and **Oskarshamn-2** (needs an NDA with the
  plant owner).
- **Peach Bottom turbine trip.** The specification is public, but the data is
  **measured operating-plant data**, which `DATA_POLICY.md` excludes.
  Maintainer decision.
- KATHY (likely proprietary), HZDR MORENA (restricted), GEXL (proprietary).

## Where Carey (*Liquid-Vapor Phase-Change Phenomena*) is needed

The maintainer holds it. Only one chapter number was confirmed: 2nd edition
**ch. 12 is internal-flow convective boiling**, which covers Steiner–Taborek,
annular flow and dryout, and post-CHF. The reports mark "→ Carey" for:

- ONB criteria (Davis–Anderson, Sato–Matsumura);
- the annular film-flow dryout model;
- Steiner–Taborek;
- pool-boiling CHF (Zuber) and the Nukiyama curve;
- T_min (Henry, Berenson).

**The annular dryout model is the one to ask for first**, because it is item 1
above and its originals (AERE reports, the IHTC-5 paper) were not found openly.

## Open questions for the maintainer

1. Commit the 2006 Groeneveld LUT values, or validate only against the
   NUREG/KM-0011 data?
2. Peach Bottom TT: is public-spec operating-plant data acceptable?
3. Where does the heated BWR channel live? The candidates are
   `tampines::multiphase_1d` (closest; adiabatic today), the GeN-Foam two-phase
   driver (unported, bead `op-p6p.7.12`) and `bedok`.
4. The GeN-Foam licence was not verified by the fleet. It matters for porting
   its `lookUpTableCHF`.
