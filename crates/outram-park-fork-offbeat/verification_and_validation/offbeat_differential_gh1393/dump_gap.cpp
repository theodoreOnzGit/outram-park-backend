// SPDX-License-Identifier: GPL-3.0-only
//
// Dump driver 7 for gh:#1393 round 2: OFFBEAT 80e84450a115b0c411e1bfa5d166379f6bf6c084
// `gapFRAPCON::kappa`, `gapFRAPCON::a` and the per-face body of
// `fuelRodGapFvPatchScalarField::hGap` (gas, radiation, contact; F = 1,
// delta = 0, relax = 1), transcribed.  Same route and caveats as
// dump_conductivity.cpp.  Species data are the defaults of gapFRAPCON.C
// (speciesW, conductivity_A, conductivity_B).  Species order is the port's
// (He, Ar, Kr, Xe), the upstream order being the dictionary's.
//
// Radiation: physicoChemical::sigma of OpenFOAM-ESI v2506 =
//   pi^2/60 * k^4 / ((h/(2 pi))^3 c^2), k = 1.38065e-23, h = 6.62607e-34,
//   c = 2.99792e8 (etc/controlDict:1098-1115), emitted as _Foam; and the CODATA
//   2018 value the port uses as _2018.
//
// Output: name <inputs> scale value
// Build: g++ -O2 -ffp-contract=off -o dump_gap dump_gap.cpp
#include <cmath>
#include <cstdio>
#include <algorithm>
#include <initializer_list>
#include <vector>

typedef double scalar;
using std::pow; using std::exp; using std::sqrt; using std::log;
static scalar min(scalar a, scalar b) { return (b < a) ? b : a; }
static scalar max(scalar a, scalar b) { return (a < b) ? b : a; }
static scalar sqr(scalar x) { return x * x; }
static scalar A_(scalar x) { return std::fabs(x); }
static const scalar SMALL = 1e-15;

static const double kFoam = 1.38065e-23, hFoam = 6.62607e-34, cFoam = 2.99792e8;
static double sigmaFoam()
{
    const double pi = M_PI;
    const double hr = hFoam / (2 * pi);
    return sqr(pi) / 60.0 * pow(kFoam, 4) / (pow(hr, 3) * sqr(cFoam));
}
static const double sigma2018 = 5.670374419e-8;

struct Gas
{
    std::vector<scalar> W, As, Bs, M;
};
static Gas makeGas(scalar fHe, scalar fAr, scalar fKr, scalar fXe)
{
    Gas g;
    const scalar allW[4] = {4.0026, 39.948, 83.8, 131.3};
    const scalar allA[4] = {2.531e-3, 4.092e-4, 1.966e-4, 9.825e-5};
    const scalar allB[4] = {0.7146, 0.6748, 0.7006, 0.7334};
    const scalar f[4] = {fHe, fAr, fKr, fXe};
    for (int i = 0; i < 4; i++)
        if (f[i] > 0) { g.W.push_back(allW[i]); g.As.push_back(allA[i]); g.Bs.push_back(allB[i]); g.M.push_back(f[i]); }
    return g;
}

// ---- gapFRAPCON::kappa -----------------------------------------------------
static scalar kappa(const Gas& g, const scalar T)
{
    const size_t n = g.W.size();
    std::vector<scalar> ks_(n, 0.0);
    for (size_t i = 0; i < n; i++) ks_[i] = g.As[i]*pow(T, g.Bs[i]);     // (ks_ = conductivityAs_*pow(T, conductivityBs_))
    scalar kMix(0.0);
    for (size_t specieI = 0; specieI < n; specieI++)
    {
            std::vector<scalar> deltas_(n, 0.0);
            deltas_[specieI] = 1.0;                                      // UPSTREAM
            scalar summTerm(0.0);
            scalar kI = ks_[specieI];                                    // UPSTREAM
            scalar nI = g.M[specieI];
            for (size_t specieJ = 0; specieJ < n; specieJ++)
            {
                    scalar phiIJ(0.0);
                    scalar psiIJ(0.0);
                    scalar kJ = ks_[specieJ];                            // UPSTREAM
                    scalar MI = g.W[specieI]/1000;
                    scalar MJ = g.W[specieJ]/1000;
                    phiIJ =                                              // UPSTREAM
                    (
                        pow( (1 + pow(kI/kJ, 0.5)*pow(MI/MJ, 0.25)), 2.0)
                    )/(pow(2, 1.5)*pow(1 + MI/MJ , 0.5));
                    psiIJ = phiIJ*                                       // UPSTREAM
                    (
                        1 + 2.41*
                        (
                            (MI - MJ)*(MI - 0.142*MJ)
                        )/(pow(MI + MJ, 2.0))
                    );
                    summTerm += (1-deltas_[specieJ])*psiIJ*g.M[specieJ]; // UPSTREAM (M_ -> g.M)
            }
            kMix += ( (kI*nI)/(nI + summTerm) ) ;                        // UPSTREAM
    }
    return kMix;
}

