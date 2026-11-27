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

What a reader can study and challenge here today is narrow: one copied code,
RustBCA, the binary-collision Monte Carlo that follows an ion and every recoil
it knocks on. A student can read its scattering integrals (Gauss-Mehler,
Gauss-Legendre, MAGIC) and its electronic-stopping branches beside upstream's
Rust, with every local change listed, and check them against a record that
shows the copy reproducing upstream bit for bit once the `libm` calls are
swapped for the platform's. The crate's own damage physics (dpa, gas
production, property changes) is not written yet.

## Knowledge in this crate

See the workspace [vision](../../VISION.md).

| Knowledge artifact | In this crate |
|---|---|
| Scientific literature | RustBCA, `lcpp-org/RustBCA` commit `a356280d` (v3.0.0 plus 17 commits), GPL v3, by Jon Drobny and Davide Curreli; paper: Drobny and Curreli, *JOSS* 6(64), 3298 (2021), doi:10.21105/joss.03298. Upstream comments cite Biersack and Haggmark (1980), Mendenhall and Weller (1991, 2005) and Ziegler, Biersack and Ziegler (2008). Attribution and every local change: `src/rustbca/mod.rs`, `src/rustbca/README.md`. Nothing in `crates/kovan-literature/CATALOGUE.md` names this crate. |
| Mathematical equations | In code, as upstream wrote them: the binary-collision scattering integral by Gauss-Mehler and Gauss-Legendre quadrature and the MAGIC approximation, Lindhard screening length and reduced energy, Oen-Robinson local electronic loss (`src/rustbca/bca.rs`, `src/rustbca/interactions.rs`). No separate derivation document. |
| Source code | `src/rustbca/` (`bca.rs`, `interactions.rs`, `material.rs`, `particle.rs`, `physics.rs`, `geometry.rs`, `sphere.rs`, `input.rs`, `output.rs`, `simple.rs`), copied from upstream and made `no_std` + `alloc`. |
| Interactive simulations | None yet. One headless CLI example: `examples/rustbca_cascades.rs`. |
| Visualisations | `verification_and_validation/rustbca_cascades/triso_stack_mesh1d.svg`, the TRISO layer stack as RustBCA's `Mesh1D` sees it. |
| Verification and validation evidence | `verification_and_validation/rustbca_differential/` (predictions, then results, 2026-11-13, gh:#1426), pinned by `tests/rustbca_upstream_differential.rs`: with all transcendentals routed through glibc the copy is bit-identical to upstream v3.0.0 on 24 624 / 24 624 kernel values, 2 116 / 2 116 function values and 39 848 / 39 848 cascade values; as shipped (the `libm` crate) the differences are libm-level, worst relative 3.06e-11 (kernel) and 1.08e-11 (cascades). `verification_and_validation/rustbca_cascades/` (2026-10-16): a cost probe in an HTR-10 TRISO particle; the light fission fragment's range (10.7 µm) missed its 4–9 µm prediction, and no range has been compared with measured data. |
| Human reviews and corrections | 0 functions carry a kovan human-review stamp. No entry in `docs/human-corrections-to-ai-work.md` names this crate. Struck-through correction: the upstream commit, once recorded as "v3.0.0, `a356280d`", corrected 2026-11-13 to v3.0.0 plus 17 commits (`src/rustbca/mod.rs`). |

**Status in one sentence:** the RustBCA copy is verified code to code against
upstream; nothing in it is validated against measurement, and dpa, gas
production and property changes do not exist.

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
