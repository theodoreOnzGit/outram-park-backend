//! **Size the S(α,β) emission grid by measurement:** sweep `N_EMIT_GRID` against
//! NJOY2016 THERMR's own scattering matrix and find where the sampled kernel
//! stops improving, for both graphite and light water.
//!
//! # Why this exists
//!
//! `ThermalScattering` pre-tabulates the secondary-energy/angle law on a
//! log-spaced incident-energy grid of `N_EMIT_GRID` points and picks between the
//! two bracketing tables by ACE statistical interpolation. That choice is
//! unbiased in the **mean** — it reproduces a linear interpolation of ⟨E′⟩
//! between the two grid points — but it adds a variance
//!
//! ```text
//!   var_spurious = r(1-r)(m2 - m1)^2
//! ```
//!
//! that the true kernel has not got, where `m1`, `m2` are the two tables' means
//! and `r` the interpolation factor. At 48 points over 1e-5 … 4 eV adjacent
//! tables are **31.6 % apart** in incident energy, and that term is large enough
//! to dominate wherever the kernel's own spread is small — above ~0.4 eV, where
//! the relative width has fallen to ~0.12. It was worth **+39 % too broad at
//! 2 eV** in graphite (GitHub #190, bead `op-x77y`).
//!
//! The fix is a finer grid, and the size of that grid is a *measurement*, not a
//! preference: `var_spurious` falls as the square of the spacing, so each
//! doubling should quarter the excess until some other error floor is reached.
//! This program finds that floor, and prices it — reconstruction time and memory
//! both scale linearly in `N_EMIT_GRID`.
//!
//! # Method
//!
//! For each grid size, build the law with
//! [`ThermalScattering::from_tape_with_emission_grid`], time the build, then at
//! each probe energy draw `N` samples and reduce them to
//!
//! - `⟨E′⟩/E` — the first moment, which the coarse grid does **not** spoil;
//! - `sqrt(var(E′))/⟨E′⟩` — the relative width, which it does;
//! - `ξ = ⟨ln(E/E′)⟩` — the logarithmic decrement.
//!
//! The oracle side is a trapezoid quadrature on THERMR's MF=6 matrix
//! (MT=229 for graphite at 600 K, MT=222 for H-in-H₂O at 293.6 K) — no sampling
//! on that side. The coherent-elastic channel is removed from the sampled side
//! by the one unambiguous signature, `E′ == E`, since MF=6 describes the
//! inelastic channel only.
//!
//! # Running it
//!
//! ```text
//! GRAPHITE_THERMR=/home/user/graphiteoracle/tape23 \
//! H2O_THERMR=/home/user/h2ooracle/tape23 \
//! cargo run --release -p outram-mc-libs --features endf-pebble-cases \
//!     --example thermal_emission_grid_convergence
//! ```
//!
//! Both tapes are 24–28 MB NJOY outputs and are not checked in; without them
//! the program prints the build-cost sweep alone, which needs no oracle.

use outram_mc_libs::material::thermal::ThermalScattering;
use std::time::Instant;

/// `(n_emit, n_outgoing)` configurations swept.
///
/// The first block doubles the **incident-energy** grid at the crate's
/// 16 outgoing bins: 48 is the value the defect was found at, and the
/// `1/spacing^2` law predicts each doubling quarters the spurious variance.
/// The second block holds the incident grid at 384 and doubles the
/// **equiprobable outgoing-energy bins** instead, which is the other error and
/// the one that does not respond to the first.
const CONFIGS: &[(usize, usize)] = &[
    (48, 16),
    (96, 16),
    (192, 16),
    (384, 16),
    (768, 16),
    (1536, 16),
    (384, 32),
    (384, 64),
    (384, 128),
    (192, 64),
    (768, 64),
    (1536, 64),
];

/// Samples per probe energy per grid size.
const N: usize = 400_000;

/// Samples used for the transport-loop throughput measurement.
const THROUGHPUT_N: usize = 20_000_000;

