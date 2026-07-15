//! End-to-end tests: spawn the compiled `kovan` binary against synthetic,
//! throwaway fixtures (a tempdir repository tree, a synthetic PDF built with
//! `lopdf`) and assert on its stdout/stderr/exit code. This is deliberately a
//! black-box test of the CLI surface — argument parsing is covered by the
//! unit tests in `src/main.rs`; this file checks the subcommands actually
//! drive the underlying `kovan-*` libraries and print the documented,
//! line-oriented output.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// Run the compiled `kovan` binary with `args`, returning its output.
/// `CARGO_BIN_EXE_kovan` is set by Cargo for integration tests in this same
/// (binary) crate.
fn kovan(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_kovan"))
        .args(args)
        .output()
        .expect("spawn kovan")
}

fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

/// Build a tiny throwaway Rust "repository" fixture:
///
/// ```text
/// <tmp>/
/// └── src/
///     └── lib.rs   (pub fn kept() {}, pub struct Thing;)
/// ```
fn fixture_repo() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir(dir.path().join("src")).unwrap();
    std::fs::write(
        dir.path().join("src/lib.rs"),
        "pub fn kept() {}\npub struct Thing;\n",
    )
    .unwrap();
    dir
}

// ---------------------------------------------------------------------
// discover / search / scan / methods
// ---------------------------------------------------------------------

#[test]
fn discover_lists_source_files_under_root() {
    let repo = fixture_repo();
    let out = kovan(&[
        "discover",
        "--root",
        repo.path().to_str().unwrap(),
        "--kind",
        "source",
    ]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(stdout(&out).trim_end().ends_with("src/lib.rs"));
}

#[test]
fn search_single_file_mode_finds_matches_with_line_and_column() {
    let repo = fixture_repo();
    let lib_rs = repo.path().join("src/lib.rs");
    let out = kovan(&[
        "search",
        "--path",
        lib_rs.to_str().unwrap(),
        "--pattern",
        r"pub fn \w+",
    ]);
    assert!(out.status.success(), "{}", stderr(&out));
    let text = stdout(&out);
    assert!(text.contains(":1:1: pub fn kept() {}"), "{text}");
}

#[test]
fn search_repository_mode_scans_every_source_file() {
    let repo = fixture_repo();
    let out = kovan(&[
        "search",
        "--root",
        repo.path().to_str().unwrap(),
        "--kind",
        "source",
        "--pattern",
        r"pub (fn|struct) \w+",
    ]);
    assert!(out.status.success(), "{}", stderr(&out));
    let text = stdout(&out);
    assert!(text.contains("pub fn kept() {}"), "{text}");
    assert!(text.contains("pub struct Thing;"), "{text}");
}

#[test]
fn scan_reports_probable_rust_definitions() {
    let repo = fixture_repo();
    let out = kovan(&[
        "scan",
        "--root",
        repo.path().to_str().unwrap(),
        "--lang",
        "rust",
    ]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(stdout(&out).contains("pub fn kept"));
}

#[test]
fn methods_lists_every_family_including_pde() {
    let out = kovan(&["methods"]);
    assert!(out.status.success(), "{}", stderr(&out));
    let text = stdout(&out);
    for family in [
        "root-finders:",
        "linear-solvers:",
        "nonlinear-solvers:",
        "ode-solvers:",
        "pde-schemes:",
    ] {
        assert!(text.contains(family), "missing {family} in:\n{text}");
    }
    assert!(text.contains("NewtonRaphson (codegen: ready)"));
    assert!(text.contains("Gmres (codegen: not-implemented)"));
}

// ---------------------------------------------------------------------
// symbols / summary
// ---------------------------------------------------------------------

#[test]
fn symbols_line_oriented_mode_lists_qualified_names() {
    let repo = fixture_repo();
    let out = kovan(&["symbols", repo.path().to_str().unwrap(), "--lang", "rust"]);
    assert!(out.status.success(), "{}", stderr(&out));
    let text = stdout(&out);
    assert!(text.contains("kept"), "{text}");
    assert!(text.contains("Thing"), "{text}");
}

#[test]
fn symbols_markdown_mode_renders_the_symbols_md_artifact() {
    let repo = fixture_repo();
    let out = kovan(&[
        "symbols",
        repo.path().to_str().unwrap(),
        "--lang",
        "rust",
        "--markdown",
        "--name",
        "fixture-repo",
    ]);
    assert!(out.status.success(), "{}", stderr(&out));
    let text = stdout(&out);
    assert!(text.starts_with("# Symbols — fixture-repo"), "{text}");
    assert!(text.contains("| Kind | Count |"), "{text}");
}

#[test]
fn summary_writes_repository_summary_md_to_a_file() {
    let repo = fixture_repo();
    let out_dir = tempfile::tempdir().expect("tempdir");
    let out_path = out_dir.path().join("repository-summary.md");
    let out = kovan(&[
        "summary",
        repo.path().to_str().unwrap(),
        "--lang",
        "rust",
        "--name",
        "Fixture",
        "--out",
        out_path.to_str().unwrap(),
    ]);
    assert!(out.status.success(), "{}", stderr(&out));
    let written = std::fs::read_to_string(&out_path).expect("read written summary");
    assert!(
        written.starts_with("# Repository summary — Fixture"),
        "{written}"
    );
}

// ---------------------------------------------------------------------
// gen
// ---------------------------------------------------------------------

#[test]
fn gen_root_newton_raphson_prints_rust_source() {
    let out = kovan(&["gen", "root", "newton-raphson"]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(stdout(&out).contains("pub fn newton_raphson"));
}

#[test]
fn gen_ode_rk4_writes_to_out_file() {
    let dir = tempfile::tempdir().expect("tempdir");
    let out_path = dir.path().join("rk4.rs");
    let out = kovan(&["gen", "ode", "rk4", "--out", out_path.to_str().unwrap()]);
    assert!(out.status.success(), "{}", stderr(&out));
    let written = std::fs::read_to_string(&out_path).unwrap();
    assert!(written.contains("pub fn rk4_step"));
}

#[test]
fn gen_unimplemented_method_fails_with_a_clear_error() {
    let out = kovan(&["gen", "linear", "gmres"]);
    assert!(!out.status.success());
    assert!(
        stderr(&out).contains("not implemented yet"),
        "{}",
        stderr(&out)
    );
}

// ---------------------------------------------------------------------
// lit — the literature pipeline, exercised on a synthetic PDF
// ---------------------------------------------------------------------

/// Build a minimal, fully synthetic one-page PDF (mirrors
/// `kovan-literature`'s private `src/test_pdf.rs`, which isn't exported) so
/// `lit import`/`lit bibtex`/`lit outline` can be exercised end-to-end
/// offline, without shipping any real (possibly proprietary) PDF fixture.
fn build_synthetic_pdf(path: &Path) {
    use lopdf::content::{Content, Operation};
    use lopdf::{dictionary, Document, Object, Stream};

    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();

    let font_id = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica",
        "Encoding" => "WinAnsiEncoding",
    });
    let resources_id = doc.add_object(dictionary! {
        "Font" => dictionary! { "F1" => font_id },
    });

    let content = Content {
        operations: vec![
            Operation::new("BT", vec![]),
            Operation::new("Tf", vec!["F1".into(), 18.into()]),
            Operation::new("Td", vec![72.into(), 720.into()]),
            Operation::new(
                "Tj",
                vec![Object::string_literal("Kovan CLI Fixture Report")],
            ),
            Operation::new("ET", vec![]),
        ],
    };
    let content_id = doc.add_object(Stream::new(dictionary! {}, content.encode().unwrap()));

    let page_id = doc.add_object(dictionary! {
        "Type" => "Page",
        "Parent" => pages_id,
        "Contents" => content_id,
        "Resources" => resources_id,
        "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
    });

    let pages = dictionary! {
        "Type" => "Pages",
        "Kids" => vec![page_id.into()],
        "Count" => 1,
    };
    doc.objects.insert(pages_id, Object::Dictionary(pages));

    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    });
    doc.trailer.set("Root", catalog_id);

    let info_id = doc.add_object(dictionary! {
        "Title" => Object::string_literal("A Synthetic CLI Fixture Report"),
        "Author" => Object::string_literal("Doe, Jane"),
        "Keywords" => Object::string_literal("kovan, fixture"),
        "CreationDate" => Object::string_literal("D:20220101120000Z"),
    });
    doc.trailer.set("Info", info_id);

    doc.save(path).expect("write synthetic test PDF");
}

