// Part of the kovan Zotero port (GitHub #747, #751).
//
// Upstream: Zotero, https://github.com/zotero/zotero (commit 9cbba8c4d281):
// chrome/content/zotero/xpcom/collectionTreeRow.js:406-535
// (CollectionTreeRow.getSearchObject: how the items pane builds the quick
// search), elements/quickSearchTextbox.js (the modes and the
// `search.quicksearch-mode` preference).
// Copyright (c) Corporation for Digital Scholarship, Vienna, Virginia, USA.
// Licence: AGPL-3.0. See this crate's NOTICE, "Upstream: Zotero".

//! The quick search, built the way Zotero's items pane builds it.

use super::search::{Search, SearchError};

/// The quick-search mode (`search.quicksearch-mode`, quickSearchTextbox.js).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuickSearchMode {
    /// "Title, Creator, Year" (`titleCreatorYear`).
    TitleCreatorYear,
    /// "All Fields & Tags" (`fields`).
    Fields,
    /// "Everything" (`everything`): also note text and attachment content.
    Everything,
    /// Title, creator, year and note text, notes only
    /// (`titleCreatorYearNote`, the citation dialog's note search,
    /// integration/citationDialog/searchHandler.mjs:319).
    TitleCreatorYearNote,
}

impl QuickSearchMode {
    /// The condition name, `quicksearch-<mode>`.
    pub fn condition(self) -> &'static str {
        match self {
            QuickSearchMode::TitleCreatorYear => "quicksearch-titleCreatorYear",
            QuickSearchMode::Fields => "quicksearch-fields",
            QuickSearchMode::Everything => "quicksearch-everything",
            QuickSearchMode::TitleCreatorYearNote => "quicksearch-titleCreatorYearNote",
        }
    }
}

/// The collection-tree row being searched.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuickSearchScope {
    /// The library root ("My Library").
    Library,
    /// A collection, by key; `recursive` is the `recursiveCollections`
    /// preference.
    Collection {
        /// The collection key.
        key: String,
        /// Include subcollections.
        recursive: bool,
    },
    /// My Publications.
    Publications,
    /// The trash.
    Trash,
}

/// `CollectionTreeRow.getSearchObject` (collectionTreeRow.js:406) for the
/// rows a `ZoteroLibrary` has: the inner (scope) search, and the outer
/// filter search with the quick-search condition and the selected tags.
pub fn quick_search(
    text: &str,
    mode: QuickSearchMode,
    scope: &QuickSearchScope,
    tags: &[&str],
) -> Result<Search, SearchError> {
    let mut s = Search::new();
    let mut include_scope_children = false;
    match scope {
        QuickSearchScope::Library => {
            s.add_condition("noChildren", "true", "")?;
            include_scope_children = true;
        }
        QuickSearchScope::Collection { key, recursive } => {
            s.add_condition("noChildren", "true", "")?;
            s.add_condition("collectionID", "is", key)?;
            if *recursive {
                s.add_condition("recursive", "true", "")?;
            }
            include_scope_children = true;
        }
        QuickSearchScope::Publications => s.add_condition("publications", "true", "")?,
        QuickSearchScope::Trash => s.add_condition("deleted", "true", "")?,
    }
    let mut s2 = Search::new();
    if *scope == QuickSearchScope::Trash {
        s2.add_condition("deleted", "true", "")?;
    }
    s2.set_scope(s, include_scope_children);
    if !text.is_empty() {
        s2.add_condition(mode.condition(), "contains", text)?;
    }
    for t in tags {
        s2.add_condition("tag", "is", t)?;
    }
    Ok(s2)
}
