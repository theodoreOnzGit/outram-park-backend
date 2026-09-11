//! Compare this crate's **H-in-H₂O** S(α,β) *kernel* — the outgoing-energy
//! distribution the transport actually samples — against **NJOY2016's THERMR
//! MF=6 scattering matrix**.
//!
//! # Why this exists
//!
//! `h2o_vs_njoy_thermr.rs` compares the *cross section* and finds a consistent
//! **+1 %**, against graphite's ±0.05 %. That is a real discrepancy but it has
//! the wrong sign to explain the deficit being chased, and it is too small.
//!
//! The cross section is only half the law. The other half is the **kernel**:
//! where a neutron goes in energy when it scatters off bound hydrogen. Graphite's
//! kernel was checked against this same MF=6 matrix (≤ 0.5 % on ⟨E′⟩/E over
//! 0.1–4 eV). **Water's never was** — and water's law sits under both systems
//! this code disagrees with, while the one thermal benchmark it reproduces,
//! HEU-SOL-THERM-009, is *homogeneous*, where the spatial thermal flux
//! distribution this law controls does not matter.
//!
//! The measurement that pointed here: LEU-COMP-THERM-008's independently
//! critical cases give `Δk` of **+2950 / +2271 / +1713 pcm** at 1511 / 1335.5 /
//! 794 ppm soluble boron on one pin cell — a 14σ spread that no resonance-escape
//! error can produce, tracking the thermal poison. B-10's cross section is
//! correct to 0.3 %, so what is wrong is the thermal flux where the absorber
//! sits, and this law sets it.
//!
//! # Generating the oracle
//!
//! Same THERMR run as `h2o_vs_njoy_thermr.rs` — see its module docs. `tape23`
//! carries MF=6/MT=222.
//!
//! ```text
//! H2O_THERMR=/home/user/h2ooracle/tape23 cargo run --release \
//!     -p outram-mc-libs --features endf-pebble-cases --example h2o_kernel_vs_njoy_thermr
//! ```

//! # Results (2026-09-11, ENDF/B-VIII.0 `tsl-HinH2O`, 293.6 K, 400 000 samples)
//!
//! **The kernel is wrong, systematically, and in one direction.**
//!
//! | E \[eV\] | ⟨E′⟩/E NJOY | ours | rel | ξ NJOY | ξ ours |
//! |---|---|---|---|---|---|
//! | 0.00101 | 11.1954 | 10.6249 | **−5.10 %** | −1.0992 | −1.0891 |
//! | 0.0015 | 7.3050 | 6.9000 | **−5.54 %** | −0.8934 | −0.8843 |
//! | 0.005 | 2.5102 | 2.4144 | −3.82 % | −0.4261 | −0.4266 |
//! | 0.01 | 1.6664 | 1.6183 | −2.89 % | −0.2356 | −0.2378 |
//! | 0.0253 | 1.1939 | 1.1763 | −1.47 % | −0.0553 | −0.0556 |
//! | 0.05 | 1.0104 | 1.0023 | −0.81 % | 0.0870 | 0.0753 |
//! | 0.1116 | 0.8002 | 0.8002 | **0.00 %** | 0.3669 | 0.3483 |
//! | 0.2 | 0.7138 | 0.7181 | +0.60 % | 0.4897 | 0.4642 |
//! | 0.625 | 0.6051 | 0.6129 | +1.29 % | 0.7105 | 0.6789 |
//! | 1.86 | 0.5393 | 0.5472 | +1.48 % | 0.8697 | 0.8412 |
//!
//! Below the crossover at ~0.11 eV the neutron should **gain** energy and ours
//! gains too little; above it the neutron should **lose** energy and ours loses
//! too little. **Both halves err the same way: this kernel moves neutrons less
//! than it should**, i.e. its outgoing-energy distribution is too narrow, sitting
//! too close to the incident energy. Consistently, `ξ = ⟨ln(E/E′)⟩` runs **3–5 %
//! low** across the whole range above 0.05 eV: this code's water moderates about
//! 4 % less per collision than NJOY's.
//!
//! Graphite, on the identical check, agrees to **≤ 0.5 %**. So this is not the
//! method and not the comparison — it is water.
//!
//! # Where it lives, and why it was not caught
//!
//! The kernel comes from
//! `njoy_outram_park_fork::thermr::scattering::IncoherentInelasticScattering`,
//! this workspace's port of NJOY2016's `thermr.f90` (`sig`/`calcem`
//! double-differential plus the short-collision-time tail). That port **does**
//! have an H-in-H₂O test — `crates/njoy-outram-park-fork/tests/thermal_h2o_sab.rs`
//! — and it passes. Its own module docs say why it cannot see this:
//!
//! > *"The four checks are analytic/limiting — no ACE oracle is required, so they
//! > are reproducible on any machine that has the (open-source) ENDF file."*
//!
//! Those four are the free-atom limit at high energy, the thermal cross section
//! against bound water, detailed balance of the double-differential, and a
//! physical effective temperature. **Every one is satisfied where the binding
//! physics is switched off, or tests only a symmetry**: the free-atom limit is
//! the *absence* of binding; detailed balance is a symmetry that a
//! uniformly-too-narrow kernel still obeys exactly; the cross section is the
//! law's *magnitude*, not its *shape*. Reproducibility was traded for an external
//! oracle, and a 2–5.5 % shape error walked through the gap.
//!
//! That is the same lesson as GitHub #178, one module over: **a limiting check
//! passes exactly where the interesting physics is turned off.**

