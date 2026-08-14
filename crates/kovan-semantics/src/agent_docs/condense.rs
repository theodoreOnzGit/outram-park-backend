//! Condense a crate's `docs/api.md` down to a signature index.
//!
//! # What this is for
//!
//! The bundle can carry only a few crates' documentation in full. Without
//! something covering the rest, an external agent asked about
//! `tampines-steam-tables` when it was handed only `outram-foam-basic-lib` has
//! no way to know the former exists — and an agent that does not know a type
//! exists will confidently invent one. The index is the cheap map that prevents
//! that: every crate, every module, every public signature, one line of prose
//! each, and nothing else.
//!
//! # What is dropped, and why that is acceptable here
//!
//! Dropped: doc-comment bodies beyond the first line, examples, status tables,
//! licence and trademark preambles, and the `# Crate Documentation` /
//! `**Format Version:**` header rustdoc-md emits. Kept: headings, the module
//! path, and every line inside a fenced block that declares a public item.
//!
//! This is **lossy in a way the reader cannot see from the output**, which is
//! why [`condensed_index_markdown`] writes a banner at the top of the file
//! saying so and pointing at the full `api.md`. An index that looks like
//! complete documentation is worse than no index.
//!
//! # A structural limit, inherited from rustdoc-md
//!
//! rustdoc-md emits **flat headings** — a submodule gets the same heading level
//! as its parent — so the module tree cannot be recovered from heading depth.
//! This is recorded in `scripts/gen_api_docs.py`'s own docstring as a knowingly
//! accepted trade-off. The condenser therefore carries the module path from the
//! `# Module \`x\`` heading text and never infers nesting from `#` count.

use std::fs;
use std::io;
use std::path::Path;

use super::{estimated_tokens, CrateEntry};

/// Render `_INDEX.md`: the **roster** of every crate in the workspace and what
/// each one would cost to request.
///
/// # Why this is a roster and not the whole index
///
/// The original design put a condensed signature index of every crate into this
/// one file. Measured on the real workspace on 2026-08-14, that file came to
/// **535,580 bytes — about 134 k estimated tokens**, or two thirds of a 200 k
/// budget consumed before a single crate's real documentation was uploaded. Its
/// bulk was evenly spread (headings 25%, signatures 44%, descriptions 29%), so
/// no single trim rescued it.
///
/// The content was not dropped; it was **split per crate** into
/// `<crate>.index.md` (see [`crate_index_markdown`]), which makes the bundle a
/// ladder the reader can climb one crate at a time:
///
/// | Tier | File | Typical size | Upload when |
/// |---|---|---|---|
/// | Roster | `_INDEX.md` | ~3 KB | always |
/// | Index | `<crate>.index.md` | ~40 KB | you need to know what a crate contains |
/// | Full | `<crate>.api.md` | ~400 KB | you are writing against that crate |
///
/// Uploading every `.index.md` reproduces the old single file exactly, so
/// nothing is lost by the split — it only becomes optional.
///
/// Output is deterministic: `entries` is consumed in the order given (which
/// [`inventory`](super::inventory) sorts by directory name) and nothing clock-
/// or machine-dependent is written.
pub fn condensed_index_markdown(entries: &[CrateEntry]) -> String {
    let mut out = String::new();

    out.push_str("# Outram Park — crate roster\n\n");
    out.push_str(
        "The workspace is ~37 Rust crates. This file lists all of them and says \
         which documentation exists for each. It deliberately contains **no API \
         detail** — that is in the per-crate files described below, which are \
         uploaded separately so you only pay for what you need.\n\n",
    );

    out.push_str("## How the documentation is layered\n\n");
    out.push_str("| File | What it is | When it is uploaded |\n");
    out.push_str("|---|---|---|\n");
    out.push_str("| `_INDEX.md` | this roster | always |\n");
    out.push_str(
        "| `<crate>.index.md` | condensed: module paths and public signatures, one line of description each | when the crate is relevant |\n",
    );
    out.push_str(
        "| `<crate>.api.md` | the crate's full rustdoc: every doc comment, valid ranges, units, caveats | when code is being written against it |\n\n",
    );
    out.push_str(
        "**A `.index.md` is not documentation.** It tells you what exists and \
         what it is called. It does **not** tell you how anything behaves, what \
         units it takes, or whether it is validated — that text was stripped. If \
         a question turns on behaviour, say you need that crate's `.api.md` \
         rather than inferring from a signature.\n\n",
    );

    let documented: Vec<&CrateEntry> = entries.iter().filter(|e| e.has_api_docs()).collect();
    let undocumented: Vec<&CrateEntry> = entries.iter().filter(|e| !e.has_api_docs()).collect();

    out.push_str("## Crates with generated documentation\n\n");
    out.push_str("| Crate | Condensed index | Full docs | Est. tokens (index / full) |\n");
    out.push_str("|---|---|---|---|\n");
    for entry in &documented {
        out.push_str(&format!(
            "| `{}` | `{}` | `{}` | ~{}k |\n",
            entry.directory,
            entry.index_filename(),
            entry.bundle_filename(),
            estimated_tokens(entry.api_bytes) / 1000
        ));
    }
    out.push('\n');

    if !undocumented.is_empty() {
        out.push_str("## Crates with NO generated documentation\n\n");
        out.push_str(&format!(
            "These {} crates are part of the workspace but have no API mirror at \
             all, so no file in this bundle describes them. **Do not assume they \
             are absent, empty, or unimportant, and do not invent their APIs.** \
             Name the crate and say its documentation was not provided.\n\n",
            undocumented.len()
        ));
        for entry in &undocumented {
            out.push_str(&format!("- `{}`\n", entry.directory));
        }
        out.push('\n');
    }

    out
}

