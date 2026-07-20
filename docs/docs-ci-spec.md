# `docs/` auto-generation CI — design spec (pending review, not yet wired up)

This is a **spec document only** — no `.github/workflows` file exists yet. It
describes the intended design for auto-updating each crate's `docs/latex/`
and `docs/code_structure.md` when `develop` merges into `main`, per the
per-crate `docs/` convention introduced 2026-07 (see e.g.
`crates/outram-park-fork-coolprop/docs/README.md` for the worked example this
spec is meant to generalize).

## Why this exists

The three-folder convention (`verification_and_validation/`,
`upstream_source/`, `docs/`) was scaffolded by hand for all 13 workspace
crates. `docs/` is explicitly meant to regenerate automatically going
forward — see the original instruction: *"docs folders should be auto
updated only when develop branch is merged into main branch... when auto
generating the docs, create python codegens for the latex, and ask me if
what I wrote contradicts the docs to be generated."*

This document is the design for that pipeline. It is **not implemented** —
implementing it (writing the actual GitHub Actions workflow) requires a
separate, explicit go-ahead per the workspace's "no CI without confirmation"
scoping decision made when this convention was introduced.

## Trigger

```yaml
on:
  push:
    branches: [main]
```

Specifically a merge landing on `main` (not every push to `develop`, and not
pull requests) — docs regenerate once, at the point code is considered
released, not on every commit of in-progress work.

## What runs, per crate

For each workspace crate that has a `dev/gen_latex_doc.py` (i.e. has opted
into this convention — not all 13 will on day one):

1. **Diff detection.** Only regenerate a crate's `docs/` if that crate's
   `src/` changed between the previous `main` and the new one
   (`git diff --name-only <prev-main-sha> <new-main-sha> -- crates/<name>/src`).
   Untouched crates are skipped — this keeps the workflow fast and avoids
   spurious PDF-diff noise in crates nobody touched.

2. **Contradiction check against manual notes.** Before regenerating, read
   any hand-maintained `docs/notes.md` (or crate-specific equivalent — see
   the exceptions below) in the target crate. Run a **model-driven diff
   check**: prompt an LLM with (a) the existing notes content and (b) a
   summary of what the regeneration step is about to write, and ask it to
   flag any factual contradiction (not just textual difference — e.g. notes
   claiming "conductivity uses the ECS method" while the regenerated
   implementation doc says "hardcoded per-fluid form" is a contradiction;
   notes adding unrelated historical context are not). On a flagged
   contradiction, the workflow **stops and opens a review issue** instead of
   overwriting — it does not auto-resolve. This is the direct implementation
   of *"ask me if what I wrote contradicts the docs to be generated."*
   Concretely this likely means a scripted call to the Claude API (or
   equivalent) from within the workflow step, not a hand-written heuristic —
   contradiction-detection is a language-understanding task, not a diff.

3. **Regenerate `code_structure.md`.** Currently hand-maintained prose (see
   `crates/outram-park-fork-coolprop/docs/code_structure.md`). Full
   auto-generation from source is out of scope for the first version of this
   pipeline — start by having the workflow *flag* the file as possibly stale
   (module list changed) rather than rewriting it, since accurately
   describing *why* a module exists from source alone is not reliably
   automatable. A human (or a follow-up AI session) updates it by hand,
   informed by the flag.

4. **Regenerate `docs/latex/*.tex`.** Same caution applies even more
   strongly — `dev/gen_latex_doc.py` today only scaffolds a *new* topic's
   skeleton (title + section headings + TODOs), it does not derive physics
   prose from Rust source. A CI step that fully auto-writes LaTeX theory
   content from source is a research problem, not an engineering one; this
   spec does **not** claim to solve it. What CI *can* safely automate:
   - Recompiling every existing `.tex` file with `latexmk -pdf` and failing
     the build if any no longer compiles (catches bit-rot, e.g. a renamed
     `\label{}` or a broken `\cite{}` key after `references.bib` changes).
   - Committing the freshly-built `.pdf` outputs if `latexmk` succeeds and
     the PDF content actually changed (byte-diff after normalizing
     `/CreationDate`).
   - Opening a tracking issue (not a code change) listing which topics'
     source files changed since their LaTeX doc was last touched, as a
     prompt for a human/AI session to write the follow-up content by hand
     using `dev/gen_latex_doc.py` as the starting scaffold.

5. **Commit.** If anything changed, commit directly to `main` with a
   recognizable bot-authored message (e.g. `docs(coolprop): recompile
   latex/, flag stale code_structure.md [skip ci]`) — `[skip ci]` to avoid
   retriggering this same workflow.

## What this spec deliberately does NOT attempt

- **Full theory-prose auto-generation.** Turning "the residual Helmholtz
  term list changed" into a correct paragraph of physics explanation is not
  something this pipeline does; it flags staleness for a human/AI writer
  instead. Overreaching here risks silently wrong physics docs, which is
  worse than a stale-but-flagged doc.
- **Touching crates with pre-existing non-LaTeX doc formats.** Two crates
  are on record as exceptions and must stay out of this workflow's write
  path unless a future decision changes that:
  - `teh-o-prke` — theory doc is Typst (`docs/prke_theory.typ`), not LaTeX.
  - `tampines-steam-tables` — has a full (not bite-sized) `ucbthesis`-class
    LaTeX derivation under `docs/derivation/`, pre-dating this convention;
    left as-is rather than retrofitted into the 4-part bite-sized structure.
- **Running on every push.** Only `main`, and only crates with `src/`
  changes since the previous `main` — see Trigger and step 1 above.

## Open questions before implementation

- Which LLM/API call handles the contradiction check (step 2), and how is
  its API key provisioned to the workflow securely (repo secret, OIDC to a
  model provider, etc.)?
- Where does the "tracking issue" from step 4 get filed — a GitHub issue, or
  a `bd` bead (per this workspace's beads convention in the root
  `CLAUDE.md`)? Likely a bead, for consistency with how the rest of the
  workspace tracks follow-up work — but beads syncs via `refs/dolt/data` on
  the git remote, which needs confirming works cleanly from a CI runner
  before committing to it here.
- Should the workflow run per-crate in a matrix (parallel, isolated
  failures) or sequentially in one job? A matrix is probably right once more
  than 2-3 crates opt in, to keep one crate's `latexmk` failure from blocking
  every other crate's docs from updating.

## Status

Spec only. No workflow file exists. Do not implement without an explicit
go-ahead — this was an explicit scoping decision when the three-folder
convention was scaffolded (2026-07-10): "scaffold only, no CI yet."
