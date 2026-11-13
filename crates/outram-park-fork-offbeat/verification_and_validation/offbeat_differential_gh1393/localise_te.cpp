// SPDX-License-Identifier: GPL-3.0-only
// Localises the non-bit-identical thermal-expansion rows (gh:#1393 round 2):
// counts, on the fixture temperature grid, the elementary operations in which
// the upstream expression and the port's expression round differently.
// Build: g++ -O2 -ffp-contract=off -o localise_te localise_te.cpp
#include <cmath>
#include <cstdio>
int main()
{
    const double Ts[] = {273, 290, 293, 293.15, 300, 333.33, 400, 500, 600, 700, 800, 900, 923, 923.5,
        1000, 1073, 1085.5, 1100, 1200, 1273, 1273.15, 1274, 1300, 1500, 1700, 1800, 2000, 2400, 2800, 3000};
    const double Trefs[] = {293.15, 300, 600};
    int n = 0, sqK = 0, cubeK = 0, sqC = 0, cubeC = 0, hornerMartinLo = 0, hornerMartinHi = 0, hornerSnead = 0;
    int hornerUPu = 0, bufDiff = 0, bufN = 0, gehrMis = 0, gehrN = 0;
    for (double T : Ts)
    {
        n++;
        const double Tc = T - 273.15;
        if (std::pow(T, 2) != T * T) sqK++;
        if (std::pow(T, 3) != T * T * T) cubeK++;
        if (std::pow(Tc, 2) != Tc * Tc) sqC++;
        if (std::pow(Tc, 3) != Tc * Tc * Tc) cubeC++;
        { double p1 = 9.828e-6, p2 = -6.39e-10, p3 = 1.33e-12, p4 = -1.757e-17;
          if (p1 + p2 * T + p3 * std::pow(T, 2) + p4 * std::pow(T, 3) != p1 + T * (p2 + T * (p3 + T * p4))) hornerMartinLo++; }
        { double p1 = 1.1833e-5, p2 = -5.013e-9, p3 = 3.756e-12, p4 = -6.125e-17;
          if (p1 + p2 * T + p3 * std::pow(T, 2) + p4 * std::pow(T, 3) != p1 + T * (p2 + T * (p3 + T * p4))) hornerMartinHi++; }
        { double p1 = -1.8276, p2 = 0.0178, p3 = -1.5544e-5, p4 = 4.5246e-9;
          if (p1 + p2 * T + p3 * std::pow(T, 2) + p4 * std::pow(T, 3) != p1 + T * (p2 + T * (p3 + T * p4))) hornerSnead++; }
        { double p1 = -3.9735e-4, p2 = 8.4955e-6, p3 = 2.15130e-9, p4 = 3.7143e-16;
          if (p1 + p2 * Tc + p3 * std::pow(Tc, 2) + p4 * std::pow(Tc, 3) != p1 + Tc * (p2 + Tc * (p3 + Tc * p4))) hornerUPu++; }
        { double p1 = -3.101e-4, p2 = 1.545e-5, p3 = 2.75e-9;
          gehrN++;
          if (p1 + p2 * Tc + p3 * std::pow(Tc, 2) != p1 + p2 * Tc + p3 * Tc * Tc) gehrMis++; }
        for (double Tref : Trefs)
        {
            bufN++;
            if ((T - 273.15) - (Tref - 273.15) != T - Tref) bufDiff++;
        }
    }
    std::printf("grid points %d\n", n);
    std::printf("pow(T,2)!=T*T: %d   pow(T,3)!=T*T*T: %d   (kelvin)\n", sqK, cubeK);
    std::printf("pow(Tc,2)!=Tc*Tc: %d   pow(Tc,3)!=Tc*Tc*Tc: %d   (celsius)\n", sqC, cubeC);
    std::printf("Martin low  explicit-with-pow != Horner: %d / %d\n", hornerMartinLo, n);
    std::printf("Martin high explicit-with-pow != Horner: %d / %d\n", hornerMartinHi, n);
    std::printf("Snead       explicit-with-pow != Horner: %d / %d\n", hornerSnead, n);
    std::printf("MatproUPuO2 PuO2 explicit-with-pow != Horner (celsius): %d / %d\n", hornerUPu, n);
    std::printf("Gehr p3*pow(Tc,2) != p3*Tc*Tc: %d / %d\n", gehrMis, gehrN);
    std::printf("PARFUME buffer (T-273.15)-(Tref-273.15) != T-Tref: %d / %d (T,Tref) pairs\n", bufDiff, bufN);
    return 0;
}
