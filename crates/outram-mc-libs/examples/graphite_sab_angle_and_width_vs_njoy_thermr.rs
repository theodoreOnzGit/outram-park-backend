//! Compare **two moments of this crate's graphite S(α,β) kernel that had no
//! oracle at all** — the scattering **angle** and the outgoing-energy **width** —
//! against NJOY2016's own THERMR output.
//!
//! # Why this exists
//!
//! Before this program, every thermal oracle in this workspace was a
//! **first-moment, energy-domain** oracle:
//! `graphite_vs_njoy_thermr` checks σ(E), `graphite_kernel_vs_njoy_thermr`
//! checks ⟨E′⟩/E, `graphite_energy_decrement` checks ξ, `slowing_down_oracle`
//! checks the whole energy treatment against an exact deterministic solve. Not
//! one of them constrains μ. Before this program the only assertion on a thermal
//! cosine anywhere in the crate was `(-1.0..=1.0).contains(&mu)` — "is it a
//! valid cosine".
//!
//! The angle is not cosmetic. It sets σ_tr = σ_s(1 − μ̄), hence the diffusion
//! coefficient, hence the thermal flux *shape* in a heterogeneous cell — which
//! is exactly the axis the `k`-residual hunt had narrowed to (thermal **and**
//! spatially heterogeneous fails; thermal-homogeneous and fast-heterogeneous
//! both pass).
//!
//! The **width** is not cosmetic either, and for an independent reason. A kernel
//! can have exactly the right mean `⟨E′⟩/E` — `graphite_kernel_vs_njoy_thermr`
//! measures 0.1 % above 0.2 eV — and the wrong *spread*, and the spread is what
//! decides how many neutrons cross the 0.625 eV group boundary per collision.
//! In a pebble whose `p` is 0.48, half the neutron economy turns on where that
//! boundary is crossed.
//!
//! # What both measurements found (2026-09-12)
//!
//! - **Angle: excluded.** μ̄ agrees with THERMR to ≤ 0.0085 absolute, on a μ̄ of
//!   ~0.05, i.e. σ_tr within 0.05 %.
//! - **Width: a real defect, and priced.** The sampled kernel is up to **+39 %
//!   too broad at 2 eV**, from the 48-point emission grid plus ACE statistical
//!   table interpolation (adjacent tables are 31.6 % apart in incident energy,
//!   and mixing them adds variance the true kernel has not got). Raising
//!   `N_EMIT_GRID` to 192 collapses it to −2.3 %, and moves the FHR ring-RPT
//!   CSG k-eff by **−63 pcm** against a +4004 pcm residual. See
//!   [`outram_mc_libs::vv::njoy_golden::GRAPHITE_KERNEL_WIDTH`].
//!
//! # The oracle
//!
//! THERMR's MF=6/MT=229 LIST record carries, per incident energy, `NEP` groups
//! of `NA + 2` numbers: `(E′, f(E′), μ₁ … μ_NA)` where the `NA = 16` cosines are
//! **equally probable** in the laboratory frame. `graphite_kernel_vs_njoy_thermr`
//! parses these same records and throws the cosines away. This program keeps
//! them and forms
//!
//! ```text
//!   μ̄_inel(E) = Σ_E′ f(E′) · mean(μ at E′)  /  Σ_E′ f(E′)
//! ```
//!
//! by the same trapezoid used for the energy moment, so the two programs differ
//! only in which column they reduce. The **width** comes off the same records,
//! as `sqrt(⟨E′²⟩ − ⟨E′⟩²)/⟨E′⟩`.
//!
//! The **coherent-elastic** channel needs no MF=6: THERMR writes it as
//! MF=3/MT=230, and `E·σ_coh(E) = Σ_{E_i < E} f_i` is a staircase whose risers
//! *are* the Bragg edges `(E_i, f_i)`. Each edge scatters at exactly
//! `μ_i = 1 − 2E_i/E`, so
//!
//! ```text
//!   μ̄_el(E) = Σ_{E_i<E} f_i (1 − 2E_i/E) / Σ_{E_i<E} f_i
//! ```
//!
//! is recovered from NJOY's own cross section with no model — an oracle for the
//! Bragg angular law that never reads this crate's edge table.
//!
//! Deck as in `graphite_vs_njoy_thermr.rs` (`tape23` carries MT=229 and MT=230):
//!
//! ```text
//! GRAPHITE_THERMR=/home/user/graphiteoracle/tape23 cargo run --release \
//!     -p outram-mc-libs --features endf-pebble-cases \
//!     --example graphite_sab_angle_and_width_vs_njoy_thermr
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
    width_golden_gate(&law_for_gate);

    let Ok(path) = std::env::var("GRAPHITE_THERMR") else {
        eprintln!(
            "\nThe live-tape comparison needs GRAPHITE_THERMR pointing at an NJOY THERMR\n\
             tape (deck in the module docs). The golden gate above already ran, so this\n\
             program is still a V&V case without it \u{2014} it just cannot re-measure the oracle."
        );
        return;
    };
    let text = std::fs::read_to_string(&path).expect("THERMR tape");
    let njoy = parse_mf6_law1_with_cosines(&text, 625, 229);
    eprintln!("read {} incident energies from MF=6/MT=229", njoy.len());
    let edges = bragg_edges_from_mf3(&text, 625, 230);
    eprintln!("recovered {} Bragg edges from MF=3/MT=230", edges.len());
    let sig229 = parse_mf3_xs(&text, 625, 229);
    let sig230 = parse_mf3_xs(&text, 625, 230);

    let tsl =
        njoy_outram_park_fork::reference_data::reference_endf("tsl-crystalline-graphite.endf")
            .expect("graphite tape");
    let g = ThermalScattering::from_endf_file(tsl.to_str().expect("path"), 30, TEMP, "c_Graphite")
        .expect("thermal law");

    println!("\ngraphite S(a,b) MEAN SCATTERING COSINE, {TEMP} K");
    println!("ours: {N} samples of ThermalScattering::sample, split by E'==E");
    println!("NJOY: MF=6/MT=229 equiprobable cosines (inelastic); Bragg staircase (elastic)\n");
    println!(
        "{:>11} {:>10} {:>10} {:>9} {:>8} | {:>10} {:>10} {:>9} | {:>10} {:>10} {:>9}",
        "E [eV]",
        "mu_in_N",
        "mu_in_o",
        "d(abs)",
        "1 sigma",
        "mu_el_N",
        "mu_el_o",
        "d(abs)",
        "mu_tot_N",
        "mu_tot_o",
        "d(abs)"
    );

    let mut seed = 20_260_912_u64;
    let mut worst_inel = 0.0_f64;
    let mut worst_tot = 0.0_f64;
    for &probe in PROBES {
        // The tape's own incident-energy grid is what it is; take its nearest
        // point to each probe rather than interpolating the matrix.
        let Some(&(e, ref dist)) = njoy
            .iter()
            .filter(|(e, _)| (1.0e-3..=4.0).contains(e))
            .min_by(|a, b| {
                ((a.0 / probe).ln().abs())
                    .partial_cmp(&(b.0 / probe).ln().abs())
                    .unwrap()
            })
        else {
            continue;
        };
        let mu_inel_n = mubar_from_matrix(dist);
        let mu_el_n = mubar_bragg(&edges, e);
        let s_in = interp(&sig229, e);
        let s_el = interp(&sig230, e);
        let mu_tot_n = if s_in + s_el > 0.0 {
            (s_in * mu_inel_n + s_el * mu_el_n) / (s_in + s_el)
        } else {
            0.0
        };

        let (mut su_in, mut sq_in, mut n_in) = (0.0, 0.0, 0usize);
        let (mut su_el, mut n_el) = (0.0, 0usize);
        for _ in 0..N {
            let Some((ep, mu)) = g.sample(e, &mut seed) else {
                continue;
            };
            if (ep - e).abs() <= 1.0e-12 * e {
                su_el += mu;
                n_el += 1;
            } else {
                su_in += mu;
                sq_in += mu * mu;
                n_in += 1;
            }
        }
        if n_in == 0 {
            continue;
        }
        let k = n_in as f64;
        let mu_inel_o = su_in / k;
        let sd = ((sq_in / k - mu_inel_o * mu_inel_o).max(0.0) / k).sqrt();
        let mu_el_o = if n_el > 0 {
            su_el / n_el as f64
        } else {
            f64::NAN
        };
        let mu_tot_o = (su_in + su_el) / (n_in + n_el) as f64;
        worst_inel = worst_inel.max((mu_inel_o - mu_inel_n).abs());
        worst_tot = worst_tot.max((mu_tot_o - mu_tot_n).abs());
        println!(
            "{e:>11.4e} {mu_inel_n:>10.5} {mu_inel_o:>10.5} {:>+9.5} {sd:>8.5} | \
             {mu_el_n:>10.5} {mu_el_o:>10.5} {:>+9.5} | \
             {mu_tot_n:>10.5} {mu_tot_o:>10.5} {:>+9.5}",
            mu_inel_o - mu_inel_n,
            mu_el_o - mu_el_n,
            mu_tot_o - mu_tot_n
        );
    }
    // ── the kernel WIDTH, which <E'>/E cannot see ──
    println!("\n  second moment <E'^2>/E^2 and relative width sqrt(var)/<E'>, inelastic only");
    println!(
        "{:>11} {:>12} {:>12} {:>9} | {:>10} {:>10} {:>9}",
        "E [eV]", "m2 NJOY", "m2 ours", "rel", "w NJOY", "w ours", "rel"
    );
    for &probe in PROBES {
        let Some(&(e, ref dist)) = njoy
            .iter()
            .filter(|(e, _)| (1.0e-3..=4.0).contains(e))
            .min_by(|a, b| {
                ((a.0 / probe).ln().abs())
                    .partial_cmp(&(b.0 / probe).ln().abs())
                    .unwrap()
            })
        else {
            continue;
        };
        let (mut norm, mut m1, mut m2) = (0.0, 0.0, 0.0);
        for w in dist.windows(2) {
            let (e0, f0, _) = w[0];
            let (e1, f1, _) = w[1];
            let de = e1 - e0;
            if de <= 0.0 {
                continue;
            }
            norm += 0.5 * (f0 + f1) * de;
            m1 += 0.5 * (f0 * e0 + f1 * e1) * de;
            m2 += 0.5 * (f0 * e0 * e0 + f1 * e1 * e1) * de;
        }
        if norm <= 0.0 {
            continue;
        }
        let (m1_n, m2_n) = (m1 / norm, m2 / norm);
        let w_n = (m2_n - m1_n * m1_n).max(0.0).sqrt() / m1_n;
        let (mut s1, mut s2, mut n) = (0.0, 0.0, 0usize);
        for _ in 0..N {
            let Some((ep, _)) = g.sample(e, &mut seed) else {
                continue;
            };
            if (ep - e).abs() <= 1.0e-12 * e {
                continue;
            }
            s1 += ep;
            s2 += ep * ep;
            n += 1;
        }
        if n == 0 {
            continue;
        }
        let k = n as f64;
        let (m1_o, m2_o) = (s1 / k, s2 / k);
        let w_o = (m2_o - m1_o * m1_o).max(0.0).sqrt() / m1_o;
        println!(
            "{e:>11.4e} {:>12.5e} {:>12.5e} {:>+8.2}% | {w_n:>10.5} {w_o:>10.5} {:>+8.2}%",
            m2_n / (e * e),
            m2_o / (e * e),
            100.0 * (m2_o / m2_n - 1.0),
            100.0 * (w_o / w_n - 1.0)
        );
    }

    println!(
        "\nWorst |d mu| : inelastic {worst_inel:.5}, total {worst_tot:.5} (absolute, not relative).\n\
         mu-bar is ~0.05 here, so a deviation of 0.005 moves sigma_tr = sigma_s(1 - mu-bar)\n\
         by 0.5 % of a factor that is itself 0.95: i.e. the transport cross section moves by\n\
         about 0.05 %. That is the number to weigh against a +4000 pcm residual."
    );
}

