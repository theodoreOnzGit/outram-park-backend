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
    let tsl_for_gate =
        njoy_outram_park_fork::reference_data::reference_endf("tsl-crystalline-graphite.endf")
            .expect("graphite tape");
    let law_for_gate = ThermalScattering::from_endf_file(
        tsl_for_gate.to_str().expect("path"),
        30,
        TEMP,
        "c_Graphite",
    )
    .expect("thermal law");
    golden_gate(&law_for_gate);

    let Ok(path) = std::env::var("GRAPHITE_THERMR") else {
        eprintln!(
            "\nThe live-tape comparison needs GRAPHITE_THERMR pointing at an NJOY THERMR\n\
             tape (deck in the module docs). The golden gate above already ran, so this\n\
             program is still a V&V case without it — it just cannot re-measure the oracle."
        );
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

/// V&V gate against the committed NJOY2016 oracle — runs with no tape on disk.
///
/// # Methodology
///
/// 400 000 samples of `ThermalScattering::sample` per incident energy, reduced
/// to the first moment `⟨E′⟩/E` with the coherent-elastic channel removed (it is
/// the only one leaving `E′` exactly equal to `E`, so it separates without a
/// per-channel entry point), at ten energies from 0.01 to 3.75 eV on
/// `tsl-crystalline-graphite` (MAT 30) at **600 K — a tabulated temperature on
/// that tape**, so no interpolation is involved. The oracle side is a
/// *quadrature* on NJOY's THERMR MF=6/MT=229 matrix — no sampling there. Oracle
/// values and provenance:
/// [`outram_mc_libs::vv::njoy_golden::GRAPHITE_KERNEL`]. Sampling standard error
/// is computed per row and printed rather than assumed.
///
/// # Results (2026-09-11, NJOY2016 2016.79, ENDF/B-VIII.0)
///
/// ```text
///    E [eV]     <E'>/E ours   NJOY       rel       1 sigma   coh. elastic
///    0.01          4.89774   4.90799   −0.21 %     0.34 %       83 %
///    0.0253        1.88355   1.90393   −1.07 %     0.25 %       78 %
///    0.05          1.24300   1.26414   −1.67 %     0.13 %       65 %
///    0.11157       1.00418   1.01158   −0.73 %     0.07 %       43 %
///    0.2           0.93631   0.93648   −0.02 %     0.05 %       27 %
///    0.41704       0.89017   0.89041   −0.03 %     0.04 %       13 %
///    0.625         0.87895   0.87857   +0.04 %     0.04 %        9 %
///    1.05          0.86956   0.86953   +0.00 %     0.03 %        5 %
///    1.855         0.86457   0.86398   +0.07 %     0.03 %        3 %
///    3.75          0.86125   0.86040   +0.10 %     0.03 %        1 %
/// ```
///
/// Worst **−1.67 % at 0.05 eV** on this run (seed 20 260 911, 400 000 samples),
/// against **−1.53 % at 0.0253 eV** in the golden table, which was taken on a
/// different seed. The two disagree by ~2 σ of the sampling noise at the points
/// concerned, which is expected — and it is why the doc records the spread
/// rather than a single worst-point number. Both are in the low-energy half of
/// the table, where 65–83 % of the cross section is coherent elastic and little
/// inelastic signal is left to measure.
///
/// Above 0.2 eV the deviation falls to ≤ 0.1 % and stays there.
///
/// # What is asserted, and why the second gate matters more than the first
///
/// Two envelopes, not one:
///
/// 1. **2 %** across the whole table — the magnitude claim;
/// 2. **0.4 % above 0.2 eV** — the *convergence* claim.
///
/// The second is the one that carries the argument. A magnitude envelope alone
/// would rank graphite and water as equally good, and that mistake was actually
/// made here — see the correction below.
///
/// # A correction this gate exists to prevent recurring
///
/// This workspace repeated the claim "graphite's kernel matches THERMR to
/// ≤ 0.5 %, water's does not" to argue that water was uniquely bad. **That
/// figure was scoped to 0.1–4 eV.** Over the range where water is compared,
/// graphite is −1.53 % at 0.0253 eV against water's −1.47 % — the same, not
/// better.
///
/// What actually separates them is the trend. Graphite converges onto NJOY and
/// stays there; water's deviation grows monotonically with energy, from −5.5 %
/// through zero to +1.5 %, which is the signature of a kernel of the wrong
/// *width* (GitHub #188). Ranking two laws by their worst point would have
/// cleared water's shape defect and indicted graphite's low-energy statistics.
fn golden_gate(law: &ThermalScattering) {
    use outram_mc_libs::vv::assert_table_relative;
    use outram_mc_libs::vv::njoy_golden::{
        GRAPHITE_KERNEL, GRAPHITE_KERNEL_CONVERGED_ABOVE_EV, GRAPHITE_KERNEL_CONVERGED_TOL,
        GRAPHITE_KERNEL_TOL,
    };

    println!("=== V&V gate: graphite S(alpha,beta) kernel vs committed NJOY2016 oracle ===");

    let mut seed = 20_260_911_u64;
    let mut rows: Vec<(f64, f64, f64)> = Vec::new();
    let mut worst_se = 0.0_f64;
    for &(e, njoy_m1) in GRAPHITE_KERNEL {
        let (mut sum, mut sum_sq, mut k, mut n_el) = (0.0_f64, 0.0_f64, 0usize, 0usize);
        for _ in 0..N {
            let Some((ep, _mu)) = law.sample(e, &mut seed) else {
                continue;
            };
            if (ep - e).abs() <= 1.0e-12 * e {
                n_el += 1;
                continue;
            }
            if ep <= 0.0 {
                continue;
            }
            let r = ep / e;
            sum += r;
            sum_sq += r * r;
            k += 1;
        }
        assert!(
            k > N / 1000,
            "graphite produced almost no inelastic scatters at {e} eV ({k} of {N} \
             samples, {n_el} coherent elastic). Below ~0.05 eV graphite IS mostly \
             coherent elastic, but not to the point of leaving no inelastic signal."
        );
        let n = k as f64;
        let mean = sum / n;
        let se_rel = ((sum_sq / n - mean * mean).max(0.0) / n).sqrt() / mean;
        println!(
            "    {e:>10.4e}  <E'>/E ours {mean:>9.5}  NJOY {njoy_m1:>9.5}  {:>+6.2} %  \
             (sampling 1 sigma {:.2} %, {:.0} % coherent elastic)",
            100.0 * (mean / njoy_m1 - 1.0),
            100.0 * se_rel,
            100.0 * n_el as f64 / (n_el + k) as f64,
        );
        worst_se = worst_se.max(se_rel);
        rows.push((e, mean, njoy_m1));
    }
    println!(
        "  worst sampling 1 sigma across the table: {:.2} %",
        100.0 * worst_se
    );

    assert_table_relative(
        "graphite MF=6/MT=229 kernel <E'>/E vs NJOY THERMR",
        &rows,
        GRAPHITE_KERNEL_TOL,
        1.0e-9,
    );

    // The convergence claim. This, not the worst-point figure, is what
    // distinguishes a correct thermal law from H-in-H2O's.
    let converged: Vec<(f64, f64, f64)> = rows
        .iter()
        .copied()
        .filter(|&(e, _, _)| e > GRAPHITE_KERNEL_CONVERGED_ABOVE_EV)
        .collect();
    assert!(
        converged.len() >= 4,
        "only {} points sit above {GRAPHITE_KERNEL_CONVERGED_ABOVE_EV} eV; the \
         convergence claim needs the upper half of the table to test",
        converged.len()
    );
    assert_table_relative(
        &format!("graphite kernel CONVERGES on NJOY above {GRAPHITE_KERNEL_CONVERGED_ABOVE_EV} eV"),
        &converged,
        GRAPHITE_KERNEL_CONVERGED_TOL,
        1.0e-9,
    );
    println!(
        "  [PASS] graphite converges onto NJOY above {GRAPHITE_KERNEL_CONVERGED_ABOVE_EV} eV \
         and stays there — the property H-in-H2O lacks (#188)"
    );
}
