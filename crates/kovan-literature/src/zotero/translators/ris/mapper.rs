// Part of the kovan Zotero port (GitHub #747, #749).
//
// Upstream: Zotero translators, https://github.com/zotero/translators
//   (commit 3d1c78530f42): RIS.js `TagMapper` :519-645 (`isDeprecated`
//   :526, `getField` :538, `reverseLookup` :598).
// Copyright (C) 2006-2023 Simon Kornblith, Aurimas Vinckevicus, Abe Jellinek.
// Licence: AGPL-3.0 (upstream: AGPL-3.0-or-later).

//! RIS tag <-> Zotero field mapping.

use super::tables::{Sel, TagMap};

/// A list of tag maps (upstream's `fieldMap`-shaped objects).
pub type MapTable = &'static [(&'static str, TagMap)];

/// `TagMapper` (:519): maps tried in order; `deprecated` is the map whose
/// tags are only used when nothing else maps to their field.
///
/// Upstream caches results per item type and tag; the lookups are pure, so
/// this port recomputes them.
#[derive(Debug, Clone)]
pub struct TagMapper {
    maps: Vec<MapTable>,
    deprecated: Option<MapTable>,
    /// The `RIS.import.keepID` hidden preference sets
    /// `degenerateImportFieldMap.ID = true` (:1837-1839): a value that is
    /// neither a string nor an object, so `getField` finds no field for `ID`
    /// in that map (the tag then counts as unknown), while `isDeprecated`
    /// still reports it.
    keep_id: bool,
}

/// The entry of `map` for `tag`, honouring `keep_id` for the degenerate map.
fn lookup(map: MapTable, tag: &str) -> Option<&'static TagMap> {
    map.iter().find(|(t, _)| *t == tag).map(|(_, m)| m)
}

impl TagMapper {
    /// `new TagMapper(mapList, deprecatedMap)`.
    pub fn new(maps: Vec<MapTable>, deprecated: Option<MapTable>, keep_id: bool) -> Self {
        TagMapper {
            maps,
            deprecated,
            keep_id,
        }
    }

    fn entry(&self, map: MapTable, tag: &str) -> Option<&'static TagMap> {
        if self.keep_id && tag == "ID" && self.deprecated.is_some_and(|d| std::ptr::eq(d, map)) {
            return None;
        }
        lookup(map, tag)
    }

    /// `isDeprecated(tag)` (:526): the deprecated map has the tag.
    pub fn is_deprecated(&self, tag: &str) -> bool {
        self.deprecated.is_some_and(|d| lookup(d, tag).is_some())
    }

    /// `getField(itemType, tag)` (:538-587).
    ///
    /// Ported with upstream's `var def` quirk: `def` is declared with `var`
    /// inside the loop without an initialiser, so a `__default` seen in one
    /// map survives into the next map's check (`exclude` is reset).
    pub fn get_field(&self, item_type: &str, tag: &str) -> Option<&'static str> {
        let mut field: Option<&'static str> = None;
        let mut def: Option<&'static str> = None;
        for map in &self.maps {
            match self.entry(map, tag) {
                Some(TagMap::ByType(entries)) => {
                    let mut exclude = false;
                    for (f, sel) in entries.iter() {
                        if *f == "__default" {
                            if let Sel::Field(d) = sel {
                                def = Some(d);
                            }
                            continue;
                        }
                        if *f == "__exclude" {
                            if let Sel::Types(types) = sel {
                                if types.contains(&item_type) {
                                    exclude = true;
                                }
                            }
                            continue;
                        }
                        let hit = match sel {
                            Sel::Types(types) => types.contains(&item_type),
                            // String.prototype.includes: a substring test.
                            Sel::Field(s) => s.contains(item_type),
                        };
                        if hit {
                            field = Some(f);
                            break;
                        }
                    }
                    if field.is_none() && !exclude {
                        field = def;
                    }
                }
                Some(TagMap::Field(f)) => field = Some(f),
                None => {}
            }
            if field.is_some() {
                break;
            }
        }
        // An empty string is falsy, as `false` would be.
        field.filter(|f| !f.is_empty())
    }

    /// `reverseLookup(itemType, zField)` (:598-645): the first RIS tag that
    /// maps to `z_field` for `item_type`.
    pub fn reverse_lookup(&self, item_type: &str, z_field: &str) -> Option<&'static str> {
        for map in &self.maps {
            for (ris_tag, type_map) in map.iter() {
                if self.keep_id
                    && *ris_tag == "ID"
                    && self.deprecated.is_some_and(|d| std::ptr::eq(d, *map))
                {
                    // `true == zField` is false and `typeof true` is not "object".
                    continue;
                }
                match type_map {
                    TagMap::Field(f) => {
                        if *f == z_field {
                            return Some(ris_tag);
                        }
                    }
                    TagMap::ByType(entries) => {
                        let get = |k: &str| entries.iter().find(|(f, _)| *f == k).map(|(_, s)| s);
                        let includes = |s: &Sel| match s {
                            Sel::Types(t) => t.contains(&item_type),
                            Sel::Field(f) => f.contains(item_type),
                        };
                        if let Some(sel) = get(z_field) {
                            if includes(sel) {
                                return Some(ris_tag);
                            }
                        }
                        let excluded = get("__exclude").is_some_and(includes);
                        let default_is =
                            matches!(get("__default"), Some(Sel::Field(d)) if *d == z_field);
                        if !excluded && default_is {
                            let prevent = entries.iter().any(|(_, s)| includes(s));
                            if !prevent {
                                return Some(ris_tag);
                            }
                        }
                    }
                }
            }
        }
        None
    }
}
