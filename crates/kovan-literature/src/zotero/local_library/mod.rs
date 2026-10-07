// Part of the kovan Zotero port (GitHub #747, #750).
//
// Upstream: Zotero, https://github.com/zotero/zotero (commit 9cbba8c4d281):
//   resource/schema/userdata.sql (the tables read), system.sql, triggers.sql;
//   chrome/content/zotero/xpcom/schema.js (`updateSchema` :88, the upgrade
//   steps of `_migrateUserDataSchema` :3012-3850, `_maxCompatibility` :45);
//   xpcom/db.js (locking and WAL handling, :28-32, :1575-1660, :1840-1935);
//   xpcom/data/items.js (`_primaryDataSQLParts` :37, `_loadItemData` :259,
//   `_loadCreators` :397, `_loadNotes` :470, `_loadAnnotations` :561,
//   `_loadAnnotationsDeferred` :640, `_loadTags` :865, `_loadCollections`
//   :922); xpcom/data/item.js (`setField` :662, `getField` :237,
//   `_parseRowData` :344, `getFilePath` :2899, `attachmentFilename` :3601,
//   `toJSON` :6008); xpcom/data/dataObjects.js (`_loadRelations` :613);
//   xpcom/data/collections.js (:35-60); xpcom/data/searches.js
//   (`_loadConditions` :125); xpcom/data/search.js (`toJSON` :893);
//   xpcom/data/searchConditions.js (`parseCondition` :993);
//   xpcom/data/tags.js (`cleanData` :908); xpcom/attachments.js (link modes
//   :35-41, `resolveRelativePath` :2899, `getStorageDirectoryByLibraryAndKey`
//   :2842); xpcom/uri.js (`getLibraryPath`); xpcom/users.js; and Zotero
//   utilities (commit 4051881d59c6) utilities.js (`htmlSpecialChars` :668).
// Copyright (c) 2009 Center for History and New Media, George Mason
// University, Fairfax, Virginia, USA; (c) Corporation for Digital
// Scholarship, Vienna, Virginia, USA.
// Licence: AGPL-3.0 (upstream: AGPL-3.0-or-later; this crate is
// AGPL-3.0-only). See this crate's NOTICE, "Upstream: Zotero (local library
// reader)".

