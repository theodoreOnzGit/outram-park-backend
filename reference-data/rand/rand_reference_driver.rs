// Reference driver: draws random streams from the Rand project's own crates
// (`rand` 0.10.3, whose `rngs::ChaCha8Rng` is `chacha20` 0.10.2's, and
// `rand_distr` 0.6.0), for PETIR's `rng` code-to-code comparison
// (crates/petir/tests/rng_vs_rand.rs, gh:#995).
//
// Build and run: see README.md beside this file. It is a standalone main.rs,
// NOT a workspace member.
//
// Every operation is spelled the way boon-lay's RustBCA copy spelled it
// before gh:#995: `ChaCha8Rng::seed_from_u64`, `ChaCha8Rng::from_rng`,
// `set_stream`, `rng.random::<f64>()`, `Uniform::new(lo, hi).unwrap()` +
// `sample`, `Normal::new(mean, sd).unwrap()` + `sample`; plus the raw
// `next_u32` / `next_u64` / `fill_bytes` underneath them.
//
// Values are printed as hex bit patterns, so the comparison is exact.

use rand::distr::{Distribution, Uniform};
use rand::rngs::ChaCha8Rng;
use rand::{Rng, RngExt, SeedableRng};
use rand_distr::Normal;

/// FNV-1a over the 8 little-endian bytes of each value.
struct Fnv(u64);
impl Fnv {
    fn new() -> Self {
        Fnv(0xcbf2_9ce4_8422_2325)
    }
    fn add(&mut self, v: u64) {
        for b in v.to_le_bytes() {
            self.0 ^= u64::from(b);
            self.0 = self.0.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
}

const PREFIX: usize = 160;
const LONG: usize = 1_000_000;
const UNIFORMS: [(f64, f64); 3] = [(0.0, 1.0), (-2.5, 7.25), (1.0e-3, 1.0e3)];
const NORMALS: [(f64, f64); 2] = [(0.0, 1.0), (5.0e3, 1.0e2)];
// RustBCA's tail threshold (rand_distr ZIG_NORM_R), to count tail draws.
const ZIG_NORM_R: f64 = 3.654152885361008796;

fn rng_for(seed: u64, stream: u64) -> ChaCha8Rng {
    let mut r = ChaCha8Rng::seed_from_u64(seed);
    r.set_stream(stream);
    r
}

fn hex_line(tag: &str, vals: &[u64]) {
    print!("{tag} {}", vals.len());
    for v in vals {
        print!(" {v:016x}");
    }
    println!();
}

/// One step of the interleaved cycle; returns the values it drew.
fn mixed_step(r: &mut ChaCha8Rng, k: usize, u: &Uniform<f64>, n: &Normal<f64>) -> u64 {
    match k % 5 {
        0 => u64::from(r.next_u32()),
        1 => r.next_u64(),
        2 => r.random::<f64>().to_bits(),
        3 => u.sample(r).to_bits(),
        _ => n.sample(r).to_bits(),
    }
}

fn case(seed: u64, stream: u64) {
    println!("case {seed:016x} {stream:016x}");

    // Raw words, from a fresh generator each, so a buffer boundary is crossed
    // (64 words per refill) and, for u64 after one u32, straddled.
    let mut r = rng_for(seed, stream);
    let v: Vec<u64> = (0..PREFIX).map(|_| u64::from(r.next_u32())).collect();
    hex_line("u32", &v);
    let mut r = rng_for(seed, stream);
    let mut v = vec![u64::from(r.next_u32())];
    v.extend((0..PREFIX).map(|_| r.next_u64()));
    hex_line("u32+u64", &v);

    // fill_bytes with odd lengths (partial words), then one u32 after.
    let mut r = rng_for(seed, stream);
    let mut v = Vec::new();
    for len in [1usize, 3, 5, 7, 32, 13, 64, 250] {
        let mut buf = vec![0u8; len];
        r.fill_bytes(&mut buf);
        v.extend(buf.iter().map(|b| u64::from(*b)));
    }
    v.push(u64::from(r.next_u32()));
    hex_line("bytes", &v);

    // from_rng: child seeded from the parent's next 32 bytes.
    let mut parent = rng_for(seed, stream);
    let mut child = ChaCha8Rng::from_rng(&mut parent);
    let mut v: Vec<u64> = (0..40).map(|_| child.next_u64()).collect();
    v.push(parent.next_u64());
    hex_line("from_rng", &v);

    // random::<f64>()
    let mut r = rng_for(seed, stream);
    let v: Vec<u64> = (0..PREFIX).map(|_| r.random::<f64>().to_bits()).collect();
    hex_line("f64", &v);

    for (lo, hi) in UNIFORMS {
        let u = Uniform::new(lo, hi).unwrap();
        let mut r = rng_for(seed, stream);
        let v: Vec<u64> = (0..PREFIX).map(|_| u.sample(&mut r).to_bits()).collect();
        hex_line(&format!("uniform {:016x} {:016x}", lo.to_bits(), hi.to_bits()), &v);
    }
    for (m, s) in NORMALS {
        let n = Normal::new(m, s).unwrap();
        let mut r = rng_for(seed, stream);
        let v: Vec<u64> = (0..PREFIX).map(|_| n.sample(&mut r).to_bits()).collect();
        hex_line(&format!("normal {:016x} {:016x}", m.to_bits(), s.to_bits()), &v);
    }

    let u = Uniform::new(UNIFORMS[1].0, UNIFORMS[1].1).unwrap();
    let n = Normal::new(0.0, 1.0).unwrap();
    let mut r = rng_for(seed, stream);
    let v: Vec<u64> = (0..PREFIX).map(|k| mixed_step(&mut r, k, &u, &n)).collect();
    hex_line("mixed", &v);

    // Digests over LONG draws of each method (and the tail count for the
    // standard normal, so a reader can see the tail branch was exercised).
    let mut r = rng_for(seed, stream);
    let mut h = Fnv::new();
    (0..LONG).for_each(|_| h.add(u64::from(r.next_u32())));
    println!("digest u32 {LONG} {:016x}", h.0);
    let mut r = rng_for(seed, stream);
    let mut h = Fnv::new();
    (0..LONG).for_each(|_| h.add(r.next_u64()));
    println!("digest u64 {LONG} {:016x}", h.0);
    let mut r = rng_for(seed, stream);
    let mut h = Fnv::new();
    (0..LONG).for_each(|_| h.add(r.random::<f64>().to_bits()));
    println!("digest f64 {LONG} {:016x}", h.0);
    let mut r = rng_for(seed, stream);
    let mut h = Fnv::new();
    (0..LONG).for_each(|_| h.add(u.sample(&mut r).to_bits()));
    println!("digest uniform {LONG} {:016x}", h.0);
    let mut r = rng_for(seed, stream);
    let mut h = Fnv::new();
    let mut tail = 0usize;
    for _ in 0..LONG {
        let z = n.sample(&mut r);
        if z.abs() > ZIG_NORM_R {
            tail += 1;
        }
        h.add(z.to_bits());
    }
    println!("digest normal {LONG} {:016x} {tail}", h.0);
    let mut r = rng_for(seed, stream);
    let mut h = Fnv::new();
    (0..LONG).for_each(|k| h.add(mixed_step(&mut r, k, &u, &n)));
    println!("digest mixed {LONG} {:016x}", h.0);
}

fn main() {
    println!("# rand 0.10.3 / chacha20 0.10.2 / rand_distr 0.6.0 reference streams");
    println!("# generated by reference-data/rand/rand_reference_driver.rs (gh:#995)");
    println!("# format: see reference-data/rand/README.md");
    let cases: [(u64, u64); 8] = [
        (0, 0),
        (1, 0),
        (42, 0),
        (876, 0),
        (876, 876_000_001),
        (20_261_016, 0),
        (u64::MAX, 0),
        (u64::MAX, u64::MAX),
    ];
    for (seed, stream) in cases {
        case(seed, stream);
    }
}
