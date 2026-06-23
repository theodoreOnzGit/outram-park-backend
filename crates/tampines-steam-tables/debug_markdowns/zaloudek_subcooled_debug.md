# Zaloudek Subcooled (Outside-Dome) Stagnation — Debug Reference

Test file: `src/steam_turbine_equations/converging_diverging_nozzles/tests/zaloudek_critical_mass_flux_homogeneous_eqm/outside_dome_stagnation_subcooled.rs`

Solver file: `src/steam_turbine_equations/converging_diverging_nozzles/choked_flow/stagnation_point_outside_vle_ph_dome_multiphase.rs`

---

## Golden-Section Solver (subcooled / outside-dome)

```rust
// stagnation_point_outside_vle_ph_dome_multiphase.rs

use uom::ConstZero;
use uom::si::f64::*;
use uom::si::mass_flux::kilogram_per_square_meter_second;
use uom::si::pressure::megapascal;
use uom::si::pressure::pascal;

use crate::interfaces::functional_programming::ph_flash_eqm::s_ph_eqm;
use crate::prelude::functional_programming::ps_flash_eqm::h_ps_eqm;
use crate::prelude::functional_programming::ps_flash_eqm::v_ps_eqm;
use super::basic_multiphase_equations::bubble_point_pressure_from_entropy;

/// Critical pressure & mass flux for a subcooled-liquid stagnation state
/// (OUTSIDE the dome, left side).
/// Precondition: ph_flash_region(p0, h0) == Region1.
///
/// Method:
///   G(p) = rho(p,s0) * sqrt(2*(h0 - h(p,s0)))   along the isentrope s=s0
///
/// * Single-phase liquid [p_bubble, p0]: G rises monotonically → max at bubble point.
/// * Two-phase [p_min, p_bubble]: G may have an interior peak (flashing choke).
///
/// Global choke = max(G(p_bubble), max_{[p_min,p_bubble]} G(p))
///   interior two-phase peak wins → flashing choke
///   bubble-point value wins     → bubble-point choke (strongly subcooled)
#[inline]
pub fn get_critical_pressure_and_mass_flux_subcooled_liquid_ph(
    p0: Pressure,
    h0: AvailableEnergy,
) -> (Pressure, MassFlux) {

    let s0 = s_ph_eqm(p0, h0);
    let p_min = Pressure::new::<megapascal>(0.000_611_212_677 * 1.01);

    let g_of_p = |p_pa: f64| -> MassFlux {
        let p = Pressure::new::<pascal>(p_pa);
        let h = h_ps_eqm(p, s0);
        let ke = h0 - h;
        if ke < AvailableEnergy::ZERO {
            return MassFlux::ZERO;
        }
        let rho = v_ps_eqm(p, s0).recip();
        rho * (2.0 * ke).sqrt()
    };

    // bubble point: single evaluation at x=0 saturation intersection
    let p_bubble = bubble_point_pressure_from_entropy(s0);
    let g_bubble = g_of_p(p_bubble.get::<pascal>());

    // golden-section search over two-phase region [p_min, p_bubble]
    // gr = (sqrt(5)-1)/2 ≈ 0.618
    let gr = (5.0_f64.sqrt() - 1.0) / 2.0;
    let mut a = p_min.get::<pascal>();
    let mut b = p_bubble.get::<pascal>();
    let mut c = b - gr * (b - a);
    let mut d = a + gr * (b - a);
    for _ in 0..100 {
        if (b - a).abs() < 1.0 { break; }   // 1 Pa bracket width
        let gc = g_of_p(c).get::<kilogram_per_square_meter_second>();
        let gd = g_of_p(d).get::<kilogram_per_square_meter_second>();
        if gc > gd {
            b = d;   // peak is in [a, d]
        } else {
            a = c;   // peak is in [c, b]
        }
        c = b - gr * (b - a);
        d = a + gr * (b - a);
    }
    let p_two_phase = Pressure::new::<pascal>(0.5 * (a + b));
    let g_two_phase = g_of_p(p_two_phase.get::<pascal>());

    // global maximum = choke condition
    if g_two_phase.get::<kilogram_per_square_meter_second>()
        >= g_bubble.get::<kilogram_per_square_meter_second>()
    {
        (p_two_phase, g_two_phase)   // flashing choke
    } else {
        (p_bubble, g_bubble)         // bubble-point choke
    }
}
```

