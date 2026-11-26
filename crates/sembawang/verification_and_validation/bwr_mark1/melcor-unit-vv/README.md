# MELCOR 2.2 unit-level V&V references for `bwr_mark1`

> Research, education and V&V only. AI-extracted draft, **not reviewed by a
> human**. Not validated.

**What this is.** These are the reference items the unit tests of
`sembawang::bwr_mark1` (and `outram_park_fork_offbeat::melcor`) are checked
against: the "concrete V&V at unit-test level" that the root `AGENTS.md` rule
asks for before any regression pin ("No regression pins until there is
concrete V&V at unit-test level"; maintainer, 2026-11-26: "Take whatever is
suitable for AI v&v, put it in markdown, then begin unit testing. That's when
u shld pin").

Each item is a self-contained correlation, closure, property, rate law or
balance. It carries:

- its citation: section, equation, table and page label;
- its default value, from the Users' Guide's sensitivity-coefficient card
  (`Cnnnn`);
- the equation restated in our own symbols;
- worked cases computed with `python3` (scripts in `calc/`), to at least 6
  significant figures, with edge cases;
- where it is implemented, and which test checks it;
- every discrepancy found between our code and the manual, and inside the
  manuals themselves.

## Provenance and access

| | |
|---|---|
| Sources | **V2**: Sandia National Laboratories (2023), *MELCOR Computer Code Manuals, Vol. 2: Reference Manual, Version 2.2 r2023.0*, SAND2023-10994 O (page labels `XXX-RM-nn`). **V1**: Sandia National Laboratories (2017), *MELCOR Computer Code Manuals, Vol. 1: Primer and Users' Guide, Version 2.2.9541*, SAND2017-0455 O (page labels `XXX-UG-nn`) |
| Held in | the maintainer's private repository `propreitrary-literature-theodore`, `reports/` |
| Access tier | **Proprietary** (no public-release or licence statement), as for `crates/kovan-literature/derived/melcor-2.2-manuals-digest.md`. The PDFs and their extracted text stay private. These files hold **facts, equations and numbers with citation**, restated in our own words; no prose is copied |
| Processing | 2026-11-26: both manuals extracted with `pdftotext -layout` to a session scratchpad outside every repository. Five AI agents each read one package group. Every expected value was computed with `python3` from the manual's equations and constants, never from our code |
| Review | **None yet.** Where the extraction damaged an equation, the agent rebuilt it and says so in the item ("check against the PDF"). Those items must be checked against the printed manual before a test relies on them |

## The files

| File | Package(s) | Items |
|---|---|---|
| [`cor-part1.md`](cor-part1.md) | COR, first half: emissivities, gray radiation, exchange factors, gap conductance, convection and boiling, Lipinski, quench, ACOPO, Urbanic-Heidrick and other oxidation, heats of reaction, steel and B4C oxidation, eutectics, holdup and collapse, time at temperature, candling, debris area | 27 |
| [`cor-part2.md`](cor-part2.md) | COR, second half: core plate (yield, stress, creep, buckling), lower head (Larson-Miller creep, mechanics, heat transfer, melting), debris ejection and opening ablation, Ag-In-Cd and Booth release, heat-pipe limit, falling-debris quench, Lipinski, ACOPO, B4C | 22 |
| [`cvh-dch-hs.md`](cvh-dch-hs.md) | CVH/FL pool and flow paths (bubble rise, swollen level, critical flow), DCH decay heat, HS convection, condensation, boiling. **It also answers what MELCOR's pool swell counts**, against `swollen_level_from_sources` | 22 |
| [`bur-cav-fdi-lhc.md`](bur-cav-fdi-lhc.md) | BUR combustion, CAV core-concrete interaction, FDI fuel dispersal, LHC | 26 |
| [`rn-aerosol-sparc-mp.md`](rn-aerosol-sparc-mp.md) | RN release (CORSOR, CORSOR-M, Booth), aerosol physics, vapour, SPARC pool scrubbing, iodine, MP material properties, SPR | 25 |

122 items in all.

## Status of the unit tests against these items

Updated as the tests are written. A pin on any output that depends on an
item is allowed only once that item's test exists and passes (root
`AGENTS.md`).

| Item | Test | Result |
|---|---|---|
| (filled as tests land) | | |

## Discrepancies found, for decision (not fixed in this extraction)

The items list them in full. The ones that change the physics most:

- **B4C oxidation runs too early** (COR1-22, COR2-17). It is missing the
  1500 K threshold (C1005(4)) and the sheath-failure gate (C1005(3)), so the
  2 % cap can be spent in about 19 s at 1200 K.
- **Gray exchange between core surfaces** (COR1-03) matches MELCOR's series
  solution only for a view factor of 1: a factor 2.04 low at F = 0.25.
- **The swollen level** (`swollen_level_from_sources`, CVH items) fixes the
  first-relocation quench but still differs from MELCOR in six stated ways.
  The doc comment's "1 cm grid" explanation of its 1.25 % offset is wrong.
- **CORSOR-M** uses R = 8.314462 J/(mol K) where the manual's
  1.987e-3 kcal/(mol K) applies: release rates are 0.14 to 0.33 % off.
- **Zircaloy enthalpy** runs +109 J/kg high from 1113 K. This is inherited
  from the OFFBEAT MATPRO port, which keeps an upstream interpolation quirk
  on purpose. Needs a decision.
- **Cs vapour pressure:** the two manuals give different curves, 5.6 decades
  apart at 1000 K. Needs a decision.
- **Lower-head creep** (COR2-07) uses a plain node mean, not the
  mass-averaged wall temperature: up to a factor of 0.59 in rupture time.
- **Smaller ones**, each in its item: steel diffusion constant +20.5 %,
  holdup minimum oxide thickness, rod-collapse temperature (fuel vs
  cladding), gravity 9.80665 vs 9.8, Zircaloy density 6550 vs 6500.

Contradictions **inside** the manuals (V1 against V2, or an equation against
its own table) are listed in each file. Where one matters, a test cites
which reading it pins.