//! Read a Zotero data folder (`zotero.sqlite` + `storage/`) into
//! [`ZoteroLibrary`]s, and import it into kovan ([`import`]). GitHub #750.
//!
//! ```no_run
//! use kovan_literature::zotero::local_library::{read_data_folder, ReadOptions};
//! let folder = read_data_folder("/home/me/Zotero".as_ref(), &ReadOptions::default()).unwrap();
//! for lib in &folder.libraries {
//!     println!("{:?}: {} items", lib.kind, lib.contents.items.len());
//! }
//! let import = kovan_literature::zotero::local_library::import::import(&folder, &Default::default());
//! println!("{} documents, lossy: {:?}", import.documents.len(), import.losses);
//! ```
//!
//! ## Never touching the live database
//!
//! Zotero opens `zotero.sqlite` with `PRAGMA locking_mode=EXCLUSIVE` and, in
//! this version, `journal_mode=WAL` (db.js:28-32, :1644-1653), so while Zotero
//! runs no other SQLite connection can read it. **This reader never opens the
//! file in the data folder at all.** It copies `zotero.sqlite` and, when they
//! exist, its `-journal` and `-wal` files into a private temporary directory
//! and opens the copy, which is how Zotero itself reads a database it does
//! not own (db.js:1871-1885 copies the database and its WAL to a temporary
//! file before touching them). SQLite replays the copied WAL or rolls back the
//! copied journal **in the private copy**; the connection is then set
//! `PRAGMA query_only=ON`, and `PRAGMA quick_check` must report `ok`. If the
//! copy fails that check (Zotero was mid-write while the bytes were copied),
//! the reader falls back to `zotero.sqlite.bak`, Zotero's own periodic backup
//! (db.js:1358, :2472), when [`ReadOptions::fall_back_to_backup`] is set
//! (the default), and records which file it read in
//! [`ZoteroDataFolder::source`]. [`DbSource::Backup`] reads the backup
//! directly. The temporary copy is deleted when reading ends. Nothing is
//! ever written to the data folder.
//!
//! With Zotero running, the copy is a best-effort snapshot (a write landing
//! between copying the database and copying its WAL can still give a
//! structurally valid but slightly stale copy); for an exact read, close
//! Zotero first.
//!
//! ## Database versions
//!
//! Zotero 5.0 and later: `userdata` schema version **80** (the Zotero 5
//! migration, schema.js:3012) up to **130**, the version of the bundled
//! `userdata.sql` at the upstream commit. Older databases (Zotero 4, e.g.
//! upstream's `test/tests/data/zotero-4.0.sqlite.zip`, userdata 77) have a
//! different layout and are refused with [`ZoteroDbError::TooOld`]: open them
//! once in a current Zotero, which upgrades them. A database whose
//! `compatibility` version exceeds 9 (`_maxCompatibility`, schema.js:45) is
//! refused like Zotero refuses it ([`ZoteroDbError::TooNew`]) unless
//! [`ReadOptions::allow_newer_schema`] is set. Tables and columns added after
//! 80 are probed, not assumed: `itemAnnotations` (112; `authorName` 119),
//! `deletedCollections`/`deletedSearches` (111), `publicationsItems` (93),
//! `itemAttachments.lastRead` (124/130).
//!
//! Type, field and creator-type ids are **not** assumed: they are read from
//! the database's own `itemTypesCombined`, `fieldsCombined` and
//! `creatorTypes` tables and resolved by name, because they differ between a
//! database created fresh (ids assigned in `schema.json` order by
//! `_updateGlobalSchema`, schema.js:459) and one upgraded from Zotero 4
//! (`system-107.sql` ids).
//!
//! ## What is read, and how (faithful to Zotero's own loaders)
//!
//! | Zotero tables | Read into | Upstream behaviour mirrored |
//! |---|---|---|
//! | `libraries`, `groups`, `settings` (`account` `userID`/`localUserKey`) | one [`LocalLibrary`] per user/group library, its URI | uri.js `getLibraryPath` |
//! | `items`, `deletedItems`, `publicationsItems` | key, version, type, `dateAdded`/`dateModified` (ISO), `deleted`/`inPublications` (only when true) | items.js `_primaryDataSQLParts`, item.js `toJSON`, dataObject.js `_postToJSON` |
//! | `itemData`, `itemDataValues`, `fieldsCombined` | [`ZoteroItem::fields`](kovan_common::zotero::ZoteroItem::fields) | `_loadItemData` + `setField(.., loadIn)`: trim, base -> type-specific field, fields invalid for the type ignored (and reported), newlines stripped outside `abstractNote`/`extra`/`address`; `getField`: multipart dates -> user string; `toJSON`: `accessDate` -> ISO |
//! | `itemNotes` | `note` | `_loadNotes`: the `zotero-note znv` wrapper stripped; a plain-text note converted to HTML in memory (upstream also writes it back; this reader does not) |
//! | `itemCreators`, `creators`, `creatorTypes` | creators, in `orderIndex` order (regular items only) | `_loadCreators`, `toJSON` |
//! | `itemTags`, `tags` | tags; `type: 1` kept, 0 dropped | `_loadTags`, tags.js `cleanData` |
//! | `collectionItems` | `collections` of top-level items | `_loadCollections`, `toJSON` |
//! | `itemRelations`, `relationPredicates` | `relations` | `_loadRelations` |
//! | `itemAttachments`, `charsets` | [`AttachmentData`](kovan_common::zotero::AttachmentData) and [`LocalLibrary::files`] | `_parseRowData`, `toJSON`, `attachmentFilename`, `getFilePath`; `md5`/`mtime` from `storageHash`/`storageModTime` (the `syncedStorageProperties` form) |
//! | `itemAnnotations` | [`AnnotationData`](kovan_common::zotero::AnnotationData); `isExternal` as `other["annotationIsExternal"]` when true | `_loadAnnotations`, `_loadAnnotationsDeferred`, `toJSON` |
//! | `collections`, `deletedCollections`, `collectionRelations` | [`ZoteroCollection`](kovan_common::zotero::ZoteroCollection)s (the tree through `parentCollection`) | collections.js, collection.js `toJSON` |
//! | `savedSearches`, `savedSearchConditions`, `deletedSearches` | [`ZoteroSearch`](kovan_common::zotero::ZoteroSearch)es, conditions as stored | searches.js `_loadConditions`, search.js `toJSON` |
//!
//! ## Not read, and why
//!
//! * **Feed libraries** (`libraries.type = 'feed'`, `feeds`, `feedItems`):
//!   RSS subscriptions that Zotero cleans up by itself (`cleanupReadAfter`),
//!   not part of the user's library and not in Zotero's own exports. Skipped
//!   and counted in [`ReadReport::notes`].
//! * **Sync bookkeeping** (`syncCache`, `syncDeleteLog`, `syncQueue`,
//!   `storageDeleteLog`, `synced`/`clientVersion` columns): state of the
//!   Zotero server sync, which kovan replaces with git.
//! * **`fulltextItems`** (and the `.zotero-ft-cache` files): a search index
//!   derived from the attachment files, which are read instead.
//! * **`retractedItems`**: Zotero's cached copy of Retraction Watch data,
//!   refreshed from a web service; not the user's data.
//! * **`groupItems`, `users`**: who created/modified a group item; personal
//!   data of other people, not in `toJSON`.
//! * **`syncedSettings`, `settings`** (beyond the account ids for URIs):
//!   UI preferences such as tag colours.
//! * **`proxies`, `translatorCache`, `custom*` tables**: application state;
//!   `customItemTypes`/`customFields` are unused upstream ("These shouldn't
//!   be used yet", userdata.sql).
//!
//! ## Deliberate differences from upstream, all stated
//!
//! * **Unicode normalisation.** `setField` and `cleanData` NFC-normalise on
//!   load; this reader only trims. Zotero normalises on save as well, so
//!   values Zotero wrote are already NFC.
//! * **ISBN hyphenation** on load (item.js:829) is not ported; Zotero also
//!   hyphenates on save, so stored ISBNs are normally already hyphenated.
//! * **Saved-search conditions** that Zotero no longer registers are kept
//!   (upstream drops them on load); the registry (searchConditions.js) is not
//!   ported. The conversions `_loadConditions` applies are mirrored
//!   (`childNote` -> `note` + `resultLevel`, `itemTypeID` -> `itemType`,
//!   `0_KEY` -> `KEY`, NULL value -> `""`).
//! * **Rows upstream would throw on** (an unknown item or annotation type, an
//!   unknown creator type) are skipped and listed in [`ReadReport::skipped`]
//!   instead of failing the whole read.
//! * **`attachments:` paths** (relative to the linked-attachment base
//!   directory) are resolved only when [`ReadOptions::base_attachment_path`]
//!   is given: that directory is a Zotero *preference* (`baseAttachmentPath`
//!   in `prefs.js` of the Zotero profile), not stored in the data folder.
//!
//! **Maturity: AI draft (1).** Verified code-to-code against databases built
//! from upstream's own schema files (`tests/zotero_local_library.rs`); not yet
//! run on a real library by a human.

