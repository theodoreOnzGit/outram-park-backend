# Upstream source

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** All code in this workspace is **unverified and untrusted** unless a specific verification & validation (V&V) case demonstrates otherwise. V&V cases are human-reviewed and are intended for journal / arXiv publication — that is the trust workflow. See the workspace `VERIFICATION_AND_VALIDATION.md` and `RESPONSIBLE_USE.md`. Not for nuclear facility operation, reactor control, safety-critical, or licensing decisions.


- **Project:** CoolProp
- **Repository:** <https://github.com/CoolProp/CoolProp>
- **License:** MIT
- **Branch tracked:** `master`
- **Commit at last sync:** `0e67fe74b30a2fe9526af9bc64ea026a96f56ebf` (2026-07-05)
- **Clone command:** `git clone --depth 1 https://github.com/CoolProp/CoolProp.git upstream_source/CoolProp`

## Provenance

`outram-park-fork-coolprop` is a fork / pure-Rust translation of CoolProp — a
C++ thermophysical-property library covering ~120 pure/pseudo-pure fluids,
mixtures, incompressibles and humid air via Helmholtz-energy-explicit
equations of state. This crate ports CoolProp's fluid/incompressible/mixture
*data* (JSON coefficient tables under `upstream_source/CoolProp/dev/`) into
hardcoded Rust `const`s via the `dev/gen_*.py` / `dev/regen_*_all.py` codegen
scripts (see the crate README's "Regenerating … data" sections), and
re-implements CoolProp's evaluation algorithms (the Helmholtz-EOS term forms,
transport correlations, GERG-2008 mixture reducing/departure functions,
ASHRAE RP-1485 humid-air model) from its C++ source
(`upstream_source/CoolProp/src/`, `include/CoolProp/`) following the
workspace's enum-dispatch, no-runtime-JSON, no-trait-objects design rules.

The individual equations of state themselves are the work of their respective
original authors (e.g. IAPWS-95 for water: Wagner & Pruß, 2002; GERG-2008 for
natural-gas mixtures: Kunz & Wagner, 2012) as compiled and packaged by
CoolProp — CoolProp is the direct upstream, not the ultimate primary source
for any single equation of state.

## Licensing note

CoolProp is MIT-licensed. `outram-park-fork-coolprop` itself is GPL-3.0 (the
OUTRAM PARK workspace default) — MIT-to-GPL relicensing of a derivative work
is permitted under MIT's terms. See `NOTICE` (attribution) and
`TRADEMARKS.md` (non-endorsement / naming) in the crate root for the full
statement; this is an **independent fork**, not the CoolProp project, and not
endorsed by or affiliated with CoolProp or its authors.

---

The actual clone lives at `upstream_source/CoolProp/` and is **gitignored**
(see `.gitignore`) — never committed, present for development only. AI
agents doing porting/codegen/verification work against CoolProp should clone
it fresh if not already present:

```bash
git clone --depth 1 https://github.com/CoolProp/CoolProp.git upstream_source/CoolProp
```
