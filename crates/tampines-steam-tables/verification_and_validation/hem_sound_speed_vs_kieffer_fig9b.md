# HEM two-phase speed of sound vs Kieffer (1977) Fig. 9b (gh:#29)

> Research, education and V&V only. AI draft, unreviewed.

**Generated:** 2026-11-10 (UTC)
**Crate version / commit:** `tampines-steam-tables` 0.3.1 on develop `3ec6c3592` (gh:#29, gh:#1317 item 4)
**Test:** `src/region_4_vap_liq_equilibrium/tests_kieffer_fig9b.rs`

## Methodology

**Computed.** The homogeneous-equilibrium (HEM) speed of sound inside the
saturation dome, along five isobars, as a function of steam quality `x`. At
each point:

1. `h = x h_g + (1 - x) h_f` at `T_sat(p)`. The end states come from
   `h_tp_eqm_two_phase`, which handles region 3 at 200 bar.
2. `s = s_ph_eqm(p, h)`.
3. The speed is computed by two routes:
   - `w_ps_eqm_region4_kieffer`, Kieffer eq. 28 with saturation-line
     derivatives;
   - `w_ps_eqm_region4_finite_diff_vol`, `c = v / sqrt(-dv/dp|_s)` by
     central difference with `dp = 1e-4 p`.

**Reference.** Kieffer (1977) Fig. 9b, isobars at 5, 10, 50, 100 and 200 bar
(81 points in all). The maintainer digitised them with graphreader.com and
posted them on gh:#29 on 2026-08-21. They are used exactly as posted.
graphreader keeps no axis-calibration record, so the points have no
`kovan-digitise` provenance record. The paper is not in the open literature
submodule: none of the 161 files at `reactor-literature` `a1b8a4df` matches.
So the figure was not re-read in this change.

**Pass criterion.** It was posted on gh:#29 before any point was run:
`|log10 w_model - log10 w_fig9b| <= 0.1` at every point, on both routes and
all five isobars. This is the tolerance of the existing `w_px_eqm_1_bar`
test. It was not widened.

## Reference

```bibtex
@article{kieffer1977sound,
  author  = {Kieffer, Susan Werner},
  title   = {Sound speed in liquid-gas mixtures: Water-air and water-steam},
  journal = {Journal of Geophysical Research},
  volume  = {82}, number = {20}, pages = {2895--2904}, year = {1977},
  doi     = {10.1029/JB082i020p02895},
  note    = {Fig. 9b; points digitised in gh:#29 (2026-08-21)}
}
```

## Results

| isobar | points | Kieffer eq. 28: worst dlog10 | finite-difference volume: worst dlog10 |
|---|---|---|---|
| 5 bar   | 20 | +0.0358 at x = 0.0177 | +0.0358 at x = 0.0177 |
| 10 bar  | 19 | +0.0309 at x = 0.0198 | +0.0309 at x = 0.0198 |
| 50 bar  | 19 | +0.0519 at x = 0.00286 | +0.0519 at x = 1.24e-5 |
| 100 bar | 13 | -0.0433 at x = 0.264 | +0.0894 at x = 1.20e-5 |
| 200 bar | 10 | +0.0395 at x = 0.886 | **+0.1089 at x = 1.11e-5: FAILS** |

**Kieffer eq. 28 route: passes at all 81 points.** The worst deviation is
0.052 in log10 (12 %), about the scatter expected of graph-read points on a
log axis.

**Finite-difference-volume route: one miss, at 200 bar, x = 1.105e-5.** It
gives 188.06 m/s against the figure's 146.36 m/s; the eq. 28 route gives
145.70 m/s there.
- **Cause.** The stencil `dp = 1e-4 p` (2 kPa) crosses the saturated-liquid
  line. Near 200 bar, IF97 `s_f` rises by about 6.6e-3 kJ/(kg K) per bar
  (3.685 at 150 bar, 4.014 at 200 bar), so by about 1.3e-4 across `+dp`.
  But `s - s_f = x s_fg` is only about 1.0e-5 here (`s_fg` = 0.913). The
  `+dp` state is therefore subcooled liquid, and its stiffness raises the
  computed speed.
- **The estimate matches the measurements.** It puts the crossing at
  `x ≈ 1.4e-4`. Measured: `x = 3.1e-5` gives +0.0903 and `x = 1.6e-4` gives
  -0.0003.
- **The same artefact at 100 bar** gives +0.0894 at `x = 1.2e-5`, inside the
  criterion.
- **Status.** This is a defect of the route near `x = 0` (filed as gh:#1325), not of the figure,
  and the criterion stays at 0.1. The miss is pinned by
  `finite_diff_route_misses_fig9b_200_bar_only_at_lowest_quality`. That test
  passes only while every other 200-bar point passes and this one point still
  misses, at between 0.1 and 0.12.

**Agreement between the two routes.** Above `x ≈ 1e-4` they agree to four
figures at 5-100 bar. At 200 bar they differ by about 3 %, because the eq. 28
route evaluates the region-1/2 equations at `T_sat` = 638.9 K, past the
region-1 limit of 623.15 K.

**Prediction against outcome.** The posted prediction was right for 5-50 bar
(worst 0.03-0.06), and 100 bar passed although it was flagged as at risk. The
prediction for 200 bar was wrong in both directions. It expected the eq. 28
route to be the risk, and that route passed. The finite-difference route
missed instead, for a reason the prediction did not foresee.

**Not the same data as the legacy tests.** `tests.rs` carries older
`w_px_eqm_{5,10,50,100,200}_bar` datasets on the finite-difference route,
two of them at `epsilon = 0.2`. They have no figure citation and differ from
these Fig. 9b points; at 5 bar, for example, the low-`x` plateau is 4.74 m/s
there and 4.60 m/s here. Their source was not re-checked. These Fig. 9b tests
do not replace them.

### Every point

```csv
pressure_bar,route,x,w_model_m_per_s,w_fig9b_m_per_s,dlog10
5,FiniteDiffVolume,0.00001105,4.574,4.598,-0.0023
5,FiniteDiffVolume,0.00002125,4.590,4.536,+0.0051
5,FiniteDiffVolume,0.00005182,4.637,4.598,+0.0036
5,FiniteDiffVolume,0.00011782,4.738,4.726,+0.0011
5,FiniteDiffVolume,0.00032559,5.056,4.992,+0.0056
5,FiniteDiffVolume,0.00086290,5.876,5.646,+0.0174
5,FiniteDiffVolume,0.00196217,7.539,7.321,+0.0128
5,FiniteDiffVolume,0.00337732,9.654,9.624,+0.0013
5,FiniteDiffVolume,0.00706424,15.027,15.114,-0.0025
5,FiniteDiffVolume,0.01043237,19.774,18.556,+0.0276
5,FiniteDiffVolume,0.01770805,29.553,27.215,+0.0358
5,FiniteDiffVolume,0.02615099,40.182,39.372,+0.0088
5,FiniteDiffVolume,0.03970996,55.902,53.927,+0.0156
5,FiniteDiffVolume,0.06647221,83.166,81.285,+0.0099
5,FiniteDiffVolume,0.10235253,114.191,115.998,-0.0068
5,FiniteDiffVolume,0.15760028,153.993,156.722,-0.0076
5,FiniteDiffVolume,0.23274174,198.573,208.867,-0.0220
5,FiniteDiffVolume,0.38959644,272.139,274.580,-0.0039
5,FiniteDiffVolume,0.59989247,349.132,337.108,+0.0152
5,FiniteDiffVolume,0.89833381,436.682,431.211,+0.0055
5,KiefferEq28,0.00001105,4.574,4.598,-0.0023
5,KiefferEq28,0.00002125,4.590,4.536,+0.0051
5,KiefferEq28,0.00005182,4.637,4.598,+0.0036
5,KiefferEq28,0.00011782,4.738,4.726,+0.0011
5,KiefferEq28,0.00032559,5.056,4.992,+0.0056
5,KiefferEq28,0.00086290,5.876,5.646,+0.0174
5,KiefferEq28,0.00196217,7.539,7.321,+0.0128
5,KiefferEq28,0.00337732,9.654,9.624,+0.0013
5,KiefferEq28,0.00706424,15.027,15.114,-0.0025
5,KiefferEq28,0.01043237,19.774,18.556,+0.0276
5,KiefferEq28,0.01770805,29.553,27.215,+0.0358
5,KiefferEq28,0.02615099,40.182,39.372,+0.0088
5,KiefferEq28,0.03970996,55.902,53.927,+0.0156
5,KiefferEq28,0.06647221,83.166,81.285,+0.0099
5,KiefferEq28,0.10235253,114.191,115.998,-0.0068
5,KiefferEq28,0.15760028,153.993,156.722,-0.0076
5,KiefferEq28,0.23274174,198.573,208.867,-0.0220
5,KiefferEq28,0.38959644,272.139,274.580,-0.0039
5,KiefferEq28,0.59989247,349.132,337.108,+0.0152
5,KiefferEq28,0.89833381,436.682,431.211,+0.0055
10,FiniteDiffVolume,0.00001120,8.362,8.627,-0.0136
10,FiniteDiffVolume,0.00001849,8.372,8.746,-0.0190
10,FiniteDiffVolume,0.00004324,8.406,8.866,-0.0231
10,FiniteDiffVolume,0.00008319,8.462,8.746,-0.0143
10,FiniteDiffVolume,0.00017400,8.589,8.746,-0.0079
10,FiniteDiffVolume,0.00042419,8.937,8.866,+0.0035
10,FiniteDiffVolume,0.00092512,9.632,9.757,-0.0056
10,FiniteDiffVolume,0.00201758,11.139,11.187,-0.0019
10,FiniteDiffVolume,0.00372307,13.465,13.182,+0.0092
10,FiniteDiffVolume,0.00658917,17.303,16.862,+0.0112
10,FiniteDiffVolume,0.01215907,24.527,24.063,+0.0083
10,FiniteDiffVolume,0.01979462,33.966,31.634,+0.0309
10,FiniteDiffVolume,0.02923240,44.980,42.739,+0.0222
10,FiniteDiffVolume,0.04628251,63.327,60.163,+0.0223
10,FiniteDiffVolume,0.06375276,80.433,78.017,+0.0132
10,FiniteDiffVolume,0.10524286,115.956,115.998,-0.0002
10,FiniteDiffVolume,0.18368534,170.374,172.468,-0.0053
10,FiniteDiffVolume,0.32964799,247.837,252.947,-0.0089
10,FiniteDiffVolume,0.87366242,437.032,419.575,+0.0177
10,KiefferEq28,0.00001120,8.362,8.627,-0.0136
10,KiefferEq28,0.00001849,8.372,8.746,-0.0190
10,KiefferEq28,0.00004324,8.406,8.866,-0.0231
10,KiefferEq28,0.00008319,8.462,8.746,-0.0143
10,KiefferEq28,0.00017400,8.589,8.746,-0.0079
10,KiefferEq28,0.00042419,8.937,8.866,+0.0035
10,KiefferEq28,0.00092512,9.632,9.757,-0.0056
10,KiefferEq28,0.00201758,11.139,11.187,-0.0019
10,KiefferEq28,0.00372307,13.465,13.182,+0.0092
10,KiefferEq28,0.00658917,17.303,16.862,+0.0112
10,KiefferEq28,0.01215907,24.527,24.063,+0.0083
10,KiefferEq28,0.01979462,33.966,31.634,+0.0309
10,KiefferEq28,0.02923240,44.980,42.739,+0.0222
10,KiefferEq28,0.04628251,63.327,60.163,+0.0223
10,KiefferEq28,0.06375276,80.433,78.017,+0.0132
10,KiefferEq28,0.10524286,115.956,115.998,-0.0002
10,KiefferEq28,0.18368534,170.374,172.468,-0.0053
10,KiefferEq28,0.32964799,247.837,252.947,-0.0089
10,KiefferEq28,0.87366242,437.032,419.575,+0.0177
50,FiniteDiffVolume,0.00001235,37.651,33.412,+0.0519
50,FiniteDiffVolume,0.00002038,34.460,33.412,+0.0134
50,FiniteDiffVolume,0.00004700,34.486,32.959,+0.0197
50,FiniteDiffVolume,0.00010540,34.541,32.959,+0.0204
50,FiniteDiffVolume,0.00020000,34.631,32.511,+0.0274
50,FiniteDiffVolume,0.00040684,34.827,32.959,+0.0239
50,FiniteDiffVolume,0.00078277,35.183,32.511,+0.0343
50,FiniteDiffVolume,0.00148524,35.847,32.511,+0.0424
50,FiniteDiffVolume,0.00285764,37.139,32.959,+0.0519
50,FiniteDiffVolume,0.00668155,40.703,38.310,+0.0263
50,FiniteDiffVolume,0.01477608,48.075,47.034,+0.0095
50,FiniteDiffVolume,0.02405502,56.256,56.186,+0.0005
50,FiniteDiffVolume,0.04083133,70.386,69.931,+0.0028
50,FiniteDiffVolume,0.06200189,87.143,81.285,+0.0302
50,FiniteDiffVolume,0.10093726,115.363,114.422,+0.0036
50,FiniteDiffVolume,0.14496886,144.049,146.362,-0.0069
50,FiniteDiffVolume,0.26751291,211.451,220.612,-0.0184
50,FiniteDiffVolume,0.49364503,307.825,314.825,-0.0098
50,FiniteDiffVolume,0.91092955,440.153,425.353,+0.0149
50,KiefferEq28,0.00001235,34.453,33.412,+0.0133
50,KiefferEq28,0.00002038,34.460,33.412,+0.0134
50,KiefferEq28,0.00004700,34.486,32.959,+0.0197
50,KiefferEq28,0.00010540,34.541,32.959,+0.0204
50,KiefferEq28,0.00020000,34.631,32.511,+0.0274
50,KiefferEq28,0.00040684,34.827,32.959,+0.0239
50,KiefferEq28,0.00078277,35.183,32.511,+0.0343
50,KiefferEq28,0.00148524,35.847,32.511,+0.0424
50,KiefferEq28,0.00285764,37.139,32.959,+0.0519
50,KiefferEq28,0.00668155,40.703,38.310,+0.0263
50,KiefferEq28,0.01477608,48.075,47.034,+0.0095
50,KiefferEq28,0.02405502,56.256,56.186,+0.0005
50,KiefferEq28,0.04083133,70.386,69.931,+0.0028
50,KiefferEq28,0.06200189,87.143,81.285,+0.0302
50,KiefferEq28,0.10093726,115.363,114.422,+0.0036
50,KiefferEq28,0.14496886,144.049,146.362,-0.0069
50,KiefferEq28,0.26751291,211.451,220.612,-0.0184
50,KiefferEq28,0.49364503,307.826,314.825,-0.0098
50,KiefferEq28,0.91092955,440.153,425.353,+0.0149
100,FiniteDiffVolume,0.00001201,85.916,69.931,+0.0894
100,FiniteDiffVolume,0.00003658,65.605,68.044,-0.0158
100,FiniteDiffVolume,0.00020000,65.714,67.120,-0.0092
100,FiniteDiffVolume,0.00068102,66.032,68.981,-0.0190
100,FiniteDiffVolume,0.00213314,66.992,68.981,-0.0127
100,FiniteDiffVolume,0.00757358,70.556,70.894,-0.0021
100,FiniteDiffVolume,0.02151936,79.487,80.181,-0.0038
100,FiniteDiffVolume,0.05246146,98.331,97.102,+0.0055
100,FiniteDiffVolume,0.09680781,123.333,120.856,+0.0088
100,FiniteDiffVolume,0.14700150,149.285,152.493,-0.0092
100,FiniteDiffVolume,0.26381391,202.411,223.650,-0.0433
100,FiniteDiffVolume,0.41191079,259.524,278.362,-0.0304
100,FiniteDiffVolume,0.80363969,378.974,408.253,-0.0323
100,KiefferEq28,0.00001201,65.589,69.931,-0.0278
100,KiefferEq28,0.00003658,65.605,68.044,-0.0158
100,KiefferEq28,0.00020000,65.714,67.120,-0.0092
100,KiefferEq28,0.00068102,66.032,68.981,-0.0190
100,KiefferEq28,0.00213314,66.991,68.981,-0.0127
100,KiefferEq28,0.00757358,70.556,70.894,-0.0021
100,KiefferEq28,0.02151936,79.486,80.181,-0.0038
100,KiefferEq28,0.05246146,98.331,97.102,+0.0055
100,KiefferEq28,0.09680781,123.333,120.856,+0.0088
100,KiefferEq28,0.14700150,149.285,152.493,-0.0092
100,KiefferEq28,0.26381391,202.411,223.650,-0.0433
100,KiefferEq28,0.41191079,259.524,278.362,-0.0304
100,KiefferEq28,0.80363969,378.974,408.253,-0.0323
200,FiniteDiffVolume,0.00001105,188.057,146.362,+0.1089
200,FiniteDiffVolume,0.00003139,180.191,146.362,+0.0903
200,FiniteDiffVolume,0.00016005,146.267,146.362,-0.0003
200,FiniteDiffVolume,0.00056826,141.390,146.362,-0.0150
200,FiniteDiffVolume,0.00277916,141.841,144.374,-0.0077
200,FiniteDiffVolume,0.00973078,143.255,150.421,-0.0212
200,FiniteDiffVolume,0.03503289,148.341,152.493,-0.0120
200,FiniteDiffVolume,0.11283086,163.434,165.535,-0.0055
200,FiniteDiffVolume,0.34370953,204.168,195.060,+0.0198
200,FiniteDiffVolume,0.88591224,283.421,263.542,+0.0316
200,KiefferEq28,0.00001105,145.696,146.362,-0.0020
200,KiefferEq28,0.00003139,145.700,146.362,-0.0020
200,KiefferEq28,0.00016005,145.727,146.362,-0.0019
200,KiefferEq28,0.00056826,145.811,146.362,-0.0016
200,KiefferEq28,0.00277916,146.267,144.374,+0.0057
200,KiefferEq28,0.00973078,147.696,150.421,-0.0079
200,KiefferEq28,0.03503289,152.834,152.493,+0.0010
200,KiefferEq28,0.11283086,168.069,165.535,+0.0066
200,KiefferEq28,0.34370953,209.087,195.060,+0.0302
200,KiefferEq28,0.88591224,288.626,263.542,+0.0395
```
