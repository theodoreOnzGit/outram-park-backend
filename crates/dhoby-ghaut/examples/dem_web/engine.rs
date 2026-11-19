//! The DEM pour, off the UI thread. **Moved 2026-11-19 to
//! `nee_soon::dem_pour::engine`** (gh:#1655, maintainer: the DEM interface
//! "shld be reusable library in nee soon"); this file keeps the path the
//! DEM page, its worker and the HTR-10 rung's `#[path]` include use.
//! Unchanged in behaviour: the vessel now comes from the CSG API and pours
//! HTR-10's bed bit for bit (that module's tests).

pub use nee_soon::dem_pour::engine::*;
