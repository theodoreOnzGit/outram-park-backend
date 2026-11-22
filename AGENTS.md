# Agent Instructions

This project tracks work in **GitHub issues** on
`theodoreOnzGit/outram-park-backend`, using the `gh` CLI. The workspace rules
are in [`CLAUDE.md`](CLAUDE.md) and they bind every agent, not only Claude.
This file is a short pointer for agents that read `AGENTS.md` first.

## Quick Reference

```bash
gh issue list --state open                       # what is open
gh issue view <number>                           # details + comments
gh issue create --title "..." --body "..."       # file one
gh issue comment <number> --body "..."           # progress
```

## Rules

- Use `gh` issues for ALL task and roadmap tracking. Do not use TodoWrite,
  TaskCreate or markdown TODO lists.
- **Do not close issues on your own initiative.** Propose the closure with its
  evidence; the maintainer decides.
- **Labels:** `bug`, `enhancement`, `epic`, `P0`…`P3`. There is one epic per
  member crate. GitHub has no dependency graph, so state blocking relationships
  in the issue body in words ("blocked by #123").
- **After a plan is approved, convert it into issues before writing code**:
  one child issue per deliverable, under the relevant crate's epic.
- **`op-*` ids are historical** references into the retired issue tracker. Do not
  look them up in `gh` and do not mint new ones.
- **If `gh` is unavailable**, fall back to the harness's task tools and say so
  in the hand-off.
- The tracker holds *work to do*. Durable facts and preferences go in the
  per-project `memory/` files and `MEMORY.md`.
