//! **Partitioned semantic indexing** (GitHub #1668, #1669, #1671): index a
//! group of workspace members without analysing the whole workspace.
//!
//! The monolithic step was one `rust-analyzer scip` over the whole
//! workspace, about 16 GiB at peak. Kovan uses its index only to resolve
//! reference positions, by SCIP symbol string across every document
//! (`kovan::scip`), so indexes written for disjoint groups of crates merge
//! as a union of their documents. This module is the pure half of one
//! partition: from `cargo metadata` and the partition's `cargo check
//! --message-format=json` output it writes a `rust-project.json` holding
//! **only the partition's dependency closure**, with **only its members**
//! marked as workspace members. `rust-analyzer scip
//! --exclude-vendored-libraries` over that project then writes documents
//! for the members alone (`crates/kovan/docs/partitioned-indexing.md`).
//!
//! It also plans the partitions ([`plan`]): every workspace member is put in
//! exactly one, so coverage is complete by construction and checked again at
//! the merge.
//!
//! **Faithful to rust-analyzer's own cargo loader** where it matters:
//!
//! - cfgs are the resolved features (as `cargo metadata` unifies them over
//!   the workspace, which is what the monolithic run saw) plus the build
//!   script's `cargo:rustc-cfg`; `test` is set on members only, as
//!   rust-analyzer sets it on workspace members;
//! - env is Cargo's `CARGO_PKG_*` set, `OUT_DIR` and the build script's
//!   `cargo:rustc-env`;
//! - a crate's include dirs are its **package root** and its `OUT_DIR`
//!   (~~its root module's folder~~ corrected 2026-11-19: a `#[path]` module
//!   elsewhere in the package, `boon-lay`'s tests reaching
//!   `verification_and_validation/`, was not loaded); a member's targets also
//!   include every folder **outside** its package that its `#[path]`
//!   attributes reach ([`external_path_dirs`]; `dhoby-ghaut`'s examples load
//!   files of `outram-mc-libs/examples/common/`), so those files are indexed
//!   in the member's context as the monolithic run did;
//! - proc macros point at the dylib `cargo check` built;
//! - the metadata must be `--filter-platform <host>`, as rust-analyzer's
//!   loader asks for: unfiltered, a Linux closure also held `windows`,
//!   `objc2` and `web-sys` crates (83 of the `kovan` closure, 2026-11-19).
//!
//! Pure: no I/O, no processes. The worker is `kovan-cli index-partition`.

use std::collections::{BTreeMap, BTreeSet};

use serde::Deserialize;
use serde_json::{json, Value};

