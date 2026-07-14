# Documentation

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** All code in this workspace is **unverified and untrusted** unless a specific verification & validation (V&V) case demonstrates otherwise. V&V cases are human-reviewed and are intended for journal / arXiv publication — that is the trust workflow. See the workspace `VERIFICATION_AND_VALIDATION.md` and `RESPONSIBLE_USE.md`. Not for nuclear facility operation, reactor control, safety-critical, or licensing decisions.


This folder has three kinds of content, kept deliberately separate:

## `api.md` — auto-generated API reference

A single-file Markdown mirror of every public item's doc comment (structs,
enums, fields, functions, module docs), generated from rustdoc's own JSON
output — not scraped HTML — via
[`rustdoc-md`](https://github.com/tqwewe/rustdoc-md). Regenerate it any time
with:

```bash
python3 scripts/gen_api_docs.py outram-park-fork-coolprop
```

(needs a nightly toolchain — `rustup toolchain install nightly` — since
rustdoc's JSON output is nightly-only; the script installs `rustdoc-md`
itself if it isn't already on `PATH`.) It fully overwrites `api.md`, so hand
edits there don't survive a regeneration — this file is a mirror of the doc
comments, not a place to write prose. See `scripts/gen_api_docs.py`'s own
docstring for why this uses the JSON pipeline instead of an HTML scrape (an
earlier version of the script did scrape `cargo doc`'s HTML output with
pandoc; it silently truncated large enums because rustdoc's HTML defers
long variant lists to a JS-driven "Show N variants" widget, and it was
replaced before ever being committed).

## `code_structure.md` — how the code is laid out

Markdown, describing the crate's file/module structure: what lives where and
why, cross-references between modules. Read this first if you're orienting
yourself in the source tree. Update it by hand when the module structure
changes materially (a new top-level module, a file split, etc.) — it is not
auto-generated.

## `latex/` — bite-sized theory → numerics → implementation → walkthrough docs

Each topic gets a short (≤10 page) series of `.tex` files, read in order:

1. **Theory** — the physics/mathematics on its own terms, independent of any
   particular implementation. What is being modelled and why the equations
   take the form they do.
2. **Numerical methods** — how that theory becomes a computable algorithm:
   singularities to guard against, iterative solves, tolerances, and why.
3. **Implementation** — how *this crate specifically* structures the code for
   that algorithm (the enum-dispatch pattern, data layout, where the const
   data comes from).
4. **Walkthrough** — a concrete worked example, traced function-by-function
   from the public API down to the verified numerical result, naming exact
   files and functions so a reader isn't left guessing what to open next.

This progression (theory first, pure of any code; numerics next; then
implementation; then a full walkthrough) is intentional — it's written to be
usable as a script for a video tutorial as much as a reference document: a
viewer should be able to follow the *idea* in Parts 1–2 before any Rust
appears in Part 3, then see that idea land as real, running, verified code by
Part 4.

**Worked example:** `latex/01_theory_non_analytic_term.tex` through
`latex/04_walkthrough.tex` — the IAPWS-95 non-analytic critical-region term
(bead op-kbc.6), chosen because it has a complete, self-contained story this
session: a real piece of physics, a real numerical subtlety (the
branch-point offset guard), a real enum-dispatch implementation, and a real
verified result (`5.2e-14` relative error at Water's critical point). Use it
as the template for new bite-sized doc series — same four-file structure,
same shared `preamble.tex`/`references.bib`.

Build any file with `latexmk -pdf <file>.tex` (needs `pdflatex`, `biber`);
`latexmk -c` cleans build artifacts (`.aux`/`.bbl`/`.bcf`/… — see
`latex/.gitignore`, the standard GitHub LaTeX template). Compiled `.pdf`s are
committed (so they're viewable on GitHub without a local LaTeX install); the
build-artifact files are not.

## What's *not* here (yet)

- **Auto-generation.** The plan is for `latex/` (and `code_structure.md`) to
  regenerate automatically when `develop` merges into `main`, via a Python
  codegen (`dev/gen_latex_doc.py` — currently a scaffold that emits a new
  bite-sized file's skeleton from a title + section outline, not full
  auto-generation from source) and a CI workflow. The workflow itself isn't
  written yet — see the workspace-root `docs/docs-ci-spec.md` for the
  intended design, pending review before it's wired up.
- **Manual notes contradicting generated docs.** When the auto-generation
  pipeline above is built, it should check any hand-added notes in this
  folder against what it's about to generate and flag contradictions rather
  than silently overwrite — this crate doesn't yet have hand-added notes to
  reconcile against (some other OUTRAM PARK crates do, e.g.
  `tampines-steam-tables/docs/notes.md`).
