// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors.
//
// Ported from OpenMC (https://github.com/openmc-dev/openmc), commit
// 8f79d9d02483c90e4134f03a9df4b75ce6b062bf: src/collision_track.cpp
// (should_record_event, collision_track_flush_bank, collision_track_record),
// include/openmc/settings.h (CollisionTrackConfig), src/settings.cpp (the
// <collision_track> reader), src/reaction.cpp (reaction_mt).
// Copyright (c) 2011-2026 Massachusetts Institute of Technology, UChicago
// Argonne LLC, and OpenMC contributors. MIT licence (LICENSE.openmc).

//! **Collision tracks**: one record per collision, OpenMC's
//! `<collision_track>` (gh:#1392).
//!
//! Upstream's `collision_track_record` runs after every `collision()` and
//! appends a `CollisionTrackSite` (position, outgoing direction, incident
//! energy and the energy lost, time, weight, MT, cell, material, universe,
//! nuclide, collision count, particle id and progeny count) to a capped bank
//! when the event passes the user's filters. At the end of each batch
//! `collision_track_flush_bank` writes the bank to `collision_track.h5` (or
//! `.mcpl`) once it is full or the run is over.
//!
//! # How it attaches, and why it changes nothing
//!
//! [`CollisionTrackBank`] rides in [`crate::physics::transport_csg::ExtraScoring`]
//! ([`ExtraScoring::with_collision_track`](crate::physics::transport_csg::ExtraScoring::with_collision_track)),
//! so it is reached by [`crate::physics::transport_csg::CsgPowerIteration::step_scored`]
//! and `step_par`. The CSG history kernel hands it one plain record per
//! collision. **It is off by default** (no bank, nothing recorded) and
//! **draws no random number** whether on or off, so every history, `k` and
//! bit-identity pin is the same with it (pinned by
//! `tests/collision_track_output.rs`). The transport loop opens no file: the
//! bank is turned into bytes at the run boundary by
//! [`write_collision_track_file`], through `njoy_outram_park_fork::hdf5::collision_track`
//! (HDF5, OpenMC's layout) or [`crate::source::mcpl`].
//!
//! # Batches: `discard` and `flush`
//!
//! Upstream records only in active batches (`current_batch > n_inactive`).
//! Here the kernel does not know the batch, so records of the generation in
//! progress are held apart and follow `ExtraScoring`'s own contract: an
//! active generation's `flush()` keeps them, an inactive one's `discard()`
//! drops them. The cap is applied in recording order either way, so the bank
//! holds exactly upstream's first `max_collisions` active records.
//!
//! # Where the record differs from upstream's, and why
//!
//! This crate's kernel is not OpenMC's draw for draw, so a record cannot be
//! compared with OpenMC's history by history (that needs the shared data and
//! draw order listed on gh:#1386). The fields are filled as follows:
//!
//! - **`event_mt`**: 18 for fission (OpenMC writes the sampled fission
//!   reaction's MT, 18 on these evaluations), 101 (`N_DISAPPEAR`) for capture,
//!   2 for elastic (free-gas, resonance or S(α,β), as upstream), the level's
//!   own MT for a discrete inelastic level, 91 for the continuum, 16 / 17 / 5
//!   for (n,2n) / (n,3n) / MT=5, and the channel's MT for the other
//!   neutron-emitting reactions.
//! - **`wgt`**: after the collision, so 0 for an analog capture or fission,
//!   as upstream's `p.wgt() = 0`.
//! - **`n_collision`**: collisions since the **history** began. Upstream gives
//!   a secondary its parent's count at the time it was banked; here a
//!   secondary continues from where the history's count stands when it is
//!   popped. Identical for every history without (n,xn) secondaries or splits.
//! - **`progeny_id`**: fission sites banked plus secondaries and split
//!   daughters created so far in the history (upstream's `n_progeny`).
//! - **`parent_id`**: the history's 1-based index in its generation, which is
//!   OpenMC's particle id on one rank (`compute_particle_id`).
//! - **Not recorded**: collisions inside a delta-tracked region (extras are
//!   not scored there, as for every `ExtraScoring` tally), and a collision
//!   whose particle is rouletted by survival biasing before its reaction is
//!   drawn.

