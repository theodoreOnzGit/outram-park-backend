# References — Zircaloy elastic constants validation material

Provenance record for the validation material in this directory, per the
workspace `CLAUDE.md` data-provenance rule and `DATA_POLICY.md`
("Data Provenance").

**Scope of this file.** Sources for the elastic constants of Zircaloy — Young's
modulus `E`, shear modulus `G`, and Poisson's ratio `nu` — as functions of
temperature and, where available, cold work, oxygen content and fast neutron
fluence. These support
[`poisson_ratio_zircaloy.md`](./poisson_ratio_zircaloy.md), the validation case
for `PoissonRatioModel::MatproZircaloy`.

**Data-scope compliance.** Every entry below is open-source software, a publicly
released government-sponsored technical report, or an item of published
open-literature. Nothing here is NUS Confidential/Restricted, proprietary,
partner or industrial confidential, unpublished third-party research data, or
operational facility data.

---

## READ THIS FIRST — retrieval status and what is *not* in this file

This file records **bibliographic provenance**. With one exception (entry S-1,
the upstream OFFBEAT source, which was retrieved and read in full) **no
full text was retrieved, and consequently no numerical data from any of the
cited literature has been transcribed anywhere in this directory.**

The session that compiled this file had outbound network access restricted to
code-hosting domains by an egress policy. Direct retrieval of every literature
host was refused at the proxy (`osti.gov`, `nrc.gov`, `pnnl.gov`,
`digital.library.unt.edu`, `inis.iaea.org`, `sciencedirect.com`, `link.aps.org`,
`doi.org`, `arxiv.org`, `fast.labworks.org`, `one.oecd.org` — all returned
`403` at the CONNECT stage or to the fetch tool). Bibliographic metadata below
was therefore assembled from **web-search result listings only**.

Each entry is tagged with a confidence level:

| Tag | Meaning |
|---|---|
| **VERIFIED** | Retrieved and read directly in this session. |
| **IDENTIFIER-DERIVED** | Key fields decoded from a literal, unambiguous identifier that appeared in a search-result URL (an Elsevier PII, an APS DOI, an ADS bibcode, an OSTI ID). These are reliable because the identifier itself encodes journal, volume, page and year. |
| **SEARCH-METADATA** | Taken from a search-result summary. Adequate to locate the document; **must be checked against the document itself before being cited in a publication.** |
| **GAP** | Wanted but not obtained. Stated explicitly rather than guessed. |

**No number appearing anywhere in this directory was taken from any of these
publications.** Every numerical value in
[`poisson_ratio_zircaloy.md`](./poisson_ratio_zircaloy.md) is either printed by
this crate's own code and transcribed, or derived algebraically from the
correlation coefficients that are themselves transcribed from the upstream
source in entry S-1. Where a measured comparison value is needed, the table
cell says **GAP — not sourced**, and does not contain a number.

Date of this retrieval attempt: **2026-08-05**.

---

## S — Software / code sources

### S-1. OFFBEAT upstream source — **VERIFIED**

- **Author / organization:** A. Scolaro (main author), E. Brunetto, C. Fiorina
  — EPFL, Laboratory for Reactor Physics and Systems Behaviour; I. Clifford —
  Paul Scherrer Institut (PSI). Attribution taken verbatim from the
  `\mainauthor` / `\contribution` blocks of the source files.
- **Title / project:** OFFBEAT (foam-for-nuclear), fuel-performance code on
  OpenFOAM.
- **Version:** git commit `80e84450a115b0c411e1bfa5d166379f6bf6c084`
  (committed 2026-01-05).
- **Licence:** GPL-3.0-or-later (OpenFOAM licence header carried on each file).
- **URL:** <https://gitlab.com/foam-for-nuclear/offbeat>
- **Date accessed:** 2026-08-05 (cloned and read).
- **Files used:**
  - `offbeatLib/materials/materialModel/thermoMechanicalPropertiesModels/PoissonRatio/PoissonRatioMatproZy.{C,H}`
  - `offbeatLib/materials/materialModel/thermoMechanicalPropertiesModels/YoungModulus/YoungModulusMatproZy.{C,H}`
  - `offbeatLib/materials/materialModel/thermoMechanicalPropertiesModels/PoissonRatio/constantPoissonRatioZy.{C,H}`
  - `offbeatLib/materials/materialModel/zircaloy.C`
- **What was taken from it:** the correlation coefficients `par1`–`par10`, the
  branch structure (alpha below 1073 K, linear interpolation to 1273 K, beta
  above), the stated validity range, and the fact that upstream's default
  Zircaloy material selects the *constant* Poisson model. See
  [`poisson_ratio_zircaloy.md`](./poisson_ratio_zircaloy.md) § "Upstream
  behaviour" for the exact lines.
