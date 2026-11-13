//! boon-lay's RustBCA copy against UNMODIFIED upstream RustBCA v3.0.0 (gh:#1426,
//! epic #1336).
//!
//! # Methodology
//!
//! `verification_and_validation/rustbca_differential/shared.rs` is ONE harness
//! that drives a RustBCA crate through three layers; it is compiled twice, once
//! against upstream `lcpp-org/RustBCA` tag v3.0.0 (`673bd3f`, driver
//! `driver/driver_main.rs`, built outside the workspace) and once here against
//! `boon_lay::rustbca`. `rustbca_v3.0.0_reference.txt.gz` is the upstream output.
//! Floats are compared as raw bit patterns.
//!
//! 1. **Kernel** (`K`): `calculate_binary_collision`, 4320 rows (3 projectiles x 3
//!    targets x 5 potentials x 4 integrals x 4 energies x 6 impact parameters),
//!    no random numbers.
//! 2. **Functions** (`F`): screening lengths, interaction potentials, `phi`/`dphi`,
//!    Lindhard-Scharff and Bethe-Bloch cross sections.
//! 3. **Seeded cascades** (`N`, `P`, `T`, `U`, `D`): 90 cases x 8 ions, the
//!    same `ChaCha8Rng` seed and stream on both sides (boon-lay's
//!    `petir::rng::ChaCha8Rng` is a draw-for-draw port, gh:#995). Every output particle
//!    (digest), the incident ion and every particle that left (full), and the
//!    incident ion's collision-by-collision trajectory (digest, first and last
//!    five entries in full).
//!
//! The only intended difference between the two builds is `libm`
//! (`petir::real::Real`, musl-derived) versus the platform libm.
//!
//! # Results
//!
//! Recorded in `verification_and_validation/rustbca_differential/RESULTS.md`
//! together with the predictions that preceded the runs (`PREDICTIONS.md`).

#[path = "../verification_and_validation/rustbca_differential/shared.rs"]
mod shared;

use boon_lay::rustbca as rb;
use miniz_oxide::inflate::decompress_to_vec;
use petir::rng::{ChaCha8Rng, SeedableRng};
use std::collections::BTreeMap;

pub type Rng8 = ChaCha8Rng;

pub fn make_rng(seed: u64, stream: u64) -> Rng8 {
    let mut r = ChaCha8Rng::seed_from_u64(seed);
    r.set_stream(stream);
    r
}

const REFERENCE_GZ: &[u8] = include_bytes!(
    "../verification_and_validation/rustbca_differential/rustbca_v3.0.0_reference.txt.gz"
);

fn reference_lines() -> Vec<String> {
    let raw = decompress_to_vec(&REFERENCE_GZ[10..REFERENCE_GZ.len() - 8]).expect("inflate");
    String::from_utf8(raw).expect("utf8").lines().map(str::to_string).collect()
}

fn is_bits(tok: &str) -> bool {
    tok.len() == 16 && tok.bytes().all(|b| b.is_ascii_hexdigit())
}

/// Split a line into a key and its values. K and F lines: the key is every token
/// that is not a 16-hex-digit value. P, T, U, D, N: a fixed-length key.
fn split(line: &str) -> (String, Vec<String>) {
    let toks: Vec<&str> = line.split(' ').collect();
    let key_len = match toks[0] {
        "P" => 4,
        "T" => 5,
        "U" => 4,
        "D" => 3,
        "N" => 3,
        "H" => toks.len(),
        _ => 0,
    };
    if key_len > 0 {
        let k = key_len.min(toks.len());
        return (toks[..k].join(" "), toks[k..].iter().map(|s| s.to_string()).collect());
    }
    let key: Vec<&str> = toks.iter().copied().filter(|t| !is_bits(t)).collect();
    let vals: Vec<String> = toks.iter().copied().filter(|t| is_bits(t)).map(|s| s.to_string()).collect();
    (key.join(" "), vals)
}

