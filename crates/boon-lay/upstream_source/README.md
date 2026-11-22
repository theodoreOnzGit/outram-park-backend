# Upstream source

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** All code in this workspace is **unverified and untrusted** unless a specific verification & validation (V&V) case demonstrates otherwise. V&V cases are human-reviewed and are intended for journal / arXiv publication — that is the trust workflow. See the workspace `VERIFICATION_AND_VALIDATION.md` and `RESPONSIBLE_USE.md`. Not for nuclear facility operation, reactor control, safety-critical, or licensing decisions.


**The Lagrangian core is original work.** boon-lay's Lagrangian decay / TRISO
release simulator (single-particle Monte Carlo, CRP-6 release-fraction cases,
TRISO fuel-particle simulation) has no single upstream repository it forks or
translates — it implements original Lagrangian physics from published
methodology (e.g. IAEA CRP-6 benchmark cases).

**The Eulerian module *is* a fork.** `src/triso_atops_fork/` is a Rust fork of
an external upstream:

| Field | Value |
|---|---|
| Upstream | TRISO-ATOPS (INL) — https://github.com/IdahoLabResearch/TRISO-ATOPS |
| Commit | `de374c8` |
| License | MIT — © 2026 Battelle Energy Alliance, LLC (DOE contract DE-AC07-05ID14517) |
| Date accessed | 2026-07-15 |
| Clone | `TRISO-ATOPS/` (this directory) — reference-only, gitignored, **never compiled**, GUI intentionally not ported |

See `TRISO-ATOPS/PROVENANCE.md` here and `docs/triso-atops-fork.md` for the full
provenance and the Python→Rust module map. Attribution artifacts live at the
crate root: `LICENSE.triso-atops`, `NOTICE.triso-atops`, and per-file headers.

**Upstream has moved on (checked 2026-11-13, gh:#1426):** `origin/HEAD` is `0e87d36` (release 2.2.0), two releases
past `de374c8`, with two physics changes (`release_activity` normalisation, `R_B_fail` silver correction).
The fork stays pinned to `de374c8`; see
`verification_and_validation/triso_atops_upstream_drift/README.md` for the diff and its size.

If further reusable upstream reference implementations are identified later,
document them here with the same fields.
