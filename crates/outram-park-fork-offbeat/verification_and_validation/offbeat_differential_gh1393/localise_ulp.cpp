// SPDX-License-Identifier: GPL-3.0-only
// Localises the 1-4 ulp conductivity differences (gh:#1393): compares, on the
// fixture temperature grid, the upstream operation with the port's operation.
// Build: g++ -O2 -ffp-contract=off -o localise_ulp localise_ulp.cpp
#include <cmath>
#include <cstdio>
#include <initializer_list>

int main()
{
    const double Ts[] = {290, 300, 333.33, 400, 500, 600, 700, 800, 900, 1000, 1100,
        1234.5678, 1364, 1500, 1600, 1700, 1800, 1834.5, 2000, 2200, 2400, 2600,
        2718.2818, 2800, 3000, 3113};
    int cube = 0, inv2 = 0, sq = 0, n = 0, tau = 0;
    for (double t : Ts)
    {
        n++;
        if (std::pow(t, 3.0) != t * t * t) cube++;          // Brancheria, Philipponneau
        if (1.5e9 * std::pow(t, -2.0) != 1.5e9 / (t * t)) inv2++;  // LanningBeyer
        if (std::pow(t, 2.0) != t * t) sq++;                // MatproUo2, Magni
    }
    const double Bus[] = {0, 5e3, 2.5e4, 5.5e4, 9e4, 1.2e5};
    for (double bu : Bus)
    {
        if (bu * 1e-3 / 9.5 / 100 != (bu * 1e-3) / 950.0) tau++;  // Philipponneau
    }
    // Properties (dump_properties.cpp): operand / association differences.
    int dens = 0, sck = 0, sch = 0, ns = 0;
    for (double p : {0.0, 0.05, 0.12, 0.2})
    {
        if (1 - (1 - p) != p) dens++;                       // Young MatproUo2/Mox: (1-densityFrac_) vs porosity
    }
    for (double t : Ts)
    {
        for (double p : {0.05, 0.12, 0.2})
        {
            double X = 218.74 * (1.0 + 0.1 * t) / 219.0;     // arbitrary E*fT/f273 stand-in
            double A = std::pow(1 - p, 2.0), B = 1.0 + p * 1.1;
            ns++;
            if ((X * A) / B != X * (A / B)) sck++;          // SckCen: nominal *= A/B vs (nominal*A)/B
        }
        double Ti = t - 273.15;
        double l = -3.101e-4 + 1.545e-5 * Ti + 2.75e-9 * std::pow(Ti, 2);
        double b = 1 / (1 + l);
        if (std::pow(b, 3) != b * b * b) sch++;             // Schumann: pow(.,3) vs powi/cube
    }
    std::printf("Young: 1-(1-p) != p on 4 porosities : %d\n", dens);
    std::printf("SckCen (X*A)/B != X*(A/B) on %d samples : %d\n", ns, sck);
    std::printf("Schumann pow(b,3) != b*b*b on %d T : %d\n", n, sch);
    std::printf("T grid points %d\n", n);
    std::printf("pow(T,3.0) != T*T*T : %d\n", cube);
    std::printf("C*pow(T,-2.0) != C/(T*T) (C=1.5e9) : %d\n", inv2);
    std::printf("pow(T,2.0) != T*T : %d\n", sq);
    std::printf("Bu*1e-3/9.5/100 != (Bu*1e-3)/950 on 6 burnups : %d\n", tau);
    return 0;
}
