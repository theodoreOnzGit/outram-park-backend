// Part of the kovan Zotero port (GitHub #747, #751).
//
// Upstream: Zotero, https://github.com/zotero/zotero (commit 9cbba8c4d281):
// chrome/content/zotero/xpcom/data/searchConditions.js (the condition and
// operator tables, hasOperator, parseSearchString, parseCondition) and
// search.js:2323 (_conditionLevel); Zotero utilities
// (https://github.com/zotero/utilities, commit 4051881d59c6) utilities.js
// (allowedKeyChars, isValidObjectKey).
// Copyright (c) Corporation for Digital Scholarship, Vienna, Virginia, USA
// (searchConditions.js: (c) 2006-2016 Center for History and New Media,
// George Mason University). Licence: AGPL-3.0. See this crate's NOTICE,
// "Upstream: Zotero".

//! The search conditions and operators (`Zotero.SearchConditions`).

use std::sync::OnceLock;

use kovan_common::zotero::schema::is_valid_for_type;
use kovan_common::zotero::{Field, ItemType};
use regex::Regex;

/// A search operator (searchConditions.js:43-63), plus the four
/// `resultLevel` "operators" (the level rides in the operator,
/// searchConditions.js:259).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Operator {
    /// `is`
    Is,
    /// `isNot`
    IsNot,
    /// `beginsWith`
    BeginsWith,
    /// `contains`
    Contains,
    /// `doesNotContain`
    DoesNotContain,
    /// `isLessThan`
    IsLessThan,
    /// `isGreaterThan`
    IsGreaterThan,
    /// `isBefore`
    IsBefore,
    /// `isAfter`
    IsAfter,
    /// `isInTheLast`
    IsInTheLast,
    /// `isEmpty`
    IsEmpty,
    /// `isNotEmpty`
    IsNotEmpty,
    /// `any` (joinMode)
    Any,
    /// `all` (joinMode)
    All,
    /// `true`
    True,
    /// `false`
    False,
    /// `item` (resultLevel)
    Item,
    /// `attachment` (resultLevel)
    Attachment,
    /// `note` (resultLevel)
    Note,
    /// `annotation` (resultLevel)
    Annotation,
}

impl Operator {
    /// Every operator.
    pub const ALL: [Operator; 20] = [
        Operator::Is,
        Operator::IsNot,
        Operator::BeginsWith,
        Operator::Contains,
        Operator::DoesNotContain,
        Operator::IsLessThan,
        Operator::IsGreaterThan,
        Operator::IsBefore,
        Operator::IsAfter,
        Operator::IsInTheLast,
        Operator::IsEmpty,
        Operator::IsNotEmpty,
        Operator::Any,
        Operator::All,
        Operator::True,
        Operator::False,
        Operator::Item,
        Operator::Attachment,
        Operator::Note,
        Operator::Annotation,
    ];

    /// Zotero's name.
    pub const fn as_str(self) -> &'static str {
        match self {
            Operator::Is => "is",
            Operator::IsNot => "isNot",
            Operator::BeginsWith => "beginsWith",
            Operator::Contains => "contains",
            Operator::DoesNotContain => "doesNotContain",
            Operator::IsLessThan => "isLessThan",
            Operator::IsGreaterThan => "isGreaterThan",
            Operator::IsBefore => "isBefore",
            Operator::IsAfter => "isAfter",
            Operator::IsInTheLast => "isInTheLast",
            Operator::IsEmpty => "isEmpty",
            Operator::IsNotEmpty => "isNotEmpty",
            Operator::Any => "any",
            Operator::All => "all",
            Operator::True => "true",
            Operator::False => "false",
            Operator::Item => "item",
            Operator::Attachment => "attachment",
            Operator::Note => "note",
            Operator::Annotation => "annotation",
        }
    }

    /// Parse Zotero's name.
    pub fn from_name(s: &str) -> Option<Operator> {
        Operator::ALL.into_iter().find(|o| o.as_str() == s)
    }

    /// `isNot`, `doesNotContain` or `isEmpty`: the SQL becomes
    /// `itemID NOT IN (...)` (search.js:1401).
    pub fn is_negation(self) -> bool {
        matches!(
            self,
            Operator::IsNot | Operator::DoesNotContain | Operator::IsEmpty
        )
    }
}

/// An item level of the cross-level search (search.js:2234-2249).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Level {
    /// A top-level (regular) item.
    Item,
    /// An attachment.
    Attachment,
    /// A note.
    Note,
    /// An annotation.
    Annotation,
    /// Level-agnostic (tags): matches at any level.
    Any,
}

