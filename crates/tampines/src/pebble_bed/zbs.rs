//! # Zehner-Bauer-Schlunder (ZBS) pebble-bed effective thermal conductivity
//!
//! **Moved 2026-11-03 (gh:#1148, maintainer decision D2).** The correlation,
//! its documentation and its tests now live in
//! [`outram_foam_appbuilder_lib::htgr_closures::zbs`], beside the other
//! pebble-bed closures (Breitbach bed conductivity, KTA 3102.2 heat transfer,
//! KTA 3102.3 pressure drop). This module re-exports it so existing
//! `tampines::pebble_bed::zbs::ZbsBed` paths keep working; there is one copy.
//!
//! Its methods return
//! [`outram_foam_appbuilder_lib::htgr_closures::HtgrClosureError`], which
//! converts into [`crate::TampinesError`] with `?`.
//!
//! **Not the physics default for bed conductivity.** Since 2026-11-03
//! (maintainer decision D3) the default is the IAEA-TECDOC-1694 Breitbach
//! correlation:
//! [`outram_foam_appbuilder_lib::htgr_closures::BedConductivityModel`].

pub use outram_foam_appbuilder_lib::htgr_closures::zbs::*;
