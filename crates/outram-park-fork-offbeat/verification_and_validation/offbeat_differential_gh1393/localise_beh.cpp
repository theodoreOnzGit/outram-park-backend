// SPDX-License-Identifier: GPL-3.0-only
// Localises the non-bit-identical densification / relocation / swelling rows
// (gh:#1393 round 2): re-evaluates each family twice in C++ (glibc libm), once
// in upstream's operation order and once in the port's operation order, and
// counts the grid rows where the two differ.  If that count equals the Rust
// test's "not bit-identical" count, the operation-order difference is the
// whole cause (libm differences would add rows).
// Build: g++ -O2 -ffp-contract=off -o localise_beh localise_beh.cpp
#include <cmath>
#include <cstdio>
#include <initializer_list>
using std::pow; using std::exp; using std::min;

int main()
{
    // --- DensEmpirical: upstream (asym/3)*(1-exp(-Bui/tau)); port (asym*(1-exp(-bu/tau)))/3 with bu = (Bu/1000/0.881)*1000
    int nE = 0, dE = 0, dE_conv = 0, dE_assoc = 0;
    for (double frac : {0.9, 0.95, 0.97})
        for (double dc : {0.5, 1.5})
            for (double tau : {1000.0, 5000.0})
                for (double Bu : {0.0, 100.0, 1e3, 5e3, 1e4, 2e4, 5e4, 1e5})
                {
                    nE++;
                    const double dLmax = -dc / (frac * 100 + dc) / 3;
                    const double Bui = Bu / 0.881;
                    const double up = dLmax * (1 - exp(-Bui / tau));
                    const double asym = -dc / (frac * 100 + dc);
                    const double bu_port = (Bu / 1000 / 0.881) * 1000;
                    const double port = (asym * (1 - exp(-bu_port / tau))) / 3;
                    if (up != port) dE++;
                    if (Bui != bu_port) dE_conv++;
                    // association only (same Bui)
                    const double port_assoc = (asym * (1 - exp(-Bui / tau))) / 3;
                    if (up != port_assoc) dE_assoc++;
                }
    std::printf("DensEmpirical rows %d: upstream != port-order %d; of which burnup conversion (Bu/1000/0.881*1000 != Bu/0.881) differs on %d rows; association alone differs on %d rows\n",
                nE, dE, dE_conv, dE_assoc);

    // --- Relocation
    int nR = 0, dR = 0, dR_assoc = 0;
    const double pi = M_PI;
    for (double D : {0.0082, 0.0093})
        for (double Gap : {1.7e-4, 2.5e-4})
            for (double Q : {1e8, 2e8, 3e8, 5e8, 8e8, 1.2e9})
                for (double Buslice : {0.0, 40.0, 82.5, 82.51, 82.6, 91.0, 450.0, 1e3, 3e3, 4.4e3, 4.5e3, 6e3, 1e4, 3e4, 5e4, 1e5})
                    for (int mod : {1, 0})
                    {
                        nR++;
                        const double Bu = Buslice / 1000 / 0.881;
                        const double Qp = Q * pi * pow(D / 2.0, 2.0) / 1000;
                        double deltaGap, closure;
                        if (mod == 1)
                        {
                            if (Bu < 0.0937) { deltaGap = 0.055 * 100; closure = 0.055; }
                            else
                            {
                                double reloc = Qp < 20 ? 0.345 : (Qp <= 40 ? 0.345 + (Qp - 20.0) / 200.0 : 0.445);
                                deltaGap = (0.055 + min(reloc, reloc * (0.5795 + 0.2447 * std::log(Bu)))) * 100;
                                closure = 0.055 + min(reloc, reloc * (0.5795 + 0.2447 * std::log(Bu)));
                            }
                        }
                        else
                        {
                            double PF = (Qp - 20) * 5 / 20;
                            double FBu = Bu < 5 ? Bu / 5 : 1;
                            if (Qp < 20 + 1e-6) deltaGap = (30 + 10 * FBu);
                            else if (Qp < 40 + 1e-6) deltaGap = (28 + PF + (12 + PF) * FBu);
                            else deltaGap = (32 + 18 * FBu);
                            closure = deltaGap / 100;
                        }
                        const double up = deltaGap * (Gap / D) / 100;
                        const double port = closure * (Gap / D);
                        if (up != port) dR++;
                        if (deltaGap * 1.0 / 100 != closure) dR_assoc++;
                    }
    std::printf("Reloc rows %d: upstream != port-order %d (deltaGap/100 != closure on %d rows)\n", nR, dR, dR_assoc);

    // --- SwFrapcon: upstream (Bui-a1)*b1*rho/3 ; port ((burnup-LOW)*RATE)*rho then /3, burnup=(Bu/1000/0.881)*1000
    int nF = 0, dF = 0, dF_conv = 0;
    for (double p : {0.0, 0.05, 0.12})
        for (double Bu : {0.0, 1e3, 5e3, 5.29e3, 5.3e3, 5.4e3, 1e4, 2e4, 5e4, 7e4, 7.05e4, 7.1e4, 8e4, 1e5})
        {
            nF++;
            const double rho = 10960.0 * (1.0 - p);
            const double Bui = Bu / 0.881;
            const double bu = (Bu / 1000 / 0.881) * 1000;
            const double b1 = 2.974e+10 * 2.315e-23 * 86.4, b2 = 2.974e+10 * 3.211e-23 * 86.4;
            double up, port;
            if (Bui < 6000) up = 0.0; else if (Bui < 80000) up = (Bui - 6000) * b1 * rho / 3;
            else up = ((80000 - 6000) * b1 + (Bui - 80000) * b2) * rho / 3;
            if (bu < 6000) port = 0.0; else if (bu < 80000) port = ((bu - 6000) * b1 * rho) / 3;
            else port = (((80000 - 6000) * b1 + (bu - 80000) * b2) * rho) / 3;
            if (up != port) dF++;
            if (Bui != bu) dF_conv++;
        }
    std::printf("SwFrapcon rows %d: upstream != port-order %d (burnup conversion differs on %d rows)\n", nF, dF, dF_conv);

    // --- AIM1: upstream (1/3)*par1*exp(par2*pow(x,2))*pow(f*1e-22,par5)*1e-2 ; port (0.01*a*exp(b*x*x)*pow(f/1e26,p))/3
    int nA = 0, dA = 0, dA_phi = 0;
    for (double T : {500.0, 600.0, 700.0, 763.15, 800.0, 900.0, 1000.0})
        for (double fc : {1e21, 5e21, 1e22, 5e22})
        {
            nA++;
            const double Ti = T - 273.15, par1 = 1.3e-5, par2 = -2.5, par3 = 490, par4 = 100, par5 = 3.9;
            const double fl = fc * 1e-22;
            const double up = 1.0 / 3.0 * par1 * exp(par2 * (pow((Ti - par3) / par4, 2))) * pow(fl, par5) * 1e-2;
            const double fm2 = fc * 1e4;
            const double phi = fm2 / 1.0e26;
            const double x = (Ti - par3) / par4;
            const double port = (0.01 * par1 * exp(par2 * x * x) * pow(phi, par5)) / 3.0;
            if (up != port) dA++;
            if (fl != phi) dA_phi++;
        }
    std::printf("SwAim1 rows %d: upstream != port-order %d (fluence base f*1e-22 != (f*1e4)/1e26 on %d rows)\n", nA, dA, dA_phi);
    return 0;
}
