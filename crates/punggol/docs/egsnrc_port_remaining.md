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
`ElectronTransport` trait), `UPHI` (IENTRY 1, 2, 3), `mscat`, `sscat`,
`spin_rejection`, the MS-table text reader of `init_ms_SR`, Russian roulette,
and the stack, azimuth and `$RANDOMSET` macros.

## Not ported, and what each needs

| Item | Upstream | Why it is a chain, not a routine |
|---|---|---|
| `ELECTR` | `:985-1825` (840 lines) | The condensed-history loop. Needs the macros `$EVALUATE-SIG0`, `$SCALE-SIG0`, `$CALCULATE-TSTEP-FROM-DEMFP`, `$COMPUTE-RANGE`, `$COMPUTE-ELOSS-G`, `$SET-USTEP`, `$SET-TVSTEP`, `$SET-SKINDEPTH` (`egsnrc.macros:1308-1360, 2358, 3113-3330`), `msdist_pII` (`:4049`) and `msdist_pI` (`:4309`), and the per-medium tables below. Plug in through `ElectronTransport`. |
| `mscati` | `:2749-2980` | Builds range, `tmxs` and the maximum cross sections per medium from `HATCH`'s tables. |
| `init_spin` | `egsnrc.mortran` (spin data) | Fills `SpinData::spin_rej`; reads a data file. |
| `HATCH` | `:1827-2598` | Reads PEGS4 data and builds every per-medium table the ported routines take as arguments (`PairMedium`, `BremsMedium`, `PhotonMediumData`, `ComptonData`, `RelaxData`, `PhotoData`). Needs PEGS4 data files (not vendored). |
| `EDGSET` | `:6311-6438` | Photo-absorption edge fits (`PhotoData`), called from `HATCH`. |
| `PHOTONUC` | `:9001` | Photonuclear reactions; off by default. |
| `egs_rayleigh_sampling` | `:8558` | Rayleigh scattering; off by default (refused by `photon`). |
| `sample_triplet`, NRC pair, NIST/NRC brems tables, EII (`eii_sample`), shellwise PE, EADL relaxation | various | Off by default; each is refused with `EgsError::Unsupported`. |
| Whole files | `pegs4/pegs4.mortran`, `get_media_inputs`, `egs_utilities`, `nrcaux`, `eii` (`src/`) | Not started. |

## Why `ELECTR` was not rushed

A first port of `ELECTR` without `msdist_pII`, `HATCH`'s tables and the range
and energy-loss macros would compile and could not be tested for anything but
its own consistency. The routines above were each checked against a physical
identity (energy, momentum, norm); `ELECTR` needs its tables first so the same
can be done.