### Bubble-point helper (`basic_multiphase_equations.rs`)

```rust
/// Pressure where isentrope s0 first touches saturation (x=0).
/// Bisection on s_f(p) = s0; s_f is monotonically increasing in p.
/// Seeds the bracket from the precomputed saturation lookup table.
#[inline]
pub fn bubble_point_pressure_from_entropy(s0: SpecificHeatCapacity) -> Pressure {
    let p_min = Pressure::new::<megapascal>(0.000_611_212_677 * 1.01);
    let p_crit = p_crit_water();

    if s0 >= s_crit_water() { return p_crit; }

    let s_f = |p: Pressure| -> SpecificHeatCapacity {
        s_tp_eqm_two_phase(sat_temp_4(p), p, 0.0)
    };

    if s0 <= s_f(p_min) { return p_min; }

    let (mut p_lo, mut p_hi) = bubble_point_bracket(s0);   // table lookup
    for _ in 0..40 {
        let p_mid = 0.5 * (p_lo + p_hi);
        if ((p_hi - p_lo) / p_mid).get::<ratio>() < 1e-9 { break; }
        if s_f(p_mid) < s0 { p_lo = p_mid; } else { p_hi = p_mid; }
    }
    0.5 * (p_lo + p_hi)
}
```

---

## Test Harness

```rust
// outside_dome_stagnation_subcooled.rs — imports and shared helper

use uom::si::f64::*;
use uom::si::area::square_foot;
use uom::si::mass_flux::kilogram_per_square_meter_second;
use uom::si::mass_rate::pound_per_second;
use uom::si::pressure::{kilopascal, pound_force_per_square_inch};
use uom::si::available_energy::kilojoule_per_kilogram;

use crate::interfaces::object_oriented_programming::TampinesSteamTableCV;
use crate::interfaces::functional_programming::ph_flash_eqm::ph_flash_region;
use crate::prelude::functional_programming::pt_flash_eqm::FwdEqnRegion;
use crate::steam_turbine_equations::choked_flow::get_stagnation_conditions_from_throat_ph;
use crate::steam_turbine_equations::choked_flow::get_critical_pressure_and_mass_flux_subcooled_liquid_ph;

fn validate_zaloudek_curve_subcooled(
    x_t: f64,
    data: &[(f64, f64, f64)],
    critical_pressure_tolerance: f64,
    mass_flux_log_tolerance: f64,
) {
    let ref_vol = TampinesSteamTableCV::get_ref_vol();

    for &(p_psia, g_expected_val, _h0_expected_val) in data {
        let p_throat_ref = Pressure::new::<pound_force_per_square_inch>(p_psia);
        let g_expected = MassRate::new::<pound_per_second>(g_expected_val)
            / Area::new::<square_foot>(1.0);

        // throat state -> stagnation (backward map)
        let state_t = TampinesSteamTableCV::new_from_sat_pressure_quality(p_throat_ref, x_t, ref_vol);
        let h_t = state_t.get_specific_enthalpy();
        let (p0, h0, _g_throat) = get_stagnation_conditions_from_throat_ph(p_throat_ref, h_t);

        // only test subcooled-liquid stagnation (Region1)
        let region = ph_flash_region(p0, h0);
        if region != FwdEqnRegion::Region1 {
            eprintln!("skip p={p_psia} psia, x_t={x_t}: stagnation not Region1 ({region:?})");
            continue;
        }

        let (p_crit_calc, g_calc) =
            get_critical_pressure_and_mass_flux_subcooled_liquid_ph(p0, h0);

        println!(
            "p_throat={:8.1} psia | p0={:9.3} kPa | h0={:8.3} kJ/kg | \
             p_crit_calc={:9.3} kPa | p_throat_ref={:9.3} kPa | \
             G_calc={:9.2} kg/m2s | G_ref={:9.2} kg/m2s",
            p_psia,
            p0.get::<kilopascal>(),
            h0.get::<kilojoule_per_kilogram>(),
            p_crit_calc.get::<kilopascal>(),
            p_throat_ref.get::<kilopascal>(),
            g_calc.get::<kilogram_per_square_meter_second>(),
            g_expected.get::<kilogram_per_square_meter_second>(),
        );

        approx::assert_relative_eq!(
            p_crit_calc.get::<kilopascal>(),
            p_throat_ref.get::<kilopascal>(),
            max_relative = critical_pressure_tolerance,
        );

        approx::assert_relative_eq!(
            g_calc.get::<kilogram_per_square_meter_second>().log10(),
            g_expected.get::<kilogram_per_square_meter_second>().log10(),
            max_relative = mass_flux_log_tolerance,
        );
    }
}
```

