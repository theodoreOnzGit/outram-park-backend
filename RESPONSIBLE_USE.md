# Responsible Use Statement

Outram Park, the Open Source Unified TRAnsient Multi-Phase Advanced Reactor Simulation Kit, is an open-source nuclear engineering simulation ecosystem developed for education, research, capability building, and verification and validation.

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
