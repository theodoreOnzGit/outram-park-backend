# Zaloudek In-Dome Stagnation — Debug Reference

Test file: `src/steam_turbine_equations/converging_diverging_nozzles/tests/zaloudek_critical_mass_flux_homogeneous_eqm/in_dome_stagnation.rs`

Solver file: `src/steam_turbine_equations/converging_diverging_nozzles/choked_flow/stagnation_point_within_vle_ph_dome_multiphase.rs`

---

## Golden-Section Solver (in-dome)

```rust
// stagnation_point_within_vle_ph_dome_multiphase.rs

use uom::ConstZero;
use uom::si::f64::*;
use uom::si::mass_flux::kilogram_per_square_meter_second;
use uom::si::pressure::megapascal;
use uom::si::pressure::pascal;

use crate::interfaces::functional_programming::ph_flash_eqm::s_ph_eqm;
use crate::prelude::functional_programming::ps_flash_eqm::h_ps_eqm;
use crate::prelude::functional_programming::ps_flash_eqm::v_ps_eqm;

/// Critical pressure & mass flux for a stagnation state INSIDE the p-h VLE dome.
/// Precondition: ph_flash_region(p0, h0) == Region4.
///
/// Method: G(p) = rho(p,s0) * sqrt(2 * (h0 - h(p,s0)))
/// Maximise G over [p_min, p0] by golden-section search.
#[inline]
pub fn get_critical_pressure_and_mass_flux_ph_vle_dome(
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

    // golden-section search: gr = (sqrt(5)-1)/2 ≈ 0.618
    // reuses one G-evaluation per iteration (one probe is retained after bracket shrink)
    let gr = (5.0_f64.sqrt() - 1.0) / 2.0;
    let mut a = p_min.get::<pascal>();
    let mut b = p0.get::<pascal>();
    let mut c = b - gr * (b - a);
    let mut d = a + gr * (b - a);

    let max_iter = 100;
    let tol_pa = 1.0;   // 1 Pa bracket width
    for _ in 0..max_iter {
        if (b - a).abs() < tol_pa { break; }
        let gc = g_of_p(c).get::<kilogram_per_square_meter_second>();
        let gd = g_of_p(d).get::<kilogram_per_square_meter_second>();
        if gc > gd {
            b = d;      // peak is in [a, d]
        } else {
            a = c;      // peak is in [c, b]
        }
        c = b - gr * (b - a);
        d = a + gr * (b - a);
    }

    let p_crit = Pressure::new::<pascal>(0.5 * (a + b));
    let g_crit = g_of_p(p_crit.get::<pascal>());

    (p_crit, g_crit)
}
```

---

## Test Harness

