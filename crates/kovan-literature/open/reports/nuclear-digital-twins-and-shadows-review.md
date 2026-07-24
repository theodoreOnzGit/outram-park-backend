<!--
KOVAN literature archive — open/reports/
Visibility: OPEN (open literature; safe to commit).
Provenance: condensed from a longer working literature review by
  Theodore Ong Kay Chen (24 July 2026). Only the neutral survey of PUBLIC
  literature is retained here; Outram-Park-internal research strategy,
  proposed research directions, and personal working notes were removed as
  out of scope for the open archive.
Status of citations: a structured verification pass (24 July 2026) has
  confirmed most entries against authoritative sources; a few fields remain
  unconfirmed and are marked VERIFY inline (see the note at the head of the
  References section) — treat those specific fields as unconfirmed.
Not a validation artifact; an educational/orientation survey only, consistent
  with the workspace RESPONSIBLE_USE.md.
-->

# Nuclear Digital Twins and Digital Shadows: Literature Review and Technology Landscape

**Compiled by:** Theodore Ong Kay Chen · **Date:** 24 July 2026
**Scope:** open-literature survey of the nuclear digital-twin / digital-shadow landscape.

---

## Executive summary

The nuclear industry is transitioning from traditional simulation-centric
engineering toward digital engineering, digital shadows, and digital twins.
While "digital twin" is used widely, most current nuclear implementations are
more accurately **digital shadows** — systems that ingest operational data from
physical assets and provide monitoring, diagnostics, and prediction without
closed-loop interaction with plant systems [@rwth_shadow; @oxford_shadow].

True digital twins remain an aspirational endpoint involving bidirectional
coupling between physical facilities and high-fidelity physics-based models.
Major efforts are underway in the United States (ARPA-E GEMINA, EPRI, NRC),
China (CNNC/RINPO), and Europe (EDF, Assystem) spanning reactor design,
construction, operation, maintenance, licensing, and safeguards
[@epri_dt_2022; @nrc2023digitaltwin; @li2022rinpo].

A recurring theme in recent work is **agentic AI applied to engineering
documentation and simulation** — automated model construction, input-deck
generation, and LLM agents coupled to physics simulators. §7 surveys the
published state of that area, which is more developed than a first pass
suggests.

---

## 1. Definitions

### 1.1 Digital model

A static digital representation of a physical asset or process — e.g. CAD
models, P&IDs, thermal-hydraulic nodalizations, core-design models, RELAP or
MELCOR input decks, OpenFOAM cases. There is no automatic synchronization with
the physical asset [@ibm_digital_twin; @oxford_shadow].

```text
Physical Plant        (no automatic link)        Digital Model
```

### 1.2 Digital shadow

A one-way information flow from plant to model.

```text
Physical Plant → Sensors → Digital Shadow
```

Characteristics: real-time monitoring, historical archiving, state estimation,
predictive analytics, machine diagnostics — but **no feedback** to physical
systems. Most industrial nuclear "digital twins" deployed today are more
accurately digital shadows [@rwth_shadow; @oxford_shadow].

### 1.3 Digital twin

A bidirectional interaction.

```text
Physical Plant ⇕ Digital Twin ⇕ (Predictions / Optimization / Control support)
```

Characteristics: real-time synchronization, physics-based simulation, data
assimilation, what-if analysis, predictive forecasting, decision support, and
potential control recommendations. Digital twins are best understood as
systems-of-systems integrating sensors, simulation, analytics, data management,
and visualization [@ibm_digital_twin; @kochunas2021digital].

Kochunas and Huan provide the most-cited conceptual treatment for the nuclear
domain. They conclude that prevailing DT concepts are broadly applicable to
nuclear power systems but need modification, and that some existing modelling
and simulation infrastructure adapts well to DT development while some newer
advanced M&S efforts are less suitable. Their recommendation — that nuclear DTs
should rest first on mechanistic model-based methods, with data-driven
techniques used selectively to augment model-based limitations — is a useful
framing [@kochunas2021digital].

