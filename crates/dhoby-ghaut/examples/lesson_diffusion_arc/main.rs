//! **The data of the diffusion arc's two Fission products lessons**
//! (gh:#1209, maintainer direction 2026-11-04: "start from the diffusion
//! equation, then the analytical solution, then an approximate (numerical)
//! one, then say that TRISO-ATOPS uses the analytical solution
//! conservatively", the last taken as a question to measure):
//! `diffusion-three-ways.html` and `atops-conservative.html`. Writes
//! `diffarc-data.js` in `docs/site/science/fission-products/`.
//!
//! ```bash
//! cargo run --release -p dhoby-ghaut --example lesson_diffusion_arc [out_dir]
//! cargo run --release -p dhoby-ghaut --example lesson_diffusion_arc -- --inputs
//! ```
//!
//! `--inputs` prints only the inputs (D per layer, radii), which were read
//! before the predictions below were written; it computes no result.
//!
//! **Predictions, written 2026-11-04 after reading only the `--inputs`
//! output (D per layer and each layer's l²/D) and before any result:**
//!
//! Part A, one sphere three ways (CRP-6 Case 1a: Cs-137, 212.5 µm kernel,
//! 1200 °C, D = 2.2519e-15 m²/s, 0-200 h, so Dt/a² reaches 0.0359):
//!
//! - A1. Finite volumes with 80 cells and 2000 steps stay within 1e-3 of
//!   Crank's series at all 51 times, and within 3e-4 at 200 h. At 200 h the
//!   error falls 3-5× per halving of the cell size from 5 to 10 to 20 cells
//!   (second order in space), until the time-step error takes over.
//! - A2. The concentration profile at Dt/a² = 0.005, 0.02 and 0.08 (80 cells,
//!   4000 steps) is within 0.01 of the starting concentration of Crank's exact
//!   profile at every cell centre.
//!
//! Part B, the HTR-10 particle (radii 250/340/380/415/455 µm), isothermal
//! holds of 500 h at 1200, 1400, 1600 and 1800 °C, the whole inventory
//! starting uniformly in the kernel, no decay, sink at the outer surface:
//!
//! - B1. Failed particle, Cs and Sr: the bare-kernel finite volumes agree
//!   with TRISO-ATOPS's `booth_transient` within 1e-3 absolute at every hold
//!   and time. Booth on ∫D dt is the exact solution of that problem, so the
//!   formula itself is not an approximation; any conservatism is in treating
//!   a failed particle as a bare kernel. Expected values at 500 h from
//!   Dt/a²: Cs ≈ 0.67 (1200 °C) and ≈ 1 above; Sr ≈ 0.002, 0.02, 0.13, 0.5.
//! - B2. Intact particle, Cs and Sr, where TRISO-ATOPS releases exactly 0:
//!   - Cs: < 1e-6 at 1200 °C and < 1e-4 at 1400 °C (the SiC time lag
//!     l²/6D is 2.8e4 h and 6.8e3 h); 2e-3 to 1e-2 at 1600 °C (lag 615 h,
//!     estimated from the planar time-lag series diluted over kernel, buffer
//!     and IPyC); 0.5 to 0.95 at 1800 °C (lag 31 h).
//!   - Sr: never above the bare kernel's own release (the kernel holds Sr;
//!     its l²/D is 1.6e9 h at 1200 °C), and within a factor of 3 of it at
//!     1600 and 1800 °C, where the coatings are faster than the kernel.
//!   - So, for Cs at 1600 °C, intact particles release more in total than a
//!     failed fraction of 1e-4 does (1e-4 × ≈1): **TRISO-ATOPS's zero for
//!     intact particles is NOT conservative there**, with these SiC D values.
//! - B3. Silver, every particle: TRISO-ATOPS's planar breakthrough is at or
//!   above the well-mixed-inside-SiC finite volumes at every hold and time
//!   (kernel concentration and kernel area at the SiC face, no depletion),
//!   and the well-mixed run is at or above the intact five-layer run (the
//!   kernel and PyC only slow it). So the breakthrough formula **is
//!   conservative for silver**: the ratio breakthrough / intact at 500 h lies
//!   between 1 and 10 at 1200-1600 °C, and both are near 1 at 1800 °C (the
//!   formula clamps at 1).
//! - B4. Numerical uncertainty: doubling the cells and the steps changes
//!   every 500 h value by less than 2 % relative or 1e-6 absolute.
//!
//! **Results of the FIRST run, 2026-11-04** (base mesh kernel/buffer/IPyC/
//! SiC/OPyC = 100/20/10/20/10 cells, 5000 steps; refinement compared at
//! 500 h only). Kept as run:
//!
//! - A1: **MISSED** on the worst time: 80 cells gave 1.44e-3 (prediction
//!   1e-3), all of it early, where the √t start is hardest to resolve; at
//!   200 h 1.28e-4 (held, < 3e-4). The 200 h error fell 4.9× (5→10 cells) and
//!   4.0× (10→20): held.
//! - A2 held: 9.1e-4, 2.3e-4, 8.7e-5 at Dt/a² = 0.005, 0.02, 0.08.
//! - B1: Cs held at 1200/1400/1800 °C (worst 6.8e-4, 5.8e-4, 9.2e-4) and
//!   **missed at 1600 °C (1.17e-3)**. For Sr it **failed badly** at 1200 °C:
//!   Booth 1.90e-3 against finite volumes 1.89e-4. It also missed at
//!   1400 °C (2.05e-2 against 1.62e-2) and was slightly over 1e-3 at
//!   1600 °C (4.6e-3). B4 showed why: refining changed the Sr result by 50 %
//!   (1200 °C) and 29 % (1400 °C). Sr's diffusion length in the kernel over
//!   500 h is about 0.14 µm at 1200 °C, against 2.5 µm cells, so the finite
//!   volumes, not Booth, were wrong.
//! - B2: Cs 5.2e-25, 1.4e-11 and 5.56e-3 (held), and **0.491 at 1800 °C
//!   (missed the 0.5-0.95 band, just under)**. Sr intact was never above
//!   bare, and was 0.52 and 0.96 of it at 1600 and 1800 °C (held).
//! - B3: **the first clause MISSED**. The breakthrough formula fell below
//!   the well-mixed finite volumes at 7 early times at 1200 °C (60-120 h)
//!   and 1 at 1400 °C (10 h), all with releases below 3e-6, while staying
//!   above the intact run. At 500 h the ratio breakthrough / intact was
//!   2.70, 1.83, 1.34 and 1.005 (held). Suspected cause, not yet tested: the
//!   leading edge of the front is smeared on a 1.75 µm SiC mesh, which the
//!   500 h-only refinement check could not see.
//! - B4: **missed for Sr** at 1200 and 1400 °C (above); the rest were below
//!   0.7 %.
//!
//! **Protocol corrected after the first run (the criteria are unchanged).**
//! The kernel is split into a core and an outer shell with the same D. The
//! shell is 10 diffusion lengths √(D t_end) thick (capped at half the
//! kernel) with 200 cells. The base SiC mesh goes from 20 to 80 cells, the
//! PyC layers from 10 to 20, and the buffer stays at 20. B4 now compares the
//! refined run at every time.
//!
//! **Results of the corrected run, 2026-11-04** (these are the numbers the
//! pages quote):
//!
//! - A1 and A2: unchanged (part A was not re-meshed).
//! - B1: Sr now agrees with Booth to 7.2e-6, 2.7e-5, 1.7e-4 and 1.6e-4
//!   (1200-1800 °C), and Cs to 1.8e-4, 4.5e-4 and 9.2e-4 at 1200, 1400 and
//!   1800 °C. **Cs at 1600 °C still misses: 1.14e-3**, at early times. There
//!   the shell is capped at half the kernel and the √t start is resolved
//!   least well. Booth on ∫D dt is exact for the bare kernel; the residual is
//!   the mesh's.
//! - B2: Cs 1.4e-40, 2.6e-13, **5.55e-3** and **0.491** (1200-1800 °C): the
//!   1800 °C value still just misses its band. Sr 1.9e-6, 2.3e-3, 6.8e-2 and
//!   0.49, which is 1.0e-3, 0.11, 0.52 and 0.96 of the bare kernel (held).
//!   The first run's Sr 1200 °C value (9.2e-8) was mesh error. **The finding
//!   stands: at 1600 °C an intact particle releases 5.6e-3 of its Cs in 500 h,
//!   about 56 times what a failed fraction of 1e-4 releases, and
//!   TRISO-ATOPS counts it as zero.** It rests on boon-lay's walker D for Cs
//!   in SiC (9.2e-17 m²/s at 1600 °C), whose own uncertainty is not assessed
//!   here.
//! - B3: at 1200, 1600 and 1800 °C the breakthrough formula is at or above
//!   the well-mixed run at every time; at every hold and time it is at or
//!   above the intact run. At 1400 °C it fell below the well-mixed run at
//!   10 h only (2.58e-8 against 8.6e-8). `--converge-ag1400` showed this was
//!   the 500 h run's time step: with 5000 to 40 000 steps over the first
//!   10 h the well-mixed value converges to 1.84e-8 (2.04, 1.89, 1.85,
//!   1.84e-8), below the formula. So **conservative for silver at every hold
//!   and time**, by 2.72, 1.83, 1.34 and 1.005 at 500 h.
//! - B4 at every time: the bare and intact curves change by under 1.1 % on
//!   refining, except Cs at 1600 °C (4.1 %) and 1800 °C (12.9 %) and Ag at
//!   1200, 1400 and 1600 °C (3.5 %, 11.6 %, 4.0 %). All of those are on the
//!   leading edge of the release, at backward Euler's coarse early steps.
//!   **B4 therefore MISSED for those curves' early points.** The 500 h values
//!   quoted above change by at most 7.0e-4 relative (Ag 1200 °C; measured,
//!   `refineEnd` in the data).
//!
//! Driver glue (`main`, `native::run`, formatting): writes what `arc.rs`
//! computes; its output is checked by `check-diffarc.js` and the functions are
//! reached by `tests/lesson_diffusion_arc.rs`. Research, education and V&V
//! only (`RESPONSIBLE_USE.md`).

