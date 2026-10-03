//! Run a DOVER deck headless and print the CSV.
//!
//! ```text
//! cargo run --release -p dover --example smr_cstr -- crates/dover/decks/smr_cstr.toml
//! ```
//!
//! With no argument it runs the bundled base deck. There is no GUI and no
//! window: DOVER is headless by construction, so this is simply how it runs.

fn main() {
    let path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "crates/dover/decks/smr_cstr.toml".to_string());

    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("cannot read deck {path}: {e}");
            std::process::exit(2);
        }
    };

    match dover::headless::run_toml(&text) {
        Ok(csv) => print!("{csv}"),
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    }
}
