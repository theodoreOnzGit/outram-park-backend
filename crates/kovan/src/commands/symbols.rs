//! `kovan symbols` and `kovan summary` — repository symbol cataloguing via
//! `kovan-semantics`'s ripgrep-first extractor
//! ([`kovan_semantics::catalogue_symbols_detailed`]), rendered either as
//! agent-facing line-oriented text or as the documented Markdown artifacts
//! (`docs/kovan.md`, "# KOVAN Semantics" → "Outputs": `symbols.md`,
//! `repository-summary.md`).

use std::path::{Path, PathBuf};

use kovan_common::KovanRepository;
use kovan_semantics::{
    catalogue_symbols, repository_summary_markdown, symbols_markdown, LanguageAdapter,
};

use super::LangArg;

/// `kovan symbols <root> --lang <lang> [--markdown] [--out <path>] [--name <name>]`.
///
/// Default output is line-oriented: `path:line: kind qualified_name`, one
/// symbol per line (sorted by discovery order — see
/// [`catalogue_symbols`]). `--markdown` (or passing `--out`) renders
/// the full `symbols.md` artifact instead.
pub fn run_symbols(
    root: PathBuf,
    lang: LangArg,
    markdown: bool,
    out: Option<PathBuf>,
    name: Option<String>,
) -> Result<(), String> {
    let adapter: LanguageAdapter = lang.into();
    let symbols = catalogue_symbols(&root, adapter).map_err(|e| e.to_string())?;

    if markdown || out.is_some() {
        let repo_name = name.unwrap_or_else(|| default_repo_name(&root));
        let md = symbols_markdown(&repo_name, &symbols);
        return write_or_print(&md, out.as_deref());
    }

    for s in &symbols {
        println!("{}:{}: {} {}", s.file, s.line, s.kind, s.qualified_name);
    }
    Ok(())
}

/// `kovan summary <root> --lang <lang> [--id <id>] [--name <name>] [--out <path>]`
/// — renders `repository-summary.md`. There is no persisted `KovanRepository`
/// catalogue yet, so the repository record is synthesised from `root`'s
/// directory name (or `--id`/`--name`) and `lang`.
pub fn run_summary(
    root: PathBuf,
    lang: LangArg,
    id: Option<String>,
    name: Option<String>,
    out: Option<PathBuf>,
) -> Result<(), String> {
    let adapter: LanguageAdapter = lang.into();
    let symbols = catalogue_symbols(&root, adapter).map_err(|e| e.to_string())?;

    let repo_name = name.unwrap_or_else(|| default_repo_name(&root));
    let repo_id = id.unwrap_or_else(|| repo_name.to_ascii_lowercase().replace(' ', "-"));
    let repo = KovanRepository {
        id: repo_id,
        name: repo_name,
        language: lang.display_name().to_string(),
    };

    let md = repository_summary_markdown(&repo, &symbols);
    write_or_print(&md, out.as_deref())
}

/// The default repository display name: `root`'s final path component, or
/// `"repository"` if `root` has none (e.g. `.` or `/`).
fn default_repo_name(root: &Path) -> String {
    root.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("repository")
        .to_string()
}

/// Write `content` to `out` if given, else print it to stdout.
fn write_or_print(content: &str, out: Option<&Path>) -> Result<(), String> {
    match out {
        Some(path) => {
            std::fs::write(path, content).map_err(|e| format!("write {}: {e}", path.display()))?;
            println!("wrote {}", path.display());
            Ok(())
        }
        None => {
            print!("{content}");
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_repo_name_falls_back_for_dot() {
        assert_eq!(default_repo_name(Path::new(".")), "repository");
    }

    #[test]
    fn default_repo_name_uses_final_component() {
        assert_eq!(default_repo_name(Path::new("/a/b/tampines")), "tampines");
    }
}
