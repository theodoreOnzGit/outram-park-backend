//! `kovan-cli zotero lookup <identifier or text>` (GitHub #756): Zotero's
//! "Add Item by Identifier". Finds a DOI, ISBN, arXiv ID, ADS bibcode or
//! PMID in the argument, fetches its record with the ported search
//! translators (`kovan_literature::zotero::search`) over the network
//! ([`crate::lookup_net`]; the user asked, so the network is used), prints
//! it, and with `--save --to <folder>` writes it as a `KovanDocument` JSON
//! (the fetched Zotero item kept in `zotero_item`). It never overwrites a
//! file, and refuses to write inside the outram-park-backend repository
//! (a user's corpus is not this repository).
//!
//! A failed lookup prints the reason (offline, timeout, HTTP status,
//! rate-limited, not found, ...) and writes nothing.

use std::path::{Path, PathBuf};

use clap::Args;
use kovan_common::zotero::ZoteroItem;
use kovan_common::KovanDocument;
use kovan_literature::zotero::framework::JsObject;
use kovan_literature::zotero::search::{
    identifier_for_search, Identifier, LookupError, LookupSession, SearchOptions, SearchRun,
};

use crate::lookup_net::HttpBackend;

/// `kovan-cli zotero lookup`.
#[derive(Args, Debug)]
pub struct LookupArgs {
    /// An identifier (DOI, ISBN, arXiv ID, ADS bibcode, PMID), or text
    /// containing one (the first one found is used, as Zotero does).
    pub text: String,
    /// Write the record as a KovanDocument JSON file (needs `--to`).
    #[arg(long, requires = "to")]
    pub save: bool,
    /// The corpus folder to write into (outside this repository).
    #[arg(long)]
    pub to: Option<PathBuf>,
    /// Print the record as Zotero Web API JSON instead of a summary.
    #[arg(long)]
    pub json: bool,
    /// Your email, sent as `mailto:` in the User-Agent and to Crossref's
    /// polite pool. Never sent unless given.
    #[arg(long)]
    pub mailto: Option<String>,
}

/// The options a lookup runs with: the email, if the user gave one, as the
/// Crossref REST hidden pref (`CrossrefREST.email`).
pub fn search_options(mailto: Option<&str>) -> SearchOptions {
    let mut o = SearchOptions::default();
    if let Some(m) = mailto.filter(|m| !m.trim().is_empty()) {
        let mut p = JsObject::new();
        p.set("CrossrefREST.email", m.trim());
        o.hidden_prefs = p;
    }
    o
}

/// Look `id` up over the network (the caller has the user's go-ahead).
pub fn lookup_identifier(id: Identifier, mailto: Option<&str>) -> Result<SearchRun, LookupError> {
    let backend = HttpBackend::for_this_build(mailto);
    LookupSession::for_identifier(id, search_options(mailto)).run_with(|req| backend.fetch(req))
}

/// The run's items as kovan-common Zotero items (the first is the record;
/// notes follow as child items).
pub fn run_items(run: &SearchRun) -> Result<Vec<ZoteroItem>, String> {
    run.zotero_items(&now_iso()).map_err(|e| e.to_string())
}

fn now_iso() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    kovan_literature::zotero::search::context::epoch_to_iso(secs)
}

/// Run `kovan-cli zotero lookup`.
pub fn run(args: LookupArgs) -> Result<(), String> {
    let id =
        identifier_for_search(&args.text).ok_or_else(|| LookupError::NoIdentifier.to_string())?;
    println!("identifier: {} {}", id.kind(), id.value());
    // Check the destination before going online.
    let target = if args.save {
        Some(check_target(
            args.to.as_deref().ok_or("--save needs --to <folder>")?,
        )?)
    } else {
        None
    };
    let run =
        lookup_identifier(id, args.mailto.as_deref()).map_err(|e| format!("lookup failed: {e}"))?;
    println!("translator: {}", run.used.metadata().label);
    if args.json {
        let json =
            serde_json::to_string_pretty(&run.api_json(&now_iso())).map_err(|e| e.to_string())?;
        println!("{json}");
    }
    let items = run_items(&run)?;
    let Some(record) = items.iter().find(|i| i.item_type.as_str() != "note") else {
        return Err("lookup returned no record".to_owned());
    };
    if !args.json {
        print_record(record);
    }
    if let Some(dir) = target {
        let doc = KovanDocument::from_zotero_item(record);
        let path = write_new(&dir, &doc)?;
        println!("saved: {}", path.display());
    }
    Ok(())
}