mod containers;
pub mod import;
mod items;
mod open;

use kovan_common::zotero::ZoteroLibrary;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// The oldest `userdata` schema version read (Zotero 5.0's migration step,
/// schema.js:3012).
pub const MIN_USERDATA_VERSION: i64 = 80;

/// The `userdata` version of the upstream `userdata.sql` this port follows.
pub const BUNDLED_USERDATA_VERSION: i64 = 130;

/// Zotero's `_maxCompatibility` (schema.js:45) at the upstream commit.
pub const MAX_COMPATIBILITY: i64 = 9;

/// Which database file to read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DbSource {
    /// A private copy of `zotero.sqlite` (with its `-journal`/`-wal`).
    #[default]
    LiveCopy,
    /// A private copy of `zotero.sqlite.bak`, Zotero's own backup.
    Backup,
}

/// How to read a data folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadOptions {
    /// Which file to read (default [`DbSource::LiveCopy`]).
    pub source: DbSource,
    /// Read `zotero.sqlite.bak` when the copy of the live database fails
    /// `PRAGMA quick_check` (default true).
    pub fall_back_to_backup: bool,
    /// Zotero's `baseAttachmentPath` preference, to resolve `attachments:`
    /// linked-file paths (default none: they stay unresolved).
    pub base_attachment_path: Option<PathBuf>,
    /// Read a database whose `compatibility` version is above
    /// [`MAX_COMPATIBILITY`] (default false: refused, as Zotero refuses it).
    pub allow_newer_schema: bool,
}

impl Default for ReadOptions {
    fn default() -> Self {
        ReadOptions {
            source: DbSource::LiveCopy,
            fall_back_to_backup: true,
            base_attachment_path: None,
            allow_newer_schema: false,
        }
    }
}