/// V&V gate against the committed NJOY2016 oracle \u{2014} runs with no tape on disk.
///
/// # Methodology
///
/// 400 000 samples of `ThermalScattering::sample` per incident energy at the
/// thirteen energies of [`outram_mc_libs::vv::njoy_golden::GRAPHITE_MUBAR`],
/// split into the two thermal channels by the one unambiguous signature
/// (coherent elastic is the only channel that leaves `E\u{2032}` exactly equal to `E`),
/// and reduced to the mean cosine of each. `tsl-crystalline-graphite` (MAT 30)
/// at **600 K \u{2014} a tabulated temperature on that tape**, so no interpolation is
/// involved. The oracle side is NJOY2016 THERMR: MF=6/MT=229's equiprobable
/// laboratory cosines for the inelastic channel, and the Bragg staircase
/// recovered from MF=3/MT=230 for the elastic one. Neither involves sampling.
///
/// # Results (2026-09-12, NJOY2016 2016.79, ENDF/B-VIII.0)
///
/// Worst **+0.0085 absolute on \u{3bc}\u{304}_inelastic** at 0.0253 eV (1\u{3c3} of the sampling
/// there is 0.0016, so this is a real 5\u{3c3} deviation, not noise) and **+0.0030 on
/// \u{3bc}\u{304}_elastic** at 0.2 eV. The full table, with the per-row standard errors, is
/// in the golden table's doc comment.
///
/// # What this rules out
///
/// \u{3bc}\u{304}_total is \u{2248} 0.05 over the whole thermal range, so an error of 0.005 moves
/// \u{3c3}_tr = \u{3c3}_s(1 \u{2212} \u{3bc}\u{304}) by **0.05 %**. The FHR ring-RPT residual is +4004 pcm.
/// The thermal scattering angle is therefore **excluded** as its cause \u{2014} which
/// is the point of running it: it was the leading hypothesis precisely because
/// it was the one moment of the thermal kernel with no oracle on it.
fn golden_gate(law: &ThermalScattering) {
    use outram_mc_libs::vv::njoy_golden::{
        GRAPHITE_MUBAR, GRAPHITE_MUBAR_ELASTIC_TOL, GRAPHITE_MUBAR_TOL,
    };
    use outram_mc_libs::vv::assert_absolute;

    let mut seed = 20_260_912_u64;
    println!("graphite S(a,b) mean cosine vs the committed NJOY THERMR oracle, {TEMP} K");
    for &(e, mu_in_n, mu_el_n, _mu_tot_n) in GRAPHITE_MUBAR {
        let (mut su_in, mut n_in, mut su_el, mut n_el) = (0.0, 0usize, 0.0, 0usize);
        for _ in 0..N {
            let Some((ep, mu)) = law.sample(e, &mut seed) else {
                continue;
            };
            if (ep - e).abs() <= 1.0e-12 * e {
                su_el += mu;
                n_el += 1;
            } else {
                su_in += mu;
                n_in += 1;
            }
        }
        assert!(n_in > N / 100, "no inelastic scatters at {e} eV");
        assert_absolute(
            &format!("graphite mubar inelastic @ {e:.4e} eV"),
            su_in / n_in as f64,
            mu_in_n,
            GRAPHITE_MUBAR_TOL,
        );
        if n_el > N / 100 {
            assert_absolute(
                &format!("graphite mubar coherent-elastic @ {e:.4e} eV"),
                su_el / n_el as f64,
                mu_el_n,
                GRAPHITE_MUBAR_ELASTIC_TOL,
            );
        }
    }
    println!(
        "  all {} rows inside {GRAPHITE_MUBAR_TOL} (inelastic) / \
         {GRAPHITE_MUBAR_ELASTIC_TOL} (elastic) absolute on mu-bar",
        GRAPHITE_MUBAR.len()
    );
}

