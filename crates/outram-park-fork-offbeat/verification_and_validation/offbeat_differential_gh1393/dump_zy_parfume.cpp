// SPDX-License-Identifier: GPL-3.0-only
//
// Dump driver 5 for gh:#1393 round 2: OFFBEAT 80e84450a115b0c411e1bfa5d166379f6bf6c084
// YoungModulusMatproZy, PoissonRatioMatproZy, YoungModulusPARFUME{Buffer,PyC,SiC},
// YoungModulusSneadSiC and the constant Poisson classes, transcribed.  Same
// route and caveats as dump_conductivity.cpp.  Member look-ups
// (`phi_->internalField()[cellI]`, `T[cellI]`) are replaced by locals.
//
// Output: name <inputs> scale value
//
// Build: g++ -O2 -ffp-contract=off -o dump_zy_parfume dump_zy_parfume.cpp
#include <cmath>
#include <cstdio>
#include <algorithm>
#include <initializer_list>

typedef double scalar;
using std::pow; using std::exp; using std::min; using std::max;
static scalar A_(scalar x) { return std::fabs(x); }

// ---- YoungModulusMatproZy.C ------------------------------------------------
// scaleOut = sum of |terms| of the numerator over |K3| (or the beta analogue)
static scalar youngMatproZy(scalar Ti, scalar fluence_cm2, scalar oxC_, scalar cW_, scalar* scaleOut)
{
    scalar par1 = 6.61e11, par2 = 5.912e8, par3 = 2.6e10, par4 = 0.88, par5 = 0.12, par6 = 1e25,
           par7 = 1.088e11, par8 = 5.475e7, par9 = 9.21e10, par10 = 4.05e7;
        const scalar phi = fluence_cm2 * 1e4;                            // (phi_->internalField()[cellI] * 1e4)
        scalar nominalValue(0.0);
        if (Ti < 1073)
        {
            scalar K1 = (par1 + par2*Ti)*oxC_;                           // UPSTREAM
            scalar K2 = -par3*cW_;                                       // UPSTREAM
            scalar K3 = par4 + par5*exp(-phi/par6);                      // UPSTREAM
            nominalValue = (par7 - par8*Ti + K1 + K2) / K3;              // UPSTREAM
            *scaleOut = (A_(par7) + A_(par8*Ti) + A_(K1) + A_(K2)) / K3;
        }
        else if (Ti < 1273)
        {
            scalar TAlpha = 1073;                                        // UPSTREAM
            scalar TBeta = 1273;                                         // UPSTREAM
            scalar K1 = (par1 + par2*TAlpha)*oxC_;                       // UPSTREAM
            scalar K2 = -par3*cW_;                                       // UPSTREAM
            scalar K3 = par4 + par5*exp(-phi/par6);                      // UPSTREAM
            scalar EAlpha = (par7 - par8*TAlpha + K1 + K2) / K3;         // UPSTREAM
            scalar EBeta = par9 - par10*TBeta;                           // UPSTREAM
            nominalValue = EAlpha*(TBeta - Ti)/(TBeta - TAlpha) + EBeta*(Ti - TAlpha)/(TBeta - TAlpha); // UPSTREAM
            *scaleOut = (A_(par7) + A_(par8*TAlpha) + A_(K1) + A_(K2)) / K3 * (TBeta - Ti) / (TBeta - TAlpha)
                      + (A_(par9) + A_(par10*TBeta)) * (Ti - TAlpha) / (TBeta - TAlpha);
        }
        else
        {
            nominalValue = (par9- par10*Ti);                             // UPSTREAM
            *scaleOut = A_(par9) + A_(par10*Ti);
        }
    return nominalValue;
}