const GRAPHITE_PROBES: &[f64] = &[
    1.012e-3, 2.6e-3, 5.0e-3, 1.0e-2, 2.53e-2, 5.0e-2, 0.1035, 0.2, 0.39, 0.625, 1.05, 2.02, 3.75,
];

const H2O_PROBES: &[f64] = &[
    1.5e-3, 5.0e-3, 1.0e-2, 2.53e-2, 5.0e-2, 0.11157, 0.2, 0.41704, 0.625, 1.05, 1.855,
];

fn main() {
    sweep(
        "graphite (tsl-crystalline-graphite MAT 30, 600 K)",
        "tsl-crystalline-graphite.endf",
        30,
        600.0,
        "c_Graphite",
        "GRAPHITE_THERMR",
        625,
        229,
        GRAPHITE_PROBES,
    );
    sweep(
        "H in H2O (tsl-HinH2O MAT 1, 293.6 K)",
        "tsl-HinH2O.endf",
        1,
        293.6,
        "c_H_in_H2O",
        "H2O_THERMR",
        125,
        222,
        H2O_PROBES,
    );
}

/// One material's sweep: build cost per grid size, then the three moments at
/// every probe energy against the MF=6 quadrature.
#[allow(clippy::too_many_arguments)]
fn sweep(
    label: &str,
    tsl: &str,
    mat: i32,
    temp_k: f64,
    name: &str,
    env: &str,
    njoy_mat: i32,
    njoy_mt: i32,
    probes: &[f64],
) {
    use njoy_outram_park_fork::endf::tape::Tape;

    println!("\n================================================================");
    println!("  {label}");
    println!("================================================================");

    let Some(path) = njoy_outram_park_fork::reference_data::reference_endf(tsl) else {
        println!("  SKIP: {tsl} not in reference-data/endf/");
        return;
    };
    let tape = Tape::read(std::fs::File::open(&path).expect("tsl")).expect("parse tsl");

    // The oracle: (probe -> (E_tape, m1/E, width, xi)) by quadrature on MF=6.
    let oracle: Vec<(f64, f64, f64, f64)> = match std::env::var(env) {
        Ok(p) => {
            let text = std::fs::read_to_string(&p).expect("THERMR tape");
            let matrix = parse_mf6_law1(&text, njoy_mat, njoy_mt);
            println!(
                "  oracle: {} incident energies from MF=6/MT={njoy_mt} of {p}",
                matrix.len()
            );
            probes
                .iter()
                .map(|&probe| oracle_moments(&matrix, probe))
                .collect()
        }
        Err(_) => {
            println!("  no {env} set — build-cost sweep only, no oracle");
            Vec::new()
        }
    };

    println!(
        "\n  {:>6} {:>6} {:>10} {:>10} {:>9} {:>12}",
        "n_emit", "n_out", "build [s]", "mem [kB]", "s/table", "Msample/s"
    );
    let mut laws: Vec<(String, ThermalScattering)> = Vec::new();
    for &(n, n_out) in CONFIGS {
        let t0 = Instant::now();
        let law = ThermalScattering::from_tape_with_grids(&tape, mat, temp_k, name, n, n_out)
            .expect("thermal law");
        let dt = t0.elapsed().as_secs_f64();
        // n_out outgoing energies + n_out*8 cosines per table, f64.
        let mem_kb = n as f64 * (n_out as f64 * 9.0) * 8.0 / 1024.0;
        // Transport-loop cost: the table is chosen at random out of `n_emit`
        // and one bin out of `n_out` inside it, so a larger table is a larger
        // random-access working set. That, not the build, is what a k-eff run
        // pays for a finer grid.
        let mut seed = 20_260_912_u64;
        let t1 = Instant::now();
        let mut acc = 0.0f64;
        for i in 0..THROUGHPUT_N {
            // Sweep the incident energy across the whole table so the access
            // pattern is the scattered one transport produces, not one hot row.
            let e = 1.0e-3 * (4.0e3f64).powf(i as f64 / THROUGHPUT_N as f64);
            if let Some((ep, mu)) = law.sample(e, &mut seed) {
                acc += ep + mu;
            }
        }
        let msamp = THROUGHPUT_N as f64 / t1.elapsed().as_secs_f64() / 1.0e6;
        std::hint::black_box(acc);
        println!(
            "  {n:>6} {n_out:>6} {dt:>10.2} {mem_kb:>10.1} {:>9.4} {msamp:>12.2}",
            dt / n as f64
        );
        laws.push((format!("{n}/{n_out}"), law));
    }

    if oracle.is_empty() {
        return;
    }

    for (col, title) in [
        (
            0usize,
            "relative WIDTH sqrt(var(E'))/<E'>  — what the coarse grid spoils",
        ),
        (
            1usize,
            "first moment <E'>/E                — what it does not",
        ),
        (2usize, "xi = <ln(E/E')>"),
    ] {
        println!("\n  {title}");
        print!("  {:>10} {:>10}", "E [eV]", "NJOY");
        for (tag, _) in &laws {
            print!(" {tag:>9}");
        }
        println!();
        let mut worst = vec![(0.0f64, 0.0f64); laws.len()];
        let mut rms = vec![0.0f64; laws.len()];
        for (i, &_probe) in probes.iter().enumerate() {
            let (e_tape, m1_n, w_n, xi_n) = oracle[i];
            let ref_v = [w_n, m1_n, xi_n][col];
            print!("  {e_tape:>10.4e} {ref_v:>10.5}");
            for (j, (_, law)) in laws.iter().enumerate() {
                let mut seed = 20_260_912_u64;
                let (m1, w, xi) = sampled_moments(law, e_tape, N, &mut seed);
                let ours = [w, m1, xi][col];
                // xi passes through zero, so report its deviation absolutely.
                let rel = if col == 2 {
                    ours - ref_v
                } else {
                    ours / ref_v - 1.0
                };
                print!(" {:>+8.2}%", 100.0 * rel);
                if rel.abs() > worst[j].0.abs() {
                    worst[j] = (rel, e_tape);
                }
                rms[j] += rel * rel;
            }
            println!();
        }
        print!("  {:>10} {:>10}", "worst", "");
        for (j, _) in laws.iter().enumerate() {
            print!(" {:>+8.2}%", 100.0 * worst[j].0);
        }
        println!();
        print!("  {:>10} {:>10}", "rms", "");
        for (j, _) in laws.iter().enumerate() {
            print!(" {:>8.2}%", 100.0 * (rms[j] / probes.len() as f64).sqrt());
        }
        println!();
    }
}