/// V&V gate on the kernel **width** against the committed NJOY2016 oracle —
/// runs with no tape on disk.
///
/// # Methodology
///
/// The same 400 000 samples per incident energy as [`golden_gate`], reduced
/// instead to the relative width `sqrt(var(E'))/<E'>` of the *inelastic*
/// outgoing-energy distribution (coherent elastic removed by the `E' == E`
/// signature, since it would contribute a spurious zero-width spike). The
/// oracle side is a quadrature on THERMR's MF=6/MT=229 matrix — no sampling —
/// recorded in [`outram_mc_libs::vv::njoy_golden::GRAPHITE_KERNEL_WIDTH`].
///
/// # Results (2026-09-12, NJOY2016 2016.79, ENDF/B-VIII.0)
///
/// **This gate records an open defect, deliberately.** The width is −2.6 % to
/// −11.6 % *narrow* below 0.2 eV and up to **+39.0 % broad at 2 eV**, with the
/// sign flipping at 0.39 eV. The broad half is the 48-point emission grid plus
/// ACE statistical table interpolation; the narrow half is the 16-bin
/// equiprobable representation. Both halves, their cause, and the measured
/// k-worth (**−63 pcm** on the FHR ring-RPT CSG pebble) are in the golden
/// table's doc comment.
///
/// Two envelopes, because the two halves of the table fail for different
/// reasons and one envelope would hide that: 50 % across the table (the
/// characterisation bound on the known defect) and 15 % one-signed below
/// 0.2 eV. **Tighten the first to ~15 % when the emission grid is refined;
/// never widen either.**
fn width_golden_gate(law: &ThermalScattering) {
    use outram_mc_libs::vv::njoy_golden::{
        GRAPHITE_KERNEL_WIDTH, GRAPHITE_KERNEL_WIDTH_INTRINSIC_BELOW_EV,
        GRAPHITE_KERNEL_WIDTH_NARROW_TOL, GRAPHITE_KERNEL_WIDTH_TOL,
    };

    let mut seed = 20_260_912_u64;
    println!("\ngraphite S(a,b) kernel WIDTH vs the committed NJOY THERMR oracle, {TEMP} K");
    let (mut worst, mut worst_e) = (0.0_f64, 0.0_f64);
    for &(e, w_njoy) in GRAPHITE_KERNEL_WIDTH {
        let w = sampled_width(law, e, N, &mut seed);
        let rel = w / w_njoy - 1.0;
        println!(
            "  {e:>9.4e}  NJOY {w_njoy:>8.5}  ours {w:>8.5}  {:>+7.2} %",
            100.0 * rel
        );
        if rel.abs() > worst.abs() {
            worst = rel;
            worst_e = e;
        }
        if e < GRAPHITE_KERNEL_WIDTH_INTRINSIC_BELOW_EV {
            assert!(
                rel < 0.0 && rel.abs() < GRAPHITE_KERNEL_WIDTH_NARROW_TOL,
                "graphite's kernel width is {:+.2} % from NJOY at {e:.4e} eV. Below \
                 {GRAPHITE_KERNEL_WIDTH_INTRINSIC_BELOW_EV} eV it is the equiprobable \
                 representation's own narrowing that is being measured, and it was \
                 one-signed and inside {GRAPHITE_KERNEL_WIDTH_NARROW_TOL} on 2026-09-12",
                100.0 * rel
            );
        }
    }
    assert!(
        worst.abs() < GRAPHITE_KERNEL_WIDTH_TOL,
        "graphite's kernel width is {:+.2} % from NJOY at {worst_e:.4e} eV — worse than \
         the +39.0 % recorded on 2026-09-12. This is a CHARACTERISATION bound on a known \
         defect: do not widen it",
        100.0 * worst
    );
    println!("  worst {:+.2} % at {worst_e:.4e} eV", 100.0 * worst);
}