/// Statistics of one comparison.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Stats {
    /// Floating-point values compared (16-hex tokens other than digests).
    pub values: usize,
    pub bit_identical: usize,
    /// Differences of exactly 1 ulp, and of 2..=4 ulp, and of more.
    pub ulp1: usize,
    pub ulp2_4: usize,
    pub ulp_more: usize,
    pub worst_rel: f64,
    pub worst_key: String,
    /// Lines on one side only, or whose non-float fields (flags, counts, status) differ.
    pub structural: usize,
    pub first_structural: String,
    /// `D` and `U` digests that differ (an ion's particles, a trajectory).
    pub digest_differs: usize,
}

fn ulps(a: f64, b: f64) -> u64 {
    if a.is_nan() || b.is_nan() || (a.is_sign_negative() != b.is_sign_negative() && a != b) {
        return u64::MAX;
    }
    let (x, y) = (a.to_bits() as i64, b.to_bits() as i64);
    x.abs_diff(y)
}

fn rel(a: f64, b: f64) -> f64 {
    if a == b {
        return 0.0;
    }
    (a - b).abs() / a.abs().max(b.abs())
}

/// Compare the lines of `kind` ('K', 'F', or the cascade kinds) of the
/// reference and of this build. Digest fields (`D`, `U` lines and the `N` count) are
/// compared exactly and counted as `structural` when they differ.
pub fn compare(reference: &[String], got: &[String], kinds: &str) -> Stats {
    let pick = |v: &[String]| -> BTreeMap<String, Vec<String>> {
        v.iter()
            .filter(|l| kinds.contains(&l[..1]))
            .map(|l| split(l))
            .collect()
    };
    let r = pick(reference);
    let g = pick(got);
    let mut s = Stats::default();
    let note = |s: &mut Stats, k: &str| {
        s.structural += 1;
        if s.first_structural.is_empty() {
            s.first_structural = k.to_string();
        }
    };
    for (k, rv) in &r {
        match g.get(k) {
            None => note(&mut s, k),
            Some(gv) => {
                // D: digest of every particle of an ion. U: trajectory length, then
                // the digest of the trajectory. N: particle count. A digest says only
                // "identical or not"; the length and the count are structural.
                if k.starts_with('D') || k.starts_with('U') || k.starts_with('N') {
                    if rv.len() != gv.len() {
                        note(&mut s, k);
                        continue;
                    }
                    for (i, (a, b)) in rv.iter().zip(gv.iter()).enumerate() {
                        if a != b {
                            if is_bits(a) && i + 1 == rv.len() && !k.starts_with('N') {
                                s.digest_differs += 1;
                            } else {
                                note(&mut s, k);
                            }
                        }
                    }
                    continue;
                }
                if rv.len() != gv.len() {
                    note(&mut s, k);
                    continue;
                }
                let mut line_structural = false;
                for (a, b) in rv.iter().zip(gv.iter()) {
                    if is_bits(a) && is_bits(b) {
                        let fa = f64::from_bits(u64::from_str_radix(a, 16).unwrap());
                        let fb = f64::from_bits(u64::from_str_radix(b, 16).unwrap());
                        s.values += 1;
                        let u = ulps(fa, fb);
                        match u {
                            0 => s.bit_identical += 1,
                            1 => s.ulp1 += 1,
                            2..=4 => s.ulp2_4 += 1,
                            _ => s.ulp_more += 1,
                        }
                        let d = rel(fa, fb);
                        if d > s.worst_rel {
                            s.worst_rel = d;
                            s.worst_key = k.clone();
                        }
                    } else if a != b {
                        line_structural = true;
                    }
                }
                if line_structural {
                    note(&mut s, k);
                }
            }
        }
    }
    for k in g.keys() {
        if !r.contains_key(k) {
            note(&mut s, k);
        }
    }
    s
}

fn print(name: &str, s: &Stats) {
    println!(
        "{name}: values {} bit-identical {} ulp1 {} ulp2-4 {} ulp>4 {} worst_rel {:e} at [{}] structural {} first [{}] digest_differs {}",
        s.values, s.bit_identical, s.ulp1, s.ulp2_4, s.ulp_more, s.worst_rel, s.worst_key, s.structural, s.first_structural, s.digest_differs
    );
}

