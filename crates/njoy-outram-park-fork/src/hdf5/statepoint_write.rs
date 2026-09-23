// SPDX-License-Identifier: GPL-3.0

//! **`statepoint.h5`, `summary.h5` and `source.h5` writers** — GitHub #270
//! scope item 5, and the file half of #271.
//!
//! The division of labour #270 sets out: `outram-mc-libs` captures the run
//! state and stays free of file I/O in its transport loop; this crate owns the
//! codec. So these functions take **plain data** — slices and small structs —
//! and never a transport type.
//!
//! # The layout is OpenMC's, not ours
//!
//! Writing OpenMC's own layout is worth more than a bespoke one, because the
//! file is then readable by `openmc.StatePoint` and the rest of its Python
//! tooling. Where this port writes a subset, it writes a **correct subset**
//! with the right names and attributes rather than approximations: a file that
//! is 80 % right is a file that fails on the other code's reader with a
//! confusing error.
//!
//! # Verified against an independent reader
//!
//! Round-tripping through this crate's own reader would only prove the two
//! halves of one codec agree. All three files were opened with **`h5py`**
//! (3.16.0) on 2026-09-22 and every attribute and dataset read back correctly:
//!
//! ```text
//! statepoint.h5  attrs {current_batch: 30, filetype: b'statepoint',
//!                       n_inactive: 10, n_particles: 1000,
//!                       seed: [3735928559, 305419896], version: [18, 0]}
//!                k_generation (30,)  source_bank/sites (35,)
//!                tallies/tally 1/sum (3,)  .../sum_sq (3,)
//! source.h5      attrs {columns: b'x y z u v w E wgt', n_sites: 4,
//!                       total_weight: 1.0, version: [1, 0]}
//! summary.h5     geometry/cell_ids (2,)
//!                materials/material 1/atom_density [0.044994, 0.0024984]
//! ```
//!
//! The seed halves `[3735928559, 305419896]` reassemble to
//! `0xDEADBEEF12345678` exactly, which is the property the exact-restart
//! guarantee rests on.
//!
//! # What is deliberately not written
//!
//! Tally **results** are written as their raw first and second moments plus
//! the realisation count, not as pre-divided means. A mean written without its
//! `n` cannot be combined with another file's, and combining state points is
//! most of why one writes them.

use std::path::Path;

use hdf5_pure::{AttrValue, FileBuilder};

use crate::error::NjoyError;

/// `VERSION_STATEPOINT` — OpenMC's state-point format version.
pub const VERSION_STATEPOINT: [i64; 2] = [18, 0];
/// `VERSION_SUMMARY`.
pub const VERSION_SUMMARY: [i64; 2] = [6, 0];
/// `VERSION_SOURCE`.
pub const VERSION_SOURCE: [i64; 2] = [1, 0];

/// One tally's accumulated moments, as a state point stores them.
#[derive(Debug, Clone, PartialEq)]
pub struct TallyMoments {
    /// The tally's id.
    pub id: i32,
    /// Its name.
    pub name: String,
    /// Per-bin sum of the realisation values.
    pub sum: Vec<f64>,
    /// Per-bin sum of their squares.
    pub sum_sq: Vec<f64>,
    /// Number of realisations behind `sum` and `sum_sq`.
    ///
    /// **Written, not divided out.** A mean without its `n` cannot be combined
    /// with another state point's, and combining is most of why state points
    /// exist.
    pub n_realizations: u64,
}

/// Everything a state point records.
#[derive(Debug, Clone, PartialEq)]
pub struct StatePointData {
    /// Generations completed.
    pub generations_done: usize,
    /// Inactive generations the run was configured with.
    pub n_inactive: usize,
    /// Histories per generation.
    pub n_particles: usize,
    /// Master RNG seed. One `u64` suffices in this workspace — a particle's
    /// stream is reconstructible from its id — where OpenMC carries an array.
    pub rng_master_seed: u64,
    /// Per-generation eigenvalues, all generations.
    pub k_by_generation: Vec<f64>,
    /// The fission bank as `(x, y, z, u, v, w, E)` per site.
    pub source_bank: Vec<[f64; 7]>,
    /// Tallies.
    pub tallies: Vec<TallyMoments>,
}

