# EGSnrc alias tables: PETIR port vs upstream, compiled and run

> Research, education and V&V only. Not for operational, licensing or
> safety-critical use (see `RESPONSIBLE_USE.md`). AI-assisted work, not yet
> human-reviewed.

**Issue:** gh:#1289. **Date:** 2026-11-05. **Port:**
`crates/petir/src/rng/alias_table.rs`. **Pinned by:**
`crates/petir/tests/alias_table_vs_egsnrc.rs`.

## What is compared

EGSnrc's `HEN_HOUSE/egs++/egs_alias_table.cpp` (NRC, AGPL-3.0-or-later),
commit `bd49a47aae3938f49f4100b8e018091861408b19` (2026-09-28), compiled
unmodified, against PETIR's port:

- `EGS_AliasTable`: construction (`make()`) for type 0 (discrete), 1
  (histogram), 2 (linear interpolation within a bin) and 3 (same as 2), the
  function-driven `initialize(xmin, xmax, accu, nmax, func, data)`, and
  sampling with `sample()` and `sampleBin()`;
- `EGS_SimpleAliasTable`: construction and `sample()`.

## Methodology

1. **Upstream built from source.** `harness.cpp` links upstream's
   `egs_alias_table.cpp` from the gitignored `vendor/egsnrc` clone.
   `stub/egs_config1.h` stands in for EGSnrc's configure-generated header
   (`EGS_Float = double`). The harness defines the three message hooks
   (`egsWarning` counts warnings) and `EGS_RandomGenerator`'s two
   out-of-line members (as `egs_rndm.cpp:49-81`). Build:
   `g++ -O2 -ffp-contract=off -std=c++17` (`build_and_run.sh OUTDIR`; set
   `EGSNRC=` when `vendor/` is not at the repository root, e.g. in a
   worktree).
2. **Same random numbers on both sides.** A fixed list of 250 000 uniforms
   `k * 2^-53`: five edge values first (`0`, `1 - 2^-53`, `1/2`, `2^-53`,
   `1 - 2^-52`), then `splitmix64(seed 1289) >> 11`. Upstream gets them from
   a replay `EGS_RandomGenerator` whose `fillArray()` copies from the list.
   The Rust side gets them from a test-only replay generator implementing
   PETIR's `Rng` trait, with `next_u64 = k << 11`, so `random::<f64>()` is
   exactly `k * 2^-53`.
3. **The same inputs.** Every input is built by the same recipe on both
   sides from `+ - * /` and exact powers of two, so the input bits agree
   (the full-dump comparison checks `xi` too).
4. **Cases** (23): discrete, histogram and linear tables, each uniform,
   steep (`f_i = 2^-i`), with zeros, a single bin, and 10^4 bins
   (`f = u^3`, `u` uniform); a type-3 table; `func_lorentz`
   (`1/(1+x^2)` on `[0, 4]`, `accu = 1e-3`, met: return 0, 64 points);
   `func_nmax` (`x^3 + 0.01` on `[0, 2]`, `accu = 1e-8`, `nmax = 50`: return
   1, 65 points); simple tables uniform, steep, with zeros, single, 10^4 bins.
5. **What is compared, bit for bit, as hex:** every table entry (`n`, `np`,
   `average`, `xi`, `fi`, `wi`, `bin`), 10^5 `sample()` values and 10^5
   `sampleBin()` indices per alias table (stream restarted for each), and
   10^5 `sample()` indices per simple table.
