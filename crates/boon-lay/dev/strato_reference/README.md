# strato-reference: run upstream STRATO, write boon-lay's reference CSVs

Research, education and V&V only (`RESPONSIBLE_USE.md`). Nothing here is
validation against an experiment.

**Credit.** STRATO (Spherical TRISO Analysis and Transport Operations) is the
work of **Wee Zhern Ray (GitHub `weewzr`)**, <https://github.com/weewzr/STRATO>.
This harness runs it **unmodified**, at commit
`36f1a98848bec9d66fe515994e075cff09d6cefb` (2026-11-25), through its public
library API only.

## What it does

`src/main.rs` calls upstream's
`strato::analytical::sturm_liouville::ModalRelease` (Code C) and
`strato::numerical::finite_volume::{build_mesh, run}` (Code B), with
`strato::config::Config` built in code and checked with `Config::validate`,
and writes CSV files (header row, every float `{:.17e}`, which round-trips an
`f64` exactly) to the directory given on the command line. The committed
output is `crates/boon-lay/tests/data/strato_reference/`, with its
`PROVENANCE.md`. The cases and the gates that read these files are fixed in
`crates/boon-lay/verification_and_validation/strato_triso_diffusion/PLAN.md`.

| Files | Content |
|---|---|
| `sl_<id>_meta.csv` | `lambda_max`, `max_modes` (20000, upstream's `MAX_MODES`), `n_modes`, `lambda_covered`, `min_flux_ratio`, `accuracy_floor`, `min_valid_time`; A4 cases add `mean_exit_time_s` |
| `sl_<id>_modes.csv` | `n,eigenvalue_per_s,weight` |
| `sl_<id>_release.csv` | `t_s,release_fraction` from `ModalRelease::release_fraction` (a plain `f64` at every t; validity is `t >= min_valid_time`, in the meta file) |
| `fv_<case>_c<N>_dt<dt>.csv` | upstream FV at 2e3, 1e4, 5e4, 2e5 s: `release_fraction`, `inventory`, `cumulative_release`, `balance_residual`, `min_concentration` |
| `fv_timing.csv` | wall time of each FV run (`build_mesh` + `run`) |
| `oracle.csv` | upstream's Python-oracle value, F(2e5 s) = 0.4325556390930383, with its upstream file:line |

`lambda_max = 45 / t_min` as in upstream's `exact_release`, except the A7
cases, which set `lambda_max = m / 2e3` for m = 10, 20, 30, 45, 60.

## Reproduce

This package is **not** a member of the outer workspace (its `Cargo.toml`
has an empty `[workspace]` table). `Cargo.lock` is upstream's lock plus this
package, so the dependency versions are upstream's.

```bash
cd crates/boon-lay/dev/strato_reference
git clone https://github.com/weewzr/STRATO upstream/STRATO
git -C upstream/STRATO checkout 36f1a98848bec9d66fe515994e075cff09d6cefb
cargo run --release -- ../../tests/data/strato_reference
```

`upstream/` and `target/` are gitignored. The whole run takes well under a
second. Regenerating the committed data because a boon-lay test failed is
forbidden by the plan; regenerate only for a deliberate change of upstream
commit, and record it in `PROVENANCE.md`.

## Licence

STRATO is AGPL-3.0-only (since its commit `8c44fb7`, 2026-11-10; tags
`v1.0.0`/`v1.0.1` were MIT). This harness is AGPL-3.0-only, the same as
boon-lay.
