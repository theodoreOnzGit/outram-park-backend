# HTR-10: fuel-defect fractions and accident source-term assumptions (Liu & Cao 2002)

The design fuel-defect fractions and the depressurisation / water-ingress
release assumptions behind Liu & Cao's HTR-10 source term. The **numerical
tables** of the same paper are already in code, in `changi::activity`:
Table 1 (inventory), Table 3 (circulating activity), Table 5 (annual release),
Table 8 (accident release), and in `buangkok::published` (Tables 7 and 9,
doses). This record holds the **text-level facts** that no module carried
before 2026-09-30. Needed by the conservative-bound plan #434/#435.

**What kind of numbers these are:**
- The defect fractions are **design specifications**.
- The release assumptions are the authors' **modelling choices**, several of
  them attributed to INTERATOM (1989) and KFA Jül-Spez-335 (1985).
- None is a measurement on HTR-10.

---

## Provenance

| Field | Value |
|---|---|
| Source | Liu Yuanzhong and Cao Jianzhu, *Fission product release and its environment impact for normal reactor operations and for relevant accidents*, Nuclear Engineering and Design **218** (2002) 81–90, doi:10.1016/S0029-5493(02)00200-5 |
| Sections | §2.1 (p.82), §4.1.1.1–4.1.1.4 (pp.86–87), §4.1.2 (p.87) |
| Copy held | the maintainer's private repository, `literature/proprietary/yuanzhong2002fission.pdf` (Kovan folder `local-kovan-repo`) |
| Access tier | **Proprietary** (© 2002 Elsevier). Facts and short quotations only; no full text in this repository |
| Processing | Read by an AI agent from the PDF text layer (`pdftotext`), 2026-09-30. No figure reading. **Not reviewed by a human.** |

---

## 1. Fuel-defect fractions (§2.1, p.82)

| Quantity | Value | Kind |
|---|---|---|
| As-manufactured free uranium | **< 3×10⁻⁴** | design value |
| Particle failure fraction from irradiation | **5×10⁻⁴** | design value |
| Total defective fraction, maximum design burnup | **< 8×10⁻⁴** | design value (the sum) |
| Defective fraction **used to compute the primary-helium activity (Table 3)** | **5×10⁻³** | *"somewhat arbitrarily set"* because INET was manufacturing the fuel for the first time; *"the radioactivity … in the primary circuit is over-estimated very conservatively"* |

Temperatures (p.82, citing Reutler & Lohnert 1983 for the 1600 °C onset of failure):

| Condition | Maximum fuel temperature |
|---|---|
| Normal operation | 864 °C |
| Depressurisation accident | 1033 °C |

## 2. Depressurisation-accident release assumptions (§4.1.1)

| Pathway | Assumption | Attributed to |
|---|---|---|
| Primary-helium activity (§4.1.1.1) | *"Almost the entire radioactivity in the primary helium will be released"* | — |
| Desorption of plate-out (§4.1.1.2) | additional release ≈ **2.4 ×** the originally present coolant activity, for 30 bar → ambient; *"might be conservatively overestimated by a factor of 3"* | INTERATOM 1989 |
| Dust (§4.1.1.3): AVR basis | Cs: 73 % on SG tubes, **5 %** on dust, the rest on the reflector. Sr: SG tubes/dust **80/20 %**. I: **100 %** adsorbed on SG tubes | KFA Jül-Spez-335, 1985 |
| Dust (§4.1.1.3): as used | on dust: **5 %** of Cs; **20 %** of Sr, Rb and Ag; **1 %** of I | the authors, "with reference to these data" |
| Dust released to the building (§4.1.1.3) | **10 %** of the dust | INTERATOM 1989 |
| Helium-purification system (§4.1.1.4), if isolation fails | **100 %** of noble gases, H-3 and C-14; **10 %** of iodine and metals | the authors, "conservatively" |
| Reactor building (§4.1.1.4) | discharged via the exhaust chimney; *"filtering and plate-out effects in the reactor building are not taken into account"* | — |

## 3. Water-ingress accident (§4.1.2)

| Quantity | Value |
|---|---|
| Scenario | double-ended rupture of **two** SG heat-transfer tubes, with the steam relief system failing simultaneously |
| Maximum water ingress into the primary circuit | **129.9 kg** (citing Gao & Shi) |
