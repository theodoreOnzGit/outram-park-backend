# References: the DRAFT HTR-10 SB-LOCA fault tree

**DRAFT: top-event definition pending Adolphus Lye (#1719).** A research and
teaching model of the simulator's pressure boundary, not a PSA of the HTR-10
or of any facility (`RESPONSIBLE_USE.md`, `crates/raffles/AGENTS.md`).

Searched 2026-11-23: the open corpus in
`crates/kovan-literature/reactor-literature/` (`htgr/`, `theodore-open-corpus/`,
`kovan-standard-open-corpus/`), all 137 PDFs text-searched with `pdftotext`
for break, leak and rupture frequencies, pipe failure rates, LOCA frequencies,
initiating-event frequencies and HTR-10. Nothing was ingested into kovan.

## What was found and is used

| Use in the tree | Value | Source | Status |
|---|---|---|---|
| `SV1/SV2_FAILS_TO_RESEAT` | 1e-2 per demand | NUREG-75/014 (WASH-1400) Main Report, p. 167 (PDF p. 197 of NRC ML15334A199, `kovan-standard-open-corpus/nrc/`): "the failure of safety valves (PCF) to reseat is about 10-2 per demand" | **GENERIC-LWR**, an approximate general PWR value, borrowed |
| The 11.28 mm "leak" band of the vessel, duct and housing leaks | 1 cm2 flow area, `d = sqrt(4A/pi)` | Kugeler, Nabielek and Buckthorpe (2017), *The High Temperature Gas-cooled Reactor: Safety considerations of the (V)HTR-Modul*, EUR 28712 EN, doi:10.2760/270321, s. 5.12, p. 84 (PDF p. 92, `kovan-standard-open-corpus/eu-jrc/kjna28712enn.pdf`): depressurisation tracks "for 1 cm2 for small leaks and one of 30 cm2 corresponding to a break with diameter of 65 mm" | HTR-Module design basis, not HTR-10 |
| The 65 mm upper bands, and the default range's upper bound | 62 / 65 mm loading tube, 65 mm discharge penetration | `docs/reactor-scoping/htr10-plant-data.md` s. 4.3 ([S2] s. 2, [S5] s. 2); the DN65 depressurisation of Gao and Shi (2002) s. 5.3.1 (`physics::depressurisation`) | HTR-10, quoted |

## Found, and not used as inputs

| Value | Source | Why not an input |
|---|---|---|
| PWR small LOCA S2 (1/2 to 2 in): 1e-3 /reactor-year; S1 (2 to 6 in): 3e-4 /plant-year; large LOCA 1e-4 /reactor-year | WASH-1400 Main Report pp. 62-63 (PDF pp. 92-93 of ML15334A199). The text layer reads the large-LOCA value as "1 x 10-1", an OCR error: p. 63 calls S2 "10 times larger than that for a large RCS rupture" | A whole-PWR-RCS frequency at 15.5 MPa; splitting it onto HTR-10 components would invent the split. Quoted for scale only. |
| Accident classes by probability: DBA 1e-2 to 1e-3 /yr, BDBA 1e-3 to 1e-6 /yr | Kugeler et al. (2017) Table 39, p. 128 (PDF p. 136) | A classification, not component data |
| Normal helium leakage 50-100 % of inventory per year | Kugeler et al. (2017) p. 96 (PDF p. 104) | Operational leakage, not a break |

## Not found

No open HTR-10 or HTGR pressure-boundary failure data, vessel or pipe break
frequency, or HTGR PSA study is in the corpus. Every other basic-event value
is therefore **DUMMY**, marked at its definition in `sbloca_draft.xml`, in
this module's docs and with a DUMMY tag in the simulator.

## The sample

`reference-data/scram/upstream-input/Theatre/theatre.xml`: upstream SCRAM's
own input model (from OpenFTA), GPL-3.0-or-later with SCRAM, already in the
workspace; SCRAM's results for it are `reference-data/scram/oracle.txt`.
