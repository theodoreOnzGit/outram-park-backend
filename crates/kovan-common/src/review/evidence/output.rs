//! Parse `cargo test` output, one line at a time, as it streams.
//!
//! Only **stable** formats are read: libtest's default human output and
//! cargo's `--message-format=json-render-diagnostics` build messages. The
//! unstable libtest JSON (`-Z unstable-options --format json`) is not used.
//!
//! ```text
//! {"reason":"compiler-artifact",…,"executable":"…/deps/x-hash"}   cargo, stdout
//!      Running unittests src/lib.rs (target/release/deps/x-hash)   cargo, stderr
//! running 3 tests                                                  libtest
//! test a::b ... ok | FAILED | ignored[, reason]
//! test c - should panic ... ok
//! failures: / successes:            (detail sections, not parsed)
//! test result: FAILED. 2 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; …
//! ```
//!
//! cargo's stderr and the test binaries' stdout must arrive **in one stream,
//! in order** (the CLI gives both the same pipe), because the `Running` line
//! is what says which binary the following results belong to.
//!
//! The JSON artifact message names the package and the target's root file
//! exactly; it is matched to a `Running` line by the executable's file name.
//! Without it (plain output) the package is left empty and the source path is
//! taken, package-relative, from the `Running` line.
//!
//! Result lines are read only between `running N tests` and the first
//! `failures:`/`successes:` section or the summary, so a test's captured
//! output that happens to look like a result line is not mistaken for one.

use std::collections::BTreeMap;

use super::{BinaryKind, BinaryResults, Summary};

/// What a line was, so the caller can decide what to echo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineKind {
    /// A cargo JSON message: consumed, not for the terminal.
    Json,
    /// Anything else: show it.
    Text,
}

#[derive(Debug, Clone)]
struct Artifact {
    package: String,
    kind: BinaryKind,
    target: String,
    src: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Outside,
    Results,
    Details,
}

/// The streaming parser.
#[derive(Debug, Clone)]
pub struct OutputParser {
    root: String,
    artifacts: BTreeMap<String, Artifact>,
    binaries: Vec<BinaryResults>,
    state: State,
    build_ok: Option<bool>,
}

/// What the parser found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedRun {
    pub binaries: Vec<BinaryResults>,
    /// cargo's `build-finished` message said success (absent counts as
    /// success only if at least one binary ran).
    pub build_ok: bool,
}

impl OutputParser {
    /// `workspace_root` is the absolute path that source paths are made
    /// relative to.
    pub fn new(workspace_root: &str) -> OutputParser {
        OutputParser {
            root: workspace_root.trim_end_matches('/').to_string(),
            artifacts: BTreeMap::new(),
            binaries: Vec::new(),
            state: State::Outside,
            build_ok: None,
        }
    }

    fn relative(&self, path: &str) -> String {
        let p = path.replace('\\', "/");
        match p.strip_prefix(&self.root) {
            Some(rest) if rest.starts_with('/') => rest[1..].to_string(),
            _ => p,
        }
    }

    /// Feed one line (without its newline).
    pub fn feed(&mut self, line: &str) -> LineKind {
        let line = line.trim_end_matches(['\r', '\n']);
        if line.starts_with('{') {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(line) {
                self.json(&v);
                return LineKind::Json;
            }
        }
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("Running ") {
            self.running(rest);
        } else if t.starts_with("Doc-tests ") {
            self.state = State::Outside;
        } else if let Some(n) = t
            .strip_prefix("running ")
            .and_then(|r| r.strip_suffix(" tests").or_else(|| r.strip_suffix(" test")))
        {
            if let Some(b) = self.binaries.last_mut() {
                b.expected = n.parse().ok();
            }
            self.state = State::Results;
        } else if let Some(rest) = t.strip_prefix("test result: ") {
            if let Some(b) = self.binaries.last_mut() {
                b.summary = parse_summary(rest);
            }
            self.state = State::Outside;
        } else if self.state == State::Results {
            if t == "failures:" || t == "successes:" {
                self.state = State::Details;
            } else if let Some(rest) = line.strip_prefix("test ") {
                self.result(rest);
            }
        }
        LineKind::Text
    }