/// `(<E'>/E, sqrt(var)/<E'>, xi)` from `n` samples of the law at `e`, with the
/// coherent-elastic spike (`E' == E`) removed.
fn sampled_moments(law: &ThermalScattering, e: f64, n: usize, seed: &mut u64) -> (f64, f64, f64) {
    let (mut s1, mut s2, mut sxi, mut k) = (0.0, 0.0, 0.0, 0usize);
    for _ in 0..n {
        let Some((ep, _mu)) = law.sample(e, seed) else {
            continue;
        };
        if (ep - e).abs() <= 1.0e-12 * e || ep <= 0.0 {
            continue;
        }
        s1 += ep;
        s2 += ep * ep;
        sxi += (e / ep).ln();
        k += 1;
    }
    assert!(k > n / 100, "no inelastic scatters at {e} eV");
    let kf = k as f64;
    let (m1, m2) = (s1 / kf, s2 / kf);
    (m1 / e, (m2 - m1 * m1).max(0.0).sqrt() / m1, sxi / kf)
}

/// `(E_tape, <E'>/E, sqrt(var)/<E'>, xi)` by trapezoid quadrature on the MF=6
/// row nearest `probe` in log energy.
fn oracle_moments(matrix: &[(f64, Vec<(f64, f64)>)], probe: f64) -> (f64, f64, f64, f64) {
    let (e, dist) = matrix
        .iter()
        .filter(|(e, _)| (1.0e-4..=4.0).contains(e))
        .min_by(|a, b| {
            (a.0 / probe)
                .ln()
                .abs()
                .partial_cmp(&(b.0 / probe).ln().abs())
                .unwrap()
        })
        .expect("no MF=6 rows");
    let (mut norm, mut m1, mut m2, mut mlog) = (0.0, 0.0, 0.0, 0.0);
    for w in dist.windows(2) {
        let ((e0, f0), (e1, f1)) = (w[0], w[1]);
        let de = e1 - e0;
        if de <= 0.0 {
            continue;
        }
        norm += 0.5 * (f0 + f1) * de;
        m1 += 0.5 * (f0 * e0 + f1 * e1) * de;
        m2 += 0.5 * (f0 * e0 * e0 + f1 * e1 * e1) * de;
        let (l0, l1) = (
            if e0 > 0.0 { (e / e0).ln() } else { 0.0 },
            if e1 > 0.0 { (e / e1).ln() } else { 0.0 },
        );
        mlog += 0.5 * (f0 * l0 + f1 * l1) * de;
    }
    let (m1, m2, mlog) = (m1 / norm, m2 / norm, mlog / norm);
    (*e, m1 / e, (m2 - m1 * m1).max(0.0).sqrt() / m1, mlog)
}