/// Render one crate's `<crate>.index.md` — the middle rung of the ladder.
///
/// Reads that crate's `docs/api.md` and condenses it with
/// [`condense_api_markdown`]. Returns `Ok(None)` when the crate has no mirror.
pub fn crate_index_markdown(
    workspace_root: &Path,
    entry: &CrateEntry,
) -> io::Result<Option<String>> {
    let Some(api_relative) = &entry.api_md else {
        return Ok(None);
    };
    let body = fs::read_to_string(workspace_root.join(api_relative))?;

    let mut out = format!("# `{}` — condensed API index\n\n", entry.directory);
    out.push_str(&format!(
        "Package `{}`. **Condensed, not documentation**: module paths and public \
         signatures with at most one line of description each. Doc-comment \
         bodies, examples, valid ranges, units and caveats were stripped to fit \
         a context budget. For behaviour, ask for `{}`.\n\n---\n\n",
        entry.package,
        entry.bundle_filename()
    ));
    out.push_str(&condense_api_markdown(&body));
    Ok(Some(out))
}

/// Reduce one `api.md` body to headings, module paths, public signatures, and a
/// single line of description per item.
///
/// The transform is a line-oriented state machine rather than a Markdown parse:
/// the input is machine-generated by one tool with a stable shape, so a parser
/// would buy nothing and cost a dependency. It is exact about fenced blocks
/// (tracking ``` openers and closers) because that is the one place a naive
/// filter would go wrong — prose containing the word `pub` must not be mistaken
/// for a signature.
pub fn condense_api_markdown(body: &str) -> String {
    let mut out = String::new();
    let mut in_fence = false;
    // Set when a heading has just been emitted, so exactly one following prose
    // line is kept as that item's description.
    let mut want_description = false;

    for line in body.lines() {
        let trimmed = line.trim();

        if trimmed.starts_with("```") {
            in_fence = !in_fence;
            continue;
        }

        if in_fence {
            // Inside a code fence, keep only what declares a public item.
            if is_public_signature(trimmed) {
                out.push_str(trimmed);
                out.push('\n');
            }
            continue;
        }

        if let Some(heading) = condensed_heading(trimmed) {
            out.push_str(&heading);
            out.push('\n');
            want_description = true;
            continue;
        }

        if want_description && !trimmed.is_empty() {
            // One line of description, and only if it is prose -- a table row or
            // a list bullet is structure we are deliberately dropping.
            if !trimmed.starts_with('|') && !trimmed.starts_with('-') && !trimmed.starts_with('*') {
                out.push_str(trimmed);
                out.push('\n');
            }
            want_description = false;
        }
    }

    out
}

