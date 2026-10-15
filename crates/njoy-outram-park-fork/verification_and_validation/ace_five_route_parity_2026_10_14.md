# The five-route ACE library against NJOY2016, word for word (GitHub #872)

**Research, education and V&V only** (`RESPONSIBLE_USE.md`).

## Question

Are the continuous-energy ACE tables this crate builds for the five-route
ICSBEP campaign the tables NJOY2016 builds, in every block, including photon
production and charged-particle production? #872 found that they were not:
H-1 had no photon production (NTRP 0 against 1), O-16 none either (0 against
131), and O-16, Al-27 and others had no charged-particle production (NTYPE 0
against 4).

## Methodology

- **Nuclides:** the campaign's 12
  (`crates/outram-mc-libs/verification_and_validation/icsbep/five_route_keff/nuclides.tsv`):
  U-234, U-235, U-238, F-19, O-16, H-1, Al-27, Si-28, Si-29, Si-30, Mn-55,
  B-10, all ENDF/B-VIII.0 from `reference-data/endf/`.
- **Reference:** NJOY2016 2016.79 (commit `ac5adf5`, built from
  `upstream_source/NJOY2016` in `build2/`), the campaign's own decks:
  - 293.6 K: `RECONR (.001) -> BROADR (293.6 K, .001) -> PURR (20 bins, 64
    ladders) -> ACER (iopt 1, newfor 1, iopp 1)`;
  - 0 K: `RECONR (.001) -> ACER`;
  - no HEATR (the decks run none, so the ESZ heating column is zero on both
    sides);
  - made by `make_njoy_library.sh`; the uranium 293.6 K tables and U-235
    0 K come from the `reference-data/ace` submodule, built by the same deck.
  - B-10: NJOY2016's Type-1 formatter stops in `change` (`acefc.f90:13942`,
    "Undefined law for dlwh block: 0") on B-10's law-0 DLWH entry, so the
    script reruns it as Type 2.
- **Ours:** `process::cross_sections` on the same tape at the same
  temperature with `without_heatr()` (and, at 0 K, `without_purr()`), PURR
  at 20 / 64 / 10 000, written as Type 1 and read back
  (`examples/ace_word_parity.rs`).
- **Comparison:** `acer::compare`: every judged header field, every NXS and
  JXS entry, every XSS word, block by block and as a whole, with **no
  tolerance** (Type-1 text is compared as printed). A Type-2 reference is
  printed through Type 1 first, integer words with our table's mask and
  reals to 12 figures, which is what NJOY2016's own Type-1 writer would
  print.
- **Pass criterion:** `AceComparison::identical()` for every nuclide and
  temperature.
- **Instrument:** `verification_and_validation/ace_five_route_parity/run_survey.sh`
  (builds the NJOY2016 library, then one line per table). Standing gate:
  `tests/ace_five_route_parity.rs` on the five nuclides that exercise every
  fix, against tables committed in `reference-data/acer/five_route/`.

## What was wrong, and the fixes

Each difference was traced to the upstream routine that owns the behaviour
and fixed by porting it. In the order they were found:

