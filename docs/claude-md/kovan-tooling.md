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

### Agents change kovan's files only through `kovan-cli`

**Maintainer direction, 2026-10-22: "agents should use kovan-cli to interact
with kovan.toml".** An agent reads and writes kovan's own files (`kovan.toml`,
and the review artefacts kovan manages beside them, such as `review.md`
entries and the `[[reviewer]]` registry in `kovan_root.toml`) **through
`kovan-cli`, never by editing them by hand.** The CLI validates, hashes and
formats what it writes; a hand edit can silently break an anchor, a hash or
a signature.

- **If the command an agent needs does not exist,** do not hand-edit as a
  workaround: say so in the hand-off and file an issue for the command
  (e.g. agent justifications on the reviewer roster, GitHub #1019).
- **`kovan-cli` makes unsigned review entries only** (maintainer,
  2026-10-22: "kovan-cli should only make unsigned reviews"): agent
  justifications, drafts and needs-fix entries, labelled agent-drafted. It
  has no signing command, by design (#762), and must never gain one. A
  signed stamp is made only in desktop kovan, by a human with their key; an
  unsigned entry never counts as review (GitHub #1019).

**KOPITIAM is no longer dogfooded here** (maintainer direction, 2026-09-21).
It may still be used where it helps, but it is not mandated and its rough
edges no longer need filing from this workspace. The former rule, and the
`docs/kopitiam-issues/` queue it fed, are preserved in
[`docs/claude-md-rationale/tooling-kopitiam-kovan.md`](../claude-md-rationale/tooling-kopitiam-kovan.md).

### Literature and digitisation

- **The maintainer supplies the kovan corpus** and ingests literature
  manually. Do not ingest literature into kovan on your own initiative.
- **From time to time the maintainer may ask you to hardcode the mind map, the
  standard corpus or annotations in `kovan-literature`.** Do it when asked;
  how is in [`crates/kovan-literature/CLAUDE.md`](../../crates/kovan-literature/CLAUDE.md).
- `kovan lit` and `kovan-cli digitise` are optional tools, not mandates.
- `kovan-cli zotero` imports a Zotero library or file into a Kovan folder and
  exports to every Zotero format (GitHub #752; reference in
  `crates/kovan/README.md`). It acts only on paths the user names: never
  read a user's Zotero library unasked, and never import into or export to
  this repository or `reactor-literature` (refused by default).
- The provenance, access-tier and digitisation-record obligations come from
  `DATA_POLICY.md` and the root `CLAUDE.md` ("Responsible use & data policy",
  "Verification & validation documentation"), not from kovan.
- **The retired kovan rules** (literature ingestion, read-through-`kovan lit`,
  `kovan-cli digitise`) and their history are in
  [`docs/claude-md-rationale/tooling-kopitiam-kovan.md`](../claude-md-rationale/tooling-kopitiam-kovan.md).
  Do not act on them.