/// `sqrt(var(E'))/<E'>` of the inelastic channel from `n` samples at `e`.
fn sampled_width(law: &ThermalScattering, e: f64, n: usize, seed: &mut u64) -> f64 {
    let (mut s1, mut s2, mut k) = (0.0, 0.0, 0usize);
    for _ in 0..n {
        let Some((ep, _mu)) = law.sample(e, seed) else {
            continue;
        };
        if (ep - e).abs() <= 1.0e-12 * e {
            continue; // coherent elastic: a zero-width spike, not the kernel
        }
        s1 += ep;
        s2 += ep * ep;
        k += 1;
    }
    assert!(k > n / 100, "no inelastic scatters at {e} eV");
    let (m1, m2) = (s1 / k as f64, s2 / k as f64);
    (m2 - m1 * m1).max(0.0).sqrt() / m1
}

const PROBES: &[f64] = &[
    1.0e-3, 2.53e-3, 5.0e-3, 1.0e-2, 2.53e-2, 5.0e-2, 0.1, 0.2, 0.4, 0.625, 1.0, 2.0, 3.75,
];

/// μ̄ over one incident energy's outgoing distribution: the f-weighted trapezoid
/// of the per-bin equiprobable-cosine mean.
fn mubar_from_matrix(rows: &[(f64, f64, f64)]) -> f64 {
    let (mut norm, mut num) = (0.0, 0.0);
    for w in rows.windows(2) {
        let (e0, f0, m0) = w[0];
        let (e1, f1, m1) = w[1];
        let de = e1 - e0;
        if de <= 0.0 {
            continue;
        }
        norm += 0.5 * (f0 + f1) * de;
        num += 0.5 * (f0 * m0 + f1 * m1) * de;
    }
    if norm <= 0.0 {
        return 0.0;
    }
    num / norm
}