// ---- PoissonRatioMatproZy.C : G with phiG (raw field value or phi*1e4) -------
static scalar shearMatproZy(scalar Ti, scalar phi, scalar oxC_, scalar cW_, scalar* scaleOut)
{
    scalar par1 = 7.07e11, par2 = 2.315e8, par3 = 2.6e10, par4 = 0.88, par5 = 0.12, par6 = 1e25,
           par7 = 4.04e10, par8 = 2.168e7, par9 = 3.49e10, par10 = 1.66e7;
    scalar G(0.0);
        if (Ti < 1073) //- Alpha phase
        {
            scalar K1 = (par1 - par2*Ti)*oxC_;                           // UPSTREAM
            scalar K2 = -par3*cW_;                                       // UPSTREAM
            scalar K3 = par4 + par5*exp(-phi/par6);                      // UPSTREAM
            G = (par7 - par8*Ti + K1 + K2) / K3;                         // UPSTREAM
            *scaleOut = (A_(par7) + A_(par8*Ti) + A_(K1) + A_(K2)) / K3;
        }
        else if (Ti < 1273) //- Interpolate between alpha and beta values
        {
            scalar TAlpha = 1073;                                        // UPSTREAM
            scalar TBeta = 1273;                                         // UPSTREAM
            scalar K1 = (par1 - par2*TAlpha)*oxC_;                       // UPSTREAM
            scalar K2 = -par3*cW_;                                       // UPSTREAM
            scalar K3 = par4 + par5*exp(-phi/par6);                      // UPSTREAM
            scalar GAlpha = (par7 - par8*TAlpha + K1 + K2) / K3;         // UPSTREAM
            scalar GBeta = par9 - par10*TBeta;                           // UPSTREAM
            G = GAlpha*(TBeta - Ti)/(TBeta - TAlpha) + GBeta*(Ti - TAlpha)/(TBeta - TAlpha); // UPSTREAM
            *scaleOut = (A_(par7) + A_(par8*TAlpha) + A_(K1) + A_(K2)) / K3 * (TBeta - Ti) / (TBeta - TAlpha)
                      + (A_(par9) + A_(par10*TBeta)) * (Ti - TAlpha) / (TBeta - TAlpha);
        }
        else //- Beta phase
        {
             G = (par9 - par10*Ti);                                      // UPSTREAM
             *scaleOut = A_(par9) + A_(par10*Ti);
        }
    return G;
}
// nu = E/(2G) - 1 ; scaleOut = (nu+1) * (condE + condG)
static scalar nuMatproZy(scalar Ti, scalar fluence_cm2, scalar oxC_, scalar cW_, bool consistent, scalar* scaleOut)
{
    scalar sE, sG;
    const scalar E = youngMatproZy(Ti, fluence_cm2, oxC_, cW_, &sE);
    const scalar phiG = consistent ? fluence_cm2 * 1e4 : fluence_cm2;
    const scalar G = shearMatproZy(Ti, phiG, oxC_, cW_, &sG);
    const scalar nominalValue = (E/(2*G) - 1);                           // UPSTREAM
    *scaleOut = (nominalValue + 1) * (sE / A_(E) + sG / A_(G)) + 1.0 * 0;
    return nominalValue;
}

// ---- YoungModulusPARFUMEBuffer.C ---------------------------------------------
static scalar youngBuffer(scalar T, scalar fluence_cm2, scalar rho)
{
    scalar par1 = 25.5, par2 = 0.384, par3 = 0.324e-3, par4 = 0.23, par5 = 1.5e-4, par6 = 20.0;
        const scalar Ti = T - 273.15;                                    // (T[cellI] - 273.15)
        scalar phi = fluence_cm2 * 1e4 / 1e25;                           // (phi_->internalField()[cellI] * 1e4 / 1e25)
        phi = std::min(phi, 3.96);                                       // UPSTREAM
        scalar nominalValue(0.0);
        nominalValue = (par1*(par2+par3*rho)*(1.0+par4*phi)*(1.0+par5*(Ti-par6)))*1e9; // UPSTREAM
    return nominalValue;
}

// ---- YoungModulusPARFUMEPyC.C --------------------------------------------------
static scalar youngPyC(scalar T, scalar fluence_cm2, scalar rho, scalar BAF_, scalar Lc_)
{
    scalar par1 = 25.5, par2 = 0.384, par3 = 0.324e-3, par4 = 1.463, par5 = -0.463, par6 = 2.985,
           par7 = -0.0662, par8 = 0.23, par9 = 0.00015, par10 = 20.0, par11 = 0.481, par12 = 0.519;
        const scalar Ti = T - 273.15;                                    // (T[cellI] - 273.15)
        scalar phi = fluence_cm2*1e4 / 1e25;                             // (phi_->internalField()[cellI]*1e4 / 1e25)
        phi = std::min(phi, 3.96);                                       // UPSTREAM
        const scalar Er = par1*(par2+par3*rho)*(par4+par5*BAF_)*(par6+par7*Lc_) // UPSTREAM
                          *(1.0+par8*phi)*(1.0+par9*(Ti-par10));         // UPSTREAM
        const scalar Et = par1*(par2+par3*rho)*(par11+par12*BAF_)*(par6+par7*Lc_) // UPSTREAM
                          *(1.0+par8*phi)*(1.0+par9*(Ti-par10));         // UPSTREAM
        scalar nominalValue(0.0);
        nominalValue = ((Er + 2.0*Et) / 3.0)*1e9;                        // UPSTREAM
    return nominalValue;
}

