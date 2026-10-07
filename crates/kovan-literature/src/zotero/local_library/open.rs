// Part of the kovan Zotero port (GitHub #747, #750).
//
// Upstream: Zotero, https://github.com/zotero/zotero (commit 9cbba8c4d281):
//   xpcom/db.js (exclusive locking and WAL, :28-32, :1575-1660; copying a
//   database with its WAL before reading it, :1840-1935; `.bak` backups,
//   :1358, :2472) and xpcom/schema.js (`getDBVersion`, the upgrade steps that
//   add the probed tables and columns).
// Copyright (c) 2009 Center for History and New Media, George Mason
// University; (c) Corporation for Digital Scholarship, Vienna, Virginia, USA.
// Licence: AGPL-3.0 (upstream: AGPL-3.0-or-later).

//! Opening an in-memory snapshot of the database, the schema versions, and
//! which optional tables/columns the database has. See the parent module
//! docs.

use super::db::{is_hot_journal, Db, Param};
use super::{DatabaseFiles, DbSource, ReadOptions, ReadReport, SchemaVersions, ZoteroDbError};
use std::path::Path;

/// The open snapshot and which file it came from.
pub(super) struct Snapshot {
    pub db: Db,
    pub source: DbSource,
}

/// Open `main` (with its WAL) from memory and check it. `name` is the file
/// name used in messages.
fn open_checked(
    name: &str,
    main: &[u8],
    wal: Option<&[u8]>,
    journal: Option<&[u8]>,
    source: DbSource,
) -> Result<Snapshot, ZoteroDbError> {
    // A hot rollback journal means the main file may hold half of a
    // transaction; SQLite would roll it back first. This reader refuses
    // the file instead (and the caller falls back to the backup).
    if is_hot_journal(main, journal) {
        return Err(ZoteroDbError::Corrupt(format!(
            "{name} has a hot rollback journal ({name}-journal): it was being written \
             when copied; SQLite would roll the journal back, this reader does not"
        )));
    }
    let db = Db::open(main, wal)?;
    let check = db
        .first_row("PRAGMA quick_check", &[])?
        .map(|r| r.text(0))
        .transpose()?
        .flatten()
        .unwrap_or_default();
    if check != "ok" {
        return Err(ZoteroDbError::Corrupt(format!("{name}: {check}")));
    }
    Ok(Snapshot { db, source })
}

/// Open the database the options ask for, falling back to the backup when
/// the live database is not consistent. `backup` supplies
/// `zotero.sqlite.bak` only when it is needed (it can be as large as the
/// database itself).
pub(super) fn open_snapshot(
    files: &DatabaseFiles,
    backup: impl FnOnce() -> Result<Option<Vec<u8>>, ZoteroDbError>,
    data_dir: &Path,
    opts: &ReadOptions,
    report: &mut ReadReport,
) -> Result<Snapshot, ZoteroDbError> {
    let read_backup = |report: &mut ReadReport, why: &str| {
        let Some(bak) = backup()? else {
            return Err(ZoteroDbError::NoDatabase(data_dir.to_path_buf()));
        };
        report.notes.push(format!("read zotero.sqlite.bak ({why})"));
        // Zotero writes the backup as a whole closed file (db.js:2472):
        // there is no WAL or journal to go with it.
        open_checked("zotero.sqlite.bak", &bak, None, None, DbSource::Backup)
    };
    match opts.source {
        DbSource::Backup => read_backup(report, "asked for"),
        DbSource::LiveCopy => {
            let Some(main) = files.database.as_deref() else {
                if opts.fall_back_to_backup {
                    return read_backup(report, "no zotero.sqlite");
                }
                return Err(ZoteroDbError::NoDatabase(data_dir.to_path_buf()));
            };
            match open_checked(
                "zotero.sqlite",
                main,
                files.wal.as_deref(),
                files.journal.as_deref(),
                DbSource::LiveCopy,
            ) {
                Ok(s) => {
                    report
                        .notes
                        .push("read a private in-memory copy of zotero.sqlite".to_owned());
                    Ok(s)
                }
                Err(e @ (ZoteroDbError::Corrupt(_) | ZoteroDbError::Sqlite(_)))
                    if opts.fall_back_to_backup =>
                {
                    match read_backup(report, &format!("zotero.sqlite failed: {e}")) {
                        // No backup to fall back to: report the live failure.
                        Err(ZoteroDbError::NoDatabase(_)) => Err(e),
                        other => other,
                    }
                }
                Err(e) => Err(e),
            }
        }
    }
}

/// The `version` table (schema.js `getDBVersion`).
pub(super) fn schema_versions(db: &Db) -> Result<SchemaVersions, ZoteroDbError> {
    if !table_exists(db, "version")? {
        return Err(ZoteroDbError::NotZotero("no version table".into()));
    }
    let get = |schema: &str| -> Result<Option<i64>, ZoteroDbError> {
        match db.first_row(
            "SELECT version FROM version WHERE schema=?1",
            &[Param::from(schema)],
        )? {
            Some(r) => Ok(Some(r.i64(0)?)),
            None => Ok(None),
        }
    };
    let userdata =
        get("userdata")?.ok_or_else(|| ZoteroDbError::NotZotero("no userdata version".into()))?;
    Ok(SchemaVersions {
        userdata,
        system: get("system")?,
        triggers: get("triggers")?,
        compatibility: get("compatibility")?,
    })
}

pub(super) fn table_exists(db: &Db, name: &str) -> Result<bool, ZoteroDbError> {
    Ok(db
        .first_row(
            "SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1 COLLATE NOCASE",
            &[Param::from(name)],
        )?
        .is_some())
}

pub(super) fn column_exists(db: &Db, table: &str, column: &str) -> Result<bool, ZoteroDbError> {
    Ok(db
        .first_row(
            "SELECT 1 FROM pragma_table_info(?1) WHERE name=?2 COLLATE NOCASE",
            &[Param::from(table), Param::from(column)],
        )?
        .is_some())
}

/// Optional tables and columns, probed (added by the upgrade step named).
#[derive(Debug, Clone, Copy)]
pub(super) struct Caps {
    /// `itemAnnotations` (112).
    pub annotations: bool,
    /// `itemAnnotations.authorName` (119).
    pub annotation_author: bool,
    /// `publicationsItems` (93).
    pub publications: bool,
    /// `deletedCollections` (111).
    pub deleted_collections: bool,
    /// `deletedSearches` (111).
    pub deleted_searches: bool,
    /// `itemAttachments.lastRead` (124, re-run in 130).
    pub last_read: bool,
    /// `itemTypesCombined` / `fieldsCombined` (else the plain tables).
    pub combined: bool,
}

impl Caps {
    pub(super) fn probe(db: &Db) -> Result<Caps, ZoteroDbError> {
        let annotations = table_exists(db, "itemAnnotations")?;
        Ok(Caps {
            annotations,
            annotation_author: annotations && column_exists(db, "itemAnnotations", "authorName")?,
            publications: table_exists(db, "publicationsItems")?,
            deleted_collections: table_exists(db, "deletedCollections")?,
            deleted_searches: table_exists(db, "deletedSearches")?,
            last_read: column_exists(db, "itemAttachments", "lastRead")?,
            combined: table_exists(db, "itemTypesCombined")? && table_exists(db, "fieldsCombined")?,
        })
    }
}