impl Level {
    /// `_levelParent` (search.js:2234).
    pub fn parent(self) -> Option<Level> {
        match self {
            Level::Annotation => Some(Level::Attachment),
            Level::Attachment | Level::Note => Some(Level::Item),
            Level::Item | Level::Any => None,
        }
    }

    /// `_levelCanBeStandalone` (search.js:2245).
    pub fn can_be_standalone(self) -> bool {
        matches!(self, Level::Attachment | Level::Note)
    }

    /// The level of a `resultLevel` operator.
    pub fn from_operator(op: Operator) -> Option<Level> {
        match op {
            Operator::Item => Some(Level::Item),
            Operator::Attachment => Some(Level::Attachment),
            Operator::Note => Some(Level::Note),
            Operator::Annotation => Some(Level::Annotation),
            _ => None,
        }
    }
}

/// A resolved condition name (searchConditions.js:75-813, indexed by name
/// and alias as `_conditions` is).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Deleted,
    IncludeDeleted,
    NoChildren,
    Unfiled,
    Retracted,
    Publications,
    Feed,
    IncludeParentsAndChildren,
    IncludeParents,
    IncludeChildren,
    Recursive,
    JoinMode,
    QuickTitleCreatorYear,
    QuickTitleCreatorYearNote,
    QuickFields,
    QuickEverything,
    QuickDeprecated,
    GroupStart,
    GroupEnd,
    ResultLevel,
    CollectionId,
    SavedSearchId,
    Collection,
    SavedSearch,
    DateAdded,
    DateModified,
    LastRead,
    ItemTypeId,
    ItemType,
    FileTypeId,
    AttachmentStorageType,
    TagId,
    Tag,
    NumTags,
    NumNotes,
    NumAttachments,
    NumAnnotations,
    Note,
    Creator,
    LastName,
    Author,
    Editor,
    BookAuthor,
    /// `field`; `Some` for an alias (a field name).
    Field(Option<Field>),
    AnyField,
    TitleCreatorYear,
    /// `datefield`; `Some` for an alias.
    DateField(Option<Field>),
    Year,
    /// `numberfield`; `Some` for an alias.
    NumberField(Option<Field>),
    LibraryId,
    Key,
    ItemId,
    AnnotationText,
    AnnotationComment,
    AnnotationType,
    AnnotationColor,
    AnnotationAuthor,
    FulltextContent,
    TempTable,
}

/// `numberfield`'s aliases (searchConditions.js:690).
const NUMBER_FIELDS: [&str; 6] = [
    "pages",
    "numPages",
    "numberOfVolumes",
    "section",
    "seriesNumber",
    "issue",
];

/// Resolve a condition name (without `/mode`), or `None` for an unknown
/// one (upstream throws "Invalid condition").
pub fn resolve(name: &str) -> Option<Kind> {
    use Kind::*;
    Some(match name {
        "deleted" => Deleted,
        "includeDeleted" => IncludeDeleted,
        "noChildren" => NoChildren,
        "unfiled" => Unfiled,
        "retracted" => Retracted,
        "publications" => Publications,
        "feed" => Feed,
        "includeParentsAndChildren" => IncludeParentsAndChildren,
        "includeParents" => IncludeParents,
        "includeChildren" => IncludeChildren,
        "recursive" => Recursive,
        "joinMode" => JoinMode,
        "quicksearch-titleCreatorYear" => QuickTitleCreatorYear,
        "quicksearch-titleCreatorYearNote" => QuickTitleCreatorYearNote,
        "quicksearch-fields" => QuickFields,
        "quicksearch-everything" => QuickEverything,
        "quicksearch" => QuickDeprecated,
        "groupStart" => GroupStart,
        "groupEnd" => GroupEnd,
        "resultLevel" => ResultLevel,
        "collectionID" => CollectionId,
        "savedSearchID" => SavedSearchId,
        "collection" => Collection,
        "savedSearch" => SavedSearch,
        "dateAdded" => DateAdded,
        "dateModified" => DateModified,
        "lastRead" => LastRead,
        "itemTypeID" => ItemTypeId,
        "itemType" => ItemType,
        "fileTypeID" => FileTypeId,
        "attachmentStorageType" => AttachmentStorageType,
        "tagID" => TagId,
        "tag" => Tag,
        "numTags" => NumTags,
        "numNotes" => NumNotes,
        "numAttachments" => NumAttachments,
        "numAnnotations" => NumAnnotations,
        "note" => Note,
        "creator" => Creator,
        "lastName" => LastName,
        "author" => Author,
        "editor" => Editor,
        "bookAuthor" => BookAuthor,
        "field" => Field(None),
        "anyField" => AnyField,
        "titleCreatorYear" => TitleCreatorYear,
        "datefield" => DateField(None),
        "year" => Year,
        "numberfield" => NumberField(None),
        "libraryID" => LibraryId,
        "key" => Key,
        "itemID" => ItemId,
        "annotationText" => AnnotationText,
        "annotationComment" => AnnotationComment,
        "annotationType" => AnnotationType,
        "annotationColor" => AnnotationColor,
        "annotationAuthor" => AnnotationAuthor,
        "fulltextContent" => FulltextContent,
        "tempTable" => TempTable,
        other => {
            let f = kovan_common::zotero::Field::from_name(other)?;
            // TEMP - NSF (searchConditions.js:821-828): dateDue/accepted
            // only when the nsfReviewer type exists (it does not in schema 45).
            if matches!(other, "dateDue" | "accepted")
                && kovan_common::zotero::ItemType::from_name("nsfReviewer").is_none()
            {
                return None;
            }
            if f.is_date() || f == kovan_common::zotero::Field::AccessDate {
                DateField(Some(f))
            } else if NUMBER_FIELDS.contains(&other) {
                NumberField(Some(f))
            } else {
                Field(Some(f))
            }
        }
    })
}