// ---- YoungModulusPARFUMESiC.C ----------------------------------------------------
static scalar youngSiC(scalar T, scalar* scaleOut)
{
    scalar temp1 = 25.0, temp2 = 940.0, temp3 = 1215.0, temp4 = 1600.0, E1 = 428.0, E2 = 375.0, E3 = 340.0, E4 = 198.0;
        const scalar Ti = T - 273.15;                                    // (T[cellI] - 273.15)
        scalar nominalValue(0.0);
        scalar sc = 0;
        if (Ti <= temp1)
        {
          nominalValue = E1;                                             // UPSTREAM
          sc = E1;
        }
        else if (Ti > temp1 && Ti <= temp2)
        {
          nominalValue = (E2-E1)/(temp2-temp1)*(Ti-temp1) + E1;          // UPSTREAM
          sc = A_((E2-E1)/(temp2-temp1)*(Ti-temp1)) + E1;
        }
        else if (Ti > temp2 && Ti <= temp3)
        {
          nominalValue = (E3-E2)/(temp3-temp2)*(Ti-temp2) + E2;          // UPSTREAM
          sc = A_((E3-E2)/(temp3-temp2)*(Ti-temp2)) + E2;
        }
        else if (Ti > temp3 && Ti <= temp4)
        {
          nominalValue = (E4-E3)/(temp4-temp3)*(Ti-temp3) + E3;          // UPSTREAM
          sc = A_((E4-E3)/(temp4-temp3)*(Ti-temp3)) + E3;
        }
        else
        {
          nominalValue = E4;                                             // UPSTREAM
          sc = E4;
        }
    *scaleOut = 1e9 * sc;
    return 1e9*nominalValue;                                             // UPSTREAM (sf[cellI] = 1e9*nominalValue)
}

// ---- YoungModulusSneadSiC.C ----------------------------------------------------------
static scalar youngSnead(scalar Ti, scalar Vp_, scalar C, scalar* scaleOut)
{
    scalar E0 = 460e9, T0 = 962.0, B = 0.04e9;
        scalar nominalValue(0.0);
        nominalValue = E0*exp(-C*Vp_)-B*Ti*exp(-T0/Ti);                  // UPSTREAM
    *scaleOut = E0*exp(-C*Vp_) * (1 + A_(C * Vp_)) + B*Ti*exp(-T0/Ti) * (1 + T0 / Ti);
    return nominalValue;
}

static void pr(const char* name, std::initializer_list<scalar> c, scalar scale, scalar v)
{
    std::printf("%s", name);
    for (scalar x : c) std::printf(" %.17g", x);
    std::printf(" %.17g %.17g\n", scale, v);
}

int main()
{
    scalar sc, v;
    const scalar Ts[] = {290, 300, 400, 600, 800, 1000, 1072.9, 1073, 1100, 1200, 1272.9, 1273, 1400, 1600, 1800};
    for (scalar T : Ts)
        for (scalar f : {0.0, 1e20, 1e21, 5e21})
            for (scalar oxC : {0.0, 0.001})
                for (scalar cW : {0.0, 0.1})
                {
                    v = youngMatproZy(T, f, oxC, cW, &sc);               pr("EMatproZy", {T, f, oxC, cW}, sc, v);
                    v = nuMatproZy(T, f, oxC, cW, true, &sc);            pr("NuMatproZy_cons", {T, f, oxC, cW}, sc, v);
                    v = nuMatproZy(T, f, oxC, cW, false, &sc);           pr("NuMatproZy_asw", {T, f, oxC, cW}, sc, v);
                }
    for (scalar T : {293.15, 300.0, 500.0, 900.0, 1300.0, 1800.0, 2200.0, 2273.15})
        for (scalar f : {0.0, 1e20, 1e21, 5e21})
            for (scalar rho : {1000.0, 1900.0, 2000.0})
            {
                v = youngBuffer(T, f, rho);                              pr("EParfumeBuffer", {T, f, rho}, std::fabs(v), v);
                for (scalar baf : {1.0, 1.05, 1.3})
                    for (scalar lc : {20.0, 30.0})
                    {
                        v = youngPyC(T, f, rho, baf, lc);                pr("EParfumePyC", {T, f, rho, baf, lc}, std::fabs(v), v);
                    }
            }
    for (scalar T : {298.15, 300.0, 500.0, 900.0, 1000.0, 1213.15, 1215 + 273.15, 1400.0, 1700.0, 1873.15})
    {
        v = youngSiC(T, &sc);                                            pr("EParfumeSiC", {T}, sc, v);
    }
    for (scalar T : {300.0, 500.0, 900.0, 1300.0, 1873.0})
        for (scalar por : {0.0, 0.05, 0.1})
            for (scalar C : {0.0, 2.9})
            {
                v = youngSnead(T, por, C, &sc);                          pr("ESneadSiC", {T, por, C}, sc, v);
            }
    return 0;
}
