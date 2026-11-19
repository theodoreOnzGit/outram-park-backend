# GROUPR — multigroup cross sections and matrices

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** All code in this workspace is **unverified and untrusted** unless a specific verification & validation (V&V) case demonstrates otherwise. V&V cases are human-reviewed and are intended for journal / arXiv publication — that is the trust workflow. See the workspace `VERIFICATION_AND_VALIDATION.md` and `RESPONSIBLE_USE.md`. Not for nuclear facility operation, reactor control, safety-critical, or licensing decisions.


> NJOY2016 module port. Theory summarised from the NJOY2016 manual
> (LA-UR-17-20093, §GROUPR); upstream Fortran: `groupr.f90` (~12.7k lines,
> NJOY2016 commit `ac5adf5f33d893e42f2eed7fb286b0d51c7580da`).

## Theory

GROUPR collapses continuous-energy data onto a **multigroup** structure for
deterministic transport. A group cross section is the flux-weighted average

$$\sigma_{x,g} = \frac{\int_g \sigma_x(E)\,\phi(E)\,dE}{\int_g \phi(E)\,dE}$$

with $\phi(E)$ a weighting spectrum (built-in analytic weights — $1/E$,
Maxwellian, thermal + $1/E$ + fission — or a user tabulation, optionally
self-shielded via the Bondarenko $\sigma_0$ from UNRESR). Beyond vector cross
sections GROUPR builds group-to-group scattering matrices (with Legendre order
for anisotropy), photon-production matrices, ratio quantities ($\bar\mu$,
$\bar\nu$, inverse velocity, photon yield), fission as a full group-to-group
matrix, delayed-neutron spectra, and anisotropic thermal scattering. Output is a
**GENDF** tape, the input to CCCCR/MATXSR/DTFR/POWR/WIMSR.

## Port status — PARTIAL (AI draft, untrusted until reviewed)

This is a self-contained partial port: the **input/selector/data** layer is
ported and unit-tested; the **numeric group-averaging engine** is explicitly
`NotPorted` (it needs a PENDF tape + flux integration that cannot be exercised
standalone). Files:

| File | Ports | Upstream |
|---|---|---|
| `input.rs` | Card deck `GrouprInput` + `parse`, selector enums (`WeightSelection`, `WeightOption`, `PrintOption`, `SmoothingOption`), `neutron_group_from_ign` adapter, `ReactionRequest` | `ruinb` `groupr.f90:1044-1130`; card reads `:628`, `:993`; option tables `:126-215` |
| `photon_groups.rs` | `PhotonGroupStructure` enum + `photon_group_structure(igg)` boundary tables (eV) | `gengpg` `groupr.f90:4651-4863` |
| `weights.rs` | `AnalyticWeight` closed-form weights (`iwt=2,3,4,6,7,11,12`); `ThermalFissionParams`; preserved built-in TAB1 tables (`iwt=5,8,9`) | `genwtf` `groupr.f90:4865-5113`; `getwtf` `:5115-5307` |
| `unresolved.rs` | Bondarenko flux (`genflx_bondarenko`), URR self-shielded table + retrieval (`UnresolvedTable::store`/`shield`) | `genflx` `:5309-5684`; `stounr`/`getunr` `:6802-6994` |
| `urr_pendf.rs` | PENDF MF=2/MT=152 tape reader: `read_urr_table` (LIST-body decode) + `read_urr_from_tape` (tape-locate) | `stounr` `:6802-6875` |
| `pendf_feed.rs` | PENDF MF=3/MF=13 smooth cross-section tape reader: `classify_mtd` + `read_pendf_cross_section` (ordinary single-TAB1 case only) | `getsig` init branch `:6646-6753` |
| `mod.rs` | Module map, re-exports, `run()` pipeline skeleton | `subroutine groupr` `:97-1042` |

### Ported (self-contained, tested)

- **Input card deck** — cards 1-10 modelled as `GrouprInput` with named selector
  enums; a free-format `GrouprInput::parse` reads the numeric cards, conditional
  group-grid cards (6/7), the analytic weight card (8c), the reaction list
  (card 9) and the next-material pointer (card 10).
- **Photon (gamma) group structures** — all built-in `igg = 2..10` tables
  (CSEWG-94, LANL-12/24/48, Steiner-21, Straker-22, VITAMIN-C-36/E-38/J-42),
  in eV, including the VITAMIN-C `eg8` splice. `igg = 0` = none; `igg = 1` =
  read-in (`NotPorted`).
