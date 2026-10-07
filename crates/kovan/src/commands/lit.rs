//! `kovan-cli lit` — the literature pipeline (`kovan-literature`): PDF import,
//! BibTeX generation, and Markdown heading outlines.
//!
//! Implements the canonical workflow from `docs/kovan.md`, "Literature
//! Workflow": `PDF → Markdown → KovanDocument → BibTeX`. The Rust
//! [`KovanDocument`] is authoritative; `lit bibtex` only ever *renders* from
//! it, never the reverse.

use std::path::{Path, PathBuf};

use clap::Subcommand;

use kovan_common::KovanDocument;

/// `kovan-cli lit <subcommand>`.
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
        /// Look up the identifier found in the PDF (DOI, arXiv, ISBN, PMID)
        /// with Zotero's search translators and show the fetched record
        /// field by field next to the extracted one (GitHub #756). Uses the
        /// network; without this flag nothing goes online and a found
        /// identifier is only reported. Nothing is applied unless
        /// `--accept` names it.
        #[arg(long)]
        lookup: bool,
        /// With `--lookup`: the fields to take from the fetched record,
        /// comma-separated (`title,authors,year,doi,journal,institution,
        /// publisher,volume,number,pages,abstract_text,keywords,
        /// document_type`), or `all`. Default: none (review only).
        #[arg(long, requires = "lookup", value_delimiter = ',')]
        accept: Vec<String>,
        /// With `--lookup`: your email for Crossref's polite pool (never
        /// sent unless given).
        #[arg(long, requires = "lookup")]
        mailto: Option<String>,
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
            lookup,
            accept,
            mailto,
        } => import(
            &pdf,
            json_out.as_deref(),
            markdown_out.as_deref(),
            LookupRequest {
                lookup,
                accept,
                mailto,
            },
        ),
        LitCommand::Bibtex { input } => bibtex(&input),
        LitCommand::Outline { pdf } => outline(&pdf),
    }
}

/// `lit import`'s lookup options (#756).
#[derive(Debug, Default)]
pub struct LookupRequest {
    /// `--lookup`.
    pub lookup: bool,
    /// `--accept`.
    pub accept: Vec<String>,
    /// `--mailto`.
    pub mailto: Option<String>,
}

fn import(
    pdf: &Path,
    json_out: Option<&Path>,
    markdown_out: Option<&Path>,
    lookup: LookupRequest,
) -> Result<(), String> {
    let doc = kovan_literature::extract_metadata(pdf).map_err(|e| e.to_string())?;
    print_summary(&doc);
    let doc = offer_lookup(doc, &lookup)?;

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

/// The PDF follow-up (#756): report a found identifier; with `--lookup`,
/// fetch its record and show it field by field next to the extraction,
/// applying only the `--accept`ed fields. A failed lookup is reported as
/// `lookup_unavailable: <reason>` and the import continues with the
/// extracted document unchanged.
fn offer_lookup(doc: KovanDocument, req: &LookupRequest) -> Result<KovanDocument, String> {
    use kovan_literature::lookup_review::{
        all_changes, apply, identifiers_in_document, propose, DocField,
    };
    let ids = identifiers_in_document(&doc);
    let Some(id) = ids.first().cloned() else {
        return Ok(doc);
    };
    if !req.lookup {
        println!(
            "lookup_available: {} {} (rerun with --lookup to fetch the record; uses the network)",
            id.kind(),
            id.value()
        );
        return Ok(doc);
    }
    let accepted_names: Vec<String> = req.accept.iter().map(|a| a.trim().to_owned()).collect();
    let mut accepted: Vec<DocField> = Vec::new();
    for a in &accepted_names {
        if a == "all" || a.is_empty() {
            continue;
        }
        accepted
            .push(DocField::from_name(a).ok_or_else(|| format!("--accept: unknown field {a}"))?);
    }
    println!("lookup: {} {}", id.kind(), id.value());
    let run = match super::zotero_lookup::lookup_identifier(id, req.mailto.as_deref()) {
        Ok(run) => run,
        Err(e) => {
            println!("lookup_unavailable: {e}");
            return Ok(doc);
        }
    };
    let items = match super::zotero_lookup::run_items(&run) {
        Ok(items) => items,
        Err(e) => {
            println!("lookup_unavailable: {e}");
            return Ok(doc);
        }
    };
    let Some(record) = items.iter().find(|i| i.item_type.as_str() != "note") else {
        println!("lookup_unavailable: no record returned");
        return Ok(doc);
    };
    println!("lookup_translator: {}", run.used.metadata().label);
    // The CLI has no edit step, so no field counts as user-edited.
    let proposal = propose(&doc, record, &[]);
    for row in &proposal.fields {
        println!(
            "lookup_field: {} | {:?} | extracted: {} | fetched: {}",
            row.field.name(),
            row.decision,
            one_line(&row.current),
            one_line(&row.fetched)
        );
    }
    if accepted_names.iter().any(|a| a == "all") {
        accepted = all_changes(&proposal);
    }
    if accepted.is_empty() {
        println!("lookup_applied: none (review only; pass --accept <fields>|all)");
        return Ok(doc);
    }
    let out = apply(&doc, &proposal, &accepted, false);
    println!(
        "lookup_applied: {}",
        accepted
            .iter()
            .map(|f| f.name())
            .collect::<Vec<_>>()
            .join(",")
    );
    Ok(out)
}

fn one_line(s: &str) -> String {
    let t: String = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if t.chars().count() > 100 {
        format!("{}...", t.chars().take(100).collect::<String>())
    } else {
        t
    }
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
