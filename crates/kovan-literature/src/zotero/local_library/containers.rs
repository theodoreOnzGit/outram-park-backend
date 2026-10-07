// Part of the kovan Zotero port (GitHub #747, #750).
//
// Upstream: Zotero, https://github.com/zotero/zotero (commit 9cbba8c4d281):
//   xpcom/data/libraries.js and groups.js (the `libraries`/`groups` tables),
//   xpcom/uri.js (`getLibraryPath`), xpcom/users.js (account settings),
//   xpcom/data/collections.js (`_primaryDataSQLParts` :35-60),
//   xpcom/data/collection.js (`toJSON` :826), xpcom/data/dataObjects.js
//   (`_loadRelations` :613), xpcom/data/searches.js (`_primaryDataSQLParts`
//   :33, `_loadConditions` :125), xpcom/data/search.js (`toJSON` :893),
//   xpcom/data/searchConditions.js (`parseCondition` :993), xpcom/schema.js
//   (`_updateCustomTables` :1170, the *Combined tables).
// Copyright (c) 2009 Center for History and New Media, George Mason
// University; (c) Corporation for Digital Scholarship, Vienna, Virginia, USA.
// Licence: AGPL-3.0 (upstream: AGPL-3.0-or-later).

//! Id -> name lookups, libraries, collections and saved searches.

use super::open::Caps;
use super::{LibraryKind, ReadReport, ZoteroDbError};
use kovan_common::zotero::{SearchCondition, ZoteroCollection, ZoteroSearch};
use rusqlite::types::ValueRef;
use rusqlite::{Connection, OptionalExtension};
use std::collections::{BTreeMap, HashMap};

/// A cell as text, the way JavaScript would stringify what mozStorage
/// returns: text as is, an integer or real as a number string, a blob as
/// (lossy) UTF-8; `None` for NULL.
pub(super) fn cell_text(v: ValueRef<'_>) -> Option<String> {
    match v {
        ValueRef::Null => None,
        ValueRef::Integer(i) => Some(i.to_string()),
        ValueRef::Real(f) => Some(if f.fract() == 0.0 && f.abs() < 1e15 {
            format!("{}", f as i64)
        } else {
            f.to_string()
        }),
        ValueRef::Text(t) => Some(String::from_utf8_lossy(t).into_owned()),
        ValueRef::Blob(b) => Some(String::from_utf8_lossy(b).into_owned()),
    }
}

/// The database's own id -> name tables (ids differ between databases).
pub(super) struct Lookups {
    pub item_types: HashMap<i64, String>,
    pub fields: HashMap<i64, String>,
    pub creator_types: HashMap<i64, String>,
}

fn id_name_map(conn: &Connection, sql: &str) -> Result<HashMap<i64, String>, ZoteroDbError> {
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))?;
    let mut m = HashMap::new();
    for row in rows {
        let (id, name) = row?;
        m.insert(id, name);
    }
    Ok(m)
}

impl Lookups {
    pub(super) fn load(conn: &Connection, caps: &Caps) -> Result<Lookups, ZoteroDbError> {
        // `itemTypesCombined`/`fieldsCombined` are what Zotero resolves ids
        // through (cachedTypes.js); they equal the plain tables plus custom
        // types, which are unused upstream.
        let (it, f) = if caps.combined {
            (
                "SELECT itemTypeID, typeName FROM itemTypesCombined",
                "SELECT fieldID, fieldName FROM fieldsCombined",
            )
        } else {
            (
                "SELECT itemTypeID, typeName FROM itemTypes",
                "SELECT fieldID, fieldName FROM fields",
            )
        };
        Ok(Lookups {
            item_types: id_name_map(conn, it)?,
            fields: id_name_map(conn, f)?,
            creator_types: id_name_map(
                conn,
                "SELECT creatorTypeID, creatorType FROM creatorTypes",
            )?,
        })
    }
}

/// One row of `libraries`, with its group and URI.
pub(super) struct LibraryInfo {
    pub library_id: i64,
    pub kind: LibraryKind,
    pub editable: bool,
    pub files_editable: bool,
    pub version: i64,
    pub uri: String,
}

fn account_setting(conn: &Connection, key: &str) -> Result<Option<String>, ZoteroDbError> {
    let v: Option<Option<String>> = conn
        .query_row(
            "SELECT value FROM settings WHERE setting='account' AND key=?1",
            [key],
            |r| Ok(cell_text(r.get_ref(0)?)),
        )
        .optional()?;
    Ok(v.flatten().filter(|s| !s.is_empty()))
}

