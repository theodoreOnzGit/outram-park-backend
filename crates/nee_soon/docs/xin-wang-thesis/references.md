<!--
PROVENANCE / AI-ASSISTED EXTRACTION NOTICE
==========================================
AI-ASSISTED extraction. The primary citation is verified against the source PDF
cover page; the secondary references are named as they appear in the extracted
Chapter 2 / Chapter 4 / Appendix passages and are NOT independently re-verified
bibliographic entries. Check the dissertation's own Bibliography (p. 119+) for
exact citation details before citing any secondary source.
-->

# References

## Primary source (this extraction)

Wang, Xin. *Coupled neutronics and thermal-hydraulics modeling for pebble-bed
Fluoride-Salt-Cooled, High-Temperature Reactor (FHR).* Ph.D. dissertation,
Doctor of Philosophy in Engineering — Nuclear Engineering, University of
California, Berkeley, Summer 2018. Committee: Prof. Per F. Peterson (Chair),
Prof. Massimiliano Fratoni, Prof. Anil Aswani.
Permalink: <https://escholarship.org/uc/item/40q3985m>
Copyright © 2018 Xin Wang. Peer-reviewed, open-access via UC eScholarship /
California Digital Library.

**Data-policy note.** This is open, published literature and is used here as
such. No confidential, proprietary, operational, or unpublished third-party data
is introduced by this extraction (RESPONSIBLE_USE.md / DATA_POLICY.md).

## Key methods / tools named in the extracted sections

These are the tools and correlations Wang relies on in the parts extracted here.
They are recorded for traceability; the bracketed numbers are the dissertation's
own bibliography indices, not resolved citations.

- **Serpent** — continuous-energy Monte Carlo reactor physics code (group-constant
  generation + reference), ENDF/B-VII.0 nuclear data library. [ref 29]
- **FIG** — "FHR Monte Carlo modeling Input Generator", Wang's open-source Python
  package that emits Serpent input for PB-FHR cores. [ref 47]
- **COMSOL Multiphysics** — used via the General-Form-PDE ("user-defined PDE")
  interface to implement multi-group diffusion and $SP_3$ (Appendix D), coupled to
  porous-media CFD; automated through LiveLink for MATLAB.
- **PyRK** — Python package for nuclear Reactor Kinetics (0-D reflector-corrected
  point kinetics); ODEs solved with an implicit Runge-Kutta (4)5 scheme via SciPy.
- **Ergun correlation** — pebble-bed pressure drop. [ref 10]
- **Wakao correlation** — packed-bed Nusselt number
  ($Nu = 2 + 1.1\,Pr^{1/3}Re^{0.6}$). [ref 45]
- **$SP_N$ / simplified-$P_N$** transport — the $SP_3$ control-rod treatment. [ref 25 for the multipoint/reflector kinetics]
- **Mark-1 PB-FHR design report** — the 236 MW(th) UC Berkeley reference design
  whose geometry/materials Wang models. [ref 1]

## OUTRAM PARK re-implementation crates (targets, not sources)

- `njoy-outram-park-fork` — nuclear data / MGXS (replaces the ENDF+Serpent-tally
  group-constant path).
- `outram-mc-libs` — Monte Carlo transport / geometry / tallies (replaces Serpent
  as the MC reference + MGXS tally engine).
- `outram-foam-appbuilder-lib` (`genfoam::neutronics::sp3`, porous-media TH,
  `multi_region`) — replaces the COMSOL $SP_3$ + porous-media multiphysics.
- `teh-o-prke` — reflector-corrected point kinetics (replaces PyRK).
- `nee_soon` — the coupling driver (`xin_wang_sp3_workflow`).

See [`03-njoy-openmc-genfoam-workflow.md`](03-njoy-openmc-genfoam-workflow.md)
for the full stage-by-stage mapping and the tracking beads.
