// Part of the kovan Zotero port (GitHub #747, #750).
//
// Uses turso_core 0.8.2 (Turso's Rust rewrite of SQLite; MIT,
// https://github.com/tursodatabase/turso) as the SQLite engine. No upstream
// Zotero code is ported in this file. The hot-journal test mirrors SQLite's
// `hasHotJournal` (pager.c; SQLite 3.53.2 amalgamation, public domain).
// Licence: AGPL-3.0-only (this crate). See this crate's NOTICE.

//! The database snapshot the reader queries: turso_core over in-memory files.
//!
//! The reader never needs a file on disk. The bytes of `zotero.sqlite` (and
//! of its `-wal`, which holds committed transactions not yet checkpointed
//! into the main file) are placed in a private in-memory file system
//! ([`turso_core::MemoryIO`]) and turso opens them there: it validates the
//! WAL header, frame salts and checksums and replays committed frames up to
//! the last commit frame, as SQLite's `wal.c` does (checked against real
//! SQLite in `tests/zotero_sqlite_format.rs`). Anything turso writes while
//! opening (a checkpoint, a header rewrite) lands in that memory and is
//! dropped with the snapshot; the user's data folder is only ever read.
//!
//! Two things differ from turso's stock `MemoryIO`, both for wasm32:
//!
//! * **The clock.** `MemoryIO` reads `std::time::Instant`, which panics on
//!   `wasm32-unknown-unknown` (observed 2026-10-07 under
//!   `wasm-bindgen-test-runner`: the panic is in `Instant::now` reached from
//!   turso's `MonotonicInstant`). [`SnapshotIo`] supplies a logical clock
//!   instead: a counter for monotonic time (turso uses it only for query
//!   timeouts and busy back-off, neither of which a private snapshot can
//!   reach) and the Unix epoch for wall-clock time (used by SQL date
//!   functions with `'now'`, which the reader never calls).
//! * **A unique path per snapshot.** turso keeps a process-wide registry of
//!   open databases keyed by file identity, which for a memory file is a hash
//!   of its path; two snapshots opened at the same path would share one
//!   database. Each snapshot gets its own path from a counter.
//!
//! No SQL is translated: the reader's queries are SQLite SQL, and turso runs
//! them as written.

use super::ZoteroDbError;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use turso_core::{
    Buffer, Clock, Completion, Connection, Database, MemoryIO, MonotonicInstant, Numeric,
    OpenFlags, OpenOptions, SqliteDialect, StepResult, Value, WallClockInstant, IO,
};

/// Source of the per-snapshot paths (see the module docs).
static NEXT_SNAPSHOT: AtomicU64 = AtomicU64::new(0);

/// turso's in-memory file system with a clock that works on every target.
struct SnapshotIo {
    files: MemoryIO,
    ticks: AtomicU64,
}

impl Clock for SnapshotIo {
    fn current_time_monotonic(&self) -> MonotonicInstant {
        MonotonicInstant::from_nanos(u128::from(self.ticks.fetch_add(1, Ordering::Relaxed)))
    }

    fn current_time_wall_clock(&self) -> WallClockInstant {
        WallClockInstant { secs: 0, micros: 0 }
    }
}

impl IO for SnapshotIo {
    fn open_file(
        &self,
        path: &str,
        flags: OpenFlags,
        direct: bool,
    ) -> turso_core::Result<Arc<dyn turso_core::File>> {
        self.files.open_file(path, flags, direct)
    }

    fn remove_file(&self, path: &str) -> turso_core::Result<()> {
        self.files.remove_file(path)
    }

    fn file_id(&self, path: &str) -> turso_core::Result<turso_core::io::FileId> {
        self.files.file_id(path)
    }

    fn step(&self) -> turso_core::Result<()> {
        self.files.step()
    }

    fn supports_shared_wal_coordination(&self) -> bool {
        false
    }
}

