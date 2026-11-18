// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors.
//
// Ported from OpenMC (https://github.com/openmc-dev/openmc), commit
// 8f79d9d02483c90e4134f03a9df4b75ce6b062bf:
//   src/collision_track.cpp  (h5_collision_track_banktype,
//                             write_h5_collision_track)
//   include/openmc/particle_data.h  (struct CollisionTrackSite)
//   include/openmc/constants.h      (VERSION_COLLISION_TRACK)
//   openmc/source.py                (read_collision_track_hdf5)
// Copyright (c) 2011-2026 Massachusetts Institute of Technology, UChicago
// Argonne LLC, and OpenMC contributors. MIT licence:
//
//   Permission is hereby granted, free of charge, to any person obtaining a
//   copy of this software and associated documentation files (the
//   "Software"), to deal in the Software without restriction, including
//   without limitation the rights to use, copy, modify, merge, publish,
//   distribute, sublicense, and/or sell copies of the Software, and to permit
//   persons to whom the Software is furnished to do so, subject to the
//   following conditions: The above copyright notice and this permission
//   notice shall be included in all copies or substantial portions of the
//   Software. THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND.

//! **`CollisionTrackSite`**, OpenMC's per-collision record, as plain data.
//!
//! Moved out of [`crate::hdf5::collision_track`] (gh:#1605) so it builds
//! without `std`: `outram-mc-libs` records collisions during transport in
//! every build, and only writing them to `collision_track.h5` needs the HDF5
//! codec. `crate::hdf5::collision_track::CollisionTrackSite` still names it.

/// One collision record, `struct CollisionTrackSite`
/// (`include/openmc/particle_data.h:61`), as plain data.
///
/// Field meanings are upstream's (`collision_track_record`,
/// `src/collision_track.cpp`): `e` is the energy **before** the collision
/// (`E_last`), `de = E_last - E` the energy it lost, `u` the direction and
/// `wgt` the weight **after** it; ids are user-facing ids, not indices;
/// `nuclide_id` and `particle` are PDG numbers (`1000000000 + 10000 Z + 10 A
/// + m` and 2112 for a neutron); `parent_id` is the particle's id and
/// `progeny_id` its `n_progeny` at the time of the collision.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CollisionTrackSite {
    /// Position \[cm\].
    pub r: [f64; 3],
    /// Direction after the collision.
    pub u: [f64; 3],
    /// Energy before the collision \[eV\].
    pub e: f64,
    /// Energy lost in the collision \[eV\].
    pub de: f64,
    /// Time \[s\].
    pub time: f64,
    /// Weight after the collision.
    pub wgt: f64,
    /// ENDF MT of the reaction.
    pub event_mt: i32,
    /// Delayed group (0 = prompt).
    pub delayed_group: i32,
    /// Cell id.
    pub cell_id: i32,
    /// Target nuclide's PDG number, 0 when there is none.
    pub nuclide_id: i32,
    /// Material id.
    pub material_id: i32,
    /// Universe id.
    pub universe_id: i32,
    /// Collisions so far.
    pub n_collision: i32,
    /// Particle PDG number.
    pub particle: i32,
    /// The particle's id.
    pub parent_id: i64,
    /// The particle's progeny count.
    pub progeny_id: i64,
}
