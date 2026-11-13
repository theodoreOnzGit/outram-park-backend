//! **The data of two Fission products chapter 2 lessons** (gh:#1207,
//! maintainer direction 2026-11-04: the TRISO-ATOPS mdBook's rungs 2 and 3
//! folded into the chapter): "An atom's clock: sampling a decay"
//! (`decay-clock.html`) and "Walking on spheres" (`walk-on-spheres.html`).
//! Writes `decaywalk-data.js` in `docs/site/science/fission-products/`.
//!
//! ```bash
//! cargo run --release -p dhoby-ghaut --example lesson_decay_walk [out_dir]
//! ```
//!
//! `decay.rs` calls boon-lay's decay Monte Carlo and its CRAM48 expected
//! inventory; `walk.rs` calls its Gaussian step, its Walk-on-Spheres engine,
//! the assembled CRP-6 cell and the CRP-6 Case 1 check. The demo rungs
//! `decay` and `walk` (`examples/triso_atops_web/engine.rs`) make the same
//! calls live; this generator records them, with larger populations, for
//! the lesson pages.
//!
//! **Predictions, written 2026-11-04 before the first run:**
//!
//! Decay (20 000 atoms per parent, `SingleNuclideSimulatorMC` from one
//! `DecayLibrary`, its generator seeded 77; parents Kr-88, I-131, I-135,
//! Cs-137, Bi-212; 101 times from 0 to 5 parent half-lives):
//!
//! - D1. The library's half-lives are the handbook ones within 1 %: Kr-88
//!   2.84 h, I-131 8.02 d, I-135 6.58 h, Cs-137 30.1 y, Bi-212 60.6 min.
//! - D2. Each parent's branching ratios sum to 1 within 1e-6, so gh:#538's
//!   "ratios below 1" panic is not reachable from these five.
//! - D3. The surviving-parent fraction (each atom's sampled first lifetime)
//!   is within 4 sigma of 2^(-t/T½) at every point where sigma is a fair
//!   yardstick (at least 10 expected atoms each side); worst about 2-3 sigma.
//! - D4. Stepping the same 20 000 simulators with `advance_timestep` (100
//!   steps of 0.05 T½, so a short-lived daughter can decay within the step
//!   it is born in) and counting with `count_nuclides_in_sims_linear` gives
//!   every species within 4 sigma of CRAM48 at every fair point, and the
//!   stepped parent count equals D3's count to within 1 atom.
//! - D5. Bi-212's first branch goes to Tl-208 with the library's alpha ratio
//!   (expected near 0.3594) within 3 sigma (sigma about 0.0034); Cs-137's to
//!   Ba-137m with its ratio (near 0.946) within 3 sigma.
//! - D6. The 300 atoms drawn on the page: surviving fraction at 1 T½ within
//!   3 sigma (0.029) of 0.5 for every parent.
//!
//! Walk (Cs-137, the CRP-6 particle of `TrisoCell::new_crp6_geometry`):
//!
//! - W1. boon-lay's Cs diffusion coefficient in the buffer at 1200 °C is
//!   within a factor 10 of the 1e-8 m²/s that
//!   `crates/boon-lay/docs/buffer_clt_failure_analysis.md` uses.
//! - W2. One Gaussian step from 20 000 uniform buffer births (Δt chosen so
//!   the per-component sigma is 14, 45, 141 and 447 µm): the fraction landing
//!   in the SiC or beyond, i.e. having jumped the whole 40 µm IPyC, is
//!   < 1 %, 1-10 %, 15-40 % and > 50 %.
//! - W3. The mean of 200 000 sampled dimensionless hop times is 1/6 within
//!   3 standard errors.
//! - W4. A kernel walker takes 10-40 hops on average before it is captured
//!   within 1e-3 R (2000 walkers), and every recorded hop sphere lies inside
//!   the kernel (|centre| + radius <= R to 1e-9 relative).
//! - W5. The D the assembled cell gives a kernel walker equals
//!   `kernel_diffusion_coefficient`'s bit for bit.
//! - W6. `mc_kernel_release_fraction` at 200 h, 40 000 histories, seed
//!   0x0C1A5EED, gives the 2026-10-04 re-measurement exactly: 0.5331 at
//!   1200 °C (Case 1a), 1.0000 at 1600 °C (1b); Crank 0.5337 and 1.0000.
//! - W7. One set of 10 000 walker release times gives a release curve
//!   within 4 sigma of Crank at every fair point, at 1200 °C (0-200 h) and
//!   1600 °C (0-8 h).
//! - W8. A fixed-step Gaussian walk out of the same kernel, checked only at
//!   the end of each step, under-releases at 200 h (1200 °C). Derived
//!   beforehand: a discrete walk overshoots the boundary by about 0.583
//!   sigma (Siegmund's constant), as if the sink were that much further out;
//!   the atoms of that thin layer are missing from the release, a deficit of
//!   about 3 x 0.583 sigma / R. So sigma = R/10, R/30, R/100 under-release
//!   by 0.12-0.22, 0.04-0.08 and 0.01-0.03 (10 000 atoms).
//!
//! **Results, first run 2026-11-04 (predictions above unedited; output
//! byte-identical on a re-run; CRAM48 inventories and hop clocks printed
//! to ten figures after `check-decaywalk.js` caught six-figure ties, md5
//! `7ff1c62d…`):**
//!
//! - D1 held: Kr-88 1.0224e4 s (2.84 h), I-131 6.9338e5 s (8.025 d), I-135
//!   2.3652e4 s (6.57 h), Cs-137 9.4925e8 s (30.08 y), Bi-212 3633 s
//!   (60.55 min).
//! - D2 held: every branch sum is 1.000000000.
//! - D3 held: worst 1.12, 1.46, 1.24, 2.72 and 2.49 sigma (Kr-88, I-131,
//!   I-135, Cs-137, Bi-212).
//! - D4 held: worst species 3.22 sigma (Bi-212 → Tl-208); the stepped parent
//!   count equals the clock count to 0 atoms for all five. Short-lived
//!   daughters (Ba-137m, Po-212) have no fair points, as expected.
//! - D5 held: Bi-212 → Tl-208 0.3587 against 0.3594 (0.22 sigma); Cs-137 →
//!   Ba-137m 0.9480 against 0.9470 (0.63 sigma).
//! - D6 held: 0.493, 0.533, 0.517, 0.453, 0.517.
//! - W1 held, but trivially: D = 1.0000e-8 m²/s exactly, because boon-lay's
//!   buffer D is the same constant 1e-8 the doc uses
//!   (`diffusion_coeffs/mod.rs`), not an independent value.
//! - W2: three of four bands held; **one MISSED**. SiC or beyond: 0.0002
//!   (< 1 %), 0.070 (1–10 %), **0.460 against 15–40 %**, 0.907 (> 50 %).
//! - W3 held: 0.166747 ± 0.000235 (0.34 se from 1/6).
//! - W4 held: 16.34 hops on average (min 0, max 73); worst (|c| + r)/R − 1 is
//!   2.2e-16.
//! - W5 held: both 2.251932151988885e-15 m²/s, bit for bit.
//! - W6 held exactly: 0.5331 (Crank 0.5337) at 1200 °C, 1.0000 (1.0000) at
//!   1600 °C.
//! - W7 held: worst 0.82 sigma (1200 °C) and 0.78 sigma (1600 °C) over 50
//!   points.
//! - W8: the sign held and the deficit shrinks with sigma as predicted, but
//!   **R/10 MISSED its band**: 0.1015 against 0.12–0.22. R/30 gave 0.0400,
//!   on the lower edge of 0.04–0.08, and R/100 gave 0.0163, inside
//!   0.01–0.03. Relative to the derived 3 × 0.583 σ/R (0.175, 0.058, 0.0175)
//!   the measured deficit is 0.58, 0.69 and 0.93 of it. The overshoot
//!   correction is an asymptotic small-σ result, so it overstates the
//!   deficit at coarse steps, where the shell it removes is no longer thin.
//!
//! Driver glue (`main`, `native::run`, formatting): writes what `decay.rs`
//! and `walk.rs` compute; its output is checked by `check-decaywalk.js` and
//! the functions are reached by `tests/lesson_decay_walk.rs`. Research,
//! education and V&V only (`RESPONSIBLE_USE.md`).

