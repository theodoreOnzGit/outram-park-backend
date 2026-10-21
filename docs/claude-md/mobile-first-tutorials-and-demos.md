# Tutorials and demos work on mobile, and never lag (HARD RULES)

**Maintainer direction, 2026-10-04.** Every tutorial page and every interactive
demo or simulation it uses must work on a phone. That covers the backend site
(deep dives and their demos), the frontend teaching site, and any egui app
built for the web (`dhoby-ghaut` demos, `outram-park-digital-twin-engine`
simulators when they get a wasm build). A layout that only works with a mouse
and a wide screen is a defect, not a missing nicety.

## The layout every interactive demo uses

```
┌───────────────────────────────────────────────┐
│ [Controls »]               [+] [−] [Reset]    │  ← always visible, finger-sized
│                                               │
│                main view                      │
│        (the picture: geometry, plot, map)     │
│                                               │
│ ── 1 cm                                       │  ← scale bar where there is a length
└───────────────────────────────────────────────┘
       side panel folded away (phone default)

┌───────────────┬───────────────────────────────┐
│ Title [« Hide]│               [+] [−] [Reset] │
│ controls…     │                               │
│ (scrolls)     │          main view            │
│               │                               │
└───────────────┴───────────────────────────────┘
       side panel open (wide-screen default)
```

1. **The main view fills the page.** It is the picture: geometry, plot or map.
2. **Zoom in, zoom out, and reset as on-screen buttons**, always visible on
   the main view. Reset centres the subject and fits it to the screen. Scroll,
   pinch, drag and double-click may also work, but **they never replace the
   buttons**: a phone has no wheel, and gestures are not discoverable.
3. **All controls sit in a collapsible side panel.** It has a visible button
   to fold it ("« Hide") and one on the main view to bring it back
   ("Controls »"). Do not rely on a drag handle: it is a few pixels wide and
   hard to hit with a finger. The panel scrolls when it is taller than the
   screen.
4. **It opens folded on a narrow screen** (under about 700 px wide) and open
   on a wide one.
5. **Touch-sized targets.** Buttons are at least about 36 px tall. Nothing
   that matters is shown only on hover.
6. **Status the reader needs** (loading progress, errors) is drawn on the main
   view, so it shows with the panel folded.

**Reference implementation (since 2026-10-04): the library module
`dhoby_ghaut::web_demo`** (`crates/dhoby-ghaut/src/web_demo/`): `view::View`
(`zoom_about`, `fit`), `view::zoom_buttons`, `panel::Panel` (folding, "«
Hide", "Controls »"), `loading::Loading`. **Build on it rather than writing a
second version**; `crates/dhoby-ghaut/CLAUDE.md` says how a new track's demo
plugs in. ~~`examples/monte_carlo_web/app.rs` (`View::zoom_about`, `View::fit`,
`McApp::canvas`, the panel folding in `McApp::ui`)~~, moved into the library
the same day; `monte_carlo_web` is its first user.

## No lagging: computation runs in the background (HARD RULE)

**Maintainer direction, 2026-10-04.** The app never freezes. The UI thread
only draws and handles input. Every calculation that can take more than a
frame runs in the background: processing nuclear data, transport, eigenvalue
cycles, sweeps, loading and parsing.

- **On the web,** that means a Web Worker. **Natively,** it means a background
  thread.
- **The UI reads results without blocking.** Shared state follows the
  workspace rule (`Arc<RwLock<T>>`, e.g. the mailbox in
  `monte_carlo_web/engine.rs`, ~~`triso_pebble_web`~~ renamed 2026-10-04).
  Never hold a lock across a long computation;
  take `try_read`-style snapshots for drawing.
- **Long work streams results:** per generation, per batch, per nuclide. The
  view updates as results arrive, with progress and elapsed time on screen and
  a way to stop or restart.
- **Work per frame stays small** (a few milliseconds). Animating many
  particles is precomputed in the background and only interpolated on the UI
  thread.
- **How to check:** while the heaviest computation in the demo is running, pan,
  zoom, open and fold the panel, and move a slider. All of it must respond
  immediately. Say in the report that you checked, alongside the phone-width
  check.

**Reference implementation:** `dhoby_ghaut::web_demo::link` (since
2026-10-04: `Link`, `NativeEngine` / `start_native`, `WorkerEngine` /
`worker_main` with the hello handshake), used by
`crates/dhoby-ghaut/examples/monte_carlo_web/engine.rs`
(~~`triso_pebble_web/engine.rs`~~, renamed 2026-10-04): processing and
transport in a worker, the page animating while the ENDF data are processed,
and the Godiva Run k_eff running one power-iteration generation per worker
message.

## Pages that are mostly text

mdBook pages are responsive already. Keep them that way:
- no fixed-width tables or figures that force horizontal scrolling of the page
  (wide tables scroll inside their own box);
- embedded demos follow the layout above.

## Demo links open in a new tab

**Maintainer direction, 2026-10-05.** A link from any page into a demo opens
in a new tab, so the reader keeps the lesson or menu they came from. Links
inside one demo (its geometry page, its "Up" link) stay in the same tab.

Nothing per page is needed. `docs/site/site-nav.js` marks every link that
resolves into `demos/<demo>/` with `target="_blank" rel="noopener"`, and it
also catches links a page adds after loading. `scripts/build-pages.sh` adds
that script to every built page, and the main menu loads it itself. Write
demo links as ordinary links. Do not remove the script from a page, and do
not add a `target` of your own, because the script leaves a link that
already has one unchanged.

## How to check

Before calling a demo or tutorial done, check it at phone width (about
390 × 844, with the browser's device toolbar or a real phone), and say in the
report that you did. A compile check does not count as this check.
