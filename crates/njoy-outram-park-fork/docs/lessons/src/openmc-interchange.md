# Extended: OpenMC interchange (HDF5 and XML)

> **Research, education and V&V only.** Not for reactor operation, licensing,
> safety decisions or emergency response.

> **Review status: first draft, 2026-10-04, AI-assisted, not yet reviewed by a
> human.**

*Part II, extended deep dive. Module: `hdf5` (plus the WMP writer in `wmp`).*

## The question

OpenMC [(Romano et al., 2015)](#ref-romano2015openmc) does not read ACE at run time; it reads its own **HDF5** libraries,
converted from ACE by its Python API, plus two small XML files. **Can this
workspace write and read those files directly, so either code can use the
other's data?**

## The shortest answer

Yes, for the formats below, by a maintainer decision recorded in the module
doc: HDF5 is nuclear data, so its reading and writing live in this crate, and
`outram-mc-libs` stays file-I/O-free in its inner loop.

| format | file | read | write | module |
|---|---|---|---|---|
| incident-neutron nuclide | `U235.h5` | `hdf5::nuclide_read::read_nuclide` | `hdf5::nuclide_write::write_nuclide` | `nuclide_read`, `nuclide_write`, `nuclide_laws` |
| windowed multipole | `092238.h5` | `wmp::WindowedMultipole::load_h5` (`wmp-hdf5`) | `WindowedMultipole::write_h5` | `wmp::h5`, `wmp::h5_write` |
| multigroup cross sections | `mgxs.h5` | — | `hdf5::mgxs_write` | `mgxs_write` |
| statepoint | `statepoint.N.h5` | — | `hdf5::statepoint_write` | `statepoint_write` |
| library index | `cross_sections.xml` | yes | yes | `cross_sections_xml` |
| depletion chain | `chain.xml` | yes | — | `depletion_chain_xml`, `xml_scan` |

The HDF5 codec is `hdf5-pure`, a pure-Rust implementation, so no C HDF5
library is needed and the crate still builds for Android and wasm.

## In the code: writing a nuclide

> **On the [Code Review map](../../code-review/):** start at [`write_nuclide`](../../code-review/#crates/njoy-outram-park-fork/src/hdf5/nuclide_write.rs::write_nuclide). Each card opens the function's source and shows its review stamp.

> **The physics functions:** this page is format conversion and has none. See [Where the physics is](./physics-functions.md#openmc-interchange) (a proposal for maintainer review, 2026-10-14; the entry-point link above stays until it is accepted).

**Reading the path.** The table below is the path the Code Review map link above follows, with what each hop does:

| hop | function | what it does | resolved (kopitiam pass, 2026-10-04) |
|---|---|---|---|
| 0 | `hdf5::nuclide_write::write_nuclide` | a `NuclideData` → OpenMC's `IncidentNeutron.export_to_hdf5` layout | entry point ([source](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/njoy-outram-park-fork/src/hdf5/nuclide_write.rs#@@L:crates/njoy-outram-park-fork/src/hdf5/nuclide_write.rs:fn=write_nuclide@@)) |
| 1 | `skipped_by_upstream`, `nuclide_laws::reaction_label`, `as_str` | which reactions upstream writes; their HDF5 names | kopitiam |
| 1 | `nuclide_laws::…::write` | product distributions (`AngleDistribution`, `Tabulated1D`, energy laws) | kopitiam |


## The checks

All cross-code: **OpenMC itself** reads what this crate wrote, or this crate
reads what OpenMC wrote. The OpenMC decks are committed beside each record.

| what | result | record |
|---|---|---|
| synthetic nuclide `Syn1` (elastic + 1/v capture) written, read and transported by OpenMC | every field read back as written; flux 10.8256 ± 0.0044 cm per source in a one-mean-free-path sphere; found and fixed `kTs` written as an array where upstream writes a scalar | [`hdf5_nuclide_write/README.md`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/njoy-outram-park-fork/verification_and_validation/hdf5_nuclide_write/README.md) (2026-09-22) |
| fissile nuclide written and transported by OpenMC | row 1 met (U-235 `.h5` read and transported); **LCT-008 on the written library not yet run**, stated as blocked in the record | [`nuclide_h5_fissile/fissile_write_2026_09_24.md`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/njoy-outram-park-fork/verification_and_validation/nuclide_h5_fissile/fissile_write_2026_09_24.md) |
| reading OpenMC's own ENDF/B-VIII.0 library | reader verified against upstream's output; the downstream `k` comparison **not yet run** (stated) | [`nuclide_h5_read/nuclide_h5_read_2026_09_24.md`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/njoy-outram-park-fork/verification_and_validation/nuclide_h5_read/nuclide_h5_read_2026_09_24.md) |
| WMP write | round trip exact to the bit (U-238 and Fe-56); OpenMC reads it | [`wmp_h5_write/wmp_h5_write_2026_09_24.md`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/njoy-outram-park-fork/verification_and_validation/wmp_h5_write/wmp_h5_write_2026_09_24.md) |
| depletion chain read | checked against the hand transcription it replaced | [`depletion_chain_xml/chain_xml_read_2026_09_23.md`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/njoy-outram-park-fork/verification_and_validation/depletion_chain_xml/chain_xml_read_2026_09_23.md) |
| MGXS write read by OpenMC | `tests/mgxs_write_read_by_openmc.rs` | test doc |

**A number worth knowing, and how not to read it.** The nuclide-read record
measured, on an 11-nuclide simplified LCT-008: OpenMC loading its HDF5
library took 1.54 ± 0.06 s; `outram-mc` reconstructing from ENDF tapes took
158.57 ± 10.80 s. The record itself warns this is **not** a slowdown: one side
deserialises a pre-built table, the other runs RECONR, BROADR and PURR. It is
why the reader was written.

**Lesson from the fissile record:** a writer verified only on a two-reaction
synthetic nuclide could not see three defects (an MT restriction, the
threshold convention, the zero-at-threshold invariant) that the first real
fissile nuclide exposed. A check built around what a component *does* is not a
check of what it is *for*.

**Next:** [Windowed multipole and the GPU path](./wmp.md).

<!-- references:begin -->
## References

<p class="csl-entry" id="ref-romano2015openmc" style="padding-left: 2em; text-indent: -2em;">Romano, P. K., Horelik, N. E., Herman, B. R., Nelson, A. G., Forget, B., &#38; Smith, K. (2015). OpenMC: A state-of-the-art Monte Carlo code for research and development. <i>Annals of Nuclear Energy</i>, <i>82</i>, 90–97.</p>

<!-- references:end -->
