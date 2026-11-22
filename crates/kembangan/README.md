# KEMBANGAN

**K**inetics, **E**lasticity and **M**aterials **B**ehaviour for **A**ging in
**N**uclear Environments through **G**enerative **A**I and **N**umerical
Modelling (backronym set by the maintainer, 2026-11-22;
`docs/ecosystem-naming.md`).

> ⚠️ **Research, education and V&V only.** Not for nuclear facility operation,
> reactor control, licensing, safety-critical decisions or emergency response.
> See the workspace `RESPONSIBLE_USE.md`. Damage and lifetime estimates from
> this crate are not an authority for any licensing or safety decision.

**Scope.** Materials damage under irradiation: what particles do to a
material, and how its properties age as a result. The one question it answers
is **"How much has this material been damaged, and how has it changed?"**

## Status

Stated plainly: **the only code is the RustBCA copy, moved here from
`boon-lay` on 2026-11-22 (gh:#1699).** dpa, gas production and property
changes are not implemented. AI draft, untrusted until human review.

| Part | State |
|---|---|
| `rustbca`: a copy of RustBCA (`lcpp-org/RustBCA` `a356280d`, GPL-3.0, Jon Drobny and Davide Curreli), the binary-collision Monte Carlo for ions and the recoils they knock on | moved from `boon-lay`, unchanged; `boon_lay::rustbca` re-exports it. Differentially tested against upstream (`verification_and_validation/rustbca_differential/`); cascade cost probe in `verification_and_validation/rustbca_cascades/`. Attribution and every local change: `src/rustbca/mod.rs`, `src/rustbca/README.md` |
| dpa from a neutron flux (`outram-mc-libs`) and NJOY HEATR damage energy (`njoy-outram-park-fork`) | **not implemented** (planned, gh:#1699) |
| Gas production (He, H) | **not implemented** |
| Property changes (graphite dimensional change, embrittlement, irradiation creep, swelling) | **not implemented**; reuse `outram-park-fork-offbeat`'s material models where they exist |

## Dependency direction

KEMBANGAN **serves** `boon-lay` and other crates, which depend on it.
`punggol`, `outram-mc-libs`, `njoy-outram-park-fork` and `petir` **serve**
KEMBANGAN. None of those may ever depend on it (maintainer, 2026-11-22).

## `no_std` + `alloc`

Without the default `std` feature the crate is `no_std` + `alloc`, checked on
a bare-metal target by `scripts/check-nostd.sh`
(`cargo check --release -p kembangan --lib --no-default-features --target x86_64-unknown-none`).
RustBCA's TOML/file-output/batch-driver layer is behind the `rustbca-std`
feature, off by default, as it was in `boon-lay`.

## Running

```bash
cargo test --release -p kembangan --lib --tests
cargo run  --release -p kembangan --example rustbca_cascades
```

## Bookkeeping status

> Maintainer sign-off tracker (see the workspace `CLAUDE.md` "Bookkeeping
> pass" command). A crate is **complete** only once the maintainer has
> personally signed off on BOTH axes below.

| Axis | Status |
|---|---|
| Verification & Validation (V&V) — human-reviewed | ❌ Not yet manually checked |
| Human / user interface — human-reviewed | ❌ Not yet manually checked |

**Status: INCOMPLETE** until both axes are manually checked and cleared by the maintainer.

## License

**AGPL-3.0-only**, like the whole workspace. Part of the
[OUTRAM PARK](../../README.md) workspace. Provenance: `src/rustbca/` is copied
from RustBCA, whose upstream licence is the GNU GPL v3; its attribution
headers and upstream `LICENSE` are kept, and `LICENSE.GPL-3.0` ships beside
`LICENSE` for them. See `NOTICE`.