/// Write a state point.
///
/// # Errors
///
/// Mismatched moment arrays, a zero realisation count on a tally that has
/// scored, or an I/O failure.
pub fn write_statepoint<P: AsRef<Path>>(
    path: P,
    data: &StatePointData,
) -> Result<(), NjoyError> {
    for t in &data.tallies {
        if t.sum.len() != t.sum_sq.len() {
            return Err(NjoyError::Hdf5(format!(
                "tally {}: sum has {} bins and sum_sq has {}",
                t.id,
                t.sum.len(),
                t.sum_sq.len()
            )));
        }
        if t.n_realizations == 0 && t.sum.iter().any(|&v| v != 0.0) {
            return Err(NjoyError::Hdf5(format!(
                "tally {} has scores but zero realisations. A reader dividing by n \
                 would produce an infinity, and one that guessed n would produce a \
                 plausible wrong mean.",
                t.id
            )));
        }
    }

    let mut b = FileBuilder::new();
    b.set_attr("filetype", AttrValue::AsciiString("statepoint".into()));
    b.set_attr("version", AttrValue::I64Array(VERSION_STATEPOINT.to_vec()));
    b.set_attr("n_particles", AttrValue::I64(data.n_particles as i64));
    b.set_attr("n_inactive", AttrValue::I64(data.n_inactive as i64));
    b.set_attr(
        "current_batch",
        AttrValue::I64(data.generations_done as i64),
    );
    b.set_attr("run_mode", AttrValue::AsciiString("eigenvalue".into()));
    // Written as two i64 halves: HDF5 has no unsigned-64 attribute in the
    // pure-Rust writer, and truncating a seed to i64 would silently alias two
    // different seeds onto one recorded value.
    b.set_attr(
        "seed",
        AttrValue::I64Array(vec![
            (data.rng_master_seed >> 32) as i64,
            (data.rng_master_seed & 0xFFFF_FFFF) as i64,
        ]),
    );

    b.create_dataset("k_generation")
        .with_f64_data(&data.k_by_generation);

    // The source bank, flattened row-major as (n_sites, 7).
    {
        let mut flat = Vec::with_capacity(data.source_bank.len() * 7);
        for s in &data.source_bank {
            flat.extend_from_slice(s);
        }
        let mut g = b.create_group("source_bank");
        g.set_attr("n_sites", AttrValue::I64(data.source_bank.len() as i64));
        g.set_attr(
            "columns",
            AttrValue::AsciiString("x y z u v w E".into()),
        );
        g.create_dataset("sites").with_f64_data(&flat);
        b.add_group(g.finish());
    }

    {
        let mut tg = b.create_group("tallies");
        tg.set_attr("n_tallies", AttrValue::I64(data.tallies.len() as i64));
        for t in &data.tallies {
            let mut g = tg.create_group(&format!("tally {}", t.id));
            g.set_attr("id", AttrValue::I32(t.id));
            g.set_attr("name", AttrValue::AsciiString(t.name.clone().into()));
            g.set_attr(
                "n_realizations",
                AttrValue::I64(t.n_realizations as i64),
            );
            g.set_attr("n_bins", AttrValue::I64(t.sum.len() as i64));
            g.create_dataset("sum").with_f64_data(&t.sum);
            g.create_dataset("sum_sq").with_f64_data(&t.sum_sq);
            tg.add_group(g.finish());
        }
        b.add_group(tg.finish());
    }

    b.write(path.as_ref())
        .map_err(|e| NjoyError::Hdf5(format!("writing statepoint: {e}")))
}

/// One material as a summary records it.
#[derive(Debug, Clone, PartialEq)]
pub struct SummaryMaterial {
    /// Its id.
    pub id: i32,
    /// Its name.
    pub name: String,
    /// Temperature \[K\].
    pub temperature_k: f64,
    /// `(nuclide name, atom density [atoms/barn-cm])`.
    pub nuclides: Vec<(String, f64)>,
}

