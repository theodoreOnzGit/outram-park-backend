// SPDX-License-Identifier: GPL-3.0-only
//
// Dump driver 8 for gh:#1393 round 2: OFFBEAT 80e84450a115b0c411e1bfa5d166379f6bf6c084
// MatproCreepModel (rate `fTrial` and Newton derivative `fTrialDev`) and powerLaw
// (`creepRate`) pointwise kernels, transcribed.  Same route and caveats as
// dump_conductivity.cpp.  `sigmaEff` is an input (the stress state, the implicit loop
// and the relaxation are not exercised); `F = heatSource/312e-13` is emitted as the
// fission rate input.
//
// Output: name <inputs> scale value
// Build: g++ -O2 -ffp-contract=off -o dump_creep dump_creep.cpp
#include <cmath>
#include <cstdio>
#include <algorithm>
#include <initializer_list>

typedef double scalar;
using std::pow; using std::exp;
static scalar A_(scalar x) { return std::fabs(x); }
static scalar min(scalar a, scalar b) { return (b < a) ? b : a; }
static scalar max(scalar a, scalar b) { return (a < b) ? b : a; }
static const scalar GREAT = 1e15;
namespace Foam { inline double log(double x) { return std::log(x); } }

// SakaiCorrection_ -> sakai ; returns rate (fTrial) and derivative (fTrialDev)
static void matpro(scalar T, scalar sigmaEff, scalar heatSource, scalar grainRadius, scalar rho, scalar OM,
                   bool SakaiCorrection_, scalar* fTrial_out, scalar* fTrialDev_out, scalar* scale)
{
    scalar A1 = 0.3919, A2 = 1.3100e-19, A3 = -87.7, A4 = 2.0391e-25, A6 = -90.5;
    scalar A7 = SakaiCorrection_ ? 7.78e-37 : 3.7226e-35;
    scalar R = 8.3143;
    scalar creepIncrementPrime1(0), creepIncrementPrime2(0), creepIncrementPrime3(0);
                scalar f = 1/(exp(-20/Foam::log(max(OM - 2, 1e-15)) - 8) + 1); // UPSTREAM
                scalar Q1 = 74829*f + 301762;                            // UPSTREAM
                scalar Q2 = 83143*f + 469191;                            // UPSTREAM
                scalar Q3 = 21759;                                       // UPSTREAM
                scalar F = heatSource/312e-13;                           // UPSTREAM (heatSource[cellI])
                scalar G = grainRadius*2*1e6;                            // UPSTREAM (grainRadius[cellI])
                scalar D = rho/10970*100;                                // UPSTREAM (rho[cellI])
                scalar creepIncrement1 =                                 // UPSTREAM
                ( (A1 + A2*F)/((A3 + D)*G*G) )                           // UPSTREAM
                *sigmaEff*exp(-Q1/R/T);                                  // UPSTREAM
                creepIncrementPrime1 =                                   // UPSTREAM
                ( (A1 + A2*F)/((A3 + D)*G*G) )*exp(-Q1/R/T);             // UPSTREAM
                scalar creepIncrement2 = ( A4/(A6 + D) )*pow(sigmaEff, 4.5)*exp(-Q2/R/T); // UPSTREAM
                creepIncrementPrime2 = 4.5*( A4/(A6 + D) )*pow(sigmaEff, 3.5)*exp(-Q2/R/T); // UPSTREAM
                scalar creepIncrement3(0.0);
                if(SakaiCorrection_)
                {
                    creepIncrement3 = A7*F*sigmaEff;                     // UPSTREAM
                    creepIncrementPrime3 = A7*F;                         // UPSTREAM
                }
                else
                {
                    creepIncrement3 = A7*F*sigmaEff*exp(-Q3/R/T);        // UPSTREAM
                    creepIncrementPrime3 = A7*F*exp(-Q3/R/T);            // UPSTREAM
                }
                scalar fTrial = min                                      // UPSTREAM
                (
                    (creepIncrement1 + creepIncrement2 + creepIncrement3), // UPSTREAM
                    GREAT
                );
                scalar fTrialDev = (creepIncrementPrime1 + creepIncrementPrime2 + creepIncrementPrime3); // UPSTREAM
    *fTrial_out = fTrial;
    *fTrialDev_out = fTrialDev;
    const scalar qmax = max(max(Q1, Q2), SakaiCorrection_ ? 0.0 : Q3) / (R * T);
    *scale = 3.0 + qmax;
}

static scalar powerLawRate(scalar sigmaEff, scalar B, scalar sigmaC, scalar n)
{
    const scalar creepRate = B * pow(sigmaEff*1e-6/sigmaC, n);           // UPSTREAM
    return creepRate;
}

static void pr(const char* name, std::initializer_list<scalar> c, scalar scale, scalar v)
{
    std::printf("%s", name);
    for (scalar x : c) std::printf(" %.17g", x);
    std::printf(" %.17g %.17g\n", scale, v);
}

int main()
{
    for (scalar T : {600.0, 800.0, 1000.0, 1200.0, 1500.0, 1800.0, 2200.0})
        for (scalar sigma : {1e5, 1e6, 1e7, 5e7, 1e8, 3e8})
            for (scalar Q : {1e8, 3e8, 6e8})
                for (scalar gr : {3e-6, 6e-6, 1e-5})
                    for (scalar p : {0.0, 0.03, 0.05})
                        for (scalar OM : {2.0, 2.0001, 2.01, 2.03})
                            for (int sakai : {1, 0})
                            {
                                const scalar rho = 10970.0 * (1.0 - p);
                                scalar r, d, sc;
                                matpro(T, sigma, Q, gr, rho, OM, sakai == 1, &r, &d, &sc);
                                pr("CreepMatproRate", {T, sigma, Q, gr, p, OM, (scalar)sakai, rho}, r * sc, r);
                                pr("CreepMatproDeriv", {T, sigma, Q, gr, p, OM, (scalar)sakai, rho}, d * sc, d);
                            }
    for (scalar B : {1e-5, 5e-3})
        for (scalar sigmaC : {1.0, 20.0, 100.0})
            for (scalar n : {1.0, 3.5, 5.0})
                for (scalar sigma : {1e5, 1e6, 1e7, 1e8, 2e9})
                {
                    scalar v = powerLawRate(sigma, B, sigmaC, n);
                    // port is per second: emit creepRate/3600; scale: pow base error x n
                    pr("CreepPowerLaw", {B, sigmaC, n, sigma}, A_(v / 3600.0) * (2.0 + n), v / 3600.0);
                }
    return 0;
}