/// The parts of `cargo metadata --format-version 1` used here.
#[derive(Debug, Clone, Deserialize)]
pub struct Metadata {
    pub packages: Vec<Package>,
    pub workspace_members: Vec<String>,
    pub resolve: Resolve,
    pub workspace_root: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Package {
    pub id: String,
    pub name: String,
    pub version: String,
    pub manifest_path: String,
    pub targets: Vec<Target>,
    #[serde(default)]
    pub authors: Vec<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub license: Option<String>,
    #[serde(default)]
    pub repository: Option<String>,
    #[serde(default)]
    pub homepage: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Target {
    pub name: String,
    pub kind: Vec<String>,
    pub src_path: String,
    pub edition: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Resolve {
    pub nodes: Vec<Node>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Node {
    pub id: String,
    #[serde(default)]
    pub deps: Vec<NodeDep>,
    #[serde(default)]
    pub features: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NodeDep {
    pub name: String,
    pub pkg: String,
    #[serde(default)]
    pub dep_kinds: Vec<DepKind>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DepKind {
    /// `None` normal, `"dev"`, `"build"`.
    pub kind: Option<String>,
}

impl Metadata {
    pub fn parse(text: &str) -> Result<Metadata, String> {
        serde_json::from_str(text).map_err(|e| format!("cargo metadata: {e}"))
    }

    fn package(&self, id: &str) -> Option<&Package> {
        self.packages.iter().find(|p| p.id == id)
    }

    fn node(&self, id: &str) -> Option<&Node> {
        self.resolve.nodes.iter().find(|n| n.id == id)
    }

    /// The package id of workspace member `name`.
    pub fn member_id(&self, name: &str) -> Result<String, String> {
        self.workspace_members
            .iter()
            .find(|id| self.package(id).is_some_and(|p| p.name == name))
            .cloned()
            .ok_or_else(|| format!("{name} is not a workspace member"))
    }

    /// The names of every workspace member, sorted.
    pub fn member_names(&self) -> Vec<String> {
        let mut v: Vec<String> = self
            .workspace_members
            .iter()
            .filter_map(|id| self.package(id).map(|p| p.name.clone()))
            .collect();
        v.sort();
        v
    }

    /// `(extern name, package id)` of `id`'s dependencies of the given kinds.
    fn deps_of(&self, id: &str, dev: bool) -> Vec<(String, String)> {
        let Some(n) = self.node(id) else { return Vec::new() };
        n.deps
            .iter()
            .filter(|d| {
                d.dep_kinds
                    .iter()
                    .any(|k| k.kind.is_none() || (dev && k.kind.as_deref() == Some("dev")))
            })
            .map(|d| (d.name.clone(), d.pkg.clone()))
            .collect()
    }
}

/// The package ids a partition of `members` loads: the members, their normal
/// and dev dependencies, and those dependencies' normal dependencies,
/// transitively. Build dependencies are not loaded (a build script's own
/// dependencies are compiled by `cargo check`, never analysed).
pub fn closure(meta: &Metadata, members: &[String]) -> Result<BTreeSet<String>, String> {
    let mut need = BTreeSet::new();
    let mut stack = Vec::new();
    for m in members {
        let id = meta.member_id(m)?;
        stack.extend(meta.deps_of(&id, true).into_iter().map(|(_, p)| p));
        stack.push(id);
    }
    while let Some(p) = stack.pop() {
        if need.insert(p.clone()) {
            stack.extend(meta.deps_of(&p, false).into_iter().map(|(_, q)| q));
        }
    }
    Ok(need)
}

/// What `cargo check --message-format=json` reports that rust-analyzer needs:
/// each package's build-script `OUT_DIR`, cfgs and env, and each proc
/// macro's dylib.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BuildOutputs {
    pub out_dir: BTreeMap<String, String>,
    pub cfgs: BTreeMap<String, Vec<String>>,
    pub env: BTreeMap<String, Vec<(String, String)>>,
    pub proc_macro_dylib: BTreeMap<String, String>,
}

impl BuildOutputs {
    /// Read the JSON lines of `cargo check --message-format=json`; lines
    /// that are not a message (cargo's human output) are skipped.
    pub fn from_messages(text: &str) -> BuildOutputs {
        let mut b = BuildOutputs::default();
        for line in text.lines().filter(|l| l.starts_with('{')) {
            let Ok(m) = serde_json::from_str::<Value>(line) else { continue };
            let Some(pid) = m["package_id"].as_str() else { continue };
            match m["reason"].as_str() {
                Some("build-script-executed") => {
                    if let Some(d) = m["out_dir"].as_str() {
                        b.out_dir.insert(pid.into(), d.into());
                    }
                    let cfgs = m["cfgs"].as_array().into_iter().flatten();
                    b.cfgs.insert(pid.into(), cfgs.filter_map(|c| c.as_str().map(String::from)).collect());
                    let env = m["env"].as_array().into_iter().flatten().filter_map(|kv| {
                        Some((kv[0].as_str()?.to_string(), kv[1].as_str()?.to_string()))
                    });
                    b.env.insert(pid.into(), env.collect());
                }
                Some("compiler-artifact") => {
                    let kinds = m["target"]["kind"].as_array().into_iter().flatten();
                    if kinds.filter_map(|k| k.as_str()).any(|k| k == "proc-macro") {
                        if let Some(f) = m["filenames"][0].as_str() {
                            b.proc_macro_dylib.insert(pid.into(), f.into());
                        }
                    }
                }
                _ => {}
            }
        }
        b
    }
}

const LIB_KINDS: [&str; 6] = ["lib", "rlib", "dylib", "cdylib", "staticlib", "proc-macro"];
/// A member's other targets, each its own crate in rust-analyzer as in the
/// monolithic run: integration tests, examples, binaries and benches.
const OTHER_KINDS: [&str; 4] = ["test", "example", "bin", "bench"];

fn lib_target(p: &Package) -> Option<&Target> {
    p.targets.iter().find(|t| t.kind.iter().any(|k| LIB_KINDS.contains(&k.as_str())))
}

/// `path` with the workspace root replaced by `view` (the worker's view
/// directory), so document paths stay workspace-relative; paths outside the
/// workspace are kept.
pub fn in_view(workspace_root: &str, view: Option<&str>, path: &str) -> String {
    match (view, path.strip_prefix(workspace_root)) {
        (Some(v), Some(rest)) if rest.starts_with('/') => format!("{v}{rest}"),
        _ => path.to_string(),
    }
}

fn parent(path: &str) -> &str {
    path.rsplit_once('/').map_or(path, |(d, _)| d)
}

fn env_of(p: &Package, t: &Target, b: &BuildOutputs) -> BTreeMap<String, String> {
    let mut v = p.version.split(['.', '-', '+']);
    let mut e = BTreeMap::new();
    let mut set = |k: &str, val: String| {
        e.insert(k.to_string(), val);
    };
    set("CARGO", "cargo".into());
    set("CARGO_PKG_NAME", p.name.clone());
    set("CARGO_PKG_VERSION", p.version.clone());
    set("CARGO_PKG_VERSION_MAJOR", v.next().unwrap_or("0").into());
    set("CARGO_PKG_VERSION_MINOR", v.next().unwrap_or("0").into());
    set("CARGO_PKG_VERSION_PATCH", v.next().unwrap_or("0").into());
    set("CARGO_PKG_VERSION_PRE", p.version.split_once('-').map_or("", |(_, pre)| pre).into());
    set("CARGO_PKG_AUTHORS", p.authors.join(":"));
    set("CARGO_PKG_DESCRIPTION", p.description.clone().unwrap_or_default());
    set("CARGO_PKG_LICENSE", p.license.clone().unwrap_or_default());
    set("CARGO_PKG_REPOSITORY", p.repository.clone().unwrap_or_default());
    set("CARGO_PKG_HOMEPAGE", p.homepage.clone().unwrap_or_default());
    set("CARGO_MANIFEST_DIR", parent(&p.manifest_path).into());
    set("CARGO_CRATE_NAME", t.name.replace('-', "_"));
    if let Some(d) = b.out_dir.get(&p.id) {
        set("OUT_DIR", d.clone());
    }
    for (k, val) in b.env.get(&p.id).into_iter().flatten() {
        set(k, val.clone());
    }
    e
}

/// Folders outside `package_dir` that `#[path = "…"]` attributes in `files`
/// (`(absolute path, text)` of the package's `.rs` files) reach, sorted and
/// de-duplicated. A path is taken relative to its file's folder (the rule
/// for a declaration outside inline modules,
/// [`crate::call_graph::modules`]), normalised lexically.
pub fn external_path_dirs(package_dir: &str, files: &[(String, String)]) -> Vec<String> {
    let mut out = BTreeSet::new();
    for (file, text) in files {
        for line in text.lines() {
            let t = line.trim_start();
            let Some(rest) = t.strip_prefix("#[path") else { continue };
            let Some(q) = rest.split('"').nth(1) else { continue };
            let joined = format!("{}/{q}", parent(file));
            let mut parts: Vec<&str> = Vec::new();
            for seg in joined.split('/') {
                match seg {
                    "" | "." => {}
                    ".." => {
                        parts.pop();
                    }
                    s => parts.push(s),
                }
            }
            let resolved = format!("/{}", parts.join("/"));
            let dir = parent(&resolved).to_string();
            if dir != package_dir && !dir.starts_with(&format!("{package_dir}/")) {
                out.insert(dir);
            }
        }
    }
    out.into_iter().collect()
}

/// The `rust-project.json` for a partition of `members` (module doc).
/// `sysroot` is `rustc --print sysroot`; `view` the worker's view directory
/// (`None`: the workspace root itself); `external` the folders outside each
/// member's package that its `#[path]` attributes reach, by member name
/// ([`external_path_dirs`]).
pub fn rust_project(
    meta: &Metadata,
    outputs: &BuildOutputs,
    members: &[String],
    sysroot: &str,
    view: Option<&str>,
    external: &BTreeMap<String, Vec<String>>,
) -> Result<Value, String> {
    let need = closure(meta, members)?;
    let member_ids: BTreeSet<String> = members.iter().map(|m| meta.member_id(m)).collect::<Result<_, _>>()?;
    let root = meta.workspace_root.as_str();
    let mut order: Vec<&Package> = need.iter().filter_map(|id| meta.package(id)).collect();
    order.sort_by(|a, b| (&a.name, &a.version, &a.id).cmp(&(&b.name, &b.version, &b.id)));

    let crate_json = |p: &Package, t: &Target, member: bool| {
        let features = meta.node(&p.id).map(|n| n.features.clone()).unwrap_or_default();
        let mut cfg: Vec<String> = features.iter().map(|f| format!("feature=\"{f}\"")).collect();
        cfg.extend(outputs.cfgs.get(&p.id).cloned().unwrap_or_default());
        if member {
            cfg.push("test".into());
        }
        let root_module = in_view(root, view, &t.src_path);
        let mut include = vec![in_view(root, view, parent(&p.manifest_path))];
        if member {
            include.extend(external.get(&p.name).into_iter().flatten().map(|d| in_view(root, view, d)));
        }
        include.extend(outputs.out_dir.get(&p.id).cloned());
        let is_proc_macro = t.kind.iter().any(|k| k == "proc-macro");
        let mut c = json!({
            "display_name": p.name,
            "version": p.version,
            "root_module": root_module,
            "edition": t.edition,
            "deps": [],
            "cfg": cfg,
            "env": env_of(p, t, outputs),
            "is_workspace_member": member,
            "is_proc_macro": is_proc_macro,
            "source": { "include_dirs": include, "exclude_dirs": [] },
        });
        if is_proc_macro {
            if let Some(d) = outputs.proc_macro_dylib.get(&p.id) {
                c["proc_macro_dylib_path"] = json!(d);
            }
        }
        c
    };

    let mut crates: Vec<Value> = Vec::new();
    let mut lib_index: BTreeMap<&str, usize> = BTreeMap::new();
    for p in &order {
        if let Some(t) = lib_target(p) {
            lib_index.insert(p.id.as_str(), crates.len());
            crates.push(crate_json(p, t, member_ids.contains(&p.id)));
        }
    }
    let deps_json = |id: &str, dev: bool| -> Vec<Value> {
        meta.deps_of(id, dev)
            .into_iter()
            .filter_map(|(name, pkg)| lib_index.get(pkg.as_str()).map(|&i| json!({ "crate": i, "name": name })))
            .collect()
    };
    for p in &order {
        if let Some(&i) = lib_index.get(p.id.as_str()) {
            crates[i]["deps"] = json!(deps_json(&p.id, member_ids.contains(&p.id)));
        }
    }
    for p in order.iter().filter(|p| member_ids.contains(&p.id)) {
        for t in p.targets.iter().filter(|t| t.kind.iter().any(|k| OTHER_KINDS.contains(&k.as_str()))) {
            let mut deps = deps_json(&p.id, true);
            if let (Some(&i), Some(lib)) = (lib_index.get(p.id.as_str()), lib_target(p)) {
                deps.push(json!({ "crate": i, "name": lib.name.replace('-', "_") }));
            }
            let mut c = crate_json(p, t, true);
            c["deps"] = json!(deps);
            crates.push(c);
        }
    }
    Ok(json!({
        "sysroot": sysroot,
        "sysroot_src": format!("{sysroot}/lib/rustlib/src/rust/library"),
        "crates": crates,
    }))
}

/// One partition of the plan ([`plan`]).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, Deserialize)]
pub struct Partition {
    /// `p00`, `p01`, … in plan order.
    pub id: String,
    /// Workspace members, sorted.
    pub members: Vec<String>,
    /// Packages its closure loads (the cost estimate: memory grows with the
    /// closure, not with the members' size; measured 2026-11-19).
    pub closure: usize,
}

/// Every workspace member in exactly one partition. Members are taken
/// largest closure first (then by name) and each joins the first partition
/// whose closure, united with its own, stays within `max_closure` packages
/// and which has fewer than `max_members`; otherwise it starts a new one.
/// A member whose own closure exceeds `max_closure` gets a partition of its
/// own (reported by the caller, never dropped). Deterministic.
pub fn plan(meta: &Metadata, max_closure: usize, max_members: usize) -> Result<Vec<Partition>, String> {
    let mut members: Vec<(BTreeSet<String>, String)> = meta
        .member_names()
        .into_iter()
        .map(|m| closure(meta, std::slice::from_ref(&m)).map(|c| (c, m)))
        .collect::<Result<_, _>>()?;
    members.sort_by(|a, b| b.0.len().cmp(&a.0.len()).then_with(|| a.1.cmp(&b.1)));
    let mut groups: Vec<(BTreeSet<String>, Vec<String>)> = Vec::new();
    for (c, m) in members {
        let fits = groups.iter_mut().find(|(gc, gm)| {
            gm.len() < max_members.max(1) && gc.union(&c).count() <= max_closure
        });
        match fits {
            Some((gc, gm)) => {
                gc.extend(c);
                gm.push(m);
            }
            None => groups.push((c, vec![m])),
        }
    }
    Ok(groups
        .into_iter()
        .enumerate()
        .map(|(i, (c, mut m))| {
            m.sort();
            Partition { id: format!("p{i:02}"), members: m, closure: c.len() }
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A three-member workspace: `app` depends on `core` (and dev-depends
    /// on `serde_like`), `core` on the registry crate `itoa_like`, `lone` on
    /// nothing; `pm` is a proc macro `app` uses.
    fn meta() -> Metadata {
        let pkg = |id: &str, name: &str, path: &str, kinds: &[&str]| {
            json!({
                "id": id, "name": name, "version": "1.2.3-rc.1",
                "manifest_path": format!("{path}/Cargo.toml"),
                "targets": [{ "name": name, "kind": kinds, "src_path": format!("{path}/src/lib.rs"), "edition": "2021" }],
            })
        };
        let mut app = pkg("app#1", "app", "/ws/crates/app", &["lib"]);
        app["targets"].as_array_mut().unwrap().push(json!({
            "name": "it", "kind": ["test"], "src_path": "/ws/crates/app/tests/it.rs", "edition": "2021"
        }));
        let dep = |name: &str, pkg: &str, kind: Option<&str>| json!({ "name": name, "pkg": pkg, "dep_kinds": [{ "kind": kind }] });
        Metadata::parse(
            &json!({
                "workspace_root": "/ws",
                "workspace_members": ["app#1", "core#1", "lone#1"],
                "packages": [
                    app,
                    pkg("core#1", "core", "/ws/crates/core", &["lib"]),
                    pkg("lone#1", "lone", "/ws/crates/lone", &["lib"]),
                    pkg("itoa#1", "itoa_like", "/reg/itoa", &["lib"]),
                    pkg("serde#1", "serde_like", "/reg/serde", &["lib"]),
                    pkg("pm#1", "pm", "/reg/pm", &["proc-macro"]),
                ],
                "resolve": { "nodes": [
                    { "id": "app#1", "features": ["fast"], "deps": [dep("core", "core#1", None), dep("serde_like", "serde#1", Some("dev")), dep("pm", "pm#1", None)] },
                    { "id": "core#1", "deps": [dep("itoa_like", "itoa#1", None)] },
                    { "id": "lone#1", "deps": [] },
                    { "id": "itoa#1", "deps": [] },
                    { "id": "serde#1", "deps": [] },
                    { "id": "pm#1", "deps": [] },
                ]},
            })
            .to_string(),
        )
        .unwrap()
    }

    /// The closure holds the members, their dev dependencies and the normal
    /// dependencies below; a dev dependency of a non-member is not loaded.
    #[test]
    fn the_closure_is_members_dev_deps_and_normal_deps_below() {
        let m = meta();
        let ids = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<BTreeSet<_>>();
        assert_eq!(closure(&m, &["app".into()]).unwrap(), ids(&["app#1", "core#1", "itoa#1", "pm#1", "serde#1"]));
        assert_eq!(closure(&m, &["core".into()]).unwrap(), ids(&["core#1", "itoa#1"]));
        assert!(closure(&m, &["nope".into()]).is_err());
    }

    /// Build outputs: OUT_DIR, cfgs and env from a build script, a proc
    /// macro's dylib; other lines skipped.
    #[test]
    fn build_outputs_are_read_from_cargo_messages() {
        let text = [
            r#"{"reason":"build-script-executed","package_id":"core#1","out_dir":"/t/out","cfgs":["has_x"],"env":[["K","v"]]}"#,
            r#"{"reason":"compiler-artifact","package_id":"pm#1","target":{"kind":["proc-macro"]},"filenames":["/t/libpm.so"]}"#,
            r#"{"reason":"compiler-artifact","package_id":"core#1","target":{"kind":["lib"]},"filenames":["/t/libcore.rmeta"]}"#,
            "   Compiling core",
        ]
        .join("\n");
        let b = BuildOutputs::from_messages(&text);
        assert_eq!(b.out_dir["core#1"], "/t/out");
        assert_eq!(b.cfgs["core#1"], ["has_x"]);
        assert_eq!(b.env["core#1"], [("K".to_string(), "v".to_string())]);
        assert_eq!(b.proc_macro_dylib, BTreeMap::from([("pm#1".to_string(), "/t/libpm.so".to_string())]));
    }

    /// The project for `app`: only its closure; only `app`'s targets are
    /// workspace members, with `test` cfg and their dev dependency; `core`
    /// is a library crate under the view; build outputs, features, env and
    /// the proc macro's dylib are carried.
    #[test]
    fn the_project_holds_only_the_closure_with_members_marked() {
        let m = meta();
        let b = BuildOutputs::from_messages(
            r#"{"reason":"build-script-executed","package_id":"core#1","out_dir":"/t/out","cfgs":["has_x"],"env":[["K","v"]]}
{"reason":"compiler-artifact","package_id":"pm#1","target":{"kind":["proc-macro"]},"filenames":["/t/libpm.so"]}"#,
        );
        let external = BTreeMap::from([("app".to_string(), vec!["/ws/crates/core/shared".to_string()])]);
        let p = rust_project(&m, &b, &["app".into()], "/sys", Some("/view"), &external).unwrap();
        let crates = p["crates"].as_array().unwrap();
        let names: Vec<&str> = crates.iter().map(|c| c["display_name"].as_str().unwrap()).collect();
        assert_eq!(names, ["app", "core", "itoa_like", "pm", "serde_like", "app"], "libs sorted, then app's test");
        let by = |i: usize| &crates[i];
        assert_eq!(by(0)["is_workspace_member"], true);
        assert_eq!(by(0)["root_module"], "/view/crates/app/src/lib.rs");
        assert_eq!(by(0)["cfg"], json!(["feature=\"fast\"", "test"]));
        let dep_names = |c: &Value| c["deps"].as_array().unwrap().iter().map(|d| d["name"].as_str().unwrap().to_string()).collect::<Vec<_>>();
        assert_eq!(dep_names(by(0)), ["core", "serde_like", "pm"]);
        assert_eq!(by(1)["is_workspace_member"], false);
        assert_eq!(by(1)["root_module"], "/view/crates/core/src/lib.rs");
        assert_eq!(by(1)["cfg"], json!(["has_x"]));
        assert_eq!(by(1)["env"]["OUT_DIR"], "/t/out");
        assert_eq!(by(1)["env"]["K"], "v");
        assert_eq!(by(1)["env"]["CARGO_PKG_VERSION_PRE"], "rc.1");
        assert_eq!(by(1)["source"]["include_dirs"], json!(["/view/crates/core", "/t/out"]), "the package root");
        assert_eq!(by(0)["source"]["include_dirs"], json!(["/view/crates/app", "/view/crates/core/shared"]), "plus #[path] folders");
        assert_eq!(by(2)["root_module"], "/reg/itoa/src/lib.rs", "outside the workspace: not rewritten");
        assert_eq!(by(3)["is_proc_macro"], true);
        assert_eq!(by(3)["proc_macro_dylib_path"], "/t/libpm.so");
        assert_eq!(by(5)["root_module"], "/view/crates/app/tests/it.rs");
        assert_eq!(dep_names(by(5)), ["core", "serde_like", "pm", "app"]);
        assert_eq!(p["sysroot_src"], "/sys/lib/rustlib/src/rust/library");
        assert!(!names.contains(&"lone"), "an unrelated member is not loaded");
    }

    /// `#[path]` folders outside the package are found (relative to the
    /// file, `..` resolved); ones inside it, and other attributes, are not.
    #[test]
    fn external_path_folders_are_found() {
        let files = vec![
            ("/ws/crates/d/examples/x/main.rs".to_string(), "#[path = \"../../../m/examples/common/a.rs\"]\nmod a;".to_string()),
            ("/ws/crates/d/tests/t.rs".to_string(), "  #[path = \"../v/shared.rs\"]\nmod s;\n#[cfg(test)]".to_string()),
            ("/ws/crates/d/src/lib.rs".to_string(), "#[path=\"../../e/f.rs\"] mod f;".to_string()),
        ];
        assert_eq!(external_path_dirs("/ws/crates/d", &files), ["/ws/crates/e", "/ws/crates/m/examples/common"]);
    }

    /// Every member is in exactly one partition; a member whose closure
    /// alone exceeds the budget gets its own; small members are grouped
    /// while the union stays within the budget; the result is the same
    /// every time.
    #[test]
    fn the_plan_covers_every_member_once_within_the_budget() {
        let m = meta();
        let all = |ps: &[Partition]| {
            let mut v: Vec<String> = ps.iter().flat_map(|p| p.members.clone()).collect();
            v.sort();
            v
        };
        let roomy = plan(&m, 10, 10).unwrap();
        assert_eq!(all(&roomy), ["app", "core", "lone"]);
        assert_eq!(roomy.len(), 1, "{roomy:?}");
        let tight = plan(&m, 3, 10).unwrap();
        assert_eq!(all(&tight), ["app", "core", "lone"]);
        assert_eq!(tight[0].members, ["app"], "largest first, over budget alone");
        assert_eq!(tight[0].closure, 5);
        assert_eq!(tight[1].members, ["core", "lone"]);
        assert_eq!(plan(&m, 3, 10).unwrap(), tight);
        let one_each = plan(&m, 10, 1).unwrap();
        assert_eq!(one_each.len(), 3);
        assert_eq!(in_view("/ws", None, "/ws/a.rs"), "/ws/a.rs");
        assert_eq!(in_view("/ws", Some("/v"), "/wsx/a.rs"), "/wsx/a.rs");
    }
}
