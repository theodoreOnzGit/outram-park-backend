//! V&V of `kovan_literature::zotero::local_library` (GitHub #750): reading a
//! Zotero data folder, code-to-code against Zotero's own schema and save
//! paths.
//!
//! **Methodology.** Every database here is built from upstream Zotero's own
//! files and code paths (Zotero commit 9cbba8c4d281, AGPL-3.0, (c) Corporation
//! for Digital Scholarship; maintainer-authorised 2026-10-07): `system.sql`,
//! `userdata.sql` (userdata 130) and `triggers.sql` are executed verbatim
//! (copies in `tests/data/zotero/`), the global-schema tables are filled as
//! `_updateGlobalSchema` fills them, and objects are inserted with the SQL of
//! `Zotero.Item#_saveData` and its collection/search/tag/relation
//! counterparts (`tests/zotero_db/mod.rs`, each statement cited). Upstream's
//! triggers and foreign keys are on, so a row Zotero would refuse is refused
//! here too. The reader's output is then compared with **exactly** what was
//! inserted, in the JSON shape `Zotero.Item#toJSON` writes.
//!
//! The item fixture is upstream's `test/tests/data/itemJSON.js` (37 regular
//! items, one per item type, every field; `tests/data/zotero/itemJSON.json`,
//! identical to kovan-common's copy).
//!
//! **Predictions, written before the first run (2026-10-07).** (1) all 37
//! itemJSON items read back equal to the fixture, field by field; (2) the
//! hand-built library reads back exactly, including the children, the trash
//! flag, the attachment files and the search-condition conversions; (3) the
//! same with ids handed out in reverse order (no id is assumed); (4) an item
//! present only in the WAL of a database held open in EXCLUSIVE locking mode
//! is read, and the data folder's files are byte-identical afterwards; (5) a
//! corrupt `zotero.sqlite` falls back to `zotero.sqlite.bak`; (6) userdata 77
//! is refused as too old, compatibility 10 as too new.
//!
//! **Results (2026-10-07, `cargo test --release -p kovan-literature`):** all
//! six predictions held on the first run; see each test's doc comment.
//! **The tests can fail:** with two deliberate defects in the reader (date
//! fields returned in their stored multipart form, automatic tags read as
//! manual) 4 of the 8 tests failed (2026-10-07); the defects were reverted.

#![cfg(not(any(target_arch = "wasm32", target_os = "android")))]

mod zotero_db;

use kovan_common::zotero::{ZoteroCollection, ZoteroItem};
use kovan_common::KovanDocument;
use kovan_literature::zotero::local_library::import::{import, write_documents, ImportOptions};
use kovan_literature::zotero::local_library::{
    read_data_folder, AttachmentFile, DbSource, LibraryKind, ReadOptions, ZoteroDbError,
};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use zotero_db::{IdOrder, ZoteroDb};

const ITEM_JSON: &str = include_str!("data/zotero/itemJSON.json");

fn item(v: Value) -> ZoteroItem {
    ZoteroItem::from_json_value(&v).unwrap()
}

fn collection(v: Value) -> ZoteroCollection {
    ZoteroCollection::from_json_value(&v).unwrap()
}

/// **Result (2026-10-07, `cargo test --release`): pass, 37 of 37.** Each of
/// upstream's itemJSON items is saved through the `_saveData` SQL (date
/// fields stored multipart, `accessDate` and `dateAdded` stored as SQL
/// dates, creators deduplicated through `getIDFromData`) and read back equal
/// to the fixture as `ZoteroItem`s, i.e. every field, creator (including the
/// single-field "Institutional Author"), key, version and date. Nothing is
/// dropped or skipped in the read report.
#[test]
fn every_upstream_item_json_reads_back_exactly() {
    let tmp = tempfile::tempdir().unwrap();
    let db = ZoteroDb::create(tmp.path(), IdOrder::Forward);
    let fixture: serde_json::Map<String, Value> = serde_json::from_str(ITEM_JSON).unwrap();
    let mut expected = Vec::new();
    for v in fixture.values() {
        let it = item(v.clone());
        db.save_item(1, &it);
        expected.push(it);
    }
    drop(db);
    let folder = read_data_folder(tmp.path(), &ReadOptions::default()).unwrap();
    assert_eq!(folder.libraries.len(), 1);
    let got = &folder.libraries[0].contents.items;
    assert_eq!(got.len(), 37);
    let mut equal = 0;
    for e in &expected {
        let g = folder.libraries[0]
            .contents
            .item(e.key.as_deref().unwrap())
            .unwrap();
        assert_eq!(g, e, "item type {}", e.item_type.as_str());
        equal += 1;
    }
    assert_eq!(equal, 37);
    assert!(
        folder.report.skipped.is_empty(),
        "{:?}",
        folder.report.skipped
    );
    assert!(
        folder.report.dropped.is_empty(),
        "{:?}",
        folder.report.dropped
    );
    assert_eq!(folder.schema.userdata, 130);
    assert_eq!(folder.source, DbSource::LiveCopy);
}

