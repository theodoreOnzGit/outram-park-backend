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

Files here are **waiting to be upstreamed**. They are not a private bug tracker
and they are not where OUTRAM PARK's own work is tracked — that stays in this
workspace's issue store. Anything filed here should be mentioned in the session
hand-off so it does not rot.

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
