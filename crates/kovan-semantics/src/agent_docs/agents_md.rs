//! The hardcoded `AGENTS.md` uploaded alongside the API documentation.
//!
//! # Why this is hardcoded rather than derived from `CLAUDE.md`
//!
//! The workspace's own `CLAUDE.md` is written for an agent running *inside* the
//! repository. Most of it — the issue tracker, the git hooks, the token-usage
//! trailers, the push policy, the working-hours guardrail — is harness policy a
//! chat agent can neither follow nor act on, and it would consume a large slice
//! of the very context budget this bundle exists to protect.
//!
//! What is reproduced here is the subset that changes **the code the agent
//! writes**: the Rust design rules, the `uom` convention, the documentation
//! standard, the V&V standard, and the scope limits on what this software may
//! be used for.
//!
//! # Keeping it honest
//!
//! [`agents_markdown`] takes the finished [`BundleReport`] so the document can
//! state **what the agent was not given**. An agent that is not told a crate
//! exists will invent its API rather than ask, so the omissions are part of the
//! instructions, not an afterthought.

use super::BundleReport;

/// The workspace rules that govern generated code, verbatim in the uploaded
/// bundle.
///
/// Kept as one `const` so the text is reviewable as a document rather than
/// assembled from fragments. Everything machine-specific — which crates were
/// included, which were omitted, how large the bundle is — is appended by
/// [`agents_markdown`] instead.
const RULES: &str = r#"# Outram Park — how to write code for this workspace

You are advising on **OUTRAM PARK** (Open-source TRAnsient Multi-Phase Advanced
Reactor simulator Kit), a Rust workspace of ~37 crates for nuclear reactor
simulation: thermal hydraulics, neutronics, CFD, thermophysical properties, and
offline digital twins.

Your job is to help scaffold and structure solutions **that match this
workspace's conventions**. Code that ignores the rules below will be rejected in
review even if it compiles and is numerically correct.

---

## Scope limits — read first

This software is for **education, research, capability building, and
verification/validation only**. It is **not** for nuclear facility operation,
reactor control, licensing decisions, safety-critical decision-making, emergency
response, safeguards- or security-sensitive analysis, real-time plant
monitoring, or operational digital-twin deployment. Do not frame any output as
authoritative for those purposes, and do not design toward them.

**Only open-source, published, or properly licensed public data may be used.**
Never introduce confidential, proprietary, partner, unpublished, or operational
facility data, and never credentials, API keys, tokens, or internal
infrastructure details.

**Anything you write is untrusted draft material until a human reviews it.**
Say so. Document assumptions, limitations, and known errors rather than
presenting a first draft as finished. Do not describe unverified functionality
as working, and never report a validation result that was not actually produced
by running the check.

---

## Rust design rules — these are hard rules, not style preferences

### No trait objects. Use enums for dispatch.

Do not use `Box<dyn Trait>`, `&dyn Trait`, or `Arc<dyn Trait>` for dispatch. The
set of physics models (equations of state, turbulence models, numerical schemes,
boundary conditions) is closed and known at compile time, so an enum is correct
and a trait object is not.

Enums give exhaustiveness (a new variant forces every `match` to be updated —
a compile error, not a runtime surprise), zero heap allocation, and working
go-to-definition. Traits are still used, but as a **compiler-enforced contract**
on each concrete struct, never for runtime dispatch.

```rust
// The trait enforces the interface -- the compiler checks every model has it.
pub trait TurbulenceKernel {
    fn div_dev_rho_reff(&self, u: &VolVectorField) -> FvVectorMatrix;
    fn correct(&mut self);
}

// The enum dispatches, without Box or dyn.
pub enum TurbulenceModel {
    Laminar(LaminarModel),
    KOmegaSST(KOmegaSSTModel),
    KEpsilon(KEpsilonModel),
}

impl TurbulenceModel {
    pub fn correct(&mut self) {
        match self {
            Self::Laminar(m) => m.correct(),
            Self::KOmegaSST(m) => m.correct(),
            Self::KEpsilon(m) => m.correct(),
        }
    }
}
```

### No `Box<T>`

Own data by value, or share it with `Arc<T>`. `Box<T>` is justified only for
recursive data structures, which do not appear in this codebase.

### No lifetime parameters

Do not add `'a` to structs, traits, or impl blocks.