    fn json(&mut self, v: &serde_json::Value) {
        match v.get("reason").and_then(|r| r.as_str()) {
            Some("compiler-artifact") => {
                let Some(exe) = v.get("executable").and_then(|e| e.as_str()) else {
                    return;
                };
                let target = &v["target"];
                let kinds: Vec<&str> = target["kind"]
                    .as_array()
                    .map(|a| a.iter().filter_map(|k| k.as_str()).collect())
                    .unwrap_or_default();
                let kind = if kinds.iter().any(|k| k.ends_with("lib")) {
                    BinaryKind::Lib
                } else if kinds.contains(&"bin") {
                    BinaryKind::Bin
                } else if kinds.contains(&"test") {
                    BinaryKind::Test
                } else {
                    BinaryKind::Other
                };
                let a = Artifact {
                    package: package_name(v["package_id"].as_str().unwrap_or("")),
                    kind,
                    target: target["name"].as_str().unwrap_or("").to_string(),
                    src: self.relative(target["src_path"].as_str().unwrap_or("")),
                };
                self.artifacts.insert(file_name(exe).to_string(), a);
            }
            Some("build-finished") => {
                self.build_ok = v.get("success").and_then(|s| s.as_bool());
            }
            _ => {}
        }
    }

    /// `unittests src/lib.rs (target/release/deps/x-hash)` or
    /// `tests/foo.rs (target/release/deps/foo-hash)`.
    fn running(&mut self, rest: &str) {
        self.state = State::Outside;
        let (desc, exe) = match (rest.rfind(" ("), rest.ends_with(')')) {
            (Some(i), true) => (&rest[..i], &rest[i + 2..rest.len() - 1]),
            _ => (rest, rest),
        };
        let b = match self.artifacts.get(file_name(exe)) {
            Some(a) => BinaryResults {
                package: a.package.clone(),
                kind: a.kind,
                target: a.target.clone(),
                src: a.src.clone(),
                expected: None,
                summary: None,
                complete: false,
                passed: Vec::new(),
                failed: Vec::new(),
                ignored: Vec::new(),
            },
            None => {
                let (kind, src) = match desc.strip_prefix("unittests ") {
                    Some(s) if s.contains("/bin/") || s.ends_with("main.rs") => {
                        (BinaryKind::Bin, s)
                    }
                    Some(s) => (BinaryKind::Lib, s),
                    None if desc.starts_with("tests/") => (BinaryKind::Test, desc),
                    None => (BinaryKind::Other, desc),
                };
                let stem = file_name(exe);
                let target = stem.rsplit_once('-').map_or(stem, |(t, _)| t);
                BinaryResults {
                    package: String::new(),
                    kind,
                    target: target.to_string(),
                    src: src.to_string(),
                    expected: None,
                    summary: None,
                    complete: false,
                    passed: Vec::new(),
                    failed: Vec::new(),
                    ignored: Vec::new(),
                }
            }
        };
        self.binaries.push(b);
    }

    /// `a::b ... ok`, `c - should panic ... ok`, `d ... ignored, reason`.
    fn result(&mut self, rest: &str) {
        let Some((name, outcome)) = rest.split_once(" ... ") else {
            return;
        };
        let name = name
            .strip_suffix(" - should panic")
            .unwrap_or(name)
            .to_string();
        let Some(b) = self.binaries.last_mut() else {
            return;
        };
        let outcome = outcome.trim();
        if outcome == "ok" {
            b.passed.push(name);
        } else if outcome == "FAILED" {
            b.failed.push(name);
        } else if outcome.starts_with("ignored") {
            b.ignored.push(name);
        }
    }

    /// Everything parsed so far.
    pub fn finish(self) -> ParsedRun {
        let build_ok = self.build_ok.unwrap_or(!self.binaries.is_empty());
        ParsedRun {
            binaries: self.binaries,
            build_ok,
        }
    }
}

