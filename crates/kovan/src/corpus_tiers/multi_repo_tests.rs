//! Several repositories per tier, end to end (GitHub issue #458): real Git
//! repositories in a temporary directory, with local **bare** repositories
//! standing in for the remotes. No network and no real corpus repository is
//! touched; every PDF is synthetic.
//!
//! The fixture is a Kovan folder with two repositories in every tier:
//!
//! | tier | name | mount | remote holds |
//! |---|---|---|---|
//! | standard | `kovan-standard` (built-in) | `literature/standard-corpus` | WASH-1400's corpus file |
//! | standard | `htgr-standard` | `literature/standard-htgr` | NUREG-2201's corpus file |
//! | open | `open` (`[corpora]`) | `literature/open-corpus` | `me-open-corpus/b.pdf` |
//! | open | `open2` (default) | `literature/open2` | `open2-open-corpus/o.pdf` |
//! | proprietary | `proprietary` (`[corpora]`) | `literature/proprietary` | `papers/c.pdf` |
//! | proprietary | `prop2` | `literature/prop2` | `books/d.pdf` |

use super::*;
use crate::ingest::{self, ExistingEntry, IngestChoice, IngestError};
use crate::repository::save_repository_with_message;
use crate::root::{CorporaConfig, KovanRoot, RootError};
use crate::save_push::{push_after_save, push_after_save_warning_at, PushOutcome, PushRepo};
use crate::standard_corpus::{entry, StandardCorpus};
use std::process::Command;

/// `git` in `dir` with a test identity; panics with Git's stderr on failure.
fn g(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["-c", "user.name=t", "-c", "user.email=t@t"])
        .args(["-c", "protocol.file.allow=always"])
        .args(args)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git {args:?} in {}: {}",
        dir.display(),
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

/// A bare repository at `tmp/<name>.git` whose `main` holds `file` with
/// `bytes` (distinct per remote, so no two seeds are duplicates).
fn bare_remote(tmp: &Path, name: &str, file: &str, bytes: &[u8]) -> String {
    let work = tmp.join(format!("{name}-seed"));
    let f = work.join(file);
    std::fs::create_dir_all(f.parent().unwrap()).unwrap();
    std::fs::write(&f, bytes).unwrap();
    g(&work, &["init", "-q", "-b", "main"]);
    g(&work, &["add", "."]);
    g(&work, &["commit", "-q", "-m", "seed"]);
    let bare = tmp.join(format!("{name}.git"));
    g(
        tmp,
        &[
            "clone",
            "-q",
            "--bare",
            &work.to_string_lossy(),
            &bare.to_string_lossy(),
        ],
    );
    bare.to_string_lossy().to_string()
}

/// Every file name in the history of the bare repository at `remote`.
fn listed(remote: &str) -> String {
    g(
        Path::new(remote),
        &["log", "--all", "--name-only", "--format="],
    )
}

fn tip(remote: &str) -> String {
    g(Path::new(remote), &["rev-parse", "refs/heads/main"])
}

/// A tiny valid one-page PDF with an `/Info` `/Title`, as `ingest`'s own
/// tests build.
fn write_test_pdf(path: &Path, title: &str) {
    use lopdf::{dictionary, Document, Object};
    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();
    let page_id = doc.add_object(dictionary! { "Type" => "Page", "Parent" => pages_id });
    let pages =
        dictionary! { "Type" => "Pages", "Kids" => vec![Object::Reference(page_id)], "Count" => 1 };
    doc.objects.insert(pages_id, Object::Dictionary(pages));
    let catalog_id = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
    doc.trailer.set("Root", catalog_id);
    let info_id = doc.add_object(dictionary! { "Title" => Object::string_literal(title) });
    doc.trailer.set("Info", info_id);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    doc.save(path).unwrap();
}

struct Fixture {
    tmp: tempfile::TempDir,
    root: KovanRoot,
    htgr: String,
    open: String,
    open2: String,
    prop: String,
    prop2: String,
    parent: String,
}

fn repo_entry(name: &str, remote: &str, path: &str) -> CorpusRepoConfig {
    CorpusRepoConfig {
        name: name.into(),
        path: path.into(),
        remote: Some(remote.into()),
        branch: Some("main".into()),
        default: false,
        writable: false,
    }
}