#[cfg(not(any(target_arch = "wasm32", target_os = "android")))]
#[path = "decay.rs"]
mod decay;
#[cfg(not(any(target_arch = "wasm32", target_os = "android")))]
#[path = "walk.rs"]
mod walk;

#[cfg(any(target_arch = "wasm32", target_os = "android"))]
fn main() {}

#[cfg(not(any(target_arch = "wasm32", target_os = "android")))]
fn main() -> Result<(), String> {
    native::run()
}

#[cfg(not(any(target_arch = "wasm32", target_os = "android")))]
mod native {
    use super::decay::*;
    use super::walk::*;
    use boon_lay::prelude::decay_library::DecayLibrary;
    use std::fmt::Write as _;

    /// Atoms per parent in the counted population.
    pub const N_BIG: usize = 20_000;
    /// Atoms per parent drawn on the page.
    pub const N_SHOW: usize = 300;

    fn e(x: f64) -> String {
        if x.is_finite() { format!("{x:.5e}") } else { "null".into() }
    }
    fn arr(v: impl IntoIterator<Item = f64>) -> String {
        format!("[{}]", v.into_iter().map(e).collect::<Vec<_>>().join(","))
    }
    /// Ten significant figures, for fields a check sums or orders (CRAM48
    /// inventories, hop clocks): six figures tie or round by 1e-6.
    fn arr9(v: impl IntoIterator<Item = f64>) -> String {
        format!("[{}]", v.into_iter().map(|x| if x.is_finite() { format!("{x:.9e}") } else { "null".into() }).collect::<Vec<_>>().join(","))
    }
    fn f4(v: impl IntoIterator<Item = f64>) -> String {
        format!("[{}]", v.into_iter().map(|x| format!("{x:.4}")).collect::<Vec<_>>().join(","))
    }
    fn strs(v: impl IntoIterator<Item = String>) -> String {
        format!("[{}]", v.into_iter().map(|s| format!("\"{s}\"")).collect::<Vec<_>>().join(","))
    }

