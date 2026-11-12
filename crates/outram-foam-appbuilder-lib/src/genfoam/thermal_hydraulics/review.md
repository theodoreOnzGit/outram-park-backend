# Review record: crates/outram-foam-appbuilder-lib/src/genfoam/thermal_hydraulics

```toml
[kovan]
id = "review-record-crates-outram-foam-appbuilder-lib-src-genfoam-thermal-hydraulics"
kind = "note"
origin = "kovan"
created = "2026-10-11T15:00:45Z"
modified = "2026-10-11T15:00:45Z"
```

The review record of this folder's code, created empty by kovan's "Index fresh".
Reviews, needs-fix notes and annotations are added by kovan's review wizard.
This file is human-owned: kovan never overwrites it.

## Status note, 2026-11-04 (gh:#1218)

Not a review: an agent status note. `solver/two_phase/` (new, unreviewed)
ports `solvers/twoPhase` with the implicitUpwind alpha solver; MULES is not
ported. It runs upstream `1D_boiling`; ~~no upstream parity is measured.~~
**CORRECTED 2026-11-12** (gh:#1361 round 3): parity with the real binary is measured, see
`verification_and_validation/genfoam_vs_upstream_gh1361/README.md` "Round 3". The
closure leaves it calls are listed in its module doc.
