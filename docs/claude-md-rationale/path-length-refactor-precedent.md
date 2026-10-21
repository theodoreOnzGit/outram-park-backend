# Rationale: the 170-character path-length refactor precedent

> Split out of the root `CLAUDE.md` on 2026-09-21. **Full original text,
> verbatim.** The 170-character cap itself remains a hard rule in `CLAUDE.md`;
> this records how the 2026-08-19 workspace-wide refactor was carried out and
> why it is considered low-risk to repeat.

### Path-length refactor precedent — this is a well-trodden, low-risk operation

**An agent session did this exact refactor workspace-wide on 2026-08-19** (the
170-char backlog above) and it is safe to ask for again: shortening directory
and file names to fix path-length violations, keeping the whole workspace
compiling (including test binaries) throughout, is a routine that has already
been proven out here, not a novel or risky undertaking. Concretely, that pass:

- Renamed the offending directories/files in `tampines-steam-tables` (2 files,
  one private nested test-module directory, no public API involved) and
  `tuas_boussinesq_solver` (51 files, 17 directory renames + 8 file renames).
- **Checked every renamed identifier for public-API exposure first** (a
  workspace-wide grep for real `use`/`pub use` references, not just a
  directory listing) before touching anything — four `tuas_boussinesq_solver`
  identifiers turned out to be genuinely public, cross-crate API: old names
  `ciet_steady_state_natural_circulation_test_components`,
  `array_control_vol_and_fluid_component_collections`,
  `one_d_fluid_array_with_lateral_coupling`,
  `one_d_solid_array_with_lateral_coupling`, renamed to
  `ciet_nat_circ_tests`, `array_fluid_collections`,
  `fluid_array_lateral_coupling`, `solid_array_lateral_coupling`.
- **First landed with a `pub use new_name as old_name;` compatibility
  re-export** at each original declaration site, so the first push broke
  nothing downstream without touching any consumer. **Immediately followed,
  same day, by migrating every downstream reference** (`tampines`,
  `tampines-steam-tables`, `teh-o-prke`, and
  `outram-park-digital-twin-engine`'s CIET v2 binary/examples — 178 files
  workspace-wide) to the new short names directly, then deleting the four
  aliases once a workspace-wide grep for each old identifier came back empty.
  The alias was a same-session bridge, not a standing policy — the maintainer
  wanted the short names used everywhere, not kept behind a compatibility
  shim. If a future pass repeats this on a crate with real crates.io
  consumers outside this workspace, keeping the alias for at least one
  published version is worth raising with the maintainer explicitly, since
  those consumers cannot be grepped for and fixed in the same session.
- Verified with `cargo check --workspace --lib --tests` and
  `cargo test --workspace --no-run --release` (all test binaries, not just
  `--lib`) after each stage, all clean.
- **kopitiam's `rename` command could not be used** (a real bug: rust-analyzer
  rejected every rename request with "Client does not support rename
  capability", LSP error -32803 — filed as
  [kopitiam#30](https://github.com/theodoreOnzGit/kopitiam/issues/30)). The
  fallback this file's own "Workflow rules" section already prescribes for
  when the LSP rename tooling is unavailable — enumerate every reference with
  a text search, apply the rename by hand, and let the compiler (`cargo
  check`) be the reference-checker that catches anything missed — carried the
  whole refactor without incident.

