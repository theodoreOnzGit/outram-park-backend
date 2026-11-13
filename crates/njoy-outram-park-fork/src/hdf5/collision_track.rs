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

//! **`collision_track.h5`: OpenMC's per-collision record file**, written and
//! read in OpenMC's own HDF5 layout (gh:#1392; the HDF5 codec lives here by
//! #270's division of labour).
//!
//! `outram-mc-libs` collects the records in memory during transport
//! (`physics::collision_track`) and hands them here at the run boundary as
//! plain [`CollisionTrackSite`]s. This module turns them into bytes.
//!
//! # The layout is OpenMC's
//!
//! One root dataset, `collision_track_bank`, of a compound type built exactly
//! as `h5_collision_track_banktype()` builds it (`src/collision_track.cpp`):
//! the C struct `CollisionTrackSite` (`include/openmc/particle_data.h`), 128
//! bytes, with the two nested `{x, y, z}` compounds `r` and `u` and every
//! member at its C offset ([`collision_track_datatype`]). Two root
//! attributes: `filetype = "collision_track"` and `version = [1, 2]`
//! (`VERSION_COLLISION_TRACK`).
//!
//! **One deliberate difference, of width only:** upstream writes `version` as
//! two 32-bit integers; `hdf5-pure` 0.20.1 has no 32-bit integer array
//! attribute, so it is written as two 64-bit integers with the same values.
//! OpenMC's reader (`openmc.read_collision_track_hdf5`) reads only the
//! dataset, and numpy compares the two widths equal.
//!
//! # Verified against independent readers
//!
//! Upstream's own regression fixtures (`tests/regression_tests/collision_track`,
//! files written by the C HDF5 library) are decoded by [`decode_collision_track_h5`]
//! and re-encoded by [`encode_collision_track_h5`] to the same records bit for
//! bit (unit tests below). The written file is read back by
//! `openmc.read_collision_track_hdf5` itself: see
//! `crates/outram-mc-libs/verification_and_validation/collision_track_mcpl/`.

use std::path::Path;

use hdf5_pure::{
    make_f64_type, make_i32_type, make_i64_type, AttrValue, CompoundTypeBuilder, Datatype,
    File, FileBuilder,
};

use crate::error::NjoyError;

/// `VERSION_COLLISION_TRACK` (`include/openmc/constants.h:38`).
pub const VERSION_COLLISION_TRACK: [i64; 2] = [1, 2];

/// `sizeof(CollisionTrackSite)` on the platforms OpenMC builds on: three
/// `f64` triples' worth of `Position`/`Direction` (48), four `f64` (32),
/// eight `int` (32), two `int64_t` (16).
pub const COLLISION_TRACK_SITE_SIZE: u32 = 128;

/// The member names and C offsets of `CollisionTrackSite`, in declaration
/// order: `(name, byte offset, kind)`. `kind` is `'p'` for a nested
/// `{x, y, z}` position, `'d'` for `double`, `'i'` for `int`, `'l'` for
/// `int64_t`. Shared by the writer's datatype and the reader's checks so the
/// two cannot drift.
const MEMBERS: [(&str, u64, char); 16] = [
    ("r", 0, 'p'),
    ("u", 24, 'p'),
    ("E", 48, 'd'),
    ("dE", 56, 'd'),
    ("time", 64, 'd'),
    ("wgt", 72, 'd'),
    ("event_mt", 80, 'i'),
    ("delayed_group", 84, 'i'),
    ("cell_id", 88, 'i'),
    ("nuclide_id", 92, 'i'),
    ("material_id", 96, 'i'),
    ("universe_id", 100, 'i'),
    ("n_collision", 104, 'i'),
    ("particle", 108, 'i'),
    ("parent_id", 112, 'l'),
    ("progeny_id", 120, 'l'),
];

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

/// `h5_collision_track_banktype()` (`src/collision_track.cpp`): the 128-byte
/// compound with nested `r` and `u`, every member at its C offset.
pub fn collision_track_datatype() -> Result<Datatype, NjoyError> {
    let pos = CompoundTypeBuilder::new()
        .f64_field("x")
        .f64_field("y")
        .f64_field("z")
        .build();
    let mut b = CompoundTypeBuilder::with_size(COLLISION_TRACK_SITE_SIZE);
    for (name, off, kind) in MEMBERS {
        let dt = match kind {
            'p' => pos.clone(),
            'd' => make_f64_type(),
            'i' => make_i32_type(),
            _ => make_i64_type(),
        };
        b = b.field(name, off, dt);
    }
    b.build()
        .map_err(|e| NjoyError::Hdf5(format!("collision-track datatype: {e:?}")))
}

