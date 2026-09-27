//! **HTR-10 multi-seed, multi-arm pooled k-eff study** — the driver for
//! studies too long to trust to one process (hours per arm). **Linux only.**
//!
//! ```bash
//! cargo build --release -p nee_soon --example htr10_rmc_keff --example htr10_pooled_study
//! OUTRAM_POOL_DIR=/some/dir cargo run --release -p nee_soon --example htr10_pooled_study
//! OUTRAM_POOL_SUMMARY_ONLY=1 ...   # just pool whatever logs exist
//! ```
//!
//! It runs `htr10_rmc_keff` once per (arm, seed), each in its own process
//! with its own log, `<dir>/pool_<tag>_s<seed>.log`. A log that already holds
//! a result is skipped, so a killed or restarted container resumes where it
//! stopped. Runs go seed-major (arm A seed 1, arm B seed 1, arm A seed 2, …),
//! so every completed pair is usable before the whole study finishes. At the
//! end, or at any time with `OUTRAM_POOL_SUMMARY_ONLY=1`, it pools each arm's
//! per-seed k values (`outram_mc_libs::vv::pooled`: mean, seed-to-seed sd,
//! sem). It prints each arm against the first arm and against RMC, with the
//! **hardware line** from every log (outram-mc-libs hard rule, 2026-09-27:
//! timings without hardware specs are useless).
//!
//! # Why Linux only
//!
//! Maintainer direction, 2026-09-27. The study is only ever run from a Linux
//! shell, the hardware line relies on `/proc`, and its behaviour under Windows
//! and PowerShell (process spawning, paths, environment passing) has never been
//! checked. On any other OS `main` says so and exits. The example still
//! compiles everywhere, including Android and wasm.
//!
//! # Knobs
//!
//! | Variable | Default | Meaning |
//! |---|---|---|
//! | `OUTRAM_POOL_ARMS` | `crystalline,10P,30P` | graphite laws (`OUTRAM_HTR10_GRAPHITE_TSL` values); the first is the baseline |
//! | `OUTRAM_POOL_SEEDS` | `4` | seeds per arm |
//! | `OUTRAM_POOL_SEED0` | `20260927` | first seed; the others follow consecutively |
//! | `OUTRAM_POOL_DIR` | `verification_and_validation/local_perf/htr10_pooled` (gitignored) | log directory |
//! | `OUTRAM_HTR10_HISTORIES` / `_INACTIVE` / `_ACTIVE` | `10000` / `40` / `100` | per run |
//! | `OUTRAM_HTR10_LAYERS` | `25` | bed layers (25 = the 123.576 cm critical loading) |
//!
//! Every run also gets `OUTRAM_HTR10_RINGS=14`, `OUTRAM_HTR10_NI_AS_FE=1` and
//! `OUTRAM_HTR10_FE57_AS_FE56=1`, the stated modelling assumptions of the
//! HTR-10 V&V record. Each arm is passed **explicitly**, even the default law,
//! so a later change of default cannot silently relabel an arm.
//!
//! **Not a V&V gate.** It produces numbers; the V&V doc records them with
//! methodology (`outram-mc-libs/verification_and_validation/htr10_rmc/`).

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("htr10_pooled_study is Linux-only (maintainer direction 2026-09-27); nothing run.");
}

#[cfg(target_os = "linux")]
fn main() {
    linux::main();
}

#[cfg(target_os = "linux")]
mod linux {
    use std::path::{Path, PathBuf};
    use std::process::{Command, Stdio};


    fn env_or(k: &str, d: &str) -> String {
        std::env::var(k).unwrap_or_else(|_| d.to_string())
    }

    /// Log tag for an arm. `crystalline` is `cryst`, which matches the logs of
    /// the 2026-09-27 study that this example was written from.
    fn tag(arm: &str) -> &str {
        if arm == "crystalline" {
            "cryst"
        } else {
            arm
        }
    }

    fn log_path(dir: &Path, arm: &str, seed: u64) -> PathBuf {
        dir.join(format!("pool_{}_s{seed}.log", tag(arm)))
    }

