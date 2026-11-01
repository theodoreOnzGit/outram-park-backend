// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.
//! Junctions joining 1-D pipes: the RELAP-7 FlowJunction and VolumeBranch
//! (gh:#1071, RELAP-7 parity epic gh:#1056).
//!
//! The framework lives beside the HEM pipe solver in
//! `tampines_steam_tables::hem::network`, because it drives `HemPipe`'s
//! boundaries and steps; it is re-exported here so balance-of-plant
//! components (pump, valve, turbine, separator, downcomer, wet well: gh:#1072
//! to gh:#1075) find it where they live. Run it with the run-time closure
//! [`crate::hem::HemEos`], e.g. `HemNetwork<HemEos>`.
//!
//! See the source module's doc for the method and the V&V record,
//! `crates/tampines-steam-tables/verification_and_validation/hem_network_junctions.md`.

pub use tampines_steam_tables::hem::{
    HemJunction, HemNetwork, PipeEndRef, PortDraw, PumpHead, TdvThermo, TurbineParams, TimeDependentBoundary, TimeTable, VolumeBranch, VolumePort,
};