| # | symptom | cause (upstream) | fix |
|---|---|---|---|
| 1 | H-1: no photon blocks | the photon pass read MF=6 photons through the LAW=1 neutron parser, which refuses LAW=2, and skipped the whole MT. `convr` moves every `ZAP=0, LAW!=0` product and writes a LAW=2 photon as one discrete line at `egamma + awr*E/(awr+1)` (`acefc.f90:4387-4480`) | walk every photon product; LAW=2 rewritten as `convr` writes it, through `a11` text |
| 2 | H-1: JXS(10/11) 0 against a locator | `ldlw = next`, `dlw = next` even when NR = 0 (`:5902-5904`) | same |
| 3 | GPD missing on F-19, Al-27, Si, O-16, B-10 | `gpd` refused an MF=12 LO=1 section with NK > 1; `convr` rebuilds that total from its partials on its own grid (`:4044-4105`) | `convr_corrected_total` |
| 4 | F-19, Si: SIGP 4 words short per MF=6 photon | `acelpp` writes a yield's interpolation regions unless there is one lin-lin region (`:8312-8322`) | `SigP::Yield::interp` |
| 5 | Al-27, Si: discrete levels isotropic | levels given only as MF=6 LAW=2 neutrons; upstream reads their angles as a regular distribution (`:5838-5876`, `acensd`'s `mfh = 6` branch) | `acensd::parse_mf6_law2_for_acer` |
| 6 | NXS(7) = 0 on H-1, O-16, Al-27, Si, Mn-55, B-10 | `acelcp` refused the capture recoil, MF=4 LTT != 0 and MF=6 LAW != 1 (`:9370-10930`) | those branches ported; MF=6 LAW = 5/6/7 and the elastic recoil still refuse by name |
| 7 | O-16: no photon blocks | anisotropic MF=14 (LI = 0) refused the whole build; `convr` converts it with `ptleg`/`pttab` to 32 equiprobable bins (`:2593-3281`, `:4617-4657`), `acelpp` writes ANDP (`:8402-8478`) | `acer::photon_angular`, `assign_mf14`, the ANDP writer |
| 8 | F-19, Si-29 lost their photons after 7 | with any LO=2 cascade, `convr` replaces every discrete-level MF=14 by an isotropic one with one subsection per cascade photon (`:4035`, `:4357-4385`) | `l2flg` / `level_mt` in `assign_mf14` |
| 9 | O-16: 102 extra MF=13 words per entry | `gamsum` evaluates every grid energy and `acelpp` keeps from the last leading zero to one past the last non-zero value (`:3700-3760`, `:8336-8371`) | `xs_window` |
| 10 | O-16: words in the 7th figure (ANDP, SIGP) | `convr`'s MF=14 and `gamsum`'s MF=13 go through text scratch tapes; `a11` keeps 6 figures for a negative number or a two-digit exponent | the `a11` round trip on both |
| 11 | B-10: 3.458281 MeV against 3.45828 | `unionx` puts each photon-file discontinuity `d` into the grid as `sigfig(d,7,-1)`, `sigfig(d,7,+1)` (`:1180-1250`) | `gamma_discontinuities`, `unionx_discontinuities` |

Two upstream behaviours are reproduced as written rather than corrected:
`acelcp`'s capture-recoil law 33 reads `awp` without setting it (0 for H-1,
confirmed by the H-1 table matching), and its `int = 1` mean-cosine sum adds
`(mu_i + mu_{i-1})/2` where a mean cosine would multiply.

## Results (2026-10-14)

Hardware: Intel Xeon @ 2.10 GHz, 4 logical cores, 15.7 GiB RAM, Linux, CPU
only, in a cloud container; one table at a time, all threads available to
it. "Ours" is the time to build our table.

| nuclide | 293.6 K | ours | 0 K | ours |
|---|---|---|---|---|
| U-234 | identical | 19.4 s | identical | 0.9 s |
| U-235 | identical | 112.6 s | identical | 30.2 s |
| U-238 | identical | 92.0 s | identical | 36.2 s |
| F-19 | identical | 0.3 s | identical | 0.2 s |
| O-16 | identical | 0.9 s | identical | 0.9 s |
| H-1 | identical | 0.1 s | identical | 0.0 s |
| Al-27 | identical | 0.4 s | identical | 0.3 s |
| Si-28 | identical | 0.6 s | identical | 0.4 s |
| Si-29 | identical | 0.5 s | identical | 0.3 s |
| Si-30 | identical | 0.4 s | identical | 0.3 s |
| Mn-55 | identical | 10.3 s | identical | 1.3 s |
| B-10 | identical (Type 2 at Type-1 print precision) | — | identical (Type 2 at Type-1 print precision) | — |

An earlier full run the same day gave the same 24 verdicts with times
within 10 % (U-235 293.6 K 123.0 s, U-238 0 K 29.8 s). B-10's time was not
recorded by the survey script's Type-2 branch.

**24 of 24 tables identical in every word.** The standing gate
(`tests/ace_five_route_parity.rs`, H-1, F-19, Al-27, B-10, O-16 at 293.6 K)
passes in 1.7 s, photon-block decode check included; U-234 at 293.6 K stays gated by `tests/process_api.rs` and
`tests/ace_compare.rs`.

## Interpretation and scope

- None of these blocks is read by a neutron-only eigenvalue calculation, so
  the five-route `k` values (routes 2 and 5) do not move. They matter for
  photon transport, heating and gas production.
- The comparison is with **no HEATR on either side**, because the
  campaign's NJOY2016 deck runs none. With HEATR, ours writes the heating
  column from the ported HEATR, which is byte-identical to NJOY2016's on 62
  tapes (`verification_and_validation/heatr_vs_njoy2016.md` §6); a
  HEATR-on table comparison against NJOY2016 is not part of this record.
