//! `kovan-cli references`: CSL citations on the Pages site's markdown
//! (GitHub #789).
//!
//! Every `[...](#ref-KEY)` link under the given paths becomes the citation the
//! CSL style renders, and each page's reference list (between
//! `<!-- references:begin -->` and `<!-- references:end -->`) is regenerated
//! from the keys it cites. The work is `kovan_literature::csl`; this module
//! walks the files, reports, and writes.
//!
//! Without `--update` it is a check: it changes nothing, lists every page
//! whose citations are stale or whose keys are unknown, and fails, so
//! `scripts/build-pages.sh` can gate on it like `code-walk-check`.
//! `--csl-json-out` writes the library as CSL-JSON instead: the exact input
//! `scripts/csl-reference.sh` gives citeproc-js for the verification test.

use std::path::{Path, PathBuf};

use kovan_literature::csl::{cite_html_page, cite_page, CslLibrary, CslStyle, LOCALE_EN_US};

/// What `run` was asked to do, parsed from the CLI.
#[derive(Debug, Clone)]
pub struct ReferencesArgs {
    /// Markdown files or directories to scan.
    pub paths: Vec<PathBuf>,
    /// Write regenerated pages instead of failing on a difference.
    pub update: bool,
    /// The `.bib` file.
    pub bib: PathBuf,
    /// A CSL style file, or the name of a pinned style (`apa`,
    /// `chicago-author-date`, `ieee`, `nature`, `vancouver`); `None` is APA 7.
    pub style: Option<PathBuf>,
    /// Write the library as a CSL-JSON array here and do nothing else.
    pub csl_json_out: Option<PathBuf>,
}

/// Run the command.
///
/// # Errors
///
/// An unreadable or invalid `.bib` or style; in check mode, any stale page or
/// unknown key (the message lists them).
pub fn run(args: ReferencesArgs) -> Result<(), String> {
    let bib = std::fs::read_to_string(&args.bib)
        .map_err(|e| format!("reading {}: {e}", args.bib.display()))?;
    let lib = CslLibrary::from_bib(&bib)?;

    if let Some(out) = &args.csl_json_out {
        let items: Vec<&serde_json::Value> = lib.keys().filter_map(|k| lib.csl_json(k)).collect();
        let text = serde_json::to_string_pretty(&items).map_err(|e| e.to_string())?;
        std::fs::write(out, text + "\n").map_err(|e| format!("writing {}: {e}", out.display()))?;
        println!("{} items written to {}", items.len(), out.display());
        return Ok(());
    }

    let style = match &args.style {
        None => CslStyle::apa(),
        Some(p) => match (!p.exists())
            .then(|| CslStyle::bundled(&p.to_string_lossy()))
            .flatten()
        {
            Some(bundled) => bundled,
            None => {
                let xml = std::fs::read_to_string(p)
                    .map_err(|e| format!("reading {}: {e}", p.display()))?;
                CslStyle::from_xml(&xml, LOCALE_EN_US)?
            }
        },
    };

    let mut pages = Vec::new();
    for p in &args.paths {
        collect_markdown(p, &mut pages)?;
    }
    pages.sort();

    let report = check_pages(&pages, &lib, &style, args.update)?;
    println!(
        "references: {} page(s) scanned, {} cite at least one work, {} distinct works cited",
        pages.len(),
        report.citing,
        report.works
    );
    if !report.problems.is_empty() {
        let list = report.problems.join("\n  ");
        return Err(if args.update {
            format!("unknown cite keys (nothing written for these pages):\n  {list}")
        } else {
            format!("stale citations:\n  {list}\nregenerate with: kovan-cli references --update --bib <bib> <paths>")
        });
    }
    if args.update {
        println!("{} page(s) rewritten", report.rewritten);
    }
    Ok(())
}

/// The outcome of [`check_pages`].
#[derive(Debug, Default, PartialEq)]
pub struct PagesReport {
    /// Pages citing at least one work.
    pub citing: usize,
    /// Distinct works cited across the pages.
    pub works: usize,
    /// Pages written (update mode).
    pub rewritten: usize,
    /// One line per stale page or unknown key.
    pub problems: Vec<String>,
}