/// The schema versions recorded in the database's `version` table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SchemaVersions {
    /// `userdata` (the last upgrade step run).
    pub userdata: i64,
    /// `system`.
    pub system: Option<i64>,
    /// `triggers`.
    pub triggers: Option<i64>,
    /// `compatibility`.
    pub compatibility: Option<i64>,
}

/// What kind of Zotero library this is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LibraryKind {
    /// My Library.
    User,
    /// A group library.
    Group {
        /// The group's id on zotero.org.
        group_id: i64,
        /// The group's name.
        name: String,
        /// The group's description (HTML).
        description: String,
    },
}

/// Where an attachment's file is, resolved as `Item#getFilePath` resolves it
/// (item.js:2899).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttachmentFile {
    /// A stored file: `<data dir>/storage/<KEY>/<filename>`.
    Stored(PathBuf),
    /// A linked file at an absolute path.
    Linked(PathBuf),
    /// A linked file relative to the base attachment directory
    /// (`attachments:` prefix); `resolved` when
    /// [`ReadOptions::base_attachment_path`] was given.
    LinkedRelative {
        /// The path after `attachments:`, as stored.
        relative: String,
        /// The absolute path, if the base directory is known.
        resolved: Option<PathBuf>,
    },
    /// A linked URL: no file.
    NoFile,
    /// No usable path; the reason is upstream's (empty path, invalid stored
    /// path, an old Mac alias record, ...).
    Unresolvable(String),
}

impl AttachmentFile {
    /// The absolute path, when there is one.
    pub fn path(&self) -> Option<&Path> {
        match self {
            AttachmentFile::Stored(p) | AttachmentFile::Linked(p) => Some(p),
            AttachmentFile::LinkedRelative { resolved, .. } => resolved.as_deref(),
            AttachmentFile::NoFile | AttachmentFile::Unresolvable(_) => None,
        }
    }

    /// Whether the file exists now (checked on the file system).
    pub fn exists(&self) -> bool {
        self.path().is_some_and(Path::is_file)
    }
}

/// One Zotero library of the data folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalLibrary {
    /// `libraries.libraryID` (1 is My Library).
    pub library_id: i64,
    /// User or group.
    pub kind: LibraryKind,
    /// `libraries.editable`.
    pub editable: bool,
    /// `libraries.filesEditable`.
    pub files_editable: bool,
    /// `libraries.version` (the last synced library version).
    pub version: i64,
    /// The library URI, e.g. `http://zotero.org/users/local/abcd1234` or
    /// `http://zotero.org/groups/123` (uri.js `getLibraryURI`); relation
    /// objects point at `<uri>/items/<KEY>`.
    pub uri: String,
    /// Collections, items (with children) and saved searches.
    pub contents: ZoteroLibrary,
    /// Attachment key -> its file.
    pub files: BTreeMap<String, AttachmentFile>,
}

impl LocalLibrary {
    /// The URI of an item of this library (uri.js `getItemURI`).
    pub fn item_uri(&self, key: &str) -> String {
        format!("{}/items/{key}", self.uri)
    }
}

/// A row that was skipped or a value that was dropped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadIssue {
    /// The library.
    pub library_id: i64,
    /// The object, e.g. `item ABCD2345` or `item ABCD2345 field foo`.
    pub what: String,
    /// Why.
    pub reason: String,
}

/// What the read could not take over as stored.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReadReport {
    /// Objects skipped whole (unknown item type, ...).
    pub skipped: Vec<ReadIssue>,
    /// Values dropped from an object that was read (a field invalid for the
    /// item type, which upstream also ignores; an unknown creator type, ...).
    pub dropped: Vec<ReadIssue>,
    /// Informational notes (which file was read, libraries skipped, ...).
    pub notes: Vec<String>,
}

/// A whole Zotero data folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZoteroDataFolder {
    /// The data folder.
    pub data_dir: PathBuf,
    /// Which database file was read.
    pub source: DbSource,
    /// Its schema versions.
    pub schema: SchemaVersions,
    /// User library first, then groups, by `libraryID`.
    pub libraries: Vec<LocalLibrary>,
    /// What could not be read as stored.
    pub report: ReadReport,
}

