# Documentation

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** All code in this workspace is **unverified and untrusted** unless a specific verification & validation (V&V) case demonstrates otherwise. V&V cases are human-reviewed and are intended for journal / arXiv publication — that is the trust workflow. See the workspace `VERIFICATION_AND_VALIDATION.md` and `RESPONSIBLE_USE.md`. Not for nuclear facility operation, reactor control, safety-critical, or licensing decisions.


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

Refreshed 2026-08-11 (the earlier version of this list named only `notes.md` and
`derivation/`, and had fallen behind):

| File | Kind |
|---|---|
| `notes.md` | Hand-maintained. Testing notes, choked-flow status/history, workspace migration log |
| `derivation/` | A full, **not** bite-sized, `ucbthesis`-class LaTeX derivation predating this convention |
| `api.md` | **Generated** rustdoc mirror (`kovan api-docs tampines-steam-tables`). Never hand-edit it — regenerate. It currently reports `Version: 0.2.2` while `Cargo.toml` is at 0.2.5, so it is stale and due a regeneration |
| `edwards_blowdown_solver_debugging.md` | Hand-maintained development-history / debugging write-up for the Edwards–O'Brien blowdown on `TampinesSteamArray` |
| `validation-scope-turbine-and-pipe.md` | Hand-maintained V&V scoping document: what is validated today, and the candidate benchmark cases |

Note that the crate also keeps hand-maintained markdown outside this folder:
`../debug_markdowns/` (choked-flow debugging trails) and
`../verification_and_validation/` (durable V&V records).

**Exception, confirmed 2026-07-10:** `derivation/` is left as-is, not retrofitted into the 4-part bite-sized structure — it is a self-contained thesis-style document with its own internal chapter structure, and splitting it would lose that structure for no benefit. Any genuinely *new* topic written under this crate's `docs/` going forward should use the bite-sized `latex/` convention (see `outram-park-fork-coolprop/docs/`); `derivation/` itself is not touched.

## What's *not* here (yet)

- **Auto-generation.** The intended design — regenerating `latex/` and
  flagging `code_structure.md` staleness automatically when `develop` merges
  into `main`, with a contradiction check against any hand-written notes
  before overwriting anything — is specified but **not implemented**. See
  the workspace-root `docs/docs-ci-spec.md`.
- **A `code_structure.md` / `latex/` topic for this crate.** This file only
  establishes the convention here; writing actual content is a follow-up
  task, not done as part of the initial scaffold.
