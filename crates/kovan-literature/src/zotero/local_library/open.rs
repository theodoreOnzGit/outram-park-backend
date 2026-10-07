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

//! Opening a private copy of the database, the schema versions, and which
//! optional tables/columns the database has. See the parent module docs.

use super::{DbSource, ReadOptions, ReadReport, SchemaVersions, ZoteroDbError};
use rusqlite::{Connection, OpenFlags, OptionalExtension};
use std::path::{Path, PathBuf};

/// The open copy; the temporary directory lives as long as the connection.
pub(super) struct Snapshot {
    // Field order matters: the connection closes before the directory goes.
    pub conn: Connection,
    pub source: DbSource,
    _dir: tempfile::TempDir,
}

fn io_err(path: &Path, e: std::io::Error) -> ZoteroDbError {
    ZoteroDbError::Io {
        path: path.to_path_buf(),
        message: e.to_string(),
    }
}

fn with_suffix(p: &Path, suffix: &str) -> PathBuf {
    let mut s = p.as_os_str().to_owned();
    s.push(suffix);
    PathBuf::from(s)
}

/// Copy `db` (and its `-journal`/`-wal`, which hold committed data not yet
/// in the main file) into a fresh temporary directory, open the copy and
/// check it. The original files are only ever read.
fn copy_and_open(db: &Path, source: DbSource) -> Result<Snapshot, ZoteroDbError> {
    let dir = tempfile::Builder::new()
        .prefix("kovan-zotero-")
        .tempdir()
        .map_err(|e| io_err(db, e))?;
    let copy = dir.path().join("zotero.sqlite");
    std::fs::copy(db, &copy).map_err(|e| io_err(db, e))?;
    // db.js:2099 moves `-journal` and `-wal` along with a database file; the
    // `-shm` index is rebuilt from the WAL (and Zotero keeps it in heap
    // memory on Linux/Windows, db.js:1617), so it is not copied.
    for suffix in ["-journal", "-wal"] {
        let side = with_suffix(db, suffix);
        if side.is_file() {
            std::fs::copy(&side, with_suffix(&copy, suffix)).map_err(|e| io_err(&side, e))?;
        }
    }
    // Read-write on the private copy only, so SQLite can replay the copied
    // WAL or roll back the copied journal; then no further writes at all.
    let conn = Connection::open_with_flags(
        &copy,
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    conn.execute_batch("PRAGMA query_only = ON;")?;
    let check: String = conn.query_row("PRAGMA quick_check", [], |r| r.get(0))?;
    if check != "ok" {
        return Err(ZoteroDbError::Corrupt(format!("{}: {check}", db.display())));
    }
    Ok(Snapshot {
        conn,
        source,
        _dir: dir,
    })
}

/// Open the database the options ask for, falling back to the backup when
/// the copy of the live file is not consistent.
pub(super) fn open_snapshot(
    data_dir: &Path,
    opts: &ReadOptions,
    report: &mut ReadReport,
) -> Result<Snapshot, ZoteroDbError> {
    let live = data_dir.join("zotero.sqlite");
    let bak = data_dir.join("zotero.sqlite.bak");
    let read_backup = |report: &mut ReadReport, why: &str| {
        if !bak.is_file() {
            return Err(ZoteroDbError::NoDatabase(data_dir.to_path_buf()));
        }
        report.notes.push(format!("read zotero.sqlite.bak ({why})"));
        copy_and_open(&bak, DbSource::Backup)
    };
    match opts.source {
        DbSource::Backup => read_backup(report, "asked for"),
        DbSource::LiveCopy => {
            if !live.is_file() {
                if opts.fall_back_to_backup && bak.is_file() {
                    return read_backup(report, "no zotero.sqlite");
                }
                return Err(ZoteroDbError::NoDatabase(data_dir.to_path_buf()));
            }
            match copy_and_open(&live, DbSource::LiveCopy) {
                Ok(s) => {
                    report
                        .notes
                        .push("read a private copy of zotero.sqlite".to_owned());
                    Ok(s)
                }
                Err(e @ (ZoteroDbError::Corrupt(_) | ZoteroDbError::Sqlite(_)))
                    if opts.fall_back_to_backup && bak.is_file() =>
                {
                    read_backup(report, &format!("the copy of zotero.sqlite failed: {e}"))
                }
                Err(e) => Err(e),
            }
        }
    }
}

/// The `version` table (schema.js `getDBVersion`).
pub(super) fn schema_versions(conn: &Connection) -> Result<SchemaVersions, ZoteroDbError> {
    if !table_exists(conn, "version")? {
        return Err(ZoteroDbError::NotZotero("no version table".into()));
    }
    let get = |schema: &str| -> Result<Option<i64>, ZoteroDbError> {
        Ok(conn
            .query_row(
                "SELECT version FROM version WHERE schema=?1",
                [schema],
                |r| r.get::<_, i64>(0),
            )
            .optional()?)
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

pub(super) fn table_exists(conn: &Connection, name: &str) -> Result<bool, ZoteroDbError> {
    Ok(conn
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1 COLLATE NOCASE",
            [name],
            |_| Ok(()),
        )
        .optional()?
        .is_some())
}

pub(super) fn column_exists(
    conn: &Connection,
    table: &str,
    column: &str,
) -> Result<bool, ZoteroDbError> {
    Ok(conn
        .query_row(
            "SELECT 1 FROM pragma_table_info(?1) WHERE name=?2 COLLATE NOCASE",
            [table, column],
            |_| Ok(()),
        )
        .optional()?
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
    pub(super) fn probe(conn: &Connection) -> Result<Caps, ZoteroDbError> {
        let annotations = table_exists(conn, "itemAnnotations")?;
        Ok(Caps {
            annotations,
            annotation_author: annotations && column_exists(conn, "itemAnnotations", "authorName")?,
            publications: table_exists(conn, "publicationsItems")?,
            deleted_collections: table_exists(conn, "deletedCollections")?,
            deleted_searches: table_exists(conn, "deletedSearches")?,
            last_read: column_exists(conn, "itemAttachments", "lastRead")?,
            combined: table_exists(conn, "itemTypesCombined")?
                && table_exists(conn, "fieldsCombined")?,
        })
    }
}
