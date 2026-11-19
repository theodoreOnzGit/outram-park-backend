//! `kovan-cli index-partition`, `index-plan` and `scip-merge`: the
//! partitioned semantic index (GitHub #1668; plan and measurements in
//! `crates/kovan/docs/partitioned-indexing.md`).
//!
//! ```text
//!  index-plan ──> plan.json (every member in exactly one partition)
//!      │
//!      ├─ index-partition --crates a,b ──> <out>/partition.scip + partition.json   (one per partition,
//!      ├─ index-partition --crates c   ──> …                                        separate processes
//!      │                                                                            or CI runners)
//!  scip-merge <out>... ──> merged.scip ──> call-graph --scip / index --scip (unchanged)
//! ```
//!
//! One partition ([`run_partition`]):
//!
//! 1. `cargo metadata --filter-platform <host>` and `cargo check --release
//!    --all-targets --message-format=json -p <members>` (build-script
//!    outputs and proc macros of the closure);
//! 2. a `rust-project.json` holding only the closure, only the members as
//!    workspace members ([`kovan_common::code_index::partition`]), written
//!    into a **view folder**: symlinks to the workspace's top-level entries,
//!    so concurrent workers never write into the checkout and document paths
//!    stay workspace-relative;
//! 3. `rust-analyzer scip <view> --exclude-vendored-libraries` (without the
//!    flag, every workspace dependency of the partition is indexed again as
//!    a "vendored" library), its peak resident memory sampled every second
//!    (Linux), stopped past the memory budget or the timeout;
//! 4. a check that the index holds documents of the members only, and a
//!    `partition.json` record.
//!
//! Unix only for now (the view folder is made of symlinks).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use kovan_common::code_index::partition::{plan, rust_project, BuildOutputs, Metadata, Partition};
use serde::{Deserialize, Serialize};

/// `partition.json`'s schema version.
pub const PARTITION_SCHEMA: u32 = 1;

/// How one partition run is bounded.
#[derive(Debug, Clone, Copy, Default)]
pub struct Limits {
    /// Stop rust-analyzer when its resident memory passes this (MiB; 0: none).
    pub memory_mib: u64,
    /// Stop it after this long (s; 0: none).
    pub timeout_s: u64,
}

/// `partition.json`: what one partition run did.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PartitionRecord {
    pub schema: u32,
    pub members: Vec<String>,
    /// Packages the closure loads.
    pub closure: usize,
    pub host: String,
    pub rustc: String,
    pub rust_analyzer: String,
    /// The cache key ([`partition_key`]).
    pub key: String,
    /// `cargo check`'s exit code (a failed check still indexes; build
    /// outputs or proc macros may then be missing).
    pub check_exit: Option<i32>,
    /// rust-analyzer's exit code; `None` when killed (see `stopped`).
    pub scip_exit: Option<i32>,
    /// Why kovan stopped it (memory budget, timeout), if it did.
    pub stopped: Option<String>,
    pub wall_s: f64,
    /// Peak resident memory of rust-analyzer (VmHWM, sampled; Linux).
    pub peak_rss_mib: Option<u64>,
    /// Documents written per member (by its folder).
    pub documents: BTreeMap<String, usize>,
    /// Documents outside every member's folder (should be none).
    pub outside_members: Vec<String>,
    /// Whether the index is complete for these members (exit 0, every
    /// member has documents, none outside).
    pub ok: bool,
}