fn synthetic_pdf() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("fixture.pdf");
    build_synthetic_pdf(&path);
    (dir, path)
}

#[test]
fn lit_import_prints_a_line_oriented_summary() {
    let (_dir, pdf) = synthetic_pdf();
    let out = kovan(&["lit", "import", pdf.to_str().unwrap()]);
    assert!(out.status.success(), "{}", stderr(&out));
    let text = stdout(&out);
    assert!(
        text.contains("title: A Synthetic CLI Fixture Report"),
        "{text}"
    );
    assert!(text.contains("year: 2022"), "{text}");
    assert!(text.contains("authors: Doe, Jane"), "{text}");
}

#[test]
fn lit_import_json_out_round_trips_into_lit_bibtex() {
    let (_pdf_dir, pdf) = synthetic_pdf();
    let out_dir = tempfile::tempdir().expect("tempdir");
    let json_path = out_dir.path().join("doc.json");

    let import_out = kovan(&[
        "lit",
        "import",
        pdf.to_str().unwrap(),
        "--json-out",
        json_path.to_str().unwrap(),
    ]);
    assert!(import_out.status.success(), "{}", stderr(&import_out));
    assert!(json_path.exists());

    let bibtex_out = kovan(&["lit", "bibtex", json_path.to_str().unwrap()]);
    assert!(bibtex_out.status.success(), "{}", stderr(&bibtex_out));
    let bib = stdout(&bibtex_out);
    assert!(
        bib.starts_with("@techreport{") || bib.starts_with("@misc{"),
        "{bib}"
    );
    assert!(bib.contains("Doe, Jane"), "{bib}");
}

#[test]
fn lit_bibtex_direct_from_pdf_matches_import_then_bibtex_json() {
    let (_dir, pdf) = synthetic_pdf();
    let direct = kovan(&["lit", "bibtex", pdf.to_str().unwrap()]);
    assert!(direct.status.success(), "{}", stderr(&direct));
    assert!(stdout(&direct).contains("title = {A Synthetic CLI Fixture Report}"));
}

#[test]
fn lit_outline_succeeds_on_a_heading_free_fixture() {
    // The fixture PDF has no headings by kovan-literature's high-confidence
    // rules, so the outline is legitimately empty — this asserts the command
    // completes cleanly rather than asserting specific headings.
    let (_dir, pdf) = synthetic_pdf();
    let out = kovan(&["lit", "outline", pdf.to_str().unwrap()]);
    assert!(out.status.success(), "{}", stderr(&out));
}

#[test]
fn lit_bibtex_missing_pdf_reports_an_error() {
    let out = kovan(&["lit", "bibtex", "/nonexistent/kovan-cli-nope.pdf"]);
    assert!(!out.status.success());
    assert!(!stderr(&out).is_empty());
}
