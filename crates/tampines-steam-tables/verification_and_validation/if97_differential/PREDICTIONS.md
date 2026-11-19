# Predictions, written BEFORE any comparison run (gh:#1346, 2026-11-13)

Committed ahead of the first run so the comparison can fail. Upstreams: CoolProp
`IF97.h` v2.4.0 (CoolProp/IF97 commit `138ec05fd64c60167515f6a4f5448cdd19466d27`,
header compiled directly with g++ 13.3 / glibc 2.39) and rust-steam
(marciorvneto/rusteam commit `2b1f74dbed98bcfeaae2d68fa8ebd9d5b4e080b8`, v0.2.0, MIT).

What I read before predicting (source, not the run): IF97.h `RegionDetermination_TP`,
`Region3::rhomass` (Newton on the basic Helmholtz equation, tolerance 1e-14, seeded
from the SR5 backward v(p,T)), `RegionOutputBackward` (clips T to Tsat +- 1e-6 K since
2024); rusteam `region()` and its `iapws97.rs` API; this crate's
`region_fwd_eqn_single_phase` and `v_tp_3` (documented as imported from rusteam: the
SR5 backward v(p,T) used directly, no refinement).

| # | claim | predicted outcome |
|---|---|---|
| P1 | (p,T) forward, Regions 1, 2, 5: v,h,s,u,cp,cv,w vs IF97.h | worst relative difference < 1e-12; v,h,u,s bit-identical on > 50 % of points (integer powers via `powi`; summation order the only difference); cp,cv,w less often (sqrt / division chains). Differences are libm `exp`/`ln` (region 2 ideal part, region 5) |
| P2 | (p,T) Region 3 vs IF97.h | NOT < 1e-12. This crate takes v from the SR5 backward v(p,T) unrefined, IF97.h Newton-refines on the basic equation. v worst ~1e-5 (SR5 stated band 0.001 %), h,s,u worst <= 1e-4, cp,cv,w <= 5e-3 away from the critical point and up to ~5e-2 within a few K / MPa of it |
| P3 | region assignment | identical on every point where both return, EXCEPT (a) T = 1073.15 K exactly with p <= 50 MPa (this crate and rusteam: Region 5, IF97.h: Region 2, a convention difference on the shared boundary), (b) states exactly on p = psat(T) (both refuse: CoolProp throws, this crate panics). A mismatch anywhere else is a defect |
| P4 | rust-steam vs this crate, forward Regions 1, 2, 5 | bit-identical wherever rusteam returns (code copied), difference list empty; Region 3 also bit-identical (same SR5 transcription). Rusteam returns `OutOfBounds` at exactly p = 100 MPa for T > 623.15 K (half-open `..100e6`) where this crate (fixed 2026-08-11) accepts it |
| P5 | saturation: psat(T), Tsat(p), sigma(T) | psat, Tsat < 1e-12 relative; >= 90 % bit-identical for psat; sigma < 1e-12 |
| P6 | transport mu, lambda (single phase) | Regions 1, 2, 5: < 1e-10 relative. Region 3: inherits the density difference (<= 1e-4 for mu, larger for lambda); lambda near the critical point (\|T - Tc\| < 10 K, \|p - pc\| < 3 MPa) reported separately, worst expected 1e-2 or worse because lambda2 uses cp, cv, drho/dp |
| P7 | (p,h) -> T, (p,s) -> T, Regions 1, 2, 3 | pure backward equations on both sides: < 1e-12 away from saturation. Within 25 mK of the saturation line IF97.h clips to Tsat +- 1e-6 K, this crate does not: differences up to ~1e-4 relative, counted separately |
| P8 | (p,h), (p,s) quality in the two-phase region | agree < 1e-9 (linear interpolation between sat. states) |
| P9 | (h,s) -> T, p | this crate has known gaps (AGENTS.md: 1000 bar, triple point); I expect panics in a minority of points and no silent disagreement above 1e-6 where both return |
| P10 | IF97-vs-IAPWS-95 row (issue case 5) | NOT run this round: needs the `outram-park-fork-coolprop` Water port plus the permissible-differences table; deferred, see the issue comment |

Any observed number outside these is reported as-is with the localisation; none of the
bounds above will be edited after the run.
