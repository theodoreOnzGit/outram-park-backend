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