    /// What one finished log says.
    struct RunResult {
        k: f64,
        sigma: f64,
        /// RMC interpolated to this run's bed height (the log's
        /// `HEIGHT-MATCHED ... RMC(interp) = ` line).
        rmc: Option<f64>,
        transport_s: Option<f64>,
        hardware: Option<String>,
    }

    fn parse(log: &Path) -> Option<RunResult> {
        let text = std::fs::read_to_string(log).ok()?;
        let field = |prefix: &str| {
            text.lines()
                .find(|l| l.trim_start().starts_with(prefix))
                .and_then(|l| l.split_once(if prefix.starts_with("k_eff") { '=' } else { ':' }))
                .map(|(_, v)| v.trim().to_string())
        };
        let k_line = field("k_eff  ")?;
        let mut it = k_line.split("+/-").map(|s| s.trim().parse::<f64>());
        let (Some(Ok(k)), Some(Ok(sigma))) = (it.next(), it.next()) else {
            return None;
        };
        let transport_s = field("transport    :")
            .and_then(|v| v.trim_end_matches('s').trim().parse().ok());
        let rmc = text
            .lines()
            .find(|l| l.contains("HEIGHT-MATCHED"))
            .and_then(|l| l.split("RMC(interp) =").nth(1))
            .and_then(|v| v.trim().parse().ok());
        Some(RunResult {
            k,
            sigma,
            rmc,
            transport_s,
            hardware: field("hardware     :"),
        })
    }

    pub fn main() {
        let arms: Vec<String> = env_or("OUTRAM_POOL_ARMS", "crystalline,10P,30P")
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        let n_seeds: u64 = env_or("OUTRAM_POOL_SEEDS", "4").parse().expect("OUTRAM_POOL_SEEDS");
        let seed0: u64 = env_or("OUTRAM_POOL_SEED0", "20260927").parse().expect("OUTRAM_POOL_SEED0");
        let dir = std::env::var("OUTRAM_POOL_DIR").map_or_else(
            |_| {
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../verification_and_validation/local_perf/htr10_pooled")
            },
            PathBuf::from,
        );
        std::fs::create_dir_all(&dir).expect("create log directory");
        let seeds: Vec<u64> = (0..n_seeds).map(|i| seed0 + i).collect();

        if std::env::var("OUTRAM_POOL_SUMMARY_ONLY").is_err() {
            let exe = std::env::current_exe()
                .expect("own path")
                .with_file_name("htr10_rmc_keff");
            assert!(
                exe.exists(),
                "{} not built: cargo build --release -p nee_soon --example htr10_rmc_keff",
                exe.display()
            );
            for &seed in &seeds {
                for arm in &arms {
                    let log = log_path(&dir, arm, seed);
                    if parse(&log).is_some() {
                        println!("skip  {} (done)", log.display());
                        continue;
                    }
                    println!("run   {arm} seed {seed} -> {}", log.display());
                    let out = std::fs::File::create(&log).expect("create log");
                    let err = out.try_clone().expect("clone log handle");
                    let status = Command::new(&exe)
                        .env("OUTRAM_HTR10_GRAPHITE_TSL", arm)
                        .env("OUTRAM_HTR10_SEED", seed.to_string())
                        .env("OUTRAM_HTR10_RINGS", "14")
                        .env("OUTRAM_HTR10_LAYERS", env_or("OUTRAM_HTR10_LAYERS", "25"))
                        .env("OUTRAM_HTR10_HISTORIES", env_or("OUTRAM_HTR10_HISTORIES", "10000"))
                        .env("OUTRAM_HTR10_INACTIVE", env_or("OUTRAM_HTR10_INACTIVE", "40"))
                        .env("OUTRAM_HTR10_ACTIVE", env_or("OUTRAM_HTR10_ACTIVE", "100"))
                        .env("OUTRAM_HTR10_NI_AS_FE", "1")
                        .env("OUTRAM_HTR10_FE57_AS_FE56", "1")
                        .stdout(Stdio::from(out))
                        .stderr(Stdio::from(err))
                        .status()
                        .expect("spawn htr10_rmc_keff");
                    if !status.success() || parse(&log).is_none() {
                        eprintln!("FAILED {arm} seed {seed} ({status}); see {}", log.display());
                    }
                }
            }
        }

        summarise(&arms, &seeds, &dir);
    }

