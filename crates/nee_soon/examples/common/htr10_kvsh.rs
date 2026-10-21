// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK.
//
// OUTRAM PARK is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the
// Free Software Foundation, either version 3 of the License, or (at your
// option) any later version.
//
// OUTRAM PARK is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
// General Public License for more details.
//
// You should have received a copy of the GNU General Public License along
// with OUTRAM PARK.  If not, see <https://www.gnu.org/licenses/>.

//! The driver shared by `htr10_endf8_kvsh_quick` and `htr10_endf8_kvsh_heavy`
//! (gh:#501): the two differ only in their [`SweepStatistics`].
//!
//! The nuclear data are processed ONCE and every height reuses them, as do the
//! materials and the bed majorant (neither depends on the bed height). Each
//! height is then assembled, transported and logged to `logs/N<n>.log`, and
//! after every height the record is rewritten — `keff_vs_height.py`,
//! `results_table.md`, `RUN_PARAMETERS.md` — so a run stopped part-way keeps
//! every finished height.
//!
//! Arguments (all optional): `--threads <n>` (default 8), `--out <dir>`
//! (default `./htr10_endf8_kvsh_<sweep>`), `--layers 10,12,20` (default
//! N = 10..20). The thread count changes only the wall clock.

#![allow(dead_code)]

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use nee_soon::htr10_rmc::core_model::assemble_explicit_triso;
use nee_soon::htr10_rmc::data::{load_htr10_nuclides, Htr10DataConfig, Htr10NuclideLayout};
use nee_soon::htr10_rmc::keff_vs_height::script::{
    plot_script, results_table_md, run_parameters_md, SweepProvenance,
};
use nee_soon::htr10_rmc::keff_vs_height::{
    bed_majorant, run_core, HeightPoint, SweepStatistics, RINGS, SWEEP_LAYERS, TEMPERATURE_K,
};
use nee_soon::htr10_rmc::materials::{htr10_material_set, Htr10MaterialConfig};
use outram_mc_libs::run_diagnostics::RunDiagnostics;
use uom::si::f64::ThermodynamicTemperature;
use uom::si::thermodynamic_temperature::kelvin;

struct Args {
    threads: usize,
    out: PathBuf,
    layers: Vec<usize>,
}

fn parse_args(stats: &SweepStatistics) -> Args {
    let mut a = Args {
        threads: 8,
        out: PathBuf::from(format!("htr10_endf8_kvsh_{}", stats.name)),
        layers: SWEEP_LAYERS.to_vec(),
    };
    let v: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;
    while i < v.len() {
        let val = v.get(i + 1).cloned().unwrap_or_default();
        match v[i].as_str() {
            "--threads" => a.threads = val.parse().expect("--threads takes a positive integer"),
            "--out" => a.out = PathBuf::from(val),
            "--layers" => {
                a.layers = val
                    .split(',')
                    .map(|s| s.trim().parse().expect("--layers takes N,N,..."))
                    .collect();
            }
            other => {
                eprintln!("unknown argument {other:?}; use --threads n, --out dir, --layers 10,12");
                std::process::exit(2);
            }
        }
        i += 2;
    }
    assert!(a.threads >= 1, "--threads must be at least 1");
    a
}

/// `git rev-parse --short=10 HEAD` in `dir`, `-dirty` if a tracked file differs.
fn git_commit(dir: &Path) -> String {
    let run = |args: &[&str]| {
        Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(args)
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
    };
    match run(&["rev-parse", "--short=10", "HEAD"]) {
        Some(c) => {
            let dirty = run(&["status", "--porcelain", "--untracked-files=no"])
                .is_some_and(|s| !s.is_empty());
            if dirty {
                format!("{c}-dirty")
            } else {
                c
            }
        }
        None => "unknown (not a git checkout)".to_string(),
    }
}

