# kopi-beans: `bn`'s help text is still branded as `bd` (`bn upgrade` says it upgrades `bd`)

- **Tool / version when observed:** kopi-beans 0.1.1 (from `cargo install --list`)
- **Platform:** Linux x86_64 (Arch)
- **Date observed:** 2026-08-07
- **Upstream:** [kopitiam#14](https://github.com/theodoreOnzGit/kopitiam/issues/14)

## What I ran

    $ bn upgrade --help
    $ bn prime
    $ bn ready

## What happened

`bn`'s own generated text referred to `bd` rather than `bn` even though only
`bn` was installed. `bn upgrade` described upgrading the `bd` binary — which
on a machine running both trackers would have overwritten the other tool.
`bn ready`'s footer and `bn prime`'s workflow text carried the same `bd`
branding.

## What I expected

A tool installed as `bn` to describe itself as `bn` throughout, and never to
act on a `bd` binary it does not own.

## Impact here

Confusing for agents following `CLAUDE.md` — the workspace had to carry an
explicit note that `bn`'s own output saying `bd` was a branding bug and not an
instruction to install beads-rs.

## Resolved

- **Fixed in:** kopi-beans 0.1.2
- **Verified:** 2026-08-07
- **Re-ran:** `bn upgrade --help`, `bn --help`, `bn prime`, `bn ready`
- **New output:**

      $ bn upgrade --help
      Report how to upgrade `bn`. kopi-beans publishes no prebuilt release
      binaries -- only the crates.io package -- so `bn` does not self-install;
      run `cargo install kopi-beans` instead.

      Self-upgrade was removed in kopitiam#14: the implementation inherited
      from beads-rs fetched beads-rs's own GitHub releases and installed them
      over a binary named `bd`, so on a machine running both trackers it would
      have overwritten the other tool.

  `bn prime` now reads "Track ALL work in beads … Run `bn prime` after context
  compaction" and every command it lists is `bn`. `bn ready`'s footer prints
  real counts (`90 blocked, 291 closed`). The only remaining `bd` mention in
  `bn --help` is a deliberate disambiguation: "It is a SEPARATE tool from
  beads-rs (`bd`) … `bn` never reads, writes, or upgrades the `bd` binary."

- **Not re-verified:** `bn setup claude`'s generated hook. It was not re-run,
  because running it would overwrite this repo's hand-maintained
  `.claude/settings.json`. That file's hook (`bn prime --mcp`) was confirmed
  working (exit 0) on 2026-08-07 and is maintained by hand per `CLAUDE.md`.

  **Addendum, 2026-08-12 (kopi-beans v0.1.3):** the overwrite concern above
  turned out not to apply to the `--project` form. `bn setup claude --project`
  was run and wrote `SessionStart` + `PreCompact` `bn prime` hooks into the
  **gitignored** `.claude/settings.local.json`, leaving the committed
  `.claude/settings.json` byte-identical (`git diff HEAD -- .claude/settings.json`
  empty) with its `Stop` hook intact. The generated hooks are `bn`-branded, so
  the branding fix holds there too. The **unflagged / global form was still not
  run** and remains unverified.

- **Note:** the upstream GitHub issue was still marked **OPEN** at the time of
  this verification. It needs the maintainer to close it:

      gh issue close 14 --repo theodoreOnzGit/kopitiam \
        --comment "Fixed in kopi-beans 0.1.2 — verified 2026-08-07; bn upgrade, bn prime and bn ready are all bn-branded."
