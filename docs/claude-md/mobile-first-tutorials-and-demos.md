# Tutorials and demos work on mobile (HARD RULE)

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

**Reference implementation:** `crates/dhoby-ghaut/examples/triso_pebble_web/app.rs`
(`View::zoom_about`, `View::fit`, `TrisoApp::canvas`, and the panel folding in
`TrisoApp::ui`). Reuse it rather than writing a second version.

## Pages that are mostly text

mdBook pages are responsive already. Keep them that way:
- no fixed-width tables or figures that force horizontal scrolling of the page
  (wide tables scroll inside their own box);
- embedded demos follow the layout above.

## How to check

Before calling a demo or tutorial done, check it at phone width (about
390 × 844, with the browser's device toolbar or a real phone), and say in the
report that you did. A compile check does not count as this check.