/// Regenerate every page; in update mode write the changed ones, otherwise
/// report them.
///
/// # Errors
///
/// A page that cannot be read or written.
pub fn check_pages(
    pages: &[PathBuf],
    lib: &CslLibrary,
    style: &CslStyle,
    update: bool,
) -> Result<PagesReport, String> {
    let mut report = PagesReport::default();
    let mut all_keys = std::collections::BTreeSet::new();
    for page in pages {
        let text = std::fs::read_to_string(page)
            .map_err(|e| format!("reading {}: {e}", page.display()))?;
        let cited = if page.extension().is_some_and(|x| x == "html") {
            cite_html_page(&text, lib, style)
        } else {
            cite_page(&text, lib, style)
        };
        match cited {
            Err(errors) => {
                for e in errors {
                    report.problems.push(format!("{}: {e}", page.display()));
                }
            }
            Ok(out) => {
                if !out.keys.is_empty() {
                    report.citing += 1;
                }
                all_keys.extend(out.keys);
                if out.text != text {
                    if update {
                        std::fs::write(page, &out.text)
                            .map_err(|e| format!("writing {}: {e}", page.display()))?;
                        report.rewritten += 1;
                    } else {
                        report.problems.push(format!(
                            "{}: citations or reference list out of date",
                            page.display()
                        ));
                    }
                }
            }
        }
    }
    report.works = all_keys.len();
    Ok(report)
}

/// Every `.md` file at or under `path`, except mdBook's `SUMMARY.md`.
fn collect_markdown(path: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    if path.is_file() {
        out.push(path.to_path_buf());
        return Ok(());
    }
    let dir = std::fs::read_dir(path).map_err(|e| format!("reading {}: {e}", path.display()))?;
    for entry in dir {
        let p = entry.map_err(|e| e.to_string())?.path();
        if p.is_dir() {
            collect_markdown(&p, out)?;
        } else if p.extension().is_some_and(|x| x == "md")
            && p.file_name().is_some_and(|n| n != "SUMMARY.md")
        {
            out.push(p);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const BIB: &str = "@article{openmc,\n  author = {Romano, Paul K. and Forget, Benoit},\n  \
                       title = {OpenMC},\n  journal = {Annals of Nuclear Energy},\n  year = {2015}\n}\n";

    #[test]
    fn check_reports_stale_pages_and_update_fixes_them() {
        let dir = std::env::temp_dir().join(format!("kovan-references-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("book")).unwrap();
        std::fs::write(dir.join("refs.bib"), BIB).unwrap();
        std::fs::write(dir.join("book/a.md"), "Uses [](#ref-openmc).\n").unwrap();
        std::fs::write(dir.join("book/b.md"), "No citations.\n").unwrap();
        std::fs::write(dir.join("book/SUMMARY.md"), "- [A](a.md) [](#ref-nope)\n").unwrap();

        let mut pages = Vec::new();
        collect_markdown(&dir.join("book"), &mut pages).unwrap();
        pages.sort();
        assert_eq!(pages.len(), 2, "SUMMARY.md is skipped");

        let lib = CslLibrary::from_bib(BIB).unwrap();
        let style = CslStyle::apa();
        let check = check_pages(&pages, &lib, &style, false).unwrap();
        assert_eq!(check.citing, 1);
        assert_eq!(check.problems.len(), 1, "{:?}", check.problems);
        assert!(check.problems[0].contains("a.md"));

        let upd = check_pages(&pages, &lib, &style, true).unwrap();
        assert_eq!(upd.rewritten, 1);
        let a = std::fs::read_to_string(dir.join("book/a.md")).unwrap();
        assert!(a.contains("[(Romano & Forget, 2015)](#ref-openmc)"), "{a}");
        assert!(check_pages(&pages, &lib, &style, false)
            .unwrap()
            .problems
            .is_empty());

        // The CLI path end to end, CSL-JSON dump included.
        let out = dir.join("items.json");
        run(ReferencesArgs {
            paths: vec![],
            update: false,
            bib: dir.join("refs.bib"),
            style: None,
            csl_json_out: Some(out.clone()),
        })
        .unwrap();
        let items: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(out).unwrap()).unwrap();
        assert_eq!(items[0]["id"], "openmc");
        run(ReferencesArgs {
            paths: vec![dir.join("book")],
            update: false,
            bib: dir.join("refs.bib"),
            style: None,
            csl_json_out: None,
        })
        .unwrap();
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn an_html_page_named_on_the_command_line_is_cited_and_a_style_can_be_named() {
        let dir =
            std::env::temp_dir().join(format!("kovan-references-html-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("refs.bib"), BIB).unwrap();
        let page = dir.join("index.html");
        std::fs::write(&page, "<p>See <a href=\"#ref-openmc\"></a>.</p>\n").unwrap();
        let args = |update| ReferencesArgs {
            paths: vec![page.clone()],
            update,
            bib: dir.join("refs.bib"),
            style: Some(PathBuf::from("ieee")),
            csl_json_out: None,
        };
        assert!(run(args(false)).is_err(), "stale before the update");
        run(args(true)).unwrap();
        let text = std::fs::read_to_string(&page).unwrap();
        assert!(text.contains("<a href=\"#ref-openmc\">[1]</a>"), "{text}");
        assert!(
            text.contains("<h2>References</h2>") && text.contains("id=\"ref-openmc\""),
            "{text}"
        );
        run(args(false)).unwrap();
        std::fs::remove_dir_all(&dir).ok();
    }
}
