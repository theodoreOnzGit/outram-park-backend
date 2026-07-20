# chem-eng-real-time-process-control-simulator — migration notes (read on demand)

Reference material: the 2026-06 OUTRAM PARK consolidation log for this crate
(workspace dependency inheritance, profile moves, Apache-2.0 license retention).
Consulted on demand — not per-turn guidance. The crate's purpose, layout, and
build commands live in CLAUDE.md.

## Migration notes (OUTRAM PARK consolidation, 2026-06)

Done while moving this crate into the workspace and bumping to latest deps:

- Now built/tested/published from the workspace; standalone git history dropped.
- Dependencies (`approx`, `csv`, `thiserror`, `uom`) switched to
  `*.workspace = true`. Notably `uom` 0.36 → **0.38** and `thiserror` 1 → 2.
  Unifying `uom` to a single version across the workspace is what made the TUAS /
  TAMPINES controller-based tests compile again (they were hitting two
  incompatible `uom::Quantity` types).
- The crate's `[profile.release]` (opt-level 2) and `[profile.dev.package."*"]`
  (opt-level 2) sections were **removed** — Cargo only honors `[profile.*]` on
  the workspace root. The `dev.package."*"` optimization was re-added at the
  root; the `release` opt-level override was intentionally dropped so the
  numerical solvers build at the default `-O3`.
- License kept as **Apache-2.0** (explicit, not inherited).

The library and its tests compile cleanly on the bumped dependencies; no source
changes were required here.