/// Per-ion identity: an ion is identical when every line of its group matches
/// the reference exactly as text.
fn ions_identical(reference: &[String], got: &[String]) -> (usize, usize) {
    fn groups(v: &[String]) -> BTreeMap<(usize, usize), Vec<&String>> {
        let mut m: BTreeMap<(usize, usize), Vec<&String>> = BTreeMap::new();
        for l in v {
            let t: Vec<&str> = l.split(' ').collect();
            if matches!(t[0], "N" | "P" | "T" | "U" | "D") {
                m.entry((t[1].parse().unwrap(), t[2].parse().unwrap())).or_default().push(l);
            }
        }
        m
    }
    let r = groups(reference);
    let g = groups(got);
    let same = r.iter().filter(|(k, v)| g.get(*k) == Some(v)).count();
    (same, r.len())
}

/// The harness records the panics of unimplemented combinations (MAGIC with
/// Lenz-Jensen) as `PANIC` rows, as upstream's driver does; keep stderr quiet.
fn quiet_panics() {
    std::panic::set_hook(Box::new(|_| {}));
}

#[test]
fn reference_has_the_expected_shape() {
    let r = reference_lines();
    let count = |c: &str| r.iter().filter(|l| l.starts_with(c)).count();
    assert_eq!(count("K "), 4320);
    assert_eq!(count("F "), 418);
    assert_eq!(count("H "), 90);
    assert_eq!(count("N "), 720);
    assert_eq!(count("D "), 720);
}

#[test]
fn kernel_and_functions_against_upstream() {
    quiet_panics();
    let r = reference_lines();
    let k = compare(&r, &shared::kernel_lines(), "K");
    print("kernel", &k);
    // Pinned from the first run (2026-11-13, libm 0.2.16 in Cargo.lock): counts of
    // values bit-identical to upstream / 1 ulp / 2-4 ulp / more, in that order.
    // The bound on the worst relative difference is the measured 3.0553e-11 rounded
    // up; it was set from the measurement, never loosened since.
    assert_eq!(
        (k.values, k.bit_identical, k.ulp1, k.ulp2_4, k.ulp_more, k.structural),
        (24624, 21088, 1910, 499, 1127, 0),
        "kernel: bit-identical/ulp counts moved"
    );
    assert!(k.worst_rel <= 3.1e-11, "kernel worst relative difference {:e}", k.worst_rel);
    let f = compare(&r, &shared::function_lines(), "F");
    print("functions", &f);
    assert_eq!(
        (f.values, f.bit_identical, f.ulp1, f.ulp2_4, f.ulp_more, f.structural),
        (2116, 1918, 139, 41, 18, 0),
        "functions: bit-identical/ulp counts moved"
    );
    assert!(f.worst_rel <= 1.9e-15, "functions worst relative difference {:e}", f.worst_rel);
}

#[test]
fn cascades_against_upstream() {
    let r = reference_lines();
    let got = shared::cascade_lines();
    let c = compare(&r, &got, "PTUDN");
    print("cascades", &c);
    let (same, total) = ions_identical(&r, &got);
    println!("ions bit-identical in every line: {same} of {total}");
    // Structure (particle counts, flags, collision counts, trajectory lengths)
    // identical in all 720 ions; floating-point values differ at libm level only.
    assert_eq!(c.structural, 0, "first structural difference: {}", c.first_structural);
    assert_eq!(
        (c.values, c.bit_identical, c.ulp1, c.ulp2_4, c.ulp_more, c.digest_differs),
        (39848, 26625, 3345, 4460, 5418, 1378),
        "cascades: bit-identical/ulp counts moved"
    );
    assert!(c.worst_rel <= 1.1e-11, "cascade worst relative difference {:e}", c.worst_rel);
    assert_eq!((same, total), (9, 720));
    let _ = rb::bca::single_ion_bca::<rb::geometry::Mesh0D>;
}

/// Localisation aid, a no-op unless `RBCA_DUMP_FULL=<path>` is set: writes this
/// build's full-detail output (kernel, functions, every cascade particle and
/// trajectory entry) in the format of `driver_main.rs --full`, so that the two
/// can be diffed line by line to find the first divergent operation.
#[test]
fn dump_full_output_when_asked() {
    quiet_panics();
    if let Ok(path) = std::env::var("RBCA_DUMP_FULL") {
        shared::set_full(true);
        let mut text = shared::all_lines().join("\n");
        text.push('\n');
        std::fs::write(path, text).expect("write dump");
        shared::set_full(false);
    }
}