/// Why a data folder could not be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ZoteroDbError {
    /// Neither `zotero.sqlite` nor (when allowed) `zotero.sqlite.bak` exists.
    NoDatabase(PathBuf),
    /// Copying the database failed.
    Io {
        /// The file.
        path: PathBuf,
        /// The error.
        message: String,
    },
    /// SQLite reported an error.
    Sqlite(String),
    /// The copy failed `PRAGMA quick_check` (and no usable backup).
    Corrupt(String),
    /// The file is not a Zotero database (no `version` table / `userdata`).
    NotZotero(String),
    /// Older than Zotero 5.0 (`userdata` < [`MIN_USERDATA_VERSION`]).
    TooOld {
        /// The `userdata` version found.
        userdata: i64,
    },
    /// Newer than this reader knows (`compatibility` > [`MAX_COMPATIBILITY`]).
    TooNew {
        /// The `compatibility` version found.
        compatibility: i64,
    },
}

impl std::fmt::Display for ZoteroDbError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ZoteroDbError::NoDatabase(p) => write!(f, "no zotero.sqlite in {}", p.display()),
            ZoteroDbError::Io { path, message } => write!(f, "{}: {message}", path.display()),
            ZoteroDbError::Sqlite(m) => write!(f, "sqlite: {m}"),
            ZoteroDbError::Corrupt(m) => write!(f, "database copy is not consistent: {m}"),
            ZoteroDbError::NotZotero(m) => write!(f, "not a Zotero database: {m}"),
            ZoteroDbError::TooOld { userdata } => write!(
                f,
                "Zotero database schema {userdata} is older than Zotero 5.0 \
                 ({MIN_USERDATA_VERSION}); open it once in a current Zotero to upgrade it"
            ),
            ZoteroDbError::TooNew { compatibility } => write!(
                f,
                "Zotero database compatibility {compatibility} is newer than this reader \
                 ({MAX_COMPATIBILITY}); set allow_newer_schema to try anyway"
            ),
        }
    }
}

impl std::error::Error for ZoteroDbError {}

impl From<rusqlite::Error> for ZoteroDbError {
    fn from(e: rusqlite::Error) -> Self {
        ZoteroDbError::Sqlite(e.to_string())
    }
}

/// Read a Zotero data folder: the directory holding `zotero.sqlite` and
/// `storage/` (Zotero's "Data Directory Location"). Read-only; see the
/// module docs for how the live database is left untouched.
pub fn read_data_folder(
    data_dir: &Path,
    opts: &ReadOptions,
) -> Result<ZoteroDataFolder, ZoteroDbError> {
    let mut report = ReadReport::default();
    let snap = open::open_snapshot(data_dir, opts, &mut report)?;
    let conn = &snap.conn;
    let schema = open::schema_versions(conn)?;
    if schema.userdata < MIN_USERDATA_VERSION {
        return Err(ZoteroDbError::TooOld {
            userdata: schema.userdata,
        });
    }
    if let Some(c) = schema.compatibility {
        if c > MAX_COMPATIBILITY {
            if !opts.allow_newer_schema {
                return Err(ZoteroDbError::TooNew { compatibility: c });
            }
            report.notes.push(format!(
                "compatibility {c} > {MAX_COMPATIBILITY}: read anyway (allow_newer_schema)"
            ));
        }
    }
    if schema.userdata > BUNDLED_USERDATA_VERSION {
        report.notes.push(format!(
            "userdata schema {} is newer than {BUNDLED_USERDATA_VERSION}; \
             tables are probed, unknown additions are not read",
            schema.userdata
        ));
    }
    let caps = open::Caps::probe(conn)?;
    let lookups = containers::Lookups::load(conn, &caps)?;
    let storage_dir = data_dir.join("storage");
    let mut libraries = Vec::new();
    for info in containers::libraries(conn, &mut report)? {
        let ctx = items::LibCtx {
            library_id: info.library_id,
            is_user: info.kind == LibraryKind::User,
            storage_dir: storage_dir.clone(),
            base_attachment_path: opts.base_attachment_path.clone(),
        };
        let collections = containers::collections(conn, &caps, info.library_id)?;
        let searches = containers::searches(conn, &caps, &lookups, info.library_id)?;
        let (items, files) = items::load_items(conn, &caps, &lookups, &ctx, &mut report)?;
        libraries.push(LocalLibrary {
            library_id: info.library_id,
            kind: info.kind,
            editable: info.editable,
            files_editable: info.files_editable,
            version: info.version,
            uri: info.uri,
            contents: ZoteroLibrary {
                collections,
                items,
                searches,
            },
            files,
        });
    }
    Ok(ZoteroDataFolder {
        data_dir: data_dir.to_path_buf(),
        source: snap.source,
        schema,
        libraries,
        report,
    })
}