impl Kind {
    /// The operators the condition accepts (searchConditions.js).
    pub fn operators(self) -> &'static [Operator] {
        use Kind::*;
        use Operator as O;
        const BOOL: &[Operator] = &[O::True, O::False];
        const IS: &[Operator] = &[O::Is, O::IsNot];
        const QUICK: &[Operator] = &[O::Is, O::IsNot, O::Contains, O::DoesNotContain];
        const DATE: &[Operator] = &[O::Is, O::IsNot, O::IsBefore, O::IsAfter, O::IsInTheLast];
        const COUNT: &[Operator] = &[O::Is, O::IsNot, O::IsLessThan, O::IsGreaterThan];
        const CONTAINS: &[Operator] = &[O::Contains, O::DoesNotContain];
        const CREATOR: &[Operator] = &[
            O::Is,
            O::IsNot,
            O::Contains,
            O::DoesNotContain,
            O::IsEmpty,
            O::IsNotEmpty,
        ];
        const FIELD: &[Operator] = &[
            O::Is,
            O::IsNot,
            O::Contains,
            O::DoesNotContain,
            O::BeginsWith,
            O::IsEmpty,
            O::IsNotEmpty,
        ];
        const ANYFIELD: &[Operator] = &[
            O::Is,
            O::IsNot,
            O::Contains,
            O::DoesNotContain,
            O::BeginsWith,
        ];
        const DATEFIELD: &[Operator] = &[
            O::Is,
            O::IsNot,
            O::IsBefore,
            O::IsAfter,
            O::IsInTheLast,
            O::IsEmpty,
            O::IsNotEmpty,
        ];
        const NUMBER: &[Operator] = &[
            O::Is,
            O::IsNot,
            O::Contains,
            O::DoesNotContain,
            O::IsLessThan,
            O::IsGreaterThan,
            O::IsEmpty,
            O::IsNotEmpty,
        ];
        match self {
            Deleted
            | IncludeDeleted
            | NoChildren
            | Unfiled
            | Retracted
            | Publications
            | Feed
            | IncludeParentsAndChildren
            | IncludeParents
            | IncludeChildren
            | Recursive => BOOL,
            JoinMode => &[O::Any, O::All],
            QuickTitleCreatorYear
            | QuickTitleCreatorYearNote
            | QuickFields
            | QuickEverything
            | QuickDeprecated
            | Tag
            | LastName
            | Year => QUICK,
            GroupStart | GroupEnd => &[O::True],
            ResultLevel => &[O::Item, O::Attachment, O::Note, O::Annotation],
            CollectionId
            | SavedSearchId
            | Collection
            | SavedSearch
            | ItemTypeId
            | ItemType
            | FileTypeId
            | AttachmentStorageType
            | TagId
            | LibraryId
            | ItemId
            | AnnotationType
            | AnnotationColor
            | AnnotationAuthor => IS,
            DateAdded | DateModified | LastRead => DATE,
            NumTags | NumNotes | NumAttachments | NumAnnotations => COUNT,
            Note | AnnotationText | AnnotationComment | FulltextContent => CONTAINS,
            Creator | Author | Editor | BookAuthor => CREATOR,
            Field(_) => FIELD,
            AnyField | TitleCreatorYear => ANYFIELD,
            DateField(_) => DATEFIELD,
            NumberField(_) => NUMBER,
            Key => &[O::Is, O::IsNot, O::BeginsWith],
            TempTable => &[O::Is],
        }
    }

    /// `hasOperator` (searchConditions.js:901), including the text
    /// operators date fields accept for older saved searches (line 921).
    pub fn has_operator(self, op: Operator) -> bool {
        if matches!(self, Kind::DateField(_))
            && matches!(
                op,
                Operator::Contains | Operator::DoesNotContain | Operator::BeginsWith
            )
        {
            return true;
        }
        self.operators().contains(&op)
    }

    /// Whether the definition has a `table` (search.js:1074 builds SQL for
    /// these, and for `savedSearch`/`tempTable`).
    pub fn has_table(self) -> bool {
        use Kind::*;
        matches!(
            self,
            Collection
                | DateAdded
                | DateModified
                | LastRead
                | ItemTypeId
                | ItemType
                | FileTypeId
                | AttachmentStorageType
                | TagId
                | Tag
                | NumTags
                | NumNotes
                | NumAttachments
                | NumAnnotations
                | Note
                | Creator
                | LastName
                | Author
                | Editor
                | BookAuthor
                | Field(_)
                | DateField(_)
                | Year
                | NumberField(_)
                | LibraryId
                | Key
                | ItemId
                | AnnotationText
                | AnnotationComment
                | AnnotationType
                | AnnotationColor
                | AnnotationAuthor
        )
    }

    /// Whether the definition has a `normalizedField` (accent- and
    /// case-insensitive matching).
    pub fn normalized(self) -> bool {
        use Kind::*;
        matches!(
            self,
            Tag | Creator
                | LastName
                | Author
                | Editor
                | BookAuthor
                | Field(_)
                | AnnotationText
                | AnnotationComment
        )
    }

    /// Whether the definition has an `inlineFilter`.
    pub fn inline_filter(self) -> bool {
        use Kind::*;
        matches!(
            self,
            NumTags | NumNotes | NumAttachments | NumAnnotations | Key
        )
    }

    /// `Zotero.Search._conditionLevel` (search.js:2323): the level(s) the
    /// condition matches at.
    pub fn levels(self) -> Vec<Level> {
        use Kind::*;
        match self {
            Tag => vec![Level::Any],
            NumTags => vec![
                Level::Item,
                Level::Attachment,
                Level::Note,
                Level::Annotation,
            ],
            NumNotes | NumAttachments => vec![Level::Item],
            NumAnnotations => vec![Level::Item, Level::Attachment],
            Note => vec![Level::Note],
            LastRead | FileTypeId | AttachmentStorageType => vec![Level::Attachment],
            AnnotationText | AnnotationComment | AnnotationType | AnnotationColor
            | AnnotationAuthor => vec![Level::Annotation],
            Field(f) | DateField(f) | NumberField(f) => {
                let fields: Vec<kovan_common::zotero::Field> = match f {
                    Some(f) => vec![f],
                    None => alias_fields(self),
                };
                if fields
                    .iter()
                    .any(|&f| is_valid_for_type(f, kovan_common::zotero::ItemType::Attachment))
                {
                    vec![Level::Item, Level::Attachment]
                } else {
                    vec![Level::Item]
                }
            }
            _ => vec![Level::Item],
        }
    }
}

