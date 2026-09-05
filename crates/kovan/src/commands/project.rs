//! `kovan-cli project` — the "kovan folder" project format (op-63u0's
//! design, `docs/kovan-folder-format.md`): rescanning a project and
//! (re)writing its `kovan.toml` index (op-b1y5).
//!
//! Wraps `crate::project` directly — this module is the `clap` surface
//! and line-oriented output only; the scan/regenerate/write logic lives in
//! the library so a future GUI action (a "regenerate now" button, or a
//! markdown-save hook) can call it without going through the CLI.

use std::path::PathBuf;

use clap::Subcommand;

/// `kovan-cli project <subcommand>`.
#[derive(Subcommand)]
pub enum ProjectCommand {
    /// Rescan a "kovan folder" project and rewrite its `kovan.toml` index.
    ///
    /// Prints one line per document found, then a summary. `kovan.toml` is
    /// generated — see the module docs — so this always fully replaces it,
    /// never merges.
    Regen {
        /// The project root (containing `kovan.toml`, one `.bib` file,
        /// `pdf/`, `markdown/`).
        #[arg(default_value = ".")]
        root: PathBuf,
    },
}

/// Dispatch a `kovan-cli project` subcommand.
pub fn run(cmd: ProjectCommand) -> Result<(), String> {
    match cmd {
        ProjectCommand::Regen { root } => regen(&root),
    }
}

fn regen(root: &std::path::Path) -> Result<(), String> {
    let index = crate::project::regenerate_and_write(root).map_err(|e| e.to_string())?;
    for doc in &index.documents {
        println!("{}: {} <-> {}", doc.id, doc.pdf, doc.markdown);
    }
    println!(
        "{} document(s) indexed, bib_file = {}, wrote {}",
        index.documents.len(),
        index.bib_file,
        root.join("kovan.toml").display()
    );
    Ok(())
}