- **Processing / assumptions:** none. Coefficients were read directly from the
  constructor initialiser list and compared character-by-character with this
  crate's transcription.
- **Known limitations:** upstream cites its provenance only as "MATPROv11". It
  reproduces **no** experimental references, no fit uncertainty, and no
  statement of the data base underlying the fit. Everything in section R below
  therefore had to be chased independently, and the link from MATPRO's
  published equations to upstream's `par1`–`par10` is **not yet confirmed
  against MATPRO itself** (see GAP-1).

---

## R — Primary correlation source (MATPRO)

### R-1. MATPRO-Version 11 — **SEARCH-METADATA** (full text not retrieved)

- **Authors / editors:** D. L. Hagrman and G. A. Reymann (compiled and edited).
- **Organization:** EG&G Idaho, Inc., Idaho Falls, Idaho, for the U.S. Nuclear
  Regulatory Commission.
- **Title:** *MATPRO-Version 11: A Handbook of Materials Properties for Use in
  the Analysis of Light Water Reactor Fuel Rod Behavior*.
- **Report numbers:** NUREG/CR-0497; TREE-1280.
- **Date:** February 1979.
- **OSTI identifier:** 6442256.
- **URLs (all refused by the egress policy in this session):**
  - <https://www.osti.gov/biblio/6442256>
  - <https://digital.library.unt.edu/ark:/67531/metadc1205115/> (UNT Digital
    Library mirror of the same OSTI item, full PDF)
- **Access / licence terms:** publicly released NUREG-series contractor report.
  NUREG-series reports prepared for the U.S. NRC are U.S. Government-sponsored
  works and are distributed without access restriction. **The report's own
  distribution statement was not read** and should be checked when the document
  is obtained.