- **Not covered:** MF=6 LAW = 5/6/7 charged particles and the elastic
  recoil (no five-route nuclide reaches them; they still refuse the whole
  block by name), and `unionx`'s second discontinuity pair (`edl`/`edh`,
  for a discontinuity in MT=1 itself).

## LCT-008's full nuclide list (2026-10-15, GitHub #875)

### Question

LCT-008's full model (gh:#874) put 25 more ENDF/B-VIII.0 nuclides through
the same comparison: 37 nuclides, 74 tables. On 2026-10-14, 24 of them
differed from NJOY2016 (Na-23, Ti-48, Cr-50/52/53/54, Fe-57/58, Cu-65,
Zn-64/66/67/68/70). Can every difference be traced to something upstream
does and the port did not, and removed?

### Methodology

As above: the same decks, the same NJOY2016 2016.79 tables
(`target/five_route_keff/njoy/{293.6K,0K}/`, made by
`make_njoy_library.sh` on 2026-10-14), `examples/ace_word_parity.rs`, no
tolerance. Where a difference was one unit in the 7th figure, the cause was
found by **instrumenting a copy of NJOY2016** (`gamsum`, `emerge`, `rdfil2`,
`resxs`, `lunion`) to print its intermediates at full precision at the
differing energies, and printing ours at the same points. Each run of the
instrumented build reproduced its reference table byte for byte apart from
the comment line.

### What was wrong, and the fixes