/// The little-endian bytes of `sites` in the C struct layout.
pub fn collision_track_bytes(sites: &[CollisionTrackSite]) -> Vec<u8> {
    let mut raw = Vec::with_capacity(sites.len() * COLLISION_TRACK_SITE_SIZE as usize);
    for s in sites {
        for v in s.r.iter().chain(&s.u).chain(&[s.e, s.de, s.time, s.wgt]) {
            raw.extend_from_slice(&v.to_le_bytes());
        }
        for v in [
            s.event_mt,
            s.delayed_group,
            s.cell_id,
            s.nuclide_id,
            s.material_id,
            s.universe_id,
            s.n_collision,
            s.particle,
        ] {
            raw.extend_from_slice(&v.to_le_bytes());
        }
        raw.extend_from_slice(&s.parent_id.to_le_bytes());
        raw.extend_from_slice(&s.progeny_id.to_le_bytes());
    }
    raw
}

/// `write_h5_collision_track`'s file, in memory: the attributes and the one
/// dataset. An empty bank writes a zero-length dataset, as upstream does on
/// a last batch that recorded nothing.
pub fn encode_collision_track_h5(sites: &[CollisionTrackSite]) -> Result<Vec<u8>, NjoyError> {
    let mut b = FileBuilder::new();
    b.set_attr("filetype", AttrValue::AsciiString("collision_track".into()));
    b.set_attr("version", AttrValue::I64Array(VERSION_COLLISION_TRACK.to_vec()));
    b.create_dataset("collision_track_bank").with_compound_data(
        collision_track_datatype()?,
        collision_track_bytes(sites),
        sites.len() as u64,
    );
    b.finish()
        .map_err(|e| NjoyError::Hdf5(format!("writing collision track: {e}")))
}

/// Upstream's file-name rule in `write_h5_collision_track`: no extension
/// gets `.h5` appended; a different extension is kept and reported (upstream
/// warns and writes HDF5 anyway). Returns `(name, warning)`.
pub fn collision_track_h5_filename(name: &str) -> Result<(String, Option<String>), NjoyError> {
    if name.is_empty() {
        return Err(NjoyError::Hdf5(
            "write_h5_collision_track filename needs a nonempty name.".into(),
        ));
    }
    // `get_file_extension` (`src/file_utils.cpp`): the text after the last
    // dot of the last path component.
    let base = name.rsplit('/').next().unwrap_or(name);
    match base.rfind('.') {
        None => Ok((format!("{name}.h5"), None)),
        Some(i) if &base[i + 1..] == "h5" => Ok((name.to_string(), None)),
        Some(_) => Ok((
            name.to_string(),
            Some(
                "write_h5_collision_track was passed a file extension differing from .h5, \
                 but an hdf5 file will be written."
                    .into(),
            ),
        )),
    }
}

/// Write a collision-track file (`write_h5_collision_track`). `name` follows
/// [`collision_track_h5_filename`]; returns the path written and upstream's
/// warning, if any.
pub fn write_collision_track_h5(
    name: &str,
    sites: &[CollisionTrackSite],
) -> Result<(String, Option<String>), NjoyError> {
    let (path, warning) = collision_track_h5_filename(name)?;
    let bytes = encode_collision_track_h5(sites)?;
    std::fs::write(Path::new(&path), bytes)
        .map_err(|e| NjoyError::Hdf5(format!("writing {path}: {e}")))?;
    Ok((path, warning))
}

