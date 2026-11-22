# Data Policy

Outram Park uses only open-source data, public literature data, and properly licensed public benchmark data.

This policy applies to all source code, examples, tests, benchmarks, validation cases, documentation, figures, notebooks, scripts, AI-assisted development workflows, and publications associated with Outram Park.

## Allowed Data

Outram Park may use:

- Open-source data
- Public literature data
- Properly licensed public benchmark data
- Public-domain data
- Open-access data
- Synthetic data generated for educational or verification purposes
- Reproducible data derived from published equations, correlations, or benchmark specifications

All data must be traceable to a public or properly licensed source.

## Prohibited Data

Outram Park must not use:

- Confidential data
- Restricted data
- Proprietary data
- Partner or industrial confidential data
- Unpublished research data from other groups
- Operational facility data
- Real-time plant or facility data
- Security-sensitive data
- Safeguards-sensitive data
- System logs
- Credentials
- API keys
- Access tokens
- Private repository secrets
- Internal infrastructure information

Do not include prohibited data in:

- Source code
- Tests
- Examples
- Benchmark inputs
- Validation datasets
- Documentation
- Figures
- Issue reports
- Pull requests
- AI prompts
- AI-generated outputs
- Publications

## AI-Assisted Development

AI tools may only be given public, open-source, or properly licensed data.

Do not provide AI tools or agents with:

- Credentials
- Secrets
- Internal system details
- Confidential or restricted information
- Proprietary datasets
- System logs
- Operational data
- Sensitive infrastructure information

AI-generated data, references, or benchmark values must be independently checked against public sources before use.

## Maintainer exception: published EBR-II benchmark reports (2026-11-10)

The maintainer decided on 2026-11-10 that **numerical data transcribed from the following published reports** may be used in this repository and in `theodoreOnzGit/fast-reactors`, with a citation on every value. In this policy that data counts as public literature and benchmark-specification data:

- ANL-ARC-226 Rev. 1 (Sumner & Wei, 2012), *Benchmark Specifications and Data Requirements for EBR-II Shutdown Heat Removal Tests SHRT-17 and SHRT-45R*;
- ANL-ARC-228 Rev. 1 (Fei, Mohamed & Kim, 2013), *Neutronics Benchmark Specifications for EBR-II Shutdown Heat Removal Test SHRT-45R*;
- IAEA-TECDOC-1819 (2017), *Benchmark Analysis of EBR-II Shutdown Heat Removal Tests*;
- ANL-6864 (Kirn & Loewenstein, 1964), *EBR-II Wet Critical Experiments*;
- CONF-850722-1 (Lentz, Buschman & Smith, 1985), *EBR-II: Twenty Years of Operating Experience*.

Scope and limits:

- **Numbers only:** dimensions, compositions, kinetics parameters, boundary conditions, tabulated measurements and stated uncertainties. Each file or value names the document, table or figure, and page.
- **Not allowed here:** the PDFs, page images, figures and copied prose. These stay in the maintainer's private literature repository. Plot-only data may be digitised, with the digitisation recorded under "Data Provenance".
- **Licence terms:** each report's licence or access terms are recorded where the data lands (e.g. `reference-data/ebr2/README.md`). The ANL reports carry no unlimited-distribution statement. TECDOC-1819 is IAEA copyright with a reuse permission.
- **Not operational data:** these are published benchmark specifications and measurements of tests on a reactor shut down in 1994. Transcribing them is not operational facility data. The "Prohibited Data" list still applies to anything else.
- **AI transcriptions** of these reports are marked as unreviewed until a human has checked them.

## Data Provenance

Each benchmark, validation case, or data-derived example should document:

- Data source
- Author or originating organization
- Publication title or dataset name
- Licence or access terms, where available
- URL or DOI, where available
- Date accessed, where relevant
- Any processing or digitization steps
- Assumptions made
- Known limitations

Recommended file:

```text
References.md