fn capture(root: &Path, program: &str, args: &[&str]) -> Result<String, String> {
    let out = Command::new(program)
        .args(args)
        .current_dir(root)
        .output()
        .map_err(|e| format!("{program}: {e}"))?;
    if !out.status.success() {
        return Err(format!("{program} {}: {}", args.join(" "), String::from_utf8_lossy(&out.stderr).trim()));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// The host target triple, from `rustc -vV`.
pub fn host_triple(root: &Path) -> Result<String, String> {
    let v = capture(root, "rustc", &["-vV"])?;
    v.lines()
        .find_map(|l| l.strip_prefix("host: "))
        .map(str::to_string)
        .ok_or_else(|| "rustc -vV gave no host".into())
}

/// `cargo metadata` filtered to the host, as rust-analyzer's own loader asks.
pub fn metadata(root: &Path, host: &str) -> Result<Metadata, String> {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let json = capture(root, &cargo, &["metadata", "--format-version", "1", "--filter-platform", host])?;
    Metadata::parse(&json)
}

/// The cache key of a partition (GitHub #1672): its members' call-graph keys
/// (transitive over workspace dependencies, [`super::call_graph::crate_keys`]),
/// the `Cargo.lock` bytes (registry dependency versions), the host target,
/// rustc and rust-analyzer. Same inputs, same key.
pub fn partition_key(member_keys: &[(String, String)], cargo_lock: &[u8], host: &str, rustc: &str, ra: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    let mut keys = member_keys.to_vec();
    keys.sort();
    for (m, k) in keys {
        h.update(format!("{m} {k}\n"));
    }
    h.update(Sha256::digest(cargo_lock));
    h.update(format!("\n{host}\n{rustc}\n{ra}\nschema {PARTITION_SCHEMA}"));
    format!("{:x}", h.finalize())
}

/// [`partition_key`] for `members` of the workspace at `root`, gathering
/// its inputs (host, rustc, rust-analyzer, `Cargo.lock`, the members'
/// call-graph keys). Cheap next to indexing: no build.
pub fn partition_key_for(root: &Path, members: &[String]) -> Result<String, String> {
    let host = host_triple(root)?;
    let rustc = capture(root, "rustc", &["-V"])?;
    let ra = super::call_graph::rust_analyzer_version();
    let lock = std::fs::read(root.join("Cargo.lock")).unwrap_or_default();
    let mut m = members.to_vec();
    m.sort();
    m.dedup();
    Ok(partition_key(&super::call_graph::crate_keys(root, Some(m))?, &lock, &host, &rustc, &ra))
}

/// The workspace-relative folder of each member (its `Cargo.toml`'s folder).
fn member_dirs(meta: &Metadata, members: &[String]) -> BTreeMap<String, String> {
    members
        .iter()
        .filter_map(|m| {
            let p = meta.packages.iter().find(|p| &p.name == m && meta.workspace_members.contains(&p.id))?;
            let dir = p.manifest_path.rsplit_once('/').map_or("", |(d, _)| d);
            let rel = dir.strip_prefix(&meta.workspace_root).unwrap_or(dir).trim_start_matches('/');
            Some((m.clone(), rel.to_string()))
        })
        .collect()
}

/// Count `paths` per member folder; the rest are outside every member.
/// The longest matching folder wins (a member nested in another's folder).
pub fn documents_by_member(dirs: &BTreeMap<String, String>, paths: &[String]) -> (BTreeMap<String, usize>, Vec<String>) {
    let mut by: BTreeMap<String, usize> = dirs.keys().map(|m| (m.clone(), 0)).collect();
    let mut outside = Vec::new();
    for p in paths {
        let best = dirs
            .iter()
            .filter(|(_, d)| d.is_empty() || p.starts_with(&format!("{d}/")))
            .max_by_key(|(_, d)| d.len());
        match best {
            Some((m, _)) => *by.get_mut(m).unwrap() += 1,
            None => outside.push(p.clone()),
        }
    }
    (by, outside)
}

/// The view folder: `view/<entry>` → `root/<entry>` for every top-level
/// entry but `target` and `.git` (unix).
fn make_view(root: &Path, view: &Path) -> Result<(), String> {
    if view.exists() {
        std::fs::remove_dir_all(view).map_err(|e| format!("{}: {e}", view.display()))?;
    }
    std::fs::create_dir_all(view).map_err(|e| format!("{}: {e}", view.display()))?;
    for e in std::fs::read_dir(root).map_err(|e| format!("{}: {e}", root.display()))? {
        let e = e.map_err(|e| e.to_string())?;
        let name = e.file_name();
        if name == "target" || name == ".git" {
            continue;
        }
        #[cfg(unix)]
        std::os::unix::fs::symlink(e.path(), view.join(&name)).map_err(|err| format!("symlink {}: {err}", e.path().display()))?;
        #[cfg(not(unix))]
        return Err("index-partition needs symlinks (unix) for its view folder".into());
    }
    Ok(())
}

/// Peak and current resident memory of `pid` in MiB (Linux `/proc`).
fn rss_mib(pid: u32) -> Option<(u64, u64)> {
    let s = std::fs::read_to_string(format!("/proc/{pid}/status")).ok()?;
    let kb = |key: &str| {
        s.lines()
            .find_map(|l| l.strip_prefix(key))
            .and_then(|v| v.split_whitespace().next())
            .and_then(|v| v.parse::<u64>().ok())
    };
    Some((kb("VmHWM:")? / 1024, kb("VmRSS:")? / 1024))
}

/// Index one partition of `members` into `out` (module doc). Returns the
/// record, also written to `out/partition.json`. A cancel on `ctl` stops
/// rust-analyzer (recorded as stopped, never `ok`). I/O glue; reached by
/// `tests/index_partition_cli.rs`.
pub fn run_partition(
    root: &Path,
    members: &[String],
    out: &Path,
    limits: Limits,
    ctl: &super::index_control::RunControl,
) -> Result<PartitionRecord, String> {
    let started = Instant::now();
    let root = dunce::canonicalize(root).map_err(|e| format!("{}: {e}", root.display()))?;
    std::fs::create_dir_all(out).map_err(|e| format!("{}: {e}", out.display()))?;
    let out = dunce::canonicalize(out).map_err(|e| format!("{}: {e}", out.display()))?;
    let host = host_triple(&root)?;
    let rustc = capture(&root, "rustc", &["-V"])?;
    let ra = super::call_graph::rust_analyzer_version();
    let meta = metadata(&root, &host)?;
    let mut members = members.to_vec();
    members.sort();
    members.dedup();
    let closure = kovan_common::code_index::partition::closure(&meta, &members)?.len();
    let key = partition_key_for(&root, &members)?;

    eprintln!("index-partition: {} ({closure} packages in the closure)", members.join(", "));
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let mut check = Command::new(&cargo);
    check.current_dir(&root).args(["check", "--release", "--all-targets", "--message-format=json"]);
    for m in &members {
        check.args(["-p", m]);
    }
    let check_out = check.stderr(Stdio::inherit()).output().map_err(|e| format!("cargo check: {e}"))?;
    std::fs::write(out.join("check.jsonl"), &check_out.stdout).map_err(|e| e.to_string())?;
    let outputs = BuildOutputs::from_messages(&String::from_utf8_lossy(&check_out.stdout));

    let view = out.join("view");
    make_view(&root, &view)?;
    let sysroot = capture(&root, "rustc", &["--print", "sysroot"])?;
    let project = rust_project(&meta, &outputs, &members, &sysroot, Some(&view.to_string_lossy()))?;
    std::fs::write(view.join("rust-project.json"), serde_json::to_string_pretty(&project).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;

    let scip = out.join("partition.scip");
    let _ = std::fs::remove_file(&scip);
    let log = std::fs::File::create(out.join("rust-analyzer.log")).map_err(|e| e.to_string())?;
    let (program, _) = super::scip_stack::scip_program(&root);
    let mut cmd = Command::new(program);
    cmd.arg("scip").arg(&view).arg("--output").arg(&scip).arg("--exclude-vendored-libraries");
    cmd.current_dir(&view).stdout(log.try_clone().map_err(|e| e.to_string())?).stderr(log);
    super::scip_stack::raise_child_stack_limit(&mut cmd);
    let ra_started = Instant::now();
    let mut child = cmd.spawn().map_err(|e| format!("rust-analyzer scip: {e}"))?;
    let (mut peak, mut stopped) = (None::<u64>, None::<String>);
    let status = loop {
        if let Some(s) = child.try_wait().map_err(|e| e.to_string())? {
            break Some(s);
        }
        if let Some((hwm, now)) = rss_mib(child.id()) {
            peak = Some(peak.unwrap_or(0).max(hwm));
            if limits.memory_mib > 0 && now > limits.memory_mib {
                stopped = Some(format!("resident memory {now} MiB passed the budget of {} MiB", limits.memory_mib));
            }
        }
        if limits.timeout_s > 0 && ra_started.elapsed() > Duration::from_secs(limits.timeout_s) {
            stopped = Some(format!("still running after the timeout of {} s", limits.timeout_s));
        }
        if ctl.cancelled() {
            stopped = Some(super::index_control::CANCELLED.to_string());
        }
        if stopped.is_some() {
            let _ = child.kill();
            let _ = child.wait();
            break None;
        }
        std::thread::sleep(Duration::from_secs(1));
    };
    let scip_exit = status.and_then(|s| s.code());

    let paths: Vec<String> = match std::fs::read(&scip) {
        Ok(b) => crate::scip::ScipIndex::decode(&b)?.documents.iter().map(|d| d.path.clone()).collect(),
        Err(_) => Vec::new(),
    };
    let (documents, outside_members) = documents_by_member(&member_dirs(&meta, &members), &paths);
    let ok = scip_exit == Some(0) && outside_members.is_empty() && documents.values().all(|&n| n > 0);
    let record = PartitionRecord {
        schema: PARTITION_SCHEMA,
        members,
        closure,
        host,
        rustc,
        rust_analyzer: ra,
        key,
        check_exit: check_out.status.code(),
        scip_exit,
        stopped,
        wall_s: (started.elapsed().as_secs_f64() * 10.0).round() / 10.0,
        peak_rss_mib: peak,
        documents,
        outside_members,
        ok,
    };
    std::fs::write(out.join("partition.json"), serde_json::to_string_pretty(&record).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    let _ = std::fs::remove_dir_all(&view);
    Ok(record)
}

/// `plan.json`: the partitions and the budget they were planned for.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlanFile {
    pub host: String,
    pub max_closure: usize,
    pub max_members: usize,
    pub partitions: Vec<Partition>,
}

/// `kovan-cli index-plan`: plan every workspace member into partitions
/// whose closure stays within `max_closure` packages (where one member's
/// own closure is larger, it gets a partition alone, and that is listed).
pub fn run_plan(root: &Path, max_closure: usize, max_members: usize) -> Result<PlanFile, String> {
    let host = host_triple(root)?;
    let meta = metadata(root, &host)?;
    let partitions = plan(&meta, max_closure, max_members)?;
    for p in partitions.iter().filter(|p| p.closure > max_closure) {
        eprintln!(
            "index-plan: {} alone loads {} packages, over the budget of {max_closure}",
            p.members.join(", "),
            p.closure
        );
    }
    Ok(PlanFile { host, max_closure, max_members, partitions })
}

/// Local partitioned indexing settings (GitHub #1672, Phase F), from the
/// environment: `KOVAN_INDEX_JOBS` (concurrent workers, default 1: each
/// loads up to ~12 GiB on this workspace), `KOVAN_INDEX_MAX_CLOSURE` (750),
/// `KOVAN_INDEX_MAX_MEMBERS` (8), `KOVAN_INDEX_MEMORY_MIB` (0: none),
/// `KOVAN_INDEX_TIMEOUT` (s, 0: none), `KOVAN_INDEX_RETRIES` (1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocalSettings {
    pub jobs: usize,
    pub max_closure: usize,
    pub max_members: usize,
    pub memory_mib: u64,
    pub timeout_s: u64,
    pub retries: usize,
}

impl LocalSettings {
    /// Read from `get` (the environment in use; a closure in tests).
    pub fn from(get: impl Fn(&str) -> Option<String>) -> LocalSettings {
        let n = |k: &str, d: u64| get(k).and_then(|v| v.trim().parse::<u64>().ok()).unwrap_or(d);
        LocalSettings {
            jobs: n("KOVAN_INDEX_JOBS", 1).max(1) as usize,
            max_closure: n("KOVAN_INDEX_MAX_CLOSURE", 750) as usize,
            max_members: n("KOVAN_INDEX_MAX_MEMBERS", 8).max(1) as usize,
            memory_mib: n("KOVAN_INDEX_MEMORY_MIB", 0),
            timeout_s: n("KOVAN_INDEX_TIMEOUT", 0),
            retries: n("KOVAN_INDEX_RETRIES", 1) as usize,
        }
    }
}

/// Which indexer [`super::call_graph::generate_scip_controlled`] runs:
/// `KOVAN_INDEXER=partitioned` or `monolithic` (module doc; plan Phase G in
/// `crates/kovan/docs/partitioned-indexing.md`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Indexer {
    Partitioned,
    Monolithic,
}

impl Indexer {
    /// From `KOVAN_INDEXER`'s value; anything else is the default:
    /// partitioned on unix (plan Phase G, 2026-11-19), monolithic elsewhere,
    /// since the worker's view folder is made of symlinks (Windows is
    /// GitHub #1672's open item).
    pub fn from_value(v: Option<&str>) -> Indexer {
        match v.map(str::trim) {
            Some("monolithic") => Indexer::Monolithic,
            Some("partitioned") => Indexer::Partitioned,
            _ if cfg!(unix) => Indexer::Partitioned,
            _ => Indexer::Monolithic,
        }
    }
}

/// Whether a stored partition record can be reused for a run whose key is
/// `key`: complete and written for the same inputs (Phase E).
pub fn reusable(rec: &PartitionRecord, members: &[String], key: &str) -> bool {
    rec.ok && rec.key == key && rec.members == members
}

/// The local partitioned index (Phase F): plan, then index every partition
/// on `settings.jobs` worker threads (each runs `cargo check` and one
/// `rust-analyzer` process: memory is per process), reusing a partition
/// whose stored record has the same key, retrying a failed one, and merge
/// into `<root>/target/kovan-scip/index.scip`, where the monolithic run
/// writes, so everything downstream reads it unchanged. An incomplete set
/// is an error and nothing is written. I/O glue over tested parts (plan,
/// keys, [`reusable`], merge, coverage); reached by
/// `tests/index_partition_cli.rs`.
pub fn generate_partitioned(
    root: &Path,
    ctl: &super::index_control::RunControl,
    settings: LocalSettings,
) -> Result<PathBuf, String> {
    let dir = root.join("target").join("kovan-scip");
    let parts_dir = dir.join("parts");
    std::fs::create_dir_all(&parts_dir).map_err(|e| format!("{}: {e}", parts_dir.display()))?;
    let plan = run_plan(root, settings.max_closure, settings.max_members)?;
    let keys = partition_keys(root, &plan)?;
    ctl.say(format!(
        "index: partitioned rust-analyzer index, {} partitions, {} at a time (KOVAN_INDEXER=monolithic for one whole-workspace run)",
        plan.partitions.len(),
        settings.jobs
    ));
    let queue = std::sync::Mutex::new(plan.partitions.clone());
    let failures = std::sync::Mutex::new(Vec::<String>::new());
    let limits = Limits { memory_mib: settings.memory_mib, timeout_s: settings.timeout_s };
    std::thread::scope(|s| {
        for _ in 0..settings.jobs {
            s.spawn(|| loop {
                let Some(p) = queue.lock().ok().and_then(|mut q| if q.is_empty() { None } else { Some(q.remove(0)) }) else {
                    return;
                };
                if ctl.cancelled() {
                    return;
                }
                let out = parts_dir.join(&p.id);
                let key = keys.get(&p.id).cloned().unwrap_or_default();
                let stored: Option<PartitionRecord> = std::fs::read_to_string(out.join("partition.json"))
                    .ok()
                    .and_then(|t| serde_json::from_str(&t).ok());
                if stored.as_ref().is_some_and(|r| reusable(r, &p.members, &key)) && out.join("partition.scip").is_file() {
                    ctl.say(format!("index: {} unchanged, reused ({})", p.id, p.members.join(", ")));
                    continue;
                }
                let mut last = String::new();
                for attempt in 0..=settings.retries {
                    ctl.say(format!("index: {} ({}), attempt {}", p.id, p.members.join(", "), attempt + 1));
                    match run_partition(root, &p.members, &out, limits, ctl) {
                        Ok(r) if r.ok => {
                            ctl.say(format!(
                                "index: {} done in {:.0} s, peak {} MiB",
                                p.id,
                                r.wall_s,
                                r.peak_rss_mib.map_or("?".into(), |m| m.to_string())
                            ));
                            last.clear();
                            break;
                        }
                        Ok(r) => last = format!("{}: incomplete ({:?})", p.id, r.stopped),
                        Err(e) => last = format!("{}: {e}", p.id),
                    }
                    if ctl.cancelled() {
                        break;
                    }
                }
                if !last.is_empty() {
                    ctl.say(format!("index: {last}"));
                    if let Ok(mut f) = failures.lock() {
                        f.push(last);
                    }
                }
            });
        }
    });
    ctl.check()?;
    let failed = failures.into_inner().unwrap_or_default();
    if !failed.is_empty() {
        return Err(format!("partitioned index incomplete, nothing written: {}", failed.join("; ")));
    }
    let plan_path = dir.join("plan.json");
    std::fs::write(&plan_path, serde_json::to_string_pretty(&plan).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    let dirs: Vec<PathBuf> = plan.partitions.iter().map(|p| parts_dir.join(&p.id)).collect();
    let out = dir.join("index.scip");
    run_merge(root, &dirs, Some(&plan_path), &out, false)?;
    Ok(out)
}

/// The plan as a GitHub Actions matrix:
/// `{"include":[{"id":"p00","crates":"a,b","key":"…"},…]}`, each partition
/// with its cache key from `keys` (by partition id; empty when absent).
pub fn matrix_json(plan: &PlanFile, keys: &BTreeMap<String, String>) -> String {
    let include: Vec<serde_json::Value> = plan
        .partitions
        .iter()
        .map(|p| {
            let key = keys.get(&p.id).cloned().unwrap_or_default();
            serde_json::json!({ "id": p.id, "crates": p.members.join(","), "key": key })
        })
        .collect();
    serde_json::json!({ "include": include }).to_string()
}

/// Every partition's cache key ([`partition_key`]), by partition id, with the
/// members' call-graph keys computed once for the whole workspace. I/O
/// glue over [`partition_key`] (tested) and `crate_keys`; reached by
/// `tests/index_partition_cli.rs`.
pub fn partition_keys(root: &Path, plan: &PlanFile) -> Result<BTreeMap<String, String>, String> {
    let rustc = capture(root, "rustc", &["-V"])?;
    let ra = super::call_graph::rust_analyzer_version();
    let lock = std::fs::read(root.join("Cargo.lock")).unwrap_or_default();
    let all: BTreeMap<String, String> = super::call_graph::crate_keys(root, None)?.into_iter().collect();
    Ok(plan
        .partitions
        .iter()
        .map(|p| {
            let keys: Vec<(String, String)> =
                p.members.iter().filter_map(|m| all.get(m).map(|k| (m.clone(), k.clone()))).collect();
            (p.id.clone(), partition_key(&keys, &lock, &plan.host, &rustc, &ra))
        })
        .collect())
}

/// What `scip-merge` found about coverage.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Coverage {
    /// Planned members with no successful partition.
    pub missing: Vec<String>,
    /// Partitions whose run was not complete (`ok = false`), by members.
    pub failed: Vec<String>,
}

impl Coverage {
    pub fn complete(&self) -> bool {
        self.missing.is_empty() && self.failed.is_empty()
    }
}

/// The coverage of `records` against the members of `plan` (or, with no
/// plan, of the records themselves).
pub fn coverage(plan: Option<&PlanFile>, records: &[PartitionRecord]) -> Coverage {
    let done: std::collections::BTreeSet<&str> =
        records.iter().filter(|r| r.ok).flat_map(|r| r.members.iter().map(String::as_str)).collect();
    let wanted: Vec<String> = match plan {
        Some(p) => p.partitions.iter().flat_map(|p| p.members.clone()).collect(),
        None => records.iter().flat_map(|r| r.members.clone()).collect(),
    };
    let mut missing: Vec<String> = wanted.into_iter().filter(|m| !done.contains(m.as_str())).collect();
    missing.sort();
    missing.dedup();
    Coverage {
        missing,
        failed: records.iter().filter(|r| !r.ok).map(|r| r.members.join(",")).collect(),
    }
}

/// `kovan-cli scip-merge`: merge the partitions in `dirs` (each holding
/// `partition.json` and `partition.scip`) into `out`, after checking
/// coverage against `plan`. An incomplete merge is refused unless
/// `allow_incomplete`, and then said loudly; it is never written as
/// complete.
pub fn run_merge(root: &Path, dirs: &[PathBuf], plan: Option<&Path>, out: &Path, allow_incomplete: bool) -> Result<Coverage, String> {
    let plan: Option<PlanFile> = match plan {
        Some(p) => Some(
            serde_json::from_str(&std::fs::read_to_string(p).map_err(|e| format!("{}: {e}", p.display()))?)
                .map_err(|e| format!("{}: {e}", p.display()))?,
        ),
        None => None,
    };
    let mut records = Vec::new();
    let mut parts = Vec::new();
    for d in dirs {
        let rec: PartitionRecord = serde_json::from_str(
            &std::fs::read_to_string(d.join("partition.json")).map_err(|e| format!("{}: {e}", d.display()))?,
        )
        .map_err(|e| format!("{}: {e}", d.display()))?;
        if rec.ok {
            parts.push(std::fs::read(d.join("partition.scip")).map_err(|e| format!("{}: {e}", d.display()))?);
        }
        records.push(rec);
    }
    let cov = coverage(plan.as_ref(), &records);
    if !cov.complete() {
        let msg = format!("scip-merge: INCOMPLETE: missing {:?}; failed partitions {:?}", cov.missing, cov.failed);
        if !allow_incomplete {
            return Err(msg);
        }
        eprintln!("{msg}");
    }
    let root = dunce::canonicalize(root).map_err(|e| format!("{}: {e}", root.display()))?;
    let uri = format!("file://{}", root.to_string_lossy());
    let (bytes, report) = crate::scip::merge(&parts, &uri)?;
    std::fs::write(out, bytes).map_err(|e| format!("{}: {e}", out.display()))?;
    eprintln!(
        "scip-merge: {} partitions, {} documents, {} external symbols, rust-analyzer {} -> {}",
        parts.len(),
        report.documents.iter().sum::<usize>(),
        report.external_symbols,
        report.tool_version,
        out.display()
    );
    Ok(cov)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Settings come from the environment with defaults; jobs and members
    /// are at least one. The indexer is partitioned unless `monolithic`.
    #[test]
    fn local_settings_and_indexer_choice() {
        let none = LocalSettings::from(|_| None);
        assert_eq!(none, LocalSettings { jobs: 1, max_closure: 750, max_members: 8, memory_mib: 0, timeout_s: 0, retries: 1 });
        let set = LocalSettings::from(|k| match k {
            "KOVAN_INDEX_JOBS" => Some("0".into()),
            "KOVAN_INDEX_MEMORY_MIB" => Some(" 9000 ".into()),
            "KOVAN_INDEX_MAX_CLOSURE" => Some("junk".into()),
            _ => None,
        });
        assert_eq!((set.jobs, set.memory_mib, set.max_closure), (1, 9000, 750));
        let default = if cfg!(unix) { Indexer::Partitioned } else { Indexer::Monolithic };
        assert_eq!(Indexer::from_value(None), default);
        assert_eq!(Indexer::from_value(Some("monolithic")), Indexer::Monolithic);
        assert_eq!(Indexer::from_value(Some("partitioned")), Indexer::Partitioned);
    }

    /// Documents are counted under the longest matching member folder; a
    /// path under no member is listed as outside.
    #[test]
    fn documents_are_counted_per_member_folder() {
        let dirs = BTreeMap::from([
            ("a".to_string(), "crates/a".to_string()),
            ("a-inner".to_string(), "crates/a/inner".to_string()),
        ]);
        let paths = ["crates/a/src/lib.rs", "crates/a/inner/src/lib.rs", "crates/ab/src/lib.rs"].map(String::from);
        let (by, outside) = documents_by_member(&dirs, &paths);
        assert_eq!(by["a"], 1);
        assert_eq!(by["a-inner"], 1);
        assert_eq!(outside, ["crates/ab/src/lib.rs"]);
    }

    /// The key changes with any input and not with member order.
    #[test]
    fn the_partition_key_covers_every_input() {
        let k = |keys: &[(&str, &str)], lock: &[u8], host: &str| {
            let keys: Vec<(String, String)> = keys.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect();
            partition_key(&keys, lock, host, "rustc 1.99.1", "rust-analyzer 1.99.1")
        };
        let base = k(&[("a", "1"), ("b", "2")], b"lock", "x86_64-unknown-linux-gnu");
        assert_eq!(base, k(&[("b", "2"), ("a", "1")], b"lock", "x86_64-unknown-linux-gnu"));
        assert_ne!(base, k(&[("a", "1"), ("b", "3")], b"lock", "x86_64-unknown-linux-gnu"));
        assert_ne!(base, k(&[("a", "1"), ("b", "2")], b"lock2", "x86_64-unknown-linux-gnu"));
        assert_ne!(base, k(&[("a", "1"), ("b", "2")], b"lock", "wasm32-unknown-unknown"));
    }

    /// Coverage: every planned member must be in a successful partition; a
    /// failed one is listed, and its members are missing.
    #[test]
    fn coverage_lists_missing_members_and_failed_partitions() {
        let rec = |m: &[&str], ok: bool| PartitionRecord { members: m.iter().map(|s| s.to_string()).collect(), ok, ..Default::default() };
        let part = |id: &str, m: &[&str]| Partition { id: id.into(), members: m.iter().map(|s| s.to_string()).collect(), closure: 1 };
        let plan = PlanFile { host: "h".into(), max_closure: 1, max_members: 1, partitions: vec![part("p00", &["a"]), part("p01", &["b", "c"])] };
        let c = coverage(Some(&plan), &[rec(&["a"], true), rec(&["b", "c"], false)]);
        assert_eq!(c.missing, ["b", "c"]);
        assert_eq!(c.failed, ["b,c"]);
        assert!(!c.complete());
        assert!(coverage(Some(&plan), &[rec(&["a"], true), rec(&["b", "c"], true)]).complete());
        assert_eq!(coverage(Some(&plan), &[rec(&["a"], true)]).missing, ["b", "c"]);
        let keys = BTreeMap::from([("p00".to_string(), "k0".to_string())]);
        let stored = rec(&["a"], true);
        let stored = PartitionRecord { key: "k".into(), ..stored };
        assert!(reusable(&stored, &["a".to_string()], "k"));
        assert!(!reusable(&stored, &["a".to_string()], "other key"));
        assert!(!reusable(&stored, &["a".to_string(), "b".to_string()], "k"));
        assert!(!reusable(&PartitionRecord { ok: false, ..stored.clone() }, &["a".to_string()], "k"));
        assert_eq!(
            matrix_json(&plan, &keys),
            r#"{"include":[{"crates":"a","id":"p00","key":"k0"},{"crates":"b,c","id":"p01","key":""}]}"#
        );
    }
}
