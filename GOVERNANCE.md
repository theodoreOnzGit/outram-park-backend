# OUTRAM PARK — Contribution Governance

**Version:** Draft 0.1
**Status:** Maintainer policy draft

> **This file is maintained in two repositories** —
> [`outram-park`](https://github.com/theodoreOnzGit/outram-park) and
> [`outram-park-backend`](https://github.com/theodoreOnzGit/outram-park-backend).
> They are intended to be identical. If you find them disagreeing, the copy
> in `outram-park-backend` is the one to trust, and please open an issue.

For the practical "how do I contribute" workflows, see
[`CONTRIBUTING.md`](./CONTRIBUTING.md). This document covers **who decides
what**, and why the project is organised the way it is.

---

## Purpose

OUTRAM PARK is an open-source scientific software ecosystem for reactor
simulation, thermal hydraulics, neutronics, severe accident analysis,
consequence assessment, digital twins, and education.

Although the software is open source, contribution to the core codebase is not
treated as an unrestricted activity.

This project operates in a technically sensitive domain. Contributions can
affect scientific correctness, numerical reliability, safety analysis, and
downstream research claims. OUTRAM PARK therefore uses a conservative
contribution model, intended to encourage useful contributions while protecting:

- scientific integrity,
- software reliability,
- supply-chain security,
- maintainer time,
- verification and validation quality,
- long-term sustainability.

---

## Core principle

**Open source does not mean unreviewed source.**

OUTRAM PARK welcomes ideas, reports, tests, benchmarks, papers, and
collaborations. Direct modification of the main codebase is restricted.

The maintainer does not have capacity to personally audit every line of
external code, and automated AI review is not considered sufficient.

Contributors should assume the project is actively concerned about:

- malicious code injection,
- prompt injection,
- compromised generated code,
- accidental scientific errors,
- unverifiable models,
- undocumented assumptions,
- unreviewable large pull requests,
- maintainability regressions.

None of this is a judgement about any individual contributor. It is the
posture a scientific codebase has to adopt to stay trustworthy.

---

## Relationship to the compliance documents

OUTRAM PARK already carries binding policy in five root-level documents. This
governance document **does not restate or override them** — where a rule lives
there, this file summarises it in a sentence and links out. If the two ever
disagree, **the compliance document wins.**

| Document | What it binds |
|---|---|
| [`RESPONSIBLE_USE.md`](./RESPONSIBLE_USE.md) | Intended and prohibited use, data scope, the V&V stage pipeline |
| [`DATA_POLICY.md`](./DATA_POLICY.md) | What data may and may not be used or referenced, anywhere |
| [`AI_USAGE.md`](./AI_USAGE.md) | Which AI systems the project uses, required human review, restricted inputs |
| [`RESEARCH_INTEGRITY_AND_PROVENANCE.md`](./RESEARCH_INTEGRITY_AND_PROVENANCE.md) | Scientific and software provenance, licence and attribution compliance |
| [`VERIFICATION_AND_VALIDATION.md`](./VERIFICATION_AND_VALIDATION.md) | The project's V&V philosophy and what "verified" and "validated" mean here |

Two rules from those documents that contributors hit immediately:

- **Data scope.** Only open-source data, public literature data, and properly
  licensed public benchmark data may be used or referenced — in source, tests,
  examples, benchmark inputs, docs, figures, issues, or pull requests. Never
  introduce confidential, proprietary, partner, operational-facility, or
  unpublished third-party data. See [`DATA_POLICY.md`](./DATA_POLICY.md).
- **Intended use.** OUTRAM PARK is for education, research, capability building,
  and V&V. It is **not** for reactor operation, licensing decisions,
  safety-critical decision-making, emergency response, or operational digital
  twin deployment. Do not submit contributions framed as authoritative for those
  purposes. See [`RESPONSIBLE_USE.md`](./RESPONSIBLE_USE.md).

---

## Trusted contributor model

OUTRAM PARK distinguishes several kinds of participant. Not all have the same
privileges.

### Users

May use the software, report issues, request features, ask questions, and
provide feedback.

### Issue reporters

May describe bugs, provide reproduction cases, submit failing tests, and
suggest expected behaviour.

### Documentation contributors

May propose improvements to tutorials, examples, explanatory notes, diagrams,
comments, and references. Documentation changes still require review —
especially where they make scientific claims, because a confident wrong
sentence in a tutorial propagates further than a bug.

### Test contributors

May submit tests, benchmarks, and V&V cases. See the V&V section of
[`CONTRIBUTING.md`](./CONTRIBUTING.md) — this is among the most valuable things
an external contributor can offer.

### Trusted contributors

May submit pull requests against selected areas of the codebase. Trusted
contributor status is granted by maintainers, based on demonstrated
reliability, domain competence, communication quality, and respect for project
standards. It is earned, and it is scoped — trust in one subsystem is not
trust in all of them.

### Maintainers

May review code, approve and merge pull requests, run AI-assisted
implementation workflows, reject changes, request additional evidence, and
define release criteria.

Maintainers are responsible for protecting the scientific and engineering
quality of the project.

---

## Maintainer scaling

OUTRAM PARK cannot depend indefinitely on one maintainer. The project intends
to develop a maintainer structure with roles such as:

- thermal hydraulics maintainer,
- neutronics maintainer,
- severe accident maintainer,
- consequence analysis maintainer,
- V&V maintainer,
- documentation maintainer,
- release maintainer,
- security reviewer.

Maintainer authority is earned through sustained contribution and demonstrated
good judgement.

---

## Architecture boundaries

Contributions must respect module ownership. A change in the right place is
reviewable; the same change in the wrong place creates ambiguity that outlives
the contributor.

The authoritative, more detailed statement of scope and naming is
[`docs/ecosystem-naming.md`](./docs/ecosystem-naming.md) and
[`docs/architecture.md`](./docs/architecture.md) in `outram-park-backend`. The
summary below must stay consistent with them.

**Status column:** whether the component exists today. Reserved names are
planned scope, not places to send code yet.

| Component | Owns | Status |
|---|---|---|
| **TUAS** | Boussinesq thermal hydraulics, incompressible flow, natural and forced circulation, molten-salt TH, heat transfer, pipe networks, heat exchangers | Exists (`tuas_boussinesq_solver`) |
| **TAMPINES** | Thermophysical properties, steam tables, equations of state, compressible-flow infrastructure, HEM and future drift-flux / two-fluid / six-equation support, balance-of-plant components | Exists (`tampines`, `tampines-steam-tables`) |
| **NEE SOON** | **Integration across neutronics and nuclear data, and only that domain** — see the note below | Exists (`nee_soon`, scaffold) |
| **BEDOK** | System-level multiphysics coupling — TH and neutronics coupled above 1-D neutronics fidelity but below CFD fidelity; reactor transient simulation, reduced-order multiphysics | Exists (`bedok`) |
| **OUTRAM-FOAM** | OpenFOAM-derived workflows, CFD, high-fidelity multiphysics, GeN-Foam-derived capability | Exists (`outram-foam-*`) |
| **SEMBAWANG** | Severe accident progression — melt behaviour, relocation, vessel failure, MCCI, hydrogen, aerosols, source term. *"What gets released?"* | Reserved name |
| **CHANGI** | Atmospheric dispersion, plume transport, deposition, ground contamination, dose assessment. *"What happens after release?"* | Reserved name |
| **REDHILL** | Groundwater and geological transport, subsurface radionuclide migration, porous-media flow. *"What happens after deposition?"* | Reserved name |

**TUAS does not own** compressible flow, multiphase flow, general steam tables,
or general equation-of-state infrastructure. Those are TAMPINES.

**NEE SOON is an integration crate, not a solver umbrella.** This is a settled
decision and a common misunderstanding, so it is spelled out:

- Nuclear data — cross sections, ENDF processing, resonance reconstruction —
  belongs in `njoy-outram-park-fork`.
- Monte Carlo transport belongs in `outram-mc-libs`, which is *data-free* and
  pulls cross sections from `njoy-outram-park-fork`.
- Point reactor kinetics belongs in `teh-o-prke`.
- NEE SOON *composes* those. It does not reimplement them.
- Coupling that reaches into thermal hydraulics is **BEDOK's**, not NEE SOON's.
  The two are separated by *what* they couple, not by layer: NEE SOON couples
  within neutronics and nuclear data; BEDOK couples across physics.

**CHANGI is scoped to research, education and V&V only**, consistent with the
intended-use limits in [`RESPONSIBLE_USE.md`](./RESPONSIBLE_USE.md). It is not
an emergency-response or operational dose-assessment tool.

BEDOK should not duplicate foundational physics libraries without a clear
architectural reason.

---

## Review burden

Review is real work, and it is the scarce resource in this project.

Maintainers are not obligated to review every external contribution. A
contribution may be rejected or deferred if it imposes excessive review burden
— for example large unstructured patches, unclear scientific justification,
missing tests, poor documentation, unclear architectural fit, bulk AI-generated
code, or changes requiring domain expertise no current maintainer has.

Contributors should make their work easy to review. That is not a courtesy; it
is the main determinant of whether a contribution lands.

Response times are not guaranteed. The project maintains a
[`DEVELOPER_HEALTH_WARNING.md`](./DEVELOPER_HEALTH_WARNING.md) and defines
working hours for maintainer activity; silence outside those hours is
intentional. The corresponding limit on the project's own AI automation is
**opt-in** rather than always-on: it is off by default and the assistant
applies it only when the maintainer turns it on — see the "Working-hours
guardrail" section of [`CLAUDE.md`](./CLAUDE.md).

---

## AI-assisted development

OUTRAM PARK permits and actively uses AI-assisted development tools.
**AI-generated code is not automatically trusted** — it is untrusted draft
material until a human maintainer has reviewed, tested, understood, and
documented it. Nothing is merged merely because it compiles, and for sensitive
changes maintainers may independently re-implement a solution using trusted
local workflows.

Full policy — which systems are used, required human review, restricted inputs,
and publication-disclosure wording — is in [`AI_USAGE.md`](./AI_USAGE.md).

---

## Prompt injection and supply-chain security

The project treats prompt injection and malicious contribution attempts as
realistic risks, not hypotheticals.

Contributions must not include hidden instructions aimed at AI tools,
misleading documentation, obfuscated code, unexplained generated files,
suspicious build scripts, unjustified network calls, unsafe code without
review, or dependencies without clear need.

Maintainers should be cautious reviewing large generated patches, unfamiliar
dependencies, benchmark files from unknown sources, scripts that modify the
environment or trigger external processes, and comments that appear designed to
influence AI tools rather than inform humans.

The project prefers small, inspectable, auditable changes.

Licence and provenance requirements for ported or vendored code are in
[`RESEARCH_INTEGRITY_AND_PROVENANCE.md`](./RESEARCH_INTEGRITY_AND_PROVENANCE.md).
Any new dependency or ported code must remain GPLv3-compatible and must keep its
upstream attribution header.

---

## Paid review and sponsored contribution

The project may consider a sponsored contribution or paid review model for
external contributors, organisations, or hackathon participants.

**This is not a fee to buy acceptance.** Payment, sponsorship, or participation
fees do not guarantee that code will be merged. Such support may fund maintainer
review time, benchmark development, documentation review, V&V assessment,
infrastructure, workshops, hackathons, or educational events.

Acceptance into the codebase remains based on technical merit, scientific
quality, maintainability, and security.

> Payment may support review.
> Payment does not buy merge rights.

---

## Final principle

OUTRAM PARK is open source, but it is not an open dumping ground for code.

The project exists to build trustworthy scientific software.

- Trust is more important than speed.
- Verification is more important than feature count.
- Maintainability is more important than cleverness.

A contribution should leave the ecosystem more reliable, more transparent, and
more useful than before.