---

## Individual Tests

Data tuple format: `(p_throat_psia, G_ref_lb_per_s_per_ft2, h0_ref_btu_per_lb)`

Default tolerances: `critical_pressure_tolerance = 0.03`, `mass_flux_log_tolerance = 0.05`.

---

### `quality_bubble_point_subcooled` — x_t = 1e-4 (ACTIVE FAILING CANARY)

`#[ignore]` is commented out — this test is intentionally failing and under active investigation.

Tol: p=0.03, G-log=0.05. 16 points tested (5–2000 psia); 3000 psia skipped (stagnation Region 3).

```
Measured results:
   5 psia | p0=   34.820 | h0=  302.991 | p_crit=  34.556 | p_ref=  34.474 | G_calc= 3346.72 | G_ref=  457.22  ← G×7 too high, p ok
  10 psia | p0=   69.627 | h0=  375.234 | p_crit=  69.088 | p_ref=  68.948 | G_calc= 2547.37 | G_ref=  748.12  ← G×3.4 too high, p ok
  15 psia | p0=  104.256 | h0=  421.642 | p_crit=  91.825 | p_ref= 103.421 | G_calc=  941.83 | G_ref= 1033.95  ← p 11% low
  20 psia | p0=  138.872 | h0=  456.693 | p_crit= 116.708 | p_ref= 137.895 | G_calc= 1137.63 | G_ref= 1331.93  ← p 15% low
  30 psia | p0=  208.324 | h0=  509.443 | p_crit= 167.560 | p_ref= 206.843 | G_calc= 1527.17 | G_ref= 1866.90  ← p 19% low
  50 psia | p0=  348.498 | h0=  582.253 | p_crit= 271.739 | p_ref= 344.738 | G_calc= 2282.98 | G_ref= 2887.55  ← p 21% low
  75 psia | p0=  525.958 | h0=  645.908 | p_crit= 408.095 | p_ref= 517.107 | G_calc= 3219.18 | G_ref= 4162.84  ← p 21% low
 100 psia | p0=  705.429 | h0=  694.708 | p_crit= 552.540 | p_ref= 689.476 | G_calc= 4171.45 | G_ref= 5515.55  ← p 20% low
 150 psia | p0= 1068.923 | h0=  769.406 | p_crit= 869.398 | p_ref=1034.214 | G_calc= 6179.85 | G_ref= 7307.83  ← p 16% low
 200 psia | p0= 1437.038 | h0=  827.225 | p_crit=1232.351 | p_ref=1378.952 | G_calc= 8413.47 | G_ref= 9282.36  ← p 11% low
 300 psia | p0= 2183.679 | h0=  916.757 | p_crit=2069.862 | p_ref=2068.427 | G_calc=16299.94 | G_ref=12828.85  ← G 27% high
 500 psia | p0= 3709.556 | h0= 1046.095 | p_crit=3448.116 | p_ref=3447.379 | G_calc=24749.49 | G_ref=20697.94  ← G 20% high
 750 psia | p0= 5670.410 | h0= 1165.907 | p_crit=5171.955 | p_ref=5171.068 | G_calc=29811.73 | G_ref=27423.74  ← G  9% high
1000 psia | p0= 7689.466 | h0= 1263.217 | p_crit=6897.329 | p_ref=6894.758 | G_calc=34571.17 | G_ref=36335.09  ← PASSES
1500 psia | p0=11908.827 | h0= 1424.988 | p_crit=10068.648| p_ref=10342.137| G_calc=44863.22 | G_ref=49516.03  ← p 3% low, G 9% low
2000 psia | p0=16383.782 | h0= 1566.840 | p_crit=13790.784| p_ref=13789.516| G_calc=57200.58 | G_ref=58622.67  ← PASSES
3000 psia | (skipped — stagnation Region 3)

Failure pattern:
   5–10 psia : G fails (log-error 32%, 18%); p passes. Spurious HEM spike at bubble point.
  15–200 psia: p fails (11–21% below throat); G passes (1–3% log-error).
 300–750 psia: p passes; G fails (2–8% log-error, G too high).
    ≥1000 psia: both pass.
```