---

## 2. Historical evolution in nuclear engineering

**Phase 1 — Standalone simulation era (1970s–1990s).** Major tools of the
period: RELAP, MELCOR, COBRA, CATHARE. Later system codes in the same lineage —
TRACE (consolidating the TRAC and RELAP lines) and ASTEC — belong to the late
1990s and 2000s rather than this phase. Workflow: engineer → input deck →
simulation → safety analysis. Applications: design-basis accidents, licensing,
severe accidents, thermal hydraulics, reactor physics. No digital linkage
existed between operating plants and simulation models.

**Phase 2 — Full-scope simulator era (1980s–2010s).** Full-scope simulators
predate Three Mile Island; the post-TMI regulatory response made
plant-referenced simulator training a standard requirement rather than
introducing the technology. Applications: operator certification, human-factors
studies, emergency response, severe-accident simulation. CNNC identifies
full-scope simulators as the technological precursor to operational digital
twins [@li2022rinpo].

**Phase 3 — Digital shadow era (2010s–present).** Dominated by plant monitoring,
asset-health monitoring, prognostics, predictive maintenance, and digital work
management (plant → sensors → historian → analytics). This remains the dominant
commercial deployment model today [@cappa2024advancedreactors].

**Phase 4 — Digital twin era (present–future).** Emerging programs seek tighter
integration between live reactor operation and simulation: online
thermal-hydraulics and neutronics, autonomous diagnostics, predictive operation,
automated engineering support. Major initiatives: ARPA-E GEMINA, EPRI advanced-
reactor DT research, NRC regulatory-readiness work, EDF digital engineering,
CNNC operational digital twins
[@gemina_arpae; @epri_dt_2022; @li2022rinpo; @nrc2023digitaltwin].

---

## 3. Major application areas

**3.1 Predictive-maintenance twins** — the most mature commercial application:
pump diagnostics, valve-degradation monitoring, heat-exchanger fouling
detection, rotating-equipment assessment, vibration analysis. Benefits: lower
maintenance cost, reduced downtime, outage optimization, extended component
life. Predictive maintenance was one of two use cases EPRI selected for detailed
development in 3002023904 (the other being construction-sequence simulation)
[@cappa2024advancedreactors].

**3.2 Operational digital twins** — real-time virtual representation of plant
condition (plant sensors → data assimilation → physics models → forecasting →
operator support): forecasting, fault diagnosis, operational optimization,
condition monitoring [@li2022rinpo; @nrc2023digitaltwin].

**3.3 Construction digital twins** — highly relevant for SMRs and Gen-IV:
module tracking, construction planning, schedule optimization, resource
allocation, cost management. EPRI selected construction-sequence simulation as
one of two deeply-developed use cases in 3002023904; the motivation is cost —
construction duration is a top cost driver for new nuclear
[@cappa2024advancedreactors; @epri_dt_2022].

**3.4 Reactor-physics digital twins** — research-focused (in-core detectors →
data assimilation → neutronics solver → core-state estimation → fuel-cycle
prediction): power-distribution forecasting, fuel management, core surveillance.
Challenges: computational expense, measurement uncertainty, licensing hurdles
[@kochunas2021digital; @nrc2023digitaltwin].

**3.5 Thermal-hydraulic digital twins** — plant measurements → data assimilation
→ TH solver → future-state prediction, over code bases such as TRACE, RELAP,
SAM, OpenFOAM: transient forecasting, accident management, online diagnostics.
Recent work already couples LLM agents to physics-based TH simulators in this
configuration; Ndum and co-workers demonstrated an advanced-reactor DT with
bidirectional OPC-UA connectivity and an LLM agent invoking a physics simulator
to generate quantitative control recommendations
[@llm_dt_2025; @mondal2024advanced; @nrc2023digitaltwin].

