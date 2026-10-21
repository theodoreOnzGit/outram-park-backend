# How kovan finds an artifact's end

```toml
[kovan]
id = "artifact-span-walkthrough"
kind = "lesson_section"
created = "2026-10-06T09:00:00+08:00"
modified = "2026-10-06T09:00:00+08:00"
origin = "ai"

[[relation]]
target = "code:crates/kovan/src/artifact.rs::heading_span"
kind = "related_to"
```

The page title is the header artifact. Each step below is its own `#`
artifact.

## Review: ⚠ AI draft, not reviewed

# Step 1: a fence hides a heading

```toml
[kovan]
id = "step-1-fence-hides-heading"
kind = "walk_step"
created = "2026-10-06T09:05:00+08:00"
modified = "2026-10-06T09:05:00+08:00"
origin = "ai"

[[relation]]
target = "code:crates/kovan/src/artifact.rs::heading_span@L943"
kind = "implements"
commit = "2f2cf7d599"

[[relation]]
target = "paper:synthetic2020example"
kind = "supports"
page = 87
quote = "nominal operating conditions"
```

A `#` at the start of a line inside a fenced block is data, not a
heading, so the span skips fences.

### Code

```rust
if line.trim_start().starts_with("```") {
    in_fence = !in_fence;
    continue;
}
```

## Review: ⚠ AI draft, not reviewed

# Walk: parse to span

```toml
[kovan]
id = "walk-parse-to-span"
kind = "code_walk"
created = "2026-10-06T09:10:00+08:00"
modified = "2026-10-06T09:10:00+08:00"
origin = "human"

[[relation]]
target = "code:crates/kovan/src/artifact.rs::parse_document"
kind = "related_to"

[[relation]]
target = "code:crates/kovan/src/artifact.rs::block_span"
kind = "related_to"
```

### Hops

```text
parse_document -> Artifact { line, level }
block_span -> heading_span
```

# Step 1: open the folder

```toml
[kovan]
id = "recipe-open-folder"
kind = "recipe_step"
created = "2026-10-06T09:15:00+08:00"
modified = "2026-10-06T09:15:00+08:00"
```

Choose the Kovan folder with the file picker.