/// The two-repositories-per-tier Kovan folder, set up (every repository a
/// submodule), saved and pushed once.
fn fixture() -> Option<Fixture> {
    if !crate::advanced_git::system_git_available() {
        return None;
    }
    let tmp = tempfile::tempdir().unwrap();
    let t = tmp.path();
    let wash = entry("wash-1400").unwrap().corpus_file.unwrap();
    let nureg = entry("nureg-2201").unwrap().corpus_file.unwrap();
    let standard = bare_remote(t, "standard", wash, b"%PDF-1.4 synthetic wash");
    let htgr = bare_remote(t, "htgr", nureg, b"%PDF-1.4 synthetic nureg-2201");
    let open = bare_remote(t, "open", "me-open-corpus/b.pdf", b"%PDF-1.4 open b");
    let open2 = bare_remote(t, "open2", "open2-open-corpus/o.pdf", b"%PDF-1.4 open2 o");
    let prop = bare_remote(t, "prop", "papers/c.pdf", b"%PDF-1.4 prop c");
    let prop2 = bare_remote(t, "prop2", "books/d.pdf", b"%PDF-1.4 prop2 d");
    let parent = t.join("parent.git").to_string_lossy().to_string();
    g(t, &["init", "-q", "--bare", "-b", "main", &parent]);

    let config = crate::root::RootConfig::new("lib", "Lib").with_private_submodule(prop.clone());
    let mut root = KovanRoot::create(&t.join("lib"), config, true).unwrap();
    root.set_corpora(CorporaConfig {
        open_remote: Some(open.clone()),
        proprietary_remote: Some(prop.clone()),
    })
    .unwrap();
    let mut open2_entry = repo_entry("open2", &open2, "literature/open2");
    open2_entry.default = true;
    root.set_repos(RepoTiers {
        standard: vec![repo_entry(
            "htgr-standard",
            &htgr,
            "literature/standard-htgr",
        )],
        open: vec![open2_entry],
        proprietary: vec![repo_entry("prop2", &prop2, "literature/prop2")],
        ..RepoTiers::default()
    })
    .unwrap();

    let setup = crate::corpus_repos::ensure_library_corpora_with(&root, &standard, "main");
    assert!(
        setup.standard.is_ok() && setup.open.is_ok() && setup.proprietary.is_ok(),
        "{setup:?}"
    );
    let others: Vec<(&str, bool)> = setup
        .others
        .iter()
        .map(|(_, n, r)| (n.as_str(), r.is_ok()))
        .collect();
    assert_eq!(
        others,
        [("htgr-standard", true), ("open2", true), ("prop2", true)],
        "{setup:?}"
    );
    g(root.path(), &["remote", "add", "origin", &parent]);
    save_repository_with_message(&root, "").unwrap();
    let first = push_after_save(&root);
    assert!(!first.has_problem(), "{:?}", first.lines());
    Some(Fixture {
        tmp,
        root,
        htgr,
        open,
        open2,
        prop,
        prop2,
        parent,
    })
}

fn dir_of(root: &KovanRoot, name: &str) -> PathBuf {
    root.repo_named(name).unwrap().dir
}

/// Discovery: every repository of every tier is set up as a submodule and
/// found; a standard entry is located in whichever standard repository
/// holds its `corpus_file`, not only the built-in one.
#[test]
fn every_repository_of_every_tier_is_set_up_and_discovered() {
    let Some(f) = fixture() else { return };
    let repos = f.root.corpus_repos();
    assert_eq!(repos.len(), 6);
    assert!(repos.iter().all(|r| r.is_downloaded()), "{repos:?}");
    let gitmodules = std::fs::read_to_string(f.root.path().join(".gitmodules")).unwrap();
    let ignored = std::fs::read_to_string(f.root.path().join(".gitignore")).unwrap();
    for rel in [
        "literature/standard-htgr",
        "literature/open2",
        "literature/prop2",
    ] {
        assert!(gitmodules.contains(rel), "{gitmodules}");
        assert!(ignored.contains(&format!("/{rel}/")), "{ignored}");
    }

    let corpus = StandardCorpus::for_root(Some(&f.root));
    let nureg = entry("nureg-2201").unwrap();
    let located = corpus.locate(nureg).unwrap();
    assert!(
        located.starts_with(dir_of(&f.root, "htgr-standard")),
        "{}",
        located.display()
    );
    let wash = corpus.locate(entry("wash-1400").unwrap()).unwrap();
    assert!(wash.starts_with(f.root.standard_corpus_dir()));

    // The proprietary repositories stay out of the Kovan repository's own
    // tree: only gitlinks are recorded for them.
    let files = g(f.root.path(), &["ls-tree", "-r", "--name-only", "HEAD"]);
    assert!(!files.contains("books/d.pdf") && !files.contains("papers/c.pdf"));
    let links = g(f.root.path(), &["ls-tree", "HEAD", "literature/"]);
    assert!(
        links.contains("commit") && links.contains("literature/prop2"),
        "{links}"
    );
}