#[cfg(not(any(target_arch = "wasm32", target_os = "android")))]
#[path = "arc.rs"]
mod arc;

#[cfg(any(target_arch = "wasm32", target_os = "android"))]
fn main() {}

#[cfg(not(any(target_arch = "wasm32", target_os = "android")))]
fn main() -> Result<(), String> {
    native::run()
}

#[cfg(not(any(target_arch = "wasm32", target_os = "android")))]
mod native {
    use super::arc::*;
    use std::fmt::Write as _;

    pub const HOLDS_C: [f64; 4] = [1200.0, 1400.0, 1600.0, 1800.0];

    pub fn run() -> Result<(), String> {
        if std::env::args().any(|a| a == "--converge-ag1400") {
            // The one early point where the breakthrough formula stayed
            // below the well-mixed run after the protocol fix: refine it.
            let hours: Vec<f64> = (0..=50).map(|i| 10.0 * i as f64 / 50.0).collect();
            for refine in [1usize, 2, 4, 8] {
                let (layers, dd, start) = setup(Particle::WellMixedInsideSic, 47, boon_lay::Nuclide::Ag110m, 1400.0, 500.0, refine);
                let f = fv_curve(&layers, &dd, start, 5000 * refine, &hours);
                println!("well-mixed Ag 1400 C at 10 h, refine {refine} (steps over 10 h: {}): {:.4e}", 5000 * refine, f[50]);
            }
            println!("breakthrough at 10 h: {:.4e}", atops_silver(1400.0, 10.0));
            return Ok(());
        }
        if std::env::args().any(|a| a == "--inputs") {
            inputs();
            return Ok(());
        }
        let out_dir = std::env::args().nth(1).filter(|a| !a.starts_with("--")).unwrap_or_else(|| {
            concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/site/science/fission-products").to_string()
        });
        let mut js = String::new();
        writeln!(js, "// The data of the Fission products lessons \"The diffusion equation, three ways\" and").unwrap();
        writeln!(js, "// \"Is TRISO-ATOPS's analytical solution conservative?\" (gh:#1209).").unwrap();
        writeln!(js, "// GENERATED by crates/dhoby-ghaut/examples/lesson_diffusion_arc/main.rs:").unwrap();
        writeln!(js, "//   cargo run --release -p dhoby-ghaut --example lesson_diffusion_arc").unwrap();
        writeln!(js, "// Do not edit; checked by check-diffarc.js. Release fractions; SI units; temperatures in °C.").unwrap();
        writeln!(js, "window.DIFFARC_DATA = {{").unwrap();
        part_a(&mut js);
        part_b(&mut js);
        writeln!(js, "}};").unwrap();
        let path = std::path::Path::new(&out_dir).join("diffarc-data.js");
        std::fs::write(&path, &js).map_err(|e| format!("{}: {e}", path.display()))?;
        println!("wrote {} ({} bytes)", path.display(), js.len());
        Ok(())
    }