/// The hand-built library of [`build_library`]: what is inserted, and the
/// JSON `toJSON` would write for it.
struct Fixture {
    tmp: tempfile::TempDir,
    expected_items: Vec<ZoteroItem>,
    expected_collections: Vec<ZoteroCollection>,
    group_lib: i64,
}

fn data_dir(f: &Fixture) -> &Path {
    f.tmp.path()
}

/// A user library exercising every table the reader reads, a group library
/// and a feed library.
fn build_library(order: IdOrder) -> Fixture {
    let tmp = tempfile::tempdir().unwrap();
    let db = ZoteroDb::create(tmp.path(), order);
    let uri = "http://zotero.org/users/local/Ab3dE6gH";

    let parent = collection(json!({"key": "COLLPAR1", "version": 3, "name": "Reactors",
        "parentCollection": false, "relations": {}}));
    let child = collection(json!({"key": "COLLCHI2", "version": 4, "name": "HTGR",
        "parentCollection": "COLLPAR1",
        "relations": {"owl:sameAs": ["http://zotero.org/groups/1/collections/ABCDEFGH"]}}));
    let trashed = collection(json!({"key": "COLLDEL3", "version": 1, "name": "Old",
        "parentCollection": false, "relations": {}, "deleted": true}));
    for c in [&parent, &child, &trashed] {
        db.save_collection(1, c);
    }

    let article = item(json!({
        "key": "ARTICLE1", "version": 12, "itemType": "journalArticle",
        "title": "Pebble bed heat transfer", "date": "March 2010",
        "publicationTitle": "Nuclear Engineering and Design", "volume": "240",
        "pages": "1-10", "DOI": "10.1016/j.nucengdes.2009.01.001",
        "abstractNote": "Line one\nline two", "accessDate": "2020-01-02T03:04:05Z",
        "extra": "Note: kept", "creators": [
            {"creatorType": "author", "firstName": "Ada", "lastName": "Lovelace"},
            {"creatorType": "author", "name": "Oak Ridge National Laboratory"},
            {"creatorType": "editor", "firstName": "Enrico", "lastName": "Fermi"}],
        "tags": [{"tag": "pebble bed"}, {"tag": "HTGR", "type": 1}],
        "collections": ["COLLPAR1", "COLLCHI2"],
        "relations": {"dc:relation": [format!("{uri}/items/REPORT01")]},
        "dateAdded": "2015-04-12T09:00:22Z", "dateModified": "2016-05-13T10:01:23Z"}));
    let report = item(json!({
        "key": "REPORT01", "version": 2, "itemType": "report", "title": "HTR-10 benchmark",
        "institution": "IAEA", "creators": [], "tags": [], "collections": [],
        "relations": {"dc:relation": [format!("{uri}/items/ARTICLE1")]},
        "inPublications": true,
        "dateAdded": "2015-04-12T09:00:22Z", "dateModified": "2015-04-12T09:00:22Z"}));
    let in_trash = item(json!({
        "key": "TRASHED1", "version": 1, "itemType": "book", "title": "Discarded",
        "creators": [], "tags": [], "collections": [], "relations": {}, "deleted": true,
        "dateAdded": "2015-04-12T09:00:22Z", "dateModified": "2015-04-12T09:00:22Z"}));
    let child_note = item(json!({
        "key": "NOTE0001", "version": 1, "itemType": "note", "parentItem": "ARTICLE1",
        "note": "<p>Read <b>section 3</b></p>", "tags": [{"tag": "todo"}], "relations": {},
        "dateAdded": "2015-04-12T09:00:22Z", "dateModified": "2015-04-12T09:00:22Z"}));
    let pdf = item(json!({
        "key": "PDFSTOR1", "version": 5, "itemType": "attachment", "parentItem": "ARTICLE1",
        "title": "Full Text PDF", "linkMode": "imported_file", "contentType": "application/pdf",
        "charset": "", "filename": "paper.pdf", "md5": "0123456789abcdef0123456789abcdef",
        "mtime": 1428829222000_i64, "lastRead": 1700000000, "tags": [], "relations": {},
        "dateAdded": "2015-04-12T09:00:22Z", "dateModified": "2015-04-12T09:00:22Z"}));
    let snapshot = item(json!({
        "key": "SNAPSHT1", "version": 5, "itemType": "attachment", "parentItem": "ARTICLE1",
        "title": "Snapshot", "url": "https://example.org/a", "linkMode": "imported_url",
        "contentType": "text/html", "charset": "utf-8", "filename": "a.html",
        "note": "<p>attachment note</p>", "tags": [], "relations": {},
        "accessDate": "2021-02-03T04:05:06Z",
        "dateAdded": "2015-04-12T09:00:22Z", "dateModified": "2015-04-12T09:00:22Z"}));
    let linked = item(json!({
        "key": "LINKED01", "version": 1, "itemType": "attachment", "parentItem": "REPORT01",
        "title": "report.pdf", "linkMode": "linked_file", "contentType": "application/pdf",
        "charset": "", "path": "/srv/papers/report.pdf", "tags": [], "relations": {},
        "dateAdded": "2015-04-12T09:00:22Z", "dateModified": "2015-04-12T09:00:22Z"}));
    let relative = item(json!({
        "key": "LINKREL1", "version": 1, "itemType": "attachment", "parentItem": "REPORT01",
        "title": "x.pdf", "linkMode": "linked_file", "contentType": "application/pdf",
        "charset": "", "path": "attachments:sub/x.pdf", "tags": [], "relations": {},
        "dateAdded": "2015-04-12T09:00:22Z", "dateModified": "2015-04-12T09:00:22Z"}));
    let link_url = item(json!({
        "key": "LINKURL1", "version": 1, "itemType": "attachment", "parentItem": "REPORT01",
        "title": "Home page", "url": "https://www.iaea.org", "linkMode": "linked_url",
        "contentType": "", "charset": "", "tags": [], "relations": {},
        "dateAdded": "2015-04-12T09:00:22Z", "dateModified": "2015-04-12T09:00:22Z"}));
    let highlight = item(json!({
        "key": "ANNOHIL1", "version": 1, "itemType": "annotation", "parentItem": "PDFSTOR1",
        "annotationType": "highlight", "annotationAuthorName": "",
        "annotationText": "the Nusselt number", "annotationComment": "<p>key</p>",
        "annotationColor": "#ffd400", "annotationPageLabel": "3",
        "annotationSortIndex": "00002|001234|00056",
        "annotationPosition": "{\"pageIndex\":2,\"rects\":[[1,2,3,4]]}",
        "tags": [{"tag": "important"}], "relations": {},
        "dateAdded": "2015-04-12T09:00:22Z", "dateModified": "2015-04-12T09:00:22Z"}));
    let mut ink = item(json!({
        "key": "ANNOINK1", "version": 1, "itemType": "annotation", "parentItem": "PDFSTOR1",
        "annotationType": "ink", "annotationAuthorName": "Theodore",
        "annotationComment": "", "annotationColor": "#000000", "annotationPageLabel": "",
        "annotationSortIndex": "00003|000000|00000",
        "annotationPosition": "{\"pageIndex\":3,\"width\":2,\"paths\":[[1,2,3,4]]}",
        "tags": [], "relations": {},
        "dateAdded": "2015-04-12T09:00:22Z", "dateModified": "2015-04-12T09:00:22Z"}));
    ink.other
        .insert("annotationIsExternal".into(), Value::Bool(true));
    let standalone = item(json!({
        "key": "NOTE0002", "version": 1, "itemType": "note", "note": "<p>loose note</p>",
        "tags": [], "collections": ["COLLPAR1"], "relations": {},
        "dateAdded": "2015-04-12T09:00:22Z", "dateModified": "2015-04-12T09:00:22Z"}));
    let expected_items = vec![
        article.clone(),
        report.clone(),
        in_trash.clone(),
        child_note.clone(),
        pdf.clone(),
        snapshot.clone(),
        linked.clone(),
        relative.clone(),
        link_url.clone(),
        highlight.clone(),
        ink.clone(),
        standalone.clone(),
    ];
    for it in &expected_items {
        db.save_item(1, it);
    }
    // A field that is not valid for the item type, as an old or foreign
    // database may hold one: `setField(.., loadIn)` ignores it (item.js:808).
    let article_id: i64 = db
        .conn
        .query_row("SELECT itemID FROM items WHERE key='ARTICLE1'", [], |r| {
            r.get(0)
        })
        .unwrap();
    db.put_item_data(article_id, db.field_id("runningTime"), "90 min");
    // A plain-text (pre-HTML) note stored without the wrapper.
    let mut plain = item(json!({
        "key": "NOTEPLN1", "version": 1, "itemType": "note", "note": "x",
        "tags": [], "relations": {},
        "dateAdded": "2015-04-12T09:00:22Z", "dateModified": "2015-04-12T09:00:22Z"}));
    let plain_id = db.save_item(1, &plain);
    db.conn
        .execute(
            "UPDATE itemNotes SET note=?1 WHERE itemID=?2",
            rusqlite::params!["a < b\nsecond", plain_id],
        )
        .unwrap();
    plain.note = Some("<p>a &lt; b</p><p>second</p>".into());
    let mut expected_items = expected_items;
    expected_items.push(plain);

    db.save_search(
        1,
        "SEARCH01",
        "HTGR papers",
        &[
            ("title/any", Some("contains"), Some("pebble")),
            ("collection", Some("is"), Some("0_COLLPAR1")),
            (
                "itemTypeID",
                Some("is"),
                Some(&db.item_type_id("report").to_string()),
            ),
            ("childNote", Some("contains"), Some("todo")),
            ("joinMode", Some("any"), None),
        ],
        false,
    );
    db.save_search(1, "SEARCH02", "Old search", &[], true);

    let group_lib = db.add_group(4242, "Reactor group", "<p>shared</p>");
    let gcoll = collection(json!({"key": "GCOLL001", "version": 1, "name": "Shared",
        "parentCollection": false, "relations": {}}));
    db.save_collection(group_lib, &gcoll);
    // Same key as a user-library item: keys are unique per library only.
    let gitem = item(json!({
        "key": "ARTICLE1", "version": 3, "itemType": "thesis", "title": "Group thesis",
        "university": "NUS", "creators": [{"creatorType": "author", "firstName": "A", "lastName": "B"}],
        "tags": [], "collections": ["GCOLL001"], "relations": {},
        "dateAdded": "2015-04-12T09:00:22Z", "dateModified": "2015-04-12T09:00:22Z"}));
    db.save_item(group_lib, &gitem);
    let feed = db.add_feed("Feed", "https://example.org/rss");
    db.save_item(
        feed,
        &item(
            json!({"key": "FEEDITM1", "itemType": "webpage", "title": "Feed entry",
            "creators": [], "tags": [], "relations": {}}),
        ),
    );
    drop(db);
    // The stored PDF exists on disk; the snapshot does not.
    let sdir = tmp.path().join("storage").join("PDFSTOR1");
    std::fs::create_dir_all(&sdir).unwrap();
    std::fs::write(sdir.join("paper.pdf"), b"%PDF-1.4\n").unwrap();
    Fixture {
        tmp,
        expected_items,
        expected_collections: vec![parent, child, trashed],
        group_lib,
    }
}