/// Whether a line inside a code fence declares a public item worth indexing.
///
/// Accepts `pub` declarations and the visibility-restricted `pub(crate)` family,
/// plus the `impl` lines that give a method block its receiver type. Rejects
/// everything else, including `//` comments and rustdoc-md's `/* ... */` body
/// elisions on their own line.
fn is_public_signature(line: &str) -> bool {
    line.starts_with("pub ") || line.starts_with("pub(") || line.starts_with("impl ")
}

/// Map an `api.md` heading onto its condensed form, or `None` to drop it.
///
/// rustdoc-md's boilerplate headings (`# Crate Documentation`, `# Overview`,
/// `## Modules`) carry no information once the bodies are gone, so they are
/// dropped. Everything else is kept at a fixed depth, because the source
/// headings are flat and their `#` count means nothing (see the module docs).
fn condensed_heading(line: &str) -> Option<String> {
    let rest = line.trim_start_matches('#');
    if rest.len() == line.len() {
        return None; // not a heading
    }
    let text = rest.trim();
    if text.is_empty() {
        return None;
    }
    match text {
        "Crate Documentation" | "Overview" | "Modules" => None,
        _ => Some(format!("\n### {text}\n")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\
# Crate Documentation

**Version:** 0.1.0

**Format Version:** 60

# Module `turbulence`

Turbulence closures for the momentum solver.

Every model implements the trait; dispatch is static, never `dyn`.

| Module | Model | Status |
|---|---|---|
| k_omega_sst | Menter | Implemented |

```rust
pub mod turbulence { /* ... */ }
```

## Struct `KOmegaSSTModel`

Menter's two-equation SST closure.

```rust
pub struct KOmegaSSTModel {
    private_field: f64,
}
```

```rust
impl KOmegaSSTModel {
    pub fn correct(&mut self) { /* ... */ }
}
```
";

    #[test]
    fn boilerplate_headings_are_dropped_but_real_ones_survive() {
        let condensed = condense_api_markdown(SAMPLE);
        assert!(!condensed.contains("Crate Documentation"));
        assert!(!condensed.contains("Format Version"));
        assert!(condensed.contains("Module `turbulence`"));
        assert!(condensed.contains("Struct `KOmegaSSTModel`"));
    }

    #[test]
    fn public_signatures_survive_and_private_fields_do_not() {
        let condensed = condense_api_markdown(SAMPLE);
        assert!(condensed.contains("pub mod turbulence"));
        assert!(condensed.contains("pub struct KOmegaSSTModel"));
        assert!(condensed.contains("pub fn correct"));
        assert!(condensed.contains("impl KOmegaSSTModel"));
        assert!(
            !condensed.contains("private_field"),
            "a non-public field is not part of the API surface"
        );
    }

    #[test]
    fn exactly_one_description_line_is_kept_and_tables_are_dropped() {
        let condensed = condense_api_markdown(SAMPLE);
        assert!(condensed.contains("Turbulence closures for the momentum solver."));
        assert!(
            !condensed.contains("dispatch is static"),
            "only the FIRST prose line is kept"
        );
        assert!(
            !condensed.contains("| k_omega_sst |"),
            "status tables are structure we deliberately drop"
        );
    }

    /// Prose mentioning `pub` outside a fence must not be indexed as a
    /// signature -- the reason fences are tracked exactly rather than filtered
    /// by prefix.
    ///
    /// The first prose line after a heading *is* kept, deliberately, as that
    /// item's one-line description -- so the discriminating case is the
    /// **second** such line, which must be dropped like any other prose however
    /// much it looks like a declaration.
    #[test]
    fn prose_outside_a_fence_is_never_mistaken_for_a_signature() {
        let input = "## Notes\n\nThe description line.\n\npub fn looks_like_a_signature() {}\n";
        let condensed = condense_api_markdown(input);
        assert!(
            condensed.contains("The description line."),
            "the first prose line after a heading is the item's description"
        );
        assert!(
            !condensed.contains("looks_like_a_signature"),
            "prose outside a code fence must be dropped even when it is shaped \
             like a declaration -- only fenced lines are signatures"
        );
    }

    #[test]
    fn condensing_is_a_large_reduction() {
        let condensed = condense_api_markdown(SAMPLE);
        assert!(
            condensed.len() < SAMPLE.len(),
            "condensed {} bytes vs {} original",
            condensed.len(),
            SAMPLE.len()
        );
    }
}