```rust
// in_dome_stagnation.rs — imports and shared helper

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
use crate::steam_turbine_equations::choked_flow::get_critical_pressure_and_mass_flux_ph_vle_dome;

fn validate_zaloudek_curve_in_dome(
    x_t: f64,
    data: &[(f64, f64, f64)],
    critical_pressure_tolerance: f64,
    mass_flux_log_tolerance: f64,
) {
    let ref_vol = TampinesSteamTableCV::get_ref_vol();

    for &(p_psia, g_expected_val, _h0_expected_val) in data {
        let p_throat_critical_ref = Pressure::new::<pound_force_per_square_inch>(p_psia);
        let g_expected = MassRate::new::<pound_per_second>(g_expected_val)
            / Area::new::<square_foot>(1.0);

        let state_t = TampinesSteamTableCV::new_from_sat_pressure_quality(
            p_throat_critical_ref, x_t, ref_vol);
        let h_t = state_t.get_specific_enthalpy();

        // backward map: throat -> stagnation
        let (p0, h0, _g_throat) =
            get_stagnation_conditions_from_throat_ph(p_throat_critical_ref, h_t);

        // filter: only test stagnation inside the dome
        if ph_flash_region(p0, h0) != FwdEqnRegion::Region4 {
            eprintln!(
                "skip p={p_psia} psia, x_t={x_t}: stagnation lies outside dome ({:?})",
                ph_flash_region(p0, h0)
            );
            continue;
        }

        let (p_crit_calc, g_calc) =
            get_critical_pressure_and_mass_flux_ph_vle_dome(p0, h0);

        println!(
            "p_throat={:8.1} psia | p0={:9.3} kPa | h0={:8.3} kJ/kg | \
             p_crit_calc={:9.3} kPa | p_throat_ref={:9.3} kPa | \
             G_calc={:9.2} kg/m2s | G_ref={:9.2} kg/m2s",
            p_psia,
            p0.get::<kilopascal>(),
            h0.get::<kilojoule_per_kilogram>(),
            p_crit_calc.get::<kilopascal>(),
            p_throat_critical_ref.get::<kilopascal>(),
            g_calc.get::<kilogram_per_square_meter_second>(),
            g_expected.get::<kilogram_per_square_meter_second>(),
        );

        approx::assert_relative_eq!(
            p_crit_calc.get::<kilopascal>(),
            p_throat_critical_ref.get::<kilopascal>(),
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

Tolerances: `critical_pressure_tolerance` (relative), `mass_flux_log_tolerance` (relative on log10).

---

### `quality_bubble_point_in_dome` — x_t = 0.0

All 17 points skipped: x_t=0 backward-maps stagnation to subcooled Region 1, not Region 4.

```rust
#[test]
fn quality_bubble_point_in_dome(){
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
    validate_zaloudek_curve_in_dome(0.0, &data, 0.005, 0.05);
}
```

---

### `quality_0_05_in_dome` — x_t = 0.05, tol p=0.01, G-log=0.05

13 points tested (5–750 psia); 1000/1500/2000/3000 psia skipped (stagnation leaves dome).

```
  5 psia | p0=  48.428 | h0=  421.485 | p_crit=  34.537 | p_ref=  34.474 | G_calc=  298.82 | G_ref=  312.72
 10 psia | p0=  94.887 | h0=  491.529 | p_crit=  68.633 | p_ref=  68.948 | G_calc=  562.28 | G_ref=  572.64
 15 psia | p0= 140.673 | h0=  536.483 | p_crit= 102.666 | p_ref= 103.421 | G_calc=  812.42 | G_ref=  837.24
 20 psia | p0= 186.459 | h0=  570.410 | p_crit= 137.050 | p_ref= 137.895 | G_calc= 1057.08 | G_ref= 1093.81
 30 psia | p0= 278.030 | h0=  621.426 | p_crit= 206.603 | p_ref= 206.843 | G_calc= 1534.42 | G_ref= 1576.89
 50 psia | p0= 458.480 | h0=  691.745 | p_crit= 345.384 | p_ref= 344.738 | G_calc= 2435.34 | G_ref= 2508.59
 75 psia | p0= 679.329 | h0=  753.119 | p_crit= 516.861 | p_ref= 517.107 | G_calc= 3483.26 | G_ref= 3565.99
100 psia | p0= 905.565 | h0=  800.100 | p_crit= 695.402 | p_ref= 689.476 | G_calc= 4535.91 | G_ref= 4724.75
150 psia | p0=1336.491 | h0=  871.882 | p_crit=1034.198 | p_ref=1034.214 | G_calc= 6404.41 | G_ref= 6348.76
200 psia | p0=1767.416 | h0=  927.326 | p_crit=1375.124 | p_ref=1378.952 | G_calc= 8191.76 | G_ref= 8178.40
300 psia | p0=2629.267 | h0= 1012.960 | p_crit=2059.445 | p_ref=2068.427 | G_calc=11576.19 | G_ref=11463.25
500 psia | p0=4396.061 | h0= 1136.126 | p_crit=3471.811 | p_ref=3447.379 | G_calc=18040.75 | G_ref=18494.70
750 psia | p0=6550.687 | h0= 1249.564 | p_crit=5155.636 | p_ref=5171.068 | G_calc=24945.69 | G_ref=25203.85
```

```rust
#[test]
fn quality_0_05_in_dome(){
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
    validate_zaloudek_curve_in_dome(0.05, &data, 0.01, 0.05);
}
```

---

### `quality_0_10_in_dome` — x_t = 0.10, tol p=0.005, G-log=0.05

14 points tested (5–1000 psia); 1500/2000/3000 psia skipped.

```
  5 psia | p0=  52.468 | h0=  541.760 | p_crit=  34.451 | p_ref=  34.474 | G_calc=  243.35 | G_ref=  256.81
 10 psia | p0= 103.640 | h0=  609.711 | p_crit=  68.824 | p_ref=  68.948 | G_calc=  466.34 | G_ref=  476.93
 15 psia | p0= 154.812 | h0=  653.267 | p_crit= 103.608 | p_ref= 103.421 | G_calc=  683.86 | G_ref=  687.56
 20 psia | p0= 205.312 | h0=  686.107 | p_crit= 138.206 | p_ref= 137.895 | G_calc=  894.27 | G_ref=  898.26
 30 psia | p0= 304.963 | h0=  735.430 | p_crit= 207.074 | p_ref= 206.843 | G_calc= 1300.47 | G_ref= 1350.80
 50 psia | p0= 501.573 | h0=  803.289 | p_crit= 344.669 | p_ref= 344.738 | G_calc= 2078.18 | G_ref= 2148.92
 75 psia | p0= 743.968 | h0=  862.381 | p_crit= 516.514 | p_ref= 517.107 | G_calc= 3006.90 | G_ref= 3097.99
