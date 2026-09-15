# KOVAN — Epics and Beads (registry-first redesign)

> **Staging doc — pending import into the tracker.** This is the maintainer's
> planning sketch for the KOVAN knowledge layer, to be filed as issues under the
> existing KOVAN epic **`op-5v5`** once kopi-beans (`bn`) can actually read
> this repo's store (see `CLAUDE.md` → "Issue tracking & roadmap — kopi-beans"
> — as of 2026-08-07 this is blocked by
> [kopitiam#16](https://github.com/theodoreOnzGit/kopitiam/issues/16)). File one
> sub-epic per `E#`, one child issue per `E#.n`, wiring the dependency edges
> described in the ordering notes. Preserve the `[VERIFY]` flags as issue notes —
> those issues likely overlap existing `op-5v5.1..7` children and must be
> reconciled (not blindly duplicated) against them before/at filing.
>
> **Status:** planning sketch, written from design discussion. Not reconciled
> against the existing `kovan-*` crates — several beads below may already be
> done or partly done. Marked `[VERIFY]` where overlap is most likely.
>
> **Ordering principle:** the correlation registry is the load-bearing piece.
> Everything else hangs off its schema. Build it first; it is the only part
> that can invalidate the rest of the design.
>
> **Deferred:** the LLM layer (E7) is deliberately last. Nothing above it
> depends on a model existing.

---

## E1 — Correlation registry (foundation)

The typed schema for empirical correlations, stored as reviewable TOML,
content-hashed for reproducibility. This is the first thing to build and the
thing that proves or breaks the design.

**Done when:** five real correlations from real papers are encoded, CI
validates them, and `kovan query` returns them as JSON.

### E1.1 — `Correlation` record type
- Struct in `kovan-common`: stable ID, role, functional form, coefficients,
  validity, provenance, V&V status.
- `#[serde(deny_unknown_fields)]` — a typo'd validity key must be a load
  error, not a silently widened range.
- Stable serialized discriminants for all enums; Rust variant names are
  display-only and must be renameable without breaking stored records.
- Round-trip test: struct → TOML → struct, byte-identical.

### E1.2 — Functional form enum
- Closed enum: power-law product, polynomial-in-log, piecewise blend,
  tabulated (reserve the variant now, digitisation tooling comes much later).
- Adjacent tagging for serde — internally-tagged breaks on some nested TOML
  cases. Test both against a real record before committing.
- Each variant knows how to evaluate itself.
- **Explicitly out of scope:** arbitrary expression parsing. Revisit only if
  E1.6 shows fixed forms failing on real literature.

### E1.3 — Role enum and closure seam
- `ClosureRole` — closed, reviewable set (wall heat transfer, two-phase
  friction multiplier, …).
- Trait at the solver boundary so `tampines` asks for a number without
  knowing which correlation answers.
- Trait implemented over the `Correlation` data, not over one struct per
  correlation. Anything to be cited, hashed, or put in a V&V case must be
  data, not a type.

### E1.4 — Units decision
- Decide: fixed SI everywhere, or units carried in the record.
- Document it in the schema and enforce it in E1.5.
- Small bead, but a classic source of silent error — literature is
  inconsistent and the failure is invisible.

### E1.5 — Validation binary
- Loads every TOML in `closures/`. Checks ID uniqueness, form matches
  coefficient set, validity ranges non-empty and correctly ordered,
  provenance present, units conform.
- Non-zero exit on any failure. Wire to CI.
- Cheap now; it is what makes a merged PR trustworthy without re-reading
  every number by hand.

### E1.6 — Seed the registry with real correlations
- Five correlations from papers actually on hand. Dittus-Boelter is the easy
  one; pick at least one piecewise or blended form, since that is where a
  schema usually cracks.
- **This bead is the design review.** If encoding these is painful, stop and
  revise E1.2 before building anything on top.

### E1.7 — Canonical serialization and content hashing
- Parse → canonical form (sorted keys, fixed float formatting) → hash.
- Shortest-roundtrip float formatting, documented and pinned. Hashing raw
  file bytes gives false diffs on whitespace.
- Simulation runs record the hashes of every correlation used.

### E1.8 — Append-only ID discipline
- Written policy plus a CI check: an existing ID must never change meaning.
- Corrections get a new ID; the old one is deprecated with a pointer.
- Otherwise a pinned config silently changes behaviour between runs.

### E1.9 — File layout
- `closures/<role>.toml`, split by role rather than by paper, so reviewers
  see all candidates for a role together.
- `[[correlation]]` arrays of tables — keeps IDs as validatable string
  fields and avoids dotted-key awkwardness.

---

## E2 — Query surface

`kovan-cli` as a tool protocol first, human CLI second. Designed so the
eventual model layer is just another caller.

**Done when:** `kovan query --role wall-heat-transfer --re 4e4 --pr 6`
returns matching records as JSON, deterministically ordered.

### E2.1 — Query type in `kovan-common` `[VERIFY]`
- One `KovanQuery` → `Vec<KovanDocument>` shape with a source discriminant,
  rather than a separate retrieval API per layer.
- Likely overlaps existing `kovan-common` types — check before writing.

### E2.2 — Registry filter
- Filter by role, then by conditions falling inside validity ranges.
- Out-of-range is a legitimate empty result, not an error.

### E2.3 — Output renderers
- Line-oriented (existing intent) and `--format json`, same command surface.
- JSON is what a tool harness needs; line-oriented stays good for piping.

### E2.4 — Deterministic ordering
- Sort by a stable key, always. Identical queries must return identical
  ordering or caching and reproducibility claims quietly break.

### E2.5 — Error taxonomy
- Distinguish empty-result (success, zero records) from malformed-query
  (error) from registry-corrupt (error). Distinct exit codes.
- A caller that cannot tell these apart will retry pointlessly.

### E2.6 — Bounded output
- Result limit and explicit truncation marker on every command.
- An unbounded search must not silently return a partial result.

### E2.7 — Read-only by default
- Query commands never mutate. Anything that writes emits a draft for human
  review instead.

### E2.8 — Surface versioning
- `--api-version`, and a version field in JSON output.
- A caller built against an older surface should fail loudly rather than
  misread fields.

---

## E3 — Literature ingestion

PDF in, searchable KovanDocument out. Born-digital only for now.

**Done when:** a supplied PDF is ingested, searchable offline, and openable
at the page a result came from.

### E3.1 — Ingest command `[VERIFY]`
- `kovan literature ingest <path.pdf>`. User supplies files directly — no
  API keys, no bulk archive plumbing.
- Likely overlaps existing `kovan-literature` work.

### E3.2 — Text layer detection
- Detect whether a text layer exists and is plausible.
- Record the answer as an extraction-confidence field on the document.
  Downstream behaviour depends on it.

### E3.3 — Born-digital extraction
- PDF → Markdown. `pdf-extract` / `lopdf`, or shell out to `pdftotext` if
  the dependency is acceptable on desktop.
- Note: shelling out is a problem on Android — may push toward
  ingest-on-desktop, sync-extracted-Markdown-to-phone.

### E3.4 — Store originals and page images
- Keep the source PDF and per-page rendered images.
- The high-value feature is "here is the page it came from, open it" — not
  perfect extraction. This sidesteps extraction quality as a correctness
  concern entirely.

### E3.5 — Content-hash dedup
- Same paper supplied twice under different filenames is one document.

### E3.6 — Sidecar metadata
- Optional `.toml` beside the PDF, or metadata flags on ingest.
- Do not try to parse title pages. For ORNL reports the report number is the
  reliable identifier.

### E3.7 — Per-document license field
- Public domain / CC-BY / publisher-licensed.
- Decides what could ever ship in a bundled corpus.

### E3.8 — Offline search index
- SQLite FTS5 over extracted Markdown. Derived data, rebuildable by
  re-ingesting — never the source of truth.
- Works offline with no model, which fits the Android-first constraint.

### E3.9 — Draft extraction into `closures/drafts/` `[VERIFY]`
- Extraction proposes a candidate record with `vv_status = "unvalidated"`
  and provenance pointing at document, page, and equation number.
- Validation binary checks draft schema; the loader ignores drafts.
- **Promotion is a human editing a file and committing it.** This is the only
  point where someone checks the exponent is 0.8 and not 0.6.

---

## E4 — Cross-layer links

The reason to unify the layers at all. Cheap to add as fields now, expensive
to retrofit once each layer has its own conventions.

### E4.1 — Correlation → source document
- Record points at the ingested document ID and the original DOI/report
  number, plus page.

### E4.2 — Correlation → implementing symbol
- Which crate and function implements this correlation.

### E4.3 — V&V case as a linking record
- A V&V case points at the correlations it validates and the code it
  exercises; records point back at the case.
- Closes the loop the README's trust workflow describes.

### E4.4 — Cross-layer query
- Answer "which crate implements this correlation, from what paper, and has
  it been validated" in one call.

---

## E5 — Repo semantics

**Done when:** symbol lookup and search work across the 22-crate workspace
fast enough to be interactive.

### E5.1 — Libraries, not binaries `[VERIFY]`
- `grep-searcher` / `grep-regex` / `grep-matcher` + `ignore` — ripgrep's
  actual engine, giving structured match results instead of parsed stdout.
- `gix` for git-awareness. `tokei` as a lib if line counts are wanted.
- Not eza: it is a display tool with no library core. Render trees directly
  in `kovan-tui`; reuse `ignore` for the walk.
- A shelled-out binary is a runtime dependency that must exist on the
  target — a real problem on Android.

### E5.2 — Symbol index
- Persistent index populated by ripgrep, refined lazily by a language server.
- LSP startup across 22 crates is slow and likely infeasible on Android, so
  the ripgrep-first path must stand alone.

### E5.3 — Symbol lookup over similarity
- `find_symbol`, `find_references`, `show_impl`.
- Do not embed the codebase. For Rust, symbol-level lookup beats similarity
  search almost always.

---

## E6 — Deterministic codegen

Generation for known numerical methods. Typed and testable; nothing about
correctness depends on a model.

### E6.1 — Typed generator interface
- A generator takes a parameter struct of enums and numbers, not free text.
  An invalid request fails at construction.
- `RkIntegrator { order: Rk4, state_dim: 6 }`, not template substitution.

### E6.2 — Emit via syntax tree
- `syn` + `quote` + `prettyplease`. Malformed output becomes near-impossible
  and formatting is free.
- String templates work until a parameter contains a brace.

### E6.3 — Generator catalogue
- Stable IDs, provenance (which paper defines the method), V&V status.
- Mirrors the correlation registry so "which RK scheme did this run use" is
  answerable the same way "which Nusselt correlation" is.

### E6.4 — First generators
- RK integrators, TDMA/Thomas, MUSCL limiters, PID discretisations.
- Chosen because they are hand-written repeatedly and mistakes in them are
  subtle.

### E6.5 — Generated code is derived
- Committed with a do-not-edit header and a hash of the generator input, so
  staleness is detectable.

### E6.6 — Property tests
- Generated RK4 against an analytic solution, etc. Ordinary Rust tests.

---

## E7 — LLM layer (deferred)

Nothing above depends on this. Listed for shape only; do not start until
E1–E2 are done and `kopitiam-runtime` is logit-verified.

### E7.0 — Logit verification of `kopitiam-runtime` **(blocks everything else here)**
- Fixed prompt, f32, first-token logits vs HF transformers or llama.cpp.
  Max abs diff under ~1e-4.
- Repeat at token ~20 to catch KV cache and RoPE position errors.
- On divergence, bisect by layer: after embedding, after layer 0, after
  layer 1. Usual culprits are RoPE theta/indexing, RMSNorm epsilon placement,
  attention scaling, GQA head grouping.
- Commit reference logits as a test fixture so it stays true under
  optimization.
- **Do before quantization and before wgpu** — debugging both at once is
  miserable.

### E7.1 — Dependency seam
- `kopitiam-ai` only, pinned by git rev. Nothing in outram-park names
  `kopitiam-runtime`, `-loader`, or `-tensor`.
- If a KOVAN crate wants a type from below the seam, that means `ModelAdapter`
  is missing something — not that a second dependency is warranted.
- Depend from as few crates as possible, ideally only where generation
  actually happens.
- `[patch]` at the workspace root for local development; git rev is the
  committed truth.

### E7.2 — Templated rendering fallback
- Runtime branch, not a feature gate: no model available → render from
  templates.
- Deterministic, zero hallucination risk, and reads acceptably for
  structured records. Keeps the deterministic path always available.

### E7.3 — Baseline measurement
- Unmodified Llama 3.2 1B, retrieved records in prompt, twenty real
  questions. Measure before assuming a fine-tune is needed.
- Also the only way to know later whether fine-tuning helped.

### E7.4 — Query parsing without a model
- Try a hand-written grammar over the role enum plus a units-aware number
  parser first. It will beat a small model on accuracy, run in microseconds,
  and cannot hallucinate a role that does not exist.

### E7.5 — Tool schema and constrained decoding
- Fixed, enumerated, typed commands. No general "run this command" escape
  hatch, at any model size.
- Logit masking against the schema at decode time. For a 1B this matters
  more than fine-tuning: malformed output becomes structurally impossible.

### E7.6 — Grounding checks
- Require a stable ID citation for any correlation mentioned; post-validate
  that every cited ID exists and was actually returned by a retrieval call
  in that turn.
- V&V status and validity ranges ride along in context so an unvalidated
  correlation cannot be presented as settled.

### E7.7 — Training data generator
- `kovan` subcommand emitting JSONL from the registry. Rust side owns the
  logic; Python is a thin fine-tuning script.
- Include negative cases — out-of-range conditions, roles with no match,
  queries that should return nothing. Those teach the model not to fabricate.

### E7.8 — Fine-tune, if the baseline warrants it
- LoRA rank 16–32 on attention and MLP projections. Python: `transformers` +
  `peft` + `trl`, convert to GGUF.
- A few thousand examples. Train in Python, ship in Rust — training is a
  one-off, inference ships.

### E7.9 — Adapter provenance
- Base model ID, adapter hash, registry content hashes, timestamp.
- Slots into `kopitiam-models`' existing catalog + sha256 structure, so an
  answer traces to a specific model trained on specific records.

### E7.10 — Write path, if ever
- Diffs against a known base, not whole files. Scratch branch via `gix`,
  never the working tree. `cargo check` as a gate.
- The registry is never model-writable. Drafts only, promotion stays human.

---

## E8 — Later / parked

Reserve the schema slot now, build the tooling when needed.

### E8.1 — OCR for scanned reports
- ORNL MSRE-era material is 1960s typescript; OCR quality on tables and
  equations will be poor.
- Treat OCR output as a search index and a pointer to the page image, never
  as a source of numbers. Never auto-draft correlations from it.

### E8.2 — Vector-PDF figure extraction
- Curves in born-digital figures are path objects — extract coordinates
  directly, no computer vision. This is the automatable case; build it first.

### E8.3 — Raster figure digitisation
- Threshold, skeletonize, trace. Mandatory visual overlay verification
  before promotion: extraction proposes, human confirms the overlay matches.
- Multi-curve 1960s scans with gridlines will fail silently. Do not trust
  them.

### E8.4 — FSAR parameter extraction
- Extract typed parameters (geometry, materials, operating conditions,
  setpoints), not narrative.
- Same draft/promote path as everything else.

### E8.5 — Unvalidated status propagation
- **Structural, not conventional:** any result computed from an unvalidated
  input is itself marked unvalidated, all the way into simulation output.
- A simulation built from FSAR-extracted parameters looks authoritative in a
  way a hand-built case does not. This is what keeps the README banner honest
  at the far end of the chain.
