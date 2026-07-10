# Documentation

This folder follows the OUTRAM PARK `docs/` convention (introduced
2026-07-10) of two content types, kept deliberately separate:

## `code_structure.md` — how the code is laid out (not yet written here)

Markdown describing the crate's file/module structure: what lives where and
why. Hand-maintained, not auto-generated. Not yet written for this crate —
see `outram-park-fork-coolprop/docs/code_structure.md` for the pattern to
follow when it is.

## `latex/` — bite-sized theory -> numerics -> implementation -> walkthrough docs (not yet written here)

Each topic gets a short (<=10 page) series of four `.tex` files, read in
order: **theory** (physics/math on its own terms, no code) ->
**numerical methods** (how that theory becomes a computable algorithm) ->
**implementation** (how *this crate* structures the Rust code for it) ->
**walkthrough** (a concrete worked example, traced function-by-function to a
verified result). Written to double as a script for a video-tutorial
explainer of the topic.

No topic has been written for this crate yet. The full worked example — four
compiled `.tex`/`.pdf` files, a shared `preamble.tex`/`references.bib`, and
the scaffold generator — lives in
`outram-park-fork-coolprop/docs/latex/` (see that crate's `docs/README.md`
for the write-up). To start a new topic here: copy that crate's
`preamble.tex`/`references.bib`/`.gitignore` into this crate's `docs/latex/`,
then either copy `dev/gen_latex_doc.py` from coolprop into this crate's
`dev/` (adjusting paths) or write the four files directly following the same
structure.

## Existing content in this folder

This folder already has `port-reference.md` and `validation.md` (hand-maintained) — untouched by this convention. Note `validation.md` is a different thing from the new top-level `verification_and_validation/` folder: this file is prose porting/validation notes, while `verification_and_validation/` holds the markdown+embedded-CSV benchmark-comparison records with BibTeX citations. Reconcile/cross-link them by hand if they start to overlap.

## What's *not* here (yet)

- **Auto-generation.** The intended design — regenerating `latex/` and
  flagging `code_structure.md` staleness automatically when `develop` merges
  into `main`, with a contradiction check against any hand-written notes
  before overwriting anything — is specified but **not implemented**. See
  the workspace-root `docs/docs-ci-spec.md`.
- **A `code_structure.md` / `latex/` topic for this crate.** This file only
  establishes the convention here; writing actual content is a follow-up
  task, not done as part of the initial scaffold.
