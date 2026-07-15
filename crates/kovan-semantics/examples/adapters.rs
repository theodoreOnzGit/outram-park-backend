//! List the language adapters and the semantic tool each one drives.
//!
//! Run with: `cargo run -p kovan-semantics --example adapters`

use kovan_semantics::LanguageAdapter;

fn main() {
    let adapters = [
        LanguageAdapter::Rust,
        LanguageAdapter::Cpp,
        LanguageAdapter::Python,
        LanguageAdapter::Fortran,
    ];
    for a in adapters {
        println!("{a:?} -> {}", a.preferred_tool());
    }
}
