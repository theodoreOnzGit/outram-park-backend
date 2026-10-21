# kovan-semantics `zotero`: verification of the duplicates, relations and merge port

GitHub #751, epic #747. Upstream: Zotero @ `9cbba8c4d281`, Zotero utilities @ `4051881d59c6`.
**Maturity: AI draft (1).** Nothing here has been reviewed by a human yet.

## Methodology

**What is computed.** Duplicate sets (`find_duplicates`), relation operations (`relations`), and merges (`merge_items`), all over a `ZoteroLibrary` held in memory.

**Reference.** Upstream Zotero's own test suites. Zotero desktop cannot run here. The translation server running at `http://127.0.0.1:1969` exposes translation only, not duplicate detection, relations or merging. So there is no code-to-code run against upstream; that work is #752. The tests below are ported case by case, with the same inputs and the same expected results.

**Fixtures.** Upstream tests build their state through the database, the file system or the UI: `createDataObject`, `importPDFAttachment`, `createGroup`, the duplicates pane. Here that state is written as a `ZoteroLibrary`.

Upstream also reads attachment files: their MD5, their text, whether they exist, and whether they have embedded annotations. Here those facts are written as `AttachmentEvidence`, chosen to satisfy what each upstream test asserts about its files. For example, mergeItemsTest.js:243 asserts the two files have equal text and different hashes.

The test PDFs' actual text is not available here (JSTOR, wonderland, watermarked). Where a test's outcome depends on that text, synthetic text is used that has the property the test asserts:

| Upstream test | Property the synthetic text has |
|---|---|
| JSTOR | the texts differ, but the 50 most common words are the same |
| wonderland | the short and long texts have different top-50 words |
| watermarked | the texts are equal except for one-off watermark words |

Each test's doc comment says so.

**Pass criterion.** Every upstream assertion holds, unchanged. No expected value was altered and no tolerance applies (all results are exact).

**Prediction before the first run.** All cases pass. The one I was least sure of was the internal-character scan in `cleanISBN`. Rust's `char::is_whitespace` includes U+0085 while JavaScript's `\s` does not, so the port uses its own `is_js_whitespace`.

**Check that the suite can fail.** After the first green run, three mutations were applied to `merge.rs` and then reverted:

1. drop the "skip the master" guard in `moveRelations`;
2. drop the linked-URL title-only exclusion;
3. raise the 50-word text hash to 500.

Four ported tests failed: `duplicates_merge_no_relation_to_self`, `merge_keeps_linked_url_with_different_url`, `merge_allows_small_text_differences` and `merge_matches_one_to_one`.

## Results (2026-10-07, `cargo test --release -p kovan-semantics`)

`tests/zotero_upstream.rs`: **77 passed, 0 failed**. The module's own unit tests: 20 passed.

| Upstream file | Cases ported | Pass | Notes |
|---|---|---|---|
| utilities `test/tests/utilitiesTest.js`, `#cleanISBN()` | 10 | 10 | includes the scan over U+0001 to U+052E |
| utilities `utilitiesTest.js`, `toISBN13` | 3 | 3 | upstream "throws" is `None` here |
| `duplicatesTest.js` | 4 | 4 | the three UI merges run as: duplicate set, then pane order, then `merge_items` |
| `relationsTest.js` | 3 | 3 | |
| `dataObjectTest.js` (relations, `_getLinkedObject`, `_addLinkedObject`) | 9 | 9 | two libraries are passed as `LibraryRef`s |
| `itemTest.js` (`#addRelatedItem`, `#_eraseData` relations) | 4 | 4 | |
| `mergeItemsTest.js` | 43 | 43 | all `it` cases; file facts supplied as evidence |
| (not upstream) `replaceAllItemKeys` empty-map behaviour | 1 | 1 | pins a flagged upstream behaviour, see below |

## Equivalence notes

These are the places where the port is not exactly upstream:

- **Duplicates.** Rows come from the items, not from SQL. The resulting partition does not depend on row order, so it is the same as upstream's.
  - A date's year comes from the stored multipart form, made by `str_to_multipart`.
  - String values sort in UTF-16 order.
  - Sets are reported sorted.
  - Only the top-level `items` list is a candidate.
- **Merge.**
  - Children are taken in library order. Upstream sorts them by title, unless the user has chosen chronological order.
  - The text hash is the sorted word list itself, not its MD5. Only equality of hashes is ever used, so this does not change any result.
  - Ties at the 50th word are broken by diacritic-folded UTF-16 order, not by ICU collation.
  - The 500 MB size limit is not applied, because there are no file sizes here.
  - A split-off note gets a deterministic key instead of a random one.
  - `dateModified` is not stamped.
- **Relations.**
  - The upstream registry is replaced by a scan of the library.
  - `_getPrefixAndValue`'s full-URI branch references an undefined variable upstream, so it throws. The port gives an error for that input too.
  - `users/<id>` ids must be numeric.

## Flagged upstream behaviour (ported as is)

`Zotero.Notes.replaceAllItemKeys` builds the regex `%2Fitems%2F(<keys>)` from the remap of merged attachments (notes.js:301). When no PDF was merged, the remap is empty and the regex becomes `%2Fitems%2F()`. That empty group matches before every key, so a link `%2Fitems%2FKEY` becomes `%2Fitems%2FundefinedKEY`.

`mergeItems` calls this function on every moved note. Reading the code, this corrupts note links whenever a merge merges no PDF. The port reproduces it, and `flagged_note_links_when_no_attachment_was_merged` pins it. This comes from reading the code only; it has not been observed in a running Zotero (#752).

## Not ported

- **Moving annotations stored in the PDF file.** `PDFWorker` and the PDF file reads behind it are not ported. The facts the merge needs from them (`hasEmbeddedAnnotations`, MD5, text) are inputs.
- **Duplicates-view UI details.** The temporary-table search object and the `removeDuplicatesMaster` notifier are not ported; `DuplicateSets` replaces them.
- **Cross-library effects.** In upstream, subjects in other libraries are explicitly skipped by `moveRelations`. Here the merge sees only one library, so there is nothing to skip.
- **`multiDiff` member diffs.** The member diffs for collections, tags and relations, and the HTML diff of notes, are not ported. The merge pane does not offer these as alternatives.
- **Upstream assertions about files.** mergeItemsTest.js:636 also checks that the master's file still exists. There are no files here, so that one assertion is omitted.
- **`itemTest.js` clone and `toJSON` relation cases.** These are at itemTest.js:2408, 2662 and 2791. They test `clone` and `toJSON`, which belong to `kovan_common::zotero`, not this module.

## The relation, collection and tag mapping onto kovan

This is the table in `src/zotero/kovan_map.rs`. In short:

| Zotero | Becomes in kovan | Lost or changed |
|---|---|---|
| `dc:relation` between two live top-level items | one `related_to` relation per pair, written by `relation::add_connection` | which direction was stored |
| collections | topic concepts at `collection:<slug path>` | names reduced to slugs |
| collection membership | the paper's topics | an item in no live collection gets `unsorted` |
| tags | the `KovanDocument` conversion already maps them | nothing beyond that conversion |

`owl:sameAs`, `dc:replaces`, other predicates, relations held by child items, and collection relations are all recorded as losses. They survive only inside `KovanDocument::zotero_item`.
