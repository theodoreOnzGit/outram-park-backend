// ---------------------------------------------------------------------------
// Ported from SCRAM (a probabilistic risk analysis tool).
//
//   Upstream project: SCRAM — Olzhas Rakhimov
//   Upstream repo:    https://github.com/rakhimov/scram
//   Upstream commit:  b85b78940de38996eeffec54d946824bd4280a1c
//   Commit date:      2019-07-03
//   Accessed:         2026-09-21
//
//   Copyright (C) 2014-2018 Olzhas Rakhimov
//
//   This program is free software; you can redistribute it and/or modify
//   it under the terms of the GNU General Public License as published by
//   the Free Software Foundation; either version 3 of the License, or
//   (at your option) any later version.
//
//   This program is distributed in the hope that it will be useful,
//   but WITHOUT ANY WARRANTY; without even the implied warranty of
//   MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
//   GNU General Public License for more details.
//
// SCRAM is GPL-3.0-or-later and RAFFLES is GPL-3.0-only, so this is a
// same-licence port, NOT the one-way Apache-2.0 -> GPLv3 relicensing that
// governs the RAVEN-derived parts of this crate. Code may flow SCRAM ->
// RAFFLES freely under GPLv3. Contributing anything back upstream is still a
// decision for the crate owner and the copyright holders, not an assistant.
// ---------------------------------------------------------------------------

//! # SCRAM port — fault-tree quantification
//!
//! A translation of the probabilistic-risk-analysis core of
//! [SCRAM](https://github.com/rakhimov/scram): taking the **minimal cut sets**
//! of a fault tree and turning them into a top-event probability and a ranking
//! of which basic events matter.
//!
//! ## What a cut set is, for a reader who has not met one
//!
//! A **fault tree** is a Boolean expression for how a system fails, written
//! over *basic events* — a pump not starting, a valve stuck shut. A **cut set**
//! is a set of basic events whose simultaneous occurrence is sufficient to
//! cause the top event. It is **minimal** when removing any member stops it
//! being sufficient.
//!
//! The minimal cut sets are the complete qualitative answer: the system fails
//! exactly when at least one of them occurs. Everything in this module takes
//! that set as given and answers the quantitative questions — how likely, and
//! which events drive it.
//!
//! **Producing the cut sets is not done here yet.** SCRAM derives them with
//! MOCUS, BDD or ZBDD over a Boolean graph; those are the larger part of the
//! upstream and are not ported. This module starts at the point where the cut
//! sets already exist, which is the layer with the cleanest oracles and the
//! one a caller can reach with cut sets from any source.
//!
//! ## Where this sits relative to the rest of the crate
//!
//! [`crate::imprecise::SystemStructure`] also computes system reliability, and
//! the two are **not** duplicates:
//!
//! | | `imprecise::SystemStructure` | this module |
//! |---|---|---|
//! | input | component reliabilities | minimal cut sets + event probabilities |
//! | structure | series, parallel, k-out-of-n | arbitrary coherent fault tree |
//! | numbers | **interval-valued** | point-valued |
//! | question | reliability under dependence assumptions | top-event probability and event importance |
//!
//! Reach for `imprecise` when the structure is simple and the inputs are
//! bounds; reach for this when the structure is a real fault tree.
//!
//! ## Verification
//!
//! Every function here is checked against **upstream SCRAM compiled and run**,
//! not against a reading of its source. The oracle harness, the exact upstream
//! commit, the one build patch that was needed, and the measured agreement are
//! recorded in `crates/raffles/docs/scram-port-verification.md`.

pub mod importance;
pub mod probability;

pub use importance::{ImportanceFactors, importance_factors};
pub use probability::{Approximation, CutSet, cut_set_probability, top_event_probability};