    pub fn run() -> Result<(), String> {
        let out_dir = std::env::args().nth(1).unwrap_or_else(|| {
            concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/site/science/fission-products").to_string()
        });
        let mut js = String::new();
        writeln!(js, "// The data of the Fission products lessons \"An atom's clock\" and \"Walking on spheres\".").unwrap();
        writeln!(js, "// GENERATED by crates/dhoby-ghaut/examples/lesson_decay_walk/main.rs:").unwrap();
        writeln!(js, "//   cargo run --release -p dhoby-ghaut --example lesson_decay_walk").unwrap();
        writeln!(js, "// Do not edit; checked by check-decaywalk.js. boon-lay's decay Monte Carlo on its ENDF/B-VIII.0").unwrap();
        writeln!(js, "// decay library, CRAM48 expected inventories, its Walk-on-Spheres engine and CRP-6 Case 1. SI units.").unwrap();
        writeln!(js, "window.DECAYWALK_DATA = {{").unwrap();
        decay_section(&mut js);
        walk_section(&mut js);
        writeln!(js, "}};").unwrap();
        let path = std::path::Path::new(&out_dir).join("decaywalk-data.js");
        std::fs::write(&path, &js).map_err(|e| format!("{}: {e}", path.display()))?;
        println!("wrote {} ({} bytes)", path.display(), js.len());
        Ok(())
    }