/// Ingest: with no choice, an open document goes to the open tier's default
/// repository (`open2`) and a restricted one to the first proprietary one;
/// a chosen repository is honoured and recorded in `[source] repo`; a
/// restricted document cannot be put in a published tier, nor anything in
/// a read-only standard repository.
#[test]
fn ingest_goes_to_the_chosen_repository_and_records_it() {
    let Some(f) = fixture() else { return };
    let incoming = f.tmp.path().join("incoming");
    let choice = |citekey: &str, access, target| IngestChoice {
        citekey: citekey.into(),
        access,
        topics: vec![],
        projects: vec![],
        target,
    };
    let repo_of = |citekey: &str| {
        crate::entity::EntityConfig::load(&f.root.paper_dir(citekey))
            .unwrap()
            .source
            .unwrap()
    };

    let a = incoming.join("a.pdf");
    write_test_pdf(&a, "Open by default");
    let p = ingest::preview(&f.root, &a).unwrap();
    ingest::ingest(
        &f.root,
        &p,
        choice("opendefault", crate::entity::Access::Open, None),
    )
    .unwrap();
    let stored = dir_of(&f.root, "open2").join("open2-open-corpus/opendefault.pdf");
    assert!(stored.is_file(), "into open2's own open-corpus folder");
    assert_eq!(repo_of("opendefault").repo.as_deref(), Some("open2"));

    let b = incoming.join("b.pdf");
    write_test_pdf(&b, "Restricted into prop2");
    let p = ingest::preview(&f.root, &b).unwrap();
    let to_prop2 = Some(RepoRef {
        tier: Tier::Proprietary,
        name: "prop2".into(),
    });
    ingest::ingest(
        &f.root,
        &p,
        choice("secretbook", crate::entity::Access::Restricted, to_prop2),
    )
    .unwrap();
    assert!(dir_of(&f.root, "prop2").join("secretbook.pdf").is_file());
    assert_eq!(repo_of("secretbook").repo.as_deref(), Some("prop2"));

    let c = incoming.join("c.pdf");
    write_test_pdf(&c, "Restricted by default");
    let p = ingest::preview(&f.root, &c).unwrap();
    ingest::ingest(
        &f.root,
        &p,
        choice("secretdefault", crate::entity::Access::Restricted, None),
    )
    .unwrap();
    assert!(f
        .root
        .restricted_sources_dir()
        .join("secretdefault.pdf")
        .is_file());
    assert_eq!(
        repo_of("secretdefault").repo.as_deref(),
        Some("proprietary")
    );

    let d = incoming.join("d.pdf");
    write_test_pdf(&d, "Refused targets");
    let p = ingest::preview(&f.root, &d).unwrap();
    let to_open = Some(RepoRef {
        tier: Tier::Open,
        name: "open".into(),
    });
    let refused = ingest::ingest(
        &f.root,
        &p,
        choice("leak", crate::entity::Access::Restricted, to_open),
    );
    assert!(
        matches!(refused, Err(IngestError::Target { .. })),
        "{refused:?}"
    );
    let to_standard = Some(RepoRef {
        tier: Tier::Standard,
        name: "htgr-standard".into(),
    });
    let refused = ingest::ingest(
        &f.root,
        &p,
        choice("ro", crate::entity::Access::Open, to_standard),
    );
    assert!(
        matches!(&refused, Err(IngestError::Target { reason }) if reason.contains("read-only")),
        "{refused:?}"
    );
    assert!(!f.root.paper_dir("leak").exists() && !f.root.paper_dir("ro").exists());

    // ...and an open document may not be sent to a proprietary repository
    // either (maintainer, 2026-10-06: open and proprietary kept apart).
    let to_proprietary = Some(RepoRef {
        tier: Tier::Proprietary,
        name: "proprietary".into(),
    });
    let refused = ingest::ingest(
        &f.root,
        &p,
        choice("openprop", crate::entity::Access::Open, to_proprietary),
    );
    assert!(
        matches!(&refused, Err(IngestError::Target { reason }) if reason.contains("kept apart")),
        "{refused:?}"
    );
    assert!(!f.root.paper_dir("openprop").exists());

    // The form's choices follow the access.
    let names = |access| {
        ingest::target_choices(&f.root, access)
            .into_iter()
            .map(|r| r.name)
            .collect::<Vec<_>>()
    };
    assert_eq!(
        names(crate::entity::Access::Restricted),
        ["proprietary", "prop2"]
    );
    // Open and proprietary are kept apart (maintainer, 2026-10-06): the
    // dropdown for open literature lists no proprietary repository.
    assert_eq!(
        names(crate::entity::Access::Open),
        ["open", "open2"]
    );
}