    fn summarise(arms: &[String], seeds: &[u64], dir: &Path) {
        println!("\nHTR-10 pooled study; per-seed k, then pooled per arm\n");
        let mut hardware: Vec<String> = Vec::new();
        let mut rmcs: Vec<f64> = Vec::new();
        // (arm, n, mean k, seed sd, sem, pooled residual vs each run's own
        // height-matched RMC, in pcm)
        let mut pooled: Vec<(String, usize, f64, f64, f64, f64)> = Vec::new();
        for arm in arms {
            let mut ks = Vec::new();
            let mut dks = Vec::new();
            for &seed in seeds {
                match parse(&log_path(dir, arm, seed)) {
                    Some(r) => {
                        // Each run against ITS OWN reference: logs written
                        // before gh:#333 matched a different height.
                        dks.push(r.rmc.map_or(f64::NAN, |x| (r.k - x) * 1e5));
                        if let Some(x) = r.rmc {
                            rmcs.push(x);
                        }
                        println!(
                            "  {arm:<12} seed {seed}: k = {:.6} +/- {:.6}  transport {}",
                            r.k,
                            r.sigma,
                            r.transport_s.map_or("?".into(), |t| format!("{t:.0} s"))
                        );
                        hardware.push(r.hardware.unwrap_or_else(|| "hardware not recorded".into()));
                        ks.push(r.k);
                    }
                    None => println!("  {arm:<12} seed {seed}: not finished"),
                }
            }
            let (mean, sd, sem) = outram_mc_libs::vv::pooled(&ks);
            let (dk, _, _) = outram_mc_libs::vv::pooled(&dks);
            pooled.push((arm.clone(), ks.len(), mean, sd, sem, dk));
        }
        println!("\n| arm | seeds | pooled k | sem [pcm] | seed sd [pcm] | vs RMC [pcm] | vs {} [pcm] |", arms[0]);
        println!("|---|---|---|---|---|---|---|");
        let base = pooled.first().map(|p| (p.2, p.4));
        for (arm, n, mean, sd, sem, dk) in &pooled {
            if *n == 0 {
                println!("| {arm} | 0 | — | — | — | — | — |");
                continue;
            }
            let vs_base = match base {
                Some((b, bs)) if arm != &arms[0] && b.is_finite() => {
                    let d = (mean - b) * 1e5;
                    let e = (sem * sem + bs * bs).sqrt() * 1e5;
                    if e > 0.0 {
                        format!("{d:+.0} ± {e:.0} ({:.1}σ)", d / e)
                    } else {
                        format!("{d:+.0} (no sem: needs ≥ 2 seeds per arm)")
                    }
                }
                _ => "—".into(),
            };
            println!(
                "| {arm} | {n} | {mean:.6} | {} | {} | {dk:+.0} | {vs_base} |",
                if *n > 1 { format!("{:.0}", sem * 1e5) } else { "n/a".into() },
                if *n > 1 { format!("{:.0}", sd * 1e5) } else { "n/a".into() },
            );
        }
        rmcs.sort_by(f64::total_cmp);
        rmcs.dedup();
        println!("\nRMC references (height-matched, from the logs): {rmcs:?}");
        if rmcs.len() > 1 {
            println!("  !! the logs disagree on the reference height; vs-RMC is per run");
        }
        hardware.sort();
        hardware.dedup();
        println!("\nhardware (every distinct line in the logs):");
        for h in &hardware {
            println!("  {h}");
        }
        println!(
            "\nsem is from the seed-to-seed scatter (n = seeds), not the within-run sigma. \
             With one seed it is 0 and means nothing."
        );
    }
}
