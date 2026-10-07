//! `kovan-cli zotero` — Zotero import into a Kovan folder, export to every
//! Zotero format, the format list, duplicate detection and quick search
//! (GitHub #752, epic #747). The work is in [`crate::zotero`]; this is the
//! `clap` surface and the printing.
//!
//! Translators are listed from `Translator::ALL` and their own metadata, so
//! a translator added to kovan-literature appears here with no change.

use std::path::{Path, PathBuf};

use clap::{Subcommand, ValueEnum};

use kovan_discovery::zotero::{
    quick_search, QuickSearchMode, QuickSearchScope, SearchClock, SearchLibrary,
};
use kovan_literature::zotero::translators::Translator;

use crate::root::{KovanRoot, RootConfig};
use crate::zotero::import::{
    detect_source, next_generated_key_index, plan, read_source, write, ImportSource, PlanOptions,
    SourceOptions, WriteOptions,
};
use crate::zotero::load::{as_library, load_items, CorpusItem};
use crate::zotero::protected_location;

/// `kovan-cli zotero <subcommand>`.
#[derive(Subcommand)]
pub enum ZoteroCommand {
    /// List the Zotero translators: format name, directions, label, file
    /// extension.
    Formats,
    /// Import a Zotero data folder (the folder holding `zotero.sqlite`) or a
    /// file in any importable format into a Kovan folder, as papers.
    Import {
        /// A Zotero data folder, or a file (BibTeX, RIS, CSL JSON, ...).
        source: PathBuf,
        /// The Kovan folder to import into (it must already be one unless
        /// `--init`).
        #[arg(long)]
        to: PathBuf,
        /// The file's format (`kovan-cli zotero formats`); detected when
        /// omitted. Ignored for a data folder.
        #[arg(long)]
        format: Option<String>,
        /// Import items in Zotero's trash too.
        #[arg(long)]
        include_trashed: bool,
        /// Copy each PDF into the folder's proprietary repository instead of
        /// referencing it where it is.
        #[arg(long)]
        copy_attachments: bool,
        /// Report what would be written, and write nothing.
        #[arg(long)]
        dry_run: bool,
        /// Create a new Kovan folder at `--to` first.
        #[arg(long)]
        init: bool,
        /// File every imported paper under this topic path (repeatable);
        /// default `unsorted`.
        #[arg(long = "topic")]
        topics: Vec<String>,
        /// Zotero's "Linked Attachment Base Directory", to resolve linked
        /// files stored relative to it.
        #[arg(long)]
        base_attachment_path: Option<PathBuf>,
        /// Allow a target inside the outram-park-backend repository or
        /// reactor-literature (refused by default: data policy).
        #[arg(long)]
        allow_inside_repo: bool,
    },
    /// Export kovan documents in a Zotero format.
    Export {
        /// The format (`kovan-cli zotero formats`, export direction).
        #[arg(long)]
        format: String,
        /// Kovan folders, `.json` KovanDocument files or folders of them
        /// (default: the Kovan folder containing the current directory).
        #[arg(long, num_args = 1..)]
        from: Vec<PathBuf>,
        /// The output file (default: standard output).
        #[arg(short = 'o', long)]
        out: Option<PathBuf>,
        /// Overwrite an existing output file.
        #[arg(long)]
        force: bool,
        /// Allow an output file inside the outram-park-backend repository or
        /// reactor-literature (refused by default: data policy).
        #[arg(long)]
        allow_inside_repo: bool,
    },
    /// Report sets of probable duplicates (Zotero's duplicate detection).
    /// Report only; nothing is merged or changed.
    Duplicates {
        /// Kovan folders, `.json` KovanDocument files or folders of them.
        #[arg(required = true)]
        corpus: Vec<PathBuf>,
    },
    /// Zotero's quick search over a corpus: prints the matching papers.
    Search {
        /// A Kovan folder, `.json` KovanDocument file or folder of them.
        corpus: PathBuf,
        /// The quick-search text.
        text: String,
        /// Which fields are searched.
        #[arg(long, value_enum, default_value = "title-creator-year")]
        mode: SearchModeArg,
    },
}

/// `clap`-facing mirror of [`QuickSearchMode`].
#[derive(Clone, Copy, ValueEnum)]
pub enum SearchModeArg {
    /// Title, creator, year.
    TitleCreatorYear,
    /// All fields and tags.
    Fields,
    /// Everything, including notes.
    Everything,
}