/// The duplicate guard hashes across every repository of every tier: a
/// copy of a loose file in the second proprietary repository, of the second
/// open repository's file, and of a corpus document held only in the second
/// standard repository are all refused; the file itself, ingested in place,
/// is not a duplicate of itself.
#[test]
fn duplicates_are_found_across_every_repository() {
    let Some(f) = fixture() else { return };
    let corpus = StandardCorpus::for_root(Some(&f.root));
    let mut cache = crate::fingerprint::HashCache::in_memory();
    let incoming = f.tmp.path().join("copies");
    std::fs::create_dir_all(&incoming).unwrap();

    let copy = |from: PathBuf, name: &str| {
        let to = incoming.join(name);
        std::fs::copy(from, &to).unwrap();
        to
    };
    let d = copy(dir_of(&f.root, "prop2").join("books/d.pdf"), "d-copy.pdf");
    match ingest::find_existing(&f.root, &corpus, &d, &mut cache) {
        Some(ExistingEntry::RepoFile {
            tier,
            repo,
            matched,
            ..
        }) => {
            assert_eq!((tier, repo.as_str()), (Tier::Proprietary, "prop2"));
            assert!(matches!(matched, ingest::MatchKind::SameContent { .. }));
        }
        other => panic!("expected prop2's file, got {other:?}"),
    }
    let o = copy(
        dir_of(&f.root, "open2").join("open2-open-corpus/o.pdf"),
        "o-copy.pdf",
    );
    assert!(matches!(
        ingest::find_existing(&f.root, &corpus, &o, &mut cache),
        Some(ExistingEntry::RepoFile { ref repo, .. }) if repo == "open2"
    ));
    let nureg = corpus.locate(entry("nureg-2201").unwrap()).unwrap();
    let n = copy(nureg.clone(), "nureg-copy.pdf");
    assert!(matches!(
        ingest::find_existing(&f.root, &corpus, &n, &mut cache),
        Some(ExistingEntry::StandardCorpus {
            id: "nureg-2201",
            ..
        })
    ));
    // The loose file itself is an ingest in place, not a duplicate.
    let own = dir_of(&f.root, "prop2").join("books/d.pdf");
    assert_eq!(
        ingest::find_existing(&f.root, &corpus, &own, &mut cache),
        None
    );
    // And a fresh document is not a duplicate of anything.
    let fresh = incoming.join("fresh.pdf");
    std::fs::write(&fresh, b"%PDF-1.4 prop2 X").unwrap(); // same length as d.pdf
    assert_eq!(
        ingest::find_existing(&f.root, &corpus, &fresh, &mut cache),
        None
    );
}