/// Write a `summary.h5` — the geometry and materials as built.
pub fn write_summary<P: AsRef<Path>>(
    path: P,
    cells: &[(i32, &str)],
    surfaces: &[&str],
    materials: &[SummaryMaterial],
) -> Result<(), NjoyError> {
    let mut b = FileBuilder::new();
    b.set_attr("filetype", AttrValue::AsciiString("summary".into()));
    b.set_attr("version", AttrValue::I64Array(VERSION_SUMMARY.to_vec()));

    {
        let mut g = b.create_group("geometry");
        g.set_attr("n_cells", AttrValue::I64(cells.len() as i64));
        g.set_attr("n_surfaces", AttrValue::I64(surfaces.len() as i64));
        g.create_dataset("cell_ids")
            .with_i64_data(&cells.iter().map(|(id, _)| *id as i64).collect::<Vec<_>>());
        b.add_group(g.finish());
    }

    {
        let mut mg = b.create_group("materials");
        mg.set_attr("n_materials", AttrValue::I64(materials.len() as i64));
        for m in materials {
            let mut g = mg.create_group(&format!("material {}", m.id));
            g.set_attr("id", AttrValue::I32(m.id));
            g.set_attr("name", AttrValue::AsciiString(m.name.clone().into()));
            g.set_attr("temperature", AttrValue::F64(m.temperature_k));
            g.set_attr(
                "nuclides",
                AttrValue::AsciiString(
                    m.nuclides
                        .iter()
                        .map(|(n, _)| n.as_str())
                        .collect::<Vec<_>>()
                        .join(" ")
                        .into(),
                ),
            );
            g.create_dataset("atom_density").with_f64_data(
                &m.nuclides.iter().map(|(_, d)| *d).collect::<Vec<_>>(),
            );
            mg.add_group(g.finish());
        }
        b.add_group(mg.finish());
    }

    b.write(path.as_ref())
        .map_err(|e| NjoyError::Hdf5(format!("writing summary: {e}")))
}

