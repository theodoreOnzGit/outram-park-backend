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
`ElectronTransport` trait), `ELECTR` (behind `ElectronGeometry`; no EM fields or range rejection; EII through `ElectrTables::eii`; **assumption:** `NEXT :TSTEP:` restarts the loop without re-testing its `UNTIL`, to be settled by a differential run), `UPHI` (IENTRY 1, 2, 3), `mscat`, `sscat`,
`spin_rejection`, the MS-table text reader of `init_ms_SR`, the `EDGSET` data readers (`edgset_from_text`), `msdist_pI` and `msdist_pII` (`msdist_p1`, `msdist_p2`), the PEGS4 intake of `HATCH` (`parse_pegs4_media`, `Pegs4Medium::apply_dunit`), `fix_brems`, `init_compton` (non-EADL) and `mscati`, `init_spin` (from decoded records), `set_spline`, `spline`, `hatch_media` and `adjust_rhor_ecut_pcut`, and `erf1` from `egs_utilities`, Russian roulette,
and the stack, azimuth and `$RANDOMSET` macros.

## Not ported, and what each needs

| Item | Upstream | Why it is a chain, not a routine |
|---|---|---|
| ~~`spinms.data` decoder~~ | `init_spin` (`:3369-3420`) | **PORTED 2026-11-21** as `SpinFile::from_bytes` (1104-byte records, byte order from the file's own `1234`/`4321` token; round-trip tested in both orders on synthetic bytes). **Not re-checked against a real `spinms.data`** (not in the clone), nor is the assumption that `$RECL-FACTOR` leaves the physical record at 276 four-byte words. `NrcPairFile::from_bytes` does the same for `pair_nrc1.data` (65-word records). |
| `HATCH` (rest) | `:1827-2598` | The PEGS4 intake (`parse_pegs4_media`) and `DUNIT` scaling (`Pegs4Medium::apply_dunit`) are ported and were checked on a real `pb.pegs4dat` (`EGSNRC_PEGS4_DATA`). Not yet: turning `Pegs4Medium` into the per-medium tables the routines take (`PairMedium`, `BremsMedium`, `PhotonMediumData`, `ComptonData`, `ElectronMediumData`), ~~`egs_init_user_photon`~~ **PORTED 2026-11-21** (`init_user_photon`, `user_xsec_data`, `egs_kn_sigma0`; text in, no `.xsections` listing, no MCDF shellwise PE, no custom Rayleigh form-factor file; toy-data tests only),  `eii_init`, `init_triplet`. The Rayleigh initialiser from `pgs4form.dat` (`FormFactorTable::from_text`, `init_rayleigh_sampling`) is ported; `hatch_media` copies the PEGS4 `cohe` fit into `PhotonMediumData.cohe` but leaves `rayleigh: None`; the caller attaches `init_rayleigh_sampling(..)` (needs `pgs4form.dat`) before using `iraylr = 1`. |
| `PHOTONUC` | `:9001-9015` | Ported (`photonuc`, with the `$PHOTONUC-CORRECTION` and `$PHOTONUCLEAR` hooks in `photon`). Upstream is a placeholder: it zeroes the photon's energy and weight, and `photon` then discards it with `edep = peig`, so scoring is told the energy was deposited. Pinned by a test. |
| ~~`egs_rayleigh_sampling`~~ | `:8558` | **PORTED 2026-11-21** with `egs_init_rayleigh_sampling`, `prepare_rayleigh_data`, `egs_rayleigh_sigma`, `zero()` and the `$RAYLEIGH-*` hooks in `photon` (first port, not differentially tested; tests are toy-data and energy balance, not compared to EGSnrc). |
| ~~`sample_triplet`~~ **PORTED 2026-11-21** (`sample_triplet`, `init_triplet`, `pair_with_triplet`; first port, toy-data and conservation tests only, `photon` does not yet carry the triplet state so `itriplet > 0` there is still refused), ~~NRC pair~~ **PORTED 2026-11-21** (`init_nrc_pair` takes decoded records, and `NrcPairFile::from_bytes` decodes the direct-access file (layout inferred from the record contents, not re-checked against a real file); `pair_with_nrc`; `prepare_alias_table`), ~~NIST/NRC brems tables~~ **PORTED 2026-11-21** (`init_nist_brems`, `gauss_legendre`, and the `ibr_nist >= 1` branch of `brems`; toy-data tests only, `nist_brems.data`/`nrc_brems.data` are not in the clone so the real files are untested), EII (`eii_sample`), shellwise PE, EADL relaxation | various | Off by default; each is refused with `EgsError::Unsupported`. |
| Whole files | `pegs4/pegs4.mortran`, `get_media_inputs`, `egs_utilities` (the operating-system glue only: `egs_init`/`egs_finish`/`egs_open_file`/`egs_combine_runs`/`egs_check_arguments` and the unit and byte readers, deliberately not ported in a `no_std` crate), `eii` (`src/`) | `nrcaux` is ported except `WATCH` (event-trace printing to Fortran units, `nrcaux.mortran:68-400`; deliberately not ported in a `no_std` library: it holds no physics, only diagnostic output); `erf1`, `set_spline`, `spline`, `alias_sample1` from `egs_utilities`. `eii` is ported except the `ELECTR` wiring: see the findings below. The rest is not started. |

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
- ~~**`eii_init` (`eii.mortran`) is not ported**~~ **CORRECTED 2026-11-21:**
  `eii_init`, `eii_sample` and the EII branch that opens `MOLLER`
  (`moller_eii_select`) are ported (first port, toy-data tests only; `eii_ik.data`
  is not in the clone, so the real file is untested). Four things in the source
  look wrong and only a built EGSnrc can say which behaviour is real; the first
  three are switches in `EiiReadings` (intended reading by default, literal on
  request), the fourth is reproduced literally:
  1. `eii.mortran:176` writes `binding_energies(iZ, ish)` for an array declared
     `(shell, iZ)` (`binding_energy_swapped`);
  2. `eii.mortran:215` reads `ap(medium)` for `Ecc` where `imed` is meant
     (`ecc_ap`);
  3. `eii.mortran:227` uses `eii_a(jjj)` (shell type) where `eii_a(jj)` looks
     intended (`bin_index_by_shell_type`);
  4. `eii.mortran:345` sets the primary's total energy to `peie - prm`, not
     `peie - wx` as `MOLLER` does with its own `pese1`. That loses `prm` per
     event and, below `T = prm`, makes `cos^2 < 0` (NaN directions upstream); the
     port reproduces the energy and returns `EgsError::Fatal` for the NaN case.
     Pinned by `eii_sample_follows_upstream_including_its_primary_energy`, which
     is not a statement that the physics is right.

  ~~Still open: `ELECTR` does not call `moller_eii_select` yet~~ **CORRECTED
  2026-11-21:** `electr` calls `moller_eii_select` when
  `ElectrTables::eii` is `Some`, and skips the `thmoll` force-brems check
  when EII is on (`egsnrc.mortran:1672`); pinned by
  `electr_uses_the_eii_selection_when_eii_is_on` (toy numbers; the EII energy
  deposit is reported in `ElectrResult::edep_eii`).
- **`NEXT :TSTEP:` in `ELECTR`** is read as a loop restart (see above).
