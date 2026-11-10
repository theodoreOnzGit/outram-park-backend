//! **CPU against GPU pours, judged on packing fraction and g(r)** (gh:#1250;
//! maintainer 2026-11-05: "the key thing for the WGSL LIGGGHTS to emulate is
//! the packing fraction and rdf. If those match well enough, then it's ok").
//!
//! Pours the same `n` pebbles on the CPU with each of `k` seeds and on the
//! GPU with the first seed, and settles each. A chaotic pour gives a
//! different realisation from any change in the last bit, so once settled
//! the GPU pour is compared with the CPU's *distribution*, not pebble by
//! pebble. For each quantity (whole-core φ, surface, g(r) at the contact
//! peak, √2 d, √3 d, 2 d) the CPU pours give a mean `m` and sample standard
//! deviation `s`; the GPU pour passes if it lies inside the 95 % prediction
//! interval for one more pour from the same physics,
//! `m ± t(0.975, k − 1) · s · √(1 + 1/k)`. Decided before the first run
//! (gh:#1250, 2026-11-05), replacing a first idea, "within the |a − b| of two
//! CPU seeds", which a perfect GPU would fail half the time.
//!
//! ```text
//! cargo run --release -p outram-park-fork-liggghts --example gpu_pour_compare -- \
//!     [--n 16890] [--seeds 1,2,3] [--threads 4] [--max-steps 80000] [--csv FILE]
//! ```
//!
//! Prints one row per pour and the verdict. Timing is wall time on this
//! machine; state the hardware with any number quoted.

use outram_park_fork_liggghts::prelude::*;
use outram_park_fork_liggghts::htr10_fill::Htr10GpuFill;
use uom::si::length::meter;

/// What a settled pour measures.
#[derive(Debug, Clone)]
struct Measure {
    steps: usize,
    seconds: f64,
    phi: f64,
    surface: f64,
    peak: f64,
    g_sqrt2: f64,
    g_sqrt3: f64,
    g_2: f64,
    /// The whole curve: (r / d, g).
    curve: Vec<(f64, f64)>,
}

/// g(r) of the core (`z > 0`), `bed_rdf_check`'s instrument (r_max 5 d,
/// 250 bins, the bed's own cylinder), at the points that tell random from
/// ordered.
fn measure(p: FillProgress, centres: &[Vec3], seconds: f64) -> Measure {
    let core: Vec<Vec3> = centres.iter().copied().filter(|c| c.z > 0.0).collect();
    let d = 0.06;
    let rdf = radial_distribution(&core, RdfSettings::for_diameter(d, RdfDomain::from_positions(&core)), ComputeType::CpuMultiThread(ThreadCount::Auto));
    let at = |x: f64| {
        let i = rdf.r.iter().position(|r| *r >= x * d).unwrap_or(0);
        rdf.g[i.saturating_sub(1)..=(i + 1).min(rdf.g.len() - 1)].iter().sum::<f64>() / 3.0
    };
    Measure {
        steps: p.steps,
        seconds,
        phi: p.phi_whole_core,
        surface: p.surface_height.get::<meter>(),
        peak: rdf.g.iter().copied().fold(0.0, f64::max),
        g_sqrt2: at(2f64.sqrt()),
        g_sqrt3: at(3f64.sqrt()),
        g_2: at(2.0),
        curve: rdf.r.iter().zip(&rdf.g).map(|(r, g)| (r / d, *g)).collect(),
    }
}