impl From<turso_core::LimboError> for ZoteroDbError {
    fn from(e: turso_core::LimboError) -> Self {
        ZoteroDbError::Sqlite(e.to_string())
    }
}

/// Write `bytes` as the whole content of the memory file `path`.
fn put_file(io: &SnapshotIo, path: &str, bytes: &[u8]) -> Result<(), ZoteroDbError> {
    let file = io.open_file(path, OpenFlags::Create, false)?;
    if bytes.is_empty() {
        return Ok(());
    }
    let buf = Arc::new(Buffer::new(bytes.to_vec()));
    let done = file.pwrite(0, buf, Completion::new_write(|_| {}))?;
    io.wait_for_completion(done)?;
    Ok(())
}

/// One cell of a result row, as SQLite stores it.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum Cell {
    Null,
    Integer(i64),
    Real(f64),
    Text(String),
    Blob(Vec<u8>),
}

impl Cell {
    fn from_value(v: &Value) -> Cell {
        match v {
            Value::Null => Cell::Null,
            Value::Numeric(Numeric::Integer(i)) => Cell::Integer(*i),
            Value::Numeric(Numeric::Float(f)) => Cell::Real(f64::from(*f)),
            Value::Text(t) => Cell::Text(t.as_str().to_owned()),
            Value::Blob(b) => Cell::Blob(b.as_slice().to_vec()),
        }
    }

    fn type_name(&self) -> &'static str {
        match self {
            Cell::Null => "NULL",
            Cell::Integer(_) => "INTEGER",
            Cell::Real(_) => "REAL",
            Cell::Text(_) => "TEXT",
            Cell::Blob(_) => "BLOB",
        }
    }
}

/// A bound parameter.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum Param {
    Int(i64),
    Text(String),
}

impl From<i64> for Param {
    fn from(i: i64) -> Self {
        Param::Int(i)
    }
}

impl From<&str> for Param {
    fn from(s: &str) -> Self {
        Param::Text(s.to_owned())
    }
}

/// One result row. The typed getters are as strict as rusqlite's `get`
/// (which the reader used before 2026-10-07): an integer getter refuses a
/// REAL or TEXT cell, a string getter refuses anything but TEXT, and the
/// non-`Option` getters refuse NULL; the error names the column.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Row {
    cells: Vec<Cell>,
}

impl Row {
    pub(super) fn cell(&self, i: usize) -> Result<&Cell, ZoteroDbError> {
        self.cells
            .get(i)
            .ok_or_else(|| ZoteroDbError::Sqlite(format!("no column {i} in the result")))
    }

    fn wrong(&self, i: usize, want: &str) -> ZoteroDbError {
        let got = self.cells.get(i).map_or("nothing", Cell::type_name);
        ZoteroDbError::Sqlite(format!("column {i}: expected {want}, found {got}"))
    }

    pub(super) fn opt_i64(&self, i: usize) -> Result<Option<i64>, ZoteroDbError> {
        match self.cell(i)? {
            Cell::Null => Ok(None),
            Cell::Integer(v) => Ok(Some(*v)),
            _ => Err(self.wrong(i, "INTEGER")),
        }
    }

    pub(super) fn i64(&self, i: usize) -> Result<i64, ZoteroDbError> {
        self.opt_i64(i)?.ok_or_else(|| self.wrong(i, "INTEGER"))
    }

    pub(super) fn opt_string(&self, i: usize) -> Result<Option<String>, ZoteroDbError> {
        match self.cell(i)? {
            Cell::Null => Ok(None),
            Cell::Text(s) => Ok(Some(s.clone())),
            _ => Err(self.wrong(i, "TEXT")),
        }
    }

    pub(super) fn string(&self, i: usize) -> Result<String, ZoteroDbError> {
        self.opt_string(i)?.ok_or_else(|| self.wrong(i, "TEXT"))
    }