fn check_library(
    f: &Fixture,
    opts: &ReadOptions,
) -> kovan_literature::zotero::local_library::ZoteroDataFolder {
    let folder = read_data_folder(data_dir(f), opts).unwrap();
    assert_eq!(
        folder.libraries.len(),
        2,
        "user + group; the feed is skipped"
    );
    let user = &folder.libraries[0];
    assert_eq!(user.kind, LibraryKind::User);
    assert_eq!(user.uri, "http://zotero.org/users/local/Ab3dE6gH");
    let c = &user.contents;
    assert_eq!(c.items.len(), f.expected_items.len());
    for e in &f.expected_items {
        let key = e.key.as_deref().unwrap();
        let g = c.item(key).unwrap_or_else(|| panic!("{key} missing"));
        assert_eq!(
            g,
            e,
            "{key}:\n got {}\nwant {}",
            g.to_json_value(),
            e.to_json_value()
        );
    }
    assert_eq!(c.collections, f.expected_collections);
    assert_eq!(c.subcollections(Some("COLLPAR1")).count(), 1);
    // Saved searches, conditions as `_loadConditions` + `toJSON` give them.
    assert_eq!(c.searches.len(), 2);
    assert_eq!(
        c.searches[0].to_json_value(),
        json!({"key": "SEARCH01", "version": 2, "name": "HTGR papers", "conditions": [
            {"condition": "title/any", "operator": "contains", "value": "pebble"},
            {"condition": "collection", "operator": "is", "value": "COLLPAR1"},
            {"condition": "itemType", "operator": "is", "value": "report"},
            {"condition": "note", "operator": "contains", "value": "todo"},
            {"condition": "joinMode", "operator": "any", "value": ""},
            {"condition": "resultLevel", "operator": "item", "value": ""}]})
    );
    assert_eq!(c.searches[1].deleted, Some(true));
    // Files (getFilePath).
    let storage = data_dir(f).join("storage");
    assert_eq!(
        user.files["PDFSTOR1"],
        AttachmentFile::Stored(storage.join("PDFSTOR1").join("paper.pdf"))
    );
    assert!(user.files["PDFSTOR1"].exists());
    assert!(!user.files["SNAPSHT1"].exists());
    assert_eq!(
        user.files["LINKED01"],
        AttachmentFile::Linked(PathBuf::from("/srv/papers/report.pdf"))
    );
    assert_eq!(
        user.files["LINKREL1"],
        AttachmentFile::LinkedRelative {
            relative: "sub/x.pdf".into(),
            resolved: opts
                .base_attachment_path
                .as_ref()
                .map(|b| b.join("sub/x.pdf")),
        }
    );
    assert_eq!(user.files["LINKURL1"], AttachmentFile::NoFile);
    // The invalid field was dropped and reported, nothing skipped.
    assert_eq!(
        folder.report.dropped.len(),
        1,
        "{:?}",
        folder.report.dropped
    );
    assert!(folder.report.dropped[0].what.contains("runningTime"));
    assert!(folder.report.skipped.is_empty());
    assert!(folder
        .report
        .notes
        .iter()
        .any(|n| n.contains("1 feed library")));
    // Group library.
    let group = &folder.libraries[1];
    assert_eq!(group.library_id, f.group_lib);
    assert_eq!(
        group.kind,
        LibraryKind::Group {
            group_id: 4242,
            name: "Reactor group".into(),
            description: "<p>shared</p>".into()
        }
    );
    assert_eq!(group.uri, "http://zotero.org/groups/4242");
    assert_eq!(group.contents.items.len(), 1);
    assert_eq!(group.contents.items[0].item_type.as_str(), "thesis");
    assert_eq!(
        group.contents.items[0].collections,
        vec!["GCOLL001".to_owned()]
    );
    folder
}