use crate::material::nuclide::Nuclide;
pub use njoy_outram_park_fork::hdf5::collision_track::CollisionTrackSite;

/// `CollisionTrackConfig` (`include/openmc/settings.h`).
///
/// Empty filter lists match everything, as upstream's empty sets do.
#[derive(Debug, Clone, PartialEq)]
pub struct CollisionTrackConfig {
    /// Write MCPL rather than HDF5.
    pub mcpl_write: bool,
    /// Cell ids to record in.
    pub cell_ids: Vec<i32>,
    /// MT numbers to record.
    pub mt_numbers: Vec<i32>,
    /// Universe ids to record in.
    pub universe_ids: Vec<i32>,
    /// Material ids to record in.
    pub material_ids: Vec<i32>,
    /// Nuclide names (`"U235"`) to record.
    pub nuclides: Vec<String>,
    /// Record only collisions depositing more than this \[eV\]; 0 records all.
    pub deposited_energy_threshold: f64,
    /// Records per file (upstream's default 1000).
    pub max_collisions: usize,
    /// Files to write (upstream's default 1).
    pub max_files: i64,
}

impl Default for CollisionTrackConfig {
    fn default() -> Self {
        Self {
            mcpl_write: false,
            cell_ids: Vec::new(),
            mt_numbers: Vec::new(),
            universe_ids: Vec::new(),
            material_ids: Vec::new(),
            nuclides: Vec::new(),
            deposited_energy_threshold: 0.0,
            max_collisions: 1000,
            max_files: 1,
        }
    }
}

/// `reaction_mt` (`src/reaction.cpp:409`): a reaction name
/// (`"(n,fission)"`, `"elastic"`, …) or an MT number as text.
pub fn reaction_mt(name: &str) -> Result<i32, String> {
    if let Some(mt) = crate::plotter::endf_tables::reaction_mt(name) {
        return Ok(mt);
    }
    match name.trim().parse::<i32>() {
        Ok(mt) if mt >= 1 => Ok(mt),
        _ => Err(format!("Unknown reaction name \"{name}\".")),
    }
}

impl CollisionTrackConfig {
    /// The `reactions` entry, as `settings.cpp` reads it: names or MT
    /// numbers, each through [`reaction_mt`].
    pub fn with_reactions(mut self, names: &[&str]) -> Result<Self, String> {
        for n in names {
            let mt = reaction_mt(n)?;
            if !self.mt_numbers.contains(&mt) {
                self.mt_numbers.push(mt);
            }
        }
        Ok(self)
    }
}

/// `should_record_event` (`src/collision_track.cpp`). `active` is upstream's
/// `current_batch > n_inactive`; `bank_full` its `collision_track_bank.full()`.
#[allow(clippy::too_many_arguments)]
pub fn should_record_event(
    cfg: &CollisionTrackConfig,
    active: bool,
    bank_full: bool,
    cell_id: i32,
    mt_event: i32,
    nuclide: &str,
    universe_id: i32,
    material_id: i32,
    energy_loss: f64,
) -> bool {
    let ids = |set: &[i32], v: i32| set.is_empty() || set.contains(&v);
    active
        && !bank_full
        && ids(&cfg.cell_ids, cell_id)
        && ids(&cfg.mt_numbers, mt_event)
        && ids(&cfg.universe_ids, universe_id)
        && ids(&cfg.material_ids, material_id)
        && (cfg.nuclides.is_empty() || cfg.nuclides.iter().any(|n| n == nuclide))
        && (cfg.deposited_energy_threshold == 0.0 || cfg.deposited_energy_threshold < energy_loss)
}

