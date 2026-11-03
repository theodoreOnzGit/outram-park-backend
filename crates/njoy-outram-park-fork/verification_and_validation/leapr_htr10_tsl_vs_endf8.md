# LEAPR regeneration of the four HTR-10 scattering laws vs ENDF/B-VIII.0

**Generated:** 2026-11-03 (UTC), runs between 03:10 and 03:40
**Crate version / commit:** worktree on `c7552b366` plus this change

GitHub #1140 (prerequisite for HTR-10 per-zone temperatures, epic #1137).
Gate: `tests/leapr_htr10_tsl_vs_endf8.rs`.
**AI-assisted draft, not yet human-reviewed.**

## Question

The HTR-10 model will generate the scattering laws of reactor graphite 10P
(MAT 31), reactor graphite 30P (MAT 32), C in SiC (MAT 44) and Si in SiC
(MAT 43) by LEAPR at arbitrary zone temperatures (20–1800 °C). Does this
crate's LEAPR reproduce the published ENDF/B-VIII.0 tapes at the
temperatures those tapes tabulate?

## Methodology

**Under test:** the path `outram-mc-libs`'
`ThermalScattering::from_embedded_leapr_deck` (the wasm-safe path the demos
use): the deck compiled into the crate (`leapr::decks::embedded_deck_text`)
→ `LeaprDeck::parse` → `leapr::run::run_deck_at_temperature` (the deck cut to
one temperature, LEAPR's whole driver run on it) → `LeaprRun::write_text`
(`endout` applies NJOY's `sigfig`) → `Tape::read` →
`thermr::mf7::parse_mf7_at_temperature`. One generation per tabulated
temperature. The deck's own `EVAL` date selects the physical constants
(`EVAL-SEP17`, pre-2018 NJOY constants), as for every deck here.

**Reference:** `reference-data/endf/tsl-reactor-graphite-10P.endf`,
`tsl-reactor-graphite-30P.endf`, `tsl-CinSiC.endf`, `tsl-SiinSiC.endf`
(ENDF/B-VIII.0 thermal sub-library, NCSU LEIP; provenance in
`reference-data/endf/README.md`), read through the same parser at the same
temperature. Graphite tabulates 296, 400, 500, 600, 700, 800, 1000, 1200, 1600,
2000 K; SiC 300, 400, 500, 600, 700, 800, 1000, 1200 K.

**Compared, and the criterion fixed before measuring:**

| quantity | how | gate |
|---|---|---|
| MT=4 grids | `beta`, every `alpha` row, `LAT`/`LASYM`/`LLN` | exact |
| MT=4 `S(α,β)` | every stored value; bit-identical count over all 60 000; worst relative difference over reference values > 1e-30 | 1e-9 relative (the gate `tests/leapr_run_at_temperature.rs` uses for this driver against NJOY2016) |
| MT=4 `B(1..6)`, `T_eff(T)` | exact / relative | exact / 1e-9 |
| graphite MT=2, single-T run | edge energies and `S(E_i,T)` position by position up to the shorter list's last slot; the step functions `S(E)` at every edge of either list past it; `S(E→∞)` | **one unit in the 7th significant figure** ("print round-off", the bound #1140 set) |
| graphite MT=2, whole deck through `run_deck` | every row of the 10-temperature section | bit-identical |
| graphite MT=2, cause check | the 296 K run's `LeaprOutput` and the T run's spliced into one two-temperature `endout` call, row T vs the tape's | bit-identical |
| `SabRequest` path (`generate::thermal_scattering_tape`) | MT=4 (and graphite MT=2) at the base temperature | as above |
| SiC MT=2 | the elastic law kind (`LTHR`) of each | reported; the difference is pinned, not gated as agreement |

## Reference

```bibtex
@article{brown2018endfb8,
  author  = {Brown, D. A. and others},
  title   = {{ENDF/B-VIII.0}: The 8th Major Release of the Nuclear Reaction
             Data Library with {CIELO}-project Cross Sections, New Standards
             and Thermal Scattering Data},
  journal = {Nuclear Data Sheets}, volume = {148}, pages = {1--142}, year = {2018},
  doi     = {10.1016/j.nds.2018.02.001}
}
```

Thermal sub-library files `tsl-reactor-graphite-10P.endf` (MAT 31),
`tsl-reactor-graphite-30P.endf` (MAT 32), `tsl-CinSiC.endf` (MAT 44),
`tsl-SiinSiC.endf` (MAT 43), MF=7/MT=2 and MT=4; the LEAPR decks are the
evaluators' (NCSU LEIP, `EVAL-SEP17`), provenance in
`docs/leapr-deck-provenance.md`.

## Results

### MT=4 (incoherent inelastic) — bit-identical at all 36 temperatures

"identical" counts all 60 000 stored values (150 β × 400 α for graphite,
300 × 200 for SiC); "> 1e-30" is the count the relative difference is taken
over. `B(1..6)` was identical at every temperature.

| material | T (K) | values > 1e-30 | identical / 60 000 | worst rel. | `T_eff` ours = tape (K) |
|---|---|---|---|---|---|
| 10P | 296 | 54 049 | 60 000 | 0 | 690.5411 |
| 10P | 400 | 54 691 | 60 000 | 0 | 738.3295 |
| 10P | 500 | 55 085 | 60 000 | 0 | 794.8527 |
| 10P | 600 | 55 360 | 60 000 | 0 | 859.6977 |
| 10P | 700 | 55 575 | 60 000 | 0 | 931.0943 |
| 10P | 800 | 55 741 | 60 000 | 0 | 1007.600 |
| 10P | 1000 | 55 988 | 60 000 | 0 | 1171.708 |
| 10P | 1200 | 56 167 | 60 000 | 0 | 1345.883 |
| 10P | 1600 | 56 421 | 60 000 | 0 | 1711.644 |
| 10P | 2000 | 56 599 | 60 000 | 0 | 2090.186 |
| 30P | 296 | 53 675 | 60 000 | 0 | 573.8257 |
| 30P | 400 | 54 269 | 60 000 | 0 | 631.8319 |
| 30P | 500 | 54 637 | 60 000 | 0 | 698.1828 |
| 30P | 600 | 54 898 | 60 000 | 0 | 772.1301 |
| 30P | 700 | 55 098 | 60 000 | 0 | 851.6220 |
| 30P | 800 | 55 263 | 60 000 | 0 | 935.1903 |
| 30P | 1000 | 55 506 | 60 000 | 0 | 1110.715 |
| 30P | 1200 | 55 688 | 60 000 | 0 | 1293.505 |
| 30P | 1600 | 55 943 | 60 000 | 0 | 1671.103 |
| 30P | 2000 | 56 132 | 60 000 | 0 | 2057.257 |
| C in SiC | 300 | 46 471 | 60 000 | 0 | 553.2627 |
| C in SiC | 400 | 47 412 | 60 000 | 0 | 604.7319 |
| C in SiC | 500 | 48 056 | 60 000 | 0 | 670.2986 |
| C in SiC | 600 | 48 547 | 60 000 | 0 | 745.1514 |
| C in SiC | 700 | 48 946 | 60 000 | 0 | 826.1828 |
| C in SiC | 800 | 49 265 | 60 000 | 0 | 911.4496 |
| C in SiC | 1000 | 49 764 | 60 000 | 0 | 1090.169 |
| C in SiC | 1200 | 50 147 | 60 000 | 0 | 1275.609 |
| Si in SiC | 300 | 43 976 | 60 000 | 0 | 421.3892 |
| Si in SiC | 400 | 44 735 | 60 000 | 0 | 495.6893 |
| Si in SiC | 500 | 45 267 | 60 000 | 0 | 578.5322 |
| Si in SiC | 600 | 45 687 | 60 000 | 0 | 666.4107 |
| Si in SiC | 700 | 46 025 | 60 000 | 0 | 757.4464 |
| Si in SiC | 800 | 46 296 | 60 000 | 0 | 850.5713 |
| Si in SiC | 1000 | 46 763 | **59 999** | 0 | 1040.752 |
| Si in SiC | 1200 | 47 140 | 60 000 | 0 | 1234.096 |

**The one non-identical value** (Si in SiC, 1000 K, β = 197.6285 — the last
β — α = 16.42225): this port writes `1.82254e-75`, the tape `0`. It is below
the 1e-30 floor NJOY's own THERMR reads `S` against, so it changes no cross
section. **Not root-caused.** NJOY2016's source is not present in this
checkout (`upstream_source/NJOY2016` holds only its README), and the tape was
made with NJOY99 (update 396, ifort, per `tsl-SiinSiC.readme`), so reading
NJOY2016 would not settle it anyway. The leading hypothesis, untested, is an
underflow/flush-to-zero difference in that build at `e^-172` scale.

The `SabRequest` path (`thermal_scattering_tape`, i.e. `generate_tape`) gives
the same MT=4 at each material's base temperature: identical in all 60 000,
`B` and `T_eff` identical.

### MT=2, reactor graphite — the published section, reproduced

**Whole deck (`run_deck`, all ten temperatures in one run): bit-identical**
at every temperature, 169 edges (10P) and 160 (30P), every edge energy and
every `S`.

**Single-temperature run (the HTR-10 path):**

| material | T (K) | edges ours / tape | compared | identical | worst rel. | worst, 7th-fig. units | tail units | `S(E→∞)` ours / tape | spliced vs tape |
|---|---|---|---|---|---|---|---|---|---|
| 10P | 296 | 169 / 169 | 169 | 169 | 0 | 0 | – | 0.1790434 / 0.1790434 | – |
| 10P | 400 | 146 / 169 | 145 | 144 | 1.05e-7 | 1 | 0 | 0.1334277 / 0.1334277 | identical |
| 10P | 500 | 133 / 169 | 132 | 127 | 9.41e-7 | 1 | 0 | 0.1062921 / 0.1062921 | identical |
| 10P | 600 | 124 / 169 | 123 | 110 | 1.50e-7 | 1 | 1 | 0.08780605 / 0.08780605 | identical |
| 10P | 700 | 114 / 169 | 113 | 103 | 1.98e-7 | 1 | 1 | 0.07444894 / 0.07444894 | identical |
| 10P | 800 | 109 / 169 | 108 | 99 | 2.21e-7 | 1 | 1 | 0.06437161 / 0.06437161 | identical |
| 10P | 1000 | 99 / 169 | 98 | 93 | 2.02e-7 | 1 | 1 | 0.05022636 / 0.05022635 | identical |
| 10P | 1200 | 95 / 169 | 94 | 87 | 2.64e-7 | 1 | 0 | 0.04082125 / 0.04082125 | identical |
| 10P | 1600 | 78 / 169 | 77 | 69 | 3.85e-7 | 1 | 0 | 0.02921709 / 0.02921709 | identical |
| 10P | 2000 | 70 / 169 | 69 | 63 | 5.48e-7 | 1 | 0 | 0.02245243 / 0.02245243 | identical |
| 30P | 296 | 160 / 160 | 160 | 160 | 0 | 0 | – | 0.1654315 / 0.1654315 | – |
| 30P | 400 | 139 / 160 | 138 | 133 | 1.64e-7 | 1 | 0 | 0.1231915 / 0.1231915 | identical |
| 30P | 500 | 128 / 160 | 127 | 112 | 4.77e-7 | 1 | 1 | 0.0980314 / 0.0980314 | identical |
| 30P | 600 | 118 / 160 | 117 | 106 | 2.04e-7 | 1 | 1 | 0.0808915 / 0.0808915 | identical |
| 30P | 700 | 109 / 160 | 108 | 97 | 1.53e-7 | 1 | 1 | 0.06851255 / 0.06851254 | identical |
| 30P | 800 | 109 / 160 | 108 | 91 | 1.99e-7 | 1 | 0 | 0.05917888 / 0.05917888 | identical |
| 30P | 1000 | 95 / 160 | 94 | 87 | 2.59e-7 | 1 | 1 | 0.04609177 / 0.04609177 | identical |
| 30P | 1200 | 86 / 160 | 85 | 69 | 5.89e-7 | 1 | 1 | 0.03740685 / 0.03740685 | identical |
| 30P | 1600 | 81 / 160 | 80 | 79 | 3.74e-7 | 1 | 0 | 0.02672768 / 0.02672768 | identical |
| 30P | 2000 | 70 / 160 | 69 | 66 | 7.01e-7 | 1 | 0 | 0.02053046 / 0.02053046 | identical |

`S` here is eV·b per atom (cumulative), so σ_coh(E) = S(E)/E.

**Why a single-temperature run is not the tape's row, and why that is
upstream's behaviour, not a port defect.** Two rules in NJOY's `endout`
(ported in `leapr::endout::build_coherent_elastic`, which the whole-deck run
shows reproduces the evaluator's output bit for bit):

1. The 1/E tail of the Bragg list is thinned once, with the **first**
   temperature's Debye-Waller coefficient (`leapr.f90` ~3204–3216): an edge
   is kept while it adds more than 0.9e-7 of the running sum. A run whose
   first temperature is T keeps the edges significant at T (fewer at higher
   T, since `exp(-4 W' E)` damps the tail); the published tape keeps those
   significant at 296 K. The dropped edges' contributions are folded into the
   last slot, so `S(E→∞)` agrees.
2. The first temperature's `S` is accumulated with the unrounded edge energy;
   every later temperature's with `sigfig(E, 7)` (`leapr.f90` ~3256–3286).
   That moves the 7th figure of some values — the "1 unit" column.

The **cause check** pins both: splicing the 296 K run's law with the T run's
into one `endout` call — exactly what a two-temperature deck does —
reproduces the tape's row at T **bit for bit, all edges**, at all 18
non-base temperatures. So the single-temperature result is NJOY's output for a
one-temperature deck, and the differences from the multi-temperature tape are
print-precision artefacts of the deck shape, not physics: every `S` agrees to
within one unit in its 7th figure.

**What this means for HTR-10:** a law generated at a zone temperature has
the Bragg edges significant at *that* temperature, which is what NJOY2016
itself would write for a one-temperature deck. Neither choice is more
correct; both agree to print precision.

### MT=2, SiC — the generation path decides the law

| path | elastic law written | vs published (LTHR = 1, coherent) |
|---|---|---|
| `run_deck_at_temperature` (demo path, `from_embedded_leapr_deck`) | **incoherent**, LTHR = 2 | a different law |
| `generate_tape` / `SabRequest` (`from_leapr`) | coherent, LTHR = 1, from the 3C-SiC catalogue | −3 % at 0.0253 eV, root-caused (`docs/leapr-sic-coherent-elastic-vv.md`) |

The SiC decks set `iel = 0` and have `twt = 0`, and `run_deck` follows
`leapr.f90:3043`: a solid with no translational term gets `iel = -1`,
incoherent elastic with `sb·npr`. That is what NJOY2016 itself would write
from these decks (stated from the port's citation; NJOY2016's source is not
in this checkout to re-read). The published MT=2 did not come from LEAPR at
all (`tsl-SiinSiC.readme`: "the provided leapr input may reproduce MT = 4
only"). **So the demo path's SiC elastic channel is neither the published
law nor this crate's catalogue law.** Gated as a recorded difference
(`gate_sic` asserts `LTHR` (2, 1)), so a change to it cannot go unnoticed.
Not changed here: which elastic law the HTR-10 model should use for SiC is a
modelling decision for the maintainer (see the hand-off on #1140).

## Generation at the HTR-10 slider range

`every_htr10_law_generates_across_the_slider_range` (no reference data, not
gated): all four laws generate at 293.15 K, 300.15 K and **2073.15 K
(1800 °C)** through the demo path; at each, every stored `S` is finite and
non-negative, `T_eff ≥ T`, and the tape is tabulated at the request.

| material | T (K) | wall time (s) | `T_eff` (K) | positive `S` / 60 000 | elastic |
|---|---|---|---|---|---|
| 10P | 293.15 | 9.340 | 689.41 | 56 584 | coherent |
| 10P | 300.15 | 9.308 | 692.20 | 56 585 | coherent |
| 10P | 2073.15 | 9.282 | 2160.26 | 56 969 | coherent |
| 30P | 293.15 | 9.468 | 572.43 | 55 986 | coherent |
| 30P | 300.15 | 9.538 | 575.88 | 55 988 | coherent |
| 30P | 2073.15 | 9.563 | 2128.43 | 56 476 | coherent |
| C in SiC | 293.15 | 0.406 | 550.41 | 49 994 | incoherent |
| C in SiC | 300.15 | 0.399 | 553.33 | 50 000 | incoherent |
| C in SiC | 2073.15 | 0.399 | 2117.34 | 51 637 | incoherent |
| Si in SiC | 293.15 | 0.404 | 416.76 | 46 608 | incoherent |
| Si in SiC | 300.15 | 0.401 | 421.49 | 46 617 | incoherent |
| Si in SiC | 2073.15 | 0.414 | 2093.01 | 48 787 | incoherent |

**Timing protocol and hardware.** Wall time of deck parse +
`run_deck_at_temperature` + `write_text`, one generation each, single-threaded
code, measured by running that test alone (`cargo test --release … every_htr10`;
nothing else of ours running, load average ~3 from the preceding build).
Machine: cloud container, 4 vCPU "Intel(R) Xeon(R) Processor" (model not
exposed), 1 thread per core, 8 MiB L2 per core, 15 GiB RAM, Linux 6.18, CPU only,
rustc 1.99.1, release profile with overflow checks. Cold cache is not
distinguished: there is no disk cache on this path.

**The porous grades cost ~8.5× crystalline graphite on the same machine**
(crystalline graphite measured the same way: 1.131 / 1.107 / 1.095 s at
293.15 / 300.15 / 2073.15 K). The deck is the reason, not the temperature:
the 10P/30P decks ask for `nphon = 200` phonon-expansion terms where
crystalline graphite asks for 100 (card 3: `10 2 200` against `10 2 100`).
Timing is flat in temperature for all four. This machine runs crystalline
graphite in ~1.1 s against the 1.83 s `leapr/generate.rs` quotes for a
12-core workstation (2026-08-13), so timings do not transfer between the two
without re-measuring. Either way a porous-graphite law costs seconds per
temperature natively, and more on a phone, so the HTR-10 demo must generate it
off the UI thread (the mobile "no lagging" rule).

## Interpretation

- **MT=4 of all four laws is reproduced exactly** at every temperature the
  evaluation tabulates, through the very path HTR-10 will use. Combined with
  `T_eff` being exact, the inelastic channel of these laws can be trusted at
  the tapes' temperatures; at other temperatures the result is LEAPR's
  (same spectrum, new `tev`), which is also what the evaluators did for their
  own nine higher temperatures.
- **Graphite MT=2** is reproduced exactly by the whole-deck run, and to print
  precision (one unit in the 7th figure) by the single-temperature run, for
  reasons fully accounted for by NJOY's `endout`.
- **SiC MT=2 is the open item** for HTR-10, and it is a modelling choice, not
  a port defect.

## Not verified

- Anything at temperatures the tapes do not tabulate (e.g. 2073.15 K): there
  is no reference; only sanity (finite, non-negative `S`, `T_eff ≥ T`) is
  checked. Above 2000 K the deck's 296 K phonon spectrum is reused, an
  approximation LEAPR and the evaluation share (no thermal expansion or
  anharmonicity).
- The single 1.8e-75 vs 0 value (cause not established).
- That NJOY2016 run on these decks gives what the published (NJOY99) tapes
  hold — no NJOY2016 run was made; the comparison is against the published
  tapes only.
- The SiC incoherent-elastic numbers themselves (`sb·npr`, `W'(T)`): nothing
  to compare them to.
- Wasm: the test runs natively; the demo path's wasm build was not run here.

## Reproduce

```bash
ulimit -v 12582912
cargo test --release -p njoy-outram-park-fork --test leapr_htr10_tsl_vs_endf8 -- --nocapture
```

Whole file: 7 tests, 196 s on the machine above (4 tests in parallel).
