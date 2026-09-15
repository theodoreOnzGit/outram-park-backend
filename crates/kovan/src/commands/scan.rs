//! `kovan-cli scan` — a ripgrep-first scan of a repository for probable
//! definition lines of a given language (see
//! [`kovan_semantics::rough_definition_scan`]). This is the cheap heuristic
//! pre-filter; [`super::symbols`] is the real named-symbol catalogue.

use std::path::PathBuf;

use super::LangArg;

/// Print `path:line: text` for every probable-definition line found.
pub fn run(root: PathBuf, lang: LangArg) -> Result<(), String> {
    let hits =
        kovan_semantics::rough_definition_scan(&root, lang.into()).map_err(|e| e.to_string())?;
    for (path, m) in hits {
        println!("{}:{}: {}", path.display(), m.line, m.text);
    }
    Ok(())
}
