// Part of the kovan Zotero port (GitHub #747, #750): test-only builder of
// Zotero databases, following Zotero's own code paths.
//
// Upstream: Zotero, https://github.com/zotero/zotero (commit 9cbba8c4d281):
//   xpcom/schema.js (`_initializeSchema` :2453, `_updateGlobalSchema` :459,
//   `_updateCustomTables` :1170); xpcom/data/item.js (`_saveData` :1616 and
//   the note, attachment, annotation, collection, tag and relation blocks
//   :1999-2400; `setField` :662 for date and accessDate storage);
//   xpcom/data/creators.js (`getIDFromData` :102, `cleanData` :186);
//   xpcom/data/tags.js (`create`, :122); xpcom/data/dataObject.js (relations,
//   :1240-1262); xpcom/data/collection.js (`_saveData` :290); xpcom/data/
//   search.js (`_saveData` :194-240); xpcom/data/notes.js (:32-33).
// Copyright (c) 2009 Center for History and New Media, George Mason
// University; (c) Corporation for Digital Scholarship, Vienna, Virginia, USA.
// Licence: AGPL-3.0 (upstream: AGPL-3.0-or-later).
//
// The schema itself is not re-typed here: `create` executes upstream's own
// system.sql, userdata.sql and triggers.sql (copied verbatim into
// tests/data/zotero/), then fills the global-schema tables the way
// `_updateGlobalSchema` does from schema.json (here from kovan-common's
// tables generated from the same schema.json v45).

#![allow(dead_code)]

use kovan_common::zotero::date::{iso_to_sql, str_to_multipart, DateOptions};
use kovan_common::zotero::schema::field_from_type_and_base;
use kovan_common::zotero::schema_generated::ITEM_TYPE_SCHEMAS;
use kovan_common::zotero::{
    AnnotationType, CreatorName, Field, ItemType, LinkMode, ZoteroCollection, ZoteroItem,
};
use rusqlite::{params, Connection, OptionalExtension};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub const SYSTEM_SQL: &str = include_str!("../data/zotero/system.sql");
pub const USERDATA_SQL: &str = include_str!("../data/zotero/userdata.sql");
pub const TRIGGERS_SQL: &str = include_str!("../data/zotero/triggers.sql");

/// `_getSchemaSQLVersion`: the number on the first line (`-- 130`).
pub fn sql_version(sql: &str) -> i64 {
    sql.lines()
        .next()
        .and_then(|l| l.trim_start_matches("--").trim().parse().ok())
        .expect("schema file version line")
}

/// In which order ids are handed out to item types, fields and creator
/// types. Zotero resolves them by name; a reader must not assume them.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum IdOrder {
    /// schema.json order, as `_updateGlobalSchema` does for a new database.
    Forward,
    /// Reversed, standing in for a database whose ids came from elsewhere
    /// (e.g. upgraded from Zotero 4 through `system-107.sql`).
    Reverse,
}

pub struct ZoteroDb {
    pub conn: Connection,
    pub dir: PathBuf,
    item_types: HashMap<String, i64>,
    fields: HashMap<String, i64>,
    creator_types: HashMap<String, i64>,
}

fn next_id(conn: &Connection, table: &str, col: &str) -> i64 {
    // Zotero.ID.get: the next free id of the table.
    conn.query_row(
        &format!("SELECT IFNULL(MAX({col}), 0) + 1 FROM {table}"),
        [],
        |r| r.get(0),
    )
    .unwrap()
}