/// **Result (2026-10-07): pass.** Every table of the module docs' table is
/// exercised: 13 user-library items (regular, trashed, in My Publications,
/// child and standalone notes, a plain-text note converted as `_loadNotes`
/// converts it, stored/snapshot/linked/relative/URL attachments, highlight
/// and external ink annotations) read back equal to the inserted JSON; the
/// collection tree, a trashed collection and collection relations; both
/// saved searches with `_loadConditions`' four conversions; the group
/// library with its own `ARTICLE1`; the feed library skipped; the one field
/// invalid for its type dropped and reported.
#[test]
fn a_full_library_reads_back_exactly() {
    let f = build_library(IdOrder::Forward);
    let base = PathBuf::from("/home/me/papers");
    let opts = ReadOptions {
        base_attachment_path: Some(base),
        ..ReadOptions::default()
    };
    check_library(&f, &opts);
    check_library(&f, &ReadOptions::default());
}

/// **Result (2026-10-07): pass.** The same library with every item-type,
/// field and creator-type id handed out in reverse order reads back the
/// same: the reader resolves ids through the database's own tables.
#[test]
fn ids_are_resolved_by_name_not_assumed() {
    let f = build_library(IdOrder::Reverse);
    check_library(&f, &ReadOptions::default());
}

fn file_bytes(p: &Path) -> Option<Vec<u8>> {
    std::fs::read(p).ok()
}

