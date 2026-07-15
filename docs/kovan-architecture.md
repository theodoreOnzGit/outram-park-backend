# KOVAN architecture

This document sketches how the four KOVAN crates fit together. It is a
placeholder to be expanded as the crates are implemented.

## Dependency graph

```text
kovan-common   (leaf — no internal deps)
     ▲   ▲   ▲
     │   │   │
kovan-literature  kovan-semantics  kovan-codegen
```

- `kovan-common` is the shared vocabulary. Every other crate depends on it and
  nothing else internally (placeholder stage).
- `kovan-literature`, `kovan-semantics`, and `kovan-codegen` are siblings that
  each depend on `kovan-common`. Cross-crate wiring (e.g. semantics linking a
  symbol to a literature document) happens through `kovan-common` types, not
  through direct sibling dependencies, to keep each crate publishable and
  reusable on its own.

## Data flow (target)

```text
PDF ─▶ Markdown ─▶ KovanDocument ─▶ BibTeX ─▶ generated artifacts
                        │
                        ▼
                  KOVAN Semantics ── links symbols/repos/benchmarks ──▶ knowledge maps
                        │
                        ▼
                  KOVAN Codegen ── deterministic numerical-method templates
```

## Source-of-truth rule

The Rust struct (`KovanDocument`, `KovanSymbol`, …) is authoritative. BibTeX,
TOML, and Markdown metadata are **generated** from it and must never be treated
as the canonical record.

## Platform constraints

Every crate must compile and run on Linux, Windows, macOS, and Android (Termux).
Keep dependencies pure-Rust and avoid system libraries, GPUs, Docker, and any
assumption that an AI assistant or cloud service is available at runtime.
