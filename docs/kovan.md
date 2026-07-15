# KOVAN – Project Instructions for Claude Code

## Project Overview

KOVAN is the knowledge layer of the Outram Park ecosystem.

KOVAN consists of:

- kovan-literature
- kovan-semantics
- kovan-codegen
- kovan-common

Mission:

> Knowledge without hallucination.

KOVAN is intended to help engineers understand literature, software, benchmarks, validation evidence, and numerical methods using deterministic tooling.

KOVAN is not an AI coding assistant.

KOVAN must remain useful:

- offline
- on Android
- on Linux
- on Windows
- on macOS

without requiring cloud services.

---

# Design Principles

## Deterministic First

Prefer:

```text
Typed Rust Data
    >
Markdown
    >
Generated Artifacts
```

over:

```text
Databases
Search Engines
Vector Stores
LLMs
Cloud Services
```

All generated information should be:

- inspectable
- reviewable
- version controllable
- human-readable

---

## Local First

KOVAN should operate locally.

Avoid dependencies on:

- cloud APIs
- hosted services
- SaaS products

A user should be able to clone KOVAN and perform all core operations offline.

---

## Android First

Android is a first-class platform.

Target environments:

- Linux
- Windows
- macOS
- Android (Termux)

Avoid assumptions that:

- Claude Code is available
- GPUs are available
- Docker is available

---

# Workspace Layout

Create a Rust workspace.

```text
kovan/
├── Cargo.toml

├── kovan-common/
├── kovan-literature/
├── kovan-semantics/
└── kovan-codegen/
```

All crates should include:

- rustdoc
- tests
- examples
- CI-ready structure

---

# KOVAN Common

Purpose:

Shared types.

Examples:

```rust
KovanDocument
KovanSymbol
KovanRepository
KovanCorrelation
KovanBenchmark
KovanValidationCase
```

Do not duplicate core types across crates.

---

# KOVAN Literature

## Purpose

KOVAN-Literature is the nuclear engineering knowledge archive.

Responsibilities:

- papers
- reports
- standards
- benchmarks
- manuals
- reference material

---

## Storage Layout

```text
kovan-literature/

├── open/
│   ├── papers/
│   ├── reports/
│   ├── standards/
│   └── benchmarks/
│
├── proprietary/
│   ├── papers/
│   ├── reports/
│   ├── standards/
│   └── benchmarks/
│
├── generated/
│   ├── markdown/
│   ├── bibtex/
│   └── assets/
│
└── src/
```

---

## Literature Workflow

Canonical workflow:

```text
PDF
    ↓
Markdown
    ↓
KovanDocument
    ↓
BibTeX
    ↓
Generated Knowledge Artifacts
```

---

## Canonical Representation

The Rust struct is the source of truth.

Everything else is generated from it.

Do not make:

- BibTeX
- TOML
- Markdown metadata

authoritative.

Only the Rust struct is authoritative.

---

## Canonical Document Type

```rust
pub struct KovanDocument {
    pub id: String,
    pub slug: String,

    pub visibility: Visibility,
    pub document_type: DocumentType,

    pub title: String,
    pub authors: Vec<Author>,

    pub abstract_text: String,

    pub year: Option<u32>,
    pub doi: Option<String>,

    pub journal: Option<String>,
    pub institution: Option<String>,
    pub publisher: Option<String>,

    pub keywords: Vec<String>,
    pub tags: Vec<String>,

    pub source_url: Option<String>,

    pub related_symbols: Vec<String>,
    pub related_repositories: Vec<String>,
    pub related_benchmarks: Vec<String>,

    pub markdown_body: String,
}
```

---

## Open vs Proprietary

### Open

Contains redistributable content.

Examples:

- NRC reports
- public regulatory documents
- arXiv papers
- open access journal papers
- public theses

May be committed.

---

### Proprietary

Contains user-owned content.

Examples:

- textbooks
- proprietary reports
- paywalled literature

Must remain local.

Must not be committed.

Support:

```gitignore
proprietary/
```

workflows.

---

## PDF Processing

Pipeline requirements:

- PDF import
- Markdown generation
- Metadata extraction
- BibTeX generation
- Asset extraction

Target Markdown size:

```text
≤ 30 pages
```

per Markdown document.

Split larger documents as needed.

---

# KOVAN Semantics

## Purpose