/// **Result (2026-10-07): pass.** With a writer connection holding the
/// database in `locking_mode=EXCLUSIVE` + `journal_mode=WAL` (Zotero's own
/// settings, db.js:1644-1653) and an item committed only to the `-wal`
/// (checkpointing off), a second connection cannot read the file (`database
/// is locked`), the reader still returns the item (it read the private
/// copy), and `zotero.sqlite` and `zotero.sqlite-wal` are byte-identical
/// before and after.
#[test]
fn a_locked_live_database_is_read_through_a_private_copy() {
    let tmp = tempfile::tempdir().unwrap();
    let db = ZoteroDb::create(tmp.path(), IdOrder::Forward);
    db.conn
        .execute_batch(
            "PRAGMA journal_mode=WAL; PRAGMA wal_autocheckpoint=0; \
             PRAGMA main.locking_mode=EXCLUSIVE;",
        )
        .unwrap();
    db.save_item(
        1,
        &item(
            json!({"key": "WALITEM1", "itemType": "book", "title": "Only in the WAL",
            "creators": [], "tags": [], "relations": {},
            "dateAdded": "2015-04-12T09:00:22Z", "dateModified": "2015-04-12T09:00:22Z"}),
        ),
    );
    let live = tmp.path().join("zotero.sqlite");
    let wal = tmp.path().join("zotero.sqlite-wal");
    assert!(
        file_bytes(&wal).is_some_and(|b| !b.is_empty()),
        "item is in the WAL"
    );
    // Another connection is locked out, as with Zotero running.
    let other = rusqlite::Connection::open(&live).unwrap();
    let locked = other.query_row("SELECT COUNT(*) FROM items", [], |r| r.get::<_, i64>(0));
    assert!(locked.is_err(), "exclusive lock holds: {locked:?}");
    drop(other);
    let before = (file_bytes(&live), file_bytes(&wal));
    let folder = read_data_folder(tmp.path(), &ReadOptions::default()).unwrap();
    let after = (file_bytes(&live), file_bytes(&wal));
    assert_eq!(before, after, "the data folder is not written");
    assert_eq!(folder.source, DbSource::LiveCopy);
    let title = folder.libraries[0]
        .contents
        .item("WALITEM1")
        .and_then(|i| i.fields.get("title").cloned());
    assert_eq!(title.as_deref(), Some("Only in the WAL"));
    drop(db);
}