/// Save commits each repository independently and push sends each to its
/// own remote; the read-only standard repositories are neither committed
/// into nor pushed; the parent's gitlinks point at commits the corpus
/// remotes have.
#[test]
fn save_and_push_each_repository_independently() {
    let Some(f) = fixture() else { return };
    let htgr_before = tip(&f.htgr);
    std::fs::write(
        dir_of(&f.root, "open2").join("open2-open-corpus/n.pdf"),
        b"n2",
    )
    .unwrap();
    std::fs::write(dir_of(&f.root, "prop2").join("books/new.pdf"), b"p2").unwrap();
    std::fs::write(
        f.root.restricted_sources_dir().join("papers/new.pdf"),
        b"p1",
    )
    .unwrap();
    std::fs::write(dir_of(&f.root, "htgr-standard").join("stray.pdf"), b"s").unwrap();
    save_repository_with_message(&f.root, "several repos")
        .unwrap()
        .unwrap();

    let report = push_after_save(&f.root);
    assert!(!report.has_problem(), "{:?}", report.lines());
    let names: Vec<(PushRepo, &str)> = report
        .repos
        .iter()
        .map(|r| (r.repo, r.name.as_str()))
        .collect();
    assert_eq!(
        names,
        [
            (PushRepo::ProprietaryCorpus, "proprietary"),
            (PushRepo::ProprietaryCorpus, "prop2"),
            (PushRepo::OpenCorpus, "open"),
            (PushRepo::OpenCorpus, "open2"),
            (PushRepo::KovanRepository, ""),
        ]
    );
    assert!(matches!(
        report.named("prop2"),
        Some(PushOutcome::Pushed { .. })
    ));
    assert!(matches!(
        report.named("open"),
        Some(PushOutcome::UpToDate { .. })
    ));
    assert!(listed(&f.prop2).contains("books/new.pdf"));
    assert!(listed(&f.prop).contains("papers/new.pdf"));
    assert!(listed(&f.open2).contains("open2-open-corpus/n.pdf"));
    for public in [&f.open, &f.open2, &f.htgr] {
        let l = listed(public);
        assert!(!l.contains("books/new.pdf") && !l.contains("papers/new.pdf"));
    }
    assert_eq!(tip(&f.htgr), htgr_before, "standard is never pushed");
    let htgr_dir = dir_of(&f.root, "htgr-standard");
    assert!(g(&htgr_dir, &["status", "--porcelain"]).contains("stray.pdf"));

    let rev = tip(&f.parent);
    let link = g(
        Path::new(&f.parent),
        &["ls-tree", &rev, "--", "literature/prop2"],
    );
    let sha = link.split_whitespace().nth(2).unwrap();
    assert_eq!(sha, tip(&f.prop2), "the parent links prop2's pushed commit");
    for dir in [dir_of(&f.root, "prop2"), dir_of(&f.root, "open2")] {
        let msg = g(&dir, &["log", "-1", "--format=%B"]);
        assert!(msg.contains("several repos"), "{msg}");
    }
}

/// A standard repository configured `writable` is committed into and pushed
/// (for the corpus maintainer), still only to its own remote.
#[test]
fn a_writable_standard_repository_is_committed_and_pushed() {
    let Some(mut f) = fixture() else { return };
    let mut repos = f.root.config().repos.clone();
    repos.standard[0].writable = true;
    f.root.set_repos(repos).unwrap();
    std::fs::write(dir_of(&f.root, "htgr-standard").join("added.pdf"), b"a").unwrap();
    save_repository_with_message(&f.root, "").unwrap().unwrap();
    let report = push_after_save(&f.root);
    assert!(!report.has_problem(), "{:?}", report.lines());
    assert!(matches!(
        report.named("htgr-standard"),
        Some(PushOutcome::Pushed { .. })
    ));
    assert_eq!(report.all(PushRepo::StandardCorpus).len(), 1);
    assert!(listed(&f.htgr).contains("added.pdf"));
    // Ingest may now target it.
    let target = ingest::resolve_target(
        &f.root,
        crate::entity::Access::Open,
        Some(&RepoRef {
            tier: Tier::Standard,
            name: "htgr-standard".into(),
        }),
    );
    assert!(target.is_ok(), "{target:?}");
}

/// The proprietary-remote guard: a second proprietary repository whose
/// remote is listed in `known_public`, or is an open repository's remote,
/// is refused before anything is sent; the other repositories still push,
/// and the Kovan repository is held back.
#[test]
fn a_proprietary_repository_is_never_pushed_to_a_public_remote() {
    let Some(mut f) = fixture() else { return };
    let prop2_dir = dir_of(&f.root, "prop2");
    std::fs::write(prop2_dir.join("books/secret.pdf"), b"secret").unwrap();
    save_repository_with_message(&f.root, "").unwrap().unwrap();
    let before = tip(&f.prop2);

    let mut repos = f.root.config().repos.clone();
    // Listed in another spelling (`file://`): URLs are compared normalised.
    repos.known_public = vec![format!("file://{}", f.prop2)];
    f.root.set_repos(repos).unwrap();
    let report = push_after_save(&f.root);
    match report.named("prop2") {
        Some(PushOutcome::Refused { reason }) => {
            assert!(reason.contains("known_public"), "{reason}")
        }
        other => panic!("expected a refusal, got {other:?}"),
    }
    assert_eq!(tip(&f.prop2), before, "nothing was sent");
    assert!(matches!(
        report.named("proprietary"),
        Some(PushOutcome::UpToDate { .. })
    ));
    assert!(matches!(
        report.get(PushRepo::KovanRepository),
        Some(PushOutcome::Skipped { .. })
    ));

    // prop2 configured with open2's remote: refused, open2 never receives it.
    let mut repos = f.root.config().repos.clone();
    repos.known_public.clear();
    repos.proprietary[0].remote = Some(f.open2.clone());
    f.root.set_repos(repos).unwrap();
    g(&prop2_dir, &["remote", "set-url", "origin", &f.open2]);
    let report = push_after_save(&f.root);
    assert!(matches!(
        report.named("prop2"),
        Some(PushOutcome::Refused { .. })
    ));
    assert!(
        matches!(report.named("open2"), Some(PushOutcome::Refused { .. })),
        "open2's remote is now also a proprietary remote"
    );
    assert!(!listed(&f.open2).contains("secret.pdf"));
}