KOVAN-Semantics is a repository understanding engine.

Its goal is to understand software repositories and connect them to engineering knowledge.

---

## Responsibilities

Generate:

```text
Repository Summaries
Symbol Catalogues
Dependency Graphs
Module Graphs
Validation Graphs
Literature Links
Knowledge Maps
```

---

## Important Philosophy

KOVAN does not attempt to reimplement compilers.

KOVAN should consume semantic information from mature language tooling whenever possible.

---

## Language Support

### Rust

Preferred tooling:

```text
rust-analyzer
```

Use semantic information where available.

---

### C++

Preferred tooling:

```text
clangd
libclang
```

Target:

```text
OpenFOAM
```

and related projects.

---

### Python

Preferred tooling:

```text
Pyright
Jedi
```

Target:

```text
OpenMC
```

and supporting tools.

---

### Fortran

Preferred tooling:

```text
fortls
```

Target:

```text
NJOY
```

and related scientific software.

---

## Tree-Sitter Policy

Tree-sitter is not a core dependency.

Use official semantic tooling first.

Only consider Tree-sitter if:

- no suitable semantic tooling exists
- implementation complexity is lower than alternatives

Tree-sitter should be a last resort.

---

## Repository Targets

Primary targets include:

- TUAS
- TAMPINES
- BOON LAY
- Outram Park ecosystem repositories
- OpenFOAM
- OpenMC
- NJOY
- DWSIM (future support)

---

## Shared Semantic Model

KOVAN should normalize language-specific information into common structures.

Examples:

```rust
KovanSymbol
KovanModule
KovanRepository
KovanValidationCase
```

Avoid building a giant universal AST.

Represent semantic facts instead.

---

## Outputs

Prefer generated Markdown artifacts.

Examples:

```text
repository-summary.md
symbols.md
validation-links.md
dependency-graph.md
```

Avoid hidden databases whenever possible.

---

# KOVAN Codegen

## Purpose

KOVAN-Codegen is a deterministic engineering code generation framework.

It eliminates boilerplate.

It does not generate speculative code.

It does not function as an AI coding assistant.

---

## Core Philosophy

Generate:

- known algorithms
- known engineering patterns
- known numerical methods

Do not generate arbitrary software.

---

## Numerical Methods

### Root Finding

Implement patterns for:

- Bisection
- Regula Falsi
- Illinois
- Pegasus
- Secant
- Newton-Raphson
- Brent

---

### Linear Solvers

Implement patterns for:

- Jacobi
- Gauss-Seidel
- SOR
- Conjugate Gradient
- BiCGSTAB
- GMRES
- LU
- QR
- Cholesky

---

### Nonlinear Solvers

Implement patterns for:

- Newton
- Quasi-Newton
- Broyden
- Trust Region

---

### ODE Solvers

Implement patterns for:

- Euler
- RK2
- RK4
- Dormand-Prince
- Backward Euler
- Crank-Nicolson

---

### PDE Infrastructure

Implement patterns for:

- finite volume methods
- finite difference methods
- discretization helpers
- boundary condition scaffolds

---

## Nuclear Engineering Templates

Future templates may include:

- point kinetics
- thermal hydraulic models
- heat exchanger models
- validation harnesses
- benchmark scaffolds
- reactor simulation components

---

## Macro Support

Use:

```text
derive macros
attribute macros
procedural macros
declarative macros
build.rs generation
```

where appropriate.

---

## Engineering Pattern Library

Long-term goal:

Create a reusable repository of scientific computing patterns.

Priority order:

```text
Correctness
    >
Traceability
    >
Maintainability
    >
Performance
    >
Convenience
```

---

# Integration Vision

Long-term architecture:

```text
KOVAN Literature
        ↓
KOVAN Semantics
        ↓
KOVAN Codegen
```

Workflow example:

```text
Paper
    ↓
Equation
    ↓
Correlation
    ↓
Implementation
    ↓
Validation
```

with complete provenance.

---

# Agent Roles

Claude Code should parallelize work across specialized agents.

---

## Agent A

Workspace Architecture

Responsibilities:

- Rust workspace
- crate layout
- CI layout
- tooling

---

## Agent B

Core Types

Responsibilities:

- KovanDocument
- KovanSymbol
- shared models

---

## Agent C

Literature Pipeline

Responsibilities:

- PDF import
- Markdown generation
- metadata extraction

---

## Agent D

BibTeX Pipeline

Responsibilities:

- citation generation
- bibliography support

---

## Agent E

Rust Semantics

Responsibilities:

- rust-analyzer integration
- repository understanding

---

## Agent F

C++ Semantics

Responsibilities:

- clang integration
- OpenFOAM repository understanding

---

## Agent G

Python Semantics

Responsibilities:

- OpenMC support
- semantic extraction

---

## Agent H

Fortran Semantics

Responsibilities:

- NJOY support
- fortls integration

---

## Agent I

Code Generation

Responsibilities:

- algorithm templates
- macro framework
- engineering patterns

---

## Agent J

Android Compatibility

Responsibilities:

- Termux support
- mobile constraints
- cross-platform validation

---

## Agent K

Documentation

Responsibilities:

- rustdoc
- architecture documentation
- examples
- developer guides

---

# Non-Goals

KOVAN should not become:

- a cloud platform
- a hosted service
- an agentic coding system
- a repository modification agent
- an autonomous pull request generator

The focus is:

```text
Knowledge
Understanding
Traceability
Engineering Reproducibility
```

---

# Success Criteria

Phase 1 is successful if:

- Workspace builds.
- All crates compile independently.
- KovanDocument exists.
- Open/proprietary literature storage exists.
- PDF → Markdown pipeline exists.
- BibTeX generation exists.
- Semantic adapters are scaffolded.
- Numerical-method codegen is scaffolded.
- Android compatibility is considered.
- Documentation exists.
- Tests exist.

Mission:

> Build an open, reproducible, engineering-focused knowledge ecosystem for reactor simulation, validation, literature management, and scientific software development.

---

## Scaffold status (placeholder stage)

KOVAN lives as **member crates of the `outram-park-backend` workspace** under
`crates/kovan-*` (not a standalone workspace). Seven crates, all building and
tested, wired to the Rust ecosystem where it is safe and offline/Android-friendly:

- `kovan-common` — canonical `KovanDocument` + shared types (`Author`,
  `Visibility`, `DocumentType`, `KovanSymbol`, `KovanRepository`,
  `KovanCorrelation`, `KovanBenchmark`, `KovanValidationCase`), all
  `serde`-derived (`serde` / `serde_json` / `toml`).
- `kovan-discovery` — **real** `.gitignore`-aware file discovery via `ignore`
  (the `fd` walker) and **real** regex search via the ripgrep engine
  (`grep-searcher` / `grep-regex` / `grep-matcher`).
- `kovan-literature` — open/proprietary/generated storage tree (`proprietary/`
  gitignored at the repo root), a **real** `pulldown-cmark` Markdown-outline
  helper, a placeholder BibTeX renderer, and `// TODO(kovan)` PDF-import stubs.
- `kovan-semantics` — language-adapter enum (Rust/C++/Python/Fortran) naming its
  language server; a **real** ripgrep-first `rough_definition_scan` built on
  `kovan-discovery`; `catalogue_symbols` stubbed pending the language servers.
  The `ra_ap_*` / `libclang` integrations are deferred (heavy / Android-hostile)
  behind a future target-gated feature.
- `kovan-codegen` — numerical-method catalogue enums (root finders,
  linear/nonlinear/ODE solvers) and a `generate` stub.
- `kovan-cli` (binary `kovan`, `clap`) — **agent-facing** front-end:
  `discover` / `search` / `scan` are wired to real functionality; `methods`
  lists the codegen catalogue.
- `kovan-tui` (binary, `ratatui`) — **human-facing** front-end; a static
  overview screen for now. Desktop scope: `ratatui` is pulled only under
  `cfg(not(target_os = "android"))`, and on Android the binary compiles to a
  stub that redirects to the CLI.

**Verification (2026-07-15):** all seven crates build with **no warnings**,
**14 unit tests pass**, `cargo check --target aarch64-linux-android` is clean for
every kovan crate (tui via its stub path), and the `kovan` CLI runs. Remaining
work is marked `// TODO(kovan)`; nothing not-yet-working is presented as done.

Dependency choices honour the recommended minimal stack (serde/toml,
pulldown-cmark, ignore + ripgrep engine, clap, ratatui) and intentionally avoid
Tree-sitter, SQLite, Tantivy, and vector/LLM layers for the first version.