impl ZoteroDb {
    /// A new data folder at `dir` with `zotero.sqlite`, created as
    /// `_initializeSchema` (schema.js:2453) creates it.
    pub fn create(dir: &Path, order: IdOrder) -> ZoteroDb {
        std::fs::create_dir_all(dir).unwrap();
        let conn = Connection::open(dir.join("zotero.sqlite")).unwrap();
        conn.execute_batch("PRAGMA page_size = 4096; PRAGMA encoding = 'UTF-8';")
            .unwrap();
        conn.execute_batch("BEGIN").unwrap();
        conn.execute_batch(SYSTEM_SQL).unwrap();
        conn.execute_batch(USERDATA_SQL).unwrap();
        conn.execute_batch(TRIGGERS_SQL).unwrap();
        let mut db = ZoteroDb {
            conn,
            dir: dir.to_path_buf(),
            item_types: HashMap::new(),
            fields: HashMap::new(),
            creator_types: HashMap::new(),
        };
        db.update_global_schema(order);
        db.update_custom_tables();
        for (schema, v) in [
            ("system", sql_version(SYSTEM_SQL)),
            ("userdata", sql_version(USERDATA_SQL)),
            ("triggers", sql_version(TRIGGERS_SQL)),
            ("compatibility", 9),
        ] {
            db.conn
                .execute(
                    "REPLACE INTO version (schema, version) VALUES (?1, ?2)",
                    params![schema, v],
                )
                .unwrap();
        }
        db.conn
            .execute(
                "INSERT INTO libraries (libraryID, type, editable, filesEditable) VALUES (1, 'user', 1, 1)",
                [],
            )
            .unwrap();
        db.conn
            .execute(
                "INSERT INTO settings VALUES ('account', 'localUserKey', 'Ab3dE6gH')",
                [],
            )
            .unwrap();
        db.conn
            .execute_batch("COMMIT; PRAGMA foreign_keys = ON;")
            .unwrap();
        db
    }

    /// `_updateGlobalSchema` (schema.js:459-600) on an empty database.
    fn update_global_schema(&mut self, order: IdOrder) {
        let mut types: Vec<_> = ITEM_TYPE_SCHEMAS.iter().collect();
        // JS Sets keep insertion order: fields (then their base field) and
        // creator types as met while walking the item types.
        let mut fields: Vec<&str> = Vec::new();
        let mut ctypes: Vec<&str> = Vec::new();
        for s in &types {
            for f in s.fields {
                for name in
                    std::iter::once(f.field.as_str()).chain(f.base_field.map(|b| b.as_str()))
                {
                    if !fields.contains(&name) {
                        fields.push(name);
                    }
                }
            }
            for c in s.creator_types {
                if !ctypes.contains(&c.as_str()) {
                    ctypes.push(c.as_str());
                }
            }
        }
        if order == IdOrder::Reverse {
            types.reverse();
            fields.reverse();
            ctypes.reverse();
        }
        for f in &fields {
            let id = next_id(&self.conn, "fields", "fieldID");
            self.conn
                .execute("INSERT INTO fields VALUES (?1, ?2, NULL)", params![id, f])
                .unwrap();
            self.fields.insert((*f).to_owned(), id);
        }
        for c in &ctypes {
            let id = next_id(&self.conn, "creatorTypes", "creatorTypeID");
            self.conn
                .execute("INSERT INTO creatorTypes VALUES (?1, ?2)", params![id, c])
                .unwrap();
            self.creator_types.insert((*c).to_owned(), id);
        }
        for s in &types {
            let id = next_id(&self.conn, "itemTypes", "itemTypeID");
            self.conn
                .execute(
                    "INSERT INTO itemTypes VALUES (?1, ?2, NULL, 1)",
                    params![id, s.item_type.as_str()],
                )
                .unwrap();
            self.item_types.insert(s.item_type.as_str().to_owned(), id);
            for (index, f) in s.fields.iter().enumerate() {
                let fid = self.fields[f.field.as_str()];
                self.conn
                    .execute(
                        "INSERT INTO itemTypeFields VALUES (?1, ?2, 0, ?3)",
                        params![id, fid, index as i64],
                    )
                    .unwrap();
                if let Some(b) = f.base_field {
                    self.conn
                        .execute(
                            "INSERT INTO baseFieldMappings VALUES (?1, ?2, ?3)",
                            params![id, self.fields[b.as_str()], fid],
                        )
                        .unwrap();
                }
            }
            for c in s.creator_types {
                let primary = s.primary_creator_type == Some(*c);
                self.conn
                    .execute(
                        "INSERT INTO itemTypeCreatorTypes VALUES (?1, ?2, ?3)",
                        params![id, self.creator_types[c.as_str()], primary as i64],
                    )
                    .unwrap();
            }
        }
    }