/// **Result (2026-10-07): pass.** A `zotero.sqlite` overwritten with
/// garbage, next to a good `zotero.sqlite.bak`: the default read falls back
/// to the backup (`source = Backup`, noted in the report); with
/// `fall_back_to_backup = false` it is an error; `DbSource::Backup` reads the
/// backup directly.
#[test]
fn a_corrupt_live_database_falls_back_to_the_backup() {
    let f = build_library(IdOrder::Forward);
    let dir = data_dir(&f);
    std::fs::copy(dir.join("zotero.sqlite"), dir.join("zotero.sqlite.bak")).unwrap();
    std::fs::write(dir.join("zotero.sqlite"), vec![0x5a_u8; 8192]).unwrap();
    let folder = read_data_folder(dir, &ReadOptions::default()).unwrap();
    assert_eq!(folder.source, DbSource::Backup);
    assert!(folder
        .report
        .notes
        .iter()
        .any(|n| n.contains("zotero.sqlite.bak")));
    assert_eq!(
        folder.libraries[0].contents.items.len(),
        f.expected_items.len()
    );
    let strict = ReadOptions {
        fall_back_to_backup: false,
        ..ReadOptions::default()
    };
    assert!(read_data_folder(dir, &strict).is_err());
    let bak = ReadOptions {
        source: DbSource::Backup,
        ..ReadOptions::default()
    };
    assert_eq!(
        read_data_folder(dir, &bak).unwrap().source,
        DbSource::Backup
    );
}

