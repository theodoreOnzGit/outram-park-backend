// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 OUTRAM PARK contributors
//
// gh:#1872 (round 50): dump EVERY property of GeN-Foam's real `Foam::H2` class (652b3da, ESI v2506, the
// UNMODIFIED libFFSSrc2) on a (p, T) grid at 17 significant digits ("%.17g" round-trips a double exactly).
// Upstream's own test application `tests/hydrogenThermophysicalProperties/Test-hydrogenThermophysicalProperties.C`
// only compares against a handful of tabulated reference values at a 5-15 % tolerance (and prints pass/fail), so
// it cannot serve as a bit-level reference; this driver calls the same class on a far larger grid.
//
// The derived members (n_, alpha0_, b_, c_) are private (class default access); they are re-evaluated below with the
// SAME expressions as H2.C's constructor initialiser list, from the public accessors, and printed as K lines.
//
// Output:  "K <name> <value>" constants, "N <name> <name> ..." one header line naming the columns, then
//          "P <p> <T> <value> <value> ..."  one line per grid point (nan/inf as printed by printf).
#include "H2.H"
#include <cmath>
#include <cstdio>
#include <vector>

using namespace Foam;

int main()
{
    H2 h;

    const scalar R_(8.314472e3);
    const scalar n_(0.4986 + 1.1735*h.omega() + 0.4754*sqr(h.omega()));
    const scalar alpha0_(0.42748*sqr(R_*h.Tc()) / h.Pc());
    const scalar b_(0.08664*R_*h.Tc() / h.Pc());
    const scalar c_(R_*h.Tc() / (h.Pc() + alpha0_ / (h.Vc() * (h.Vc()+b_))) + b_ - h.Vc());
    std::printf("K n_ %.17g\nK alpha0_ %.17g\nK b_ %.17g\nK c_ %.17g\nK R_ %.17g\n", n_, alpha0_, b_, c_, R_);
    std::printf("K W %.17g\nK Tc %.17g\nK Pc %.17g\nK Vc %.17g\nK omega %.17g\n",
        h.W(), h.Tc(), h.Pc(), h.Vc(), h.omega());
    // constant-folded by the compiler in upstream's own translation unit (a correctly rounded pow at compile time)
    std::printf("K pow_0p297_3 %.17g\n", std::pow(0.297, 3.0));

    const char* names =
        "Kp x_H w_H omega11_11 omega22_11 omega22_22 omega11_12 omega22_12 A_12 B_12 C_12 "
        "densityH densityH2 rho psi CpMCv Hs Ha idealEntropyH2 idealEntropyH entropyH2 S "
        "specificEnthalpyH specificEnthalpyH2 h cvPerfectGasH2 cpPerfectGasH2 heatCapacityH heatCapacityH2 Cp B "
        "viscosity0 viscosity1 viscosityH mu thermalConductivityH2 kappa diffusionH2 D alphah Pr Hc";
    std::printf("N %s\n", names);

    // T [K]: cryogenic (tutorial inlet 83.3 K), critical point, the B() switch at 50 K, heatCapacityH switch at
    // 298 K, the omega11_11/omega22_11 switch at 1000 K, the cp sigmoid centres (137.8745, 1102.411), the
    // dissociation range to 10 000 K (correlations are tabulated to 6000 K; beyond is extrapolation)
    const std::vector<double> Ts = {
        14.0, 20.0, 30.0, 33.145, 40.0, 49.999, 50.0, 50.001, 60.0, 83.3, 83.33334, 100.0, 137.8745, 150.0,
        200.0, 250.0, 273.15, 297.999, 298.0, 298.001, 300.0, 400.0, 500.0, 600.0, 800.0, 999.999, 1000.0,
        1000.001, 1102.411, 1200.0, 1500.0, 1800.0, 2000.0, 2200.0, 2500.0, 2800.0, 3000.0, 3300.0, 3500.0,
        4000.0, 4500.0, 5000.0, 5500.0, 6000.0, 7000.0, 8000.0, 10000.0
    };
    // p [Pa]: 1 psia .. 4500 psia (upstream's own test), 1 atm, the NTP tutorial range 3.4-3.7 MPa, and higher
    const std::vector<double> ps = {
        1.0e3, 6894.76, 1.0e5, 101325.0, 1.0e6, 3.427e6, 3.7e6, 1.0e7, 3.1026420e7, 1.0e8
    };

    for (double p : ps)
    {
        for (double T : Ts)
        {
            const double v[] = {
                h.Kp(T), h.x_H(p, T), h.w_H(p, T),
                h.omega11_11(T), h.omega22_11(T), h.omega22_22(T), h.omega11_12(T), h.omega22_12(T),
                h.A_12(T), h.B_12(T), h.C_12(T),
                h.densityH(p, T), h.densityH2(p, T), h.rho(p, T), h.psi(p, T), h.CpMCv(p, T),
                h.Hs(p, T), h.Ha(p, T), h.idealEntropyH2(p, T), h.idealEntropyH(p, T), h.entropyH2(p, T),
                h.S(p, T), h.specificEnthalpyH(T), h.specificEnthalpyH2(p, T), h.h(p, T),
                h.cvPerfectGasH2(T), h.cpPerfectGasH2(T), h.heatCapacityH(T), h.heatCapacityH2(p, T),
                h.Cp(p, T), h.B(p, T),
                h.viscosity0(T), h.viscosity1(T), h.viscosityH(T), h.mu(p, T),
                h.thermalConductivityH2(p, T), h.kappa(p, T), h.diffusionH2(p, T), h.D(p, T),
                h.alphah(p, T), h.Pr(p, T), h.Hc()
            };
            std::printf("P %.17g %.17g", p, T);
            for (double x : v) std::printf(" %.17g", x);
            std::printf("\n");
        }
    }
    return 0;
}
