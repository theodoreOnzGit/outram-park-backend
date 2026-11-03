//! **Which crates a `Cargo.lock` change touches** (GitHub #1048,
//! maintainer 2026-10-29): a dependency fingerprint per workspace member,
//! so "pending workspace test" fires only for a crate whose resolved
//! dependency tree moved, not for all 48 whenever any line of the lock
//! does.
//!
//! ```text
//!   Cargo.lock ─► [[package]] graph ─► member M: walk `dependencies`
//!                                       transitively (through members)
//!                                       ─► external packages reached
//!                                       ─► sha256 of their sorted
//!                                          "name version source checksum"
//! ```
//!
//! **Members are walked, never hashed.** Their code is covered by function
//! hashes; a member's version bump is not a dependency change. A member's
//! own dependency edits show up as the external packages its closure
//! reaches.
//!
//! **What it cannot see.** `Cargo.lock` holds the workspace's resolved
//! graph with features unified across members, not each crate's features,
//! so a closure is a superset of what the crate compiles against, and a
//! feature change that leaves the lock alone is invisible (as it was to
//! the whole-lock hash before).

use std::collections::{BTreeMap, BTreeSet};

use serde::Deserialize;

use super::hash::sha256_tagged;

#[derive(Debug, Deserialize)]
struct Lock {
    #[serde(default, rename = "package")]
    packages: Vec<Package>,
}

#[derive(Debug, Deserialize)]
struct Package {
    name: String,
    version: String,
    #[serde(default)]
    source: Option<String>,
    #[serde(default)]
    checksum: Option<String>,
    #[serde(default)]
    dependencies: Vec<String>,
}

/// Member name -> `sha256:` of its external dependency closure, for every
/// workspace member (a package with no `source`) in `lock`. `None` when
/// the text is not a readable `Cargo.lock`: the caller then falls back to
/// comparing whole-lock hashes.
pub fn crate_fingerprints(lock: &str) -> Option<BTreeMap<String, String>> {
    let lock: Lock = toml::from_str(lock).ok()?;
    let pk = &lock.packages;
    let mut by_name: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
    for (i, p) in pk.iter().enumerate() {
        by_name.entry(p.name.as_str()).or_default().push(i);
    }
    // A dependency entry is "name", "name version" or
    // "name version (source)"; resolve it to package indices.
    let resolve = |dep: &str| -> Vec<usize> {
        let mut parts = dep.splitn(3, ' ');
        let name = parts.next().unwrap_or("");
        let version = parts.next();
        by_name
            .get(name)
            .into_iter()
            .flatten()
            .copied()
            .filter(|&i| version.is_none_or(|v| pk[i].version == v))
            .collect()
    };
    let mut out = BTreeMap::new();
    for (m, member) in pk.iter().enumerate().filter(|(_, p)| p.source.is_none()) {
        let mut seen: BTreeSet<usize> = BTreeSet::from([m]);
        let mut stack = vec![m];
        let mut lines: BTreeSet<String> = BTreeSet::new();
        while let Some(i) = stack.pop() {
            for d in &pk[i].dependencies {
                for j in resolve(d) {
                    if seen.insert(j) {
                        let p = &pk[j];
                        if let Some(src) = &p.source {
                            lines.insert(format!(
                                "{} {} {} {}",
                                p.name,
                                p.version,
                                src,
                                p.checksum.as_deref().unwrap_or("")
                            ));
                        }
                        stack.push(j);
                    }
                }
            }
        }
        let text: Vec<&str> = lines.iter().map(String::as_str).collect();
        out.insert(member.name.clone(), sha256_tagged(text.join("\n").as_bytes()));
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lock(serde_ver: &str, wgpu_ver: &str) -> String {
        format!(
            r#"version = 4

[[package]]
name = "physics"
version = "0.1.0"
dependencies = ["helper", "serde"]

[[package]]
name = "helper"
version = "0.2.0"
dependencies = ["libm"]

[[package]]
name = "gui"
version = "0.1.0"
dependencies = ["physics", "wgpu"]

[[package]]
name = "serde"
version = "{serde_ver}"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "s{serde_ver}"

[[package]]
name = "libm"
version = "0.2.8"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "l"

[[package]]
name = "wgpu"
version = "{wgpu_ver}"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "w{wgpu_ver}"
"#
        )
    }

    /// Methodology: a synthetic three-member lock (`gui` -> `physics` ->
    /// `helper`), bumped once outside `physics`' closure (wgpu) and once
    /// inside it (serde). Pass: the wgpu bump moves only `gui`; the serde
    /// bump moves `physics` and `gui`, which reaches it through a member;
    /// `helper` moves in neither; an unreadable lock gives `None`.
    #[test]
    fn only_crates_whose_closure_changed_move() {
        let a = crate_fingerprints(&lock("1.0.0", "27.0.0")).unwrap();
        let wgpu = crate_fingerprints(&lock("1.0.0", "28.0.0")).unwrap();
        let serde = crate_fingerprints(&lock("1.0.1", "27.0.0")).unwrap();
        assert_eq!(a.keys().collect::<Vec<_>>(), ["gui", "helper", "physics"]);
        assert_eq!(a["physics"], wgpu["physics"]);
        assert_eq!(a["helper"], wgpu["helper"]);
        assert_ne!(a["gui"], wgpu["gui"]);
        assert_ne!(a["physics"], serde["physics"]);
        assert_ne!(a["gui"], serde["gui"], "reached through the physics member");
        assert_eq!(a["helper"], serde["helper"]);
        assert_eq!(crate_fingerprints("not a lock ["), None);
    }

    /// Methodology: two versions of one package in the lock, each named by
    /// "name version" as Cargo writes them. Pass: a member depending on
    /// one version does not move when the other is bumped.
    #[test]
    fn a_versioned_dependency_resolves_to_that_version_only() {
        let text = |other: &str| {
            format!(
                r#"[[package]]
name = "m"
version = "0.1.0"
dependencies = ["rand 0.8.5"]

[[package]]
name = "rand"
version = "0.8.5"
source = "registry+x"
checksum = "a"

[[package]]
name = "rand"
version = "{other}"
source = "registry+x"
checksum = "{other}"
"#
            )
        };
        let a = crate_fingerprints(&text("0.9.0")).unwrap();
        let b = crate_fingerprints(&text("0.9.1")).unwrap();
        assert_eq!(a["m"], b["m"]);
    }
}
