//! **The Monte Carlo demo as a library** (gh:#1763, feature `mc_demo`): the
//! whole of the `monte_carlo_web` example's app, its engine and every rung,
//! moved here from `examples/monte_carlo_web/` so another simulator can draw
//! it in its own window. The example keeps only its `main` functions
//! (`examples/monte_carlo_web/main.rs`); the files moved unchanged but for
//! their crate paths (`crate::` became `crate::mc_demo::`, `dhoby_ghaut::`
//! became `crate::`) and the four `"./worker.js"` literals, which now read
//! [`worker_js`].
//!
//! **Why the whole app and not only the HTR-10 rung.** The rung's screen
//! (`htr10::core::screen`) is driven by [`app::McApp`]'s phase logic (data
//! priming, the worker pool, the bed, the pour) and by the rung table; taking
//! the screen alone would have meant writing a second driver for it, which
//! the workspace's "search before building" rule forbids. A host opens
//! the app on the rung and view it wants with [`McApp::with_query`].
//!
//! **First host:** `outram-park-digital-twin-engine`'s `htgr_sim_v1`, whose
//! MC page draws the HTR-10 core with its DEM pour natively and in the
//! browser (gh:#1763; it was an iframe of the demo page, gh:#1761). In the
//! browser the host's workers are the Monte Carlo demo's own
//! (`../monte-carlo/worker.js`, set with [`set_worker_js`]), so the tapes
//! come from that page's `data/` and the processed-data cache is the one it
//! fills.
//!
//! Education and research only, per the workspace `RESPONSIBLE_USE.md`. Not
//! for reactor operation, licensing, or safety-critical decisions.

pub mod anim;
pub mod app;
pub mod engine;
pub mod history;
pub mod keff;
pub mod keffplot;
pub mod rungs;
pub mod tapes;
pub mod xs;
pub mod raster;
pub mod sweep;
// The `htr10` rung's liberties toggle, lattice bed against DEM bed (gh:#787),
// and the baked beds it draws (shared with `dem_web`, which uses them from
// here since gh:#1763; they were `examples/common/htr10_beds.rs`).
pub mod beds;
pub mod htr10_beds;
pub mod walkdemo;
// What the small-CSG benchmark rungs share, and the tracks every benchmark
// rung ships with the page (gh:#859).
pub mod csgrung;
pub mod recorded;
// Run k_eff of a benchmark rung on a pool of workers (gh:#905, #907).
pub mod benchpool;
// Data processing on a pool of workers (maintainer, 2026-10-16).
pub mod prime_pool;
// The processed nuclear data every demo shares, cached in the browser
// (gh:#818): the library's `mc_web::processed_cache`, under the name the
// rungs use.
pub use crate::mc_web::processed_cache;

// The rung modules, declared plainly so kovan's module walk (which reads
// `mod` lines and does not expand macros) finds each rung's folder: the
// workspace rule "never declare modules through a macro" (root `CLAUDE.md`).
pub mod godiva;
pub mod natu;
pub mod jemima;
pub mod hst009;
pub mod ugraphite;
pub mod lumped;
pub mod lct008;
pub mod triso;
pub mod dhshort;
pub mod packing;
pub mod htr10;

// THE RUNG TABLE, in ladder order: `table.rs`, written out by hand (one
// variant and one `match` arm per rung; see `rungs.rs` for what a new rung
// adds).
pub mod table;

pub use app::McApp;

/// The page-relative URL of the Monte Carlo worker script, set once by a
/// host page before the app starts ([`set_worker_js`]).
static WORKER_JS: std::sync::OnceLock<String> = std::sync::OnceLock::new();

/// The worker script every pool and link of the demo starts: `./worker.js`
/// on the demo's own page, or what a host set with [`set_worker_js`].
pub fn worker_js() -> &'static str {
    WORKER_JS.get().map_or("./worker.js", String::as_str)
}

/// Make the demo's workers run `url` (page-relative) instead of
/// `./worker.js`. Call before the app starts; the first call wins and a
/// later one returns `false`. A host on another page points this at the
/// demo's own worker (`../monte-carlo/worker.js`), whose fetches of tapes
/// resolve against the worker script, so they come from the demo's `data/`.
pub fn set_worker_js(url: &str) -> bool {
    WORKER_JS.set(url.to_string()).is_ok()
}

/// Read a reference tape from `reference-data/endf/` and strip its covariances
/// — the same bytes the browser gets once it has inflated the download.
#[cfg(not(target_arch = "wasm32"))]
pub fn native_tape(tape: &str) -> Result<Vec<u8>, String> {
    let path = njoy_outram_park_fork::reference_data::reference_endf(tape)
        .ok_or_else(|| format!("reference tape {tape} is not present"))?;
    let raw = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(tapes::strip_covariances(&raw))
}

/// Process a rung's tapes natively, in order. `report` sees each job's label
/// and seconds.
#[cfg(not(target_arch = "wasm32"))]
pub fn load_native(rung: table::Rung, tier: engine::Tier, mut report: impl FnMut(&str, f64)) -> Result<table::Loaded, String> {
    let jobs = rung.jobs_for(rung.tier(tier));
    let mut post = |e: engine::Event| {
        if let engine::Event::JobDone { index, secs, .. } = e {
            report(jobs[index].0, secs);
        }
    };
    // The headless tools process every tape (no cache), as before #818.
    engine::native_load(0, rung, tier, &mut processed_cache::DataStore::off(), &mut post)
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests;