/// User and group libraries, by `libraryID`; feeds skipped (noted).
pub(super) fn libraries(
    conn: &Connection,
    report: &mut ReadReport,
) -> Result<Vec<LibraryInfo>, ZoteroDbError> {
    // uri.js getLibraryPath: `users/<userID>` once synced, else
    // `users/local/<localUserKey>`.
    let user_segment = match account_setting(conn, "userID")? {
        Some(id) => id,
        None => format!(
            "local/{}",
            account_setting(conn, "localUserKey")?.unwrap_or_default()
        ),
    };
    let mut stmt = conn.prepare(
        "SELECT L.libraryID, L.type, L.editable, L.filesEditable, L.version, \
         G.groupID, G.name, G.description \
         FROM libraries L LEFT JOIN groups G USING (libraryID) ORDER BY L.libraryID",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, Option<i64>>(2)?.unwrap_or(0) != 0,
            r.get::<_, Option<i64>>(3)?.unwrap_or(0) != 0,
            r.get::<_, Option<i64>>(4)?.unwrap_or(0),
            r.get::<_, Option<i64>>(5)?,
            r.get::<_, Option<String>>(6)?,
            r.get::<_, Option<String>>(7)?,
        ))
    })?;
    let mut out = Vec::new();
    let mut feeds = 0;
    for row in rows {
        let (library_id, ty, editable, files_editable, version, gid, gname, gdesc) = row?;
        let (kind, uri) = match ty.as_str() {
            "user" => (
                LibraryKind::User,
                format!("http://zotero.org/users/{user_segment}"),
            ),
            "group" => {
                let group_id = gid.unwrap_or_default();
                (
                    LibraryKind::Group {
                        group_id,
                        name: gname.unwrap_or_default(),
                        description: gdesc.unwrap_or_default(),
                    },
                    format!("http://zotero.org/groups/{group_id}"),
                )
            }
            "feed" => {
                feeds += 1;
                continue;
            }
            other => {
                report.notes.push(format!(
                    "library {library_id} of unknown type '{other}' skipped"
                ));
                continue;
            }
        };
        out.push(LibraryInfo {
            library_id,
            kind,
            editable,
            files_editable,
            version,
            uri,
        });
    }
    if feeds > 0 {
        report.notes.push(format!(
            "{feeds} feed librar{} not read (RSS subscriptions)",
            if feeds == 1 { "y" } else { "ies" }
        ));
    }
    Ok(out)
}

/// Relations of every object of one library, keyed by the object's id
/// (dataObjects.js `_loadRelations`).
fn relations(
    conn: &Connection,
    sql: &str,
    library_id: i64,
) -> Result<HashMap<i64, BTreeMap<String, Vec<String>>>, ZoteroDbError> {
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map([library_id], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
        ))
    })?;
    let mut m: HashMap<i64, BTreeMap<String, Vec<String>>> = HashMap::new();
    for row in rows {
        let (id, pred, obj) = row?;
        m.entry(id).or_default().entry(pred).or_default().push(obj);
    }
    Ok(m)
}

pub(super) fn item_relations(
    conn: &Connection,
    library_id: i64,
) -> Result<HashMap<i64, BTreeMap<String, Vec<String>>>, ZoteroDbError> {
    // The PRIMARY KEY (itemID, predicateID, object) index is the order the
    // upstream join visits rows in.
    relations(
        conn,
        "SELECT R.itemID, P.predicate, R.object FROM items I \
         JOIN itemRelations R USING (itemID) JOIN relationPredicates P USING (predicateID) \
         WHERE I.libraryID=?1 ORDER BY R.itemID, R.predicateID, R.object",
        library_id,
    )
}

/// Collections of one library, by `collectionID` (collections.js,
/// collection.js `toJSON`).
pub(super) fn collections(
    conn: &Connection,
    caps: &Caps,
    library_id: i64,
) -> Result<Vec<ZoteroCollection>, ZoteroDbError> {
    let rels = relations(
        conn,
        "SELECT R.collectionID, P.predicate, R.object FROM collections C \
         JOIN collectionRelations R USING (collectionID) \
         JOIN relationPredicates P USING (predicateID) \
         WHERE C.libraryID=?1 ORDER BY R.collectionID, R.predicateID, R.object",
        library_id,
    )?;
    let deleted = if caps.deleted_collections {
        "DC.collectionID IS NOT NULL"
    } else {
        "0"
    };
    let join = if caps.deleted_collections {
        "LEFT JOIN deletedCollections DC ON (O.collectionID=DC.collectionID) "
    } else {
        ""
    };
    let sql = format!(
        "SELECT O.collectionID, O.collectionName, O.key, O.version, CP.key, {deleted} \
         FROM collections O {join}\
         LEFT JOIN collections CP ON (O.parentCollectionID=CP.collectionID) \
         WHERE O.libraryID=?1 ORDER BY O.collectionID"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([library_id], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            cell_text(r.get_ref(1)?).unwrap_or_default(),
            r.get::<_, String>(2)?,
            r.get::<_, Option<i64>>(3)?.unwrap_or(0),
            r.get::<_, Option<String>>(4)?,
            r.get::<_, i64>(5)? != 0,
        ))
    })?;
    let mut out = Vec::new();
    for row in rows {
        let (id, name, key, version, parent, is_deleted) = row?;
        let mut c = ZoteroCollection::new(name);
        c.key = Some(key);
        c.version = Some(version.max(0) as u64);
        c.parent_collection = parent;
        c.relations = rels.get(&id).cloned().unwrap_or_default();
        c.deleted = is_deleted.then_some(true);
        out.push(c);
    }
    Ok(out)
}

