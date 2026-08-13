# LEAPR card decks — provenance, licence finding, and V&V

Data-provenance record for the LEAPR card decks the crate's
`leapr::generate` path regenerates thermal-scattering `S(alpha, beta)` from,
per the workspace `DATA_POLICY.md` "Data Provenance" rule.

**Status as of 2026-08-13: no deck is embedded in this crate.** The reason is
the licence finding in §2. Everything else — the generator, the vintage-selected
physical constants, the cache — is built and working against a *local* copy of
the decks, which the user supplies.

---

## 1. The data in question

| Field | Value |
|---|---|
| Dataset | ENDF/B-VIII.0 Thermal Neutron Scattering Sublibrary, `thermal_scatt/` |
| Files of interest | `tsl-crystalline-graphite.leapr` (12,444 B), `tsl-reactor-graphite-10P.leapr` (14,146 B), `tsl-reactor-graphite-30P.leapr` (13,838 B) |
| What they are | NJOY2016 LEAPR input card decks — the jobs that generated the corresponding `tsl-*.endf` MF=7 tapes. Each carries a grid specification, a tabulated phonon frequency spectrum `rho(E)`, a temperature list, and a block of descriptive comment cards. |
| Companion tapes (**not** used by the default path) | `tsl-crystalline-graphite.endf` (8,730,804 B), `-10P` (8,724,040 B), `-30P` (8,722,444 B) |
| Sublibrary author / distributor | D. A. Brown, National Nuclear Data Center (NNDC), Brookhaven National Laboratory. Sublibrary `README.txt` dated 2 Feb 2018. |
| Evaluator of the graphite files | Low Energy Interaction Physics (LEIP) group, North Carolina State University — A. I. Hawari, Y. Zhu, J. L. Wormald. The decks' own card-20 comments say `EVAL-SEP17`, and the per-material `.readme` is signed "LEIP Laboratories (Oct 17 2017)". |
| Method | Ab-initio lattice dynamics (AILD); coherent-elastic cross sections from the cubic approximation in LEAPR/NJOY, using the AILD lattice constant. |
| Generating code | NJOY2016.20, compiled with `ifort` from the intel-2017 composer suite (per the `.readme`). |
| Publication (sublibrary) | D. Brown et al., "ENDF/B-VIII.0: The 8th Major Release of the Nuclear Reaction Data Library with CIELO-project Cross Sections, New Standards and Thermal Scattering Data", *Nuclear Data Sheets* **148**, 1 (2018). |
| Publication (graphite method) | 1. A. I. Hawari, "Modern Techniques in Inelastic Thermal Neutron Scattering Analysis", *Nuclear Data Sheets* **118** (2014) 172. 2. J. L. Wormald, A. I. Hawari, "Thermal neutron scattering law calculations using ab initio molecular dynamics", *EPJ Web of Conferences* **146**, 13002 (2017). (Both cited in the decks' own comment cards.) |
| Local copy consulted | `/home/teddy0/Documents/research/ENDF-B-VIII.0/thermal_scatt/`, file mtimes 9 Jan 2018 (decks) / 17 Jan 2018 (tapes). **Read-only; never modified.** |
| Date accessed | 2026-08-13 |
| Distribution points | NNDC (`https://www.nndc.bnl.gov/endf-b8.0/`), IAEA NDS (`https://www-nds.iaea.org/public/download-endf/ENDF-B-VIII.0/tsl/`) |
| Processing applied by this crate | **None to the deck.** It is read verbatim and parsed. The *derived* artifact (a regenerated MF=7 tape) is recorded by its own `.recipe` sidecar; see §4. |

---

## 2. Licence finding — terms could not be established, so nothing is embedded

**What was checked, on 2026-08-13:**

| Source | Result |
|---|---|
| `thermal_scatt/README.txt` (the sublibrary README, D. A. Brown, NNDC) | No copyright, licence, terms-of-use, or redistribution statement. |
| `thermal_scatt/CHANGELOG.txt` | No such statement. |
| `tsl-crystalline-graphite.readme`, `-10P`, `-30P` | No such statement. Attribution and method only. |
| The `.leapr` decks themselves (all 34 comment cards read) | No such statement. |
| `https://www.nndc.bnl.gov/endf-b8.0/download.html` | No licence, copyright, terms, citation requirement, or redistribution statement on the page. |
| `https://www.nndc.bnl.gov/endf/` | Same — none found. |
| `https://www.nndc.bnl.gov/endf-releases/` | Same — none found. |
| `https://www-nds.iaea.org/` and `https://www-nds.iaea.org/public/download-endf/` | Not retrievable from this environment (HTTP 402 from the fetch proxy). **Not checked**, rather than checked and found empty. |
| Web search for an ENDF/B public-domain declaration or CSEWG distribution policy | Returned only "publicly available" phrasing in papers and release notes. No licence grant located. |

**Conclusion: the redistribution terms of these files are unestablished.**

That is not the same as "restricted", and it is not the same as "free". What can
be said honestly is: the files carry no copyright notice, are distributed
publicly without registration or a click-through, and are the product of US
government-funded work — none of which is a grant of redistribution rights.

The workspace `CLAUDE.md` rule is directional and applies exactly here:

> Do not assume "public data" means "redistributable" — the workspace has a
> standing rule that public hosting grants no redistribution rights. […] Unsure
> means **proprietary**; that failure direction is recoverable and the other is
> a licence violation in a public repository.

So the decks are **not** embedded in this GPL-3.0 crate, which is published to
crates.io. The cost of that choice is that a user must supply a local copy; the
cost of the other choice would be a possible licence violation in a public
repository. The first is a one-line change to undo, the second is not.

### How to close this out

Someone with the standing to do so needs to establish terms — the practical
routes are (a) a written statement from NNDC/CSEWG, (b) an explicit terms-of-use
page on the IAEA NDS site (unreachable from here, so genuinely unchecked), or
(c) a determination that the *deck* specifically is uncopyrightable fact rather
than expression. Route (c) is a legal judgement, not an engineering one, and the
decks' comment cards contain clearly expressive prose (a "Background" section
and a reference list), so it is not obviously available.