// ---- gapFRAPCON::a ----------------------------------------------------------
static scalar accommodation(const Gas& g, const scalar T)
{
    scalar a_He = 0.425 - (2.3e-4*min(T, 1300));                         // UPSTREAM
    scalar a_Xe = 0.749 - (2.5e-4*min(T, 1300));                         // UPSTREAM
    const scalar W_He = 4.0026;                                          // UPSTREAM
    const scalar W_Xe = 131.3;                                           // UPSTREAM
    scalar a_mix = 0;
    for (size_t i = 0; i < g.W.size(); i++)
    {
        scalar W_i = g.W[i];
        scalar a_i = a_He + (W_i - W_He)/(W_Xe - W_He)*(a_Xe - a_He);    // UPSTREAM
        a_mix += g.M[i]*a_i/sqrt(W_i);                                   // UPSTREAM
    }
    return a_mix;
}

// ---- fuelRodGapFvPatchScalarField::hGap, gas part -----------------------------
static scalar hGas(const Gas& g, scalar pT, scalar nbrT, scalar gapWidth_, scalar R_, scalar nbrR,
                   scalar interfaceP, scalar gasP, scalar* scale)
{
            scalar T = 0.5*(pT + nbrT);                                  // UPSTREAM
            const scalar k = kappa(g, T);
            scalar pI = interfaceP/1e4/9.8;                              // UPSTREAM
            scalar d_gap = max(gapWidth_, 0);                            // UPSTREAM (gapWidth_[i])
            scalar d_eff = 0;
            d_eff = exp(-1.25e-3*pI)*(R_ + nbrR);                        // UPSTREAM
            scalar d_jump = 0.0137*k*sqrt(T) / (gasP*accommodation(g, T)); // UPSTREAM (gapGas.p(), gapGas.a(T))
            scalar d_gap_total = max(d_gap + d_eff - 1.397e-6, 0.0 );    // UPSTREAM
            scalar h = k / (d_gap_total + 1.8*d_jump);                   // UPSTREAM
            h = 1.0*max(h, 0.0) + 0.0;                                   // (F_hGas = 1, delta_hGas = 0)
    const scalar den = d_gap_total + 1.8 * d_jump;
    *scale = A_(h) * (2.0 + 1.0 + (d_gap + d_eff + 1.397e-6) / den);
    return h;
}

// ---- hRad ----------------------------------------------------------------------
static scalar hRad(scalar T1, scalar T2, scalar eps_i, scalar nbrEps_i, double sigmaValue, scalar* scale)
{
            const scalar eps1 = max(eps_i, SMALL);                       // UPSTREAM
            const scalar eps2 = max(nbrEps_i, SMALL);                    // UPSTREAM
            scalar h = 1.0*(sigmaValue*                                  // (F_hRad.value()*(constant::physicoChemical::sigma.value()*)
                      (T1 + T2)*(T1*T1 + T2*T2)/(1/eps1 + 1/eps2 - 1)) + 0.0;
    *scale = A_(h) * (2.0 + 1.0 / A_(1/eps1 + 1/eps2 - 1) * (1/eps1 + 1/eps2 + 1));
    return h;
}

