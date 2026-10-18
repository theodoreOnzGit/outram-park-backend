### pairB: pass rate of each part of the window criterion on stationary windows (40 seeds)

| trace | start | W | windows | k MSER in first half | k Geweke no drift | k both | H(5^3) both | k and H stationary | normality p>=.05 | normality p>=.01 | all (alpha .05) |
|---|---|---|---|---|---|---|---|---|---|---|---|
(c_true of the default mesh = 120)

| production 10000, start >= c_true (should pass) | 120 | 20 | 2920 | 1.00 | 0.37 | 0.37 | 0.27 | 0.10 | 0.94 | 0.97 | 0.09 |
| production 10000, end <= c_true (should FAIL) | 0 | 20 | 840 | 1.00 | 0.33 | 0.33 | 0.23 | 0.07 | 0.92 | 0.94 | 0.07 |
| production 10000, start >= c_true (should pass) | 120 | 30 | 2840 | 0.64 | 0.48 | 0.48 | 0.34 | 0.16 | 0.94 | 0.98 | 0.15 |
| production 10000, end <= c_true (should FAIL) | 0 | 30 | 760 | 0.66 | 0.48 | 0.48 | 0.31 | 0.13 | 0.92 | 0.94 | 0.12 |
| production 10000, start >= c_true (should pass) | 120 | 50 | 2680 | 0.55 | 0.56 | 0.52 | 0.34 | 0.17 | 0.94 | 0.98 | 0.16 |
| production 10000, end <= c_true (should FAIL) | 0 | 50 | 600 | 0.56 | 0.56 | 0.52 | 0.26 | 0.14 | 0.90 | 0.92 | 0.13 |

Entropy half alone, W = 30, by mesh:

| mesh | stationary windows pass | transient windows pass |
|---|---|---|
| 3^3 | 0.32 (2840) | 0.29 (760) |
| 5^3 | 0.34 (2840) | 0.31 (760) |
| 8^3 | 0.34 (2840) | 0.34 (760) |
| 12^3 | 0.33 (2840) | 0.35 (760) |
