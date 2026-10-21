<!-- Moved verbatim from the workspace CLAUDE.md on 2026-09-21 (maintainer direction: physics
     rules stay in the root, everything else here behind a pointer). STILL BINDING. -->

## Bookkeeping pass (maintainer command)

When the maintainer asks for a **"bookkeeping pass"** (or "bookkeeping", "book
keeping", "update the docs + flags") over one or more crates, run this fixed
routine. It keeps the docs, the completeness flags, and the issue tracker honest
and in sync with the code. It is a recurring command, not a one-off.

**The four steps:**

1. **Doc-comment pass — fill gaps + fix stale (NOT a rewrite).** For every
   public `fn` / `struct` / `enum` / `trait` / `mod` in scope: add an accurate
   `///` / `//!` where missing; fix any doc that contradicts the current code
   (stale "scaffold only" / `todo!()` claims, wrong counts, renamed items);
   and **leave already-accurate docs untouched** — do not reword good docs.
   Obey the "Human interface layer" rule above (what physical quantity, valid
   ranges/assumptions, units even when `uom`-typed). Never strip `uom`.

   **Then regenerate the rustdoc → markdown API mirror** for each crate whose
   doc comments changed, so `docs/<crate>-api.md` stays in sync with the code:

   ```bash
   kovan-cli api-docs <crate-dir-name>               # e.g. outram-foam-basic-lib
   ```

   Use **`kovan-cli`**, not `kovan` — `kovan` is the egui GUI binary and will
   hang a non-interactive session trying to open a display.

   This runs `cargo +nightly doc --no-deps` → rustdoc JSON → the `rustdoc-md`
   binary → `crates/<crate>/docs/<crate>-api.md`. Both prerequisites are **mandatory —
   install them, do not skip the mirror** (see "API-doc toolchain" below).
   `docs/` is `exclude`d from the packaged crate, so this mirror is repo-only
   and never
   ships to crates.io.

2. **Completeness flags in the README.** Every crate's `README.md` carries a
   **`## Bookkeeping status`** block with two axes the *human maintainer* must
   personally sign off:
   - **Verification & Validation (V&V) — human-reviewed**
   - **Human / user interface — human-reviewed**

   Both default to **❌ Not yet manually checked** and a crate is marked
   **INCOMPLETE** until the maintainer clears both. AI assistants must **not**
   flip either axis to checked/✅ on their own — only the human does, because
   these axes record *human* review (see `RESPONSIBLE_USE.md`: AI output is
   untrusted draft material until a human reviews it). A crate flagged
   INCOMPLETE on either axis is not ready to be described as validated or
   trusted. The canonical block:

   ```markdown
   ## Bookkeeping status

   > Maintainer sign-off tracker (see the workspace `CLAUDE.md` "Bookkeeping
   > pass" command). A crate is **complete** only once the maintainer has
   > personally signed off on BOTH axes below.

   | Axis | Status |
   |---|---|
   | Verification & Validation (V&V) — human-reviewed | ❌ Not yet manually checked |
   | Human / user interface — human-reviewed | ❌ Not yet manually checked |

   **Status: INCOMPLETE** until both axes are manually checked and cleared by the maintainer.
   ```

3. **Staleness audit.** Sweep the READMEs, the open GitHub issues, and every
   markdown file (recursively) for drift versus the actual code/state:
   internal contradictions, references to renamed/removed crates or files,
   "planned/TODO" items that are actually done, wrong member lists or crate
   counts, issues that should be closed (or reopened). Fix in-crate drift; for
   cross-cutting or issue-state changes, report candidates rather than
   silently editing — **issues are closed by the maintainer's decision**, and
   the read-only auditor never mutates them.

4. **Codify / update** this command here if the routine itself changes.

**How to run it as a fleet:** partition strictly by crate (one agent per crate,
no shared files → `cargo fmt -p` is safe to avoid per the parallel-agent rule),
plus a separate **read-only** agent for the cross-cutting markdown + issue
staleness audit (it must skip the crates being actively edited to avoid read
races). Commit any pending verified work first so the tree is clean, and
**exclude from the pass any crate with a publish in flight** (an uncommitted
doc edit trips `cargo publish`'s dirty-tree guard).

## API-doc toolchain: install it, never route around it (HARD RULE)

**`rustdoc-md` and a nightly Rust toolchain are required tooling in this
workspace, not optional extras. If either is missing, INSTALL IT.**

```bash
rustup toolchain install nightly          # rustdoc's JSON output is nightly-only
cargo install rustdoc-md --locked         # rustdoc JSON -> markdown
```

Two things depend on them, and both are load-bearing:

- **`kovan-cli api-docs <crate>`** — regenerates `crates/<crate>/docs/<crate>-api.md`, the
  committed markdown mirror of a crate's public API and the third leg of the
  per-crate `docs/` convention. Step 1 of the bookkeeping pass runs it. (It
  replaced `scripts/gen_api_docs.py`, retired 2026-08-14, so the doc toolchain
  needs no Python interpreter — same reasoning as epic `op-yz7b`.) Use
  `kovan-cli`, **not** `kovan` (the GUI binary — it hangs a headless session).
- **`kovan-cli agent-docs-gen --regenerate-missing`** — generates a mirror for a
  crate that has none, so it can be bundled for an external agent.

**Never report a mirror as un-regenerable because a tool is missing.** Installing
`rustdoc-md` takes one command; skipping the mirror leaves `docs/<crate>-api.md`
silently contradicting the code, which is exactly the drift the bookkeeping pass
exists to prevent. "The toolchain isn't installed" is a task, not a finding.

### Check before you claim a tool is absent

**Run the check. Do not infer it from a failure, and do not assume.**

```bash
which rustdoc-md && cargo install --list | grep rustdoc-md
rustup toolchain list
```

This is a rule because an agent once shipped an issue/commit/hand-off claiming
`rustdoc-md`/nightly were missing on a host where both were installed —
running the check instead immediately exposed a real bug in
`--regenerate-missing` that the false "tool missing" assumption had been
hiding. The general form: an untested code path plus an assumed-missing
prerequisite produces a confident, false statement about both. Check the
prerequisite, then run the path.

## No Python for documentation or accounting — build it into `kovan` (HARD RULE)

**Documentation generation and repository accounting are `kovan`'s job. Do not
write, restore, or reach for a Python script to do either. If `kovan` cannot do
it yet, extend `kovan`.**

This is settled direction, not a preference. Five Python scripts have been
retired under it — `historian.py`, `token_usage.py`, `gen_api_docs.py`,
`gen_aster_behaviour_registry.py` and `kloc_accounting.py` — replaced by
`kovan-cli historian` / `tokens` / `api-docs` and `kovan kloc`.
**`scripts/` now holds no tracked Python.**

**Why, concretely.** A script merely has to exist; an interpreter has to be
installed, on `PATH`, and not shadowed. On Windows `python3` routinely
resolves to a Microsoft Store alias stub that prints an advert and exits —
which silently turned the token-accounting git hooks into no-ops and let
commits ship with no `API-Usage` trailer at all. That is the failure mode this
rule exists to prevent: not an error, a **silent** no-op in the thing that
keeps the records honest.

**Scope.** Documentation generation, repository accounting, and the artifacts
either produces. It does **not** reach into `collaboration/` (gitignored
scratch owned by collaborators), `reference-data/` (vendored upstream trees),
or a third-party tool that happens to be written in Python.

**Seven first-party Python files remain and are NOT covered by this rule as
written:** `crates/outram-park-fork-coolprop/dev/*.py`. Six are **code
generation** (they read the gitignored upstream CoolProp JSON clone and emit
Rust), which is neither documentation nor accounting; `gen_latex_doc.py`
arguably is. Whether to bring them in is a maintainer decision that has not
been made. **Do not delete them under this rule without asking.** Everything
else matching `*.py` is vendored upstream source under `upstream_source/` or
gitignored `collaboration/` scratch — both explicitly out of scope.

**When porting, gate parity — do not waive it.** If the old output is not
committed anywhere, generate it with the Python *before* deleting the script,
commit it, then port and diff. `op-w44a.7` did this and it was a real check;
`op-yz7b` did not, and that gap is recorded as a known weakness.

**A Python script that is a published reproducibility artifact is a different
question — ask, do not delete.** Where a script exists so that a *journal
reader* can re-derive a table or figure, replacing it with a Rust binary
raises the reproduction bar from "run this script" to "build a 44-crate Rust
workspace", and may break a byte-identical copy held in a manuscript
repository. Raise it with the maintainer rather than applying this rule
mechanically. `kloc_accounting.py` was exactly that case: it was put to the
maintainer on 2026-08-14 with the consequence stated, and they chose the full
port.

> The retirement table, the full Windows-alias incident and the
> `kloc_accounting.py` decision:
> [`docs/claude-md-rationale/accounting-and-no-python.md`](../claude-md-rationale/accounting-and-no-python.md).
