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
recoils) stay in ~~`boon-lay`~~ `kembangan` (moved 2026-11-22 with the RustBCA copy, gh:#1699).

What this crate makes studyable and challengeable is two photon and electron
codes, translated routine by routine. A reader can set an EGSnrc Mortran
routine (`COMPT`, `PAIR`, `BREMS`, `ELECTR`, `HATCH`) beside the Rust function
that ports it, each cited to `file:line`, and set OpenMC's `photon.cpp`
samplers beside `src/openmc_photon/`, then check the recorded differential
tests against compiled upstream builds, including the residuals that are not
bit-identical and the reasons found for them. It does not supply a geometry,
so it cannot yet show a shielding problem end to end.

## Knowledge in this crate

See the workspace [vision](../../VISION.md).

| Knowledge artifact | In this crate |
|---|---|
| Scientific literature | Ported upstreams (`NOTICE`): EGSnrc, `nrc-cnrc/EGSnrc` commit `bd49a47a`, AGPL-3.0 (parts from SLAC's EGS4), never vendored; OpenMC, `openmc-dev/openmc` commit `8f79d9d0`, MIT (`photon.cpp`, `bremsstrahlung.cpp`, `physics.cpp`, `decay.py`, `material.py`, `univariate.py`). Nothing in `crates/kovan-literature/CATALOGUE.md` names this crate. |
| Mathematical equations | In module docs and code comments, as upstream states them: Klein-Nishina and bound Compton with Doppler broadening, Rayleigh, Bethe-Heitler and NIST/NRC bremsstrahlung, pair and triplet production, Moller and Bhabha scattering, condensed-history electron transport with PRESTA-I/II (`src/egsnrc/egsnrc_mortran.rs`, `src/openmc_photon/`). No separate derivation document. |
| Source code | `src/decay_spectrum.rs` (decay-photon source spectra); `src/egsnrc/` (one module per upstream Mortran file, plus `estar_cpp.rs`); `src/openmc_photon/` (samplers, `bremsstrahlung.rs`, `reaction.rs`, `ttb_tables.rs`). |
| Interactive simulations | None yet. |
| Visualisations | None yet. |
| Verification and validation evidence | `verification_and_validation/decay_photon_spectra/` (2026-11-20, `tests/decay_photon_vs_openmc.rs`): every compared value bit-identical with OpenMC, e.g. 394 of 394 per-nuclide values on `simple_decay`. `docs/openmc_photon_phase2.md` (2026-11-21, `tests/openmc_photon_vs_openmc.rs`): 5400 uranium samples of `compton_scatter`, `rayleigh_scatter` and `pair_production` with worst difference 0. `docs/egsnrc_phase2_differential.md` (2026-11-21): PEGS4, the HATCH tables and 200 `tutor1` histories against a compiled EGSnrc; the histories follow the same sequence of discarded particles in 200 of 200, worst relative kinetic energy 1.1e-8 against the `-fdefault-real-8` build, flagged as above the 1e-9 code-to-code bound. Those bit-identity results were taken before PETIR became the maths route in every build (gh:#1706) and were not re-run after it. All tests that need a compiled upstream skip unless their reference variables are set. |
| Human reviews and corrections | 0 functions carry a kovan human-review stamp. No entry in `docs/human-corrections-to-ai-work.md` names this crate. Struck-through corrections in this README and in `docs/egsnrc_phase2_differential.md` record the EGSnrc port moving from "barely begun" to differentially tested, and bit identity moving from a `std`-only claim to last-bit round-off. |

**Status in one sentence:** decay-photon spectra and the OpenMC samplers have
code-to-code evidence against OpenMC, and the EGSnrc port has it against a
compiled EGSnrc; nothing is validated against measurement, and no geometry
exists here to transport through.

## Status

Stated plainly: ~~**decay-photon source spectra and a first port of OpenMC's photon samplers (`compton_scatter`, `rayleigh_scatter`, `pair_production` bit-identical to a built OpenMC for uranium: `docs/openmc_photon_phase2.md`); no transport loop yet.**~~ **CORRECTED 2026-11-21 (gh:#1682, gh:#1685):** decay-photon source spectra; the EGSnrc port (PEGS4, HATCH, the interactions, `PHOTON`/`ELECTR`/`SHOWER` transport loops, EM fields, `egs_parallel`), differentially tested against a compiled EGSnrc and ~~bit-identical with the `std` feature (to round-off without it)~~ **to last-bit round-off in every build since 2026-11-22**, when PETIR became the maths route under `std` too (gh:#1706; the bit-identical results were taken on the platform route): `docs/egsnrc_phase2_differential.md`; and OpenMC's photon samplers, TTB and reaction sampling, differentially tested for U and H (`docs/openmc_photon_phase2.md`). **The transport loops run behind a geometry seam; this crate supplies no geometry.** AI draft, untrusted until human review.

| Part | State |
|---|---|
| `decay_spectrum`: OpenMC's `decay_photon_energy`, `Material.get_decay_photon_energy` and `DecaySpectrum` (gh:#1391), from the depletion chain's `<source>` elements | implemented, AI draft; checked against OpenMC in `verification_and_validation/decay_photon_spectra/` |
| `egsnrc`: the EGSnrc port (gh:#976, gh:#1680) | ~~**barely begun**~~ **CORRECTED 2026-11-21 (gh:#1682):** ported and differentially tested, see the status line above and `docs/egsnrc_phase2_differential.md` (the list below is the phase-1 record): 21 modules, one per upstream Mortran file; `ANNIH_AT_REST`, `ANNIH`, `COMPT`, `RELAX` (built-in scheme, not the EADL tables), `PAIR` (NRC tables through `pair_with_nrc`/`init_nrc_pair` with caller-decoded records; triplets through `pair_with_triplet`, `sample_triplet`, `init_triplet`), `PHOTO` (no MCDF shellwise or EADL branches), `BHABHA`, `MOLLER` (its EII selection is `moller_eii_select`), `BREMS` (Bethe-Heitler, or the NIST/NRC tables built by `init_nist_brems`), `PHOTON` (the photon transport loop behind a compile-time `Geometry` seam; Rayleigh with `RayleighData`), `PHOTONUC` (the upstream placeholder that discards the photon), `SHOWER` (with the pi-zero option), `ELECTR` (condensed-history electron transport, PRESTA-I and II, behind a compile-time `ElectronGeometry` seam; no EM fields or range rejection; EII through `ElectrTables::eii` (four flagged oddities in `docs/egsnrc_port_remaining.md`)), the `ELECTR` helpers `mscat`, `sscat`, `spin_rejection` the MS-table text reader (`real*4` storage kept) the `EDGSET` data readers, `msdist_pI`/`msdist_pII` and the PEGS4 intake of `HATCH` (fixed-width reader, checked on a real PEGS4 file, and the `DUNIT` scaling), `fix_brems`, `init_compton` (non-EADL; alias tables checked exactly), `mscati` (range, `tmxs`, maximum cross sections), the table assembly of `HATCH` (`hatch_media`, `adjust_rhor_ecut_pcut`) `erf1`, `set_spline`/`spline`, `alias_sample1`, the `nrcaux` routines `SIGMA`, `prepare_alias_sampling` and `alias_sample`, and `init_spin` (from `spinms.data` records; `SpinFile::from_bytes` decodes the binary file, not re-checked against a real one), `UPHI` (IENTRY 1, 2, 3), Russian roulette and the stack/azimuth macros are a first port, **differentially tested only in part** (PEGS4, the HATCH tables and a 200-history `tutor1` run agree with a built EGSnrc: `docs/egsnrc_phase2_differential.md`); RANMAR/RANLUX stay in PETIR (`petir::egsnrc`); ~~everything else … `PHOTONUC` … is still a placeholder~~ **CORRECTED 2026-11-21** (`PHOTONUC`, `egs_init_user_photon`, PEGS4 intake, `hatch_media`, triplet, NRC pair, NIST/NRC brems and Rayleigh are ported; checked against `egsnrc_port_remaining.md` and the test names): still not ported are the plotting and file opening of `pegs4.mortran` (its option logic and functions are ported), the operating-system glue of `egs_utilities` (deliberately), `egs_compoundstoelements` (not in the clone), `rad_compton1` and `nrcaux`'s `WATCH` (diagnostic printing, deliberately) |
| `openmc_photon`: OpenMC photon samplers (`photon.cpp`), thick-target bremsstrahlung (`bremsstrahlung.cpp`) and `sample_photon/electron/positron_reaction`, `process_charged_secondary` (`physics.cpp`) (gh:#975, gh:#1683) | first port, ~~**not differentially tested**~~ **CORRECTED 2026-11-21:** differentially tested for U and H (`docs/openmc_photon_phase2.md`); AI draft; data passed in, no HDF5 reader; TTB and atomic relaxation on by default with `without_*` ablations |

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
  by the file name with `.` read as `_`. ~~Each module says what its routine
  family does and that nothing is ported. No stub functions.~~ **CORRECTED
  2026-11-27** — the scaffold has since been filled: the modules hold ported
  routines (see Status above and `docs/egsnrc_port_remaining.md`), and
  `estar_cpp.rs` ports `HEN_HOUSE/egs++/estar` (checked against `src/egsnrc/`
  and `NOTICE`).
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
(MIT) is the upstream of `decay_spectrum` and `openmc_photon`, and EGSnrc
(AGPL-3.0) of `egsnrc`; see `NOTICE`.
