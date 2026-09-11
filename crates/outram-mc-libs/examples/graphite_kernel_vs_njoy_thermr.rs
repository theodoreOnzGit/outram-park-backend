//! Compare this crate's graphite S(α,β) **outgoing-energy distribution** against
//! NJOY2016's own THERMR scattering matrix — the last piece of the FHR pebble's
//! physics with no oracle on it.
//!
//! # Why this is the one left
//!
//! `graphite_vs_njoy_thermr.rs` checks the bound **cross sections** (±0.05 %),
//! which fix how *often* a neutron collides. This checks how much energy it
//! loses when it does, which is a completely independent function and the one
//! that sets where the 1/E slowing-down spectrum joins the Maxwellian.
//!
//! Two things that look like they cover it do not:
//!
//! - **Detailed balance does not pin it.** The kernel demonstrably equilibrates
//!   at the right temperature (`epithermal_slowing_down.rs`: graphite settles at
//!   0.97 × 2kT). But detailed balance constrains the *ratio* `P(E→E′)/P(E′→E)`;
//!   it leaves the thermalisation *rate* free. A kernel that moves neutrons
//!   through 0.1–4 eV too fast satisfies it exactly and still shifts the
//!   epithermal/thermal flux ratio.
//! - **The free-gas asymptote does not pin it either.** `ξ/ξ_fg = 1.002` at
//!   3.9 eV says the kernel approaches free gas at the top of its range, which is
//!   necessary but says nothing about 0.1–2 eV, where binding matters most.
//!
//! And it has the right signature: too much energy transfer per collision in the
//! joining region means fewer collisions inside the U-238 resonances, so `p` up,
//! `ε` down, `k` up — the shape of the residual (`op-mzvp.2.12`).
//!
//! # The oracle
//!
//! THERMR writes MF=6/MT=229 — the incoherent-inelastic `E → E′` matrix it
//! builds by integrating `S(α,β)` — from which `⟨E′⟩` and `⟨E′²⟩` follow by
//! quadrature, with no sampling and no model. The comparison is against the
//! **inelastic** channel only, so this program separates it from coherent
//! elastic by the one unambiguous signature: Bragg scattering leaves `E′ = E`
//! exactly.
//!
//! Deck as in `graphite_vs_njoy_thermr.rs` (`tape23` carries MT=229 and MT=230):
//!
//! ```text
//! GRAPHITE_THERMR=/home/user/graphiteoracle/tape23 cargo run --release \
//!     -p outram-mc-libs --features endf-pebble-cases \
//!     --example graphite_kernel_vs_njoy_thermr
//! ```

use outram_mc_libs::material::thermal::ThermalScattering;

const TEMP: f64 = 600.0;
const N: usize = 400_000;