**3.6 Safeguards and security twins** — emerging (IAEA, national safeguards
agencies, fuel-cycle operators): diversion-pathway analysis, facility
monitoring, safeguards-by-design, inspection planning. Stewart and colleagues
built a digital twin of the AGN-201 reactor specifically to simulate
proliferation scenarios [@stewart2023safeguards].

---

## 4. International landscape

### 4.1 United States

**ARPA-E GEMINA** (Generating Electricity Managed by Intelligent Nuclear
Assets) — an **ARPA-E** program (not a general DOE Office of Nuclear Energy
program). Aim: develop digital-twin technology for advanced reactors and
transform operations-and-maintenance (O&M) systems, with performers designing
tools for greater system flexibility, increased operational autonomy, and
faster design iteration. The program is named after the constellation Gemini
and is oriented toward drastically reducing O&M costs of next-generation plants
[@gemina_arpae; @anl_gemina]. Initial funding was ~$28 million across nine
projects; Argonne National Laboratory participated in four, with its largest
single award ($4.5 million) supporting **SSR APPLIED** with Moltex Energy — a
digital twin of the Stable Salt Reactor–Wasteburner alongside an instrumented
molten-salt loop [@anl_gemina]. GEMINA's primary framing is **O&M cost
reduction**, not autonomous operation, though autonomy features in several
awards.

**EPRI digital-twin research** — key publication EPRI 3002023904, "Program on
Technology Innovation: Digital Twin Applications for Advanced Reactors" (a
~134-page report, Q3 2022). Contributions: a list of DT use cases for advanced
reactors, criteria to prioritize them, two selected for deeper development
(construction-sequence simulation and predictive maintenance), a DT development
framework, and use-case best practices. EPRI notes nuclear DT adoption has
lagged other industries, attributing this partly to the regulatory environment
and initial capital investment [@epri_dt_2022; @epri_insights_2022;
@cappa2024advancedreactors].

**NRC** — has run workshops on enabling technologies for DT applications in
advanced reactors and plant modernization, and published technical assessments
of DT-enabling technology gaps [@nrc2023digitaltwin; @yadav2021gaps]. This
regulatory track matters: acceptance criteria, not modelling capability, is the
binding constraint on physics-based twins entering the licensing basis (see §8).

### 4.2 China — CNNC / RINPO

Among the most advanced national efforts: operational digital twins, online
simulation, AI integration, real-time plant monitoring, training-platform
integration [@li2022rinpo]. *(This section rests on a single 2022 conference
source and should be strengthened before any downstream use.)*

### 4.3 Europe

**EDF** — Varé and Morilhat frame digital twins as a step toward long-term
operation of nuclear power plants [@vare2020ltO]. **Assystem** — engineering-
lifecycle management, construction digital twins, new-nuclear project delivery
[@assystem2024].

---

## 5. Artificial intelligence and agentic engineering

Digital twins are increasingly linked with large language models and agentic
workflows:

```text
Engineering documentation → Agentic AI → Knowledge graph → Simulation model → Digital twin
```

Potential capabilities: automated model construction, automatic nodalization,
design verification, input-deck generation, automated report generation. The
effect is to shift engineering effort from manual model construction toward
automated knowledge extraction. As §7 shows, several of these blocks are no
longer speculative.

---

## 6. State of published work on AI-assisted simulation-model generation

Several capabilities often described as "future" are already published:

- **Automated input generation and simulation orchestration.** *AutoFLUKA* uses
  domain-knowledge-embedded LLM agents to automate a Monte Carlo workflow end to
  end — input interpretation, file generation, execution management,
  post-processing — with a stated extension path to MCNP and PHITS
  [@autofluka2025].
- **LLM agents coupled to physics simulators in a DT loop.** An advanced-reactor
  framework integrates a simulator-based DT with a domain-enhanced LLM over
  OPC-UA, the agent invoking the simulator and issuing quantitative
  recommendations that matched a conventional reference governor [@llm_dt_2025].
- **LLM agents against reactor documentation.** An agent architecture
  integrating documentation, functions and retrieval generated a new operating
  procedure paralleling a provided manual without additional training
  [@llm_operation_2025].