| # | tables | symptom | cause (upstream) | fix |
|---|---|---|---|---|
| 12 | Cu-65, Fe-57 | GPD one unit off in the 7th figure at a few energies | `convr` writes an MF=12 LO=2 cascade's converted yield to its scratch tape as text (`tab1io`, `acefc.f90:4318-4346`), so `gamsum` sums `a11`'s rounding: Cu-65's MT=52 yield is 1.00006072 there, not 1.0000607248. NJOY's sum at 1.5 MeV is 0.6004242483, ours was 0.6004242500, either side of a rounding edge | the LO=2 record goes through the `a11` round trip |
| 13 | Na-23, Cr-50/52/53/54 | DLWP 2 or 6 words short | `acelpp` copies an MF=15 continuum's probability record into the law header (NR, regions, NE, E, P), writing NR = 0 only for one lin-lin region (`:8540-8566`). Cr's records are histograms; Na-23's has two regions over three points. The port wrote the fixed two-point form | `PhotonApplicability` on `PhotonEntry`, written by the DLWP serialiser |
| 14 | Ti-48 | ESZ total and elastic one unit off at 4 energies near 6.7 keV | RECONR's `facphi` sets an l >= 2 phase shift to 0 when `phi/rho < 1e-6` (`reconr.f90:4441-4471`); the port kept `phi ~ 5e-8` for Ti-48's d-wave, which moved the Reich-Moore elastic by 3.4e-8 relative (17.40361465 against NJOY's 17.40361523). Also `facts` adds `3*r2+r4+9` in that order | `slbw::facphi` for RECONR's SLBW, MLBW and RM; `unresr` keeps the unguarded form, as `unfac` |
| 15 | Zn-64, Zn-67 | one extra energy, 0.1299999 MeV (0.06999999 MeV) | `resxs` walks the union grid's own nodes and ends on the last below `eresh` (`:2513-2521`). `rdfil2` adds `sigfig(eh,7,-1)` as a node, but MT=107's histogram step at `eh` makes `lunion` write `sigfig(sigfig(eh,7,-1),7,-1)` in its place (`:2048-2060`), so NJOY never has the node. The port added it to the resonance grid unconditionally | the resonance pass takes the range end from the union grid only |
| 16 | Zn-64/66/67/68/70 | URR bands differ at 0.35 MeV, a node PURR inserts | PURR's `intr2` interpolates Case C parameters under the J-state's own INT (`purr.f90:1539-1570`); the port reused UNRESR's `intr`, which `unresl` calls with `int = 2`. Zn's INT is 5 | `unresr::range_sequences_purr`, `interp_case_c` taking the law |
| 17 | Fe-58 | MT=4 missing from the reaction list; UNR 248 words against 1 579 | PURR's own `rdf2un` builds the grid with `etop = 5 MeV` as primer and refinement limit (`purr.f90:709-718`, `:997`); the port had UNRESR's `rdunf2` with 1 MeV, so Fe-58's 0.35-3 MeV table stopped at 1 MeV on 2 energies. With the grid's top at 3 MeV, PURR finds more than one competing level and sets the inelastic flag to 4, and ACER keeps MT=4 for it (`acefc.f90:1164-1179`, `:1728`) | PURR's grid ported from `rdf2un`: `etop`, `et+et/100`, the evaluation's nodes unrounded and inside `[el, eh]`, no `sigfig(.,7,2)` pass |

Removing UNRESR's `sigfig(.,7,2)` pass in fix 17 exposed one more defect it
had been hiding: the evaluation's Case B/C nodes were inserted through
`sigfig(e,7,0)`, whose 1e-13 bias made U-235's 3.5 and 20 keV miss
`egridu`'s exact values in `ilist2`'s exact-duplicate test (21 energies
against 19). Upstream inserts the raw value.

Four of the six fixes (14 to 17) are upstream of ACER, in RECONR and
PURR. They change the ENDF route (route 4 of the five-route campaign, and the
browser demo) on the nuclides concerned as well as the ACE route.

### Results (2026-10-15)

**All 76 tables identical in every word**: the 37 nuclides of
`nuclides.tsv` plus U-236 (added to HEU-SOL-THERM-009's model the same day),
each at 293.6 K and 0 K. That includes U-234, U-235, U-238 and the twelve
nuclides of 2026-10-14, which the PURR grid change could have moved and did
not. U-236's 293.6 K reference is the `reference-data/ace` submodule's
`reference-njoy/endf-b-viii.0/293.6K/U236.ace.gz`; its 0 K reference was
made here with `make_ace_0k.sh` and the same NJOY2016 build. Hardware:
Intel Xeon, 4 logical cores, 15.7 GiB, Linux, CPU only, cloud container,
shared with a test build during the sweep, so no timings are quoted.

Standing gates: `tests/ace_five_route_parity.rs` (now 11 tables: the six of
2026-10-14 plus Cu-65, Na-23, Ti-48 and Zn-64 at 0 K and Zn-70 at 293.6 K)
and `purr::tables::urr_grid_tests` (Fe-58's 13 and U-235's 19 UNR energies
against NJOY2016's).

**What this does not cover.** The five-route `k` values of 2026-10-14 for
the full LCT-008 model were taken before these fixes. Routes 2 and 5 read the
old Rust ACE, and routes 4 and 5 ran the old RECONR and PURR for Ti-48, Zn
and Fe-58. Those rows have not been re-measured.

## Unrounded: every table against NJOY2016's Type 2 (2026-10-15)

### Question

"Identical in every word" above is at Type-1 print precision: 12
significant figures, and integer words through `nint`. B-10 was the one
table compared against a Type-2 (binary) reference, and even there it was
printed through Type 1 first, with our integer mask. Do our tables hold the
**same doubles** NJOY2016 holds, bit for bit?

### Methodology

- **Reference:** NJOY2016 2016.79 (`ac5adf5`, `build2`), the five-route
  decks of `make_ace.sh` / `make_ace_0k.sh` with ACER card 2's `itype` set
  to **2**, run here, one table at a time, for all 38 nuclides of
  `nuclides.tsv` (the 36 tapes of LCT-008's full model,
  `lct008_model::TAPES`, plus HST-009's F-19 and U-236) at
  293.6 K and 0 K: 76 tables.
- **Ours:** as above (`process::ace`, no HEATR), compared **in memory**,
  before any formatting: NXS, JXS, and every XSS word by `f64::to_bits`.
  `examples/ace_word_parity.rs` prints this as `EXACT` lines, naming each
  differing word's JXS block.
- **Pass:** zero differing bits in every table.
- **Thermal (H in H₂O, the LEAPR route):** NJOY2016 cannot write it
  unrounded. `aceth.f90:541` sets `itype = 1` in the thermal writer whatever
  card 2 says (checked: card 2 `2 0 2` still gives a text file), and a
  Type-2 thermal table only comes from `iopt = 8` re-reading the text. So
  the comparison is the whole Type-1 file: `write_ace_library
  --thermal-leapr HinH2O:HinH2O:n-001_H_001-ENDF8.0-Beta6.endf:lwtr` (the
  LCT-008 library's own call) against OpenMC's `make_ace_thermal(...,
  iwt=1)` on NJOY2016 (route 1's table), byte for byte from line 3.

### Results

**In progress (2026-10-15).** Filled in when the 76-table run completes.