6. **Committed pin:** `reference_summary.txt` (the harness's own output):
   FNV-1a digests of every table and every sample sequence, the first 16
   samples, and the full `wi`/`bin` of every table with at most 40 bins.
   `digests_match_upstream` checks all of it on every `cargo test`. The full
   dumps (69 MB) are not committed;
   `EGS_ALIAS_DUMP_DIR=OUTDIR cargo test --release -p petir --test alias_table_vs_egsnrc -- --nocapture`
   compares them entry by entry and prints the table below.

## Prediction (written before running)

Every case bit-identical: tables, samples and bins. Both sides do the same
binary64 operations in the same order (the port keeps upstream's grouping,
e.g. `fcum*(...)/(3*(...))`, and its first-high/first-low pairing order);
the only non-basic operation is `sqrt`, correctly rounded on both sides; the
uniforms are exact; `(int)` and `as usize` truncate alike for non-negative
values; `-ffp-contract=off` on the C++ side and Rust never contracts, so no
FMA can enter. Risk named in advance: `linear_zeros` has a bin with `f = 0`
at both edges, so upstream's first moment is `0*(..)/0 = NaN`; the default
x86-64 NaN `0xfff8000000000000` was expected on both sides.

## Results (2026-11-05, x86-64, g++ / Rust 1.99.1, `--release`)

**Prediction confirmed: bit-identical everywhere.** Worst difference 0 in
every row; the `linear_zeros` NaN average has the same bits on both sides.

| case | table entries compared | bit-identical | samples + bins compared | bit-identical | worst difference | cause |
|---|---|---|---|---|---|---|
| discrete_uniform | 41 | 41 | 200 000 | 200 000 | 0 | none |
| discrete_steep | 161 | 161 | 200 000 | 200 000 | 0 | none |
| discrete_zeros | 49 | 49 | 200 000 | 200 000 | 0 | none |
| discrete_single | 5 | 5 | 200 000 | 200 000 | 0 | none |
| discrete_many | 40 001 | 40 001 | 200 000 | 200 000 | 0 | none |
| histogram_uniform | 42 | 42 | 200 000 | 200 000 | 0 | none |
| histogram_steep | 130 | 130 | 200 000 | 200 000 | 0 | none |
| histogram_zeros | 50 | 50 | 200 000 | 200 000 | 0 | none |
| histogram_single | 6 | 6 | 200 000 | 200 000 | 0 | none |
| histogram_many | 40 002 | 40 002 | 200 000 | 200 000 | 0 | none |
| linear_uniform | 43 | 43 | 200 000 | 200 000 | 0 | none |
| linear_steep | 131 | 131 | 200 000 | 200 000 | 0 | none |
| linear_zeros | 51 | 51 | 200 000 | 200 000 | 0 (NaN average, same bits) | none |
| linear_single | 7 | 7 | 200 000 | 200 000 | 0 | none |
| linear_many | 40 003 | 40 003 | 200 000 | 200 000 | 0 | none |
| linear_type3 | 131 | 131 | 200 000 | 200 000 | 0 | none |
| func_lorentz | 255 | 255 | 200 000 | 200 000 | 0 | none |
| func_nmax | 259 | 259 | 200 000 | 200 000 | 0 | none |
| simple_uniform | 20 | 20 | 100 000 | 100 000 | 0 | none |
| simple_steep | 80 | 80 | 100 000 | 100 000 | 0 | none |
| simple_zeros | 24 | 24 | 100 000 | 100 000 | 0 | none |
| simple_single | 2 | 2 | 100 000 | 100 000 | 0 | none |
| simple_many | 20 000 | 20 000 | 100 000 | 100 000 | 0 | none |

Table entries counted: `average` + `xi` + `fi` + `wi` + `bin` (alias
tables), `wi` + `bins` (simple tables). The `initialize()` return code
(0 / 1) and upstream's warning count (0 in every case) also agree.

### Statistical check: chi-squared, `alpha = 0.001` (fixed in advance)

The same 10^5 draws per case, binned and compared with the INPUT
distribution's exact cell probabilities: one cell per bin for `sampleBin()`
and for simple tables; four equal sub-intervals per bin for histogram and
linear `sample()` values (linear: trapezoid integral of the linear density
over each sub-interval). Cells are pooled in order until each expects at
least 5. `p` is `1 - P(dof/2, chi2/2)` through PETIR's `gamma_inc_p` (P), or
the Wilson-Hilferty approximation (WH) where that port refuses the branch
(`dof >= 20`). A function-driven table is judged against its own built
linear table, which is what it samples. **All 36 tests pass.** The replay
stream is fixed, so the result is deterministic; with 36 tests at 0.001, a
correct sampler would fail one by chance with probability about 3.5 %.

| case | sampler | cells | dof (pooled) | chi2 | p | zero-prob hits (explained) |
|---|---|---|---|---|---|---|
| discrete_uniform | sampleBin | 10 | 9 | 11.5 | 0.240 P | 0 (0) |
| discrete_steep | sampleBin | 40 | 14 | 10.2 | 0.744 P | 0 (0) |
| discrete_zeros | sampleBin | 12 | 4 | 4.7 | 0.318 P | 1 (1) |
| discrete_single | sampleBin | 1 | 0 | 0.0 | 1.000 | 0 (0) |
| discrete_many | sampleBin | 10000 | 5423 | 5338.5 | 0.791 WH | 0 (0) |
| histogram_uniform | sampleBin | 10 | 9 | 11.5 | 0.240 P | 0 (0) |
| histogram_uniform | sample | 40 | 39 | 44.2 | 0.261 WH | 0 (0) |
| histogram_steep | sampleBin | 32 | 17 | 21.7 | 0.198 P | 0 (0) |
| histogram_steep | sample | 128 | 64 | 70.7 | 0.264 WH | 0 (0) |
| histogram_zeros | sampleBin | 12 | 4 | 4.7 | 0.318 P | 1 (1) |
| histogram_zeros | sample | 48 | 19 | 14.6 | 0.745 P | 1 (1) |
| histogram_single | sampleBin | 1 | 0 | 0.0 | 1.000 | 0 (0) |
| histogram_single | sample | 4 | 3 | 0.3 | 0.964 P | 0 (0) |
| histogram_many | sampleBin | 10000 | 5369 | 5272.0 | 0.825 WH | 0 (0) |
| histogram_many | sample | 40000 | 13740 | 13655.4 | 0.694 WH | 0 (0) |
| linear_uniform | sampleBin | 10 | 9 | 11.5 | 0.240 P | 0 (0) |
| linear_uniform | sample | 40 | 39 | 44.2 | 0.261 WH | 0 (0) |
| linear_steep | sampleBin | 32 | 17 | 21.7 | 0.198 P | 0 (0) |
| linear_steep | sample | 128 | 64 | 64.6 | 0.456 WH | 0 (0) |
| linear_zeros | sampleBin | 12 | 9 | 6.5 | 0.694 P | 0 (0) |
| linear_zeros | sample | 48 | 39 | 36.9 | 0.566 WH | 0 (0) |
| linear_single | sampleBin | 1 | 0 | 0.0 | 1.000 | 0 (0) |
| linear_single | sample | 4 | 3 | 0.1 | 0.994 P | 0 (0) |
| linear_many | sampleBin | 10000 | 7081 | 7063.8 | 0.555 WH | 0 (0) |
| linear_many | sample | 40000 | 14679 | 14686.0 | 0.482 WH | 0 (0) |
| linear_type3 | sampleBin | 32 | 17 | 21.7 | 0.198 P | 0 (0) |
| linear_type3 | sample | 128 | 64 | 64.6 | 0.456 WH | 0 (0) |
| func_lorentz | sampleBin | 63 | 62 | 56.7 | 0.668 WH | 0 (0) |
| func_lorentz | sample | 252 | 251 | 263.4 | 0.282 WH | 0 (0) |
| func_nmax | sampleBin | 64 | 63 | 59.0 | 0.618 WH | 0 (0) |
| func_nmax | sample | 256 | 236 | 236.6 | 0.477 WH | 0 (0) |
| simple_uniform | sample | 10 | 9 | 14.0 | 0.121 P | 0 (0) |
| simple_steep | sample | 40 | 14 | 9.8 | 0.773 P | 0 (0) |
| simple_zeros | sample | 12 | 4 | 1.1 | 0.890 P | 0 (0) |
| simple_single | sample | 1 | 0 | 0.0 | 1.000 | 0 (0) |
| simple_many | sample | 10000 | 5420 | 5326.8 | 0.814 WH | 0 (0) |

## Findings about upstream (kept by the port, not fixed)

1. **A zero-probability bin can be returned.** `sample()`/`sampleBin()` keep
   bin `j` unless `aj > wi[j]`; when `r1 * np` is exactly the integer `j`,
   `aj = 0`, and a bin with `wi[j] = 0` is kept. The stream's leading
   uniform `0` does this once in `discrete_zeros` and `histogram_zeros`
   (bin 0 has `f = 0`): the "zero-prob hits" column, each explained by the
   test's own count of exact-integer draws. It needs `r1 * np` to be an
   exact integer, so its rate is at most about `np * 2^-53` per draw for
   53-bit uniforms. `EGS_SimpleAliasTable` compares `u2 < wi`, strictly, and
   does not have it.
2. **`getAverage()` is NaN** for a linear table with a bin zero at both
   edges (`0 * (...) / 0` in that bin's first moment), although sampling is
   correct. Seen in `linear_zeros`.
3. **Type 3 is type 2.** `make()` and `sample()` branch only on 0 and 1,
   and `linear_type3` is bit-identical to `linear_steep` in every sample.

## Deviations of the port (none moves a bit on these cases)

Listed in the module docs of `alias_table.rs`: `Vec` and `Clone` for
`allocate`/`copy`; errors (`Domain`, `Invalid`, `MaxIterations`) where
upstream would read out of bounds or call `egsFatal`; warnings as
`aliasing_incomplete()` flags; the bin index clamped to `np - 1` (unreachable
from `random::<f64>()`); type 3 mapped to type 2.

## Not covered

- Only x86-64 was run. `sqrt` and the basic operations are correctly
  rounded everywhere, so other targets should agree, but that is not
  measured; a NaN's sign bit (finding 2) can differ on targets with a
  different default NaN.
- The uniforms are all of the form `k * 2^-53`. EGSnrc's own RANMAR and
  RANLUX streams are being ported separately (gh:#1289); feeding them
  through PETIR's `Rng` reproduces EGSnrc only if their `next_u64` returns
  `k << 11` for the uniform `k * 2^-53`.
- No comparison against a published benchmark: this is code-to-code
  verification, not validation.

## Files

| file | what |
|---|---|
| `harness.cpp` | the C++ harness (includes upstream's header; links its `.cpp`) |
| `stub/egs_config1.h` | stand-in for the configure-generated header |
| `build_and_run.sh` | build and run; writes `summary.txt` and the per-case dumps |
| `reference_summary.txt` | the harness's `summary.txt`, committed; the test's reference |
