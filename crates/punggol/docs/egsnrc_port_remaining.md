# EGSnrc port (gh:#1680): what is ported, what is not (2026-11-20)

Upstream: EGSnrc commit `bd49a47aae3938f49f4100b8e018091861408b19`
(`HEN_HOUSE/src/egsnrc.mortran`, line numbers below are that file's). Nothing
here has been compared with a built EGSnrc: every ported routine is a **first
port, not differentially tested** (phase 2).

## Ported (all in `src/egsnrc/egsnrc_mortran.rs` and `egsnrc_macros.rs`)

`ANNIH_AT_REST`, `ANNIH`, `COMPT` (RELAX returned as a seam), `RELAX` (built-in
scheme), `PAIR` (no NRC tables or triplets), `PHOTO` (no MCDF or EADL),
`BHABHA`, `MOLLER` (no EII), `BREMS` (Bethe-Heitler), `PHOTON` (behind the
`Geometry` trait, no Rayleigh or photonuclear), `SHOWER` (behind the
`ElectronTransport` trait), `ELECTR` (behind `ElectronGeometry`; no EM fields, range rejection or EII; **assumption:** `NEXT :TSTEP:` restarts the loop without re-testing its `UNTIL`, to be settled by a differential run), `UPHI` (IENTRY 1, 2, 3), `mscat`, `sscat`,
`spin_rejection`, the MS-table text reader of `init_ms_SR`, the `EDGSET` data readers (`edgset_from_text`), `msdist_pI` and `msdist_pII` (`msdist_p1`, `msdist_p2`), the PEGS4 intake of `HATCH` (`parse_pegs4_media`, `Pegs4Medium::apply_dunit`), `fix_brems`, `init_compton` (non-EADL) and `mscati` (without `init_spin`), and `erf1` from `egs_utilities`, Russian roulette,
and the stack, azimuth and `$RANDOMSET` macros.

## Not ported, and what each needs

| Item | Upstream | Why it is a chain, not a routine |
|---|---|---|
| `init_spin` | `egsnrc.mortran` (spin data) | Fills `SpinData::spin_rej`; reads a data file. |
| `HATCH` (rest) | `:1827-2598` | The PEGS4 intake (`parse_pegs4_media`) and `DUNIT` scaling (`Pegs4Medium::apply_dunit`) are ported and were checked on a real `pb.pegs4dat` (`EGSNRC_PEGS4_DATA`). Not yet: turning `Pegs4Medium` into the per-medium tables the routines take (`PairMedium`, `BremsMedium`, `PhotonMediumData`, `ComptonData`, `ElectronMediumData`), `egs_init_user_photon`, `init_nist_brems`, `init_nrc_pair`, `eii_init`, `init_triplet`, `egs_init_rayleigh_sampling`. |
| `PHOTONUC` | `:9001` | Photonuclear reactions; off by default. |
| `egs_rayleigh_sampling` | `:8558` | Rayleigh scattering; off by default (refused by `photon`). |
| `sample_triplet`, NRC pair, NIST/NRC brems tables, EII (`eii_sample`), shellwise PE, EADL relaxation | various | Off by default; each is refused with `EgsError::Unsupported`. |
| Whole files | `pegs4/pegs4.mortran`, `get_media_inputs`, `egs_utilities`, `nrcaux`, `eii` (`src/`) | Not started. |

## What now binds `ELECTR` to data

`electr` takes the per-medium tables (`ElectrTables`) as arguments; `HATCH`,
`mscati` and `init_spin` are what would fill them from PEGS4 data, and they are
not ported. Until then `ELECTR` runs on tables the caller builds (the tests use
a synthetic constant-stopping-power medium and check the energy balance of
full showers through `SHOWER`, `ELECTR` and `PHOTON`).
