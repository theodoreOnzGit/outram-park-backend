<!-- Moved verbatim from the workspace CLAUDE.md on 2026-09-21 (maintainer direction: physics
     rules stay in the root, everything else here behind a pointer). STILL BINDING. -->

## Use KOVAN for repository context

`kovan` is this workspace's own deterministic knowledge layer — reach for it
first for repo understanding, symbol/code queries, and literature scoped to
this codebase. **Use `kovan-cli`**, the agent front end.

- **Token-frugal reading:** `kovan-cli cost <path>` (real BPE-approximation
  estimate), `kovan-cli outline <file>` (declarations skeleton),
  `kovan-cli slice <file> <start> <end>`. Prefer this
  `cost → outline → refs → slice` loop over reading whole large files.
- **Symbol queries:** `kovan-cli def|sig|refs <symbol> --file <file>`,
  rust-analyzer-backed (needs `rust-analyzer` on PATH). The *first* query for
  a workspace root pays a real indexing wait (up to `KOVAN_RA_TIMEOUT_SECS`,
  default 180 s); that root then stays warm in a background `lsp-daemon`, so
  every later call answers in well under a second. There is deliberately **no**
  idle timeout. Stop it explicitly with
  `kovan-cli lsp-daemon-stop --root <root>`.
- **Never run `kovan`, `kovan-tui` or `kovan-cli lsp-daemon-serve` directly**
  in a non-interactive session — the first two are GUI/TUI front ends and the
  third is the daemon's own foreground process; all three will hang.

Run `kovan-cli skill-gen` to (re)generate `kovan_skill.md` for an agent
session that hasn't read this file.

**KOPITIAM is no longer dogfooded here** (maintainer direction, 2026-09-21).
It may still be used where it helps, but it is not mandated and its rough
edges no longer need filing from this workspace. The former rule, and the
`docs/kopitiam-issues/` queue it fed, are preserved in
[`docs/claude-md-rationale/tooling-kopitiam-kovan.md`](../claude-md-rationale/tooling-kopitiam-kovan.md).

### Literature handling (the kovan ingestion mandate was RETIRED 2026-09-21)

**~~ANY literature ingested OR USED goes into kovan (HARD RULE)~~ and ~~READ
AND WRITE THE LITERATURE LIBRARY THROUGH `kovan` (HARD RULE)~~ — RETIRED
2026-09-21 at the maintainer's request. Neither applies any more.** Literature
does **not** have to be routed into `crates/kovan-literature`, and reads of it
do not have to go through the `kovan lit` CLI. Do not enforce either rule, and
do not treat a paper that is not in the archive as a defect to fix. The full
original text of both rules is preserved in
[`docs/claude-md-rationale/tooling-kopitiam-kovan.md`](../claude-md-rationale/tooling-kopitiam-kovan.md).

`kovan lit import` / `outline` / `bibtex` and `kopitiam pdf2md` all remain
available and are still perfectly good tools — using them is now a choice, not
an obligation. `crates/kovan-literature/` and its `CATALOGUE.md` stay where
they are; existing citations into the archive remain valid.

**What survives this retirement, because it never came from `kovan` in the
first place** — these are `DATA_POLICY.md` and compliance obligations and they
still bind wherever a document lives:

- **The open/proprietary split.** Public, openly published literature is
  committable; anything restricted is not and must stay out of the repository.
  **Decide the access tier from the document's own copyright page**, not from
  where it was downloaded — public hosting (INIS, gen-4.org, a lab's website)
  grants no redistribution rights. **Unsure means proprietary**; that failure
  direction is recoverable and the other is a licence violation in a public
  repository. The root-level `collaboration/` directory is gitignored scratch
  and is **not** automatically open — ask if provenance is not stated.
- **Text-and-data-mining / AI-training reservations.** Where a copyright line
  carries one, record metadata and factual findings only and do **not** extract
  the full text — facts are not copyrightable, but the corpus is what that
  clause reserves.
- **Provenance for anything the code depends on.** If a document informs the
  code — a correlation taken from it, a benchmark value cited, a number in a
  doc comment — record its source, author, title, licence/access terms,
  URL/DOI, date accessed and any processing steps, per the "Responsible use &
  data policy" section. A citation that points at nothing a reader can reach
  is a dead reference. Where to put that record is now your judgement: a
  `References.md` beside the example, the relevant validation report, or the
  kovan archive.
### Graph digitisation (the `kovan-cli digitise` mandate was RETIRED 2026-09-21)

**~~Graph digitisation: dogfood `kovan-cli digitise` (HARD RULE)~~ — RETIRED
2026-09-21 at the maintainer's request, alongside the kovan literature
mandate above. It no longer applies.** Getting data off a published figure
does not have to go through `kovan-cli digitise`, the `kovan-tui` Digitiser
tab or the `kovan` GUI. Do not enforce it. The full original text is preserved
in [`docs/claude-md-rationale/tooling-kopitiam-kovan.md`](../claude-md-rationale/tooling-kopitiam-kovan.md).

The digitiser still exists in `crates/kovan/src/digitiser/` and still works —
using it is now a choice. Two of its properties are worth knowing if you do:
its accuracy is verified **against synthetic ground truth only** (never
against real published figures), and **only a human can mark a dataset
reviewed** — an agent session cannot, and a CLI run can only ever emit
`Unreviewed`.

**What survives, because it was never a `kovan` rule:** if a number in this
codebase came off a plot, **say so and say how**. Record the figure it came
from, the axis calibration or reference points used, the scale (linear or
log), and whether the reading was automatic or by eye. That is the
"Responsible use & data policy" provenance obligation and the
"Verification & validation documentation" rule, not a tooling preference — a
digitised value with no record of how it was read is not a citable number,
whatever produced it.