/// `(E_in, [(E', f)])` rows of a THERMR MF=6 LAW=1 section — the same walk the
/// other `*_vs_njoy_thermr` examples use, with the cosine columns dropped.
fn parse_mf6_law1(text: &str, mat: i32, mt: i32) -> Vec<(f64, Vec<(f64, f64)>)> {
    let lines: Vec<&str> = text
        .lines()
        .filter(|l| {
            l.len() >= 75
                && l[66..70].trim().parse::<i32>() == Ok(mat)
                && l[70..72].trim().parse::<i32>() == Ok(6)
                && l[72..75].trim().parse::<i32>() == Ok(mt)
        })
        .collect();
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < lines.len() {
        let l = lines[i];
        let (n1, n2) = (ifield(l, 4), ifield(l, 5));
        if n1 > 0 && n2 > 2 && n1 % n2 == 0 && field(l, 0) == 0.0 {
            let (nw, stride) = (n1 as usize, n2 as usize);
            let e_in = field(l, 1);
            let mut vals: Vec<f64> = Vec::with_capacity(nw);
            let mut j = i + 1;
            while vals.len() < nw && j < lines.len() {
                for c in 0..6 {
                    if vals.len() == nw {
                        break;
                    }
                    let s = &lines[j][c * 11..(c + 1) * 11];
                    if s.trim().is_empty() {
                        break;
                    }
                    vals.push(endf_f(s));
                }
                j += 1;
            }
            let dist: Vec<(f64, f64)> = vals.chunks_exact(stride).map(|c| (c[0], c[1])).collect();
            if dist.len() > 2 {
                out.push((e_in, dist));
            }
            i = j;
            continue;
        }
        i += 1;
    }
    out
}

fn field(l: &str, i: usize) -> f64 {
    endf_f(&l[i * 11..(i + 1) * 11])
}

fn ifield(l: &str, i: usize) -> i64 {
    l[i * 11..(i + 1) * 11].trim().parse().unwrap_or(0)
}

/// ENDF's compressed float form: `1.234567+5` means `1.234567e5`.
fn endf_f(s: &str) -> f64 {
    let t = s.trim();
    if t.is_empty() {
        return 0.0;
    }
    if let Ok(v) = t.parse::<f64>() {
        return v;
    }
    let b = t.as_bytes();
    for k in (1..b.len()).rev() {
        if (b[k] == b'+' || b[k] == b'-') && b[k - 1] != b'e' && b[k - 1] != b'E' {
            let mut s2 = String::with_capacity(t.len() + 1);
            s2.push_str(&t[..k]);
            s2.push('e');
            s2.push_str(&t[k..]);
            return s2.parse().unwrap_or(0.0);
        }
    }
    0.0
}