/// μ̄ of coherent-elastic (Bragg) scattering at `e`, from the edge table.
fn mubar_bragg(edges: &[(f64, f64)], e: f64) -> f64 {
    let (mut w, mut num) = (0.0, 0.0);
    for &(ei, fi) in edges {
        if ei < e {
            w += fi;
            num += fi * (1.0 - 2.0 * ei / e);
        }
    }
    if w <= 0.0 {
        return 0.0;
    }
    num / w
}

/// Recover the Bragg edge table `(E_i, f_i)` from THERMR's own MF=3/MT=230:
/// `E·σ_coh(E)` is a staircase and its risers are the edges.
fn bragg_edges_from_mf3(text: &str, mat: i32, mt: i32) -> Vec<(f64, f64)> {
    let xs = parse_mf3_xs(text, mat, mt);
    let mut out: Vec<(f64, f64)> = Vec::new();
    let mut prev = 0.0f64;
    for &(e, s) in &xs {
        let cum = e * s;
        if cum > prev * (1.0 + 1.0e-9) + 1.0e-12 {
            out.push((e, cum - prev));
            prev = cum;
        }
    }
    out
}

/// `(E [eV], σ [b])` pairs of an MF=3 section.
fn parse_mf3_xs(text: &str, mat: i32, mt: i32) -> Vec<(f64, f64)> {
    let lines: Vec<&str> = section_lines(text, mat, 3, mt);
    if lines.len() < 3 {
        return Vec::new();
    }
    // HEAD, then a TAB1: CONT with NR/NP, NR interpolation pairs, then NP (x,y).
    let np = ifield(lines[1], 5) as usize;
    let nr = ifield(lines[1], 4) as usize;
    let skip = 2 + (nr * 2).div_ceil(6);
    let mut vals: Vec<f64> = Vec::with_capacity(2 * np);
    for l in lines.iter().skip(skip) {
        for c in 0..6 {
            if vals.len() == 2 * np {
                break;
            }
            let s = &l[c * 11..(c + 1) * 11];
            if s.trim().is_empty() {
                break;
            }
            vals.push(endf_f(s));
        }
        if vals.len() == 2 * np {
            break;
        }
    }
    vals.chunks_exact(2).map(|c| (c[0], c[1])).collect()
}