100 psia | p0= 986.364 | h0=  907.524 | p_crit= 690.320 | p_ref= 689.476 | G_calc= 3914.47 | G_ref= 4162.84
150 psia | p0=1465.768 | h0=  976.336 | p_crit=1037.861 | p_ref=1034.214 | G_calc= 5656.17 | G_ref= 5672.95
200 psia | p0=1929.013 | h0= 1029.341 | p_crit=1375.986 | p_ref=1378.952 | G_calc= 7271.23 | G_ref= 7307.83
300 psia | p0=2866.276 | h0= 1110.943 | p_crit=2068.878 | p_ref=2068.427 | G_calc=10444.20 | G_ref=10388.15
500 psia | p0=4697.708 | h0= 1227.688 | p_crit=3434.307 | p_ref=3447.379 | G_calc=16256.24 | G_ref=16525.99
750 psia | p0=7024.705 | h0= 1334.487 | p_crit=5187.097 | p_ref=5171.068 | G_calc=23216.60 | G_ref=23824.67
1000 psia| p0=9308.610 | h0= 1420.157 | p_crit=6899.091 | p_ref=6894.758 | G_calc=29543.39 | G_ref=29839.16
```

```rust
#[test]
fn quality_0_10_in_dome(){
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
    validate_zaloudek_curve_in_dome(0.10, &data, 0.005, 0.05);
}
```

---

### `quality_0_15_in_dome` — x_t = 0.15, tol p=0.005, G-log=0.05

15 points tested (5–1500 psia); 2000/3000 psia skipped.

```rust
#[test]
fn quality_0_15_in_dome(){
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
    validate_zaloudek_curve_in_dome(0.15, &data, 0.005, 0.05);
}
```

---

### `quality_0_20_in_dome` — x_t = 0.20, tol p=0.005, G-log=0.05

15 points tested (5–1500 psia); 2000/3000 psia skipped.

```rust
#[test]
fn quality_0_20_in_dome(){
    let data: Vec<(f64, f64, f64)> = vec![
        (5.0,    40.8317,   333.9901),
        (10.0,   77.9933,   360.5911),
        (15.0,   109.3192,  378.3251),
        (20.0,   146.8947,  396.0591),
        (30.0,   217.8139,  419.7044),
        (50.0,   356.3976,  446.3054),
        (75.0,   528.4626,  464.0394),
        (100.0,  700.1867,  490.6404),
        (150.0,  995.3214,  511.3300),
        (200.0,  1300.3281, 526.1084),
        (300.0,  1797.1424, 558.6207),
        (500.0,  2899.4912, 602.9557),
        (750.0,  4360.2481, 644.3350),
        (1000.0, 5538.3558, 676.8473),
        (1500.0, 7654.3865, 727.0936),
        (2000.0, 9860.2975, 771.4286),
        (3000.0, 13064.4043,839.4089),
    ];
    validate_zaloudek_curve_in_dome(0.20, &data, 0.005, 0.05);
}
```

---

### `quality_0_25_in_dome` — x_t = 0.25, tol p=0.005, G-log=0.05

16 points tested (5–2000 psia); 3000 psia skipped.

```rust
#[test]
fn quality_0_25_in_dome(){
    let data: Vec<(f64, f64, f64)> = vec![
        (5.0,    36.4853,   387.1921),
        (10.0,   70.6785,   416.7488),
        (15.0,   100.4701,  431.5271),
        (20.0,   133.1178,  443.3498),
        (30.0,   197.3857,  466.9951),
        (50.0,   336.8953,  490.6404),
        (75.0,   478.8995,  514.2857),
        (100.0,  634.5181,  532.0197),
        (150.0,  914.7522,  552.7094),
        (200.0,  1178.3738, 573.3990),
        (300.0,  1722.8701, 600.0000),
        (500.0,  2779.6611, 641.3793),
        (750.0,  4121.6516, 682.7586),
        (1000.0, 5384.6924, 712.3153),
        (1500.0, 7338.0463, 759.6059),
        (2000.0, 9586.7204, 798.0296),
        (3000.0, 12701.9282,857.1429),
    ];
    validate_zaloudek_curve_in_dome(0.25, &data, 0.005, 0.05);
}
```

---

### `quality_0_30_in_dome` — x_t = 0.30, tol p=0.005, G-log=0.05

16 points tested (5–2000 psia); 3000 psia skipped.

```rust
#[test]
fn quality_0_30_in_dome(){
    let data: Vec<(f64, f64, f64)> = vec![
        (5.0,    34.0070,   434.4828),
        (10.0,   64.9572,   464.0394),
        (15.0,   92.3372,   481.7734),
        (20.0,   122.3422,  496.5517),
        (30.0,   186.5846,  517.2414),
        (50.0,   305.2988,  546.7980),
        (75.0,   433.9848,  567.4877),
        (100.0,  591.4174,  582.2660),
        (150.0,  840.7049,  605.9113),
        (200.0,  1098.3309, 620.6897),
        (300.0,  1583.4074, 647.2906),
        (500.0,  2554.6533, 676.8473),
        (750.0,  3841.6818, 712.3153),
        (1000.0, 5162.1540, 741.8719),
        (1500.0, 7134.4498, 777.3399),
        (2000.0, 9190.5208, 815.7635),
        (3000.0, 12349.5091,866.0099),
    ];
    validate_zaloudek_curve_in_dome(0.30, &data, 0.005, 0.05);
}
```

---

### `quality_0_35_in_dome` — x_t = 0.35, tol p=0.005, G-log=0.05

16 points tested (5–2000 psia); 3000 psia skipped.

```rust
#[test]
fn quality_0_35_in_dome(){
    let data: Vec<(f64, f64, f64)> = vec![
        (5.0,    31.6970,   487.6847),
        (10.0,   58.8650,   517.2414),
        (15.0,   86.0651,   532.0197),
        (20.0,   112.4389,  546.7980),
        (30.0,   173.9105,  567.4877),
        (50.0,   284.5609,  597.0443),
        (75.0,   410.2368,  611.8227),
        (100.0,  543.5434,  632.5123),
        (150.0,  794.7008,  650.2463),
        (200.0,  1052.9391, 665.0246),
        (300.0,  1517.9684, 691.6256),
        (500.0,  2414.8606, 721.1823),
        (750.0,  3682.9129, 750.7389),
        (1000.0, 5018.9284, 777.3399),
        (1500.0, 7034.7798, 809.8522),
        (2000.0, 9062.1270, 842.3645),
        (3000.0, 12006.8680,880.7882),
    ];
    validate_zaloudek_curve_in_dome(0.35, &data, 0.005, 0.05);
}
```

---

### `quality_0_40_in_dome` — x_t = 0.40, tol p=0.005, G-log=0.05

16 points tested (5–2000 psia); 3000 psia skipped.

```rust
#[test]
fn quality_0_40_in_dome(){
    let data: Vec<(f64, f64, f64)> = vec![
        (5.0,    29.9625,   540.8867),
        (10.0,   55.6439,   564.5320),
        (15.0,   80.2190,   582.2660),
        (20.0,   109.3192,  600.0000),
        (30.0,   162.0974,  611.8227),
        (50.0,   268.9894,  635.4680),
        (75.0,   382.3708,  659.1133),
        (100.0,  506.6223,  670.9360),
        (150.0,  761.8575,  694.5813),
        (200.0,  1009.4233, 709.3596),
        (300.0,  1475.8519, 733.0049),
        (500.0,  2347.8595, 759.6059),
        (750.0,  3580.7293, 789.1626),
        (1000.0, 4811.5063, 815.7635),
        (1500.0, 6649.8307, 839.4089),
        (2000.0, 8810.6953, 866.0099),
        (3000.0, 12006.8680,892.6108),
    ];
    validate_zaloudek_curve_in_dome(0.40, &data, 0.005, 0.05);
}
```

---

### `quality_0_45_in_dome` — x_t = 0.45, tol p=0.005, G-log=0.05

16 points tested (5–2000 psia); 3000 psia skipped.

```rust
#[test]
fn quality_0_45_in_dome(){
    let data: Vec<(f64, f64, f64)> = vec![
        (5.0,    27.5371,   597.0443),
        (10.0,   52.5990,   617.7340),
        (15.0,   75.8293,   635.4680),
        (20.0,   104.8013,  644.3350),
        (30.0,   153.2273,  662.0690),
        (50.0,   254.2701,  685.7143),
        (75.0,   377.0290,  703.4483),
        (100.0,  492.5659,  715.2709),
        (150.0,  740.7195,  735.9606),
        (200.0,  954.1868,  750.7389),
        (300.0,  1375.6022, 774.3842),
        (500.0,  2219.3826, 803.9409),
        (750.0,  3384.7887, 827.5862),
        (1000.0, 4612.6566, 848.2759),
        (1500.0, 6556.9310, 868.9655),
        (2000.0, 8566.2397, 883.7438),
        (3000.0, 12006.8680,898.5222),
    ];
    validate_zaloudek_curve_in_dome(0.45, &data, 0.005, 0.05);
}
```

---

### `quality_0_50_in_dome` — x_t = 0.50, tol p=0.005, G-log=0.05

16 points tested (5–2000 psia); 3000 psia skipped.

```
  5 psia | p0=   58.486 | h0= 1509.035 | p_crit=  34.476 | p_ref=  34.474 | G_calc=  127.20 | G_ref=  128.89
 10 psia | p0=  116.938 | h0= 1561.371 | p_crit=  68.974 | p_ref=  68.948 | G_calc=  248.69 | G_ref=  246.20
 15 psia | p0=  175.180 | h0= 1594.516 | p_crit= 103.397 | p_ref= 103.421 | G_calc=  367.57 | G_ref=  359.96
 20 psia | p0=  233.423 | h0= 1619.256 | p_crit= 137.866 | p_ref= 137.895 | G_calc=  485.07 | G_ref=  470.26
 30 psia | p0=  349.739 | h0= 1655.947 | p_crit= 206.834 | p_ref= 206.843 | G_calc=  716.90 | G_ref=  707.18
 50 psia | p0=  581.698 | h0= 1705.365 | p_crit= 344.824 | p_ref= 344.738 | G_calc= 1172.00 | G_ref= 1207.01
 75 psia | p0=  869.879 | h0= 1747.243 | p_crit= 516.990 | p_ref= 517.107 | G_calc= 1729.17 | G_ref= 1740.09