    /// The cell as text, the way JavaScript would stringify what mozStorage
    /// returns: text as is, an integer or real as a number string, a blob
    /// as (lossy) UTF-8; `None` for NULL.
    pub(super) fn text(&self, i: usize) -> Result<Option<String>, ZoteroDbError> {
        Ok(match self.cell(i)? {
            Cell::Null => None,
            Cell::Integer(v) => Some(v.to_string()),
            Cell::Real(f) => Some(if f.fract() == 0.0 && f.abs() < 1e15 {
                format!("{}", *f as i64)
            } else {
                f.to_string()
            }),
            Cell::Text(t) => Some(t.clone()),
            Cell::Blob(b) => Some(String::from_utf8_lossy(b).into_owned()),
        })
    }
}

/// An open snapshot of a database.
pub(super) struct Db {
    // Field order: the connection goes before the database and its files.
    conn: Arc<Connection>,
    _db: Arc<Database>,
    _io: Arc<SnapshotIo>,
}

impl Db {
    /// Open the database `main` with its WAL `wal` (if any), from memory.
    pub(super) fn open(main: &[u8], wal: Option<&[u8]>) -> Result<Db, ZoteroDbError> {
        let n = NEXT_SNAPSHOT.fetch_add(1, Ordering::Relaxed);
        let path = format!("kovan-zotero-snapshot-{n}.sqlite");
        let io = Arc::new(SnapshotIo {
            files: MemoryIO::new(),
            ticks: AtomicU64::new(0),
        });
        put_file(&io, &path, main)?;
        if let Some(w) = wal.filter(|w| !w.is_empty()) {
            put_file(&io, &format!("{path}-wal"), w)?;
        }
        let db = Database::open(io.clone(), &path, OpenOptions::new(Arc::new(SqliteDialect)))?;
        let conn = db.connect()?;
        Ok(Db {
            conn,
            _db: db,
            _io: io,
        })
    }

    /// Run `sql` with `params` bound to `?1`, `?2`, ... and collect every row.
    pub(super) fn rows(&self, sql: &str, params: &[Param]) -> Result<Vec<Row>, ZoteroDbError> {
        let mut stmt = self.conn.prepare(sql)?;
        for (i, p) in params.iter().enumerate() {
            let idx = std::num::NonZero::new(i + 1).expect("i + 1 > 0");
            let v = match p {
                Param::Int(v) => Value::from_i64(*v),
                Param::Text(s) => Value::build_text(s.clone()),
            };
            stmt.bind_at(idx, v)?;
        }
        let mut out = Vec::new();
        loop {
            match stmt.step()? {
                StepResult::Row => {
                    let row = stmt
                        .row()
                        .ok_or_else(|| ZoteroDbError::Sqlite("row without values".into()))?;
                    out.push(Row {
                        cells: row.get_values().map(Cell::from_value).collect(),
                    });
                }
                StepResult::Done => return Ok(out),
                StepResult::IO | StepResult::Yield => stmt._io().step()?,
                other => {
                    return Err(ZoteroDbError::Sqlite(format!(
                        "unexpected step result {other:?} for {sql}"
                    )))
                }
            }
        }
    }

    /// The first row, if any.
    pub(super) fn first_row(
        &self,
        sql: &str,
        params: &[Param],
    ) -> Result<Option<Row>, ZoteroDbError> {
        Ok(self.rows(sql, params)?.into_iter().next())
    }
}

/// Whether `journal` is a hot rollback journal for `main`: SQLite's
/// `hasHotJournal` (pager.c) with no other process holding a lock, which is
/// the situation of a copy: the journal is non-empty, its first byte is not
/// zero (a zeroed header is a finished `journal_mode=PERSIST` journal), and
/// the database has at least one page. SQLite would roll such a journal
/// back before reading; this reader does not (see the parent module docs).
pub(super) fn is_hot_journal(main: &[u8], journal: Option<&[u8]>) -> bool {
    journal.is_some_and(|j| j.first().is_some_and(|b| *b != 0)) && !main.is_empty()
}
