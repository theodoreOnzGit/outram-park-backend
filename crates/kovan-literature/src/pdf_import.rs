//! PDF import: plain-text extraction, Markdown generation, and image-asset
//! extraction.
//!
//! Implements the "PDF import", "Markdown generation" and "Asset extraction"
//! steps of `docs/kovan.md`, "PDF Processing". Everything here is offline and
//! deterministic:
//!
//! - Text extraction uses the pure-Rust [`pdf_extract`] crate (no OCR, no
//!   network). It is wrapped in [`std::panic::catch_unwind`] because
//!   `pdf-extract` can panic on malformed/unsupported PDFs; a panic is turned
//!   into a [`LiteratureError::Io`] instead of aborting the caller.
//! - Asset extraction uses the pure-Rust [`lopdf`] object model to pull embedded
//!   raster images out of the PDF.
//!
//! Both crates build for `aarch64-linux-android`, so this module keeps the crate
//! Android-buildable (`docs/kovan.md`, "Android First").

use crate::LiteratureError;
use lopdf::{Dictionary, Document, Object};
use std::path::{Path, PathBuf};

/// Extract the raw plain text of a PDF, in reading order, as `pdf-extract`
/// reports it (pages separated by form-feed `U+000C`).
///
/// Deterministic and offline. Returns [`LiteratureError::Io`] if the file cannot
/// be parsed, and — because `pdf-extract` panics on some malformed inputs —
/// converts any internal panic into the same error rather than unwinding into
/// the caller.
pub fn extract_pdf_text(pdf: &Path) -> Result<String, LiteratureError> {
    let path = pdf.to_path_buf();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        pdf_extract::extract_text(&path)
    }));
    match result {
        Ok(Ok(text)) => Ok(text),
        Ok(Err(e)) => Err(LiteratureError::Io(format!(
            "pdf text extraction failed for {}: {e}",
            pdf.display()
        ))),
        Err(_) => Err(LiteratureError::Io(format!(
            "pdf-extract panicked while parsing {} (malformed or unsupported PDF)",
            pdf.display()
        ))),
    }
}

/// Import a PDF and produce a structured Markdown body.
///
/// Pipeline: [`extract_pdf_text`] → [`crate::text_to_markdown`]. The result is a
/// single Markdown string; split it with [`crate::split_markdown_by_page_limit`]
/// to keep each generated document within [`crate::MAX_MARKDOWN_PAGES`].
///
/// Deterministic and offline.
pub fn pdf_to_markdown(pdf: &Path) -> Result<String, LiteratureError> {
    let text = extract_pdf_text(pdf)?;
    Ok(crate::markdown::text_to_markdown(&text))
}

/// Extract embedded raster images from a source PDF into `out_dir`, returning
/// the written file paths in deterministic (PDF object-id) order.
///
/// Scope (best-effort): only images whose PDF filter is already a standalone
/// file format are written — `DCTDecode` (JPEG, `.jpg`) and `JPXDecode`
/// (JPEG-2000, `.jp2`) — because their stream bytes *are* the encoded file.
/// Images stored under other filters (e.g. raw `FlateDecode` samples that would
/// need re-encoding to PNG with the right colour space/bit depth) are skipped,
/// not fabricated. `out_dir` is created only if at least one image is written.
///
/// Files are named `<pdf-stem>-imgNNN.<ext>`. Deterministic and offline.
pub fn extract_assets(pdf: &Path, out_dir: &Path) -> Result<Vec<PathBuf>, LiteratureError> {
    let doc = Document::load(pdf)
        .map_err(|e| LiteratureError::Io(format!("load {}: {e}", pdf.display())))?;

    let stem = pdf
        .file_stem()
        .map(|s| sanitize_stem(&s.to_string_lossy()))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "document".to_string());

    let mut written = Vec::new();
    // `Document::objects` is a BTreeMap keyed by object id, so iteration order is
    // already sorted and stable → deterministic output.
    for obj in doc.objects.values() {
        let Object::Stream(stream) = obj else {
            continue;
        };
        if !dict_name_eq(&stream.dict, b"Subtype", b"Image") {
            continue;
        }
        let Some(ext) = image_extension(&stream.dict) else {
            continue;
        };
        if written.is_empty() {
            std::fs::create_dir_all(out_dir)
                .map_err(|e| LiteratureError::Io(format!("create {}: {e}", out_dir.display())))?;
        }
        let filename = format!("{stem}-img{:03}.{ext}", written.len());
        let path = out_dir.join(filename);
        std::fs::write(&path, &stream.content)
            .map_err(|e| LiteratureError::Io(format!("write {}: {e}", path.display())))?;
        written.push(path);
    }
    Ok(written)
}

/// True if `dict[key]` is a `/Name` equal to `value`.
fn dict_name_eq(dict: &Dictionary, key: &[u8], value: &[u8]) -> bool {
    matches!(dict.get(key), Ok(Object::Name(n)) if n.as_slice() == value)
}

/// Return the file extension for an image XObject whose codec is a standalone
/// file format, or `None` if it uses a filter we do not re-encode.
fn image_extension(dict: &Dictionary) -> Option<&'static str> {
    let filter = dict.get(b"Filter").ok()?;
    let names: Vec<&[u8]> = match filter {
        Object::Name(n) => vec![n.as_slice()],
        Object::Array(arr) => arr
            .iter()
            .filter_map(|o| match o {
                Object::Name(n) => Some(n.as_slice()),
                _ => None,
            })
            .collect(),
        _ => return None,
    };
    if names.contains(&b"DCTDecode".as_slice()) {
        Some("jpg")
    } else if names.contains(&b"JPXDecode".as_slice()) {
        Some("jp2")
    } else {
        None
    }
}

/// Reduce a file stem to `[a-z0-9-]` for use in generated asset filenames.
fn sanitize_stem(stem: &str) -> String {
    stem.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_pdf::{build_text_pdf, tmp_path};

    #[test]
    fn pdf_to_markdown_extracts_body_text() {
        let path = tmp_path("kovan_lit_body.pdf");
        build_text_pdf(&path);
        let md = pdf_to_markdown(&path).expect("markdown");
        assert!(
            !md.trim().is_empty(),
            "expected non-empty markdown, got {md:?}"
        );
        assert!(
            md.contains("Steam"),
            "expected extracted text to contain 'Steam', got {md:?}"
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn extract_text_on_missing_file_is_error() {
        let err = extract_pdf_text(Path::new("/nonexistent/kovan-nope.pdf"));
        assert!(matches!(err, Err(LiteratureError::Io(_))));
    }

    #[test]
    fn extract_assets_on_image_free_pdf_writes_nothing() {
        let path = tmp_path("kovan_lit_noimg.pdf");
        build_text_pdf(&path);
        let out = tmp_path("kovan_lit_assets_out");
        let written = extract_assets(&path, &out).expect("assets");
        assert!(written.is_empty());
        let _ = std::fs::remove_file(&path);
    }
}
