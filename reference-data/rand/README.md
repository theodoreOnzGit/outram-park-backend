# `rand` 0.10.3 / `rand_distr` 0.6.0 reference streams

Code-to-code reference data for PETIR's `rng` port of the `rand` pieces that
`boon-lay`'s RustBCA copy used (gh:#995), produced by **compiling and running
the upstream crates**, not by transcription. Same discipline as
`reference-data/roots/` and `reference-data/pyclaw/`: the driver is committed
beside its output so anyone can reproduce it.
`crates/petir/tests/rng_vs_rand.rs` compares PETIR against it, every value
and every digest exactly.

## Upstream

| | |
|---|---|
| crates | `rand` 0.10.3 (`rand::rngs::ChaCha8Rng` is a re-export of `chacha20` 0.10.2's), `rand_core` 0.10.1, `rand_distr` 0.6.0 |
| authors | The Rand Project Developers; The RustCrypto Project Developers (`chacha20`) |
| licence | MIT OR Apache-2.0 |
| source | the crates.io registry copies pinned by the workspace `Cargo.lock` on 2026-10-20 |
| features | as boon-lay declared them: `rand` with `chacha`, `alloc` (no default features); `rand_distr` with `alloc` (no default features) |

## Regenerating

`rand_reference_driver.rs` is a standalone `main.rs`. It is **not** a member
of this workspace and must not become one: PETIR ports these crates, it does
not depend on them.

```bash
mkdir -p /tmp/randref/src && cd /tmp/randref
cat > Cargo.toml <<'TOML'
[package]
name = "randref"
version = "0.1.0"
edition = "2024"
[dependencies]
rand = { version = "=0.10.3", default-features = false, features = ["chacha", "alloc"] }
rand_distr = { version = "=0.6.0", default-features = false, features = ["alloc"] }
chacha20 = "=0.10.2"
rand_core = "=0.10.1"
[features]
# Off for the committed file. On, rand_distr's exp/ln go to the platform libm
# (see "The libm question" below).
std_math = ["rand_distr/std_math"]
TOML
cp <this directory>/rand_reference_driver.rs src/main.rs
cargo run --release > rand-0.10.3-reference.txt
```

Generated 2026-10-20 on x86_64 Linux (Arch, glibc), Rust 1.99.1, release. On
x86_64 `chacha20` picks its AVX2 backend at run time; PETIR ports the portable
backend, so agreement here is also agreement between the two backends.

## Format

`#` lines are comments. Each case starts `case <seed> <stream>` (hex). Then,
one record per line:

| record | meaning |
|---|---|
| `u32 <n> <v>...` | `n` `next_u32` from `seed_from_u64(seed)` + `set_stream(stream)` |
| `u32+u64 <n> <v>...` | one `next_u32`, then `n - 1` `next_u64` (crosses and straddles the 64-word refill) |
| `bytes <n> <v>...` | the bytes of `fill_bytes` calls of length 1, 3, 5, 7, 32, 13, 64, 250, then one `next_u32` |
| `from_rng <n> <v>...` | 40 `next_u64` of `ChaCha8Rng::from_rng(&mut parent)`, then the parent's next `next_u64` |
| `f64 <n> <v>...` | `random::<f64>()` |
| `uniform <lo> <hi> <n> <v>...` | `Uniform::new(lo, hi).unwrap().sample(..)` |
| `normal <mean> <sd> <n> <v>...` | `Normal::new(mean, sd).unwrap().sample(..)` |
| `mixed <n> <v>...` | the cycle u32, u64, f64, `Uniform(-2.5, 7.25)`, `Normal(0, 1)` |
| `digest <what> <n> <fnv> [tail]` | FNV-1a 64 over the bits of `n` draws; `normal` also counts draws with `|z| > 3.654` |

Every value is a 16-digit hex bit pattern (an `f64` as `to_bits`).

## The libm question

`rand_distr` reaches `exp` and `ln` (the ziggurat's wedge test and its tail)
through `num_traits::Float`. With its own features that is the `libm` crate;
if anything else in a build turns on `num-traits/std`, it becomes the
platform libm. In boon-lay's build something did. PETIR uses `libm`
everywhere. Regenerating with `--features std_math` gives the platform-libm
streams; the result of that comparison is recorded in
`crates/petir/src/rng/mod.rs`.
