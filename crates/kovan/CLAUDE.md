# CLAUDE.md

Guidance for Claude Code (and other AI assistants) working in the `kovan`
crate. The workspace `CLAUDE.md` binds here too; `README.md`, `NOTICE` (the
AGPL boundary) and `DECISIONS.md` hold the crate's own records.

## Files and folders are chosen with a file picker (HARD RULE)

**Maintainer direction, 2026-10-05. Binds `dhoby-ghaut`, `kovan` and `dover`.**
Whenever the program needs the user to open, save or choose a file or a
folder (an ENDF library, a kovan root, a recipe or input deck, an export
folder, an image, a PDF), it offers a **file picker**: `egui-file-dialog` in
the egui apps (the workspace dependency; one shared `FileDialog` routed by
what asked for it, as `kovan/src/app/mod.rs` `FileDialogTarget` and
`dhoby-ghaut/src/bin/dhoby-ghaut/app.rs` `Pick` do), a file-path argument in
a CLI. A typed path box is never the only way in; the picker opens where the
current value is. Show the chosen path as text so the user can see it.

**Why.** A typed path is error-prone and undiscoverable: the user has to know
the exact folder and spell it. The maintainer asked for pickers everywhere
after the workbench's first build had typed ENDF and kovan-root fields.

**Status in kovan (checked 2026-10-05).** The two typed path fields found,
the setup folder (`src/app/setup.rs`) and the digitiser image path
(`src/app/mod.rs`), both already have a "Browse…" picker beside them.
