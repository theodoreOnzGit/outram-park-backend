//! **Derived maturity** from valid stamps (#739 feeding #735): for one
//! crate, is each human-rung claim in its `Cargo.toml` tag backed by stamps?
//!
//! The tag (#733, parsed by [`crate::code_map`]) gives the crate's
//! `maturity` (its lowest part) and `maturity_modules` (modules rated
//! higher). Rungs 0-2 are AI rungs that stamps do not speak to; rungs 3
//! (human reviewed) and 4 (human V&V) are what a stamp records. So a claim
//! at rung 3 or 4 is **SUPPORTED** when every function in its scope has a
//! valid stamp at that rung or higher, and **UNSUPPORTED** otherwise, with
//! the functions that fall short listed. A claim at rung 0-2 is reported as
//! not applicable.
//!
//! **Scope.** The crate claim covers the library: `lib.rs` in the library's
//! folder and every module reachable from it through `mod name;` and inline
//! `mod name { ... }`. A module claim covers that module and its
//! submodules. Excluded: `#[cfg(test)]` modules and `#[test]` functions (the
//! tests are not the code being reviewed), examples, binaries, and
//! integration tests. A `mod` with a `#[path]` attribute is not followed and
//! is listed as a problem, which makes a claim unsupported rather than
//! silently smaller. A module with no functions (constants and types only)
//! cannot be supported by stamps yet, and says so.
//!
//! This only reports. It never edits a `Cargo.toml`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use super::parse::{self, FnEntry, ParsedFile};
use super::{check, Scope, Verdict};
use crate::code_map::CrateNode;

/// Where a module's code starts: its file, and the inline-module path
/// inside that file (empty when the module is the whole file).
type ModuleStart = (PathBuf, Vec<String>);

/// Whether a claim is backed by stamps.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Support {
    Supported,
    /// Why not, in one line.
    Unsupported(String),
    /// Rungs 0-2: stamps record human review only.
    NotApplicable,
}

/// One claim from the tag, judged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Claim {
    /// `crate`, or the module path from the library root (`a::b`).
    pub scope: String,
    pub level: u8,
    /// Functions in scope (tests excluded).
    pub functions: usize,
    /// Functions with a valid stamp at `level` or above.
    pub at_level: usize,
    /// `file::Type::name`, valid stamp below `level`.
    pub below_level: Vec<String>,
    /// `file::Type::name`, only void stamps.
    pub void: Vec<String>,
    /// `file::Type::name`, never stamped.
    pub unstamped: Vec<String>,
    /// Modules not found or not followed, files that do not parse.
    pub problems: Vec<String>,
    pub support: Support,
}

/// The derived levels of one crate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DerivedLevels {
    pub crate_name: String,
    /// The crate claim first, then each `maturity_modules` entry in order.
    pub claims: Vec<Claim>,
    /// The lowest valid rung over every library function, when every one has
    /// a valid stamp; `None` otherwise (stamps cannot place the crate).
    pub from_stamps: Option<u8>,
}

/// [`derived_levels_for`] the crate `crate_name` of the workspace at
/// `workspace`, its tag read through `cargo metadata`.
pub fn derived_levels(workspace: &Path, crate_name: &str) -> Result<DerivedLevels, String> {
    let map = crate::code_map::load_workspace(workspace)?;
    let node = map
        .get(crate_name)
        .ok_or_else(|| format!("no member crate named {crate_name}"))?;
    derived_levels_for(workspace, node)
}