**When terms are established:** drop the deck files into
`src/leapr/decks/`, change the relevant arm of
`leapr::decks::embedded_deck_text` to
`Some(include_str!("decks/tsl-crystalline-graphite.leapr"))`, update the
tripwire test `no_deck_is_embedded_while_the_licence_question_is_open`, and
record the established terms in this file. Nothing else changes —
`locate_deck` already prefers the embedded copy over any on-disk one.

### Why this is not in `crates/kovan-literature`

The workspace rule is that ingested or *used* literature goes into KOVAN,
catalogued. Judgement here: a `.leapr` deck is a **numeric input file for a code
in this crate**, not a document — it has no bibliographic identity, no DOI, and
`kovan lit import`/`bibtex`/`outline` have nothing to work on. Its natural home
is this crate's own provenance record, which is what `DATA_POLICY.md`'s
"recommended file: `References.md`" is asking for. The KOVAN rule *does* bite on
the two papers cited above (Hawari 2014; Wormald & Hawari 2017) — they are the
literature this code now depends on, they are not in the archive, and
**cataloguing them is an outstanding follow-up**, not something done here.
Note that EPJ Web of Conferences is open access, so the 2017 paper is a
straightforward `open/` candidate.

---

## 3. What the crate does instead

`leapr::decks::locate_deck` resolves a deck from, in order:

1. `leapr::decks::embedded_deck_text` — **empty today**, per §2.
2. `$OUTRAM_PARK_TSL_DIR` (or the legacy `$GRAPHITE_TSL_DIR`), pointing at an
   unpacked distribution's `thermal_scatt/` directory.
3. The crate's artifact cache, `<cache>/ENDF-B-VIII.0/<base>.leapr`.

When none is found the error names every path tried and the variable to set.

---

## 4. Verification & validation

### 4.1 Methodology

The generation path is `LeaprDeck::parse` -> `FrequencyModel::start` ->
`phonon_expansion` -> `coher` -> `endout` -> ENDF tape, driven by
`leapr::generate::generate_tape`, with the physical constants taken from the
deck's own `EVAL-SEP17` field (see `leapr::vintage`). The reference is the
official `tsl-crystalline-graphite.endf` (MAT 30) that this very deck produced.

Two checks, at different levels:

