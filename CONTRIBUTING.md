# Contributing to OUTRAM PARK

**Version:** Draft 0.1
**Status:** Maintainer policy draft

> **This file is maintained in two repositories** —
> [`outram-park`](https://github.com/theodoreOnzGit/outram-park) and
> [`outram-park-backend`](https://github.com/theodoreOnzGit/outram-park-backend).
> They are intended to be identical. If you find them disagreeing, the copy in
> `outram-park-backend` is the one to trust, and please open an issue.

This document is the practical guide: what to send, and in what shape. For
**who decides what** — trust tiers, maintainer roles, module ownership, review
policy — see [`GOVERNANCE.md`](./GOVERNANCE.md).

---

## Read this first

OUTRAM PARK is a scientific codebase in a technically sensitive domain. A pull
request here is best understood as **an application for integration**, not a
drive-by improvement. Pull requests are not automatically welcomed merely
because the repository is open source.

The single most important thing to know:

> **For bug fixes, we would rather have your failing test than your patch.**

That is not a brush-off. A well-designed failing test is a complete,
high-value contribution — often more valuable than the fix itself.

Before contributing anything, please skim
[`RESPONSIBLE_USE.md`](./RESPONSIBLE_USE.md) and
[`DATA_POLICY.md`](./DATA_POLICY.md). Two rules bite immediately:

- **Only open data.** Open-source, public literature, or properly licensed
  public benchmark data — in code, tests, examples, docs, figures, issues, and
  pull requests alike. Never confidential, proprietary, partner, operational,
  or unpublished third-party data.
- **Intended use is education, research, capability building, and V&V.** Not
  reactor operation, licensing, safety-critical decisions, emergency response,
  or operational digital twins. Don't frame contributions as authoritative for
  those.

---

## 1. Bug reports

Bug reports are welcome, and they are the easiest way to help.

A good report includes:

- a clear description of the problem,
- expected behaviour,
- observed behaviour,
- minimal reproduction steps where possible,
- relevant input conditions,
- relevant output values,
- numerical evidence where applicable.

Focus on the problem rather than proposing a large code change.

---

## 2. Bug fixes

**Direct pull requests for bug fixes are generally not accepted.**

Instead, submit:

1. a description of the bug,
2. a minimal failing test,
3. Rust unit or integration tests that should pass once it is fixed,
4. any scientific or numerical reasoning justifying the expected result.

A maintainer then decides how to implement the fix.

This exists because a bug fix can introduce subtler problems than the original
bug — particularly in numerical code, where a change that makes one case pass
can quietly degrade a regime nobody is testing.

**Preferred:**

```text
Problem report + failing test + expected behaviour
```

**Not:**

```text
External patch directly modifying production code
```

### Workflow

```text
Contributor
  1. Reports the bug
  2. Provides a minimal reproduction
  3. Provides a failing Rust test
  4. Explains the expected behaviour

Maintainer
  5. Reviews the report
  6. Confirms or rejects the issue
  7. Implements via a trusted workflow
  8. Runs the test suite
  9. Adds a regression test
 10. Merges if appropriate
```

---

## 3. New features

New features are **high-trust contributions**, normally accepted only from
trusted contributors, maintainers, or approved collaborators (see
[`GOVERNANCE.md`](./GOVERNANCE.md)).

A new feature should be accompanied by a technical write-up strong enough to
become an arXiv-style preprint, internal report, or formal technical note,
describing:

- the physical model,
- governing equations,
- numerical method,
- assumptions,
- limitations,
- verification cases,
- validation status, if applicable,
- references,
- expected integration points in OUTRAM PARK,
- test strategy,
- examples.

**Preferred:**

```text
Paper-quality technical justification + code + tests
```

**Not:**

```text
Code first, explanation later
```

### Paper-first workflow

```text
Contributor
  1. Discusses the feature with maintainers
  2. Provides a technical note or paper-style justification
  3. Defines the physical model
  4. Defines assumptions
  5. Defines a verification plan
  6. Provides a prototype or reference implementation if appropriate

Maintainers
  7. Review the scientific basis
  8. Review the software architecture
  9. Decide the ownership boundary
 10. Decide whether the feature belongs in OUTRAM PARK at all
 11. Review the implementation
 12. Require tests and documentation
 13. Merge only if maintainable
```

This is deliberately more demanding than a conventional software project.
OUTRAM PARK is not collecting code; it is building a scientific software
ecosystem.

> If it cannot be explained in a technical note, it is not ready for the core
> codebase.

Features must also fit the module ownership described in
[`GOVERNANCE.md`](./GOVERNANCE.md) — a feature that blurs the boundary between
two crates will be sent back regardless of its quality.

---

## 4. Verification and validation contributions

**V&V contributions are strongly encouraged and may be worth more than a new
feature.** The project prefers a smaller set of well-verified models over a
larger set of poorly documented ones.

V&V contributions must be **traceable**. Include:

- benchmark description,
- source of the reference data,
- experimental or analytical basis,
- input files,
- expected outputs,
- tolerance criteria,
- comparison plots or tables where appropriate,
- explanation of any discrepancies.

Provenance is mandatory, not decorative: source, author or organisation,
publication or dataset title, licence and access terms, URL or DOI, date
accessed, and any digitisation steps and assumptions. See
[`RESEARCH_INTEGRITY_AND_PROVENANCE.md`](./RESEARCH_INTEGRITY_AND_PROVENANCE.md).

Two project rules that apply to every V&V contribution — see
[`VERIFICATION_AND_VALIDATION.md`](./VERIFICATION_AND_VALIDATION.md):

- **Document methodology *and* results.** A V&V test whose documentation says
  what it does but not what it produced is incomplete. State the measured
  numbers with uncertainty, the date, and the interpretation.
- **Never report a result that was not actually produced by running the check**,
  and never describe unverified functionality as working.

---

## 5. Documentation

Documentation contributions are welcome — tutorials, examples, explanatory
notes, diagrams, comments, references.

Documentation still gets reviewed, particularly where it makes scientific
claims. A confident wrong sentence in a tutorial travels further than a bug.

Every `README.md` must render correctly on GitHub. Math is allowed via
MathJax, but keep to a conservative subset: no matrix or `cases` environments,
no `\boxed`, no Unicode Greek inside math. Validate before submitting:

```bash
pandoc -f gfm+tex_math_dollars -t html --mathml README.md > /dev/null
```

Exit 0 with no warnings means the math converted.

---

## Pull request policy

Grounds for rejection include:

- unclear purpose,
- no tests,
- no documentation,
- no physical explanation,
- no verification rationale,
- excessive scope,
- hidden generated code,
- unexplained AI-generated logic,
- poor error handling,
- excessive use of `unwrap()`,
- dimensional inconsistency,
- unclear units,
- insufficient traceability,
- changes too large to review safely.

### Code conventions

Contributions to the Rust codebase should follow the workspace conventions:

- **Build and test in release mode** — `cargo test --release`.
- **No trait objects for dispatch.** The set of physics models is closed and
  known at compile time; use enums, so adding a variant is a compile error at
  every match site rather than a runtime surprise.
- **No `Box<T>`, no lifetime parameters in structs.** Own by value or share
  with `Arc<T>`.
- **Keep `uom` on public signatures** where the crate uses it. Type-level unit
  checking is a primary safety net; do not strip it for convenience.
- **Never loosen a tolerance to make a test pass.** If a V&V test fails, the
  model or the boundary detection is wrong, not the tolerance.
- **Document every public item**, including what physical quantity it
  represents, valid ranges, and units — spelled out in prose even when `uom`
  enforces them.

---

## AI-assisted development

The project permits and actively uses AI-assisted tools, and you may use them.
Disclose AI assistance where appropriate.

**AI output is untrusted draft material until a human has reviewed it.** It may
contain subtle bugs, hallucinated APIs, unit errors, incomplete algorithms, or
confident but incorrect scientific reasoning. Nothing is merged because it
compiles. Understand what you submit — a contribution you cannot explain will
not be accepted.

Do not include hidden instructions aimed at AI tools, obfuscated code,
unexplained generated files, or comments designed to influence a reviewing
agent rather than inform a human. See [`AI_USAGE.md`](./AI_USAGE.md) and the
prompt-injection section of [`GOVERNANCE.md`](./GOVERNANCE.md).

---

## What makes a contribution likely to be accepted

**More likely:** small, well documented, well tested, scientifically justified,
architecturally clean, traceable to references, accompanied by verification
cases, understandable by future maintainers, consistent with project style.

**Less likely:** large, rushed, undocumented, generated without understanding,
scientifically vague, difficult to review, dependent on unnecessary libraries,
full of hidden assumptions, inconsistent with module ownership.

---

## Contributor expectations

Contributors are expected to communicate clearly, respect maintainer time,
submit small changes, document assumptions, include tests, avoid overclaiming,
disclose AI assistance where appropriate, understand their own contribution,
and accept maintainer decisions.

Contributors should **not** expect immediate review, a guaranteed merge, free
consulting, maintainers to debug incomplete work, or acceptance on the grounds
that effort was spent.

Review capacity is the project's scarce resource, and response times are not
guaranteed. See [`DEVELOPER_HEALTH_WARNING.md`](./DEVELOPER_HEALTH_WARNING.md);
quiet periods are deliberate.

---

## Rules of thumb

**Bug fixes**

```text
Submit the problem and the test.
Let maintainers decide the implementation.
```

**New features**

```text
Submit the paper-quality explanation, tests, and code.
Expect scientific review.
```

**Major new models**

```text
If it cannot be explained in a technical note,
it is not ready for the core codebase.
```
