# CLAUDE.md — kovan-web (web-kovan)

The workspace `CLAUDE.md` binds here in full, with `crates/kovan/CLAUDE.md`
for the kovan family. This file adds what is specific to this crate.

## What it is

The read-only Code Review UI (GitHub #735, #736, #738), one egui UI for the
browser (wasm32, Pages `code-review/`) and, later, desktop kovan. Decisions
are recorded on #735, #738 and #740 (all 2026-10-06); the crate README says
what is on screen.

## Rules

- **Never depend on `kovan` or `dhoby-ghaut`.** Desktop kovan will embed
  this crate; either dependency is a cycle (dhoby-ghaut depends on kovan).
  Pure data and layout live in `kovan-common` (moved there 2026-10-06; see
  `NOTICE`). The small pieces of `dhoby_ghaut::web_demo` this needs are
  ported in `src/ui/camera.rs`, with the citation.
- **Web is read-only.** No Stamp, Needs fix, rename or code action is ever
  enabled in `Mode::Web`. AI agents never stamp (`kovan::review_stamps`).
- **One bar, one UI.** The review bar is one component parameterised by
  `Mode`; desktop stamping (#740) enables its buttons, it does not get a
  second bar.
- **Nothing on a frame waits.** Every file loads through `data::Store`
  (`spawn_local` fetch on the web, a thread natively) into
  `Arc<RwLock<Shared>>`; the frame takes `try_read` snapshots.
- **Canvas text sizes are rounded** (`ui::canvas::quantise`): every distinct
  size fills egui's font atlas, and a size that followed the zoom panicked
  epaint within a few frames.
- **Fonts:** egui's bundled fonts lack arrows and triangles (← → ↑ ↗ ▸ ■
  render as boxes); write words or draw shapes.
- **Mobile-first and no-lag** (workspace rule): zoom buttons on the canvas,
  side panel folded under 700 pt, source as a full-screen sheet on a phone.
  Check at phone width with headless Chromium before calling a UI change
  done.

## Data

`web/data.sh` writes the data folder (`kovan_web::data` lists the files):
the code map, the call graph split one file per crate without source text,
`search.json`, the rustdoc pages that exist, and `build.json` (the commit the
source panel fetches files at, and since #772 the call graph's backend and
its stale or missing crates, shown on the map panel). The call graph is
incremental: per-crate documents cached under
~~`target/kovan-index/<crate>/<key>.json`~~ **CORRECTED 2026-10-07 (#772)**
`target/kovan-index/<backend>/<crate>/<key>.json`, keyed by
`kovan-cli call-graph-keys`, merged with `call-graph --merge`;
`data.sh --check` pins that a merge equals one run. The backend is SCIP by
default (one `rust-analyzer scip` over the workspace, about 4 min and 16 GiB
peak; `KOVAN_CALL_GRAPH_BACKEND=lsp` for the old per-call LSP path). A crate
that cannot be rebuilt keeps its last cached graph and is named as stale,
never silently (Leak Before Break). Nothing generated is committed.

## Known gaps (2026-10-06)

- ~~`Mode::Desktop` is a placeholder: the bar's buttons only post a status.~~
  **CORRECTED 2026-10-10** (#820): in `Mode::Desktop` the bar's buttons
  queue a `ui::HostRequest` (`CodeReview::take_host_request`) and the host
  pushes stamp states back with `CodeReview::set_stamps`; ~~desktop kovan's
  stamp dialog is still a placeholder window (#740, #770).~~ **CORRECTED
  2026-10-10** (#770): desktop kovan's stamp dialog is written
  (`kovan/src/app/stamp_dialog.rs` over `kovan/src/stamping/flow.rs`): key
  set-up, the review wizard, sign and write `review.md`, needs fix, then
  `set_stamps` with the states from `review.md`.
- Definition, type, references and implementations in the source panel's
  menu are greyed out until the link index (#745) fills `SourceFile::links`.
- Walkthroughs ("Show walks through here") are a stub (#741).
- The map's in-place module list is an overlay under the card, not a
  relayout of the topic box.
- Crates with very many modules (petir, boon-lay) give a tall crate tree; at
  Fit on a phone the names are clipped to 8 pt.
- The wasm is about 11.5 MB uncompressed (no `wasm-opt`).