    fn e(x: f64) -> String {
        format!("{x:.6e}")
    }
    fn arr(v: &[f64]) -> String {
        format!("[{}]", v.iter().map(|x| e(*x)).collect::<Vec<_>>().join(","))
    }
    fn grid(t_end: f64) -> Vec<f64> {
        (0..=50).map(|i| t_end * i as f64 / 50.0).collect()
    }

    fn part_a(js: &mut String) {
        let a = CRP6_KERNEL_M;
        let d = crp6_d(1200.0);
        let hours = grid(200.0);
        let crank = crank_curve(d, a, &hours);
        writeln!(js, "  a: {{ radius: {}, d: {}, hours: {}, crank: {},", e(a), e(d), arr(&hours), arr(&crank)).unwrap();
        write!(js, "    fv: [").unwrap();
        for cells in [5usize, 10, 20, 80] {
            let fv = fv_curve(&[(a, cells)], &[d], 1, 2000, &hours);
            let worst = fv.iter().zip(&crank).map(|(x, y)| (x - y).abs()).fold(0.0, f64::max);
            let end = (fv[50] - crank[50]).abs();
            println!("A1 {cells} cells: worst |FV - Crank| {worst:.3e}; at 200 h {end:.3e}");
            write!(js, "{{ cells: {cells}, frac: {}, worst: {}, end: {} }},", arr(&fv), e(worst), e(end)).unwrap();
        }
        writeln!(js, "],").unwrap();
        write!(js, "    profiles: [").unwrap();
        for tau in [0.005, 0.02, 0.08] {
            let (x, c) = fv_profile(80, tau, 4000);
            let exact: Vec<f64> = x.iter().map(|xi| exact_profile(*xi, tau, 400)).collect();
            let worst = c.iter().zip(&exact).map(|(p, q)| (p - q).abs()).fold(0.0, f64::max);
            println!("A2 tau {tau}: worst |FV - exact| {worst:.3e}");
            write!(js, "{{ tau: {tau}, x: {}, fv: {}, exact: {}, worst: {} }},", arr(&x), arr(&c), arr(&exact), e(worst)).unwrap();
        }
        writeln!(js, "] }},").unwrap();
    }

