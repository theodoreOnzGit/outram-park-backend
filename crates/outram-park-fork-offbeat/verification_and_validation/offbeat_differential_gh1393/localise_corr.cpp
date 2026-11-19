// SPDX-License-Identifier: GPL-3.0-only
// Localises the non-bit-identical EPRI_KWU_CE rows (gh:#1393 round 2): upstream
// takes the cube root as pow(x, 1.0/3.0), the port as cbrt(x).  1.0/3.0 is not
// 1/3, so pow(x, 1.0/3.0) = cbrt(x) * x^(1.0/3.0 - 1/3) differs from cbrt(x) by
// a relative amount of about |1.0/3.0 - 1/3| * |ln x|.  Counts, on the grid of
// cubes the driver forms, how many rows differ and by how much; also the
// amplification in dt_pre = (S_trans - S0)/(S_pre - S0)*dt.
// Build: g++ -O2 -ffp-contract=off -o localise_corr localise_corr.cpp
#include <cmath>
#include <cstdio>
#include <initializer_list>
int main()
{
    int n = 0, diff = 0;
    double worst = 0, worstDt = 0;
    const double C1 = 6.3e9 * 1e-18, Q1R = 32324 / 1.9872;
    for (double S0 : {0.0, 1e-7, 1e-6, 1.9e-6})
        for (double T : {500.0, 600.0, 650.0, 672.9})
            for (double dt : {1.0, 3600.0, 86400.0, 8.64e6, 3.15e7})
            {
                const double x = C1 * std::exp(-Q1R / T) * (dt / 3600.0 / 24.0) + std::pow(S0, 3.0);
                const double a = std::pow(x, 1.0 / 3.0), b = std::cbrt(x);
                n++;
                if (a != b) diff++;
                if (a != 0) worst = std::max(worst, std::fabs(a - b) / a);
                if (S0 < 2e-6 && a > 2e-6 && b > 2e-6)
                {
                    const double dta = (2e-6 - S0) / (a - S0) * dt, dtb = (2e-6 - S0) / (b - S0) * dt;
                    worstDt = std::max(worstDt, std::fabs(dta - dtb) / dta);
                }
            }
    std::printf("cube roots %d: pow(x,1.0/3.0) != cbrt(x) on %d; worst relative difference %.2e (|1.0/3.0-1/3| = %.2e)\n",
                n, diff, worst, (double)std::fabs((long double)(1.0 / 3.0) - 1.0L / 3.0L));
    std::printf("crossing rows: worst relative difference of dt_pre between the two cube roots %.2e\n", worstDt);
    return 0;
}