- **Knowledge-informed LLMs over nuclear regulatory text.** Xian and co-workers
  classify shutdown initiating events from licensee event reports for PRA using a
  knowledge-informed LLM framework [@xian2025sdie].
- **Graph representations of plant systems for DTs.** Liu and colleagues develop
  whole-system DTs for advanced reactors using graph neural networks with SAM
  simulations [@liu2024gnn].

**Comparatively less covered:** the specific chain from a *licensing-grade FSAR*
to a *validated system-code nodalization*, with automated verification of the
resulting model against the source document. The published work above targets
Monte Carlo decks, operating procedures, event reports, and pre-existing
simulators — not the FSAR-to-nodalization transformation with a traceable
verification argument.

*Search caveat:* this assessment rests on a handful of web searches — enough to
falsify a broad "little prior work" claim, not to establish a narrow gap.
Nuclear work concentrates in venues that index poorly (NURETH, ANS Annual/Winter
Meetings, ICAPP, SMiRT proceedings, vendor technical reports, national-lab
publications). A structured search across Scopus, Web of Science, INIS, OSTI,
and Google Scholar — plus a manual sweep of recent NURETH and ANS proceedings —
is needed before treating any gap as established.

---

## 7. Verification, validation, and regulatory acceptance

For any FSAR-to-simulation pipeline (or machine-generated engineering model),
the binding questions are:

- **Verification of extraction.** How is it established that extracted system
  topology, geometry, and boundary conditions match the source document? What is
  the error rate, and how are errors detected rather than propagated silently?
- **Validation of the resulting model.** Automated generation does not relieve
  the need for validation against experimental or plant data.
- **Traceability.** Licensing requires an auditable chain from design basis to
  analysis; a machine-generated intermediate breaks that chain unless every
  extraction step is traceable to a document location.
- **Regulatory posture.** The NRC has assessed technical gaps in DT-enabling
  technologies but has not established acceptance criteria for AI-generated
  safety-analysis inputs [@nrc2023digitaltwin; @yadav2021gaps].

The verification problem is arguably more novel — and less contested — than the
generation problem.

---

## 8. Coverage gaps in this review

Thin or absent, flagged for future revision:

- **IAEA** digital-twin and digitalization activities.
- **Korea (KAERI)** and Korean-utility DT work.
- **UK** — Rolls-Royce SMR, and the unified DT-architecture proof-of-concept in
  the UK nuclear sector [@bowman2022unified].
- **Rosatom** and Russian programmes.
- **OECD/NEA** working groups.
- **Cybersecurity** of digital twins, an active area [@hahn_cyber].
- §4.2 (China) rests on one 2022 source.

---

## Recommended resources

- IAEA — https://www.iaea.org
- EPRI Digital Transformation Wiki — https://dx-wiki.epri.com
- Idaho National Laboratory — https://inl.gov
- Oak Ridge National Laboratory — https://ornl.gov
- ARPA-E GEMINA — https://arpa-e.energy.gov/programs-and-initiatives/view-all-programs/gemina
- NRC Digital Twins — https://www.nrc.gov/reactors/power/digital-twins
- CNNC — https://en.cnnc.com.cn
- Assystem — https://www.assystem.com

---

## References

