# kopi-beans: `bn` cannot read a beads-rs store — `unsupported meta format_version 1`

- **Tool / version when observed:** kopi-beans 0.1.1 (from `cargo install --list`)
- **Platform:** Linux x86_64 (Arch)
- **Date observed:** 2026-08-06 / 2026-08-07
- **Upstream:** [kopitiam#16](https://github.com/theodoreOnzGit/kopitiam/issues/16)

## What I ran

    $ bn status
    $ bn list
    $ bn init

## What happened

All three failed with:

    error: invalid field value: unsupported meta format_version 1

kopi-beans and beads-rs use the identical ref name (`refs/heads/beads/store`)
and file layout (`state.jsonl`, `deps.jsonl`, `tombstones.jsonl`,
`meta.json`), but kopi-beans 0.1.1 rejected the existing store's `meta.json`
outright rather than adopting or migrating it.

## What I expected

`bn` to either read the v1 store directly, or migrate it to its own format on
first use — rather than refusing every store-touching command.

## Impact here

This workspace had no working CLI issue tracker for the duration. The
`CLAUDE.md` "Issue tracking & roadmap" section had to carry a
harness-task-tools fallback, and the ~335 issues in the v1 store were
unreachable.

## Resolved

- **Fixed in:** kopi-beans 0.1.2
- **Verified:** 2026-08-07
- **Re-ran:** `bn --version` then `bn status`
- **New output:**

      $ bn --version
      bn 0.1.2

      $ bn status

      Issue Database Status
      =====================

      Summary:
        Total Issues:      626
        Open:              289
        In Progress:       46
        Blocked:           90
        Closed:            291
        Ready to Work:     245
        Deleted:           6 (tombstones)
        Epics Ready to Close: 2

  The store was migrated in place to `format_version 2`;
  `refs/heads/beads/store` is now `9d791891`. A pre-migration snapshot of the
  v1 store is preserved at `refs/beads/premigration-v1-20260807`
  (`4e3f3518`) and must not be deleted.

- **Note:** the upstream GitHub issue was still marked **OPEN** at the time of
  this verification. It needs the maintainer to close it:

      gh issue close 16 --repo theodoreOnzGit/kopitiam \
        --comment "Fixed in kopi-beans 0.1.2 — verified 2026-08-07, store migrated to format_version 2, bn status reads 626 issues."
