# Sour water and sour gas vs upstream DWSIM (code-to-code)

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** Code-to-code verification against
> upstream DWSIM, not validation against experiment. Not for nuclear facility
> operation, reactor control, safety-critical, or licensing decisions.

**Generated:** 2026-10-02
**Crate commit:** the commit that adds this file (branch
`worktree-agent-a0373aff4202acade` off `develop` `bcac8b58a0`).
**Upstream:** DWSIM 9.0.5.0 built from `1abf72d1b6b41d3e9a8cc770d3cc4e8fc76e5766`
(`docs/upstream-harness/README.md`).

## Methodology

`docs/upstream-harness/sourwater_driver.cs` exercises four upstream paths:

- `spec`: the liquid kernel `FlashAlgorithms/SourWater.vb::CalculateEquilibriumConcentrations`, called directly on total molalities;
- `flash`: the full `SourWaterPropertyPackage` PT flash;
- `prflash`: a sour-gas PR flash, printing upstream's ChemSep constants and k_ij;
- `prphi`: PR ln φ at a given composition.

`tests/upstream_sourwater_parity.rs` (8 tests) runs `thermo::sour_water` and the PR flash on identical inputs. A second upstream build copy carries `sourwater_diagnostic_hs_closure.patch`, which changes line 475 only. It is used to prove that the HS⁻ disagreement is that line. The speciation gate is 1e-6, above upstream's absolute charge tolerance of 1e-10 mol/kg (`:501`) propagated to the species.

**Cases** (refinery sour-water stripper):
- **Feed:** 313.15 K, NH3/H2S/CO2 0.6/0.4/0.05 mol/kg.
- **Hot liquid:** 380 K / 1.8 bar, the liquid of upstream's own flash.
- **Bottoms:** 393.15 K, NH3/H2S 0.003/0.0003 mol/kg.
- **Sour gas:** CH4/CO2/H2S/H2O 0.78/0.08/0.12/0.02 at 300 K, 5 MPa.

## Reference

```bibtex
@software{dwsim_1abf72d1,
  author  = {Medeiros, Daniel Wagner Oliveira de},
  title   = {{DWSIM} -- Open Source Chemical Process Simulator},
  version = {9.0.5.0, commit 1abf72d1b6b41d3e9a8cc770d3cc4e8fc76e5766},
  url     = {https://github.com/DanWBR/dwsim},
  license = {GPL-3.0},
  note    = {FlashAlgorithms/SourWater.vb, PropertyPackages/SourWater.vb, PengRobinson.vb}
}
```

## Results

```csv
quantity,comparison,worst_gap,note
SWEQ K1-K6 and Kw (313.15/380/393.15 K),pristine,2.3e-13,
Henry volatility NH3/CO2/H2S,pristine,2.0e-16,
speciation bottoms,diagnostic build (:475 patched),1.8e-7,pH 8.2734823 vs 8.2734824
speciation hot liquid,diagnostic build (:475 patched),7.5e-9,pH 9.32284741 both
pH feed / hot / bottoms,pristine,+0.592 / +0.212 / +0.115,"upstream 9.589/9.535/8.388 vs port 8.997/9.323/8.273 (#487)"
CO2-bearing speciation,pristine,fails,"max-iterations or NaN; port closes C/N/S to 1.9e-16 (#488)"
sour-water flash vapour,pristine,"H2S x573, CO2 x565","Henry on total molality, speciation discarded (#489)"
PR pure-water liquid A/B/Z,pristine,1e-15,
PR pure-water liquid ln phi,pristine,2.3e-6,upstream sqrt(2)=1.414213
sour gas Z (mean k_ij),pristine,1.6e-15,
sour gas K-values k_ij=0,pristine,"K_CH4 -99.5%, K_CO2 +531%, K_H2O +35.8%",missing k_ij table (#491)
sour gas K-values transposed k_ij,pristine,0.64%,upstream table asymmetric (#490)
```

**Interpretation.** The port's sour-water thermodynamic data (K(T) and Henry volatilities) agree with upstream to round-off. Its speciation solver agrees to 1.8e-7 with upstream once one upstream line is corrected, so the port's liquid chemistry is verified against upstream's *intended* model.

Pristine upstream is not a usable reference for sour water:
- two closure defects (#487, #488) make its speciation wrong or unsolvable;
- its VLE flash does not use the speciation at all (#489).

On the sour-gas side, the PR EOS agrees exactly; the cost is entirely the missing k_ij data and H2S compound (#491). Neither the port nor upstream currently provides a trustworthy sour-water VLE flash.
