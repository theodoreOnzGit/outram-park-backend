# EGSnrc port (gh:#1680): what is ported, what is not (2026-11-20)

Upstream: EGSnrc commit `bd49a47aae3938f49f4100b8e018091861408b19`
(`HEN_HOUSE/src/egsnrc.mortran`, line numbers below are that file's). Nothing
here has been compared with a built EGSnrc: every ported routine is a **first
port, not differentially tested** (phase 2).

## Ported (all in `src/egsnrc/egsnrc_mortran.rs` and `egsnrc_macros.rs`)

`ANNIH_AT_REST`, `ANNIH`, `COMPT` (RELAX returned as a seam), `RELAX` (built-in
scheme), `PAIR` (no NRC tables; triplets via `pair_with_triplet`), `PHOTO` (no MCDF or EADL),
`BHABHA`, `MOLLER` (no EII), `BREMS` (Bethe-Heitler), `PHOTON` (behind the
`Geometry` trait; Rayleigh when the medium carries `RayleighData`), `PHOTONUC`, `SHOWER` (behind the
`ElectronTransport` trait), `ELECTR` (behind `ElectronGeometry`; no EM fields, range rejection or EII; **assumption:** `NEXT :TSTEP:` restarts the loop without re-testing its `UNTIL`, to be settled by a differential run), `UPHI` (IENTRY 1, 2, 3), `mscat`, `sscat`,
`spin_rejection`, the MS-table text reader of `init_ms_SR`, the `EDGSET` data readers (`edgset_from_text`), `msdist_pI` and `msdist_pII` (`msdist_p1`, `msdist_p2`), the PEGS4 intake of `HATCH` (`parse_pegs4_media`, `Pegs4Medium::apply_dunit`), `fix_brems`, `init_compton` (non-EADL) and `mscati`, `init_spin` (from decoded records), `set_spline`, `spline`, `hatch_media` and `adjust_rhor_ecut_pcut`, and `erf1` from `egs_utilities`, Russian roulette,
and the stack, azimuth and `$RANDOMSET` macros.

## Not ported, and what each needs

| Item | Upstream | Why it is a chain, not a routine |
|---|---|---|
| `spinms.data` decoder | `init_spin` (`:3369-3420`) | `init_spin` is ported from decoded records (`SpinFile`); the binary layout depends on the build-time macros `$BYTE_ORDER` and `$RECL-FACTOR`, which are not in the source read. A decoder must fill `SpinFile`. |
| `HATCH` (rest) | `:1827-2598` | The PEGS4 intake (`parse_pegs4_media`) and `DUNIT` scaling (`Pegs4Medium::apply_dunit`) are ported and were checked on a real `pb.pegs4dat` (`EGSNRC_PEGS4_DATA`). Not yet: turning `Pegs4Medium` into the per-medium tables the routines take (`PairMedium`, `BremsMedium`, `PhotonMediumData`, `ComptonData`, `ElectronMediumData`), `egs_init_user_photon`, `init_nist_brems`, `init_nrc_pair`, `eii_init`, `init_triplet`. The Rayleigh initialiser from `pgs4form.dat` (`FormFactorTable::from_text`, `init_rayleigh_sampling`) is ported; `hatch_media` copies the PEGS4 `cohe` fit into `PhotonMediumData.cohe` but leaves `rayleigh: None`; the caller attaches `init_rayleigh_sampling(..)` (needs `pgs4form.dat`) before using `iraylr = 1`. |
| `PHOTONUC` | `:9001-9015` | Ported (`photonuc`, with the `$PHOTONUC-CORRECTION` and `$PHOTONUCLEAR` hooks in `photon`). Upstream is a placeholder: it zeroes the photon's energy and weight, and `photon` then discards it with `edep = peig`, so scoring is told the energy was deposited. Pinned by a test. |
| ~~`egs_rayleigh_sampling`~~ | `:8558` | **PORTED 2026-11-21** with `egs_init_rayleigh_sampling`, `prepare_rayleigh_data`, `egs_rayleigh_sigma`, `zero()` and the `$RAYLEIGH-*` hooks in `photon` (first port, not differentially tested; tests are toy-data and energy balance, not compared to EGSnrc). |
| ~~`sample_triplet`~~ **PORTED 2026-11-21** (`sample_triplet`, `init_triplet`, `pair_with_triplet`; first port, toy-data and conservation tests only, `photon` does not yet carry the triplet state so `itriplet > 0` there is still refused), NRC pair, NIST/NRC brems tables, EII (`eii_sample`), shellwise PE, EADL relaxation | various | Off by default; each is refused with `EgsError::Unsupported`. |
| Whole files | `pegs4/pegs4.mortran`, `get_media_inputs`, `egs_utilities` (rest), `eii` (`src/`) | `nrcaux` is ported except `WATCH` (event-trace printing); `erf1`, `set_spline`, `spline`, `alias_sample1` from `egs_utilities`. `eii` is not ported: see the findings below. The rest is not started. |

## What now binds `ELECTR` to data

`electr` takes the per-medium tables (`ElectrTables`) as arguments; `HATCH`,
`mscati` and `init_spin` are what would fill them from PEGS4 data, and they are
not ported. Until then `ELECTR` runs on tables the caller builds (the tests use
a synthetic constant-stopping-power medium and check the energy balance of
full showers through `SHOWER`, `ELECTR` and `PHOTON`).


## Findings from reading upstream (to confirm in phase 2)

- **`prepare_alias_sampling` (`nrcaux.mortran:507-557`) leaves an exactly-average
  bin unfinalised** (negative weight, always redirected through its initial
  alias, bin 1). A uniform histogram or a tie such as `0.4 - 0.15 = 0.25`
  triggers it; `init_compton` avoids it with a final loop. Ported as is and
  pinned by `exactly_average_bins_stay_unfinalised_as_upstream`.
- **`eii_init` (`eii.mortran`) is not ported** because three things in it
  cannot be reproduced from the source alone: it indexes `binding_energies(iZ,
  ish)` with the arguments swapped relative to the declaration
  `binding_energies(shell, iZ)`; it reads `ap(medium)` for `Ecc` where `imed`
  is meant (the global `medium` at that point is not `imed`); and the shell
  loop uses `eii_a(jjj)` (shell type) where `eii_a(jj)` looks intended. Each
  would need a built EGSnrc to see which behaviour is real.
- **`NEXT :TSTEP:` in `ELECTR`** is read as a loop restart (see above).
