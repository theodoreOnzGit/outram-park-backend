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

//! **Run an HTR-10 MGXS case** (gh:#845): the command a long run is started
//! with. Everything it does is in `nee_soon::htr10_mgxs`; this file is
//! command-line glue only.
//!
//! ```text
//! cargo run --release -p nee_soon --example htr10_mgxs_case -- --list
//! cargo run --release -p nee_soon --example htr10_mgxs_case -- --case smoke
//! cargo run --release -p nee_soon --example htr10_mgxs_case -- --case core_n12_t300
//! cargo run --release -p nee_soon --example htr10_mgxs_case -- --queue
//! ```
//!
//! - `--case <name>` runs one case; `--queue` runs every recorded case that
//!   has no `record.json` yet, in registry order, and stops at the first
//!   error.
//! - `--index` only rewrites `INDEX.md` from the records present (after a
//!   merge conflict in it, for instance).
//! - `--threads <n>` (default: every logical CPU) changes the wall clock only.
//! - `--out <dir>` overrides the folder. Recorded cases default to
//!   `crates/nee_soon/verification_and_validation/htr10_mgxs_runs/<case>/`;
//!   `smoke` to `target/htr10_mgxs_smoke/`, outside version control.
//!
//! Each case writes `record.json`, `summary.csv`, `README.md` and `run.log`
//! into its folder and rewrites the folder's `INDEX.md`. The last line
//! printed is always `RESULT ...` or `ERROR ...`. Exit code: 0 when every
//! check passed, 3 when the case finished with a failed check (the record is
//! still written and is still the result), 1 on an error, 2 on bad arguments.
//!
//! Functions here are untested command-line glue (the workspace's `main`/CLI
//! exception); everything they call is tested in `nee_soon::htr10_mgxs`.

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use nee_soon::htr10_mgxs::cases::MgxsCase;
use nee_soon::htr10_mgxs::{run_case, write_index, CaseRecord, Provenance};

/// CPU time per history measured on the full core with the hybrid tracker,
/// low and high \[ms\]: 14.4 on an i9-13900K
/// (`verification_and_validation/htr10_dem_bed_keff_2026_10_08/`) and about
/// 37 on a Xeon W-2195 (`htr10_seker_2026_10_07_10k/`, 7 500 s × 7 threads
/// for 1.4 M histories). Used only for the `--list` estimate.
const MS_PER_HISTORY: (f64, f64) = (14.4, 37.5);

/// Parsed command line. CLI glue.
struct Args {
    list: bool,
    queue: bool,
    index: bool,
    case: Option<String>,
    threads: usize,
    out: Option<PathBuf>,
}

/// Parse `std::env::args`, exiting with code 2 on anything unknown. CLI glue.
fn parse() -> Args {
    let mut a = Args {
        list: false,
        queue: false,
        index: false,
        case: None,
        threads: std::thread::available_parallelism().map_or(1, |n| n.get()),
        out: None,
    };
    let v: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;
    while i < v.len() {
        let val = v.get(i + 1).cloned();
        let need = |val: Option<String>, flag: &str| {
            val.unwrap_or_else(|| {
                eprintln!("{flag} needs a value");
                std::process::exit(2)
            })
        };
        match v[i].as_str() {
            "--list" => a.list = true,
            "--queue" => a.queue = true,
            "--index" => a.index = true,
            "--case" => {
                a.case = Some(need(val, "--case"));
                i += 1;
            }
            "--threads" => {
                a.threads = need(val, "--threads").parse().unwrap_or(0);
                i += 1;
            }
            "--out" => {
                a.out = Some(PathBuf::from(need(val, "--out")));
                i += 1;
            }
            other => {
                eprintln!("unknown argument {other:?}; see --list, --case <name>, --queue, --index, --threads <n>, --out <dir>");
                std::process::exit(2);
            }
        }
        i += 1;
    }
    if a.threads == 0 {
        eprintln!("--threads takes a positive integer");
        std::process::exit(2);
    }
    a
}

/// `git rev-parse --short=10 HEAD` in `dir`, `-dirty` if a tracked file
/// differs. CLI glue.
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
        Some(c) if run(&["status", "--porcelain", "--untracked-files=no"]).is_some_and(|s| !s.is_empty()) => {
            format!("{c}-dirty")
        }
        Some(c) => c,
        None => "unknown (not a git checkout)".to_string(),
    }
}

/// The V&V folder the recorded cases go under. CLI glue.
fn runs_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("verification_and_validation/htr10_mgxs_runs")
}

/// Where `case` writes. CLI glue.
fn out_dir(case: &MgxsCase, over: Option<&PathBuf>) -> PathBuf {
    match over {
        Some(p) => p.clone(),
        None if case.is_record => runs_root().join(case.name),
        None => PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/htr10_mgxs_smoke"),
    }
}