- **Lockstep crates.io releases: tests green before each publish** (maintainer,
  2026-10-21, gh:#971). When publishing to crates.io in lockstep, every
  crate's tests must be green before that crate is published. Low-level crates
  are tested and published first, concurrently with the tests of the
  higher-level crates still running, so the publish queue starts early and
  crates.io's rate limits are spread over the run instead of hitting at the
  end. Ideally a script drives this (`scripts/release-lockstep.sh`); the agent
  manages it.
- **No dependency cycles between workspace crates, ever: check before merging
  to `develop`** (maintainer, 2026-11-05). `cargo publish` resolves normal,
  dev and optional dependencies that carry a version against crates.io, so
  every internal edge of any kind must form a DAG. A cycle blocks the lockstep
  release, and it was found only at release time for v0.0.4.
  - **Check before every merge to `develop`:**
    `bash scripts/release-lockstep.sh --dry-run`. It reads `cargo metadata`
    and stops at "dry run: stopping before tests". If it says "dependency
    cycle", fix the cycle before merging, and delete the `release/` output it
    writes.
  - **The directions are fixed:**
    - `bugis` (GUI art and theme) is standalone: no workspace crate except
      `petir`.
    - `nee_soon` depends on `tampines`, never the reverse.
    - `outram-park-digital-twin-engine` depends on `dhoby-ghaut`, never the
      reverse.
  - **Fix a cycle by moving the code, not by hiding the edge.** An example or
    a test that needs a higher-level crate moves to that crate. For example, a
    web demo that uses the engine deploys from the engine, and widget logic
    that calls physics crates lives in the engine while its art stays in
    `bugis`.
- **HARD RULE: a merge to `develop` must compile natively AND for wasm**
  (maintainer, 2026-11-05). Before merging or pushing to `develop`, run both
  gates in release mode and merge only when both are clean:
  - **Native:** `cargo check --release --workspace --lib --tests`.
  - **Wasm:** `bash scripts/check-wasm.sh`, the `wasm32-unknown-unknown`
    gate over the in-scope crates.
  - **Pages demos (since 2026-11-06, gh:#1304):** `bash scripts/check-wasm-demos.sh`,
    a `wasm32-unknown-unknown` check of every Pages demo example, which
    `check-wasm.sh` (`--lib` only) cannot see. CI runs it as
    `.github/workflows/wasm-demos.yml` on push to `develop` and on pull requests.

  **Why:** develop 16240d670 compiled natively but broke the Pages site
  build. `kovan-common`'s founder ballots (`review/signing/founders.rs`,
  #1248) used `chrono`, which `Cargo.toml` listed only for native targets,
  so the wasm build stopped at "unresolved import `chrono`". A native check
  cannot see a target-gated dependency. Compiling is the bar here, not
  running: a wasm compile does not make a crate wasm-ready (root
  `CLAUDE.md`, Portability).
- **Kovan-reviewed functions are closed to modification (HARD RULE,**
  maintainer 2026-11-05): extend with new code instead; never edit a
  function carrying a kovan review stamp (body, signature, docs, refactor,
  fix) without explicit confirmation from a human maintainer, founder or
  reviewer for that change. Full rule and how to check: `CLAUDE.md`.
- **Avoid writing macros** (maintainer, 2026-11-05): no new `macro_rules!`
  or proc macros; write functions, generics and enums, so the code is
  readable in review and visible to kovan. Using std and dependency macros
  is fine. Full rule: `CLAUDE.md`, "Avoid writing macros".
- **Transcendental maths goes through PETIR, everywhere (HARD RULE,**
  maintainer 2026-11-05). In every crate's non-test code, call `exp`, `ln`,
  `log10`, `log2`, `powi`, `powf`, and transcendentals such as
  `sin`, `cos`, `tan`, `acos`, `atan2`, `sinh`, `tanh`, `hypot`, `cbrt`
  and the rest through `petir::mathf::RealMath` (`x.r_exp()`,
  `x.r_acos()`, `x.r_powi(n)`, ...), never the inherent `f64`/`f32`
  methods (`x.exp()`). The inherent methods take their last bit from the
  platform's C library (glibc, MSVC, Apple, wasm), so a run gives different
  bits on different machines; PETIR's are one implementation everywhere
  (gh:#985). A raw call compiles and runs silently, so the gate
  `crates/petir/tests/no_platform_libm.rs` (`cargo test --release -p petir
  --test no_platform_libm`) must stay green: run it after adding maths, and
  fix any line it lists in code you add. It went red twice on 2026-11-05
  (#1290: 41 calls, then 4 more from new cfMesh code). Full rationale:
  `crates/petir/AGENTS.md` and `crates/petir/src/mathf.rs`.
- **Never declare modules through a macro (HARD RULE,** maintainer
  2026-11-04): every `mod` is a plain line in source, never `mod $m;` in a
  `macro_rules!`. kovan reads `mod` lines without expanding macros, so a
  macro-declared module is missing from review. Full rule: `CLAUDE.md`.

## Crate-level interfaces may follow SOLID principles (maintainer, 2026-11-03)

**Maintainer direction:** "At the crate level I am ok with interfaces using
SOLID principles … the reason is so we don't introduce lifetimes
unnecessarily."

- **A crate may define a trait as its public interface.** Callers in other
  crates implement it, and the crate consumes it **generically** (`M: Trait` or
  `impl Trait`), never as `dyn Trait`. This covers dependency inversion (a
  library states what it needs, and a downstream physics crate supplies it),
  interface segregation (small traits, one job each), and open/closed (a new
  implementer needs no edit to the library). Example: `raffles::fidelity`'s
  `HighFidelityModel` and `ParametricModel` (gh:#1194).
- **Why:** a generic trait bound lets the library own its implementer by value
  or hold it in an `Arc`, so no lifetime parameter is needed. The alternatives
  this replaces each drag in `'a`: a struct holding `&'a dyn Trait`, a borrowed
  callback `&'a dyn Fn`, or a framework of borrowed references between
  objects.
- **What does not change:** the [Rust design rules](docs/claude-md/rust-design-rules.md)
  still bind. There are still no trait objects (`Box<dyn>`, `&dyn`,
  `Arc<dyn>`), no `Box<T>`, and no lifetime parameters. A closed set of
  variants that the crate itself provides (its models, schemes, routes) is
  still an enum. Traits are for the open seam *between* crates; enums are for
  dispatch *within* a crate.

- **Differential testing whenever there is more than one way to compute the same
  thing** (maintainer, 2026-11-04, gh:#1215). Some situations where it applies:
  - a port alongside its upstream;
  - two ports of the same upstream algorithm;
  - two strategies for the same job, e.g. cfMesh layer thinning vs snappy
    layer collapse;
  - a new solver path beside an old one.

  In each case, run every route on the **same inputs** and compare their
  outputs quantity by quantity. Then:
  1. **Write the predicted agreement before running.** Say which quantities
     should match exactly and which only within a stated tolerance, and why.
  2. **Explain every difference.** An unexplained difference is a finding to
     investigate, never noise to average away.
  3. **Test upstream first where it is runnable.** When upstream can be run
     (OpenFOAM, GeN-Foam, NJOY, cfMesh), the port is checked against it on
     shared cases. Bit-identity is the goal; below `1e-8` relative is
     acceptable only with an explained cause, per `CLAUDE.md`.
  4. **Keep the alternative routes callable** as named options, so the
     comparison can be re-run. Pin the agreement with a test.

  Record the methodology and the comparison table in the V&V record.

## Non-Interactive Shell Commands

**ALWAYS use non-interactive flags** with file operations to avoid hanging on confirmation prompts.

Shell commands like `cp`, `mv`, and `rm` may be aliased to include `-i` (interactive) mode on some systems, causing the agent to hang indefinitely waiting for y/n input.

**Use these forms instead:**
```bash
# Force overwrite without prompting
cp -f source dest           # NOT: cp source dest
mv -f source dest           # NOT: mv source dest
rm -f file                  # NOT: rm file

# For recursive operations
rm -rf directory            # NOT: rm -r directory
cp -rf source dest          # NOT: cp -r source dest
```

**Other commands that may prompt:**
- `scp` - use `-o BatchMode=yes` for non-interactive
- `ssh` - use `-o BatchMode=yes` to fail instead of prompting
- `apt-get` - use `-y` flag
- `brew` - use `HOMEBREW_NO_AUTO_UPDATE=1` env var

## GitHub rulesets: the "Bypassed rule violations" notice is expected

Checked against the repository's rulesets on 2026-10-16
(`gh api repos/theodoreOnzGit/outram-park-backend/rulesets/<id>`):

| ruleset | branch | rules | always-bypass actors |
|---|---|---|---|
| protection for develop | `develop` | no deletion, no force-push, pull request required | `theodoreOnzGit` (the maintainer), `snrthyr`, Repository admin role |
| Protection for main | `main` | no deletion, no force-push, no update, code quality | `theodoreOnzGit` |

- **Check which account is logged in before pushing to `develop`**
  (maintainer direction, 2026-10-22): run `gh api user --jq .login`. The
  founder, `theodoreOnzGit`, may bypass the pull-request rule and push to
  `develop` directly; so may the other always-bypass actors in the table.
  Any other account opens a pull request into `develop` instead.
- **A push to `develop` as `theodoreOnzGit` is allowed.** GitHub prints
  `Bypassed rule violations for refs/heads/develop … Changes must be made
  through a pull request` as a notice, not an error, and the push lands. Do not
  report it as a problem, and do not open a PR to work around it (maintainer
  direction, 2026-10-16).
- **GitHub will NOT stop a push to `main` from this account**, because the
  maintainer bypasses that ruleset too. The only guard is the rule in
  `CLAUDE.md`: never push to `main` unless the maintainer asks for it in so
  many words.
- If a push is ever *rejected* (not merely noticed), stop and report the exact
  error. Never force-push.

## Session Completion

This protocol applies when ending an implementation workflow. It is
subordinate to explicit user, repository, and orchestrator instructions.

1. **File issues for remaining work** with `gh issue create`.
2. **Run quality gates** (if code changed): tests, linters and builds, all in
   release mode (`--release`). Say which suite you ran.
3. **Update issue status**: comment progress on in-progress issues and propose
   closures rather than making them.
4. **Handle git conservatively**: report `git status` and the proposed
   commands, and wait for approval. Do not commit or push unless the user or
   the maintainer's configured stop hook asks for it, and never push to `main`
   without an explicit request.
5. **Hand off**: summarise changes, validation, issue status, and any blocked
   commit/push step with the exact command and error.