fn print_record(item: &ZoteroItem) {
    let doc = KovanDocument::from_zotero_item(item);
    println!("item_type: {}", item.item_type.as_str());
    println!("title: {}", doc.title);
    println!(
        "authors: {}",
        doc.authors
            .iter()
            .map(|a| if a.given.is_empty() {
                a.family.clone()
            } else {
                format!("{}, {}", a.family, a.given)
            })
            .collect::<Vec<_>>()
            .join("; ")
    );
    println!(
        "year: {}",
        doc.year
            .map(|y| y.to_string())
            .unwrap_or_else(|| "-".into())
    );
    println!("doi: {}", doc.doi.as_deref().unwrap_or("-"));
    println!("journal: {}", doc.journal.as_deref().unwrap_or("-"));
    println!("publisher: {}", doc.publisher.as_deref().unwrap_or("-"));
    println!("volume: {}", doc.volume.as_deref().unwrap_or("-"));
    println!("issue: {}", doc.number.as_deref().unwrap_or("-"));
    println!("pages: {}", doc.pages.as_deref().unwrap_or("-"));
    println!("url: {}", doc.source_url.as_deref().unwrap_or("-"));
}

/// Whether `dir` lies inside the outram-park-backend repository (a folder
/// above it holds `crates/kovan-literature/Cargo.toml`).
pub fn inside_this_repository(dir: &Path) -> bool {
    let Ok(abs) = std::path::absolute(dir) else {
        return true;
    };
    // The folder may not exist yet: test its nearest existing ancestor.
    let mut p = abs.as_path();
    loop {
        if p.join("crates/kovan-literature/Cargo.toml").is_file() && p.join("Cargo.toml").is_file()
        {
            return true;
        }
        match p.parent() {
            Some(parent) => p = parent,
            None => return false,
        }
    }
}

fn check_target(dir: &Path) -> Result<PathBuf, String> {
    if inside_this_repository(dir) {
        return Err(format!(
            "refusing to write into {}: it is inside the outram-park-backend repository; choose your own corpus folder",
            dir.display()
        ));
    }
    if dir.exists() && !dir.is_dir() {
        return Err(format!("{} is not a folder", dir.display()));
    }
    Ok(dir.to_owned())
}

/// Write `doc` as `<dir>/<slug>.json`, never replacing an existing file.
pub fn write_new(dir: &Path, doc: &KovanDocument) -> Result<PathBuf, String> {
    use std::io::Write;
    std::fs::create_dir_all(dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
    let slug = if doc.slug.is_empty() {
        "lookup".to_owned()
    } else {
        doc.slug.clone()
    };
    let path = dir.join(format!("{slug}.json"));
    let json = serde_json::to_string_pretty(doc).map_err(|e| e.to_string())?;
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(|e| match e.kind() {
            std::io::ErrorKind::AlreadyExists => {
                format!("{} already exists; not overwritten", path.display())
            }
            _ => format!("write {}: {e}", path.display()),
        })?;
    f.write_all(json.as_bytes())
        .map_err(|e| format!("write {}: {e}", path.display()))?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refuses_this_repository() {
        let here = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        assert!(inside_this_repository(&here.join("some/new/folder")));
        assert!(check_target(&here).is_err());
    }

    #[test]
    fn never_overwrites() {
        let dir = std::env::temp_dir().join(format!("kovan-lookup-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let doc = KovanDocument::new(
            "id",
            "slug",
            kovan_common::Visibility::Open,
            kovan_common::DocumentType::Paper,
            "T",
        );
        assert!(write_new(&dir, &doc).is_ok());
        let e = write_new(&dir, &doc).unwrap_err();
        assert!(e.contains("not overwritten"), "{e}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn email_only_when_given() {
        assert!(search_options(None).hidden_prefs.is_empty());
        assert_eq!(
            search_options(Some("a@b.org"))
                .hidden_prefs
                .get_str("CrossrefREST.email"),
            Some("a@b.org")
        );
    }
}