/// What the CSG kernel knows at one collision, in this crate's terms. Built
/// only when a bank is attached; costs nothing otherwise.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct CollisionRecord {
    pub r: [f64; 3],
    pub u: [f64; 3],
    pub e_last: f64,
    pub e: f64,
    pub time: f64,
    pub wgt: f64,
    pub event_mt: i32,
    pub delayed_group: Option<usize>,
    pub cell_id: i32,
    pub universe_id: i32,
    pub material_id: i32,
    pub nuclide_idx: usize,
    pub n_collision: u64,
    pub progeny: u64,
}

/// One file's worth of records, from [`CollisionTrackBank::flush_bank`].
#[derive(Debug, Clone, PartialEq)]
pub struct CollisionTrackFile {
    /// Upstream's file name, `collision_track[.N].h5|mcpl`, relative to the
    /// output directory.
    pub filename: String,
    /// The records.
    pub sites: Vec<CollisionTrackSite>,
    /// Write as MCPL.
    pub mcpl: bool,
}

/// The collision-track bank (`simulation::collision_track_bank`) and its
/// file counter (`simulation::ct_current_file`).
#[derive(Debug, Clone, PartialEq)]
pub struct CollisionTrackBank {
    config: CollisionTrackConfig,
    sites: Vec<CollisionTrackSite>,
    pending: Vec<CollisionTrackSite>,
    current_file: i64,
    history_id: i64,
    /// `(name, PDG number)` per nuclide index, filled on first use.
    nuclide_ids: Vec<Option<(String, i32)>>,
}

impl CollisionTrackBank {
    /// An empty bank recording under `config`.
    pub fn new(config: CollisionTrackConfig) -> Self {
        Self {
            config,
            sites: Vec::new(),
            pending: Vec::new(),
            current_file: 1,
            history_id: 0,
            nuclide_ids: Vec::new(),
        }
    }

    /// The configuration.
    pub fn config(&self) -> &CollisionTrackConfig {
        &self.config
    }

    /// Records kept so far (active generations, not yet written).
    pub fn sites(&self) -> &[CollisionTrackSite] {
        &self.sites
    }

    /// Records of the generation in progress, not yet flushed or discarded.
    pub fn pending(&self) -> &[CollisionTrackSite] {
        &self.pending
    }

    /// `collision_track_bank.full()`, counting the generation in progress.
    pub fn is_full(&self) -> bool {
        self.sites.len() + self.pending.len() >= self.config.max_collisions
    }

    /// The particle id the next records carry (1-based in its generation).
    pub(crate) fn begin_history(&mut self, id: i64) {
        self.history_id = id;
    }

    /// `(name, PDG)` of nuclide `idx`: `1000000000 + 10000 Z + 10 A + m`
    /// (`ParticleType(Z, A, m)`), 0 when the name is not a GNDS nuclide.
    fn nuclide_id(&mut self, idx: usize, nuclides: &[Nuclide]) -> (String, i32) {
        if self.nuclide_ids.len() <= idx {
            self.nuclide_ids.resize(idx + 1, None);
        }
        if self.nuclide_ids[idx].is_none() {
            let name = nuclides.get(idx).map(|n| n.name.clone()).unwrap_or_default();
            let pdg = crate::plotter::endf_tables::zam(&name).map_or(0, |(z, a, m)| {
                1_000_000_000 + 10_000 * z as i32 + 10 * a as i32 + m as i32
            });
            self.nuclide_ids[idx] = Some((name, pdg));
        }
        self.nuclide_ids[idx].clone().unwrap_or_default()
    }