| Instead of | Use |
|---|---|
| `&'a FvMesh` in a struct | `Arc<FvMesh>` |
| `&'a f64` or a uom quantity in a struct | own it by value — all uom types are `Copy` |
| `Box<dyn Fn(&'a T) -> U>` | a newtype struct that owns its captured state |
| `&'a Cell` for graph or topology links | `CellId(usize)` — an index into a `Vec` |

### Shared state

Use `Arc<RwLock<T>>` for shared **mutable** simulation state, and `Arc<T>` with
no lock for data that is read-only after construction (mesh topology, lookup
tables, material constants).

Prefer `RwLock` over `Mutex`: `RwLock` allows concurrent reads, where `Mutex`
serialises even read-only access and defeats parallelism during the compute
phase of a timestep.

Do **not** use channels (`mpsc`, `crossbeam`) for simulation state. Channels
suit pipelines where data is produced, consumed and discarded; a timestep loop
is a shared-state pattern — threads compute over non-overlapping regions of the
same fields, then synchronise.

---

## Units: `uom` throughout

Physical quantities are `uom` types, not bare `f64`. A function taking a
temperature takes a `ThermodynamicTemperature`, not "a float in kelvin".

**Give complex `uom` types a named alias.** A reader hovering in their editor
should see `SpecificEnthalpy`, not a raw `Quantity<ISQ<...>, SI<f64>, f64>`.

**Spell the units out in prose anyway**, even though `uom` enforces them — the
doc comment is what a human reads first.

---

## Documentation standard

Every public function, type, trait, and module carries a `///` or `//!` doc
comment answering:

- What **physical quantity** does this compute or represent?
- What are the **valid input ranges** and assumptions?
- What **units** do the parameters represent?

Every module's `lib.rs` / `mod.rs` carries a `//!` comment saying what belongs
in the module **and what does not**.

The governing principle: every public API must be navigable by a Rust developer
using rust-analyzer alone, with no AI assistant and no prior knowledge of the
codebase. If understanding a function requires holding three other modules in
mind at once, the interface is wrong regardless of how correct the physics is.
Do not add type parameters, trait indirection, or macro magic in the name of
generality if it raises the mental load on a human reader.

---

## Verification & validation

The workspace distinguishes them and so must you:

- **Verification** — "is it implemented correctly?"
- **Validation** — "does it represent physical reality well enough for its
  intended purpose?"

A test that checks physics against a reference must document **both the
methodology and the results** in its doc comment:

- **Methodology** — what is computed, the reference or benchmark it is judged
  against, the inputs (geometry, material, data source, tolerances), and the
  pass criterion.
- **Results** — the measured numbers *with uncertainty*, the date they were
  taken, and what the result implies about the model.

A V&V test documenting what it does but not what it produced is incomplete.

Assert **physical invariants explicitly** — second law, mass conservation,
bounded temperatures. A passing suite is only evidence about the properties
someone thought to assert; this workspace has been bitten by exactly that, with
a coolant leaving a core hotter than the solid heating it, sitting unnoticed in
a passing test's recorded output.

---

## Reuse before writing

This is a 40-crate workspace containing ports of several mature codes. **The
prior is "this already exists", not "this needs writing."** Before proposing an
implementation, say what you would search for to check — the domain noun, not
your intended API name. When you propose writing something new, say why the
existing crates do not cover it.

Prefer reuse over porting, and porting over writing from scratch. When porting
logic, cite the reference implementation in the doc comment so the two cannot
drift apart unnoticed.

---

## Build conventions

- Always **release mode**: `cargo build --release`, `cargo test --release`.
- Third-party dependency versions live in the root `[workspace.dependencies]`;
  members inherit with `<dep>.workspace = true`. To change a shared dependency,
  edit the root manifest only.
- Non-GUI library code must compile for Android (`aarch64-linux-android`).
  Anything needing system BLAS/LAPACK, a C/Fortran toolchain, or windowing must
  be behind a target gate. Note Android's `target_os` is `"android"`, not
  `"linux"`. Terminal apps (CLI, `ratatui` TUI) are in scope; only
  `egui`/`eframe`/`wgpu` windowing is exempt.
"#;