> **Verification status (24 July 2026).** A structured verification pass has
> now confirmed most entries against authoritative sources (NASA ADS bibcodes,
> Taylor & Francis, OSTI, IAEA Indico, institutional pages). Confirmed:
> `autofluka2025`, `llm_operation_2025`, `liu2024gnn`, `anl_gemina`,
> `kochunas2021digital`, `bowman2022unified`, `vare2020ltO`, `xian2025sdie`,
> and the three web resources (`rwth_shadow`, `oxford_shadow`,
> `ibm_digital_twin`). Corrected: `llm_dt_2025` (journal is *Progress in
> Nuclear Energy* 192, not "Energy"); `liu2024gnn` (print volume 211(9), 2025).
> Still `⚠ NEEDS VERIFICATION` — do not cite those fields downstream until
> resolved: `hahn_cyber` (venue/year/co-authors), `li2022rinpo` (parent-
> conference name), and the exact author masthead of `llm_dt_2025`. A further
> set is `⚠ UNVERIFIED` — plausible metadata carried over as-supplied but not
> independently re-checked in this pass (`epri_dt_2022`, `epri_insights_2022`,
> `mondal2024advanced`, `nrc2023digitaltwin`, `yadav2021gaps`,
> `stewart2023safeguards`, `cappa2024advancedreactors`, `kropaczek2023dt`,
> `assystem2024`). **Every entry now carries an inline `✓` / `⚠` status line**
> (see the legend at the top of the BibTeX block). Nothing here is fabricated;
> unconfirmed fields are marked, not guessed.

