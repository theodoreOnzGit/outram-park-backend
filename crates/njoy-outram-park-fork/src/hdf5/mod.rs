// SPDX-License-Identifier: GPL-3.0

//! **Read and write for the OpenMC interchange formats.** GitHub #270.
//!
//! Mostly HDF5, plus the two small XML files OpenMC ships beside its HDF5
//! libraries and cannot be used without: the `cross_sections.xml` library index
//! and the depletion chain. They live here because they are nuclear data and
//! the same maintainer direction applies to them.
//!
//! Maintainer direction: HDF5 belongs on the njoy side, for both reading and
//! writing, because it is nuclear data. `outram-mc-libs` stays data-free and
//! file-I/O-free in its inner transport loop and hands structured data to this
//! codec at the run boundary.
//!
//! Reading already existed in `crate::wmp::h5` and
//! `crate::nuclear_data::secondary`. **Writing did not exist anywhere in the
//! workspace** before this module.
//!
//! **Not every writer lives here, though.** `WMP_Library` write is
//! `crate::wmp::h5_write` (a private module; its method is [`crate::wmp::WindowedMultipole::write_h5`]), beside its reader: a reader and writer of one
//! format are the pair most likely to drift, and the round-trip test that stops
//! them drifting has to see both. The formats in this module had no
//! pre-existing reader, which is why their writers are here.

pub mod cross_sections_xml;
pub mod depletion_chain_xml;
pub mod mgxs_write;
pub mod nuclide_write;
pub mod statepoint_write;
mod xml_scan;