    fn part_b(js: &mut String) {
        let hours = grid(500.0);
        let r = htr10_radii();
        writeln!(js, "  b: {{ radii: {}, hours: {}, holds: {:?}, cases: [", arr(&r), arr(&hours), HOLDS_C).unwrap();
        for (name, z, nuc) in elements() {
            for t in HOLDS_C {
                let d = d_layers(z, nuc, t);
                let run = |p, refine: usize, steps| {
                    let (layers, dd, start) = setup(p, z, nuc, t, 500.0, refine);
                    fv_curve(&layers, &dd, start, steps, &hours)
                };
                let intact = run(Particle::Intact, 1, 5000);
                let intact2 = run(Particle::Intact, 2, 10000);
                // B4 at every time: relative change where the value is at
                // least 1e-6, absolute below.
                let rel = |a: &[f64], b: &[f64]| {
                    a.iter().zip(b).map(|(x, y)| {
                        let m = x.abs().max(y.abs());
                        if m < 1e-6 { if (x - y).abs() <= 1e-6 { 0.0 } else { f64::INFINITY } } else { (x - y).abs() / m }
                    }).fold(0.0, f64::max)
                };
                let mut b4 = rel(&intact, &intact2);
                let mut b4_end = rel(&intact[50..], &intact2[50..]);
                write!(js, "    {{ el: \"{name}\", t: {t}, d: {}, intact: {},", arr(&d), arr(&intact)).unwrap();
                if z == 47 {
                    let mixed = run(Particle::WellMixedInsideSic, 1, 5000);
                    let mixed2 = run(Particle::WellMixedInsideSic, 2, 10000);
                    b4 = b4.max(rel(&mixed, &mixed2));
                    b4_end = b4_end.max(rel(&mixed[50..], &mixed2[50..]));
                    let atops: Vec<f64> = hours.iter().map(|h| atops_silver(t, *h)).collect();
                    let above = atops.iter().zip(&mixed).all(|(a, m)| a + 1e-9 >= *m);
                    let mixed_above = mixed.iter().zip(&intact).all(|(m, i)| m + 1e-9 >= *i);
                    let atops_above_intact = atops.iter().zip(&intact).all(|(a, i)| a + 1e-12 >= *i);
                    let below: Vec<f64> = hours.iter().zip(atops.iter().zip(&mixed)).filter(|(_, (a, m))| *a + 1e-9 < **m).map(|(h, _)| *h).collect();
                    println!("B3 Ag {t} C: ATOPS >= intact always {atops_above_intact}; ATOPS below mixed at hours {below:?}");
                    println!("B3 Ag {t} C at 500 h: ATOPS {:.4e}, well-mixed FV {:.4e}, intact FV {:.4e}; ratio ATOPS/intact {:.3}; ATOPS >= mixed always {above}; mixed >= intact always {mixed_above}",
                        atops[50], mixed[50], intact[50], atops[50] / intact[50]);
                    write!(js, " mixed: {}, atops: {}, atopsAboveMixed: {above}, mixedAboveIntact: {mixed_above}, atopsAboveIntact: {atops_above_intact},", arr(&mixed), arr(&atops)).unwrap();
                } else {
                    let bare = run(Particle::BareKernel, 1, 5000);
                    let bare2 = run(Particle::BareKernel, 2, 10000);
                    b4 = b4.max(rel(&bare, &bare2));
                    b4_end = b4_end.max(rel(&bare[50..], &bare2[50..]));
                    let atops: Vec<f64> = hours.iter().map(|h| atops_failed_kernel(z, t, *h)).collect();
                    let worst = bare.iter().zip(&atops).map(|(x, y)| (x - y).abs()).fold(0.0, f64::max);
                    println!("B1 {name} {t} C: failed: ATOPS Booth {:.4e}, bare FV {:.4e} at 500 h; worst |diff| {worst:.3e}", atops[50], bare[50]);
                    println!("B2 {name} {t} C: intact FV {:.4e} at 500 h (ATOPS: 0); intact/bare {:.3e}", intact[50], intact[50] / bare[50]);
                    write!(js, " bare: {}, atops: {}, bareWorst: {},", arr(&bare), arr(&atops), e(worst)).unwrap();
                }
                println!("B4 {name} {t} C: worst relative change on refining, all times {b4:.3e}; at 500 h {b4_end:.3e}");
                writeln!(js, " refine: {}, refineEnd: {} }},", e(b4), e(b4_end)).unwrap();
            }
        }
        writeln!(js, "  ] }}").unwrap();
    }

    fn inputs() {
        let r = htr10_radii();
        println!("HTR-10 radii (um): {:?}", r.map(|x| x * 1e6));
        println!("CRP-6 Cs D at 1200 C: {:.4e} m2/s", crp6_d(1200.0));
        for (name, z, nuc) in elements() {
            for t in HOLDS_C {
                let d = d_layers(z, nuc, t);
                let thick = [r[0], r[1] - r[0], r[2] - r[1], r[3] - r[2], r[4] - r[3]];
                let tau: Vec<String> = (0..5).map(|i| format!("{:.2e}", thick[i] * thick[i] / d[i] / 3600.0)).collect();
                println!("{name} {t} C: D [k,b,IPyC,SiC,OPyC] = {:?}; l^2/D (h) = {:?}", d.map(|x| format!("{x:.2e}")), tau);
            }
        }
    }
}
