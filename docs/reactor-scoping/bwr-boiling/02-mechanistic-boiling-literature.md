# Mechanistic boiling models: open-literature review (epic #895, fleet slice)

Date: 2026-10-16. Scope: open/public literature only (CLAUDE.md data policy). Read-only on the repo.

## How to read this report

- **Verification method.** Every bibliographic fact below came from a web-search result snippet.
  WebFetch could not resolve any host from this sandbox, and the Crossref API was blocked by the proxy,
  so **I could not resolve a single DOI directly.** A DOI is given only where a search result printed
  it in full; otherwise it reads **"not verified"**. Venue, volume and page numbers marked *(secondary)*
  come from other papers' reference lists, not from the original article.
- **Access column.** "Free" means a full text is readable without a subscription. "Author-hosted" means
  free to read but publisher-copyrighted. "Paywalled" means only the publisher sells it.
- **→ Carey** means: ask the maintainer to read the passage in Van P. Carey, *Liquid-Vapor Phase-Change
  Phenomena* (2nd ed. 2008 / 3rd ed. 2020). Only one chapter number is confirmed: a library record shows
  2nd-edition **ch. 12 = internal-flow convective boiling** (§12.7 post-CHF, §12.8 microchannels). The
  other chapter numbers are **not verified** and may differ between editions. Pool boiling and
  heterogeneous nucleation sit in the "boiling near immersed bodies" part, before ch. 12.

## 0. What the workspace already has (search-before-building, checked 2026-10-16)

Before any of the literature below is ported, note what already exists:

| Location | Content | Status (from its own doc comments) |
|---|---|---|
| `crates/outram-foam-multiphase/src/wall_boiling.rs` | RPI (Kurul–Podowski). Closures: Lemmert–Chawla N, Tolubinski–Kostanchuk d_dep, Cole f, Del Valle–Kenning influence area, Mikic–Rohsenow quench. Ported from OpenFOAM `multiphaseEuler` `wallBoiling` (GPL-3.0). | 0-D point model. Not coupled to a wall function or mass source. Classic evaporative term (OpenFOAM's `A2E` cap is not ported). The other regimes return `NotImplemented`. |
| `crates/outram-foam-multiphase/src/chf.rs` | Biasi, W-3 (Tong), Bowring CHF, `GroeneveldLut` (trilinear interpolation, `from_csv`) | **The LUT ships only an "illustrative made-up grid" and says the real values are "copyrighted".** See §5 for the availability question. |
| `crates/outram-foam-multiphase/src/dryout.rs` | Dryout/post-dryout enums. Worked closure: Dougall–Rohsenow film boiling. | CHF-based onset and film-depletion onset are `NotImplemented`. |
| `crates/outram-foam-appbuilder-lib/src/genfoam/thermal_hydraulics/closures/heat_transfer/{boiling,chf}.rs` | GeN-Foam port: Chen F and S factors, COBRA-TF F/S, Rezkallah–Sims, Saha–Zuber subcooled-boiling fraction, Basu ONB, Groeneveld–Stewart T_min, constant CHF, TRACE-style multi-regime dispatcher | GeN-Foam's `lookUpTableCHF` is **deferred**, as are Cachard post-CHF models. |

So **RPI with its baseline closures, the Chen family, Saha–Zuber, Basu ONB, the T_min model, Dougall–Rohsenow and three
CHF correlations are already present.** The real gaps are:

- the subcooled void profile models (Levy, Lahey, Bowring);
- alternative RPI closures (Hibiki–Ishii, Kocamustafaogullari) and the MIT framework;
- the Gungor–Winterton, Kandlikar, Liu–Winterton and Steiner–Taborek correlations;
- mechanistic DNB models;
- annular film dryout, which is the core BWR CHF mechanism;
- real LUT data.

---

## 1. Onset of nucleate boiling (ONB) and onset of significant void / net vapour generation (OSV / NVG)

| Model | Original reference | Predicts / validity | Best open source | Open code implementing it |
|---|---|---|---|---|
| Bergles–Rohsenow (1964) | A.E. Bergles, W.M. Rohsenow, *J. Heat Transfer* 86 (1964), "The determination of forced-convection surface-boiling heat transfer". DOI 10.1115/1.3688697 **not verified** (the search did not return the ASME record). | Wall superheat at ONB versus heat flux. Water only, about 1–138 bar, from a graphical fit. | **Free:** the precursor MIT HTL Tech. Rep. 1962-021 (Bergles & Rohsenow, forced-convection surface boiling and burnout in small tubes), https://dspace.mit.edu/handle/1721.1/61458. The ASME paper is paywalled. | TRACE and RELAP5 manuals document ONB criteria (NUREG, free). OpenFOAM: not verified. |
| Davis–Anderson (1966) | E.J. Davis, G.H. Anderson, "The incipience of nucleate boiling in forced convection flow", *AIChE J.* 12(4):774–780 *(secondary)*. DOI not verified. | Analytic tangency criterion between the liquid temperature profile and the cavity superheat curve. Contact-angle generalisation of Bergles–Rohsenow. | Paywalled (Wiley). → Carey (heterogeneous nucleation / ONB section, ch. no. not verified). | — |
| Sato–Matsumura (1964) | T. Sato, H. Matsumura, "On the conditions of incipient subcooled-boiling with forced convection", *Bull. JSME* 7(26):392–398 *(secondary)*. DOI not verified. | The same tangency criterion as Davis–Anderson, with contact angle 90°. | Bull. JSME is on J-STAGE; free access for this volume **not verified**. → Carey. | — |
| Basu–Warrier–Dhir (ONB) | not searched this session | ONB wall superheat including contact angle | — | **Already ported**: `OnsetOfNucleateBoilingTemperature::Basu` (GeN-Foam lineage) |
| Saha–Zuber (1974) | P. Saha, N. Zuber, "Point of net vapor generation and vapor void fraction in subcooled boiling", *Proc. 5th IHTC* (Tokyo) Vol. 4, pp. 175–179 *(secondary)*. No DOI found; this is likely a pre-DOI conference paper. | NVG/OSV: thermally controlled branch when Pe < 70 000 (Nu = 455), hydrodynamically controlled branch when Pe > 70 000 (St = 0.0065). Built on wide water and Freon data. | IHTC-5 proceedings are in Begell's digital archive, paywalled. **Free summaries:** a 2021 Purdue review hosted by Mudawar's lab, https://engineering.purdue.edu/mudawar/files/articles-all/2021/2021-07.pdf (author-hosted). That review found Saha–Zuber gives the best predictions of both x_NVG and the axial quality distribution. The TRACE theory manual (below) is also free. | **Already ported**: `SubcooledBoilingFraction::SahaZuber`. GeN-Foam; TRACE (code not open). |
| Levy (1967) OSV | S. Levy, "Forced convection subcooled boiling — prediction of vapor volumetric fraction", *Int. J. Heat Mass Transfer* 10 (1967) 951 *(secondary)*. DOI not verified. | OSV from a bubble-detachment force balance (y_b⁺), plus a profile-fit void model (see §2). | Paywalled. Free derivative: NASA NTRS 19700013954 (a report citing and using Levy), https://ntrs.nasa.gov/api/citations/19700013954/downloads/19700013954.pdf. Not examined in detail. | — |

## 2. Subcooled flow-boiling void models

| Model | Original reference | Predicts / validity | Best open source | Open code |
|---|---|---|---|---|
| Levy profile-fit (1967) | as in §1 | Maps the equilibrium quality to a "true" flowing quality with an exponential profile, then applies drift flux (Zuber–Findlay) to get void. Water, BWR/PWR pressures. | Paywalled. **Lahey & Moody, *The Thermal-Hydraulics of a BWR* (ANS 1993)** covers it; that book is not open. The workspace already cites it in `chf.rs`. | — |
| Lahey mechanistic (≈1978) | R.T. Lahey Jr., "A mechanistic subcooled boiling model", *Proc. 6th IHTC* (Toronto) 1978 — **venue not verified this session** | Wall-flux split into vapour generation and condensation (Rouhani–Axelsson style), with a pumping factor | TRACE V5.0 Theory Manual (draft, free): https://www.nrc.gov/docs/ML0710/ML071000097.pdf. This is a TRACE derivative, not Lahey's paper. | TRACE (not open) |
| Bowring (1962) | R.W. Bowring, "Physical model based on bubble detachment and calculation of steam voidage in the subcooled region of a heated channel", OECD Halden report **HPR-10** (1962) *(secondary)*. No DOI. | Two-region void model (wall voidage, then detached bubbles). Introduces the **pumping factor ε**. Water, 11–138 bar. | Original not found online. RELAP5 and SPACE use a Bowring pumping factor (KNS paper, https://www.kns.org/files/pre_paper/43/20S-050-이태범.pdf). Free description: RELAP5/MOD3 Vol. IV, Models & Correlations (NUREG/CR-5535, OSTI ID 106466, https://www.osti.gov/biblio/106466 — OSTI record found, full-text availability not verified). | RELAP5 (not open). |
| Saha–Zuber + Zuber–Findlay | see §1 | The usual open-literature pairing for void prediction | see §1 | GeN-Foam (fraction only) |

Comparative note: the 2021 Purdue review cited in §1 is a free entry point to this whole family.

## 3. Wall heat-flux partitioning (RPI) and its closures

| Item | Original reference | Predicts / validity | Best open source | Open code (licence) |
|---|---|---|---|---|
| **RPI model** | N. Kurul, M.Z. Podowski, "Multidimensional effects in forced convection subcooled boiling", *Proc. 9th IHTC* (Jerusalem, 1990). **DOI 10.1615/IHTC9.40** (printed in the search result; the abstract matches; the author list is not on that page). | q_w = q_c + q_q + q_e. Developed for subcooled water at high pressure. | Paywalled (Begell). **Free:** arXiv:1709.01783, "Implementation and validation of two-phase boiling flow models in OpenFOAM" (authors not verified, full text not read). Liu & Dinh, arXiv:1809.09184, "Validation and UQ for wall boiling closure relations in multiphase-CFD solver" (Nucl. Sci. Eng. 193(1–2):81–99, 2019). | **OpenFOAM** `multiphaseEuler`/`alphatWallBoilingWallFunction` (GPL-3.0, already the port source). **TrioCFD** (CEA, 3-clause BSD, GPL-compatible): has `Flux_parietal_*` classes. ANSYS CFX/Fluent are closed. |
| N — Lemmert–Chawla | M. Lemmert, J.M. Chawla, "Influence of flow velocity on surface boiling heat transfer coefficient", in *Heat Transfer in Boiling* (Hahne & Grigull eds.), 1977. The EPA HERO record dates it 1974 (Munich), so the **year is ambiguous**. No DOI. | N = [m·ΔT_sup]^p, with OpenFOAM defaults m = 185, p = 1.805. Empirical. | Not online. Coefficients documented in OpenFOAM's source (free). | OpenFOAM (GPL-3.0). **Already ported.** |
| N — Hibiki–Ishii (2003) | T. Hibiki, M. Ishii, "Active nucleation site density in boiling systems", *IJHMT* 46(14) (July 2003) *(IIR record)*. DOI not verified. | Mechanistic: derived from cavity-size and contact-angle distributions. Validated on pool and flow boiling, 1–198 bar *(range from memory; not verified)*. | Paywalled. Free code reference: TrioCFD `Hibiki_Ishii_nucleation_site_density.h` (readthedocs, BSD). | TrioCFD (BSD). OpenFOAM: Kocamustafaogullari–Ishii N (see next row); Hibiki–Ishii in OpenFOAM not verified. |
| N — Kocamustafaogullari–Ishii (1983) | G. Kocamustafaogullari, M. Ishii, "Interfacial area and nucleation site density in boiling systems", *IJHMT* 26(9):1377–1387 (1983). Citation printed in OpenFOAM's header. DOI not verified. | N from a cavity-size-distribution scaling | Paywalled. Coefficients are in OpenFOAM's source. | OpenFOAM (GPL-3.0) |
| d_dep — Tolubinsky–Kostanchuk (1970) | V.I. Tolubinsky, D.M. Kostanchuk, "Vapour bubbles growth rate and heat transfer intensity at subcooled water boiling", *Proc. 4th IHTC* (Paris, 1970), Vol. 5, paper B-2.8 *(from memory; not verified this session)* | d = d_ref·exp(−ΔT_sub/ΔT_ref), with d_ref = 0.6 mm, ΔT_ref = 45 K, clipped to [0.1 mm, 1.4 mm]. Water, about 1–10 bar. | Not online. OpenFOAM source. | **Already ported.** |
| d_dep — Kocamustafaogullari (1983) | as cited in OpenFOAM (`KocamustafaogullariIshiiDepartureDiameter` header cites the 1983 IJHMT paper above) | Fritz-type d ∝ φ·√(σ/gΔρ)·(Δρ/ρ_v)^0.9. Needs contact angle φ. | OpenFOAM source (free) | OpenFOAM (GPL-3.0) |
| d_dep — Unal (1976) | H.C. Ünal, "Maximum bubble diameter, maximum bubble-growth time and bubble-growth rate during the subcooled nucleate flow boiling of water up to 17.7 MN/m²", *IJHMT* 19 (1976) *(from memory; not verified)* | Water, 0.1–17.7 MPa, subcooled flow | Paywalled. | Not verified in OpenFOAM or TrioCFD. |
| f — Cole (1960) | R. Cole, *AIChE J.* 6 (1960) *(from memory; OpenFOAM cites "Cole (1960)")* | f = √(4g(ρ_l−ρ_v)/(3 C_D d ρ_l)) | OpenFOAM source | **Already ported.** OpenFOAM also has Kocamustafaogullari–Ishii (1995) frequency (cited in its v12 header). |
| Quenching / area — Del Valle–Kenning (1985) | V.H. Del Valle, D.B.R. Kenning, "Subcooled flow boiling at high heat flux", *IJHMT* 28(10):1907–1920 (1985) (ANSYS CFX reference list; an arXiv bibliography gives "J. Heat Transfer", which looks wrong). DOI not verified. | Bubble influence area A₂ = K·πd²N/4 (K≈2–4); quench HTC from Mikic–Rohsenow transient conduction | Paywalled. | **Already ported.** |
| MIT framework (Gilman–Baglietto) | L. Gilman, E. Baglietto, "A self-consistent, physics-based boiling heat transfer modeling framework…", *IJMF* (2017) — **venue and DOI not verified**. Gilman's MIT PhD thesis on a subgrid wall-boiling model is on DSpace (handle found via search; exact handle not isolated). | Adds bubble sliding, site interaction and dry area | MIT DSpace thesis (free to read; rights statement not checked). | — |
| MIT framework (Kommajosyula) | R. Kommajosyula, "Development and assessment of a physics-based model for subcooled flow boiling with application to CFD", MIT PhD thesis, 2020 (adv. Baglietto) | Second-generation partitioning: sliding, site interaction, microlayer evaporation, plus new d_dep and f closures. Subcooled flow boiling in PWR-type conditions. | **Free PDF (19.55 MB)** on DSpace@MIT, https://dspace.mit.edu/entities/publication/a4e8fd29-2ff8-4894-bd37-3ae861267986. The reuse licence is not confirmed. | **TrioCFD `Flux_parietal_Kommajosyula`** (3-clause BSD, CEA 2021): the best open reference implementation found. |

Caution, free: arXiv:2509.17637 (Končar et al., 2025) finds that the **heat-flux partitioning model is the main error source**
above about 3 m/s and 10 MW/m². It is useful as a stated validity limit.

## 4. Saturated flow-boiling heat transfer

| Model | Reference | Predicts / validity | Open source | Open code |
|---|---|---|---|---|
| Chen (1966) | J.C. Chen, *Ind. Eng. Chem. Process Des. Dev.* 5(3):322–329. **DOI 10.1021/i260019a023** (printed by the EES library page). | h = F·h_DB + S·h_FZ (Forster–Zuber). Water and organics, saturated, vertical tubes. F and S were curves in the original; later curve fits differ. | Paywalled (ACS). **Free:** the TRACE theory manual and the CTF Theory Manual v4.4 (ORNL/SPR-2023/3125, https://info.ornl.gov/sites/publications/Files/Pub203334.pdf). | **Already ported** (GeN-Foam lineage). Python `ht` (Caleb Bell, **MIT licence**, GPL-compatible) has flow-boiling functions; exact function names were not confirmed this session. |
| Gungor–Winterton (1986) | K.E. Gungor, R.H.S. Winterton, *IJHMT* 29(3):351–358. **DOI 10.1016/0017-9310(86)90205-X** (bibsonomy record). | E·h_l + S·h_pool(Cooper). 4300+ points (water, refrigerants, ethylene glycol). Mean deviation 21.4 % saturated, 25 % subcooled. | Paywalled. | `ht` (MIT): not verified |
| Kandlikar (1990) | S.G. Kandlikar, *J. Heat Transfer* 112(1):219–228. DOI not verified (only "10.1115/1.2…" seen). | Convective and nucleate branches with a fluid factor F_fl. 5246 points, 10 fluids. Mean deviation 15.9 % for water. | **RIT Digital Archive record** https://digitalarchive.rit.edu/xmlui/handle/1850/5525 (full text likely free; not confirmed). | — |
| Liu–Winterton (1991) | Z. Liu, R.H.S. Winterton, *IJHMT* 34(11):2759–2766. DOI not verified. | Asymptotic power-2 sum. Saturated and subcooled, tubes and annuli. | Paywalled. Free docs: `ht.boiling_flow` documentation, https://ht.readthedocs.io/en/latest/ht.boiling_flow.html (lists it). | `ht` (MIT) |
| Steiner–Taborek (1992) | D. Steiner, J. Taborek, *Heat Transfer Eng.* 13(2) (1992). DOI not verified. | Power-3 asymptotic model. 13 000+ vertical-tube points. Often called the most accurate for pure fluids in vertical tubes. | Paywalled. → Carey ch. 12 (2nd ed.). | — |

## 5. CHF / DNB (low quality)

| Model | Reference | Predicts / validity | Open source | Open code |
|---|---|---|---|---|
| Weisman–Pei (1983) | J. Weisman, B.S. Pei, *IJHMT* 26(10):1463–1478 *(secondary)*. DOI not verified. | Bubble crowding: CHF when the bubbly-layer void reaches 0.82. Tubes, high G, low quality. Extended to low G by Weisman–Ying, and later to bundles. | Paywalled. Free discussion: Frontiers in Energy Research 2019, https://www.frontiersin.org/journals/energy-research/articles/10.3389/fenrg.2019.00137/pdf (open access). | — |
| Lee–Mudawar (1988) | C.H. Lee, I. Mudawar, "A mechanistic critical heat flux model for subcooled flow boiling based on local bulk flow conditions", *IJMF* 14(6):711–728. DOI not verified. | Sublayer dryout under a vapour blanket (Helmholtz instability). PWR subcooled conditions. | **Author-hosted free PDF:** https://engineering.purdue.edu/mudawar/files/articles-all/1988/1988_6.pdf | — |
| Katto (1990) | Y. Katto, sublayer-dryout subcooled CHF model, *IJHMT* (1990) *(secondary)*. Venue and DOI not verified. | Subcooled CHF, 1–200 bar. Invalid where local void > 0.7. | Paywalled. | — |
| Celata et al. (1994–96) | Celata, Cumo, Mariani, Simoncini, Zummo, "Rationalization of existing mechanistic models for the prediction of water subcooled flow boiling CHF" (IIR record, ≈1994–95, IJHMT not verified). A related *Fusion Sci. Technol.* 1996 paper exists. | Sublayer dryout with no empirical constants. 1888-point database, high G and high subcooling. | Paywalled. | — |
| **Groeneveld 2006 LUT** | D.C. Groeneveld, J.Q. Shan, A.Z. Vasić, L.K.H. Leung, A. Durmayaz, J. Yang, S.C. Cheng, A. Tanase, "The 2006 CHF look-up table", *Nucl. Eng. Des.* 237(15–17):1909–1922 (2007). DOI 10.1016/j.nucengdes.2007.02.014 **not verified** (search could not confirm it). | CHF(P, G, x) for an 8 mm vertical water tube, plus correction factors. More than 30 000 points. Covers 0.1–21 MPa, 0–8000 kg/m²s, x from −0.5 to 1 *(ranges from memory)*. | **The data behind it is US-government and free:** NUREG/KM-0011 (Groeneveld, Jan 2019), "Critical Heat Flux Data Used to Generate the 2006 Groeneveld Lookup Tables": 25 000+ points from 62 datasets, a 14 MB report and a 6.1 MB data attachment, https://www.nrc.gov/reading-rm/doc-collections/nuregs/knowledge/km0011/ (PDF at https://forms.nrc.gov/docs/ML1902/ML19029B306.pdf). **Whether the LUT values themselves are in that NUREG was not verified.** The table paper is paywalled. A university course copy exists (eclass.uth.gr "CHF_Groeneveld_2006_table.pdf"), but its redistribution licence is unclear, so do not use it as a source. | GeN-Foam `lookUpTableCHF` (licence: GPL per its OpenFOAM lineage; GeN-Foam's licence was **not verified** this session). Niu, Scolaro, Fiorina, NED 416 (2024) 112748 validated GeN-Foam DNB with a LUT against PSBT. TRACE and RELAP5 embed LUTs, but their code is not open. **Action for maintainer:** decide whether the published numeric table may be committed. Facts in a table are arguably not copyrightable, but this is a licence call for the human. The existing doc comment in `chf.rs` calls the values "copyrighted"; that claim was not checked here. |

## 6. Annular-flow dryout (BWR CHF)

| Model | Reference | Predicts / validity | Open source | Open code |
|---|---|---|---|---|
| Whalley–Hutchinson–Hewitt film-flow model | P.B. Whalley (with Hutchinson and Hewitt), "The calculation of critical heat flux in forced convection boiling", *Proc. 5th IHTC* (1974), per a Begell author index (co-authors not shown there). The AERE (Harwell) report precursor was **not found**. | Integrate dW_f/dz = D − E − q/h_fg from the onset of annular flow. Dryout occurs at W_f = 0. Valid for tubes, annular regime, high quality. | Paywalled. → Carey ch. 12 (annular flow / dryout). Free derivative: Glasgow PhD thesis (Riley 1973), https://theses.gla.ac.uk/4062/1/1973RileyPhD.pdf (not examined). | — |
| Hewitt–Govan (1990) entrainment/deposition | G.F. Hewitt, A.H. Govan, *IJHMT* 33 (1990) 229 *(secondary)*. DOI not verified. | Deposition and entrainment rate correlations for film-flow models | Paywalled. | — |
| Okawa et al. (2003) film-flow CHF | T. Okawa, A. Kotani, I. Kataoka, M. Naito, "Prediction of critical heat flux in annular flow using a film flow model", *J. Nucl. Sci. Technol.* 40(6):388–396 (NDL record). DOI not verified (likely J-STAGE/T&F). | Entrainment and deposition rates for steam-water at BWR pressures. RMSE 17.3 % on the KAIST 6058-point tube database (2016 NED). | JNST 2003 sits on T&F/J-STAGE; free access not verified. Free critique: CNS proceedings, "Critical Review of the Film Flow Model by Okawa et al.", https://proceedings.cns-snc.ca/index.php/pcns/article/view/8238, plus a TUM mediaTUM entry. | — |
| Ishii–Mishima (1989) entrainment | M. Ishii, K. Mishima, *IJHMT* 32 (1989) 1835 (requested citation; **not confirmed by search**). | Equilibrium entrained fraction E(We, Re_f) and an entrance-length term. Built on air-water data, low pressure. | **Free (OSTI):** the ANL reports. Ishii–Grolmes/Mishima entrainment work on OSTI: "Correlation for liquid entrainment in annular two-phase flow of viscous fluid", https://doi.org/10.2172/12038509 (OSTI DOI shown by search). ANL "Entrainment measurements in annular flow", https://www.osti.gov/biblio/505284. Sawant–Ishii high-pressure update, https://www.osti.gov/biblio/236180. | — |
| CISE-4 | Gaspari, Bertoletti et al. (CISE, Italy, 1960s–70s): critical-quality / boiling-length correlation. **The original report was not found online.** | x_c(G, P, L_B, D_h) for BWR bundles | MIT M2-/M3-CISE4 modifications (DSpace, free), https://dspace.mit.edu/handle/1721.1/103648. MIT 22.312 open courseware also uses CISE-4. | — |
| Hench–Levy | GE limit lines (GEAP-series reports, 1960s). **Report number and online copy not found.** | Bundle CHF limit lines (historic GE design basis) | Not found openly. | — |
| **GEXL** | GE, GETAB, NEDE-10958-P-A (proprietary). The non-proprietary version is NEDO-10958-A. Later variants (GEXL14, GEXL97) appear in NRC SERs. | Empirical critical quality versus boiling length for each fuel type, fitted to ATLAS bundle tests | **PROPRIETARY — do not use.** NRC SERs, e.g. https://www.nrc.gov/docs/ML1921/ML19211A012.pdf, describe it only qualitatively (mean CPR prediction 0.9879 ± 0.0234 for 8x8R). The constants are not public and must not be sought. | none |

## 7. Post-dryout and film boiling (brief)

| Model | Reference | Notes | Open source | Code |
|---|---|---|---|---|
| Dougall–Rohsenow (1963) | MIT HTL report, 1963 | Dittus–Boelter on a homogeneous-equilibrium velocity. Neglects non-equilibrium. | MIT DSpace HTL reports are free; the exact handle was not isolated. | **Already ported** (`dryout.rs`) |
| Groeneveld (1973) 5.7/5.9 forms | AECL-4594 (report not located online) | Modified Dittus–Boelter with a Y factor. Validity ranges for 5.7 and 5.9 are tabulated in a free Czech paper: https://ojs.cvut.cz/ojs/index.php/APP/article/download/3723/3793/8765 | free (paper above), plus UNENE lecture slides (nuceng.ca) | — |
| Groeneveld film-boiling LUT | not searched this session | — | — | — |
| T_min: Groeneveld–Stewart (1982) | NUREG/CP-0022 (1982), per the workspace doc | TRACE uses max(Groeneveld–Stewart, 725 K) | NRC conference proceedings (free in principle; ML number not verified) | **Already ported** |
| T_min: Henry (1974), Berenson (1961) | not verified | Henry is a Berenson-based correction for wall properties | → Carey (film boiling / Leidenfrost section) | — |

## 8. Pool-boiling foundations

| Model | Reference | Notes | Open source | Code |
|---|---|---|---|---|
| Nukiyama boiling curve | S. Nukiyama, *J. JSME* 37(206):367–374 (1934). English translations in *IJHMT* 9 (1966) 1419–1433 and 27 (1984) 959–970. DOIs not verified. | Discovery of the maximum and minimum heat flux | → Carey (pool boiling) | — |
| Rohsenow (1952) | W.M. Rohsenow, "A method of correlating heat transfer data for surface boiling of liquids" | Surface–fluid constant C_sf; the basis of superposition | **Free:** MIT HTL Tech. Rep. No. 5 (1951), https://dspace.mit.edu/bitstream/1721.1/61431/1/HTL_TR_1951_005.pdf | `ht` (MIT licence; function names not verified) |
| Zuber CHF (1959) | N. Zuber, "Hydrodynamic aspects of boiling heat transfer", AEC report **AECU-4439**, June 1959 (UCLA PhD) | q_max = (π/24)·h_fg·ρ_v^½·[σg(ρ_l−ρ_v)]^¼. Lienhard–Dhir later changed the constant to 0.149. | AEC report, so likely public domain. An OSTI record was **not found**; a UNT catalog entry exists. → Carey (pool-boiling CHF). | — |
| Forster–Zuber (1955) | not searched this session | The nucleate term in Chen | — | already ported inside Chen |

---

## Recommended port / implementation order

1. **Annular film-flow dryout model** (Whalley/Hewitt structure, with Okawa or Ishii–Mishima entrainment and deposition):
   - It is the *BWR* CHF mechanism, the core of epic #895, and nothing for it exists in the workspace yet: `dryout.rs` returns `NotImplemented` for `LiquidFilmDepletion`.
   - It is genuinely mechanistic: an ODE built from conservation, which fits the "hierarchical surrogate, run uncalibrated first" rule.
   - The Ishii–Mishima side has free OSTI/ANL sources.
   - It replaces the need for proprietary GEXL in an open model. Report the uncalibrated disagreement against public tube data, e.g. NUREG/KM-0011 high-quality points.
2. **Real Groeneveld 2006 LUT data in `GroeneveldLut`.** The interpolation code already exists, so this is cheap. It also fixes a doc claim ("copyrighted") that needs a maintainer licence decision. Even if the table cannot be committed, the free NUREG/KM-0011 *data* gives a public CHF validation set for items 1 and 4.
3. **Subcooled void profile:** Saha–Zuber (already present), then a Levy profile-fit or Lahey model, plus Zuber–Findlay drift flux. BWR lower-core void and NVG depend on it. The formulas are short and well described in free NUREG manuals (TRACE, RELAP5 Vol. IV).
4. **Saturated flow boiling alternatives:** Gungor–Winterton, Liu–Winterton, Steiner–Taborek. These are simple algebraic correlations, and the MIT-licensed `ht` library offers an independent cross-check. Add them as enum variants beside Chen. They are calibrated empirical fits, so label them as such.
5. **RPI closure alternatives:** Kocamustafaogullari d_dep and N, Hibiki–Ishii N, and the OpenFOAM `A2E` evaporative cap. These are ports from the GPL OpenFOAM source already used as upstream.
6. **MIT (Kommajosyula) partitioning,** using TrioCFD (BSD-3, GPL-compatible) as the reference implementation and the free MIT thesis as the specification. It is the most physics-complete option, but it targets PWR subcooled CFD and needs a wall-function field coupling that does not exist yet.
7. **Mechanistic DNB:** Lee–Mudawar first (free author-hosted PDF), then Weisman–Pei. These are PWR-oriented and lower priority for a BWR epic.
8. **Post-dryout extras:** Groeneveld 5.9 and Henry T_min. Last, because they are mostly empirical and the BWR focus is on avoiding dryout.

Licence summary for port sources:
- OpenFOAM: GPL-3.0, OK.
- TrioCFD: BSD-3, OK.
- `ht`: MIT, OK.
- GeN-Foam: OpenFOAM-derived, but its licence was not verified this session.
- CTF: distributed through RSICC (CCC-871) under per-user licences; **not open source** (no open licence found). Its theory manuals are free to read on ORNL.
- MOOSE THM `Closures1PhaseTHM`: has only single-phase HTCs (no boiling closure found); LGPL-2.1.
- TRACE and RELAP5: the manuals are free (NUREG, OSTI); the code is not open.
- NEPTUNE_CFD, GEXL: closed or proprietary.
