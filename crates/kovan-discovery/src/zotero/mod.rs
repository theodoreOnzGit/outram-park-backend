// Part of the kovan Zotero port (GitHub #747, #751).
//
// Upstream: Zotero, https://github.com/zotero/zotero (commit 9cbba8c4d281):
// chrome/content/zotero/xpcom/data/search.js, data/searchConditions.js,
// xpcom/fulltext.js, xpcom/utilities_internal.js, xpcom/collectionTreeRow.js;
// Zotero utilities, https://github.com/zotero/utilities (commit
// 4051881d59c6), utilities.js.
// Copyright (c) Corporation for Digital Scholarship, Vienna, Virginia, USA
// (older files: (c) 2006-2016 Center for History and New Media, George Mason
// University, Fairfax, Virginia, USA). Licence: AGPL-3.0. See this crate's
// NOTICE, "Upstream: Zotero".

//! Zotero's search engine, evaluated in memory over a
//! [`kovan_common::zotero::ZoteroLibrary`] (GitHub #751, epic #747).
//!
//! Zotero builds an SQL query from a search's conditions and lets SQLite run
//! it. This port follows `Zotero.Search._buildQuery` step by step and
//! replaces each piece of SQL by the set of rows it selects, so *what
//! matches* is upstream's; only the mechanism differs. Every predicate is
//! `itemID [NOT] IN (...)` upstream and a row set here; `AND`/`OR` are
//! intersection/union; the cross-level mapping (`mapPredicate`) walks the
//! same parent links.
//!
//! ```
//! use kovan_common::zotero::{Field, ItemType, ZoteroItem, ZoteroLibrary};
//! use kovan_discovery::zotero::{Search, SearchClock, SearchLibrary};
//!
//! let mut item = ZoteroItem::new(ItemType::Book);
//! item.key = Some("ABCD2345".into());
//! item.set_field(Field::Title, "Neutron Transport");
//! let zl = ZoteroLibrary { items: vec![item], ..Default::default() };
//! let lib = SearchLibrary::new(&zl, SearchClock::utc(0));
//!
//! let mut s = Search::new();
//! s.add_condition("title", "contains", "neutron").unwrap();
//! assert_eq!(s.run(&lib).unwrap(), vec!["ABCD2345".to_string()]);
//! ```
//!
//! | Module | What | Ported from |
//! |---|---|---|
//! | [`search`] | [`Search`]: conditions, `addCondition` (incl. quick-search expansion), `removeCondition`, `setScope`, `search()`, `toJSON`/`fromJSON` | search.js |
//! | [`conditions`] | condition and operator tables, `hasOperator`, `parseSearchString`, `_conditionLevel`, `isValidObjectKey` | searchConditions.js, search.js, utilities.js |
//! | [`levels`] | `combineConditions`, `mapPredicate`, `_rollUpAnyToLevel` | search.js:2111-2543 |
//! | [`library`] | [`SearchLibrary`]: the database rows the SQL reads, built from a `ZoteroLibrary` | userdata.sql/system.sql tables; item.js `setField` storage |
//! | [`fulltext`] | the content-index and scan matching of `fulltextContent` | fulltext.js |
//! | [`normalize`] | `normalizeForSearch`; SQLite `LIKE`, `COLLATE NOCASE`, `CAST`, `SUBSTR` | utilities_internal.js; SQLite |
//! | [`dates`] | the date-condition arithmetic (`DATE(..., 'localtime')`, `'-N days'`) | search.js:1721-1875; SQLite |
//! | [`quick`] | the items pane's quick search ([`quick_search`]) | collectionTreeRow.js:406-535 |
//!
//! Results are item keys, sorted. Nothing here touches the file system,
//! the network or a process; the clock is an input ([`SearchClock`]).
//!
//! ## Conditions: what is equivalent and what is not
//!
//! "Equivalent" means the in-memory rule selects the same rows as the SQL
//! cited, given the same data. The SQL is in search.js unless named.
//!
//! | Condition | SQL upstream generates | Here |
//! |---|---|---|
//! | `joinMode` any/all, `groupStart`/`groupEnd` | `combineConditions`, 2111 | equivalent (first `joinMode` of a group wins, unbalanced `groupEnd` ignored) |
//! | `resultLevel` item/attachment/note/annotation | 1324-1342 + `mapPredicate`, 2366 | equivalent, including the multi-ancestor union (2445-2472) and the up-only roll-up of tags (2512) |
//! | `deleted`, `includeDeleted` | `_deletedItemsSQL`, 2215 | equivalent (`deletedItems` = `deleted: true`) |
//! | `noChildren` | 1281-1288 | equivalent |
//! | `unfiled` | 1290-1301 | equivalent; a collection key absent from `ZoteroLibrary.collections` does not count as filing (a database cannot hold such a row) |
//! | `publications` | 1307 | equivalent (`inPublications`) |
//! | `retracted`, `feed` | 1303, 1311 | **no data**: a `ZoteroLibrary` has no `retractedItems`/`feedItems`, so nothing matches |
//! | `includeParentsAndChildren`, `includeParents`, `includeChildren` | 2007-2039, 778-817 | equivalent (a trashed child does not bring in its parent) |
//! | `recursive` | 1543 | equivalent (non-trashed subcollections, collection.js:852) |
//! | `collection` (`collectionID`) | 1484-1548 | equivalent; `collectionID` takes a key (no database ids). A missing collection matches nothing (`itemID IN (0)`) |
//! | `savedSearch` (`savedSearchID`) | 1549-1584 | equivalent for the library's stored searches (`ZoteroLibrary::searches`, kovan-common's `ZoteroSearch`, read by [`SearchLibrary::new`]) and searches added with [`SearchLibrary::add_saved_search`]; self-reference skipped as upstream (1523); a cycle is an error (upstream recurses without end); with `includeParents*` upstream emits invalid SQL, so this is an error here |
//! | `field` and every field alias (`title`, `publicationTitle`, ...) | 1436-1461 | equivalent, including the base-field mapping (`fieldID IN (base, type fields)`) and accent/case folding of the normalized column |
//! | `datefield` aliases (`date`, `filingDate`, `accessDate`, ...) | 1721-1875 | equivalent: date fields compared on their stored multipart form, `accessDate` as a local date; text operators compare the stored text |
//! | `numberfield` aliases (`pages`, `numPages`, ...) | 1893-1904 | equivalent (`CAST AS INT` with the canonical-integer guard) |
//! | `year` | 1463-1482 | equivalent (`SUBSTR(value, 1, 4)` of the stored date) |
//! | `anyField`, `titleCreatorYear` | expansion, 1214-1265 | equivalent (OR-group, AND-group for negations) |
//! | `creator`, `author`, `editor`, `bookAuthor`, `lastName` | 1669-1682 | equivalent (`TRIM(firstName ' ' lastName)`, normalized per part) |
//! | `tag` | 1645 | equivalent (level-agnostic) |
//! | `numTags`, `numNotes`, `numAttachments`, `numAnnotations` | searchConditions.js:426-503, 1650-1667 | equivalent (trashed children not counted; inline `IN` merging of consecutive `is`/`isNot`) |
//! | `itemType` | 1589 | equivalent (`typeName`, case-insensitive) |
//! | `dateAdded`, `dateModified` | 1721-1875 | equivalent given the clock; `today`/`yesterday`/`tomorrow` are English only (upstream also accepts the locale's words) |
//! | `lastRead` | 1779 | equivalent (`DATE(lastRead, 'unixepoch', 'localtime')`) |
//! | `attachmentStorageType` | 1613-1643 | equivalent |
//! | `fileTypeID` | 1594-1611 | equivalent: the `fileTypes` table of system.sql is built in; the value is the id (`1`) or the name (`webpage`) |
//! | `key` | searchConditions.js:707 | equivalent (case-sensitive `=`, inline `IN`) |
//! | `note` (and the obsolete `childNote`, migrated by `fromJSON`) | 1694-1705, fulltext.js:2528 | **differs slightly**: the note index matches the normalized plain text of the note HTML; Mozilla's HTML-to-text converter is approximated ([`normalize::note_plain_text`]) |
//! | `annotationText`, `annotationComment`, `annotationType`, `annotationColor` | searchConditions.js:734-782 | equivalent (`annotationType` takes the type number, `1` = highlight) |
//! | `annotationAuthor` | 1686 | **unsupported**: needs `groupItems.createdByUserID`, which item JSON lacks |
//! | `fulltextContent` (`/regexp`, `/regexpCS`) | 677-776, 1156-1190, fulltext.js | **equivalent only where content is supplied** ([`SearchLibrary::set_full_text`]); the FTS5 tokenizer is approximated by `[\p{L}\p{N}]+` (upstream's own `_wordTokenRE`), regular expressions use Rust syntax |
//! | `quicksearch-titleCreatorYear`, `-fields`, `-everything`, `-titleCreatorYearNote` | `addCondition`, 315-377 | equivalent expansion, including the key detection and the short-term rules of `canSearchNotes`/`canSearchContent` |
//! | `libraryID`, `itemID`, `itemTypeID`, `tagID`, `tempTable` | | **unsupported**: database ids and temporary tables do not exist for a `ZoteroLibrary` ([`SearchError::Unsupported`]) |
//!
//! Two upstream defects are not reproduced and are documented where they
//! would apply: consecutive `isLessThan`/`isGreaterThan` conditions with an
//! inline filter are merged into an array upstream binds as one integer
//! (only `is`/`isNot` are merged here), and a `savedSearch` combined with
//! `includeParents*` produces invalid SQL upstream (an error here).
//!
//! ## Verification
//!
//! **Reference.** Upstream Zotero desktop cannot run here (it is a Firefox
//! application with a SQLite profile), and the translation server running at
//! 127.0.0.1:1969 does not expose search, so Zotero's own test suite is the
//! reference: `tests/zotero_search.rs` ports test/tests/searchTest.js case by
//! case, with each test's database objects rebuilt as a `ZoteroLibrary`
//! fixture and upstream's expected results unchanged. The prediction for
//! every case is upstream's assertion, written before the port ran.
//!
//! **Results (2026-10-07, `cargo test --release -p kovan-discovery`):**
//! searchTest.js has 128 `it(...)` cases. 118 are ported as 115 test
//! functions (the four `mapPredicate` cases share one; the three
//! `combineConditions` and four `mapPredicate` cases assert SQL text
//! upstream and are asserted on row sets here), and one more ("Loading ...
//! should migrate a stored `childNote`", line 43) reaches the same
//! migration as the ported `fromJSON` case (line 2224). **All pass, with
//! upstream's expected values unchanged.** A mutation check (disabling
//! `mapPredicate`'s same-level shortcut) fails 17 of them, so they
//! discriminate. Not ported (9), with reasons: `#save()` x4 and `#deleted`
//! x2 (database persistence), "Loading ... old-style 'collection'" (line 25;
//! a database reload of the conversion the ported line-10 case covers),
//! "should collect params in order" (SQL bind parameters), and "should
//! populate normalized columns ... on backfill" (a schema migration).
//! One kovan-own test (`stored_zotero_searches_are_evaluated`) checks that
//! saved searches stored as kovan-common's `ZoteroSearch` (the form the
//! Zotero database reader, #750, produces) are read, run by key and
//! round-trip; the upstream fixture stores its saved searches the same way.
//! searchQueryTest.js (the `field:value` query language of searchQuery.js)
//! and advancedSearchTest.js (the dialog UI) are outside this port.
//!
//! **Maturity: AI draft (1).** Not yet human-reviewed.

pub mod conditions;
pub mod dates;
pub mod fulltext;
pub mod levels;
pub mod library;
pub mod normalize;
pub mod quick;
pub mod search;

pub use conditions::{Kind, Level, Operator, SearchPart};
pub use library::{SearchClock, SearchLibrary};
pub use quick::{quick_search, QuickSearchMode, QuickSearchScope};
pub use search::{Condition, Search, SearchError, SearchScope};