/// Pulling follows every repository: a second open repository whose remote
/// moved on is fast-forwarded.
#[test]
fn pulling_follows_every_repository() {
    let Some(f) = fixture() else { return };
    let other = f.tmp.path().join("other-clone");
    g(
        f.tmp.path(),
        &["clone", "-q", &f.open2, &other.to_string_lossy()],
    );
    std::fs::write(other.join("open2-open-corpus/late.pdf"), b"late").unwrap();
    g(&other, &["add", "."]);
    g(&other, &["commit", "-q", "-m", "late"]);
    g(&other, &["push", "-q", "origin", "main"]);

    let pulled = crate::save_push::pull_corpora_with(&f.root, "unused", "main");
    let open2 = pulled.iter().find(|p| p.name == "open2").unwrap();
    assert!(
        matches!(
            open2.outcome,
            crate::save_push::CorpusPullOutcome::Updated { .. }
        ),
        "{:?}",
        open2.outcome
    );
    assert!(dir_of(&f.root, "open2")
        .join("open2-open-corpus/late.pdf")
        .is_file());
    assert_eq!(pulled.len(), 6, "one result per repository");
}

/// The size warning reaches the push report.
#[test]
fn a_repository_near_the_size_limit_is_warned_about() {
    let Some(f) = fixture() else { return };
    let report = push_after_save_warning_at(&f.root, 1);
    assert_eq!(report.warnings.len(), 6, "{:?}", report.warnings);
    assert!(report.lines().iter().any(|l| l.starts_with("Warning: ")));
    assert!(!report.has_problem(), "a warning is advice, not a problem");
    assert!(push_after_save(&f.root).warnings.is_empty());
}

/// Backward compatibility: a folder whose `kovan_root.toml` predates
/// `[repos]` (the maintainer's layout, 2026-09-30) opens with one repository
/// per tier at its old paths, and a layout that nests a proprietary
/// repository inside an open one is refused when opened and when written.
#[test]
fn a_single_repository_layout_is_unchanged_and_unsafe_layouts_are_refused() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(
        tmp.path().join(crate::root::ROOT_MARKER),
        r#"schema_version = 1

[library]
id = "teddy-kovan-repo"
name = "Teddy's Kovan"

[paths]
bibliography = "bibliography.bib"
papers = "papers"
topics = "topics"
projects = "projects"
open_sources = "literature/open-corpus"
restricted_sources = "literature/proprietary"
standard_corpus = "literature/standard-corpus"

[private_submodule]
remote = "https://github.com/example/private-literature.git"

[corpora]
open_remote = "https://github.com/theodoreOnzGit/reactor-literature.git"
proprietary_remote = "https://github.com/example/private-literature.git"
"#,
    )
    .unwrap();
    let mut root = KovanRoot::open(tmp.path()).unwrap();
    let repos = root.corpus_repos();
    assert_eq!(repos.len(), 3);
    assert_eq!(repos[1].dir, root.open_corpus_dir());
    assert_eq!(repos[2].dir, root.restricted_sources_dir());
    assert_eq!(root.default_repo(Tier::Open).unwrap().name, LEGACY_OPEN);

    let nested = RepoTiers {
        proprietary: vec![CorpusRepoConfig {
            name: "inside".into(),
            path: "literature/open-corpus/private".into(),
            remote: None,
            branch: None,
            default: false,
            writable: false,
        }],
        ..RepoTiers::default()
    };
    let err = root.set_repos(nested.clone()).unwrap_err();
    assert!(err.contains("must not share or nest"), "{err}");
    let text = std::fs::read_to_string(root.marker_path()).unwrap();
    assert!(!text.contains("[repos]"), "nothing was written");

    let mut config: crate::root::RootConfig = toml::from_str(&text).unwrap();
    config.repos = nested;
    std::fs::write(root.marker_path(), config.to_toml().unwrap()).unwrap();
    assert!(matches!(
        KovanRoot::open(tmp.path()),
        Err(RootError::InvalidRepos { .. })
    ));
}
