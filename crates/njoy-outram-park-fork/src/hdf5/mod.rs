// SPDX-License-Identifier: GPL-3.0

//! **HDF5 read and write for the OpenMC interchange formats.** GitHub #270.
//!
//! Maintainer direction: HDF5 belongs on the njoy side, for both reading and
//! writing, because it is nuclear data. `outram-mc-libs` stays data-free and
//! file-I/O-free in its inner transport loop and hands structured data to this
//! codec at the run boundary.
//!
//! Reading already existed in `crate::wmp::h5` and
//! `crate::nuclear_data::secondary`. **Writing did not exist anywhere in the
//! workspace** before this module.

pub mod mgxs_write;
