# PUNGGOL

**P**article transport, **UN**certainty, **G**eometry, **G**amma transport,
**O**ptimization **L**ibrary (backronym set by the maintainer, 2026-10-20;
`docs/ecosystem-naming.md`).

> ⚠️ **Research, education and V&V only.** Not for nuclear facility operation,
> reactor control, licensing, safety-critical decisions or emergency response.
> See the workspace `RESPONSIBLE_USE.md`. Shielding and dose-adjacent output
> from this crate is not an authority for any licensing or safety decision.

**Scope.** Radiation transport beyond neutrons: photons (gammas), electrons and
positrons, coupled to `outram-mc-libs`' neutrons, for heating, ionisation and
shielding. The one question it answers is **"Where do the gammas and betas go,
and where do they deposit their energy?"** Ions (fission fragments, alphas,
recoils) stay in `boon-lay`.

## Status

Stated plainly: **decay-photon source spectra and a first port of OpenMC's photon samplers; no transport loop yet.**

| Part | State |
|---|---|
| `decay_spectrum`: OpenMC's `decay_photon_energy`, `Material.get_decay_photon_energy` and `DecaySpectrum` (gh:#1391), from the depletion chain's `<source>` elements | implemented, AI draft; checked against OpenMC in `verification_and_validation/decay_photon_spectra/` |
| `egsnrc`: the EGSnrc port (gh:#976, gh:#1680) | **barely begun**: 21 modules, one per upstream Mortran file; `ANNIH_AT_REST`, `ANNIH`, `COMPT`, `RELAX` (built-in scheme, not the EADL tables), `PAIR` (no NRC tables or triplets), `PHOTO` (no MCDF shellwise or EADL branches), `BHABHA`, `MOLLER` (no EII branch), `BREMS` (Bethe-Heitler only, no NIST/NRC tables), `PHOTON` (the photon transport loop behind a compile-time `Geometry` seam; no Rayleigh or photonuclear), `SHOWER` (with the pi-zero option), `ELECTR` (condensed-history electron transport, PRESTA-I and II, behind a compile-time `ElectronGeometry` seam; no EM fields, range rejection or EII), the `ELECTR` helpers `mscat`, `sscat`, `spin_rejection` the MS-table text reader (`real*4` storage kept) the `EDGSET` data readers, `msdist_pI`/`msdist_pII` and the PEGS4 intake of `HATCH` (fixed-width reader, checked on a real PEGS4 file, and the `DUNIT` scaling), `fix_brems`, `init_compton` (non-EADL; alias tables checked exactly), `mscati` (range, `tmxs`, maximum cross sections), the table assembly of `HATCH` (`hatch_media`, `adjust_rhor_ecut_pcut`) `erf1`, `set_spline`/`spline` and `init_spin` (from decoded `spinms.data` records; the binary decoder is not ported), `UPHI` (IENTRY 1, 2, 3), Russian roulette and the stack/azimuth macros are a first port, **not differentially tested**; RANMAR/RANLUX stay in PETIR (`petir::egsnrc`); everything else (PEGS4, media, `HATCH` (the photon-library, NIST, NRC, EII, triplet and Rayleigh initialisers), the `spinms.data` binary decoder, `PHOTONUC`, `egs_utilities`, `nrcaux`, `eii`) is still a placeholder |
| `openmc_photon`: OpenMC photon samplers (`photon.cpp`), thick-target bremsstrahlung (`bremsstrahlung.cpp`) and `sample_photon/electron/positron_reaction`, `process_charged_secondary` (`physics.cpp`) (gh:#975, gh:#1683) | first port, **not differentially tested**; AI draft; data passed in, no HDF5 reader; TTB and atomic relaxation on by default with `without_*` ablations |

## Boundaries

- **Neutrons stay in `outram-mc-libs`.** PUNGGOL depends on it, **never the
  reverse**. The chain codec (`outram_mc_libs::depletion::DepletionChainXml`)
  and the univariate samplers (`outram_mc_libs::source::distribution`) are
  reused, not copied.
- General numerics (generators, alias tables, interpolation) go to `petir`.
- `no_std` + `alloc` without the default `std` feature.

## EGSnrc scaffold (gh:#976)

Upstream: <https://github.com/nrc-cnrc/EGSnrc>, commit
`bd49a47aae3938f49f4100b8e018091861408b19` (file list read from a shallow
clone on 2026-11-20; EGSnrc is not vendored and none of its text is committed).

- **In scaffold:** `HEN_HOUSE/src/*.mortran` and `*.macros` (20 files) and
  `HEN_HOUSE/pegs4/pegs4.mortran`, one module each under `src/egsnrc/`, named
  by the file name with `.` read as `_`. Each module says what its routine
  family does and that nothing is ported. No stub functions.
- **Out of scope for now:** the `egs++` C++ geometry library
  (`HEN_HOUSE/egs++`) and the user codes (`HEN_HOUSE/user_codes`).
- **Licence:** EGSnrc is AGPL-3.0 (file headers: version 3 or, at the
  licensee's option, later), the same as this workspace; parts originate from
  SLAC's EGS4 and keep their own notice. Recorded in `NOTICE`. A ported file
  carries the attribution header block (upstream project, file, commit,
  copyright, licence).

## Bookkeeping status

> Maintainer sign-off tracker (see the workspace `CLAUDE.md` "Bookkeeping
> pass" command). A crate is **complete** only once the maintainer has
> personally signed off on BOTH axes below.

| Axis | Status |
|---|---|
| Verification & Validation (V&V) — human-reviewed | ❌ Not yet manually checked |
| Human / user interface — human-reviewed | ❌ Not yet manually checked |

**Status: INCOMPLETE** until both axes are manually checked and cleared by the maintainer.

## License

**AGPL-3.0-only.** Part of the [OUTRAM PARK](../../README.md) workspace. OpenMC
(MIT) is the upstream of `decay_spectrum`; see `NOTICE`.