use outram_mc_libs::material::thermal::ThermalScattering;

const TEMP: f64 = 293.6;
const N: usize = 400_000;

fn main() {
    let tsl_for_gate = njoy_outram_park_fork::reference_data::reference_endf("tsl-HinH2O.endf")
        .expect("H(H2O) tape");
    let law_for_gate = ThermalScattering::from_endf_file(
        tsl_for_gate.to_str().expect("path"),
        1,
        TEMP,
        "c_H_in_H2O",
    )
    .expect("thermal law");
    golden_gate(&law_for_gate);

    let Ok(path) = std::env::var("H2O_THERMR") else {
        eprintln!(
            "\nThe live-tape comparison needs H2O_THERMR pointing at an NJOY THERMR tape\n\
             (deck in the module docs). The golden gate above already ran, so this program\n\
             is still a V&V case without it — it just cannot re-measure the oracle."
        );
        return;
    };
    let text = std::fs::read_to_string(&path).expect("THERMR tape");
    let njoy = parse_mf6_law1(&text, 125, 222);
    eprintln!(
        "read {} incident energies from MF=6/MT=222 of {path}",
        njoy.len()
    );

    let tsl = njoy_outram_park_fork::reference_data::reference_endf("tsl-HinH2O.endf")
        .expect("H(H2O) tape");
    let g = ThermalScattering::from_endf_file(tsl.to_str().expect("path"), 1, TEMP, "c_H_in_H2O")
        .expect("thermal law");

    println!("\nH-in-H2O S(a,b) INCOHERENT INELASTIC outgoing energy, {TEMP} K");
    println!("ours: {N} samples of ThermalScattering::sample (water has no coherent elastic)");
    println!("NJOY: quadrature on the THERMR MF=6/MT=222 matrix, no sampling\n");
    println!(
        "{:>11} {:>12} {:>12} {:>9} {:>12} {:>12} {:>9} {:>8}",
        "E [eV]", "<E'>/E NJOY", "<E'>/E ours", "rel", "xi NJOY", "xi ours", "rel", "elastic%"
    );

    let mut seed = 20_260_911_u64;
    let mut worst = 0.0_f64;
    for &(e, ref dist) in &njoy {
        if !(0.001..=2.0).contains(&e) {
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
        "\nWorst |Δ⟨E′⟩/E| = {:.2} %. Water's law sits under BOTH systems this code\n\
         disagrees with, while the one thermal benchmark it reproduces\n\
         (HEU-SOL-THERM-009) is homogeneous — where the spatial thermal flux\n\
         distribution this law controls does not matter.\n\n\
         CORRECTION. This program used to close by saying graphite passes the same\n\
         check at <= 0.5 % and water never did. That figure was scoped to 0.1-4 eV.\n\
         Over water's own range graphite is -1.53 % at 0.0253 eV against water's\n\
         -1.47 % — the same, not better. What actually separates them is the TREND:\n\
         graphite converges to <= 0.1 % above 0.2 eV and stays there; water's\n\
         deviation instead grows monotonically to +1.5 %. See\n\
         outram_mc_libs::vv::njoy_golden::GRAPHITE_KERNEL.\n\n\
         Read ⟨E′⟩/E, not ξ. ξ = ⟨ln(E/E′)⟩ passes through **zero** where net\n\
         up-scatter turns into net down-scatter, so a *relative* difference on it\n\
         blows up there for arithmetic reasons and means nothing. ⟨E′⟩/E has no\n\
         zero anywhere in this range.",
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
/// 200 000 samples of `ThermalScattering::sample` per incident energy, reduced
/// to the first moment `⟨E′⟩/E`, at eleven energies from 1.5 meV to 1.855 eV on
/// `tsl-HinH2O` (MAT 1) at 293.6 K. The oracle side is a *quadrature* on NJOY's
/// THERMR MF=6/MT=222 matrix — no sampling there, so the two sides are not
/// sharing a sampler and cannot agree by construction.
/// Oracle values and provenance: [`outram_mc_libs::vv::njoy_golden::H2O_KERNEL`].
///
/// **Sampling uncertainty is measured, not assumed.** `⟨E′⟩/E` is the mean of a
/// *wide* distribution wherever up-scatter dominates, so the sub-0.1 % figure a
/// binomial estimate would give is wrong at the bottom of the range. The gate
/// computes and prints the standard error of every row, and asserts that each
/// deviation is larger than its own noise — so a finding can never be an
/// artefact of too few samples.
///
/// # Results (2026-09-11, NJOY2016 2016.79, ENDF/B-VIII.0)
///
/// **This crate's H-in-H₂O kernel is too narrow.** `⟨E′⟩/E` runs from −5.5 % at
/// 1.5 meV, through an exact crossing at 0.1116 eV, to +1.5 % at 1.855 eV, with
/// `ξ` 3–5 % low throughout. In plain terms, **water moderates about 4 % less
/// per collision here than in NJOY2016**. This is the open defect GitHub #188.
///
/// Measured by this gate at 400 000 samples, with its own standard error:
///
/// ```text
///    E [eV]     <E'>/E ours   NJOY       rel       sampling 1 sigma
///    0.0015        6.93543   7.30495   −5.06 %       0.28 %
///    0.005         2.40857   2.51021   −4.05 %       0.21 %
///    0.01          1.61897   1.66643   −2.85 %       0.15 %
///    0.0253        1.17642   1.19386   −1.46 %       0.08 %
///    0.05          1.00304   1.01044   −0.73 %       0.06 %
///    0.11157       0.79970   0.80017   −0.06 %       0.06 %
///    0.2           0.71711   0.71383   +0.46 %       0.07 %
///    0.41704       0.64737   0.64228   +0.79 %       0.07 %
///    0.625         0.61312   0.60511   +1.32 %       0.08 %
///    1.05          0.57303   0.56593   +1.26 %       0.08 %
///    1.855         0.54694   0.53926   +1.42 %       0.09 %
/// ```
///
/// The −5.06 % at 1.5 meV here against the −5.54 % in the golden table is ~1 σ
/// of the combined sampling noise of the two runs (different seed, different
/// sample count); the tables agree.
///
/// # What is asserted
///
/// The magnitude envelope is **6 %**, sized to the −5.54 % it documents: it
/// catches the defect getting worse. **Tighten it when #188 is fixed; never
/// widen it.**
///
/// The **sign structure** is asserted separately and matters more than the
/// magnitude. Below the crossover this crate must under-*gain* energy and above
/// it under-*lose* it. That signature is what identifies a *width* error rather
/// than a scale error — a kernel uniformly 5 % low would produce one sign
/// everywhere and needs a different diagnosis.
///
/// # Why this defect survived an entire analytic test file
///
/// `tests/thermal_h2o_sab.rs` checks the free-atom limit, detailed balance, the
/// cross section and the effective temperature. A too-narrow kernel satisfies
/// detailed balance **exactly** (it is a symmetry, not a width), reproduces the
/// free-atom limit (that tests the absence of binding), has nearly the right
/// area (+1 %, see `examples/h2o_vs_njoy_thermr.rs`) and the right effective
/// temperature (a scalar). Only a direct comparison of the outgoing-energy
/// distribution sees it — which is what this gate is.
fn golden_gate(law: &ThermalScattering) {
    use outram_mc_libs::vv::assert_table_relative;
    use outram_mc_libs::vv::njoy_golden::{H2O_KERNEL, H2O_KERNEL_CROSSOVER_EV, H2O_KERNEL_TOL};

    println!("=== V&V gate: H-in-H2O S(alpha,beta) kernel vs committed NJOY2016 oracle ===");

    let mut seed = 20_260_911_u64;
    let mut rows: Vec<(f64, f64, f64)> = Vec::new();
    let mut worst_se = 0.0_f64;
    for &(e, njoy_m1, _njoy_xi) in H2O_KERNEL {
        let (mut sum, mut sum_sq, mut k) = (0.0_f64, 0.0_f64, 0usize);
        for _ in 0..N {
            let Some((ep, _mu)) = law.sample(e, &mut seed) else {
                continue;
            };
            // Coherent elastic is the only channel leaving E' exactly equal to
            // E; water has none, but the guard also drops degenerate samples.
            if (ep - e).abs() <= 1.0e-12 * e || ep <= 0.0 {
                continue;
            }
            let r = ep / e;
            sum += r;
            sum_sq += r * r;
            k += 1;
        }
        assert!(
            k > N / 100,
            "the H(H2O) law produced almost no inelastic scatters at {e} eV \
             ({k} of {N} samples). The law's range or its sampler has changed."
        );
        let n = k as f64;
        let mean = sum / n;
        // Standard error of the mean of E'/E. At the most up-scattering-dominated
        // energies the spread of E'/E is wide, so this is NOT the sub-0.1 % figure
        // a binomial estimate would suggest -- it is printed rather than assumed.
        let se_rel = ((sum_sq / n - mean * mean).max(0.0) / n).sqrt() / mean;
        println!(
            "    {e:>10.4e}  <E'>/E ours {mean:>9.5}  NJOY {njoy_m1:>9.5}  {:>+6.2} %  \
             (sampling 1 sigma {:.2} %)",
            100.0 * (mean / njoy_m1 - 1.0),
            100.0 * se_rel,
        );
        worst_se = worst_se.max(se_rel);
        rows.push((e, mean, njoy_m1));
    }
    println!(
        "  worst sampling 1 sigma across the table: {:.2} %",
        100.0 * worst_se
    );

    assert_table_relative(
        "H(H2O) MF=6/MT=222 kernel <E'>/E vs NJOY THERMR",
        &rows,
        H2O_KERNEL_TOL,
        1.0e-9,
    );

    // The sign structure IS the finding. A kernel uniformly low would pass the
    // envelope above and mean something entirely different.
    for &(e, ours, njoy) in &rows {
        let rel = ours / njoy - 1.0;
        if e < 0.9 * H2O_KERNEL_CROSSOVER_EV {
            assert!(
                rel < 0.005,
                "below the {H2O_KERNEL_CROSSOVER_EV} eV crossover this crate's kernel \
                 should under-GAIN energy (negative rel); at {e} eV it is {:+.2} %. \
                 The recorded defect (#188) is a too-NARROW kernel; one sign everywhere \
                 would be a scale error instead, and needs a different diagnosis.",
                rel * 100.0
            );
        } else if e > 1.3 * H2O_KERNEL_CROSSOVER_EV {
            assert!(
                rel > -0.005,
                "above the {H2O_KERNEL_CROSSOVER_EV} eV crossover this crate's kernel \
                 should under-LOSE energy (positive rel); at {e} eV it is {:+.2} %.",
                rel * 100.0
            );
        }
    }
    println!(
        "  [PASS] the sign structure is intact: under-gain below {H2O_KERNEL_CROSSOVER_EV} eV, \
         under-loss above it (a too-narrow kernel, not a scale error)"
    );

    // The finding must be larger than the noise that produced it. Without this,
    // a gate that quietly lost sample count would keep reporting the defect from
    // pure scatter -- or stop reporting it -- and nothing would say so.
    let (at, ours, njoy) = rows[0];
    let rel = (ours / njoy - 1.0).abs();
    assert!(
        rel > 5.0 * worst_se,
        "the kernel deviation at {at} eV is {:.2} % against a sampling 1 sigma of \
         {:.2} % -- under 5 sigma, so this run cannot distinguish the recorded \
         defect (#188) from scatter. Either the sample count has dropped or the \
         defect has been fixed; find out which before touching this gate.",
        rel * 100.0,
        worst_se * 100.0,
    );
    println!(
        "  [PASS] the finding exceeds its own noise: {:.2} % deviation against \
         {:.2} % sampling 1 sigma ({:.0} sigma)",
        rel * 100.0,
        worst_se * 100.0,
        rel / worst_se,
    );
}