- **Coherent-elastic parity** — `tests/leapr_graphite_coherent_elastic_parity.rs`
  compares the generated MF=7/MT=2 Bragg grid and `S(E, T)` against the tape at
  all ten tabulated temperatures. Pass criterion: identical retained-edge count,
  max relative deviation < 2e-6, RMS < 1e-6.
- **Raw-kernel parity** — `tests/leapr_graphite_deck_parity.rs` compares the
  unrounded `f64` output of the kernels against the tape's stored values. Pass
  criterion: max relative deviation < 1e-5, RMS < 2e-6, identical zero pattern.
- **End-to-end tape parity** — `examples/graphite_sab_generation.rs` compares
  the *stored* values of the generated ENDF tape against the official tape's, at
  296 K, over the full 150 x 400 grid. Pass criterion: nothing looser than the
  raw-kernel figure; bit-identity would be the strongest possible outcome.

Both are run against the local ENDF/B-VIII.0 copy identified in §1, in release
mode, on 2026-08-13.

### 4.2 Results (2026-08-13, release, 12-core workstation)

**Raw-kernel parity, MF=7/MT=4** (from `tests/leapr_graphite_deck_parity.rs`):

| Constant set | max rel. dev. | RMS |
|---|---|---|
| `bk = 8.617385e-5` (EVAL-SEP17 era, auto-selected from the deck) | 4.917e-6 | 6.390e-7 |
| `bk = 8.617342e-5` (CODATA2014) | 3.086e-4 | 5.687e-5 |
| `bk = 8.617333262e-5` (CODATA2018, crate default) | 3.711e-4 | 6.842e-5 |

at 296 K over 48,941 points, zero pattern identical (6,645 points, zero
mismatches). 4.838e-6 / 5.817e-7 at 1000 K.

**End-to-end tape parity, MF=7/MT=4 at 296 K:**

| Quantity | Measured |
|---|---|
| Stored `S` values bit-identical to the official tape | **60,000 / 60,000** |
| max relative deviation (points above 1e-30) | **0.000e0** over 48,941 points |
| RMS relative deviation | **0.000e0** |

**Interpretation.** The 4.917e-6 raw-kernel residual is the tape's own storage
round-off, and once `endout` applies the same `sigfig(x, 7, 0)` /
`sigfig(x, 6, 0)` rounding NJOY applies, it disappears: the generated ENDF
section is *the same numbers* as the published one, to the last stored digit.
For the inelastic channel, the 12 KB deck is not an approximation to the 8.7 MB
tape — it reproduces it.

### 4.2b Coherent elastic (MF=7/MT=2), and why the *whole* constant set matters

The elastic channel does not depend on `bk`. Its Bragg edges sit at
`E = tau^2 / econ` with `econ = ev * 8 * (amassn * amu / hbar) / hbar`
(`leapr.f90:2543`), so it depends on `ev`, `amu`, `hbar` and `amassn`. Measured
over 221 retained Bragg edges and 2,200 `S(E, T)` values at all ten tabulated
temperatures:

| Constants used | Bragg edge energies | `S(E, T)` |
|---|---|---|
| crate default (CODATA2018), `bk` alone corrected | max 9.937e-7, RMS 5.512e-7 | max 9.986e-7, RMS 2.408e-7 |
| full `PhysicalConstants::Njoy2016Legacy` set | **max 1.001e-13** | **max 1.001e-13** |

Through `leapr::generate` — i.e. after `endout`'s 7-significant-figure storage
rounding — the 296 K comparison gives **221 / 221 grid points with max relative
deviation 0.000e0** on both the edge energies and `S(E)`.

**Why the 1e-6 residual was attributable to the constants and not to the
hand-transcribed lattice constants:** it is a *uniform multiplicative offset*,
fitted at `+5.115e-7` over 220 edges with only `2.088e-7` scatter about that one
factor. `tau^2` depends only on `a` and `c`, so a transcription slip would move
`(00l)` and `(hk0)` families by different amounts. Only `econ` scales every
family alike. (The 21 hand-transcribed lattice constants across all six lattices
were separately diffed against `leapr.f90:2508-2528` and all match; clearing the
`HUMAN RE-VERIFY` marker at `src/leapr/coher.rs:81` remains a human's call, but
the evidence now exists.)