```rust
#[test]
//#[ignore = "HEM fundamental limitation near the saturated-liquid line (x≈0)"]
fn quality_bubble_point_subcooled(){
    let data: Vec<(f64, f64, f64)> = vec![
        (5.0,    93.6455,   135.9606),
        (10.0,   153.2273,  165.5172),
        (15.0,   211.7706,  183.2512),
        (20.0,   272.8005,  200.9852),
        (30.0,   382.3708,  221.6749),
        (50.0,   591.4174,  254.1872),
        (75.0,   852.6162,  277.8325),
        (100.0,  1129.6741, 307.3892),
        (150.0,  1496.7620, 331.0345),
        (200.0,  1901.1764, 354.6798),
        (300.0,  2627.5557, 393.1034),
        (500.0,  4239.2716, 449.2611),
        (750.0,  5616.8242, 496.5517),
        (1000.0, 7442.0131, 549.7537),
        (1500.0, 10141.6818,623.6453),
        (2000.0, 12006.8680,682.7586),
        (3000.0, 13820.6838,803.9409),
    ];
    validate_zaloudek_curve_subcooled(1e-4, &data, 0.03, 0.05);
}
```

---

### `quality_0_05_subcooled` — x_t = 0.05, tol p=0.03, G-log=0.05

3 points tested: 1000, 1500, 2000 psia. All others skipped (stagnation not Region1).

```
1000 psia | p0= 8766.855 | h0=1341.165 | p_crit= 6826.690 | p_ref= 6894.758 | G_calc=31266.87 | G_ref=32927.32
1500 psia | p0=13136.743 | h0=1492.221 | p_crit=10227.697 | p_ref=10342.137 | G_calc=43046.47 | G_ref=46152.57
2000 psia | p0=17538.145 | h0=1623.184 | p_crit=13980.993 | p_ref=13789.516 | G_calc=55105.07 | G_ref=55414.79
```

```rust
#[test]
fn quality_0_05_subcooled(){
    let data: Vec<(f64, f64, f64)> = vec![
        (5.0,    64.0497,   177.3399),
        (10.0,   117.2861,  212.8079),
        (15.0,   171.4810,  230.5419),
        (20.0,   224.0296,  248.2759),
        (30.0,   322.9721,  271.9212),
        (50.0,   513.8002,  301.4778),
        (75.0,   730.3714,  328.0788),
        (100.0,  967.7059,  345.8128),
        (150.0,  1300.3281, 375.3695),
        (200.0,  1675.0686, 401.9704),
        (300.0,  2347.8595, 434.4828),
        (500.0,  3788.0125, 490.6404),
        (750.0,  5162.1540, 534.9754),
        (1000.0, 6744.0467, 576.3547),
        (1500.0, 9452.7916, 647.2906),
        (2000.0, 11349.8420,697.5369),
        (3000.0, 14016.4977,795.0739),
    ];
    validate_zaloudek_curve_subcooled(0.05, &data, 0.03, 0.05);
}
```

---

### `quality_0_10_subcooled` — x_t = 0.10, tol p=0.03, G-log=0.05

1 point tested: 1500 psia only. All others skipped.

```
1500 psia | p0=13926.823 | h0=1560.145 | p_crit=10356.041 | p_ref=10342.137 | G_calc=41382.87 | G_ref=42416.62
```

```rust
#[test]
fn quality_0_10_subcooled(){
    let data: Vec<(f64, f64, f64)> = vec![
        (5.0,    52.5990,   230.5419),
        (10.0,   97.6825,   260.0985),
        (15.0,   140.8238,  283.7438),
        (20.0,   183.9780,  295.5665),
        (30.0,   276.6656,  319.2118),
        (50.0,   440.1336,  345.8128),
        (75.0,   634.5181,  369.4581),
        (100.0,  852.6162,  390.1478),
        (150.0,  1161.9117, 419.7044),
        (200.0,  1496.7620, 443.3498),
        (300.0,  2127.6601, 475.8621),
        (500.0,  3384.7887, 532.0197),
        (750.0,  4879.6766, 579.3103),
        (1000.0, 6111.5408, 620.6897),
        (1500.0, 8687.6077, 679.8030),
        (2000.0, 10578.8855,730.0493),
        (3000.0, 13820.6838,803.9409),
    ];
    validate_zaloudek_curve_subcooled(0.10, &data, 0.03, 0.05);
}
```

