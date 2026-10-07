// Part of the kovan Zotero port (GitHub #747, #750).
//
// Upstream: Zotero, https://github.com/zotero/zotero (commit 9cbba8c4d281):
//   xpcom/data/items.js (`_primaryDataSQLParts`/`_primaryDataSQLFrom` :37-97,
//   `_loadItemData` :259, `_loadCreators` :397, `_loadNotes` :470,
//   `_loadAnnotations` :561, `_loadAnnotationsDeferred` :640, `_loadTags`
//   :865, `_loadCollections` :922); xpcom/data/item.js (`getField` :237,
//   `_parseRowData` :344, `setField` :662, `filenameContainsPath` :2887,
//   `getFilePath` :2899, `attachmentFilename` :3601, `toJSON` :6008);
//   xpcom/data/itemFields.js (`isMultiline` :401); xpcom/data/notes.js
//   (`notePrefix`/`noteSuffix` :32-33); xpcom/data/tags.js (`cleanData`
//   :908); xpcom/annotations.js (type ids :31-36); xpcom/attachments.js (link
//   modes :35-41, `resolveRelativePath` :2899, `fixPathSlashes`,
//   `getStorageDirectoryByLibraryAndKey` :2842); Zotero utilities
//   (commit 4051881d59c6) utilities.js (`htmlSpecialChars` :668).
// Copyright (c) 2009 Center for History and New Media, George Mason
// University; (c) Corporation for Digital Scholarship, Vienna, Virginia, USA.
// Licence: AGPL-3.0 (upstream: AGPL-3.0-or-later).

//! Items of one library, loaded the way `Zotero.Items` loads them and
//! shaped the way `Zotero.Item#toJSON` writes them.

use super::containers::{cell_text, item_relations, Lookups};
use super::open::Caps;
use super::{AttachmentFile, ReadIssue, ReadReport, ZoteroDbError};
use kovan_common::zotero::date::{multipart_to_str, sql_to_iso8601};
use kovan_common::zotero::schema::{field_from_type_and_base, is_valid_for_type};
use kovan_common::zotero::{
    AnnotationData, AnnotationType, AttachmentData, Creator, CreatorName, CreatorType, Field,
    ItemType, LinkMode, Tag, ZoteroItem,
};
use rusqlite::Connection;
use serde_json::Value;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::PathBuf;

/// What the item loader needs to know about the library.
pub(super) struct LibCtx {
    pub library_id: i64,
    pub is_user: bool,
    pub storage_dir: PathBuf,
    pub base_attachment_path: Option<PathBuf>,
}

/// `Zotero.Notes.notePrefix` (notes.js:32).
const NOTE_PREFIX: &str = r#"<div class="zotero-note znv1">"#;
/// `Zotero.Notes.noteSuffix` (notes.js:33).
const NOTE_SUFFIX: &str = "</div>";

/// `Zotero.ItemFields.isMultiline` (itemFields.js:401).
fn is_multiline(field: &str) -> bool {
    matches!(field, "abstractNote" | "extra" | "address")
}

