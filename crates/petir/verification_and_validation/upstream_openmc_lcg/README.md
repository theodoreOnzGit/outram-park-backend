# V&V: `rng::lcg` against OpenMC `random_lcg.cpp` (gh:#1338, case 6)

> Research, education and V&V only (`RESPONSIBLE_USE.md`). AI draft, unreviewed.

## Methodology

- **Upstream:** OpenMC commit `8f79d9d0` (MIT), `src/random_lcg.cpp` and
  `include/openmc/random_lcg.h`, compiled **unmodified** with
  `g++ -O2 -std=c++17 -ffp-contract=off` (g++ 13.3.0). Only that one `.cpp`
  is built; OpenMC itself is not built. Its header needs only `<cstdint>`, so no
  stubs were required.
- **Driver:** `driver.cpp` prints the reference to `reference.txt`. `run.sh`
  checks the commit, builds and runs it.
- **Port:** `crates/petir/src/rng/lcg.rs` (`prn`, `future_seed`, `init_seed`),
  tested in `crates/petir/tests/lcg_vs_openmc_code_to_code.rs`.
- **Quantities:** the first 1000 `prn` draws (state and double bits) from seeds
  1, 42 and 2^63-1; `future_seed` for n in {0, 1, 2, 10, 1000, 2^20, 2^40, 2^62};
  `advance_prn_seed` on the same grid; `future_prn`; `init_seed` for masters
  1 and 42, ids {0, 1, 2, 3, 10, 1000, 65535} and offsets 0-3; the start state and
  an FNV-1a digest of 10^4 draws for particle ids 0-15; FNV-1a digests of 10^6
  sequential draws from each seed.
- **Pass criterion:** bit identity, no tolerance.

## Not ported

- `advance_prn_seed` and `future_prn` have **no PETIR function**. The Rust tests
  check the composition the C++ performs (`future_seed` into the state, and
  `future_seed` then `prn`). They are not PETIR APIs.
- `init_particle_seeds` (`random_lcg.cpp` lines 70-76) is **not ported** and is
  not compared. It uses a different formula from `init_seed`
  (`multiplier * (master + i) + increment` for `N_STREAMS` streams).
- `openmc_set_seed` / `openmc_set_stride` are replaced by explicit arguments,
  as recorded in `lcg.rs`. The driver calls `openmc_set_seed` to select the master.

## Prediction (written before the run)

Bit-identical in every compared quantity. Integer arithmetic throughout, and the
only floating-point step is one multiply by 2^-64, which is exact in both codes.

## Results (2026-11-10)

Toolchain: rustc 1.99.1, g++ 13.3.0 (-O2, -ffp-contract=off).

| quantity | values | bit-identical | worst difference |
|---|---|---|---|
| `prn` state and bits, 3 seeds x 1000 draws | 3000 | 3000 (100%) | 0 |
| `future_seed`, 8 strides x 3 seeds | 24 | 24 (100%) | 0 |
| `future_prn` composition | 24 | 24 (100%) | 0 |
| `advance_prn_seed` composition | 24 | 24 (100%) | 0 |
| `init_seed`, 2 masters x 7 ids x 4 offsets | 56 | 56 (100%) | 0 |
| particle streams, ids 0-15: start state and 10^4-draw digest | 32 | 32 (100%) | 0 |
| stride walk: one stream advanced by `DEFAULT_STRIDE` reproduces each start | 16 | 16 (100%) | 0 |
| sequential digests, 10^6 draws x 3 seeds (digest and end state) | 6 | 6 (100%) | 0 |

Test command and result:
`CARGO_BUILD_JOBS=1 CARGO_TARGET_DIR=/home/user/outram-park-backend/target cargo test --release -p petir --test lcg_vs_openmc_code_to_code`
gives 6 passed, 0 failed.

No difference was observed, so no cause needs explaining.