    fn decay_section(js: &mut String) {
        let mut lib = DecayLibrary::new();
        let xs: Vec<f64> = (0..=100).map(|i| 0.05 * i as f64).collect();
        writeln!(js, "  decay: {{ nBig: {N_BIG}, nShow: {N_SHOW}, xs: {}, law: {}, parents: [", f4(xs.iter().copied()), arr(xs.iter().map(|x| 0.5f64.powf(*x)))).unwrap();
        for &p in PARENTS.iter() {
            let t12 = half_life_s(&lib, p).expect("unstable parent");
            let br = branches(&lib, p);
            let sum: f64 = br.iter().map(|b| b.ratio).sum();
            println!("D1/D2 {}: T½ = {t12:.6e} s, branches {:?}, sum {sum:.9}", pretty(p), br.iter().map(|b| (b.to.map(pretty), b.ratio, b.mode.clone())).collect::<Vec<_>>());
            // The drawn atoms first, then the counted population, all from
            // the library's one generator.
            let shown: Vec<Atom> = (0..N_SHOW).map(|_| sample_atom(&mut lib, p).1).collect();
            let (mut sims, atoms): (Vec<_>, Vec<_>) = (0..N_BIG).map(|_| sample_atom(&mut lib, p)).unzip();
            let ts: Vec<f64> = xs.iter().map(|x| x * t12).collect();
            let (species, expected) = cram(&lib, p, &ts);
            let stepped = count_by_stepping(&mut sims, &species, &ts);
            let survive: Vec<f64> = ts.iter().map(|&t| atoms.iter().filter(|a| a.ends[0] > t).count() as f64 / N_BIG as f64).collect();
            let law: Vec<f64> = xs.iter().map(|x| 0.5f64.powf(*x)).collect();
            let (sw, su) = worst_sigma(&survive, &law, N_BIG);
            let step_vs_clock = survive.iter().zip(&stepped[0]).map(|(a, b)| ((a - b) * N_BIG as f64).abs()).fold(0.0, f64::max);
            println!("D3 {}: surviving parents worst {sw:.2} sigma over {su} points; D4 stepped parent vs clock: {step_vs_clock} atoms", pretty(p));
            let mut worst = Vec::new();
            for (k, s) in species.iter().enumerate() {
                let (w, u) = worst_sigma(&stepped[k], &expected[k], N_BIG);
                println!("D4 {} -> {}: worst {w:.2} sigma over {u} points", pretty(p), pretty(*s));
                worst.push((w, u));
            }
            let daughters: Vec<_> = br.iter().filter_map(|b| b.to).collect();
            let fb = first_branch_fractions(&atoms, &daughters);
            for ((d, f), b) in daughters.iter().zip(&fb).zip(br.iter().filter(|b| b.to.is_some())) {
                let s = (b.ratio * (1.0 - b.ratio) / N_BIG as f64).sqrt();
                println!("D5 {} -> {}: sampled {f:.4}, library {:.4}, {:.2} sigma", pretty(p), pretty(*d), b.ratio, if s > 0.0 { (f - b.ratio).abs() / s } else { 0.0 });
            }
            let show1 = shown.iter().filter(|a| a.ends[0] > t12).count() as f64 / N_SHOW as f64;
            println!("D6 {}: drawn atoms surviving at 1 T½ {show1:.3}", pretty(p));
            let idx = |n| species.iter().position(|s| *s == n).expect("species in the chain");
            let atoms_js: Vec<String> = shown
                .iter()
                .map(|a| {
                    let mut v = Vec::new();
                    for (k, s) in a.stages.iter().enumerate() {
                        v.push(idx(*s).to_string());
                        if let Some(end) = a.ends.get(k).filter(|x| x.is_finite()) {
                            v.push(format!("{:.4e}", end / t12));
                        }
                    }
                    format!("[{}]", v.join(","))
                })
                .collect();
            writeln!(js, "    {{ name: \"{}\", t12: {}, species: {}, speciesT12: {}, branches: [{}], firstBranch: {}, survive: {}, surviveWorst: [{:.3},{su}], stepVsClockAtoms: {step_vs_clock}, show1: {show1:.4},",
                pretty(p), e(t12), strs(species.iter().map(|s| pretty(*s))),
                format!("[{}]", species.iter().map(|s| half_life_s(&lib, *s).map_or("null".into(), e)).collect::<Vec<_>>().join(",")),
                br.iter().map(|b| format!("{{ to: \"{}\", ratio: {}, mode: \"{}\" }}", b.to.map_or("none".into(), pretty), e(b.ratio), b.mode)).collect::<Vec<_>>().join(","),
                f4(fb.iter().copied()), f4(survive.iter().copied()), sw).unwrap();
            writeln!(js, "      mc: [{}],", stepped.iter().map(|r| f4(r.iter().copied())).collect::<Vec<_>>().join(",")).unwrap();
            writeln!(js, "      cram: [{}],", expected.iter().map(|r| arr9(r.iter().copied())).collect::<Vec<_>>().join(",")).unwrap();
            writeln!(js, "      worst: [{}],", worst.iter().map(|(w, u)| format!("[{w:.3},{u}]")).collect::<Vec<_>>().join(",")).unwrap();
            writeln!(js, "      atoms: [{}] }},", atoms_js.join(",")).unwrap();
        }
        writeln!(js, "  ] }},").unwrap();
    }

