# Upstream source

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** All code in this workspace is **unverified and untrusted** unless a specific verification & validation (V&V) case demonstrates otherwise. V&V cases are human-reviewed and are intended for journal / arXiv publication — that is the trust workflow. See the workspace `VERIFICATION_AND_VALIDATION.md` and `RESPONSIBLE_USE.md`. Not for nuclear facility operation, reactor control, safety-critical, or licensing decisions.


- **Project:** rust-steam
- **Repository:** <https://github.com/marciorvneto/rusteam>
- **License:** MIT
- **Branch tracked:** `main` (when cloned — see below)
- **Commit at last sync:** not currently tracked — no persistent local clone (see note)
- **Clone command:** `git clone --depth 1 https://github.com/marciorvneto/rusteam.git upstream_source/rusteam`

## Provenance

TAMPINES Steam Tables **relies heavily upon** rust-steam for its initial
IAPWS-IF97 implementation (per the crate README's own history) but is not a
1:1 translation the way `outram-park-fork-coolprop` is of CoolProp — rust-steam
was incomplete and lacked dimensioned units, so TAMPINES extends it with `uom`
throughout, adds verification against the International Steam Tables
(Kretzschmar & Wagner, 2019), and adds substantial original scope (steam-turbine
equations, choked-flow HEM solvers, the OpenFOAM finite-volume algorithms).

Significant portions of code were copied from the rust-steam package early
in the crate's history — the rust-steam license is reproduced in this
crate's README (`# Rust-steam license:` section) per MIT's attribution
requirement.

**Note:** no persistent local rust-steam clone is currently maintained (the
relevant code was incorporated directly rather than via an ongoing
codegen-from-clone pipeline). Clone it with the command above if doing
comparative verification work against the original.

## Licensing note

rust-steam is MIT-licensed; TAMPINES Steam Tables itself is GPL-3.0 (the
OUTRAM PARK workspace default) — MIT-to-GPL relicensing of a derivative work
is permitted under MIT's terms, and the MIT license text is preserved in the
crate README per its attribution requirement.

---

If a clone is added here, it is expected to be **gitignored** — never
committed, present for development only.
