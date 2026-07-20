//! `kovan lit` — the literature pipeline (`kovan-literature`): PDF import,
//! BibTeX generation, and Markdown heading outlines.
//!
//! Implements the canonical workflow from `docs/kovan.md`, "Literature
//! Workflow": `PDF → Markdown → KovanDocument → BibTeX`. The Rust
//! [`KovanDocument`] is authoritative; `lit bibtex` only ever *renders* from
//! it, never the reverse.

use std::path::{Path, PathBuf};

use clap::Subcommand;

use kovan_common::KovanDocument;

/// `kovan lit <subcommand>`.
#[derive(Subcommand)]
pub enum LitCommand {
    /// Import a PDF: extract metadata + generate the Markdown body into a
    /// `KovanDocument`, and print a line-oriented summary.
    Import {
        /// Source PDF.
        pdf: PathBuf,
        /// Also write the full document as pretty JSON to this path (the
        /// canonical on-disk form — re-readable by `lit bibtex`).
        #[arg(long)]
        json_out: Option<PathBuf>,
        /// Also write just the generated Markdown body to this path.
        #[arg(long)]
        markdown_out: Option<PathBuf>,
    },
    /// Emit a BibTeX entry — from a source PDF (metadata is extracted first)
    /// or from a previously-saved `KovanDocument` JSON file (`.json`
    /// extension, e.g. from `lit import --json-out`).
    Bibtex {
        /// Source PDF, or a `.json` `KovanDocument` (dispatched by extension).
        input: PathBuf,
    },
    /// Print the Markdown heading outline of a PDF, one heading per line
    /// (`"#"` repeated `level` times, a space, then the heading text — mirrors
    /// the Markdown itself).
    Outline {
        /// Source PDF.
        pdf: PathBuf,
    },
}

/// Dispatch a parsed [`LitCommand`].
pub fn run(command: LitCommand) -> Result<(), String> {
    match command {
        LitCommand::Import {
            pdf,
            json_out,
            markdown_out,
        } => import(&pdf, json_out.as_deref(), markdown_out.as_deref()),
        LitCommand::Bibtex { input } => bibtex(&input),
        LitCommand::Outline { pdf } => outline(&pdf),
    }
}

fn import(pdf: &Path, json_out: Option<&Path>, markdown_out: Option<&Path>) -> Result<(), String> {
    let doc = kovan_literature::extract_metadata(pdf).map_err(|e| e.to_string())?;
    print_summary(&doc);

    if let Some(path) = json_out {
        let json = serde_json::to_string_pretty(&doc).map_err(|e| e.to_string())?;
        std::fs::write(path, json).map_err(|e| format!("write {}: {e}", path.display()))?;
        println!("json_out: {}", path.display());
    }
    if let Some(path) = markdown_out {
        std::fs::write(path, &doc.markdown_body)
            .map_err(|e| format!("write {}: {e}", path.display()))?;
        println!("markdown_out: {}", path.display());
    }
    Ok(())
}

fn bibtex(input: &Path) -> Result<(), String> {
    let is_json = input
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("json"));

    let doc = if is_json {
        load_document(input)?
    } else {
        kovan_literature::extract_metadata(input).map_err(|e| e.to_string())?
    };
    print!("{}", kovan_literature::to_bibtex(&doc));
    Ok(())
}

fn outline(pdf: &Path) -> Result<(), String> {
    let markdown = kovan_literature::pdf_to_markdown(pdf).map_err(|e| e.to_string())?;
    for h in kovan_literature::markdown_outline(&markdown) {
        println!("{} {}", "#".repeat(h.level as usize), h.text);
    }
    Ok(())
}

/// Load a `KovanDocument` previously written by `lit import --json-out`.
fn load_document(path: &Path) -> Result<KovanDocument, String> {
    let text =
        std::fs::read_to_string(path).map_err(|e| format!("read {}: {e}", path.display()))?;
    serde_json::from_str(&text).map_err(|e| format!("parse {}: {e}", path.display()))
}

/// Print a deterministic, line-oriented `key: value` summary of `doc` — the
/// agent-facing view of a [`KovanDocument`]. Pass `--json-out` for the full
/// record.
fn print_summary(doc: &KovanDocument) {
    println!("id: {}", doc.id);
    println!("slug: {}", doc.slug);
    println!("visibility: {:?}", doc.visibility);
    println!("document_type: {:?}", doc.document_type);
    println!("title: {}", doc.title);
    println!(
        "authors: {}",
        doc.authors
            .iter()
            .map(|a| {
                if a.given.is_empty() {
                    a.family.clone()
                } else {
                    format!("{}, {}", a.family, a.given)
                }
            })
            .collect::<Vec<_>>()
            .join("; ")
    );
    println!(
        "year: {}",
        doc.year
            .map(|y| y.to_string())
            .unwrap_or_else(|| "-".to_string())
    );
    println!("doi: {}", doc.doi.as_deref().unwrap_or("-"));
    println!("keywords: {}", doc.keywords.join(", "));
    println!(
        "page_count: {}",
        doc.page_count
            .map(|p| p.to_string())
            .unwrap_or_else(|| "-".to_string())
    );
    println!("source_path: {}", doc.source_path.as_deref().unwrap_or("-"));
    println!("markdown_chars: {}", doc.markdown_body.chars().count());
    println!("markdown_lines: {}", doc.markdown_body.lines().count());
}

#[cfg(test)]
mod tests {
    use super::*;
    use kovan_common::{DocumentType, Visibility};

    fn sample() -> KovanDocument {
        let mut doc = KovanDocument::new(
            "kovan-1",
            "doe2021test",
            Visibility::Open,
            DocumentType::Report,
            "A Test Report",
        );
        doc.year = Some(2021);
        doc.markdown_body = "line one\nline two\n".to_string();
        doc
    }

    #[test]
    fn document_json_round_trips_through_load_document() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("doc.json");
        let doc = sample();
        std::fs::write(&path, serde_json::to_string_pretty(&doc).unwrap()).unwrap();

        let loaded = load_document(&path).expect("load ok");
        assert_eq!(loaded, doc);
    }

    #[test]
    fn load_document_missing_file_is_error() {
        let missing = std::path::Path::new("/nonexistent/kovan-cli-nope.json");
        assert!(load_document(missing).is_err());
    }
}