    /// `collision_track_record`: filter one collision and keep it.
    pub(crate) fn record(&mut self, c: CollisionRecord, nuclides: &[Nuclide]) {
        let full = self.is_full();
        if full {
            return;
        }
        let (name, nuclide_id) = self.nuclide_id(c.nuclide_idx, nuclides);
        let de = c.e_last - c.e;
        if !should_record_event(
            &self.config,
            true,
            full,
            c.cell_id,
            c.event_mt,
            &name,
            c.universe_id,
            c.material_id,
            de,
        ) {
            return;
        }
        self.pending.push(CollisionTrackSite {
            r: c.r,
            u: c.u,
            e: c.e_last,
            de,
            time: c.time,
            wgt: c.wgt,
            event_mt: c.event_mt,
            delayed_group: c.delayed_group.map_or(0, |g| g as i32 + 1),
            cell_id: c.cell_id,
            nuclide_id,
            material_id: c.material_id,
            universe_id: c.universe_id,
            n_collision: c.n_collision as i32,
            particle: 2112,
            parent_id: self.history_id,
            progeny_id: c.progeny as i64,
        });
    }

    /// An active generation ends: keep its records.
    pub fn flush_generation(&mut self) {
        let room = self.config.max_collisions.saturating_sub(self.sites.len());
        self.pending.truncate(room);
        self.sites.append(&mut self.pending);
    }

    /// An inactive generation ends: drop its records.
    pub fn discard_generation(&mut self) {
        self.pending.clear();
    }

    /// Add a parallel chunk's records after this bank's, in chunk order.
    pub(crate) fn absorb(&mut self, chunk: &Self) {
        let room = self.config.max_collisions.saturating_sub(self.sites.len() + self.pending.len());
        self.pending.extend(chunk.pending.iter().take(room).copied());
    }

    /// An empty bank with the same configuration, for one parallel chunk.
    /// Its `sites` stand in for this bank's so the cap is applied as if the
    /// chunk recorded after everything already kept.
    pub(crate) fn scratch(&self) -> Self {
        Self {
            config: CollisionTrackConfig {
                max_collisions: self
                    .config
                    .max_collisions
                    .saturating_sub(self.sites.len() + self.pending.len()),
                ..self.config.clone()
            },
            sites: Vec::new(),
            pending: Vec::new(),
            current_file: self.current_file,
            history_id: 0,
            nuclide_ids: self.nuclide_ids.clone(),
        }
    }

    /// `collision_track_flush_bank`, called at the end of every batch: the
    /// file to write now, if any. `last_batch` is upstream's
    /// `current_batch == n_batches`.
    pub fn flush_bank(&mut self, last_batch: bool) -> Option<CollisionTrackFile> {
        let cfg = &self.config;
        if self.current_file > cfg.max_files {
            return None;
        }
        let full = self.sites.len() >= cfg.max_collisions;
        if !full && !last_batch {
            return None;
        }
        if self.sites.is_empty() && !last_batch {
            return None;
        }
        let ext = if cfg.mcpl_write { "mcpl" } else { "h5" };
        let filename = if cfg.max_files == 1 || (self.current_file == 1 && last_batch) {
            format!("collision_track.{ext}")
        } else {
            format!("collision_track.{}.{ext}", self.current_file)
        };
        let file = CollisionTrackFile {
            filename,
            sites: std::mem::take(&mut self.sites),
            mcpl: cfg.mcpl_write,
        };
        self.current_file += 1;
        Some(file)
    }
}