// ---- hContact ---------------------------------------------------------------------
static scalar hContact(scalar interfaceP_i, scalar k_i, scalar nbrK_i, scalar R_i, scalar nbrR_i, scalar* scale)
{
    scalar hC;
            const scalar meyerHardness = 680e6;                          // UPSTREAM
            scalar Prel = interfaceP_i/meyerHardness;                    // UPSTREAM
            scalar km = 2*k_i*nbrK_i/(k_i + nbrK_i);                     // UPSTREAM
            scalar R = sqrt(sqr(R_i) + sqr(nbrR_i));                     // UPSTREAM
            scalar Rf = 0.5*(R_i + nbrR_i);                              // UPSTREAM
            scalar Rmult;
            if (Prel <= 0.0087)
            {
                Rmult = 333.3*Prel;                                      // UPSTREAM
            }
            else
            {
                Rmult = 2.9 ;                                            // UPSTREAM
            }
            scalar E = exp(5.738 - 0.528*log(3.937e7*Rf));               // UPSTREAM
            if (Prel > 0.003)
            {
                hC = 0.4166*km*Prel*Rmult/R/E;                           // UPSTREAM
            }
            else if (Prel > 9e-6)
            {
                hC = 0.00125*km/R/E;                                     // UPSTREAM
            }
            else
            {
                hC = 0.4166*km*sqrt(Prel)/R/E;                           // UPSTREAM
            }
            hC = 1.0*hC + 0.0;                                           // (F_hContact = 1, delta = 0)
    // E = exp(5.738 - 0.528 ln(3.937e7 Rf)): an argument with |5.738| + |0.528 ln(..)| terms of mixed sign
    *scale = A_(hC) * (3.0 + A_(5.738 - 0.528*log(3.937e7*Rf)) + 0.528 * A_(log(3.937e7*Rf)));
    return hC;
}

static void pr(const char* name, std::initializer_list<scalar> c, scalar scale, scalar v)
{
    std::printf("%s", name);
    for (scalar x : c) std::printf(" %.17g", x);
    std::printf(" %.17g %.17g\n", scale, v);
}

int main()
{
    const scalar mixes[][4] = {{1, 0, 0, 0}, {0, 0, 0, 1}, {0.5, 0, 0, 0.5}, {0.5, 0.25, 0.125, 0.125},
                               {0.75, 0, 0.125, 0.125}, {0.25, 0.25, 0.25, 0.25}, {0, 0.5, 0, 0.5}};
    for (const auto& m : mixes)
    {
        const Gas g = makeGas(m[0], m[1], m[2], m[3]);
        for (scalar T : {300.0, 500.0, 800.0, 1000.0, 1300.0, 1500.0, 2000.0})
        {
            scalar k = kappa(g, T);
            pr("GapKappa", {T, m[0], m[1], m[2], m[3]}, 2.0 * A_(k), k);
            scalar a = accommodation(g, T);
            pr("GapA", {T, m[0], m[1], m[2], m[3]}, 2.0 * A_(a), a);
        }
        for (scalar pT : {500.0, 800.0, 1100.0})
            for (scalar nT : {450.0, 600.0, 700.0})
                for (scalar w : {0.0, 2e-6, 5e-6, 1e-5, 8e-5})
                    for (scalar R : {1e-6, 2e-6})
                        for (scalar P : {0.0, 1e6, 1e7})
                            for (scalar gasP : {1e5, 2e6, 5e6})
                            {
                                scalar sc;
                                scalar v = hGas(g, pT, nT, w, R, 0.5e-6, P, gasP, &sc);
                                pr("GapHGas", {pT, nT, w, R, 0.5e-6, P, gasP, m[0], m[1], m[2], m[3]}, sc, v);
                            }
    }
    for (scalar T1 : {500.0, 800.0, 1100.0, 1500.0})
        for (scalar T2 : {450.0, 600.0, 700.0})
            for (scalar e1 : {0.1, 0.5, 0.85, 1.0})
                for (scalar e2 : {0.3, 0.8, 1.0})
                {
                    scalar sc;
                    scalar v = hRad(T1, T2, e1, e2, sigmaFoam(), &sc);   pr("GapHRad_Foam", {T1, T2, e1, e2}, sc, v);
                    v = hRad(T1, T2, e1, e2, sigma2018, &sc);            pr("GapHRad_2018", {T1, T2, e1, e2}, sc, v);
                }
    for (scalar P : {1e3, 1e5, 2e6, 5e6, 3e7, 6.8e7, 2e8, 6e8})
        for (scalar k1 : {3.0, 5.0})
            for (scalar k2 : {10.0, 20.0})
                for (scalar R1 : {1e-6, 2e-6})
                    for (scalar R2 : {0.5e-6, 1e-6})
                    {
                        scalar sc;
                        scalar v = hContact(P, k1, k2, R1, R2, &sc);
                        pr("GapHContact", {P, k1, k2, R1, R2}, sc, v);
                    }
    return 0;
}