/// Saved searches of one library (searches.js, search.js `toJSON`).
pub(super) fn searches(
    conn: &Connection,
    caps: &Caps,
    lookups: &Lookups,
    library_id: i64,
) -> Result<Vec<ZoteroSearch>, ZoteroDbError> {
    let (deleted, join) = if caps.deleted_searches {
        (
            "DS.savedSearchID IS NOT NULL",
            "LEFT JOIN deletedSearches DS ON (O.savedSearchID=DS.savedSearchID) ",
        )
    } else {
        ("0", "")
    };
    let sql = format!(
        "SELECT O.savedSearchID, O.savedSearchName, O.key, O.version, {deleted} \
         FROM savedSearches O {join}WHERE O.libraryID=?1 ORDER BY O.savedSearchID"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([library_id], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            cell_text(r.get_ref(1)?).unwrap_or_default(),
            r.get::<_, String>(2)?,
            r.get::<_, Option<i64>>(3)?.unwrap_or(0),
            r.get::<_, i64>(4)? != 0,
        ))
    })?;
    let mut list: Vec<(i64, ZoteroSearch)> = Vec::new();
    for row in rows {
        let (id, name, key, version, is_deleted) = row?;
        let mut s = ZoteroSearch::new(name);
        s.key = Some(key);
        s.version = Some(version.max(0) as u64);
        s.deleted = is_deleted.then_some(true);
        list.push((id, s));
    }
    let mut cstmt = conn.prepare(
        "SELECT C.savedSearchID, C.condition, C.operator, C.value \
         FROM savedSearches S JOIN savedSearchConditions C USING (savedSearchID) \
         WHERE S.libraryID=?1 ORDER BY C.savedSearchID, C.searchConditionID",
    )?;
    let crows = cstmt.query_map([library_id], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            cell_text(r.get_ref(1)?).unwrap_or_default(),
            cell_text(r.get_ref(2)?),
            cell_text(r.get_ref(3)?),
        ))
    })?;
    let mut by_id: HashMap<i64, Vec<StoredCondition>> = HashMap::new();
    for row in crows {
        let (id, c, op, v) = row?;
        by_id.entry(id).or_default().push((c, op, v));
    }
    for (id, s) in &mut list {
        let rows = by_id.remove(id).unwrap_or_default();
        s.conditions = load_conditions(rows, lookups);
    }
    Ok(list.into_iter().map(|(_, s)| s).collect())
}

/// One `savedSearchConditions` row: `condition`, `operator`, `value`.
type StoredCondition = (String, Option<String>, Option<String>);

/// `_loadConditions` (searches.js:145-215) followed by `toJSON`'s
/// `condition[/mode]` (search.js:902).
fn load_conditions(rows: Vec<StoredCondition>, lookups: &Lookups) -> Vec<SearchCondition> {
    let mut out = Vec::new();
    let mut migrated_child_note = false;
    for (stored, operator, value) in rows {
        // parseCondition: "condition[/mode]".
        let (mut name, mode) = match stored.find('/') {
            Some(p) => (stored[..p].to_owned(), Some(stored[p + 1..].to_owned())),
            None => (stored.clone(), None),
        };
        // NULL value -> ''.
        let mut value = value.unwrap_or_default();
        if name == "childNote" {
            name = "note".into();
            migrated_child_note = true;
        }
        if name == "itemTypeID" {
            name = "itemType".into();
            value = value
                .parse::<i64>()
                .ok()
                .and_then(|id| lookups.item_types.get(&id).cloned())
                .unwrap_or_default();
        } else if (name == "collection" || name == "savedSearch") && value.contains('_') {
            // '0_ABCD2345' -> 'ABCD2345' (`split('_')[1]`).
            value = value.split('_').nth(1).unwrap_or_default().to_owned();
        }
        let condition = match mode {
            Some(m) => format!("{name}/{m}"),
            None => name,
        };
        out.push(SearchCondition {
            condition,
            operator,
            value,
        });
    }
    if migrated_child_note {
        out.push(SearchCondition {
            condition: "resultLevel".into(),
            operator: Some("item".into()),
            value: String::new(),
        });
    }
    out
}