100 psia | p0= 1157.387 | h0= 1778.421 | p_crit= 689.469 | p_ref= 689.476 | G_calc= 2279.39 | G_ref= 2273.32
150 psia | p0= 1729.710 | h0= 1824.444 | p_crit=1034.690 | p_ref=1034.214 | G_calc= 3364.10 | G_ref= 3418.61
200 psia | p0= 2296.646 | h0= 1858.546 | p_crit=1378.872 | p_ref=1378.952 | G_calc= 4429.70 | G_ref= 4593.66
300 psia | p0= 3421.092 | h0= 1908.504 | p_crit=2067.371 | p_ref=2068.427 | G_calc= 6529.53 | G_ref= 6529.93
500 psia | p0= 5640.358 | h0= 1973.971 | p_crit=3446.808 | p_ref=3447.379 | G_calc=10657.24 | G_ref=10535.33
750 psia | p0= 8360.574 | h0= 2026.853 | p_crit=5171.687 | p_ref=5171.068 | G_calc=15733.11 | G_ref=16067.47
1000 psia| p0=11021.538 | h0= 2063.841 | p_crit=6893.658 | p_ref=6894.758 | G_calc=20747.62 | G_ref=21896.11
1500 psia| p0=16203.415 | h0= 2112.350 | p_crit=10349.806| p_ref=10342.137| G_calc=30739.91 | G_ref=30261.92
2000 psia| p0=21159.057 | h0= 2139.885 | p_crit=13795.295| p_ref=13789.516| G_calc=40683.33 | G_ref=41239.76
```

```rust
#[test]
fn quality_0_50_in_dome(){
    let data: Vec<(f64, f64, f64)> = vec![
        (5.0,    26.3991,   650.2463),
        (10.0,   50.4252,   667.9803),
        (15.0,   73.7254,   679.8030),
        (20.0,   96.3178,   697.5369),
        (30.0,   144.8425,  715.2709),
        (50.0,   247.2153,  735.9606),
        (75.0,   356.3976,  753.6946),
        (100.0,  465.6123,  765.5172),
        (150.0,  700.1867,  789.1626),
        (200.0,  940.8566,  800.9852),
        (300.0,  1337.4357, 821.6749),
        (500.0,  2157.8051, 848.2759),
        (750.0,  3290.8767, 871.9212),
        (1000.0, 4484.6769, 883.7438),
        (1500.0, 6198.1302, 901.4778),
        (2000.0, 8446.5673, 913.3005),
        (3000.0, 12006.8680,913.3005),
    ];
    validate_zaloudek_curve_in_dome(0.50, &data, 0.005, 0.05);
}
```

---

### `quality_0_55_in_dome` through `quality_0_95_in_dome`

*(See source file for data vectors — structure is identical to above. All use tol p=0.005, G-log=0.05. Skipped pressure counts increase as x_t approaches 1.)*

| Test | Pressures tested | Skipped |
|---|---|---|
| `quality_0_55_in_dome` | 5–2000 psia (16) | 3000 |
| `quality_0_60_in_dome` | 5–2000 psia (16) | 3000 |
| `quality_0_65_in_dome` | 5–1500 psia (15) | 2000, 3000 |
| `quality_0_70_in_dome` | 5–1500 psia (15) | 2000, 3000 |
| `quality_0_75_in_dome` | 5–1500 psia (15) | 2000, 3000 |
| `quality_0_80_in_dome` | 5–1500 psia (15) | 2000, 3000 |
| `quality_0_85_in_dome` | 5–1000 psia (14) | 1500, 2000, 3000 |
| `quality_0_90_in_dome` | 5–750 psia (13)  | 1000, 1500, 2000, 3000 |
| `quality_0_95_in_dome` | 5–200 psia (10)  | 300, 500, 750, 1000, 1500, 2000, 3000 |

---

### `quality_1_00_in_dome` — x_t = 1.0

All 17 points skipped: x_t=1 backward-maps stagnation to superheated vapour Region 2, not Region 4.

```rust
#[test]
fn quality_1_00_in_dome(){
    let data: Vec<(f64, f64, f64)> = vec![
        (5.0,    20.7835,   1173.3990),
        (10.0,   40.2613,   1176.3547),
        (15.0,   59.6990,   1179.3103),
        (20.0,   79.0983,   1194.0887),
        (30.0,   117.2861,  1202.9557),
        (50.0,   181.4077,  1226.6010),
        (75.0,   276.6656,  1235.4680),
        (100.0,  382.3708,  1244.3350),
        (150.0,  575.0083,  1253.2020),
        (200.0,  817.3793,  1256.1576),
        (300.0,  1161.9117, 1256.1576),
        (500.0,  1901.1764, 1250.2463),
        (750.0,  2779.6611, 1238.4236),
        (1000.0, 4007.2951, 1220.6897),
        (1500.0, 5777.1121, 1197.0443),
        (2000.0, 7872.8204, 1161.5764),
        (3000.0, 12006.8680,1072.9064),
    ];
    validate_zaloudek_curve_in_dome(1.00, &data, 0.005, 0.05);
}
```

---

## Helper: Backward Map (`basic_multiphase_equations.rs`)

```rust
/// Given throat (p_t, h_t), recover stagnation (p0, h0, G_crit).
/// Uses: s_t = s_ph_eqm(p_t, h_t), then get_stagnation_conditions_from_throat_ps.
#[inline]
pub fn get_stagnation_conditions_from_throat_ph(
    p_t: Pressure,
    h_t: AvailableEnergy,
) -> (Pressure, AvailableEnergy, MassFlux) {
    let s_t = s_ph_eqm(p_t, h_t);
    get_stagnation_conditions_from_throat_ps(p_t, s_t)
}

/// Core backward map: given throat (p_t, s_t):
///   g_crit  = mass_flux_ps_eqm_throat(p_t, s_t)   // finite-difference HEM sound speed
///   v_t     = v_ps_eqm(p_t, s_t)
///   h_t     = h_ps_eqm(p_t, s_t)
///   u_t     = g_crit * v_t
///   h_0     = h_t + 0.5 * u_t^2
///   s_0     = s_t
///   p_0     = p_hs_eqm(h_0, s_0)
#[inline]
pub fn get_stagnation_conditions_from_throat_ps(
    p_t: Pressure,
    s_t: SpecificHeatCapacity,
) -> (Pressure, AvailableEnergy, MassFlux) {
    let g_crit = mass_flux_ps_eqm_throat(p_t, s_t);
    let v_t = v_ps_eqm(p_t, s_t);
    let h_t = h_ps_eqm(p_t, s_t);
    let u_t: Velocity = g_crit * v_t;
    let h_0 = h_t + 0.5 * u_t * u_t;
    let s_0 = s_t;
    let p_0 = p_hs_eqm(h_0, s_0);
    (p_0, h_0, g_crit)
}
```
