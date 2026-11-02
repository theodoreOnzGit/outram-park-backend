# BUGIS

**B**rowser-based **U**ser **G**raphics **I**nteraction **S**ystem (backronym set by the maintainer, 2026-11-02).

> ⚠️ **Research, education and V&V only.** Not for nuclear facility operation,
> reactor control, licensing, safety-critical decisions or emergency response.
> See the workspace `RESPONSIBLE_USE.md`.

The workspace's home for **colour theming in general** (maintainer,
2026-11-02), and the one place the egui simulators get their look from. It
answers
**"what does an OUTRAM PARK simulator look like?"**: the **Gruvbox** palette as
a dark and a light `egui::Visuals`, and a Dark/Light selector (maintainer,
2026-11-02: "give all simulators a gruvbox theme").

```rust,ignore
let mut theme = bugis::Theme::default(); // Gruvbox Dark
theme.apply(ctx);                        // once, at start-up
bugis::selector(ui, &mut theme);         // in the side panel; applies on change
let series = theme.accent(0);            // a plot colour that follows the theme
```

## Status

Implemented: `Theme` (Gruvbox Dark, the default, and Gruvbox Light), `apply`,
`selector`, `accent`, the full `palette`, `gruvbox_visuals`. It depends on
`egui` only (never `eframe`), so it compiles for Android and wasm32.

The visuals were moved here from `kovan`'s `src/app/theme.rs`; `kovan` and the
`tampines-steam-tables` GUI now call this crate instead of carrying copies.

Not done: the simulators' hard-coded drawing colours (plot series, schematics)
are not all routed through `Theme::accent`; only the egui chrome follows the
theme everywhere.

## Bookkeeping status

| Axis | Status |
|---|---|
| Human reviewed | ☐ |
| Human V&V | ☐ |

## Licence

AGPL-3.0-only. The Gruvbox palette is MIT (morhetz/gruvbox); see `NOTICE`.
