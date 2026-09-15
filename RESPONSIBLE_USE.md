# Responsible Use Statement

Outram Park, the Open-source Unified TRAnsient Multi-Physics Advanced Reactor simulation Kit, is an open-source nuclear engineering simulation ecosystem developed for education, research, capability building, and verification and validation.

Outram Park is intended to support:

- Reproducible scientific software development
- Public-domain benchmark studies
- Education and outreach
- Verification and validation methodology
- Transparent multiphysics simulation research
- Training of scientific software developers and nuclear engineering practitioners

## Intended Use

Outram Park is intended for education and research use only.

It may be used for:

- Teaching and learning
- Literature-based benchmark reproduction
- Verification studies
- Validation studies using public data
- Numerical-method development
- Scientific software engineering training
- Outreach demonstrations using public and non-sensitive information

## Prohibited or Unsupported Use

Outram Park is not intended for:

- Nuclear facility operation
- Reactor control
- Licensing decisions
- Safety-critical decision-making
- Emergency response
- Safeguards-sensitive analysis
- Security-sensitive analysis
- Real-time plant monitoring
- Operational digital twin deployment
- Use with confidential, restricted, proprietary, operational, or unpublished data

Outram Park outputs must not be treated as authoritative for safety, licensing, operational, regulatory, or emergency-response purposes.

## Data Scope

Outram Park uses only:

- Open-source data
- Public literature data
- Properly licensed public benchmark data
- Publicly reproducible reference cases

Outram Park does not use:

- NUS Confidential data
- NUS Restricted data
- Proprietary data
- Partner or industrial confidential data
- Unpublished research data from other groups
- Operational facility data
- System logs
- Credentials, secrets, tokens, or internal infrastructure information

All benchmark and validation data must be traceable to public sources and documented in the relevant `References.md`, example folder, validation report, or publication.

## AI-Assisted Development

Outram Park may use AI-assisted coding, translation, refactoring, documentation, and test generation.

AI-generated or AI-assisted outputs are treated as untrusted draft material until reviewed.

### What counts as the initial human review (maintainer convention, 2026-09-15)

**Anything the maintainer posts to arXiv constitutes an initial human review of the material it reports.** Preparing a manuscript is where the generated numbers are read, the fitting domains and failure conditions are stated, and the author puts his name to them; posting is the point at which that has happened. Readers are actively invited to scrutinise, reproduce and challenge the result.

This matters most for generated V&V reports. Those files carry a machine-written line saying no human has reviewed them, which is true **as of generation** and is deliberately not edited afterwards — the files are regenerated mechanically and their value is provenance. Where such a report's numbers are carried into a posted paper, the paper is that review, and the stale line in the generated file should be read as a statement about generation time rather than about the numbers' current standing.

Two limits are part of the convention, not caveats bolted onto it:

- arXiv is **not peer review**. Posting establishes authorship of the claims and opens them to scrutiny; it does not confer correctness.
- It satisfies the **human inspection** requirement below and nothing else. Licence provenance review, unit testing, verification against analytical or published references, and validation against benchmarks are separate legs and are unaffected. A posted paper does not make a crate validated, does not move a maturity bar, and does not extend a correlation beyond the domain it was fitted on.

AI-assisted contributions must undergo:

- Human inspection
- Licence provenance review
- Unit testing
- Verification against analytical or published reference cases
- Validation against public-domain benchmarks where applicable
- Documentation of assumptions, limitations, and known errors

AI assistance does not replace engineering judgement, scientific review, or verification and validation.

AI tools and agents must not be provided with:

- Credentials
- API keys
- Access tokens
- Internal system details
- Confidential or restricted data
- Private repository secrets
- System logs
- Production data
- Sensitive infrastructure information

AI agents must not be granted autonomous access to:

- Institutional IT resources
- Credentials
- Production systems
- Sensitive datasets
- Operational infrastructure
- Restricted systems

## Verification and Validation

Outram Park follows a verification-before-optimization philosophy.

The priority order is:

1. Correctness
2. Stability
3. Maintainability
4. Performance

Features should progress through the following stages:

```text
Prototype
→ Unit Tested
→ Integrated
→ Verified
→ Validated
→ Published