```bibtex
% Verification legend:  ✓ = confirmed against an authoritative source on 24 Jul 2026;
%   ⚠ NEEDS VERIFICATION = a specific field is known-unconfirmed, do not cite it downstream;
%   ⚠ UNVERIFIED = metadata as-supplied, not independently checked this pass.
% ⚠ UNVERIFIED — not independently checked this pass (metadata as-supplied)
@techreport{epri_dt_2022,
  title = {Program on Technology Innovation: Digital Twin Applications for Advanced Reactors},
  author = {{Electric Power Research Institute}},
  institution = {EPRI}, number = {3002023904}, year = {2022},
  note = {~134 pp. Published Q3 2022},
  url = {https://restservice.epri.com/publicdownload/000000003002023904/0/Product}
}
% ⚠ UNVERIFIED — not independently checked this pass (metadata as-supplied)
@techreport{epri_insights_2022,
  title = {Insights and Innovations, Third Quarter 2022},
  author = {{Electric Power Research Institute}}, institution = {EPRI}, year = {2022},
  url = {https://restservice.epri.com/publicdownload/000000003002025805/0/Product}
}
% ✓ VERIFIED (24 Jul 2026) — GEMINA program & framing confirmed (see anl_gemina)
@misc{gemina_arpae,
  title = {Generating Electricity Managed by Intelligent Nuclear Assets (GEMINA)},
  author = {{Advanced Research Projects Agency-Energy}},
  organization = {ARPA-E, U.S. Department of Energy},
  url = {https://arpa-e.energy.gov/programs-and-initiatives/view-all-programs/gemina},
  note = {Accessed 24 July 2026}
}
% ✓ VERIFIED (24 Jul 2026) — $28M/9 projects, ANL in 4 for $8M, $4.5M SSR APPLIED/Moltex
@misc{anl_gemina,
  title = {Argonne to Explore How Digital Twins May Transform Nuclear Energy with ARPA-E's GEMINA Program},
  author = {{Argonne National Laboratory}}, year = {2020},
  url = {https://www.anl.gov/article/argonne-to-explore-how-digital-twins-may-transform-nuclear-energy-with-8-million-from-arpaes-gemina},
  note = {Confirmed: GEMINA is $28M across 9 projects; Argonne is in 4 of the 9 for $8M total; its largest single award is $4.5M for SSR APPLIED with Moltex Energy (SSR-W).}
}
% ⚠ UNVERIFIED — not independently checked this pass (DOI/OSTI ID as-supplied)
@article{mondal2024advanced,
  title = {Advanced Manufacturing and Digital Twin Technology for Nuclear Energy},
  author = {Mondal, Kunal and Martinez, Oscar and Jain, Prashant},
  journal = {Frontiers in Energy Research}, volume = {12}, pages = {1339836}, year = {2024},
  doi = {10.3389/fenrg.2024.1339836}, note = {Review article. ORNL. OSTI ID 2317775}
}
% ⚠ UNVERIFIED — not independently checked this pass (report number as-supplied)
@techreport{nrc2023digitaltwin,
  title = {State-of-Technology and Technical Challenges in Advanced Sensors, Instrumentation, and Communication to Support Digital Twin for Nuclear Energy Application},
  institution = {US Nuclear Regulatory Commission}, number = {TLR-RES/DE/REB-2023-02}, year = {2023}
}
% ⚠ UNVERIFIED — not independently checked this pass (author list as-supplied)
@techreport{yadav2021gaps,
  title = {Technical Challenges and Gaps in Digital-Twin-Enabling Technologies for Nuclear Reactor Applications},
  author = {Yadav, Vaibhav and Agarwal, Vivek and Gribok, Andrei V. and Hays, Ross D. and Pluth, Adam J. and Ritter, Christopher S. and Zhang, Hongbin and Jain, Prashant K. and Ramuhalli, Pradeep and Eskins, Doug and Carlson, Jesse and Gascot, Ram\'{o}n L. and Ulmer, Christopher and Iyengar, Raj},
  institution = {US Nuclear Regulatory Commission}, year = {2021}
}
% ⚠ NEEDS VERIFICATION — parent-conference name unconfirmed (Indico event 298); no page numbers (it is a slide deck)
@misc{li2022rinpo,
  title = {Nuclear Power Plant Digital Twinning for Efficient Operation},
  author = {Li, Qing},
  organization = {Research Institute of Nuclear Power Operation (RINPO/CNNC)}, year = {2022},
  howpublished = {Conference presentation (slide deck), IAEA Indico event 298},
  url = {https://conferences.iaea.org/event/298/contributions/24882/},
  note = {Presentation, not paginated proceedings (no page numbers). VERIFY parent-conference name — Indico labels event 298 the "8th DEMO Programme Workshop", which is topically inconsistent with a CNNC fission-NPP talk.}
}
% ⚠ UNVERIFIED — not independently checked this pass (venue/author list as-supplied)
@inproceedings{stewart2023safeguards,
  title = {A Digital Twin of the AGN-201 Reactor to Simulate Nuclear Proliferation},
  author = {Stewart, Ryan and Shields, Ashley and Pope, Chad and Darrington, Jake and Wilsdon, Kathryn and Bays, Samuel and Heaps, Kenneth and Scott, James and Reyes, Gabriel and Schanfein, Mark},
  booktitle = {Proceedings of the INMM/ESARDA 2023 Joint Annual Meeting}, year = {2023}
}
% ✓ VERIFIED (24 Jul 2026) — Energies 14(14):4235, doi 10.3390/en14144235
@article{kochunas2021digital,
  title = {Digital Twin Concepts with Uncertainty for Nuclear Power Applications},
  author = {Kochunas, Brendan and Huan, Xun},
  journal = {Energies}, volume = {14}, number = {14}, pages = {4235}, year = {2021},
  doi = {10.3390/en14144235}
}
% ⚠ UNVERIFIED — not independently checked this pass (SMiRT-27 venue/authors as-supplied)
@inproceedings{cappa2024advancedreactors,
  title = {Digital Twin Applications for Advanced Reactors: Summary of EPRI 3002023904 and Ongoing Industry Efforts},
  author = {Cappa, Riccardo and Grant, Frederic and Charkas, Hasan},
  booktitle = {27th International Conference on Structural Mechanics in Reactor Technology (SMiRT-27)}, year = {2024}
}
% ✓ VERIFIED (24 Jul 2026) — Energy and AI 21:100555, doi 10.1016/j.egyai.2025.100555
@article{autofluka2025,
  title = {Automating Monte Carlo Simulations in Nuclear Engineering with Domain Knowledge-Embedded Large Language Model Agents},
  author = {Ndum Ndum, Zavier and Tao, Jian and Ford, John and Liu, Yang},
  journal = {Energy and AI}, volume = {21}, pages = {100555}, year = {2025},
  doi = {10.1016/j.egyai.2025.100555},
  note = {AutoFLUKA. ADS bibcode 2025EneAI..2100555N},
  url = {https://www.sciencedirect.com/science/article/pii/S2666546825000874}
}
% ⚠ NEEDS VERIFICATION — journal/vol/doi confirmed; exact author masthead order/count still to be eyeballed on the published page
@article{llm_dt_2025,
  title = {Large Language Model-Assisted Digital Twin for Remote Monitoring and Control of Advanced Reactors},
  author = {Ndum, Zavier and Lim, Doyeong and Ford, John and Adu, Simon and Tao, Jian and Hassan, Yassin and Liu, Yang},
  journal = {Progress in Nuclear Energy}, volume = {192}, pages = {106172}, year = {2026},
  doi = {10.1016/j.pnucene.2025.106172},
  note = {DOI stem 2025; print issue Feb 2026. Author list taken from the SSRN preprint of the same title -- eyeball the published masthead for exact order/count. LLM + DT over OPC-UA for advanced reactors.},
  url = {https://www.sciencedirect.com/science/article/pii/S0149197025005700}
}
% ✓ VERIFIED (24 Jul 2026) — Nucl. Eng. Technol. 57:103842, doi 10.1016/j.net.2025.103842
@article{llm_operation_2025,
  title = {Large Language Model Agent for Nuclear Reactor Operation Assistance},
  author = {Lee, Yoon Pyo and Cha, Joowon and Yu, Yonggyun and Kim, Seung Geun},
  journal = {Nuclear Engineering and Technology}, volume = {57}, pages = {103842}, year = {2025},
  doi = {10.1016/j.net.2025.103842},
  note = {KAERI. ADS bibcode 2025NuEnT..5703842L},
  url = {https://www.sciencedirect.com/science/article/pii/S1738573325004103}
}
% ✓ VERIFIED (24 Jul 2026) — J. Risk Reliab. 239(6):1257-1264
@article{xian2025sdie,
  title = {A Knowledge-Informed Large Language Model Framework for U.S. Nuclear Power Plant Shutdown Initiating Event Classification for Probabilistic Risk Assessment},
  author = {Xian, Min and Wang, Tao and Zhang, Sai and Xu, Fei and Ma, Zhegang},
  journal = {Proceedings of the Institution of Mechanical Engineers, Part O: Journal of Risk and Reliability},
  volume = {239}, number = {6}, pages = {1257--1264}, year = {2025}
}
% ✓ VERIFIED (24 Jul 2026) — Nucl. Technol. 211(9):2206-2223 (2025), doi 10.1080/00295450.2024.2385214
@article{liu2024gnn,
  title = {Development of Whole System Digital Twins for Advanced Reactors: Leveraging Graph Neural Networks and SAM Simulations},
  author = {Liu, Yang and Alsafadi, Farah and Mui, Travis and O'Grady, Daniel and Hu, Rui},
  journal = {Nuclear Technology}, volume = {211}, number = {9}, pages = {2206--2223}, year = {2025},
  doi = {10.1080/00295450.2024.2385214},
  note = {Online-first 2024 (hence the cite key); assigned print volume 211(9), September 2025}
}
% ✓ VERIFIED (24 Jul 2026) — LNME, pp 96-103, doi 10.1007/978-3-030-48021-9_11
@incollection{vare2020ltO,
  title = {Digital Twins, a New Step for Long Term Operation of Nuclear Power Plants},
  author = {Var\'{e}, Christophe and Morilhat, Patrick},
  booktitle = {Lecture Notes in Mechanical Engineering}, pages = {96--103}, year = {2020},
  doi = {10.1007/978-3-030-48021-9_11}
}
% ✓ VERIFIED (24 Jul 2026) — IEEE Access 10:44691-44709, doi 10.1109/ACCESS.2022.3161626
@article{bowman2022unified,
  title = {A Unified Approach to Digital Twin Architecture — Proof-of-Concept Activity in the Nuclear Sector},
  author = {Bowman, David and Dwyer, Lynn and Levers, Andrew and Patterson, Eann A. and Purdie, Sally and Vikhorev, Konstantin},
  journal = {IEEE Access}, volume = {10}, pages = {44691--44709}, year = {2022},
  doi = {10.1109/ACCESS.2022.3161626}
}
% ⚠ NEEDS VERIFICATION — lead author & OSTI ID 2585051 confirmed; venue/conference, year, full co-author list still unconfirmed
@techreport{hahn_cyber,
  title = {Digital Twins in Nuclear Power: Cybersecurity},
  author = {Hahn, Andrew S. and others},
  institution = {Sandia National Laboratories}, number = {OSTI ID 2585051},
  note = {Lead author and OSTI ID confirmed. VERIFY venue/conference, year and full co-author list against the OSTI bibliographic record.},
  url = {https://www.osti.gov/servlets/purl/2585051}
}
% ⚠ UNVERIFIED — not independently checked this pass (Springer DOI as-supplied)
@incollection{kropaczek2023dt,
  title = {Digital Twins for Nuclear Power Plants and Facilities},
  author = {Kropaczek, David J. and Badalassi, Vittorio and Jain, Prashant K. and Ramuhalli, Pradeep and Pointer, W. David},
  booktitle = {The Digital Twin}, volume = {2}, pages = {971--1022},
  publisher = {Springer International Publishing}, year = {2023},
  doi = {10.1007/978-3-031-21343-4_31}
}
% ⚠ UNVERIFIED — not independently checked this pass (author/year as-supplied)
@misc{assystem2024,
  title = {Digital Twin: A Winning Equation for the Nuclear Industry},
  author = {Richet, Victor}, organization = {Assystem}, year = {2024}
}
% ✓ VERIFIED (24 Jul 2026) — se-rwth.de/research/Digital-Twins/
@misc{rwth_shadow,
  title = {Digital Twins and Digital Shadows in Engineering and Production},
  author = {{RWTH Aachen Software Engineering Group}}, year = {2024},
  url = {https://www.se-rwth.de/research/Digital-Twins/}
}
% ✓ VERIFIED (24 Jul 2026) — oxfordinsights.com, 23 Oct 2023
@misc{oxford_shadow,
  title = {Exploring the Concepts of Digital Twin, Digital Shadow and Digital Model},
  author = {Martinescu, Livia}, organization = {Oxford Insights}, year = {2023},
  url = {https://oxfordinsights.com/insights/exploring-the-concepts-of-digital-twin-digital-shadow-and-digital-model/},
  note = {Published 23 October 2023}
}
% ✓ VERIFIED (24 Jul 2026) — ibm.com/think/topics/digital-twin
@misc{ibm_digital_twin,
  title = {What is a Digital Twin?}, author = {{IBM}}, year = {2025},
  url = {https://www.ibm.com/think/topics/digital-twin}
}
```