    /// `_updateCustomTables` (schema.js:1170-1207), verbatim SQL.
    fn update_custom_tables(&self) {
        let offset = 10000;
        self.conn
            .execute_batch(&format!(
                "DELETE FROM itemTypesCombined; DELETE FROM fieldsCombined; \
                 DELETE FROM itemTypeFieldsCombined; DELETE FROM baseFieldMappingsCombined; \
                 INSERT INTO itemTypesCombined SELECT itemTypeID, typeName, display, 0 AS custom FROM itemTypes UNION \
                 SELECT customItemTypeID + {offset} AS itemTypeID, typeName, display, 1 AS custom FROM customItemTypes; \
                 INSERT INTO fieldsCombined SELECT fieldID, fieldName, NULL AS label, fieldFormatID, 0 AS custom FROM fields UNION \
                 SELECT customFieldID + {offset} AS fieldID, fieldName, label, NULL, 1 AS custom FROM customFields; \
                 INSERT INTO itemTypeFieldsCombined SELECT itemTypeID, fieldID, hide, orderIndex FROM itemTypeFields UNION \
                 SELECT customItemTypeID + {offset} AS itemTypeID, COALESCE(fieldID, customFieldID + {offset}) AS fieldID, hide, orderIndex FROM customItemTypeFields; \
                 INSERT INTO baseFieldMappingsCombined SELECT itemTypeID, baseFieldID, fieldID FROM baseFieldMappings UNION \
                 SELECT customItemTypeID + {offset} AS itemTypeID, baseFieldID, customFieldID + {offset} AS fieldID FROM customBaseFieldMappings;"
            ))
            .unwrap();
    }

    pub fn field_id(&self, name: &str) -> i64 {
        self.fields[name]
    }

    pub fn item_type_id(&self, name: &str) -> i64 {
        self.item_types[name]
    }

    /// A group library (groups.js / group.js save).
    pub fn add_group(&self, group_id: i64, name: &str, description: &str) -> i64 {
        let lib = next_id(&self.conn, "libraries", "libraryID");
        self.conn
            .execute(
                "INSERT INTO libraries (libraryID, type, editable, filesEditable, version) \
                 VALUES (?1, 'group', 1, 0, 7)",
                [lib],
            )
            .unwrap();
        self.conn
            .execute(
                "INSERT INTO groups VALUES (?1, ?2, ?3, ?4, 7)",
                params![group_id, lib, name, description],
            )
            .unwrap();
        lib
    }

    /// A feed library (feed.js save).
    pub fn add_feed(&self, name: &str, url: &str) -> i64 {
        let lib = next_id(&self.conn, "libraries", "libraryID");
        self.conn
            .execute(
                "INSERT INTO libraries (libraryID, type, editable, filesEditable) VALUES (?1, 'feed', 0, 0)",
                [lib],
            )
            .unwrap();
        self.conn
            .execute(
                "INSERT INTO feeds (libraryID, name, url) VALUES (?1, ?2, ?3)",
                params![lib, name, url],
            )
            .unwrap();
        lib
    }

    fn id_of(&self, table: &str, id_col: &str, library_id: i64, key: &str) -> i64 {
        self.conn
            .query_row(
                &format!("SELECT {id_col} FROM {table} WHERE libraryID=?1 AND key=?2"),
                params![library_id, key],
                |r| r.get(0),
            )
            .unwrap_or_else(|_| panic!("{table} {key} not saved yet"))
    }