fn file_name(path: &str) -> &str {
    let p = path.rsplit(['/', '\\']).next().unwrap_or(path);
    p.strip_suffix(".exe").unwrap_or(p)
}

/// `path+file:///ws/crates/x#0.1.0` → `x`; `…/crates/kovan#long-name@0.0.1`
/// → `long-name`; the older `name 0.1.0 (path+file://…)` → `name`.
fn package_name(id: &str) -> String {
    if !id.contains("://") || id.contains(' ') {
        return id.split_whitespace().next().unwrap_or("").to_string();
    }
    let (path, frag) = id.split_once('#').unwrap_or((id, ""));
    match frag.split_once('@') {
        Some((name, _)) => name.to_string(),
        None => path.rsplit('/').next().unwrap_or("").to_string(),
    }
}

/// `ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s`.
fn parse_summary(rest: &str) -> Option<Summary> {
    let (status, counts) = rest.split_once(". ")?;
    let mut s = Summary {
        ok: status == "ok",
        ..Summary::default()
    };
    for part in counts.split(';') {
        let part = part.trim();
        let Some((n, what)) = part.split_once(' ') else {
            continue;
        };
        let Ok(n) = n.parse::<u32>() else {
            continue;
        };
        match what {
            "passed" => s.passed = n,
            "failed" => s.failed = n,
            "ignored" => s.ignored = n,
            "measured" => s.measured = n,
            "filtered out" => s.filtered_out = n,
            _ => {}
        }
    }
    Some(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Methodology: package names from both cargo package-id formats, and a
    /// summary line, parse to the values written in them.
    ///
    /// Result (2026-10-07): passes.
    #[test]
    fn package_ids_and_summaries_parse() {
        assert_eq!(
            package_name("path+file:///ws/crates/bishan#0.0.1"),
            "bishan"
        );
        assert_eq!(
            package_name("path+file:///ws/crates/kovan#knowledge-oriented-kovan@0.0.1"),
            "knowledge-oriented-kovan"
        );
        assert_eq!(
            package_name("bishan 0.0.1 (path+file:///ws/crates/bishan)"),
            "bishan"
        );
        let s = parse_summary(
            "FAILED. 4 passed; 1 failed; 1 ignored; 0 measured; 2 filtered out; finished in 0.00s",
        )
        .unwrap();
        assert_eq!(
            s,
            Summary {
                ok: false,
                passed: 4,
                failed: 1,
                ignored: 1,
                measured: 0,
                filtered_out: 2
            }
        );
    }

    /// Methodology: a result-shaped line inside a `failures:` detail
    /// section (a test's captured output) is not counted, and plain output
    /// without cargo JSON still names the binary from the `Running` line.
    ///
    /// Result (2026-10-07): passes.
    #[test]
    fn detail_sections_are_not_results_and_plain_output_still_parses() {
        let text = "     Running tests/flow.rs (target/release/deps/flow-0123abcd)\n\
                    \n\
                    running 2 tests\n\
                    test a ... ok\n\
                    test b ... FAILED\n\
                    \n\
                    failures:\n\
                    \n\
                    ---- b stdout ----\n\
                    test fake ... ok\n\
                    \n\
                    failures:\n    b\n\
                    \n\
                    test result: FAILED. 1 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s\n";
        let mut p = OutputParser::new("/ws");
        for l in text.lines() {
            assert_eq!(p.feed(l), LineKind::Text);
        }
        let run = p.finish();
        assert!(run.build_ok);
        let b = &run.binaries[0];
        assert_eq!(
            (b.kind, b.target.as_str(), b.src.as_str()),
            (BinaryKind::Test, "flow", "tests/flow.rs")
        );
        assert_eq!(b.passed, vec!["a"]);
        assert_eq!(b.failed, vec!["b"]);
        assert!(b.summary_matches());
        assert_eq!(b.expected, Some(2));
    }
}