- **Analytic weighting functions** — constant, `1/E`, thermal+1/E+fission,
  thermal-1/E-fission+fusion (with optional temperature dependence), and
  VITAMIN-E, evaluated from the `getwtf` closed forms. Built-in *tabulated*
  weight point tables (EPRI-CELL, fast-reactor, CLAW) are preserved verbatim.

### Neutron group structures — live in ERRORR (not duplicated)

The neutron `gengpn` tables (`ign` selector, `groupr.f90:1599-4649`) were already
ported in `src/errorr/groups.rs`. GROUPR **re-exports** them
(`groupr::neutron_group_structure`, `groupr::NeutronGroupStructure`) and maps
`ign → structure` with `groupr::neutron_group_from_ign` (forbidding the
ERRORR-only `ign = -1`). No boundary table is copied.

### NotPorted (explicit gap list — do not fabricate)

**Note (op-3ut, 2026-07-20):** this list predates the op-bsz self-shielding
pass and the op-3ut tape-feeder pass, both landed since it was written, and is
stale on several entries below — `genflx`/`getunr`/`stounr` (see
`unresolved.rs`/`urr_pendf.rs`), `panel`/`displa` (see `panel.rs`/`matrix.rs`),
and `getsig` (see `pendf_feed.rs`, ordinary MF=3/MF=13 case only) are now
**partially ported**; the rest of this list has not been re-audited item by
item. A `mod.rs`-level `run()` NotPorted entry point remains accurate — the
end-to-end pipeline is not wired together.

The numeric engine and its dependencies return `NjoyError::NotPorted`:

- reaction retrieval & feed functions: ~~`getmf6`~~ (**CORRECTED 2026-11-13, gh:#1369:** `getmf6` is `mf6_feed.rs`, wired into the deck driver's `panel` for `mfd = 8/18`; `gam102` LAW=2 is `njoy_panel::Gam102`; `conver`'s lists, `nextr` and `getyld`'s MF=6/9 product yields are `deck/groupr_auto.rs`), `getff`, `getfwt`, `getflx`,
  `getyld`, `getsig` (MF=10 photon-production + `MT=257/258/259` derived
  quantities only — the ordinary MF=3/MF=13 case is ported, see
  `pendf_feed.rs`), `getdis` (ported for neutron File-4 two-body channels
  in `two_body.rs` with `getfle`/`getco` in `file4.rs`; charged-particle
  Coulomb term~~, `MT=251-253` and File-6 two-body data~~ remain;
  **CORRECTED 2026-11-14 (gh:#1370):** `MT = 251-253` and MF=6 LAW 2-5 two-body
  data are written by the deck driver (`deck/groupr.rs`), and `getdis` with
  `mft = 4` takes `getfle`'s MF=4 AWR, which differs from MF=3's on Be-9), ~~`getgfl`, `getgyl`~~ **CORRECTED 2026-11-11** - `getgyl` (`mfd = 12` and the `nyl = 1` MF=13 case, with the constant-range scan), `getgfl` (`LI = 1`, and `LTT = 1` for one photon) and `getff` label 300 (incl. `econst`/`jconst` and the label-395 yield-only path) are ported in `photon_prod.rs` (NJOY2016 test 01 MF=16 MT 51 and 102 identical); MF=13 with several photons, `getgfl` with several anisotropic photons, `LTT = 2` and `getsed` photons remain, ~~`getsed`, `anased`~~ **CORRECTED 2026-11-11** — `getsed` (analytic law 9 only) and `anased` law 9 are ported in `continuum.rs` (tape driver, NJOY2016 test 01 MT 91 identical); ~~`getsed` law 1 and `anased` laws 5/7/11/12 remain~~ **CORRECTED 2026-11-13 (gh:#1369):** `getsed` law 1 and `anased` laws 5, 7 and 9 are ported (`continuum.rs`, `PhotonSed`; laws 7/9 in round 5, decks 02 and 11 identical), and `anased` laws 11 (`hnab`) and 12 (`e1`, `gami`) remain;
- quadrature / accumulation: `epanel`, `gengr`, `glmol` (the vector
  `panel`/`displa` reduction is ported — see `panel.rs`; the full matrix
  `panel`/`displa` with Lobatto re-evaluation of the feed function, the
  `rndoff`/`delta` shading and the `ig1`/`iglo`/`igt` bookkeeping is ported
  statement for statement in `matrix_panel.rs`; `matrix.rs` keeps the
  earlier trapezoid reduction for its isotropic kernel);
- flux calculator & URR self-shielding: ~~`getfwt`~~ (`flux_table::FwtReader`, 2026-11-14); ~~the slowing-down branch of
  `genflx` (`nflmax > 0`)~~ (**CORRECTED 2026-11-14, gh:#1370:** in the tape
  driver as `flux_table::genflx_card8a`, `ninwt` included) — the Bondarenko branch and `getunr`/`stounr` are
  ported, see `unresolved.rs`/`urr_pendf.rs`;
- kinematics / matrix machinery: `cm2lab`, `f6cm`, `f6lab`, `ll2lab`, `getaed`,
  `aedi`, ~~`getaed`~~, ~~`aedi`~~ (ported 2026-11-11 in `thermal.rs`: inelastic, incoherent and coherent elastic; test 01 MT 221/229/230 identical), `getco`, ~~`gam102`~~ (LAW=2 ported 2026-11-13; LAW=4 recoil not), ~~`conver`~~ (the reaction lists only, 2026-11-13; its MF=12 LO=2 conversion and MT=456 insertion are not), `hnab`;
- the GENDF matrix-record writer is ported (see `matrix.rs`/`gendf.rs`); the
  vector-record writer was already ported before this note.

~~Also `NotPorted` in the input layer: the flux-calculator card 8a (`iwt < 0`),
the TAB1 weight card 8b (`iwt = 1`), the resonance-flux card 8d (`iwt = 0`), the
card-9a extended residual format (`mfd = -1`), and *evaluation* of the built-in
tabulated weights (`iwt = 5, 8, 9`) which needs the ENDF TAB1 (`terpa`)
interpolator.~~ **CORRECTED 2026-11-13 (gh:#1370):** `GrouprInput::parse` still
refuses cards 8a, 8b, 8d and 9a, but the deck driver reads all four
(`deck::modules::read_groupr`, `card9a_reaction`; upstream deck 86 is identical).
The tape driver's `njoy_panel::getwtf` evaluates every `iwt` from 1 to 12, the
tabulated ones by `terpa` (decks 07 and 11 for `iwt = 9, 5`;
`tests/deck_driver_groupr_weights_vs_njoy2016.rs` for 1, 7, 8, 10, 11, 12 and for
`iwt = -3` with one sigma-zero). Still refused: ~~`iwt = 0` (`getfwt`)~~ (**CORRECTED
2026-11-14, gh:#1370:** `flux_table::genflx_fwt` with `FwtReader`, deck `FW1`), ~~the
slowing-down flux (`iwt < 0` with `nsigz > 1`; `slowing_down.rs` exists but is not
wired)~~, and a card 9a with a negative `mtd`. **CORRECTED 2026-11-14 (gh:#1370):**
`iwt < 0` with `nsigz > 1` is `flux_table::genflx_card8a`, whose solve is
`slowing_down::solve_flux` (shared with `genflx_slowing_down`); decks `SD0`-`SD3`
in `tests/deck_driver_groupr_flux_calculator_vs_njoy2016.rs` are identical to
NJOY2016's GENDF and `ninwt` file. The `run()` entry point returns
`NotPorted("groupr")`; decks go through `deck::run_deck`.

## Testing (V&V — methodology + results, 2026-07-15, commit `ac5adf5`)

Inline `#[cfg(test)]` tests in each file, run via
`crates/njoy-outram-park-fork/scripts/test.sh groupr` (12 GB cap, `--release`).

- **`photon_groups.rs`** — `igg` round-trip (0..10); every built-in structure's
  group count, strict-ascending ordering, and endpoint energies vs the Fortran
  `eg*` tables; `igg=0` empty / `igg=1` NotPorted; the VITAMIN-C 36-group splice
  (removes `eg8(7)=0.075 MeV`, drops `eg8(39)=20 MeV`) reproduced exactly.
- **`weights.rs`** — analytic-only `from_iwt` selection; constant/`1/E` values;
  `iwt=4` piecewise continuity at both breakpoints (rel err < 1e-12); `iwt=6`
  segment values + the 14 MeV fusion bump; VITAMIN-E six-segment values (rel err
  < 1e-6); temperature dependence active only for `iwt=7`; the preserved TAB1
  tables' length/header/monotone-`x` structure.
- **`input.rs`** — the `ign` adapter round-trips against ERRORR's authoritative
  `NeutronGroupStructure::ign` (1..36, cannot drift); every card-2 selector
  round-trips through its Fortran integer; the default deck matches NJOY's init
  block; a representative U-235 VITAMIN-J deck parses field-by-field; conditional
  cards 6 + 8c parse; NotPorted weight cards (8a/8b/8d) are reported honestly.
- **`mod.rs`** — `run()` → `NotPorted("groupr")`; the neutron + photon surfaces
  both resolve (ign=17 → 176 boundaries; igg=10 → 43 boundaries).

Test counts: see the porting agent's hand-off / CI.

### Golden-file validation vs NJOY2016 (2026-09-10)

The **vector path with the Bondarenko self-shielded flux** (`genflx` narrow-
resonance branch + `panel`/`displa`, i.e. `unresolved::genflx_bondarenko`,
`panel::group_integral`, `self_shielded::self_shielded_group_xs`) is now
validated against a real NJOY2016 GENDF tape:
`tests/groupr_u238_gendf_golden.rs`, golden data + the exact deck in
`reference-data/gendf/` (U-238, ENDF/B-VIII.0, 293.6 K, 29 user groups,
`iwt=3`, six `sigz`, MT 1/2/18/102). Measured, all 29 groups × 6 dilutions:

- fed NJOY's **own PENDF** (engine isolated): `sigma_g` within **2.65e-6** and
  the group flux within **4.93e-7** of the GENDF — the 7-significant-figure
  storage floor;
- fed the crate's **own RECONR + BROADR**: MT 1/2/102 within **9.24e-4**, flux
  within 6.06e-5; MT=18 within 1.14e-2, which is NJOY's own `errmax = 10*err`
  slack on sub-threshold fission under its `errint` floor (`reconr.f90:109-117`),
  not an engine discrepancy.

- fed NJOY's PENDF **after UNRESR** (a second deck with `unresr` and the same
  six `sigz`), with `urr_pendf::read_urr_from_tape` supplying the MF=2/MT=152
  table: `sigma_g` within **2.65e-6**, flux within **4.93e-7** — i.e. the URR
  groups (20–149 keV) agree to the storage floor too. This measurement found
  and fixed one port defect: `genflx` shields the *total* in the flux
  denominator through `getunr(1, …)` (`groupr.f90:5636-5650`); the port used
  the smooth total, which left the group-22 flux 6.8 % low at `sigz = 1` b
  (`unresolved::genflx_bondarenko_urr` is the fix; `sigma_g` had only moved
  1.5e-3, so a cross-section-only check would have missed it).

- the **flux calculator** (`iwt = -3`, `fehi = 1e4 eV`, `sigpot = 11.29 b`;
  `slowing_down::genflx_slowing_down`, homogeneous branch) fed NJOY's PENDF:
  `sigma_g` within **2.87e-6**, flux within **3.89e-7**, and the flux table
  reproduces NJOY's point counts (926 tail / 99,934 solved / 55,488 NR). This
  measurement found three port defects, all read out of `groupr.f90` before
  being confirmed: the weight-shape tail below `felo` must sit on `getwtf`'s
  1 % ladder (`:5563-5577`), the NR extension above `fehi` also steps by 1 %
  (`:5626-5631`), and the NR in-scatter source uses the weight at `fehi`,
  not at `e` (`:5460`).

**Elastic transfer matrix, golden-validated (2026-09-10,
`tests/groupr_u238_elastic_matrix_golden.rs`):** the same U-238 deck with
`6 2` added (oracle `reference-data/dtfr/*-mf6.gendf`, `NL = 1`, `NZ = 6`).
On NJOY's own PENDF, with the File-4 `LTT = 3` distribution from the ENDF
tape, `two_body.rs` (`getdis`) + `matrix_panel.rs` (`panel`/`displa`)
reproduce all 29 initial-group records with identical `ig2lo`/`ng2`; the
516 words agree to **3.42e-7** (transfer elements) and **1.44e-7** (group
fluxes), i.e. the seven-figure GENDF floor. Prediction before the run was
1e-5. A second oracle with **`lord = 3`** (`NL = 4`, six sigma-zero values,
`reference-data/gendf/*-lord3-mf6.gendf`, the `genflx` `fac^(il+1)` flux
components from `unresolved::genflx_bondarenko_components`) agrees on all
2064 words: P0/P1 and the large P2/P3 elements to the seven-figure floor,
the P2/P3 elements below `1e-5 sigma_g` within 0.02 units of the feed
function's own `1e-7` rounding. That oracle exposed one port defect first:
`getfle` writes the coefficient count back into `getdis`'s `nld`, which
sets the Gauss order — with `nld` left at 21 the group-1 P2 feed used the
seven-digit 20-point table and came out 2.5 % (one `1e-7` unit) low.

**Derived quantities `MT=257/258/259`, golden-validated (2026-09-10,
`tests/groupr_u238_derived_quantities_golden.rs`,
`reference-data/gendf/*-mt257-259.gendf`):** `getsig`'s analytic branch
(`pendf_feed.rs` → `PointwiseXs::Derived`, `1.01 E` retrieval step) through
the vector `panel`: all 29 groups of the three quantities within
**5.59e-7** of NJOY, fluxes within 2.97e-7.

**Flux-calculator heterogeneity / multi-moderator terms, golden-validated
(2026-09-10, `tests/groupr_u238_gendf_golden.rs` tier 5,
`reference-data/gendf/*-iwt-3-fehi1e4-het-6sigz.gendf`):** card 8a
`alpha2 = 0.7768, sam = 0.5, beta = 0.3, alpha3 = 0.7143, gamma = 0.4`
(`nalph = 3`) through `slowing_down::genflx_slowing_down`: every `sigma_g`
within **2.72e-6** and every group flux within 3.89e-7 of NJOY, on a golden
that differs from the homogeneous one by 2x in group 1 at `sigma_0 = 1 b`.

**Discrete-level inelastic vectors and P0-P3 matrices (MT=51/52/60/89),
golden-validated (2026-09-10, `tests/groupr_u238_inelastic_matrix_golden.rs`,
`reference-data/gendf/*-{6sigz,1sigz}-lord3-inelastic-mf3-mf6.gendf`):** the
`q < 0` threshold path of `getdis` on two decks (`nsigz = 6` and `nsigz = 1`,
which exercise the two `getflx` branches): vectors within 3.2e-6, every
transfer element within 1e-5 or under one unit of `1e-7 sigma_g`. Three
findings on the way: `GroupFlux::analytic` now steps at `getwtf`'s 1.01
(`GETWTF_STEP`), not GAMINR's 1.05; `nz = 1` reactions in an `nsigz > 1`
deck take the tabulated `genflx` flux (a test-construction error, not a
port defect); and `getfle`'s label-210 slide keeps stale high-order
coefficients, now replicated in `File4Angular` (an 8 % P3 element).

**MF=10 residual production (`mfd = 4zzzaaam`), golden-validated
(2026-09-10, `tests/groupr_u235_mf10_golden.rs`,
`reference-data/gendf/u235-*-1sigz-mf10.gendf`):** `getsig`'s MF=10
subsection search (`groupr.f90:6719-6746`) for U-235 `MT=4` into the ground
state and the 235m isomer; see the test's doc comment for the measured
agreement on NJOY's PENDF table (strict) and on the ENDF table (looser).

Still **not** golden-validated: File-6 continuum feeds,
`LSSF = 0` materials, and more than one temperature. **CORRECTED 2026-11-13
(gh:#1369):** through the deck driver (`njoy_panel::group_general_nz`,
`flux_table`), not this engine, NJOY2016's own decks are identical line for
line with several temperatures and sigma-zeros: deck 11 (Pu-238 ENDF/B-IV,
`LSSF = 0`, 3 temperatures x 7 sigma-zeros, MF=6 continuum MT=16/17/91) and
deck 47's second GROUPR (Pu-239, 2 sigma-zeros). See
`verification_and_validation/deck_driver/README.md` section 16. GAMINR's engine now has
its own oracle (`src/gaminr/README.md`).

## Caveats

- **AI-generated draft — untrusted until human-reviewed** per crate CLAUDE.md.
  A human must check the transcribed `eg*`/`w*` data tables and the `getwtf`
  closed-form constants against `groupr.f90` before any use.
- **Not required by OpenMC CE** — Phase 5, deterministic/sensitivity workflows.
- Self-shielded matrices depend on UNRESR/PURR outputs; port those first for URR
  nuclides.
- Huge surface area — the engine must be ported feed-function by feed-function,
  verifying each against the oracle.

## References

- NJOY2016 manual §GROUPR (LA-UR-17-20093)
- `groupr.f90` (NJOY2016, commit `ac5adf5f33d893e42f2eed7fb286b0d51c7580da`)
- Boltzmann constant `bk = 8.617333262e-5` eV/K (`phys.f90:22`)
