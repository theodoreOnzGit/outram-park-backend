# Trademarks and Attribution

Outram Park is a nuclear engineering simulation suite written in Rust,
released under the GNU General Public License v3.0 (GPL-3.0-only).

Outram Park derives numerical methods and algorithms from several
upstream open-source projects. Outram Park honours the trademarks,
identities, and licensing of each upstream project, and is not
affiliated with, endorsed by, or connected to any of them.

## Upstream projects and their trademarks

### OpenFOAM®
- **Trademark holder:** OpenCFD Ltd. (part of ESI Group)
- **Upstream URL:** https://www.openfoam.com
- **Upstream license:** GPL-3.0-or-later
- **Used descriptively in accordance with the OpenFOAM® Trademark Policy**
  (https://www.openfoam.com/trademark-policy).
- Outram Park's `outram-foam-basic-lib`, `outram-foam-turbulence-lib`, and
  `outram-foam-appbuilder-lib` crates are Rust translations of selected
  OpenFOAM® numerical methods. They are not an official OpenFOAM® product
  and are not sanctioned by OpenCFD Ltd. or ESI Group.

### OpenMC
- **Copyright / stewardship:** MIT (Massachusetts Institute of Technology)
  and Argonne National Laboratory
- **Upstream URL:** https://openmc.org
- **Upstream license:** MIT
- Outram Park's `outram-mc-libs` crate is a Rust translation of selected
  OpenMC transport methods, GPL-3.0 relicensed as permitted by the terms of
  the upstream MIT license. It is not an official OpenMC product and is not
  sanctioned by MIT or Argonne National Laboratory.

### NJOY
- **Stewardship:** Los Alamos National Laboratory (LANL) and the
  NJOY development team
- **Upstream URL:** https://www.njoy21.io  (and https://github.com/njoy)
- **Upstream license:** BSD-style (see NJOY2016 LICENSE)
- Outram Park's `outram-park-njoy-fork` crate is a Rust translation of
  selected NJOY nuclear data processing modules (RECONR, BROADR, ACER),
  GPL-3.0 relicensed as permitted by the terms of the upstream BSD
  license. It is not an official NJOY product and is not sanctioned by
  LANL or the NJOY development team.

### DWSIM
- **Trademark holder:** DWSIM Inc.
- **Copyright:** Daniel Medeiros and DWSIM contributors
- **Upstream URL:** https://dwsim.org
- **Upstream license:** GPL-3.0
- Outram Park's `outram-park-dwsim-fork` crate is a Rust translation of
  selected DWSIM chemical process simulation methods. It is not an
  official DWSIM product and is not sanctioned by DWSIM Inc. or its
  contributors.

## Good-faith attribution commitment

Every source file in Outram Park that ports code from an upstream project
carries a header block naming:
- The upstream project and specific source file
- The upstream version or commit SHA
- The upstream copyright holder and license
- The upstream trademark (if any) and its holder
- An explicit non-affiliation notice

This documentary discipline is described in
`docs/CONTRIBUTING.md` and enforced during code review.

## Publication and citation

When citing Outram Park in academic work, please also cite the upstream
projects from which any invoked functionality derives. See `CITATION.cff`
for the recommended citation format.

## Concerns or corrections

If you are a trademark holder or copyright steward of an upstream project
and have concerns about how your project is referenced in Outram Park,
please open a GitHub issue or contact the maintainers directly. We will
respond promptly and in good faith.

Contact: <fill in — probably snrokct@nus.edu.sg>
Repository: <fill in — Outram Park GitHub URL>