Two further quantities were pinned for the first time by this comparison: the
**absolute** Debye-Waller scale (`|delta| <= 4.91e-7`, against 0.48 % for the
earlier trend-only method — roughly a 10,000x tightening; `W'(296 K) =
2.860298 /eV`) and the structure-factor normalisation `scoh`/`scon`/`formf`
(`|epsilon| <= 5.12e-7`), which nothing had ever checked against a reference.

**The legacy constants are sourced, not fitted.** They were read out of
NJOY2016's own `src/phys.f90` at the commit preceding `007828d` (2017-10-23,
"Incorporating Skip's changes"):

```text
bk = 8.617385e-5    ev  = 1.60217733e-12    amu = 1.6605402e-24
hbar = 1.05457266e-27                       amassn = 1.008664904
```

That `bk` from that same file is independently the value the *inelastic* channel
needed corroborates the vintage inference rather than merely fitting it: two
channels, two disjoint combinations of constants, one commit.

### 4.2c Timings

Two independent cold-cache runs, same machine and day (12-core workstation
shared with other work):

| Operation | Run A | Run B |
|---|---|---|
| First call, cold cache (generate + write + parse), 296 K | 2.082 s | 1.836 s |
| First call, cold cache, 393 K | 2.721 s | 1.830 s |
| First call, cold cache, 523 K | 2.180 s | 1.832 s |
| Disk-cache hit (read + parse the ~1.7 MB cached tape) | 0.009 s | 0.009 s |
| In-process memo hit | < 0.001 s | < 0.001 s |

A ~200x speed-up from the disk cache and a further order of magnitude from the
memo. Contention can only inflate a timing, so **~1.83 s is an upper bound** on
the uncontended per-temperature cost; the run-A figures are the same work under
heavier load. The cost is flat across 296-523 K, as expected: the work is set by
`nphon` and the spectrum length, not by temperature.

The cached artifact is ~1.69 MB per temperature (a single-temperature MF=7 tape
stores the base temperature as TAB1 `(alpha, S)` pairs, so it is larger per
temperature than the 10-temperature reference tape's 0.87 MB average). It is a
**derived, regenerable** file in the platform cache directory, not something
committed.

**Untabulated temperatures.** 393 K and 523 K — the HTR-10 operating points the
tape does not tabulate — generate through the identical code path and produce
full 150 x 400 grids (221 and 191 retained Bragg points respectively, against
221 at 296 K). Subject to the `rho(E)` caveat in §4.3.

### 4.3 Scope limits — what is *not* validated

- **MF=7/MT=2 coherent elastic *is* validated for crystalline graphite**, as of
  the same day, by `tests/leapr_graphite_coherent_elastic_parity.rs` — see
  §4.2b. That closes what was the dominant gap: MT=2 is roughly 90 % of
  graphite's thermal cross section (4.55 b against 0.49 b inelastic at
  0.0253 eV) while being 0.4 % of the tape's bytes.
- **`rho(E)` is reused as temperature-independent.** The phonon spectrum is a
  deck input, not a computed quantity, so generating at a new temperature reuses
  the spectrum the evaluator calculated at theirs; thermal expansion and
  anharmonicity are not modelled. The shipped tape shares this exactly — nine of
  its ten temperature blocks are "reuse the 296 K spectrum" entries — so this is
  **not a regression against reading the tape**, but it is the approximation
  that limits how far outside the deck's range one should go.
- **One material, one deck shape.** The validated case is crystalline graphite
  with `twt = c = 0`, `nd = 0`. The translational, diffusive,
  discrete-oscillator and cold-hydrogen branches of LEAPR are untouched by it.
  **The 10P/30P reactor grades are registered and parse, but their parity has
  not been measured** — `SabRequest::validation` reports them as unvalidated
  for both channels, and it should keep doing so until someone measures them.
  Sharing a code path with a validated case is evidence about the code, not
  about the material.
- **Incoherent-elastic output (`iel < 0`) is refused, not approximated** —
  `endout` can write the section but the bound cross section `sb` it needs is
  not computed anywhere in this port, so `generate_tape` returns
  `NjoyError::NotPorted` rather than inventing a number.