/// Print the registry with a cost estimate. CLI glue.
fn list(threads: usize) {
    println!("{:<24} {:>7} {:>22} {:>16}  purpose", "case", "T [K]", "budget per pass", "est. wall clock");
    for c in MgxsCase::all() {
        let cpu_s = |ms: f64| 2.0 * c.histories_per_pass() as f64 * ms / 1000.0;
        let (lo, hi) = (cpu_s(MS_PER_HISTORY.0) / threads as f64, cpu_s(MS_PER_HISTORY.1) / threads as f64);
        let done = if runs_root().join(c.name).join(CaseRecord::JSON).exists() { " [done]" } else { "" };
        println!(
            "{:<24} {:>7} {:>22} {:>7.1}-{:<5.1} h  {}{done}",
            c.name,
            c.temperature().get::<uom::si::thermodynamic_temperature::kelvin>(),
            format!("{} x [{} + {}]", c.particles, c.inactive, c.active),
            lo / 3600.0,
            hi / 3600.0,
            c.purpose
        );
    }
    println!("\nestimate: 2 passes x histories x {}-{} ms CPU per history / {threads} threads, plus a few minutes of nuclear data", MS_PER_HISTORY.0, MS_PER_HISTORY.1);
}

/// Run one case end to end and return its exit code. CLI glue.
fn run_one(case: &MgxsCase, threads: usize, over: Option<&PathBuf>) -> i32 {
    let dir = out_dir(case, over);
    if let Err(e) = std::fs::create_dir_all(&dir) {
        println!("ERROR {}: cannot create {}: {e}", case.name, dir.display());
        return 1;
    }
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let prov = Provenance {
        commit: git_commit(&manifest),
        ace_commit: git_commit(&manifest.join("../../reference-data/ace")),
    };
    let mut log = std::fs::File::create(dir.join("run.log")).ok();
    let t0 = Instant::now();
    let say = |line: &str| {
        let stamped = format!("[{:>7.0} s] {line}", t0.elapsed().as_secs_f64());
        println!("{stamped}");
        if let Some(f) = log.as_mut() {
            let _ = writeln!(f, "{stamped}");
        }
    };
    match run_case(case, threads, prov, say) {
        Ok(rec) => {
            if let Err(e) = rec.write(&dir) {
                println!("ERROR {}: the run finished but writing the record failed: {e}", case.name);
                return 1;
            }
            if case.is_record {
                if let Err(e) = write_index(&runs_root()) {
                    println!("warning: INDEX.md not rewritten: {e}");
                }
            }
            let (k, s) = rec.k();
            let ok = rec.all_checks_passed();
            for c in rec.checks.iter().filter(|c| !c.passed) {
                println!("FAILED CHECK {}: {}", c.name, c.detail);
            }
            println!(
                "RESULT {} k={k:.6}+/-{s:.6} checks={} wall={:.0}s out={}",
                case.name,
                if ok { "pass" } else { "FAIL" },
                rec.total_s,
                dir.display()
            );
            if ok {
                0
            } else {
                3
            }
        }
        Err(e) => {
            println!("ERROR {}: {e}", case.name);
            1
        }
    }
}

/// Entry point. CLI glue.
fn main() {
    let a = parse();
    if a.list {
        list(a.threads);
        return;
    }
    if a.index {
        match write_index(&runs_root()) {
            Ok(()) => println!("RESULT index rewritten: {}", runs_root().join("INDEX.md").display()),
            Err(e) => {
                println!("ERROR index: {e}");
                std::process::exit(1);
            }
        }
        return;
    }
    if a.queue {
        let todo: Vec<MgxsCase> = MgxsCase::all()
            .into_iter()
            .filter(|c| c.is_record && !runs_root().join(c.name).join(CaseRecord::JSON).exists())
            .collect();
        if todo.is_empty() {
            println!("RESULT queue empty: every recorded case has a record.json");
            return;
        }
        println!("queue: {}", todo.iter().map(|c| c.name).collect::<Vec<_>>().join(", "));
        let mut worst = 0;
        for c in &todo {
            let code = run_one(c, a.threads, None);
            if code == 1 {
                std::process::exit(1);
            }
            worst = worst.max(code);
        }
        std::process::exit(worst);
    }
    let Some(name) = a.case else {
        eprintln!("give --list, --case <name>, --queue or --index");
        std::process::exit(2);
    };
    let Some(case) = MgxsCase::find(&name) else {
        eprintln!("no case {name:?}; --list shows them");
        std::process::exit(2);
    };
    std::process::exit(run_one(&case, a.threads, a.out.as_ref()));
}
