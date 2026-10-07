# CSL style and locale files (GitHub #789, #790)

Pinned copies used by `kovan_literature::csl` and by the citeproc-js reference
(`scripts/csl-reference.sh`). Both engines read these exact files.

| File | Source | Commit | Licence |
|---|---|---|---|
| `apa.csl` | [citation-style-language/styles](https://github.com/citation-style-language/styles), `apa.csl` ("APA Style 7th edition", updated 2026-02-07) | `0151fd1ed467ec54f4f2554d68fce9ccc6ff0118` | CC BY-SA 3.0 |
| `locales-en-US.xml` | [citation-style-language/locales](https://github.com/citation-style-language/locales) | `a89adece41013402236e2c9020972d7e931fbab8` | CC BY-SA 3.0 |

Fetched 2026-10-08, unmodified. The licence and author credits are in each
file's `<info>` block. To update, fetch a new commit, then re-run
`scripts/csl-reference.sh` and the comparison test.