---

### `quality_0_15_subcooled` through `quality_1_00_subcooled`

All 17 data points skipped for each of these tests — for x_t ≥ 0.15, backward-mapped stagnation
never lands in Region 1 (subcooled liquid) at any of the 17 reference pressures. The stagnation
is two-phase (Region 4) or supercritical (Region 3), so the subcooled solver is never invoked.

These tests pass trivially (zero assertions executed).

```rust
// Structure is the same for all of quality_0_15 through quality_1_00.
// Example: quality_0_15_subcooled
#[test]
fn quality_0_15_subcooled(){
    let data: Vec<(f64, f64, f64)> = vec![
        (5.0,    44.9199,   283.7438),
        (10.0,   85.6893,   313.3005),
        (15.0,   120.0241,  333.9901),
        (20.0,   161.1822,  348.7685),
        (30.0,   242.1845,  369.4581),
        (50.0,   390.3583,  401.9704),
        (75.0,   562.3413,  419.7044),
        (100.0,  755.1770,  437.4384),
        (150.0,  1057.7676, 466.9951),
        (200.0,  1342.9158, 484.7291),
        (300.0,  1907.6022, 520.1970),
        (500.0,  3074.7151, 570.4433),
        (750.0,  4367.6105, 611.8227),
        (1000.0, 5783.5584, 650.2463),
        (1500.0, 8100.9618, 703.4483),
        (2000.0, 10141.3918,750.7389),
        (3000.0, 13241.9279,815.7635),
    ];
    validate_zaloudek_curve_subcooled(0.15, &data, 0.03, 0.05);
}
// quality_0_20_subcooled through quality_1_00_subcooled follow the same
// pattern with their respective x_t values and Zaloudek reference data.
// See the source file for the full data vectors.
```

---

## Key Observations for Debugging

### Why does `quality_bubble_point_subcooled` fail?

The stagnation is very close to the saturated-liquid line (x_t = 1e-4 means
almost pure liquid at the throat). The backward map gives `h0 ≈ h_f(p0)`:
the stagnation is barely subcooled.

When `get_critical_pressure_and_mass_flux_subcooled_liquid_ph` runs:

1. `p_bubble = bubble_point_pressure_from_entropy(s0)` → finds the saturation pressure.
   Since `s0 ≈ s_f(p_bubble)`, `p_bubble ≈ p0`. The liquid stretch is nearly zero.

2. `g_bubble = g_of_p(p_bubble)` → G at the bubble point. Because `h_f(p_bubble) ≈ h0`,
   `ke = h0 - h(p_bubble, s0)` is tiny → `G_bubble` should be small. BUT if `h_ps_eqm`
   crosses into the two-phase region slightly below `p_bubble`, the specific volume
   `v` drops discontinuously (liquid vs two-phase) and G spikes.

3. The golden-section then searches `[p_min, p_bubble]` (two-phase). For x_t ≈ 0 the
   HEM G(p) has an artifact spike right near `p_bubble` from the density discontinuity
   at the bubble point — the search finds this artifact as the "maximum".

4. Result: the returned choke pressure is near `p_bubble ≈ p0` at 5–10 psia (p is ok),
   but G is the artifact value (3–7× too high). At 15–200 psia the search bracket starts
   below the true choke pressure because `p_bubble` itself is underestimated (s0 is only
   marginally subcooled and `bubble_point_pressure_from_entropy` may return a value lower
   than expected).

### Why do `quality_0_05_subcooled` and `quality_0_10_subcooled` pass?

At x_t = 0.05 (genuinely subcooled stagnation at 1000–2000 psia), the stagnation is
well inside Region 1. The liquid stretch `[p_bubble, p0]` is substantial, and `p_bubble`
is well below `p0`. The two-phase golden-section correctly finds the interior choke.
The bubble-point G is small (large liquid enthalpy drop from p0 down to p_bubble but
very small volume change → G well-behaved). The true flashing choke in the two-phase
region wins cleanly.