/// Render the `AGENTS.md` that ships with a bundle.
///
/// Appends to [`RULES`] a manifest of what the agent actually received and —
/// more importantly — what it did not, drawn from `report`.
pub fn agents_markdown(report: &BundleReport) -> String {
    let mut out = String::from(RULES);

    out.push_str("\n---\n\n## What you have been given\n\n");

    if report.included.is_empty() {
        out.push_str(
            "**No crate's full API documentation is in this bundle** — only the \
             condensed `_INDEX.md`. Treat every signature you see there as a \
             name you may reference, not as an interface you understand in \
             detail.\n\n",
        );
    } else {
        out.push_str(
            "Full public-API documentation, generated from the source by \
             rustdoc, for these crates:\n\n",
        );
        for name in &report.included {
            out.push_str(&format!("- `{name}` — see `{name}.api.md`\n"));
        }
        out.push('\n');
    }

    out.push_str(
        "`_INDEX.md` carries a **condensed** signature index of every crate that \
         has generated documentation: module paths and public signatures, with \
         at most one line of description each. Doc-comment bodies, valid ranges, \
         units and caveats were stripped from it to fit the context budget. Use \
         it to learn what exists and what it is called — never to decide how \
         something behaves.\n\n",
    );

    if !report.missing_api_docs.is_empty() {
        out.push_str("## What you have NOT been given\n\n");
        out.push_str(&format!(
            "These {} crates are part of this workspace but have **no generated \
             API documentation at all**, so they appear nowhere in this bundle — \
             not even in the index:\n\n",
            report.missing_api_docs.len()
        ));
        for name in &report.missing_api_docs {
            out.push_str(&format!("- `{name}`\n"));
        }
        out.push_str(
            "\n**Do not invent APIs for these crates.** If a question needs one \
             of them, say which crate you need and that its documentation was \
             not provided, rather than guessing at type or function names. \
             Guessed names are the single most expensive failure mode here, \
             because they look exactly like knowledge.\n\n",
        );
    }

    out.push_str("## Bundle size\n\n");
    out.push_str(&format!(
        "{} files, {} bytes, **approximately {} tokens** — an estimate at four \
         bytes per token, not a measurement. Generated API markdown tokenizes \
         worse than prose, so the true figure is likely higher.\n",
        report.files.len(),
        report.total_bytes(),
        report.total_estimated_tokens(),
    ));

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn report_with(included: &[&str], missing: &[&str]) -> BundleReport {
        let mut files = BTreeMap::new();
        files.insert("AGENTS.md".to_string(), 100_u64);
        BundleReport {
            files,
            included: included.iter().map(|s| s.to_string()).collect(),
            indexed: included.iter().map(|s| s.to_string()).collect(),
            missing_api_docs: missing.iter().map(|s| s.to_string()).collect(),
        }
    }

    /// The rules that change generated code must actually be present — this is
    /// the whole reason the file is uploaded.
    #[test]
    fn the_rules_that_govern_generated_code_are_present() {
        let md = agents_markdown(&report_with(&[], &[]));
        for required in [
            "Box<dyn Trait>",
            "No lifetime parameters",
            "Arc<RwLock<T>>",
            "uom",
            "Verification",
            "Validation",
            "release mode",
            "aarch64-linux-android",
        ] {
            assert!(md.contains(required), "AGENTS.md must mention {required}");
        }
    }

    /// Harness policy must NOT leak in: it is inapplicable to a chat agent and
    /// would spend the context budget this bundle exists to protect.
    #[test]
    fn harness_policy_is_excluded() {
        let md = agents_markdown(&report_with(&[], &[]));
        for excluded in [
            "kopi-beans",
            "bn ready",
            "git push",
            "API-Usage",
            "working-hours",
        ] {
            assert!(
                !md.contains(excluded),
                "AGENTS.md must not carry harness policy ({excluded})"
            );
        }
    }

    /// **The omissions are part of the instructions.** An agent not told a
    /// crate exists will invent its API rather than ask, and a guessed name
    /// looks exactly like knowledge.
    #[test]
    fn the_omitted_crates_are_named_and_the_agent_is_told_not_to_guess() {
        let md = agents_markdown(&report_with(
            &["tampines-steam-tables"],
            &["bedok", "raffles"],
        ));
        assert!(md.contains("`bedok`"));
        assert!(md.contains("`raffles`"));
        assert!(md.contains("Do not invent APIs"));
        assert!(md.contains("tampines-steam-tables.api.md"));
    }

    /// The size line must call the figure an estimate, never a token count.
    #[test]
    fn the_size_is_reported_as_an_estimate() {
        let md = agents_markdown(&report_with(&[], &[]));
        assert!(md.contains("an estimate"));
        assert!(md.contains("not a measurement"));
    }

    #[test]
    fn an_empty_selection_is_stated_rather_than_left_blank() {
        let md = agents_markdown(&report_with(&[], &[]));
        assert!(md.contains("No crate's full API documentation is in this bundle"));
    }
}
