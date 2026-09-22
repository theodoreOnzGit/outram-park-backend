# CLAUDE.md — kovan-literature

The workspace-root `CLAUDE.md` applies in full; this file adds what is
specific to the literature archive. **`CATALOGUE.md` is the record of every
document**: what it is, its access tier and why, and where its files live.
Read it before adding, moving or citing anything.

## Where the literature lives (since 2026-09-22)

This crate no longer holds PDFs. It keeps each open document's metadata JSON,
extracted Markdown and BibTeX; the documents themselves live in three places:

| Where | What | Tracked here? |
|---|---|---|
| `reactor-literature/` (Git submodule of [`theodoreOnzGit/reactor-literature`](https://github.com/theodoreOnzGit/reactor-literature), public) | `kovan-standard-open-corpus/`: the documents Kovan hardcodes as its built-in nuclear-engineering corpus (`crates/kovan/src/corpus.rs`). `theodore-open-corpus/`: the maintainer's other open literature, not hardcoded. Each folder's README states every document's licence basis | As a submodule pointer only |
| The private repository `propreitrary-literature-theodore` (maintainer's machine, not a submodule) | Proprietary documents with their extracted text | Never |
| `proprietary/`, `generated/*/proprietary/` | Local, git-ignored proprietary working copies | Never |

Fetch the submodule with `git submodule update --init
crates/kovan-literature/reactor-literature`. A plain clone leaves it an empty
directory, not an error.

## Rules

- **Only documents in `reactor-literature/kovan-standard-open-corpus/` are
  hardcoded into Kovan** (maintainer direction). Adding one means: the PDF and
  its README row there, then its `CorpusLiterature` entry in
  `crates/kovan/src/corpus.rs`, with metadata read from the document itself.
- **The access tier comes from the document's own licence or copyright
  statement**, never from where it is hosted (workspace `DATA_POLICY.md`).
  Nothing without a verified redistribution basis goes into
  `reactor-literature`, which is public: it goes to the private repository.
- **Record every move in `CATALOGUE.md`** with its date, keeping the old
  entry (struck through or annotated), and fix every doc comment that names
  the old path in the same change.
- `reactor-literature/` is excluded from the published crate (`Cargo.toml`
  `exclude`); keep it that way.
