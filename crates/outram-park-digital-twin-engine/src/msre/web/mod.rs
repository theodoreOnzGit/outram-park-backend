//! **The MSRE simulator's engine, as a library** (gh:#1578; maintainer,
//! 2026-11-14: "msre code shld reside in library, because I want to build on
//! the simulation outside outram-park"). Everything the `msre_web` demo runs
//! that is not drawing, moved from the example unchanged in behaviour:
//!
//! | module | what it is |
//! |---|---|
//! | [`engine`] | the plant engine: one `session::Session` and Moltres's loop drift, answering [`engine::Request`]s with [`engine::Event`]s, self-paced by wall-clock ticks (gh:#1573); its browser Web Worker ([`engine::worker`], wasm only) and its native clock ([`engine::start_native_ticker`]); the wire codes |
//! | [`keff`] | a k_eff worker on the MSRE N-1 model (gh:#1571): the processing jobs, the processed-data cache, the assembled CSG model, source and chunk answers bit for bit `run_keff_csg_par`'s; the slice raster drawn from the assembled geometry |
//! | [`keff_pool`] | the page side: a pool of k_eff workers for one study, the measured fuel/graphite split (gh:#1574) and the accuracy presets |
//!
//! Built on `dhoby_ghaut::mc_web` (the browser Monte Carlo pipeline, gh:#1572)
//! and `bugis::web_demo` (links, workers, the platform clock). Off Android,
//! as both are; on wasm32 the engine also carries its Web Worker.
//!
//! Research, education and V&V only. NOT VALIDATED: the N-1 model's k sits
//! +1726 ± 23 pcm above ORNL-4233's (gh:#1128). Never for operation,
//! licensing or safety decisions.

pub mod engine;
pub mod keff;
pub mod keff_pool;