fn interp(xs: &[(f64, f64)], x: f64) -> f64 {
    if xs.is_empty() {
        return 0.0;
    }
    if x <= xs[0].0 {
        return xs[0].1;
    }
    if x >= xs[xs.len() - 1].0 {
        return xs[xs.len() - 1].1;
    }
    let mut i = 0;
    while i + 1 < xs.len() && xs[i + 1].0 < x {
        i += 1;
    }
    let ((x0, y0), (x1, y1)) = (xs[i], xs[i + 1]);
    if x1 > x0 {
        y0 + (y1 - y0) * (x - x0) / (x1 - x0)
    } else {
        y0
    }
}

fn section_lines<'a>(text: &'a str, mat: i32, mf: i32, mt: i32) -> Vec<&'a str> {
    text.lines()
        .filter(|l| {
            l.len() >= 75
                && l[66..70].trim().parse::<i32>() == Ok(mat)
                && l[70..72].trim().parse::<i32>() == Ok(mf)
                && l[72..75].trim().parse::<i32>() == Ok(mt)
        })
        .collect()
}

fn field(l: &str, i: usize) -> f64 {
    endf_f(&l[i * 11..(i + 1) * 11])
}
fn ifield(l: &str, i: usize) -> i64 {
    l[i * 11..(i + 1) * 11].trim().parse().unwrap_or(0)
}

/// Same walk as `graphite_kernel_vs_njoy_thermr::parse_mf6_law1`, but each row
/// keeps the mean of that bin's equiprobable cosines: `(E′, f, μ̄(E′))`.
fn parse_mf6_law1_with_cosines(text: &str, mat: i32, mt: i32) -> Vec<(f64, Vec<(f64, f64, f64)>)> {
    let lines = section_lines(text, mat, 6, mt);
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
            let dist: Vec<(f64, f64, f64)> = vals
                .chunks_exact(stride)
                .map(|c| {
                    let n = (stride - 2) as f64;
                    let m = if n > 0.0 {
                        c[2..].iter().sum::<f64>() / n
                    } else {
                        0.0
                    };
                    (c[0], c[1], m)
                })
                .collect();
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