/// `Zotero.Utilities.htmlSpecialChars` (utilities.js:668).
fn html_special_chars(s: &str) -> String {
    let escaped = s
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
        .replace('<', "&lt;")
        .replace('>', "&gt;");
    // `/&lt;ZOTERO([^\/]+)\/&gt;/g` -> <br/>, &#8230; or the bare name.
    let mut out = String::with_capacity(escaped.len());
    let mut rest = escaped.as_str();
    while let Some(start) = rest.find("&lt;ZOTERO") {
        out.push_str(&rest[..start]);
        let after = &rest[start + "&lt;ZOTERO".len()..];
        let slash = after.find('/');
        match slash {
            Some(p) if p > 0 && after[p..].starts_with("/&gt;") => {
                let name = &after[..p];
                out.push_str(match name {
                    "BREAK" => "<br/>",
                    "HELLIP" => "&#8230;",
                    other => other,
                });
                rest = &after[p + "/&gt;".len()..];
            }
            _ => {
                out.push_str("&lt;ZOTERO");
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

/// The `zotero-note znvN` wrapper at the start of a stored note
/// (`/^<div class="zotero-note znv[0-9]+">/` on the first 36 characters).
fn wrapper_len(note: &str) -> Option<usize> {
    let head: String = note.chars().take(36).collect();
    let rest = head.strip_prefix(r#"<div class="zotero-note znv"#)?;
    let digits = rest.chars().take_while(char::is_ascii_digit).count();
    if digits == 0 || !rest[digits..].starts_with("\">") {
        return None;
    }
    Some(r#"<div class="zotero-note znv"#.len() + digits + 2)
}

/// `_loadNotes` (items.js:470-525): the stored note without its wrapper,
/// converting a plain-text note to HTML (upstream also writes the converted
/// note back to the database; this reader does not write).
fn note_from_db(stored: Option<String>) -> String {
    let Some(mut note) = stored else {
        return String::new();
    };
    if note.is_empty() {
        return note;
    }
    if wrapper_len(&note).is_none() {
        let mut body = html_special_chars(&note)
            .replace('\n', "</p><p>")
            .replace('\t', "&nbsp;&nbsp;&nbsp;&nbsp;")
            .replace("  ", "&nbsp;&nbsp;");
        body = format!("{NOTE_PREFIX}<p>{body}</p>{NOTE_SUFFIX}");
        // `/<p>\s*<\/p>/g` -> `<p>&nbsp;</p>`
        note = replace_empty_paragraphs(&body);
    }
    let start = wrapper_len(&note).unwrap_or(0);
    let chars: Vec<char> = note.chars().collect();
    let end = chars.len().saturating_sub(NOTE_SUFFIX.len()).max(start);
    // JS `substr(startLen, len - startLen - endLen)` counts UTF-16 units;
    // the wrapper and suffix are ASCII, so counting chars is equivalent.
    chars[start..end].iter().collect()
}

fn replace_empty_paragraphs(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find("<p>") {
        out.push_str(&rest[..i]);
        let after = &rest[i + 3..];
        let ws = after.len() - after.trim_start().len();
        if after[ws..].starts_with("</p>") {
            out.push_str("<p>&nbsp;</p>");
            rest = &after[ws + 4..];
        } else {
            out.push_str("<p>");
            rest = after;
        }
    }
    out.push_str(rest);
    out
}

/// `filenameContainsPath` (item.js:2887).
fn filename_contains_path(f: &str) -> bool {
    let b = f.as_bytes();
    f.contains('/')
        || (b.len() >= 3
            && b[0].is_ascii_alphabetic()
            && b[1] == b':'
            && (b[2] == b'\\' || b[2] == b'/'))
        || f.starts_with("\\\\")
}

/// `attachmentFilename` (item.js:3601).
fn attachment_filename(path: &str, stored: bool) -> String {
    if path.is_empty() {
        return String::new();
    }
    for prefix in ["attachments:", "storage:"] {
        if let Some(rest) = path.strip_prefix(prefix) {
            return rest.rsplit('/').next().unwrap_or("").to_owned();
        }
    }
    if stored {
        return path.rsplit(['/', '\\']).next().unwrap_or("").to_owned();
    }
    // PathUtils.filename: the last component of a native path.
    std::path::Path::new(path)
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// `getFilePath` (item.js:2899).
fn resolve_file(ctx: &LibCtx, key: &str, link_mode: LinkMode, path: &str) -> AttachmentFile {
    if link_mode == LinkMode::LinkedUrl {
        return AttachmentFile::NoFile;
    }
    if path.is_empty() {
        return AttachmentFile::Unresolvable("Attachment path is empty".into());
    }
    let stored = matches!(
        link_mode,
        LinkMode::ImportedFile | LinkMode::ImportedUrl | LinkMode::EmbeddedImage
    );
    if stored {
        if !path.contains("storage:") {
            return AttachmentFile::Unresolvable(format!("Invalid attachment path '{path}'"));
        }
        // `path.substr(8)` ("storage:".length), as upstream.
        let name: String = path.chars().skip(8).collect();
        if filename_contains_path(&name) {
            return AttachmentFile::Unresolvable(format!(
                "Invalid stored-file attachment filename '{name}'"
            ));
        }
        if name.starts_with(".zotero") {
            return AttachmentFile::Unresolvable(format!("Ignoring attachment file {name}"));
        }
        // getStorageDirectoryByLibraryAndKey: key must be /^[A-Z0-9]{8}$/.
        let key_ok = key.len() == 8
            && key
                .bytes()
                .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit());
        if !key_ok {
            return AttachmentFile::Unresolvable(format!("key '{key}' is not an 8-character key"));
        }
        return AttachmentFile::Stored(ctx.storage_dir.join(key).join(name));
    }
    if link_mode == LinkMode::LinkedFile {
        if let Some(rel) = path.strip_prefix("attachments:") {
            // resolveRelativePath + fixPathSlashes (non-Windows: '\' -> '/').
            let fixed = if cfg!(windows) {
                rel.replace('/', "\\")
            } else {
                rel.replace('\\', "/")
            };
            return AttachmentFile::LinkedRelative {
                relative: rel.to_owned(),
                resolved: ctx.base_attachment_path.as_ref().map(|b| b.join(fixed)),
            };
        }
    }
    if path.starts_with("AAAA") {
        return AttachmentFile::Unresolvable(
            "old-style Mac OS alias record; resolvable only by Zotero on a Mac".into(),
        );
    }
    AttachmentFile::Linked(PathBuf::from(path))
}

/// One `items` row with its child-table columns (`_primaryDataSQLParts`).
struct Primary {
    id: i64,
    type_name: String,
    date_added: Option<String>,
    date_modified: Option<String>,
    key: String,
    version: i64,
    deleted: bool,
    in_publications: bool,
    parent_attachment: Option<String>,
    parent_note: Option<String>,
    parent_annotation: Option<String>,
    link_mode: Option<i64>,
    content_type: Option<String>,
    charset: Option<String>,
    path: Option<String>,
    storage_mod_time: Option<i64>,
    storage_hash: Option<String>,
    last_read: Option<i64>,
}

fn load_primary(
    conn: &Connection,
    caps: &Caps,
    lookups: &Lookups,
    library_id: i64,
) -> Result<Vec<Primary>, ZoteroDbError> {
    let (ann_join, ann_parent) = if caps.annotations {
        (
            "LEFT JOIN itemAnnotations IAn ON (O.itemID=IAn.itemID) \
             LEFT JOIN items IAnP ON (IAn.parentItemID=IAnP.itemID) ",
            "IAnP.key",
        )
    } else {
        ("", "NULL")
    };
    let (pub_join, pub_col) = if caps.publications {
        (
            "LEFT JOIN publicationsItems PI ON (O.itemID=PI.itemID) ",
            "PI.itemID IS NOT NULL",
        )
    } else {
        ("", "0")
    };
    let last_read = if caps.last_read {
        "IA.lastRead"
    } else {
        "NULL"
    };
    let sql = format!(
        "SELECT O.itemID, O.itemTypeID, O.dateAdded, O.dateModified, O.key, O.version, \
         DI.itemID IS NOT NULL, {pub_col}, IAP.key, INoP.key, {ann_parent}, \
         IA.linkMode, IA.contentType, CS.charset, IA.path, IA.storageModTime, \
         IA.storageHash, {last_read} \
         FROM items O \
         LEFT JOIN itemAttachments IA USING (itemID) \
         LEFT JOIN items IAP ON (IA.parentItemID=IAP.itemID) \
         LEFT JOIN itemNotes INo ON (O.itemID=INo.itemID) \
         LEFT JOIN items INoP ON (INo.parentItemID=INoP.itemID) \
         {ann_join}\
         LEFT JOIN deletedItems DI ON (O.itemID=DI.itemID) \
         {pub_join}\
         LEFT JOIN charsets CS ON (IA.charsetID=CS.charsetID) \
         WHERE O.libraryID=?1 ORDER BY O.itemID"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([library_id], |r| {
        let type_id: i64 = r.get(1)?;
        Ok(Primary {
            id: r.get(0)?,
            type_name: lookups
                .item_types
                .get(&type_id)
                .cloned()
                .unwrap_or_else(|| format!("#{type_id}")),
            date_added: cell_text(r.get_ref(2)?),
            date_modified: cell_text(r.get_ref(3)?),
            key: r.get(4)?,
            version: r.get::<_, Option<i64>>(5)?.unwrap_or(0),
            deleted: r.get::<_, i64>(6)? != 0,
            in_publications: r.get::<_, i64>(7)? != 0,
            parent_attachment: r.get(8)?,
            parent_note: r.get(9)?,
            parent_annotation: r.get(10)?,
            link_mode: r.get(11)?,
            content_type: cell_text(r.get_ref(12)?),
            charset: r.get(13)?,
            path: cell_text(r.get_ref(14)?),
            storage_mod_time: r.get(15)?,
            storage_hash: cell_text(r.get_ref(16)?),
            last_read: r.get(17)?,
        })
    })?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

/// `setField(field, value, loadIn=true)` (item.js:662-860) for one stored
/// value, then `getField(field)` (unformatted = false, item.js:237) and
/// `toJSON`'s `accessDate` conversion (item.js:6134). Returns the JSON field
/// name and value, or why the value is ignored.
fn field_value(
    item_type: ItemType,
    field_name: &str,
    raw: Option<String>,
) -> Result<Option<(String, String)>, String> {
    // "Normalize values": numbers become strings (cell_text), strings are
    // trimmed (NFC normalisation not applied; see the module docs).
    let Some(raw) = raw else { return Ok(None) };
    let mut value = raw.trim().to_owned();
    if value.is_empty() {
        return Ok(None);
    }
    let Some(field) = Field::from_name(field_name) else {
        return Err(format!("'{field_name}' is not a field this port knows"));
    };
    // "Make sure to use type-specific field ID if available".
    let field = field_from_type_and_base(item_type, field).unwrap_or(field);
    if !is_valid_for_type(field, item_type) {
        return Err(format!(
            "'{}' is not a valid field for type '{}' -- ignoring value '{value}'",
            field.as_str(),
            item_type.as_str()
        ));
    }
    if !is_multiline(field.as_str()) && (value.contains('\n') || value.contains('\r')) {
        // `value.replace(/[\r\n]+/g, " ")`
        let mut out = String::with_capacity(value.len());
        let mut in_run = false;
        for c in value.chars() {
            if c == '\r' || c == '\n' {
                if !in_run {
                    out.push(' ');
                }
                in_run = true;
            } else {
                out.push(c);
                in_run = false;
            }
        }
        value = out;
    }
    if field.is_date() {
        value = multipart_to_str(&value);
    }
    if field == Field::AccessDate {
        if let Some(iso) = sql_to_iso8601(&value) {
            value = iso;
        }
    }
    Ok(Some((field.as_str().to_owned(), value)))
}

fn issue(ctx: &LibCtx, what: String, reason: String) -> ReadIssue {
    ReadIssue {
        library_id: ctx.library_id,
        what,
        reason,
    }
}

/// Every item of one library, in `itemID` order, with each attachment's
/// file. See the parent module docs for the table-by-table mapping.
pub(super) fn load_items(
    conn: &Connection,
    caps: &Caps,
    lookups: &Lookups,
    ctx: &LibCtx,
    report: &mut ReadReport,
) -> Result<(Vec<ZoteroItem>, BTreeMap<String, AttachmentFile>), ZoteroDbError> {
    let lib = ctx.library_id;
    let mut items: Vec<ZoteroItem> = Vec::new();
    let mut index: HashMap<i64, usize> = HashMap::new();
    let mut files = BTreeMap::new();
    // Items upstream would fail to load (an unknown annotation type),
    // dropped at the end so the id -> index map stays valid.
    let mut dead: HashSet<usize> = HashSet::new();

    for p in load_primary(conn, caps, lookups, lib)? {
        let Some(item_type) = ItemType::from_name(&p.type_name) else {
            report.skipped.push(issue(
                ctx,
                format!("item {}", p.key),
                format!("unknown item type '{}'", p.type_name),
            ));
            continue;
        };
        let mut item = ZoteroItem::new(item_type);
        item.key = Some(p.key.clone());
        item.version = Some(p.version.max(0) as u64);
        item.date_added = p.date_added.as_deref().and_then(sql_to_iso8601);
        item.date_modified = p.date_modified.as_deref().and_then(sql_to_iso8601);
        item.deleted = p.deleted.then_some(true);
        item.in_publications = p.in_publications.then_some(true);
        item.parent_item = match item_type {
            ItemType::Attachment => p.parent_attachment.clone(),
            ItemType::Note => p.parent_note.clone(),
            ItemType::Annotation => p.parent_annotation.clone(),
            _ => None,
        };
        if item_type == ItemType::Attachment {
            // `_parseRowData`: a NULL link mode "shouldn't happen" and is
            // read as LINK_MODE_IMPORTED_URL; '.zotero*' paths are cleared.
            let mode = p
                .link_mode
                .map(|m| {
                    usize::try_from(m)
                        .ok()
                        .and_then(|i| LinkMode::ALL.get(i).copied())
                })
                .unwrap_or(Some(LinkMode::ImportedUrl));
            let Some(link_mode) = mode else {
                report.skipped.push(issue(
                    ctx,
                    format!("item {}", p.key),
                    format!("unknown attachment link mode {:?}", p.link_mode),
                ));
                continue;
            };
            let path = p
                .path
                .clone()
                .filter(|s| !s.starts_with(".zotero"))
                .unwrap_or_default();
            let stored = matches!(
                link_mode,
                LinkMode::ImportedFile | LinkMode::ImportedUrl | LinkMode::EmbeddedImage
            );
            let mut a = AttachmentData {
                link_mode: Some(link_mode),
                content_type: Some(p.content_type.clone().unwrap_or_default()),
                ..AttachmentData::default()
            };
            if link_mode != LinkMode::EmbeddedImage {
                a.charset = Some(p.charset.clone().unwrap_or_default());
            }
            match link_mode {
                LinkMode::LinkedFile => a.path = Some(path.clone()),
                LinkMode::LinkedUrl => {}
                _ => a.filename = Some(attachment_filename(&path, stored)),
            }
            if ctx.is_user {
                a.last_read = p.last_read.filter(|&t| t != 0);
            }
            if stored {
                a.mtime = p.storage_mod_time;
                a.md5 = p.storage_hash.clone();
            }
            item.attachment = Some(a);
            files.insert(p.key.clone(), resolve_file(ctx, &p.key, link_mode, &path));
        }
        index.insert(p.id, items.len());
        items.push(item);
    }

    // itemData (`_loadItemData`; notes have no item data).
    {
        let mut stmt = conn.prepare(
            "SELECT I.itemID, D.fieldID, V.value FROM items I \
             JOIN itemData D USING (itemID) JOIN itemDataValues V USING (valueID) \
             WHERE I.libraryID=?1 ORDER BY I.itemID, D.fieldID",
        )?;
        let rows = stmt.query_map([lib], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, i64>(1)?,
                cell_text(r.get_ref(2)?),
            ))
        })?;
        for row in rows {
            let (id, field_id, raw) = row?;
            let Some(&i) = index.get(&id) else { continue };
            let item = &mut items[i];
            if item.item_type == ItemType::Note {
                continue;
            }
            let key = item.key.clone().unwrap_or_default();
            let Some(name) = lookups.fields.get(&field_id) else {
                report.dropped.push(issue(
                    ctx,
                    format!("item {key}"),
                    format!("field id {field_id} not in the database's field table"),
                ));
                continue;
            };
            match field_value(item.item_type, name, raw) {
                Ok(Some((f, v))) => {
                    item.fields.insert(f, v);
                }
                Ok(None) => {}
                Err(reason) => {
                    report
                        .dropped
                        .push(issue(ctx, format!("item {key} field {name}"), reason))
                }
            }
        }
    }

    // Creators (`_loadCreators`), regular items only in `toJSON`.
    {
        let mut stmt = conn.prepare(
            "SELECT IC.itemID, C.firstName, C.lastName, C.fieldMode, IC.creatorTypeID \
             FROM items I JOIN itemCreators IC USING (itemID) JOIN creators C USING (creatorID) \
             WHERE I.libraryID=?1 ORDER BY IC.itemID, IC.orderIndex",
        )?;
        let rows = stmt.query_map([lib], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                cell_text(r.get_ref(1)?).unwrap_or_default(),
                cell_text(r.get_ref(2)?).unwrap_or_default(),
                r.get::<_, Option<i64>>(3)?.unwrap_or(0),
                r.get::<_, i64>(4)?,
            ))
        })?;
        for row in rows {
            let (id, first, last, field_mode, type_id) = row?;
            let Some(&i) = index.get(&id) else { continue };
            let item = &mut items[i];
            if !item.item_type.is_regular() {
                continue;
            }
            let type_name = lookups
                .creator_types
                .get(&type_id)
                .cloned()
                .unwrap_or_default();
            let Some(creator_type) = CreatorType::from_name(&type_name) else {
                report.dropped.push(issue(
                    ctx,
                    format!("item {}", item.key.clone().unwrap_or_default()),
                    format!("unknown creator type '{type_name}' (id {type_id})"),
                ));
                continue;
            };
            let name = if field_mode == 1 {
                CreatorName::SingleField { name: last }
            } else {
                CreatorName::TwoField {
                    first_name: first,
                    last_name: last,
                }
            };
            item.creators.push(Creator { creator_type, name });
        }
    }

    // Notes (`_loadNotes`): notes always carry `note`; attachments only
    // when it is non-empty (`toJSON`, item.js:6081).
    {
        let mut stmt = conn.prepare(
            "SELECT N.itemID, N.note FROM items I JOIN itemNotes N USING (itemID) \
             WHERE I.libraryID=?1",
        )?;
        let rows = stmt.query_map([lib], |r| {
            Ok((r.get::<_, i64>(0)?, cell_text(r.get_ref(1)?)))
        })?;
        for row in rows {
            let (id, raw) = row?;
            let Some(&i) = index.get(&id) else { continue };
            let note = note_from_db(raw);
            let item = &mut items[i];
            match item.item_type {
                ItemType::Note => item.note = Some(note),
                ItemType::Attachment if !note.is_empty() => item.note = Some(note),
                _ => {}
            }
        }
        for item in &mut items {
            if item.item_type == ItemType::Note && item.note.is_none() {
                item.note = Some(String::new());
            }
        }
    }

    // Annotations (`_loadAnnotations` + `_loadAnnotationsDeferred`).
    if caps.annotations {
        let author = if caps.annotation_author {
            "A.authorName"
        } else {
            "NULL"
        };
        let sql = format!(
            "SELECT A.itemID, A.type, {author}, A.text, A.comment, A.color, A.pageLabel, \
             A.sortIndex, A.position, A.isExternal \
             FROM items I JOIN itemAnnotations A USING (itemID) WHERE I.libraryID=?1"
        );
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map([lib], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, i64>(1)?,
                cell_text(r.get_ref(2)?),
                cell_text(r.get_ref(3)?),
                cell_text(r.get_ref(4)?),
                cell_text(r.get_ref(5)?),
                cell_text(r.get_ref(6)?),
                cell_text(r.get_ref(7)?),
                cell_text(r.get_ref(8)?),
                r.get::<_, Option<i64>>(9)?.unwrap_or(0) != 0,
            ))
        })?;
        for row in rows {
            let (
                id,
                type_id,
                author,
                text,
                comment,
                color,
                page_label,
                sort_index,
                position,
                external,
            ) = row?;
            let Some(&i) = index.get(&id) else { continue };
            // annotations.js:31-36: ids 1..6 in AnnotationType::ALL order.
            let Some(t) = usize::try_from(type_id - 1)
                .ok()
                .and_then(|k| AnnotationType::ALL.get(k).copied())
            else {
                dead.insert(i);
                report.skipped.push(issue(
                    ctx,
                    format!("item {}", items[i].key.clone().unwrap_or_default()),
                    format!("Unknown annotation type id {type_id}"),
                ));
                continue;
            };
            let s = |v: Option<String>| Some(v.unwrap_or_default());
            let item = &mut items[i];
            item.annotation = Some(AnnotationData {
                annotation_type: Some(t),
                author_name: s(author),
                // toJSON writes text for highlights and underlines only.
                text: matches!(t, AnnotationType::Highlight | AnnotationType::Underline)
                    .then(|| text.unwrap_or_default()),
                comment: s(comment),
                color: s(color),
                page_label: s(page_label),
                sort_index: s(sort_index),
                position: s(position),
            });
            if external {
                item.other
                    .insert("annotationIsExternal".into(), Value::Bool(true));
            }
        }
    }

    // Tags (`_loadTags` + `Zotero.Tags.cleanData`): the PRIMARY KEY
    // (itemID, tagID) is the order the upstream join visits rows in.
    {
        let mut stmt = conn.prepare(
            "SELECT IT.itemID, T.name, IT.type FROM items I \
             JOIN itemTags IT USING (itemID) JOIN tags T USING (tagID) \
             WHERE I.libraryID=?1 ORDER BY IT.itemID, IT.tagID",
        )?;
        let rows = stmt.query_map([lib], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                cell_text(r.get_ref(1)?).unwrap_or_default(),
                r.get::<_, Option<i64>>(2)?.unwrap_or(0),
            ))
        })?;
        for row in rows {
            let (id, name, ty) = row?;
            let Some(&i) = index.get(&id) else { continue };
            let item = &mut items[i];
            if item.attachment.as_ref().and_then(|a| a.link_mode) == Some(LinkMode::EmbeddedImage) {
                continue; // toJSON writes no tags for embedded images
            }
            let tag_type = match ty {
                0 => None,
                1 => Some(1),
                other => {
                    report.dropped.push(issue(
                        ctx,
                        format!("item {} tag {name}", item.key.clone().unwrap_or_default()),
                        format!("Tag 'type' must be 0 or 1 (found {other}); kept as manual"),
                    ));
                    None
                }
            };
            item.tags.push(Tag {
                tag: name.trim().to_owned(),
                tag_type,
            });
        }
    }

    // Collections (`_loadCollections`), top-level items only in `toJSON`.
    {
        let mut stmt = conn.prepare(
            "SELECT CI.itemID, C.key FROM items I JOIN collectionItems CI USING (itemID) \
             JOIN collections C USING (collectionID) \
             WHERE I.libraryID=?1 ORDER BY CI.itemID, CI.rowid",
        )?;
        let rows = stmt.query_map([lib], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))?;
        for row in rows {
            let (id, ckey) = row?;
            let Some(&i) = index.get(&id) else { continue };
            let item = &mut items[i];
            if item.parent_item.is_none() {
                item.collections.push(ckey);
            }
        }
    }

    // Relations (`_loadRelations`).
    let mut rels = item_relations(conn, lib)?;
    for (id, &i) in &index {
        if let Some(r) = rels.remove(id) {
            let item = &mut items[i];
            if item.attachment.as_ref().and_then(|a| a.link_mode) != Some(LinkMode::EmbeddedImage) {
                item.relations = r;
            }
        }
    }

    let items = items
        .into_iter()
        .enumerate()
        .filter(|(i, _)| !dead.contains(i))
        .map(|(_, it)| it)
        .collect();
    Ok((items, files))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `_loadNotes` on a wrapped note, a plain-text note (converted as
    /// items.js:497-503 does) and an empty one.
    #[test]
    fn notes_are_unwrapped_and_plain_text_converted() {
        assert_eq!(
            note_from_db(Some(
                r#"<div class="zotero-note znv1"><p>Hi</p></div>"#.into()
            )),
            "<p>Hi</p>"
        );
        assert_eq!(
            note_from_db(Some("a < b\n\nc  d".into())),
            "<p>a &lt; b</p><p>&nbsp;</p><p>c&nbsp;&nbsp;d</p>"
        );
        assert_eq!(note_from_db(None), "");
        assert_eq!(note_from_db(Some(String::new())), "");
    }

    /// `htmlSpecialChars`' `<ZOTERO.../>` entities (utilities.js:682-692).
    #[test]
    fn html_special_chars_zotero_entities() {
        assert_eq!(
            html_special_chars("a<ZOTEROBREAK/>b<ZOTEROHELLIP/>c<ZOTEROX/>\"'&"),
            "a<br/>b&#8230;cX&quot;&apos;&amp;"
        );
    }

    /// `attachmentFilename` (item.js:3601) on each path form.
    #[test]
    fn attachment_filenames() {
        assert_eq!(attachment_filename("storage:paper.pdf", true), "paper.pdf");
        assert_eq!(attachment_filename("attachments:a/b/c.pdf", false), "c.pdf");
        assert_eq!(attachment_filename("../x\\y.pdf", true), "y.pdf");
        assert_eq!(attachment_filename("/home/u/z.pdf", false), "z.pdf");
        assert_eq!(attachment_filename("", true), "");
    }

    /// `filenameContainsPath` (item.js:2887).
    #[test]
    fn filename_path_detection() {
        assert!(filename_contains_path("a/b.pdf"));
        assert!(filename_contains_path("C:\\b.pdf"));
        assert!(filename_contains_path("\\\\server\\b.pdf"));
        assert!(!filename_contains_path("b.pdf"));
    }
}
