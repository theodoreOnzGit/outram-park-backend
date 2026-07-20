# Upstream source

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** All code in this workspace is **unverified and untrusted** unless a specific verification & validation (V&V) case demonstrates otherwise. V&V cases are human-reviewed and are intended for journal / arXiv publication — that is the trust workflow. See the workspace `VERIFICATION_AND_VALIDATION.md` and `RESPONSIBLE_USE.md`. Not for nuclear facility operation, reactor control, safety-critical, or licensing decisions.


- **Project:** DWSIM
- **Repository:** <https://github.com/DanWBR/dwsim>
- **License:** GPL-3.0 (verify against the upstream `LICENSE` file when a
  clone is made — not yet independently confirmed against a local checkout
  by this crate)
- **Branch tracked:** `dwsim8` / `master` (confirm current default branch when cloning)
- **Commit at last sync:** not currently tracked — no persistent local clone (see note)
- **Clone command:** `git clone --depth 1 https://github.com/DanWBR/dwsim.git upstream_source/DWSIM`

## Provenance

`outram-park-fork-dwsim-libs` is a pure-Rust port of DWSIM's chemical-process modelling
kernels (thermal-hydraulics and thermodynamics). This crate is early-stage —
its own README/CLAUDE.md do not yet describe the ported scope in detail; see
beads (`bd show op-qo2`) for the current backlog status.

**Note:** no persistent local DWSIM clone is currently maintained. Clone it
with the command above when beginning active porting/verification work, and
update this file with the exact commit and confirmed license text at that
point.

## Licensing note

DWSIM is (per its public repository) GPL-3.0-licensed, matching this crate's
own license (the OUTRAM PARK workspace default) directly. This is an
**independent translation**, not a DWSIM project release, and is not
endorsed by or affiliated with DWSIM or its maintainers.

---

If a clone is added here, it is expected to be **gitignored** — never
committed, present for development only.
