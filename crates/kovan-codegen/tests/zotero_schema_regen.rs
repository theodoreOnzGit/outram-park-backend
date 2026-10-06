//! The committed `kovan_common::zotero::schema_generated` is exactly what
//! `kovan_codegen::zotero::generate_schema_rs` makes from zotero-schema's
//! `schema.json` (GitHub #748).
//!
//! **Methodology.** Find `schema.json` (the `ZOTERO_SCHEMA_JSON` variable,
//! else `vendor/zotero-schema/schema.json` in this checkout or any ancestor
//! directory, so a git worktree under `.claude/worktrees/` finds the main
//! checkout's `vendor/`), generate twice, and compare with each other and
//! with `crates/kovan-common/src/zotero/schema_generated.rs`.
//!
//! **Pass:** the two runs are byte-identical, and equal to the committed
//! file. When no `schema.json` is found (CI has no `vendor/`), the test
//! prints why and passes without checking: it cannot fail there, so a green
//! CI run is no evidence for it; run it locally.
//!
//! **Result (2026-10-07):** pass against zotero-schema b86c79b56479
//! (schema version 45); 120,966 bytes, 2,710 lines.

use kovan_codegen::zotero::{generate_schema_rs, SCHEMA_SOURCE_COMMIT};
use std::path::{Path, PathBuf};

fn find_schema() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("ZOTERO_SCHEMA_JSON") {
        return Some(PathBuf::from(p));
    }
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    manifest
        .ancestors()
        .map(|d| d.join("vendor/zotero-schema/schema.json"))
        .find(|p| p.is_file())
}

#[test]
fn committed_schema_tables_regenerate_identically() {
    let Some(schema) = find_schema() else {
        eprintln!(
            "SKIPPED: no vendor/zotero-schema/schema.json found (set ZOTERO_SCHEMA_JSON to check)"
        );
        return;
    };
    let text = std::fs::read_to_string(&schema).expect("read schema.json");
    let first = generate_schema_rs(&text, SCHEMA_SOURCE_COMMIT).expect("generates");
    let second = generate_schema_rs(&text, SCHEMA_SOURCE_COMMIT).expect("generates");
    assert_eq!(first, second, "generation must be deterministic");
    let committed_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../kovan-common/src/zotero/schema_generated.rs");
    let committed = std::fs::read_to_string(&committed_path).expect("read committed tables");
    assert!(
        first == committed,
        "{} is stale relative to {}: regenerate with `cargo run --release -p kovan-codegen \
         --example zotero_schema -- {} {}`",
        committed_path.display(),
        schema.display(),
        schema.display(),
        committed_path.display()
    );
    eprintln!(
        "checked against {} ({} bytes)",
        schema.display(),
        first.len()
    );
}
