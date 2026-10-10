//! # kovan (library)
//!
//! Shared modules behind KOVAN's three front ends, which live as separate
//! binaries in this same crate — exactly three, per the final interface spec
//! on GitHub issue #30 (2026-08-21):
//!
//! - **`kovan`** (human-facing GUI, [`digitiser::gui`]) — a thin wrapper
//!   around the egui digitiser window, KOVAN's one GUI surface. Desktop-only:
//!   the `gui` feature (default everywhere except Android) target-gates its
//!   egui/eframe dependencies off Android, and [`digitiser::gui::run`]
//!   branches internally to a redirect message there.
//! - **`kovan-cli`** (agent-facing CLI, [`commands`]) — deterministic,
//!   line-oriented subcommands for a coding agent, including `digitise`
//!   (the automatic-only path over [`digitiser::frontend::AutoArgs`]).
//! - **`kovan-tui`** (human-facing terminal UI, [`tui`]) — a `ratatui`
//!   browser over the same sibling `kovan-*` crates, plus interactive
//!   literature ingestion (Ingest tab) and interactive graph digitisation
//!   (Digitiser tab, over the same [`digitiser`] engine `kovan-cli digitise`
//!   uses). Genuinely Android/Termux-usable, not just buildable — see
//!   `src/bin/kovan-tui.rs`.
//!
//! This crate was consolidated from the former separate `kovan-cli` and
//! `kovan-tui` crates on 2026-08-21 so one crate carries all three interfaces
//! over the same knowledge-layer libraries — see `DECISIONS.md` for the
//! merge rationale and each front end's original design history. The
//! [`digitiser`] module joined the same day, moved here from
//! `kovan-literature` so it can depend on `kopitiam-pdf` (this crate's own
//! AGPL-3.0-only relicense, see `NOTICE`) without dragging
//! `kovan-literature` — used well beyond the GUI — into that relicense too.
//! The binaries were briefly five (`kovan`, `kovan-tui`, `kovan-gui`,
//! `kovan-digitise`, `kovan-digitise-tui`) before collapsing to the three
//! above later the same day, per GitHub issue #30's final spec — see
//! `NOTICE` and `src/tui/digitiser.rs`.

pub mod advanced_git;
/// The KOVAN application shell (GH issue #35 checkpoint §22, `op-1arj`) —
/// [`app::DigitiseApp`] and its view panels (Wiki, Mindmap, PDF Reader,
/// kvim editor, Bibliography, Save Repository, …), moved here from
/// `digitiser::gui::desktop` 2026-09-01. It previously lived nested under
/// the graph digitiser, which was backwards: the digitiser is one panel
/// *of* the app shell, not its owner — op-9vo6's own scoping-pass finding
/// #1. Desktop-only, mirroring `digitiser::gui`'s own gating: behind this
/// crate's `gui` feature (default everywhere except Android) and belt-
/// and-suspenders target-gated off Android directly, same as the module it
/// replaces was.
///
/// This pass is the module-path relocation only — a pure move, no
/// behaviour change, and (per grep) touched no reference outside this
/// crate's own doc comments. Renaming `DigitiseApp` itself to a name that
/// reflects shell (not digitiser) ownership is a separate, deliberately
/// deferred follow-up — see the crate's `bn` tracker.
#[cfg(all(feature = "gui", not(target_os = "android")))]
pub mod app;
pub mod artifact;
pub mod autocomplete;
/// The workspace call graph, crate → module → function, with function
/// source (GitHub #737). Moved 2026-10-06 to the wasm-clean
/// [`kovan_common::call_graph`] for web-kovan (#736); re-exported here.
pub use kovan_common::call_graph;
pub mod classify;
/// The code map of a Cargo workspace from its `[package.metadata.kovan]`
/// tags (GitHub #734): model, layout and SVG. GUI-free; see [`code_map`].
pub mod code_map;
/// The data folder the app's Code Review view reads, built from a
/// workspace's saved SCIP index (GitHub #820). GUI-free.
pub mod code_review_data;
pub(crate) mod collection_picker;
pub mod concept_ops;
pub mod commands;
pub mod connections;
pub mod corpus;
pub mod corpus_repos;
pub mod corpus_tiers;
pub mod digitiser;
pub mod entity;
pub mod fingerprint;
/// Moved 2026-10-06 to the wasm-clean [`kovan_common::fuzzy`] (web-kovan's
/// search bar, #736); re-exported here under its old path.
pub use kovan_common::fuzzy;
pub mod graph;
pub mod index;
pub mod index_fresh;
pub mod mindmap;
pub mod mindmap_layout;
pub mod mindmap_model;
/// Moved 2026-10-06 to the wasm-clean [`kovan_common::mindmap_view`] for
/// web-kovan (#736); re-exported here under its old path.
pub use kovan_common::mindmap_view;
pub mod navigation;
pub mod node_id;
pub mod page_rotation;
pub mod ingest;
/// The network behind identifier lookup (GitHub #756), opt-in.
pub mod lookup_net;
pub mod session;
pub mod standard_corpus;
pub mod project;
pub mod relation;
pub mod repository;
pub mod research_record;
/// Human review stamps (`review/stamps.toml`, GitHub #739): hash-based,
/// voided deterministically from git. GUI-free. See [`review_stamps`].
pub mod review_stamps;
/// Recently opened papers — local, derived state in `.kovan/`, surfaced
/// in the map and the Wiki beside Unsorted. See [`recent`].
pub mod recent;
pub mod root;
pub mod runtime_graph;
/// Is rust-analyzer installed, and installing it with rustup (GitHub #820):
/// the Code Map view's check before "Index fresh". GUI-free.
pub mod rust_analyzer_setup;
pub mod save_push;
pub mod scip;
/// Signed review stamps for desktop kovan's stamp dialog (GitHub #770,
/// #762, #765): draft, write `review.md`, register keys, per-function
/// states from `review.md`. GUI-free. See [`stamping`].
pub mod stamping;
pub mod sync;
pub mod tui;
/// Zotero import into a Kovan folder and export from it (GitHub #752,
/// epic #747); `kovan-cli zotero` is its front end.
pub mod zotero;
