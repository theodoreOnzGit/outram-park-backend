# Kovan redesign — keep / adapt / migrate / replace / delete-after-parity map

Deliverable for **§47.3** of [GitHub issue #35](https://github.com/theodoreOnzGit/outram-park-backend/issues/35)
("Produce an internal keep/adapt/migrate/replace/delete-after-parity map"),
which the issue requires *before* any code moves.

**Surveyed:** 2026-08-31, `develop` @ `5e504e3f`. `crates/kovan/src` is
**19,811 lines** across 44 files.

§47.8 governs how to read this table: *"Do not blindly rewrite code merely
because this architecture differs from the current implementation."* Every
**REPLACE** and **DELETE AFTER PARITY** below names the thing that must exist
and pass first.

## 1. Inventory and verdicts

| Path | Lines | Verdict | Why |
|---|---|---|---|
| `digitiser/gui/desktop/mod.rs` (`DigitiseApp`, `View`) | 1,547 | **REPLACE shell, KEEP panels** | Its `View` enum *is* the five implementation tabs §25 removes |
| `digitiser/gui/desktop/pdf_reader.rs` | 2,174 | **DELETE AFTER PARITY** | §24 — superseded by `kopitiam-pdf`'s reusable reader |
| `digitiser/gui/desktop/pdf_annots.rs` | 649 | **DELETE AFTER PARITY** | Annotation rendering belongs to the same reader |
| `digitiser/gui/desktop/markdown_editor.rs` | 274 | **REPLACE** | §26/§27 — `kopitiam-neovim` engine + egui adapter |
| `digitiser/gui/desktop/bibliography.rs` | 344 | **ADAPT** | Becomes §29 citation autocomplete over `kopitiam-bibliography` |
| `digitiser/gui/desktop/table_digitiser.rs` | 325 | **KEEP, demote** | §25 contextual tool; §36 defers its UI |
| `digitiser/gui/desktop/{theme,csv_preview}.rs` | 152 + ~90 | **KEEP** | Presentation, architecture-neutral |
| `digitiser/{calibration,dataset,trace,detect,auto,raster,synthetic,table_ocr,frontend}.rs` | ~3,300 | **KEEP** | The digitiser *engine*. §36 explicitly defers; §20's output contract is already CSV + provenance |
| `project.rs` | 909 | **MIGRATE** | Different model, not an older version of the same one — see §3 below |
| `tui/**` | ~3,600 | **KEEP, out of scope** | Android/Termux-critical; issue #35 is GUI-scoped |
| `commands/**` | ~3,000 | **KEEP** | Agent-facing CLI, orthogonal to the wiki redesign |
| `bin/{kovan,kovan-cli,kovan-tui}.rs` | ~800 | **KEEP** | The three-binary layout survives unchanged |

Roughly **3,100 lines are delete-after-parity**, **~1,850 replace/adapt**, and
the remaining **~14,800 are keep**. This is not a rewrite.

## 2. Structural findings

**The app root is misnamed and mis-nested.** The whole GUI lives under
`src/digitiser/gui/desktop/`, and its root struct is `DigitiseApp` — the
digitiser *owns* the application. §25 inverts this: the Research workspace owns
the application, and digitisers become contextual tools launched from it. The
move is `src/digitiser/gui/` -> `src/gui/`, with the digitiser demoted to a
panel. This is a module move plus a rename, not a rewrite of the panels.

**`View`'s default is literally what §2 forbids.** `desktop/mod.rs:42`:

```rust
enum View { Digitiser, #[default] PdfReader, MarkdownEditor, Bibliography, TableDigitiser }
```

§2 requires landing on recent-roots / open / create; §8 requires landing in the
Wiki after a root is open. Both replace this default.

**`FileDialogTarget` already has the two directory targets** (`ProjectFolder`,
`BibliographyFolder`, `desktop/mod.rs:56-71`) that §2's directory picker needs.
Adapt, don't add.

## 3. `project.rs` is a *different model*, not an earlier draft

`src/project.rs` (909 lines, beads `op-63u0` design / `op-b1y5` implementation,
spec in `docs/kovan-folder-format.md`) is **Done** and working. It is superseded
by §3/§5/§7, and the two models are incompatible in three ways:

| | Today (`op-63u0`) | Issue #35 |
|---|---|---|
| Layout | `pdf/` + `markdown/` + one `.bib` | `papers/` + `topics/` + `projects/` + `literature/{open,proprietary}/pdf/` |
| Root marker | `kovan.toml` (generated index) | `kovan_root.toml` (library) + per-entity `kovan.toml` |
| Section model | **closed** set of 6 (`SECTION_ORDER`: `ai_summary`, `author_summary`, `full_text`, `table_csvs`, `graph_csvs`, `annotations`) addressed by regenerated **line ranges** | **open** set of artifacts = heading + fenced `toml` with `[kovan]`, addressed by **stable `id`** (§13/§14/§40) |
| Document id | filename stem of `pdf/<stem>.pdf` ∩ `markdown/<stem>.md` | BibTeX **citekey** (§7 amendment) |

Line ranges are invalidated by every edit and must be regenerated; stable ids
are not. **Do not try to evolve `SECTION_ORDER` into the artifact model** —
write the fenced-TOML parser (step 12) alongside it and migrate, keeping
`project.rs`'s 8 tests green until a migration path exists.

**What to reuse from it:** the atomic temp-file+rename `write_index`, the
`GENERATED FILE — do not edit by hand` header convention, and its
reconciliation with `docs/kovan.md`'s "only the Rust struct is authoritative"
rule — all three carry over verbatim to `.kovan/` derived state.

**`save_into_project` (`op-96am`) is the closest existing analogue to §19/§20.**
It already appends a digitiser CSV, with page/pixel/date/author provenance, into
a named section of a project markdown file, driven by `CropProvenance`. The
artifact-append path should be adapted from it, not written fresh.

## 4. Verified dependency facts

- **`gix` is present but read-only.** Root `Cargo.toml:172` pins
  `gix = "0.85"`, `default-features = false`, features
  `sha1, revision, status, blame, dirwalk, index` — used by `kovan-discovery`.
  Step 4 needs **write/init/commit features added**, not a new dependency, as
  the issue's own checklist states. Confirmed accurate.
- **`kopitiam-pdf` step 11 is genuinely blocked.** Latest published is
  **0.3.1**, whose features are `default = ["kpdf"]`,
  `kpdf = ["dep:eframe", "dep:egui", "dep:rfd"]`. The separate `egui` feature
  that kopitiam#96 describes exists in no published version. `crates/kovan`
  already consumes `kopitiam_pdf::mupdf::{PdfDocument, rasterize_page,
  page_to_stext, interpret::Processor}` directly — the *rendering* half is
  already reused; only the *reader shell* is duplicated.
- **`kopitiam-bibliography` closes the citekey gap.** It is **unpublished**
  (crates.io index 404, verified 2026-08-31); the maintainer will publish it as
  **v0.0.1**. Its `bibtex::parse::parse_database` is the BibTeX **parser**
  `op-b1y5` recorded as missing and §7's amendment requires. Tracked as
  `op-k25f`; AGPL-3.0-only, approved (see `crates/kovan-literature/NOTICE`
  when written).

## 5. Ordering consequences

1. **Steps 5 and 12–16 are gated on the BibTeX parser** (`op-k25f`), because
   citekey-as-id is load-bearing for the paper directory name, the Markdown
   filename, and every `[[…]]`/`[@…]` target.
2. **Step 11 is gated on `kopitiam-pdf` 0.3.2+** (kopitiam#96). Steps 2–10 and
   12–16 do not touch the reader and can proceed in parallel.
3. **Step 4 is gated on nothing** — it is a feature-flag change to a dependency
   already in the tree.
4. §47.7's "migrate in small compilable stages" plus this workspace's
   release-mode rule means `cargo check --workspace --lib --tests` and
   `cargo test --release -p kovan` gate every stage, not just the last.