/// Read `collision_track_bank` from a file's bytes:
/// `openmc.read_collision_track_hdf5`, for files written by this module **or by
/// OpenMC itself** (member offsets are taken from the file's own datatype and
/// checked against upstream's struct, so a file with another layout is refused
/// rather than misread).
pub fn decode_collision_track_h5(bytes: Vec<u8>) -> Result<Vec<CollisionTrackSite>, NjoyError> {
    let err = |what: String| NjoyError::Hdf5(format!("collision track: {what}"));
    let f = File::from_bytes(bytes).map_err(|e| err(format!("not an HDF5 file: {e}")))?;
    let attrs = f.root().attrs().map_err(|e| err(format!("root attributes: {e}")))?;
    match attrs.get("filetype") {
        Some(AttrValue::AsciiString(s)) | Some(AttrValue::String(s))
            if s.trim_end_matches('\0') == "collision_track" => {}
        other => return Err(err(format!("filetype is {other:?}, not \"collision_track\""))),
    }
    let ds = f
        .dataset("collision_track_bank")
        .map_err(|e| err(format!("no collision_track_bank dataset: {e}")))?;
    let dt = ds.datatype().map_err(|e| err(format!("datatype: {e}")))?;
    check_layout(&dt).map_err(err)?;
    let raw = ds.read_raw().map_err(|e| err(format!("reading the bank: {e}")))?;
    if raw.len() % COLLISION_TRACK_SITE_SIZE as usize != 0 {
        return Err(err(format!("{} bytes is not a whole number of records", raw.len())));
    }
    Ok(raw
        .chunks_exact(COLLISION_TRACK_SITE_SIZE as usize)
        .map(site_from_bytes)
        .collect())
}

/// Read a collision-track file from disk ([`decode_collision_track_h5`]).
pub fn read_collision_track_h5<P: AsRef<Path>>(
    path: P,
) -> Result<Vec<CollisionTrackSite>, NjoyError> {
    let bytes = std::fs::read(path.as_ref()).map_err(|e| {
        NjoyError::Hdf5(format!("reading {}: {e}", path.as_ref().display()))
    })?;
    decode_collision_track_h5(bytes)
}

/// Refuse any layout other than upstream's 128-byte struct.
fn check_layout(dt: &Datatype) -> Result<(), String> {
    let Datatype::Compound { size, members } = dt else {
        return Err("collision_track_bank is not a compound dataset".into());
    };
    if *size != COLLISION_TRACK_SITE_SIZE {
        return Err(format!("record size {size}, upstream's is {COLLISION_TRACK_SITE_SIZE}"));
    }
    for (name, off, kind) in MEMBERS {
        let m = members
            .iter()
            .find(|m| m.name == name)
            .ok_or_else(|| format!("member {name} missing"))?;
        let want = match kind {
            'p' => 24,
            'i' => 4,
            _ => 8,
        };
        if m.byte_offset != off || m.datatype.type_size() != want {
            return Err(format!(
                "member {name} at offset {} size {}, upstream's is {off} size {want}",
                m.byte_offset,
                m.datatype.type_size()
            ));
        }
    }
    Ok(())
}

