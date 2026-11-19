# RustBCA differential: boon-lay's copy against upstream v3.0.0 (gh:#1426, epic #1336)

Research, education and V&V only. Not for facility operation, licensing or safety decisions
(`RESPONSIBLE_USE.md`).

| File | What |
|---|---|
| `PREDICTIONS.md` | written and committed before any run |
| `RESULTS.md` | what was measured, the per-operation attribution, predictions scored, deferrals |
| `shared.rs` | ONE harness, compiled against both codes: kernel, functions, seeded cascades |
| `driver/driver_main.rs`, `driver/driver_Cargo.toml` | the upstream side (`libRustBCA` by path; `--full` writes the 63 MB uncommitted full dump) |
| `driver/libm_probe_main.rs`, `driver/probe_Cargo.toml` | per-operation libm mismatch probe (path to `petir` is absolute in the manifest; edit it) |
| `rustbca_v3.0.0_reference.txt.gz` | upstream's output (15 665 lines, 0.5 MB): kernel, functions, per ion the incident ion, the particles that left, a trajectory digest with the first/last 5 entries, and a digest of every particle |
| `../../tests/rustbca_upstream_differential.rs` | replays `shared.rs` on `boon_lay::rustbca`, pins the counts and bounds |

## Reproduce the reference

```bash
git clone https://github.com/lcpp-org/RustBCA.git up && git -C up checkout v3.0.0   # 673bd3f2b907b14517a6b3cafa1c5372a5eb4ae2
mkdir -p drv/src && cp driver/driver_Cargo.toml drv/Cargo.toml          # [dependencies] libRustBCA = { package = "RustBCA", path = "../up" }
cp driver/driver_main.rs drv/src/main.rs && cp shared.rs drv/src/shared.rs
(cd drv && cargo build --release) && ./drv/target/release/rbca_driver | gzip -9 -n > rustbca_v3.0.0_reference.txt.gz
```

Build in a scratch directory outside the workspace, with its own target directory (the upstream tree is
never modified). `RBCA_DUMP_FULL=<path> cargo test --release -p boon-lay --test rustbca_upstream_differential`
writes the copy's full dump, in the format of `rbca_driver --full`, for line-by-line localisation.

## Provenance and licences

Upstream RustBCA: <https://github.com/lcpp-org/RustBCA>, GPL-3.0, tag v3.0.0, accessed 2026-11-13. The harness
inputs (Z, mass, cutoff and surface-binding energies, densities of W, Si, C) are fixed round numbers for a
code-against-code comparison, not recommended data. Nothing here is proprietary or restricted.

## Bookkeeping status

| Axis | State |
|---|---|
| Human code review | unchecked |
| V&V stage | unchecked |