/// The derived levels of `node`, judged on the working tree's stamps.
pub fn derived_levels_for(workspace: &Path, node: &CrateNode) -> Result<DerivedLevels, String> {
    let lib_dir = node
        .lib_dir
        .as_deref()
        .ok_or_else(|| format!("{} has no library", node.name))?;
    let lib_dir = PathBuf::from(lib_dir);
    let lib_dir = if lib_dir.is_absolute() {
        lib_dir
    } else {
        workspace.join(lib_dir)
    };
    let ws = workspace
        .canonicalize()
        .map_err(|e| format!("{}: {e}", workspace.display()))?;
    let lib_dir = lib_dir
        .canonicalize()
        .map_err(|e| format!("{}: {e}", lib_dir.display()))?;
    let rel = |p: &Path| -> String {
        let r = p.strip_prefix(&ws).unwrap_or(p);
        r.components()
            .map(|c| c.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/")
    };

    // Stamp state per located function: (file, qualname, first line).
    let report = check(workspace, &Scope::All)?;
    let mut best: BTreeMap<(String, String, u32), u8> = BTreeMap::new();
    let mut void: Vec<(String, String)> = Vec::new();
    for c in &report.checked {
        match (&c.verdict, &c.qualname, c.now_lines) {
            (Verdict::Valid, Some(q), Some(l)) => {
                let e = best
                    .entry((c.stamp.file.clone(), q.clone(), l[0]))
                    .or_insert(0);
                *e = (*e).max(c.stamp.rung);
            }
            (Verdict::Void(_), _, _) => {
                if let Some(f) = &c.stamp.function {
                    if let Ok((file, qual)) = crate::commands::code_walk::builder::split_spec(f) {
                        void.push((file, qual));
                    }
                }
            }
            _ => {}
        }
    }

    let mut cache: BTreeMap<PathBuf, Result<ParsedFile, String>> = BTreeMap::new();
    let lib_rs = lib_dir.join("lib.rs");
    let mut claims = Vec::new();
    let mut scopes: Vec<(String, u8, Result<ModuleStart, String>)> = vec![(
        "crate".into(),
        node.maturity,
        Ok((lib_rs.clone(), Vec::new())),
    )];
    for m in &node.maturity_modules {
        let segs: Vec<String> = m.module.split("::").map(str::to_string).collect();
        scopes.push((
            m.module.clone(),
            m.level,
            resolve_module(&lib_rs, &segs, &mut cache),
        ));
    }
    let mut lowest: Option<u8> = None;
    let mut all_stamped = true;
    for (scope, level, start) in scopes {
        let mut problems = Vec::new();
        let mut fns: Vec<(String, FnEntry)> = Vec::new();
        match start {
            Ok((file, prefix)) => collect(&file, &prefix, &mut cache, &mut fns, &mut problems),
            Err(e) => problems.push(e),
        }
        let mut claim = Claim {
            scope: scope.clone(),
            level,
            functions: fns.len(),
            at_level: 0,
            below_level: Vec::new(),
            void: Vec::new(),
            unstamped: Vec::new(),
            problems,
            support: Support::NotApplicable,
        };
        for (file, f) in &fns {
            let file = rel(Path::new(file));
            let name = format!("{file}::{}", f.qualname());
            match best.get(&(file.clone(), f.qualname(), f.lines[0])) {
                Some(&r) => {
                    if scope == "crate" {
                        lowest = Some(lowest.map_or(r, |l| l.min(r)));
                    }
                    if r >= level {
                        claim.at_level += 1;
                    } else {
                        claim.below_level.push(name);
                    }
                }
                None => {
                    if scope == "crate" {
                        all_stamped = false;
                    }
                    if void
                        .iter()
                        .any(|(vf, q)| *vf == file && parse::matches(f, q))
                    {
                        claim.void.push(name);
                    } else {
                        claim.unstamped.push(name);
                    }
                }
            }
        }
        claim.problems = claim
            .problems
            .into_iter()
            .map(|p| p.replace(&format!("{}/", ws.display()), ""))
            .collect();
        claim.support = if level < 3 {
            Support::NotApplicable
        } else if !claim.problems.is_empty() {
            Support::Unsupported(format!(
                "{} problem(s) reading the module",
                claim.problems.len()
            ))
        } else if claim.functions == 0 {
            Support::Unsupported(
                "no functions to stamp (constants and types are not stampable yet)".into(),
            )
        } else if claim.at_level == claim.functions {
            Support::Supported
        } else {
            Support::Unsupported(format!(
                "{} of {} functions have a valid stamp at rung {level} or above",
                claim.at_level, claim.functions
            ))
        };
        claims.push(claim);
    }
    let from_stamps = if all_stamped
        && claims
            .first()
            .is_some_and(|c| c.problems.is_empty() && c.functions > 0)
    {
        lowest
    } else {
        None
    };
    Ok(DerivedLevels {
        crate_name: node.name.clone(),
        claims,
        from_stamps,
    })
}

fn parsed<'a>(
    file: &Path,
    cache: &'a mut BTreeMap<PathBuf, Result<ParsedFile, String>>,
) -> &'a Result<ParsedFile, String> {
    cache.entry(file.to_path_buf()).or_insert_with(|| {
        let text = std::fs::read_to_string(file).map_err(|e| format!("{}: {e}", file.display()))?;
        parse::parse_file(&text).map_err(|e| format!("{}: {e}", file.display()))
    })
}

/// The folder a file's `mod name;` children live in: its own folder for
/// `lib.rs`, `main.rs` and `mod.rs`, else `<folder>/<stem>/`; then one level
/// per enclosing inline module.
fn child_dir(file: &Path, inline: &[String]) -> PathBuf {
    let parent = file.parent().unwrap_or(Path::new("")).to_path_buf();
    let stem = file
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let mut dir = if matches!(stem.as_str(), "lib" | "main" | "mod") {
        parent
    } else {
        parent.join(stem)
    };
    for m in inline {
        dir.push(m);
    }
    dir
}

