# dover

**DOVER** — ***D**eck-based **O**pen-source **V**isualisation **E**ngine for
**R**eactors*.

> ⚠️ **Research, education and V&V only.** Not for nuclear facility operation,
> reactor control, licensing, safety-critical decisions or emergency response.
> See the workspace `RESPONSIBLE_USE.md`.

> ⚠️ **Unverified until validated.** All code in this workspace is
> **unverified and untrusted** unless a specific verification & validation
> (V&V) case demonstrates otherwise.

## Status: empty skeleton, scope to be decided

This crate was created on 2026-09-25 at the maintainer's direction as an
**empty skeleton**. The name points towards visualisation driven by input
decks, but **the scope is still to be decided**. Nothing is implemented: there
is no deck format, no visualisation engine, no physics, no public API and no
dependency. Do not describe this crate as providing anything until code has
actually been written here.

## Build and test

```bash
cargo build --release -p dover
cargo test  --release -p dover --lib --tests
```

The only test asserts that the crate builds and links.

## Bookkeeping status

> Maintainer sign-off tracker (see the workspace `CLAUDE.md` "Bookkeeping
> pass" command). A crate is **complete** only once the maintainer has
> personally signed off on BOTH axes below.

| Axis | Status |
|---|---|
| Verification & Validation (V&V) — human-reviewed | ❌ Not yet manually checked |
| Human / user interface — human-reviewed | ❌ Not yet manually checked |

**Status: INCOMPLETE** until both axes are manually checked and cleared by the maintainer.

## Licence

GPL-3.0-only, inherited from the workspace.