/// One record from its 128 little-endian bytes.
fn site_from_bytes(b: &[u8]) -> CollisionTrackSite {
    let d = |o: usize| f64::from_le_bytes(b[o..o + 8].try_into().unwrap_or([0; 8]));
    let i = |o: usize| i32::from_le_bytes(b[o..o + 4].try_into().unwrap_or([0; 4]));
    let l = |o: usize| i64::from_le_bytes(b[o..o + 8].try_into().unwrap_or([0; 8]));
    CollisionTrackSite {
        r: [d(0), d(8), d(16)],
        u: [d(24), d(32), d(40)],
        e: d(48),
        de: d(56),
        time: d(64),
        wgt: d(72),
        event_mt: i(80),
        delayed_group: i(84),
        cell_id: i(88),
        nuclide_id: i(92),
        material_id: i(96),
        universe_id: i(100),
        n_collision: i(104),
        particle: i(108),
        parent_id: l(112),
        progeny_id: l(120),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Upstream's fixture `collision_track/case_1_Reactions/collision_track_true.h5`
    /// (OpenMC `8f79d9d0`, MIT), written by the C HDF5 library.
    const CASE_1: &[u8] =
        include_bytes!("../../tests/data/openmc_collision_track/case_1_reactions_true.h5");
    /// `case_7_all_parameters_used_together`.
    const CASE_7: &[u8] =
        include_bytes!("../../tests/data/openmc_collision_track/case_7_all_parameters_true.h5");

    /// **Upstream's own file decodes**, with the values `h5dump` prints for it
    /// (first record: fission, MT 18, on U238 = PDG 1000922380, in cell 22 of
    /// universe 77, material 1, neutron, parent 13, progeny 8).
    #[test]
    fn upstream_fixture_decodes_to_h5dumps_values() {
        let s = decode_collision_track_h5(CASE_1.to_vec()).unwrap();
        assert_eq!(s.len(), 2);
        let a = s[0];
        assert_eq!(
            (a.event_mt, a.cell_id, a.nuclide_id, a.material_id, a.universe_id),
            (18, 22, 1000922380, 1, 77)
        );
        assert_eq!((a.n_collision, a.particle, a.parent_id, a.progeny_id), (2, 2112, 13, 8));
        assert_eq!((a.de, a.wgt), (0.0, 0.0), "an absorbed neutron keeps E and has weight 0");
        assert!((a.e - 2.78462e6).abs() < 10.0, "E = {}", a.e);
        assert_eq!(s[1].nuclide_id, 1000922350);
        assert_eq!(s[1].parent_id, 62);
    }

    /// **Decode, encode, decode is the identity, bit for bit**, on both
    /// upstream fixtures: the writer emits upstream's layout and the reader
    /// accepts it from either source.
    #[test]
    fn upstream_fixtures_round_trip_bit_for_bit() {
        for fixture in [CASE_1, CASE_7] {
            let a = decode_collision_track_h5(fixture.to_vec()).unwrap();
            let bytes = encode_collision_track_h5(&a).unwrap();
            let b = decode_collision_track_h5(bytes).unwrap();
            assert_eq!(collision_track_bytes(&a), collision_track_bytes(&b));
            assert!(!a.is_empty());
        }
    }

    /// The datatype is upstream's: 128 bytes, members at the C offsets.
    #[test]
    fn the_datatype_is_upstreams_struct() {
        let dt = collision_track_datatype().unwrap();
        check_layout(&dt).unwrap();
        let Datatype::Compound { members, .. } = &dt else { unreachable!() };
        assert_eq!(members.len(), 16);
        assert_eq!(collision_track_bytes(&[site()]).len(), 128);
    }

    fn site() -> CollisionTrackSite {
        CollisionTrackSite {
            r: [1.0, -2.0, 3.5],
            u: [0.0, 0.6, 0.8],
            e: 2.0e6,
            de: 1.5e5,
            time: 1.0e-8,
            wgt: 1.0,
            event_mt: 51,
            delayed_group: 0,
            cell_id: 22,
            nuclide_id: 1000922380,
            material_id: 1,
            universe_id: 77,
            n_collision: 3,
            particle: 2112,
            parent_id: i64::MAX - 1,
            progeny_id: 4,
        }
    }

    /// A file written to disk reads back, an empty bank writes, and a
    /// non-collision-track file is refused rather than misread.
    #[test]
    fn files_write_read_and_wrong_files_are_refused() {
        let d = std::env::temp_dir().join("njoy_collision_track_test");
        std::fs::create_dir_all(&d).unwrap();
        let name = d.join("collision_track");
        let (path, warn) =
            write_collision_track_h5(name.to_str().unwrap(), &[site(), site()]).unwrap();
        assert!(path.ends_with("collision_track.h5") && warn.is_none());
        assert_eq!(read_collision_track_h5(&path).unwrap(), vec![site(), site()]);
        assert!(decode_collision_track_h5(encode_collision_track_h5(&[]).unwrap())
            .unwrap()
            .is_empty());
        std::fs::remove_dir_all(&d).ok();

        let mut b = FileBuilder::new();
        b.set_attr("filetype", AttrValue::AsciiString("source".into()));
        let err = decode_collision_track_h5(b.finish().unwrap()).unwrap_err();
        assert!(format!("{err}").contains("not \"collision_track\""), "{err}");
    }

    /// Upstream's file-name rule.
    #[test]
    fn file_names_follow_upstream() {
        assert_eq!(collision_track_h5_filename("a/ct").unwrap().0, "a/ct.h5");
        assert_eq!(collision_track_h5_filename("a.b/ct.h5").unwrap(), ("a.b/ct.h5".into(), None));
        assert!(collision_track_h5_filename("ct.dat").unwrap().1.is_some());
        assert!(collision_track_h5_filename("").is_err());
    }
}