fn main() {
    let Ok(path) = std::env::var("GRAPHITE_THERMR") else {
        eprintln!("SKIP: set GRAPHITE_THERMR to an NJOY THERMR tape (see the module docs)");
        return;
    };
    let text = std::fs::read_to_string(&path).expect("THERMR tape");
    let njoy = parse_mf6_law1(&text, 625, 229);
    eprintln!(
        "read {} incident energies from MF=6/MT=229 of {path}",
        njoy.len()
    );

    let tsl =
        njoy_outram_park_fork::reference_data::reference_endf("tsl-crystalline-graphite.endf")
            .expect("graphite tape");
    let g = ThermalScattering::from_endf_file(tsl.to_str().expect("path"), 30, TEMP, "c_Graphite")
        .expect("thermal law");

    println!("\ngraphite S(a,b) INCOHERENT INELASTIC outgoing energy, {TEMP} K");
    println!("ours: {N} samples of ThermalScattering::sample, coherent-elastic (E'==E) removed");
    println!("NJOY: quadrature on the THERMR MF=6/MT=229 matrix, no sampling\n");
    println!(
        "{:>11} {:>12} {:>12} {:>9} {:>12} {:>12} {:>9} {:>8}",
        "E [eV]", "<E'>/E NJOY", "<E'>/E ours", "rel", "xi NJOY", "xi ours", "rel", "elastic%"
    );

    let mut seed = 20_260_911_u64;
    let mut worst = 0.0_f64;
    for &(e, ref dist) in &njoy {
        if !(0.01..=4.0).contains(&e) {
            continue;
        }
        let (m1_n, mlog_n) = moments_from_matrix(dist, e);

        let (mut sum_ratio, mut sum_xi, mut n_inel, mut n_el) = (0.0, 0.0, 0usize, 0usize);
        for _ in 0..N {
            let Some((ep, _mu)) = g.sample(e, &mut seed) else {
                continue;
            };
            // Coherent elastic is the only channel that leaves the energy
            // exactly unchanged, so this separates the two without needing a
            // per-channel sampling entry point.
            if (ep - e).abs() <= 1.0e-12 * e {
                n_el += 1;
                continue;
            }
            if ep > 0.0 {
                sum_ratio += ep / e;
                sum_xi += (e / ep).ln();
                n_inel += 1;
            }
        }
        if n_inel == 0 {
            continue;
        }
        let k = n_inel as f64;
        let (m1_o, xi_o) = (sum_ratio / k, sum_xi / k);
        let rel1 = (m1_o - m1_n) / m1_n;
        let rel2 = if mlog_n.abs() > 1.0e-6 {
            (xi_o - mlog_n) / mlog_n.abs()
        } else {
            0.0
        };
        worst = worst.max(rel1.abs());
        println!(
            "{e:>11.4e} {m1_n:>12.5} {m1_o:>12.5} {:>8.2}% {mlog_n:>12.5} {xi_o:>12.5} \
             {:>8.2}% {:>7.1}%",
            100.0 * rel1,
            100.0 * rel2,
            100.0 * n_el as f64 / (n_el + n_inel) as f64
        );
    }
    println!(
        "\nWorst |Δ⟨E′⟩/E| = {:.2} %. The residual needs ~8 % fewer neutrons above\n\
         0.625 eV, and the moderator's energy transfer per collision is the term\n\
         that would do it, so a few percent here is an exclusion and a large\n\
         number would be the answer.\n\n\
         Read ⟨E′⟩/E, not ξ. ξ = ⟨ln(E/E′)⟩ passes through **zero** near 0.082 eV,\n\
         where net up-scatter turns into net down-scatter, so a *relative*\n\
         difference on it blows up there for arithmetic reasons and means\n\
         nothing. ⟨E′⟩/E has no zero anywhere in this range.",
        100.0 * worst
    );
}

/// `(⟨E′⟩/E, ⟨ln(E/E′)⟩)` from one incident energy's tabulated outgoing
/// distribution, by trapezoid on the `(E′, f)` pairs.
///
/// The `f` column is the probability density in `E′`; the `μ` columns that
/// follow each pair are the angular data and are not needed for an energy
/// moment. Bins with `E′ = 0` are skipped in the log moment only.
fn moments_from_matrix(pairs: &[(f64, f64)], e: f64) -> (f64, f64) {
    let (mut norm, mut m1, mut mlog) = (0.0, 0.0, 0.0);
    for w in pairs.windows(2) {
        let ((e0, f0), (e1, f1)) = (w[0], w[1]);
        let de = e1 - e0;
        if de <= 0.0 {
            continue;
        }
        norm += 0.5 * (f0 + f1) * de;
        m1 += 0.5 * (f0 * e0 + f1 * e1) * de;
        if e0 > 0.0 {
            mlog += 0.5 * (f0 * (e / e0).ln() + f1 * (e / e1).ln()) * de;
        }
    }
    if norm <= 0.0 {
        return (0.0, 0.0);
    }
    (m1 / norm / e, mlog / norm)
}

/// Pull the LAW=1 outgoing-energy tables out of an ENDF MF=6 section.
///
/// THERMR writes, per incident energy, a LIST record whose body is `NEP` groups
/// of `NA + 2` numbers — `(E′, f, μ₁…μ_NA)`. The group width is carried in the
/// record's last field (`NA + 2`), and `NEP` follows from the body length, which
/// is how this reader recovers the layout rather than assuming `NA`.
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

    let field = |l: &str, i: usize| -> f64 { endf_f(&l[i * 11..(i + 1) * 11]) };
    let ifield = |l: &str, i: usize| -> i64 { l[i * 11..(i + 1) * 11].trim().parse().unwrap_or(0) };

    let mut out = Vec::new();
    let mut i = 0usize;
    // Walk to the LAW=1 TAB2 (the record whose 6th field is the incident-energy
    // count and whose body is LIST records), then read each LIST.
    while i < lines.len() {
        let l = lines[i];
        let (n1, n2) = (ifield(l, 4), ifield(l, 5));
        // A LIST record for one incident energy: N1 = NW (body length), N2 =
        // values per outgoing energy, and field 1 is the incident energy.
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

/// ENDF's compressed float form: `1.234567+5` means `1.234567e5`.
fn endf_f(s: &str) -> f64 {
    let t = s.trim();
    if t.is_empty() {
        return 0.0;
    }
    if let Ok(v) = t.parse::<f64>() {
        return v;
    }
    // Insert the missing 'e' before the last +/- that is not the leading sign.
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