---

## Provenance & revision note

Condensed for the KOVAN open archive from a longer working review (24 July
2026). Retained: the neutral survey of public literature. Removed as out of
scope for the open archive: Outram-Park-internal research strategy, proposed
research directions/titles, and personal working notes. Factual corrections
carried over from the source's own revision log include: GEMINA attributed to
**ARPA-E** (not DOE-NE) with O&M-cost framing; `kochunas2021digital` corrected to
*Energies* 14(14):4235 (not *Nuclear Engineering and Design*); TRACE/ASTEC placed
in the late-1990s–2000s lineage rather than Phase 1; and full-scope simulators
noted as pre-dating (not introduced by) the post-TMI regulatory response. A
citation-verification pass on 24 July 2026 further corrected `llm_dt_2025` (the
LLM-assisted advanced-reactor DT paper is Ndum et al. in *Progress in Nuclear
Energy* 192, not a Cammi lead-cooled-fast-reactor work — the §3.5/§6 attribution
was fixed accordingly) and fixed `liu2024gnn`'s print volume to 211(9), 2025.
The TRACE/ASTEC and pre-TMI-simulator points, and the `VERIFY`-tagged fields of
`hahn_cyber`, `li2022rinpo`, and `llm_dt_2025`'s author masthead, remain to be
independently confirmed.
