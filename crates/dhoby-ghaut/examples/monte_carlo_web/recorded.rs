//! **Recorded tracks shipped with the page** (gh:#859): what Watch plays
//! while a rung's nuclear data are still being processed, so the reader sees
//! neutrons in the geometry at once and not a progress bar for minutes.
//!
//! The tracks are real histories, recorded natively by the rung's own Watch
//! chain ([`crate::rungs::LoadedRung::run_next`]) and written to
//! `<rung>/data/tracks.bin.z` by
//!
//! ```text
//! cargo run -p dhoby-ghaut --example monte_carlo_web --release -- --bake-tracks <rung> [n]
//! ```
//!
//! in the format the HTR-10 core view already ships its tracks in
//! (`htr10/core/screen.rs`: zlib-compressed little-endian `f64`s). A rung
//! names its file in [`crate::rungs::McRung::recorded_tracks`]. The page says
//! on the picture that the tracks are recorded, and switches to live ones,
//! transported in this tab, as soon as the data are ready.

use crate::history::History;
use crate::htr10::core::screen::decode_tracks;

/// Histories recorded per rung by `--bake-tracks` when no count is given.
#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
pub const DEFAULT_COUNT: usize = 40;

/// The histories in a rung's shipped track file, in recorded order.
///
/// # Errors
///
/// A file that is not a track file (truncated, or not zlib).
pub fn histories(z: &[u8]) -> Result<Vec<History>, String> {
    Ok(decode_tracks(z)?
        .into_iter()
        .map(|(_, i, track)| {
            let (birth, e) = track.states.first().map_or((outram_mc_libs::geometry::position::Position::ZERO, 0.0), |s| (s.r, s.energy));
            History::from_track(i as u64, birth, e, false, track)
        })
        .collect())
}

/// Record `n` histories of `rung`'s Watch chain and encode them. Native only:
/// it processes the rung's tapes first, at the demo's one tier, 0.001
/// (~~the 0.01 Watch tier~~ until 2026-10-16, gh:#876 item 3).
#[cfg(not(target_arch = "wasm32"))]
pub fn bake(rung: crate::table::Rung, n: usize, mut report: impl FnMut(&str, f64)) -> Result<Vec<u8>, String> {
    let mut loaded = crate::load_native(rung, crate::engine::Tier::Exact, &mut report)?;
    let tracks: Vec<(usize, usize, outram_mc_libs::physics::track_output::Track)> =
        (0..n).map(|i| (0, i, loaded.run_next().track)).collect();
    if let Some((_, i, t)) = tracks.iter().find(|(_, _, t)| t.dropped_states > 0 || t.states.len() < 2) {
        return Err(format!("history {i} is truncated or empty ({} states, {} dropped)", t.states.len(), t.dropped_states));
    }
    Ok(crate::htr10::core::screen::encode_tracks(&tracks))
}

/// Where `rung`'s shipped track file lives in the source tree.
#[cfg(not(target_arch = "wasm32"))]
pub fn path(rung: crate::table::Rung) -> std::path::PathBuf {
    use dhoby_ghaut::web_demo::lesson::Rung as _;
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/monte_carlo_web").join(rung.name()).join("data/tracks.bin.z")
}
