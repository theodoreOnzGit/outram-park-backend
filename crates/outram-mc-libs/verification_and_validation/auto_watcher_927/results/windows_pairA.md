### pairA: pass rate of each part of the window criterion on stationary windows (40 seeds)

| trace | start | W | windows | k MSER in first half | k Geweke no drift | k both | H(5^3) both | k and H stationary | normality p>=.05 | normality p>=.01 | all (alpha .05) |
|---|---|---|---|---|---|---|---|---|---|---|---|
(c_true of the default mesh = 70)

| production 10000, start >= c_true (should pass) | 70 | 20 | 1320 | 1.00 | 0.36 | 0.36 | 0.24 | 0.09 | 0.93 | 0.97 | 0.09 |
| production 10000, end <= c_true (should FAIL) | 0 | 20 | 440 | 1.00 | 0.34 | 0.34 | 0.04 | 0.01 | 0.90 | 0.92 | 0.01 |
| production 10000, start >= c_true (should pass) | 70 | 30 | 1240 | 0.65 | 0.49 | 0.49 | 0.31 | 0.14 | 0.94 | 0.98 | 0.13 |
| production 10000, end <= c_true (should FAIL) | 0 | 30 | 360 | 0.66 | 0.47 | 0.47 | 0.06 | 0.02 | 0.87 | 0.89 | 0.02 |
| production 10000, start >= c_true (should pass) | 70 | 50 | 1080 | 0.56 | 0.57 | 0.53 | 0.31 | 0.16 | 0.96 | 0.99 | 0.16 |
| production 10000, end <= c_true (should FAIL) | 0 | 50 | 200 | 0.53 | 0.53 | 0.47 | 0.03 | 0.01 | 0.80 | 0.81 | 0.01 |

Entropy half alone, W = 30, by mesh:

| mesh | stationary windows pass | transient windows pass |
|---|---|---|
| 3^3 | 0.29 (1240) | 0.03 (360) |
| 5^3 | 0.31 (1240) | 0.06 (360) |
| 8^3 | 0.31 (1240) | 0.04 (360) |
| 12^3 | 0.30 (1240) | 0.03 (360) |