/// **Result (2026-10-07): pass.** userdata 77 (the version of upstream's
/// `zotero-4.0.sqlite.zip` fixture) is `TooOld`; compatibility 10 is
/// `TooNew` unless `allow_newer_schema`; a folder without a database is
/// `NoDatabase`. Checked once by hand on upstream's own
/// `test/tests/data/zotero-4.0.sqlite.zip` (unzipped outside the repo, read
/// through `opt_in_real_data_folder_summary`): refused with "Zotero database
/// schema 77 is older than Zotero 5.0 (80)", file unchanged (md5
/// 32535428ce7c026d0afb6ec53f028412 before and after).
#[test]
fn unsupported_versions_are_refused() {
    let tmp = tempfile::tempdir().unwrap();
    let db = ZoteroDb::create(tmp.path(), IdOrder::Forward);
    db.conn
        .execute("UPDATE version SET version=77 WHERE schema='userdata'", [])
        .unwrap();
    drop(db);
    assert_eq!(
        read_data_folder(tmp.path(), &ReadOptions::default()).unwrap_err(),
        ZoteroDbError::TooOld { userdata: 77 }
    );
    let tmp = tempfile::tempdir().unwrap();
    let db = ZoteroDb::create(tmp.path(), IdOrder::Forward);
    db.conn
        .execute(
            "UPDATE version SET version=10 WHERE schema='compatibility'",
            [],
        )
        .unwrap();
    drop(db);
    assert_eq!(
        read_data_folder(tmp.path(), &ReadOptions::default()).unwrap_err(),
        ZoteroDbError::TooNew { compatibility: 10 }
    );
    let anyway = ReadOptions {
        allow_newer_schema: true,
        ..ReadOptions::default()
    };
    assert!(read_data_folder(tmp.path(), &anyway).is_ok());
    let empty = tempfile::tempdir().unwrap();
    assert!(matches!(
        read_data_folder(empty.path(), &ReadOptions::default()),
        Err(ZoteroDbError::NoDatabase(_))
    ));
}

