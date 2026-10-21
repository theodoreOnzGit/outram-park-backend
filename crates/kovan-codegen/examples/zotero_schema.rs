//! Regenerate `kovan_common::zotero::schema_generated` from Zotero's
//! `schema.json` (GitHub #748).
//!
//! ```text
//! cargo run --release -p kovan-codegen --example zotero_schema -- \
//!     vendor/zotero-schema/schema.json crates/kovan-common/src/zotero/schema_generated.rs
//! ```
//!
//! With one argument the source goes to stdout. The zotero-schema commit
//! recorded in the header is `kovan_codegen::zotero::SCHEMA_SOURCE_COMMIT`;
//! override it with the `ZOTERO_SCHEMA_COMMIT` environment variable when
//! generating from a different checkout.

use kovan_codegen::zotero::{generate_schema_rs, SCHEMA_SOURCE_COMMIT};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(input) = args.first() else {
        eprintln!("usage: zotero_schema <schema.json> [output.rs]");
        std::process::exit(2);
    };
    let text = std::fs::read_to_string(input).unwrap_or_else(|e| {
        eprintln!("cannot read {input}: {e}");
        std::process::exit(1);
    });
    let commit =
        std::env::var("ZOTERO_SCHEMA_COMMIT").unwrap_or_else(|_| SCHEMA_SOURCE_COMMIT.to_owned());
    let src = generate_schema_rs(&text, &commit).unwrap_or_else(|e| {
        eprintln!("{e}");
        std::process::exit(1);
    });
    match args.get(1) {
        Some(out) => {
            std::fs::write(out, &src).unwrap_or_else(|e| {
                eprintln!("cannot write {out}: {e}");
                std::process::exit(1);
            });
            eprintln!("wrote {} bytes to {out}", src.len());
        }
        None => print!("{src}"),
    }
}
