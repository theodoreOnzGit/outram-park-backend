# kovan-web (web-kovan)

**KOVAN-WEB**: **K**nowledge **O**riented **V**&V **A**nalysis for
**N**uclear **S**ciences, on the **Web**. The kovan family's name (`docs/ecosystem-naming.md`)
with the place it runs.

> **On crates.io (2026-10-07):** the KOVAN app is published as
> `knowledge-oriented-vv-analysis-for-nuclear-sciences-kovan`, its backronym
> spelled out (`kovan` on crates.io is an unrelated crate). Install it with
> `cargo install knowledge-oriented-vv-analysis-for-nuclear-sciences-kovan`;
> the binaries are still `kovan`, `kovan-cli` and `kovan-tui`, and the library
> is still `kovan`.

> ⚠️ **Research, education and V&V only.** Not for nuclear facility operation,
> reactor control, licensing, safety-critical decisions or emergency response.
> See the workspace `RESPONSIBLE_USE.md`.

## Scope

The **read-only Code Review UI** (GitHub #735, #736, #738), written once in
egui so that it runs in the browser (wasm32, published on GitHub Pages at
`code-review/`) and natively, and so that desktop kovan can later embed the
same UI with stamping added (#740). It answers one question: **what is in
this workspace, how far has each part been reviewed, and what must be
reviewed first?**

- **Code map:** every crate in its topic box (`kovan_common::code_map`, the
  same model and layout as `kovan-cli code-map`). ▸ on a crate card shows
  its top-level modules in place.
- **Crate view:** the crate's whole module tree (one card per source file,
  following `mod` declarations; examples as their own group). No functions
  here: a module card shows its path, maturity and function count.
  Module-to-module calls are drawn faintly and lit for the selected module.
- **Module view:** the module's functions on a ring around it. ▸ unfolds a
  function's callees and ◂ its callers, level by level; a function already
  on screen is drawn once. Cards show stamp state (unreviewed, valid,
  stale) and maturity. The side panel lists the functions as rustdoc does,
  with "API ↗" where the site has the rustdoc page.
- **Review bar:** the selected function's stamp state, maturity and "blocked
  by N functions underneath" (callees without a valid stamp, as links).
  Stamp and Needs fix are desktop-only and shown disabled.
- **Source panel:** the function's lines, fetched from the repository at the
  commit the site was built from; pop-out to a new tab on a deep link
  (`code-review/#<function id>`); a full-screen sheet on a phone.
- **Search bar:** crates, modules and functions by name and path fragments.
- **Back to the site (web only, 2026-10-07):** two big buttons at the top of
  the side panel, "Go back to JavaScript map" (`code-map/#<crate>` on the
  crate on screen) and "Go to homepage". The JavaScript map's details panel
  has the way in: a big "Go to Code Review map" button that opens
  `code-review/#crate=<crate>` for the selected crate, or the map with none.

~~`Mode::Desktop` is reserved and **not implemented**.~~ **CORRECTED
2026-10-10** (#820): `Mode::Desktop` is the same UI inside desktop kovan's
Code Review tab. Stamp and Needs fix are enabled there and handed to
desktop kovan as a `ui::HostRequest`; the stamp dialog itself is not
written yet (a placeholder window, #740, #770).

## Status

**AI draft (maturity 1), 2026-10-06.** Builds for wasm32 and natively; the
pure model has unit tests; checked by screenshots in headless Chromium at
desktop and phone width. Not reviewed by a human. Known gaps are in
`CLAUDE.md`.

## Running it

```text
crates/kovan-web/web/data.sh <out>/data        # code map + call graph (rust-analyzer scip, ~4 min, ~16 GiB)
bash crates/kovan-web/web/build.sh <out>       # the wasm page
python3 -m http.server -d <out> 8000
cargo run -p kovan-web --example web --release -- --data <out>/data --workspace .
```

## Licence

AGPL-3.0-only, like ~~the rest of the kovan front ends~~ all of kovan since
2026-10-07 (see `NOTICE`).

## Bookkeeping status

> Maintainer sign-off tracker (see the workspace `CLAUDE.md` "Bookkeeping
> pass" command). A crate is **complete** only once the maintainer has
> personally signed off on BOTH axes below.

| Axis | Status |
|---|---|
| Verification & Validation (V&V) — human-reviewed | ❌ Not yet manually checked |
| Human / user interface — human-reviewed | ❌ Not yet manually checked |

**Status: INCOMPLETE** until both axes are manually checked and cleared by the maintainer.