/// **Result (2026-10-07): pass.** The import of the full fixture library:
/// 3 documents (user `ARTICLE1` and `REPORT01`, group `ARTICLE1`); the
/// trashed book and the 2 standalone/plain notes are not documents. The
/// article carries its child attachments and note in `zotero_item`
/// (export shape) and its `source_path` is the stored PDF; the report's
/// `source_path` is its linked PDF (the conversion's own rule). Losses: 1
/// trashed item, 2 standalone notes, 2 annotations, 4 collections, 2
/// searches, the duplicated id `zotero:ARTICLE1`, and 3 attachments without a
/// file on disk (snapshot, absolute linked, relative linked). Writing goes
/// to `user/` and `group-4242/`, round-trips through JSON, and refuses to
/// overwrite.
#[test]
fn import_into_kovan_documents_reports_what_is_lost() {
    let f = build_library(IdOrder::Forward);
    let folder = read_data_folder(data_dir(&f), &ReadOptions::default()).unwrap();
    let imp = import(&folder, &ImportOptions::default());
    let keys: Vec<(i64, &str)> = imp
        .documents
        .iter()
        .map(|d| (d.library_id, d.key.as_str()))
        .collect();
    assert_eq!(
        keys,
        vec![(1, "ARTICLE1"), (1, "REPORT01"), (f.group_lib, "ARTICLE1")]
    );
    let art = &imp.documents[0].document;
    assert_eq!(art.title, "Pebble bed heat transfer");
    assert_eq!(art.year, Some(2010));
    assert_eq!(art.tags, vec!["pebble bed".to_owned()]);
    assert_eq!(art.keywords, vec!["HTGR".to_owned()]);
    let pdf = data_dir(&f).join("storage/PDFSTOR1/paper.pdf");
    assert_eq!(art.source_path.as_deref(), Some(pdf.to_str().unwrap()));
    let z = art.zotero_item.as_ref().unwrap();
    assert_eq!(z.attachments.len(), 2);
    assert_eq!(z.notes.len(), 1);
    let rep = &imp.documents[1].document;
    assert_eq!(rep.source_path.as_deref(), Some("/srv/papers/report.pdf"));
    let l = &imp.losses;
    assert_eq!(l.trashed_items_skipped, 1);
    assert_eq!(l.standalone_notes, 2);
    assert_eq!(l.annotations, 2);
    assert_eq!(l.collections, 4);
    assert_eq!(l.saved_searches, 2);
    assert_eq!(l.duplicate_ids, vec!["zotero:ARTICLE1".to_owned()]);
    let mut missing = l.missing_files.clone();
    missing.sort();
    assert_eq!(missing, vec!["LINKED01", "LINKREL1", "SNAPSHT1"]);
    assert_eq!(l.summary().len(), 7);
    let with_trash = import(
        &folder,
        &ImportOptions {
            include_trashed: true,
        },
    );
    assert_eq!(with_trash.documents.len(), 4);

    let out = tempfile::tempdir().unwrap();
    let written = write_documents(&folder, &imp, out.path()).unwrap();
    assert_eq!(written.len(), 3);
    assert!(out.path().join("user/ARTICLE1.json").is_file());
    assert!(out.path().join("group-4242/ARTICLE1.json").is_file());
    let back: KovanDocument =
        serde_json::from_str(&std::fs::read_to_string(&written[0]).unwrap()).unwrap();
    assert_eq!(&back, art);
    let again = write_documents(&folder, &imp, out.path()).unwrap_err();
    assert_eq!(again.kind(), std::io::ErrorKind::AlreadyExists);
}

/// Opt-in: read a real Zotero data folder named by `KOVAN_ZOTERO_DATA_DIR`
/// (read-only, through a private copy) and print a summary of counts only.
/// Skipped (passes, printing a note) when the variable is unset. Nothing is
/// written anywhere and nothing from the library enters the repository.
#[test]
fn opt_in_real_data_folder_summary() {
    let Some(dir) = std::env::var_os("KOVAN_ZOTERO_DATA_DIR") else {
        eprintln!("KOVAN_ZOTERO_DATA_DIR not set: skipping the real-library read");
        return;
    };
    let dir = PathBuf::from(dir);
    let folder = match read_data_folder(&dir, &ReadOptions::default()) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("read failed: {e}");
            panic!("{e}");
        }
    };
    eprintln!(
        "source {:?}, schema {:?}, {} libraries",
        folder.source,
        folder.schema,
        folder.libraries.len()
    );
    for lib in &folder.libraries {
        let mut by_type = std::collections::BTreeMap::new();
        for it in &lib.contents.items {
            *by_type.entry(it.item_type.as_str()).or_insert(0usize) += 1;
        }
        let files = lib.files.values().filter(|f| f.exists()).count();
        eprintln!(
            "library {} ({}): {} items, {} collections, {} searches, {}/{} attachment files present",
            lib.library_id,
            match &lib.kind {
                LibraryKind::User => "user".to_owned(),
                LibraryKind::Group { group_id, .. } => format!("group {group_id}"),
            },
            lib.contents.items.len(),
            lib.contents.collections.len(),
            lib.contents.searches.len(),
            files,
            lib.files.len()
        );
        eprintln!("  item types: {by_type:?}");
    }
    eprintln!(
        "report: {} skipped, {} dropped values; notes: {:?}",
        folder.report.skipped.len(),
        folder.report.dropped.len(),
        folder.report.notes
    );
    let imp = import(&folder, &ImportOptions::default());
    eprintln!(
        "import: {} documents; losses: {:?}",
        imp.documents.len(),
        imp.losses.summary()
    );
}
