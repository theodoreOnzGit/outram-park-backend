// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.
//! Embedded validation tables of the HTR-10 coupled transients (gh:#1151),
//! moved from `tampines::htr10_primary_loop::data` on 2026-11-03.
//!
//! Each file under `data/` is a byte-for-byte copy of the file of the same
//! name in `docs/reactor-scoping/htr10-th-data/`, which carries the citation
//! header, the digitisation method and the read error. The test
//! `data_copies_match_the_input_sheet` fails if a copy drifts. The loop's own
//! tables (power map, coastdown) stay in `tampines`.
//!
//! > Research, education and V&V only. AI draft, unreviewed.

/// Chen et al. (2009) Fig 6, measured relative fission power, 0-10800 s.
pub const POWER_LONG_C09_FIG6: &str = include_str!("data/lofc-power-long-c09-fig6.csv");
/// Chen et al. (2009) Fig 4, measured relative fission power, 0-500 s.
pub const POWER_SHORT_C09_FIG4: &str = include_str!("data/lofc-power-short-c09-fig4.csv");
