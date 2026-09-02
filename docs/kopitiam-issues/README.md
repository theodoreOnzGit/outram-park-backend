# kopitiam / kopi-beans issue queue

Local fallback for upstream issues against the maintainer's first-party tools —
**`kopitiam`** (Semantic Runtime CLI) and **`kopi-beans`** (the `bn` work-item
tracker). Both live in one upstream repo:
<https://github.com/theodoreOnzGit/kopitiam>.

See the workspace `CLAUDE.md`, "Dogfood KOPITIAM and KOPI-BEANS", for the full
rule. In short:

1. **Prefer a real GitHub issue.** If `gh` is available and authenticated, file
   it directly:

   ```bash
   gh issue create --repo theodoreOnzGit/kopitiam \
     --title "kopitiam: <one-line summary>" \
     --body-file <your write-up>
   ```

2. **Only if `gh` is unavailable or unauthenticated**, write the issue here as
   one markdown file, named `<tool>-<short-kebab-slug>.md` — for example
   `kopitiam-check-has-no-release-flag.md` or
   `kopi-beans-bn-init-fails-on-termux.md`.

## This directory is a queue, not a tracker

Files at the **top level are the live queue** — outstanding, waiting to be
upstreamed. They are not a private bug tracker and they are not where OUTRAM
PARK's own work is tracked; that stays in this workspace's issue store. Anything
filed here should be mentioned in the session hand-off so it does not rot.

## `resolved/` — HARD RULE

When an issue is actually fixed upstream, **move its file into
`resolved/`**. Never delete it, and never leave a fixed issue at the top level.

**"Resolved" means verified, not announced.** Before moving a file:

1. Install the published version that claims the fix
   (`cargo install kopitiam` / `cargo install kopi-beans`).
2. **Re-run the exact reproduction recorded in the file.**
3. Confirm the behaviour actually changed.

Then append the closing evidence to the file as you move it:

```markdown
## Resolved

- **Fixed in:** kopitiam 0.2.7
- **Verified:** YYYY-MM-DD
- **Re-ran:** `<the exact command from the reproduction above>`
- **New output:** <what it printed now>
```

A file in `resolved/` **without** that evidence is not a resolution, it is a
claim. If the workspace `CLAUDE.md` carries a matching "known friction" note,
update or remove it in the same change so the docs never advertise friction
that no longer exists.

## Template

```markdown
# <tool>: <one-line summary>

- **Tool / version:** kopitiam 0.2.5 (from `cargo install --list`)
- **Platform:** Linux x86_64 / Termux aarch64 / Windows
- **Date observed:** YYYY-MM-DD

## What I ran

    $ <the exact command>

## What happened

<the actual output, pasted, not paraphrased>

## What I expected

<the expected behaviour, and why>

## Impact here

<what it blocked or forced a workaround for in this workspace>
```

## Honesty rules

These carry over from `CLAUDE.md` and are not negotiable:

- Report **what you actually ran and what it actually printed**. Do not
  paraphrase output from memory.
- Do not invent version numbers — read them from `cargo install --list`.
- Do not fabricate a reproduction you have not run.
- Filing is the end of your involvement. **Do not follow an issue up with a
  patch** — this workspace consumes released binaries only and never modifies
  either tool's source.

## Upstream state, re-checked 2026-08-13

All three files that were queued here have been filed upstream, and the four
previously-resolved issues have been closed:

| upstream | subject | state |
|---|---|---|
| #14 | `bd` branding in `bn`'s help / `bn upgrade` | closed (re-verified on 0.1.4) |
| #15 | stray unprefixed `tailnet_proxy` binary | closed (re-verified on 0.1.4) |
| #16 | `unsupported meta format_version 1` | closed |
| #19 | store ref could not be pushed to a non-local remote | closed 2026-08-13, re-verified on 0.1.4 |
| #23 | 0.1.4 daemon wedges after upgrade, needs `pkill` | **open** |
| #25 | `bn sync` hangs on a store with nothing pending | **open** |
| #26 | daemon holds ~37% of a CPU core while idle | **open** |
| #27 | daemon sync fails with `slotmap too small` | **open** |
| #28 | writes acknowledged, reported synced, then lost | **open** |
| #106 | kopitiam-pdf 0.3.2: `PdfReaderConfig.reflow` not wired to the `R` key | **open** (filed 2026-09-02, dogfooding kovan) |
| #107 | kopitiam-pdf 0.3.2: no host overlay / page-layout API on the embedded reader | **open** (filed 2026-09-02, dogfooding kovan) |

A caution recorded from closing #19: the first attempt to verify it used a
**stale remote-tracking ref** and concluded, wrongly, that the daemon had not
pushed. `git fetch --all` earlier in the session, then comparing against
`refs/remotes/origin/beads/store` hours later, is not a valid check — the
daemon pushes in between. Re-fetch that specific ref before judging, or just
try the manual push and read whether it says `Everything up-to-date`.