    fn walk_section(js: &mut String) {
        let cell = crp6_cell(1200.0);
        let radii = radii_um(&cell);
        let db = buffer_d(1200.0);
        println!("W1: Cs in the buffer at 1200 C: D = {db:.4e} m2/s ({:.2}x the doc's 1e-8)", db / 1e-8);
        writeln!(js, "  walk: {{ radii: {}, bufferD: {}, steps: [", arr(radii), e(db)).unwrap();
        for (k, sigma_um) in [14.14f64, 44.72, 141.4, 447.2].iter().enumerate() {
            let s = sigma_um * 1e-6;
            let dt = s * s / (2.0 * db);
            let all = gaussian_steps(&cell, db, dt, 20_000, 0x6A55 + k as u64);
            let fr = region_fractions(&all);
            println!("W2: sigma {sigma_um} um (dt {dt:.3e} s): landing fractions {:?}; SiC or beyond {:.4}", fr.map(|x| (x * 1e4).round() / 1e4), fr[3] + fr[4] + fr[5]);
            let pts: Vec<String> = all[..300].iter().map(|g| format!("[{:.1},{:.1},{:.1},{:.1},{}]", g.from[0], g.from[1], g.to[0], g.to[1], g.region)).collect();
            writeln!(js, "    {{ sigmaUm: {sigma_um}, dt: {}, frac: {}, pts: [{}] }},", e(dt), f4(fr), pts.join(",")).unwrap();
        }
        writeln!(js, "  ],").unwrap();

        let (tm, tse) = theta_mean(200_000, 0x7E7A);
        println!("W3: mean theta {tm:.6} +- {tse:.6} (1/6 = {:.6}, {:.2} se)", 1.0 / 6.0, (tm - 1.0 / 6.0).abs() / tse);
        let paths = wos_paths(1200.0, 2000, 0x0B5E);
        let hops: Vec<usize> = paths.iter().map(|p| p.len() - 1).collect();
        let mean_hops = hops.iter().sum::<usize>() as f64 / hops.len() as f64;
        let worst_inside = paths.iter().flat_map(|p| p.windows(2).map(|w| ((w[0][0].powi(2) + w[0][1].powi(2) + w[0][2].powi(2)).sqrt() + w[1][3]) / 212.5 - 1.0)).fold(f64::MIN, f64::max);
        println!("W4: mean hops {mean_hops:.2} (min {}, max {}); worst (|c| + r)/R - 1 = {worst_inside:.3e}", hops.iter().min().unwrap(), hops.iter().max().unwrap());
        let (dk, dc) = (kernel_d(1200.0), cell_kernel_d(1200.0));
        println!("W5: kernel D {dk:e} vs cell {dc:e}: equal {}", dk == dc);
        let drawn: Vec<String> = paths[..8].iter().map(|p| format!("[{}]", p.iter().map(|r| format!("[{:.2},{:.2},{:.2},{:.2},{:.9e}]", r[0], r[1], r[2], r[3], r[4])).collect::<Vec<_>>().join(","))).collect();
        writeln!(js, "  theta: [{tm:.6},{tse:.6}], hopsMean: {mean_hops:.3}, hopsN: {}, hopsMax: {}, insideWorst: {}, kernelD: {}, cellKernelD: {},", hops.len(), hops.iter().max().unwrap(), e(worst_inside), e(dk), e(dc)).unwrap();
        writeln!(js, "  paths: [{}],", drawn.join(",")).unwrap();

        writeln!(js, "  release: [").unwrap();
        for (temp, tmax) in [(1200.0, 200.0), (1600.0, 8.0)] {
            let hours: Vec<f64> = (0..=50).map(|i| tmax * i as f64 / 50.0).collect();
            let times = release_times(temp, 10_000, CASE1_SEED);
            let wos = fraction_by(&times, &hours);
            let cr: Vec<f64> = hours.iter().map(|&h| crank(temp, h)).collect();
            let (w, u) = worst_sigma(&wos, &cr, 10_000);
            let c1 = case1(temp);
            let c1c = crank(temp, 200.0);
            println!("W6 {temp} C: case1 MC {c1:.4}, Crank {c1c:.4}; W7 curve worst {w:.2} sigma over {u} points; D = {:.4e}", kernel_d(temp));
            write!(js, "    {{ temp: {temp}, d: {}, hours: {}, wos: {}, crank: {}, worst: [{w:.3},{u}], case1: {c1:.4}, case1Crank: {c1c:.4}", e(kernel_d(temp)), f4(hours.iter().copied()), f4(wos), f4(cr.iter().copied())).unwrap();
            if temp == 1200.0 {
                write!(js, ", gauss: [").unwrap();
                for s in [0.1, 1.0 / 30.0, 0.01] {
                    let gt = gaussian_release_times(temp, s, 10_000, 0x6A0 + (1.0 / s) as u64, tmax);
                    let g = fraction_by(&gt, &hours);
                    println!("W8 sigma = R/{:.0}: release at 200 h {:.4}, Crank {:.4}, deficit {:.4}", 1.0 / s, g[50], cr[50], cr[50] - g[50]);
                    write!(js, "{{ sOverR: {s:.5}, frac: {} }},", f4(g)).unwrap();
                }
                write!(js, "]").unwrap();
            }
            writeln!(js, " }},").unwrap();
        }
        writeln!(js, "  ] }}").unwrap();
    }
}