    /// `Zotero.Collection#_saveData` (collection.js:290) + relations +
    /// `deletedCollections` (collection.js:671).
    pub fn save_collection(&self, library_id: i64, c: &ZoteroCollection) -> i64 {
        let id = next_id(&self.conn, "collections", "collectionID");
        let parent = c
            .parent_collection
            .as_deref()
            .map(|k| self.id_of("collections", "collectionID", library_id, k));
        self.conn
            .execute(
                "INSERT INTO collections (collectionID, collectionName, parentCollectionID, \
                 libraryID, key, version) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    id,
                    c.name,
                    parent,
                    library_id,
                    c.key.as_deref().unwrap(),
                    c.version.unwrap_or(0) as i64
                ],
            )
            .unwrap();
        for (pred, objs) in &c.relations {
            for o in objs {
                let p = self.predicate_id(pred);
                self.conn
                    .execute(
                        "INSERT INTO collectionRelations (collectionID, predicateID, object) VALUES (?1, ?2, ?3)",
                        params![id, p, o],
                    )
                    .unwrap();
            }
        }
        if c.deleted == Some(true) {
            self.conn
                .execute(
                    "INSERT OR IGNORE INTO deletedCollections (collectionID) VALUES (?1)",
                    [id],
                )
                .unwrap();
        }
        id
    }

    /// `Zotero.Search#_saveData` (search.js:194-240): conditions stored as
    /// `condition[/mode]`, operator and value NULL when empty.
    pub fn save_search(
        &self,
        library_id: i64,
        key: &str,
        name: &str,
        conditions: &[(&str, Option<&str>, Option<&str>)],
        deleted: bool,
    ) -> i64 {
        let id = next_id(&self.conn, "savedSearches", "savedSearchID");
        self.conn
            .execute(
                "INSERT INTO savedSearches (savedSearchID, savedSearchName, libraryID, key, version) \
                 VALUES (?1, ?2, ?3, ?4, 2)",
                params![id, name, library_id, key],
            )
            .unwrap();
        for (i, (c, op, v)) in conditions.iter().enumerate() {
            self.conn
                .execute(
                    "INSERT INTO savedSearchConditions \
                     (savedSearchID, searchConditionID, condition, operator, value) \
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![id, i as i64, c, op, v],
                )
                .unwrap();
        }
        if deleted {
            self.conn
                .execute(
                    "INSERT OR IGNORE INTO deletedSearches (savedSearchID) VALUES (?1)",
                    [id],
                )
                .unwrap();
        }
        id
    }

    /// `Zotero.RelationPredicates.add`.
    fn predicate_id(&self, predicate: &str) -> i64 {
        if let Some(id) = self
            .conn
            .query_row(
                "SELECT predicateID FROM relationPredicates WHERE predicate=?1",
                [predicate],
                |r| r.get(0),
            )
            .optional()
            .unwrap()
        {
            return id;
        }
        self.conn
            .execute(
                "INSERT INTO relationPredicates (predicateID, predicate) VALUES (NULL, ?1)",
                [predicate],
            )
            .unwrap();
        self.conn.last_insert_rowid()
    }

    /// Store one itemData value (item.js:1745-1790): look the value up in
    /// `itemDataValues`, insert it when new, point `itemData` at it.
    pub fn put_item_data(&self, item_id: i64, field_id: i64, value: &str) {
        let vid: Option<i64> = self
            .conn
            .query_row(
                "SELECT valueID FROM itemDataValues WHERE value=?1",
                [value],
                |r| r.get(0),
            )
            .optional()
            .unwrap();
        let vid = match vid {
            Some(v) => v,
            None => {
                let v = next_id(&self.conn, "itemDataValues", "valueID");
                self.conn
                    .execute(
                        "INSERT INTO itemDataValues (valueID, value, valueNormalized) VALUES (?1, ?2, ?3)",
                        params![v, value, value.to_lowercase()],
                    )
                    .unwrap();
                v
            }
        };
        self.conn
            .execute(
                "REPLACE INTO itemData VALUES (?1, ?2, ?3)",
                params![item_id, field_id, vid],
            )
            .unwrap();
    }

    /// Save `item` as `Zotero.Item#_saveData` (item.js:1616-2400) stores it,
    /// from the values `fromJSON` + `setField` would hold. The parent, the
    /// collections and the related items it names must already be saved.
    pub fn save_item(&self, library_id: i64, item: &ZoteroItem) -> i64 {
        let c = &self.conn;
        let id = next_id(c, "items", "itemID");
        let t = item.item_type;
        let sql_date = |iso: &Option<String>| {
            iso.as_deref()
                .map(|d| iso_to_sql(d).expect("ISO date"))
                .unwrap_or_else(|| "2026-10-07 00:00:00".to_owned())
        };
        c.execute(
            "INSERT INTO items (itemID, itemTypeID, dateAdded, dateModified, libraryID, key, version) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                id,
                self.item_types[t.as_str()],
                sql_date(&item.date_added),
                sql_date(&item.date_modified),
                library_id,
                item.key.as_deref().unwrap(),
                item.version.unwrap_or(0) as i64
            ],
        )
        .unwrap();
        // itemData, as `setField` (not loadIn) would hold the values.
        for (name, value) in &item.fields {
            let value = value.trim();
            if value.is_empty() {
                continue;
            }
            let field = Field::from_name(name).expect("known field");
            let field = field_from_type_and_base(t, field).unwrap_or(field);
            let stored = if field.is_date() {
                str_to_multipart(value, &DateOptions::default())
            } else if field == Field::AccessDate {
                iso_to_sql(value).unwrap_or_else(|| value.to_owned())
            } else {
                value.to_owned()
            };
            self.put_item_data(id, self.fields[field.as_str()], &stored);
        }
        // Creators (item.js:1795-1840, creators.js getIDFromData).
        for (order, cr) in item.creators.iter().enumerate() {
            let (first, last, mode) = match &cr.name {
                CreatorName::TwoField {
                    first_name,
                    last_name,
                } => (first_name.trim().to_owned(), last_name.trim().to_owned(), 0),
                CreatorName::SingleField { name } => (String::new(), name.trim().to_owned(), 1),
            };
            let cid: Option<i64> = c
                .query_row(
                    "SELECT creatorID FROM creators WHERE firstName=?1 AND lastName=?2 AND fieldMode=?3",
                    params![first, last, mode],
                    |r| r.get(0),
                )
                .optional()
                .unwrap();
            let cid = cid.unwrap_or_else(|| {
                let n = next_id(c, "creators", "creatorID");
                c.execute(
                    "INSERT INTO creators (creatorID, firstName, lastName, fieldMode, \
                     firstNameNormalized, lastNameNormalized) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![
                        n,
                        first,
                        last,
                        mode,
                        first.to_lowercase(),
                        last.to_lowercase()
                    ],
                )
                .unwrap();
                n
            });
            c.execute(
                "INSERT OR REPLACE INTO itemCreators (itemID, creatorID, creatorTypeID, orderIndex) \
                 VALUES (?1, ?2, ?3, ?4)",
                params![id, cid, self.creator_types[cr.creator_type.as_str()], order as i64],
            )
            .unwrap();
        }
        let parent_id = item
            .parent_item
            .as_deref()
            .map(|k| self.id_of("items", "itemID", library_id, k));
        if item.deleted == Some(true) {
            c.execute("REPLACE INTO deletedItems (itemID) VALUES (?1)", [id])
                .unwrap();
        }
        if item.in_publications == Some(true) {
            c.execute(
                "INSERT OR IGNORE INTO publicationsItems (itemID) VALUES (?1)",
                [id],
            )
            .unwrap();
        }
        // Note (item.js:2120-2150): wrapped, parentItemID only for notes.
        let is_note = t == ItemType::Note;
        if is_note || item.note.as_deref().is_some_and(|n| !n.is_empty()) {
            let mut text = item.note.clone().unwrap_or_default();
            if !(text.starts_with("<div class=\"zotero-note znv") && text.ends_with("</div>")) {
                text = format!("<div class=\"zotero-note znv1\">{text}</div>");
            }
            c.execute(
                "INSERT INTO itemNotes (itemID, parentItemID, note, title) VALUES (?1, ?2, ?3, '')",
                params![id, if is_note { parent_id } else { None }, text],
            )
            .unwrap();
        }
        // Attachment (item.js:2172-2215).
        if let Some(a) = &item.attachment {
            let mode = a.link_mode.expect("link mode");
            let path = match mode {
                LinkMode::LinkedFile => a.path.clone(),
                LinkMode::LinkedUrl => None,
                _ => a.filename.as_ref().map(|f| format!("storage:{f}")),
            };
            let charset_id: Option<i64> =
                a.charset.as_deref().filter(|s| !s.is_empty()).map(|cs| {
                    c.query_row(
                        "SELECT charsetID FROM charsets WHERE charset=?1",
                        [cs],
                        |r| r.get(0),
                    )
                    .unwrap()
                });
            let mode_n = LinkMode::ALL.iter().position(|m| *m == mode).unwrap() as i64;
            c.execute(
                "INSERT INTO itemAttachments (itemID, parentItemID, linkMode, contentType, charsetID, \
                 path, syncState, storageModTime, storageHash, lastProcessedModificationTime, lastRead) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0, ?7, ?8, NULL, ?9)",
                params![
                    id,
                    parent_id,
                    mode_n,
                    a.content_type.as_deref().filter(|s| !s.is_empty()),
                    charset_id,
                    path,
                    a.mtime,
                    a.md5,
                    a.last_read
                ],
            )
            .unwrap();
        }
        // Annotation (item.js:2254-2300).
        if let Some(an) = &item.annotation {
            let t = an.annotation_type.expect("annotation type");
            let type_id = AnnotationType::ALL.iter().position(|x| *x == t).unwrap() as i64 + 1;
            let external =
                item.other.get("annotationIsExternal") == Some(&serde_json::Value::Bool(true));
            let nz = |s: &Option<String>| s.clone().filter(|v| !v.is_empty());
            c.execute(
                "REPLACE INTO itemAnnotations (itemID, parentItemID, type, authorName, text, \
                 textNormalized, comment, commentNormalized, color, pageLabel, sortIndex, position, \
                 isExternal) VALUES (?1, ?2, ?3, ?4, ?5, NULL, ?6, NULL, ?7, ?8, ?9, ?10, ?11)",
                params![
                    id,
                    parent_id.expect("annotation parent"),
                    type_id,
                    nz(&an.author_name),
                    nz(&an.text),
                    nz(&an.comment),
                    nz(&an.color).unwrap_or_else(|| "#ffd400".into()),
                    nz(&an.page_label),
                    an.sort_index.clone().unwrap_or_default(),
                    an.position.clone().unwrap_or_default(),
                    external as i64
                ],
            )
            .unwrap();
        }
        // Collections (item.js:2325-2345).
        for ck in &item.collections {
            let cid = self.id_of("collections", "collectionID", library_id, ck);
            let order: i64 = c
                .query_row(
                    "SELECT IFNULL(MAX(orderIndex)+1, 0) FROM collectionItems WHERE collectionID=?1",
                    [cid],
                    |r| r.get(0),
                )
                .unwrap();
            c.execute(
                "INSERT OR IGNORE INTO collectionItems (collectionID, itemID, orderIndex) VALUES (?1, ?2, ?3)",
                params![cid, id, order],
            )
            .unwrap();
        }
        // Tags (item.js:2350-2380, tags.js:122).
        for tag in &item.tags {
            let name = tag.tag.trim();
            let tid: Option<i64> = c
                .query_row("SELECT tagID FROM tags WHERE name=?1", [name], |r| r.get(0))
                .optional()
                .unwrap();
            let tid = tid.unwrap_or_else(|| {
                let n = next_id(c, "tags", "tagID");
                c.execute(
                    "INSERT INTO tags (tagID, name, nameNormalized) VALUES (?1, ?2, ?3)",
                    params![n, name, name.to_lowercase()],
                )
                .unwrap();
                n
            });
            c.execute(
                "INSERT OR REPLACE INTO itemTags (itemID, tagID, type) VALUES (?1, ?2, ?3)",
                params![id, tid, tag.tag_type.unwrap_or(0) as i64],
            )
            .unwrap();
        }
        // Relations (dataObject.js:1240-1262).
        for (pred, objs) in &item.relations {
            let p = self.predicate_id(pred);
            for o in objs {
                c.execute(
                    "INSERT INTO itemRelations (itemID, predicateID, object) VALUES (?1, ?2, ?3)",
                    params![id, p, o],
                )
                .unwrap();
            }
        }
        id
    }
}
