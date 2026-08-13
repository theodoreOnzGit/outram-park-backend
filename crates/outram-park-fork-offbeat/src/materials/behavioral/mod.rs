// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from OFFBEAT (https://gitlab.com/foam-for-nuclear/offbeat),
// upstream commit 80e84450a115b0c411e1bfa5d166379f6bf6c084, GPL-3.0.
// Corresponds to upstream
// `offbeatLib/materials/materialModel/behavioralModels/`.
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.

//! Behavioural models — how the material's volume and integrity evolve.
//!
//! Distinct from [`properties`](super::properties): a property answers "what is
//! the conductivity of this material right now?", whereas a behavioural model
//! answers "how much has this material swollen, densified, cracked or
//! relocated?". Both are pure functions of
//! [`MaterialState`](crate::materials::MaterialState) — the accumulation of
//! state over time is the caller's job, not theirs.
//!
//! | Module | Answers |
//! |---|---|
//! | [`swelling`] | volume growth from solid and gaseous fission products |
//! | [`densification`] | early-life shrinkage as fabrication porosity sinters out |
//! | [`relocation`] | outward movement of cracked fuel fragments, closing the gap |
//! | [`phase_transition`] | solid-phase changes (e.g. Zircaloy alpha to beta) |
//! | [`failure`] | failure criteria, including Weibull statistics for ceramics |

pub mod densification;
pub mod failure;
pub mod phase_transition;
pub mod relocation;
pub mod swelling;