/// `Cpus_allowed_list` of this process (Linux), else "unknown".
fn cpu_affinity() -> String {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("Cpus_allowed_list:"))
                .map(|l| l["Cpus_allowed_list:".len()..].trim().to_string())
        })
        .unwrap_or_else(|| "unknown".to_string())
}

fn write_record(out: &Path, points: &[HeightPoint], prov: &SweepProvenance, layers: &[usize]) {
    std::fs::write(out.join("keff_vs_height.py"), plot_script(points, prov)).expect("write script");
    std::fs::write(out.join("results_table.md"), results_table_md(points, prov))
        .expect("write table");
    std::fs::write(
        out.join("RUN_PARAMETERS.md"),
        run_parameters_md(prov, layers),
    )
    .expect("write parameters");
}

/// Run a whole ENDF/B-VIII.0 sweep at `stats`.
pub fn run_sweep(example: &str, stats: SweepStatistics) {
    let args = parse_args(&stats);
    std::fs::create_dir_all(args.out.join("logs")).expect("create output directory");
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let root = manifest.join("../..");
    let t_all = Instant::now();

    println!(
        "HTR-10 k vs height, ENDF/B-VIII.0, {} sweep ({example}, gh:#501)",
        stats.name
    );
    println!("==========================================================================");
    println!(
        "  {} particles x [{} inactive + {} active], seed {}, {} threads, layers {:?}",
        stats.particles, stats.inactive, stats.active, stats.seed, args.threads, args.layers
    );

    let data_cfg = Htr10DataConfig {
        temperature: ThermodynamicTemperature::new::<kelvin>(TEMPERATURE_K),
        ..Htr10DataConfig::default()
    };
    let layout =
        Htr10NuclideLayout::plan(&data_cfg).expect("the default data configuration is valid");
    let mut diag = RunDiagnostics::new(example);
    eprintln!("Reconstructing cross sections:");
    let nucs = match load_htr10_nuclides(&data_cfg, &layout, &mut diag) {
        Ok(v) => v,
        Err(e) => {
            println!("REFUSED: {e}");
            std::process::exit(1);
        }
    };
    let data_s = diag.data_seconds();
    println!(
        "  nuclear data processed once, {data_s:.1} s ({} items)",
        diag.data_item_count()
    );

    let prov = SweepProvenance {
        example: example.to_string(),
        library: "ENDF/B-VIII.0".to_string(),
        statistics: stats,
        threads: args.threads,
        commit: git_commit(&manifest),
        ace_commit: git_commit(&root.join("reference-data/ace")),
        host: diag.hardware().headline(),
        cpu_affinity: cpu_affinity(),
        model_notes: vec![
            format!(
                "graphite S(a,b) {:?}, SiC and UO2 bound laws",
                data_cfg.graphite_law
            ),
            format!("carbon {:?}", data_cfg.carbon),
            format!("coolant {:?}", data_cfg.coolant),
            format!("rod metal {}", data_cfg.rod_metal.label()),
            "TECDOC-1382 reflector zone 22, natural boron, fixed core cavity".to_string(),
        ],
    };
    println!("  commit {}, ace {}", prov.commit, prov.ace_commit);
    println!("  hardware     : {}", prov.host);
    println!("  CPUs allowed : {}", prov.cpu_affinity);
    for n in &prov.model_notes {
        println!("  model        : {n}");
    }

    let mats = htr10_material_set(
        &layout,
        Htr10MaterialConfig::benchmark_default(TEMPERATURE_K),
    );
    for m in &mats {
        for c in &m.components {
            assert!(
                c.nuclide_idx < nucs.len(),
                "{} names an unloaded slot",
                m.name
            );
        }
    }
    let majorant = bed_majorant(&mats, &nucs);

    let mut points: Vec<HeightPoint> = Vec::new();
    for &n in &args.layers {
        let mut log = String::new();
        let t_geo = Instant::now();
        let core = assemble_explicit_triso(RINGS, n, 0);
        let geo_s = t_geo.elapsed().as_secs_f64();
        let _ = writeln!(log, "HTR-10 {example}: N = {n}");
        let _ = writeln!(
            log,
            "  geometry: {} tiles, {} cells, {} universes (assembled in {geo_s:.1} s)",
            core.tiles, core.cells, core.universes
        );
        print!("\n{log}");
        let (p, res) = run_core(n, &core, &mats, &nucs, &majorant, &stats, args.threads);
        let mut tail = String::new();
        let _ = writeln!(
            tail,
            "  bed {:.3} cm built, {} balls, reference read at {:.3} cm (equal ball count, gh:#472)",
            p.built_height_cm(),
            p.balls.map_or_else(|| "n/a".to_string(), |b| b.to_string()),
            p.reference_height_cm()
        );
        let _ = writeln!(tail, "  k_eff        = {:.6} +/- {:.6}", p.k, p.sigma);
        for (label, r) in [
            ("RMC", p.rmc),
            ("MCNP T3", p.mcnp_t3),
            ("MCNP T4", p.mcnp_t4),
        ] {
            match r {
                Some(v) => {
                    let _ = writeln!(
                        tail,
                        "  {label:<8}     = {v:.6}   k - ref = {:+.0} +/- {:.0} pcm",
                        (p.k - v) * 1e5,
                        p.sigma * 1e5
                    );
                }
                None => {
                    let _ = writeln!(tail, "  {label:<8}     = none at this height");
                }
            }
        }
        let n_hist = p.histories.max(1) as f64;
        let _ = writeln!(
            tail,
            "  histories    = {} (planned {})",
            p.histories,
            stats.planned_histories()
        );
        let _ = writeln!(
            tail,
            "  lost locate  = {} ({:.3} %)",
            p.lost_locate,
            100.0 * p.lost_locate as f64 / n_hist
        );
        let _ = writeln!(tail, "  stuck events = {}", res.stuck_events);
        let _ = writeln!(tail, "  neg distance = {}", res.neg_dist);
        let _ = writeln!(
            tail,
            "  leak vacuum  = {} ({:.3} %)",
            res.leak_vacuum,
            100.0 * res.leak_vacuum as f64 / n_hist
        );
        if !res.entropy.is_empty() {
            let step = (res.entropy.len() / 12).max(1);
            let _ = write!(tail, "  entropy trace:");
            for (i, h) in res.entropy.iter().enumerate() {
                if i % step == 0 || i + 1 == res.entropy.len() {
                    let _ = write!(tail, " {h:.3}");
                }
            }
            let _ = writeln!(tail);
        }
        let _ = writeln!(
            tail,
            "  transport    = {:.1} s on {} threads ({})",
            p.transport_time.get::<uom::si::time::second>(),
            args.threads,
            prov.host
        );
        print!("{tail}");
        log.push_str(&tail);
        std::fs::write(args.out.join(format!("logs/N{n}.log")), log).expect("write height log");
        points.push(p);
        write_record(&args.out, &points, &prov, &args.layers);
    }

    let mut d = diag.render();
    let _ = writeln!(
        d,
        "\nwhole sweep wall clock: {:.1} s",
        t_all.elapsed().as_secs_f64()
    );
    std::fs::write(args.out.join("logs/run_diagnostics.md"), d).expect("write diagnostics");
    println!("\n{}", results_table_md(&points, &prov));
    println!(
        "whole sweep wall clock: {:.1} s",
        t_all.elapsed().as_secs_f64()
    );

    // Draw the figure if a Python with matplotlib is at hand; the script is the
    // record either way.
    let python = std::env::var("OUTRAM_PYTHON").unwrap_or_else(|_| "python3".into());
    let py = args.out.join("keff_vs_height.py");
    match Command::new(&python).arg(&py).status() {
        Ok(s) if s.success() => println!("figure: {}", py.with_extension("png").display()),
        Ok(_) => println!(
            "python FAILED drawing {}; the script is still the record",
            py.display()
        ),
        Err(_) => println!("no python ({python}); run {} by hand", py.display()),
    }
}
