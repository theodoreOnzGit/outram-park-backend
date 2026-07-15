//! Ripgrep-first semantics, end to end: list the language adapters, then scan
//! this very crate and print a `symbols.md` preview.
//!
//! Deterministic and offline — no language server, no network. Run with:
//! `cargo run -p kovan-semantics --example adapters`

use kovan_semantics::{catalogue_symbols, symbols_markdown, LanguageAdapter};

fn main() {
    println!("== Language adapters ==");
    for a in [
        LanguageAdapter::Rust,
        LanguageAdapter::Cpp,
        LanguageAdapter::Python,
        LanguageAdapter::Fortran,
    ] {
        println!(
            "{a:?}: ripgrep-first now, escalates to `{}` (server binary `{}`)",
            a.preferred_tool(),
            a.server_binary()
        );
    }

    // Scan this crate's own `src/` with the ripgrep-first Rust extractor.
    let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let symbols = catalogue_symbols(repo, LanguageAdapter::Rust).expect("scan should succeed");

    println!("\n== Catalogued {} Rust symbols ==", symbols.len());
    let md = symbols_markdown("kovan-semantics", &symbols);
    // Print just the head so the example stays readable.
    for line in md.lines().take(20) {
        println!("{line}");
    }
}
