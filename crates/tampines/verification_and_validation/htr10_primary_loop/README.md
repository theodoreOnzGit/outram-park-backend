# HTR-10 primary loop: loop-only thermal-hydraulics (gh:#1151)

> Research, education and V&V only. AI draft (Claude), unreviewed.

`tampines::htr10_primary_loop` holds the HTR-10 primary circuit (circulator
coastdown, baffle, hot gas duct, SG, loop momentum) and the core models it
carries (`CoreModel`: the lumped stand-in, and since gh:#1151 part B the
R-Z porous core `RzPorousCore` over appbuilder's `Htr10RzTransientCore`).
**The heat source is an input** (maintainer, 2026-11-03:
"Tampines will have fixed or user determined heat sources"): nothing here
computes power from reactivity.

**The coupled record moved.** The point kinetics, the coupled transient and
the LOFC / CRW validation live in `nee_soon` since 2026-11-03:

- part A (lumped stand-in): `crates/nee_soon/verification_and_validation/htr10_loop_lofc/README.md`;
- part B (R-Z core): `crates/nee_soon/verification_and_validation/htr10_rz_transients/README.md`.

Loop-only checks here (`cargo test --release -p tampines --lib htr10_primary_loop`):
the embedded table copies, the power-map zone fractions, the bypass splits,
the stand-in's steady energy balance at a fixed 3315 kW (exact to 1e-6), and
the drawing `lumped_core_layout.svg` (`draw_the_lumped_core_layout`; what
was checked in it is in the part A record, Sec 6).