/// The alias list of a template condition (`field`, `datefield`,
/// `numberfield`), searchConditions.js:610-613, 654-659, 690.
fn alias_fields(kind: Kind) -> Vec<Field> {
    let all = all_fields();
    match kind {
        Kind::Field(_) => all
            .into_iter()
            .filter(|f| {
                !matches!(
                    f.as_str(),
                    "accessDate" | "pages" | "section" | "seriesNumber" | "issue"
                ) && !f.is_date()
            })
            .collect(),
        Kind::DateField(_) => all
            .into_iter()
            .filter(|f| f.is_date() || *f == Field::AccessDate)
            .collect(),
        Kind::NumberField(_) => NUMBER_FIELDS
            .iter()
            .filter_map(|n| Field::from_name(n))
            .collect(),
        _ => Vec::new(),
    }
}

/// Every field of the schema (`fieldsCombined`).
fn all_fields() -> Vec<Field> {
    let mut out: Vec<Field> = Vec::new();
    for t in all_item_types() {
        for f in t.fields() {
            if !out.contains(&f) {
                out.push(f);
            }
        }
    }
    out
}

fn all_item_types() -> Vec<ItemType> {
    kovan_common::zotero::schema_generated::ITEM_TYPE_SCHEMAS
        .iter()
        .map(|s| s.item_type)
        .collect()
}

