# CSL style and locale files (GitHub #789, #790, #791)

Pinned copies used by `kovan_literature::csl` and by the citeproc-js reference
(`scripts/csl-reference.sh`). Both engines read these exact files.

| File | Source | Commit | Licence |
|---|---|---|---|
| `apa.csl` | [citation-style-language/styles](https://github.com/citation-style-language/styles), `apa.csl` ("APA Style 7th edition", updated 2026-02-07) | `0151fd1ed467ec54f4f2554d68fce9ccc6ff0118` | CC BY-SA 3.0 |
| `chicago-author-date.csl` | same repository ("Chicago Manual of Style 18th edition (author-date)", updated 2025-02-09) | `0151fd1ed467ec54f4f2554d68fce9ccc6ff0118` | CC BY-SA 3.0 |
| `ieee.csl` | same ("IEEE Reference Guide version 11.29.2023", updated 2024-03-27) | `0151fd1ed467ec54f4f2554d68fce9ccc6ff0118` | CC BY-SA 3.0 |
| `nature.csl` | same ("Nature", updated 2025-09-10; `default-locale` en-GB) | `0151fd1ed467ec54f4f2554d68fce9ccc6ff0118` | CC BY-SA 3.0 |
| `nlm-citation-sequence.csl` | same ("NLM/Vancouver: Citing Medicine 2nd edition (citation-sequence)", updated 2026-01-07). This is the **Vancouver** style of the site set: the repository has no independent `vancouver.csl`; `dependent/vancouver-nlm.csl` ("Vancouver - NLM (citation-sequence)", updated 2026-01-28) is a dependent style whose `independent-parent` is this file, so the parent is taken | `0151fd1ed467ec54f4f2554d68fce9ccc6ff0118` | CC BY-SA 3.0 |
| `locales-en-US.xml` | [citation-style-language/locales](https://github.com/citation-style-language/locales) | `a89adece41013402236e2c9020972d7e931fbab8` | CC BY-SA 3.0 |
| `locales-en-GB.xml` | same repository, for Nature's `default-locale` | `a89adece41013402236e2c9020972d7e931fbab8` | CC BY-SA 3.0 |

Fetched 2026-10-08 (#791 added the four styles and `locales-en-GB.xml`), unmodified. The licence and author credits are in each
file's `<info>` block. To update, fetch a new commit, then re-run
`scripts/csl-reference.sh` and the comparison test.
