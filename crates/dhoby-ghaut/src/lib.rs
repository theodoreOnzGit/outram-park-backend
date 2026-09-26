//! # DHOBY GHAUT
//!
//! **D**igital **H**igh-fidelity **O**rchestration **by** **G**UI for a
//! **H**uman-friendly **A**utomated **U**nified **T**oolkit.
//!
//! The home for OUTRAM PARK's **graphical** studios — the mesh-authoring
//! toolbox and the Monte Carlo studio — and for any future GUI that drives
//! physics rather than merely displaying it.
//!
//! # STATUS: ~~PLACEHOLDER. Nothing is implemented.~~ The LIBRARY is a placeholder; the two studios are examples.
//!
//! ~~This crate was created on 2026-09-17 to hold a decision, not code. It has
//! no GUI, no dependencies, and no behaviour. Do not describe it as providing
//! anything, and do not cite it as the location of a working studio until
//! something has actually moved here.~~
//!
//! **CORRECTED 2026-09-25** — the library target is still a placeholder: its
//! only public item is [`EXPANSION`] and `[dependencies]` in `Cargo.toml` is
//! empty. But both studios **did** move here, as examples, in the commit that
//! created the crate (`eeaf739b0`, 2026-09-17): `examples/mc_studio/main.rs`
//! and `examples/mesh_studio/main.rs`, carried by target-gated
//! dev-dependencies (`eframe`, `egui`, `egui_plot`, and `outram-blender` with
//! its `mc-export` and `foam-mesh` features). Each has a `--headless` mode and
//! four `#[test]`s against a committed CSV fixture under `tests/fixtures/`
//! (`cargo test -p dhoby-ghaut --examples --release`). Verified by reading
//! `Cargo.toml`, both example files and `git log -- crates/dhoby-ghaut`.
//!
//! # Why it exists: the GUI/headless split
//!
//! Two constraints in this workspace pull in opposite directions, and this
//! crate is where they are separated.
//!
//! **Every non-GUI library must build for Android/Termux** (workspace
//! `CLAUDE.md`, "Android / Termux portability"). `egui`/`eframe`/`wgpu` cannot,
//! and the rule names windowing GUI as the single exemption.
//!
//! **`nee_soon` is the integration layer** — it composes Monte Carlo,
//! deterministic/TH, nuclear data and PRKE. That makes it the natural owner of
//! a bridge between a mesh authoring frontend and a transport solver. But it is
//! a CLI/headless crate and must stay Android-buildable, so it cannot own a
//! window.
//!
//! The split that follows:
//!
//! | Concern | Crate |
//! |---|---|
//! | Surface authoring, no solver dependency | `outram-blender` |
//! | Headless bridges + ungated integration examples | `nee_soon` |
//! | **The windowed studios that drive those bridges** | **`dhoby-ghaut`** |
//! | Offline digital-twin simulators (`htgr_sim_v1`, `fhr_sim_v2`) | `outram-park-digital-twin-engine` |
//!
//! # The defect this split is a response to
//!
//! `outram-blender` carried its Monte Carlo bridge behind an optional
//! `mc-export` feature. On 2026-09-17 a merge changed
//! `outram_mc_libs::tally::Tally::filters` from `Vec<Box<dyn Filter>>` to
//! `Vec<FilterKind>`, and the bridge stopped compiling — but
//! `cargo check --workspace --lib --tests` reported **zero errors**, because it
//! builds neither examples nor non-default features. The breakage was found
//! only by a tracker-reconciliation sweep, days of work later (`bn:op-sb2t`).
//!
//! The lesson is structural, not procedural: **an optional integration behind a
//! feature gate is code nothing routinely compiles.** An ungated example in the
//! integration crate is compiled by every ordinary check and therefore cannot
//! rot the same way. Moving the GUI out is what makes that ungated example
//! possible, because the GUI is the only part that cannot live in an
//! Android-buildable crate.
//!
//! # What is expected to move here
//!
//! ~~Recorded so the next reader knows the intent, **not** as a claim that any of
//! it has happened:~~ **CORRECTED 2026-09-25** — both items below have moved
//! (commit `eeaf739b0`, 2026-09-17) and now live in this crate's `examples/`:
//!
//! - **MC Studio** — `outram-blender`'s `examples/mc_studio`, the egui app that
//!   drives surface authoring into Monte Carlo transport.
//! - **Mesh Studio** — the tet-dual volume-meshing frontend over
//!   `outram-park-fork-cfmesh`.
//!
//! The non-GUI halves of those bridges (`outram-blender/src/sim.rs`'s material
//! and tally construction, and `export.rs`'s `to_mc_geometry`) are expected to
//! go to `nee_soon` as ungated code, **not** here.
//!
//! # Rules this crate inherits
//!
//! - **Offline demonstrations only.** `RESPONSIBLE_USE.md` binds: never connect
//!   a studio to live operational systems, plant systems, safety-critical
//!   infrastructure or restricted infrastructure.
//! - **No new physics.** Same rule as
//!   `outram-park-digital-twin-engine`: if a studio needs a physical quantity
//!   the solver crates do not expose, add it *there*. This crate is
//!   presentation and orchestration.
//! - **Android is out of scope for this crate specifically**, and that is the
//!   entire point — it is the designated place for the windowing stack, so that
//!   no other crate has to carry one. The `egui`/`eframe` dependency must never
//!   be added to `nee_soon`, `outram-blender`, or any library expected to build
//!   for `aarch64-linux-android`.

#![doc(html_no_source)]

/// The crate's own expansion, for anywhere a human needs it spelled out.
///
/// DHOBY GHAUT follows the workspace's Singapore-MRT naming convention (see
/// `docs/ecosystem-naming.md`), like OUTRAM PARK, TAMPINES, BOON LAY, BEDOK,
/// NEE SOON, FARRER PARK, CHANGI and KOVAN.
pub const EXPANSION: &str = "Digital High-fidelity Orchestration by GUI for a \
                             Human-friendly Automated Unified Toolkit";

#[cfg(test)]
mod tests {
    use super::*;

    /// The crate is a placeholder and this test says so out loud.
    ///
    /// It exists so that `cargo test -p dhoby-ghaut` is not vacuously green in
    /// a way that could be mistaken for a working GUI being exercised. When the
    /// studios actually land, delete this test — its failure to be deleted is
    /// itself a signal that nothing has moved yet.
    #[test]
    fn this_crate_is_a_placeholder_and_implements_nothing() {
        assert!(EXPANSION.starts_with("Digital High-fidelity Orchestration"));
    }
}