impl From<SearchModeArg> for QuickSearchMode {
    fn from(m: SearchModeArg) -> Self {
        match m {
            SearchModeArg::TitleCreatorYear => QuickSearchMode::TitleCreatorYear,
            SearchModeArg::Fields => QuickSearchMode::Fields,
            SearchModeArg::Everything => QuickSearchMode::Everything,
        }
    }
}

/// Dispatch a parsed [`ZoteroCommand`].
pub fn run(command: ZoteroCommand) -> Result<(), String> {
    match command {
        ZoteroCommand::Formats => {
            formats();
            Ok(())
        }
        ZoteroCommand::Import {
            source,
            to,
            format,
            include_trashed,
            copy_attachments,
            dry_run,
            init,
            topics,
            base_attachment_path,
            allow_inside_repo,
        } => import(ImportArgs {
            source,
            to,
            format,
            include_trashed,
            copy_attachments,
            dry_run,
            init,
            topics,
            base_attachment_path,
            allow_inside_repo,
        }),
        ZoteroCommand::Export {
            format,
            from,
            out,
            force,
            allow_inside_repo,
        } => export(&format, from, out.as_deref(), force, allow_inside_repo),
        ZoteroCommand::Duplicates { corpus } => duplicates(&corpus),
        ZoteroCommand::Search { corpus, text, mode } => search(&corpus, &text, mode.into()),
    }
}

/// The directions of `t`: `import`, `export` or `import,export`.
fn directions(t: Translator) -> &'static str {
    let m = t.metadata();
    match (m.can_import(), m.can_export()) {
        (true, true) => "import,export",
        (true, false) => "import",
        (false, true) => "export",
        (false, false) => "-",
    }
}

fn formats() {
    for t in Translator::ALL {
        let m = t.metadata();
        println!(
            "{}\t{}\t{}\t.{}",
            t.format_name(),
            directions(t),
            m.label,
            m.target
        );
    }
}

fn refuse_protected(path: &Path, allow: bool) -> Result<(), String> {
    match protected_location(path) {
        Some((dir, what)) if !allow => Err(format!(
            "refusing to write {} inside {what} ({}): a Zotero library is your own data and \
             goes in your own Kovan folder (DATA_POLICY.md). Pass --allow-inside-repo to \
             override.",
            path.display(),
            dir.display()
        )),
        _ => Ok(()),
    }
}

struct ImportArgs {
    source: PathBuf,
    to: PathBuf,
    format: Option<String>,
    include_trashed: bool,
    copy_attachments: bool,
    dry_run: bool,
    init: bool,
    topics: Vec<String>,
    base_attachment_path: Option<PathBuf>,
    allow_inside_repo: bool,
}

fn import(a: ImportArgs) -> Result<(), String> {
    refuse_protected(&a.to, a.allow_inside_repo)?;
    let source = detect_source(&a.source, a.format.as_deref())?;
    // A dry run with --init plans against an empty folder made in the
    // system temp dir (removed afterwards), so nothing appears at --to.
    let mut scratch: Option<PathBuf> = None;
    let root = if KovanRoot::is_root(&a.to) {
        KovanRoot::open(&a.to).map_err(|e| e.to_string())?
    } else if a.init {
        let name =
            a.to.file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| "kovan".into());
        let dir = if a.dry_run {
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0);
            let d = std::env::temp_dir().join(format!(
                "kovan-zotero-dry-run-{}-{nanos}",
                std::process::id()
            ));
            scratch = Some(d.clone());
            d
        } else {
            a.to.clone()
        };
        let root = KovanRoot::create(&dir, RootConfig::new(name.clone(), name), !a.dry_run)
            .map_err(|e| e.to_string())?;
        if !a.dry_run {
            println!("created: {}", a.to.display());
        }
        root
    } else {
        return Err(format!(
            "{} is not a Kovan folder (no kovan_root.toml); pass --init to create one there",
            a.to.display()
        ));
    };
    let result = import_into(&a, &source, &root);
    if let Some(d) = scratch {
        let _ = std::fs::remove_dir_all(d);
    }
    result
}