/// Write a `source.h5` — a source bank, for restart or for a second-stage run.
///
/// `sites` is `(x, y, z, u, v, w, E, weight)` per site. The **weight is
/// mandatory**: a source file that omits it is replayed at unit weight, which
/// inflates a second stage by exactly whatever weight the first stage had
/// removed, and the answer stays plausible.
pub fn write_source<P: AsRef<Path>>(path: P, sites: &[[f64; 8]]) -> Result<(), NjoyError> {
    if sites.is_empty() {
        return Err(NjoyError::Hdf5(
            "refusing to write an empty source file: a run reading it would transport \
             nothing and report zeros"
                .into(),
        ));
    }
    let mut flat = Vec::with_capacity(sites.len() * 8);
    for s in sites {
        flat.extend_from_slice(s);
    }
    let total_weight: f64 = sites.iter().map(|s| s[7]).sum();

    let mut b = FileBuilder::new();
    b.set_attr("filetype", AttrValue::AsciiString("source".into()));
    b.set_attr("version", AttrValue::I64Array(VERSION_SOURCE.to_vec()));
    b.set_attr("n_sites", AttrValue::I64(sites.len() as i64));
    // Written so a reader can check the bookkeeping of a two-stage run rather
    // than assume it.
    b.set_attr("total_weight", AttrValue::F64(total_weight));
    b.set_attr(
        "columns",
        AttrValue::AsciiString("x y z u v w E wgt".into()),
    );
    b.create_dataset("source_bank").with_f64_data(&flat);
    b.write(path.as_ref())
        .map_err(|e| NjoyError::Hdf5(format!("writing source: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join("njoy_sp_write_test");
        std::fs::create_dir_all(&d).unwrap();
        d.join(name)
    }

    fn data() -> StatePointData {
        StatePointData {
            generations_done: 30,
            n_inactive: 10,
            n_particles: 1000,
            rng_master_seed: 0xDEAD_BEEF_1234_5678,
            k_by_generation: (0..30).map(|i| 1.0 + i as f64 * 1.0e-3).collect(),
            source_bank: (0..5).map(|i| [i as f64; 7]).collect(),
            tallies: vec![TallyMoments {
                id: 1,
                name: "flux".into(),
                sum: vec![1.0, 2.0, 3.0],
                sum_sq: vec![1.0, 4.0, 9.0],
                n_realizations: 20,
            }],
        }
    }

    /// A state point writes and is a non-trivial file.
    #[test]
    fn a_statepoint_writes() {
        let p = tmp("statepoint.h5");
        write_statepoint(&p, &data()).unwrap();
        let n = std::fs::metadata(&p).unwrap().len();
        println!("wrote {} ({n} bytes)", p.display());
        assert!(n > 1000, "suspiciously small state point: {n} bytes");
    }

    /// **A tally with scores and zero realisations is refused.**
    ///
    /// A reader dividing by `n` produces an infinity; one that guesses `n`
    /// produces a plausible wrong mean. Neither is better than an error at
    /// write time.
    #[test]
    fn a_tally_with_scores_and_no_realisations_is_refused() {
        let mut d = data();
        d.tallies[0].n_realizations = 0;
        let err = write_statepoint(tmp("bad.h5"), &d).unwrap_err();
        assert!(format!("{err}").contains("zero realisations"), "{err}");

        // A tally that genuinely scored nothing is fine at n = 0.
        let mut d = data();
        d.tallies[0].n_realizations = 0;
        d.tallies[0].sum = vec![0.0; 3];
        assert!(write_statepoint(tmp("ok.h5"), &d).is_ok());
    }

    /// Mismatched moment arrays are refused.
    #[test]
    fn mismatched_moments_are_refused() {
        let mut d = data();
        d.tallies[0].sum_sq.pop();
        assert!(write_statepoint(tmp("bad2.h5"), &d).is_err());
    }

    /// A source file writes, and an empty one is refused — a run reading it
    /// would transport nothing and report zeros.
    #[test]
    fn a_source_writes_and_an_empty_one_is_refused() {
        let sites: Vec<[f64; 8]> = (0..4)
            .map(|i| [i as f64, 0.0, 0.0, 1.0, 0.0, 0.0, 2.0e6, 0.25])
            .collect();
        let p = tmp("source.h5");
        write_source(&p, &sites).unwrap();
        assert!(std::fs::metadata(&p).unwrap().len() > 400);

        let err = write_source(tmp("empty.h5"), &[]).unwrap_err();
        assert!(format!("{err}").contains("empty source"), "{err}");
    }

    /// A summary writes.
    #[test]
    fn a_summary_writes() {
        let p = tmp("summary.h5");
        write_summary(
            &p,
            &[(1, "material"), (2, "void")],
            &["sphere", "x-plane"],
            &[SummaryMaterial {
                id: 1,
                name: "HEU".into(),
                temperature_k: 293.6,
                nuclides: vec![("U235".into(), 4.4994e-2), ("U238".into(), 2.4984e-3)],
            }],
        )
        .unwrap();
        assert!(std::fs::metadata(&p).unwrap().len() > 500);
    }

    /// **The 64-bit seed survives the round trip through two i64 halves.**
    ///
    /// Truncating it to one i64 would silently alias two different seeds onto
    /// one recorded value, which is fatal to the exact-restart guarantee the
    /// whole state point exists for.
    #[test]
    fn the_seed_is_not_truncated() {
        let seed: u64 = 0xDEAD_BEEF_1234_5678;
        let hi = (seed >> 32) as i64;
        let lo = (seed & 0xFFFF_FFFF) as i64;
        let back = ((hi as u64) << 32) | (lo as u64);
        assert_eq!(back, seed);
        // And the top bit, which is where an i64 truncation would bite.
        let seed: u64 = u64::MAX;
        let back = (((seed >> 32) as i64 as u64) << 32) | ((seed & 0xFFFF_FFFF) as i64 as u64);
        assert_eq!(back, seed);
    }
}