/// `Zotero.SearchConditions.parseCondition` (searchConditions.js:993):
/// `name/mode` -> (`name`, `mode`).
pub fn parse_condition(condition: &str) -> (&str, Option<&str>) {
    match condition.find('/') {
        Some(p) => (&condition[..p], Some(&condition[p + 1..])),
        None => (condition, None),
    }
}

/// One part of a quick-search string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchPart {
    /// The text, without its quotes.
    pub text: String,
    /// Whether it was a `"double-quoted phrase"`.
    pub in_quotes: bool,
}

/// `Zotero.SearchConditions.parseSearchString` (searchConditions.js:965):
/// split into words and `"double-quoted phrases"`, dropping unpaired quotes
/// at word boundaries.
pub fn parse_search_string(s: &str) -> Vec<SearchPart> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        Regex::new(r#"(?m)\s*("[^"]*")\s*|"\s|\s"|^"|"$|'\s|\s'|^'|'$|\s"#).expect("valid regex")
    });
    // JavaScript split with a capture group: the pieces between matches and
    // each match's group 1 (when it participated).
    let mut parts: Vec<String> = Vec::new();
    let mut last = 0;
    for c in re.captures_iter(s) {
        let m = c.get(0).expect("match");
        parts.push(s[last..m.start()].to_owned());
        if let Some(g) = c.get(1) {
            parts.push(g.as_str().to_owned());
        }
        last = m.end();
    }
    parts.push(s[last..].to_owned());
    let mut out = Vec::new();
    for part in parts {
        if part.is_empty() {
            continue;
        }
        if part.starts_with('"') && part.ends_with('"') {
            let text = if part.len() >= 2 {
                part[1..part.len() - 1].to_owned()
            } else {
                // JavaScript substring(1, 0) swaps its arguments: the quote.
                part.clone()
            };
            out.push(SearchPart {
                text,
                in_quotes: true,
            });
        } else {
            out.push(SearchPart {
                text: part,
                in_quotes: false,
            });
        }
    }
    out
}

/// `Zotero.Utilities.allowedKeyChars` (utilities.js:1729).
pub const ALLOWED_KEY_CHARS: &str = "23456789ABCDEFGHIJKLMNPQRSTUVWXYZ";

/// `Zotero.Utilities.isValidObjectKey` (utilities.js:1741).
pub fn is_valid_object_key(key: &str) -> bool {
    key.len() == 8 && key.chars().all(|c| ALLOWED_KEY_CHARS.contains(c))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_string_parsing() {
        let p = parse_search_string("foo \"bar baz\" qux");
        assert_eq!(
            p,
            vec![
                SearchPart {
                    text: "foo".into(),
                    in_quotes: false
                },
                SearchPart {
                    text: "bar baz".into(),
                    in_quotes: true
                },
                SearchPart {
                    text: "qux".into(),
                    in_quotes: false
                },
            ]
        );
        assert_eq!(
            parse_search_string("\"foo\""),
            vec![SearchPart {
                text: "foo".into(),
                in_quotes: true
            }]
        );
        assert_eq!(parse_search_string("clim chang").len(), 2);
    }

    #[test]
    fn aliases_resolve() {
        assert_eq!(resolve("title"), Some(Kind::Field(Some(Field::Title))));
        assert_eq!(
            resolve("filingDate"),
            Some(Kind::DateField(Field::from_name("filingDate")))
        );
        assert_eq!(
            resolve("accessDate"),
            Some(Kind::DateField(Some(Field::AccessDate)))
        );
        assert_eq!(
            resolve("pages"),
            Some(Kind::NumberField(Some(Field::Pages)))
        );
        assert_eq!(resolve("nope"), None);
        assert_eq!(
            Kind::Field(Some(Field::Title)).levels(),
            vec![Level::Item, Level::Attachment]
        );
        assert_eq!(
            resolve("publicationTitle").unwrap().levels(),
            vec![Level::Item]
        );
        assert_eq!(
            Kind::Field(None).levels(),
            vec![Level::Item, Level::Attachment]
        );
    }

    #[test]
    fn object_keys() {
        assert!(is_valid_object_key("ABCD2345"));
        assert!(!is_valid_object_key("ABCD234O"));
        assert!(!is_valid_object_key("foo"));
    }
}