fn import_into(a: &ImportArgs, source: &ImportSource, root: &KovanRoot) -> Result<(), String> {
    let description = match source {
        ImportSource::DataFolder(dir) => format!("Zotero data folder {}", dir.display()),
        ImportSource::File { path, translator } => format!(
            "{} ({} import translator)",
            path.display(),
            translator.metadata().label
        ),
    };
    println!("source: {description}");
    let folder = read_source(
        source,
        &SourceOptions {
            base_attachment_path: a.base_attachment_path.clone(),
            first_key_index: next_generated_key_index(root),
        },
    )?;
    let p = plan(
        root,
        &folder,
        &PlanOptions {
            include_trashed: a.include_trashed,
            topics: a.topics.clone(),
        },
    )?;
    for (wanted, used) in &p.renamed {
        println!("renamed-citekey: {wanted} -> {used}");
    }
    for (id, citekey) in &p.already_present {
        println!("already-present: {id} {citekey}");
    }
    for id in &p.attachment_missing {
        println!("attachment-missing: {id}");
    }
    for line in p.losses.summary() {
        println!("not-carried: {line}");
    }
    let with_file = p.papers.iter().filter(|x| x.attachment.is_some()).count();
    println!("papers: {}", p.papers.len());
    println!("already_present: {}", p.already_present.len());
    println!("renamed: {}", p.renamed.len());
    println!("attachments_on_disk: {with_file}");
    if a.dry_run {
        println!("dry-run: nothing written");
        return Ok(());
    }
    let out = write(
        root,
        &p,
        &WriteOptions {
            copy_attachments: a.copy_attachments,
            topics: a.topics.clone(),
            source_description: description,
        },
    )?;
    println!("written: {}", out.papers.len());
    println!("attachments_copied: {}", out.copied.len());
    println!("attachments_referenced: {}", out.referenced);
    if let Some(r) = &out.report {
        println!("report: {}", r.display());
    }
    Ok(())
}

fn export(
    format: &str,
    from: Vec<PathBuf>,
    out: Option<&Path>,
    force: bool,
    allow_inside_repo: bool,
) -> Result<(), String> {
    let t = Translator::from_format_name(format).ok_or_else(|| {
        format!("unknown format {format:?}; `kovan-cli zotero formats` lists them")
    })?;
    if !t.metadata().can_export() {
        return Err(format!("{} ({format}) is import-only", t.metadata().label));
    }
    if let Some(o) = out {
        refuse_protected(o, allow_inside_repo)?;
        if o.exists() && !force {
            return Err(format!(
                "{} exists; pass --force to overwrite it",
                o.display()
            ));
        }
    }
    let from = if from.is_empty() {
        let cwd = std::env::current_dir().map_err(|e| e.to_string())?;
        vec![KovanRoot::discover(&cwd)
            .map_err(|e| format!("no --from and {e}"))?
            .path()
            .to_path_buf()]
    } else {
        from
    };
    let (items, warnings) = load_items(&from)?;
    for w in &warnings {
        eprintln!("warning: {w}");
    }
    let zotero: Vec<_> = items.iter().map(|c| c.item.clone()).collect();
    let text = t
        .export_zotero_items(&zotero, &t.default_options())
        .map_err(|e| format!("{} export: {e:?}", t.metadata().label))?;
    match out {
        Some(o) => {
            std::fs::write(o, &text).map_err(|e| format!("{}: {e}", o.display()))?;
            let lossless = items.iter().filter(|c| c.lossless).count();
            eprintln!(
                "exported: {} items ({lossless} from stored Zotero items) to {}",
                items.len(),
                o.display()
            );
        }
        None => print!("{text}"),
    }
    Ok(())
}

fn title(c: &CorpusItem) -> String {
    c.item
        .field_via_base(kovan_common::zotero::Field::Title)
        .unwrap_or("")
        .to_owned()
}

fn duplicates(corpus: &[PathBuf]) -> Result<(), String> {
    let (items, warnings) = load_items(corpus)?;
    for w in &warnings {
        eprintln!("warning: {w}");
    }
    let lib = as_library(&items);
    let sets = kovan_semantics::zotero::find_duplicates(&lib, &Default::default());
    for (n, set) in sets.sets.iter().enumerate() {
        println!("set {}:", n + 1);
        for &i in set {
            println!("  {}\t{}", items[i].label, title(&items[i]));
        }
    }
    println!("duplicate_sets: {}", sets.sets.len());
    Ok(())
}

fn search(corpus: &Path, text: &str, mode: QuickSearchMode) -> Result<(), String> {
    let (items, warnings) = load_items(&[corpus.to_path_buf()])?;
    for w in &warnings {
        eprintln!("warning: {w}");
    }
    let lib = as_library(&items);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let search_lib = SearchLibrary::new(&lib, SearchClock::utc(now));
    let s =
        quick_search(text, mode, &QuickSearchScope::Library, &[]).map_err(|e| format!("{e:?}"))?;
    let keys = s.run(&search_lib).map_err(|e| format!("{e:?}"))?;
    let mut hits = 0;
    for c in &items {
        if c.item.key.as_ref().is_some_and(|k| keys.contains(k)) {
            println!("{}\t{}", c.label, title(c));
            hits += 1;
        }
    }
    println!("matches: {hits}");
    Ok(())
}
