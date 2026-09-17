//! Discover this workspace's Rust source files, then grep them for
//! `pub fn` definitions — top to bottom, using only the public API of
//! `kovan-discovery`. No other module needs to be read to follow this.
//!
//! Run with: `cargo run -p kovan-discovery --release --example discover_and_search`
//!
//! What this demonstrates:
//!
//! 1. [`discover_kind`] — find every `FileKind::Source` file under this
//!    crate's own directory, honouring `.gitignore` (nothing under
//!    `target/` shows up, even though this repo has a `target/` dir).
//! 2. [`search_file`] — regex-search one of those files, reading off
//!    line/column/text for each hit.
//! 3. [`search_repository`] — the same two steps fused into one
//!    deterministic call, which is what a caller reaches for in practice.

use std::path::Path;

use kovan_discovery::{discover_kind, search_file, search_repository, FileKind};

fn main() {
    // This crate's own `src/` is the demo target, so the example is
    // reproducible without any setup: it always has at least the
    // definitions in this crate's own `lib.rs`.
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));

    // --- Step 1: discover -------------------------------------------------
    let sources = discover_kind(crate_root, FileKind::Source);
    println!(
        "discovered {} source file(s) under {crate_root:?}:",
        sources.len()
    );
    for path in &sources {
        println!("  {}", path.display());
    }

    // --- Step 2: search a single file --------------------------------------
    let lib_rs = crate_root.join("src/lib.rs");
    let pattern = r"^\s*pub fn \w+";
    let hits = search_file(&lib_rs, pattern).expect("src/lib.rs is readable UTF-8");
    println!(
        "\n'{pattern}' matched {} line(s) in {}:",
        hits.len(),
        lib_rs.display()
    );
    for hit in &hits {
        println!(
            "  {}:{}:{}: {}",
            lib_rs.display(),
            hit.line,
            hit.column,
            hit.text
        );
    }

    // --- Step 3: discover + search in one deterministic pass ---------------
    let combined = search_repository(crate_root, FileKind::Source, pattern)
        .expect("pattern is valid and every discovered file is readable UTF-8");
    println!(
        "\nsearch_repository() found the same {} hit(s) across the whole crate:",
        combined.len()
    );
    for (path, hit) in &combined {
        println!(
            "  {}:{}:{}: {}",
            path.display(),
            hit.line,
            hit.column,
            hit.text
        );
    }
}