- **Date accessed:** 2026-08-05 — **metadata only; document not retrieved.**
- **Relevance:** the origin of the `E` and `G` correlations ported here. The
  relevant subcodes are named **CELMOD** (Young's modulus) and **CSHEAR**
  (shear modulus).
- **Known limitation / caution:** this crate's doc comments and the upstream
  header cite **"MATPRO-11 (Rev. 2)"**. A revision 2 exists as a separate
  document — see R-2 — and this session could not determine which of the two
  upstream actually transcribed. Do not cite a revision number in a publication
  until that is checked.

### R-2. MATPRO-Version 11 (Revision 2) — **SEARCH-METADATA**

- **Title:** *MATPRO — Version 11 (Revision 2): A Handbook of Materials
  Properties for Use in the Analysis of Light Water Reactor Fuel and Behavior*
  (title as catalogued).
- **Report numbers:** NUREG/CR-0497 Rev. 2; TREE-1280 Rev. 2.
- **Catalogue records:** WorldCat OCLC 43550391; University of
  Wisconsin–Madison Libraries catalogue record 9910190546702121.
- **Date accessed:** 2026-08-05 — **catalogue metadata only.**
- **GAP:** publication date, authorship and an accessible full-text location for
  *this revision specifically* were not established.

### R-3. SCDAP/RELAP5 MATPRO library — **SEARCH-METADATA**

- **Title:** *SCDAP/RELAP5/MOD 3.3 Code Manual: MATPRO — A Library of Materials
  Properties for Light-Water-Reactor Accident Analysis*.
- **Report number:** NUREG/CR-6150, Volume 4, Revision 2.
- **Organization:** prepared for the U.S. Nuclear Regulatory Commission.
- **URL:** <https://www.nrc.gov/reading-rm/doc-collections/nuregs/contract/cr6150/v4/index>
- **Earlier revision (MOD 3.1, Volume 4):** OSTI 100327; UNT Digital Library
  mirror `ark:/67531/metadc622904`.
- **Access terms:** publicly released NUREG-series report; not verified from the
  document.
- **Date accessed:** 2026-08-05 — **metadata only.**
- **Relevance:** the later, more accessible restatement of the same MATPRO
  correlations, and the version most likely to state the validity range and the
  expected standard error of CELMOD/CSHEAR in a form that can be quoted.

### R-4. NUREG/CR-7024 — MATPRO / FRAPCON correlation comparison — **SEARCH-METADATA**

- **Authors:** W. G. Luscher and K. J. Geelhood.
- **Organization:** Pacific Northwest National Laboratory (PNNL).
- **Title:** *Material Property Correlations: Comparisons between FRAPCON-3.4,
  FRAPTRAN 1.4, and MATPRO*.
- **Report numbers:** NUREG/CR-7024; PNNL-19417.
- **Date:** manuscript completed August 2010; published March 2011.
- **OSTI identifier:** 1030897. **UNT mirror:** `ark:/67531/metadc843763`.
- **Revision 1** (FRAPCON-3.5 / FRAPTRAN-1.5): NRC ADAMS accession
  `ML14296A063`.
- **URLs (refused in this session):**
  <https://www.pnnl.gov/main/publications/external/technical_reports/PNNL-19417.pdf>,
  <https://www.nrc.gov/docs/ML1110/ML11101A012.pdf>,
  <https://nrc.gov/reading-rm/doc-collections/nuregs/contract/cr7024>
- **Access terms:** publicly released NUREG-series report; not verified from the
  document.
- **Date accessed:** 2026-08-05 — **metadata only.**
- **Relevance — this is the single highest-value document to obtain.** It is a
  model-to-model *and* model-to-data comparison that explicitly covers the
  cladding elastic-modulus correlations, and it is the most likely accessible
  place to find (a) MATPRO's CELMOD/CSHEAR equations restated, (b) the stated
  fit uncertainty, and (c) a digitised plot of the underlying measurements
  against which this validation case can finally be closed.

---

## E — Experimental data base (the measurements to validate against)

These are the candidate benchmark datasets. **None of their data has been
transcribed** — see the retrieval-status note above.

### E-1. Fisher & Renken (1964), single-crystal elastic moduli — **IDENTIFIER-DERIVED**

- **Authors:** E. S. Fisher and C. J. Renken.
- **Title:** *Single-Crystal Elastic Moduli and the hcp → bcc Transformation in
  Ti, Zr, and Hf*.
- **Journal:** Physical Review **135** (2A), pages A482–A494 (1964).
- **DOI:** `10.1103/PhysRev.135.A482` (literal in the APS URL returned by
  search: <https://link.aps.org/doi/10.1103/PhysRev.135.A482>).
- **Licence / access:** American Physical Society, subscription/copyrighted.
  Bibliographic metadata only is reproduced here; no article text or data.
- **Date accessed:** 2026-08-05 — **metadata only; article not retrieved.**
- **Relevance:** the foundational single-crystal elastic-constant measurement
  for zirconium. **Critically, its title states it spans the hcp → bcc
  transformation**, so it is the most likely experimental source of constraint
  in the beta phase — exactly the region where the MATPRO pair fails. Search
  metadata indicates measurements from 4 K to about 1155 K by ultrasonic wave
  interferometry; **this range statement is SEARCH-METADATA and unverified.**
- **Caveat:** single-crystal constants are anisotropic. Deriving a polycrystal
  `E`, `G` and `nu` from them requires an averaging scheme (Voigt / Reuss /
  Hill) and a texture assumption. Any such derivation is a *processing step*
  that must be documented if performed — it is not raw benchmark data.

### E-2. Northwood, London & Bähen (1975), elastic constants of Zr alloys — **IDENTIFIER-DERIVED**

- **Authors:** D. O. Northwood, I. M. London, L. E. Bähen.
- **Title:** *Elastic constants of zirconium alloys*.
- **Journal:** Journal of Nuclear Materials **55** (3), pages 299–310 (1975).
- **DOI:** `10.1016/0022-3115(75)90071-9` (decoded from the Elsevier PII
  `0022311575900719` literal in the search-result URL).
- **ADS bibcode:** `1975JNuM...55..299N` (literal in the ADS URL) — independently
  confirms journal, volume, page and year.
- **OSTI identifier:** 4239261.
- **Licence / access:** Elsevier, subscription/copyrighted. Metadata only here.
- **Date accessed:** 2026-08-05 — **metadata only; article not retrieved.**
- **Relevance — the primary alpha-phase validation target.** Search metadata
  describes dynamic elastic moduli of Zircaloy-2, Zr-1.15 wt% Cr-0.1 wt% Fe and
  Zr-2.5 wt% Nb over **293–773 K**, with `E` and `G` decreasing linearly with
  temperature. This overlaps the MATPRO alpha branch directly, and it reports
  Poisson's ratio, not just `E` and `G`.
- **⚠ UNVERIFIED CLAIM OF HIGH CONSEQUENCE.** Search-result summaries state that
  in this paper *"Poisson's ratio decreased with increasing temperature for
  Zircaloy-2 and Zr-2.5 wt% Nb but increased for Zr-1.15 wt% Cr-0.1 wt% Fe."*
  If that is what the paper says, then MATPRO — whose `nu` **increases** with
  temperature throughout (see the code-printed Table A in the validation case) —
  has the **wrong sign of `dnu/dT` for Zircaloy-2 over the whole alpha range**,
  which would be a far broader defect than the `nu > 0.5` crossover.
  **This is a paraphrase of an abstract obtained through a search engine. It was
  NOT read in the source and must not be relied on or cited until it is.** It is
  recorded here only because it is the single most important thing to check when
  the paper is obtained. See VAL-2 in the validation case.

### E-3. Schwenk & Wheeler (1978), Poisson's ratio in Zircaloy-4 — **IDENTIFIER-DERIVED**

- **Authors:** E. B. Schwenk and K. R. Wheeler. (Author surnames are literal in
  the Semantic Scholar URL slug
  `Poisson's-ratio-in-zircaloy-4-between-24°-and-316°C-Schwenk-Wheeler`.)
  Search metadata additionally names D. N. Shearer and R. T. Webster as
  co-authors — **SEARCH-METADATA, unverified.**
- **Title:** *Poisson's ratio in zircaloy-4 between 24° and 316°C.*
- **Journal:** Journal of Nuclear Materials, 1978. Volume **73**, pages
  **129–131** — **SEARCH-METADATA for volume and pages.**
- **DOI:** `10.1016/0022-3115(78)90491-9` (decoded from the Elsevier PII
  `0022311578904919`, literal in the search-result URL — this part is reliable).
- **Licence / access:** Elsevier, subscription/copyrighted. Metadata only here.
- **Date accessed:** 2026-08-05 — **metadata only; article not retrieved.**
- **Relevance — the most directly applicable benchmark of all.** It measures
  **Poisson's ratio itself** (rather than `E` and `G` separately) for
  **Zircaloy-4**, the cladding alloy, over **24–316 °C (297–589 K)**, squarely
  inside the MATPRO alpha branch and squarely inside PWR cladding operating
  temperature. A direct `nu` measurement removes the `nu = E/(2G) - 1` error
  amplification entirely, which makes this the cleanest possible acceptance
  test for VAL-2.

### E-4. Bunnell, Mellinger, Bates & Hann (1977), Zircaloy-oxygen alloys — **SEARCH-METADATA**

- **Authors:** L. R. Bunnell, G. B. Mellinger, J. L. Bates, C. R. Hann
  (initials are **SEARCH-METADATA** and unverified; surnames appeared in the
  search summary).
- **Title:** *High Temperature Properties of Zircaloy-Oxygen Alloys.*
- **Report number:** EPRI NP-524. (One OSTI title string rendered this as
  "EPRI IMP-524"; that is almost certainly an OCR artefact of "NP-524", but it
  is unconfirmed.)
- **Organization:** Electric Power Research Institute (EPRI); work performed at
  Battelle Pacific Northwest Laboratories.
- **Date:** March 1977.
- **OSTI identifier:** 7295256 (<https://www.osti.gov/biblio/7295256>).
- **Access / licence:** EPRI report hosted by OSTI. **Access terms not
  verified** — EPRI reports are frequently restricted even when an OSTI
  bibliographic record is public. Confirm redistribution terms before
  reproducing any of its data.
- **Date accessed:** 2026-08-05 — **metadata only.**
- **Relevance:** the oxygen-content dependence, i.e. the `K1` term. Search
  metadata describes thermal expansion, **elastic moduli** and thermal
  diffusivity from room temperature to **1200 °C (1473 K)** at 0.7–28 at.%
  oxygen, with Zircaloy-2 covered to 5 at.% oxygen. The 1473 K upper bound
  means this dataset **reaches into the beta phase and past the 1354.84 K
  crossover** — making it the second candidate (with E-1) for constraining the
  region where the MATPRO pair fails.

### E-5. Elastic properties of Zr-alloy cladding and pressure tubing (1979) — **IDENTIFIER-DERIVED**

- **Title:** *The elastic properties of zirconium alloy fuel cladding and
  pressure tubing materials.*
- **Journal:** Journal of Nuclear Materials, 1979.
- **DOI:** `10.1016/0022-3115(79)90444-6` (decoded from the Elsevier PII
  `0022311579904446`).
- **INIS record:** <https://inis.iaea.org/records/vqvx8-ps774>
- **GAP:** authors, volume and pages not established.
- **Date accessed:** 2026-08-05 — **metadata only.**

### E-6. Influence of oxygen on the elastic properties of Zircaloy-4 (1980) — **IDENTIFIER-DERIVED**

- **Title:** *Influence of oxygen on the elastic properties of Zircaloy-4.*
- **Journal:** Journal of Nuclear Materials, 1980.
- **DOI:** `10.1016/0022-3115(80)90019-7` (decoded from the Elsevier PII
  `0022311580900197`).
- **GAP:** authors, volume and pages not established.
- **Date accessed:** 2026-08-05 — **metadata only.**
- **Relevance:** search metadata describes dynamic elastic moduli of Zircaloy-4
  at 1000–7300 ppm oxygen by weight, room temperature to 1000 °C — a direct
  test of the `K1` oxygen term in the units this port uses (weight fraction).

### E-7. Armstrong & Brown (1964) — **SEARCH-METADATA, LOW CONFIDENCE**

- **Authors:** P. E. Armstrong and H. L. Brown.
- **Citation as reported by search:** Transactions of the Metallurgical Society
  of AIME **230** (1964) 962.
- **GAP:** the article title was not established, and no identifier
  (DOI/handle) was obtained. Confidence is low; treat the whole entry as a lead.
- **Date accessed:** 2026-08-05 — **metadata only.**

### E-8. Elastic anisotropy of zirconium alloy fuel cladding (1981) — **IDENTIFIER-DERIVED**

- **Title:** *Elastic anisotropy of zirconium alloy fuel cladding.*
- **Journal:** Nuclear Engineering and Design, 1981.
- **DOI:** `10.1016/0029-5493(81)90124-2` (decoded from the Elsevier PII
  `0029549381901242`).
- **GAP:** authors, volume and pages not established.
- **Date accessed:** 2026-08-05 — **metadata only.**
- **Relevance:** context, not a benchmark. Textured cladding tubing is
  **not** elastically isotropic, so a single scalar `nu` is already an
  approximation. This bounds how well *any* isotropic correlation can do, and
  therefore informs how tight the VAL-2 acceptance tolerance can sensibly be.

---

## Explicit gaps — data wanted and NOT obtained

Recorded per the instruction that an honest "not sourced" is correct and an
invented value is not.

| ID | What is missing | Why it matters | How to close it |
|---|---|---|---|
| **GAP-1** | MATPRO's own published CELMOD/CSHEAR equations, to confirm upstream's `par1`–`par10` really are MATPRO's. | The whole port rests on upstream's unsourced claim of "MATPROv11". Until checked, the coefficients are traceable only to OFFBEAT, not to MATPRO. | Obtain R-1/R-2, or R-4 which restates them. |
| **GAP-2** | The stated **uncertainty / expected standard error** of CELMOD and CSHEAR. | Without it no quantitative acceptance tolerance for VAL-2 can be justified; any tolerance chosen now is arbitrary. | R-1 § CELMOD/CSHEAR, or R-3, or R-4. |
| **GAP-3** | Any measured `nu`, `E` or `G` value for Zircaloy — **no measured number is recorded anywhere in this directory.** | VAL-2 (does MATPRO reproduce reality in the alpha phase?) cannot be executed at all. | Digitise E-3 first (direct `nu`), then E-2. |
| **GAP-4** | Measured elastic constants in the **beta phase**, above ~1273 K. | This is precisely where the port produces `nu > 0.5`. Without data the failure can only be judged against the admissibility bound, not against reality. | E-1 (spans hcp→bcc) and E-4 (to 1473 K) are the only candidates found. |
| **GAP-5** | The experimental reference list MATPRO itself cites for CELMOD/CSHEAR. | Section E above is a reconstruction from secondary search results, **not** a transcription of MATPRO's bibliography. Some entries may not be in MATPRO's data base at all, and others that are may be missing. | R-1 § CELMOD/CSHEAR reference list. |
| **GAP-6** | Whether the port follows MATPRO-11 or MATPRO-11 Revision 2. | Affects which document is the citable authority. | Compare coefficients once R-1 and R-2 are both in hand. |
| **GAP-7** | Confirmation of the E-2 abstract claim that Poisson's ratio *decreases* with temperature for Zircaloy-2. | If true, MATPRO has the wrong sign of `dnu/dT` across the entire alpha range — a much larger defect than the crossover. | Read E-2. Highest priority of all the gaps. |

## Reproducing the retrieval

The searches behind this file were run on 2026-08-05 against a general web
search index. Full text was unavailable; see the retrieval-status note. To
re-attempt from an unrestricted network, the four documents worth fetching
first, in order, are: **R-4** (most likely to restate the equations, the
uncertainty and the data comparison in one place), **E-3** (direct `nu`
measurement for Zircaloy-4), **E-2** (alpha-phase `nu(T)` trend), and **R-1**
(the authority, and the source of MATPRO's own reference list).