/// The file of `mod name;` declared in `file` inside `inline`.
fn mod_file(file: &Path, inline: &[String], name: &str) -> Result<PathBuf, String> {
    let dir = child_dir(file, inline);
    let a = dir.join(format!("{name}.rs"));
    let b = dir.join(name).join("mod.rs");
    if a.is_file() {
        Ok(a)
    } else if b.is_file() {
        Ok(b)
    } else {
        Err(format!(
            "module file for `{name}` not found ({} or {})",
            a.display(),
            b.display()
        ))
    }
}

/// The file and inline path where module `segs` lives, starting at `lib_rs`.
fn resolve_module(
    lib_rs: &Path,
    segs: &[String],
    cache: &mut BTreeMap<PathBuf, Result<ParsedFile, String>>,
) -> Result<ModuleStart, String> {
    let mut file = lib_rs.to_path_buf();
    let mut inline: Vec<String> = Vec::new();
    for seg in segs {
        let p = parsed(&file, cache).as_ref().map_err(Clone::clone)?;
        let mut path = inline.clone();
        path.push(seg.clone());
        if p.inline_mods.contains(&path) {
            inline = path;
            continue;
        }
        let decl = p
            .mods
            .iter()
            .find(|m| m.inline_parents == inline && &m.name == seg)
            .ok_or_else(|| {
                format!(
                    "module `{}` not declared in {}",
                    segs.join("::"),
                    file.display()
                )
            })?;
        if decl.has_path_attr {
            return Err(format!(
                "module `{seg}` has a #[path] attribute, which is not followed"
            ));
        }
        file = mod_file(&file, &inline, seg)?;
        inline.clear();
    }
    Ok((file, inline))
}

/// Every non-test function of `file` under inline path `prefix`, then of
/// every module file declared there, recursively.
fn collect(
    file: &Path,
    prefix: &[String],
    cache: &mut BTreeMap<PathBuf, Result<ParsedFile, String>>,
    out: &mut Vec<(String, FnEntry)>,
    problems: &mut Vec<String>,
) {
    let p = match parsed(file, cache) {
        Ok(p) => p.clone(),
        Err(e) => {
            problems.push(e.clone());
            return;
        }
    };
    let name = file.to_string_lossy().into_owned();
    for f in &p.fns {
        if !f.is_test && f.inline_parents.starts_with(prefix) {
            out.push((name.clone(), f.clone()));
        }
    }
    for m in &p.mods {
        if m.is_test || !m.inline_parents.starts_with(prefix) {
            continue;
        }
        if m.has_path_attr {
            problems.push(format!(
                "{}: `mod {}` has a #[path] attribute, not followed",
                name, m.name
            ));
            continue;
        }
        match mod_file(file, &m.inline_parents, &m.name) {
            Ok(child) => collect(&child, &[], cache, out, problems),
            Err(e) => problems.push(e),
        }
    }
}

/// The report `kovan-cli stamps-levels` prints.
pub fn render(d: &DerivedLevels) -> String {
    let mut out = format!(
        "stamps-levels: {} (report only; Cargo.toml is not changed)\n",
        d.crate_name
    );
    for c in &d.claims {
        let what = if c.scope == "crate" {
            "crate maturity".to_string()
        } else {
            format!("module {}", c.scope)
        };
        let verdict = match &c.support {
            Support::Supported => "SUPPORTED".to_string(),
            Support::Unsupported(why) => format!("UNSUPPORTED: {why}"),
            Support::NotApplicable => "n/a (AI rung; stamps record human review only)".to_string(),
        };
        out.push_str(&format!(
            "{what} = {} ({}): {verdict}  [{} functions, {} stamped at level, {} below, {} void, {} unstamped]\n",
            c.level,
            crate::code_map::maturity_label(c.level),
            c.functions,
            c.at_level,
            c.below_level.len(),
            c.void.len(),
            c.unstamped.len()
        ));
        if c.level >= 3 {
            for (tag, list) in [
                ("below rung", &c.below_level),
                ("void", &c.void),
                ("unstamped", &c.unstamped),
            ] {
                for n in list {
                    out.push_str(&format!("    {tag}: {n}\n"));
                }
            }
        }
        for p in &c.problems {
            out.push_str(&format!("    problem: {p}\n"));
        }
    }
    out.push_str(&match d.from_stamps {
        Some(l) => format!("from stamps: every library function is stamped; lowest rung {l}\n"),
        None => "from stamps: not every library function has a valid stamp\n".to_string(),
    });
    out
}
