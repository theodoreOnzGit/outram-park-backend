# OpenMC photon port, phase 2: differential test against a built OpenMC (2026-11-21)

Status: **the three data-driven samplers agree bit for bit; the rest is still
"first port, not differentially tested".**

## Reference (scratch, nothing committed)

Built from source in `/tmp/claude-0/`: OpenMC `8f79d9d0` (`cmake -S <clone> -B
<build> -DOPENMC_BUILD_TESTS=OFF -DOPENMC_USE_OPENMP=OFF`, then `cmake --build
<build> --target libopenmc -j 2`; apt `libhdf5-dev libpugixml-dev libfmt-dev`);
NJOY2016 (clone `njoy/NJOY2016`, cmake) run on the real ENDF/B-VIII.0
uranium photo-atomic and atomic-relaxation evaluations from
`NJOY2016/tests/resources` (`njoy < tests/48/input`, tapes 20 and 21) as a
cross-check of the ACE route; OpenMC's Python `IncidentPhoton.from_endf` (with
`endf-python`, see `AGENTS.md`) writes the HDF5 photon library `U.h5`
(`verification_and_validation/openmc_photon/mk_h5.py`; OpenMC's own
`compton_profiles.h5`, `density_effect.h5` and `BREMX.DAT` come from its source
tree). `from_ace` was not usable: it rejects NJOY's photoatomic ACE (no subshell
data).

`verification_and_validation/openmc_photon/harness.cpp` links `libopenmc`,
loads `U.h5` into a real `openmc::PhotonInteraction`, dumps the members the
samplers read and runs the samplers on fixed seeds.

## Comparison

`tests/openmc_photon_vs_openmc.rs` (`OPENMC_REFERENCE_DUMP`,
`OPENMC_REFERENCE_SAMPLES`, skipped otherwise) builds a
`punggol::openmc_photon::PhotonInteraction` from the dump and runs
`compton_scatter`, `rayleigh_scatter` and `pair_production` on the same seeds.

Uranium (Z = 92), photon energies 1 keV to 100 MeV (8), 200 seeds each, Compton
with and without Doppler broadening: **5400 samples**.

| quantity | worst difference |
|---|---|
| `Tabulated1D` probes (incoherent and coherent form factors, 20 points) | 0 (exact) |
| `compton_scatter` outgoing alpha | 0 (relative) |
| `compton_scatter` mu | 0 (absolute) |
| `compton_scatter` shell index | 0 mismatches |
| `rayleigh_scatter` mu | 0 |
| `pair_production` (E-, E+, mu-, mu+) | 0 (relative) |
| final LCG seed after every call | 0 mismatches |

So the Rust samplers consume exactly the same random numbers in the same order
and give bit-identical results for this element.

## Not yet compared

`calculate_xs` (needs an `openmc::Particle`), `atomic_relaxation`,
`thick_target_bremsstrahlung`, `sample_photon_reaction`,
`sample_electron_reaction`, `sample_positron_reaction`,
`process_charged_secondary`, and other elements (one real element, U, so the
shell structure of light elements is untested). The data come from ENDF/B-VIII.0
U only; the C++ harness can be extended to dump and call these (gh:#1690).
