// SPDX-License-Identifier: GPL-3.0-only
//
// Dump driver 9 for gh:#1393 round 2: OFFBEAT 80e84450a115b0c411e1bfa5d166379f6bf6c084
// oxidationKineticsModel `EPRI_KWU_CE`, `CathcartPawel` and the low/high combination
// (`lowHighOxidationKineticsModel<EPRI_KWU_CE, CathcartPawel>`, switch at
// `lowTModel_.upperLimit() = 673 K`), transcribed.  Same route and caveats as
// dump_conductivity.cpp.  `S` is both input and output upstream; mode 1 enters
// with S = S0 (first nonlinear iteration of a step), mode 2 with S = the result
// of mode 1 (later iterations).
//
// Output: name S0 T phi_cm2 dt mode scale value
// Build: g++ -O2 -ffp-contract=off -o dump_corrosion dump_corrosion.cpp
#include <cmath>
#include <cstdio>
#include <initializer_list>

typedef double scalar;
using std::pow; using std::exp; using std::sqrt; using std::log;
namespace Foam { inline double exp(double x) { return std::exp(x); } }

static void EPRI_KWU_CE_correct(scalar& S, const scalar S0, const scalar Ti, const scalar phi, const scalar dt)
{
    const scalar C1 = 6.3e9*1e-18;										// UPSTREAM
    const scalar C2 = (8.04e7+2.59e8*pow(7.46e-15*phi,0.25))*1e-6;	    // UPSTREAM
    const scalar Q1R = 32324 / 1.9872;      							// UPSTREAM
    const scalar Q2R = 27374 / 1.9872;									// UPSTREAM
    scalar S_trans = 2e-6;                                               // UPSTREAM
    if( S <= S_trans )                                                   // UPSTREAM
    {
        S = pow                                                          // UPSTREAM
        (
            C1*exp(-Q1R/Ti)*(dt/3600.0/24.0)                             // UPSTREAM
            + pow(S0, 3.0)                                               // UPSTREAM
            , 1.0/3.0                                                    // UPSTREAM
        );
    }
    else if( S0 < S_trans )                                              // UPSTREAM
    {
        scalar S_pre = pow                                               // UPSTREAM
        (
            C1*exp(-Q1R/Ti)*(dt/3600.0/24.0) + pow(S0, 3.0),             // UPSTREAM
            1.0/3.0                                                      // UPSTREAM
        );
        scalar dt_pre = (S_trans - S0)/(S_pre - S0)*dt;                  // UPSTREAM
        scalar dt_post = dt - dt_pre;                                    // UPSTREAM
        S = S_trans + C2*exp(-Q2R/Ti)*dt_post/3600.0/24.0;               // UPSTREAM
    }
    else
    {
        S = S0 + C2*exp(-Q2R/Ti)*dt/3600.0/24.0;                         // UPSTREAM
    }
}

static void CathcartPawel_correct(scalar& S, const scalar S0, const scalar Ti, const scalar phi, const scalar dt)
{
    scalar As;
    scalar QsR;
    if ( Ti < 1800 )                                                     // UPSTREAM
    {
        As  = 7.82e-6;                                                   // UPSTREAM
        QsR = 20214;                                                     // UPSTREAM
    }
    else if (Ti < 1900)                                                  // UPSTREAM
    {
        QsR = 1900*1800/(1900-1800)                                      // UPSTREAM
            * log                                                        // UPSTREAM
              (
                2.98e-3*Foam::exp(-28420.0/1900.0)                       // UPSTREAM
                /                                                        // UPSTREAM
                7.82e-6*Foam::exp(-20214.0/1800.0)                       // UPSTREAM
              );
        As  = 7.82e-6*exp(QsR/1900.0);                                   // UPSTREAM
    }
    else
    {
        As  = 2.98e-3;                                                   // UPSTREAM
        QsR = 28420;                                                     // UPSTREAM
    }
    S = sqrt( As*exp(-QsR/Ti)*dt + pow(S0, 2.0) );                       // UPSTREAM
}

static void lowHigh(scalar& S, const scalar S0, const scalar Ti, const scalar phi, const scalar dt)
{
    if ( Ti < 673 ) EPRI_KWU_CE_correct(S, S0, Ti, phi, dt);
    else CathcartPawel_correct(S, S0, Ti, phi, dt);
}

static void pr(const char* name, std::initializer_list<scalar> c, scalar scale, scalar v)
{
    std::printf("%s", name);
    for (scalar x : c) std::printf(" %.17g", x);
    std::printf(" %.17g %.17g\n", scale, v);
}

int main()
{
    for (scalar S0 : {0.0, 1e-7, 1e-6, 1.9e-6, 2e-6, 2.1e-6, 5e-6, 2e-5})
        for (scalar T : {500.0, 600.0, 650.0, 672.9, 673.0, 700.0, 900.0, 1500.0, 1790.0, 1800.0, 1850.0, 1899.0, 1900.0, 2100.0})
            for (scalar phiCm2 : {0.0, 1e13, 1e14})
                for (scalar dt : {1.0, 3600.0, 86400.0, 8.64e6, 3.15e7})
                {
                    // mode 1: S = S0 on entry; mode 2: S = result of mode 1
                    scalar s1 = S0, s2;
                    EPRI_KWU_CE_correct(s1, S0, T, phiCm2, dt);
                    s2 = s1;
                    EPRI_KWU_CE_correct(s2, S0, T, phiCm2, dt);
                    pr("OxEpri", {S0, T, phiCm2, dt, 1}, 4.0 * s1, s1);
                    pr("OxEpri", {S0, T, phiCm2, dt, 2}, 4.0 * s2, s2);
                    s1 = S0; CathcartPawel_correct(s1, S0, T, phiCm2, dt);
                    pr("OxCathcart", {S0, T, phiCm2, dt, 1}, 4.0 * s1, s1);
                    s1 = S0; lowHigh(s1, S0, T, phiCm2, dt);
                    s2 = s1; lowHigh(s2, S0, T, phiCm2, dt);
                    pr("OxLowHigh", {S0, T, phiCm2, dt, 1}, 4.0 * s1, s1);
                    pr("OxLowHigh", {S0, T, phiCm2, dt, 2}, 4.0 * s2, s2);
                }
    return 0;
}
