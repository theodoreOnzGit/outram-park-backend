# Upstream source

> ⚠️ **Unverified until validated.** All code in this workspace is unverified
> and untrusted unless a specific V&V case demonstrates otherwise. Not for
> nuclear facility operation, reactor control, safety-critical, or licensing
> decisions.

- **Project:** Moltres (UIUC ARFC)
- **Repository:** <https://github.com/arfc/moltres>
- **License:** LGPL-2.1 (**confirmed 2026-08-04** against the cloned `LICENSE` file)
- **Commit at last sync:** `3dd2ce7a60f59424d0d8f46748ba4e243d76b671` — shallow (`--depth 1`) clone taken 2026-08-04
  for the MSRE digital-twin port (epic `op-6w0`).
- **Clone command:** `git clone --depth 1 https://github.com/arfc/moltres.git upstream_source/moltres`

## Provenance

Pure-Rust port/translation for the OUTRAM PARK MSRE digital-twin effort. The
local clone is **gitignored** (dev-only, never committed); re-clone with the
command above if absent. This is an **independent translation**, not affiliated
with or endorsed by the upstream project.

## Licensing note

Upstream is **LGPL-2.1**, which is GPLv3-compatible; this crate is distributed as
GPL-3.0-only (the OUTRAM PARK workspace default). Ported files carry the
upstream provenance header block per the workspace provenance rule.