/// `main`/CLI glue (exempt from the reaching-test rule); the pours and the
/// instruments are tested in the library and `tests/gpu_dem.rs`.
fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let arg = |k: &str| args.iter().position(|a| a == k).and_then(|i| args.get(i + 1)).cloned();
    let n: usize = arg("--n").map_or(16_890, |v| v.parse().expect("--n"));
    let seeds: Vec<u64> = arg("--seeds").map_or(vec![1, 2, 3], |v| v.split(',').map(|x| x.parse().expect("--seeds")).collect());
    if seeds.len() < 2 {
        return Err("--seeds needs at least two seeds".into());
    }
    let threads: usize = arg("--threads").map_or(4, |v| v.parse().expect("--threads"));
    let max_steps: usize = arg("--max-steps").map_or(80_000, |v| v.parse().expect("--max-steps"));
    let settings = |seed| Htr10FillSettings { n_pebbles: n, seed, threads: ThreadCount::Fixed(threads), max_steps, ..Htr10FillSettings::default() };

    let cpu = |seed: u64| -> Result<Measure, String> {
        let mut f = Htr10Fill::new(settings(seed)).map_err(|e| format!("{e:?}"))?;
        let t0 = std::time::Instant::now();
        let mut p = f.progress();
        while !(p.settled || p.gave_up) {
            p = f.advance(2_000);
        }
        Ok(measure(p, &f.centres(), t0.elapsed().as_secs_f64()))
    };
    let csv = arg("--csv");
    let mut g = Htr10GpuFill::new(settings(seeds[0]))?;
    println!("# n {n}, CPU seeds {seeds:?} (GPU seed {}), CPU threads {threads}; GPU adapter {}", seeds[0], g.adapter());
    let t0 = std::time::Instant::now();
    let mut p = g.progress();
    while !(p.settled || p.gave_up) {
        p = g.advance(2_000);
    }
    let gpu = measure(p, &g.centres(), t0.elapsed().as_secs_f64());
    let e = g.errors();
    let cpus: Vec<Measure> = seeds.iter().map(|&sd| cpu(sd)).collect::<Result<_, _>>()?;
    println!("pour,seed,steps,wall_s,ms_per_step,phi_whole_core,surface_m,g_peak,g_sqrt2d,g_sqrt3d,g_2d");
    let row = |name: &str, seed: u64, m: &Measure| {
        println!(
            "{name},{seed},{},{:.1},{:.2},{:.4},{:.4},{:.2},{:.3},{:.3},{:.3}",
            m.steps,
            m.seconds,
            1e3 * m.seconds / m.steps.max(1) as f64,
            m.phi,
            m.surface,
            m.peak,
            m.g_sqrt2,
            m.g_sqrt3,
            m.g_2
        );
    };
    for (m, sd) in cpus.iter().zip(&seeds) {
        row("cpu", *sd, m);
    }
    row("gpu", seeds[0], &gpu);
    println!("# GPU table overflows: cells {}, contact slots {}", e.cell_overflow, e.slot_overflow);
    if let Some(path) = &csv {
        // Every pour's g(r) side by side (the same bins: same instrument).
        let mut out = String::from("r_over_d");
        for sd in &seeds {
            out.push_str(&format!(",g_cpu_seed{sd}"));
        }
        out.push_str(&format!(",g_gpu_seed{}\n", seeds[0]));
        for (k, (r, _)) in gpu.curve.iter().enumerate() {
            out.push_str(&format!("{r:.4}"));
            for m in &cpus {
                out.push_str(&format!(",{:.4}", m.curve.get(k).map_or(f64::NAN, |c| c.1)));
            }
            out.push_str(&format!(",{:.4}\n", gpu.curve[k].1));
        }
        std::fs::write(path, out).map_err(|e| e.to_string())?;
        println!("# wrote {path}");
    }
    let field = |m: &Measure| [m.phi, m.surface, m.peak, m.g_sqrt2, m.g_sqrt3, m.g_2];
    let names = ["phi_whole_core", "surface_m", "g_peak", "g_sqrt2d", "g_sqrt3d", "g_2d"];
    let k = cpus.len() as f64;
    let t = t975(cpus.len() - 1);
    let mut pass = e.cell_overflow == 0 && e.slot_overflow == 0;
    for q in 0..names.len() {
        let xs: Vec<f64> = cpus.iter().map(|m| field(m)[q]).collect();
        let mean = xs.iter().sum::<f64>() / k;
        let sd = (xs.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (k - 1.0)).sqrt();
        let half = t * sd * (1.0 + 1.0 / k).sqrt();
        let gv = field(&gpu)[q];
        let ok = (gv - mean).abs() <= half;
        pass &= ok;
        println!("# {}: GPU {gv:.4}; CPU mean {mean:.4}, s {sd:.4}; 95 % prediction interval ±{half:.4}: {}", names[q], if ok { "inside" } else { "OUTSIDE" });
    }
    println!("# verdict: {}", if pass { "the GPU pour is inside the CPU pours' 95 % prediction interval on every quantity" } else { "NOT inside on every quantity (see above)" });
    Ok(())
}

/// Student's t at 0.975 for `df` degrees of freedom (table values; beyond 10
/// the normal 1.96 is within 15 %).
fn t975(df: usize) -> f64 {
    const T: [f64; 10] = [12.706, 4.303, 3.182, 2.776, 2.571, 2.447, 2.365, 2.306, 2.262, 2.228];
    T.get(df.saturating_sub(1)).copied().unwrap_or(1.96)
}
