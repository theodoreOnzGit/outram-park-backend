// Part of the kovan Zotero port (GitHub #747, #749).
//
// Upstream: Zotero utilities, https://github.com/zotero/utilities (commit
//   4051881d59c6, the translation-server's submodule):
//   resource/zoteroTypeSchemaData.js (`itemTypes[id][2]`, the creator type
//   ids of each item type, in order, resolved through `creatorTypes`), read
//   by cachedTypes.js `getTypesForItemType` :102-121 and utilities.js
//   `getCreatorsForType` :1291-1299. Generated 2026-10-07 by loading that
//   file in Node and printing the table below.
// Copyright (c) Corporation for Digital Scholarship, Vienna, Virginia, USA.
// Licence: AGPL-3.0 (upstream: AGPL-3.0-or-later).

//! `Zotero.Utilities.getCreatorsForType` as the translation-server answers
//! it. The RDF import translator walks every creator type of an item type
//! in this order and appends the creators of each, so the order decides the
//! order of the imported creators. The framework's
//! [`get_creators_for_type`](crate::zotero::framework::utilities::get_creators_for_type)
//! takes kovan-common's schema.json order, which agrees on the first
//! (primary) type but orders some secondary types differently; this table is
//! the server's own snapshot.

/// Item type -> creator types, primary first, in the server's order.
pub const SERVER_CREATOR_TYPES: &[(&str, &[&str])] = &[
    ("note", &[]),
    (
        "book",
        &[
            "author",
            "contributor",
            "editor",
            "seriesEditor",
            "translator",
        ],
    ),
    (
        "bookSection",
        &[
            "author",
            "bookAuthor",
            "contributor",
            "editor",
            "seriesEditor",
            "translator",
        ],
    ),
    (
        "journalArticle",
        &[
            "author",
            "contributor",
            "editor",
            "reviewedAuthor",
            "translator",
        ],
    ),
    (
        "magazineArticle",
        &["author", "contributor", "reviewedAuthor", "translator"],
    ),
    (
        "newspaperArticle",
        &["author", "contributor", "reviewedAuthor", "translator"],
    ),
    ("thesis", &["author", "contributor"]),
    (
        "letter",
        &["author", "contributor", "recipient", "translator"],
    ),
    ("manuscript", &["author", "contributor", "translator"]),
    (
        "interview",
        &["interviewee", "contributor", "interviewer", "translator"],
    ),
    (
        "film",
        &[
            "director",
            "castMember",
            "contributor",
            "guest",
            "host",
            "narrator",
            "producer",
            "scriptwriter",
            "translator",
        ],
    ),
    ("artwork", &["artist", "contributor"]),
    ("webpage", &["author", "contributor", "translator"]),
    ("attachment", &[]),
    (
        "report",
        &[
            "author",
            "contributor",
            "editor",
            "seriesEditor",
            "translator",
        ],
    ),
    ("bill", &["sponsor", "contributor", "cosponsor"]),
    ("case", &["author", "contributor", "counsel"]),
    ("hearing", &["contributor"]),
    ("patent", &["inventor", "attorneyAgent", "contributor"]),
    ("statute", &["author", "contributor"]),
    (
        "email",
        &["author", "contributor", "recipient", "translator"],
    ),
    ("map", &["cartographer", "contributor", "seriesEditor"]),
    (
        "blogPost",
        &["author", "commenter", "contributor", "translator"],
    ),
    ("instantMessage", &["author", "contributor", "recipient"]),
    ("forumPost", &["author", "contributor"]),
    (
        "audioRecording",
        &[
            "performer",
            "composer",
            "contributor",
            "originalCreator",
            "translator",
            "wordsBy",
        ],
    ),
    (
        "presentation",
        &[
            "presenter",
            "chair",
            "contributor",
            "organizer",
            "translator",
        ],
    ),
    (
        "videoRecording",
        &[
            "creator",
            "castMember",
            "contributor",
            "director",
            "executiveProducer",
            "guest",
            "host",
            "narrator",
            "producer",
            "scriptwriter",
            "translator",
        ],
    ),
    (
        "tvBroadcast",
        &[
            "director",
            "castMember",
            "contributor",
            "executiveProducer",
            "guest",
            "host",
            "narrator",
            "producer",
            "scriptwriter",
            "seriesCreator",
            "translator",
        ],
    ),
    (
        "radioBroadcast",
        &[
            "creator",
            "castMember",
            "contributor",
            "director",
            "executiveProducer",
            "guest",
            "host",
            "producer",
            "scriptwriter",
            "seriesCreator",
            "translator",
        ],
    ),
    (
        "podcast",
        &[
            "podcaster",
            "castMember",
            "contributor",
            "director",
            "executiveProducer",
            "guest",
            "producer",
            "scriptwriter",
            "seriesCreator",
            "translator",
        ],
    ),
    ("computerProgram", &["programmer", "contributor"]),
    (
        "conferencePaper",
        &[
            "author",
            "contributor",
            "editor",
            "seriesEditor",
            "translator",
        ],
    ),
    (
        "document",
        &[
            "author",
            "contributor",
            "editor",
            "reviewedAuthor",
            "translator",
        ],
    ),
    (
        "encyclopediaArticle",
        &[
            "author",
            "contributor",
            "editor",
            "seriesEditor",
            "translator",
        ],
    ),
    (
        "dictionaryEntry",
        &[
            "author",
            "contributor",
            "editor",
            "seriesEditor",
            "translator",
        ],
    ),
    ("annotation", &[]),
    (
        "preprint",
        &[
            "author",
            "contributor",
            "editor",
            "reviewedAuthor",
            "translator",
        ],
    ),
    ("dataset", &["author", "contributor"]),
    ("standard", &["author", "contributor", "editor"]),
    ("nsfReviewer", &[]),
];

/// The creator types of `item_type` in the server's order (empty for an
/// unknown type, `note` and `attachment`).
pub fn server_creators_for_type(item_type: &str) -> &'static [&'static str] {
    if item_type == "attachment" || item_type == "note" {
        return &[];
    }
    SERVER_CREATOR_TYPES
        .iter()
        .find(|(t, _)| *t == item_type)
        .map_or(&[], |(_, c)| *c)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn video_recordings_list_creator_first() {
        assert_eq!(server_creators_for_type("videoRecording")[0], "creator");
        assert_eq!(
            server_creators_for_type("book"),
            [
                "author",
                "contributor",
                "editor",
                "seriesEditor",
                "translator"
            ]
        );
    }
}