/// Write one [`CollisionTrackFile`] under `path_output` (upstream's
/// `settings::path_output`, a directory prefix ending in `/`, or empty).
/// Run-boundary I/O; returns the path written.
pub fn write_collision_track_file(
    path_output: &str,
    file: &CollisionTrackFile,
) -> Result<String, String> {
    let name = format!("{path_output}{}", file.filename);
    if file.mcpl {
        let f = crate::source::mcpl::write_mcpl_collision_track(&file.sites);
        crate::source::mcpl::write_mcpl_file(&name, &f).map_err(|e| e.to_string())
    } else {
        njoy_outram_park_fork::hdf5::collision_track::write_collision_track_h5(&name, &file.sites)
            .map(|(p, _)| p)
            .map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(mt: i32, e_last: f64, e: f64) -> CollisionRecord {
        CollisionRecord {
            r: [0.0; 3],
            u: [0.0, 0.0, 1.0],
            e_last,
            e,
            time: 0.0,
            wgt: 1.0,
            event_mt: mt,
            delayed_group: Some(2),
            cell_id: 22,
            universe_id: 77,
            material_id: 1,
            nuclide_idx: 0,
            n_collision: 1,
            progeny: 0,
        }
    }

    #[test]
    fn filters_follow_should_record_event() {
        let cfg = CollisionTrackConfig::default().with_reactions(&["(n,fission)", "101"]).unwrap();
        assert_eq!(cfg.mt_numbers, vec![18, 101]);
        assert!(reaction_mt("(n,nonsense)").is_err());
        assert!(reaction_mt("0").is_err());
        assert!(should_record_event(&cfg, true, false, 1, 18, "U235", 0, 0, 0.0));
        assert!(!should_record_event(&cfg, true, false, 1, 2, "U235", 0, 0, 0.0));
        assert!(!should_record_event(&cfg, false, false, 1, 18, "U235", 0, 0, 0.0));
        assert!(!should_record_event(&cfg, true, true, 1, 18, "U235", 0, 0, 0.0));
        let cfg = CollisionTrackConfig {
            nuclides: vec!["U238".into()],
            deposited_energy_threshold: 5.0,
            ..Default::default()
        };
        assert!(should_record_event(&cfg, true, false, 1, 2, "U238", 0, 0, 6.0));
        assert!(!should_record_event(&cfg, true, false, 1, 2, "U238", 0, 0, 5.0));
        assert!(!should_record_event(&cfg, true, false, 1, 2, "U235", 0, 0, 6.0));
    }

    #[test]
    fn the_bank_caps_discards_inactive_and_names_files_as_upstream() {
        let nucs: Vec<Nuclide> = Vec::new();
        let mut b = CollisionTrackBank::new(CollisionTrackConfig { max_collisions: 3, max_files: 2, ..Default::default() });
        b.begin_history(7);
        b.record(rec(2, 2.0e6, 1.5e6), &nucs);
        b.discard_generation();
        assert!(b.sites().is_empty());
        for _ in 0..5 {
            b.record(rec(2, 2.0e6, 1.5e6), &nucs);
        }
        b.flush_generation();
        assert_eq!(b.sites().len(), 3);
        let s = b.sites()[0];
        assert_eq!((s.de, s.delayed_group, s.parent_id, s.particle, s.nuclide_id), (5.0e5, 3, 7, 2112, 0));
        let f = b.flush_bank(false).unwrap();
        assert_eq!(f.filename, "collision_track.1.h5");
        assert!(b.flush_bank(false).is_none());
        b.record(rec(2, 1.0, 0.5), &nucs);
        b.flush_generation();
        assert_eq!(b.flush_bank(true).unwrap().filename, "collision_track.2.h5");
        assert!(b.flush_bank(true).is_none(), "max_files reached");

        let mut one = CollisionTrackBank::new(CollisionTrackConfig { mcpl_write: true, ..Default::default() });
        assert_eq!(one.flush_bank(true).unwrap().filename, "collision_track.mcpl");
    }

    #[test]
    fn scratch_and_absorb_keep_recording_order_and_cap() {
        let nucs: Vec<Nuclide> = Vec::new();
        let mut b = CollisionTrackBank::new(CollisionTrackConfig { max_collisions: 4, ..Default::default() });
        b.record(rec(2, 1.0, 0.0), &nucs);
        let mut c1 = b.scratch();
        let mut c2 = b.scratch();
        for _ in 0..2 {
            c1.record(rec(51, 1.0, 0.0), &nucs);
        }
        for _ in 0..3 {
            c2.record(rec(91, 1.0, 0.0), &nucs);
        }
        b.absorb(&c1);
        b.absorb(&c2);
        b.flush_generation();
        let mts: Vec<i32> = b.sites().iter().map(|s| s.event_mt).collect();
        assert_eq!(mts, vec![2, 51, 51, 91]);
    }
}
